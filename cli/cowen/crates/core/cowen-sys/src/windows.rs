use cowen_infra::sys::{IpcBinder, ProcessManager, SysFingerprint};
use std::path::Path;
use tokio::net::TcpListener;
use tokio::sync::mpsc::Sender;

pub struct WinProcessManager {
    stop_tx: std::sync::Mutex<Option<Sender<()>>>,
}

impl WinProcessManager {
    pub fn new() -> Self {
        Self {
            stop_tx: std::sync::Mutex::new(None),
        }
    }
}

// Global Windows Stop channel (Only compiled on windows)
#[cfg(windows)]
static WIN_STOP_TX: std::sync::OnceLock<Sender<()>> = std::sync::OnceLock::new();

#[cfg(windows)]
fn trigger_win_stop() {
    if let Some(tx) = WIN_STOP_TX.get() {
        let _ = tx.blocking_send(());
    }
}

#[async_trait::async_trait]
impl ProcessManager for WinProcessManager {
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

    async fn kill_process(&self, pid: u32, _force: bool) -> anyhow::Result<()> {
        let _ = std::process::Command::new("taskkill")
            .args(&["/F", "/PID", &pid.to_string()])
            .status();
        Ok(())
    }

    async fn daemonize(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn set_stop_channel(&self, tx: Sender<()>) {
        #[cfg(windows)]
        let _ = WIN_STOP_TX.set(tx.clone());

        if let Ok(mut guard) = self.stop_tx.lock() {
            *guard = Some(tx);
        }
    }

    async fn run_as_service(
        &self,
        f: Box<dyn FnOnce() -> anyhow::Result<()> + Send>,
    ) -> anyhow::Result<()> {
        #[cfg(windows)]
        {
            if let Some(cell) = RUN_CALLBACK.get() {
                if let Ok(mut guard) = cell.lock() {
                    *guard = Some(f);
                }
            } else {
                let _ = RUN_CALLBACK.set(std::sync::Mutex::new(Some(f)));
            }
            use windows_service::service_dispatcher;
            let res = service_dispatcher::start("CowenDaemon", ffi_service_main);
            if let Err(e) = res {
                tracing::error!("Failed to start Windows Service dispatcher: {}", e);
                return Err(anyhow::anyhow!("Service dispatcher error: {}", e));
            }
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = f;
            anyhow::bail!("Windows Service is not supported on non-Windows platforms.");
        }
    }

    fn spawn_daemon(&self, cmd: &mut std::process::Command) -> anyhow::Result<u32> {
        #[cfg(windows)]
        {
            disable_std_handles_inheritance();
        }
        let child = cmd.spawn()?;
        Ok(child.id())
    }

    async fn get_port_occupier(&self, _port: u16) -> Option<u32> {
        None
    }
}

pub struct WinServiceManager;

impl WinServiceManager {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl cowen_infra::sys::ServiceManager for WinServiceManager {
    async fn install(&self, bin_name: &str, bin_path: &str, _log_dir: &str) -> anyhow::Result<()> {
        let app_dir = cowen_infra::path::get_app_dir();
        let cmd = format!(
            "\"{}\" --auto-start-all --app-dir \"{}\"",
            bin_path,
            app_dir.to_string_lossy()
        );

        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let run_key = hkcu.open_subkey_with_flags(
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
            winreg::enums::KEY_SET_VALUE,
        )?;

        run_key.set_value(bin_name, &cmd)?;
        println!("✅ Successfully installed Windows autostart (Registry HKCU\\Run).");
        Ok(())
    }

    async fn uninstall(&self, bin_name: &str) -> anyhow::Result<()> {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        if let Ok(run_key) = hkcu.open_subkey_with_flags(
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
            winreg::enums::KEY_SET_VALUE,
        ) {
            let _ = run_key.delete_value(bin_name);
        }
        println!("✅ Successfully removed Windows autostart.");
        Ok(())
    }

    async fn status(&self, bin_name: &str) -> anyhow::Result<String> {
        let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let is_installed = if let Ok(run_key) =
            hkcu.open_subkey("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run")
        {
            run_key.get_value::<String, _>(bin_name).is_ok()
        } else {
            false
        };

        let status_str = if is_installed {
            cowen_infra::sys::STATUS_ACTIVE
        } else {
            cowen_infra::sys::STATUS_NOT_REGISTERED
        };

        Ok(cowen_infra::sys::format_service_status(
            "Windows",
            &format!("HKCU\\Run\\{}", bin_name),
            is_installed,
            status_str,
        ))
    }

    async fn is_installed(&self, bin_name: &str) -> anyhow::Result<bool> {
        let is_installed = if let Ok(hkcu) =
            winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
                .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Run")
        {
            hkcu.get_value::<String, _>(bin_name).is_ok()
        } else {
            false
        };
        Ok(is_installed)
    }

    async fn start_service(&self, _bin_name: &str) -> anyhow::Result<()> {
        anyhow::bail!("start_service is not natively supported for HKCU\\Run services on Windows. Please use 'cowen daemon start' instead.");
    }

