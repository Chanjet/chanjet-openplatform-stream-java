use cowen_common::Config;
use cowen_auth::client::Client as AuthClientTrait;
use anyhow::anyhow;
use reqwest::Method;
use serde_json::Value;
use std::sync::Arc;
use cowen_search::{SearchProvider, StringMatchProvider, SidecarSearchProvider};

#[derive(serde::Serialize)]
struct ApiOperation {
    method: String,
    path: String,
    summary: String,
}

pub async fn list(
    profile: &str,
    cfg: &Config,
    auth_cli: &dyn AuthClientTrait,
    search: &Option<String>,
    page: usize,
    page_size: usize,
    format: &str,
    refresh: bool,
    _vault: Arc<dyn cowen_common::vault::Vault>,
) -> anyhow::Result<()> {
    let spec = auth_cli.get_openapi_spec(profile, cfg, refresh).await.map_err(|e| anyhow::anyhow!(e))?;

    // 1. Flatten operations
    let mut all_ops = Vec::new();
    if let Some(paths) = spec.get("paths").and_then(|p| p.as_object()) {
        for (path, methods) in paths {
            if let Some(methods_obj) = methods.as_object() {
                for (method, op) in methods_obj {
                    let summary = op.get("summary").and_then(|s| s.as_str()).unwrap_or("").to_string();
                    all_ops.push(ApiOperation {
                        method: method.to_uppercase(),
                        path: path.clone(),
                        summary,
                    });
                }
            }
        }
    }

    // 2. Sort by path then method
    all_ops.sort_by(|a, b| a.path.cmp(&b.path).then(a.method.cmp(&b.method)));

    if let Some(query) = search {
        let app_cfg = cowen_config::ConfigManager::new()?.load_app_config().await?;

        // Convert spec to SearchDocuments FIRST so we can feed them to the plugin
        let search_docs: Vec<cowen_search::SearchDocument> = all_ops.iter().map(|op| {
            cowen_search::SearchDocument {
                id: format!("{} {}", op.method, op.path),
                summary: op.summary.clone(),
                description: String::new(),
                vector: vec![],
            }
        }).collect();

        // Initialize composite search provider
        let mut primary: Option<Box<dyn SearchProvider>> = None;
        
        // Try to load the enabled plugin from config
        if !app_cfg.plugins.is_empty() {
            let plugins_dir = cowen_common::config::get_app_dir().join("plugins");
            let discovered = cowen_sys::discover_plugins(&plugins_dir);
            
            // Iterate over all enabled plugins in config
            for plugin_name in &app_cfg.plugins {
                if let Some(path) = discovered.iter().find(|p| {
                    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    stem == plugin_name
                }) {
                    // Load and check trait
                    let mut is_search_provider = false;
                    match cowen_sys::PluginLoader::new(&path) {
                        Ok(loader) => {
                            if loader.supports_trait("SearchProvider") {
                                is_search_provider = true;
                            }
                        }
                        Err(e) => {
                            eprintln!("⚠️  Refusing to load plugin at {}: {}", path.display(), e);
                        }
                    }

                    if is_search_provider {
                        println!("🔌 Using search plugin: {} ({}) [Rust Standalone Sidecar Mode]", plugin_name, path.display());
                        
                        // Dynamically generate tenant_id Namespace hash on the fly based on active profile name
                        use sha2::{Sha256, Digest};
                        let mut hasher = Sha256::new();
                        hasher.update(profile.as_bytes());
                        let tenant_id = hex::encode(hasher.finalize());

                        let provider = Box::new(SidecarSearchProvider::new(
                            plugin_name,
                            path.to_path_buf(),
                            tenant_id,
                        ));

                        // 🚀 LIFE CYCLE: Push documents to the Sidecar over standard Stdio
                        println!("🧠 Initializing AI vector index for {} APIs...", search_docs.len());
                        provider.update_index(&search_docs);
                        
                        primary = Some(provider);
                        break; // Found the active search provider, stop looking
                    }
                }
            }
            
            if primary.is_none() {
                let expected_trait = cowen_search::plugin_trait_search_provider!().to_str().unwrap();
                eprintln!("⚠️  No active plugin with '{}' trait found in config or ~/.cowen/plugins/", expected_trait);
            }
        }

        let mut results = vec![];
        let mut used_provider = String::new();
        let mut is_basic = false;

        if let Some(p) = primary {
            results = p.search(query, page * page_size);
            used_provider = p.name().to_string();
        }

        if results.is_empty() {
            // fallback
            let fallback = StringMatchProvider { docs: search_docs };
            results = fallback.search(query, page * page_size);
            used_provider = "basic_text_match".to_string();
            is_basic = true;
        }
        
        // Perform search
        println!("🔍 Searching for: '{}' (via {})", query, used_provider);
        
        let start = (page.max(1) - 1) * page_size;
        let paged_results = if start < results.len() {
            &results[start..]
        } else {
            &[]
        };

        println!("--------------------------------------------------");
        if paged_results.is_empty() {
            println!("  (No results found)");
        } else {
            for (score, doc) in paged_results {
                if is_basic {
                    println!("\x1b[1;32m{:<30}\x1b[0m [\x1b[36mBasic Match\x1b[0m]", doc.id);
                } else {
                    println!("\x1b[1;32m{:<30}\x1b[0m [Match: {:.1}%]", doc.id, score * 100.0);
                }
                println!("  Summary: {}", doc.summary);
                println!();
            }
        }
        
        return Ok(());
    }

    // 3. Paginate
    let total = all_ops.len();
    let start = (page.max(1) - 1) * page_size;
    let end = (start + page_size).min(total);
    
    let paged_ops = if start < total {
        &all_ops[start..end]
    } else {
        &[]
    };

    if format == "json" || format == "yaml" {
        return cowen_common::utils::render(&paged_ops, format).map_err(|e| anyhow::anyhow!(e));
    }

    println!("\n🌐 Available APIs for profile: \x1b[1;32m{}\x1b[0m (Page {}, Total {})", profile, page, total);
    println!("--------------------------------------------------");
    if paged_ops.is_empty() {
        println!("  (No APIs found or page out of range)");
    } else {
        for op in paged_ops {
            println!("\x1b[1;32m{:<8}\x1b[0m {:<45} {}", op.method, op.path, op.summary);
        }
    }
    
    if end < total {
        println!("\n📑 Next page available. Use '--page {}' to view more.", page + 1);
    }
    println!("\n💡 Use 'cowen api list --search <QUERY>' for semantic search.");
    println!("💡 Use 'cowen api spec <METHOD> <PATH>' to view detailed documentation.");

    Ok(())
}

