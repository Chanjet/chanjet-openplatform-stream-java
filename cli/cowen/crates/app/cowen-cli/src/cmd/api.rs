use anyhow::Result;
use cowen_common::grpc::client::DaemonResponse;

pub async fn list(
    profile: &str,
    search: &Option<String>,
    page: usize,
    page_size: usize,
    format: &str,
    refresh: bool,
) -> Result<()> {
    let ipc = cowen_common::grpc::client::DaemonClient::new(crate::get_ipc_port_path());
    match ipc
        .api_list(
            profile,
            search.as_deref(),
            page as u32,
            page_size as u32,
            refresh,
        )
        .await
    {
        Ok(DaemonResponse::ApiListData {
            total,
            json,
            plugin_used,
        }) => {
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

            println!(
                "\n🌐 Available APIs for profile: \x1b[1;32m{}\x1b[0m (Page {}, Total {})",
                profile, page, total
            );
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
                println!(
                    "\n📑 Next page available. Use '--page {}' to view more.",
                    page + 1
                );
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

fn print_schema_properties(props: &serde_json::Map<String, serde_json::Value>, indent: &str) {
    for (k, v) in props {
        let p_type = v
            .get("type")
            .and_then(|t| t.as_str())
            .or_else(|| {
                v.get("types")
                    .and_then(|ts| ts.as_array())
                    .and_then(|a| a.first())
                    .and_then(|f| f.as_str())
            })
            .unwrap_or("string");
        let p_desc = v["description"].as_str().unwrap_or("");

        let desc_str = if p_desc.is_empty() {
            "".to_string()
        } else {
            format!(" - {}", p_desc)
        };

        println!("{}- {}: <{}>{}", indent, k, p_type, desc_str);

        // If it's an object with nested properties, recurse
        if p_type == "object" {
            let mut nested_props = v.get("properties").and_then(|p| p.as_object());
            if nested_props.is_none() {
                nested_props = v
                    .get("jsonSchema")
                    .and_then(|js| js.get("properties"))
                    .and_then(|p| p.as_object());
            }
            if let Some(np) = nested_props {
                print_schema_properties(np, &format!("{}  ", indent));
            }
        }
    }
}

fn print_spec_parameters(op: &serde_json::Value) {
    if let Some(params) = op["parameters"].as_array() {
        println!("\n🎛️  Parameters:");
        for p in params {
            let name = p["name"].as_str().unwrap_or("");
            let in_ = p["in"].as_str().unwrap_or("");
            let req = if p["required"].as_bool().unwrap_or(false) {
                "*"
            } else {
                ""
            };
            println!("  - {}{} ({:<8})", name, req, in_);
        }
    }
}

fn print_spec_request_body(op: &serde_json::Value) {
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
}

fn print_response_schema(app_json: &serde_json::Map<String, serde_json::Value>) {
    if let Some(schema) = app_json.get("schema").and_then(|s| s.as_object()) {
        println!("    Schema:");
        let is_array = app_json.get("type").and_then(|t| t.as_str()) == Some("array")
            || schema.get("type").and_then(|t| t.as_str()) == Some("array");

        if is_array {
            println!("      Type: array of object");
        }

        let mut properties_opt = schema.get("properties").and_then(|p| p.as_object());
        if properties_opt.is_none() {
            properties_opt = schema
                .get("items")
                .and_then(|i| i.get("properties"))
                .and_then(|p| p.as_object());
        }
        if properties_opt.is_none() {
            properties_opt = schema
                .get("items")
                .and_then(|i| i.get("jsonSchema"))
                .and_then(|js| js.get("properties"))
                .and_then(|p| p.as_object());
        }

        if let Some(props) = properties_opt {
            let base_indent = if is_array { "        " } else { "      " };
            print_schema_properties(props, base_indent);
        }
    }
}

fn print_response_example(app_json: &serde_json::Map<String, serde_json::Value>) {
    if let Some(examples) = app_json.get("examples").and_then(|e| e.as_object()) {
        if let Some(success) = examples.get("success") {
            if let Some(val) = success.get("value") {
                println!("    Example Response:");
                let pretty_val = serde_json::to_string_pretty(val).unwrap_or_default();
                for line in pretty_val.lines() {
                    println!("      {}", line);
                }
            }
        }
    }
}

fn print_spec_responses(op: &serde_json::Value) {
    if let Some(responses) = op["responses"].as_object() {
        println!("\n📤 Responses:");
        for (status_code, resp_val) in responses {
            let desc = resp_val["description"].as_str().unwrap_or("");
            println!("  {} ({}):", status_code, desc);
            if let Some(content) = resp_val["content"].as_object() {
                if let Some(app_json) = content["application/json"].as_object() {
                    print_response_schema(app_json);
                    print_response_example(app_json);
                }
            }
        }
    }
}

pub async fn spec(profile: &str, method: &String, path: &String, raw: bool) -> Result<()> {
    let ipc = cowen_common::grpc::client::DaemonClient::new(crate::get_ipc_port_path());
    match ipc.api_spec(profile, method, path).await {
        Ok(DaemonResponse::ApiSpecData { json }) => {
            tracing::debug!(target: "sys", "DEBUG JSON: {}", json);
            if raw {
                println!("{}", json);
            } else {
                let parsed: serde_json::Value = serde_json::from_str(&json)?;
                let op = parsed.get("operation").unwrap_or(&parsed);
                println!("\n📖 API Specification Details");
                println!("--------------------------------------------------");
                println!(
                    "📍 Endpoint:    \x1b[1;32m{} {}\x1b[0m",
                    method.to_uppercase(),
                    path
                );
                println!(
                    "📌 Summary:     {}",
                    op["summary"].as_str().unwrap_or("N/A")
                );
                println!(
                    "📝 Description: {}",
                    op["description"].as_str().unwrap_or("N/A")
                );

                print_spec_parameters(op);
                print_spec_request_body(op);
                print_spec_responses(op);

                println!("\n💡 Usage Example:");
                if method.to_uppercase() == "POST" {
                    println!(
                        "cowen api {} \"{}\" -d '{{\"name\":\"John Doe\"}}'",
                        method.to_uppercase(),
                        path
                    );
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
