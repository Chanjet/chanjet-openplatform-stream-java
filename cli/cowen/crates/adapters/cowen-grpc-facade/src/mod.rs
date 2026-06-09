pub mod controller;
pub mod api_registry;
pub mod openapi_parser;

pub fn registry_supported_versions() -> std::collections::HashMap<&'static str, Vec<&'static str>> {
    let mut map = std::collections::HashMap::new();
    // The gRPC contract currently explicitly implements these versions:
    map.insert("native.api.registry", vec!["1.0.0"]);
    map.insert("native.system", vec!["1.0.0"]);
    map.insert("native.dlq", vec!["1.0.0"]);
    map.insert("native.search", vec!["1.0.0"]);
    
    // System capabilities mapped via gRPC endpoints
    map.insert("sys.vault", vec!["1.0.0"]);
    map.insert("sys.http", vec!["1.0.0"]);
    map.insert("sys.base", vec!["1.0.0"]);
    
    map
}