pub async fn spec(
    profile: &str,
    cfg: &Config,
    auth_cli: &dyn AuthClientTrait,
    method: &str,
    path: &str,
    raw: bool,
) -> anyhow::Result<()> {
    let spec = auth_cli.get_openapi_spec(profile, cfg, false).await.map_err(|e| anyhow::anyhow!(e))?;
    let method_upper = method.to_uppercase();
    let op = cowen_auth::client::get_operation(&spec, path, &method_upper)
        .ok_or_else(|| anyhow!("API endpoint not found: {} {}", method_upper, path))?;

    if raw {
        println!("{}", serde_json::to_string_pretty(&op)?);
        return Ok(());
    }

    println!("\n📖 API Specification Details");
    println!("--------------------------------------------------");
    println!("📍 Endpoint:    \x1b[1;32m{} {}\x1b[0m", method_upper, path);
    println!("📌 Summary:     {}", op["summary"].as_str().unwrap_or("N/A"));
    if let Some(tags) = op.get("tags").and_then(|t| t.as_array()) {
        let tags_str: Vec<String> = tags.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect();
        println!("🏷️  Tags:        {}", tags_str.join(", "));
    }
    println!("📝 Description: {}", op["description"].as_str().unwrap_or("N/A"));

    if let Some(params) = op.get("parameters").and_then(|p| p.as_array()) {
        println!("\n🛠️  Parameters:");
        for p in params {
            let name = p["name"].as_str().unwrap_or("?");
            let location = p["in"].as_str().unwrap_or("?");
            let required = p["required"].as_bool().unwrap_or(false);
            let ty = p.get("schema")
                .and_then(|s| s.get("type").and_then(|t| t.as_str()))
                .unwrap_or("string");
            let desc = p["description"].as_str().unwrap_or("");

            println!("  🔹 {:<15} ({:<8}) {:<10} {} {}", 
                name, 
                location,
                format!("<{}>", ty),
                if required { "\x1b[31m[required]\x1b[0m" } else { "" },
                desc
            );
        }
    }

    // 1. Request Body
    if let Some(body) = op.get("requestBody") {
        println!("\n📥 Request Body:");
        if let Some(content) = body.get("content").and_then(|c| c.as_object()) {
            for (mime, media_type) in content {
                println!("  Type: \x1b[36m{}\x1b[0m", mime);
                if let Some(schema) = media_type.get("schema") {
                    render_schema(&spec, schema, 2);
                }
            }
        }
    }

    // 2. Responses
    if let Some(responses) = op.get("responses").and_then(|r| r.as_object()) {
        println!("\n📤 Responses:");
        for (code, resp) in responses {
            println!("  \x1b[1;33m{}\x1b[0m - {}", code, resp["description"].as_str().unwrap_or("N/A"));
            if let Some(content) = resp.get("content").and_then(|c| c.as_object()) {
                for (mime, media_type) in content {
                    println!("    Type: \x1b[36m{}\x1b[0m", mime);
                    if let Some(schema) = media_type.get("schema") {
                        render_schema(&spec, schema, 3);
                    }
                }
            }
        }
    }

    // 3. Usage Example
    println!("\n💡 Usage Example:");
    let mut example_path = path.to_string();
    let mut query_params = Vec::new();

    if let Some(params) = op.get("parameters").and_then(|p| p.as_array()) {
        for p in params {
            let name = p["name"].as_str().unwrap_or("?");
            let location = p["in"].as_str().unwrap_or("?");
            let required = p["required"].as_bool().unwrap_or(false);
            
            if location == "path" {
                example_path = example_path.replace(&format!("{{{}}}", name), &format!("<{}>", name));
            } else if location == "query" && required {
                query_params.push(format!("{}={}", name, format!("<{}>", name)));
            }
        }
    }
    
    if !query_params.is_empty() {
        if !example_path.contains('?') {
            example_path.push('?');
        } else {
            example_path.push('&');
        }
        example_path.push_str(&query_params.join("&"));
    }

    let mut cmd = format!("cowen api {} \"{}\"", method.to_uppercase(), example_path);

    if let Some(body) = op.get("requestBody") {
        if let Some(content) = body.get("content").and_then(|c| c.as_object()) {
            if let Some(media_type) = content.get("application/json").or_else(|| content.values().next()) {
                if let Some(schema) = media_type.get("schema") {
                    let sample = generate_sample_json(&spec, schema, 0);
                    if !sample.is_null() {
                        cmd.push_str(&format!(" -d '{}'", serde_json::to_string(&sample).unwrap_or_default()));
                    }
                }
            }
        }
    }

    println!("  {}", cmd);

    println!();
    Ok(())
}

