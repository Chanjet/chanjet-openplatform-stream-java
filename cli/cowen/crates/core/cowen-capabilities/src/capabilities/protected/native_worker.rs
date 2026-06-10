#![allow(dead_code, unused_imports, unused_variables)]
// Worker specific capability
use cowen_auth::client::Client;
use cowen_common::{daemon::DaemonService, grpc::proto::*, vault::Vault, CowenError};
use cowen_config::ConfigManager;
use cowen_macros::{rbac, rbac_controller};
use std::sync::Arc;
use tracing::info;

#[tonic::async_trait]
pub trait NativeWorkerCapability: Send + Sync {
    async fn start_worker(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: StartWorkerRequest,
    ) -> Result<StartWorkerResponse, CowenError>;
    async fn start_all_workers(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: StartAllWorkersRequest,
    ) -> Result<StartAllWorkersResponse, CowenError>;
    async fn stop_worker(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: StopWorkerRequest,
    ) -> Result<StopWorkerResponse, CowenError>;
    async fn stop_all_workers(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: StopAllWorkersRequest,
    ) -> Result<StopAllWorkersResponse, CowenError>;
    async fn reload_worker(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: ReloadWorkerRequest,
    ) -> Result<ReloadWorkerResponse, CowenError>;
    async fn get_status(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: GetStatusRequest,
    ) -> Result<GetStatusResponse, CowenError>;
}

pub struct DefaultWorkerCapability {
    service: Arc<dyn DaemonService>,
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
}

impl DefaultWorkerCapability {
    pub fn new(
        service: Arc<dyn DaemonService>,
        vault: Arc<dyn Vault>,
        cfg_mgr: ConfigManager,
    ) -> Self {
        Self {
            service,
            vault,
            cfg_mgr,
        }
    }
}

#[rbac_controller(domain = "native.worker")]
#[tonic::async_trait]
impl NativeWorkerCapability for DefaultWorkerCapability {
    #[rbac]
    async fn start_worker(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: StartWorkerRequest,
    ) -> Result<StartWorkerResponse, CowenError> {
        info!("StartWorker requested for {}", req.profile);

        let _config = if req.config_json.is_empty() {
            match self.cfg_mgr.load(&req.profile).await {
                Ok(c) => c,
                Err(e) => {
                    return Ok(StartWorkerResponse {
                        success: false,
                        message: format!("Profile not found: {}", e),
                    })
                }
            }
        } else {
            match serde_json::from_str(&req.config_json) {
                Ok(c) => c,
                Err(e) => {
                    return Ok(StartWorkerResponse {
                        success: false,
                        message: e.to_string(),
                    })
                }
            }
        };

        match self.service.start_daemon(&req.profile).await {
            Ok(_) => Ok(StartWorkerResponse {
                success: true,
                message: format!("Worker {} started", req.profile),
            }),
            Err(e) => Ok(StartWorkerResponse {
                success: false,
                message: e.to_string(),
            }),
        }
    }

    #[rbac]
    async fn start_all_workers(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        _req: StartAllWorkersRequest,
    ) -> Result<StartAllWorkersResponse, CowenError> {
        info!("StartAllWorkers requested");
        match self.service.start_all().await {
            Ok(_) => Ok(StartAllWorkersResponse {
                success: true,
                message: "All workers started".to_string(),
            }),
            Err(e) => Ok(StartAllWorkersResponse {
                success: false,
                message: e.to_string(),
            }),
        }
    }

    #[rbac]
    async fn stop_worker(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: StopWorkerRequest,
    ) -> Result<StopWorkerResponse, CowenError> {
        info!("StopWorker requested for {}", req.profile);
        match self.service.stop_daemon(&req.profile).await {
            Ok(_) => Ok(StopWorkerResponse {
                success: true,
                message: format!("Worker {} stopped", req.profile),
            }),
            Err(e) => Ok(StopWorkerResponse {
                success: false,
                message: e.to_string(),
            }),
        }
    }

    #[rbac]
    async fn stop_all_workers(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        _req: StopAllWorkersRequest,
    ) -> Result<StopAllWorkersResponse, CowenError> {
        info!("StopAllWorkers requested");
        match self.service.stop_all().await {
            Ok(_) => Ok(StopAllWorkersResponse {
                success: true,
                message: "All workers stopped".to_string(),
            }),
            Err(e) => Ok(StopAllWorkersResponse {
                success: false,
                message: e.to_string(),
            }),
        }
    }

    #[rbac]
    async fn reload_worker(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: ReloadWorkerRequest,
    ) -> Result<ReloadWorkerResponse, CowenError> {
        info!("ReloadWorker requested for {}", req.profile);
        match self.service.reload_daemon(&req.profile).await {
            Ok(_) => Ok(ReloadWorkerResponse {
                success: true,
                message: format!("Worker {} reloaded", req.profile),
            }),
            Err(e) => Ok(ReloadWorkerResponse {
                success: false,
                message: e.to_string(),
            }),
        }
    }

    #[rbac]
    // get_status has no rbac in original
    async fn get_status(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        _req: GetStatusRequest,
    ) -> Result<GetStatusResponse, CowenError> {
        Ok(GetStatusResponse {
            statuses: std::collections::HashMap::new(),
        })
    }
}
