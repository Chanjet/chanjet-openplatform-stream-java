use crate::core::config::{ConfigManager};
use crate::core::vault::Vault;
use anyhow::{Result};

pub async fn status(
    profile: &str,
    cfg_mgr: &crate::core::config::ConfigManager,
    vault: &dyn Vault,
) -> Result<()> {
    println!("🔍 CJTC System Status Diagnostics (Profile: '{}')", profile);
    println!("--------------------------------------------------");

    // 1. Config Check
    let cfg = cfg_mgr.load(profile)?;
    if cfg.app_key.is_empty() {
        println!("  ⚙️  Configuration: [MISSING] Profile not initialized or AppKey empty.");
    } else {
        println!("  ⚙️  Configuration: [OK] AppKey: {}", cfg.app_key);
        println!("     - OpenAPI: {}", cfg.openapi_url);
        println!("     - Stream:  {}", cfg.stream_url);
    }

    // 2. Vault Security Check
    let mut missing_secrets = Vec::new();
    if vault.get(profile, "app_secret").is_err() { missing_secrets.push("AppSecret"); }
    if vault.get(profile, "certificate").is_err() { missing_secrets.push("Certificate"); }
    if vault.get(profile, "encrypt_key").is_err() { missing_secrets.push("EncryptKey"); }

    if missing_secrets.is_empty() {
        println!("  🛡️  Security (Vault): [OK] All core secrets are securely stored.");
    } else {
        println!("  🛡️  Security (Vault): [PARTIAL] Missing: {}", missing_secrets.join(", "));
    }

    // 3. Daemon Check
    use sysinfo::{ProcessExt, System, SystemExt, PidExt};
    let mut s = System::new_all();
    s.refresh_processes();
    let current_pid = std::process::id();
    let mut found_daemon_pid = None;

    for (pid, process) in s.processes() {
        let pid_u32 = pid.as_u32();
        if pid_u32 != current_pid && (process.name().contains("cjtc") || process.name().contains("cjtcr")) {
            let cmdline = process.cmd().join(" ");
            if cmdline.contains("daemon") && cmdline.contains("start") {
                found_daemon_pid = Some(pid_u32);
                break;
            }
        }
    }

    match found_daemon_pid {
        Some(pid) => println!("  📟 Daemon Process: [RUNNING] (PID: {})", pid),
        None => println!("  📟 Daemon Process: [OFFLINE] (未检测到活跃后台进程)"),
    }
    
    println!("--------------------------------------------------");
    Ok(())
}

pub async fn config(
    _profile: &str,
    cfg_mgr: &ConfigManager,
) -> Result<()> {
    let cfg = cfg_mgr.load(_profile)?;
    println!("{:#?}", cfg);
    Ok(())
}

pub async fn reset(
    _profile: &str,
    cfg_mgr: &ConfigManager,
    _vault: &dyn Vault,
) -> Result<()> {
    println!("Resetting profile '{}'...", _profile);
    cfg_mgr.save(_profile, &crate::core::config::Config::default_with_profile(_profile))?;
    println!("Done.");
    Ok(())
}
