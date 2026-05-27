use std::path::Path;
use tokio::net::TcpListener;
use tokio::sync::mpsc::Sender;
use cowen_infra::sys::{ProcessManager, IpcBinder, SysFingerprint};

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
        use sysinfo::{System, Pid, ProcessesToUpdate};
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
    
    async fn run_as_service(&self, f: Box<dyn FnOnce() -> anyhow::Result<()> + Send>) -> anyhow::Result<()> {
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
        let service_name = format!("{}Daemon", bin_name);
        let status = std::process::Command::new("sc")
            .arg("create")
            .arg(&service_name)
            .arg("binPath=")
            .arg(format!("\"{}\" daemon start --all --run-as-service", bin_path))
            .arg("start=")
            .arg("auto")
            .status()?;
        if status.success() {
            println!("✅ Successfully installed Windows Service.");
            println!("📍 Service Name: {}", service_name);
            Ok(())
        } else {
            anyhow::bail!("Failed to create Windows Service via sc create.")
        }
    }

    async fn uninstall(&self, bin_name: &str) -> anyhow::Result<()> {
        let service_name = format!("{}Daemon", bin_name);
        let _ = std::process::Command::new("sc").arg("stop").arg(&service_name).status();
        let status = std::process::Command::new("sc")
            .arg("delete")
            .arg(&service_name)
            .status()?;
        if status.success() {
            println!("✅ Successfully removed Windows Service.");
            Ok(())
        } else {
            anyhow::bail!("Failed to delete Windows Service.")
        }
    }

    async fn status(&self, bin_name: &str) -> anyhow::Result<String> {
        let service_name = format!("{}Daemon", bin_name);
        let output = std::process::Command::new("sc")
            .arg("query")
            .arg(&service_name)
            .output();

        let (is_exists, status_str) = match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.contains("RUNNING") {
                    (true, cowen_infra::sys::STATUS_ACTIVE)
                } else if stdout.contains("STOPPED") {
                    (true, cowen_infra::sys::STATUS_INACTIVE)
                } else {
                    (true, cowen_infra::sys::STATUS_UNKNOWN)
                }
            }
            _ => (false, cowen_infra::sys::STATUS_NOT_REGISTERED),
        };

        Ok(cowen_infra::sys::format_service_status("Windows", &service_name, is_exists, status_str))
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
            fn SetHandleInformation(hObject: *mut std::ffi::c_void, dwMask: u32, dwFlags: u32) -> i32;
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
static RUN_CALLBACK: std::sync::OnceLock<std::sync::Mutex<Option<Box<dyn FnOnce() -> anyhow::Result<()> + Send>>>> = std::sync::OnceLock::new();

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
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::service::ServiceControl;
    use windows_service::service::{ServiceState, ServiceStatus, ServiceType};
    use std::time::Duration;

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
            use winreg::RegKey;
            use winreg::enums::HKEY_LOCAL_MACHINE;
            if let Ok(hklm) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey("SOFTWARE\\Microsoft\\Cryptography") {
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
    
    async fn load_ipc_token(&self, token_file: &Path) -> anyhow::Result<String> {
        let content = std::fs::read_to_string(token_file)?;
        Ok(content.trim().to_string())
    }
    
    async fn save_ipc_token(&self, token_file: &Path, token: &str) -> anyhow::Result<()> {
        std::fs::write(token_file, token)?;
        Ok(())
    }
}

/// 设置当前进程的显示名称 (跨平台实现)
pub fn set_process_name(name: &str) {
    let _ = name;
    // Windows unsupported: doing nothing
}

pub mod fs {
    use std::path::Path;

    pub fn secure_write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> std::io::Result<()> {
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
