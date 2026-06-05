use crate::{SearchDocument, SearchProvider};
use tracing::warn;

pub struct FallbackProvider {
    pub primary: Option<Box<dyn SearchProvider>>,
    pub fallback: Box<dyn SearchProvider>,
}

impl SearchProvider for FallbackProvider {
    fn name(&self) -> &str {
        if let Some(ref primary) = self.primary {
            primary.name()
        } else {
            self.fallback.name()
        }
    }

    fn search(&self, query: &str, top: usize) -> Vec<(f32, SearchDocument)> {
        if let Some(ref primary) = self.primary {
            let res = primary.search(query, top);
            if !res.is_empty() {
                return res;
            }
            warn!("Primary search provider returned no results, falling back.");
        }
        self.fallback.search(query, top)
    }

    fn update_index(&self, docs: &[SearchDocument]) {
        if let Some(ref primary) = self.primary {
            primary.update_index(docs);
        }
        self.fallback.update_index(docs);
    }
}

/// Standalone Sidecar process based Search Provider (Phase 2 - 100% Rust)
pub struct SidecarSearchProvider {
    name: String,
    client: cowen_sys::plugin::RpcPluginClient,
}

impl SidecarSearchProvider {
    pub fn new(name: &str, binary_path: std::path::PathBuf, tenant_id: String, ipc_token: Option<String>) -> Self {
        Self {
            name: name.to_string(),
            client: cowen_sys::plugin::RpcPluginClient::new(binary_path, tenant_id, ipc_token),
        }
    }
}

impl SearchProvider for SidecarSearchProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn search(&self, query: &str, top: usize) -> Vec<(f32, SearchDocument)> {
        let params = serde_json::json!({
            "tenant_id": self.client.tenant_id(),
            "query": query,
            "top": top,
        });

        match self.client.call_tool("search/query", params) {
            Ok(value) => serde_json::from_value(value).unwrap_or_default(),
            Err(e) => {
                tracing::error!(target: "sys", "Sidecar search error: {}", e);
                vec![]
            }
        }
    }

    fn update_index(&self, docs: &[SearchDocument]) {
        let params = serde_json::json!({
            "tenant_id": self.client.tenant_id(),
            "documents": docs,
        });

        if let Err(e) = self.client.call_tool("search/update_index", params) {
            tracing::error!(target: "sys", "Sidecar update_index error: {}", e);
        }
    }
}

pub struct SearchProviderFactory;

impl SearchProviderFactory {
    pub fn create(tenant_id: &str) -> FallbackProvider {
        let app_yaml_path = cowen_infra::path::get_app_dir().join("app.yaml");
        let mut enabled_plugins: Vec<String> = vec![];
        if let Ok(content) = std::fs::read_to_string(&app_yaml_path) {
            if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                if let Some(plugins) = val.get("plugins").and_then(|v| v.as_sequence()) {
                    enabled_plugins = plugins.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
                }
            }
        }
        
        let plugins_dir = cowen_infra::path::get_app_dir().join("plugins");
        let mut search_plugin_name = None;
        for p in &enabled_plugins {
            let bundle_path = plugins_dir.join(p).with_extension("bundle");
            if let Ok(bundle_str) = std::fs::read_to_string(&bundle_path) {
                if let Ok(bundle) = serde_json::from_str::<serde_json::Value>(&bundle_str) {
                    if let Some(capabilities) = bundle.get("manifest").and_then(|m| m.get("capabilities")).and_then(|c| c.as_array()) {
                        let has_search = capabilities.iter().any(|c| c.as_str() == Some("SearchProvider"));
                        if has_search {
                            search_plugin_name = Some(p.clone());
                            break;
                        }
                    }
                }
            }
        }
        
        let mut primary: Option<Box<dyn SearchProvider>> = None;
        if let Some(p_name) = search_plugin_name {
            let expected_path = if cfg!(target_os = "windows") && !p_name.ends_with(".exe") {
                plugins_dir.join(format!("{}.exe", p_name))
            } else {
                plugins_dir.join(&p_name)
            };
            
            if expected_path.exists() {
                primary = Some(Box::new(SidecarSearchProvider::new(
                    &p_name,
                    expected_path,
                    tenant_id.to_string(),
                    None,
                )));
            }
        }

        FallbackProvider {
            primary,
            fallback: Box::new(crate::StringMatchProvider { docs: std::sync::RwLock::new(vec![]) }),
        }
    }
}
