use crate::pool::TokenPool;
use chrono::Local;
use cowen_common::config::Config;
use cowen_common::status::{AsStatusUI, StatusEntry, StatusLevel};
use cowen_common::CowenResult;

pub enum StoreAppTemplate {
    SecurityVault,
    SuiteAccessToken,
    AppTicket,
    DecryptionKey,
}

impl AsStatusUI for StoreAppTemplate {
    fn ui(&self) -> (String, String) {
        match self {
            Self::SecurityVault => ("Security (Vault)".to_string(), "🛡️".to_string()),
            Self::SuiteAccessToken => ("SuiteAccessToken".to_string(), "🎫".to_string()),
            Self::AppTicket => ("AppTicket".to_string(), "🎫".to_string()),
            Self::DecryptionKey => ("Decryption Key".to_string(), "🔑".to_string()),
        }
    }
}

pub(crate) async fn get_diagnostics_entries(
    pool: &(dyn TokenPool + Send + Sync),
    profile: &str,
    config: &Config,
) -> CowenResult<Vec<StatusEntry>> {
    let mut entries = Vec::new();
    let vault = pool.as_vault();

    entries.push(check_security_vault(vault.clone(), profile, config).await);
    
    if let Some(entry) = check_app_access_token(vault.clone(), config).await {
        entries.push(entry);
    }
    
    entries.push(check_app_ticket(vault.clone(), config).await);
    entries.push(check_decryption_key(vault.clone(), profile, config).await);

    Ok(entries)
}

async fn check_security_vault(vault: std::sync::Arc<dyn cowen_common::vault::Vault>, profile: &str, config: &Config) -> StatusEntry {
    let mut missing = Vec::new();
    let has_secret =
        vault.get_secret(profile, "app_secret").await.is_ok() || !config.app_secret.is_empty();
    let has_cert =
        vault.get_secret(profile, "certificate").await.is_ok() || !config.certificate.is_empty();
    let has_encrypt_key =
        vault.get_secret(profile, "encrypt_key").await.is_ok() || !config.encrypt_key.is_empty();

    if !has_secret && !has_cert {
        missing.push("app_secret or certificate".to_string());
    }
    if !has_encrypt_key {
        missing.push("encrypt_key".to_string());
    }

    let (sec_level, sec_msg) = if missing.is_empty() {
        if !has_cert {
            (StatusLevel::OK, "All core secrets (AppSecret + EncryptKey) are securely stored. (Certificate optional)".to_string())
        } else {
            (
                StatusLevel::OK,
                "All core secrets are securely stored.".to_string(),
            )
        }
    } else {
        (
            StatusLevel::ERROR,
            format!("Missing: {}", missing.join(", ")),
        )
    };

    StatusEntry::new(StoreAppTemplate::SecurityVault, sec_level, sec_msg).with_reason(
        if sec_level == StatusLevel::ERROR {
            Some("缺少必要凭据，可能导致 API 调用或解密失败。".to_string())
        } else {
            None
        },
    )
}

async fn check_app_access_token(vault: std::sync::Arc<dyn cowen_common::vault::Vault>, config: &Config) -> Option<StatusEntry> {
    if let Ok(token) = vault.get_app_access_token(&config.app_key).await {
        let is_expired = token.is_expired();
        let mut details = vec![];
        if let Some(identity) = token.extract_identity() {
            details.push(format!("User ID: {}", identity.user_id));
            details.push(format!("Org ID:  {}", identity.org_id));
            details.push(format!("App ID:  {}", identity.app_id));
        }

        return Some(StatusEntry::new(
            StoreAppTemplate::SuiteAccessToken,
            if is_expired {
                StatusLevel::ERROR
            } else {
                StatusLevel::OK
            },
            format!(
                "[{}] (Expires: {})",
                if is_expired { "EXPIRED" } else { "VALID" },
                token
                    .expires_at
                    .with_timezone(&Local)
                    .format("%Y-%m-%d %H:%M:%S")
            ),
        )
        .with_reason(if is_expired {
            Some("套件令牌已过期。".to_string())
        } else {
            None
        })
        .with_details(details));
    }
    None
}

async fn check_app_ticket(vault: std::sync::Arc<dyn cowen_common::vault::Vault>, config: &Config) -> StatusEntry {
    match vault.get_app_ticket(&config.app_key).await {
        Ok(ticket) => {
            let created_at = ticket.created_at;
            StatusEntry::new(
                StoreAppTemplate::AppTicket,
                StatusLevel::OK,
                format!(
                    "[CACHED] (Received: {})",
                    created_at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S")
                ),
            )
        }
        _ => {
            StatusEntry::new(
                StoreAppTemplate::AppTicket,
                StatusLevel::NONE,
                "[NONE] (等待 Daemon 接收推送)".to_string(),
            )
        }
    }
}

async fn check_decryption_key(vault: std::sync::Arc<dyn cowen_common::vault::Vault>, profile: &str, config: &Config) -> StatusEntry {
    let app_secret_val = vault
        .get_secret(profile, "app_secret")
        .await
        .unwrap_or_else(|_| config.app_secret.clone());
    let encrypt_key_val = vault
        .get_secret(profile, "encrypt_key")
        .await
        .unwrap_or_else(|_| config.encrypt_key.clone());

    let (dk_level, dk_msg) =
        crate::provider::utils::check_decryption_key_format(&encrypt_key_val, &app_secret_val);

    StatusEntry::new(
        StoreAppTemplate::DecryptionKey,
        dk_level,
        dk_msg,
    )
}
