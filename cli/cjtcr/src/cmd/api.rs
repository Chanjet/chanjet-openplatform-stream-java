use crate::core::config::Config;
use crate::auth::client::Client as AuthClientTrait;
use anyhow::{Result, anyhow, Context};
use reqwest::Method;
use serde_json::Value;
use serde_yaml;

pub async fn call(
    profile: &str,
    cfg: &Config,
    auth_cli: &dyn AuthClientTrait,
    method: &str,
    path: &str,
    data: &Option<String>,
) -> Result<()> {
    // PROTECT CLI: Whitelist Check
    let spec = auth_cli.get_openapi_spec(profile, cfg).await?;
    let path_no_query = path.split('?').next().unwrap_or(path);
    if !crate::auth::client::is_path_in_whitelist(path_no_query, &spec) {
        return Err(anyhow!("CLI Rejected: Target path {} is not in the OpenAPI whitelist.", path_no_query));
    }

    // 1. Get Access Token
    let token = auth_cli.get_app_access_token(profile, cfg).await?;

    // 2. Identify Content-Type from Spec
    let mut content_type = "application/json".to_string(); // Default fallback
    if let Some(operation) = crate::auth::client::get_operation(&spec, path_no_query, method) {
        if let Some(request_body) = operation.get("requestBody") {
            if let Some(content_map) = request_body.get("content").and_then(|c| c.as_object()) {
                // Pick the first supported type (priority: JSON > Form > Others)
                if content_map.contains_key("application/json") {
                    content_type = "application/json".to_string();
                } else if content_map.contains_key("application/x-www-form-urlencoded") {
                    content_type = "application/x-www-form-urlencoded".to_string();
                } else if let Some(first) = content_map.keys().next() {
                    content_type = first.to_string();
                }
            }
        }
    }

    // 3. Perform Request
    let client = reqwest::Client::new();
    let url = if path.starts_with("http") {
        path.to_string()
    } else {
        let base = cfg.openapi_url.trim_end_matches('/');
        format!("{}{}", base, path)
    };

    let req_method = Method::from_bytes(method.to_uppercase().as_bytes())
        .map_err(|_| anyhow!("Invalid HTTP method: {}", method))?;

    let mut req = client.request(req_method, &url)
        .header("Content-Type", content_type);

    // Dynamic Header Injection based on Spec (Shared Logic)
    let auth_headers = crate::auth::RequestDecorator::get_auth_headers(
        &spec, 
        path_no_query, 
        method, 
        &cfg.app_key, 
        &cfg.app_secret, 
        &token.value
    );

    for (name, value) in auth_headers {
        req = req.header(name, value);
    }

    if let Some(body_data) = data {
        req = req.body(body_data.clone());
    }

    let resp = req.send().await?;

    let status = resp.status();
    let body: Value = resp.json().await?;

    if status.is_success() {
        println!("{}", serde_json::to_string_pretty(&body)?);
    } else {
        eprintln!("Error ({}): {}", status, serde_json::to_string_pretty(&body)?);
    }

    Ok(())
}

