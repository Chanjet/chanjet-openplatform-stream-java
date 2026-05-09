use serde::Serialize;
use anyhow::Result;
use std::sync::Arc;
use crate::vault::Vault;
use crate::config::Config;
use sysinfo::System;
use std::path::PathBuf;

#[derive(Debug, Serialize, Clone, Copy, PartialEq)]
pub enum StatusLevel {
    OK,
    WARN,
    ERROR,
    #[allow(dead_code)]
    PENDING,
    NONE,
}

pub trait AsStatusUI {
    fn ui(&self) -> (String, String);
}

#[derive(Debug, Serialize, Clone)]
pub struct StatusEntry {
    pub name: String,
    pub icon: String,
    pub level: StatusLevel,
    pub message: String,
    pub reason: Option<String>,
    pub details: Vec<String>,
    pub children: Vec<StatusEntry>,
}

impl StatusEntry {
    pub fn new(template: impl AsStatusUI, level: StatusLevel, message: String) -> Self {
        let (name, icon) = template.ui();
        Self {
            name,
            icon,
            level,
            message,
            reason: None,
            details: vec![],
            children: vec![],
        }
    }

    pub fn with_reason(mut self, reason: Option<String>) -> Self {
        self.reason = reason;
        self
    }

    pub fn with_details(mut self, details: Vec<String>) -> Self {
        self.details = details;
        self
    }

    pub fn with_children(mut self, children: Vec<StatusEntry>) -> Self {
        self.children = children;
        self
    }
}

pub enum CommonTemplate {
    Configuration,
    Storage,
    Cache,
    Daemon(String),        // display_name
    ProactiveRefresh,
    BridgeConnection,
    ProviderSummary(String, String), // dynamic_name, dynamic_icon
    Custom(String, String), // name, icon
}

impl AsStatusUI for CommonTemplate {
    fn ui(&self) -> (String, String) {
        match self {
            Self::Configuration => ("Configuration".to_string(), "⚙️".to_string()),
            Self::Storage => ("Storage".to_string(), "📦".to_string()),
            Self::Cache => ("Cache".to_string(), "⚡".to_string()),
            Self::Daemon(name) => (name.clone(), "📟".to_string()),
            Self::ProactiveRefresh => ("Proactive Refresh".to_string(), "🔄".to_string()),
            Self::BridgeConnection => ("Bridge Connection".to_string(), "🌐".to_string()),
            Self::ProviderSummary(name, icon) => (name.clone(), icon.clone()),
            Self::Custom(name, icon) => (name.clone(), icon.clone()),
        }
    }
}

pub struct StatusContext<'a> {
    pub profile: String,
    pub config: &'a Config,
    pub app_config: &'a crate::config::AppConfig,
    pub vault: Arc<dyn Vault>,
}

#[async_trait::async_trait]
pub trait StatusCollector: Send + Sync {
    #[allow(dead_code)]
    fn name(&self) -> &str;
    async fn collect(&self, ctx: &StatusContext<'_>) -> Result<StatusEntry>;
}

// --- Helpers for Providers ---

pub fn get_app_dir() -> PathBuf {
    crate::config::get_app_dir()
}

pub async fn get_active_daemon_info(profile: &str) -> (Option<u32>, Option<String>) {
        let app_dir = get_app_dir();
        let pid_file = app_dir.join(format!("{}_daemon.pid", profile));

    if !pid_file.exists() {
                return (None, None);
    }

    // BUG FIX: Use file locking for reliable liveness detection.
    // If we CAN acquire an exclusive lock, it means the daemon is NOT running.
    let is_alive =     if let Ok(f) = std::fs::File::open(&pid_file) {
        use fs2::FileExt;
        let lock_res = f.try_lock_exclusive();
                lock_res.is_err()
    } else {
        false
    };

    if !is_alive {
        return (None, None);
    }

    if let Ok(pid_content) = std::fs::read_to_string(&pid_file) {
        let mut lines = pid_content.lines();
        if let Some(pid_str) = lines.next() {
            if let Ok(pid_val) = pid_str.trim().parse::<u32>() {
                // Secondary check: verify the process actually exists and looks like us.
                let mut s = System::new_all();
                s.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
                if let Some(process) = s.process(sysinfo::Pid::from_u32(pid_val)) {
                    let cmdline = process.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" ");
                    let name = process.name().to_string_lossy().to_lowercase();
                    let bin_name = crate::utils::get_bin_name().to_lowercase();
                    
                    if name.contains(&bin_name) || cmdline.contains(&bin_name) {
                        let build_id = lines.next().map(|s| s.trim().to_string());
                        return (Some(pid_val), build_id);
                    }
                }
            }
        }
    }
    (None, None)
}

