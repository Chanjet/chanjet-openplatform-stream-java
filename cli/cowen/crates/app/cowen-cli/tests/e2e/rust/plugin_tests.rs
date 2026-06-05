use assert_cmd::Command;
use tempfile::tempdir;
use std::fs;
use serde_json::json;
use predicates::prelude::*;

struct ChildGuard(std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct PluginTestGuard {
    home: String,
}
impl Drop for PluginTestGuard {
    fn drop(&mut self) {
        let pid_file = std::path::Path::new(&self.home).join("master_daemon.pid");
        if let Ok(content) = std::fs::read_to_string(&pid_file) {
            if let Some(first_line) = content.lines().next() {
                if let Ok(pid) = first_line.trim().parse::<i32>() {
                    let _ = std::process::Command::new("kill").arg("-9").arg(pid.to_string()).status();
                }
            }
        }
    }
}

#[test]
fn test_plugin_auto_discovery() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().to_str().unwrap().to_string();
    let _guard = PluginTestGuard { home: cowen_home.clone() };
    
    // 1. Create plugins directory
    let plugins_dir = dir.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();
    
    // 2. Find and copy the actual plugin executable to the plugins folder
    let search_pattern = if cfg!(target_os = "windows") {
        "libcowen_search_embedding.exe"
    } else {
        "libcowen_search_embedding"
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

    // Official naming convention: libcowen_search_embedding
    let target_name = if cfg!(target_os = "windows") {
        "libcowen_search_embedding.exe"
    } else {
        "libcowen_search_embedding"
    };
    
    fs::copy(&plugin_src, plugins_dir.join(target_name)).unwrap();

    // 3. Create app.yaml with ENABLED target_name (stem only, no extension)
    let app_yaml_path = dir.path().join("app.yaml");
    
    // strip extension from target_name
    let expected_stem = std::path::Path::new(target_name).file_stem().unwrap().to_str().unwrap();

    let free_port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let app_config = json!({
        "storage": { "store": "innerdb" },
        "log": { "level": "info" },
        "monitor_port": free_port,
        "plugins": [expected_stem]
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
       .env("COWEN_DEV_MODE", "1")
       .arg("api")
       .arg("list")
       .arg("--search")
       .arg("test")
       .arg("--profile")
       .arg(profile);

    // 6. Assertions
    let assert = cmd.assert();
    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    if !stdout.contains(&format!("🔌 Using search plugin: {}", expected_stem)) || !stdout.contains("[Rust Standalone Sidecar Mode]") {
        let log_content = std::fs::read_to_string(dir.path().join("logs").join("daemon.stdout.log")).unwrap_or_default();
        panic!("Test failed. stdout: {}\nstderr: {}\ndaemon log: {}", stdout, stderr, log_content);
    }
    
    assert
       .success()
       .stdout(predicate::str::contains(format!("🔌 Using search plugin: {}", expected_stem)))
       .stdout(predicate::str::contains("[Rust Standalone Sidecar Mode]"))
       .stdout(predicate::str::contains("🧠 Initializing AI vector index for 1 APIs..."))
       .stdout(predicate::str::contains("GET /test"))
       .stdout(predicate::str::contains("Summary: test api"));
}

#[test]
fn test_mcp_plugin_run_no_args() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().to_str().unwrap().to_string();
    let _guard = PluginTestGuard { home: cowen_home.clone() };
    let plugins_dir = dir.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let search_pattern = if cfg!(target_os = "windows") { "cowen-mcp-plugin.exe" } else { "cowen-mcp-plugin" };
    let target_dir = std::env::current_dir().unwrap().join("target").join("debug").join("deps");
    let mut plugin_src = target_dir.join(search_pattern);
    if !plugin_src.exists() {
        plugin_src = std::env::current_dir().unwrap().join("target").join("debug").join(search_pattern);
    }
    if !plugin_src.exists() { return; }

    let target_name = if cfg!(target_os = "windows") { "cowen-mcp-plugin.exe" } else { "cowen-mcp-plugin" };
    fs::copy(&plugin_src, plugins_dir.join(target_name)).unwrap();
    
    // Create plugin.json with core.rpc.stdio capability
    fs::write(plugins_dir.join("cowen-mcp-plugin.json"), json!({
        "required_capabilities": { "core.rpc.stdio": "v1" }
    }).to_string()).unwrap();

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &cowen_home)
       .arg("plugins")
       .arg("run");

    cmd.assert()
       .success()
       .stdout(predicate::str::contains("The following installed plugins implement 'core.rpc.stdio' (MCP servers):"))
       .stdout(predicate::str::contains("cowen-mcp-plugin"));
}

#[test]
fn test_mcp_plugin_run_config() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().to_str().unwrap().to_string();
    let _guard = PluginTestGuard { home: cowen_home.clone() };
    let plugins_dir = dir.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let search_pattern = if cfg!(target_os = "windows") { "cowen-mcp-plugin.exe" } else { "cowen-mcp-plugin" };
    let target_dir = std::env::current_dir().unwrap().join("target").join("debug").join("deps");
    let mut plugin_src = target_dir.join(search_pattern);
    if !plugin_src.exists() {
        plugin_src = std::env::current_dir().unwrap().join("target").join("debug").join(search_pattern);
    }
    if !plugin_src.exists() { return; }

    let target_name = if cfg!(target_os = "windows") { "cowen-mcp-plugin.exe" } else { "cowen-mcp-plugin" };
    fs::copy(&plugin_src, plugins_dir.join(target_name)).unwrap();
    
    fs::write(plugins_dir.join("cowen-mcp-plugin.json"), json!({
        "required_capabilities": { "core.rpc.stdio": "v1" }
    }).to_string()).unwrap();

    // Mock an active profile so the run command doesn't fail trying to load one
    let profile_path = dir.path().join("default.yaml");
    fs::write(profile_path, "app_key: test\napp_mode: self-built").unwrap();

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &cowen_home)
       .arg("plugins")
       .arg("run")
       .arg("cowen-mcp-plugin")
       .arg("config")
       .arg("--profile")
       .arg("default");

    cmd.assert()
       .success()
       .stdout(predicate::str::contains("\"mcpServers\": {"));
}

