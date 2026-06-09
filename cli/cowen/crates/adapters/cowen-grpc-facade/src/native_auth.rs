use std::sync::Arc;
use tonic::{Request, Response, Status};
use cowen_common::grpc::proto::native_auth_service_server::NativeAuthService;
use cowen_common::grpc::proto::{
    InitProfileRequest, InitProfileResponse, GetAuthUrlRequest, GetAuthUrlResponse,
    WaitForAuthRequest, WaitForAuthResponse, GetTokenRequest, GetTokenResponse,
    ClearTokenRequest, ClearTokenResponse,
};
use cowen_capabilities::CapabilityRegistry;

pub struct NativeAuthController {
    pub capabilities: Arc<CapabilityRegistry>,
}

#[tonic::async_trait]
impl NativeAuthService for NativeAuthController {
    async fn init_profile(&self, request: Request<InitProfileRequest>) -> Result<Response<InitProfileResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_auth.init_profile(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn get_auth_url(&self, request: Request<GetAuthUrlRequest>) -> Result<Response<GetAuthUrlResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_auth.get_auth_url(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn wait_for_auth(&self, request: Request<WaitForAuthRequest>) -> Result<Response<WaitForAuthResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_auth.wait_for_auth(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn get_token(&self, request: Request<GetTokenRequest>) -> Result<Response<GetTokenResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_auth.get_token(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn clear_token(&self, request: Request<ClearTokenRequest>) -> Result<Response<ClearTokenResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_auth.clear_token(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }
}
