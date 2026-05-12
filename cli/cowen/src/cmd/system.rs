use anyhow::Result;
use cowen_common::daemon::DaemonService;
use cowen_common::vault::Vault;
use cowen_common::{ConfigManager, CowenResult};
use serde::Serialize;
use std::sync::Arc;

use cowen_common::status::{StatusCollector, StatusContext, StatusEntry, StatusLevel};

#[derive(Serialize)]
pub struct SystemStatus {
    pub profile: String,
    pub entries: Vec<StatusEntry>,
}

pub async fn status(
    active_profile: &str,
    cfg_mgr: &ConfigManager,
    vault: Arc<dyn Vault>,
    format: &str,
    all: bool,
) -> Result<()> {
    let profiles = if all {
        cfg_mgr
            .list_profiles()
            .await
            .map_err(|e| anyhow::anyhow!(e))?
    } else {
        vec![active_profile.to_string()]
    };

    let mut results = Vec::new();
    let mut broken_profiles = Vec::new();

    for p in profiles {
        match get_system_status(&p, cfg_mgr, vault.clone()).await {
            Ok(s) => results.push(s),
            Err(e) => {
                broken_profiles.push((p.clone(), e.to_string()));
                // Also push a basic status entry so it shows up in JSON/list
                results.push(SystemStatus {
                    profile: p.to_string(),
                    entries: vec![StatusEntry {
                        name: "System".to_string(),
                        icon: "🚨".to_string(),
                        level: StatusLevel::ERROR,
                        message: format!("Profile load failed: {}", p),
                        reason: Some(e.to_string()),
                        details: vec![],
                        children: vec![],
                    }],
                });
            }
        }
    }

    if format == "json" || format == "yaml" {
        if !all && results.len() == 1 {
            cowen_common::utils::render(&results[0], format).map_err(|e| anyhow::anyhow!(e))?;
        } else {
            cowen_common::utils::render(&results, format).map_err(|e| anyhow::anyhow!(e))?;
        }
        return Ok(());
    }

    for s in results {
        println!("\n🔍 COWEN System Status Diagnostics (Profile: '{}', Build: {}, Time: {})", s.profile, cowen_common::BUILD_ID, cowen_common::BUILD_TIME);
        println!("----------------------------------");
        for entry in s.entries {
            render_entry(&entry, 0);
        }
    }

    if !broken_profiles.is_empty() {
        println!("\n❌ Profiles with Errors:");
        for (p, e) in broken_profiles {
            println!("  - {}: {}", p, e);
        }
    }

    Ok(())
}

pub async fn config(profile: &str, cfg_mgr: &ConfigManager, format: &str) -> Result<()> {
    let cfg = cfg_mgr
        .load(profile)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let app_cfg = cfg_mgr
        .load_app_config()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    #[derive(Serialize)]
    struct CombinedConfig {
        global: cowen_common::AppConfig,
        profile: cowen_common::Config,
    }

    let report = CombinedConfig {
        global: app_cfg,
        profile: cfg,
    };

    if format == "json" || format == "yaml" {
        cowen_common::utils::render(&report, format).map_err(|e| anyhow::anyhow!(e))?;
    } else {
        println!("\n🌐 Global Configuration (app.yaml)");
        println!("----------------------------------");
        println!("Storage Type:  {}", report.global.storage.store);
        if let Some(url) = &report.global.storage.db_url {
            println!(
                "Storage URL:   {}",
                cowen_common::utils::mask_url_query(url)
            );
        }
        println!("Cache Type:    {}", report.global.storage.cache);
        if let Some(url) = &report.global.storage.cache_url {
            println!(
                "Cache URL:     {}",
                cowen_common::utils::mask_url_query(url)
            );
        }

        println!("\n👤 Profile Configuration ({}.yaml)", profile);
        println!("----------------------------------");
        println!("AppKey:        {}", report.profile.app_key);
        println!("AppMode:       {:?}", report.profile.app_mode);
        println!("OpenAPI URL:   {}", report.profile.openapi_url);
        println!("Stream URL:    {}", report.profile.stream_url);
        println!("Webhook:       {}", report.profile.webhook_target);
        println!("Proxy Port:    {}", report.profile.proxy_port);
        println!("Log Level:     {}", report.profile.log.level);
        println!("AI Enabled:    {}", report.profile.ai_enabled);
        println!();
    }

    Ok(())
}

