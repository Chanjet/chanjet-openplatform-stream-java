use std::sync::Arc;
use tonic::{Request, Response, Status};
use cowen_common::grpc::proto::native_worker_service_server::NativeWorkerService;
use cowen_common::grpc::proto::{
    StartWorkerRequest, StartWorkerResponse, StopWorkerRequest, StopWorkerResponse,
    StartAllWorkersRequest, StartAllWorkersResponse, StopAllWorkersRequest, StopAllWorkersResponse,
    ReloadWorkerRequest, ReloadWorkerResponse, PingRequest, PingResponse, GetStatusRequest, GetStatusResponse,
};
use cowen_capabilities::CapabilityRegistry;

pub struct NativeWorkerController {
    pub capabilities: Arc<CapabilityRegistry>,
}

#[tonic::async_trait]
impl NativeWorkerService for NativeWorkerController {
    async fn ping(&self, _request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        Ok(Response::new(PingResponse { message: "pong".to_string() }))
    }

    async fn start_worker(&self, request: Request<StartWorkerRequest>) -> Result<Response<StartWorkerResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_worker.start_worker(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn stop_worker(&self, request: Request<StopWorkerRequest>) -> Result<Response<StopWorkerResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_worker.stop_worker(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn start_all_workers(&self, request: Request<StartAllWorkersRequest>) -> Result<Response<StartAllWorkersResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_worker.start_all_workers(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn stop_all_workers(&self, request: Request<StopAllWorkersRequest>) -> Result<Response<StopAllWorkersResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_worker.stop_all_workers(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn reload_worker(&self, request: Request<ReloadWorkerRequest>) -> Result<Response<ReloadWorkerResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_worker.reload_worker(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn get_status(&self, request: Request<GetStatusRequest>) -> Result<Response<GetStatusResponse>, Status> {
        let claims = request.extensions().get::<cowen_common::jwt::IpcClaims>().cloned();
        match self.capabilities.native_worker.get_status(claims.as_ref(), request.into_inner()).await {
            Ok(resp) => Ok(Response::new(resp)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }
}
