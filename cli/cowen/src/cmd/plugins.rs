use anyhow::Result;
use cowen_config::ConfigManager;
use cowen_common::config::get_app_dir;
use std::fs;
use cowen_infra::PluginLoader;
use std::ffi::CStr;
use std::os::raw::c_char;

pub async fn list(cfg_mgr: &ConfigManager) -> Result<()> {
    let app_config = cfg_mgr.load_app_config().await?;
    let plugins_dir = get_app_dir().join("plugins");

    println!("🔍 Scanning plugins directory: {:?}", plugins_dir);
    println!("{:<30} | {:<10} | {:<10} | DESCRIPTION", "NAME", "TYPE", "ENABLED");
    println!("{:-<30}-+-{:-<10}-+-{:-<10}-+-{:-<40}", "", "", "", "");

    if !plugins_dir.exists() {
        println!("(No plugins directory found)");
        return Ok(());
    }

    let mut found_any = false;
    for entry in fs::read_dir(plugins_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "so" || ext == "dylib" || ext == "dll" {
                found_any = true;
                let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                let mut display_trait = "unknown".to_string();
                let mut display_desc = String::new();

                if let Ok(loader) = PluginLoader::new(&path) {
                    unsafe {
                        if let Ok(trait_fn) = loader.get_symbol::<unsafe extern "C" fn() -> *const c_char>(b"v1_trait") {
                            let ptr = trait_fn();
                            if !ptr.is_null() {
                                display_trait = CStr::from_ptr(ptr).to_string_lossy().into_owned();
                            }
                        }
                        if let Ok(desc_fn) = loader.get_symbol::<unsafe extern "C" fn() -> *const c_char>(b"v1_desc") {
                            let ptr = desc_fn();
                            if !ptr.is_null() {
                                display_desc = CStr::from_ptr(ptr).to_string_lossy().into_owned();
                            }
                        }
                    }
                }

                let name = file_name;
                let is_enabled = app_config.plugins.contains(&name.to_string());
                let enabled_str = if is_enabled { "\x1b[32mYes\x1b[0m" } else { "\x1b[31mNo\x1b[0m" };

                // Because of ANSI escape codes \x1b[32m (9 chars) and \x1b[0m (4 chars),
                // we add 13 to the padding to maintain the actual visual width of 10.
                println!("{:<30} | {:<10} | {:<23} | {}", name, display_trait, enabled_str, display_desc);
            }
        }
    }

    if found_any {
        println!("\n💡 TYPE indicates the plugin's capability (e.g., SearchProvider for semantic search).");
    } else {
        println!("(No dynamic library plugins found)");
    }

    Ok(())
}

pub async fn enable(cfg_mgr: &ConfigManager, name: &String) -> Result<()> {
    let mut app_config = cfg_mgr.load_app_config().await?;
    let plugins_dir = get_app_dir().join("plugins");

    let expected_path_dylib = plugins_dir.join(format!("{}.dylib", name));
    let expected_path_so = plugins_dir.join(format!("{}.so", name));
    let expected_path_dll = plugins_dir.join(format!("{}.dll", name));

    if expected_path_dylib.exists() || expected_path_so.exists() || expected_path_dll.exists() {
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
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&target_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&target_path, perms)?;
    }

    println!("✅ Successfully installed plugin '{}' to {:?}", file_name.to_string_lossy(), plugins_dir);
    println!("💡 Use 'cowen plugins list' to view it, and 'cowen plugins enable <name>' to activate it.");
    
    Ok(())
}
