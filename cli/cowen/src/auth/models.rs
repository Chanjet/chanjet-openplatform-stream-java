use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    #[serde(rename = "access_token")]
    pub value: String,
    pub expires_at: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

impl Token {
    pub fn is_expired(&self) -> bool {
        self.is_expired_with_buffer(Duration::seconds(300))
    }

    pub fn is_expired_with_buffer(&self, min_buffer: Duration) -> bool {
        let now = Utc::now();
        let expiry = self.real_expires_at();
        
        let total_lifetime = expiry.signed_duration_since(self.created_at);
        
        // No buffer for very short-lived tokens (likely tests)
        if total_lifetime < Duration::minutes(10) {
            return now >= expiry;
        }

        // Safety buffer: 10% of total lifetime, but at least min_buffer
        let total_secs = total_lifetime.num_seconds() as f64;
        let buffer_secs = (total_secs * 0.1).max(min_buffer.num_seconds() as f64) as i64;
        let buffer = Duration::seconds(buffer_secs);

        now + buffer > expiry
    }

    pub fn real_expires_at(&self) -> DateTime<Utc> {
        self.extract_jwt_exp().unwrap_or(self.expires_at)
    }

    fn extract_jwt_exp(&self) -> Option<DateTime<Utc>> {
        self.extract_jwt_claims().ok().and_then(|v| {
            let exp = v.get("exp")?.as_i64()?;
            DateTime::from_timestamp(exp, 0)
        })
    }

