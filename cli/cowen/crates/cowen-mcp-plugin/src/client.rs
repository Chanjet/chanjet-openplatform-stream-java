pub mod proto {
    tonic::include_proto!("cowen.daemon.v1");
}
use proto::cowen_daemon_service_client::CowenDaemonServiceClient;

pub async fn get_grpc_client() -> Result<CowenDaemonServiceClient<tonic::transport::Channel>, String> {
    let port_str = std::env::var("COWEN_IPC_PORT")
        .map_err(|_| "Missing COWEN_IPC_PORT env var".to_string())?;
    let endpoint = format!("http://127.0.0.1:{}", port_str);
    CowenDaemonServiceClient::connect(endpoint)
        .await
        .map_err(|e| e.to_string())
}

pub fn inject_auth<T>(req: T) -> tonic::Request<T> {
    let mut request = tonic::Request::new(req);
    if let Ok(token) = std::env::var("COWEN_BRIDGE_TOKEN") {
        if let Ok(meta_value) = format!("Bearer {}", token).parse() {
            request.metadata_mut().insert("authorization", meta_value);
        }
    }
    request
}
