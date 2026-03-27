use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    #[serde(rename = "access_token")]
    pub value: String,
    pub expires_at: DateTime<Utc>,
}

impl Token {
    pub fn is_expired(&self) -> bool {
        // 5-minute buffer
        Utc::now() + Duration::minutes(5) > self.expires_at
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    #[serde(rename = "app_ticket")]
    pub value: String,
    pub created_at: DateTime<Utc>,
}

