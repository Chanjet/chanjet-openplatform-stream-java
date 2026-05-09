use super::models::StoreAppTokenResponse;
use super::storage;
use crate::auth::client::HttpSender;
use crate::auth::models::{OAuth2TokenPair, Token};
use crate::auth::pool::TokenPool;
use crate::core::config::Config;
use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};

pub(crate) async fn refresh_token(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
    refresh_token: &str,
) -> Result<Token> {
    let url = format!("{}/oauth2/token", cfg.openapi_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "grant_type": "refresh_token",
        "client_id": cfg.app_key.trim(),
        "client_secret": cfg.app_secret.trim(),
        "refresh_token": refresh_token,
    });

    request_token(pool, http_sender, profile, &url, body, cfg).await
}

pub(crate) async fn intercept_exchange(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
    body_bytes: &[u8],
) -> Result<serde_json::Value> {
    let url = format!("{}/oauth2/token", cfg.openapi_url.trim_end_matches('/'));

    // Parse incoming URL-encoded body
    let mut params: std::collections::HashMap<String, String> =
        serde_urlencoded::from_bytes(body_bytes).unwrap_or_default();

    tracing::info!(target: "sys", "Intercepted OAuth2 exchange request with params: {:?}", params.keys().collect::<Vec<_>>());

    // Inject App Credentials
    params.insert("client_id".to_string(), cfg.app_key.trim().to_string());
    if !cfg.app_secret.trim().is_empty() {
        params.insert(
            "client_secret".to_string(),
            cfg.app_secret.trim().to_string(),
        );
    }

    let body_json = serde_json::to_value(&params)?;

    tracing::info!(target: "sys",
        app_key = %cfg.app_key.trim(),
        redirect_uri = ?params.get("redirect_uri"),
        "Forwarding token exchange to platform"
    );

    // Forward to platform
    let headers = reqwest::header::HeaderMap::new();
    let resp = http_sender.post_form(&url, headers, body_json).await?;

    if !resp.is_success() {
        let masked_body = crate::core::utils::mask_sensitive_json(&resp.body);
        tracing::error!(target: "sys", status = %resp.status, body = %masked_body, "Platform rejected token exchange");
        return Err(anyhow!(
            "Proxy token exchange failed (HTTP {}): {}",
            resp.status,
            masked_body
        ));
    }

    tracing::info!(target: "sys", "Platform token exchange successful: {}", crate::core::utils::mask_sensitive_json(&resp.body));

    let token_resp: StoreAppTokenResponse = serde_json::from_str(&resp.body)?;
    let raw_json: serde_json::Value = serde_json::from_str(&resp.body)?;

    let now = Utc::now();
    let token = Token {
        value: token_resp.access_token.clone(),
        expires_at: now + Duration::seconds(token_resp.expires_in.unwrap_or(7200)),
        created_at: now,
    };

    let pair = OAuth2TokenPair {
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token.unwrap_or_default(),
        expires_at: token.expires_at,
        refresh_expires_at: now
            + Duration::seconds(token_resp.refresh_expires_in.unwrap_or(604800)),
        created_at: now,
    };

    let vault = pool.as_vault();

    // Determine if it's a user token or org token
    if let Some(identity) = token.extract_identity() {
        let app_key = cfg.app_key.trim();

        // Save OAuth2 pair (containing refresh_token) to vault
        let key_pair = if !identity.user_id.is_empty() && identity.user_id != "0" {
            storage::get_user_token_key(app_key, &identity.org_id, &identity.user_id)
        } else {
            storage::get_org_token_key(app_key, &identity.org_id)
        };
        vault.set_secret(profile, &key_pair, &serde_json::to_string(&pair)?).await?;

        if !identity.user_id.is_empty() && identity.user_id != "0" {
            // User-level token path
            let custom_profile = storage::get_custom_profile(profile, app_key, &identity.org_id, Some(&identity.user_id));
            vault.save_access_token(&custom_profile, token.clone()).await?;

            // 🚀 持久化维护“火种”：用户级永久码
            if let Some(upc) = &token_resp.user_permanent_code {
                vault.save_user_permanent_code(app_key, &identity.org_id, &identity.user_id, upc).await?;
            }
        } else {
            // Org-level token path
            if let Some(opc) = &token_resp.org_permanent_code {
                vault.save_org_permanent_code(app_key, &identity.org_id, opc).await?;
            }
            let custom_profile = storage::get_custom_profile(profile, app_key, &identity.org_id, None);
            vault.save_access_token(&custom_profile, token.clone()).await?;
        }
    } else {
        return Err(anyhow!("Failed to extract identity from token during proxy exchange. Multi-tenant arbitration requires a valid JWT."));
    }

    Ok(raw_json)
}

