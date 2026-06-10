use cowen_capabilities::CapabilityRegistry;
use cowen_common::grpc::proto::native_auth_service_server::NativeAuthService;
use cowen_common::grpc::proto::{
    ClearTokenRequest, ClearTokenResponse, GetAuthUrlRequest, GetAuthUrlResponse, GetTokenRequest,
    GetTokenResponse, InitProfileRequest, InitProfileResponse, WaitForAuthRequest,
    WaitForAuthResponse,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct NativeAuthController {
    pub capabilities: Arc<CapabilityRegistry>,
}

#[tonic::async_trait]
impl NativeAuthService for NativeAuthController {
    async fn init_profile(
        &self,
        request: Request<InitProfileRequest>,
    ) -> Result<Response<InitProfileResponse>, Status> {
        crate::grpc_forward!(self, native_auth, init_profile, request)
    }

    async fn get_auth_url(
        &self,
        request: Request<GetAuthUrlRequest>,
    ) -> Result<Response<GetAuthUrlResponse>, Status> {
        crate::grpc_forward!(self, native_auth, get_auth_url, request)
    }

    async fn wait_for_auth(
        &self,
        request: Request<WaitForAuthRequest>,
    ) -> Result<Response<WaitForAuthResponse>, Status> {
        crate::grpc_forward!(self, native_auth, wait_for_auth, request)
    }

    async fn get_token(
        &self,
        request: Request<GetTokenRequest>,
    ) -> Result<Response<GetTokenResponse>, Status> {
        crate::grpc_forward!(self, native_auth, get_token, request)
    }

    async fn clear_token(
        &self,
        request: Request<ClearTokenRequest>,
    ) -> Result<Response<ClearTokenResponse>, Status> {
        crate::grpc_forward!(self, native_auth, clear_token, request)
    }
}
