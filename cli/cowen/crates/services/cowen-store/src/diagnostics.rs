use async_trait::async_trait;
use anyhow::Result;
use cowen_doctor::{DoctorContext, DiagnosticTask, DiagnosticResult, DiagnosticStatus, DiagnosticRegistration};
use std::time::Instant;

pub struct StorageCheck;

async fn create_store_for_diagnostics(ctx: &DoctorContext) -> Result<std::sync::Arc<dyn cowen_common::store::Store>> {
    let app_cfg = ctx.cfg_mgr.load_app_config().await?;
    let app_dir = &ctx.cfg_mgr.app_dir;
    let fingerprint = cowen_common::security::get_machine_fingerprint()?;
    
    let url = if app_cfg.storage.store == "local" {
        "local"
    } else {
        app_cfg.storage.db_url.as_deref().unwrap_or("innerdb")
    };

    Ok(crate::create_store_from_url(url, app_dir, &fingerprint).await?)
}

#[async_trait]
impl DiagnosticTask for StorageCheck {
    fn name(&self) -> &str { "存储后端与Schema" }
    async fn run(&self, ctx: &DoctorContext) -> Result<DiagnosticResult> {
        let start = Instant::now();
        let store = match create_store_for_diagnostics(ctx).await {
            Ok(s) => s,
            Err(e) => return Ok(DiagnosticResult {
                name: self.name().to_string(),
                status: DiagnosticStatus::Error(format!("无法加载存储后端: {}", e)),
                duration_ms: start.elapsed().as_millis() as u64,
            }),
        };
        
        let status = match store.list_dlq_paged(&ctx.profile, 0, 1).await {
            Ok(_) => DiagnosticStatus::Ok,
            Err(_) => {
                if ctx.fix {
                    match store.migrate().await {
                        Ok(_) => DiagnosticStatus::Fixed("Schema 修复成功".to_string()),
                        Err(e) => DiagnosticStatus::Error(format!("Schema 修复失败: {}", e)),
                    }
                } else {
                    DiagnosticStatus::Error("Schema 可能需要更新".to_string())
                }
            }
        };

        Ok(DiagnosticResult {
            name: self.name().to_string(),
            status,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}

inventory::submit! { DiagnosticRegistration { builder: || Box::new(StorageCheck) } }

#[cfg(test)]
mod tests {
    use super::*;
    use cowen_common::config::Config;
    use cowen_config::ConfigManager;


    #[tokio::test]
    async fn test_storage_check_green() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut cfg_mgr = ConfigManager::new().unwrap();
        cfg_mgr.app_dir = temp_dir.path().to_path_buf();

        let app_cfg = cowen_common::config::AppConfig::default();
        let vault = crate::create_vault(&app_cfg, temp_dir.path(), "test_fingerprint").await.unwrap();

        let ctx = DoctorContext {
            profile: "test".to_string(),
            config: Config::default_with_profile("test"),
            verbose: false,
            fix: false,
            vault,
            cfg_mgr,
        };

        let checker = StorageCheck;
        let res = checker.run(&ctx).await.unwrap();
        
        assert!(matches!(res.status, DiagnosticStatus::Ok), "Expected DiagnosticStatus::Ok, got {:?}", res.status);
    }
}