#[test]
fn test_mcp_plugin_run_non_mcp_error() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().to_str().unwrap().to_string();
    let _guard = PluginTestGuard { home: cowen_home.clone() };
    let plugins_dir = dir.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let search_pattern = if cfg!(target_os = "windows") { "libcowen_search_embedding.exe" } else { "libcowen_search_embedding" };
    let target_dir = std::env::current_dir().unwrap().join("target").join("debug").join("deps");
    let mut plugin_src = target_dir.join(search_pattern);
    if !plugin_src.exists() {
        plugin_src = std::env::current_dir().unwrap().join("target").join("debug").join(search_pattern);
    }
    if !plugin_src.exists() { return; }

    let target_name = if cfg!(target_os = "windows") { "libcowen_search_embedding.exe" } else { "libcowen_search_embedding" };
    fs::copy(&plugin_src, plugins_dir.join(target_name)).unwrap();
    
    // Missing capability
    fs::write(plugins_dir.join("libcowen_search_embedding.json"), json!({
        "required_capabilities": { "some.other.capability": "v1" }
    }).to_string()).unwrap();

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &cowen_home)
       .arg("plugins")
       .arg("run")
       .arg("libcowen_search_embedding");

    cmd.assert()
       .failure()
       .stderr(predicate::str::contains("does not declare 'core.rpc.stdio' capability"));
}

#[test]
fn test_mcp_client_simulation() {
    use std::io::{Write, BufRead, BufReader};
    use std::process::Stdio;

    let search_pattern = if cfg!(target_os = "windows") { "cowen-mcp-plugin.exe" } else { "cowen-mcp-plugin" };
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let plugin_src = manifest_dir.join("../../target/debug").join(search_pattern);
    if !plugin_src.exists() { return; }

    let mut child = std::process::Command::new(&plugin_src)
        .arg("server")
        .env("COWEN_PROFILE", "default")
        .env("COWEN_IPC_PORT", "58478")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn mcp plugin");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let _guard = ChildGuard(child);
    
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": {
                "name": "e2e-test-client",
                "version": "1.0.0"
            }
        }
    });

    let req_str = serde_json::to_string(&init_req).unwrap() + "\n";
    stdin.write_all(req_str.as_bytes()).expect("Failed to write to stdin");
    stdin.flush().unwrap();

    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).expect("Failed to read from stdout");
    
    let resp: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse JSON response");
    
    assert_eq!(resp.get("jsonrpc").unwrap().as_str().unwrap(), "2.0");
    assert_eq!(resp.get("id").unwrap().as_u64().unwrap(), 1);
    
    let result = resp.get("result").expect("Missing result object");
    assert_eq!(result.get("protocolVersion").unwrap().as_str().unwrap(), "2025-11-25");
    assert!(result.get("serverInfo").is_some());
    
    // Send initialized notification (optional, but good practice)
    let init_notif = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    let notif_str = serde_json::to_string(&init_notif).unwrap() + "\n";
    stdin.write_all(notif_str.as_bytes()).unwrap();
    stdin.flush().unwrap();
    
    // 2. Test tools/list
    let list_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    let req_str = serde_json::to_string(&list_req).unwrap() + "\n";
    stdin.write_all(req_str.as_bytes()).unwrap();
    stdin.flush().unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).expect("Failed to read from stdout");
    let resp: serde_json::Value = serde_json::from_str(&line).expect("Failed to parse tools/list response");
    
    assert_eq!(resp.get("id").unwrap().as_u64().unwrap(), 2);
    let tools = resp.get("result").unwrap().get("tools").unwrap().as_array().unwrap();
    assert!(tools.len() >= 3, "Should have at least 3 built-in tools");
    
    let has_api_list = tools.iter().any(|t| t.get("name").unwrap().as_str().unwrap() == "cowen_api_list");
    assert!(has_api_list, "Missing cowen_api_list tool");

    // 3. Test tools/call (cowen_enable_api)
    let call_req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "cowen_enable_api",
            "arguments": {
                "tool_name": "get__v1_test"
            }
        }
    });
    let req_str = serde_json::to_string(&call_req).unwrap() + "\n";
    stdin.write_all(req_str.as_bytes()).unwrap();
    stdin.flush().unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).expect("Failed to read from stdout");
    
    // Check if we received a list_changed notification before the actual result
    let mut resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    
    // If daemon is not running, we expect a gRPC Error in the tools/call result
    if resp.get("method").and_then(|m| m.as_str()) == Some("notifications/tools/list_changed") {
        // Read the next line which should be the response to id 3
        line.clear();
        reader.read_line(&mut line).expect("Failed to read from stdout");
        resp = serde_json::from_str(&line).unwrap();
    }
    
    assert_eq!(resp.get("id").unwrap().as_u64().unwrap(), 3);
    let result = resp.get("result").unwrap();
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();
    
    // Since there's no daemon running in this test, it should return a gRPC error string gracefully
    assert!(text.contains("gRPC Error") || text.contains("transport error") || text.contains("Connection refused"), "Expected gRPC connection failure: {}", text);
    
}

