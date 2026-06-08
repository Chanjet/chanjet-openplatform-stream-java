pub mod proto {
    tonic::include_proto!("cowen.daemon.v1");
    tonic::include_proto!("cowen.daemon.api_registry.v1");
}
pub mod client;
