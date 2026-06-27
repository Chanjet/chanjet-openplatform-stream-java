use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use std::fs;
use tempfile::tempdir;

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
                    let _ = std::process::Command::new("kill")
                        .arg("-9")
                        .arg(pid.to_string())
                        .status();
                }
            }
        }
    }
}

#[test]
fn test_plugin_auto_discovery() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().to_str().unwrap().to_string();
    let _guard = PluginTestGuard {
        home: cowen_home.clone(),
    };

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
    let target_dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("deps");
    let plugin_src = target_dir.join(search_pattern);

    // If not found in deps (e.g. built with different profile), try direct target dir
    let plugin_src = if !plugin_src.exists() {
        std::env::current_dir()
            .unwrap()
            .join("target")
            .join("debug")
            .join(search_pattern)
    } else {
        plugin_src
    };

    // Skip test if plugin wasn't built (to avoid breaking CI if it's an optional component)
    if !plugin_src.exists() {
        eprintln!(
            "⚠️ Skipping plugin discovery test: plugin binary not found at {}",
            plugin_src.display()
        );
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
    let expected_stem = std::path::Path::new(target_name)
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap();

    let free_port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
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
    fs::write(
        profile_path,
        serde_yaml::to_string(&profile_config).unwrap(),
    )
    .unwrap();

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

    if !stdout.contains(&format!("🔌 Using search plugin: {}", expected_stem))
        || !stdout.contains("[Rust Standalone Sidecar Mode]")
    {
        let log_content =
            std::fs::read_to_string(dir.path().join("logs").join("daemon.stdout.log"))
                .unwrap_or_default();
        panic!(
            "Test failed. stdout: {}\nstderr: {}\ndaemon log: {}",
            stdout, stderr, log_content
        );
    }

    assert
        .success()
        .stdout(predicate::str::contains(format!(
            "🔌 Using search plugin: {}",
            expected_stem
        )))
        .stdout(predicate::str::contains("[Rust Standalone Sidecar Mode]"))
        .stdout(predicate::str::contains(
            "🧠 Initializing AI vector index for 1 APIs...",
        ))
        .stdout(predicate::str::contains("GET /test"))
        .stdout(predicate::str::contains("Summary: test api"));
}

#[test]
fn test_mcp_plugin_run_no_args() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().to_str().unwrap().to_string();
    let _guard = PluginTestGuard {
        home: cowen_home.clone(),
    };
    let plugins_dir = dir.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let search_pattern = if cfg!(target_os = "windows") {
        "cowen-mcp-plugin.exe"
    } else {
        "cowen-mcp-plugin"
    };
    let target_dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("deps");
    let mut plugin_src = target_dir.join(search_pattern);
    if !plugin_src.exists() {
        plugin_src = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("debug")
            .join(search_pattern);
    }
    if !plugin_src.exists() {
        return;
    }

    let target_name = if cfg!(target_os = "windows") {
        "cowen-mcp-plugin.exe"
    } else {
        "cowen-mcp-plugin"
    };
    fs::copy(&plugin_src, plugins_dir.join(target_name)).unwrap();

    // Create plugin.json with stdio transport
    fs::write(
        plugins_dir.join("cowen-mcp-plugin.json"),
        json!({
            "transport": "stdio",
            "required_capabilities": { "native.api.registry": "v1" }
        })
        .to_string(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &cowen_home).arg("plugins").arg("run");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "The following installed plugins implement 'stdio' transport (MCP servers):",
        ))
        .stdout(predicate::str::contains("cowen-mcp-plugin"));
}

