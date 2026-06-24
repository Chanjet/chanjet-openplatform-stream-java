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

async fn check_security_vault(
    vault: std::sync::Arc<dyn cowen_common::vault::Vault>,
    profile: &str,
    config: &Config,
) -> StatusEntry {
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

async fn check_app_access_token(
    vault: std::sync::Arc<dyn cowen_common::vault::Vault>,
    config: &Config,
) -> Option<StatusEntry> {
    if let Ok(token) = vault.get_app_access_token(&config.app_key).await {
        let is_expired = token.is_expired();
        let mut details = vec![];
        if let Some(identity) = token.extract_identity() {
            details.push(format!("User ID: {}", identity.user_id));
            details.push(format!("Org ID:  {}", identity.org_id));
            details.push(format!("App ID:  {}", identity.app_id));
        }

        return Some(
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
    None
}

async fn check_app_ticket(
    vault: std::sync::Arc<dyn cowen_common::vault::Vault>,
    config: &Config,
) -> StatusEntry {
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
        _ => StatusEntry::new(
            StoreAppTemplate::AppTicket,
            StatusLevel::NONE,
            "[NONE] (等待 Daemon 接收推送)".to_string(),
        ),
    }
}

async fn check_decryption_key(
    vault: std::sync::Arc<dyn cowen_common::vault::Vault>,
    profile: &str,
    config: &Config,
) -> StatusEntry {
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

    StatusEntry::new(StoreAppTemplate::DecryptionKey, dk_level, dk_msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::VaultTokenPool;
    use cowen_common::config::Config;
    use cowen_common::models::{AuthMode, Ticket, Token};
    use cowen_common::status::StatusLevel;
    use std::sync::Arc;

    #[test]
    fn test_store_app_template_ui() {
        let (name, emoji) = StoreAppTemplate::SecurityVault.ui();
        assert_eq!(name, "Security (Vault)");
        assert_eq!(emoji, "🛡️");

        let (name, emoji) = StoreAppTemplate::SuiteAccessToken.ui();
        assert_eq!(name, "SuiteAccessToken");
        assert_eq!(emoji, "🎫");

        let (name, emoji) = StoreAppTemplate::AppTicket.ui();
        assert_eq!(name, "AppTicket");
        assert_eq!(emoji, "🎫");

        let (name, emoji) = StoreAppTemplate::DecryptionKey.ui();
        assert_eq!(name, "Decryption Key");
        assert_eq!(emoji, "🔑");
    }

    #[tokio::test]
    async fn test_diagnostics_all_scenarios() {
        let temp_dir = tempfile::tempdir().unwrap();
        let app_cfg = cowen_common::config::AppConfig::default();
        let vault = cowen_store::create_vault(&app_cfg, temp_dir.path(), "test_fingerprint")
            .await
            .unwrap();

        let pool = Arc::new(VaultTokenPool::new(vault.clone()));
        let profile = "test_profile";

        // Scenario 1: Missing secrets (no app_secret, no encrypt_key)
        let config = Config {
            app_key: "test_app_key".to_string(),
            app_secret: "".to_string(),
            encrypt_key: "".to_string(),
            app_mode: AuthMode::StoreApp,
            ..Config::default_with_profile(profile)
        };

        let entries = get_diagnostics_entries(pool.as_ref(), profile, &config)
            .await
            .unwrap();

        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].level, StatusLevel::ERROR); // SecurityVault is ERROR
        assert!(entries[0].message.contains("Missing"));
        assert_eq!(entries[1].level, StatusLevel::NONE); // AppTicket is NONE
        assert_eq!(entries[2].level, StatusLevel::ERROR); // DecryptionKey is ERROR (because encrypt_key is empty/invalid)

        // Scenario 2: Save secrets in vault, add a valid access token and ticket
        vault
            .set_secret(profile, "app_secret", "1234567890123456")
            .await
            .unwrap();
        vault
            .set_secret(profile, "encrypt_key", "1234567890123456")
            .await
            .unwrap();

        let jwt_val =
            "eyJhbGciOiJub25lIn0.eyJ1c2VySWQiOiJVMTIzIiwib3JnSWQiOiJPNDU2IiwiYXBwSWQiOiJBNzg5In0."
                .to_string();
        let token = Token {
            value: jwt_val,
            expires_at: chrono::Utc::now() + chrono::Duration::hours(1),
            created_at: chrono::Utc::now(),
        };
        vault
            .save_app_access_token("test_app_key", token)
            .await
            .unwrap();

        let ticket = Ticket {
            value: "test_ticket".to_string(),
            created_at: chrono::Utc::now(),
        };
        vault.save_app_ticket("test_app_key", ticket).await.unwrap();

        let config = Config {
            app_key: "test_app_key".to_string(),
            app_secret: "".to_string(),
            encrypt_key: "".to_string(),
            app_mode: AuthMode::StoreApp,
            ..Config::default_with_profile(profile)
        };

        let entries = get_diagnostics_entries(pool.as_ref(), profile, &config)
            .await
            .unwrap();

        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].level, StatusLevel::OK); // SecurityVault OK
        assert_eq!(entries[1].level, StatusLevel::OK); // SuiteAccessToken OK
        assert!(entries[1]
            .details
            .iter()
            .any(|d| d.contains("User ID: U123")));
        assert!(entries[1]
            .details
            .iter()
            .any(|d| d.contains("Org ID:  O456")));
        assert!(entries[1]
            .details
            .iter()
            .any(|d| d.contains("App ID:  A789")));
        assert_eq!(entries[2].level, StatusLevel::OK); // AppTicket OK
        assert_eq!(entries[3].level, StatusLevel::OK); // DecryptionKey OK

        // Scenario 3: Expired access token
        let expired_token = Token {
            value: "eyJhbGciOiJub25lIn0.eyJ1c2VySWQiOiJVMTIzIiwib3JnSWQiOiJPNDU2IiwiYXBwSWQiOiJBNzg5In0.".to_string(),
            expires_at: chrono::Utc::now() - chrono::Duration::hours(2),
            created_at: chrono::Utc::now() - chrono::Duration::hours(3),
        };
        vault
            .save_app_access_token("test_app_key", expired_token)
            .await
            .unwrap();

        let entries = get_diagnostics_entries(pool.as_ref(), profile, &config)
            .await
            .unwrap();
        assert_eq!(entries[1].level, StatusLevel::ERROR); // SuiteAccessToken is now EXPIRED (ERROR)
    }
}
