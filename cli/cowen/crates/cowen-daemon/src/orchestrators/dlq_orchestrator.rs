use std::sync::Arc;
use cowen_config::ConfigManager;
use cowen_common::vault::Vault;
use tonic::{Response, Status};
use cowen_common::grpc::proto::{
    DlqListRequest, DlqListResponse,
    DlqViewRequest, DlqViewResponse,
    DlqRetryRequest, DlqRetryResponse,
    DlqPurgeRequest, DlqPurgeResponse,
};

pub struct DlqOrchestrator {
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
}

impl DlqOrchestrator {
    pub fn new(vault: Arc<dyn Vault>, cfg_mgr: ConfigManager) -> Self {
        Self { vault, cfg_mgr }
    }

    pub async fn dlq_list(&self, req: DlqListRequest) -> Result<Response<DlqListResponse>, Status> {
        match self.vault.list_dlq(&req.profile, req.page_size as usize).await {
            Ok(msgs) => Ok(Response::new(DlqListResponse { json: serde_json::to_string(&msgs).unwrap_or_default(), error_message: None })),
            Err(e) => Ok(Response::new(DlqListResponse { json: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }

    pub async fn dlq_view(&self, req: DlqViewRequest) -> Result<Response<DlqViewResponse>, Status> {
        let id_i64 = match req.id.parse::<i64>() {
            Ok(i) => i,
            Err(_) => return Err(Status::invalid_argument("Invalid DLQ ID format")),
        };
        match self.vault.get_dlq_by_id(id_i64).await {
            Ok(Some(msg)) => Ok(Response::new(DlqViewResponse { json: serde_json::to_string(&msg).unwrap_or_default(), error_message: None })),
            Ok(None) => Ok(Response::new(DlqViewResponse { json: "".to_string(), error_message: Some("Not found".to_string()) })),
            Err(e) => Ok(Response::new(DlqViewResponse { json: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }

    pub async fn dlq_retry(&self, req: DlqRetryRequest) -> Result<Response<DlqRetryResponse>, Status> {
        let config = match self.cfg_mgr.load(&req.profile).await {
            Ok(c) => c,
            Err(e) => return Err(Status::not_found(e.to_string()))
        };
        let app_cfg: cowen_common::config::AppConfig = self.cfg_mgr.load_app_config().await.unwrap_or_default();
        let id_i64 = match req.id.parse::<i64>() {
            Ok(i) => i,
            Err(_) => return Err(Status::invalid_argument("Invalid DLQ ID format")),
        };
        match cowen_server::daemon::forwarder::Forwarder::new(&req.profile, config, &app_cfg, self.vault.clone()) {
            Ok(forwarder) => {
                match forwarder.retry_message(id_i64).await {
                    Ok(_) => Ok(Response::new(DlqRetryResponse { success: true, message: "Retried".to_string(), error_message: None })),
                    Err(e) => Ok(Response::new(DlqRetryResponse { success: false, message: "".to_string(), error_message: Some(e.to_string()) }))
                }
            }
            Err(e) => Ok(Response::new(DlqRetryResponse { success: false, message: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }

    pub async fn dlq_purge(&self, req: DlqPurgeRequest) -> Result<Response<DlqPurgeResponse>, Status> {
        match self.vault.list_all_dlq(&req.profile).await {
            Ok(msgs) => {
                let mut count = 0;
                for m in msgs {
                    if let Some(id) = m.id {
                        if self.vault.delete_dlq_by_id(id).await.is_ok() { count += 1; }
                    }
                }
                Ok(Response::new(DlqPurgeResponse { success: true, message: format!("Purged {} messages", count), error_message: None }))
            }
            Err(e) => Ok(Response::new(DlqPurgeResponse { success: false, message: "".to_string(), error_message: Some(e.to_string()) }))
        }
    }
}