fn render_entry(entry: &StatusEntry, indent: usize) {
    let prefix = "  ".repeat(indent);
    let level_str = match entry.level {
        StatusLevel::OK => "\x1b[32m(OK)\x1b[0m",
        StatusLevel::WARN => "\x1b[33m(WARN)\x1b[0m",
        StatusLevel::ERROR => "\x1b[31m(ERROR)\x1b[0m",
        _ => "(UNKNOWN)",
    };

    println!(
        "{}{} {}: {} {}",
        prefix, entry.icon, entry.name, entry.message, level_str
    );
    if let Some(reason) = &entry.reason {
        println!("{}   \x1b[31m╰─ Reason: {}\x1b[0m", prefix, reason);
    }
    for detail in &entry.details {
        println!("{}   - {}", prefix, detail);
    }
    for child in &entry.children {
        render_entry(child, indent + 1);
    }
}

struct DaemonCollector;
#[async_trait::async_trait]
impl StatusCollector for DaemonCollector {
    fn name(&self) -> &str {
        "Daemon"
    }
    async fn collect(&self, ctx: &StatusContext<'_>) -> CowenResult<StatusEntry> {
        cowen_common::status::collect_daemon_status(
            ctx,
            "Stream Bridge (Daemon)",
            "若需实现多租户消息同步，请运行 'cowen daemon start'",
            true,
        )
        .await
    }
}

async fn get_system_status(
    profile: &str,
    cfg_mgr: &ConfigManager,
    vault: Arc<dyn Vault>,
) -> CowenResult<SystemStatus> {
    let cfg = cfg_mgr.load(profile).await?;
    let app_cfg = cfg_mgr.load_app_config().await?;
    let ctx = StatusContext {
        profile: profile.to_string(),
        config: &cfg,
        app_config: &app_cfg,
        vault: vault.clone(),
    };

        let collectors: Vec<Box<dyn StatusCollector>> = vec![
            Box::new(ConfigCollector),
            Box::new(ProviderCollector),
        ];

    let mut entries = Vec::new();
    for c in collectors {
        if let Ok(e) = c.collect(&ctx).await {
            entries.push(e);
        }
    }

    Ok(SystemStatus {
        profile: profile.to_string(),
        entries,
    })
}

// --- Collectors ---

use cowen_auth::client::Client;

struct ConfigCollector;
#[async_trait::async_trait]
impl StatusCollector for ConfigCollector {
    fn name(&self) -> &str {
        "Configuration"
    }
    async fn collect(&self, ctx: &StatusContext<'_>) -> CowenResult<StatusEntry> {
        use cowen_common::status::CommonTemplate;

        // 0.2.x Style Configuration Entry
        let mode_str = format!("{:?}", ctx.config.app_mode).to_lowercase();
        let mut details = vec![];
        details.push(format!("Build ID:   {}", cowen_common::BUILD_ID));
        details.push(format!("Build Time: {}", cowen_common::BUILD_TIME));
        details.push(format!("OpenAPI:    {}", ctx.config.openapi_url));
        details.push(format!("Stream:     {}", ctx.config.stream_url));

        let ak_level = if ctx.config.app_key.trim().is_empty() {
            StatusLevel::ERROR
        } else {
            StatusLevel::OK
        };
        let ak_msg = if ak_level == StatusLevel::OK {
            format!("AppKey: {} (Mode: {})", ctx.config.app_key, mode_str)
        } else {
            "AppKey is missing".to_string()
        };

        Ok(StatusEntry::new(
            CommonTemplate::Custom("Configuration".to_string(), "⚙️".to_string()),
            ak_level,
            ak_msg,
        )
        .with_details(details))
    }
}


struct ProviderCollector;
#[async_trait::async_trait]
impl StatusCollector for ProviderCollector {
    fn name(&self) -> &str {
        "Provider"
    }
    async fn collect(&self, ctx: &StatusContext<'_>) -> CowenResult<StatusEntry> {
        use cowen_common::status::CommonTemplate;
        let auth_cli = cowen_auth::create_auth_client_with_vault(ctx.vault.clone());
        let children = auth_cli.get_diagnostics(ctx).await?;

        let mut max_level = StatusLevel::OK;
        for c in &children {
            if c.level as i32 > max_level as i32 {
                max_level = c.level;
            }
        }

        let mode_str = format!("{:?}", ctx.config.app_mode);
        Ok(StatusEntry::new(
            CommonTemplate::ProviderSummary(
                format!("{} Mode Diagnostics", mode_str),
                "💎".to_string(),
            ),
            max_level,
            format!("Collected {} status indicators", children.len()),
        )
        .with_children(children))
    }
}

