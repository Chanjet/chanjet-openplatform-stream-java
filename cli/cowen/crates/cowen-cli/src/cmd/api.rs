use cowen_common::grpc::client::DaemonResponse;
use anyhow::Result;

pub async fn list(profile: &str, search: &Option<String>, page: usize, page_size: usize, format: &str, refresh: bool) -> Result<()> {
    let ipc = cowen_common::grpc::client::DaemonClient::new(cowen_common::config::get_ipc_port_path());
    match ipc.api_list(profile, search.as_deref(), page as u32, page_size as u32, refresh).await {
        Ok(DaemonResponse::ApiListData { total, json, plugin_used }) => {
            if let Some(p) = plugin_used {
                println!("🔌 Using search plugin: {}", p);
                println!("  [Rust Standalone Sidecar Mode] (Out-of-process isolation)");
                println!("🧠 Initializing AI vector index for {} APIs...", total);
            }
            if format == "json" || format == "yaml" {
                let val: serde_json::Value = serde_json::from_str(&json)?;
                cowen_common::utils::render(&val, format).map_err(|e| anyhow::anyhow!(e))?;
                return Ok(());
            }

            let paged_ops: Vec<serde_json::Value> = serde_json::from_str(&json)?;

            if let Some(query) = search {
                println!("🔍 Searching for: '{}'", query);
                println!("--------------------------------------------------");
                if paged_ops.is_empty() {
                    println!("  (No results found)");
                } else {
                    for doc in paged_ops {
                        let id = doc["id"].as_str().unwrap_or("");
                        let summary = doc["summary"].as_str().unwrap_or("");
                        println!("\x1b[1;32m{:<30}\x1b[0m", id);
                        println!("  Summary: {}", summary);
                        println!();
                    }
                }
                return Ok(());
            }

            println!("\n🌐 Available APIs for profile: \x1b[1;32m{}\x1b[0m (Page {}, Total {})", profile, page, total);
            println!("--------------------------------------------------");
            if paged_ops.is_empty() {
                println!("  (No APIs found or page out of range)");
            } else {
                for op in paged_ops {
                    let method = op["method"].as_str().unwrap_or("");
                    let path = op["path"].as_str().unwrap_or("");
                    let summary = op["summary"].as_str().unwrap_or("");
                    println!("\x1b[1;32m{:<8}\x1b[0m {:<45} {}", method, path, summary);
                }
            }
            
            let start = (page.max(1) - 1) * page_size;
            let end = start + page_size;
            if end < total {
                println!("\n📑 Next page available. Use '--page {}' to view more.", page + 1);
            }
            println!("\n💡 Use 'cowen api list --search <QUERY>' for semantic search.");
            println!("💡 Use 'cowen api spec <METHOD> <PATH>' to view detailed documentation.");
        }
        Ok(DaemonResponse::Error { message, .. }) => eprintln!("❌ API List failed: {}", message),
        Err(e) => eprintln!("❌ IPC Error: {}", e),
        _ => eprintln!("❌ Unexpected response"),
    }
    Ok(())
}

pub async fn spec(profile: &str, method: &String, path: &String, raw: bool) -> Result<()> {
    let ipc = cowen_common::grpc::client::DaemonClient::new(cowen_common::config::get_ipc_port_path());
    match ipc.api_spec(profile, method, path).await {
        Ok(DaemonResponse::ApiSpecData { json }) => {
            eprintln!("DEBUG JSON: {}", json);
            if raw {
                println!("{}", json);
            } else {
                // Here we just print the pre-rendered string that the daemon gave us,
                // or we could parse it and print it.
                // For simplicity, let's assume the daemon just returns the formatted text in 'json' if not raw,
                // or we parse and format it here if we want to keep the CLI logic.
                // Wait, the daemon will return the raw JSON of the Operation, and we format it here to avoid sending
                // huge formatted strings.
                let op: serde_json::Value = serde_json::from_str(&json)?;
                println!("\n📖 API Specification Details");
                println!("--------------------------------------------------");
                println!("📍 Endpoint:    \x1b[1;32m{} {}\x1b[0m", method.to_uppercase(), path);
                println!("📌 Summary:     {}", op["summary"].as_str().unwrap_or("N/A"));
                println!("📝 Description: {}", op["description"].as_str().unwrap_or("N/A"));
                
                if let Some(params) = op["parameters"].as_array() {
                    println!("\n🎛️  Parameters:");
                    for p in params {
                        let name = p["name"].as_str().unwrap_or("");
                        let in_ = p["in"].as_str().unwrap_or("");
                        let req = if p["required"].as_bool().unwrap_or(false) { "*" } else { "" };
                        println!("  - {}{} ({:<8})", name, req, in_);
                    }
                }
                
                if let Some(req_body) = op["requestBody"].as_object() {
                    println!("\n📥 Request Body:");
                    if let Some(content) = req_body["content"].as_object() {
                        if let Some(app_json) = content["application/json"].as_object() {
                            if let Some(schema) = app_json["schema"].as_object() {
                                println!("  Schema:");
                                if let Some(props) = schema["properties"].as_object() {
                                    for (k, _v) in props {
                                        println!("    - {}: <string>", k);
                                    }
                                }
                            }
                        }
                    }
                }
                
                println!("\n💡 Usage Example:");
                if method.to_uppercase() == "POST" {
                    println!("cowen api {} \"{}\" -d '{{\"name\":\"John Doe\"}}'", method.to_uppercase(), path);
                } else {
                    let p = path.replace("{id}", "<id>");
                    println!("cowen api {} \"{}\"", method.to_uppercase(), p);
                }
            }
        }
        Ok(DaemonResponse::Error { message, .. }) => eprintln!("❌ API Spec failed: {}", message),
        Err(e) => eprintln!("❌ IPC Error: {}", e),
        _ => eprintln!("❌ Unexpected response"),
    }
    Ok(())
}
