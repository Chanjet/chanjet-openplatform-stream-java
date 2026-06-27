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

    async fn is_installed(&self, _bin_name: &str) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn start_service(&self, _bin_name: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn stop_service(&self, _bin_name: &str) -> anyhow::Result<()> {
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_windows_sys_all_methods() {
        let mock = MockWindowsSys::default();

        // ProcessManager
        assert_eq!(mock.current_pid(), 1234);
        assert!(mock.is_process_alive(1234).await);
        assert!(mock.kill_process(1234, false).await.is_ok());
        assert!(mock.daemonize().await.is_ok());

        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        mock.set_stop_channel(tx);

        assert!(mock.run_as_service(Box::new(|| Ok(()))).await.is_ok());

        let mut cmd = std::process::Command::new("echo");
        let _ = mock.spawn_daemon(&mut cmd);

        assert!(mock.get_port_occupier(8080).await.is_none());

        // ServiceManager
        assert!(mock.install("bin", "path", "log").await.is_ok());
        assert!(mock.uninstall("bin").await.is_ok());
        assert!(mock.status("bin").await.is_ok());

        // SysFingerprint
        assert!(mock.get_machine_id().is_ok());

        // IpcBinder
        assert!(mock.bind_ipc_listener("127.0.0.1:0").await.is_ok());

        let path = std::path::Path::new("/tmp");
        let (_tx2, rx2) = tokio::sync::mpsc::channel(1);
        assert!(mock
            .serve_handshake(path, "".to_string(), rx2)
            .await
            .is_ok());
        assert!(mock.fetch_handshake(path).await.is_ok());
    }
}