    async fn stop_service(&self, bin_name: &str) -> anyhow::Result<()> {
        let exe_name = format!("{}.exe", bin_name);
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/IM", &exe_name])
            .status();
        Ok(())
    }
}

#[cfg(windows)]
fn disable_std_handles_inheritance() {
    unsafe {
        const STD_OUTPUT_HANDLE: i32 = -11;
        const STD_ERROR_HANDLE: i32 = -12;
        const HANDLE_FLAG_INHERIT: u32 = 1;
        const INVALID_HANDLE_VALUE: *mut std::ffi::c_void = -1isize as *mut std::ffi::c_void;

        extern "system" {
            fn GetStdHandle(nStdHandle: i32) -> *mut std::ffi::c_void;
            fn SetHandleInformation(
                hObject: *mut std::ffi::c_void,
                dwMask: u32,
                dwFlags: u32,
            ) -> i32;
        }

        let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);
        if !stdout_handle.is_null() && stdout_handle != INVALID_HANDLE_VALUE {
            SetHandleInformation(stdout_handle, HANDLE_FLAG_INHERIT, 0);
        }

        let stderr_handle = GetStdHandle(STD_ERROR_HANDLE);
        if !stderr_handle.is_null() && stderr_handle != INVALID_HANDLE_VALUE {
            SetHandleInformation(stderr_handle, HANDLE_FLAG_INHERIT, 0);
        }
    }
}

#[cfg(windows)]
static RUN_CALLBACK: std::sync::OnceLock<
    std::sync::Mutex<Option<Box<dyn FnOnce() -> anyhow::Result<()> + Send>>>,
> = std::sync::OnceLock::new();

#[cfg(windows)]
windows_service::define_windows_service!(ffi_service_main, my_service_main);

#[cfg(windows)]
fn my_service_main(arguments: Vec<std::ffi::OsString>) {
    if let Err(e) = run_service(arguments) {
        tracing::error!("Windows Service error: {}", e);
    }
}

#[cfg(windows)]
fn run_service(_arguments: Vec<std::ffi::OsString>) -> anyhow::Result<()> {
    use std::time::Duration;
    use windows_service::service::ServiceControl;
    use windows_service::service::{ServiceState, ServiceStatus, ServiceType};
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                trigger_win_stop();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register("CowenDaemon", event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: windows_service::service::ServiceControlAccept::STOP,
        exit_code: windows_service::service::ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    if let Some(cell) = RUN_CALLBACK.get() {
        if let Ok(mut guard) = cell.lock() {
            if let Some(f) = guard.take() {
                let _ = f();
            }
        }
    }

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: windows_service::service::ServiceControlAccept::empty(),
        exit_code: windows_service::service::ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

pub struct WinFingerprint;

impl WinFingerprint {
    pub fn new() -> Self {
        Self
    }
}

impl SysFingerprint for WinFingerprint {
    fn get_machine_id(&self) -> anyhow::Result<String> {
        #[cfg(windows)]
        {
            use winreg::enums::HKEY_LOCAL_MACHINE;
            use winreg::RegKey;
            if let Ok(hklm) =
                RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey("SOFTWARE\\Microsoft\\Cryptography")
            {
                if let Ok(guid) = hklm.get_value::<String, _>("MachineGuid") {
                    return Ok(guid);
                }
            }
        }
        cowen_infra::sys::derive_fallback_fingerprint("windows")
    }
}

pub struct WinIpcBinder;

impl WinIpcBinder {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl IpcBinder for WinIpcBinder {
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
        use tokio::net::windows::named_pipe::ServerOptions;
        let pipe_name = format!(
            r"\\.\pipe\cowen_ipc_{}",
            app_dir
                .to_string_lossy()
                .replace("\\", "_")
                .replace(":", "_")
        );

        loop {
            let mut server = match ServerOptions::new()
                .first_pipe_instance(false)
                .create(&pipe_name)
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to create named pipe: {}", e);
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    continue;
                }
            };

            tokio::select! {
                _ = stop_rx.recv() => {
                    break;
                }
                connect_res = server.connect() => {
                    if connect_res.is_ok() {
                        let payload_clone = payload.clone();
                        tokio::spawn(async move {
                            use tokio::io::AsyncWriteExt;
                            let _ = server.write_all(payload_clone.as_bytes()).await;
                            let _ = server.flush().await;
                        });
                    }
                }
            }
        }
        Ok(())
    }

    async fn fetch_handshake(&self, app_dir: &Path) -> anyhow::Result<String> {
        use tokio::net::windows::named_pipe::ClientOptions;
        let pipe_name = format!(
            r"\\.\pipe\cowen_ipc_{}",
            app_dir
                .to_string_lossy()
                .replace("\\", "_")
                .replace(":", "_")
        );

        let mut client = ClientOptions::new().open(&pipe_name)?;
        use tokio::io::AsyncReadExt;
        let mut buf = String::new();
        client.read_to_string(&mut buf).await?;
        Ok(buf)
    }
}

/// 设置当前进程的显示名称 (跨平台实现)
pub fn set_process_name(name: &str) {
    let _ = name;
    // Windows unsupported: doing nothing
}

pub mod fs {
    use std::path::Path;

    pub fn secure_write<P: AsRef<Path>, C: AsRef<[u8]>>(
        path: P,
        contents: C,
    ) -> std::io::Result<()> {
        std::fs::write(path, contents)
    }

    pub fn secure_open_write<P: AsRef<Path>>(path: P) -> std::io::Result<std::fs::File> {
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
    }

    pub fn secure_open_append<P: AsRef<Path>>(path: P) -> std::io::Result<std::fs::File> {
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(path)
    }

    pub fn make_executable<P: AsRef<Path>>(path: P) -> std::io::Result<()> {
        let _ = path;
        Ok(())
    }

    pub fn is_file_secure<P: AsRef<Path>>(path: P) -> bool {
        let _ = path;
        true
    }
}

#[cfg(test)]
mod tests {}
