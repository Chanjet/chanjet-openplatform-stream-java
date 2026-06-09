use crate::daemon::wasm_capabilities::{CapabilityContext, HostCapabilityProvider};
use extism::Function;

pub struct SysVaultProvider;

impl HostCapabilityProvider for SysVaultProvider {
    fn domain(&self) -> &str {
        "sys.vault"
    }

    fn create_functions(
        &self,
        version: &str,
        permissions: &[String],
        context: &CapabilityContext,
    ) -> anyhow::Result<Vec<Function>> {
        let mut funcs = vec![];

        if version != "v1" && version != "v1.0" {
            tracing::warn!("Unsupported sys.vault version: {}. Falling back to v1.", version);
        }

        if permissions.contains(&"sys.vault:read".to_string()) {
            let vault_clone = context.vault.clone();
            let get_app_ticket_fn = extism::Function::new(
                "host_vault_get_app_ticket",
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
                    let app_key_str = plugin.memory_str(handle)?;
                    let app_key = app_key_str.to_string();

                    let vault_inner = vault_clone.clone();
                    let result = tokio::task::block_in_place(move || {
                        tokio::runtime::Handle::current()
                            .block_on(async { vault_inner.get_app_ticket(&app_key).await })
                    });

                    let out_str = match result {
                        Ok(ticket) => serde_json::to_string(&ticket).unwrap_or_default(),
                        Err(_) => "".to_string(),
                    };

                    let mem = plugin.memory_new(out_str.as_bytes())?;
                    outputs[0] = extism::Val::I64(mem.offset() as i64);
                    Ok(())
                },
            );
            funcs.push(get_app_ticket_fn);

            let config_clone_for_secret = context.config.clone();
            let get_app_secret_fn = extism::Function::new(
                "host_get_app_secret",
                [],
                [extism::ValType::I64],
                extism::UserData::new(()),
                move |plugin: &mut extism::CurrentPlugin,
                      _inputs: &[extism::Val],
                      outputs: &mut [extism::Val],
                      _user_data: extism::UserData<()>|
                      -> Result<(), extism::Error> {
                    let mem = plugin.memory_new(config_clone_for_secret.app_secret.as_bytes())?;
                    outputs[0] = extism::Val::I64(mem.offset() as i64);
                    Ok(())
                },
            );
            funcs.push(get_app_secret_fn);
        }

        Ok(funcs)
    }
}
