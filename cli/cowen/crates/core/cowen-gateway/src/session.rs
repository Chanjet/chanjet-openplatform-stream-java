// PRD v0.5.0 — Gateway Session Management
//
// Provides stateless encrypted session (JWE-like) for the Identity-Aware Gateway.
// Uses AES-256-GCM (via cowen_common::security) to encrypt/decrypt session claims
// stored in the `cowen_sess_id` cookie.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Session claims stored inside the encrypted `cowen_sess_id` cookie.
///
/// Contains the open-platform identity extracted after a successful
/// code exchange, along with dual-timestamp fields for the discrete
/// sliding-window renewal algorithm (PRD §5.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayClaims {
    /// The organization ID (x-org-id).
    pub org_id: String,
    /// The user ID (x-user-id), if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// The application ID (x-app-id), if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    /// The open-platform access token (for Egress proxy injection).
    pub open_token: String,
    /// Idle expiration timestamp (Unix seconds). Reset on each activity.
    pub idle_exp: i64,
    /// Absolute expiration timestamp (Unix seconds). Never extended.
    pub abs_exp: i64,
    /// Issued-at timestamp (Unix seconds).
    pub iat: i64,
    /// Fingerprint binding (SHA256 of IP + UA).
    pub fp: String,
}

impl GatewayClaims {
    /// Create new claims with the given identity and session durations.
    ///
    /// - `idle_timeout_secs`: How long the session survives without activity (e.g. 1800 = 30min).
    /// - `absolute_timeout_secs`: Maximum session lifetime regardless of activity (e.g. 86400 = 24h).
    pub fn new(
        org_id: String,
        user_id: Option<String>,
        app_id: Option<String>,
        open_token: String,
        fp: String,
        idle_timeout_secs: i64,
        absolute_timeout_secs: i64,
    ) -> Self {
        let now = Utc::now().timestamp();
        Self {
            org_id,
            user_id,
            app_id,
            open_token,
            idle_exp: now + idle_timeout_secs,
            abs_exp: now + absolute_timeout_secs,
            iat: now,
            fp,
        }
    }

    /// Check if the session has expired (idle or absolute).
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp();
        now > self.idle_exp || now > self.abs_exp
    }

    /// Check if the session needs a discrete sliding-window refresh.
    ///
    /// Returns true if the remaining idle time is within the refresh threshold,
    /// meaning the session should be extended to prevent imminent idle expiry.
    ///
    /// - `refresh_threshold_secs`: The "临期阈值" (e.g. 600 = 10 minutes).
    ///   If `idle_exp - now <= threshold`, the session needs refresh.
    pub fn needs_refresh(&self, refresh_threshold_secs: i64) -> bool {
        let now = Utc::now().timestamp();
        let remaining_idle = self.idle_exp - now;
        remaining_idle <= refresh_threshold_secs && !self.is_expired()
    }

    /// Create a refreshed copy with extended idle expiration.
    /// The absolute expiration is NOT changed.
    pub fn refresh_idle(&self, idle_timeout_secs: i64) -> Self {
        let now = Utc::now().timestamp();
        Self {
            idle_exp: now + idle_timeout_secs,
            ..self.clone()
        }
    }

    /// Create a refreshed copy with a new open_token (if the underlying
    /// platform token was refreshed) and extended idle expiration.
    pub fn refresh_with_token(&self, new_token: String, idle_timeout_secs: i64) -> Self {
        let now = Utc::now().timestamp();
        Self {
            open_token: new_token,
            idle_exp: now + idle_timeout_secs,
            ..self.clone()
        }
    }

    /// Extract the creation time as a DateTime.
    pub fn issued_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.iat, 0).unwrap_or_default()
    }

    /// Extract the idle expiration as a DateTime.
    pub fn idle_expires_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.idle_exp, 0).unwrap_or_default()
    }

    /// Extract the absolute expiration as a DateTime.
    pub fn absolute_expires_at(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.abs_exp, 0).unwrap_or_default()
    }
}

/// Session manager handles encryption/decryption of gateway session cookies.
///
/// Uses the existing `cowen_common::security` AES-256-GCM primitives with keys
/// managed by `JwksManager`.
#[derive(Clone)]
pub struct SessionManager {
    /// JWKS Key provider for rotation.
    key_provider: std::sync::Arc<dyn crate::jwks::KeyProvider>,
    /// Idle timeout in seconds (default 30 minutes).
    idle_timeout_secs: i64,
    /// Absolute timeout in seconds (default 24 hours).
    absolute_timeout_secs: i64,
    /// Refresh threshold in seconds (default 10 minutes).
    refresh_threshold_secs: i64,
}