pub async fn list(
    profile: &str,
    cfg: &Config,
    auth_cli: &dyn AuthClientTrait,
    search_query: &Option<String>,
    top: usize,
    page: usize,
    page_size: usize,
    format: &str,
) -> Result<()> {
    let spec = auth_cli.get_openapi_spec(profile, cfg).await?;
    let paths = spec["paths"].as_object().ok_or_else(|| anyhow!("Invalid OpenAPI spec: missing paths"))?;

    if let Some(query) = search_query {
        let cache_dir = crate::core::config::get_app_dir();
        let spec_path = cache_dir.join(format!("{}_openapi.json", profile));
        let index_path = cache_dir.join(format!("{}_openapi.idx", profile));

        // 1. Check if index needs rebuild
        let mut index_ready = false;
        if index_path.exists() && spec_path.exists() {
            let spec_meta = std::fs::metadata(&spec_path)?;
            let index_meta = std::fs::metadata(&index_path)?;
            if index_meta.modified()? >= spec_meta.modified()? {
                index_ready = true;
            }
        }

        let index = if !index_ready {
            println!("🔄 Rebuilding semantic search index for profile \"{}\"...", profile);
            let mut embedder = crate::core::search::ONNXEmbedder::new()?;
            let new_index = embedder.rebuild_index(&spec)?;
            new_index.save(&index_path)?;
            println!("✅ Index rebuilt and saved to {:?}", index_path);
            new_index
        } else {
            crate::core::search::SearchIndex::load(&index_path)?
        };

        let mut embedder = crate::core::search::ONNXEmbedder::new()?;
        let query_vec = embedder.embed(query)?;
        
        let matches = index.search(&query_vec, query, top * page); // Fetch more for pagination

        let start = (page - 1) * page_size;
        if start >= matches.len() {
            println!("No results found for page {}.", page);
            return Ok(());
        }
        let end = std::cmp::min(start + page_size, matches.len());
        let paginated = &matches[start..end];

        match format {
            "json" => println!("{}", serde_json::to_string_pretty(&paginated)?),
            "yaml" => println!("{}", serde_yaml::to_string(&paginated)?),
            _ => {
                println!("\n🧠 Neural Search: \"{}\" (Page {}/{})", query, page, (matches.len() + page_size - 1) / page_size);
                println!("{}", "-".repeat(100));
                for (i, (score, doc)) in paginated.iter().enumerate() {
                    println!("{}. [{}] ({:.2}) {} - {}", start + i + 1, doc.id, score, doc.summary, doc.description);
                }
                println!("\n✅ Verified: Zero-dependency ONNX embedding engine is active.");
                println!("(TIP: Run 'api spec [METHOD] [PATH]' for full details)\n");
            }
        }
    } else {
        // Collect all APIs
        let mut all_apis = Vec::new();
        for (path, methods) in paths {
            if let Some(methods_obj) = methods.as_object() {
                for (method, op) in methods_obj {
                    let summary = op["summary"].as_str().unwrap_or("").to_string();
                    let description = op["description"].as_str().unwrap_or("").to_string();
                    all_apis.push(serde_json::json!({
                        "method": method.to_uppercase(),
                        "path": path,
                        "summary": summary,
                        "description": description
                    }));
                }
            }
        }

        let start = (page - 1) * page_size;
        if start >= all_apis.len() {
            println!("No results found for page {}.", page);
            return Ok(());
        }
        let end = std::cmp::min(start + page_size, all_apis.len());
        let paginated = &all_apis[start..end];

        match format {
            "json" => println!("{}", serde_json::to_string_pretty(&paginated)?),
            "yaml" => println!("{}", serde_yaml::to_string(&paginated)?),
            _ => {
                println!("\n📖 Available APIs (Page {}/{}):", page, (all_apis.len() + page_size - 1) / page_size);
                println!(" {:<11} {:<40} {}", "METHOD", "PATH", "INFO");
                println!("{}", "—".repeat(110));
                for api in paginated {
                    let method = api["method"].as_str().unwrap_or("");
                    let path = api["path"].as_str().unwrap_or("");
                    let summary = api["summary"].as_str().unwrap_or("");
                    let description = api["description"].as_str().unwrap_or("");
                    
                    let info = if description.is_empty() || description == summary {
                        summary.to_string()
                    } else {
                        format!("{} - {}", summary, description)
                    };

                    // Colorize Method (using ANSI codes)
                    let colored_method = match method {
                        "GET" => format!("\x1b[32m{:<8}\x1b[0m", method),    // Green
                        "POST" => format!("\x1b[36m{:<8}\x1b[0m", method),   // Cyan
                        "PUT" => format!("\x1b[33m{:<8}\x1b[0m", method),    // Yellow
                        "DELETE" => format!("\x1b[31m{:<8}\x1b[0m", method), // Red
                        _ => format!("{:<8}", method),
                    };

                    println!(" {} {:<40} {}", 
                        colored_method,
                        path,
                        info
                    );
                }
                println!("{}", "—".repeat(110));
                println!("(TIP: Run 'api spec [METHOD] [PATH]' for full details or '-s 关键词' for semantic search)\n");
            }
        }
    }

    Ok(())
}

