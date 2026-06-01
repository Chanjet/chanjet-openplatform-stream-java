use anyhow::Result;
use std::env;

pub enum ServiceAction {
    Install,
    Uninstall,
    Status,
}

pub async fn execute(action: ServiceAction) -> Result<()> {
    let bin_name = cowen_common::utils::get_bin_name();
    let manager = cowen_sys::get_service_manager();
    
    match action {
        ServiceAction::Install => {
            let exe_dir = env::current_exe()?.parent().unwrap().to_path_buf();
            let daemon_bin_name = cowen_sys::get_daemon_binary_name();
            let bin_path = std::env::var("COWEN_DAEMON_BIN")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| exe_dir.join(daemon_bin_name));
            let bin_path_str = bin_path.to_string_lossy();
            let app_dir = cowen_common::config::get_app_dir();
            let log_dir = app_dir.join("logs");
            let log_dir_str = log_dir.to_string_lossy();
            
            manager.install(&bin_name, &bin_path_str, &log_dir_str).await?;
        }
        ServiceAction::Uninstall => {
            manager.uninstall(&bin_name).await?;
        }
        ServiceAction::Status => {
            let status_msg = manager.status(&bin_name).await?;
            println!("{}", status_msg);
        }
    }
    Ok(())
}
