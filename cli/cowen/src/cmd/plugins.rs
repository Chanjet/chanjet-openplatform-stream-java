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
                
                if !cowen_infra::plugin::is_secure_plugin_path(&path) {
                    println!("{:<30} | {:<10} | {:<23} | {}", file_name, "\x1b[31mUNSAFE\x1b[0m", "\x1b[31mIgnored\x1b[0m", "Insecure file permissions or owner");
                    continue;
                }

                let mut display_trait = "unknown".to_string();
                let mut display_desc = String::new();

                match PluginLoader::new(&path) {
                    Ok(loader) => {
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
                    Err(e) => {
                        tracing::error!("ERROR LOADING PLUGIN {}: {:?}", file_name, e);
                        display_trait = "\x1b[31mSignature/Load Failed\x1b[0m".to_string();
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
    
    cowen_infra::sys::fs::make_executable(&target_path)?;

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

pub async fn refresh_signature(_cfg_mgr: &ConfigManager, name: &String) -> Result<()> {
    let plugins_dir = get_app_dir().join("plugins");
    
    let expected_path_dylib = plugins_dir.join(format!("{}.dylib", name));
    let expected_path_so = plugins_dir.join(format!("{}.so", name));
    let expected_path_dll = plugins_dir.join(format!("{}.dll", name));

    let target_path = if expected_path_dylib.exists() {
        expected_path_dylib
    } else if expected_path_so.exists() {
        expected_path_so
    } else if expected_path_dll.exists() {
        expected_path_dll
    } else {
        return Err(anyhow::anyhow!("❌ Plugin file for '{}' not found in {:?}", name, plugins_dir));
    };

    let bundle_path = target_path.with_extension("bundle");

    if !bundle_path.exists() {
        println!("⚠️  Missing .bundle signature file for {:?}", target_path);
    } else {
        println!("🔍 Re-verifying existing signature bundle...");
        match cowen_infra::pki::verify_plugin_bundle(&target_path) {
            Ok(_) => {
                println!("✅ Signature is perfectly valid. If it fails to load, it might be due to OS Quarantine.");
                #[cfg(target_os = "macos")]
                {
                    println!("   💡 Tip: macOS may block execution. You can manually run: xattr -d com.apple.quarantine {:?}", target_path);
                }
                return Ok(());
            }
            Err(e) => {
                println!("❌ Signature validation failed: {}", e);
            }
        }
    }

    if std::env::var("COWEN_DEV_MODE").unwrap_or_default() == "1" {
        println!("🛠️  COWEN_DEV_MODE is enabled. Attempting Ad-Hoc signing via cowen-signer...");
        
        let mut signer_path = std::env::current_exe()?
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .join("cowen-signer");
            
        if !signer_path.exists() {
            // fallback to workspace target
            signer_path = std::path::PathBuf::from("target/debug/cowen-signer");
            if !signer_path.exists() {
                signer_path = std::path::PathBuf::from("target/release/cowen-signer");
            }
        }

        if !signer_path.exists() {
            println!("❌ Cannot find cowen-signer binary. Please compile it first.");
            return Ok(());
        }

        // We assume dev keys exist in ./dist_assets/keys for ad-hoc signing
        let dev_key = std::path::PathBuf::from("dist_assets/keys/official_dev.pk8");
        let dev_cert = std::path::PathBuf::from("dist_assets/keys/official_dev_cert.json");
        
        if !dev_key.exists() || !dev_cert.exists() {
            println!("❌ Cannot find developer keys in ./dist_assets/keys. Please ensure you are in the workspace root.");
            return Ok(());
        }

        let output = std::process::Command::new(&signer_path)
            .arg("sign-plugin")
            .arg("--dylib").arg(&target_path)
            .arg("--name").arg(name)
            .arg("--version").arg("dev-refresh")
            .arg("--dev-key").arg(&dev_key)
            .arg("--dev-cert").arg(&dev_cert)
            .arg("--out-bundle").arg(&bundle_path)
            .output()?;

        if output.status.success() {
            println!("✅ Successfully generated Ad-Hoc signature for local development!");
        } else {
            println!("❌ Ad-Hoc signing failed: \n{}", String::from_utf8_lossy(&output.stderr));
        }

    } else {
        println!("❌ Security enforcement prevents generating official signatures automatically.");
        println!("💡 Tip: If you are a developer, export COWEN_DEV_MODE=1 and provide dev keys to generate an Ad-Hoc signature.");
        println!("💡 Tip: For end-users, please re-install the official signed version of the plugin.");
    }

    Ok(())
}
