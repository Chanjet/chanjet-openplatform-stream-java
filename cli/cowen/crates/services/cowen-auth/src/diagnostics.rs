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

        let auth_cli = crate::create_auth_client_with_vault(ctx.vault.clone());
        let status = match auth_cli
            .provider(&ctx.config.app_mode)
            .check_credentials(&*ctx.vault, &ctx.profile)
            .await
        {
            Ok(s) => s,
            Err(e) => DiagnosticStatus::Error(e),
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

    #[tokio::test]
    async fn test_credentials_check_error() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut cfg_mgr = ConfigManager::new().unwrap();
        cfg_mgr.app_dir = temp_dir.path().to_path_buf();

        let app_cfg = cowen_common::config::AppConfig::default();
        let vault = cowen_store::create_vault(&app_cfg, temp_dir.path(), "test_fingerprint")
            .await
            .unwrap();

        // 1. In SelfBuilt mode, BUT DO NOT set app_secret
        let config = Config {
            app_mode: cowen_common::models::AuthMode::SelfBuilt,
            ..Config::default_with_profile("test")
        };

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
            matches!(res.status, DiagnosticStatus::Error(_)),
            "Expected DiagnosticStatus::Error, got {:?}",
            res.status
        );
    }
}
