pub mod proto {
    tonic::include_proto!("cowen.daemon.native.api_registry.v1");
    tonic::include_proto!("cowen.daemon.native.audit.v1");
    tonic::include_proto!("cowen.daemon.native.auth.v1");
    tonic::include_proto!("cowen.daemon.native.config.v1");
    tonic::include_proto!("cowen.daemon.native.dlq.v1");
    tonic::include_proto!("cowen.daemon.native.system.v1");
    tonic::include_proto!("cowen.daemon.native.worker.v1");
    tonic::include_proto!("cowen.daemon.public.system.v1");
}
pub mod client;
