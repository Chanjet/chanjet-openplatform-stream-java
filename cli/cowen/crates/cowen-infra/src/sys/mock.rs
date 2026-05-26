use std::path::Path;
use tokio::net::TcpListener;
use tokio::sync::mpsc::Sender;
use crate::sys::{ProcessManager, IpcBinder, SysFingerprint};

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
    
    async fn run_as_service(&self, _f: Box<dyn FnOnce() -> anyhow::Result<()> + Send>) -> anyhow::Result<()> {
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
    
    async fn load_ipc_token(&self, _token_file: &Path) -> anyhow::Result<String> {
        Ok("mock-windows-ipc-token-secret".to_string())
    }
    
    async fn save_ipc_token(&self, _token_file: &Path, _token: &str) -> anyhow::Result<()> {
        Ok(())
    }
}
