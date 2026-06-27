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
            get_app_access_token_fallback(pool, http_sender, profile, cfg).await
        }
    }
}

async fn get_app_access_token_fallback(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
) -> CowenResult<cowen_common::models::Token> {
    match pool.get_app_access_token(&cfg.app_key).await {
        Ok(t) if !t.is_expired() => Ok(t),
        _ => match pool.as_vault().get_app_access_token(&cfg.app_key).await {
            Ok(t) if !t.is_expired() => {
                pool.set_app_access_token(&cfg.app_key, &t).await?;
                Ok(t)
            }
            _ => handle_concurrent_app_token_fetch(pool, http_sender, profile, cfg).await,
        },
    }
}

async fn handle_concurrent_app_token_fetch(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
) -> CowenResult<cowen_common::models::Token> {
    let store = pool.as_vault().primary_store();
    let lock_key = format!("lock:app_access:{}", cfg.app_key);
    let now = chrono::Utc::now().timestamp();

    if let Ok(expire_val) = store.get_token(profile, &lock_key).await {
        if let Ok(expire_ts) = expire_val.parse::<i64>() {
            if now < expire_ts {
                tracing::info!(target: "sys", profile = %profile, app_key = %cfg.app_key, "AppAccessToken fetch is locked by a concurrent task. Waiting...");
                for _ in 0..15 {
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                    if let Ok(t) = pool.as_vault().get_app_access_token(&cfg.app_key).await {
                        if !t.is_expired() {
                            pool.set_app_access_token(&cfg.app_key, &t).await?;
                            return Ok(t);
                        }
                    }
                }
            }
        }
    }

    let new_expire_ts = now + 10;
    let _ = store
        .set_token(profile, &lock_key, &new_expire_ts.to_string(), 10)
        .await;

    match client::get_app_access_token(pool, http_sender, profile, cfg).await {
        Ok(t) => {
            pool.set_app_access_token(&cfg.app_key, &t).await?;
            let _ = store.delete_token(profile, &lock_key).await;
            Ok(t)
        }
        Err(err) => {
            let _ = store.delete_token(profile, &lock_key).await;
            tracing::error!(target: "sys", error = %err, "Proactive AppAccessToken exchange failed");
            Err(CowenError::Auth("401 Unauthorized: Missing mandatory multi-tenant arbitration headers (x-org-id is required).".to_string()))
        }
    }
}

async fn check_token_cache(
    pool: &(dyn TokenPool + Send + Sync),
    custom_profile: &str,
) -> CowenResult<Option<cowen_common::models::Token>> {
    if let Ok(token) = pool.get_access_token(custom_profile).await {
        if !token.is_expired() {
            return Ok(Some(token));
        }
    }

    if let Ok(token) = pool.as_vault().get_access_token(custom_profile).await {
        if !token.is_expired() {
            pool.set_access_token(custom_profile, &token).await?;
            return Ok(Some(token));
        }
    }

    Ok(None)
}

async fn try_refresh_token_recovery(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
    secret_key: &str,
) -> Option<cowen_common::models::Token> {
    if let Ok(pair_str) = pool.as_vault().get_secret(profile, secret_key).await {
        if let Ok(pair) = serde_json::from_str::<cowen_common::models::OAuth2TokenPair>(&pair_str) {
            if !pair.refresh_token.is_empty() && pair.refresh_expires_at > chrono::Utc::now() {
                if let Ok(new_token) =
                    client::refresh_token(pool, http_sender, profile, cfg, &pair.refresh_token)
                        .await
                {
                    return Some(new_token);
                }
            }
        }
    }
    None
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

    let custom_profile = storage::get_custom_profile(profile, app_key, org_id, Some(uid));
    if let Ok(Some(token)) = check_token_cache(pool, &custom_profile).await {
        return Ok(token);
    }

    // 🚀 2. Fallback to Refresh Token (from Vault secret)
    tracing::info!(target: "sys", userId = %uid, orgId = %org_id, "Attempting recovery via Refresh Token...");
    let secret_key = storage::get_user_token_key(app_key, org_id, uid);
    if let Some(new_token) =
        try_refresh_token_recovery(pool, http_sender, profile, cfg, &secret_key).await
    {
        tracing::info!(target: "audit", userId = %uid, "Successfully recovered user token via Refresh Token");
        return Ok(new_token);
    }

    // 🚀 3. Fallback to User Permanent Auth Code ("Fire Seed")
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

    let custom_profile = storage::get_custom_profile(profile, app_key, org_id, None);
    if let Ok(Some(token)) = check_token_cache(pool, &custom_profile).await {
        return Ok(token);
    }

    // 🚀 2. Fallback to Refresh Token (from Vault secret)
    tracing::info!(target: "sys", orgId = %org_id, "Attempting recovery via Refresh Token...");
    let secret_key = storage::get_org_token_key(app_key, org_id);
    if let Some(new_token) =
        try_refresh_token_recovery(pool, http_sender, profile, cfg, &secret_key).await
    {
        tracing::info!(target: "audit", orgId = %org_id, "Successfully recovered org token via Refresh Token");
        return Ok(new_token);
    }

    // 🚀 3. Permanent code recovery
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
        let upc_res = vault.get_user_permanent_code(app_key, org_id, uid).await;
        if let Err(ref e) = upc_res {
            tracing::error!(target: "sys", "UPC GET ERROR: {}", e);
            println!(
                "!!!!! UPC GET ERROR: {} (app_key='{}', org_id='{}', uid='{}')",
                e, app_key, org_id, uid
            );
        }
        let opc_res = vault.get_org_permanent_code(app_key, org_id).await;
        if let Err(ref e) = opc_res {
            tracing::error!(target: "sys", "OPC GET ERROR: {}", e);
        }
        (
            upc_res.ok(),
            opc_res.ok(),
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
    } else if let Some(code) = opc {
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
