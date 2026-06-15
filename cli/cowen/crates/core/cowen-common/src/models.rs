use crate::{CowenError, CowenResult};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    #[serde(rename = "access_token")]
    pub value: String,
    pub expires_at: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenIdentity {
    pub user_id: String,
    pub org_id: String,
    pub app_id: String,
}

impl Token {
    pub fn is_expired(&self) -> bool {
        self.is_expired_with_buffer(Duration::seconds(300))
    }

    pub fn is_expired_with_buffer(&self, min_buffer: Duration) -> bool {
        let now = Utc::now();
        let expiry = self.real_expires_at();
        let total_lifetime = expiry.signed_duration_since(self.created_at);
        if total_lifetime < Duration::minutes(10) {
            return now >= expiry;
        }
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

    pub fn extract_jwt_claims(&self) -> CowenResult<serde_json::Value> {
        let parts: Vec<&str> = self.value.split('.').collect();
        if parts.len() != 3 {
            return Err(CowenError::Security("Invalid JWT format".to_string()));
        }

        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
        let payload = URL_SAFE_NO_PAD.decode(parts[1])?;
        Ok(serde_json::from_slice(&payload)?)
    }

    pub fn extract_identity(&self) -> Option<TokenIdentity> {
        let claims = match self.extract_jwt_claims() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(target: "sys", error = %e, "Failed to extract JWT claims");
                return None;
            }
        };

        let user_id = match claims
            .get("userId")
            .or(claims.get("user_id"))
            .and_then(|v| v.as_str())
        {
            Some(v) => v.to_string(),
            None => {
                tracing::warn!(target: "sys", "Failed to extract user_id from token");
                return None;
            }
        };
        let org_id = match claims
            .get("orgId")
            .or(claims.get("org_id"))
            .and_then(|v| v.as_str())
        {
            Some(v) => v.to_string(),
            None => {
                tracing::warn!(target: "sys", "Failed to extract org_id from token");
                return None;
            }
        };
        let app_id = match claims
            .get("appId")
            .or(claims.get("app_id"))
            .and_then(|v| v.as_str())
        {
            Some(v) => v.to_string(),
            None => {
                tracing::warn!(target: "sys", "Failed to extract app_id from token");
                return None;
            }
        };

        Some(TokenIdentity {
            user_id,
            org_id,
            app_id,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    pub value: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum AuthMode {
    #[default]
    #[serde(rename = "oauth2")]
    Oauth2,
    #[serde(rename = "self-built")]
    SelfBuilt,
    #[serde(rename = "store-app")]
    StoreApp,
}

impl std::str::FromStr for AuthMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "oauth2" => Ok(AuthMode::Oauth2),
            "self_built" | "self-built" => Ok(AuthMode::SelfBuilt),
            "store_app" | "store-app" => Ok(AuthMode::StoreApp),
            _ => Err(format!(
                "Invalid app-mode: '{}'. Supported: self_built, oauth2, store_app",
                s
            )),
        }
    }
}

impl std::fmt::Display for AuthMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthMode::Oauth2 => write!(f, "oauth2"),
            AuthMode::SelfBuilt => write!(f, "self-built"),
            AuthMode::StoreApp => write!(f, "store-app"),
        }
    }
}

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Item {
    pub profile: String,
    pub key: String,
    pub value: String,
    pub version: u64,
    pub updated_at: i64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub profile: String,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: serde_json::Value,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct DlqMessage {
    pub id: Option<i64>,
    pub profile: String,
    pub topic: String,
    pub payload: String,
    pub retry_count: i32,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub trait StoreItem: Serialize + for<'de> Deserialize<'de> + Send + Sync {
    fn key_prefix() -> &'static str;
}

impl StoreItem for Token {
    fn key_prefix() -> &'static str {
        "tokens"
    }
}

impl StoreItem for Ticket {
    fn key_prefix() -> &'static str {
        "tickets"
    }
}

impl StoreItem for DlqMessage {
    fn key_prefix() -> &'static str {
        "dlq"
    }
}

impl StoreItem for Item {
    fn key_prefix() -> &'static str {
        "cfg"
    }
}

impl StoreItem for AuditEntry {
    fn key_prefix() -> &'static str {
        "audit"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_mode_from_str_valid_canonical() {
        assert_eq!("oauth2".parse::<AuthMode>().unwrap(), AuthMode::Oauth2);
        assert_eq!(
            "self_built".parse::<AuthMode>().unwrap(),
            AuthMode::SelfBuilt
        );
        assert_eq!("store_app".parse::<AuthMode>().unwrap(), AuthMode::StoreApp);
    }

    #[test]
    fn auth_mode_from_str_valid_kebab_aliases() {
        assert_eq!(
            "self-built".parse::<AuthMode>().unwrap(),
            AuthMode::SelfBuilt
        );
        assert_eq!("store-app".parse::<AuthMode>().unwrap(), AuthMode::StoreApp);
    }

    #[test]
    fn auth_mode_from_str_invalid() {
        let err = "unknown".parse::<AuthMode>().unwrap_err();
        assert!(err.contains("Invalid app-mode"));
        assert!(err.contains("unknown"));
    }

    #[test]
    fn auth_mode_display_roundtrip() {
        let modes = [AuthMode::Oauth2, AuthMode::SelfBuilt, AuthMode::StoreApp];
        for mode in modes {
            let s = mode.to_string();
            let parsed: AuthMode = s.parse().unwrap();
            assert_eq!(parsed, mode);
        }
    }

    #[test]
    fn auth_mode_default_is_oauth2() {
        assert_eq!(AuthMode::default(), AuthMode::Oauth2);
    }
}
