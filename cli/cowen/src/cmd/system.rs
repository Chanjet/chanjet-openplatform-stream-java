use cowen_common::vault::Vault;
use cowen_common::{ConfigManager, CowenResult};
use anyhow::Result;
use serde::Serialize;
use std::sync::Arc;

use cowen_common::status::{StatusEntry, StatusLevel, StatusContext, StatusCollector};

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
        cfg_mgr.list_profiles().await.map_err(|e| anyhow::anyhow!(e))?
    } else {
        vec![active_profile.to_string()]
    };

    let mut results = Vec::new();
    for p in profiles {
        if let Ok(s) = get_system_status(&p, cfg_mgr, vault.clone()).await {
            results.push(s);
        }
    }

    if format == "json" || format == "yaml" {
        cowen_common::utils::render(&results, format).map_err(|e| anyhow::anyhow!(e))?;
        return Ok(());
    }

    for s in results {
        println!("\n🌍 System Status (Profile: \x1b[1;32m{}\x1b[0m)", s.profile);
        println!("----------------------------------");
        for entry in s.entries {
            render_entry(&entry, 0);
        }
    }

    Ok(())
}

pub async fn config(profile: &str, cfg_mgr: &ConfigManager, format: &str) -> Result<()> {
    let cfg = cfg_mgr.load(profile).await.map_err(|e| anyhow::anyhow!(e))?;
    let app_cfg = cfg_mgr.load_app_config().await.map_err(|e| anyhow::anyhow!(e))?;

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
            println!("Storage URL:   {}", cowen_common::utils::mask_url_query(url));
        }
        println!("Cache Type:    {}", report.global.storage.cache);
        if let Some(url) = &report.global.storage.cache_url {
            println!("Cache URL:     {}", cowen_common::utils::mask_url_query(url));
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
    let level_icon = match entry.level {
        StatusLevel::OK => "✅",
        StatusLevel::WARN => "⚠️ ",
        StatusLevel::ERROR => "❌",
        _ => "⚪",
    };

    println!("{}{} {} {} - {}", prefix, level_icon, entry.icon, entry.name, entry.message);
    if let Some(reason) = &entry.reason {
        println!("{}   \x1b[31m╰─ Reason: {}\x1b[0m", prefix, reason);
    }
    for child in &entry.children {
        render_entry(child, indent + 1);
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
    fn name(&self) -> &str { "Configuration" }
    async fn collect(&self, ctx: &StatusContext<'_>) -> CowenResult<StatusEntry> {
        use cowen_common::status::CommonTemplate;
        let mut children = Vec::new();
        
        // 1. AppKey
        let ak_level = if ctx.config.app_key.trim().is_empty() { StatusLevel::ERROR } else { StatusLevel::OK };
        children.push(StatusEntry::new(CommonTemplate::Custom("AppKey".to_string(), "🔑".to_string()), ak_level, 
            if ak_level == StatusLevel::OK { "Configured".to_string() } else { "Missing".to_string() }));

        // 2. Secret in Vault
        let has_secret = ctx.vault.get_secret(&ctx.profile, "app_secret").await.is_ok();
        let sec_level = if has_secret { StatusLevel::OK } else { StatusLevel::ERROR };
        children.push(StatusEntry::new(CommonTemplate::Custom("AppSecret".to_string(), "🔐".to_string()), sec_level,
            if has_secret { "Stored in Vault".to_string() } else { "Missing from Vault".to_string() }));

        let max_level = if ak_level == StatusLevel::ERROR || sec_level == StatusLevel::ERROR { StatusLevel::ERROR } else { StatusLevel::OK };
        
        Ok(StatusEntry::new(CommonTemplate::Configuration, max_level, "Profile identity settings".to_string())
            .with_children(children))
    }
}

struct ProviderCollector;
#[async_trait::async_trait]
impl StatusCollector for ProviderCollector {
    fn name(&self) -> &str { "Provider" }
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
        Ok(StatusEntry::new(CommonTemplate::ProviderSummary(format!("{} Mode Diagnostics", mode_str), "💎".to_string()), max_level, format!("Collected {} status indicators", children.len()))
            .with_children(children))
    }
}

pub async fn reset(profile: &str, vault: Option<&dyn Vault>, cfg_mgr: &ConfigManager, event_bus: Option<&cowen_common::events::EventBus>) -> Result<()> {
    if let Some(v) = vault {
        v.clear_profile(profile).await.map_err(|e| anyhow::anyhow!(e))?;
    }
    cfg_mgr.delete(profile).await.map_err(|e| anyhow::anyhow!(e))?;
    
    if let Some(bus) = event_bus {
        bus.publish(cowen_common::events::GlobalEvent::ProfileDeleted { name: profile.to_string() });
    }
    
    println!("✅ Profile '{}' and all associated data have been physically removed.", profile);
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
    cfg_mgr.rename(old, new).await.map_err(|e| anyhow::anyhow!(e))?;
    
    // 2. Rename in Vault (SQL/Redis)
    vault.rename_profile(old, new).await.map_err(|e| anyhow::anyhow!(e))?;

    // 3. Update default if needed
    if cfg_mgr.get_default_profile() == old {
        cfg_mgr.set_default_profile(new).map_err(|e| anyhow::anyhow!(e))?;
    }

    // 4. Broadcast event
    event_bus.publish(cowen_common::events::GlobalEvent::ProfileRenamed { 
        old: old.to_string(), 
        new: new.to_string() 
    });

    println!("✅ Profile '{}' has been renamed to '{}'", old, new);
    Ok(())
}

pub async fn ensure_daemon_running(
    _profile: &str,
    _config: &cowen_common::Config,
    _cfg_mgr: &ConfigManager,
    _vault: Arc<dyn Vault>,
    _auth_cli: &dyn cowen_auth::client::Client,
) -> Result<()> {
    // Legacy/Stub for now
    Ok(())
}
