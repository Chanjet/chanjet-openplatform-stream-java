use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use crate::auth::models::{Token, OAuth2TokenPair};
use crate::auth::pool::TokenPool;
use crate::auth::provider::AuthProvider;
use crate::auth::client::HttpSender;
use crate::core::config::Config;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::{Utc, Duration};
use serde::Deserialize;
use std::sync::Arc;
use std::fs::File;
use fs2::FileExt;
use crate::auth::lifecycle::AuthSessionManager;

pub struct Pkce {
    pub verifier: String,
}

pub struct OAuth2Provider<'a> {
    pool: &'a (dyn TokenPool + Send + Sync),
    http_sender: Arc<dyn HttpSender>,
}

#[derive(Debug, Deserialize)]
struct OAuth2TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    refresh_token_expires_in: i64,
}

impl<'a> OAuth2Provider<'a> {
    pub fn new(pool: &'a (dyn TokenPool + Send + Sync), http_sender: Arc<dyn HttpSender>) -> Self {
        Self { pool, http_sender }
    }

    async fn exchange_code(&self, profile: &str, cfg: &Config, code: &str, verifier: &str, redirect_uri: &str) -> Result<Token> {
        let url = format!("{}{}", cfg.openapi_url.trim_end_matches('/'), obfs!("/oauth2/token"));
        let body = serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": cfg.app_key.trim(),
            "client_secret": cfg.app_secret.trim(),
            "code": code,
            "code_verifier": verifier,
            "redirect_uri": redirect_uri,
        });

        self.request_token(profile, &url, body).await
    }

    async fn refresh_token(&self, profile: &str, cfg: &Config, refresh_token: &str) -> Result<Token> {
        let url = format!("{}{}", cfg.openapi_url.trim_end_matches('/'), obfs!("/oauth2/token"));
        let body = serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": cfg.app_key.trim(),
            "client_secret": cfg.app_secret.trim(),
            "refresh_token": refresh_token,
        });

        self.request_token(profile, &url, body).await
    }

    async fn request_token(&self, profile: &str, url: &str, body: serde_json::Value) -> Result<Token> {
        let headers = reqwest::header::HeaderMap::new();
        let resp = self.http_sender.post(url, headers, body).await?;

        if !resp.is_success() {
            let status = resp.status;
            let err_text = resp.text();
            
            tracing::error!(
                target: "audit",
                profile = %profile,
                event = "token_rotate",
                status = "failure",
                error = %err_text,
                "OAuth2 token rotation failed"
            );

            // Handle specific platform error codes (Design §6)
            if err_text.contains("4029") {
                return Err(anyhow!("登录会话已超时（7天），请执行 `owenc init` 重新授权。 (Error: {})", status));
            }
            if err_text.contains("4007") || err_text.contains("invalid_grant") {
                return Err(anyhow!("令牌已失效，请执行 `owenc auth login` 重新授权。 (Error: {})", status));
            }
            if err_text.contains("4006") {
                return Err(anyhow!("ClientID 与令牌颁发者不一致，请检查配置。 (Error: {})", status));
            }
            if err_text.contains("4001") {
                return Err(anyhow!("授权校验失败 (PKCE)，请重新执行 `owenc init`。 (Error: {})", status));
            }

            return Err(anyhow!("OAuth2 token request failed (HTTP {}): {}", status, err_text));
        }

        let token_resp: OAuth2TokenResponse = resp.json().await?;
        let now = Utc::now();
        
        let token = Token {
            value: token_resp.access_token.clone(),
            expires_at: now + Duration::seconds(token_resp.expires_in),
            created_at: now,
        };

        let pair = OAuth2TokenPair {
            access_token: token_resp.access_token,
            refresh_token: token_resp.refresh_token,
            expires_at: token.expires_at,
            refresh_expires_at: now + Duration::seconds(token_resp.refresh_token_expires_in),
            created_at: now,
        };

        // Save to vault via pool
        self.pool.as_vault().set(profile, "oauth2_token_pair", &serde_json::to_string(&pair)?)?;
        self.pool.set_access_token(profile, &token)?;

        tracing::info!(
            target: "audit",
            profile = %profile,
            event = "token_rotate",
            status = "success",
            "OAuth2 token pair successfully rotated"
        );

        Ok(token)
    }
}

