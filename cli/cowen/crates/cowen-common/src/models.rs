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

    pub fn extract_jwt_claims(&self) -> Result<serde_json::Value> {
        let parts: Vec<&str> = self.value.split('.').collect();
        if parts.len() != 3 {
            anyhow::bail!("Invalid JWT format");
        }
        
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        let payload = URL_SAFE_NO_PAD.decode(parts[1])?;
        Ok(serde_json::from_slice(&payload)?)
    }

    pub fn extract_identity(&self) -> Option<TokenIdentity> {
        let claims = self.extract_jwt_claims().ok()?;
        
        let user_id = claims.get("userId").or(claims.get("user_id"))?.as_str()?.to_string();
        let org_id = claims.get("orgId").or(claims.get("org_id"))?.as_str()?.to_string();
        let app_id = claims.get("appId").or(claims.get("app_id"))?.as_str()?.to_string();
        
        Some(TokenIdentity { user_id, org_id, app_id })
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
