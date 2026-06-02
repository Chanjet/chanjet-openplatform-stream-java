use anyhow::Result;
use cowen_config::ConfigManager;
use cowen_common::config::get_app_dir;
use std::fs;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub async fn list(cfg_mgr: &ConfigManager) -> Result<()> {
    let app_config = cfg_mgr.load_app_config().await?;
    let plugins_dir = get_app_dir().join("plugins");

    println!("🔍 Scanning plugins directory: {:?}", plugins_dir);
    println!("{:<30} | {:<10} | {:<10} | DESCRIPTION", "NAME", "CAPABILITY", "ENABLED");
    println!("{:-<30}-+-{:-<10}-+-{:-<10}-+-{:-<40}", "", "", "", "");

    if !plugins_dir.exists() {
        println!("(No plugins directory found)");
        return Ok(());
    }

    let mut found_any = false;
    let supported_exts = if cfg!(target_os = "macos") {
        vec!["dylib", "so"]
    } else if cfg!(target_os = "windows") {
        vec!["dll"]
    } else {
        vec!["so"]
    };

    for entry in fs::read_dir(plugins_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if supported_exts.contains(&ext) {
                found_any = true;
                let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                
                let display_trait = "unknown (Thin CLI)";
                let display_desc = "Inspected by daemon";
                let name = file_name;
                let is_enabled = app_config.plugins.contains(&name.to_string());
                let enabled_str = if is_enabled { "\x1b[32mYes\x1b[0m" } else { "\x1b[31mNo\x1b[0m" };

                println!("{:<30} | {:<10} | {:<23} | {}", name, display_trait, enabled_str, display_desc);
            }
        }
    }

    if found_any {
        println!("\n💡 CAPABILITY indicates the plugin's capability (e.g., SearchProvider for semantic search).");
    } else {
        println!("(No dynamic library plugins found)");
    }

    Ok(())
}

pub async fn enable(cfg_mgr: &ConfigManager, name: &String) -> Result<()> {
    let mut app_config = cfg_mgr.load_app_config().await?;
    let plugins_dir = get_app_dir().join("plugins");

    let expected_path = if cfg!(target_os = "windows") {
        plugins_dir.join(format!("{}.dll", name))
    } else if cfg!(target_os = "macos") {
        plugins_dir.join(format!("{}.dylib", name))
    } else {
        plugins_dir.join(format!("{}.so", name))
    };

    if expected_path.exists() {
        if !app_config.plugins.contains(name) {
            app_config.plugins.push(name.to_string());
            println!("✅ Enabled plugin '{}'.", name);
            cfg_mgr.save_app_config(&app_config).await?;
            println!("🚀 Plugin configuration updated. Restart daemon to take effect if necessary.");
        } else {
            println!("ℹ️ Plugin '{}' is already enabled.", name);
        }
    } else {
        println!("❌ Plugin file for '{}' not found in {:?} (Ensure the exact filename without extension is provided)", name, plugins_dir);
    }

    Ok(())
}

pub async fn disable(cfg_mgr: &ConfigManager, name: &String) -> Result<()> {
    let mut app_config = cfg_mgr.load_app_config().await?;
    
    if app_config.plugins.contains(name) {
        app_config.plugins.retain(|n| n != name);
        cfg_mgr.save_app_config(&app_config).await?;
        println!("✅ Disabled plugin '{}'.", name);
        println!("🚀 Plugin configuration updated. Restart daemon to take effect if necessary.");
    } else {
        println!("ℹ️ Plugin '{}' is not currently enabled.", name);
    }

    Ok(())
}

pub async fn install(_cfg_mgr: &ConfigManager, path: &String) -> Result<()> {
    let source_path = std::path::Path::new(path);
    if !source_path.exists() || !source_path.is_file() {
        return Err(anyhow::anyhow!("❌ Source plugin file not found or is not a file: {}", path));
    }

    let file_name = source_path.file_name().ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;
    let plugins_dir = get_app_dir().join("plugins");
    
    if !plugins_dir.exists() {
        std::fs::create_dir_all(&plugins_dir)?;
    }
    
    let target_path = plugins_dir.join(file_name);
    std::fs::copy(source_path, &target_path)?;
    
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&target_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&target_path, perms)?;
    }

    let bundle_source_path = source_path.with_extension("bundle");
    if bundle_source_path.exists() && bundle_source_path.is_file() {
        let bundle_file_name = bundle_source_path.file_name().unwrap();
        let bundle_target_path = plugins_dir.join(bundle_file_name);
        std::fs::copy(&bundle_source_path, &bundle_target_path)?;
        println!("✅ Automatically copied signature bundle: {}", bundle_file_name.to_string_lossy());
    } else {
        println!("⚠️  Warning: No signature bundle (.bundle) found alongside the plugin. It may fail to load due to security policy.");
    }

    println!("✅ Successfully installed plugin '{}' to {:?}", file_name.to_string_lossy(), plugins_dir);
    println!("💡 Use 'cowen plugins list' to view it, and 'cowen plugins enable <name>' to activate it.");
    
    Ok(())
}

pub async fn refresh_signature(_cfg_mgr: &ConfigManager, _name: &String) -> Result<()> {
    println!("⚠️ Signature verification and refresh is delegated to cowen-daemon in the thin CLI architecture.");
    println!("Please refer to daemon logs for validation status during startup.");
    Ok(())
}
