use clap::Parser;
use cowen_cli::{run, Cli};

#[tokio::main]
async fn main() {
    // Initialize Rustls Crypto Provider (Mandatory for Rustls 0.23+)
    let _ = rustls::crypto::ring::default_provider().install_default();

    // CAPTURE PANICS: Ensure background crashes are recorded
    std::panic::set_hook(Box::new(|info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .cloned()
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("no message");

        if payload.contains("Broken pipe") {
            return;
        }

        tracing::error!(target: "sys", "FATAL PANIC: {}", payload);
    }));

    // Execute the main task
    let cli = Cli::parse();
    let format = cli.format.clone();
    let res = run(cli).await;

    // Check results
    if let Err(e) = res {
        let err_msg = e.to_string();
        if err_msg.contains("SKIPPED:") {
            // Message already printed via eprintln!, just exit gracefully
            std::process::exit(1);
        }
        tracing::error!(target: "sys", error = %err_msg, "CLI execution failed");

        if format == "json" {
            cowen_common::utils::print_error_json(&err_msg);
        } else {
            eprintln!("❌ Error: {}", err_msg);
        }
        std::process::exit(1);
    }

    // Explicitly exit to ensure background spawns (like telemetry) do not block process teardown
    std::process::exit(0);
}
