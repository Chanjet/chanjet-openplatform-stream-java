use crate::daemon::wasm_capabilities::{CapabilityContext, HostCapabilityProvider};
use extism::Function;

pub struct SysHttpProvider;

impl HostCapabilityProvider for SysHttpProvider {
    fn domain(&self) -> &'static str {
        "sys.http"
    }

    fn create_functions(
        &self,
        version: &str,
        permissions: &[String],
        context: &CapabilityContext,
    ) -> anyhow::Result<Vec<Function>> {
        self.check_version(version)?;

        let mut builder = crate::daemon::wasm_capabilities::WasmHostFunctionBuilder::new(self.domain(), permissions);

        let caps = context.capabilities.clone();
        let profile_clone_for_token = context.profile.clone();
        let config_clone_for_token = context.config.clone();
        
        builder.register(
            "filter",
            "host_get_resolved_token",
            [extism::ValType::I64],
            [extism::ValType::I64],
            move |plugin: &mut extism::CurrentPlugin, inputs, outputs, _| {
                let handle = plugin
                    .memory_from_val(&inputs[0])
                    .ok_or_else(|| extism::Error::msg("Invalid memory handle"))?;
                let headers_json = plugin.memory_str(handle)?.to_string();

                let caps_inner = caps.clone();
                let prof = profile_clone_for_token.clone();
                let cfg = config_clone_for_token.clone();

                let result = tokio::task::block_in_place(move || {
                    tokio::runtime::Handle::current().block_on(async {
                        let mut reqwest_headers = reqwest::header::HeaderMap::new();
                        if let Ok(map) = serde_json::from_str::<std::collections::HashMap<String, String>>(&headers_json) {
                            for (k, v) in map {
                                if let Ok(name) = reqwest::header::HeaderName::from_bytes(k.as_bytes()) {
                                    if let Ok(val) = reqwest::header::HeaderValue::from_bytes(v.as_bytes()) {
                                        reqwest_headers.insert(name, val);
                                    }
                                }
                            }
                        }
                        caps_inner.sys_http.get_resolved_token(&prof, &cfg, &reqwest_headers).await
                    })
                });

                let out_str = match result {
                    Ok(token) => serde_json::to_string(&token).unwrap_or_default(),
                    Err(_) => "".to_string(),
                };

                let mem = plugin.memory_new(out_str.as_bytes())?;
                outputs[0] = extism::Val::I64(mem.offset() as i64);
                Ok(())
            },
        );

        let caps_for_spec = context.capabilities.clone();
        let profile_clone_for_spec = context.profile.clone();
        let config_clone_for_spec = context.config.clone();
        
        builder.register(
            "filter",
            "host_get_required_auth_keys",
            [extism::ValType::I64, extism::ValType::I64],
            [extism::ValType::I64],
            move |plugin: &mut extism::CurrentPlugin, inputs, outputs, _| {
                let path_handle = plugin
                    .memory_from_val(&inputs[0])
                    .ok_or_else(|| extism::Error::msg("Invalid path memory handle"))?;
                let path = plugin.memory_str(path_handle)?.to_string();
                let method_handle = plugin
                    .memory_from_val(&inputs[1])
                    .ok_or_else(|| extism::Error::msg("Invalid method memory handle"))?;
                let method = plugin.memory_str(method_handle)?.to_string();

                let caps_inner = caps_for_spec.clone();
                let prof = profile_clone_for_spec.clone();
                let cfg = config_clone_for_spec.clone();

                let keys = tokio::task::block_in_place(move || {
                    tokio::runtime::Handle::current().block_on(async {
                        caps_inner.sys_http.get_required_auth_keys(&prof, &cfg, &path, &method).await
                    })
                }).unwrap_or_default();

                let out_str = serde_json::to_string(&keys).unwrap_or_default();
                let mem = plugin.memory_new(out_str.as_bytes())?;
                outputs[0] = extism::Val::I64(mem.offset() as i64);
                Ok(())
            },
        );

        Ok(builder.build())
    }
}
