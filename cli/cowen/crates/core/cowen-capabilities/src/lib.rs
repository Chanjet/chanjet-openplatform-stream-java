pub mod rbac;
pub mod sys_http;
pub mod sys_vault;
pub mod native_search;
pub mod native_api_registry;
pub mod native_system;
pub mod native_dlq;
pub mod native_daemon;
pub mod openapi_parser;
pub mod forwarder;
pub mod dlq;

// Re-export core traits
pub use native_api_registry::NativeApiRegistryCapability;
pub use native_search::NativeSearchCapability;
pub use native_system::NativeSystemCapability;
pub use native_dlq::NativeDlqCapability;
pub use native_daemon::NativeDaemonCapability;
pub use sys_http::SysHttpCapability;
pub use sys_vault::SysVaultCapability;


use std::sync::Arc;
use cowen_common::vault::Vault;
use cowen_config::ConfigManager;
use cowen_common::daemon::DaemonService;

pub struct CapabilityRegistry {
    pub sys_vault: Arc<dyn sys_vault::SysVaultCapability>,
    pub sys_http: Arc<dyn sys_http::SysHttpCapability>,
    pub native_api_registry: Arc<dyn native_api_registry::NativeApiRegistryCapability>,
    pub native_system: Arc<dyn native_system::NativeSystemCapability<TunnelPluginStream = std::pin::Pin<Box<dyn tokio_stream::Stream<Item = Result<cowen_common::grpc::proto::TunnelPluginResponse, cowen_common::CowenError>> + Send + 'static>>>>,
    pub native_dlq: Arc<dyn native_dlq::NativeDlqCapability>,
    pub native_search: Arc<dyn native_search::NativeSearchCapability>,
    pub native_daemon: Arc<dyn native_daemon::NativeDaemonCapability>,
}

impl CapabilityRegistry {
    pub fn new(service: Arc<dyn DaemonService>, vault: Arc<dyn Vault>, cfg_mgr: ConfigManager, ipc_port: u16, supported_capabilities: std::collections::HashMap<String, String>) -> Self {
        let native_search = Arc::new(native_search::DefaultSearch::new());
        let native_dlq = Arc::new(native_dlq::DefaultDlq::new(vault.clone(), cfg_mgr.clone()));
        
        Self {
            native_api_registry: Arc::new(native_api_registry::DefaultApiRegistry::new(vault.clone(), cfg_mgr.clone(), native_search.clone())),
            native_system: Arc::new(native_system::DefaultSystem::new(vault.clone(), cfg_mgr.clone(), ipc_port, supported_capabilities)),
            native_dlq: native_dlq.clone(),
            native_search: native_search.clone(),
            sys_vault: Arc::new(sys_vault::DefaultSysVault::new(vault.clone(), cfg_mgr.clone())),
            sys_http: Arc::new(sys_http::DefaultSysHttp::new(vault.clone())),
            native_daemon: Arc::new(native_daemon::DefaultDaemonCapability::new(service, vault.clone(), cfg_mgr.clone())),
        }
    }
}
