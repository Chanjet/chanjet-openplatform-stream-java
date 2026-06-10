#[macro_use]
pub mod macros;

pub mod native_audit;
pub mod native_auth;
pub mod native_config;
pub mod native_dlq;
pub mod native_system;
pub mod native_worker;
pub mod public_system;
pub mod api_registry;

pub use cowen_capabilities::rbac;

// The facade handles Tonic Routers and bindings.
pub fn registry_supported_versions() -> std::collections::HashMap<&'static str, Vec<&'static str>> {
    let mut map = std::collections::HashMap::new();
    map.insert("native.auth", vec!["1.0.0"]);
    map.insert("native.config", vec!["1.0.0"]);
    map.insert("sys.base", vec!["1.0.0"]);
    map.insert("native.system", vec!["1.0.0"]);
    map.insert("native.search", vec!["1.0.0"]);
    map.insert("native.api.registry", vec!["1.0.0"]);
    map.insert("native.dlq", vec!["1.0.0"]);
    map
}