pub async fn spec(
    profile: &str,
    cfg: &Config,
    auth_cli: &dyn AuthClientTrait,
    method: &str,
    input_path: &str,
    raw: bool,
) -> Result<()> {
    let spec = auth_cli.get_openapi_spec(profile, cfg).await?;
    
    // 1. Resolve Path
    let matched_path = crate::auth::client::find_matching_spec_path(input_path, &spec)
        .ok_or_else(|| anyhow!("Path '{}' not found in OpenAPI spec", input_path))?;
    
    let path_item = spec["paths"][&matched_path].as_object()
        .ok_or_else(|| anyhow!("Invalid spec structure for path '{}'", matched_path))?;
    
    // 2. Resolve Method
    let op = path_item.get(&method.to_lowercase())
        .ok_or_else(|| anyhow!("Method '{}' not supported for path '{}'", method, matched_path))?;

    if raw {
        let mut clean_op = op.clone();
        if let Some(params) = clean_op.get_mut("parameters").and_then(|p| p.as_array_mut()) {
            params.retain(|p| {
                let name = p["name"].as_str().unwrap_or("");
                name != "appKey" && name != "appSecret" && name != "openToken"
            });
        }
        tracing::info!(target: "audit", method = %method, path = %matched_path, format = "raw", "view api spec");
        println!("{}", serde_json::to_string_pretty(&clean_op)?);
        return Ok(());
    }

    tracing::info!(target: "audit", method = %method, path = %matched_path, format = "text", "view api spec");

    // 3. Print Header
    let summary = op["summary"].as_str().unwrap_or("No Summary");
    let tag = op["tags"].as_array().and_then(|t| t[0].as_str()).unwrap_or("General");
    
    println!("\n{}", "=".repeat(80));
    println!("{:<10} {} - {}", method.to_uppercase(), matched_path, summary);
    println!("Tag:       {}", tag);
    println!("{}", "=".repeat(80));

    // 4. Description
    if let Some(desc) = op["description"].as_str() {
        println!("\nDescription:\n  {}\n", desc);
    }

    // 5. Parameters
    if let Some(params) = op["parameters"].as_array() {
        let filtered_params: Vec<&serde_json::Value> = params.iter()
            .filter(|p| {
                let name = p["name"].as_str().unwrap_or("-");
                name != "appKey" && name != "openToken"
            })
            .collect();

        if !filtered_params.is_empty() {
            println!("Parameters:");
            println!("  {:<15} {:<10} {:<10} {}", "NAME", "IN", "REQUIRED", "DESCRIPTION");
            println!("  {}", "-".repeat(76));
            for p in filtered_params {
                let name = p["name"].as_str().unwrap_or("-");
                let location = p["in"].as_str().unwrap_or("-");
                let req = if p["required"].as_bool().unwrap_or(false) { "Yes" } else { "No" };
                let desc = p["description"].as_str().unwrap_or("-");
                println!("  {:<15} {:<10} {:<10} {}", name, location, req, desc);
            }
            println!();
        }
    }

    // 6. Request Body
    if let Some(body) = op.get("requestBody") {
        if let Some(content_map) = body.get("content").and_then(|c| c.as_object()) {
            let empty_schema = serde_json::json!({});
            let (ctype, schema) = if let Some(s) = content_map.get("application/json").and_then(|t| t.get("schema")) {
                ("application/json", s)
            } else if let Some(s) = content_map.get("application/x-www-form-urlencoded").and_then(|t| t.get("schema")) {
                ("application/x-www-form-urlencoded", s)
            } else if let Some((k, v)) = content_map.iter().next() {
                (k.as_str(), v.get("schema").unwrap_or(&empty_schema))
            } else {
                ("", &empty_schema)
            };

            if !ctype.is_empty() {
                println!("Request Body ({}):", ctype);
                print_schema_recursive(schema, 2);
            }
        }
        println!();
    }

    // 7. Responses
    if let Some(responses) = op.get("responses").and_then(|r| r.as_object()) {
        println!("Responses:");
        for (code, resp) in responses {
            let desc = resp["description"].as_str().unwrap_or("-");
            println!("  {}: {}", code, desc);
            if let Some(content) = resp.get("content").and_then(|c| c.get("application/json")).and_then(|a| a.get("schema")) {
                print_schema_recursive(content, 4);
            }
        }
    }

    println!("\nUsage Example:");
    let cmd_example = generate_usage_example(method, matched_path.as_str(), op);
    println!("  {}", cmd_example);

    println!("\n{}", "=".repeat(80));

    Ok(())
}

