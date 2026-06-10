use crate::{create_store_from_url, Store};
use cowen_common::config::AppConfig;
use cowen_common::CowenResult;
use cowen_config::ConfigManager;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum MigrationMode {
    /// 复制模式：同步全量数据并切换 Store，保留源端数据
    Clone,
    /// 永久迁移模式：同步全量数据并切换 Store，完成后清理源端数据
    Move,
}

pub struct StoreMigrator {
    source: Arc<dyn Store>,
}

impl StoreMigrator {
    pub fn new(source: Arc<dyn Store>) -> Self {
        Self { source }
    }

    pub async fn migrate(
        &self,
        cfg_mgr: &ConfigManager,
        target_url: &str,
        mode: MigrationMode,
    ) -> CowenResult<()> {
        let app_dir = &cfg_mgr.app_dir;

        // 1. Create Target Store
        println!("🚀 Initializing target store: {}...", target_url);
        let target = create_store_from_url(target_url, app_dir, "migration").await?;

        // 2. Scan all profiles
        let profiles = self.source.list_all_profiles().await?;
        println!("📂 Found {} profiles to migrate.", profiles.len());

        for profile in profiles {
            println!("🔄 Migrating profile: \x1b[1;34m{}\x1b[0m...", profile);
            self.migrate_profile(&profile, target.as_ref()).await?;

            if mode == MigrationMode::Move {
                println!("🧹 Cleaning up source profile: {}...", profile);
                self.source.clear_profile(&profile).await?;
            }
        }

        // 3. Switch global app.yaml
        self.switch_app_config(cfg_mgr, target_url).await?;

        println!("✅ Migration completed successfully!");
        Ok(())
    }

    async fn migrate_configs(&self, profile: &str, target: &dyn Store) -> CowenResult<()> {
        let configs = self.source.list_configs(profile).await?;
        for k in configs {
            let v = self.source.get_config(profile, &k).await?;
            target.set_config(profile, &k, &v).await?;
        }
        Ok(())
    }

    async fn migrate_secrets(&self, profile: &str, target: &dyn Store) -> CowenResult<()> {
        let secrets = self.source.list_secrets(profile).await?;
        for k in secrets {
            let v = self.source.get_secret(profile, &k).await?;
            target.set_secret(profile, &k, &v).await?;
        }
        Ok(())
    }

    async fn migrate_tokens(&self, profile: &str, target: &dyn Store) -> CowenResult<()> {
        if let Ok(t) = self.source.get_access_token(profile).await {
            target.save_access_token(profile, t).await?;
        }
        if let Ok(t) = self.source.get_refresh_token(profile).await {
            target.save_refresh_token(profile, t).await?;
        }
        Ok(())
    }

    async fn migrate_dlq(&self, profile: &str, target: &dyn Store) -> CowenResult<()> {
        let dlq = self.source.list_all_dlq(profile).await?;
        for m in dlq {
            target.push_dlq(&m).await?;
        }
        Ok(())
    }

    async fn migrate_profile(&self, profile: &str, target: &dyn Store) -> CowenResult<()> {
        self.migrate_configs(profile, target).await?;
        self.migrate_secrets(profile, target).await?;
        self.migrate_tokens(profile, target).await?;
        self.migrate_dlq(profile, target).await?;
        Ok(())
    }

    async fn switch_app_config(
        &self,
        cfg_mgr: &ConfigManager,
        target_url: &str,
    ) -> CowenResult<()> {
        let mut app_cfg: AppConfig = cfg_mgr.load_app_config().await?;

        let store_type = if target_url == "local" {
            "local"
        } else if target_url.starts_with("redis://") {
            "redis"
        } else if target_url.starts_with("sqlite://") || target_url.starts_with("innerdb://") {
            "innerdb"
        } else if target_url.starts_with("mysql://") {
            "mysql"
        } else if target_url.starts_with("postgres://") {
            "postgres"
        } else if target_url.starts_with("mssql://") {
            "mssql"
        } else {
            "innerdb" // Fallback
        };

        app_cfg.storage.store = store_type.to_string();
        if target_url != "local" {
            app_cfg.storage.db_url = Some(target_url.to_string());
        } else {
            app_cfg.storage.db_url = None;
        }

        cfg_mgr.save_app_config(&app_cfg).await?;
        println!(
            "✨ app.yaml updated. Store switched to: \x1b[1;32m{}\x1b[0m",
            store_type
        );
        Ok(())
    }
}
