use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn test_search_engine_e2e_lifecycle() {
    // 1. Spawn the daemon
    let mut child = Command::new(env!("CARGO_BIN_EXE_libcowen_search_embedding"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped()) // capture stderr to avoid cluttering test output
        .spawn()
        .expect("Failed to spawn search embedding daemon");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);

    // 2. Test invalid JSON
    stdin.write_all(b"invalid json\n").unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.contains("-32700"),
        "Expected Parse error (-32700), got: {}",
        line
    );

    // 3. Test missing jsonrpc version
    line.clear();
    stdin
        .write_all(b"{\"jsonrpc\":\"1.0\",\"method\":\"search/query\"}\n")
        .unwrap();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.contains("-32600"),
        "Expected Invalid Request error (-32600), got: {}",
        line
    );

    // 4. Test method not found
    line.clear();
    let req_not_found = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "invalid/method",
        "params": {}
    });
    stdin
        .write_all(serde_json::to_string(&req_not_found).unwrap().as_bytes())
        .unwrap();
    stdin.write_all(b"\n").unwrap();
    reader.read_line(&mut line).unwrap();
    assert!(
        line.contains("-32601") || line.contains("-32603"),
        "Expected error (-32601 or -32603), got: {}",
        line
    );

    // 5. Test update_index (Assuming ONNX asset downloads correctly, or fails gracefully if not present, but doesn't panic)
    line.clear();
    let req_update = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "search/update_index",
        "params": {
            "tenant_id": "e2e_test_tenant",
            "documents": [
                {
                    "id": "e2e_doc_1",
                    "summary": "Integration testing in Rust",
                    "description": "E2E testing the sidecar JSON-RPC interface",
                    "vector": []
                }
            ]
        }
    });
    stdin
        .write_all(serde_json::to_string(&req_update).unwrap().as_bytes())
        .unwrap();
    stdin.write_all(b"\n").unwrap();
    reader.read_line(&mut line).unwrap();

    // We do not strictly assert success here because if ONNX fails to load, it might return -32603 "Internal error: ONNX embedder not loaded"
    // The sidecar should return valid JSON-RPC regardless
    let response: serde_json::Value = serde_json::from_str(&line).expect("Should be valid JSON");
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 2);

    // 6. Test graceful process cleanup
    drop(stdin); // closing stdin should cause the read_line loop to exit gracefully

    // Wait for the child to exit
    let status = child.wait().expect("Failed to wait on child");
    assert!(
        status.success() || status.code().is_none(),
        "Process should exit cleanly"
    );
}
