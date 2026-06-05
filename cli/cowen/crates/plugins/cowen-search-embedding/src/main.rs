use std::collections::HashMap;
use std::sync::Mutex;
use serde::Deserialize;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use cowen_ai::{ONNXEmbedder, SearchIndex, SearchDocument as AiDocument};
use cowen_search::SearchDocument;
use cowen_plugin::{JsonRpcRequest, JsonRpcResponse, JsonRpcError};


#[derive(Deserialize)]
struct UpdateIndexParams {
    tenant_id: String,
    documents: Vec<SearchDocument>,
}

#[derive(Deserialize)]
struct QueryParams {
    tenant_id: String,
    query: String,
    top: usize,
}

struct SidecarEngine {
    embedder: ONNXEmbedder,
    // Multitenancy: Scoped in-memory database partitions
    indexes: HashMap<String, SearchIndex>,
}

static ENGINE: Mutex<Option<SidecarEngine>> = Mutex::new(None);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app_dir = cowen_common::config::get_app_dir();
    let default_model = app_dir.join("search").join("models").join("model_quantized.onnx");
    let default_tokenizer = app_dir.join("search").join("models").join("tokenizer.json");

    // Proactively download/verify ONNX assets
    let _ = cowen_ai::SearchIndex::ensure_assets(&app_dir);

    // Warm up the ONNX inference model
    if let Ok(embedder) = ONNXEmbedder::new(&default_model.to_string_lossy(), &default_tokenizer.to_string_lossy()) {
        let engine = SidecarEngine {
            embedder,
            indexes: HashMap::new(),
        };
        *ENGINE.lock().unwrap() = Some(engine);
    } else {
        eprintln!("⚠️ Failed to initialize ONNX inference embedder.");
    }

    let mut reader = BufReader::new(tokio::io::stdin());
    let mut writer = tokio::io::stdout();
    let mut line = String::new();

    // Stdio Event loop processing standard JSON-RPC frames line by line
    while reader.read_line(&mut line).await? > 0 {
        let response = process_line(&line);
        let resp_json = serde_json::to_string(&response)?;
        writer.write_all(resp_json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
        line.clear();
    }

    Ok(())
}

fn process_line(line: &str) -> JsonRpcResponse {
    let req: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: None,
                result: None,
                error: Some(JsonRpcError {
                    code: -32700,
                    message: format!("Parse error: {}", e),
                }),
            };
        }
    };

    if req.jsonrpc != "2.0" {
        return JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: None,
            error: Some(JsonRpcError {
                code: -32600,
                message: "Invalid Request: missing or wrong jsonrpc version".to_string(),
            }),
        };
    }

    let mut engine_guard = ENGINE.lock().unwrap();
    let engine = match engine_guard.as_mut() {
        Some(e) => e,
        None => {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32603,
                    message: "Internal error: ONNX embedder not loaded".to_string(),
                }),
            };
        }
    };

    match req.method.as_str() {
        "search/update_index" => {
            let params_val = match req.params {
                Some(p) => p,
                None => return missing_params_error(req.id),
            };

            let params: UpdateIndexParams = match serde_json::from_value(params_val) {
                Ok(p) => p,
                Err(e) => return invalid_params_error(req.id, e),
            };

            let cache_dir = cowen_common::config::get_app_dir().join("search").join("cache");
            let cache_file = cache_dir.join(format!("{}.json", &params.tenant_id));

            // Load existing disk cache to reuse unmodified vector embeddings
            let mut cached_map = std::collections::HashMap::new();
            if cache_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&cache_file) {
                    if let Ok(cached_index) = serde_json::from_str::<SearchIndex>(&content) {
                        for doc in cached_index.docs {
                            cached_map.insert(doc.id.clone(), doc);
                        }
                    }
                }
            }

            let mut index = SearchIndex::default();
            for doc in params.documents {
                let mut vector = None;
                if let Some(cached_doc) = cached_map.get(&doc.id) {
                    if cached_doc.summary == doc.summary && cached_doc.description == doc.description && !cached_doc.vector.is_empty() {
                        vector = Some(cached_doc.vector.clone());
                    }
                }

                if vector.is_none() {
                    let text = format!("{} {}", doc.summary, doc.description);
                    if let Ok(v) = engine.embedder.embed(&text) {
                        vector = Some(v);
                    }
                }

                if let Some(v) = vector {
                    index.push(AiDocument {
                        id: doc.id,
                        summary: doc.summary,
                        description: doc.description,
                        vector: v,
                    });
                }
            }

            // Update memory index
            engine.indexes.insert(params.tenant_id.clone(), index.clone());

            // Write back to disk cache
            let _ = std::fs::create_dir_all(&cache_dir);
            if let Ok(serialized) = serde_json::to_string(&index) {
                let _ = std::fs::write(&cache_file, serialized);
            }

            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(json!({ "status": "success" })),
                error: None,
            }
        }
        "search/query" => {
            let params_val = match req.params {
                Some(p) => p,
                None => return missing_params_error(req.id),
            };

            let params: QueryParams = match serde_json::from_value(params_val) {
                Ok(p) => p,
                Err(e) => return invalid_params_error(req.id, e),
            };

            // Lazily pre-load index from disk cache if not in memory
            let has_index = if engine.indexes.contains_key(&params.tenant_id) {
                true
            } else {
                let cache_dir = cowen_common::config::get_app_dir().join("search").join("cache");
                let cache_file = cache_dir.join(format!("{}.json", &params.tenant_id));
                if cache_file.exists() {
                    if let Ok(content) = std::fs::read_to_string(&cache_file) {
                        if let Ok(cached_index) = serde_json::from_str::<SearchIndex>(&content) {
                            engine.indexes.insert(params.tenant_id.clone(), cached_index);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            };

            let results = if has_index {
                let index = engine.indexes.get(&params.tenant_id).unwrap();
                if let Ok(query_vector) = engine.embedder.embed(&params.query) {
                    let raw_results = index.search(&query_vector, &params.query, params.top);
                    raw_results.into_iter().map(|(score, ai_doc)| {
                        (score, SearchDocument {
                            id: ai_doc.id.clone(),
                            summary: ai_doc.summary.clone(),
                            description: ai_doc.description.clone(),
                            vector: ai_doc.vector.clone(),
                        })
                    }).collect::<Vec<_>>()
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(serde_json::to_value(results).unwrap_or(serde_json::Value::Null)),
                error: None,
            }
        }
        _ => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: req.id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", req.method),
            }),
        },
    }
}