impl SessionManager {
    /// Create a new SessionManager with the given JwksManager.
    pub fn new(key_provider: std::sync::Arc<dyn crate::jwks::KeyProvider>) -> Result<Self, String> {
        Ok(Self {
            key_provider,
            idle_timeout_secs: 1800,      // 30 minutes
            absolute_timeout_secs: 86400, // 24 hours
            refresh_threshold_secs: 600,  // 10 minutes
        })
    }

    /// Create with custom timeouts (for testing).
    #[cfg(test)]
    pub fn with_timeouts(
        key_provider: std::sync::Arc<dyn crate::jwks::KeyProvider>,
        idle_secs: i64,
        abs_secs: i64,
        refresh_threshold_secs: i64,
    ) -> Result<Self, String> {
        Ok(Self {
            key_provider,
            idle_timeout_secs: idle_secs,
            absolute_timeout_secs: abs_secs,
            refresh_threshold_secs,
        })
    }

    /// Create a new encrypted session cookie value from identity claims.
    pub async fn create_session(
        &self,
        org_id: String,
        user_id: Option<String>,
        app_id: Option<String>,
        open_token: String,
        fp: String,
    ) -> Result<String, String> {
        let claims = GatewayClaims::new(
            org_id,
            user_id,
            app_id,
            open_token,
            fp,
            self.idle_timeout_secs,
            self.absolute_timeout_secs,
        );
        self.encrypt_claims(&claims).await
    }

    /// Validate and decrypt a session cookie value.
    /// Returns the claims if valid, or an error if expired/tampered.
    pub async fn validate_session(
        &self,
        cookie_value: &str,
        current_fp: &str,
    ) -> Result<GatewayClaims, String> {
        let claims = self.decrypt_claims(cookie_value).await?;
        if claims.is_expired() {
            return Err("Session expired".to_string());
        }
        if claims.fp != current_fp {
            return Err("Session fingerprint mismatch".to_string());
        }
        Ok(claims)
    }

    /// Check if a validated session needs a sliding-window refresh.
    pub fn needs_refresh(&self, claims: &GatewayClaims) -> bool {
        claims.needs_refresh(self.refresh_threshold_secs)
    }

    /// Create a refreshed session cookie value with extended idle time.
    pub async fn refresh_session(&self, claims: &GatewayClaims) -> Result<String, String> {
        let refreshed = claims.refresh_idle(self.idle_timeout_secs);
        self.encrypt_claims(&refreshed).await
    }

    /// Create a refreshed session with a new open_token.
    pub async fn refresh_session_with_token(
        &self,
        claims: &GatewayClaims,
        new_token: String,
    ) -> Result<String, String> {
        let refreshed = claims.refresh_with_token(new_token, self.idle_timeout_secs);
        self.encrypt_claims(&refreshed).await
    }

    /// The idle timeout in seconds.
    pub fn idle_timeout_secs(&self) -> i64 {
        self.idle_timeout_secs
    }

    /// The absolute timeout in seconds.
    pub fn absolute_timeout_secs(&self) -> i64 {
        self.absolute_timeout_secs
    }

    // -- Internal encryption helpers --

    async fn encrypt_claims(&self, claims: &GatewayClaims) -> Result<String, String> {
        let (kid, key) = self
            .key_provider
            .get_active_key()
            .await
            .map_err(|e| e.to_string())?;

        let json = serde_json::to_vec(claims).map_err(|e| format!("Serialize failed: {}", e))?;
        let key_arr: &[u8; 32] = key.as_slice().try_into().map_err(|_| "Key size mismatch")?;
        let encrypted = cowen_common::security::encrypt(&json, key_arr)
            .map_err(|e| format!("Encrypt failed: {}", e))?;

        let b64 = URL_SAFE_NO_PAD.encode(&encrypted);
        Ok(format!("{}.{}", kid, b64))
    }

