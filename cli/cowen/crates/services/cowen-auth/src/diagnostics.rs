use anyhow::Result;
use async_trait::async_trait;
use cowen_doctor::{
    DiagnosticRegistration, DiagnosticResult, DiagnosticStatus, DiagnosticTask, DoctorContext,
};
use std::time::Instant;

pub struct CredentialsCheck;

#[async_trait]
impl DiagnosticTask for CredentialsCheck {
    fn name(&self) -> &str {
        "凭据与认证"
    }
    async fn run(&self, ctx: &DoctorContext) -> Result<DiagnosticResult> {
        let start = Instant::now();

        let app_secret_res = ctx.vault.get_secret(&ctx.profile, "app_secret").await;
        let encrypt_key_res = ctx.vault.get_secret(&ctx.profile, "encrypt_key").await;

        let app_secret = app_secret_res.unwrap_or_default();
        let encrypt_key = encrypt_key_res.unwrap_or_default();

        let status = if ctx.config.app_mode == cowen_common::models::AuthMode::SelfBuilt
            || ctx.config.app_mode == cowen_common::models::AuthMode::StoreApp
        {
            let decrypt_key_raw = if !encrypt_key.is_empty() {
                &encrypt_key
            } else {
                &app_secret
            };
            let decrypt_key = decrypt_key_raw.trim();

            if decrypt_key.is_empty() {
                DiagnosticStatus::Error(
                    "缺少解密密钥 (App Secret 或 Encrypt Key 均为空)".to_string(),
                )
            } else {
                let key_len = if decrypt_key.len() == 32 {
                    if hex::decode(decrypt_key).is_ok() {
                        16
                    } else {
                        32
                    }
                } else {
                    decrypt_key.len()
                };

                if key_len != 16 {
                    DiagnosticStatus::Error(format!(
                        "解密密钥不合规：必须为16字节或32字符Hex，当前 trimmed 长度为 {}",
                        decrypt_key.len()
                    ))
                } else {
                    DiagnosticStatus::Ok
                }
            }
        } else {
            // OAuth2 mode: No app_secret needed. Uses built-in client ID with PKCE.
            // Credential validation is handled by the OAuth2 provider during token operations.
            DiagnosticStatus::Ok
        };

        Ok(DiagnosticResult {
            name: self.name().to_string(),
            status,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

inventory::submit! { DiagnosticRegistration { builder: || Box::new(CredentialsCheck) } }

#[cfg(test)]
mod tests {
    use super::*;
    use cowen_common::config::Config;
    use cowen_config::ConfigManager;

    #[tokio::test]
    async fn test_credentials_check_green() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut cfg_mgr = ConfigManager::new().unwrap();
        cfg_mgr.app_dir = temp_dir.path().to_path_buf();

        let app_cfg = cowen_common::config::AppConfig::default();
        let vault = cowen_store::create_vault(&app_cfg, temp_dir.path(), "test_fingerprint")
            .await
            .unwrap();

        // 1. In SelfBuilt mode, set a valid 16-byte app_secret in vault
        let config = Config {
            app_mode: cowen_common::models::AuthMode::SelfBuilt,
            ..Config::default_with_profile("test")
        };
        vault
            .set_secret("test", "app_secret", "1234567890123456")
            .await
            .unwrap();

        let ctx = DoctorContext {
            profile: "test".to_string(),
            config,
            verbose: false,
            fix: false,
            vault,
            cfg_mgr,
        };

        let checker = CredentialsCheck;
        let res = checker.run(&ctx).await.unwrap();

        assert!(
            matches!(res.status, DiagnosticStatus::Ok),
            "Expected DiagnosticStatus::Ok, got {:?}",
            res.status
        );
    }
}
