use cowen_capabilities::CapabilityRegistry;
use cowen_common::grpc::proto::native_config_service_server::NativeConfigService;
use cowen_common::grpc::proto::{
    GetConfigRequest, GetConfigResponse, GetGlobalConfigRequest, GetGlobalConfigResponse,
    ListConfigRequest, ListConfigResponse, RenameProfileRequest, RenameProfileResponse,
    SetConfigRequest, SetConfigResponse, SetGlobalConfigRequest, SetGlobalConfigResponse,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct NativeConfigController {
    pub capabilities: Arc<CapabilityRegistry>,
}

#[tonic::async_trait]
impl NativeConfigService for NativeConfigController {
    async fn list_config(
        &self,
        request: Request<ListConfigRequest>,
    ) -> Result<Response<ListConfigResponse>, Status> {
        crate::grpc_forward!(self, native_config, list_config, request)
    }

    async fn get_config(
        &self,
        request: Request<GetConfigRequest>,
    ) -> Result<Response<GetConfigResponse>, Status> {
        crate::grpc_forward!(self, native_config, get_config, request)
    }

    async fn set_config(
        &self,
        request: Request<SetConfigRequest>,
    ) -> Result<Response<SetConfigResponse>, Status> {
        crate::grpc_forward!(self, native_config, set_config, request)
    }

    async fn rename_profile(
        &self,
        request: Request<RenameProfileRequest>,
    ) -> Result<Response<RenameProfileResponse>, Status> {
        crate::grpc_forward!(self, native_config, rename_profile, request)
    }

    async fn get_global_config(
        &self,
        request: Request<GetGlobalConfigRequest>,
    ) -> Result<Response<GetGlobalConfigResponse>, Status> {
        crate::grpc_forward!(self, native_config, get_global_config, request)
    }

    async fn set_global_config(
        &self,
        request: Request<SetGlobalConfigRequest>,
    ) -> Result<Response<SetGlobalConfigResponse>, Status> {
        crate::grpc_forward!(self, native_config, set_global_config, request)
    }
}
