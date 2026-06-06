use crate::CowenResult;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::vault::Vault;
use crate::config::Config;
use sysinfo::System;
use std::path::PathBuf;
use cowen_infra::path::get_app_dir;
use cowen_infra::process::{get_bin_name, check_port_occupancy, extract_profile_from_cmdline};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    OK,
    WARN,
    ERROR,
    PENDING,
    NONE,
}

pub trait AsStatusUI {
    fn ui(&self) -> (String, String);
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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
    fn name(&self) -> &str;
    async fn collect(&self, ctx: &StatusContext<'_>) -> CowenResult<StatusEntry>;
}

// --- Helpers for Providers ---

pub fn get_daemon_app_dir() -> PathBuf {
    get_app_dir()
}

pub struct DaemonInfo {
    pub pid: u32,
    pub build_id: Option<String>,
    pub build_time: Option<String>,
    pub monitor_port: Option<u16>,
    pub start_time: Option<String>,
    pub last_error: Option<String>,
}

pub fn get_active_daemon_info(profile: &str) -> Option<DaemonInfo> {
    let app_dir = get_daemon_app_dir();
    
    // Check for unified master daemon first
    let master_pid_file = app_dir.join("master_daemon.pid");
    println!("Checking pid file: {:?}", master_pid_file); if master_pid_file.exists() {
        if let Some(info) = read_daemon_info(&master_pid_file) {
            return Some(info);
        }
    }

    // FALLBACK: Check for legacy profile-specific pid file
    let pid_file = app_dir.join(format!("{}_daemon.pid", profile));
    if pid_file.exists() {
        return read_daemon_info(&pid_file);
    }

    None
}

fn read_daemon_info(pid_file: &std::path::Path) -> Option<DaemonInfo> {
    if let Ok(pid_content) = std::fs::read_to_string(pid_file) {
        let mut lines = pid_content.lines();
        if let Some(pid_str) = lines.next() {
            if let Ok(pid_val) = pid_str.trim().parse::<u32>() { println!("Parsed PID: {}", pid_val);
                // Secondary check: verify the process actually exists and looks like us.
                let mut s = System::new();
                let sys_pid = sysinfo::Pid::from_u32(pid_val);
                s.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[sys_pid]), true);

                if let Some(process) = s.process(sys_pid) { println!("Process: {}", process.name().to_string_lossy());
                    let name = process.name().to_string_lossy();
                    let is_target = crate::utils::is_cowen_process_name(&name, None);
                    if is_target {
                        let mut info = DaemonInfo {
                            pid: pid_val,
                            build_id: None,
                            build_time: None,
                            monitor_port: None,
                            start_time: None,
                            last_error: None,
                        };
                        
                        for line in lines {
                            if let Some(bid) = line.strip_prefix("BUILD_ID=") {
                                info.build_id = Some(bid.trim().to_string());
                            } else if let Some(bt) = line.strip_prefix("BUILD_TIME=") {
                                info.build_time = Some(bt.trim().to_string());
                            } else if let Some(mp) = line.strip_prefix("MONITOR_PORT=") {
                                info.monitor_port = mp.trim().parse::<u16>().ok();
                            } else if let Some(st) = line.strip_prefix("START_TIME=") {
                                info.start_time = Some(st.trim().to_string());
                            } else if let Some(le) = line.strip_prefix("LAST_ERROR=") {
                                let err_msg = le.trim().to_string();
                                if !err_msg.is_empty() {
                                    info.last_error = Some(err_msg);
                                }
                            }
                        }
                        return Some(info);
                    }
                }
            }
        }
    }
    None
}

pub async fn is_port_responsive(port: u16) -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    matches!(
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            tokio::net::TcpStream::connect(addr)
        ).await,
        Ok(Ok(_))
    )
}

