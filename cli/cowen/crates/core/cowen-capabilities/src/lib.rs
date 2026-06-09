pub mod capabilities;
pub mod internal;

// Re-export core traits
pub use capabilities::protected::native_api_registry;
pub use internal::native_search;
pub use capabilities::protected::native_system;
pub use capabilities::protected::native_dlq;
pub use capabilities::protected::native_daemon;
pub use capabilities::protected::sys_http;
pub use capabilities::protected::sys_vault;
pub use capabilities::public::public_system;

// Expose rbac for the macro to resolve `crate::rbac`
pub use internal::rbac;


use std::sync::Arc;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;
use cowen_common::daemon::DaemonService;

pub struct CapabilityRegistry {
    pub sys_vault: Arc<dyn capabilities::protected::sys_vault::SysVaultCapability>,
    pub sys_http: Arc<dyn capabilities::protected::sys_http::SysHttpCapability>,
    pub native_api_registry: Arc<dyn capabilities::protected::native_api_registry::NativeApiRegistryCapability>,
    pub native_system: Arc<dyn capabilities::protected::native_system::NativeSystemCapability<TunnelPluginStream = std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<cowen_common::grpc::proto::TunnelPluginResponse, cowen_common::CowenError>> + Send + 'static>>>>,
    pub native_dlq: Arc<dyn capabilities::protected::native_dlq::NativeDlqCapability>,
    pub native_search: Arc<dyn internal::native_search::NativeSearchCapability>,
    pub native_daemon: Arc<dyn capabilities::protected::native_daemon::NativeDaemonCapability>,
    pub public_system: Arc<dyn capabilities::public::public_system::PublicSystemCapability>,
}

impl CapabilityRegistry {
    pub fn new(service: Arc<dyn DaemonService>, vault: Arc<dyn Vault>, cfg_mgr: ConfigManager, ipc_port: u16, supported_capabilities: std::collections::HashMap<String, String>) -> Self {
        let native_search = Arc::new(internal::native_search::DefaultSearch::new());
        let native_dlq = Arc::new(capabilities::protected::native_dlq::DefaultDlq::new(vault.clone(), cfg_mgr.clone()));
        
        Self {
            native_api_registry: Arc::new(capabilities::protected::native_api_registry::DefaultApiRegistry::new(vault.clone(), cfg_mgr.clone(), native_search.clone())),
            native_system: Arc::new(capabilities::protected::native_system::DefaultSystem::new(vault.clone(), cfg_mgr.clone(), ipc_port)),
            native_dlq: native_dlq.clone(),
            native_search: native_search.clone(),
            sys_vault: Arc::new(capabilities::protected::sys_vault::DefaultSysVault::new(vault.clone(), cfg_mgr.clone())),
            sys_http: Arc::new(capabilities::protected::sys_http::DefaultSysHttp::new(vault.clone())),
            native_daemon: Arc::new(capabilities::protected::native_daemon::DefaultDaemonCapability::new(service, vault.clone(), cfg_mgr.clone())),
            public_system: Arc::new(capabilities::public::public_system::DefaultPublicSystem::new(supported_capabilities)),
        }
    }
}