    pub fn extract_jwt_claims(&self) -> Result<serde_json::Value> {
        let parts: Vec<&str> = self.value.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow::anyhow!("Invalid JWT format"));
        }

        let payload_b64 = parts[1];
        
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        let payload_json = URL_SAFE_NO_PAD.decode(payload_b64)
            .map_err(|e| anyhow::anyhow!("Base64 decode error: {}", e))?;
        
        let v: serde_json::Value = serde_json::from_slice(&payload_json)
            .map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))?;
        
        Ok(v)
    }

    pub fn extract_identity(&self) -> Option<TokenIdentity> {
        let claims = self.extract_jwt_claims().ok()?;
        
        let user_id = claims.get("userId").or(claims.get("user_id"))?.as_str()?.to_string();
        let org_id = claims.get("orgId").or(claims.get("org_id"))?.as_str()?.to_string();
        let app_id = claims.get("appId").or(claims.get("app_id"))?.as_str()?.to_string();
        
        Some(TokenIdentity { user_id, org_id, app_id })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenIdentity {
    pub user_id: String,
    pub org_id: String,
    pub app_id: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    #[serde(rename = "app_ticket")]
    pub value: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthMode {
    /// 传统自建应用模式 (Ticket-based)
    SelfBuilt,
    /// 官方标准 OAuth2 模式 (Fixed AppKey, Interactive flow)
    Oauth2,
    /// 商店应用/边车模式 (OAuth2-based Sidecar, requires AppKey/Secret, no interactive flow)
    StoreApp,
}

impl Default for AuthMode {
    fn default() -> Self {
        Self::Oauth2
    }
}

/// 内置的 OAuth2 Client ID (AppKey)
pub const BUILTIN_CLIENT_ID: &str = env!("BUILTIN_CLIENT_ID");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    pub refresh_expires_at: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

impl OAuth2TokenPair {
    pub fn is_expired_with_buffer(&self, min_buffer: Duration) -> bool {
        let now = Utc::now();
        let total_lifetime = self.expires_at.signed_duration_since(self.created_at);
        
        if total_lifetime < Duration::minutes(10) {
            return now >= self.expires_at;
        }

        let total_secs = total_lifetime.num_seconds() as f64;
        let buffer_secs = (total_secs * 0.1).max(min_buffer.num_seconds() as f64) as i64;
        let buffer = Duration::seconds(buffer_secs);

        now + buffer > self.expires_at
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub profile: String,
    pub code_verifier: String,
    pub state: String,
    pub redirect_uri: String,
    pub redirect_port: u16,
    pub expires_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_oauth2_token_pair_serialization() {
        let now = Utc::now();
        let pair = OAuth2TokenPair {
            access_token: "at".to_string(),
            refresh_token: "rt".to_string(),
            expires_at: now + Duration::hours(2),
            refresh_expires_at: now + Duration::days(7),
            created_at: now,
        };
        let json = serde_json::to_string(&pair).unwrap();
        let decoded: OAuth2TokenPair = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.access_token, "at");
        assert_eq!(decoded.refresh_token, "rt");
    }

    #[test]
    fn test_auth_session_serialization() {
        let now = Utc::now();
        let session = AuthSession {
            profile: "default".to_string(),
            code_verifier: "cv".to_string(),
            state: "st".to_string(),
            redirect_uri: "http://localhost:8080".to_string(),
            redirect_port: 8080,
            expires_at: now + Duration::minutes(5),
        };
        let json = serde_json::to_string(&session).unwrap();
        let decoded: AuthSession = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.profile, "default");
        assert_eq!(decoded.code_verifier, "cv");
    }

    #[test]
    fn test_authmode_serialization() {
        assert_eq!(serde_json::to_string(&AuthMode::SelfBuilt).unwrap(), "\"self-built\"");
        assert_eq!(serde_json::to_string(&AuthMode::Oauth2).unwrap(), "\"oauth2\"");
        assert_eq!(serde_json::to_string(&AuthMode::StoreApp).unwrap(), "\"store-app\"");
        
        let mode: AuthMode = serde_json::from_str("\"self-built\"").unwrap();
        assert_eq!(mode, AuthMode::SelfBuilt);
        
        let mode: AuthMode = serde_json::from_str("\"oauth2\"").unwrap();
        assert_eq!(mode, AuthMode::Oauth2);

        let mode: AuthMode = serde_json::from_str("\"store-app\"").unwrap();
        assert_eq!(mode, AuthMode::StoreApp);
    }

    #[test]
    fn test_is_expired_with_10_percent_buffer() {
        let now = Utc::now();
        
        // 1. Long lived token (24h = 1440 min)
        // 10% buffer is 144 min (2.4h)
        let token_long_safe = Token {
            value: "mock".to_string(),
            created_at: now - Duration::hours(10), // Used 10h
            expires_at: now + Duration::hours(14) + Duration::minutes(5), // Remaining 14h5m. 14h5m > 2.4h. OK.
        };
        assert!(!token_long_safe.is_expired());

        let token_long_near = Token {
            value: "mock".to_string(),
            created_at: now - Duration::hours(22), // Used 22h
            expires_at: now + Duration::hours(2) - Duration::minutes(1), // Remaining 1h59m. 1h59m < 2.4h. Expired.
        };
        assert!(token_long_near.is_expired());

        // 2. Short lived token (10 min)
        // 10% is 1 min. BUT safety buffer is 5 min (300s).
        let token_short_safe = Token {
            value: "mock".to_string(),
            created_at: now - Duration::minutes(2),
            expires_at: now + Duration::minutes(6), // Remaining 6m. 6m > 5m. OK.
        };
        assert!(!token_short_safe.is_expired());

        let token_short_buffer_hit = Token {
            value: "mock".to_string(),
            created_at: now - Duration::minutes(6),
            expires_at: now + Duration::minutes(4), // Remaining 4m. 4m < 5m. Expired.
        };
        assert!(token_short_buffer_hit.is_expired());

        // 3. Buffer boundary check (Remaining 5m 1s should be safe)
        let token_just_safe = Token {
            value: "mock".to_string(),
            created_at: now - Duration::minutes(10),
            expires_at: now + Duration::minutes(5) + Duration::seconds(1), 
        };
        assert!(!token_just_safe.is_expired());
    }

    #[test]
    fn test_extract_jwt_exp() {
        // Mock a JWT payload: {"exp": 1711526400} (2024-03-27 08:00:00 UTC)
        let payload = r#"{"exp": 1711526400}"#;
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        let encoded_payload = URL_SAFE_NO_PAD.encode(payload);
        let mock_token_value = format!("header.{}.signature", encoded_payload);

        let token = Token {
            value: mock_token_value,
            expires_at: Utc.timestamp_opt(0, 0).unwrap(), // arbitrary old date
            created_at: Utc::now(),
        };

        let real_expiry = token.extract_jwt_exp().expect("Should extract exp");
        assert_eq!(real_expiry.timestamp(), 1711526400);
        assert_eq!(token.real_expires_at().timestamp(), 1711526400);
    }

    #[test]
    fn test_extract_identity() {
        let payload = r#"{"userId": "U123", "orgId": "O456", "appId": "A789"}"#;
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        let encoded_payload = URL_SAFE_NO_PAD.encode(payload);
        let mock_token_value = format!("header.{}.signature", encoded_payload);

        let token = Token {
            value: mock_token_value,
            expires_at: Utc::now(),
            created_at: Utc::now(),
        };

        let identity = token.extract_identity().expect("Should extract identity");
        assert_eq!(identity.user_id, "U123");
        assert_eq!(identity.org_id, "O456");
        assert_eq!(identity.app_id, "A789");
    }

    #[test]
    fn test_serialization_preserves_value() {
        let ticket = Ticket {
            value: "original_ticket_value".to_string(),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&ticket).unwrap();
        // If masked, this would fail
        assert!(json.contains("original_ticket_value"), "Serialization should not mask the value: {}", json);

        let token = Token {
            value: "original_token_value".to_string(),
            expires_at: Utc::now(),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&token).unwrap();
        assert!(json.contains("original_token_value"), "Serialization should not mask the value: {}", json);
    }
}
