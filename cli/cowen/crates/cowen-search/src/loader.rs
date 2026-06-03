use crate::{SearchDocument, SearchProvider};
use tracing::warn;

pub struct FallbackProvider {
    pub primary: Option<Box<dyn SearchProvider>>,
    pub fallback: Box<dyn SearchProvider>,
}

impl SearchProvider for FallbackProvider {
    fn name(&self) -> &str {
        "fallback_search"
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
