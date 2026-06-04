use anyhow::Result;
use cowen_common::grpc::client::DaemonResponse;
use crate::Colorize;

pub async fn execute(profile: &str, _verbose: bool, _fix: bool) -> Result<()> {
    println!("\n{} {} (Profile: {})", "🩺".bold(), "Cowen Doctor - 环境诊断工具 (IPC Client)".bold(), profile.cyan());
    println!("{}\n", "=".repeat(60).dimmed());

    let ipc = cowen_common::grpc::client::DaemonClient::new(crate::get_ipc_port_path());
    match ipc.doctor(profile).await {
        Ok(DaemonResponse::DoctorReport { report }) => {
            println!("{}", report);
        }
        Ok(DaemonResponse::Error { message, .. }) => {
            eprintln!("{} {}", "❌".red(), message);
        }
        Err(e) => {
            eprintln!("{} 无法连接到 Daemon: {}", "❌".red(), e);
            eprintln!("💡 请确保后台服务正在运行 (`cowen daemon start`)");
        }
        _ => eprintln!("❌ 收到未知响应"),
    }

    Ok(())
}



