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
        let expiry = self.real_expires_at();
        Utc::now() + Duration::minutes(5) > expiry
    }

    pub fn real_expires_at(&self) -> DateTime<Utc> {
        self.extract_jwt_exp().unwrap_or(self.expires_at)
    }

    fn extract_jwt_exp(&self) -> Option<DateTime<Utc>> {
        let parts: Vec<&str> = self.value.split('.').collect();
        if parts.len() != 3 {
            return None;
        }

        let payload_b64 = parts[1];
        
        use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
        let payload_json = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
        
        let v: serde_json::Value = serde_json::from_slice(&payload_json).ok()?;
        let exp = v.get("exp")?.as_i64()?;
        
        DateTime::from_timestamp(exp, 0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    #[serde(rename = "app_ticket")]
    pub value: String,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

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
        };

        let real_expiry = token.extract_jwt_exp().expect("Should extract exp");
        assert_eq!(real_expiry.timestamp(), 1711526400);
        assert_eq!(token.real_expires_at().timestamp(), 1711526400);
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
        };
        let json = serde_json::to_string(&token).unwrap();
        assert!(json.contains("original_token_value"), "Serialization should not mask the value: {}", json);
    }
}