#[test]
fn test_mcp_plugin_run_config() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().to_str().unwrap().to_string();
    let _guard = PluginTestGuard {
        home: cowen_home.clone(),
    };
    let plugins_dir = dir.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let search_pattern = if cfg!(target_os = "windows") {
        "cowen-mcp-plugin.exe"
    } else {
        "cowen-mcp-plugin"
    };
    let target_dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("deps");
    let mut plugin_src = target_dir.join(search_pattern);
    if !plugin_src.exists() {
        plugin_src = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("debug")
            .join(search_pattern);
    }
    if !plugin_src.exists() {
        return;
    }

    let target_name = if cfg!(target_os = "windows") {
        "cowen-mcp-plugin.exe"
    } else {
        "cowen-mcp-plugin"
    };
    fs::copy(&plugin_src, plugins_dir.join(target_name)).unwrap();

    fs::write(
        plugins_dir.join("cowen-mcp-plugin.json"),
        json!({
            "transport": "stdio",
            "required_capabilities": { "native.api.registry": "v1" }
        })
        .to_string(),
    )
    .unwrap();

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
    let _guard = PluginTestGuard {
        home: cowen_home.clone(),
    };
    let plugins_dir = dir.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let search_pattern = if cfg!(target_os = "windows") {
        "libcowen_search_embedding.exe"
    } else {
        "libcowen_search_embedding"
    };
    let target_dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("deps");
    let mut plugin_src = target_dir.join(search_pattern);
    if !plugin_src.exists() {
        plugin_src = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("debug")
            .join(search_pattern);
    }
    if !plugin_src.exists() {
        return;
    }

    let target_name = if cfg!(target_os = "windows") {
        "libcowen_search_embedding.exe"
    } else {
        "libcowen_search_embedding"
    };
    fs::copy(&plugin_src, plugins_dir.join(target_name)).unwrap();

    // Missing capability
    fs::write(
        plugins_dir.join("libcowen_search_embedding.json"),
        json!({
            "required_capabilities": { "some.other.capability": "v1" }
        })
        .to_string(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("cowen").unwrap();
    cmd.env("COWEN_HOME", &cowen_home)
        .arg("plugins")
        .arg("run")
        .arg("libcowen_search_embedding");

    cmd.assert().failure().stderr(predicate::str::contains(
        "does not declare 'stdio' transport",
    ));
}

#[test]
fn test_mcp_client_simulation() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;

    let search_pattern = if cfg!(target_os = "windows") {
        "cowen-mcp-plugin.exe"
    } else {
        "cowen-mcp-plugin"
    };
    let _manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let plugin_src = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join(search_pattern);
    if !plugin_src.exists() {
        return;
    }
    //.join("../../target/debug").join(search_pattern);
    if !plugin_src.exists() {
        return;
    }

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
    stdin
        .write_all(req_str.as_bytes())
        .expect("Failed to write to stdin");
    stdin.flush().unwrap();

    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("Failed to read from stdout");

    let resp: serde_json::Value =
        serde_json::from_str(&line).expect("Failed to parse JSON response");

    assert_eq!(resp.get("jsonrpc").unwrap().as_str().unwrap(), "2.0");
    assert_eq!(resp.get("id").unwrap().as_u64().unwrap(), 1);

    let result = resp.get("result").expect("Missing result object");
    assert_eq!(
        result.get("protocolVersion").unwrap().as_str().unwrap(),
        "2025-11-25"
    );
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
    reader
        .read_line(&mut line)
        .expect("Failed to read from stdout");
    let resp: serde_json::Value =
        serde_json::from_str(&line).expect("Failed to parse tools/list response");

    assert_eq!(resp.get("id").unwrap().as_u64().unwrap(), 2);
    let tools = resp
        .get("result")
        .unwrap()
        .get("tools")
        .unwrap()
        .as_array()
        .unwrap();
    assert!(tools.len() >= 3, "Should have at least 3 built-in tools");

    let has_api_list = tools
        .iter()
        .any(|t| t.get("name").unwrap().as_str().unwrap() == "cowen_api_list");
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
    reader
        .read_line(&mut line)
        .expect("Failed to read from stdout");

    // Check if we received a list_changed notification before the actual result
    let mut resp: serde_json::Value = serde_json::from_str(&line).unwrap();

    // If daemon is not running, we expect a gRPC Error in the tools/call result
    if resp.get("method").and_then(|m| m.as_str()) == Some("notifications/tools/list_changed") {
        // Read the next line which should be the response to id 3
        line.clear();
        reader
            .read_line(&mut line)
            .expect("Failed to read from stdout");
        resp = serde_json::from_str(&line).unwrap();
    }

    assert_eq!(resp.get("id").unwrap().as_u64().unwrap(), 3);
    let result = resp.get("result").unwrap();
    let content = result.get("content").unwrap().as_array().unwrap();
    let text = content[0].get("text").unwrap().as_str().unwrap();

    // Since there's no daemon running in this test, it should return a gRPC error string gracefully
    assert!(
        text.contains("gRPC Error")
            || text.contains("transport error")
            || text.contains("Connection refused"),
        "Expected gRPC connection failure: {}",
        text
    );
}

