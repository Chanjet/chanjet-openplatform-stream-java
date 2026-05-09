use cowen_common::Config;
use cowen_auth::client::Client as AuthClientTrait;
use anyhow::anyhow;
use reqwest::Method;
use serde_json::Value;
use std::sync::Arc;

#[cfg(feature = "ai")]
async fn get_ai_embedder() -> anyhow::Result<cowen_ai::ONNXEmbedder> {
    let app_dir = cowen_common::config::get_app_dir();
    cowen_ai::SearchIndex::ensure_assets(&app_dir).map_err(|e| anyhow::anyhow!(e))?;
    let (m, t) = cowen_ai::SearchIndex::get_asset_paths(&app_dir);
    cowen_ai::ONNXEmbedder::new(&m, &t).map_err(|e| anyhow::anyhow!(e))
}

#[cfg(feature = "ai")]
async fn load_search_index(profile: &str, vault: &dyn cowen_common::vault::Vault) -> anyhow::Result<cowen_ai::SearchIndex> {
    let json = vault.get_config(profile, "search_index").await.map_err(|e| anyhow::anyhow!(e))?;
    let index: cowen_ai::SearchIndex = serde_json::from_str(&json)?;
    Ok(index)
}

#[cfg(feature = "ai")]
async fn save_search_index(profile: &str, vault: &dyn cowen_common::vault::Vault, index: &cowen_ai::SearchIndex) -> anyhow::Result<()> {
    let json = serde_json::to_string(index)?;
    vault.set_config(profile, "search_index", &json).await.map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

pub async fn list(
    profile: &str,
    cfg: &Config,
    auth_cli: &dyn AuthClientTrait,
    search: &Option<String>,
    _page: usize,
    _page_size: usize,
    format: &str,
    refresh: bool,
    vault: Arc<dyn cowen_common::vault::Vault>,
) -> anyhow::Result<()> {
    let mut spec = auth_cli.get_openapi_spec(profile, cfg, refresh).await.map_err(|e| anyhow::anyhow!(e))?;

    if let Some(query) = search {
        #[cfg(feature = "ai")]
        {
            if cfg.ai_enabled {
                let index = match load_search_index(profile, vault.as_ref()).await {
                    Ok(idx) if !refresh => idx,
                    _ => {
                        println!("🧠 Rebuilding local search index...");
                        let mut embedder = get_ai_embedder().await?;
                        let idx = embedder.rebuild_index(&spec).map_err(|e| anyhow::anyhow!(e))?;
                        save_search_index(profile, vault.as_ref(), &idx).await?;
                        idx
                    }
                };

                let mut embedder = get_ai_embedder().await?;
                let query_vec = embedder.embed(query).map_err(|e| anyhow::anyhow!(e))?;
                let results = index.search(&query_vec, query, 10);
                
                if format == "json" || format == "yaml" {
                    return cowen_common::utils::render(&results, format).map_err(|e| anyhow::anyhow!(e));
                }

                println!("\n🔍 Semantic Search Results for: '{}'", query);
                println!("--------------------------------------------------");
                for (score, doc) in results {
                    println!("\x1b[1;32m{:<30}\x1b[0m [Match: {:.1}%]", doc.id, score * 100.0);
                    println!("  Summary: {}", doc.summary);
                    println!();
                }
                return Ok(());
            }
        }
        
        // Basic filtering if AI is disabled or not compiled in
        if let Some(paths) = spec.get_mut("paths").and_then(|p| p.as_object_mut()) {
            paths.retain(|path, _| path.contains(query));
        }
    }

    cowen_common::utils::render(&spec, format).map_err(|e| anyhow::anyhow!(e))?;
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
    let op = cowen_auth::client::get_operation(&spec, path, method)
        .ok_or_else(|| anyhow!("API endpoint not found: {} {}", method.to_uppercase(), path))?;

    if raw {
        println!("{}", serde_json::to_string_pretty(&op)?);
        return Ok(());
    }

    println!("\n📖 API Specification: \x1b[1;32m{} {}\x1b[0m", method.to_uppercase(), path);
    println!("--------------------------------------------------");
    println!("Summary:     {}", op["summary"].as_str().unwrap_or("N/A"));
    println!("Description: {}", op["description"].as_str().unwrap_or("N/A"));

    if let Some(params) = op.get("parameters").and_then(|p| p.as_array()) {
        println!("\nParameters:");
        for p in params {
            println!("  - {:<15} ({}) {}", 
                p["name"].as_str().unwrap_or("?"), 
                p["in"].as_str().unwrap_or("?"),
                if p["required"].as_bool().unwrap_or(false) { "\x1b[31m[required]\x1b[0m" } else { "" }
            );
        }
    }
    println!();

    Ok(())
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

    // PRE-CHECK: Validate Parameters & Body against OpenAPI spec
    cowen_common::openapi::validate_request(&spec, method, path, &body_option).map_err(|e| anyhow::anyhow!(e))?;

    let path_no_query = path.split('?').next().unwrap_or(path);
    if !cowen_auth::client::is_path_in_whitelist(path_no_query, &spec) {
        return Err(anyhow!("CLI Rejected: Target path {} is not in the OpenAPI whitelist.", path_no_query));
    }

    // 2. Resolve Token
    let token = auth_cli.get_token(profile, cfg, &reqwest::header::HeaderMap::new()).await.map_err(|e| anyhow::anyhow!(e))?;

    // 3. Build & Execute Request
    let client = cowen_common::network::create_client(cfg).map_err(|e| anyhow::anyhow!(e))?;
    let url = if path.starts_with("http") {
        path.to_string()
    } else {
        let base = cfg.openapi_url.trim_end_matches('/');
        format!("{}{}", base, path)
    };

    let method_enum = Method::from_bytes(method.as_bytes()).map_err(|_| anyhow!("Invalid HTTP method: {}", method))?;
    
    let mut req = client.request(method_enum, &url)
        .header("openToken", token.value)
        .header("appKey", cfg.app_key.trim());

    if let Some(b) = body_option {
        let json_body: Value = serde_json::from_str(&b).map_err(|e| anyhow!("Invalid JSON payload: {}", e))?;
        req = req.json(&json_body);
    }

    let resp = req.send().await.map_err(|e| anyhow!("Request failed: {}", e))?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    // 4. Render Result
    if format == "json" || format == "yaml" {
        let json_val: Value = serde_json::from_str(&body).unwrap_or(Value::String(body));
        cowen_common::utils::render(&json_val, format).map_err(|e| anyhow::anyhow!(e))?;
    } else {
        println!("\n🚀 API Response (Status: {})", status);
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
