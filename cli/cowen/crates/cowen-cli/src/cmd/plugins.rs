use anyhow::Result;

use cowen_common::config::get_app_dir;
use std::fs;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub async fn list() -> Result<()> {
    let plugins_dir = get_app_dir().join("plugins");
    
    // Read app.yaml to see which are enabled
    let app_yaml_path = get_app_dir().join("app.yaml");
    let content = std::fs::read_to_string(&app_yaml_path).unwrap_or_else(|_| "{}".to_string());
    let mut enabled_plugins: Vec<String> = vec![];
    if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
        if let Some(plugins) = val.get("plugins").and_then(|v| v.as_sequence()) {
            enabled_plugins = plugins.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
        }
    }

    println!("🔍 Scanning plugins directory: {:?}", plugins_dir);
    println!("{:<30} | {:<10} | {:<10} | DESCRIPTION", "NAME", "CAPABILITY", "ENABLED");
    println!("{:-<30}-+-{:-<10}-+-{:-<10}-+-{:-<40}", "", "", "", "");

    if !plugins_dir.exists() {
        println!("(No plugins directory found)");
        return Ok(());
    }

    let mut found_any = false;
    let supported_exts = if cfg!(target_os = "windows") {
        vec!["exe"]
    } else {
        vec![""]
    };

    for entry in fs::read_dir(plugins_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if supported_exts.contains(&ext) {
                found_any = true;
                let file_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
                let mut display_trait = "unknown (Thin CLI)".to_string();
                let mut display_desc = "Inspected by daemon".to_string();

                // Attempt to read the .bundle file to get metadata without loading the dylib
                let bundle_path = path.with_extension("bundle");
                if bundle_path.exists() {
                    if let Ok(bundle_str) = std::fs::read_to_string(&bundle_path) {
                        if let Ok(bundle) = serde_json::from_str::<serde_json::Value>(&bundle_str) {
                            if let Some(capabilities) = bundle.get("manifest").and_then(|m| m.get("capabilities")).and_then(|c| c.as_array()) {
                                let caps: Vec<String> = capabilities.iter().filter_map(|c| c.as_str().map(|s| s.to_string())).collect();
                                if !caps.is_empty() {
                                    display_trait = caps.join(", ");
                                }
                            }
                            if let Some(version) = bundle.get("manifest").and_then(|m| m.get("version")).and_then(|v| v.as_str()) {
                                display_desc = format!("v{} (Signed)", version);
                            } else {
                                display_desc = "Signed bundle".to_string();
                            }
                        }
                    }
                }

                let name = file_name;
                let is_enabled = enabled_plugins.contains(&name.to_string());
                let enabled_str = if is_enabled { "\x1b[32mYes\x1b[0m" } else { "\x1b[31mNo\x1b[0m" };

                println!("{:<30} | {:<10} | {:<23} | {}", name, display_trait, enabled_str, display_desc);
            }
        }
    }

    if found_any {
        println!("\n💡 CAPABILITY indicates the plugin's capability (e.g., SearchProvider for semantic search).");
    } else {
        println!("(No executable plugins found)");
    }

    Ok(())
}

pub async fn enable(name: &String) -> Result<()> {
    let plugins_dir = get_app_dir().join("plugins");

    let expected_path = if cfg!(target_os = "windows") {
        plugins_dir.join(format!("{}.exe", name))
    } else {
        plugins_dir.join(name)
    };

    if expected_path.exists() {
        let port_path = cowen_common::config::get_ipc_port_path();
        let _ipc = cowen_common::grpc::client::DaemonClient::new(port_path);
        // Instead of writing app.yaml, tell daemon to set it? 
        // Wait, Daemon has SetGlobalConfig but plugins is a list.
        // We will just read/write locally using serde_yaml.
        let app_yaml_path = get_app_dir().join("app.yaml");
        let content = std::fs::read_to_string(&app_yaml_path).unwrap_or_else(|_| "{}".to_string());
        let mut val = serde_yaml::from_str::<serde_yaml::Value>(&content).unwrap_or_else(|_| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
        
        let mut enabled_plugins: Vec<String> = val.get("plugins").and_then(|v| v.as_sequence()).map(|seq| seq.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
        if !enabled_plugins.contains(name) {
            enabled_plugins.push(name.to_string());
            if let serde_yaml::Value::Mapping(ref mut map) = val {
                let seq = enabled_plugins.into_iter().map(serde_yaml::Value::String).collect();
                map.insert(serde_yaml::Value::String("plugins".to_string()), serde_yaml::Value::Sequence(seq));
            }
            std::fs::write(&app_yaml_path, serde_yaml::to_string(&val)?)?;
            println!("✅ Enabled plugin '{}'.", name);
            println!("🚀 Plugin configuration updated. Restart daemon to take effect if necessary.");
        } else {
            println!("ℹ️ Plugin '{}' is already enabled.", name);
        }
    } else {
        println!("❌ Plugin file for '{}' not found in {:?} (Ensure the exact filename without extension is provided)", name, plugins_dir);
    }

    Ok(())
}

pub async fn disable(name: &String) -> Result<()> {
    let app_yaml_path = get_app_dir().join("app.yaml");
    let content = std::fs::read_to_string(&app_yaml_path).unwrap_or_else(|_| "{}".to_string());
    let mut val = serde_yaml::from_str::<serde_yaml::Value>(&content).unwrap_or_else(|_| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
    
    let mut enabled_plugins: Vec<String> = val.get("plugins").and_then(|v| v.as_sequence()).map(|seq| seq.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
    if enabled_plugins.contains(name) {
        enabled_plugins.retain(|n| n != name);
        if let serde_yaml::Value::Mapping(ref mut map) = val {
            let seq = enabled_plugins.into_iter().map(serde_yaml::Value::String).collect();
            map.insert(serde_yaml::Value::String("plugins".to_string()), serde_yaml::Value::Sequence(seq));
        }
        std::fs::write(&app_yaml_path, serde_yaml::to_string(&val)?)?;
        println!("✅ Disabled plugin '{}'.", name);
        println!("🚀 Plugin configuration updated. Restart daemon to take effect if necessary.");
    } else {
        println!("ℹ️ Plugin '{}' is not currently enabled.", name);
    }

    Ok(())
}

pub async fn install(path: &String) -> Result<()> {
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

pub async fn refresh_signature(_name: &String) -> Result<()> {
    println!("⚠️ Signature verification and refresh is delegated to cowen-daemon in the thin CLI architecture.");
    println!("Please refer to daemon logs for validation status during startup.");
    Ok(())
}
