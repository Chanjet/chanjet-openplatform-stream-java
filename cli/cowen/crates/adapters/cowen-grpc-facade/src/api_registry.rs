use cowen_common::grpc::proto::api_registry_service_server::ApiRegistryService;
use cowen_common::grpc::proto::{
    ApiListRequest, ApiListResponse, ApiSpecRequest, ApiSpecResponse, CallApiRequest,
    CallApiResponse,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct ApiRegistryController {
    capabilities: Arc<cowen_capabilities::CapabilityRegistry>,
}

impl ApiRegistryController {
    pub fn new(capabilities: Arc<cowen_capabilities::CapabilityRegistry>) -> Self {
        Self { capabilities }
    }
}

#[tonic::async_trait]
impl ApiRegistryService for ApiRegistryController {
    async fn api_spec(
        &self,
        request: Request<ApiSpecRequest>,
    ) -> Result<Response<ApiSpecResponse>, Status> {
        let claims = request
            .extensions()
            .get::<cowen_common::jwt::IpcClaims>()
            .cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_api_registry::DomainApiSpecRequest {
            profile: inner.profile,
            method: inner.method,
            path: inner.path,
        };
        match self
            .capabilities
            .native_api_registry
            .api_spec(claims.as_ref(), domain_req)
            .await
        {
            Ok(resp) => Ok(Response::new(ApiSpecResponse {
                json: resp.json,
                error_message: resp.error_message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn call_api(
        &self,
        request: Request<CallApiRequest>,
    ) -> Result<Response<CallApiResponse>, Status> {
        let claims = request
            .extensions()
            .get::<cowen_common::jwt::IpcClaims>()
            .cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_api_registry::DomainCallApiRequest {
            profile: inner.profile,
            method: inner.method,
            path: inner.path,
            data: inner.data,
            force: inner.force,
        };
        match self
            .capabilities
            .native_api_registry
            .call_api(claims.as_ref(), domain_req)
            .await
        {
            Ok(resp) => Ok(Response::new(CallApiResponse {
                status: resp.status,
                headers: resp.headers,
                body: resp.body,
                error_message: resp.error_message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn api_list(
        &self,
        request: Request<ApiListRequest>,
    ) -> Result<Response<ApiListResponse>, Status> {
        let claims = request
            .extensions()
            .get::<cowen_common::jwt::IpcClaims>()
            .cloned();
        let inner = request.into_inner();
        let domain_req = cowen_capabilities::native_api_registry::DomainApiListRequest {
            profile: inner.profile,
            search: inner.search,
            page: inner.page,
            page_size: inner.page_size,
            refresh: inner.refresh,
        };
        match self
            .capabilities
            .native_api_registry
            .api_list(claims.as_ref(), domain_req)
            .await
        {
            Ok(resp) => Ok(Response::new(ApiListResponse {
                total: resp.total,
                json: resp.json,
                plugin_used: resp.plugin_used,
                error_message: resp.error_message,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }
}
