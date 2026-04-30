use crate::auth::client::HttpSender;
use crate::auth::lifecycle::AuthSessionManager;
use crate::auth::models::{OAuth2TokenPair, Token};
use crate::auth::pool::TokenPool;
use crate::auth::provider::AuthProvider;
use crate::core::config::Config;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use fs2::FileExt;
use serde::Deserialize;
use std::fs::File;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct StoreAppTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    #[serde(alias = "refresh_token_expires_in")]
    refresh_expires_in: Option<i64>,
    #[serde(alias = "user_auth_permanent_code")]
    user_permanent_code: Option<String>,
    #[serde(alias = "permanent_auth_code")]
    org_permanent_code: Option<String>,
}

pub struct StoreAppProvider<'a> {
    pool: &'a (dyn TokenPool + Send + Sync),
    http_sender: Arc<dyn HttpSender>,
}

impl<'a> StoreAppProvider<'a> {
    pub fn new(pool: &'a (dyn TokenPool + Send + Sync), http_sender: Arc<dyn HttpSender>) -> Self {
        Self { pool, http_sender }
    }

    fn get_user_token_key(&self, app_key: &str, org_id: &str, user_id: &str) -> String {
        format!("oauth2_token_pair_user_{}_{}_{}", app_key, org_id, user_id)
    }

    fn get_org_token_key(&self, app_key: &str, org_id: &str) -> String {
        format!("oauth2_token_pair_org_{}_{}", app_key, org_id)
    }

    fn get_user_upc_key(&self, app_key: &str, org_id: &str, user_id: &str) -> String {
        format!("user_permanent_code_{}_{}_{}", app_key, org_id, user_id)
    }

    fn get_org_opc_key(&self, app_key: &str, org_id: &str) -> String {
        format!("org_permanent_code_{}_{}", app_key, org_id)
    }

    fn get_custom_profile(&self, base_profile: &str, app_key: &str, org_id: &str, user_id: Option<&str>) -> String {
        if let Some(uid) = user_id {
            format!("{}:{}:{}:{}", base_profile, app_key, org_id, uid)
        } else {
            format!("{}:{}:{}", base_profile, app_key, org_id)
        }
    }