fn missing_params_error(id: Option<serde_json::Value>) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: None,
        error: Some(JsonRpcError {
            code: -32602,
            message: "Invalid params: missing params object".to_string(),
        }),
    }
}

fn invalid_params_error(id: Option<serde_json::Value>, err: serde_json::Error) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: None,
        error: Some(JsonRpcError {
            code: -32602,
            message: format!("Invalid params: {}", err),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn setup_test_engine(temp_dir: &std::path::Path) {
        unsafe {
            std::env::set_var("COWEN_HOME", temp_dir.to_str().unwrap());
        }
        let app_dir = cowen_common::config::get_app_dir();
        
        // Extract embedded ONNX models to sandbox
        let _ = cowen_ai::SearchIndex::ensure_assets(&app_dir);
        let default_model = app_dir.join("search").join("models").join("model_quantized.onnx");
        let default_tokenizer = app_dir.join("search").join("models").join("tokenizer.json");

        match ONNXEmbedder::new(&default_model.to_string_lossy(), &default_tokenizer.to_string_lossy()) {
            Ok(embedder) => {
                let engine = SidecarEngine {
                    embedder,
                    indexes: HashMap::new(),
                };
                *ENGINE.lock().unwrap() = Some(engine);
            }
            Err(e) => {
                panic!("Failed to initialize ONNXEmbedder in test setup: {:?}", e);
            }
        }
    }

    #[test]
    fn test_disk_cache_reuse_and_invalidation() {
        let tmp = tempdir().unwrap();
        setup_test_engine(tmp.path());

        let tenant_id = "test_tenant_123";
        
        let update_req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "search/update_index",
            "params": {
                "tenant_id": tenant_id,
                "documents": [
                    {
                        "id": "GET /v1/test",
                        "summary": "测试接口",
                        "description": "用于测试缓存的接口",
                        "vector": []
                    }
                ]
            }
        });

        let resp1 = process_line(&serde_json::to_string(&update_req).unwrap());
        if let Some(ref e) = resp1.error {
            panic!("resp1 error: code={}, message={}", e.code, e.message);
        }
        assert!(resp1.error.is_none());

        // Verify cache file created on disk
        let cache_file = tmp.path().join("search").join("cache").join(format!("{}.json", tenant_id));
        assert!(cache_file.exists(), "Cache file should be created on disk");

        // Read vector from disk cache and inject a recognizable custom vector to prove cache hit reuse
        let cache_content = fs::read_to_string(&cache_file).unwrap();
        let mut index: SearchIndex = serde_json::from_str(&cache_content).unwrap();
        assert_eq!(index.docs.len(), 1);
        
        // Inject mock vector
        let injected_vector = vec![42.0f32, 99.0f32];
        index.docs[0].vector = injected_vector.clone();
        fs::write(&cache_file, serde_json::to_string(&index).unwrap()).unwrap();

        // Clear in-memory indexes to force loading from disk cache
        if let Some(ref mut engine) = *ENGINE.lock().unwrap() {
            engine.indexes.clear();
        }

        // 2. Second update with identical content (Cache Hit, should load from cache and reuse our injected vector)
        let resp2 = process_line(&serde_json::to_string(&update_req).unwrap());
        assert!(resp2.error.is_none());

        // Verify the injected vector was reused
        let engine_guard = ENGINE.lock().unwrap();
        let engine = engine_guard.as_ref().unwrap();
        let stored_index = engine.indexes.get(tenant_id).unwrap();
        assert_eq!(stored_index.docs[0].vector, injected_vector, "Should reuse cached vector without embedding again");
        drop(engine_guard);

        // 3. Third update with modified content (Cache Miss / Invalidation, should re-embed and overwrite injected vector)
        let modified_req = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "search/update_index",
            "params": {
                "tenant_id": tenant_id,
                "documents": [
                    {
                        "id": "GET /v1/test",
                        "summary": "修改后的测试接口",
                        "description": "用于测试缓存的接口",
                        "vector": []
                    }
                ]
            }
        });

        let resp3 = process_line(&serde_json::to_string(&modified_req).unwrap());
        assert!(resp3.error.is_none());

        // Verify the vector is regenerated (not matching the injected one anymore)
        let engine_guard = ENGINE.lock().unwrap();
        let engine = engine_guard.as_ref().unwrap();
        let stored_index = engine.indexes.get(tenant_id).unwrap();
        assert_ne!(stored_index.docs[0].vector, injected_vector, "Should regenerate vector because content changed");
    }
}

