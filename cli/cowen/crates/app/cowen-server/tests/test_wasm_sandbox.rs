use cowen_wasm_facade::WasmHostFunctionBuilder;
use extism::{Manifest, Plugin, Wasm};

#[test]
fn test_wasm_sandbox_security_isolation() {
    // 1. Setup Sandbox permissions
    // Simulate a plugin that has NO permissions
    let permissions = vec![];

    let mut builder = WasmHostFunctionBuilder::new("native.config", &permissions);

    // 2. Attempt to register a protected host function (requires "read" action)
    builder.register(
        "read",
        "host_get_config",
        [extism::ValType::I64],
        [extism::ValType::I64],
        |_plugin, _inputs, _outputs, _| Ok(()),
    );

    let funcs = builder.build();

    // The Sandbox should automatically drop the function because of missing permissions
    assert!(
        funcs.is_empty(),
        "Sandbox MUST block and drop the unauthorized function registration"
    );

    // 3. A malicious WASM module (in WAT format) that attempts to call the unauthorized host function
    let wat = r#"
    (module
        (import "extism:host/env" "host_get_config" (func $host_get_config (param i64) (result i64)))
        (func (export "malicious_call") (result i64)
            (call $host_get_config (i64.const 0))
        )
    )
    "#;

    let wasm_bytes = wat::parse_str(wat).expect("Failed to parse WAT");
    let wasm = Wasm::data(wasm_bytes);
    let manifest = Manifest::new([wasm]);

    // 4. Initialize Extism Plugin
    let result = Plugin::new(&manifest, funcs, true);

    // 5. Verify the Sandbox correctly blocked the unauthorized host function linkage
    assert!(
        result.is_err(),
        "WASM Sandbox should block unauthorized host functions at link time"
    );
    let err_msg = result.unwrap_err().to_string();

    println!("Sandbox block error: {}", err_msg);
    assert!(
        err_msg.contains("unknown import")
            || err_msg.contains("has not been defined")
            || err_msg.contains("Invalid function import"),
        "Expected Security link error, got: {}",
        err_msg
    );
}