fn generate_sample_json(spec: &serde_json::Value, schema: &serde_json::Value, depth: usize) -> serde_json::Value {
    if depth > 3 { return serde_json::Value::Null; }

    if let Some(ref_str) = schema.get("$ref").and_then(|r| r.as_str()) {
        if let Some(resolved) = resolve_ref(spec, ref_str) {
            return generate_sample_json(spec, resolved, depth + 1);
        }
    }

    if let Some(example) = schema.get("example") {
        return example.clone();
    }

    let ty = schema.get("type").and_then(|t| t.as_str()).unwrap_or("object");
    match ty {
        "object" => {
            let mut obj = serde_json::Map::new();
            if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
                for (name, prop_schema) in props {
                    obj.insert(name.clone(), generate_sample_json(spec, prop_schema, depth + 1));
                }
            }
            serde_json::Value::Object(obj)
        }
        "array" => {
            if let Some(items) = schema.get("items") {
                serde_json::json!([generate_sample_json(spec, items, depth + 1)])
            } else {
                serde_json::json!([])
            }
        }
        "string" => serde_json::Value::String("<string>".to_string()),
        "integer" | "number" => serde_json::json!(0),
        "boolean" => serde_json::json!(true),
        _ => serde_json::Value::Null,
    }
}

fn resolve_ref<'a>(spec: &'a serde_json::Value, reference: &str) -> Option<&'a serde_json::Value> {
    if !reference.starts_with("#/") { return None; }
    let parts: Vec<&str> = reference.split('/').skip(1).collect();
    let mut current = spec;
    for part in parts {
        current = current.get(part)?;
    }
    Some(current)
}