pub async fn collect_daemon_status(
    ctx: &StatusContext<'_>,
    display_name: &str,
    efficiency_tip: &str,
    supports_webhooks: bool,
    daemon_info: Option<DaemonInfo>,
) -> CowenResult<StatusEntry> {
    let daemon_info = daemon_info.or_else(|| get_active_daemon_info(&ctx.profile));
    
    let (mut level, msg, mut children, port_conflict) = if let Some(info) = &daemon_info {
        (
            StatusLevel::OK, 
            format!("[RUNNING] (PID: {})", info.pid),
            vec![
                StatusEntry::new(
                    CommonTemplate::ProactiveRefresh,
                    StatusLevel::OK,
                    "主动续约: [ACTIVE] 令牌环境将保持热启动状态".to_string()
                )
            ],
            None
        )
    } else {
        let mut level = StatusLevel::WARN;
        let mut port_conflict = None;
        
        // DIAGNOSTICS: Check if the configured port is stolen by another process
        if ctx.config.proxy_enabled {
            let bin_name = get_bin_name();
            if let Some((other_pid, other_name)) = check_port_occupancy(ctx.config.proxy_port, &bin_name) {
                level = StatusLevel::ERROR;
                if other_name.to_lowercase().contains(&bin_name.to_lowercase()) {
                    let other_profile = extract_profile_from_cmdline(other_pid).unwrap_or_else(|| "unknown".to_string());
                    port_conflict = Some(format!("端口冲突: 代理端口 {} 已被 Profile '{}' (PID: {}) 占用。", ctx.config.proxy_port, other_profile, other_pid));
                } else {
                    port_conflict = Some(format!("端口冲突: 代理端口 {} 已被进程 '{}' (PID: {}) 占用。", ctx.config.proxy_port, other_name, other_pid));
                }
            }
        }

        (
            level, 
            "[OFFLINE] (未检测到活跃后台进程)".to_string(),
            vec![
                StatusEntry::new(
                    CommonTemplate::Custom("Efficiency Tip".to_string(), "💡".to_string()),
                    StatusLevel::WARN,
                    efficiency_tip.to_string()
                )
            ],
            port_conflict
        )
    };

    let mut captured_proxy_port = None;

    // Inject Connection State if running AND new version (status file exists)
    if daemon_info.is_some() {
        let status_file = get_app_dir().join(format!("{}_status.json", ctx.profile));
        if let Ok(content) = std::fs::read_to_string(status_file) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(p) = json.get("proxy_port").and_then(|v| v.as_u64()) {
                    captured_proxy_port = Some(p);
                }
                
                let conn_state = json.get("state").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
                let error_val = json.get("error").and_then(|v| v.as_str()).map(|s| s.to_string());
                
                // Freshness check: If the status file is older than 1 minute, it's considered stale
                let is_fresh = if let Some(ts_str) = json.get("updated_at").and_then(|v| v.as_str()) {
                    if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(ts_str) {
                        chrono::Utc::now().signed_duration_since(ts).num_seconds() < 60
                    } else { false }
                } else { false };

                let (mut conn_level, conn_icon_override, mut final_state) = if supports_webhooks && !is_fresh && conn_state == "Connected" {
                    (StatusLevel::ERROR, Some("💤"), format!("{} (Stale)", conn_state))
                } else {
                    match conn_state.as_str() {
                        "Connected" => (StatusLevel::OK, None, conn_state),
                        "Connecting" => (StatusLevel::WARN, Some("⏳"), conn_state), 
                        "Disconnected" => (StatusLevel::WARN, Some("💤"), conn_state),
                        "Reconnecting" => (StatusLevel::ERROR, Some("📡"), conn_state),
                        "Active" if !supports_webhooks => (StatusLevel::OK, None, conn_state),
                        _ => (StatusLevel::WARN, Some("❓"), conn_state),
                    }
                };

                if let Some(ref err) = error_val {
                    if err.contains("404") || err.contains("Nonce") || err.contains("401") || err.contains("403") {
                        conn_level = StatusLevel::ERROR;
                    }
                    final_state = format!("{} (Error: {})", final_state, err);
                }

                if conn_level as i32 > level as i32 && conn_level != StatusLevel::WARN {
                    level = conn_level;
                }

                if supports_webhooks {
                    let mut entry = StatusEntry::new(CommonTemplate::BridgeConnection, conn_level, final_state);
                    if let Some(icon) = conn_icon_override {
                        entry.icon = icon.to_string();
                    }
                    if let Some(err) = &error_val {
                        entry.details.push(format!("Error Details: {}", err));
                    }
                    if let Some(cid) = json.get("client_id").and_then(|v| v.as_str()) {
                        entry.details.push(format!("Client ID: {}", cid));
                    }
                    children.push(entry);
                }
            }
        }
    }

    let mut details = vec![];
    let mut outdated = false;
    if let Some(info) = &daemon_info {
        if let Some(bid) = &info.build_id {
            details.push(format!("Daemon Build: {}", bid));
        }
        if let Some(bt) = &info.build_time {
            details.push(format!("Daemon Time:  {}", bt));
        }
        if let Some(p) = captured_proxy_port {
            details.push(format!("Proxy Port:   {}", p));
        } else if ctx.config.proxy_enabled && ctx.config.proxy_port != 0 {
            details.push(format!("Proxy Port:   {} (configured)", ctx.config.proxy_port));
        }

        // Version Sync Logic:
        if let Some(bid) = &info.build_id {
            if bid != crate::BUILD_ID {
                outdated = true;
            }
        } else {
            outdated = true; 
        }

        if !outdated
            && (info.build_id.as_deref() != Some(crate::BUILD_ID)
                || info.build_time.as_deref() != Some(crate::BUILD_TIME))
        {
            outdated = true;
        }
    }

    let res = StatusEntry::new(CommonTemplate::Daemon(display_name.to_string()), level, msg)
        .with_reason(if daemon_info.is_none() { 
            if let Some(conflict) = port_conflict {
                Some(conflict)
            } else {
                Some("Daemon 未启动，后台自动化能力（续约/桥接）已禁用。".to_string()) 
            }
        } else if outdated {
            Some("⚠️ 当前后台进程版本已过时。建议运行 'cowen daemon restart' 以同步最新功能。".to_string())
        } else if level == StatusLevel::ERROR {
            Some("Daemon 已启动，但当前连接状态异常。".to_string())
        } else { 
            None 
        })
        .with_details(details)
        .with_children(children);

    Ok(res)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProgressInfo {
    pub profile: String,
    pub status: AuthStatus,
    pub message: String,
    pub percent: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthStatus {
    Idle,
    Starting,
    Exchanging,
    Saving,
    Completed,
    Failed,
}

#[derive(Serialize, Deserialize)]
pub struct FinalizeRequest {
    pub profile: String,
    pub code: String,
    pub state: Option<String>,
    pub session_id: String,
}

#[derive(Deserialize)]
pub struct ProgressQuery {
    pub profile: String,
}

#[cfg(feature = "reqwest")]
pub struct MonitorClient {
    base_url: String,
    http: reqwest::Client,
}

#[cfg(feature = "reqwest")]
impl MonitorClient {
    pub fn new(port: u16) -> Self {
        Self {
            base_url: format!("http://127.0.0.1:{}", port),
            http: reqwest::Client::new(),
        }
    }

    pub async fn reload_worker(&self, profile: &str) -> CowenResult<()> {
        let url = format!("{}/daemon/reload?profile={}", self.base_url, profile);
        let resp = self.http.post(&url).send().await
            .map_err(|e| crate::CowenError::api(format!("Failed to connect to monitor: {}", e)))?;
        
        if resp.status().is_success() {
            Ok(())
        } else {
            let err = resp.text().await.unwrap_or_default();
            Err(crate::CowenError::api(format!("Monitor reload failed: {}", err)))
        }
    }

    pub async fn finalize_auth(&self, profile: &str, code: &str, state: Option<&str>, session_id: &str) -> CowenResult<()> {
        let url = format!("{}/v1/mgmt/auth/finalize", self.base_url);
        let req = FinalizeRequest {
            profile: profile.to_string(),
            code: code.to_string(),
            state: state.map(|s| s.to_string()),
            session_id: session_id.to_string(),
        };

        let resp = self.http.post(&url).json(&req).send().await
            .map_err(|e| crate::CowenError::api(format!("Failed to connect to monitor for finalization: {}", e)))?;
        
        if resp.status().is_success() {
            Ok(())
        } else {
            let err = resp.text().await.unwrap_or_default();
            Err(crate::CowenError::api(format!("Auth finalization failed: {}", err)))
        }
    }

    pub async fn get_auth_progress(&self, profile: &str) -> CowenResult<AuthProgressInfo> {
        let url = format!("{}/v1/mgmt/auth/progress?profile={}", self.base_url, profile);
        let resp = self.http.get(&url).send().await
            .map_err(|e| crate::CowenError::api(format!("Failed to connect to monitor for progress: {}", e)))?;
        
        if resp.status().is_success() {
            Ok(resp.json().await.map_err(|e| crate::CowenError::api(format!("Failed to parse progress JSON: {}", e)))?)
        } else {
            let err = resp.text().await.unwrap_or_default();
            Err(crate::CowenError::api(format!("Progress query failed: {}", err)))
        }
    }
}
