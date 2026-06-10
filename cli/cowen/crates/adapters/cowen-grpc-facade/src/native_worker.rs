use cowen_capabilities::CapabilityRegistry;
use cowen_common::grpc::proto::native_worker_service_server::NativeWorkerService;
use cowen_common::grpc::proto::{
    GetStatusRequest, GetStatusResponse, PingRequest, PingResponse, ReloadWorkerRequest,
    ReloadWorkerResponse, StartAllWorkersRequest, StartAllWorkersResponse, StartWorkerRequest,
    StartWorkerResponse, StopAllWorkersRequest, StopAllWorkersResponse, StopWorkerRequest,
    StopWorkerResponse,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct NativeWorkerController {
    pub capabilities: Arc<CapabilityRegistry>,
}

#[tonic::async_trait]
impl NativeWorkerService for NativeWorkerController {
    async fn ping(&self, _request: Request<PingRequest>) -> Result<Response<PingResponse>, Status> {
        Ok(Response::new(PingResponse {
            message: "pong".to_string(),
        }))
    }

    async fn start_worker(
        &self,
        request: Request<StartWorkerRequest>,
    ) -> Result<Response<StartWorkerResponse>, Status> {
        crate::grpc_forward!(self, native_worker, start_worker, request)
    }

    async fn stop_worker(
        &self,
        request: Request<StopWorkerRequest>,
    ) -> Result<Response<StopWorkerResponse>, Status> {
        crate::grpc_forward!(self, native_worker, stop_worker, request)
    }

    async fn start_all_workers(
        &self,
        request: Request<StartAllWorkersRequest>,
    ) -> Result<Response<StartAllWorkersResponse>, Status> {
        crate::grpc_forward!(self, native_worker, start_all_workers, request)
    }

    async fn stop_all_workers(
        &self,
        request: Request<StopAllWorkersRequest>,
    ) -> Result<Response<StopAllWorkersResponse>, Status> {
        crate::grpc_forward!(self, native_worker, stop_all_workers, request)
    }

    async fn reload_worker(
        &self,
        request: Request<ReloadWorkerRequest>,
    ) -> Result<Response<ReloadWorkerResponse>, Status> {
        crate::grpc_forward!(self, native_worker, reload_worker, request)
    }

    async fn get_status(
        &self,
        request: Request<GetStatusRequest>,
    ) -> Result<Response<GetStatusResponse>, Status> {
        crate::grpc_forward!(self, native_worker, get_status, request)
    }
}