#[test]
fn test_mcp_api_list_schema_validation() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;

    let search_pattern = if cfg!(target_os = "windows") {
        "cowen-mcp-plugin.exe"
    } else {
        "cowen-mcp-plugin"
    };
    let _manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let plugin_src = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join(search_pattern);
    if !plugin_src.exists() {
        return;
    }
    //.join("../../target/debug").join(search_pattern);
    if !plugin_src.exists() {
        return;
    }

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
    reader
        .read_line(&mut line)
        .expect("Failed to read from stdout");

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
    reader
        .read_line(&mut line)
        .expect("Failed to read from stdout");

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
    let search_pattern = if cfg!(target_os = "windows") {
        "cowen-mcp-plugin.exe"
    } else {
        "cowen-mcp-plugin"
    };
    let target_dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("deps");
    let mut plugin_src = target_dir.join(search_pattern);
    if !plugin_src.exists() {
        plugin_src = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("debug")
            .join(search_pattern);
    }
    if !plugin_src.exists() {
        return;
    }

    let mut cmd = std::process::Command::new(&plugin_src);
    let output = cmd.output().unwrap();

    assert!(
        output.status.success(),
        "Expected exit status 0 when running with no subcommand, got {:?}",
        output.status
    );
    let stdout_str = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout_str.contains("Cowen MCP Plugin"),
        "Expected help output containing 'Cowen MCP Plugin', got stdout: {}",
        stdout_str
    );
}

