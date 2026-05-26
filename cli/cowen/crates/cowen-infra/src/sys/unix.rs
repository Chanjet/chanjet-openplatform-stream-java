use std::path::Path;
use tokio::net::TcpListener;
use tokio::sync::mpsc::Sender;
use std::os::unix::fs::OpenOptionsExt;
use std::io::Write;

pub struct UnixProcessManager {
    stop_tx: std::sync::Mutex<Option<Sender<()>>>,
}

impl UnixProcessManager {
    pub fn new() -> Self {
        Self {
            stop_tx: std::sync::Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl crate::sys::ProcessManager for UnixProcessManager {
    fn current_pid(&self) -> u32 {
        std::process::id()
    }
    
    async fn is_process_alive(&self, pid: u32) -> bool {
        use sysinfo::{System, Pid, ProcessesToUpdate};
        let mut s = System::new();
        let sys_pid = Pid::from_u32(pid);
        s.refresh_processes(ProcessesToUpdate::Some(&[sys_pid]), true);
        s.process(sys_pid).is_some()
    }
    
    async fn kill_process(&self, pid: u32, force: bool) -> anyhow::Result<()> {
        let signal = if force { "-9" } else { "-15" };
        let _ = std::process::Command::new("kill")
            .arg(signal)
            .arg(pid.to_string())
            .status();
        Ok(())
    }
    
    async fn daemonize(&self) -> anyhow::Result<()> {
        Ok(())
    }
    
    fn set_stop_channel(&self, tx: Sender<()>) {
        if let Ok(mut guard) = self.stop_tx.lock() {
            *guard = Some(tx.clone());
        }
        tokio::spawn(async move {
            if let Ok(mut sigterm) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                sigterm.recv().await;
                let _ = tx.send(()).await;
            }
        });
    }
    
    async fn run_as_service(&self, _f: Box<dyn FnOnce() -> anyhow::Result<()> + Send>) -> anyhow::Result<()> {
        anyhow::bail!("Unix does not support Windows Service model.")
    }
}

pub struct UnixIpcBinder;

impl UnixIpcBinder {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl crate::sys::IpcBinder for UnixIpcBinder {
    async fn bind_ipc_listener(&self, addr: &str) -> anyhow::Result<TcpListener> {
        let listener = TcpListener::bind(addr).await?;
        Ok(listener)
    }
    
    async fn load_ipc_token(&self, token_file: &Path) -> anyhow::Result<String> {
        let content = std::fs::read_to_string(token_file)?;
        Ok(content.trim().to_string())
    }
    
    async fn save_ipc_token(&self, token_file: &Path, token: &str) -> anyhow::Result<()> {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true).mode(0o600);
        let mut f = opts.open(token_file)?;
        f.write_all(token.as_bytes())?;
        Ok(())
    }
}
