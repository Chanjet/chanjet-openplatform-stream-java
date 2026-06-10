use cowen_capabilities::CapabilityRegistry;
use cowen_common::grpc::proto::native_dlq_service_server::NativeDlqService;
use cowen_common::grpc::proto::{
    DlqListRequest, DlqListResponse, DlqPurgeRequest, DlqPurgeResponse, DlqRetryRequest,
    DlqRetryResponse, DlqViewRequest, DlqViewResponse,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct NativeDlqController {
    pub capabilities: Arc<CapabilityRegistry>,
}

#[tonic::async_trait]
impl NativeDlqService for NativeDlqController {
    async fn dlq_list(
        &self,
        request: Request<DlqListRequest>,
    ) -> Result<Response<DlqListResponse>, Status> {
        let claims = request
            .extensions()
            .get::<cowen_common::jwt::IpcClaims>()
            .cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_dlq::DomainDlqListRequest {
            profile: inner.profile,
            page_size: inner.page_size,
        };
        match self
            .capabilities
            .native_dlq
            .dlq_list(claims.as_ref(), domain_req)
            .await
        {
            Ok(resp) => Ok(Response::new(DlqListResponse {
                json: resp.json,
                error_message: resp.error_message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn dlq_view(
        &self,
        request: Request<DlqViewRequest>,
    ) -> Result<Response<DlqViewResponse>, Status> {
        let claims = request
            .extensions()
            .get::<cowen_common::jwt::IpcClaims>()
            .cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_dlq::DomainDlqViewRequest {
            profile: inner.profile,
            id: inner.id,
        };
        match self
            .capabilities
            .native_dlq
            .dlq_view(claims.as_ref(), domain_req)
            .await
        {
            Ok(resp) => Ok(Response::new(DlqViewResponse {
                json: resp.json,
                error_message: resp.error_message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn dlq_retry(
        &self,
        request: Request<DlqRetryRequest>,
    ) -> Result<Response<DlqRetryResponse>, Status> {
        let claims = request
            .extensions()
            .get::<cowen_common::jwt::IpcClaims>()
            .cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_dlq::DomainDlqRetryRequest {
            profile: inner.profile,
            id: inner.id,
        };
        match self
            .capabilities
            .native_dlq
            .dlq_retry(claims.as_ref(), domain_req)
            .await
        {
            Ok(resp) => Ok(Response::new(DlqRetryResponse {
                success: resp.success,
                message: resp.message,
                error_message: resp.error_message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn dlq_purge(
        &self,
        request: Request<DlqPurgeRequest>,
    ) -> Result<Response<DlqPurgeResponse>, Status> {
        let claims = request
            .extensions()
            .get::<cowen_common::jwt::IpcClaims>()
            .cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_dlq::DomainDlqPurgeRequest {
            profile: inner.profile,
        };
        match self
            .capabilities
            .native_dlq
            .dlq_purge(claims.as_ref(), domain_req)
            .await
        {
            Ok(resp) => Ok(Response::new(DlqPurgeResponse {
                success: resp.success,
                message: resp.message,
                error_message: resp.error_message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }
}
