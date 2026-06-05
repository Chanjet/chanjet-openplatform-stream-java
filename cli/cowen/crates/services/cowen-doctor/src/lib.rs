pub mod task;

pub use task::*;
use anyhow::Result;
use std::time::Instant;
use async_trait::async_trait;

pub async fn run_all_diagnostics(ctx: &DoctorContext) -> Result<Vec<DiagnosticResult>> {
    let mut set = tokio::task::JoinSet::new();
    
    for reg in inventory::iter::<DiagnosticRegistration> {
        let task = (reg.builder)();
        let ctx_clone = ctx.clone();
        set.spawn(async move {
            task.run(&ctx_clone).await
        });
    }

    let mut results = Vec::new();
    while let Some(res) = set.join_next().await {
        if let Ok(Ok(diagnostic_res)) = res {
            results.push(diagnostic_res);
        } else if let Ok(Err(e)) = res {
            // Task failed internally, could wrap it or ignore
            tracing::error!("Diagnostic task failed: {}", e);
        }
    }

    Ok(results)
}

// ---------------------------------------------------------
// Built-in Diagnostic Plugins
// ---------------------------------------------------------

struct SystemInfoCheck;
#[async_trait]
impl DiagnosticTask for SystemInfoCheck {
    fn name(&self) -> &str { "系统与配置" }
    async fn run(&self, _ctx: &DoctorContext) -> Result<DiagnosticResult> {
        let start = Instant::now();
        let app_dir = cowen_common::config::get_app_dir();
        
        let status = if !app_dir.exists() {
            DiagnosticStatus::Error("Home Dir Missing".to_string())
        } else {
            DiagnosticStatus::Ok
        };

        Ok(DiagnosticResult {
            name: self.name().to_string(),
            status,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}
inventory::submit! { DiagnosticRegistration { builder: || Box::new(SystemInfoCheck) } }

struct StreamUrlCheck;
#[async_trait]
impl DiagnosticTask for StreamUrlCheck {
    fn name(&self) -> &str { "网络连通性 (Stream)" }
    async fn run(&self, ctx: &DoctorContext) -> Result<DiagnosticResult> {
        let start = Instant::now();
        let app_cfg = ctx.cfg_mgr.load_app_config().await.unwrap_or_default();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;

        let status = match client.get(&app_cfg.stream_url).send().await {
            Ok(_) => DiagnosticStatus::Ok,
            Err(e) => DiagnosticStatus::Error(format!("Stream URL 连接失败: {}", e)),
        };

        Ok(DiagnosticResult {
            name: self.name().to_string(),
            status,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}
inventory::submit! { DiagnosticRegistration { builder: || Box::new(StreamUrlCheck) } }

struct OpenApiCheck;
#[async_trait]
impl DiagnosticTask for OpenApiCheck {
    fn name(&self) -> &str { "网络连通性 (OpenAPI)" }
    async fn run(&self, ctx: &DoctorContext) -> Result<DiagnosticResult> {
        let start = Instant::now();
        let app_cfg = ctx.cfg_mgr.load_app_config().await.unwrap_or_default();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;

        let status = match client.get(&app_cfg.openapi_url).send().await {
            Ok(_) => DiagnosticStatus::Ok,
            Err(e) => DiagnosticStatus::Error(format!("OpenAPI 连接失败: {}", e)),
        };

        Ok(DiagnosticResult {
            name: self.name().to_string(),
            status,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}
inventory::submit! { DiagnosticRegistration { builder: || Box::new(OpenApiCheck) } }

struct MonitorPortCheck;
#[async_trait]
impl DiagnosticTask for MonitorPortCheck {
    fn name(&self) -> &str { "监控端口 (Monitor Port)" }
    async fn run(&self, ctx: &DoctorContext) -> Result<DiagnosticResult> {
        let start = Instant::now();
        let app_cfg = ctx.cfg_mgr.load_app_config().await.unwrap_or_default();
        let port = if app_cfg.monitor_port == 0 { 1588 } else { app_cfg.monitor_port };
        
        let is_occupied = std::net::TcpListener::bind(("127.0.0.1", port)).is_err();
        let daemon_info = cowen_common::status::get_active_daemon_info(&ctx.profile);
        let mut occupied_by_other = false;
        
        if is_occupied {
             if let Some(info) = daemon_info {
                 if info.monitor_port == Some(port) {
                      // It's occupied by our own daemon, which is OK
                 } else {
                      occupied_by_other = true;
                 }
             } else {
                 occupied_by_other = true;
             }
        }

        let status = if occupied_by_other {
            DiagnosticStatus::Error(format!(
                "端口 {} 被占用。\n    👉 Fix: 请杀掉占用进程或运行 'cowen config set monitor_port <NEW_PORT> --global'",
                port
            ))
        } else {
            DiagnosticStatus::Ok
        };

        Ok(DiagnosticResult {
            name: self.name().to_string(),
            status,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}
inventory::submit! { DiagnosticRegistration { builder: || Box::new(MonitorPortCheck) } }