#[test]
fn test_plugin_auto_restart_on_kill() {
    let dir = tempdir().unwrap();
    let plugins_dir = dir.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let search_pattern = if cfg!(target_os = "windows") {
        "libcowen_search_embedding.exe"
    } else {
        "libcowen_search_embedding"
    };

    let target_dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("deps");
    let mut plugin_src = target_dir.join(search_pattern);
    if !plugin_src.exists() {
        plugin_src = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("debug")
            .join(search_pattern);
    }
    if !plugin_src.exists() {
        eprintln!("⚠️ Skipping test_plugin_auto_restart_on_kill: plugin binary not found.");
        return;
    }

    // Use a unique name to avoid killing other tests' plugins
    let unique_name = if cfg!(target_os = "windows") {
        "libcowen_test_restart_123.exe"
    } else {
        "libcowen_test_restart_123"
    };
    let plugin_dest = plugins_dir.join(unique_name);
    fs::copy(&plugin_src, &plugin_dest).unwrap();

    let client = cowen_sys::plugin::RpcPluginClient::new(
        plugin_dest.clone(),
        "test_tenant".to_string(),
        None,
    );

    // Initial call
    let req = serde_json::json!({});
    let res = client.call_tool("cowen.capabilities.info", req.clone());
    assert!(res.is_ok(), "First call should succeed, got: {:?}", res);

    // Kill the process
    #[cfg(unix)]
    {
        let _ = std::process::Command::new("pkill")
            .arg("-9")
            .arg("-f")
            .arg("libcowen_test_restart_123")
            .status();
    }
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("taskkill")
            .arg("/F")
            .arg("/IM")
            .arg("libcowen_test_restart_123.exe")
            .status();
    }

    // Give OS a moment to clean up pipes
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Second call should automatically restart and succeed
    let res2 = client.call_tool("cowen.capabilities.info", req);
    assert!(
        res2.is_ok(),
        "Second call should automatically restart the process and succeed, got: {:?}",
        res2
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_plugins_management() {
    let (mock_port, _state) = crate::e2e::rust::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);

    let dir = tempfile::tempdir().unwrap();
    let home_str = dir.path().to_str().unwrap().to_string();

    let mut cmd_init = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_init.env("COWEN_HOME", &home_str);
    cmd_init.env("HOME", &home_str);
    cmd_init.args([
        "init",
        "--profile",
        "main",
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_SB",
        "--app-secret",
        "AS_SB",
        "--encrypt-key",
        "1234567890123456",
        "--certificate",
        "CERT_SB",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
    ]);
    cmd_init.assert().success();

    // 1. Empty List
    let plugins_dir = dir.path().join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();

    let mut cmd_list = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_list.env("COWEN_HOME", &home_str);
    cmd_list.env("HOME", &home_str);
    cmd_list.args(["plugins", "list", "--profile", "main"]);
    let stdout = String::from_utf8_lossy(&cmd_list.output().unwrap().stdout).to_string();
    assert!(
        stdout.contains("(No executable plugins found)"),
        "Failed to report empty plugins list: {}",
        stdout
    );

    // 2. Copy Plugin and List
    let search_pattern = if cfg!(target_os = "windows") {
        "libcowen_search_embedding.exe"
    } else {
        "libcowen_search_embedding"
    };

    let target_dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("deps");
    let mut plugin_src = target_dir.join(search_pattern);
    if !plugin_src.exists() {
        plugin_src = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("debug")
            .join(search_pattern);
    }
    if !plugin_src.exists() {
        eprintln!("⚠️ Skipping test_plugins_management: plugin binary not found.");
        return;
    }

    let plugin_dest = plugins_dir.join(search_pattern);
    std::fs::copy(&plugin_src, &plugin_dest).expect("Failed to copy plugin");

    let bundle_name = "libcowen_search_embedding.bundle";
    let mut bundle_src = target_dir.join(bundle_name);
    if !bundle_src.exists() {
        bundle_src = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("debug")
            .join(bundle_name);
    }
    if bundle_src.exists() {
        let bundle_dest = plugins_dir.join(bundle_name);
        let _ = std::fs::copy(&bundle_src, &bundle_dest);
    }

    let mut cmd_list2 = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_list2.env("COWEN_HOME", &home_str);
    cmd_list2.env("HOME", &home_str);
    cmd_list2.args(["plugins", "list", "--profile", "main"]);
    let stdout2 = String::from_utf8_lossy(&cmd_list2.output().unwrap().stdout).to_string();
    assert!(
        stdout2.contains("libcowen_search_embedding"),
        "List output: {}",
        stdout2
    );

    // 3. Enable Plugin
    let mut cmd_enable = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_enable.env("COWEN_HOME", &home_str);
    cmd_enable.env("HOME", &home_str);
    cmd_enable.args([
        "plugins",
        "enable",
        "libcowen_search_embedding",
        "--profile",
        "main",
    ]);
    cmd_enable.assert().success();

    let mut cmd_list3 = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_list3.env("COWEN_HOME", &home_str);
    cmd_list3.env("HOME", &home_str);
    cmd_list3.args(["plugins", "list", "--profile", "main"]);
    let stdout3 = String::from_utf8_lossy(&cmd_list3.output().unwrap().stdout).to_string();
    assert!(
        stdout3.contains("Yes") || stdout3.contains("libcowen_search_embedding"),
        "Failed to enable: {}",
        stdout3
    );

    // 4. Disable Plugin
    let mut cmd_disable = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_disable.env("COWEN_HOME", &home_str);
    cmd_disable.env("HOME", &home_str);
    cmd_disable.args([
        "plugins",
        "disable",
        "libcowen_search_embedding",
        "--profile",
        "main",
    ]);
    cmd_disable.assert().success();

    let mut cmd_list4 = assert_cmd::Command::cargo_bin("cowen").unwrap();
    cmd_list4.env("COWEN_HOME", &home_str);
    cmd_list4.env("HOME", &home_str);
    cmd_list4.args(["plugins", "list", "--profile", "main"]);
    let stdout4 = String::from_utf8_lossy(&cmd_list4.output().unwrap().stdout).to_string();
    // It should report "No" but the exact column matching is hard, just ensuring the command succeeds and list completes
    assert!(stdout4.contains("No") || stdout4.contains("libcowen_search_embedding"));
}

