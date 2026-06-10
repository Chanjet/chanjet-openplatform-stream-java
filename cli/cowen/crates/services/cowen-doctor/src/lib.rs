pub mod task;

use anyhow::Result;
use std::time::Instant;
pub use task::*;

pub async fn run_all_diagnostics(ctx: &DoctorContext) -> Result<Vec<DiagnosticResult>> {
    let mut set = tokio::task::JoinSet::new();

    for reg in inventory::iter::<DiagnosticRegistration> {
        let task = (reg.builder)();
        let ctx_clone = ctx.clone();
        set.spawn(async move { task.run(&ctx_clone).await });
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

macro_rules! define_diagnostic {
    ($struct_name:ident, $name:expr, |$self:ident, $ctx:ident, $start:ident| $run_body:expr) => {
        struct $struct_name;
        #[async_trait::async_trait]
        impl DiagnosticTask for $struct_name {
            fn name(&self) -> &str {
                $name
            }
            async fn run(&self, $ctx: &DoctorContext) -> Result<DiagnosticResult> {
                let $self = self;
                let $start = Instant::now();
                let status = $run_body;
                Ok(DiagnosticResult {
                    name: self.name().to_string(),
                    status,
                    duration_ms: $start.elapsed().as_millis() as u64,
                })
            }
        }
        inventory::submit! { DiagnosticRegistration { builder: || Box::new($struct_name) } }
    };
}

// ---------------------------------------------------------

define_diagnostic!(SystemInfoCheck, "系统与配置", |_self, _ctx, start| {
    let app_dir = cowen_common::config::get_app_dir();
    if !app_dir.exists() {
        DiagnosticStatus::Error("Home Dir Missing".to_string())
    } else {
        DiagnosticStatus::Ok
    }
});

define_diagnostic!(
    StreamUrlCheck,
    "网络连通性 (Stream)",
    |_self, ctx, start| {
        let app_cfg = ctx.cfg_mgr.load_app_config().await.unwrap_or_default();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;
        match client.get(&app_cfg.stream_url).send().await {
            Ok(_) => DiagnosticStatus::Ok,
            Err(e) => DiagnosticStatus::Error(format!("Stream URL 连接失败: {}", e)),
        }
    }
);

define_diagnostic!(
    OpenApiCheck,
    "网络连通性 (OpenAPI)",
    |_self, ctx, start| {
        let app_cfg = ctx.cfg_mgr.load_app_config().await.unwrap_or_default();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;
        match client.get(&app_cfg.openapi_url).send().await {
            Ok(_) => DiagnosticStatus::Ok,
            Err(e) => DiagnosticStatus::Error(format!("OpenAPI 连接失败: {}", e)),
        }
    }
);

define_diagnostic!(
    MonitorPortCheck,
    "监控端口 (Monitor Port)",
    |_self, ctx, start| {
        let app_cfg = ctx.cfg_mgr.load_app_config().await.unwrap_or_default();
        let port = if app_cfg.monitor_port == 0 {
            1588
        } else {
            app_cfg.monitor_port
        };
        let is_occupied = std::net::TcpListener::bind(("127.0.0.1", port)).is_err();
        let daemon_info = cowen_common::status::get_active_daemon_info(&ctx.profile);
        let mut occupied_by_other = false;
        if is_occupied {
            if let Some(info) = daemon_info {
                if info.monitor_port != Some(port) {
                    occupied_by_other = true;
                }
            } else {
                occupied_by_other = true;
            }
        }
        if occupied_by_other {
            DiagnosticStatus::Error(format!(
                "端口 {} 被占用。
    👉 Fix: 请杀掉占用进程或运行 'cowen config set monitor_port <NEW_PORT> --global'",
                port
            ))
        } else {
            DiagnosticStatus::Ok
        }
    }
);
