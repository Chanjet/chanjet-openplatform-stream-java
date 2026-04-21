use anyhow::{Result, Context};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub enum ServiceAction {
    Install,
    Uninstall,
    Status,
}

pub async fn execute(action: ServiceAction) -> Result<()> {
    match action {
        ServiceAction::Install => install().await,
        ServiceAction::Uninstall => uninstall().await,
        ServiceAction::Status => status().await,
    }
}

async fn install() -> Result<()> {
    let bin_path = env::current_exe()?;
    let bin_name = crate::core::utils::get_bin_name();
    let bin_path_str = bin_path.to_string_lossy();

    if cfg!(target_os = "macos") {
        install_macos(&bin_name, &bin_path_str).await
    } else if cfg!(target_os = "linux") {
        install_linux(&bin_name, &bin_path_str).await
    } else if cfg!(target_os = "windows") {
        install_windows(&bin_name, &bin_path_str).await
    } else {
        anyhow::bail!("Unsupported OS for automatic service installation.")
    }
}

async fn uninstall() -> Result<()> {
    let bin_name = crate::core::utils::get_bin_name();

    if cfg!(target_os = "macos") {
        uninstall_macos(&bin_name).await
    } else if cfg!(target_os = "linux") {
        uninstall_linux(&bin_name).await
    } else if cfg!(target_os = "windows") {
        uninstall_windows(&bin_name).await
    } else {
        anyhow::bail!("Unsupported OS for automatic service uninstallation.")
    }
}

async fn status() -> Result<()> {
    let bin_name = crate::core::utils::get_bin_name();

    if cfg!(target_os = "macos") {
        status_macos(&bin_name).await
    } else if cfg!(target_os = "linux") {
        status_linux(&bin_name).await
    } else if cfg!(target_os = "windows") {
        status_windows(&bin_name).await
    } else {
        println!("⚠️  Service management is not supported on this OS.");
        Ok(())
    }
}

// --- macOS Implementation ---

fn get_macos_plist_path(bin_name: &str) -> Result<PathBuf> {
    let home = directories::UserDirs::new()
        .context("Could not find home directory")?
        .home_dir()
        .to_path_buf();
    Ok(home.join("Library").join("LaunchAgents").join(format!("com.chanjet.{}.daemon.plist", bin_name)))
}

async fn install_macos(bin_name: &str, bin_path: &str) -> Result<()> {
    let plist_path = get_macos_plist_path(bin_name)?;
    let app_dir = crate::core::config::get_app_dir();
    let log_dir = app_dir.join("logs");
    fs::create_dir_all(&log_dir)?;

    let plist_content = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.chanjet.{bin_name}.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin_path}</string>
        <string>daemon</string>
        <string>start</string>
        <string>--all</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{log_path}/service.log</string>
    <key>StandardErrorPath</key>
    <string>{log_path}/service.error.log</string>
</dict>
</plist>"#, 
    bin_name = bin_name,
    bin_path = bin_path,
    log_path = log_dir.to_string_lossy());

    if let Some(parent) = plist_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&plist_path, plist_content)?;

    // Load the service
    let status = Command::new("launchctl")
        .arg("load")
        .arg(&plist_path)
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("✅ Successfully installed and loaded macOS LaunchAgent.");
            println!("📍 Config: {:?}", plist_path);
        },
        _ => {
            println!("⚠️  Plist created at {:?}, but failed to load via launchctl.", plist_path);
            println!("💡 You may need to run: launchctl load {:?}", plist_path);
        }
    }
    Ok(())
}

async fn uninstall_macos(bin_name: &str) -> Result<()> {
    let plist_path = get_macos_plist_path(bin_name)?;
    if plist_path.exists() {
        let _ = Command::new("launchctl")
            .arg("unload")
            .arg(&plist_path)
            .status();
        fs::remove_file(&plist_path)?;
        println!("✅ Successfully uninstalled macOS LaunchAgent.");
    } else {
        println!("ℹ️  No service found to uninstall.");
    }
    Ok(())
}

async fn status_macos(bin_name: &str) -> Result<()> {
    let plist_path = get_macos_plist_path(bin_name)?;
    let label = format!("com.chanjet.{}.daemon", bin_name);

    println!("🔍 macOS Service Status:");
    println!("  - Label: {}", label);
    println!("  - Config: {}", if plist_path.exists() { "EXISTS" } else { "MISSING" });

    let output = Command::new("launchctl")
        .arg("list")
        .arg(&label)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            println!("  - Status: \x1b[32mRUNNING (REGISTERED)\x1b[0m");
        },
        _ => {
            println!("  - Status: \x1b[33mNOT REGISTERED\x1b[0m");
        }
    }
    Ok(())
}

// --- Linux Implementation ---