pub(crate) async fn get_app_access_token(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    _profile: &str,
    cfg: &Config,
) -> Result<Token> {
    // 1. 优先尝试从持久化池中获取
    if let Ok(token) = pool.as_vault().get_app_access_token(&cfg.app_key).await {
        // 如果没过期（留出 5 分钟缓冲），直接返回
        if token.expires_at > Utc::now() + Duration::minutes(5) {
            return Ok(token);
        }
    }

    // 2. 如果没有或已过期，则从 Pool 提取动态推送的 Ticket 进行换取
    let mut retry_count = 0;
    let ticket = loop {
        match pool.as_vault().get_app_ticket(cfg.app_key.trim()).await {
            Ok(t) => break t,
            Err(_) => {
                if retry_count >= 20 {
                    return Err(anyhow!("[StoreApp] 尚未接收到平台推送的 appTicket。请确保 daemon 已启动并保持在线。 (Retried 20s)"));
                }

                if retry_count == 0 {
                    tracing::info!(target: "sys", app_key = %cfg.app_key, "AppTicket missing for StoreApp. Proactively triggering a platform push...");
                    let url = format!(
                        "{}/auth/appTicket/resend",
                        cfg.openapi_url.trim_end_matches('/')
                    );
                    let mut headers = reqwest::header::HeaderMap::new();
                    headers.insert("appKey", cfg.app_key.trim().parse()?);
                    headers.insert("appSecret", cfg.app_secret.trim().parse()?);
                    let res = http_sender.post(&url, headers, serde_json::json!({})).await;
                    match res {
                        Ok(resp) => {
                            tracing::info!(target: "sys", status = %resp.status, "Resend request sent successfully")
                        }
                        Err(e) => {
                            tracing::warn!(target: "sys", error = %e, "Failed to send resend request")
                        }
                    }
                }

                retry_count += 1;
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    };

    let url = format!(
        "{}/auth/appAuth/getAppAccessToken",
        cfg.openapi_url.trim_end_matches('/')
    );
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("appKey", cfg.app_key.trim().parse()?);
    headers.insert("appSecret", cfg.app_secret.trim().parse()?);

    let body = serde_json::json!({
        "appTicket": ticket.value
    });

    let resp = http_sender.post(&url, headers, body).await?;
    if !resp.is_success() {
        return Err(anyhow!("Failed to get appAccessToken: {}", crate::core::utils::mask_sensitive_json(&resp.body)));
    }

    let val: serde_json::Value = serde_json::from_str(&resp.body)?;
    let result = val
        .get("result")
        .ok_or_else(|| anyhow!("Invalid response: missing 'result' wrapper"))?;

    let token_val = result
        .get("appAccessToken")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("appAccessToken not found in result: {}", resp.body))?;

    let expire_time = result
        .get("expireTime")
        .and_then(|v| v.as_i64())
        .unwrap_or(7200); // 默认 2 小时

    let token = Token {
        value: token_val.to_string(),
        expires_at: Utc::now() + Duration::seconds(expire_time),
        created_at: Utc::now(),
    };
    
    pool.as_vault().save_app_access_token(cfg.app_key.trim(), token.clone()).await?;
    Ok(token)
}

pub(crate) async fn exchange_permanent_code_by_temp_code(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
    org_id: Option<&str>,
    temp_auth_code: &str,
) -> Result<String> {
    let app_at = get_app_access_token(pool, http_sender, profile, cfg)
        .await?
        .value;
    let url = format!(
        "{}/auth/orgAuth/getPermanentAuthCode",
        cfg.openapi_url.trim_end_matches('/')
    );

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("appKey", cfg.app_key.trim().parse()?);
    headers.insert("appSecret", cfg.app_secret.trim().parse()?);

    let body = serde_json::json!({
        "tempAuthCode": temp_auth_code,
        "appAccessToken": app_at
    });

    let resp = http_sender.post(&url, headers, body).await?;
    if !resp.is_success() {
        return Err(anyhow!("getPermanentAuthCode failed: {}", crate::core::utils::mask_sensitive_json(&resp.body)));
    }

    let val: serde_json::Value = serde_json::from_str(&resp.body)?;

    // Extract permanentAuthCode
    let opc = val
        .get("permanentAuthCode")
        .or_else(|| val.get("result").and_then(|r| r.get("permanentAuthCode")))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("permanentAuthCode not found in response: {}", resp.body))?;

    // 🚀 Robust OrgId Extraction: Prefer the one from the API response
    let final_org_id = val
        .get("orgId")
        .or_else(|| val.get("result").and_then(|r| r.get("orgId")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| org_id.map(|s| s.to_string()))
        .ok_or_else(|| {
            anyhow!(
                "orgId not found in response and not provided in message. Payload: {}",
                resp.body
            )
        })?;

    // 自动归档到 Vault
    pool.as_vault().save_org_permanent_code(cfg.app_key.trim(), &final_org_id, opc).await?;

    tracing::info!(target: "audit", profile = %profile, orgId = %final_org_id, "Enterprise permanent code successfully archived");
    Ok(opc.to_string())
}

pub(crate) async fn get_org_access_token_by_permanent_code(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
    org_id: &str,
    permanent_code: &str,
) -> Result<Token> {
    let app_at = get_app_access_token(pool, http_sender, profile, cfg)
        .await?
        .value;
    let url = format!(
        "{}/auth/orgAuth/getOrgAccessToken",
        cfg.openapi_url.trim_end_matches('/')
    );

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("appKey", cfg.app_key.trim().parse()?);
    headers.insert("appSecret", cfg.app_secret.trim().parse()?);

    let body = serde_json::json!({
        "appAccessToken": app_at,
        "permanentAuthCode": permanent_code
    });

    let resp = http_sender.post(&url, headers, body).await?;
    if !resp.is_success() {
        let masked_body = crate::core::utils::mask_sensitive_json(&resp.body);
        return Err(anyhow!(
            "getOrgAccessToken failed (HTTP {}): {}",
            resp.status,
            masked_body
        ));
    }

    let val: serde_json::Value = serde_json::from_str(&resp.body)?;
    let result = val
        .get("result")
        .ok_or_else(|| anyhow!("Invalid response: missing 'result' wrapper"))?;

    let token_val = result
        .get("accessToken")
        .or_else(|| result.get("orgAccessToken"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("accessToken/orgAccessToken not found in result: {}", resp.body))?;

    let expire_time = result
        .get("expireTime")
        .and_then(|v| v.as_i64())
        .unwrap_or(7200);

    let token = Token {
        value: token_val.to_string(),
        expires_at: Utc::now() + Duration::seconds(expire_time),
        created_at: Utc::now(),
    };

    let custom_profile = storage::get_custom_profile(profile, cfg.app_key.trim(), org_id, None);
    pool.as_vault().save_access_token(&custom_profile, token.clone()).await?;

    Ok(token)
}

pub(crate) async fn get_user_access_token_by_permanent_code(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    cfg: &Config,
    org_id: &str,
    user_id: &str,
    permanent_code: &str,
) -> Result<Token> {
    let app_at = get_app_access_token(pool, http_sender, profile, cfg)
        .await?
        .value;
    let url = format!(
        "{}/auth/userAuth/getUserAccessToken",
        cfg.openapi_url.trim_end_matches('/')
    );

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("appKey", cfg.app_key.trim().parse()?);
    headers.insert("appSecret", cfg.app_secret.trim().parse()?);

    let body = serde_json::json!({
        "appAccessToken": app_at,
        "userPermanentCode": permanent_code
    });

    let resp = http_sender.post(&url, headers, body).await?;
    if !resp.is_success() {
        let masked_body = crate::core::utils::mask_sensitive_json(&resp.body);
        return Err(anyhow!(
            "getUserAccessToken failed (HTTP {}): {}",
            resp.status,
            masked_body
        ));
    }

    let val: serde_json::Value = serde_json::from_str(&resp.body)?;
    let result = val
        .get("result")
        .ok_or_else(|| anyhow!("Invalid response: missing 'result' wrapper"))?;

    let token_val = result
        .get("accessToken")
        .or_else(|| result.get("userAccessToken"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("accessToken/userAccessToken not found in result: {}", resp.body))?;

    let expire_time = result
        .get("expireTime")
        .and_then(|v| v.as_i64())
        .unwrap_or(7200);

    let token = Token {
        value: token_val.to_string(),
        expires_at: Utc::now() + Duration::seconds(expire_time),
        created_at: Utc::now(),
    };

    let custom_profile = storage::get_custom_profile(profile, cfg.app_key.trim(), org_id, Some(user_id));
    pool.as_vault().save_access_token(&custom_profile, token.clone()).await?;

    Ok(token)
}

pub(crate) async fn request_token(
    pool: &(dyn TokenPool + Send + Sync),
    http_sender: &dyn HttpSender,
    profile: &str,
    url: &str,
    body: serde_json::Value,
    cfg: &Config,
) -> Result<Token> {
    let headers = reqwest::header::HeaderMap::new();
    let resp = http_sender.post_form(url, headers, body).await?;

    if !resp.is_success() {
        let status = resp.status;
        let err_text = crate::core::utils::mask_sensitive_json(&resp.text());

        tracing::error!(
            target: "audit",
            profile = %profile,
            event = "token_rotate",
            status = "failure",
            error = %err_text,
            "StoreApp token rotation failed"
        );

        // Handle specific platform error codes
        if err_text.contains("4029") {
            return Err(anyhow!(
                "登录会话已超时（7天），请执行 `owenc init` 重新授权。 (Error: {})",
                status
            ));
        }
        if err_text.contains("4007") || err_text.contains("invalid_grant") {
            let _ = pool.as_vault().set_config(profile, "oauth2_revoked", "true").await;
            return Err(anyhow!(
                "令牌已失效（可能已被吊销），请执行 `owenc auth login` 重新授权。 (Error: {})",
                status
            ));
        }
        if err_text.contains("4006") {
            return Err(anyhow!(
                "ClientID 与令牌颁发者不一致，请检查配置。 (Error: {})",
                status
            ));
        }
        if err_text.contains("4001") {
            return Err(anyhow!(
                "授权校验失败 (PKCE)，请重新执行 `owenc init`。 (Error: {})",
                status
            ));
        }

        return Err(anyhow!(
            "StoreApp token request failed (HTTP {}): {}",
            status,
            err_text
        ));
    }

    let token_resp: StoreAppTokenResponse = resp.json().await?;
    let now = Utc::now();

    let token = Token {
        value: token_resp.access_token.clone(),
        expires_at: now + Duration::seconds(token_resp.expires_in.unwrap_or(7200)),
        created_at: now,
    };

    let pair = OAuth2TokenPair {
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token.unwrap_or_default(),
        expires_at: token.expires_at,
        refresh_expires_at: now
            + Duration::seconds(token_resp.refresh_expires_in.unwrap_or(604800)),
        created_at: now,
    };

    // Save permanent codes if present
    if let Some(identity) = token.extract_identity() {
        let app_key = cfg.app_key.trim();
        if let Some(upc) = token_resp.user_permanent_code {
            pool.as_vault().save_user_permanent_code(app_key, &identity.org_id, &identity.user_id, &upc).await?;
        }
        if let Some(opc) = token_resp.org_permanent_code {
            pool.as_vault().save_org_permanent_code(app_key, &identity.org_id, &opc).await?;
        }

        // Save to vault via pool using multi-tenant keys
        let key_pair = if !identity.user_id.is_empty() && identity.user_id != "0" {
            storage::get_user_token_key(app_key, &identity.org_id, &identity.user_id)
        } else {
            storage::get_org_token_key(app_key, &identity.org_id)
        };

        pool.as_vault()
            .set_secret(profile, &key_pair, &serde_json::to_string(&pair)?)
            .await?;

        let _ = pool.as_vault().delete_config(profile, "oauth2_revoked").await;

        let custom_profile = if !identity.user_id.is_empty() && identity.user_id != "0" {
            storage::get_custom_profile(profile, app_key, &identity.org_id, Some(&identity.user_id))
        } else {
            storage::get_custom_profile(profile, app_key, &identity.org_id, None)
        };
        pool.set_access_token(&custom_profile, &token).await?;
    }

    Ok(token)
}
