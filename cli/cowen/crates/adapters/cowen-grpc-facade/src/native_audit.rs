use std::sync::Arc;
use tonic::{Request, Response, Status};
use cowen_common::grpc::proto::native_audit_service_server::NativeAuditService;
use cowen_common::grpc::proto::{TailAuditRequest, TailAuditResponse};
use cowen_capabilities::CapabilityRegistry;

pub struct NativeAuditController {
    pub capabilities: Arc<CapabilityRegistry>,
}

#[tonic::async_trait]
impl NativeAuditService for NativeAuditController {
    async fn tail_audit(&self, request: Request<TailAuditRequest>) -> Result<Response<TailAuditResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_audit.tail_audit(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }
}
