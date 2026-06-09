use std::sync::Arc;
use tonic::{Request, Response, Status};

use cowen_common::grpc::proto;
use proto::cowen_daemon_service_server::CowenDaemonService;
use proto::*;

use cowen_common::daemon::DaemonService;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;

pub struct CowenDaemonController {
    capabilities: Arc<cowen_capabilities::CapabilityRegistry>,
}

impl CowenDaemonController {
    pub fn new(_service: Arc<dyn DaemonService>, _vault: Arc<dyn Vault>, _cfg_mgr: ConfigManager, capabilities: Arc<cowen_capabilities::CapabilityRegistry>) -> Self {
        Self { capabilities }
    }
}

#[tonic::async_trait]
impl CowenDaemonService for CowenDaemonController {
    type TunnelPluginStream = std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<TunnelPluginResponse, Status>> + Send + 'static>>;

    async fn ping(&self, _request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        Ok(Response::new(PingResponse { message: "pong".to_string() }))
    }

    async fn start_worker(&self, request: Request<StartWorkerRequest>) -> Result<Response<StartWorkerResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.start_worker(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn start_all_workers(&self, request: Request<StartAllWorkersRequest>) -> Result<Response<StartAllWorkersResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.start_all_workers(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn stop_worker(&self, request: Request<StopWorkerRequest>) -> Result<Response<StopWorkerResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.stop_worker(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn stop_all_workers(&self, request: Request<StopAllWorkersRequest>) -> Result<Response<StopAllWorkersResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.stop_all_workers(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn reload_worker(&self, request: Request<ReloadWorkerRequest>) -> Result<Response<ReloadWorkerResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.reload_worker(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn get_status(&self, request: Request<GetStatusRequest>) -> Result<Response<GetStatusResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.get_status(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn init_profile(&self, request: Request<InitProfileRequest>) -> Result<Response<InitProfileResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.init_profile(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn get_auth_url(&self, request: Request<GetAuthUrlRequest>) -> Result<Response<GetAuthUrlResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.get_auth_url(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn wait_for_auth(&self, request: Request<WaitForAuthRequest>) -> Result<Response<WaitForAuthResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.wait_for_auth(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn get_token(&self, request: Request<GetTokenRequest>) -> Result<Response<GetTokenResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.get_token(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn clear_token(&self, request: Request<ClearTokenRequest>) -> Result<Response<ClearTokenResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.clear_token(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn get_global_config(&self, request: Request<GetGlobalConfigRequest>) -> Result<Response<GetGlobalConfigResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.get_global_config(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn set_global_config(&self, request: Request<SetGlobalConfigRequest>) -> Result<Response<SetGlobalConfigResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.set_global_config(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn get_config(&self, request: Request<GetConfigRequest>) -> Result<Response<GetConfigResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.get_config(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn list_config(&self, request: Request<ListConfigRequest>) -> Result<Response<ListConfigResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.list_config(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn set_config(&self, request: Request<SetConfigRequest>) -> Result<Response<SetConfigResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.set_config(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn store_status(&self, request: Request<StoreStatusRequest>) -> Result<Response<StoreStatusResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let domain_req = cowen_capabilities::native_system::DomainStoreStatusRequest {};
        match self.capabilities.native_system.store_status(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(StoreStatusResponse { json: resp.json, error_message: resp.error_message })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn system_status(&self, request: Request<SystemStatusRequest>) -> Result<Response<SystemStatusResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_system::DomainSystemStatusRequest { profile: inner.profile, all: inner.all };
        match self.capabilities.native_system.system_status(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(SystemStatusResponse { json: resp.json, error_message: resp.error_message })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn system_reset(&self, request: Request<SystemResetRequest>) -> Result<Response<SystemResetResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_system::DomainSystemResetRequest { profile: inner.profile.filter(|p| !p.trim().is_empty()), dry_run: inner.dry_run };
        match self.capabilities.native_system.system_reset(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(SystemResetResponse { success: resp.success, message: resp.message })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn rename_profile(&self, request: Request<RenameProfileRequest>) -> Result<Response<RenameProfileResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.rename_profile(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn doctor(&self, request: Request<DoctorRequest>) -> Result<Response<DoctorResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_system::DomainDoctorRequest { profile: inner.profile };
        match self.capabilities.native_system.doctor(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(DoctorResponse { report: resp.report, error_message: resp.error_message })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn dlq_list(&self, request: Request<DlqListRequest>) -> Result<Response<DlqListResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_dlq::DomainDlqListRequest { profile: inner.profile, page_size: inner.page_size };
        match self.capabilities.native_dlq.dlq_list(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(DlqListResponse { json: resp.json, error_message: resp.error_message })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn dlq_view(&self, request: Request<DlqViewRequest>) -> Result<Response<DlqViewResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_dlq::DomainDlqViewRequest { profile: inner.profile, id: inner.id };
        match self.capabilities.native_dlq.dlq_view(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(DlqViewResponse { json: resp.json, error_message: resp.error_message })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn dlq_retry(&self, request: Request<DlqRetryRequest>) -> Result<Response<DlqRetryResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_dlq::DomainDlqRetryRequest { profile: inner.profile, id: inner.id };
        match self.capabilities.native_dlq.dlq_retry(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(DlqRetryResponse { success: resp.success, message: resp.message, error_message: resp.error_message })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn dlq_purge(&self, request: Request<DlqPurgeRequest>) -> Result<Response<DlqPurgeResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_dlq::DomainDlqPurgeRequest { profile: inner.profile };
        match self.capabilities.native_dlq.dlq_purge(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(DlqPurgeResponse { success: resp.success, message: resp.message, error_message: resp.error_message })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn tail_audit(&self, request: Request<TailAuditRequest>) -> Result<Response<TailAuditResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_daemon.tail_audit(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn tunnel_plugin(
        &self,
        request: Request<tonic::Streaming<TunnelPluginRequest>>,
    ) -> Result<Response<Self::TunnelPluginStream>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let stream = request.into_inner();
        match self.capabilities.native_system.tunnel_plugin(claims.as_ref(), stream).await {
            Ok(resp_stream) => {
                let stream = tokio_stream::StreamExt::map(resp_stream, |res| match res {
                    Ok(v) => Ok(v),
                    Err(e) => Err(Status::internal(e.to_string())),
                });
                Ok(Response::new(Box::pin(stream)))
            }
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn plugin_handshake(&self, request: Request<PluginHandshakeRequest>) -> Result<Response<PluginHandshakeResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_system::DomainPluginHandshakeRequest {
            plugin_name: inner.plugin_name,
            plugin_version: inner.plugin_version.clone(),
            protocol_version: inner.plugin_version, // GRPC doesn't have protocol_version, use plugin_version for now
            required_capabilities: inner.required_capabilities,
        };
        match self.capabilities.native_system.plugin_handshake(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(PluginHandshakeResponse {
                success: resp.accepted,
                message: resp.error_message.unwrap_or_default(),
                supported_capabilities: resp.supported_capabilities,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }
}