pub async fn reset(
    profile: &str,
    vault: Option<&dyn Vault>,
    cfg_mgr: &ConfigManager,
    event_bus: Option<&cowen_common::events::EventBus>,
) -> Result<()> {
    if let Some(v) = vault {
        v.clear_profile(profile)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
    }
    cfg_mgr
        .delete(profile)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    if let Some(bus) = event_bus {
        bus.publish(cowen_common::events::GlobalEvent::ProfileDeleted {
            name: profile.to_string(),
        });
    }

    println!(
        "✅ Profile '{}' and all associated data have been physically removed.",
        profile
    );
    Ok(())
}

pub async fn rename_profile(
    old: &str,
    new: &str,
    cfg_mgr: &ConfigManager,
    vault: Arc<dyn Vault>,
    event_bus: &cowen_common::events::EventBus,
) -> Result<()> {
    if !cfg_mgr.exists(old).await {
        return Err(anyhow::anyhow!("Source profile '{}' does not exist", old));
    }
    if cfg_mgr.exists(new).await {
        return Err(anyhow::anyhow!("Target profile '{}' already exists", new));
    }

    // 1. Rename files
    cfg_mgr
        .rename(old, new)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    // 2. Rename in Vault (SQL/Redis)
    vault
        .rename_profile(old, new)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    // 3. Update default if needed
    if cfg_mgr.get_default_profile() == old {
        cfg_mgr
            .set_default_profile(new)
            .map_err(|e| anyhow::anyhow!(e))?;
    }

    // 4. Broadcast event
    event_bus.publish(cowen_common::events::GlobalEvent::ProfileRenamed {
        old: old.to_string(),
        new: new.to_string(),
    });

    println!("✅ Profile '{}' has been renamed to '{}'", old, new);
    Ok(())
}

pub async fn ensure_daemon_running(
    profile: &str,
    config: &cowen_common::Config,
    _cfg_mgr: &ConfigManager,
    vault: Arc<dyn Vault>,
    auth_cli: &dyn cowen_auth::client::Client,
) -> Result<()> {
    // 1. Check if already running
    let info = cowen_common::status::get_active_daemon_info(profile);
    if info.is_some() {
        return Ok(());
    }

    // 2. Check if recovery is recommended by provider
    let provider = auth_cli
        .get_provider(&config.app_mode)
        .ok_or_else(|| anyhow::anyhow!("No provider found for profile '{}'", profile))?;

    // We check pid file existence for extra safety (should_auto_recover might care)
    let pid_file = cowen_common::config::get_app_dir().join(format!("{}_daemon.pid", profile));
    let pid_file_exists = pid_file.exists();

    let app_cfg = _cfg_mgr.load_app_config().await.unwrap_or_default();
    let is_distributed = _cfg_mgr.is_distributed_storage(&app_cfg);

    if provider
        .should_auto_recover(profile, config, false, pid_file_exists, is_distributed)
        .await
    {
        tracing::info!(target: "sys", profile = %profile, "Daemon not running, triggering auto-recovery...");

        let daemon_svc = cowen_server::ServerDaemonService::new(_cfg_mgr.clone());
        let _ = daemon_svc.start_daemon(profile, config, vault).await;
    }

    Ok(())
}

pub async fn enforce_daemon_version_sync(
    cfg_mgr: &ConfigManager,
    vault: Arc<dyn Vault>,
) -> Result<()> {
    let profiles = cfg_mgr.list_profiles().await.unwrap_or_default();
    for p in profiles {
        if let Some(info) = cowen_common::status::get_active_daemon_info(&p) {
            let mut outdated = false;
            let daemon_bid = info.build_id.as_deref().unwrap_or("N/A");
            let daemon_bt = info.build_time.as_deref().unwrap_or("N/A");
            
            // 🚀 STRICT EQUALITY: Use unified constants for absolute matching.
            if daemon_bid != cowen_common::BUILD_ID || daemon_bt != cowen_common::BUILD_TIME {
                outdated = true;
            }

            if outdated {
                tracing::info!(target: "sys", profile = %p, "Daemon version mismatch (CLI: {} / {}, Daemon: {}). Restarting...", cowen_common::BUILD_ID, cowen_common::BUILD_TIME, daemon_bid);
                eprintln!("🔄 Profile '{}' 的后台进程版本已过时，正在自动重启以同步最新构建...", p);
                
                let config = cfg_mgr.load(&p).await.unwrap_or_else(|_| cowen_common::Config::default_with_profile(&p));
                // Execute restart
                let _ = crate::cmd::daemon::restart(&p, &config, config.proxy_port, config.proxy_enabled, false, cfg_mgr, vault.clone()).await;
            }
        }
    }
    Ok(())
}
