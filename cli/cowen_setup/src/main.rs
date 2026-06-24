use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("🚀 Starting cowen installation...");
    
    // Embed binaries
    let cowen_bytes = include_bytes!(r#"../../../bin/windows-x86_64/cowen.exe"#);
    let daemon_bytes = include_bytes!(r#"../../../bin/windows-x86_64/cowen-daemon.exe"#);
    
    #[cfg(has_search_exe)]
    let search_exe_bytes = include_bytes!(r#"../../../bin/windows-x86_64/libcowen_search_embedding.exe"#);
    #[cfg(not(has_search_exe))]
    let search_exe_bytes: &[u8] = &[];
    
    #[cfg(has_search_exe)]
    let search_exe_bundle = include_bytes!(r#"../../../bin/windows-x86_64/libcowen_search_embedding.bundle"#);
    #[cfg(not(has_search_exe))]
    let search_exe_bundle: &[u8] = &[];

    #[cfg(has_mcp_exe)]
    let mcp_exe_bytes = include_bytes!(r#"../../../bin/windows-x86_64/cowen-mcp-plugin.exe"#);
    #[cfg(not(has_mcp_exe))]
    let mcp_exe_bytes: &[u8] = &[];
    
    #[cfg(has_mcp_exe)]
    let mcp_bundle_bytes = include_bytes!(r#"../../../bin/windows-x86_64/cowen-mcp-plugin.bundle"#);
    #[cfg(not(has_mcp_exe))]
    let mcp_bundle_bytes: &[u8] = &[];
    
    // Embed system plugins
    let selfbuilt_wasm_bytes = include_bytes!(r#"../../cowen/target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm"#);
    let selfbuilt_bundle_bytes = include_bytes!(r#"../../cowen/target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.bundle"#);
    let storeapp_wasm_bytes = include_bytes!(r#"../../cowen/target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.wasm"#);
    let storeapp_bundle_bytes = include_bytes!(r#"../../cowen/target/wasm32-wasip1/release/cowen_wasm_auth_storeapp.bundle"#);
    
    let home = std::env::var("USERPROFILE").expect("Failed to find USERPROFILE");
    let install_dir = PathBuf::from(&home).join(".cowen").join("bin");
    
    if !install_dir.exists() {
        fs::create_dir_all(&install_dir).expect("Failed to create install directory");
    }
    
    let dest = install_dir.join("cowen.exe");
    fs::write(&dest, cowen_bytes).expect("Failed to write cowen.exe");
    println!("✅ Copied cowen.exe to {}", install_dir.display());
    
    let daemon_dest = install_dir.join("cowen-daemon.exe");
    fs::write(&daemon_dest, daemon_bytes).expect("Failed to write cowen-daemon.exe");
    println!("✅ Copied cowen-daemon.exe to {}", install_dir.display());
    
    // Install system plugins
    let system_plugins_dir = PathBuf::from(&home).join(".cowen").join("system_plugins");
    if !system_plugins_dir.exists() {
        fs::create_dir_all(&system_plugins_dir).expect("Failed to create system_plugins directory");
    }
    fs::write(system_plugins_dir.join("cowen_wasm_auth_selfbuilt.wasm"), selfbuilt_wasm_bytes)
        .expect("Failed to write cowen_wasm_auth_selfbuilt.wasm");
    fs::write(system_plugins_dir.join("cowen_wasm_auth_selfbuilt.bundle"), selfbuilt_bundle_bytes)
        .expect("Failed to write cowen_wasm_auth_selfbuilt.bundle");
    fs::write(system_plugins_dir.join("cowen_wasm_auth_storeapp.wasm"), storeapp_wasm_bytes)
        .expect("Failed to write cowen_wasm_auth_storeapp.wasm");
    fs::write(system_plugins_dir.join("cowen_wasm_auth_storeapp.bundle"), storeapp_bundle_bytes)
        .expect("Failed to write cowen_wasm_auth_storeapp.bundle");
    println!("📦 Installed Wasm system plugins to {}", system_plugins_dir.display());
    
    let plugins_dir = PathBuf::from(&home).join(".cowen").join("plugins");
    let mut plugins_installed = false;

    if !search_exe_bytes.is_empty() {
        if !plugins_dir.exists() { fs::create_dir_all(&plugins_dir).unwrap(); }
        fs::write(plugins_dir.join("libcowen_search_embedding.exe"), search_exe_bytes).unwrap();
        fs::write(plugins_dir.join("libcowen_search_embedding.bundle"), search_exe_bundle).unwrap();
        println!("🧩 Installed AI plugin (EXE) to {}", plugins_dir.display());
        plugins_installed = true;
    }

    if !mcp_exe_bytes.is_empty() {
        if !plugins_dir.exists() { fs::create_dir_all(&plugins_dir).unwrap(); }
        fs::write(plugins_dir.join("cowen-mcp-plugin.exe"), mcp_exe_bytes).unwrap();
        fs::write(plugins_dir.join("cowen-mcp-plugin.bundle"), mcp_bundle_bytes).unwrap();
        println!("🧩 Installed MCP plugin to {}", plugins_dir.display());
        plugins_installed = true;
    }

    if plugins_installed {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let _ = Command::new(&dest).args(&["plugins", "enable", "cowen-search-embedding"]).status();
        let _ = Command::new(&dest).args(&["plugins", "enable", "cowen-mcp-plugin"]).status();
        println!("✅ Plugins enabled.");
    }
    
    // Add to User PATH
    let current_path = get_user_path();
    let install_dir_str = install_dir.to_string_lossy().to_string();
    
    if !current_path.contains(&install_dir_str) {
        println!("Adding {} to User PATH...", install_dir_str);
        let new_path = if current_path.is_empty() {
            install_dir_str
        } else {
            format!("{};{}", current_path, install_dir_str)
        };
        
        let status = Command::new("powershell")
            .args(&["-NoProfile", "-Command", &format!("[Environment]::SetEnvironmentVariable('Path', '{}', 'User')", new_path)])
            .status();
            
        if status.is_ok() && status.unwrap().success() {
            println!("✅ PATH updated successfully.");
        } else {
            println!("⚠️ Failed to update PATH. You may need to add it manually.");
        }
    } else {
        println!("ℹ️ {} is already in PATH.", install_dir_str);
    }
    
    // Add powershell completion
    let doc_dir = std::env::var("USERPROFILE").expect("USERPROFILE").to_string() + r#"\Documents\WindowsPowerShell"#;
    let profile_path = PathBuf::from(&doc_dir).join("Microsoft.PowerShell_profile.ps1");
    
    if !PathBuf::from(&doc_dir).exists() {
        fs::create_dir_all(&doc_dir).ok();
    }
    
    let completion_cmd = "\n# cowen completion\nif (Get-Command cowen -ErrorAction SilentlyContinue) { cowen completion powershell | Out-String | Invoke-Expression }\n";
    
    let profile_content = fs::read_to_string(&profile_path).unwrap_or_default();
    if !profile_content.contains("# cowen completion") {
        println!("⚙️ Setting up PowerShell auto-completion...");
        if let Ok(_) = fs::write(&profile_path, profile_content + completion_cmd) {
            println!("✅ Auto-completion added to your PowerShell profile.");
        }
    }

    // Setup Autostart Service
    println!("📟 Setting up autostart service...");
    let _ = Command::new(&dest)
        .args(&["daemon", "service", "install"])
        .status();

    println!("\n🎉 Installation complete! Please RESTART your terminal.");
    Command::new("cmd").args(&["/c", "pause"]).status().unwrap();
}

fn get_user_path() -> String {
    let output = Command::new("powershell")
        .args(&["-NoProfile", "-Command", "[Environment]::GetEnvironmentVariable('Path', 'User')"])
        .output();
        
    if let Ok(out) = output {
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    } else {
        "".to_string()
    }
}
