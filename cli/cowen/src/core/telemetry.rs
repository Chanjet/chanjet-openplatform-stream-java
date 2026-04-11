use std::path::{Path, PathBuf};
use logroller::{LogRollerBuilder, Rotation, RotationAge, RotationSize};
use tracing_subscriber::{
    fmt,
    prelude::*,
    EnvFilter,
};
use anyhow::{Result, Context};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub event: String,
    pub fingerprint: String,
    pub app_key: String,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub timestamp: DateTime<Utc>,
    pub payload: serde_json::Value,
}

pub fn init_telemetry(log_dir: PathBuf, profile: &str, config: &crate::core::config::LogConfig) -> Result<Vec<tracing_appender::non_blocking::WorkerGuard>> {
    let log_level = &config.level;
    let bin_name = crate::core::utils::get_bin_name();
    
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir)?;
    }

    let mut guards = Vec::new();

    let global_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("warn,{}=info,connector_sdk=info,sys=info,audit=info,stream=info,dlq=info", bin_name)));

    let console_filter = EnvFilter::new(format!(
        "warn,{}={},connector_sdk={},sys={},audit={},stream={},dlq={}",
        bin_name, log_level, log_level, log_level, log_level, log_level, log_level
    ));

    let console_layer = fmt::layer()
        .with_target(false)
        .with_ansi(true)
        .with_writer(std::io::stderr)
        .with_filter(console_filter);

    let rotation = if config.max_size_mb > 0 {
        Rotation::SizeBased(RotationSize::MB(config.max_size_mb))
    } else {
        match config.rotation.to_lowercase().as_str() {
            "hourly" => Rotation::AgeBased(RotationAge::Hourly),
            "minutely" => Rotation::AgeBased(RotationAge::Minutely),
            _ => Rotation::AgeBased(RotationAge::Daily),
        }
    };

    let registry = tracing_subscriber::registry()
        .with(global_filter)
        .with(console_layer);

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
    let sys_log = format!("{}_sys.log", profile);
    let audit_log = format!("{}_audit.log", profile);
    let stream_log = format!("{}_stream.log", profile);
    let dlq_log = format!("{}_dlq.log", profile);

    let registry = add_domain_layer!(registry, &sys_log, move |m| m.target().starts_with("sys") || m.target().starts_with(&bin_name_clone));
    let registry = add_domain_layer!(registry, &audit_log, |m| m.target() == "audit");
    let registry = add_domain_layer!(registry, &stream_log, |m| m.target() == "stream" || m.target().starts_with("connector_sdk"));
    let registry = add_domain_layer!(registry, &dlq_log, |m| m.target() == "dlq");

    registry.init();

    Ok(guards)
}

/// 异步上报遥测事件 (静默失败，非阻塞)
pub fn report_event(config: &crate::core::config::Config, event_name: String, payload: serde_json::Value) {
    if !config.telemetry_enabled {
        return;
    }
    let config = config.clone();
    
    tokio::spawn(async move {
        let result: Result<()> = async {
            let client = crate::core::network::create_client(&config)?;
            let fingerprint = crate::core::security::get_machine_fingerprint()?;
            
            let url = format!("{}{}", config.stream_url.trim_end_matches('/'), obfs!("/v1/telemetry/events"));
            
            let event = TelemetryEvent {
                event: event_name,
                fingerprint,
                app_key: if config.app_key.is_empty() { "uninitialized".to_string() } else { config.app_key.clone() },
                version: env!("CARGO_PKG_VERSION").to_string(),
                os: std::env::consts::OS.to_string(),
                arch: std::env::consts::ARCH.to_string(),
                timestamp: Utc::now(),
                payload,
            };

            client.post(&url)
                .json(&event)
                .send()
                .await?;
            
            Ok(())
        }.await;

        if let Err(e) = result {
            tracing::debug!(target: "sys", "Telemetry report failed (silently ignored): {}", e);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_profile_specific_log_creation() {
        let temp_dir = std::env::temp_dir().join(format!("telemetry_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros()));
        let config = crate::core::config::Config::default_with_profile("testprof");
        
        let _guards = init_telemetry(temp_dir.clone(), "testprof", &config.log).unwrap();
        
        // Tracing appender creates files lazily upon first write OR immediately depending on the exact LogRoller configuration
        // In logroller, they are usually created right away if the builder creates the file.
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        assert!(temp_dir.join("testprof_sys.log").exists());
        assert!(temp_dir.join("testprof_audit.log").exists());
        assert!(temp_dir.join("testprof_stream.log").exists());
        assert!(temp_dir.join("testprof_dlq.log").exists());
        
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_telemetry_event_serialization() {
        let event = TelemetryEvent {
            event: "test_event".to_string(),
            fingerprint: "abc".to_string(),
            app_key: "key".to_string(),
            version: "0.1.0".to_string(),
            os: "macos".to_string(),
            arch: "arm64".to_string(),
            timestamp: Utc::now(),
            payload: json!({"cmd": "test"}),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"test_event\""));
        assert!(json.contains("\"fingerprint\":\"abc\""));
    }
}
