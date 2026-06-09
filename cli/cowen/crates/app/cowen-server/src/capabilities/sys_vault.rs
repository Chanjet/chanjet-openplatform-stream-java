use std::sync::Arc;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;
use cowen_common::models::Ticket;
use cowen_common::CowenError;

#[tonic::async_trait]
pub trait SysVaultCapability: Send + Sync {
    async fn get_app_ticket(&self, app_key: &str) -> Result<Option<Ticket>, CowenError>;
    async fn get_app_secret(&self, profile: &str) -> Result<String, CowenError>;
}

pub struct DefaultSysVault {
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
}

impl DefaultSysVault {
    pub fn new(vault: Arc<dyn Vault>, cfg_mgr: ConfigManager) -> Self {
        Self { vault, cfg_mgr }
    }
}

#[tonic::async_trait]
impl SysVaultCapability for DefaultSysVault {
    async fn get_app_ticket(&self, app_key: &str) -> Result<Option<Ticket>, CowenError> {
        match self.vault.get_app_ticket(app_key).await {
            Ok(t) => Ok(Some(t)),
            Err(CowenError::NotFound(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn get_app_secret(&self, profile: &str) -> Result<String, CowenError> {
        match self.cfg_mgr.load(profile).await {
            Ok(config) => Ok(config.app_secret.clone()),
            Err(e) => Err(CowenError::config(e.to_string())),
        }
    }
}
