use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct SearchOrchestrator {
    search_cache: Arc<RwLock<HashMap<String, (u64, u64, Arc<cowen_search::loader::FallbackProvider>)>>>,
}

impl SearchOrchestrator {
    pub fn new() -> Self {
        Self {
            search_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn search_if_needed(
        &self,
        profile: &str,
        ops: Vec<serde_json::Value>,
        query_opt: &Option<String>,
    ) -> (Vec<serde_json::Value>, Option<String>) {
        let query = match query_opt.as_ref().filter(|q| !q.is_empty()) {
            Some(q) => q,
            None => return (ops, None),
        };

        let mut docs = Vec::new();
        for op in &ops {
            docs.push(cowen_search::SearchDocument {
                id: op["id"].as_str().unwrap_or("").to_string(),
                summary: op["summary"].as_str().unwrap_or("").to_string(),
                description: op["description"].as_str().unwrap_or("").to_string(),
                vector: vec![],
            });
        }

        let docs_hash = Self::hash_docs(&docs);
        let plugins_hash = Self::get_plugins_hash();

        let provider = {
            let cache = self.search_cache.read().await;
            if let Some((cached_hash, cached_phash, provider)) = cache.get(profile) {
                if *cached_hash == docs_hash && *cached_phash == plugins_hash {
                    Some(provider.clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        let provider = if let Some(p) = provider {
            p
        } else {
            let p = Arc::new(cowen_search::loader::SearchProviderFactory::create("default_tenant"));

            use cowen_search::SearchProvider;
            p.update_index(&docs);

            let mut cache = self.search_cache.write().await;
            cache.insert(profile.to_string(), (docs_hash, plugins_hash, p.clone()));
            p
        };

        use cowen_search::SearchProvider;
        let results = provider.search(query, 100);

        let mut new_ops = Vec::new();
        for (_score, doc) in results {
            if let Some(op) = ops.iter().find(|o| o["id"].as_str().unwrap_or("") == doc.id) {
                let mut new_op = op.clone();
                if let Some(obj) = new_op.as_object_mut() {
                    // Inject the score (rounded to 4 decimal places for readability)
                    let rounded_score = (_score * 10000.0).round() / 10000.0;
                    obj.insert("score".to_string(), serde_json::json!(rounded_score));
                }
                new_ops.push(new_op);
            }
        }

        let mut used_plugin_name = None;
        let name = provider.name();
        if name != "fallback_search" && name != "string_match" {
            used_plugin_name = Some(name.to_string());
        }

        (new_ops, used_plugin_name)
    }

    fn hash_docs(docs: &[cowen_search::SearchDocument]) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        for d in docs {
            d.id.hash(&mut hasher);
            d.summary.hash(&mut hasher);
            d.description.hash(&mut hasher);
        }
        hasher.finish()
    }

    fn get_plugins_hash() -> u64 {
        let app_yaml_path = cowen_common::config::get_app_dir().join("app.yaml");
        let mut plugins = String::new();
        if let Ok(content) = std::fs::read_to_string(&app_yaml_path) {
            if let Ok(val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                if let Some(arr) = val.get("plugins").and_then(|v| v.as_sequence()) {
                    plugins = format!("{:?}", arr);
                }
            }
        }
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        plugins.hash(&mut hasher);
        hasher.finish()
    }
}
