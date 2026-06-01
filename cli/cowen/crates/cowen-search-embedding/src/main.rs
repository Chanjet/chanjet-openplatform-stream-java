use std::collections::HashMap;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use cowen_ai::{ONNXEmbedder, SearchIndex, SearchDocument as AiDocument};
use cowen_search::SearchDocument;

#[derive(Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    method: String,
    params: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

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

            // Option A: Metadata Partitioning - select/insert the tenant's partition
            let index = engine.indexes.entry(params.tenant_id).or_default();
            for doc in params.documents {
                let text = format!("{} {}", doc.summary, doc.description);
                if let Ok(vector) = engine.embedder.embed(&text) {
                    index.push(AiDocument {
                        id: doc.id,
                        summary: doc.summary,
                        description: doc.description,
                        vector,
                    });
                }
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

            // Option A: Strictly filter by query.tenant_id
            let results = if let Some(index) = engine.indexes.get(&params.tenant_id) {
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
