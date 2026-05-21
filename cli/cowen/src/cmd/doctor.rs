use anyhow::Result;
use cowen_common::config::Config;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;
use std::sync::Arc;
use colored::*;
use cowen_doctor::{DoctorContext, DiagnosticTask, DiagnosticResult, DiagnosticStatus, DiagnosticRegistration};
use async_trait::async_trait;
use std::time::Instant;

pub async fn execute(profile: &str, config: &Config, verbose: bool, fix: bool, vault: Arc<dyn Vault>, cfg_mgr: &ConfigManager) -> Result<()> {
    println!("\n{} {} (Profile: {})", "🩺".bold(), "Cowen Doctor - 环境诊断工具".bold(), profile.cyan());
    println!("{}\n", "=".repeat(60).dimmed());

    let ctx = DoctorContext {
        profile: profile.to_string(),
        config: config.clone(),
        verbose,
        fix,
        vault,
        cfg_mgr: cfg_mgr.clone(),
    };

    let start_time = Instant::now();
    let results = cowen_doctor::run_all_diagnostics(&ctx).await?;
    let duration = start_time.elapsed().as_millis();

    let mut all_ok = true;
    for (i, res) in results.iter().enumerate() {
        let status_str = match &res.status {
            DiagnosticStatus::Ok => "OK".green().to_string(),
            DiagnosticStatus::Warning(w) => {
                all_ok = false;
                format!("{} ({})", "WARNING".yellow(), w)
            }
            DiagnosticStatus::Error(e) => {
                all_ok = false;
                format!("{} ({})", "ERROR".red(), e)
            }
            DiagnosticStatus::Fixed(f) => format!("{} ({})", "FIXED".green(), f),
        };
        println!("{} [{}] {:<20} {} ({}ms)", "•".dimmed(), i + 1, res.name.bold(), status_str, res.duration_ms);
    }

    println!("\n{}", "=".repeat(60).dimmed());
    println!("诊断总耗时: {}ms", duration);
    if all_ok {
        println!("{} {}", "✅".bold(), "诊断完成，环境运行状况良好。".green().bold());
    } else {
        println!("{} {}", "⚠️".bold(), "诊断发现部分问题，建议运行 'cowen events' 查询详情。".yellow().bold());
    }

    Ok(())
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

struct StorageCheck;
#[async_trait]
impl DiagnosticTask for StorageCheck {
    fn name(&self) -> &str { "存储后端与Schema" }
    async fn run(&self, ctx: &DoctorContext) -> Result<DiagnosticResult> {
        let start = Instant::now();
        let store = crate::core::create_store(&ctx.cfg_mgr).await?;
        
        let status = match store.list_dlq_paged(&ctx.profile, 0, 1).await {
            Ok(_) => DiagnosticStatus::Ok,
            Err(_) => {
                if ctx.fix {
                    match store.migrate().await {
                        Ok(_) => DiagnosticStatus::Fixed("Schema 修复成功".to_string()),
                        Err(e) => DiagnosticStatus::Error(format!("Schema 修复失败: {}", e)),
                    }
                } else {
                    DiagnosticStatus::Error("Schema 可能需要更新".to_string())
                }
            }
        };

        Ok(DiagnosticResult {
            name: self.name().to_string(),
            status,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}
inventory::submit! { DiagnosticRegistration { builder: || Box::new(StorageCheck) } }

struct NetworkCheck;
#[async_trait]
impl DiagnosticTask for NetworkCheck {
    fn name(&self) -> &str { "网络连通性" }
    async fn run(&self, ctx: &DoctorContext) -> Result<DiagnosticResult> {
        let start = Instant::now();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;

        let mut status = DiagnosticStatus::Ok;
        match client.get(&ctx.config.stream_url).send().await {
            Ok(_) => {},
            Err(e) => status = DiagnosticStatus::Error(format!("Stream URL 连接失败: {}", e)),
        }

        Ok(DiagnosticResult {
            name: self.name().to_string(),
            status,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}
inventory::submit! { DiagnosticRegistration { builder: || Box::new(NetworkCheck) } }

struct CredentialsCheck;
#[async_trait]
impl DiagnosticTask for CredentialsCheck {
    fn name(&self) -> &str { "凭据与认证" }
    async fn run(&self, ctx: &DoctorContext) -> Result<DiagnosticResult> {
        let start = Instant::now();
        let status = match ctx.vault.get_secret(&ctx.profile, "app_secret").await {
            Ok(s) if !s.is_empty() => DiagnosticStatus::Ok,
            _ => DiagnosticStatus::Error("App Secret Missing".to_string()),
        };

        Ok(DiagnosticResult {
            name: self.name().to_string(),
            status,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}
inventory::submit! { DiagnosticRegistration { builder: || Box::new(CredentialsCheck) } }

