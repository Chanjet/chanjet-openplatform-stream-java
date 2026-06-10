use crate::sys::{IpcBinder, ProcessManager, ServiceManager, SysFingerprint};
use std::path::Path;
use tokio::net::TcpListener;
use tokio::sync::mpsc::Sender;

pub struct MockWindowsSys {
    pub mock_pid: u32,
    pub should_alive: bool,
}

impl MockWindowsSys {
    pub fn new() -> Self {
        Self {
            mock_pid: 1234,
            should_alive: true,
        }
    }
}

impl Default for MockWindowsSys {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ProcessManager for MockWindowsSys {
    fn current_pid(&self) -> u32 {
        self.mock_pid
    }

    async fn is_process_alive(&self, _pid: u32) -> bool {
        self.should_alive
    }

    async fn kill_process(&self, _pid: u32, _force: bool) -> anyhow::Result<()> {
        Ok(())
    }

    async fn daemonize(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn set_stop_channel(&self, _tx: Sender<()>) {}

    async fn run_as_service(
        &self,
        _f: Box<dyn FnOnce() -> anyhow::Result<()> + Send>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn spawn_daemon(&self, cmd: &mut std::process::Command) -> anyhow::Result<u32> {
        let child = cmd.spawn()?;
        Ok(child.id())
    }

    async fn get_port_occupier(&self, _port: u16) -> Option<u32> {
        None
    }
}

#[async_trait::async_trait]
impl ServiceManager for MockWindowsSys {
    async fn install(
        &self,
        _bin_name: &str,
        _bin_path: &str,
        _log_dir: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    async fn uninstall(&self, _bin_name: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn status(&self, _bin_name: &str) -> anyhow::Result<String> {
        Ok("🔍 Windows Service Status:\n  - Status: REGISTERED\n  - State: RUNNING".to_string())
    }
}

impl SysFingerprint for MockWindowsSys {
    fn get_machine_id(&self) -> anyhow::Result<String> {
        Ok("mock-windows-machine-uuid-123456789".to_string())
    }
}

#[async_trait::async_trait]
impl IpcBinder for MockWindowsSys {
    async fn bind_ipc_listener(&self, _addr: &str) -> anyhow::Result<TcpListener> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        Ok(listener)
    }

    async fn serve_handshake(
        &self,
        _app_dir: &Path,
        _payload: String,
        _stop_rx: tokio::sync::mpsc::Receiver<()>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn fetch_handshake(&self, _app_dir: &Path) -> anyhow::Result<String> {
        Ok(r#"{"port":1234,"token":"mock-windows-ipc-token-secret"}"#.to_string())
    }
}
