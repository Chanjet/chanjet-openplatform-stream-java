pub mod capabilities;
pub mod internal;

// Re-export core traits
pub use capabilities::protected::native_api_registry;
pub use capabilities::protected::native_audit;
pub use capabilities::protected::native_auth;
pub use capabilities::protected::native_config;
pub use capabilities::protected::native_dlq;
pub use capabilities::protected::native_system;
pub use capabilities::protected::native_worker;
pub use capabilities::public::public_system;
pub use internal::native_search;

// Expose rbac for the macro to resolve `crate::rbac`
pub use internal::rbac;

use cowen_common::daemon::DaemonService;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;
use std::sync::Arc;

pub type TunnelPluginStreamAlias = std::pin::Pin<
    Box<
        dyn tokio_stream::Stream<
                Item = Result<
                    cowen_common::grpc::proto::TunnelPluginResponse,
                    cowen_common::CowenError,
                >,
            > + Send
            + 'static,
    >,
>;

pub struct CapabilityRegistry {
    pub native_api_registry:
        Arc<dyn capabilities::protected::native_api_registry::NativeApiRegistryCapability>,
    pub native_system: Arc<
        dyn capabilities::protected::native_system::NativeSystemCapability<
            TunnelPluginStream = TunnelPluginStreamAlias,
        >,
    >,
    pub native_dlq: Arc<dyn capabilities::protected::native_dlq::NativeDlqCapability>,
    pub native_search: Arc<dyn internal::native_search::NativeSearchCapability>,
    pub native_worker: Arc<dyn capabilities::protected::native_worker::NativeWorkerCapability>,
    pub native_auth: Arc<dyn capabilities::protected::native_auth::NativeAuthCapability>,
    pub native_config: Arc<dyn capabilities::protected::native_config::NativeConfigCapability>,
    pub native_audit: Arc<dyn capabilities::protected::native_audit::NativeAuditCapability>,
    pub public_system: Arc<dyn capabilities::public::public_system::PublicSystemCapability>,
}

impl CapabilityRegistry {
    pub fn new(
        service: Arc<dyn DaemonService>,
        vault: Arc<dyn Vault>,
        cfg_mgr: ConfigManager,
        ipc_port: u16,
        supported_capabilities: std::collections::HashMap<String, String>,
    ) -> Self {
        let native_search = Arc::new(internal::native_search::DefaultSearch::new());
        let native_dlq = Arc::new(capabilities::protected::native_dlq::DefaultDlq::new(
            vault.clone(),
            cfg_mgr.clone(),
        ));

        Self {
            native_api_registry: Arc::new(
                capabilities::protected::native_api_registry::DefaultApiRegistry::new(
                    vault.clone(),
                    cfg_mgr.clone(),
                    native_search.clone(),
                ),
            ),
            native_system: Arc::new(capabilities::protected::native_system::DefaultSystem::new(
                vault.clone(),
                cfg_mgr.clone(),
                ipc_port,
            )),
            native_dlq: native_dlq.clone(),
            native_search: native_search.clone(),
            native_worker: Arc::new(
                capabilities::protected::native_worker::DefaultWorkerCapability::new(
                    service.clone(),
                    vault.clone(),
                    cfg_mgr.clone(),
                ),
            ),
            native_auth: Arc::new(
                capabilities::protected::native_auth::DefaultAuthCapability::new(
                    service.clone(),
                    vault.clone(),
                    cfg_mgr.clone(),
                ),
            ),
            native_config: Arc::new(
                capabilities::protected::native_config::DefaultConfigCapability::new(
                    service.clone(),
                    vault.clone(),
                    cfg_mgr.clone(),
                ),
            ),
            native_audit: Arc::new(
                capabilities::protected::native_audit::DefaultAuditCapability::new(
                    service.clone(),
                    vault.clone(),
                    cfg_mgr.clone(),
                ),
            ),
            public_system: Arc::new(
                capabilities::public::public_system::DefaultPublicSystem::new(
                    supported_capabilities,
                ),
            ),
        }
    }
}