fn get_linux_service_path(bin_name: &str) -> Result<PathBuf> {
    let home = directories::UserDirs::new()
        .context("Could not find home directory")?
        .home_dir()
        .to_path_buf();
    Ok(home.join(".config").join("systemd").join("user").join(format!("{}-daemon.service", bin_name)))
}

async fn install_linux(bin_name: &str, bin_path: &str) -> Result<()> {
    let service_path = get_linux_service_path(bin_name)?;
    
    let service_content = format!(r#"[Unit]
Description={bin_name} Daemon Autostart
After=network.target

[Service]
Type=oneshot
ExecStart={bin_path} daemon start --all
RemainAfterExit=yes

[Install]
WantedBy=default.target
"#, 
    bin_name = bin_name,
    bin_path = bin_path);

    if let Some(parent) = service_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&service_path, service_content)?;

    // Reload and enable
    let _ = Command::new("systemctl").arg("--user").arg("daemon-reload").status();
    let status = Command::new("systemctl")
        .arg("--user")
        .arg("enable")
        .arg("--now")
        .arg(format!("{}-daemon", bin_name))
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("✅ Successfully installed and enabled systemd user service.");
            println!("📍 Config: {:?}", service_path);
        },
        _ => {
            println!("⚠️  Service file created at {:?}, but failed to enable via systemctl.", service_path);
        }
    }
    Ok(())
}

async fn uninstall_linux(bin_name: &str) -> Result<()> {
    let service_path = get_linux_service_path(bin_name)?;
    let unit_name = format!("{}-daemon", bin_name);

    if service_path.exists() {
        let _ = Command::new("systemctl").arg("--user").arg("disable").arg("--now").arg(&unit_name).status();
        fs::remove_file(&service_path)?;
        let _ = Command::new("systemctl").arg("--user").arg("daemon-reload").status();
        println!("✅ Successfully uninstalled systemd user service.");
    } else {
        println!("ℹ️  No service found to uninstall.");
    }
    Ok(())
}

async fn status_linux(bin_name: &str) -> Result<()> {
    let service_path = get_linux_service_path(bin_name)?;
    let unit_name = format!("{}-daemon", bin_name);

    println!("🔍 Linux systemd Status:");
    println!("  - Unit: {}", unit_name);
    println!("  - Config: {}", if service_path.exists() { "EXISTS" } else { "MISSING" });

    let output = Command::new("systemctl")
        .arg("--user")
        .arg("is-active")
        .arg(&unit_name)
        .output();

    match output {
        Ok(out) => {
            let state = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if state == "active" {
                println!("  - Status: \x1b[32mACTIVE\x1b[0m");
            } else {
                println!("  - Status: \x1b[33m{}\x1b[0m", state);
            }
        },
        _ => {
            println!("  - Status: UNKNOWN");
        }
    }
    Ok(())
}

// --- Windows Implementation ---

async fn install_windows(bin_name: &str, bin_path: &str) -> Result<()> {
    let task_name = format!("{}DaemonAutostart", bin_name);
    let bin_name_caps = bin_name.to_uppercase();
    
    // schtasks /create /tn CowenDaemonAutostart /tr "'C:\path\to\cowen.exe' daemon start --all" /sc onlogon /f
    let status = Command::new("schtasks")
        .arg("/create")
        .arg("/tn")
        .arg(&task_name)
        .arg("/tr")
        .arg(format!("\"{}\" daemon start --all", bin_path))
        .arg("/sc")
        .arg("onlogon")
        .arg("/f")
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("✅ Successfully installed Task Scheduler entry for Windows.");
            println!("📍 Task Name: {}", task_name);
            println!("💡 {} will now start automatically whenever you log in.", bin_name_caps);
        },
        _ => {
            anyhow::bail!("Failed to create scheduled task via schtasks. Make sure you have necessary permissions.");
        }
    }
    Ok(())
}

async fn uninstall_windows(bin_name: &str) -> Result<()> {
    let task_name = format!("{}DaemonAutostart", bin_name);
    let status = Command::new("schtasks")
        .arg("/delete")
        .arg("/tn")
        .arg(&task_name)
        .arg("/f")
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("✅ Successfully uninstalled Task Scheduler entry.");
        },
        _ => {
            println!("ℹ️  No scheduled task found to uninstall.");
        }
    }
    Ok(())
}

async fn status_windows(bin_name: &str) -> Result<()> {
    let task_name = format!("{}DaemonAutostart", bin_name);

    println!("🔍 Windows Task Scheduler Status:");
    println!("  - Task Name: {}", task_name);

    let output = Command::new("schtasks")
        .arg("/query")
        .arg("/tn")
        .arg(&task_name)
        .arg("/fo")
        .arg("list")
        .output();

    match output {
        Ok(out) if out.status.success() => {
            println!("  - Status: \x1b[32mREGISTERED\x1b[0m");
        },
        _ => {
            println!("  - Status: \x1b[33mNOT REGISTERED\x1b[0m");
        }
    }
    Ok(())
}
