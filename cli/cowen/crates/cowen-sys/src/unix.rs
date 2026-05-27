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
impl cowen_infra::sys::ProcessManager for UnixProcessManager {
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
    
    fn spawn_daemon(&self, cmd: &mut std::process::Command) -> anyhow::Result<u32> {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
        let child = cmd.spawn()?;
        Ok(child.id())
    }

    async fn get_port_occupier(&self, port: u16) -> Option<u32> {
        let output = std::process::Command::new("lsof")
            .arg("-i")
            .arg(format!("tcp:{}", port))
            .arg("-t")
            .output();
        if let Ok(out) = output {
            let pid_str = String::from_utf8_lossy(&out.stdout);
            for line in pid_str.lines() {
                if let Ok(pid) = line.trim().parse::<u32>() {
                    return Some(pid);
                }
            }
        }
        None
    }
}

pub struct UnixIpcBinder;

impl UnixIpcBinder {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl cowen_infra::sys::IpcBinder for UnixIpcBinder {
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

pub mod fs {
    use std::path::Path;

    pub fn secure_write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> std::io::Result<()> {
        use std::os::unix::fs::OpenOptionsExt;
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(contents.as_ref())
    }

    pub fn secure_open_write<P: AsRef<Path>>(path: P) -> std::io::Result<std::fs::File> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        options.open(path)
    }

    pub fn secure_open_append<P: AsRef<Path>>(path: P) -> std::io::Result<std::fs::File> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).append(true);
        std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
        options.open(path)
    }

    pub fn make_executable<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms)?;
        Ok(())
    }

    pub fn is_file_secure<P: AsRef<Path>>(path: P) -> bool {
        use std::os::unix::fs::MetadataExt;
        
        let check_meta = |m: &std::fs::Metadata| -> bool {
            let mode = m.mode();
            let uid = m.uid();
            
            // World-writable check
            if mode & 0o002 != 0 {
                return false;
            }
            
            // Owner check (must be owned by root or the current user)
            let current_uid = unsafe { libc::getuid() };
            if uid != 0 && uid != current_uid {
                return false;
            }
            true
        };

        if let Ok(meta) = std::fs::metadata(&path) {
            if !check_meta(&meta) {
                return false;
            }
        } else {
            return false;
        }

        if let Some(parent) = path.as_ref().parent() {
            if let Ok(meta) = std::fs::metadata(parent) {
                if !check_meta(&meta) {
                    return false;
                }
            }
        }
        true
    }
}