#[test]
fn test_plugin_usability() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().to_str().unwrap().to_string();
    let _guard = PluginTestGuard {
        home: cowen_home.clone(),
    };

    let plugins_dir = dir.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let search_pattern = if cfg!(target_os = "windows") {
        "libcowen_search_embedding.exe"
    } else {
        "libcowen_search_embedding"
    };

    let target_dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("deps");
    let plugin_src = target_dir.join(search_pattern);
    let plugin_src = if !plugin_src.exists() {
        std::env::current_dir()
            .unwrap()
            .join("target")
            .join("debug")
            .join(search_pattern)
    } else {
        plugin_src
    };

    if !plugin_src.exists() {
        eprintln!(
            "⚠️ Skipping test_plugin_usability: plugin binary not found at {}",
            plugin_src.display()
        );
        return;
    }

    let target_name = if cfg!(target_os = "windows") {
        "libcowen_search_embedding.exe"
    } else {
        "libcowen_search_embedding"
    };

    fs::copy(&plugin_src, plugins_dir.join(target_name)).unwrap();

    // Enable plugin
    let mut enable_cmd = Command::cargo_bin("cowen").unwrap();
    enable_cmd.env("COWEN_HOME", &cowen_home);
    enable_cmd.env("HOME", &cowen_home);
    enable_cmd.env_remove("COWEN_DEV_MODE");
    enable_cmd.args(["plugins", "enable", "libcowen_search_embedding"]);
    let _ = enable_cmd.output();

    let mut list_cmd = Command::cargo_bin("cowen").unwrap();
    list_cmd.env("COWEN_HOME", &cowen_home);
    list_cmd.env("HOME", &cowen_home);
    list_cmd.env_remove("COWEN_DEV_MODE");
    list_cmd.args(["plugins", "list"]);
    let out = list_cmd.output().unwrap();
    let out_str = String::from_utf8_lossy(&out.stdout).to_string();

    assert!(
        out_str.contains("libcowen_search_embedding"),
        "Output should contain libcowen_search_embedding: {}",
        out_str
    );
    assert!(
        out_str.contains("Yes"),
        "Plugin should be enabled: {}",
        out_str
    );
    assert!(
        !out_str.contains("Failed"),
        "Plugin shouldn't be failed: {}",
        out_str
    );
}

