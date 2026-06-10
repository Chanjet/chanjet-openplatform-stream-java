use anyhow::Result;
use async_trait::async_trait;
use cowen_common::config::Config;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;
use std::sync::Arc;

#[derive(Clone)]
pub struct DoctorContext {
    pub profile: String,
    pub config: Config,
    pub verbose: bool,
    pub fix: bool,
    pub vault: Arc<dyn Vault>,
    pub cfg_mgr: ConfigManager,
}

#[derive(Debug)]
pub enum DiagnosticStatus {
    Ok,
    Warning(String),
    Error(String),
    Fixed(String),
}

pub struct DiagnosticResult {
    pub name: String,
    pub status: DiagnosticStatus,
    pub duration_ms: u64,
}

#[async_trait]
pub trait DiagnosticTask: Send + Sync {
    fn name(&self) -> &str;
    async fn run(&self, ctx: &DoctorContext) -> Result<DiagnosticResult>;
}

pub struct DiagnosticRegistration {
    pub builder: fn() -> Box<dyn DiagnosticTask>,
}

inventory::collect!(DiagnosticRegistration);