    async fn decrypt_claims(&self, cookie_value: &str) -> Result<GatewayClaims, String> {
        let parts: Vec<&str> = cookie_value.splitn(2, '.').collect();
        if parts.len() != 2 {
            return Err("Invalid cookie format (missing kid)".to_string());
        }
        let kid = parts[0];
        let encrypted_b64 = parts[1];

        let key = self
            .key_provider
            .get_key_by_kid(kid)
            .await
            .map_err(|e| e.to_string())?;

        let encrypted = URL_SAFE_NO_PAD
            .decode(encrypted_b64)
            .map_err(|e| format!("Base64 decode failed: {}", e))?;

        let key_arr: &[u8; 32] = key.as_slice().try_into().map_err(|_| "Key size mismatch")?;
        let json = cowen_common::security::decrypt(&encrypted, key_arr)
            .map_err(|e| format!("Decrypt failed: {}", e))?;
        serde_json::from_slice(&json).map_err(|e| format!("Deserialize failed: {}", e))
    }
}

/// Build the Set-Cookie header value for the gateway session.
///
/// Cookie attributes per PRD §5.9:
/// - `HttpOnly`: Prevents XSS cookie theft
/// - `Secure`: Forces HTTPS transport
/// - `SameSite=Lax`: CSRF protection while allowing top-level navigation
///   (compatible with app-store direct launch, per PRD §5.2)
/// - `Path=/`: Available on all paths
pub fn build_set_cookie_header(cookie_value: &str) -> String {
    format!(
        "cowen_sess_id={}; HttpOnly; Secure; SameSite=Lax; Path=/",
        cookie_value
    )
}

/// Build the Set-Cookie header to delete the gateway session.
pub fn build_delete_cookie_header() -> String {
    "cowen_sess_id=; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=0".to_string()
}

