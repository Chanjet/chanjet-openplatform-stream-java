use std::sync::Arc;
use cowen_config::ConfigManager;
use cowen_common::vault::Vault;
use cowen_macros::{rbac, rbac_controller};
use cowen_common::CowenError;

// Domain DTOs
pub struct DomainDlqListRequest {
    pub profile: String,
    pub page_size: u32,
}

pub struct DomainDlqListResponse {
    pub json: String,
    pub error_message: Option<String>,
}

pub struct DomainDlqViewRequest {
    pub profile: String,
    pub id: String,
}

pub struct DomainDlqViewResponse {
    pub json: String,
    pub error_message: Option<String>,
}

pub struct DomainDlqRetryRequest {
    pub profile: String,
    pub id: String,
}

pub struct DomainDlqRetryResponse {
    pub success: bool,
    pub message: String,
    pub error_message: Option<String>,
}

pub struct DomainDlqPurgeRequest {
    pub profile: String,
}

pub struct DomainDlqPurgeResponse {
    pub success: bool,
    pub message: String,
    pub error_message: Option<String>,
}

#[tonic::async_trait]
pub trait NativeDlqCapability: Send + Sync {
    async fn dlq_list(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: DomainDlqListRequest) -> Result<DomainDlqListResponse, CowenError>;
    async fn dlq_view(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: DomainDlqViewRequest) -> Result<DomainDlqViewResponse, CowenError>;
    async fn dlq_retry(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: DomainDlqRetryRequest) -> Result<DomainDlqRetryResponse, CowenError>;
    async fn dlq_purge(&self, claims: Option<&cowen_common::jwt::IpcClaims>, req: DomainDlqPurgeRequest) -> Result<DomainDlqPurgeResponse, CowenError>;
}

pub struct DefaultDlq {
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
}

impl DefaultDlq {
    pub fn new(vault: Arc<dyn Vault>, cfg_mgr: ConfigManager) -> Self {
        Self { vault, cfg_mgr }
    }
}

#[rbac_controller(domain = "native.dlq")]
#[tonic::async_trait]
impl NativeDlqCapability for DefaultDlq {

    #[rbac]
    async fn dlq_list(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: DomainDlqListRequest) -> Result<DomainDlqListResponse, CowenError> {
        match self.vault.list_dlq(&req.profile, req.page_size as usize).await {
            Ok(msgs) => Ok(DomainDlqListResponse { json: serde_json::to_string(&msgs).unwrap_or_default(), error_message: None }),
            Err(e) => Ok(DomainDlqListResponse { json: "".to_string(), error_message: Some(e.to_string()) })
        }
    }

    #[rbac]
    async fn dlq_view(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: DomainDlqViewRequest) -> Result<DomainDlqViewResponse, CowenError> {
        let id_i64 = match req.id.parse::<i64>() {
            Ok(i) => i,
            Err(_) => return Err(CowenError::Validation("Invalid DLQ ID format".to_string())),
        };
        match self.vault.get_dlq_by_id(id_i64).await {
            Ok(Some(msg)) => Ok(DomainDlqViewResponse { json: serde_json::to_string(&msg).unwrap_or_default(), error_message: None }),
            Ok(None) => Ok(DomainDlqViewResponse { json: "".to_string(), error_message: Some("Not found".to_string()) }),
            Err(e) => Ok(DomainDlqViewResponse { json: "".to_string(), error_message: Some(e.to_string()) })
        }
    }

    #[rbac]
    async fn dlq_retry(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: DomainDlqRetryRequest) -> Result<DomainDlqRetryResponse, CowenError> {
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(CowenError::NotFound(e.to_string()))
        };
        let app_cfg: cowen_common::config::AppConfig = self.cfg_mgr.load_app_config().await.unwrap_or_default();
        let id_i64 = match req.id.parse::<i64>() {
            Ok(i) => i,
            Err(_) => return Err(CowenError::Validation("Invalid DLQ ID format".to_string())),
        };
        match crate::forwarder::Forwarder::new(&req.profile, config, &app_cfg, self.vault.clone()) {
            Ok(forwarder) => {
                match forwarder.retry_message(id_i64).await {
                    Ok(_) => Ok(DomainDlqRetryResponse { success: true, message: "Retried".to_string(), error_message: None }),
                    Err(e) => Ok(DomainDlqRetryResponse { success: false, message: "".to_string(), error_message: Some(e.to_string()) })
                }
            }
            Err(e) => Ok(DomainDlqRetryResponse { success: false, message: "".to_string(), error_message: Some(e.to_string()) })
        }
    }

    #[rbac]
    async fn dlq_purge(&self, _claims: Option<&cowen_common::jwt::IpcClaims>, req: DomainDlqPurgeRequest) -> Result<DomainDlqPurgeResponse, CowenError> {
        match self.vault.list_all_dlq(&req.profile).await {
            Ok(msgs) => {
                let mut count = 0;
                for m in msgs {
                    if let Some(id) = m.id {
                        if self.vault.delete_dlq_by_id(id).await.is_ok() { count += 1; }
                    }
                }
                Ok(DomainDlqPurgeResponse { success: true, message: format!("Purged {} messages", count), error_message: None })
            }
            Err(e) => Ok(DomainDlqPurgeResponse { success: false, message: "".to_string(), error_message: Some(e.to_string()) })
        }
    }
}
