use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    #[cfg(target_os = "windows")]
    {
        // Legacy cleanup: If the old SYSTEM-level Windows Service exists, prompt UAC to delete it.
        // We do this in a separate elevated process so `setup.exe` remains in the current user session.
        let out1 = Command::new("sc").args(&["query", "cowen.exeDaemon"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        let out2 = Command::new("sc").args(&["query", "cowenDaemon"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        
        if out1.contains("cowen.exeDaemon") || out2.contains("cowenDaemon") {
            println!("⚠️ Detected legacy system service. Requesting Administrator privileges to remove it...");
            let script = "sc.exe stop cowen.exeDaemon; sc.exe delete cowen.exeDaemon; sc.exe stop cowenDaemon; sc.exe delete cowenDaemon; taskkill /F /T /IM cowen-daemon.exe; taskkill /F /T /IM cowen.exe";
            let _ = Command::new("powershell")
                .args(&[
                    "-NoProfile",
                    "-Command",
                    &format!("Start-Process powershell -ArgumentList '-NoProfile -Command \"{}\"' -Verb RunAs -Wait", script)
                ])
                .status();
            println!("✅ Legacy system service removed.");
        }
    }

    println!("🚀 Starting cowen installation...");
    
    // Kill running instances to avoid file lock issues on Windows
    #[cfg(target_os = "windows")]
    {
        println!("🛑 Stopping running instances...");

        println!("> sc stop cowen.exeDaemon");
        let _ = Command::new("sc")
            .args(&["stop", "cowen.exeDaemon"])
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();

        println!("> sc stop cowenDaemon");
        let _ = Command::new("sc")
            .args(&["stop", "cowenDaemon"])
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();

        let _ = Command::new("taskkill")
            .args(&["/F", "/T", "/IM", "cowen-daemon.exe"])
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();

        std::thread::sleep(std::time::Duration::from_millis(1000));
        
        let cmds = vec![
            "cowen.exe",
            "cowen-mcp-plugin.exe",
            "libcowen_search_embedding.exe"
        ];
        for p in cmds {
            println!("> taskkill /F /T /IM {}", p);
            let _ = Command::new("taskkill")
                .args(&["/F", "/T", "/IM", p])
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .status();
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

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
    
    #[cfg(has_search_exe)]
    let onnxruntime_dll = include_bytes!(r#"../../cowen/dist_assets/windows/onnxruntime.dll"#);
    #[cfg(not(has_search_exe))]
    let onnxruntime_dll: &[u8] = &[];

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
    if let Err(e) = fs::write(&dest, cowen_bytes) {
        println!("❌ Failed to write cowen.exe: {}. Please ensure it is not running and try again.", e);
        Command::new("cmd").args(&["/c", "pause"]).status().unwrap_or_default();
        std::process::exit(1);
    }
    println!("✅ Copied cowen.exe -> {}", dest.display());
    
    let daemon_dest = install_dir.join("cowen-daemon.exe");
    if let Err(e) = fs::write(&daemon_dest, daemon_bytes) {
        println!("❌ Failed to write cowen-daemon.exe: {}. Please ensure it is not running and try again.", e);
        Command::new("cmd").args(&["/c", "pause"]).status().unwrap_or_default();
        std::process::exit(1);
    }
    println!("✅ Copied cowen-daemon.exe -> {}", daemon_dest.display());
    
    // Install system plugins
    let system_plugins_dir = PathBuf::from(&home).join(".cowen").join("system_plugins");
    if !system_plugins_dir.exists() {
        fs::create_dir_all(&system_plugins_dir).unwrap_or_default();
    }
    
    let write_sys_plugin = |name: &str, data: &[u8]| {
        let dest_path = system_plugins_dir.join(name);
        if let Err(e) = fs::write(&dest_path, data) {
            println!("❌ Failed to write {}: {}", dest_path.display(), e);
            Command::new("cmd").args(&["/c", "pause"]).status().unwrap_or_default();
            std::process::exit(1);
        }
        println!("✅ Copied {} -> {}", name, dest_path.display());
    };
    
    println!("📦 Installing Wasm system plugins...");
    write_sys_plugin("cowen_wasm_auth_selfbuilt.wasm", selfbuilt_wasm_bytes);
    write_sys_plugin("cowen_wasm_auth_selfbuilt.bundle", selfbuilt_bundle_bytes);
    write_sys_plugin("cowen_wasm_auth_storeapp.wasm", storeapp_wasm_bytes);
    write_sys_plugin("cowen_wasm_auth_storeapp.bundle", storeapp_bundle_bytes);
    
    let plugins_dir = PathBuf::from(&home).join(".cowen").join("plugins");
    let mut plugins_installed = false;

    let write_plugin = |name: &str, data: &[u8]| {
        let dest_path = plugins_dir.join(name);
        if let Err(e) = fs::write(&dest_path, data) {
            println!("❌ Failed to write {}: {}", dest_path.display(), e);
            Command::new("cmd").args(&["/c", "pause"]).status().unwrap_or_default();
            std::process::exit(1);
        }
        println!("✅ Copied {} -> {}", name, dest_path.display());
    };

    if !search_exe_bytes.is_empty() {
        println!("🧩 Installing AI plugin...");
        if !plugins_dir.exists() { fs::create_dir_all(&plugins_dir).unwrap_or_default(); }
        write_plugin("libcowen_search_embedding.exe", search_exe_bytes);
        write_plugin("libcowen_search_embedding.bundle", search_exe_bundle);
        write_plugin("onnxruntime.dll", onnxruntime_dll);
        plugins_installed = true;
    }

    if !mcp_exe_bytes.is_empty() {
        println!("🧩 Installing MCP plugin...");
        if !plugins_dir.exists() { fs::create_dir_all(&plugins_dir).unwrap_or_default(); }
        write_plugin("cowen-mcp-plugin.exe", mcp_exe_bytes);
        write_plugin("cowen-mcp-plugin.bundle", mcp_bundle_bytes);
        plugins_installed = true;
    }

    // Setup Autostart Service
    println!("📟 Setting up autostart service...");
    let _ = Command::new(&dest)
        .args(&["daemon", "service", "install"])
        .status();


    if plugins_installed {
        // Use the actual binary name "libcowen_search_embedding" for enabling
        let _ = Command::new(&dest).args(&["plugins", "enable", "libcowen_search_embedding"]).status();
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



    #[cfg(target_os = "windows")]
    {
        println!("🧹 Cleaning up background services...");
        let _ = Command::new("taskkill")
            .args(&["/F", "/T", "/IM", "cowen-daemon.exe"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

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
