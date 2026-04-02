use std::path::{Path, PathBuf};
use logroller::{LogRollerBuilder, Rotation, RotationAge, RotationSize};
use tracing_subscriber::{
    fmt,
    prelude::*,
    EnvFilter,
};
use anyhow::{Result, Context};

pub fn init_telemetry(log_dir: PathBuf, config: &crate::core::config::LogConfig) -> Result<Vec<tracing_appender::non_blocking::WorkerGuard>> {
    let log_level = &config.level;
    let bin_name = crate::core::utils::get_bin_name();
    
    // 1. Create log directory if it doesn't exist
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir)?;
    }

    let mut guards = Vec::new();

    // 2. Prepare Filters
    // Global filter allows INFO for all internal targets to ensure they reach file layers for traceability.
    let global_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("warn,{}=info,connector_sdk=info,sys=info,audit=info,stream=info,dlq=info", bin_name)));

    // Console specific filter follows the user-provided log_level (defaults to ERROR).
    let console_filter = EnvFilter::new(format!(
        "warn,{}={},connector_sdk={},sys={},audit={},stream={},dlq={}",
        bin_name, log_level, log_level, log_level, log_level, log_level, log_level
    ));

    // 3. Setup Console Layer
    let console_layer = fmt::layer()
        .with_target(false)
        .with_ansi(true)
        .with_writer(std::io::stderr)
        .with_filter(console_filter);

    // 4. Determine Rotation setting
    let rotation = if config.max_size_mb > 0 {
        Rotation::SizeBased(RotationSize::MB(config.max_size_mb))
    } else {
        match config.rotation.to_lowercase().as_str() {
            "hourly" => Rotation::AgeBased(RotationAge::Hourly),
            "minutely" => Rotation::AgeBased(RotationAge::Minutely),
            _ => Rotation::AgeBased(RotationAge::Daily),
        }
    };

    // 5. Initialize global subscriber with domain-specific layers
    let registry = tracing_subscriber::registry()
        .with(global_filter)
        .with(console_layer);

    // Helper macro to create and add a domain layer
    macro_rules! add_domain_layer {
        ($reg:expr, $filename:expr, $filter_fn:expr) => {{
            let appender = LogRollerBuilder::new(log_dir.as_path(), Path::new($filename))
                .rotation(rotation.clone())
                .max_keep_files(config.max_files as u64)
                .build()
                .context(format!("Failed to build logroller for {}", $filename))?;
            let (writer, guard) = tracing_appender::non_blocking(appender);
            guards.push(guard);
            $reg.with(fmt::layer()
                .json()
                .with_writer(writer)
                .with_target(true)
                .with_line_number(true)
                .with_thread_ids(true)
                .with_filter(tracing_subscriber::filter::filter_fn($filter_fn)))
        }};
    }

    let bin_name_clone = bin_name.clone();
    let registry = add_domain_layer!(registry, "sys.log", move |m| m.target().starts_with("sys") || m.target().starts_with(&bin_name_clone));
    let registry = add_domain_layer!(registry, "audit.log", |m| m.target() == "audit");
    let registry = add_domain_layer!(registry, "stream.log", |m| m.target() == "stream" || m.target().starts_with("connector_sdk"));
    let registry = add_domain_layer!(registry, "dlq.log", |m| m.target() == "dlq");

    registry.init();

    Ok(guards)
}
