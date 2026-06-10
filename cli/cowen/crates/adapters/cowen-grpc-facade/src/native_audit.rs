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
        crate::grpc_forward!(self, native_audit, tail_audit, request)
    }
}
