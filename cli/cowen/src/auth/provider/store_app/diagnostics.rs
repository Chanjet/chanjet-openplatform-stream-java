use crate::core::config::Config;
use anyhow::Result;
use chrono::{Utc, Local};
use crate::auth::pool::TokenPool;
use crate::core::status::{StatusEntry, StatusLevel, AsStatusUI};

pub enum StoreAppTemplate {
    SecurityVault,
    SuiteAccessToken,
    AppTicket,
}

impl AsStatusUI for StoreAppTemplate {
    fn ui(&self) -> (String, String) {
        match self {
            Self::SecurityVault => ("Security (Vault)".to_string(), "🛡️".to_string()),
            Self::SuiteAccessToken => ("SuiteAccessToken".to_string(), "🎫".to_string()),
            Self::AppTicket => ("AppTicket".to_string(), "🎫".to_string()),
        }
    }
}

pub(crate) async fn get_diagnostics_entries(
    pool: &(dyn TokenPool + Send + Sync),
    profile: &str,
    config: &Config,
) -> Result<Vec<StatusEntry>> {
    let mut entries = Vec::new();
    let vault = pool.as_vault();

    // 1. Security Check
    let mut missing = Vec::new();
    if vault.get_secret(profile, "app_secret").await.is_err() { missing.push("app_secret".to_string()); }
    if vault.get_secret(profile, "certificate").await.is_err() { missing.push("certificate".to_string()); }
    if vault.get_secret(profile, "encrypt_key").await.is_err() { missing.push("encrypt_key".to_string()); }

    let (sec_level, sec_msg) = if missing.is_empty() {
        (StatusLevel::OK, "All core secrets are securely stored.".to_string())
    } else {
        (StatusLevel::ERROR, format!("Missing: {}", missing.join(", ")))
    };

    entries.push(StatusEntry::new(StoreAppTemplate::SecurityVault, sec_level, sec_msg)
        .with_reason(if sec_level == StatusLevel::ERROR { Some("缺少必要凭据，可能导致 API 调用或解密失败。".to_string()) } else { None }));

    let global_profile = format!("app:{}", config.app_key);

    // 2. App Access Token (Global)
    if let Ok(token) = pool.get_app_access_token(&config.app_key).await {
        let is_expired = token.is_expired();
        let mut details = vec![];
        if let Some(identity) = token.extract_identity() {
            details.push(format!("App ID:  {}", identity.app_id));
        }

        entries.push(StatusEntry::new(StoreAppTemplate::SuiteAccessToken, if is_expired { StatusLevel::ERROR } else { StatusLevel::OK }, format!("[{}] (Expires: {})", 
                if is_expired { "EXPIRED" } else { "VALID" },
                token.expires_at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S")))
            .with_reason(if is_expired { Some("套件令牌已过期。".to_string()) } else { None })
            .with_details(details));
    }

    // 3. AppTicket (Global)
    if let Ok(ts_str) = vault.get(&global_profile, "app_ticket_created").await {
        let created_at = chrono::DateTime::parse_from_rfc3339(&ts_str).map(|dt| dt.with_timezone(&Utc)).unwrap_or(Utc::now());
        entries.push(StatusEntry::new(StoreAppTemplate::AppTicket, StatusLevel::OK, format!("[CACHED] (Received: {})", created_at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S"))));
    } else {
        entries.push(StatusEntry::new(StoreAppTemplate::AppTicket, StatusLevel::NONE, "[NONE] (等待 Daemon 接收推送)".to_string()));
    }

    Ok(entries)
}
