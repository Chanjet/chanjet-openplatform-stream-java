use anyhow::Result;
use cowen_config::ConfigManager;
use cowen_common::config::{get_app_dir, PluginEntry};
use std::fs;
use std::path::PathBuf;
use cowen_infra::PluginLoader;
use std::ffi::CStr;
use std::os::raw::c_char;

pub async fn list(cfg_mgr: &ConfigManager) -> Result<()> {
    let app_config = cfg_mgr.load_app_config().await?;
    let plugins_dir = get_app_dir().join("plugins");

    println!("🔍 Scanning plugins directory: {:?}", plugins_dir);
    println!("{:<60} | {:<10} | {:<10} | PATH", "NAME (DESC)", "REGISTERED", "ENABLED");
    println!("{:-<60}-+-{:-<10}-+-{:-<10}-+-{:-<40}", "", "", "", "");

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
                let mut display_name = file_name.to_string();
                let mut display_desc = String::new();

                if let Ok(loader) = PluginLoader::new(&path) {
                    unsafe {
                        if let Ok(name_fn) = loader.get_symbol::<unsafe extern "C" fn() -> *const c_char>(b"v1_name") {
                            let ptr = name_fn();
                            if !ptr.is_null() {
                                display_name = CStr::from_ptr(ptr).to_string_lossy().into_owned();
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

                let title = if display_desc.is_empty() {
                    format!("{} ({})", display_name, file_name)
                } else {
                    format!("{} - {} ({})", display_name, display_desc, file_name)
                };

                let name = file_name;

                let is_registered = app_config.search.plugins.iter().any(|p| p.name == name);
                let is_enabled = app_config.search.enabled.contains(&name.to_string());

                let registered_str = if is_registered { "\x1b[32mYes\x1b[0m" } else { "\x1b[31mNo\x1b[0m" };
                let enabled_str = if is_enabled { "\x1b[32mYes\x1b[0m" } else { "\x1b[31mNo\x1b[0m" };

                // Because of ANSI escape codes \x1b[32m (9 chars) and \x1b[0m (4 chars),
                // we add 13 to the padding to maintain the actual visual width of 10.
                println!("{:<60} | {:<23} | {:<23} | {}", title, registered_str, enabled_str, path.display());
            }
        }
    }

    if !found_any {
        println!("(No dynamic library plugins found)");
    }

    Ok(())
}

pub async fn enable(cfg_mgr: &ConfigManager, name: &String) -> Result<()> {
    let mut app_config = cfg_mgr.load_app_config().await?;
    let plugins_dir = get_app_dir().join("plugins");

    let target_name = name.replace("-", "_");
    let expected_lib_name = format!("libcowen_{}", target_name);
    let expected_bin_name = format!("cowen_{}", target_name);

    let mut found_path: Option<PathBuf> = None;
    if plugins_dir.exists() {
        for entry in fs::read_dir(&plugins_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if file_stem == name || file_stem == expected_lib_name || file_stem == expected_bin_name {
                    found_path = Some(path);
                    break;
                }
            }
        }
    }

    if let Some(path) = found_path {
        // Register if not present
        if !app_config.search.plugins.iter().any(|p| &p.name == name) {
            app_config.search.plugins.push(PluginEntry {
                name: name.to_string(),
                path: path.to_string_lossy().to_string(),
                r#type: "dynamic".to_string(),
            });
            println!("✅ Registered new plugin '{}' into configuration.", name);
        }

        // Enable if not present
        if !app_config.search.enabled.contains(name) {
            app_config.search.enabled.push(name.to_string());
            println!("✅ Enabled plugin '{}'.", name);
        } else {
            println!("ℹ️ Plugin '{}' is already enabled.", name);
        }

        cfg_mgr.save_app_config(&app_config).await?;
        println!("🚀 Plugin configuration updated. Restart daemon to take effect if necessary.");
    } else {
        println!("❌ Plugin file for '{}' not found in {:?}", name, plugins_dir);
    }

    Ok(())
}

pub async fn disable(cfg_mgr: &ConfigManager, name: &String) -> Result<()> {
    let mut app_config = cfg_mgr.load_app_config().await?;
    
    if app_config.search.enabled.contains(name) {
        app_config.search.enabled.retain(|n| n != name);
        cfg_mgr.save_app_config(&app_config).await?;
        println!("✅ Disabled plugin '{}'.", name);
        println!("🚀 Plugin configuration updated. Restart daemon to take effect if necessary.");
    } else {
        println!("ℹ️ Plugin '{}' is not currently enabled.", name);
    }

    Ok(())
}