fn render_schema(spec: &serde_json::Value, schema: &serde_json::Value, indent: usize) {
    let prefix = "  ".repeat(indent);
    
    if let Some(ref_str) = schema.get("$ref").and_then(|r| r.as_str()) {
        if let Some(resolved) = resolve_ref(spec, ref_str) {
            render_schema(spec, resolved, indent);
            return;
        }
    }

    let ty = schema.get("type").and_then(|t| t.as_str()).unwrap_or("object");
    
    match ty {
        "object" => {
            let required_fields: Vec<String> = schema.get("required")
                .and_then(|r| r.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();

            if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
                for (name, prop_schema) in props {
                    let is_required = required_fields.contains(name);
                    let prop_ty = prop_schema.get("type").and_then(|t| t.as_str())
                        .or_else(|| prop_schema.get("$ref").map(|_| "object"))
                        .unwrap_or("any");
                    let desc = prop_schema.get("description").and_then(|d| d.as_str()).unwrap_or("");
                    
                    println!("{}{:<15} {:<12} {:<12} {}", 
                        prefix, 
                        name, 
                        format!("<{}>", prop_ty), 
                        if is_required { "\x1b[31m[required]\x1b[0m" } else { "" },
                        desc
                    );
                    
                    if let Some(enums) = prop_schema.get("enum").and_then(|e| e.as_array()) {
                        let enum_vals: Vec<String> = enums.iter().map(|v| v.to_string()).collect();
                        println!("{}  └─ Enums: [{}]", prefix, enum_vals.join(", "));
                    }

                    if prop_ty == "object" || prop_schema.get("properties").is_some() || prop_schema.get("$ref").is_some() {
                        render_schema(spec, prop_schema, indent + 1);
                    } else if prop_ty == "array" {
                        if let Some(items) = prop_schema.get("items") {
                            println!("{}  └─ Array Items:", prefix);
                            render_schema(spec, items, indent + 2);
                        }
                    }
                }
            }
        },
        "array" => {
             if let Some(items) = schema.get("items") {
                 println!("{}  [Array of Objects]", prefix);
                 render_schema(spec, items, indent + 1);
             }
        }
        _ => {}
    }
}

