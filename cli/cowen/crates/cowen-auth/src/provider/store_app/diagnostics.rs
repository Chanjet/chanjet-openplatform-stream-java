use crate::pool::TokenPool;
use chrono::Local;
use cowen_common::config::Config;
use cowen_common::CowenResult;
use cowen_common::status::{AsStatusUI, StatusEntry, StatusLevel};

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

    // 1. Security Check
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

    entries.push(
        StatusEntry::new(StoreAppTemplate::SecurityVault, sec_level, sec_msg).with_reason(
            if sec_level == StatusLevel::ERROR {
                Some("缺少必要凭据，可能导致 API 调用或解密失败。".to_string())
            } else {
                None
            },
        ),
    );

    // 2. App Access Token (Global)
    if let Ok(token) = vault.get_app_access_token(&config.app_key).await {
        let is_expired = token.is_expired();
        let mut details = vec![];
        if let Some(identity) = token.extract_identity() {
            details.push(format!("User ID: {}", identity.user_id));
            details.push(format!("Org ID:  {}", identity.org_id));
            details.push(format!("App ID:  {}", identity.app_id));
        }

        entries.push(
            StatusEntry::new(
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
            .with_details(details),
        );
    }

    // 3. AppTicket (Global)
    match vault.get_app_ticket(&config.app_key).await { Ok(ticket) => {
        let created_at = ticket.created_at;
        entries.push(StatusEntry::new(
            StoreAppTemplate::AppTicket,
            StatusLevel::OK,
            format!(
                "[CACHED] (Received: {})",
                created_at.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S")
            ),
        ));
    } _ => {
        entries.push(StatusEntry::new(
            StoreAppTemplate::AppTicket,
            StatusLevel::NONE,
            "[NONE] (等待 Daemon 接收推送)".to_string(),
        ));
    }}

    // 4. Decryption Key (Global / Profile)
    let app_secret_val = vault.get_secret(profile, "app_secret").await.unwrap_or_else(|_| config.app_secret.clone());
    let encrypt_key_val = vault.get_secret(profile, "encrypt_key").await.unwrap_or_else(|_| config.encrypt_key.clone());
    
    let decrypt_key_raw = if !encrypt_key_val.is_empty() {
        &encrypt_key_val
    } else {
        &app_secret_val
    };
    let decrypt_key = cowen_common::utils::sanitize_credential(decrypt_key_raw);

    let (dk_level, dk_msg) = if decrypt_key.is_empty() {
        (
            StatusLevel::ERROR,
            "Decryption key is missing (both encrypt_key and app_secret are empty)".to_string(),
        )
    } else {
        let key_len = if decrypt_key.len() == 32 {
            if decrypt_key.len().is_multiple_of(2) && decrypt_key.chars().all(|c| c.is_ascii_hexdigit()) {
                16
            } else {
                32
            }
        } else {
            decrypt_key.len()
        };

        if key_len != 16 {
            (
                StatusLevel::ERROR,
                format!("Decryption key trimmed length {} is invalid. Must be 16 bytes or 32-character hex", decrypt_key.len()),
            )
        } else {
            (
                StatusLevel::OK,
                "Decryption key format is valid (16 bytes or 32-character hex)".to_string(),
            )
        }
    };

    entries.push(
        StatusEntry::new(StoreAppTemplate::DecryptionKey, dk_level, dk_msg)
    );

    Ok(entries)
}
