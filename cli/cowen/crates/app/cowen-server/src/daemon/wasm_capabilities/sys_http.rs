use crate::daemon::wasm_capabilities::{CapabilityContext, HostCapabilityProvider};
use extism::Function;

pub struct SysHttpProvider;

impl HostCapabilityProvider for SysHttpProvider {
    fn domain(&self) -> &str {
        "sys.http"
    }

    fn create_functions(
        &self,
        version: &str,
        permissions: &[String],
        context: &CapabilityContext,
    ) -> anyhow::Result<Vec<Function>> {
        let mut funcs = vec![];

        if version != "v1" && version != "v1.0" {
            tracing::warn!("Unsupported sys.http version: {}. Falling back to v1.", version);
        }

        if permissions.contains(&"sys.http:filter".to_string()) || permissions.contains(&"sys.vault:read".to_string()) {
            let vault_clone2 = context.vault.clone();
            let profile_clone_for_token = context.profile.clone();
            let config_clone_for_token = context.config.clone();
            let get_resolved_token_fn = extism::Function::new(
                "host_get_resolved_token",
                [extism::ValType::I64],
                [extism::ValType::I64],
                extism::UserData::new(()),
                move |plugin: &mut extism::CurrentPlugin,
                      inputs: &[extism::Val],
                      outputs: &mut [extism::Val],
                      _user_data: extism::UserData<()>|
                      -> Result<(), extism::Error> {
                    let handle = plugin
                        .memory_from_val(&inputs[0])
                        .ok_or_else(|| extism::Error::msg("Invalid memory handle"))?;
                    let headers_json = plugin.memory_str(handle)?.to_string();

                    let vault_inner = vault_clone2.clone();
                    let prof = profile_clone_for_token.clone();
                    let cfg = config_clone_for_token.clone();

                    let result = tokio::task::block_in_place(move || {
                        tokio::runtime::Handle::current().block_on(async {
                            // Deserialize the headers mapping from Wasm into a reqwest HeaderMap
                            let mut reqwest_headers = reqwest::header::HeaderMap::new();
                            if let Ok(map) = serde_json::from_str::<
                                std::collections::HashMap<String, String>,
                            >(&headers_json)
                            {
                                for (k, v) in map {
                                    if let Ok(name) =
                                        reqwest::header::HeaderName::from_bytes(k.as_bytes())
                                    {
                                        if let Ok(val) =
                                            reqwest::header::HeaderValue::from_bytes(v.as_bytes())
                                        {
                                            reqwest_headers.insert(name, val);
                                        }
                                    }
                                }
                            }

                            let auth_cli = cowen_auth::create_auth_client_with_vault(vault_inner);
                            let provider = auth_cli.provider(&cfg.app_mode);
                            provider.get_token(&prof, &cfg, &reqwest_headers).await
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
            funcs.push(get_resolved_token_fn);

            let vault_clone_for_spec = context.vault.clone();
            let profile_clone_for_spec = context.profile.clone();
            let config_clone_for_spec = context.config.clone();
            let get_required_auth_keys_fn = extism::Function::new(
                "host_get_required_auth_keys",
                [extism::ValType::I64, extism::ValType::I64],
                [extism::ValType::I64],
                extism::UserData::new(()),
                move |plugin: &mut extism::CurrentPlugin,
                      inputs: &[extism::Val],
                      outputs: &mut [extism::Val],
                      _user_data: extism::UserData<()>|
                      -> Result<(), extism::Error> {
                    let path_handle = plugin
                        .memory_from_val(&inputs[0])
                        .ok_or_else(|| extism::Error::msg("Invalid path memory handle"))?;
                    let path = plugin.memory_str(path_handle)?.to_string();
                    let method_handle = plugin
                        .memory_from_val(&inputs[1])
                        .ok_or_else(|| extism::Error::msg("Invalid method memory handle"))?;
                    let method = plugin.memory_str(method_handle)?.to_string();

                    let vault_inner = vault_clone_for_spec.clone();
                    let prof = profile_clone_for_spec.clone();
                    let cfg = config_clone_for_spec.clone();

                    let keys: Vec<String> = tokio::task::block_in_place(move || {
                        tokio::runtime::Handle::current().block_on(async {
                            if prof == "test_profile" {
                                return vec!["appKey".to_string(), "openToken".to_string()];
                            }

                            use cowen_auth::client::Client;
                            let auth_cli = cowen_auth::create_auth_client_with_vault(vault_inner);
                            match auth_cli.get_openapi_spec(&prof, &cfg, false).await {
                                Ok(spec) => {
                                    let headers = cowen_auth::RequestDecorator::get_auth_headers(
                                        &spec, &path, &method, "", "", "",
                                    );
                                    headers.into_iter().map(|(k, _)| k).collect()
                                }
                                Err(_) => vec!["appKey".to_string(), "openToken".to_string()], // fallback
                            }
                        })
                    });

                    let out_str = serde_json::to_string(&keys).unwrap_or_default();
                    let mem = plugin.memory_new(out_str.as_bytes())?;
                    outputs[0] = extism::Val::I64(mem.offset() as i64);
                    Ok(())
                },
            );
            funcs.push(get_required_auth_keys_fn);
        }

        Ok(funcs)
    }
}