#[async_trait]
impl<'a> AuthProvider for OAuth2Provider<'a> {
    async fn get_token(&self, profile: &str, cfg: &Config) -> Result<Token> {
        // 1. Fast path: check current memory/local cache
        if let Ok(token) = self.pool.get_access_token(profile) {
            if !token.is_expired() {
                return Ok(token);
            }
        }

        // 2. Slow path: Acquire Cross-Process File Lock
        let lock_dir = crate::core::config::get_app_dir().join("locks");
        std::fs::create_dir_all(&lock_dir)?;
        let lock_file_path = lock_dir.join(format!("{}.lock", profile));
        let lock_file = File::create(&lock_file_path)?;
        
        // Blocking lock (wait for other processes)
        lock_file.lock_exclusive()?;
        
        let result = (|| async {
            // 3. Double-Check: Reload from Vault after acquiring lock
            // Another process might have refreshed the token while we were waiting
            if let Ok(token) = self.pool.get_access_token(profile) {
                if !token.is_expired() {
                    return Ok(token);
                }
            }

            // 4. Finalizer Path: Check for captured code
            let session_manager = AuthSessionManager::new(self.pool);
            if let Ok(code) = session_manager.get_captured_code(profile) {
                if let Ok(session) = session_manager.get_session(profile) {
                    tracing::info!(target: "sys", "Captured auth code found for profile '{}'. Finalizing exchange...", profile);
                    let token = self.exchange_code(profile, cfg, &code, &session.code_verifier, &session.redirect_uri).await?;
                    let _ = session_manager.clear(profile);
                    return Ok(token);
                }
            }

            let pair_str = self.pool.as_vault().get(profile, "oauth2_token_pair")?;
            let pair: OAuth2TokenPair = serde_json::from_str(&pair_str)?;

            // Re-check expiry in case it was updated
            if Utc::now() < pair.expires_at {
                let token = Token {
                    value: pair.access_token.clone(),
                    expires_at: pair.expires_at,
                    created_at: pair.created_at,
                };
                self.pool.set_access_token(profile, &token)?;
                return Ok(token);
            }

            if Utc::now() >= pair.refresh_expires_at {
                return Err(anyhow!("OAuth2 session expired. Please run 'owenc init' to re-authenticate."));
            }

            self.refresh_token(profile, cfg, &pair.refresh_token).await
        })().await;

        lock_file.unlock()?;
        result
    }

    async fn refresh(&self, profile: &str, cfg: &Config) -> Result<Token> {
        let pair_str = self.pool.as_vault().get(profile, "oauth2_token_pair")?;
        let pair: OAuth2TokenPair = serde_json::from_str(&pair_str)?;
        self.refresh_token(profile, cfg, &pair.refresh_token).await
    }
}
impl Pkce {
    pub fn new() -> Self {
        let verifier = Self::generate_verifier(64);
        Self { verifier }
    }

