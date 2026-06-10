use cowen_capabilities::CapabilityRegistry;
use cowen_common::grpc::proto::public_system_service_server::PublicSystemService;
use cowen_common::grpc::proto::{PluginHandshakeRequest, PluginHandshakeResponse};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct PublicSystemController {
    pub capabilities: Arc<CapabilityRegistry>,
}

#[tonic::async_trait]
impl PublicSystemService for PublicSystemController {
    async fn plugin_handshake(
        &self,
        request: Request<PluginHandshakeRequest>,
    ) -> Result<Response<PluginHandshakeResponse>, Status> {
        let claims = request
            .extensions()
            .get::<cowen_common::jwt::IpcClaims>()
            .cloned();
        let inner = request.into_inner();
        let domain_req =
            cowen_capabilities::capabilities::public::public_system::DomainPluginHandshakeRequest {
                plugin_name: inner.plugin_name,
                plugin_version: inner.plugin_version.clone(),
                required_capabilities: inner.required_capabilities,
            };
        match self
            .capabilities
            .public_system
            .plugin_handshake(claims.as_ref(), domain_req)
            .await
        {
            Ok(resp) => Ok(Response::new(PluginHandshakeResponse {
                success: resp.accepted,
                message: resp.error_message.unwrap_or_default(),
                supported_capabilities: resp.supported_capabilities,
            })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }
}
