pub mod proto {
    tonic::include_proto!("cowen.daemon.api_registry.v1");
}
use proto::api_registry_service_client::ApiRegistryServiceClient;

pub async fn get_grpc_client() -> Result<ApiRegistryServiceClient<tonic::transport::Channel>, String> {
    let port_str = match std::env::var("COWEN_IPC_PORT") {
        Ok(p) => p,
        Err(_) => {
            #[cfg(test)]
            {
                return Err("Missing COWEN_IPC_PORT env var".to_string());
            }
            #[cfg(not(test))]
            {
                eprintln!("Missing COWEN_IPC_PORT env var. Host failed to inject context. Exiting to trigger MCP Client restart.");
                std::process::exit(1);
            }
        }
    };
    let endpoint = format!("http://127.0.0.1:{}", port_str);
    ApiRegistryServiceClient::connect(endpoint)
        .await
        .map_err(|e| {
            #[cfg(not(test))]
            if e.to_string().contains("transport error") {
                eprintln!("Failed to connect to daemon (transport error). Exiting to trigger MCP Client restart.");
                std::process::exit(1);
            }
            e.to_string()
        })
}

pub fn handle_grpc_status(e: tonic::Status) -> String {
    #[cfg(not(test))]
    if e.code() == tonic::Code::Unavailable || e.to_string().contains("transport error") {
        eprintln!("Lost connection to daemon (transport error). Exiting to trigger MCP Client restart.");
        std::process::exit(1);
    }
    format!("gRPC Error: {}", e)
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