    async fn refresh_token(
        &self,
        profile: &str,
        cfg: &Config,
        refresh_token: &str,
    ) -> Result<Token> {
        let url = format!(
            "{}{}",
            cfg.openapi_url.trim_end_matches('/'),
            obfs!("/oauth2/token")
        );
        let body = serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": cfg.app_key.trim(),
            "client_secret": cfg.app_secret.trim(),
            "refresh_token": refresh_token,
        });

        self.request_token(profile, &url, body, cfg).await
    }

    pub async fn intercept_exchange(
        &self,
        profile: &str,
        cfg: &Config,
        body_bytes: &[u8],
    ) -> Result<serde_json::Value> {
        let url = format!(
            "{}{}",
            cfg.openapi_url.trim_end_matches('/'),
            obfs!("/oauth2/token")
        );

        // Parse incoming URL-encoded body
        let mut params: std::collections::HashMap<String, String> =
            serde_urlencoded::from_bytes(body_bytes).unwrap_or_default();

        // Inject App Credentials
        params.insert("client_id".to_string(), cfg.app_key.trim().to_string());
        if !cfg.app_secret.trim().is_empty() {
            params.insert(
                "client_secret".to_string(),
                cfg.app_secret.trim().to_string(),
            );
        }

        let body_json = serde_json::to_value(&params)?;

        // Forward to platform
        let headers = reqwest::header::HeaderMap::new();
        let resp = self.http_sender.post_form(&url, headers, body_json).await?;

        if !resp.is_success() {
            return Err(anyhow!(
                "Proxy token exchange failed (HTTP {}): {}",
                resp.status,
                resp.body
            ));
        }

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

        let vault = self.pool.as_vault();

        // Determine if it's a user token or org token
        if let Some(identity) = token.extract_identity() {
            let app_key = cfg.app_key.trim();
            
            if !identity.user_id.is_empty() && identity.user_id != "0" {
                // User-level token path
                let key_pair = self.get_user_token_key(app_key, &identity.org_id, &identity.user_id);
                vault
                    .set(profile, &key_pair, &serde_json::to_string(&pair)?)
                    .await?;

                // 🚀 持久化维护“火种”：用户级永久码 (以三元组为索引)
                if let Some(upc) = &token_resp.user_permanent_code {
                    let upc_key = self.get_user_upc_key(app_key, &identity.org_id, &identity.user_id);
                    vault.set(profile, &upc_key, upc).await?;
                }

                // Temporarily swap access token in pool for user context
                let mut fake_pool_token = token.clone();
                let custom_profile = self.get_custom_profile(profile, app_key, &identity.org_id, Some(&identity.user_id));
                let _ = self
                    .pool
                    .set_access_token(&custom_profile, &fake_pool_token)
                    .await;
            } else {
                // Org-level token path
                if let Some(opc) = &token_resp.org_permanent_code {
                    let opc_key = self.get_org_opc_key(app_key, &identity.org_id);
                    vault.set(profile, &opc_key, opc).await?;
                }
                // Org-level token
                let key_pair = self.get_org_token_key(app_key, &identity.org_id);
                vault
                    .set(profile, &key_pair, &serde_json::to_string(&pair)?)
                    .await?;
                
                let custom_profile = self.get_custom_profile(profile, app_key, &identity.org_id, None);
                self.pool.set_access_token(&custom_profile, &token).await?;
            }
        } else {
            return Err(anyhow!("Failed to extract identity from token during proxy exchange. Multi-tenant arbitration requires a valid JWT."));
        }

        Ok(raw_json)
    }


    /// 🚀 步骤 A：获取应用访问凭证 (SuiteAccessToken)
    /// 这里的 appTicket 是由 Stream 桥接器在后台接收并存入 Pool 的
    pub async fn get_app_access_token(&self, _profile: &str, cfg: &Config) -> Result<Token> {
        // 1. 优先尝试从持久化池中获取
        if let Ok(token) = self.pool.get_app_access_token(&cfg.app_key).await {
            // 如果没过期（留出 5 分钟缓冲），直接返回
            if token.expires_at > chrono::Utc::now() + chrono::Duration::minutes(5) {
                return Ok(token);
            }
        }

        // 2. 如果没有或已过期，则从 Pool 提取动态推送的 Ticket 进行换取
        let ticket = self.pool.get_app_ticket(cfg.app_key.trim()).await
            .map_err(|_| anyhow!("[StoreApp] 尚未接收到平台推送的 appTicket。请确保 daemon 已启动并保持在线。"))?;

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

        let resp = self.http_sender.post(&url, headers, body).await?;
        if !resp.is_success() {
            return Err(anyhow!("Failed to get appAccessToken: {}", resp.body));
        }

        let val: serde_json::Value = serde_json::from_str(&resp.body)?;
        let result = val.get("result").ok_or_else(|| anyhow!("Invalid response: missing 'result' wrapper"))?;
        
        let token_val = result.get("appAccessToken")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("appAccessToken not found in result: {}", resp.body))?;

        let expire_time = result.get("expireTime")
            .and_then(|v| v.as_i64())
            .unwrap_or(7200); // 默认 2 小时

        Ok(Token {
            value: token_val.to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(expire_time),
            created_at: chrono::Utc::now(),
        })
    }

    /// 🚀 步骤 B：换取企业永久授权码 (对齐用户提供的 curl 与响应结构)
    pub async fn exchange_permanent_code_by_temp_code(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
        temp_auth_code: &str,
    ) -> Result<String> {
        let app_at = self.get_app_access_token(profile, cfg).await?.value;
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

        let resp = self.http_sender.post(&url, headers, body).await?;
        if !resp.is_success() {
            return Err(anyhow!("getPermanentAuthCode failed: {}", resp.body));
        }

        let val: serde_json::Value = serde_json::from_str(&resp.body)?;
        let opc = val
            .get("permanentAuthCode")
            .or_else(|| val.get("result").and_then(|r| r.get("permanentAuthCode")))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("permanentAuthCode not found in response: {}", resp.body))?;

        // 自动归档到 Vault (映射回内部使用的 key: org_permanent_code)
        let opc_key = self.get_org_opc_key(cfg.app_key.trim(), org_id);
        self.pool
            .as_vault()
            .set(profile, &opc_key, opc)
            .await?;
        
        tracing::info!(target: "audit", profile = %profile, orgId = %org_id, "Enterprise permanent code successfully archived");
        Ok(opc.to_string())
    }

    async fn try_permanent_code_recovery(&self, profile: &str, cfg: &Config, org_id: &str, user_id: Option<&str>) -> Result<Token> {
        let vault = self.pool.as_vault();
        let app_key = cfg.app_key.trim();
        
        let (upc, opc, target_name) = if let Some(uid) = user_id {
            let upc_key = self.get_user_upc_key(app_key, org_id, uid);
            let opc_key = self.get_org_opc_key(app_key, org_id);
            (
                Some(vault.get(profile, &upc_key).await?),
                Some(vault.get(profile, &opc_key).await?),
                format!("user {} in org {}", uid, org_id)
            )
        } else {
            let opc_key = self.get_org_opc_key(app_key, org_id);
            (
                None,
                Some(vault.get(profile, &opc_key).await?),
                format!("org {}", org_id)
            )
        };

        tracing::info!(target: "sys", profile = %profile, target = %target_name, "Triggering permanent code exchange for store app auth recovery");

        let url = format!(
            "{}{}",
            cfg.openapi_url.trim_end_matches('/'),
            obfs!("/oauth2/token")
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

        self.request_token(profile, &url, body, cfg).await
    }

    async fn request_token(
        &self,
        profile: &str,
        url: &str,
        body: serde_json::Value,
        cfg: &Config,
    ) -> Result<Token> {
        let headers = reqwest::header::HeaderMap::new();
        let resp = self.http_sender.post_form(url, headers, body).await?;

        if !resp.is_success() {
            let status = resp.status;
            let err_text = resp.text();

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
                let _ = self
                    .pool
                    .as_vault()
                    .set(profile, "oauth2_revoked", "true")
                    .await;
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
                let upc_key = self.get_user_upc_key(app_key, &identity.org_id, &identity.user_id);
                self.pool.as_vault().set(profile, &upc_key, &upc).await?;
            }
            if let Some(opc) = token_resp.org_permanent_code {
                let opc_key = self.get_org_opc_key(app_key, &identity.org_id);
                self.pool.as_vault().set(profile, &opc_key, &opc).await?;
            }

            // Save to vault via pool using multi-tenant keys
            let key_pair = if !identity.user_id.is_empty() && identity.user_id != "0" {
                self.get_user_token_key(app_key, &identity.org_id, &identity.user_id)
            } else {
                self.get_org_token_key(app_key, &identity.org_id)
            };

            self.pool
                .as_vault()
                .set(profile, &key_pair, &serde_json::to_string(&pair)?)
                .await?;
            
            let _ = self.pool.as_vault().delete(profile, "oauth2_revoked").await;
            
            let custom_profile = if !identity.user_id.is_empty() && identity.user_id != "0" {
                self.get_custom_profile(profile, app_key, &identity.org_id, Some(&identity.user_id))
            } else {
                self.get_custom_profile(profile, app_key, &identity.org_id, None)
            };
            self.pool.set_access_token(&custom_profile, &token).await?;
        }

        // Deduplication Logic (User/Org/App/AppKey uniqueness)
        if let Some(new_id) = token.extract_identity() {
            if let Ok(cfg_mgr) = crate::core::config::ConfigManager::new() {
                if let Ok(profiles) = cfg_mgr.list_profiles().await {
                    for other_profile in profiles {
                        if other_profile == profile {
                            continue;
                        }

                        // Check if this profile has a matching identity
                        if let Ok(other_pair_raw) = self
                            .pool
                            .as_vault()
                            .get(&other_profile, "oauth2_token_pair")
                            .await
                        {
                            if let Ok(other_pair) =
                                serde_json::from_str::<OAuth2TokenPair>(&other_pair_raw)
                            {
                                let other_token = Token {
                                    value: other_pair.access_token,
                                    expires_at: other_pair.expires_at,
                                    created_at: other_pair.created_at,
                                };
                                if let Some(other_id) = other_token.extract_identity() {
                                    if other_id == new_id {
                                        // Potential duplicate found. Verify AppKey match as well.
                                        if let Ok(other_cfg) = cfg_mgr.load(&other_profile).await {
                                            if other_cfg.app_key == cfg.app_key {
                                                tracing::warn!(target: "audit", profile = %profile, duplicate = %other_profile, "Deduplication: Invalidating historical profile with same identity");
                                                let _ = self
                                                    .pool
                                                    .as_vault()
                                                    .set(&other_profile, "oauth2_revoked", "true")
                                                    .await;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        tracing::info!(
            target: "audit",
            profile = %profile,
            event = "token_rotate",
            status = "success",
            "StoreApp token pair successfully rotated"
        );

        Ok(token)
    }

    pub async fn get_user_token(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
        user_id: &str,
    ) -> Result<Token> {
        let uid = user_id.trim();
        let app_key = cfg.app_key.trim();

        // 1. Check local cache (memory/fast vault)
        let custom_profile = self.get_custom_profile(profile, app_key, org_id, Some(uid));
        if let Ok(token) = self.pool.get_access_token(&custom_profile).await {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        // 2. Slow path via Vault pair
        let key_pair = self.get_user_token_key(app_key, org_id, uid);
        let pair_str = self
            .pool
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
            self.pool.set_access_token(&custom_profile, &token).await?;
            return Ok(token);
        }

        // 3. Refresh logic for user token
        let refresh_result = if Utc::now() < pair.refresh_expires_at {
             self.refresh_token(profile, cfg, &pair.refresh_token).await
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
                self.pool
                    .as_vault()
                    .set(profile, &key_pair, &serde_json::to_string(&new_pair)?)
                    .await?;
                self.pool.set_access_token(&custom_profile, &t).await?;
                Ok(t)
            }
            Err(e) => {
                // 🚀 Fallback to User Permanent Auth Code ("Fire Seed")
                tracing::info!(target: "sys", userId = %uid, orgId = %org_id, "Refresh token failed or expired. Attempting recovery via Permanent Auth Code...");
                
                match self.try_permanent_code_recovery(profile, cfg, org_id, Some(uid)).await {
                    Ok(t) => {
                        self.pool.set_access_token(&custom_profile, &t).await?;
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

    pub async fn get_org_token(
        &self,
        profile: &str,
        cfg: &Config,
        org_id: &str,
    ) -> Result<Token> {
        let app_key = cfg.app_key.trim();
        let org_id = org_id.trim();

        // 1. Check local cache
        let custom_profile = self.get_custom_profile(profile, app_key, org_id, None);
        if let Ok(token) = self.pool.get_access_token(&custom_profile).await {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        // 2. Slow path via Vault pair
        let key_pair = self.get_org_token_key(app_key, org_id);
        let pair_str = self
            .pool
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
            self.pool.set_access_token(&custom_profile, &token).await?;
            return Ok(token);
        }

        // 3. Refresh logic
        if Utc::now() < pair.refresh_expires_at {
            match self.refresh_token(profile, cfg, &pair.refresh_token).await {
                Ok(t) => {
                    self.pool.set_access_token(&custom_profile, &t).await?;
                    return Ok(t);
                }
                Err(e) => {
                    tracing::warn!(target: "sys", orgId = %org_id, error = %e, "Org refresh token failed. Attempting recovery...");
                }
            }
        }

        // 4. Permanent code recovery
        match self.try_permanent_code_recovery(profile, cfg, org_id, None).await {
            Ok(t) => {
                self.pool.set_access_token(&custom_profile, &t).await?;
                Ok(t)
            }
            Err(e) => Err(anyhow!("Organization session expired and recovery failed for org {}: {}", org_id, e))
        }
    }
}

#[async_trait]
impl<'a> AuthProvider for StoreAppProvider<'a> {
    async fn get_token(
        &self,
        profile: &str,
        cfg: &Config,
        headers: &reqwest::header::HeaderMap,
    ) -> Result<Token> {
        let org_id = headers.get("x-org-id").and_then(|v| v.to_str().ok());
        let user_id = headers.get("x-user-id").and_then(|v| v.to_str().ok());

        // 🚀 Multi-tenant Arbitration Enforcement
        match (org_id, user_id) {
            (Some(oid), Some(uid)) if !oid.trim().is_empty() && !uid.trim().is_empty() => {
                self.get_user_token(profile, cfg, oid, uid).await
            }
            (Some(oid), _) if !oid.trim().is_empty() => {
                self.get_org_token(profile, cfg, oid).await
            }
            _ => {
                // Reject requests missing mandatory arbitration headers
                Err(anyhow!("401 Unauthorized: Missing mandatory multi-tenant arbitration headers (x-org-id is required)."))
            }
        }
    }

    async fn refresh(
        &self,
        profile: &str,
        cfg: &Config,
        _headers: &reqwest::header::HeaderMap,
    ) -> Result<Token> {
        let pair_str = self
            .pool
            .as_vault()
            .get(profile, "oauth2_token_pair")
            .await?;
        let pair: OAuth2TokenPair = serde_json::from_str(&pair_str)?;
        self.refresh_token(profile, cfg, &pair.refresh_token).await
    }

    async fn intercept_request(
        &self,
        profile: &str,
        config: &Config,
        path: &str,
        method: &str,
        mut headers: reqwest::header::HeaderMap,
        body: &[u8],
        spec: &serde_json::Value,
    ) -> Result<crate::auth::provider::ProxyRequestAction> {
        // 1. Check for short-circuit interception
        if path.ends_with("/oauth2/token") && method == "POST" {
            let json_resp = self.intercept_exchange(profile, config, body).await?;
            return Ok(crate::auth::provider::ProxyRequestAction::Respond(
                json_resp,
            ));
        }

        // 2. Not intercepted, proceed with normal forwarding logic
        let token = self.get_token(profile, config, &headers).await?;

        // 3. Decorate headers
        let auth_headers = crate::auth::RequestDecorator::get_auth_headers(
            spec,
            path,
            method,
            &config.app_key,
            &config.app_secret,
            &token.value,
        );

        for (name, value) in auth_headers {
            if let Ok(name) = reqwest::header::HeaderName::from_bytes(name.as_bytes()) {
                if let Ok(val) = reqwest::header::HeaderValue::from_bytes(value.as_bytes()) {
                    headers.insert(name, val);
                }
            }
        }

        Ok(crate::auth::provider::ProxyRequestAction::Forward { headers })
    }

    async fn intercept_response(
        &self,
        _profile: &str,
        _config: &Config,
        _path: &str,
        _method: &str,
        _status: u16,
        _response_headers: &reqwest::header::HeaderMap,
        _response_body: &[u8],
    ) -> Result<()> {
        Ok(())
    }

    async fn initialize(
        &self,
        profile: &str,
        config: &Config,
        vault: std::sync::Arc<dyn crate::core::vault::Vault>,
        cfg_mgr: &crate::core::config::ConfigManager,
    ) -> Result<()> {
        // Validation: appKey, appSecret and encryptKey are required for sidecar
        if config.app_key.trim().is_empty()
            || config.app_secret.trim().is_empty()
            || config.encrypt_key.trim().is_empty()
        {
            let bin_name = crate::core::utils::get_bin_name();
            println!("Error: --app-key, --app-secret, and --encrypt-key are required for store-app (sidecar) mode.");
            println!(
                "Example: {} init --app-mode store-app --app-key X --app-secret Y --encrypt-key Z",
                bin_name
            );
            return Err(anyhow!("Missing required credentials for StoreApp mode"));
        }

        println!(
            "✅ Profile '{}' initialized successfully (Sidecar Mode).",
            profile
        );
        println!("💡 Please perform authorization through your main application.");
        println!("🚀 Sidecar is ready. Starting background daemon...");

        let _ = crate::cmd::system::ensure_daemon_running(profile, config, cfg_mgr, vault).await;
        Ok(())
    }

    async fn on_maintenance_tick(&self, profile: &str, config: &Config) -> Result<()> {
        // Routine maintenance for Store App: Maintain the AppAccessToken (SuiteAccessToken)
        match self.get_app_access_token(profile, config).await {
            Ok(token) => {
                let remaining = token.expires_at.signed_duration_since(Utc::now());
                if remaining < Duration::minutes(15) {
                    tracing::info!(target: "sys", "Store App SuiteAccessToken expires in less than 15 mins. Proactively refreshing...");
                    // Refreshing AppAccessToken usually requires a Ticket. 
                    // get_app_access_token already handles the logic of using Ticket.
                    // We just need to trigger it if it's nearing expiry.
                    let _ = self.get_app_access_token(profile, config).await;
                }
            }
            Err(e) => {
                tracing::warn!(target: "sys", error = %e, "Store App SuiteAccessToken is missing or invalid. Waiting for platform push.");
            }
        }
        Ok(())
    }

    fn requires_initial_push(&self, _config: &Config) -> bool {
        false // Store App usually waits for platform events passively
    }

    async fn handle_platform_event(&self, profile: &str, config: &Config, event: crate::auth::provider::PlatformEvent) -> Result<()> {
        match event {
            crate::auth::provider::PlatformEvent::AppTicket(ticket_val) => {
                let ticket = crate::auth::models::Ticket {
                    value: ticket_val,
                    created_at: Utc::now(),
                };
                self.pool.set_app_ticket(&config.app_key, &ticket).await?;
                tracing::info!(target: "sys", "Store App AppTicket updated from platform push");
                Ok(())
            }
            crate::auth::provider::PlatformEvent::TempAuthCode { code, org_id } => {
                tracing::info!(target: "sys", "Store App TEMP_AUTH_CODE received. Exchanging...");
                let oid = org_id.ok_or_else(|| anyhow!("Missing org_id in TempAuthCode event for Store App"))?;
                if let Err(e) = self.exchange_permanent_code_by_temp_code(profile, config, &oid, &code).await {
                    tracing::error!(target: "sys", error = %e, orgId = %oid, "Failed to exchange TEMP_AUTH_CODE for Store App");
                    return Err(e);
                }
                Ok(())
            }
        }
    }

    async fn perform_login(&self, profile: &str, config: &Config, _force: bool, finalize: Option<&str>) -> Result<()> {
        // 1. Finalizer Implementation (Background flow)
        if let Some(_session_id) = finalize {
            return self.finalize_login(profile, config).await;
        }

        // 2. Regular Login flow
        println!("🔄 [StoreApp] Attempting to refresh token pair for profile '{}'...", profile);
        match self.refresh(profile, config, &reqwest::header::HeaderMap::new()).await {
            Ok(_) => {
                println!("✅ Success! OAuth2 Token Pair has been rotated.");
                Ok(())
            }
            Err(e) => {
                println!("❌ Refresh failed: {}", e);
                println!("💡 Suggestion: Sidecar session may have expired. Please re-authorize through your \x1b[33mMain Application\x1b[0m.");
                Err(e)
            }
        }
    }
    async fn finalize_login(&self, profile: &str, cfg: &Config) -> Result<()> {
        tracing::info!(target: "sys", profile = %profile, "Finalizer started for StoreApp auth");
        
        let session_manager = AuthSessionManager::new(self.pool);
        let session = session_manager.get_session(profile).await?;
        
        let (actual_port, rx) = crate::auth::lifecycle::listener::OAuth2CallbackListener::start(session.redirect_port, profile.to_string()).await?;
        tracing::info!(target: "sys", port = %actual_port, "Finalizer listening for callback");

        let res = tokio::select! {
            result = rx => {
                match result {
                    Ok(inner_res) => {
                        match inner_res {
                            Ok(res) => {
                                tracing::info!(target: "sys", "Callback received, saving code...");
                                session_manager.save_code(profile, &res.code, &res.state).await?;
                                
                                // Trigger exchange
                                match self.get_app_access_token(profile, cfg).await {
                                    Ok(_) => {
                                        tracing::info!(target: "sys", "Token exchange successful");
                                        Ok(())
                                    }
                                    Err(e) => {
                                        tracing::error!(target: "sys", error = %e, "Token exchange failed");
                                        Err(e)
                                    }
                                }
                            }
                            Err(e) => Err(anyhow::anyhow!("Authorization failed: {}", e))
                        }
                    }
                    Err(e) => Err(anyhow::anyhow!("Internal listener error: {}", e))
                }
            },
            _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
                Err(anyhow::anyhow!("Timeout waiting for authorization (5 mins)"))
            }
        };

        if res.is_err() {
            let _ = session_manager.clear(profile).await;
        }
        res
    }

    async fn get_status_entries(&self, profile: &str, config: &Config) -> Result<Vec<crate::core::status::StatusEntry>> {
        use crate::core::status::{StatusEntry, StatusLevel};
        let mut entries = Vec::new();
        let vault = self.pool.as_vault();

        // 1. Security Check (Sidecar Mode)
        let mut missing = Vec::new();
        if vault.get(profile, "app_secret").await.is_err() { missing.push("app_secret".to_string()); }
        if vault.get(profile, "encrypt_key").await.is_err() { missing.push("encrypt_key".to_string()); }

        let (sec_level, sec_msg) = if missing.is_empty() {
            (StatusLevel::OK, "Sidecar credentials are securely stored.".to_string())
        } else {
            (StatusLevel::WARN, format!("Missing: {}", missing.join(", ")))
        };

        entries.push(StatusEntry {
            name: "Security (Sidecar)".to_string(),
            icon: "🛡️".to_string(),
            level: sec_level,
            message: sec_msg,
            reason: if sec_level == StatusLevel::WARN { Some("缺少必要凭据，可能导致解密失败或令牌刷新失败。".to_string()) } else { None },
            details: vec![],
            children: vec![],
        });

        // 2. Token Check (SuiteAccessToken and Org/User Tokens)
        let refresh_error = vault.get(profile, "last_refresh_error").await.ok();
        let ref_revoked = vault.get(profile, "oauth2_revoked").await.is_ok();

        // 2.1 Org/User Tokens
        if let Ok(pair_raw) = vault.get(profile, "oauth2_token_pair").await {
            let pair: crate::auth::models::OAuth2TokenPair = serde_json::from_str(&pair_raw)?;
            let is_expired = Utc::now() > pair.expires_at;
            let ref_expired = Utc::now() > pair.refresh_expires_at;

            let children = vec![
                StatusEntry {
                    name: "AccessToken".to_string(),
                    icon: "🔑".to_string(),
                    level: if is_expired || ref_revoked { StatusLevel::ERROR } else { StatusLevel::OK },
                    message: format!("[{}] (Expires: {})", 
                        if is_expired || ref_revoked { "EXPIRED" } else { "VALID" },
                        pair.expires_at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S")),
                    reason: if ref_revoked {
                        Some("关联的 RefreshToken 已失效，AccessToken 无法继续自动续约。".to_string())
                    } else if is_expired { 
                        refresh_error.map(|e| format!("自动续约失败: {}", e))
                            .or(Some("AccessToken 已过期，正在等待后台续约进程处理...".to_string()))
                    } else { None },
                    details: vec![],
                    children: vec![],
                },
                StatusEntry {
                    name: "RefreshToken".to_string(),
                    icon: "🔄".to_string(),
                    level: if ref_expired || ref_revoked { StatusLevel::ERROR } else { StatusLevel::OK },
                    message: format!("[{}] (Expires: {})", 
                        if ref_revoked { "REVOKED" } else if ref_expired { "EXPIRED" } else { "VALID" },
                        pair.refresh_expires_at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S")),
                    reason: if ref_revoked {
                        Some("令牌已于服务端吊销或失效，必须重新执行 `cowen auth login`。".to_string())
                    } else if ref_expired { 
                        Some("RefreshToken 已失效，必须重新运行 'cowen auth login' 或 'init'。".to_string()) 
                    } else { None },
                    details: vec![],
                    children: vec![],
                }
            ];

            let mut details = vec![];
            let token_inner = Token {
                value: pair.access_token.clone(),
                expires_at: pair.expires_at,
                created_at: pair.created_at,
            };
            
            if let Some(identity) = token_inner.extract_identity() {
                details.push(format!("User ID: {}", identity.user_id));
                details.push(format!("Org ID:  {}", identity.org_id));
                details.push(format!("App ID:  {}", identity.app_id));
            }

            entries.push(StatusEntry {
                name: "Authentication (Org)".to_string(),
                icon: "🔐".to_string(),
                level: if ref_revoked { StatusLevel::ERROR } else if is_expired { StatusLevel::WARN } else { StatusLevel::OK },
                message: "OAuth2 tokens are locally managed via sidecar.".to_string(),
                reason: if ref_revoked { Some("会话已失效 (Revoked)".to_string()) } else { None },
                details,
                children,
            });
        }

        // 2.2 SuiteAccessToken Status
        if let Ok(sat) = self.pool.get_app_access_token(config.app_key.trim()).await {
             let is_expired = sat.is_expired();
             entries.push(StatusEntry {
                name: "SuiteAccessToken".to_string(),
                icon: "🎫".to_string(),
                level: if is_expired { StatusLevel::ERROR } else { StatusLevel::OK },
                message: format!("[{}] (Expires: {})", 
                    if is_expired { "EXPIRED" } else { "VALID" },
                    sat.expires_at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S")),
                reason: if is_expired { Some("SuiteAccessToken 已过期，正在尝试通过 AppTicket 续约。".to_string()) } else { None },
                details: vec![],
                children: vec![],
            });
        }

        // 2.3 AppTicket Status
        if let Ok(ts_str) = vault.get(profile, "app_ticket_created").await {
            let created_at = chrono::DateTime::parse_from_rfc3339(&ts_str).map(|dt| dt.with_timezone(&Utc)).unwrap_or(Utc::now());
            entries.push(StatusEntry {
                name: "AppTicket".to_string(),
                icon: "🎫".to_string(),
                level: StatusLevel::OK,
                message: format!("[CACHED] (Received: {})", created_at.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S")),
                reason: None,
                details: vec![],
                children: vec![],
            });
        } else {
            entries.push(StatusEntry {
                name: "AppTicket".to_string(),
                icon: "🎫".to_string(),
                level: StatusLevel::NONE,
                message: "[NONE] (等待 Daemon 接收推送)".to_string(),
                reason: None,
                details: vec![],
                children: vec![],
            });
        }

        Ok(entries)
    }

    fn get_default_app_key(&self) -> Option<String> {
        Some(crate::auth::models::BUILTIN_CLIENT_ID.to_string())
    }

    fn decorate_openapi_request(&self, _url: &mut String, headers: &mut reqwest::header::HeaderMap, token: &Token, config: &Config) {
        headers.insert("openToken", token.value.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
        headers.insert("appKey", config.app_key.parse().unwrap_or(reqwest::header::HeaderValue::from_static("")));
    }

    async fn on_logout(&self, profile: &str, _config: &Config) -> Result<()> {
        let vault = self.pool.as_vault();
        let _ = vault.delete(profile, "oauth2_token_pair");
        let _ = vault.delete(profile, "pending_auth_session");
        let _ = vault.delete(profile, "captured_auth_code");
        let _ = vault.delete(profile, "oauth2_revoked");
        let _ = vault.delete(profile, "last_refresh_error");
        let _ = vault.delete(profile, "app_ticket");
        let _ = vault.delete(profile, "app_ticket_created");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::pool::TokenPool;
    use crate::auth::client::{HttpSender, SimpleResponse};
    use crate::auth::VaultTokenPool;
    use std::sync::Arc;
    use async_trait::async_trait;

    // --- Manual Mocks ---

    struct MockVault {}
    #[async_trait]
    impl crate::core::vault::Vault for MockVault {
        async fn get_config(&self, _: &str, _: &str) -> Result<String> { Ok("".to_string()) }
        async fn get_config_full(&self, _: &str, _: &str) -> Result<crate::core::store::Item> { Err(anyhow!("not found")) }
        async fn set_config(&self, _: &str, _: &str, _: &str) -> Result<()> { Ok(()) }
        async fn set_config_conditional(&self, _: &str, _: &str, _: &str, _: u64) -> Result<()> { Ok(()) }
        async fn list_configs(&self, _: &str) -> Result<Vec<String>> { Ok(vec![]) }
        async fn delete_config(&self, _: &str, _: &str) -> Result<()> { Ok(()) }
        async fn get_secret(&self, _: &str, _: &str) -> Result<String> { Ok("".to_string()) }
        async fn set_secret(&self, _: &str, _: &str, _: &str) -> Result<()> { Ok(()) }
        async fn get_token(&self, _: &str, _: &str) -> Result<String> { Ok("".to_string()) }
        async fn set_token(&self, _: &str, _: &str, _: &str, _: u64) -> Result<()> { Ok(()) }
        async fn save_audit(&self, _: &crate::core::store::AuditEntry) -> Result<()> { Ok(()) }
        async fn list_audit(&self, _: &str, _: usize) -> Result<Vec<crate::core::store::AuditEntry>> { Ok(vec![]) }
        async fn push_dlq(&self, _: &crate::core::store::DlqMessage) -> Result<()> { Ok(()) }
        async fn pop_dlq(&self, _: &str, _: &str) -> Result<Option<crate::core::store::DlqMessage>> { Ok(None) }
        async fn list_dlq(&self, _: &str, _: usize) -> Result<Vec<crate::core::store::DlqMessage>> { Ok(vec![]) }
        async fn get_cache(&self, _: &str, _: &str) -> Result<String> { Ok("".to_string()) }
        async fn set_cache(&self, _: &str, _: &str, _: &str, _: u64) -> Result<()> { Ok(()) }
        async fn get(&self, _: &str, _: &str) -> Result<String> { Ok("".to_string()) }
        async fn set(&self, _: &str, _: &str, _: &str) -> Result<()> { Ok(()) }
        async fn delete(&self, _: &str, _: &str) -> Result<()> { Ok(()) }
        async fn list_keys(&self, _: &str, _: &str) -> Result<Vec<String>> { Ok(vec![]) }
        async fn clear_profile(&self, _: &str) -> Result<()> { Ok(()) }
        async fn rename_profile(&self, _: &str, _: &str) -> Result<()> { Ok(()) }
        async fn list_all_profiles(&self) -> Result<Vec<String>> { Ok(vec![]) }
        async fn notify_config_changed(&self, _: &str, _: &str) -> Result<()> { Ok(()) }
        async fn watch_config(&self, _: &str) -> Result<std::pin::Pin<Box<dyn tokio_stream::Stream<Item = String> + Send>>> {
             Ok(Box::pin(tokio_stream::iter(vec![])))
        }
        fn primary_store(&self) -> Arc<dyn crate::core::store::Store> {
            unimplemented!()
        }
    }

    struct MockHttpSender {}
    #[async_trait]
    impl HttpSender for MockHttpSender {
        async fn post(&self, _url: &str, _headers: reqwest::header::HeaderMap, _body: serde_json::Value) -> Result<SimpleResponse> {
            Ok(SimpleResponse { status: 200, body: "{}".to_string() })
        }
        async fn post_form(&self, _url: &str, _headers: reqwest::header::HeaderMap, _body: serde_json::Value) -> Result<SimpleResponse> {
            Ok(SimpleResponse { status: 200, body: "{}".to_string() })
        }
        async fn get(&self, _url: &str, _headers: reqwest::header::HeaderMap) -> Result<SimpleResponse> {
            Ok(SimpleResponse { status: 200, body: "{}".to_string() })
        }
    }

    #[tokio::test]
    async fn test_get_token_missing_org_id_rejection() {
        let vault = Arc::new(MockVault {});
        let pool = VaultTokenPool::new(vault);
        let sender = Arc::new(MockHttpSender {});
        let provider = StoreAppProvider::new(&pool, sender);
        
        let config = Config::default_with_profile("test");
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-user-id", "U123".parse().unwrap());

        let result = provider.get_token("default", &config, &headers).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("401 Unauthorized"));
        assert!(err.to_string().contains("x-org-id"));
    }

    #[tokio::test]
    async fn test_get_token_with_org_only_isolation() {
        let vault = Arc::new(MockVault {});
        let pool = VaultTokenPool::new(vault);
        let sender = Arc::new(MockHttpSender {});
        let provider = StoreAppProvider::new(&pool, sender);
        
        let mut config = Config::default_with_profile("test");
        config.app_key = "test_app".to_string();
        
        let key = provider.get_org_token_key("test_app", "ORG1");
        assert_eq!(key, "oauth2_token_pair_org_test_app_ORG1");
    }

    #[tokio::test]
    async fn test_get_token_with_user_isolation() {
        let vault = Arc::new(MockVault {});
        let pool = VaultTokenPool::new(vault);
        let sender = Arc::new(MockHttpSender {});
        let provider = StoreAppProvider::new(&pool, sender);
        
        let mut config = Config::default_with_profile("test");
        config.app_key = "test_app".to_string();
        
        let key = provider.get_user_token_key("test_app", "ORG1", "USER1");
        assert_eq!(key, "oauth2_token_pair_user_test_app_ORG1_USER1");
    }
}
