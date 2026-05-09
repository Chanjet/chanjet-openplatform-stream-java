use cowen_common::obfs;
pub use cowen_common::models::{Token, Ticket, AuthMode, TokenIdentity, AuthSession};
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};

pub use cowen_common::config::BUILTIN_CLIENT_ID;

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
