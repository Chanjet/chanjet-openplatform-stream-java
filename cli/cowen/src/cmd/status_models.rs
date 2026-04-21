use serde::Serialize;
use anyhow::Result;
use std::sync::Arc;
use crate::core::vault::Vault;
use crate::core::config::Config;

#[derive(Debug, Serialize, Clone, PartialEq)]
pub enum StatusLevel {
    OK,
    WARN,
    ERROR,
    #[allow(dead_code)]
    PENDING,
    NONE,
}

#[derive(Debug, Serialize, Clone)]
pub struct StatusEntry {
    pub name: String,
    pub icon: String,
    pub level: StatusLevel,
    pub message: String,
    pub details: Vec<String>,
    pub children: Vec<StatusEntry>,
}

pub struct StatusContext<'a> {
    pub profile: String,
    pub config: &'a Config,
    pub vault: Arc<dyn Vault>,
}

#[async_trait::async_trait]
pub trait StatusCollector: Send + Sync {
    #[allow(dead_code)]
    fn name(&self) -> &str;
    async fn collect(&self, ctx: &StatusContext<'_>) -> Result<StatusEntry>;
}
