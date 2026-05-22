use std::path::{Path, PathBuf};
use cowen_infra::obfs;
use logroller::{LogRollerBuilder, Rotation, RotationAge, RotationSize};
use tracing_subscriber::{
    fmt,
    prelude::*,
    EnvFilter,
};
use std::sync::Arc;
use anyhow::Result;
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

pub struct TelemetryControl {
    pub guards: Vec<tracing_appender::non_blocking::WorkerGuard>,
    handle: tracing_subscriber::reload::Handle<EnvFilter, tracing_subscriber::Registry>,
}

impl TelemetryControl {
    pub fn update_level(&self, level: &str) -> anyhow::Result<()> {
        let bin_name = cowen_common::utils::get_bin_name();
        let new_filter = EnvFilter::new(format!(
            "warn,{}={},connector_sdk={},sys={},audit={},stream={},dlq={}", 
            bin_name, level, level, level, level, level, level
        ));
        self.handle.reload(new_filter).map_err(|e| anyhow::anyhow!(e))
    }
}

pub fn init_telemetry(
    log_dir: PathBuf, 
    profile: &str, 
    config: &cowen_common::config::LogConfig,
    vault_rx: tokio::sync::watch::Receiver<Option<Arc<dyn cowen_common::vault::Vault>>>,
) -> Result<TelemetryControl> {
    let log_level = &config.level;
    let bin_name = cowen_common::utils::get_bin_name();
    
    if !log_dir.exists() {
        std::fs::create_dir_all(&log_dir)?;
    }

    let mut guards = Vec::new();

    let global_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!(
            "warn,{}={},connector_sdk={},sys={},audit={},stream={},dlq={}", 
            bin_name, log_level, log_level, log_level, log_level, log_level, log_level
        )));

    let (global_filter, handle) = tracing_subscriber::reload::Layer::new(global_filter);

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

    let vault_audit_layer = crate::audit::VaultAuditLayer::new(vault_rx);

    let registry = tracing_subscriber::registry()
        .with(global_filter)
        .with(console_layer)
        .with(vault_audit_layer);

    macro_rules! add_domain_layer {
        ($reg:expr, $filename:expr, $filter_fn:expr) => {{
            let (layer, guard) = match LogRollerBuilder::new(log_dir.as_path(), Path::new($filename))
                .rotation(rotation.clone())
                .max_keep_files(config.max_files as u64)
                .build() {
                Ok(appender) => {
                    let (writer, guard) = tracing_appender::non_blocking(appender);
                    (Some(fmt::layer()
                        .json()
                        .with_writer(writer)
                        .with_target(true)
                        .with_line_number(true)
                        .with_thread_ids(true)
                        .with_filter(tracing_subscriber::filter::filter_fn($filter_fn))), Some(guard))
                },
                Err(e) => {
                    eprintln!("⚠️ Warning: Failed to initialize file logger for {}: {}. Logging for this domain will be disabled but process will continue.", $filename, e);
                    (None, None)
                }
            };
            if let Some(g) = guard { guards.push(g); }
            $reg.with(layer)
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

    Ok(TelemetryControl { guards, handle })
}

/// 发送遥测请求 (添加 500ms 超时控制)
pub async fn send_telemetry_request(config: &cowen_common::Config, app_cfg: &cowen_common::config::AppConfig, event_name: String, payload: serde_json::Value) -> Result<()> {
    let ua = cowen_infra::get_user_agent(env!("CARGO_PKG_VERSION"));
    let client = cowen_infra::create_client(&ua).map_err(|e| anyhow::anyhow!(e))?;
    let fingerprint = cowen_common::security::get_machine_fingerprint()?;
    
    let url = format!("{}{}", app_cfg.stream_url.trim_end_matches('/'), obfs!("/v1/telemetry/events"));
    
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

    tokio::time::timeout(
        std::time::Duration::from_millis(500),
        client.post(&url).json(&event).send()
    ).await.map_err(|_| anyhow::anyhow!("Telemetry report timed out"))??;
    
    Ok(())
}

/// 异步上报遥测事件 (静默失败，非阻塞)
pub fn report_event(config: &cowen_common::Config, app_cfg: &cowen_common::config::AppConfig, event_name: String, payload: serde_json::Value) {
    if !app_cfg.telemetry_enabled {
        return;
    }
    let config = config.clone();
    let app_cfg = app_cfg.clone();
    
    tokio::spawn(async move {
        if let Err(e) = send_telemetry_request(&config, &app_cfg, event_name, payload).await {
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
        let config = cowen_common::Config::default_with_profile("testprof");
        let app_cfg = cowen_common::config::AppConfig::default();
        
        let (_, rx) = tokio::sync::watch::channel(None);
        let _control = init_telemetry(temp_dir.clone(), "testprof", &app_cfg.log, rx).unwrap();
        
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

    #[tokio::test]
    async fn test_report_event_timeout_and_spin_prevention() {
        let config = cowen_common::Config::default_with_profile("testprof");
        let mut app_cfg = cowen_common::config::AppConfig::default();
        app_cfg.telemetry_enabled = true;
        // Use an unroutable IP to simulate a network blackhole / connection block
        app_cfg.stream_url = "http://10.255.255.1:9999".to_string();

        let start = std::time::Instant::now();
        
        // This will block or fail, and we want to verify it does not take too long
        let res = send_telemetry_request(&config, &app_cfg, "test_timeout".to_string(), json!({})).await;
        
        let duration = start.elapsed();
        
        assert!(res.is_err());
        assert!(
            duration < std::time::Duration::from_millis(1500),
            "Telemetry request took too long without timeout mechanism: {:?}",
            duration
        );
    }
}
