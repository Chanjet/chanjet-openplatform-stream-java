use crate::auth::models::{Token, OAuth2TokenPair};
use crate::core::config::Config;
use anyhow::{Result, anyhow};
use chrono::Utc;
use crate::auth::pool::TokenPool;
use crate::auth::client::HttpSender;
use super::{client, storage};

pub(crate) async fn get_token(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
    headers: &reqwest::header::HeaderMap,
) -> Result<Token> {
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
            // Reject requests missing mandatory arbitration headers
            Err(anyhow!("401 Unauthorized: Missing mandatory multi-tenant arbitration headers (x-org-id is required)."))
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
) -> Result<Token> {
    let uid = user_id.trim();
    let app_key = cfg.app_key.trim();

    // 1. Check local cache (memory/fast vault)
    let custom_profile = storage::get_custom_profile(profile, app_key, org_id, Some(uid));
    if let Ok(token) = pool.get_access_token(&custom_profile).await {
        if !token.is_expired() {
            return Ok(token);
        }
    }

    // 2. Slow path via Vault pair
    let key_pair = storage::get_user_token_key(app_key, org_id, uid);
    let pair_str = pool
        .as_vault()
        .get(profile, &key_pair)
        .await
        .map_err(|_| {
            anyhow!(
                "User token not found for userId: {} in org: {}",
                uid,
                org_id
            )
        })?;

    let pair: OAuth2TokenPair = serde_json::from_str(&pair_str)?;

    // If AccessToken is still valid, use it
    if Utc::now() < pair.expires_at {
        let token = Token {
            value: pair.access_token.clone(),
            expires_at: pair.expires_at,
            created_at: pair.created_at,
        };
        pool.set_access_token(&custom_profile, &token).await?;
        return Ok(token);
    }

    // 3. Refresh logic for user token
    let refresh_result = if Utc::now() < pair.refresh_expires_at {
            client::refresh_token(pool, http_sender, profile, cfg, &pair.refresh_token).await
    } else {
        Err(anyhow!("User refresh token expired"))
    };

    match refresh_result {
        Ok(t) => {
            // Success via Refresh Token
            let new_pair = OAuth2TokenPair {
                access_token: t.value.clone(),
                refresh_token: pair.refresh_token.clone(), 
                expires_at: t.expires_at,
                refresh_expires_at: pair.refresh_expires_at,
                created_at: Utc::now(),
            };
            pool
                .as_vault()
                .set(profile, &key_pair, &serde_json::to_string(&new_pair)?)
                .await?;
            pool.set_access_token(&custom_profile, &t).await?;
            Ok(t)
        }
        Err(e) => {
            // 🚀 Fallback to User Permanent Auth Code ("Fire Seed")
            tracing::info!(target: "sys", userId = %uid, orgId = %org_id, "Refresh token failed or expired. Attempting recovery via Permanent Auth Code...");
            
            match try_permanent_code_recovery(pool, http_sender, profile, cfg, org_id, Some(uid)).await {
                Ok(t) => {
                    pool.set_access_token(&custom_profile, &t).await?;
                    tracing::info!(target: "audit", userId = %uid, "Successfully recovered user token via Permanent Auth Code");
                    Ok(t)
                }
                Err(err) => {
                    tracing::error!(target: "sys", userId = %uid, "Recovery via Permanent Auth Code failed: {}", err);
                    Err(anyhow!("User session expired and recovery failed: {}", e))
                }
            }
        }
    }
}

pub(crate) async fn get_org_token(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
    org_id: &str,
) -> Result<Token> {
    let app_key = cfg.app_key.trim();
    let org_id = org_id.trim();

    // 1. Check local cache
    let custom_profile = storage::get_custom_profile(profile, app_key, org_id, None);
    if let Ok(token) = pool.get_access_token(&custom_profile).await {
        if !token.is_expired() {
            return Ok(token);
        }
    }

    // 2. Slow path via Vault pair
    let key_pair = storage::get_org_token_key(app_key, org_id);
    let pair_str = pool
        .as_vault()
        .get(profile, &key_pair)
        .await
        .map_err(|_| anyhow!("Organization token not found for orgId: {}", org_id))?;

    let pair: OAuth2TokenPair = serde_json::from_str(&pair_str)?;

    if Utc::now() < pair.expires_at {
        let token = Token {
            value: pair.access_token.clone(),
            expires_at: pair.expires_at,
            created_at: pair.created_at,
        };
        pool.set_access_token(&custom_profile, &token).await?;
        return Ok(token);
    }

    // 3. Refresh logic
    if Utc::now() < pair.refresh_expires_at {
        match client::refresh_token(pool, http_sender, profile, cfg, &pair.refresh_token).await {
            Ok(t) => {
                pool.set_access_token(&custom_profile, &t).await?;
                return Ok(t);
            }
            Err(e) => {
                tracing::warn!(target: "sys", orgId = %org_id, error = %e, "Org refresh token failed. Attempting recovery...");
            }
        }
    }

    // 4. Permanent code recovery
    match try_permanent_code_recovery(pool, http_sender, profile, cfg, org_id, None).await {
        Ok(t) => {
            pool.set_access_token(&custom_profile, &t).await?;
            Ok(t)
        }
        Err(e) => Err(anyhow!("Organization session expired and recovery failed for org {}: {}", org_id, e))
    }
}

pub(crate) async fn try_permanent_code_recovery(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
    org_id: &str,
    user_id: Option<&str>
) -> Result<Token> {
    let vault = pool.as_vault();
    let app_key = cfg.app_key.trim();
    
    let (upc, opc, target_name) = if let Some(uid) = user_id {
        let upc_key = storage::get_user_upc_key(app_key, org_id, uid);
        let opc_key = storage::get_org_opc_key(app_key, org_id);
        (
            vault.get(profile, &upc_key).await.ok(),
            vault.get(profile, &opc_key).await.ok(),
            format!("user {} in org {}", uid, org_id)
        )
    } else {
        let opc_key = storage::get_org_opc_key(app_key, org_id);
        (
            None,
            vault.get(profile, &opc_key).await.ok(),
            format!("org {}", org_id)
        )
    };

    if upc.is_none() && opc.is_none() {
        return Err(anyhow!("No permanent codes found for recovery of {}", target_name));
    }

    tracing::info!(target: "sys", profile = %profile, target = %target_name, "Triggering permanent code exchange for store app auth recovery");

    let url = format!(
        "{}/oauth2/token",
        cfg.openapi_url.trim_end_matches('/')
    );

    let mut body = serde_json::json!({
        "grant_type": "permanent_code",
        "client_id": app_key,
        "client_secret": cfg.app_secret.trim(),
    });

    if let Some(u) = upc {
        body.as_object_mut().unwrap().insert("user_permanent_code".to_string(), serde_json::json!(u));
    }
    if let Some(o) = opc {
        body.as_object_mut().unwrap().insert("org_permanent_code".to_string(), serde_json::json!(o));
    }

    client::request_token(pool, http_sender, profile, &url, body, cfg).await
}