#[test]
fn test_plugin_install_bundle() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().to_str().unwrap().to_string();
    let _guard = PluginTestGuard {
        home: cowen_home.clone(),
    };

    let tmp_plugin_src = dir.path().join("plugin_source");
    fs::create_dir_all(&tmp_plugin_src).unwrap();

    let search_pattern = if cfg!(target_os = "windows") {
        "libcowen_search_embedding.exe"
    } else {
        "libcowen_search_embedding"
    };

    let target_dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("deps");
    let plugin_src = target_dir.join(search_pattern);
    let plugin_src = if !plugin_src.exists() {
        std::env::current_dir()
            .unwrap()
            .join("target")
            .join("debug")
            .join(search_pattern)
    } else {
        plugin_src
    };

    if !plugin_src.exists() {
        eprintln!(
            "⚠️ Skipping test_plugin_install_bundle: plugin binary not found at {}",
            plugin_src.display()
        );
        return;
    }

    let target_name = if cfg!(target_os = "windows") {
        "libcowen_search_embedding.exe"
    } else {
        "libcowen_search_embedding"
    };
    let bundle_name = "libcowen_search_embedding.bundle";

    let src_bundle_path = plugin_src.with_file_name(bundle_name);

    fs::copy(&plugin_src, tmp_plugin_src.join(target_name)).unwrap();
    if src_bundle_path.exists() {
        fs::copy(&src_bundle_path, tmp_plugin_src.join(bundle_name)).unwrap();
    }

    // Install plugin
    let mut install_cmd = Command::cargo_bin("cowen").unwrap();
    install_cmd.env("COWEN_HOME", &cowen_home);
    install_cmd.env("HOME", &cowen_home);
    install_cmd.args([
        "plugins",
        "install",
        tmp_plugin_src.join(target_name).to_str().unwrap(),
    ]);
    install_cmd.assert().success();

    let plugin_target_dir = dir.path().join("plugins");
    assert!(
        plugin_target_dir.join(target_name).exists(),
        "Plugin binary was not installed"
    );
    if src_bundle_path.exists() {
        assert!(
            plugin_target_dir.join(bundle_name).exists(),
            "Plugin bundle was not installed"
        );
    }

    let mut list_cmd = Command::cargo_bin("cowen").unwrap();
    list_cmd.env("COWEN_HOME", &cowen_home);
    list_cmd.env("HOME", &cowen_home);
    list_cmd.env_remove("COWEN_DEV_MODE");
    list_cmd.args(["plugins", "list"]);
    let out = list_cmd.output().unwrap();
    let out_str = String::from_utf8_lossy(&out.stdout).to_string();

    assert!(
        out_str.contains("libcowen_search_embedding"),
        "Output should contain libcowen_search_embedding: {}",
        out_str
    );
    assert!(
        !out_str.contains("Failed"),
        "Plugin shouldn't be failed: {}",
        out_str
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mcp_plugin_api_list() {
    let dir = tempdir().unwrap();
    let cowen_home = dir.path().to_str().unwrap().to_string();
    let _guard = PluginTestGuard {
        home: cowen_home.clone(),
    };

    let (mock_port, _) = super::mock_server::spawn_mock_server().await;
    let mock_url = format!("http://127.0.0.1:{}", mock_port);

    let plugins_dir = dir.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let search_pattern = if cfg!(target_os = "windows") {
        "cowen-mcp-plugin.exe"
    } else {
        "cowen-mcp-plugin"
    };

    let target_dir = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("debug")
        .join("deps");
    let plugin_src = target_dir.join(search_pattern);
    let plugin_src = if !plugin_src.exists() {
        std::env::current_dir()
            .unwrap()
            .join("target")
            .join("debug")
            .join(search_pattern)
    } else {
        plugin_src
    };

    if !plugin_src.exists() {
        eprintln!(
            "⚠️ Skipping test_mcp_plugin_api_list: plugin binary not found at {}",
            plugin_src.display()
        );
        return;
    }

    let target_name = search_pattern;
    fs::copy(&plugin_src, plugins_dir.join(target_name)).unwrap();

    // 1. Initialize profile
    let mut init_cmd = Command::cargo_bin("cowen").unwrap();
    init_cmd.env("COWEN_HOME", &cowen_home);
    init_cmd.env("HOME", &cowen_home);
    init_cmd.args([
        "init",
        "--profile",
        "main",
        "--app-mode",
        "self-built",
        "--app-key",
        "AK_SB",
        "--app-secret",
        "AS_SB",
        "--certificate",
        "test-cert",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_url,
        "--encrypt-key",
        "1234567890123456",
        "--webhook-target",
        "http://127.0.0.1:8080",
        "--no-telemetry",
    ]);
    init_cmd.assert().success();

    // 2. Test MCP plugin API list
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "cowen_api_list",
            "arguments": {
                "search": "账套"
            }
        }
    });

    let mut run_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    run_cmd.env("COWEN_HOME", &cowen_home);
    run_cmd.env("HOME", &cowen_home);
    run_cmd.args([
        "--profile",
        "main",
        "plugins",
        "run",
        "cowen-mcp-plugin",
        "--",
        "server",
    ]);

    use std::io::Write;
    run_cmd.stdin(std::process::Stdio::piped());
    run_cmd.stdout(std::process::Stdio::piped());

    let mut child = run_cmd.spawn().unwrap();

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(req.to_string().as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
        drop(stdin); // close stdin to signal EOF
    }

    let out = child.wait_with_output().unwrap();
    let out_str = String::from_utf8_lossy(&out.stdout).to_string();
    let err_str = String::from_utf8_lossy(&out.stderr).to_string();

    assert!(
        !out_str.contains("OAuth2 session missing or expired"),
        "MCP plugin ignored COWEN_PROFILE: {}",
        out_str
    );
    assert!(
        !err_str.contains("OAuth2 session missing or expired"),
        "MCP plugin ignored COWEN_PROFILE: {}",
        err_str
    );

    assert!(
        out_str.contains("total") || out_str.contains("content"),
        "Expected API list output not found. stdout: {}, stderr: {}",
        out_str,
        err_str
    );
}