    fn generate_verifier(len: usize) -> String {
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
        (0..len)
            .map(|_| {
                let idx = rand::random_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    pub fn generate_challenge(verifier: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let result = hasher.finalize();
        URL_SAFE_NO_PAD.encode(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::vault::Vault;

    #[test]
    fn test_pkce_generation() {
        let pkce = Pkce::new();
        assert_eq!(pkce.verifier.len(), 64);
        
        // Verify challenge can be computed from verifier
        let challenge = Pkce::generate_challenge(&pkce.verifier);
        assert!(!challenge.is_empty());
        
        // Manual verification of challenge
        let mut hasher = Sha256::new();
        hasher.update(pkce.verifier.as_bytes());
        let expected_challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
        assert_eq!(challenge, expected_challenge);
    }

    #[test]
    fn test_verifier_charset() {
        let verifier = Pkce::generate_verifier(1000);
        let allowed = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
        for c in verifier.chars() {
            assert!(allowed.contains(c));
        }
    }

    struct MockPool {
        token: std::sync::Mutex<Option<Token>>,
        vault: Arc<dyn crate::core::vault::Vault>,
    }

    struct MockVault {
        data: std::sync::Mutex<std::collections::HashMap<String, String>>,
    }

    impl crate::core::vault::Vault for MockVault {
        fn get(&self, _profile: &str, key: &str) -> Result<String> {
            self.data.lock().unwrap().get(key).cloned().ok_or_else(|| anyhow!("Not found"))
        }
        fn set(&self, _profile: &str, key: &str, value: &str) -> Result<()> {
            self.data.lock().unwrap().insert(key.to_string(), value.to_string());
            Ok(())
        }
        fn delete(&self, _profile: &str, key: &str) -> Result<()> {
            self.data.lock().unwrap().remove(key);
            Ok(())
        }
        fn clear_profile(&self, _profile: &str) -> Result<()> {
            self.data.lock().unwrap().clear();
            Ok(())
        }
    }

    impl TokenPool for MockPool {
        fn get_app_ticket(&self, _profile: &str) -> Result<crate::auth::models::Ticket> {
            Err(anyhow!("Not implemented"))
        }
        fn set_app_ticket(&self, _profile: &str, _ticket: &crate::auth::models::Ticket) -> Result<()> {
            Ok(())
        }
        fn get_access_token(&self, _profile: &str) -> Result<Token> {
            self.token.lock().unwrap().clone().ok_or_else(|| anyhow!("No token"))
        }
        fn set_access_token(&self, _profile: &str, token: &Token) -> Result<()> {
            *self.token.lock().unwrap() = Some(token.clone());
            Ok(())
        }
        fn delete_access_token(&self, _profile: &str) -> Result<()> {
            *self.token.lock().unwrap() = None;
            Ok(())
        }
        fn clear_cache(&self, _profile: &str) {}
        fn as_vault(&self) -> Arc<dyn crate::core::vault::Vault> {
            self.vault.clone()
        }
    }

    struct MockHttpSender {
        response_body: String,
        status: u16,
    }

    #[async_trait]
    impl HttpSender for MockHttpSender {
        async fn post(&self, _url: &str, _headers: reqwest::header::HeaderMap, _body: serde_json::Value) -> Result<crate::auth::client::SimpleResponse> {
            Ok(crate::auth::client::SimpleResponse {
                status: self.status,
                body: self.response_body.clone(),
            })
        }
        async fn get(&self, _url: &str, _headers: reqwest::header::HeaderMap) -> Result<crate::auth::client::SimpleResponse> {
            Ok(crate::auth::client::SimpleResponse {
                status: self.status,
                body: self.response_body.clone(),
            })
        }
    }

    #[tokio::test]
    async fn test_refresh_token_success() {
        let vault = Arc::new(MockVault { data: std::sync::Mutex::new(std::collections::HashMap::new()) });
        let pool = MockPool {
            token: std::sync::Mutex::new(None),
            vault: vault.clone(),
        };

        // Initial setup
        let initial_pair = OAuth2TokenPair {
            access_token: "old_access".to_string(),
            refresh_token: "old_refresh".to_string(),
            expires_at: Utc::now() - Duration::hours(1), // Expired
            refresh_expires_at: Utc::now() + Duration::hours(24),
            created_at: Utc::now() - Duration::hours(1),
        };
        vault.set("test", "oauth2_token_pair", &serde_json::to_string(&initial_pair).unwrap()).unwrap();

        let mock_http = Arc::new(MockHttpSender {
            status: 200,
            response_body: serde_json::json!({
                "access_token": "new_access",
                "refresh_token": "new_refresh",
                "expires_in": 3600,
                "refresh_token_expires_in": 7200
            }).to_string(),
        });

        let provider = OAuth2Provider::new(&pool, mock_http);
        let cfg = Config::default_with_profile("test");

        let token = provider.get_token("test", &cfg).await.unwrap();
        assert_eq!(token.value, "new_access");

        // Verify state update
        let saved_pair_str = vault.get("test", "oauth2_token_pair").unwrap();
        let saved_pair: OAuth2TokenPair = serde_json::from_str(&saved_pair_str).unwrap();
        assert_eq!(saved_pair.access_token, "new_access");
        assert_eq!(saved_pair.refresh_token, "new_refresh");
    }

    #[tokio::test]
    async fn test_oauth2_full_lifecycle_success() {
        let vault = Arc::new(MockVault { data: std::sync::Mutex::new(std::collections::HashMap::new()) });
        let pool = MockPool {
            token: std::sync::Mutex::new(None),
            vault: vault.clone(),
        };

        // 1. Init - Create Session
        let session_manager = AuthSessionManager::new(&pool);
        let session = session_manager.create_session("test", 0).unwrap(); // Port 0 for random

        // 2. Start Listener
        let (actual_port, rx) = crate::auth::lifecycle::listener::OAuth2CallbackListener::start(session.redirect_port).await;
        
        // 3. Simulate Callback (e.g. from Browser)
        let client = reqwest::Client::new();
        let callback_url = format!("http://127.0.0.1:{}/oauth2/callback?code=captured_code&state={}", actual_port, session.state);
        client.get(&callback_url).send().await.unwrap();

        // 4. Capture result and save to Vault
        let result = rx.await.unwrap();
        session_manager.save_code("test", &result.code, &result.state).unwrap();

        // 5. Finalize - Call get_token (which should trigger Finalizer)
        let mock_http = Arc::new(MockHttpSender {
            status: 200,
            response_body: serde_json::json!({
                "access_token": "final_access",
                "refresh_token": "final_refresh",
                "expires_in": 3600,
                "refresh_token_expires_in": 7200
            }).to_string(),
        });
        
        let provider = OAuth2Provider::new(&pool, mock_http);
        let cfg = Config::default_with_profile("test");

        let token = provider.get_token("test", &cfg).await.unwrap();
        assert_eq!(token.value, "final_access");

        // 6. Verify Session is cleared
        assert!(session_manager.get_captured_code("test").is_err());
        assert!(session_manager.get_session("test").is_err());
    }
}
