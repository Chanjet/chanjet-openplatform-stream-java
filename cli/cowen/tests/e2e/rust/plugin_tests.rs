use assert_cmd::Command;
use tempfile::tempdir;
use std::fs;
use serde_json::json;
use predicates::prelude::*;

#[test]
fn test_plugin_auto_discovery() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().to_str().unwrap().to_string();
    
    // 1. Create plugins directory
    let plugins_dir = dir.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    
    // 2. Find and copy the actual plugin library to the plugins folder
    // We expect the library to be in target/debug/deps/ (standard for cargo test environment)
    let search_pattern = if cfg!(target_os = "macos") {
        "libcowen_search_embedding.dylib"
    } else if cfg!(target_os = "windows") {
        "cowen_search_embedding.dll"
    } else {
        "libcowen_search_embedding.so"
    };

    // Locate the built plugin in the target directory
    let target_dir = std::env::current_dir().unwrap().join("target").join("debug").join("deps");
    let plugin_src = target_dir.join(search_pattern);
    
    // If not found in deps (e.g. built with different profile), try direct target dir
    let plugin_src = if !plugin_src.exists() {
        std::env::current_dir().unwrap().join("target").join("debug").join(search_pattern)
    } else {
        plugin_src
    };

    // Skip test if plugin wasn't built (to avoid breaking CI if it's an optional component)
    if !plugin_src.exists() {
        eprintln!("⚠️ Skipping plugin discovery test: plugin binary not found at {}", plugin_src.display());
        return;
    }

    // Official naming convention: libcowen_search_ai_embedding
    let target_name = if cfg!(target_os = "windows") {
        "cowen_search_ai_embedding.dll"
    } else if cfg!(target_os = "macos") {
        "libcowen_search_ai_embedding.dylib"
    } else {
        "libcowen_search_ai_embedding.so"
    };
    
    fs::copy(&plugin_src, plugins_dir.join(target_name)).unwrap();

    // 3. Create app.yaml with EMPTY plugins but ENABLED search-ai-embedding
    let app_yaml_path = dir.path().join("app.yaml");
    let app_config = json!({
        "storage": { "store": "innerdb" },
        "log": { "level": "info" },
        "search": {
            "plugins": [],
            "enabled": ["search-ai-embedding"]
        }
    });
    fs::write(app_yaml_path, serde_yaml::to_string(&app_config).unwrap()).unwrap();

    // 4. Setup a dummy profile to allow api list to run
    let profile = "default";
    let profile_path = dir.path().join(format!("{}.yaml", profile));
    let profile_config = json!({
        "app_key": "test",
        "app_mode": "self-built",
        "openapi_url": "http://localhost:1",
        "stream_url": "http://localhost:1",
        "webhook_target": "http://localhost:1"
    });
    fs::write(profile_path, serde_yaml::to_string(&profile_config).unwrap()).unwrap();
    
    // Mock the spec cache to avoid network calls
    let spec_path = dir.path().join(format!("{}_openapi.yaml", profile));
    let spec = json!({
        "openapi": "3.0.0",
        "paths": {
            "/test": { "get": { "summary": "test api" } }
        }
    });
    fs::write(spec_path, serde_yaml::to_string(&spec).unwrap()).unwrap();

    // 5. Execute command
    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &cowen_home)
       .arg("api")
       .arg("list")
       .arg("--search")
       .arg("test")
       .arg("--profile")
       .arg(profile);

    // 6. Assertions
    cmd.assert()
       .success()
       .stdout(predicate::str::contains("🔌 Using search plugin: search-ai-embedding"))
       .stdout(predicate::str::contains(target_name))
       .stdout(predicate::str::contains("🧠 Initializing AI vector index for 1 APIs..."))
       .stdout(predicate::str::contains("GET /test"))
       .stdout(predicate::str::contains("Summary: test api"));
}
