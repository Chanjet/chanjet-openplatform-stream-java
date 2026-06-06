pub mod mock;

pub mod traits {
    use std::path::Path;
    use tokio::net::TcpListener;
    use tokio::sync::mpsc::Sender;

    #[async_trait::async_trait]
    pub trait ProcessManager: Send + Sync {
        /// 获取当前运行进程的物理 PID
        fn current_pid(&self) -> u32;
        /// 判定目标 PID 的进程是否在本地健康存活
        async fn is_process_alive(&self, pid: u32) -> bool;
        /// 向目标进程发送优雅停止/物理终止信号
        async fn kill_process(&self, pid: u32, force: bool) -> anyhow::Result<()>;
        /// 将当前进程脱离终端，平滑退化为守护进程 (Daemonize)
        async fn daemonize(&self) -> anyhow::Result<()>;
        /// 设置全局停止事件信道 (主要用于 Windows Service 控制以及信号注册)
        fn set_stop_channel(&self, tx: Sender<()>);
        /// 运行 Windows Service 循环，仅在 Windows 下有效，其它平台抛出 Error
        async fn run_as_service(&self, f: Box<dyn FnOnce() -> anyhow::Result<()> + Send>) -> anyhow::Result<()>;
        /// 跨平台常驻后台进程启动
        fn spawn_daemon(&self, cmd: &mut std::process::Command) -> anyhow::Result<u32>;
        /// 获取占用指定本地 TCP 端口的物理进程 PID（若无占用或平台不支持则返回 None）
        async fn get_port_occupier(&self, port: u16) -> Option<u32>;
    }

    pub trait SysFingerprint: Send + Sync {
        /// 提取操作系统级的硬件唯一指纹 (Machine ID)
        fn get_machine_id(&self) -> anyhow::Result<String>;
    }

    #[async_trait::async_trait]
    pub trait IpcBinder: Send + Sync {
        /// 动态绑定本地 TCP 监听服务，自适应防冲突分配
        async fn bind_ipc_listener(&self, addr: &str) -> anyhow::Result<TcpListener>;
        /// (Out-of-Band Bootstrapping) 服务端暴露一个 UDS/Named Pipe 并向连接的客户端下发包含 Port 和 Token 的 JSON
        async fn serve_handshake(&self, app_dir: &Path, payload: String, stop_rx: tokio::sync::mpsc::Receiver<()>) -> anyhow::Result<()>;
        /// (Out-of-Band Bootstrapping) 客户端连接 UDS/Named Pipe 获取服务端的 JSON 连接凭证
        async fn fetch_handshake(&self, app_dir: &Path) -> anyhow::Result<String>;
    }

    #[async_trait::async_trait]
    pub trait ServiceManager: Send + Sync {
        /// 安装系统后台服务
        async fn install(&self, bin_name: &str, bin_path: &str, log_dir: &str) -> anyhow::Result<()>;
        /// 卸载系统后台服务
        async fn uninstall(&self, bin_name: &str) -> anyhow::Result<()>;
        /// 查询系统服务状态
        async fn status(&self, bin_name: &str) -> anyhow::Result<String>;
    }
}

pub use traits::{ProcessManager, SysFingerprint, IpcBinder, ServiceManager};

pub const SERVICE_PREFIX: &str = "com.chanjet";

/// 生成统一的操作系统硬件指纹 fallback 哈希
pub fn derive_fallback_fingerprint(os_prefix: &str) -> anyhow::Result<String> {
    let hostname = hostname::get()?.to_string_lossy().to_string();
    let base = format!("{}-{}", os_prefix, hostname);
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(base.as_bytes());
    let hash = hasher.finalize();
    Ok(hash.iter().map(|b| format!("{:02x}", b)).collect())
}

pub const STATUS_ACTIVE: &str = "\x1b[32mACTIVE (RUNNING)\x1b[0m";
pub const STATUS_INACTIVE: &str = "\x1b[33mINACTIVE (STOPPED)\x1b[0m";
pub const STATUS_NOT_REGISTERED: &str = "\x1b[31mNOT REGISTERED\x1b[0m";
pub const STATUS_UNKNOWN: &str = "UNKNOWN";

/// 统一格式化各操作系统的后台服务查询状态输出，确保多端展示的一致性与美观性
pub fn format_service_status(platform_title: &str, service_name: &str, is_config_exists: bool, status: &str) -> String {
    let config_str = if is_config_exists { "EXISTS" } else { "MISSING" };
    format!(
        "🔍 {} Service Status:\n  - Service Name: {}\n  - Config: {}\n  - Status: {}",
        platform_title,
        service_name,
        config_str,
        status
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::sys::mock::MockWindowsSys;

    #[tokio::test]
    async fn test_mock_windows_sys_unit_flows() {
        let mock = Arc::new(MockWindowsSys::new());
        assert_eq!(mock.current_pid(), 1234);
        assert!(mock.is_process_alive(1234).await);
        
        let fingerprint: Arc<dyn SysFingerprint> = mock.clone();
        assert_eq!(fingerprint.get_machine_id().unwrap(), "mock-windows-machine-uuid-123456789");

        // Verify service manager Mock
        let service: Arc<dyn ServiceManager> = mock.clone();
        assert!(service.install("test", "path", "log_dir").await.is_ok());
        assert!(service.uninstall("test").await.is_ok());
        let status = service.status("test").await.unwrap();
        assert!(status.contains("REGISTERED"));
    }
}