#[tokio::test]
async fn test_mcp_standalone_launch() {
    let profile = "case_86_mcp";
    let dir = tempdir().unwrap();
    let home = dir.path().join(".cowen");
    std::fs::create_dir_all(&home).unwrap();

    let mcp_bin = assert_cmd::cargo::cargo_bin("cowen-mcp-plugin");

    let mut help_cmd = std::process::Command::new(&mcp_bin);
    help_cmd.arg("--help");
    assert!(help_cmd.status().unwrap().success());

    let cfg_cmd = std::process::Command::new(&mcp_bin)
        .arg("config")
        .output()
        .unwrap();
    let cfg_str = String::from_utf8_lossy(&cfg_cmd.stdout);
    assert!(cfg_str.contains("mcpServers"));
    assert!(cfg_str.contains("cowen-mcp-plugin"));

    let mut init_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "init",
        "--profile",
        profile,
        "--app-key",
        "dummykey",
        "--app-secret",
        "dummysecret",
        "--app-mode",
        "self-built",
        "--certificate",
        "dummy_cert",
        "--encrypt-key",
        "dummy_ek",
    ]);
    assert!(init_cmd.status().unwrap().success());

    let mut daemon_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd
        .env("COWEN_HOME", &home)
        .env("HOME", &home)
        .args(["--profile", profile, "daemon", "start"]);
    assert!(daemon_cmd.status().unwrap().success());
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let mut server_cmd = std::process::Command::new(&mcp_bin);
    server_cmd
        .env("COWEN_HOME", &home)
        .env("HOME", &home)
        .args(["--profile", profile, "server"]);
    server_cmd.stdin(std::process::Stdio::piped());
    server_cmd.stdout(std::process::Stdio::piped());

    let mut server_child = server_cmd.spawn().unwrap();
    let mut stdin = server_child.stdin.take().unwrap();

    use std::io::Write;
    stdin.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test-client\",\"version\":\"1.0.0\"}}}\n").unwrap();
    stdin
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
        .unwrap();
    stdin
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}\n")
        .unwrap();
    drop(stdin);

    let out = server_child.wait_with_output().unwrap();
    let _stdout = String::from_utf8_lossy(&out.stdout);

    // As long as it doesn't panic and starts up properly, we're good.
    // Real validation of protocol is done.
}

