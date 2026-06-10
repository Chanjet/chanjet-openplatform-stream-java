use crate::{CapabilityContext, HostCapabilityProvider};
use extism::Function;

pub struct NativeAuthProvider;

impl NativeAuthProvider {
    fn register_get_resolved_token(
        &self,
        builder: &mut crate::WasmHostFunctionBuilder,
        context: &CapabilityContext,
    ) {
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
                        caps_inner.native_auth.get_resolved_token(None, &prof, &cfg, &reqwest_headers).await
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
    }

    fn register_get_required_auth_keys(
        &self,
        builder: &mut crate::WasmHostFunctionBuilder,
        context: &CapabilityContext,
    ) {
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
                        caps_inner.native_auth.get_required_auth_keys(None, &prof, &cfg, &path, &method).await
                    })
                }).unwrap_or_default();

                let out_str = serde_json::to_string(&keys).unwrap_or_default();
                let mem = plugin.memory_new(out_str.as_bytes())?;
                outputs[0] = extism::Val::I64(mem.offset() as i64);
                Ok(())
            },
        );
    }
}

impl HostCapabilityProvider for NativeAuthProvider {
    fn domain(&self) -> &'static str {
        "native.auth"
    }

    fn create_functions(
        &self,
        version: &str,
        permissions: &[String],
        context: &CapabilityContext,
    ) -> anyhow::Result<Vec<Function>> {
        self.check_version(version)?;

        let mut builder = crate::WasmHostFunctionBuilder::new(self.domain(), permissions);

        self.register_get_resolved_token(&mut builder, context);
        self.register_get_required_auth_keys(&mut builder, context);

        Ok(builder.build())
    }
}
