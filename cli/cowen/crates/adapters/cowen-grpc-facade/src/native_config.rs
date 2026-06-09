use std::sync::Arc;
use tonic::{Request, Response, Status};
use cowen_common::grpc::proto::native_config_service_server::NativeConfigService;
use cowen_common::grpc::proto::{
    ListConfigRequest, ListConfigResponse, GetConfigRequest, GetConfigResponse, SetConfigRequest, SetConfigResponse,
    RenameProfileRequest, RenameProfileResponse, GetGlobalConfigRequest, GetGlobalConfigResponse, SetGlobalConfigRequest, SetGlobalConfigResponse
};
use cowen_capabilities::CapabilityRegistry;

pub struct NativeConfigController {
    pub capabilities: Arc<CapabilityRegistry>,
}

#[tonic::async_trait]
impl NativeConfigService for NativeConfigController {
    async fn list_config(&self, request: Request<ListConfigRequest>) -> Result<Response<ListConfigResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_config.list_config(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn get_config(&self, request: Request<GetConfigRequest>) -> Result<Response<GetConfigResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_config.get_config(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn set_config(&self, request: Request<SetConfigRequest>) -> Result<Response<SetConfigResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_config.set_config(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn rename_profile(&self, request: Request<RenameProfileRequest>) -> Result<Response<RenameProfileResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_config.rename_profile(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn get_global_config(&self, request: Request<GetGlobalConfigRequest>) -> Result<Response<GetGlobalConfigResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_config.get_global_config(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn set_global_config(&self, request: Request<SetGlobalConfigRequest>) -> Result<Response<SetGlobalConfigResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_config.set_global_config(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }
}
