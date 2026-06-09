use std::sync::Arc;
use std::pin::Pin;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use cowen_common::grpc::proto::native_system_service_server::NativeSystemService;
use cowen_common::grpc::proto::{
    StoreStatusRequest, StoreStatusResponse, SystemStatusRequest, SystemStatusResponse,
    SystemResetRequest, SystemResetResponse, DoctorRequest, DoctorResponse,
    TunnelPluginRequest, TunnelPluginResponse,
};
use cowen_capabilities::CapabilityRegistry;

pub struct NativeSystemController {
    pub capabilities: Arc<CapabilityRegistry>,
}

#[tonic::async_trait]
impl NativeSystemService for NativeSystemController {
    type TunnelPluginStream = Pin<Box<dyn Stream<Item = Result<TunnelPluginResponse, Status>> + Send>>;

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

    async fn doctor(&self, request: Request<DoctorRequest>) -> Result<Response<DoctorResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_system::DomainDoctorRequest { profile: inner.profile };
        match self.capabilities.native_system.doctor(claims.as_ref(), domain_req).await {
            Ok(resp) => Ok(Response::new(DoctorResponse { report: resp.report, error_message: resp.error_message })),
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
}
