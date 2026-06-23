#![allow(dead_code, unused_imports, unused_variables)] // TODO: placeholder, implement properly later
                                                       // Audit specific capability
use cowen_auth::client::Client;
use cowen_common::daemon::DaemonService;
use cowen_common::grpc::proto::*;
use cowen_common::vault::Vault;
use cowen_common::CowenError;
use cowen_config::ConfigManager;
use cowen_macros::{rbac, rbac_controller};
use std::sync::Arc;
use tracing::info;

#[tonic::async_trait]
pub trait NativeAuditCapability: Send + Sync {
    async fn tail_audit(
        &self,
        claims: Option<&cowen_common::jwt::IpcClaims>,
        req: TailAuditRequest,
    ) -> Result<TailAuditResponse, CowenError>;
}

pub struct DefaultAuditCapability {
    service: Arc<dyn DaemonService>,
    vault: Arc<dyn Vault>,
    cfg_mgr: ConfigManager,
}

impl DefaultAuditCapability {
    pub fn new(
        service: Arc<dyn DaemonService>,
        vault: Arc<dyn Vault>,
        cfg_mgr: ConfigManager,
    ) -> Self {
        Self {
            service,
            vault,
            cfg_mgr,
        }
    }
}

#[rbac_controller(domain = "native.audit")]
#[tonic::async_trait]
impl NativeAuditCapability for DefaultAuditCapability {
    #[rbac]
    // tail_audit has no rbac
    async fn tail_audit(
        &self,
        _claims: Option<&cowen_common::jwt::IpcClaims>,
        req: TailAuditRequest,
    ) -> Result<TailAuditResponse, CowenError> {
        match self
            .vault
            .list_audit(&req.profile, req.lines as usize)
            .await
        {
            Ok(entries) => {
                let mut content = String::new();
                for entry in entries.iter().rev() {
                    content.push_str(&format!("[{}] {}\\n", entry.timestamp, entry.message));
                }
                Ok(TailAuditResponse {
                    content,
                    error_message: None,
                })
            }
            Err(e) => Ok(TailAuditResponse {
                content: "".to_string(),
                error_message: Some(e.to_string()),
            }),
        }
    }
}
