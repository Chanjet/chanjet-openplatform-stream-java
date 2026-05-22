use cowen_common::{CowenError, CowenResult};

use super::{client, storage};
use crate::client::HttpSender;
use crate::pool::TokenPool;
use cowen_common::config::Config;

pub(crate) async fn get_token(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
    headers: &reqwest::header::HeaderMap,
) -> CowenResult<cowen_common::models::Token> {
    let org_id = headers.get("x-org-id").and_then(|v| v.to_str().ok());
    let user_id = headers.get("x-user-id").and_then(|v| v.to_str().ok());

    // 🚀 Multi-tenant Arbitration Enforcement
    match (org_id, user_id) {
        (Some(oid), Some(uid)) if !oid.trim().is_empty() && !uid.trim().is_empty() => {
            get_user_token(pool, http_sender, profile, cfg, oid, uid).await
        }
        (Some(oid), _) if !oid.trim().is_empty() => {
            get_org_token(pool, http_sender, profile, cfg, oid).await
        }
        _ => {
            // 🚀 OCP: Fallback to AppAccessToken if no arbitration headers are provided.
            // This allows CLI commands to "see" the app-level token for status checks.
            match pool.get_app_access_token(&cfg.app_key).await {
                Ok(t) if !t.is_expired() => Ok(t),
                _ => {
                    // Slow path via Vault
                    match pool.as_vault().get_app_access_token(&cfg.app_key).await {
                        Ok(t) if !t.is_expired() => {
                            pool.set_app_access_token(&cfg.app_key, &t).await?;
                            Ok(t)
                        }
                        _ => {
                            // 🚀 Slow path recovery: Proactively exchange the AppAccessToken using the appTicket in the Vault.
                            // This guarantees that any CLI status checks or secondary nodes can successfully recover when
                            // the AppAccessToken is missing but the AppTicket is present in the shared database.
                            match client::get_app_access_token(pool, http_sender, profile, cfg).await {
                                Ok(t) => {
                                    pool.set_app_access_token(&cfg.app_key, &t).await?;
                                    Ok(t)
                                }
                                Err(err) => {
                                    tracing::error!(target: "sys", error = %err, "Proactive AppAccessToken exchange failed");
                                    Err(CowenError::Auth(format!("401 Unauthorized: Missing mandatory multi-tenant arbitration headers (x-org-id is required).")))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(crate) async fn get_user_token(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
    org_id: &str,
    user_id: &str,
) -> CowenResult<cowen_common::models::Token> {
    let uid = user_id.trim();
    let app_key = cfg.app_key.trim();

    // 1. Check local cache (memory/fast vault)
    let custom_profile = storage::get_custom_profile(profile, app_key, org_id, Some(uid));
    if let Ok(token) = pool.get_access_token(&custom_profile).await {
        if !token.is_expired() {
            return Ok(token);
        }
    }

    // 2. Slow path via Vault
    let custom_profile = storage::get_custom_profile(profile, app_key, org_id, Some(uid));
    if let Ok(token) = pool.as_vault().get_access_token(&custom_profile).await {
        if !token.is_expired() {
            pool.set_access_token(&custom_profile, &token).await?;
            return Ok(token);
        }
    }

    // 🚀 Fallback to User Permanent Auth Code ("Fire Seed")
    tracing::info!(target: "sys", userId = %uid, orgId = %org_id, "Attempting recovery via Permanent Auth Code...");

    match try_permanent_code_recovery(pool, http_sender, profile, cfg, org_id, Some(uid)).await {
        Ok(t) => {
            // Token is already saved to vault inside recovery function via save_access_token
            pool.set_access_token(&custom_profile, &t).await?;
            tracing::info!(target: "audit", userId = %uid, "Successfully recovered user token via Permanent Auth Code");
            Ok(t)
        }
        Err(err) => {
            tracing::error!(target: "sys", userId = %uid, "Recovery via Permanent Auth Code failed: {}", err);
            Err(CowenError::Auth(format!(
                "User session expired and recovery failed: {}",
                err
            )))
        }
    }
}

pub(crate) async fn get_org_token(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
    org_id: &str,
) -> CowenResult<cowen_common::models::Token> {
    let app_key = cfg.app_key.trim();
    let org_id = org_id.trim();

    // 1. Check local cache
    let custom_profile = storage::get_custom_profile(profile, app_key, org_id, None);
    if let Ok(token) = pool.get_access_token(&custom_profile).await {
        if !token.is_expired() {
            return Ok(token);
        }
    }

    // 2. Slow path via Vault
    let custom_profile = storage::get_custom_profile(profile, app_key, org_id, None);
    if let Ok(token) = pool.as_vault().get_access_token(&custom_profile).await {
        if !token.is_expired() {
            pool.set_access_token(&custom_profile, &token).await?;
            return Ok(token);
        }
    }

    // 4. Permanent code recovery

    match try_permanent_code_recovery(pool, http_sender, profile, cfg, org_id, None).await {
        Ok(t) => {
            pool.set_access_token(&custom_profile, &t).await?;
            Ok(t)
        }
        Err(e) => Err(CowenError::Auth(format!(
            "Organization session expired and recovery failed for org {}: {}",
            org_id, e
        ))),
    }
}

pub(crate) async fn try_permanent_code_recovery(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
    org_id: &str,
    user_id: Option<&str>,
) -> CowenResult<cowen_common::models::Token> {
    let vault = pool.as_vault();
    let app_key = cfg.app_key.trim();

    let (upc, opc, target_name) = if let Some(uid) = user_id {
        (
            vault
                .get_user_permanent_code(app_key, org_id, uid)
                .await
                .ok(),
            vault.get_org_permanent_code(app_key, org_id).await.ok(),
            format!("user {} in org {}", uid, org_id),
        )
    } else {
        (
            None,
            vault.get_org_permanent_code(app_key, org_id).await.ok(),
            format!("org {}", org_id),
        )
    };

    if upc.is_none() && opc.is_none() {
        return Err(CowenError::Auth(format!(
            "No permanent codes found for recovery of {}",
            target_name
        )));
    }

    tracing::info!(target: "sys", profile = %profile, target = %target_name, "Triggering permanent code exchange for store app auth recovery");

    if let Some(uid) = user_id {
        if let Some(code) = upc {
            client::get_user_access_token_by_permanent_code(
                pool,
                http_sender,
                profile,
                cfg,
                org_id,
                uid,
                &code,
            )
            .await
        } else {
            Err(CowenError::Auth(format!(
                "User permanent code missing for recovery of {}",
                target_name
            )))
        }
    } else {
        if let Some(code) = opc {
            client::get_org_access_token_by_permanent_code(
                pool,
                http_sender,
                profile,
                cfg,
                org_id,
                &code,
            )
            .await
        } else {
            Err(CowenError::Auth(format!(
                "Org permanent code missing for recovery of {}",
                target_name
            )))
        }
    }
}
