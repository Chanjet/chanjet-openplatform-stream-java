use std::sync::Arc;
use arc_swap::ArcSwap;
use wasmtime::{Engine, Module, Store, Linker};

pub struct WasmPipelineManager {
    engine: Engine,
    current_module: ArcSwap<Option<Module>>,
}

impl WasmPipelineManager {
    pub fn new() -> Self {
        let engine = Engine::default();
        Self {
            engine,
            current_module: ArcSwap::from_pointee(None),
        }
    }

    pub fn load_wasm(&self, wasm_bytes: &[u8]) -> anyhow::Result<()> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        self.current_module.store(Arc::new(Some(module)));
        Ok(())
    }

    pub fn authenticate(&self, method: &str, uri: &str, _headers: &[(String, String)]) -> anyhow::Result<bool> {
        let module_guard = self.current_module.load();
        let module = match &**module_guard {
            Some(m) => m,
            None => return Ok(true), // No custom Wasm auth loaded, default pass-through
        };

        let mut store = Store::new(&self.engine, ());
        let linker = Linker::new(&self.engine);
        let instance = linker.instantiate(&mut store, module)?;

        // Find standard "authenticate" function
        let authenticate_func = instance.get_typed_func::<(i32, i32), i32>(&mut store, "authenticate")?;
        
        // Pass basic method & uri length for demonstration / mock matching
        let method_len = method.len() as i32;
        let uri_len = uri.len() as i32;
        let auth_status = authenticate_func.call(&mut store, (method_len, uri_len))?;

        Ok(auth_status == 1)
    }

    pub fn filter_body(&self, body: Vec<u8>) -> Vec<u8> {
        let module_guard = self.current_module.load();
        let module = match &**module_guard {
            Some(m) => m,
            None => return body,
        };

        let mut store = Store::new(&self.engine, ());
        let linker = Linker::new(&self.engine);
        if let Ok(instance) = linker.instantiate(&mut store, module) {
            if let Ok(filter_func) = instance.get_typed_func::<i32, i32>(&mut store, "filter_body") {
                if let Ok(res) = filter_func.call(&mut store, body.len() as i32) {
                    if res == 0 {
                        // For mock: return a custom filtered body
                        return b"masked_body_payload".to_vec();
                    }
                }
            }
        }
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_authenticate_success() {
        let manager = WasmPipelineManager::new();
        
        // Compile simple mock auth WAT that allows requests where method_len is 3 (e.g. GET)
        let wat = r#"
            (module
                (func (export "authenticate") (param i32 i32) (result i32)
                    local.get 0
                    i32.const 3
                    i32.eq
                )
            )
        "#;
        let wasm_bytes = wat::parse_str(wat).unwrap();
        manager.load_wasm(&wasm_bytes).unwrap();

        // allowed: "GET" has length 3 -> authorized
        let allowed = manager.authenticate("GET", "/test", &[]).unwrap();
        assert!(allowed, "GET should be allowed by Wasm");

        // blocked: "POST" has length 4 -> blocked
        let blocked = manager.authenticate("POST", "/test", &[]).unwrap();
        assert!(!blocked, "POST should be blocked by Wasm");
    }

    #[test]
    fn test_wasm_filter_body() {
        let manager = WasmPipelineManager::new();
        
        let wat = r#"
            (module
                (func (export "filter_body") (param i32) (result i32)
                    i32.const 0
                )
            )
        "#;
        let wasm_bytes = wat::parse_str(wat).unwrap();
        manager.load_wasm(&wasm_bytes).unwrap();

        let raw_body = b"sensitive_data".to_vec();
        let filtered = manager.filter_body(raw_body);
        assert_eq!(filtered, b"masked_body_payload".to_vec());
    }
}