/// Extract the `cowen_sess_id` cookie value from a Cookie header string.
pub fn extract_session_cookie(cookie_header: &str) -> Option<String> {
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some(value) = cookie.strip_prefix("cowen_sess_id=") {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockKeyProvider {
        key: Vec<u8>,
    }
    impl MockKeyProvider {
        fn new(key_str: &str) -> std::sync::Arc<Self> {
            let mut key = vec![0u8; 32];
            let bytes = key_str.as_bytes();
            let len = std::cmp::min(32, bytes.len());
            key[..len].copy_from_slice(&bytes[..len]);
            std::sync::Arc::new(Self { key })
        }
    }
    #[async_trait::async_trait]
    impl crate::jwks::KeyProvider for MockKeyProvider {
        async fn get_active_key(&self) -> cowen_common::CowenResult<(String, Vec<u8>)> {
            Ok(("test-kid".to_string(), self.key.clone()))
        }
        async fn get_key_by_kid(&self, kid: &str) -> cowen_common::CowenResult<Vec<u8>> {
            if kid == "test-kid" {
                Ok(self.key.clone())
            } else {
                Err(cowen_common::CowenError::api("key not found".to_string()))
            }
        }
    }

    #[tokio::test]
    async fn test_session_create_and_validate_roundtrip() {
        let mgr = SessionManager::new(MockKeyProvider::new("test_secret_key_12345")).unwrap();
        let cookie = mgr
            .create_session(
                "org_001".to_string(),
                Some("user_42".to_string()),
                Some("app_7".to_string()),
                "jwt_token_abc".to_string(),
                "test-fp".to_string(),
            )
            .await
            .unwrap();

        assert!(!cookie.is_empty());

        let claims = mgr.validate_session(&cookie, "test-fp").await.unwrap();
        assert_eq!(claims.org_id, "org_001");
        assert_eq!(claims.user_id.as_deref(), Some("user_42"));
        assert_eq!(claims.app_id.as_deref(), Some("app_7"));
        assert_eq!(claims.open_token, "jwt_token_abc");
    }

    #[tokio::test]
    async fn test_session_expiry_detection() {
        let mgr = SessionManager::with_timeouts(MockKeyProvider::new("key"), 1, 3600, 0).unwrap();
        let claims = GatewayClaims::new(
            "org".to_string(),
            None,
            None,
            "tok".to_string(),
            "test-fp".to_string(),
            1,    // 1 second idle timeout
            3600, // 1 hour absolute
        );
        let cookie = mgr.encrypt_claims(&claims).await.unwrap();

        // Should be valid immediately
        assert!(mgr.validate_session(&cookie, "test-fp").await.is_ok());

        // After 2 seconds, idle timeout should have expired
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let result = mgr.validate_session(&cookie, "test-fp").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expired"));
    }

    #[test]
    fn test_session_sliding_window_refresh() {
        // idle=30min, abs=24h, threshold=10min
        let _mgr =
            SessionManager::with_timeouts(MockKeyProvider::new("key"), 1800, 86400, 600).unwrap();
        let claims = GatewayClaims::new(
            "org".to_string(),
            None,
            None,
            "tok".to_string(),
            "test-fp".to_string(),
            1800,
            86400,
        );

        let mgr =
            SessionManager::with_timeouts(MockKeyProvider::new("key"), 1800, 86400, 600).unwrap();
        assert!(!mgr.needs_refresh(&claims));

        // Simulate a claim that's about to expire (idle_exp is 5 minutes from now)
        let now = Utc::now().timestamp();
        let nearly_expired = GatewayClaims {
            idle_exp: now + 300, // 5 min remaining (< 10 min threshold)
            abs_exp: now + 86400,
            iat: now - 1500,
            ..claims
        };
        assert!(mgr.needs_refresh(&nearly_expired));
    }

    #[test]
    fn test_session_refresh_extends_idle() {
        let now = Utc::now().timestamp();
        let claims = GatewayClaims {
            org_id: "org".to_string(),
            user_id: None,
            app_id: None,
            open_token: "tok".to_string(),
            fp: "test-fp".to_string(),
            idle_exp: now + 300,
            abs_exp: now + 86400,
            iat: now,
        };

        let refreshed = claims.refresh_idle(1800);
        assert!(refreshed.idle_exp > claims.idle_exp);
        assert_eq!(refreshed.abs_exp, claims.abs_exp); // Absolute NOT changed
        assert_eq!(refreshed.org_id, claims.org_id);
    }

    #[test]
    fn test_session_refresh_with_new_token() {
        let now = Utc::now().timestamp();
        let claims = GatewayClaims {
            org_id: "org".to_string(),
            user_id: None,
            app_id: None,
            open_token: "old_tok".to_string(),
            fp: "test-fp".to_string(),
            idle_exp: now + 300,
            abs_exp: now + 86400,
            iat: now,
        };

        let refreshed = claims.refresh_with_token("new_tok".to_string(), 1800);
        assert_eq!(refreshed.open_token, "new_tok");
        assert!(refreshed.idle_exp > claims.idle_exp);
    }

    #[tokio::test]
    async fn test_session_tampered_data_rejected() {
        let mgr = SessionManager::new(MockKeyProvider::new("my_secret")).unwrap();
        let cookie = mgr
            .create_session(
                "org".to_string(),
                None,
                None,
                "tok".to_string(),
                "test-fp".to_string(),
            )
            .await
            .unwrap();

        // Tamper with the cookie
        let mut tampered = cookie.clone();
        if let Some(last) = tampered.pop() {
            let replacement = if last == 'A' { 'B' } else { 'A' };
            tampered.push(replacement);
        }

        let result = mgr.validate_session(&tampered, "test-fp").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_session_different_keys_cannot_decrypt() {
        let mgr1 = SessionManager::new(MockKeyProvider::new("secret_one")).unwrap();
        let mgr2 = SessionManager::new(MockKeyProvider::new("secret_two")).unwrap();

        let cookie = mgr1
            .create_session(
                "org".to_string(),
                None,
                None,
                "tok".to_string(),
                "test-fp".to_string(),
            )
            .await
            .unwrap();

        let result = mgr2.validate_session(&cookie, "test-fp").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_set_cookie_header_format() {
        let header = build_set_cookie_header("encrypted_value_123");
        assert_eq!(
            header,
            "cowen_sess_id=encrypted_value_123; HttpOnly; Secure; SameSite=Lax; Path=/"
        );
    }

    #[test]
    fn test_delete_cookie_header_format() {
        let header = build_delete_cookie_header();
        assert!(header.contains("Max-Age=0"));
        assert!(header.contains("cowen_sess_id="));
    }

    #[test]
    fn test_extract_session_cookie_present() {
        let cookie_header = "other=val; cowen_sess_id=abc123; foo=bar";
        let result = extract_session_cookie(cookie_header);
        assert_eq!(result, Some("abc123".to_string()));
    }

    #[test]
    fn test_extract_session_cookie_absent() {
        let cookie_header = "other=val; foo=bar";
        let result = extract_session_cookie(cookie_header);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_session_cookie_empty_value() {
        let cookie_header = "cowen_sess_id=; other=val";
        let result = extract_session_cookie(cookie_header);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_session_cookie_single_cookie() {
        let cookie_header = "cowen_sess_id=single_value";
        let result = extract_session_cookie(cookie_header);
        assert_eq!(result, Some("single_value".to_string()));
    }
}
