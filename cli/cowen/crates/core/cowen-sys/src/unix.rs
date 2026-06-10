use std::path::Path;
use tokio::net::TcpListener;
use tokio::sync::mpsc::Sender;

pub struct UnixProcessManager {
    stop_tx: std::sync::Mutex<Option<Sender<()>>>,
}

impl Default for UnixProcessManager {
    fn default() -> Self {
        Self::new()
    }
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
        use sysinfo::{Pid, ProcessesToUpdate, System};
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
            if let Ok(mut sigterm) =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            {
                sigterm.recv().await;
                let _ = tx.send(()).await;
            }
        });
    }

    async fn run_as_service(
        &self,
        _f: Box<dyn FnOnce() -> anyhow::Result<()> + Send>,
    ) -> anyhow::Result<()> {
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

impl Default for UnixIpcBinder {
    fn default() -> Self {
        Self::new()
    }
}

impl UnixIpcBinder {
    pub fn new() -> Self {
        Self
    }

    fn get_sock_path(app_dir: &Path) -> std::path::PathBuf {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(app_dir.to_string_lossy().as_bytes());
        let hash: String = hasher
            .finalize()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect();
        std::path::PathBuf::from(format!("/tmp/cowen_ipc_{}.sock", &hash[0..16]))
    }
}

#[async_trait::async_trait]
impl cowen_infra::sys::IpcBinder for UnixIpcBinder {
    async fn bind_ipc_listener(&self, addr: &str) -> anyhow::Result<TcpListener> {
        let listener = TcpListener::bind(addr).await?;
        Ok(listener)
    }

    async fn serve_handshake(
        &self,
        app_dir: &Path,
        payload: String,
        mut stop_rx: tokio::sync::mpsc::Receiver<()>,
    ) -> anyhow::Result<()> {
        let sock_path = Self::get_sock_path(app_dir);
        if sock_path.exists() {
            let _ = std::fs::remove_file(&sock_path);
        }
        let listener = tokio::net::UnixListener::bind(&sock_path)?;

        // Ensure 0600 permissions
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&sock_path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&sock_path, perms)?;

        loop {
            tokio::select! {
                _ = stop_rx.recv() => {
                    let _ = std::fs::remove_file(&sock_path);
                    break;
                }
                accept_res = listener.accept() => {
                    if let Ok((mut stream, _)) = accept_res {
                        let payload_clone = payload.clone();
                        tokio::spawn(async move {
                            use tokio::io::AsyncWriteExt;
                            let _ = stream.write_all(payload_clone.as_bytes()).await;
                        });
                    }
                }
            }
        }
        Ok(())
    }

    async fn fetch_handshake(&self, app_dir: &Path) -> anyhow::Result<String> {
        let sock_path = Self::get_sock_path(app_dir);
        let mut stream = tokio::net::UnixStream::connect(&sock_path).await?;
        use tokio::io::AsyncReadExt;
        let mut buf = String::new();
        stream.read_to_string(&mut buf).await?;
        Ok(buf)
    }
}

pub mod fs {
    use std::path::Path;

    pub fn secure_write<P: AsRef<Path>, C: AsRef<[u8]>>(
        path: P,
        contents: C,
    ) -> std::io::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
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

#[cfg(test)]
mod tests {}
