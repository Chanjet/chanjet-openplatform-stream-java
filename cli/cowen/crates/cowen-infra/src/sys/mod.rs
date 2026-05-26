use std::sync::Arc;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
mod windows;

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
    }

    pub trait SysFingerprint: Send + Sync {
        /// 提取操作系统级的硬件唯一指纹 (Machine ID)
        fn get_machine_id(&self) -> anyhow::Result<String>;
    }

    #[async_trait::async_trait]
    pub trait IpcBinder: Send + Sync {
        /// 动态绑定本地 TCP 监听服务，自适应防冲突分配
        async fn bind_ipc_listener(&self, addr: &str) -> anyhow::Result<TcpListener>;
        /// 读取仅对当前运行用户具有 0600 读写权限的随机鉴权 Token 字符串
        async fn load_ipc_token(&self, token_file: &Path) -> anyhow::Result<String>;
        /// 保存仅对当前运行用户具有 0600 读写权限的随机鉴权 Token 字符串
        async fn save_ipc_token(&self, token_file: &Path, token: &str) -> anyhow::Result<()>;
    }
}

pub use traits::{ProcessManager, SysFingerprint, IpcBinder};

pub fn get_process_manager() -> Arc<dyn ProcessManager> {
    #[cfg(target_os = "macos")]
    return Arc::new(macos::MacProcessManager::new());
    
    #[cfg(target_os = "linux")]
    return Arc::new(linux::LinuxProcessManager::new());
    
    #[cfg(windows)]
    return Arc::new(windows::WinProcessManager::new());
    
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    compile_error!("Unsupported platform!");
}

pub fn get_sys_fingerprint() -> Arc<dyn SysFingerprint> {
    #[cfg(target_os = "macos")]
    return Arc::new(macos::MacFingerprint::new());
    
    #[cfg(target_os = "linux")]
    return Arc::new(linux::LinuxFingerprint::new());
    
    #[cfg(windows)]
    return Arc::new(windows::WinFingerprint::new());
    
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    compile_error!("Unsupported platform!");
}

pub fn get_ipc_binder() -> Arc<dyn IpcBinder> {
    #[cfg(target_os = "macos")]
    return Arc::new(macos::MacIpcBinder::new());
    
    #[cfg(target_os = "linux")]
    return Arc::new(linux::LinuxIpcBinder::new());
    
    #[cfg(windows)]
    return Arc::new(windows::WinIpcBinder::new());
    
    #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
    compile_error!("Unsupported platform!");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::mock::MockWindowsSys;

    #[tokio::test]
    async fn test_mock_windows_sys_unit_flows() {
        let mock = Arc::new(MockWindowsSys::new());
        assert_eq!(mock.current_pid(), 1234);
        assert!(mock.is_process_alive(1234).await);
        
        let fingerprint: Arc<dyn SysFingerprint> = mock.clone();
        assert_eq!(fingerprint.get_machine_id().unwrap(), "mock-windows-machine-uuid-123456789");
    }
}
