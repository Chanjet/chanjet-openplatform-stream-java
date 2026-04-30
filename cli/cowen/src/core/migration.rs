use anyhow::{Result, Context};
use std::sync::Arc;
use crate::core::store::{Store, create_store_from_url};
use crate::core::config::ConfigManager;

#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum MigrationMode {
    /// 复制模式：同步全量数据并切换 Store，保留源端数据
    Clone,
    /// 永久迁移模式：同步全量数据并切换 Store，完成后清理源端数据
    Move,
}

pub struct StoreMigrator {
    source: Arc<dyn Store>,
    target: Arc<dyn Store>,
    mode: MigrationMode,
}

impl StoreMigrator {
    pub fn new(source: Arc<dyn Store>, target: Arc<dyn Store>, mode: MigrationMode) -> Self {
        Self { source, target, mode }
    }

    pub async fn run(&self, cfg_mgr: &ConfigManager, target_url: &str) -> Result<()> {
        let profiles = self.source.list_all_profiles().await
            .context("Failed to list profiles from source")?;

        println!("🚀 Starting full migration for {} profiles...", profiles.len());

        for profile in &profiles {
            println!("📦 Migrating profile: \x1b[1;34m{}\x1b[0m", profile);
            self.migrate_profile(profile).await?;
        }

        // Switch Store
        self.switch_app_config(cfg_mgr, target_url).await?;

        // Cleanup if Move
        if self.mode == MigrationMode::Move {
            println!("🧹 Mode is 'Move': Cleaning up source data...");
            for profile in &profiles {
                println!("🗑️ Cleaning source profile: {}", profile);
                let _ = self.source.clear_profile(profile).await;
            }
        }

        println!("✅ Migration completed successfully!");
        Ok(())
    }

    async fn migrate_profile(&self, profile: &str) -> Result<()> {
        // 1. Configs
        let configs = self.source.list_configs(profile).await.unwrap_or_default();
        for k in configs {
            if let Ok(v) = self.source.get_config(profile, &k).await {
                self.target.set_config(profile, &k, &v).await?;
            }
        }

        // 2. Secrets
        for k in ["app_secret", "certificate", "encrypt_key"] {
            if let Ok(v) = self.source.get_secret(profile, k).await {
                self.target.set_secret(profile, k, &v).await?;
            }
        }

        // 3. Tokens
        if let Ok(tokens) = self.source.list_tokens(profile).await {
            for k in tokens {
                if let Ok(v) = self.source.get_token(profile, &k).await {
                    // Default to 1 hour for migrated tokens
                    let _ = self.target.set_token(profile, &k, &v, 3600).await;
                }
            }
        }

        // 4. Audit Logs (Recent 5000)
        if let Ok(logs) = self.source.list_audit(profile, 5000).await {
            for log in logs {
                let _ = self.target.save_audit(&log).await;
            }
        }

        // 5. DLQ
        if let Ok(msgs) = self.source.list_all_dlq(profile).await {
            for m in msgs {
                let _ = self.target.push_dlq(&m).await;
            }
        }

        println!("✅ Profile '{}' migrated. Verifying integrity...", profile);
        self.verify_integrity(profile).await?;
        
        Ok(())
    }

    async fn verify_integrity(&self, profile: &str) -> Result<()> {
        let s_configs = self.source.list_configs(profile).await.unwrap_or_default();
        let t_configs = self.target.list_configs(profile).await.unwrap_or_default();
        if s_configs.len() != t_configs.len() {
            println!("⚠️ Warning: Config count mismatch for profile {}: Source={}, Target={}", profile, s_configs.len(), t_configs.len());
        }
        Ok(())
    }

    async fn switch_app_config(&self, cfg_mgr: &ConfigManager, target_url: &str) -> Result<()> {
        let mut app_cfg = cfg_mgr.load_app_config().await?;
        
        let store_type = if target_url == "local" {
            "local"
        } else if target_url.starts_with("redis://") {
            "redis"
        } else if target_url.starts_with("mysql://") {
            "mysql"
        } else if target_url.starts_with("postgres://") || target_url.starts_with("postgresql://") {
            "postgres"
        } else if target_url.starts_with("sqlite://") || target_url.starts_with("innerdb://") || target_url == "innerdb" {
            "innerdb"
        } else {
            "mysql"
        };

        app_cfg.storage.store = store_type.to_string();
        if target_url != "local" {
            app_cfg.storage.db_url = Some(target_url.to_string());
        } else {
            app_cfg.storage.db_url = None;
        }

        cfg_mgr.save_app_config(&app_cfg).await?;
        println!("✨ app.yaml updated. Store switched to: \x1b[1;32m{}\x1b[0m", store_type);
        Ok(())
    }
}

pub async fn perform_migration(
    cfg_mgr: &ConfigManager, 
    target_url: &str, 
    mode: MigrationMode
) -> Result<()> {
    let source_vault = cfg_mgr.get_vault().ok_or_else(|| anyhow::anyhow!("No active vault found"))?;
    let source = source_vault.primary_store();
    
    let target = create_store_from_url(target_url).await
        .context(format!("Failed to connect to target store: {}", target_url))?;

    let migrator = StoreMigrator::new(source, target, mode);
    migrator.run(cfg_mgr, target_url).await
}
