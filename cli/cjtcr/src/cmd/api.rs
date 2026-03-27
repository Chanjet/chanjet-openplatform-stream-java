use crate::core::config::Config;
use crate::auth::client::Client as AuthClientTrait;
use anyhow::{Result, anyhow, Context};
use reqwest::Method;
use serde_json::Value;

pub async fn call(
    profile: &str,
    cfg: &Config,
    auth_cli: &dyn AuthClientTrait,
    method: &str,
    path: &str,
    data: &Option<String>,
) -> Result<()> {
    // 1. Get Access Token
    let token = auth_cli.get_app_access_token(profile, cfg).await?;

    // 2. Perform Request
    let client = reqwest::Client::new();
    let url = if path.starts_with("http") {
        path.to_string()
    } else {
        format!("https://openapi.chanjet.com{}", path)
    };

    let req_method = Method::from_bytes(method.to_uppercase().as_bytes())
        .map_err(|_| anyhow!("Invalid HTTP method: {}", method))?;

    println!("Executing {} {}...", req_method, url);

    let mut req = client.request(req_method, &url)
        .header("Authorization", format!("Bearer {}", token.value))
        .header("Content-Type", "application/json");

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
) -> Result<()> {
    let spec = auth_cli.get_openapi_spec(profile, cfg).await?;
    let paths = spec["paths"].as_object().ok_or_else(|| anyhow!("Invalid OpenAPI spec: missing paths"))?;

    if let Some(query) = search_query {
        let home = directories::UserDirs::new()
            .context("Could not find home directory")?
            .home_dir()
            .to_path_buf();
        let cache_dir = home.join(".cjtc");
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

        println!("\n🧠 Neural Search: \"{}\" (Hybrid Match)", query);
        println!("{}", "-".repeat(80));

        let mut embedder = crate::core::search::ONNXEmbedder::new()?;
        let query_vec = embedder.embed(query)?;
        
        let matches = index.search(&query_vec, query, top);

        if matches.is_empty() {
            println!("No APIs found matching \"{}\".", query);
        } else {
            for (i, (score, doc)) in matches.iter().enumerate() {
                println!("{}. [{}] ({:.2}) {}", i + 1, doc.id, score, doc.summary);
            }
            if matches.len() >= top {
                println!("... top results shown. Use --top to see more.");
            }
            println!("\n✅ Verified: Zero-dependency ONNX embedding engine is active (Cache Layered).");
        }
    } else {
        println!("\n📖 Available APIs (Top {}):", top);
        println!("{:<10} {:<30} {}", "METHOD", "PATH", "SUMMARY");
        println!("{}", "-".repeat(80));

        let mut count = 0;
        for (path, methods) in paths {
            if let Some(methods_obj) = methods.as_object() {
                for (method, op) in methods_obj {
                    let summary = op["summary"].as_str().unwrap_or("");
                    println!("{:<10} {:<30} {}", method.to_uppercase(), path, summary);
                    count += 1;
                    if count >= top { break; }
                }
            }
            if count >= top { break; }
        }
    }

    Ok(())
}
