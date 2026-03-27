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
    
    // 1. Create log directory if it doesn't exist
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir)?;
    }

    let mut guards = Vec::new();

    // 2. Prepare EnvFilter
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!(
            "warn,cjtcr={},connector_sdk={},sys={},audit={},stream={},dlq={}", 
            log_level, log_level, log_level, log_level, log_level, log_level
        )));

    // 3. Setup Console Layer
    let console_layer = fmt::layer()
        .with_target(false)
        .with_ansi(true);

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
        .with(filter)
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
                .with_filter(tracing_subscriber::filter::filter_fn($filter_fn)))
        }};
    }

    let registry = add_domain_layer!(registry, "sys.log", |m| m.target().starts_with("sys") || m.target().starts_with("cjtcr"));
    let registry = add_domain_layer!(registry, "audit.log", |m| m.target() == "audit");
    let registry = add_domain_layer!(registry, "stream.log", |m| m.target() == "stream");
    let registry = add_domain_layer!(registry, "dlq.log", |m| m.target() == "dlq");

    registry.init();

    Ok(guards)
}