pub async fn call(
    profile: &str,
    cfg: &Config,
    auth_cli: &dyn AuthClientTrait,
    method: &str,
    path: &str,
    data: &Option<String>,
    data_file: &Option<String>,
    format: &str,
    force: bool,
) -> anyhow::Result<()> {
    if !auth_cli.supports_api_call(cfg) {
        return Err(anyhow!("Auth mode '{:?}' does not support direct CLI API calls. Please use your main application to trigger requests.", cfg.app_mode));
    }

    // 1. Resolve Body Data
    let body_str = if let Some(file_path) = data_file {
        std::fs::read_to_string(file_path).map_err(|e| anyhow!("Failed to read data file: {}", e))?
    } else {
        data.clone().unwrap_or_else(|| "{}".to_string())
    };

    let body_option = if body_str.trim() == "{}" || body_str.trim().is_empty() {
        None
    } else {
        Some(body_str)
    };

    // PROTECT CLI: Whitelist Check
    let spec = auth_cli.get_openapi_spec(profile, cfg, false).await.map_err(|e| anyhow::anyhow!(e))?;

    let method_upper = method.to_uppercase();

    // PRE-CHECK: Validate Parameters & Body against OpenAPI spec
    if !force {
        cowen_common::openapi::validate_request(&spec, &method_upper, path, &body_option).map_err(|e| anyhow::anyhow!(e))?;

        let path_no_query = path.split('?').next().unwrap_or(path);
        if !cowen_auth::client::is_path_in_whitelist(path_no_query, &spec) {
            return Err(anyhow!("CLI Rejected: Target path {} is not in the OpenAPI whitelist. (Use --force to bypass)", path_no_query));
        }
    } else {
        tracing::warn!(target: "sys", "API Validation bypassed due to --force flag.");
    }

    // SSRF Check: Prevent absolute external URLs even with --force
    let app_cfg = cowen_config::ConfigManager::new()?.load_app_config().await?;
    if path.starts_with("http") && !path.starts_with(&app_cfg.openapi_url) {
        return Err(anyhow!("CLI Security Block: Absolute external URLs are not allowed."));
    }

    // 2. Resolve Token
    let token = auth_cli.get_token(profile, cfg, &reqwest::header::HeaderMap::new()).await.map_err(|e| anyhow::anyhow!(e))?;

    // 3. Build & Execute Request
    let ua = cowen_infra::get_user_agent(env!("CARGO_PKG_VERSION"));
    let client = cowen_infra::create_client(&ua).map_err(|e| anyhow::anyhow!(e))?;
    let url = if path.starts_with("http") {
        path.to_string()
    } else {
        let base = app_cfg.openapi_url.trim_end_matches('/');
        format!("{}{}", base, path)
    };

    let method_enum = Method::from_bytes(method_upper.as_bytes()).map_err(|_| anyhow!("Invalid HTTP method: {}", method_upper))?;
    
    let mut req = client.request(method_enum, &url)
        .header("openToken", token.value)
        .header("appKey", cfg.app_key.trim());

    if let Some(b) = body_option {
        let json_body: Value = serde_json::from_str(&b).map_err(|e| anyhow!("Invalid JSON payload: {}", e))?;
        req = req.json(&json_body);
    }

    let resp = req.send().await.map_err(|e| anyhow!("Request failed: {}", e))?;
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp.text().await.unwrap_or_default();

    // 4. Render Result
    if format == "json" || format == "yaml" {
        let mut json_val: Value = serde_json::from_str(&body).unwrap_or(Value::String(body));
        
        // 🚀 OCP: Inject Trace ID into JSON if available for better observability
        if let Some(trace_id) = headers.get("x-b3-traceid")
            .or_else(|| headers.get("x-msg-id"))
            .or_else(|| headers.get("msgId"))
            .or_else(|| headers.get("x-trace-id"))
            .and_then(|v| v.to_str().ok()) {
            if let Value::Object(ref mut map) = json_val {
                map.insert("_trace_id".to_string(), Value::String(trace_id.to_string()));
            }
        }

        cowen_common::utils::render(&json_val, format).map_err(|e| anyhow::anyhow!(e))?;
    } else {
        println!("\n🚀 API Response (Status: {})", status);
        if let Some(trace_id) = headers.get("x-b3-traceid")
            .or_else(|| headers.get("x-msg-id"))
            .or_else(|| headers.get("msgId"))
            .or_else(|| headers.get("x-trace-id"))
            .and_then(|v| v.to_str().ok()) {
            println!("\x1b[1;30mTrace ID: {}\x1b[0m", trace_id);
        }
        println!("--------------------------------------------------");
        if let Ok(json_val) = serde_json::from_str::<Value>(&body) {
            println!("{}", serde_json::to_string_pretty(&json_val).unwrap());
        } else {
            println!("{}", body);
        }
        println!();
    }

    Ok(())
}