#[test]
fn test_mcp_api_list_schema_validation() {
    use std::io::{Write, BufRead, BufReader};
    use std::process::Stdio;

    let search_pattern = if cfg!(target_os = "windows") { "cowen-mcp-plugin.exe" } else { "cowen-mcp-plugin" };
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let plugin_src = manifest_dir.join("../../target/debug").join(search_pattern);
    if !plugin_src.exists() { return; }

    let mut child = std::process::Command::new(&plugin_src)
        .arg("server")
        .env("COWEN_PROFILE", "default")
        .env("COWEN_IPC_PORT", "58478")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn mcp plugin");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let _guard = ChildGuard(child);

    // Initialize
    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "e2e-test-client", "version": "1.0.0" }
        }
    });
    let req_str = serde_json::to_string(&init_req).unwrap() + "\n";
    stdin.write_all(req_str.as_bytes()).unwrap();
    stdin.flush().unwrap();

    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader.read_line(&mut line).expect("Failed to read from stdout");

    // Call cowen_api_list
    let call_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "cowen_api_list",
            "arguments": {}
        }
    });
    let req_str = serde_json::to_string(&call_req).unwrap() + "\n";
    stdin.write_all(req_str.as_bytes()).unwrap();
    stdin.flush().unwrap();

    line.clear();
    reader.read_line(&mut line).expect("Failed to read from stdout");
    
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp.get("id").unwrap().as_u64().unwrap(), 2);
    
    let result = resp.get("result").unwrap();
    
    // We expect the gRPC Error to set isError to true, and if it is false, structuredContent must exist.
    let is_error = result.get("isError").unwrap().as_bool().unwrap_or(false);
    
    if !is_error {
        assert!(result.get("structuredContent").is_some(), "MCP error -32600: Tool cowen_api_list has an output schema but did not return structuredContent! Response was: {}", resp);
    }

}

#[test]
fn test_mcp_plugin_no_subcommand_prints_help() {
    let search_pattern = if cfg!(target_os = "windows") { "cowen-mcp-plugin.exe" } else { "cowen-mcp-plugin" };
    let target_dir = std::env::current_dir().unwrap().join("target").join("debug").join("deps");
    let mut plugin_src = target_dir.join(search_pattern);
    if !plugin_src.exists() {
        plugin_src = std::env::current_dir().unwrap().join("target").join("debug").join(search_pattern);
    }
    if !plugin_src.exists() { return; }

    let mut cmd = std::process::Command::new(&plugin_src);
    let output = cmd.output().unwrap();

    assert!(output.status.success(), "Expected exit status 0 when running with no subcommand, got {:?}", output.status);
    let stdout_str = String::from_utf8(output.stdout).unwrap();
    assert!(stdout_str.contains("Cowen MCP Plugin"), "Expected help output containing 'Cowen MCP Plugin', got stdout: {}", stdout_str);
}