pub async fn is_port_responsive(port: u16) -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    match tokio::time::timeout(
        std::time::Duration::from_secs(1),
        tokio::net::TcpStream::connect(addr)
    ).await {
        Ok(Ok(_)) => true,
        _ => false,
    }
}

pub async fn collect_daemon_status(
    ctx: &StatusContext<'_>,
    display_name: &str,
    efficiency_tip: &str,
    supports_webhooks: bool,
) -> Result<StatusEntry> {
    let (found_daemon_pid, found_build_id) = get_active_daemon_info(&ctx.profile).await;
    
    let (mut level, msg, mut children) = if let Some(pid) = found_daemon_pid {
        (
            StatusLevel::OK, 
            format!("[RUNNING] (PID: {})", pid),
            vec![
                StatusEntry::new(
                    CommonTemplate::ProactiveRefresh,
                    StatusLevel::OK,
                    format!("{} 令牌环境将保持热启动状态", efficiency_tip)
                )
            ]
        )
    } else {
        (
            StatusLevel::WARN, 
            "[OFFLINE] (未检测到活跃后台进程)".to_string(),
            vec![
                StatusEntry::new(
                    CommonTemplate::Custom("Efficiency Tip".to_string(), "💡".to_string()),
                    StatusLevel::WARN,
                    efficiency_tip.to_string()
                )
            ]
        )
    };

    // Inject Connection State if running AND new version (status file exists)
    if found_daemon_pid.is_some() {
        let status_file = get_app_dir().join(format!("{}_status.json", ctx.profile));
        if let Ok(content) = std::fs::read_to_string(status_file) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                let conn_state = json.get("state").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
                
                // Freshness check: If the status file is older than 1 minute, it's considered stale
                let is_fresh = if let Some(ts_str) = json.get("updated_at").and_then(|v| v.as_str()) {
                    if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(ts_str) {
                        chrono::Utc::now().signed_duration_since(ts).num_seconds() < 60
                    } else { false }
                } else { false };

                let (conn_level, conn_icon_override, final_state) = if !is_fresh {
                    (StatusLevel::WARN, Some("💤"), format!("{} (Stale)", conn_state))
                } else {
                    match conn_state.as_str() {
                        "Connected" => (StatusLevel::OK, None, conn_state),
                        "Connecting" => (StatusLevel::OK, Some("⏳"), conn_state), // OK during startup
                        "Disconnected" => (StatusLevel::WARN, Some("💤"), conn_state),
                        "Reconnecting" => (StatusLevel::ERROR, Some("📡"), conn_state),
                        _ => (StatusLevel::WARN, Some("❓"), conn_state),
                    }
                };

                if conn_level as i32 > level as i32 && conn_level != StatusLevel::WARN {
                    level = conn_level;
                }

                if supports_webhooks {
                    let mut entry = StatusEntry::new(CommonTemplate::BridgeConnection, conn_level, final_state);
                    if let Some(icon) = conn_icon_override {
                        entry.icon = icon.to_string();
                    }
                    children.push(entry);
                }
            }
        }
        // COMPATIBILITY: If status file is missing, we don't show Bridge Connection at all 
        // to match v0.2.1 behavior.
    }

    let mut details = vec![];
    if let Some(bid) = found_build_id {
        details.push(format!("Build ID: {}", bid));
    }

    Ok(StatusEntry::new(CommonTemplate::Daemon(display_name.to_string()), level, msg)
        .with_reason(if found_daemon_pid.is_none() { 
            Some("Daemon 未启动，后台自动化能力（续约/桥接）已禁用。".to_string()) 
        } else if level == StatusLevel::ERROR {
            Some("Daemon 已启动，但当前连接状态异常。".to_string())
        } else { 
            None 
        })
        .with_details(details)
        .with_children(children))
}