fn generate_usage_example(method: &str, path: &str, op: &serde_json::Value) -> String {
    let mut final_path = path.to_string();
    
    // Append required query parameters if any
    if let Some(params) = op.get("parameters").and_then(|p| p.as_array()) {
        let mut query_parts = Vec::new();
        for p in params {
            if p["in"].as_str() == Some("query") && p["required"].as_bool() == Some(true) {
                let name = p["name"].as_str().unwrap_or("-");
                query_parts.push(format!("{}=<{}>", name, name));
            }
        }
        if !query_parts.is_empty() {
            final_path.push('?');
            final_path.push_str(&query_parts.join("&"));
        }
    }

    let mut parts = vec![format!("./cjtc api {} \"{}\"", method.to_lowercase(), final_path)];
    
    if let Some(body) = op.get("requestBody") {
        if let Some(content_map) = body.get("content").and_then(|c| c.as_object()) {
            let empty_schema = serde_json::json!({});
            // Detect preferred type
            let (ctype, schema) = if let Some(s) = content_map.get("application/json").and_then(|t| t.get("schema")) {
                ("application/json", s)
            } else if let Some(s) = content_map.get("application/x-www-form-urlencoded").and_then(|t| t.get("schema")) {
                ("application/x-www-form-urlencoded", s)
            } else if let Some((k, v)) = content_map.iter().next() {
                (k.as_str(), v.get("schema").unwrap_or(&empty_schema))
            } else {
                ("", &empty_schema)
            };

            if !ctype.is_empty() {
                let sample_data = generate_sample_data(schema, ctype);
                parts.push(format!("--data '{}'", sample_data));
            }
        }
    }
    
    parts.join(" ")
}

fn generate_sample_data(schema: &serde_json::Value, content_type: &str) -> String {
    let mut data_map = serde_json::Map::new();
    
    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        for (name, prop) in props {
            let p_type = prop["type"].as_str().unwrap_or("string");
            let val = match p_type {
                "string" => {
                    if let Some(ex) = prop.get("example").and_then(|e| e.as_str()) {
                        serde_json::json!(ex)
                    } else {
                        serde_json::json!(format!("<{}>", name))
                    }
                }
                "integer" | "number" => serde_json::json!(0),
                "boolean" => serde_json::json!(false),
                "array" => serde_json::json!([]),
                "object" => serde_json::json!({}),
                _ => serde_json::json!(null),
            };
            data_map.insert(name.to_string(), val);
        }
    }

    if content_type == "application/x-www-form-urlencoded" {
        // Convert map to key=value&key2=value2
        let mut pairs = Vec::new();
        for (k, v) in data_map {
            let v_str = match v {
                serde_json::Value::String(s) => s,
                _ => v.to_string(),
            };
            pairs.push(format!("{}={}", k, v_str));
        }
        pairs.join("&")
    } else {
        // Default to JSON
        serde_json::to_string(&serde_json::Value::Object(data_map)).unwrap_or_default()
    }
}

fn print_schema_recursive(schema: &serde_json::Value, indent: usize) {
    let spaces = " ".repeat(indent);
    if let Some(desc) = schema.get("description").and_then(|d| d.as_str()) {
        println!("{}# {}", spaces, desc);
    }

    let required_fields: Vec<&str> = schema.get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
        for (name, prop) in props {
            let p_type = prop["type"].as_str().unwrap_or("any");
            let p_desc = prop["description"].as_str().unwrap_or("");
            
            // Mark as required if in parent's required list
            let name_display = if required_fields.contains(&name.as_str()) {
                format!("{}*", name)
            } else {
                name.to_string()
            };

            // Capture enum values if any
            let enum_info = if let Some(enums) = prop.get("enum").and_then(|e| e.as_array()) {
                let vals: Vec<String> = enums.iter().map(|v| v.to_string()).collect();
                format!(" [enum: {}]", vals.join(", "))
            } else {
                "".to_string()
            };

            println!("{}{:<15} ({:<10}) {}{}", spaces, name_display, p_type, p_desc, enum_info);
            
            if p_type == "object" {
                print_schema_recursive(prop, indent + 4);
            } else if p_type == "array" {
                if let Some(items) = prop.get("items") {
                    println!("{}  [Items]:", spaces);
                    print_schema_recursive(items, indent + 4);
                }
            }
        }
    } else if let Some(p_type) = schema.get("type").and_then(|t| t.as_str()) {
        if p_type == "array" {
             if let Some(items) = schema.get("items") {
                println!("{}Array of:", spaces);
                print_schema_recursive(items, indent + 2);
            }
        }
    }
}