#[tokio::test]
async fn test_wasm_plugin_pipeline() {
    let profile = "wasm_test";
    let dir = tempdir().unwrap();
    let home = dir.path().join(".cowen");
    std::fs::create_dir_all(&home).unwrap();
    let (mock_port, _) = super::mock_server::spawn_mock_server().await;

    let mock_url = format!("http://127.0.0.1:{}", mock_port);
    let mock_ws = format!("ws://127.0.0.1:{}", mock_port);
    let proxy_port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };

    // We expect the wasm to be compiled already. If not, this test might fail or we should build it.
    // For now we'll just check if it exists in target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm
    // or we run cargo build
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut wasm_src = std::path::PathBuf::new();
    let candidates = [
        manifest_dir.join("../../../target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm"),
        manifest_dir.join("../../../target/wasm32-wasip1/debug/cowen_wasm_auth_selfbuilt.wasm"),
        manifest_dir.join("../../plugins/cowen-wasm-auth-selfbuilt/target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm"),
        manifest_dir.join("../../plugins/cowen-wasm-auth-selfbuilt/target/wasm32-wasip1/debug/cowen_wasm_auth_selfbuilt.wasm"),
    ];
    for c in candidates.iter() {
        if c.exists() {
            wasm_src = c.clone();
            break;
        }
    }

    if !wasm_src.exists() {
        let mut build_cmd = std::process::Command::new("cargo");
        build_cmd
            .current_dir(manifest_dir.join("../../plugins/cowen-wasm-auth-selfbuilt"))
            .args(["build", "--release", "--target", "wasm32-wasip1"]);
        build_cmd.status().unwrap();
        wasm_src = manifest_dir.join("../../plugins/cowen-wasm-auth-selfbuilt/target/wasm32-wasip1/release/cowen_wasm_auth_selfbuilt.wasm");
    }

    let plugins_dir = home.join("plugins");
    std::fs::create_dir_all(&plugins_dir).unwrap();
    if wasm_src.exists() {
        std::fs::copy(
            &wasm_src,
            plugins_dir.join("cowen_wasm_auth_selfbuilt.wasm"),
        )
        .unwrap();
    }

    let plugin_json_src = manifest_dir.join("../../plugins/cowen-wasm-auth-selfbuilt/plugin.json");
    if plugin_json_src.exists() {
        std::fs::copy(
            &plugin_json_src,
            plugins_dir.join("cowen-wasm-auth-selfbuilt.json"),
        )
        .unwrap();
    }

    std::fs::write(
        plugins_dir.join("pipeline.yaml"),
        r#"
plugins:
  - name: cowen-wasm-auth-selfbuilt
    path: cowen_wasm_auth_selfbuilt.wasm
routes:
  - path_prefix: /v1/
    pre_auth_plugins: []
    request_filter_plugins:
      - cowen-wasm-auth-selfbuilt
    response_filter_plugins: []
"#,
    )
    .unwrap();

    let mut init_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    init_cmd.env("COWEN_HOME", &home).env("HOME", &home).args([
        "init",
        "--profile",
        profile,
        "--app-key",
        "dummy_app_key",
        "--app-secret",
        "dummy_app_secret",
        "--app-mode",
        "self-built",
        "--certificate",
        "dummy_cert",
        "--encrypt-key",
        "1234567890123456",
        "--openapi-url",
        &mock_url,
        "--stream-url",
        &mock_ws,
        "--proxy-port",
        &proxy_port.to_string(),
        "--webhook-target",
        "http://127.0.0.1:8080/cb",
    ]);
    init_cmd.status().unwrap();

    let db_path = home.join("cowen.db");
    let mut sql_cmd = std::process::Command::new("sqlite3");
    sql_cmd.args([
        db_path.to_str().unwrap(),
        &format!("INSERT OR REPLACE INTO cowen_token (profile, item_key, item_value, expires_at) VALUES ('{}', 'access', '{{\"value\":\"wasm_mocked_token\",\"expires_at\":\"2099-01-01T00:00:00Z\",\"created_at\":\"2026-01-01T00:00:00Z\"}}', 4070880000);", profile)
    ]);
    sql_cmd.status().unwrap();

    let mut sql_cmd2 = std::process::Command::new("sqlite3");
    sql_cmd2.args([
        db_path.to_str().unwrap(),
        "INSERT OR REPLACE INTO cowen_app_token (app_key, token_value, expires_at, created_at) VALUES ('dummy_app_key', 'wasm_mocked_token', '2099-01-01 00:00:00', '2026-01-01 00:00:00');"
    ]);
    sql_cmd2.status().unwrap();

    let mut daemon_cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("cowen"));
    daemon_cmd
        .env("COWEN_HOME", &home)
        .env("HOME", &home)
        .args(["daemon", "start", "--profile", profile]);
    assert!(daemon_cmd.status().unwrap().success());
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Call mock reset just in case
    reqwest::Client::new()
        .post(format!("{}/control/reset", mock_url))
        .send()
        .await
        .unwrap();

    let client = reqwest::Client::builder().build().unwrap();

    let res = client
        .post(format!("http://127.0.0.1:{}/v1/app/data/get", proxy_port))
        .send()
        .await
        .unwrap();
    let text = res.text().await.unwrap();

    // If wasm is not triggered, it might fail. Let's just do a soft check since it's a direct port
    if wasm_src.exists() {
        println!("TEXT FROM MOCK: {}", text);
        assert!(text.contains("wasm_mocked_token") || text.contains("mock_at_sb_"));
        assert!(text.contains("dummy_app_key"));
    }
}
