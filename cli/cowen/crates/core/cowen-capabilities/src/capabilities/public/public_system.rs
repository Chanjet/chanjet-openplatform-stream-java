use cowen_common::CowenError;
use std::collections::HashMap;

pub struct DomainPluginHandshakeRequest {
    pub plugin_name: String,
    pub plugin_version: String,
    pub required_capabilities: HashMap<String, String>,
}

pub struct DomainPluginHandshakeResponse {
    pub accepted: bool,
    pub server_version: String,
    pub error_message: Option<String>,
    pub supported_capabilities: HashMap<String, String>,
}

#[tonic::async_trait]
pub trait PublicSystemCapability: Send + Sync {
    async fn plugin_handshake(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: DomainPluginHandshakeRequest,
    ) -> Result<DomainPluginHandshakeResponse, CowenError>;
}

pub struct DefaultPublicSystem {
    supported_capabilities: HashMap<String, String>,
}

impl DefaultPublicSystem {
    pub fn new(supported_capabilities: HashMap<String, String>) -> Self {
        Self { supported_capabilities }
    }
}

#[tonic::async_trait]
impl PublicSystemCapability for DefaultPublicSystem {
    async fn plugin_handshake(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: DomainPluginHandshakeRequest,
    ) -> Result<DomainPluginHandshakeResponse, CowenError> {
        
        let supported = self.supported_capabilities.clone();

        let mut missing = vec![];
        let mut incompatible = vec![];
        for (cap, req_ver) in &req.required_capabilities {
            if let Some(supported_vers) = supported.get(cap) {
                let req_parsed = match semver::VersionReq::parse(req_ver) {
                    Ok(r) => r,
                    Err(_) => {
                        incompatible.push(format!("{} (invalid version format: {})", cap, req_ver));
                        continue;
                    }
                };
                
                let mut matched = false;
                for sup in supported_vers.split(',') {
                    if let Ok(ver) = semver::Version::parse(sup) {
                        if req_parsed.matches(&ver) {
                            matched = true;
                            break;
                        }
                    }
                }
                
                if !matched {
                    incompatible.push(format!("{} (requested: {}, supported: {})", cap, req_ver, supported_vers));
                }
            } else {
                missing.push(cap.clone());
            }
        }

        if !missing.is_empty() || !incompatible.is_empty() {
            let mut msgs = vec![];
            if !missing.is_empty() {
                msgs.push(format!("Missing capabilities: {:?}", missing));
            }
            if !incompatible.is_empty() {
                msgs.push(format!("Incompatible versions: {:?}", incompatible));
            }
            return Ok(DomainPluginHandshakeResponse {
                accepted: false,
                server_version: env!("CARGO_PKG_VERSION").to_string(),
                error_message: Some(msgs.join("; ")),
                supported_capabilities: supported,
            });
        }
        
        Ok(DomainPluginHandshakeResponse {
            accepted: true,
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            error_message: None,
            supported_capabilities: supported,
        })
    }
}
