use cowen_common::{CowenResult, CowenError};
use std::sync::Arc;
use crate::{Store, create_store_from_url};
use cowen_common::ConfigManager;

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

    pub async fn run(&self, cfg_mgr: &ConfigManager, target_url: &str) -> CowenResult<Vec<String>> {
        let mut profiles = self.source.list_all_profiles().await
            .map_err(|e| CowenError::store(format!("Failed to list profiles from source: {}", e)))?;

        // 🚀 OCP: Proactively add app:AppKey profiles for each standard profile to ensure specialized data (Tickets) migrates
        let mut app_profiles = Vec::new();
        for p in &profiles {
            if let Ok(cfg) = cfg_mgr.load(p).await {
                if !cfg.app_key.is_empty() {
                    let ap = format!("app:{}", cfg.app_key);
                    if !profiles.contains(&ap) && !app_profiles.contains(&ap) {
                        app_profiles.push(ap);
                    }
                }
            }
        }
        profiles.extend(app_profiles);

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
        Ok(profiles)
    }

    pub async fn migrate_profile(&self, profile: &str) -> CowenResult<()> {
        // 1. Configs
        let configs = self.source.list_configs(profile).await.unwrap_or_default();
        for k in configs {
            // EXCLUDE: Don't migrate app_ticket keys as generic configs
            if k == "app_ticket" || k == "app_ticket_v2" || k == "app_ticket_created" {
                continue;
            }

            if let Ok(v) = self.source.get_config(profile, &k).await {
                // Specialized handling for permanent codes (FileStore -> SQL optimization)
                if profile.starts_with("app:") {
                    let ak = &profile[4..];
                    if k.starts_with("org_code:") {
                        let org_id = &k[9..];
                        let _ = self.target.save_org_permanent_code(ak, org_id, &v).await;
                        continue; // Already moved to specialized table
                    } else if k.starts_with("user_code:") {
                        let parts: Vec<&str> = k[10..].split(':').collect();
                        if parts.len() == 2 {
                             let _ = self.target.save_user_permanent_code(ak, parts[0], parts[1], &v).await;
                             continue; // Already moved to specialized table
                        }
                    }
                }
                self.target.set_config(profile, &k, &v).await?;
            }
        }

        // 2. Secrets - IMPROVED: Full migration
        let secrets = self.source.list_secrets(profile).await.unwrap_or_default();
        for k in secrets {
            if let Ok(v) = self.source.get_secret(profile, &k).await {
                self.target.set_secret(profile, &k, &v).await?;
            }
        }

        // 3. Specialized Types (Tokens, Tickets)
        // Profile-level access token
        if let Ok(token) = self.source.get_access_token(profile).await {
            tracing::info!(target: "sys", profile = %profile, "Migrating specialized: AccessToken");
            let _ = self.target.save_access_token(profile, token).await;
        }

        // Profile-level refresh token
        if let Ok(token) = self.source.get_refresh_token(profile).await {
            tracing::info!(target: "sys", profile = %profile, "Migrating specialized: RefreshToken");
            let _ = self.target.save_refresh_token(profile, token).await;
        }

        // App-level data
        if profile.starts_with("app:") {
            let app_key = &profile[4..];
            match self.source.get_app_ticket(app_key).await {
                Ok(ticket) => {
                    tracing::info!(target: "sys", profile = %profile, app_key = %app_key, "Migrating specialized: AppTicket");
                    if let Err(e) = self.target.save_app_ticket(app_key, ticket).await {
                        tracing::error!(target: "sys", profile = %profile, "Failed to save AppTicket to target: {}", e);
                    }
                },
                Err(e) => {
                    tracing::debug!(target: "sys", profile = %profile, "No AppTicket found in source (expected if not a self-built app): {}", e);
                }
            }

            if let Ok(token) = self.source.get_app_access_token(app_key).await {
                tracing::info!(target: "sys", profile = %profile, app_key = %app_key, "Migrating specialized: AppAccessToken");
                let _ = self.target.save_app_access_token(app_key, token).await;
            }
        }

        // 4. Tokens (Legacy/Custom)
        if let Ok(tokens) = self.source.list_tokens(profile).await {
            for k in tokens {
                if let Ok(v) = self.source.get_token(profile, &k).await {
                    // Default to 1 hour for migrated tokens
                    let _ = self.target.set_token(profile, &k, &v, 3600).await;
                }
            }
        }

        // 5. Audit Logs (Recent 5000)
        if let Ok(logs) = self.source.list_audit(profile, 5000).await {
            for log in logs {
                let _ = self.target.save_audit(&log).await;
            }
        }

        // 6. DLQ
        if let Ok(msgs) = self.source.list_all_dlq(profile).await {
            for m in msgs {
                let _ = self.target.push_dlq(&m).await;
            }
        }

        println!("✅ Profile '{}' migrated. Verifying integrity...", profile);
        self.verify_integrity(profile).await?;
        
        Ok(())
    }

    async fn verify_integrity(&self, profile: &str) -> CowenResult<()> {
        let s_configs = self.source.list_configs(profile).await.unwrap_or_default();
        let t_configs = self.target.list_configs(profile).await.unwrap_or_default();
        if s_configs.len() > t_configs.len() {
            println!("⚠️ Warning: Potential data loss for profile {}: Source={}, Target={}", profile, s_configs.len(), t_configs.len());
        }
        Ok(())
    }

    async fn switch_app_config(&self, cfg_mgr: &ConfigManager, target_url: &str) -> CowenResult<()> {
        let mut app_cfg = cfg_mgr.load_app_config().await?;
        
        let store_type = if target_url == "local" {
            "local"
        } else if target_url.starts_with("redis://") {
            "redis"
        } else if target_url.starts_with("mysql://") {
            "mysql"
        } else if target_url.starts_with("postgres://") || target_url.starts_with("postgresql://") {
            "postgres"
        } else if target_url.starts_with("innerdb://") || target_url == "innerdb" {
            "innerdb"
        } else if target_url.starts_with("sqlite://") {
            "sqlite"
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
    mode: MigrationMode,
    app_dir: &std::path::Path,
    fingerprint: &str,
) -> CowenResult<Vec<String>> {
    let source_vault = cfg_mgr.get_vault().ok_or_else(|| CowenError::Internal("No active vault found".to_string()))?;
    let source = source_vault.primary_store();
    
    let target = create_store_from_url(target_url, app_dir, fingerprint).await
        .map_err(|e| CowenError::store(format!("Failed to connect to target store: {}", e)))?;

    let migrator = StoreMigrator::new(source, target, mode);
    let profiles = migrator.run(cfg_mgr, target_url).await?;
    Ok(profiles)
}
