use crate::{CapabilityContext, HostCapabilityProvider};
use extism::Function;

pub struct NativeConfigProvider;

impl HostCapabilityProvider for NativeConfigProvider {
    fn domain(&self) -> &'static str {
        "native.config"
    }

    fn create_functions(
        &self,
        ver: &str,
        perms: &[String],
        ctx: &CapabilityContext,
    ) -> anyhow::Result<Vec<Function>> {
        self.check_version(ver)?;

        let mut builder = crate::WasmHostFunctionBuilder::new(self.domain(), perms);

        let caps = ctx.capabilities.clone();
        builder.register(
            "read",
            "host_vault_get_app_ticket",
            [extism::ValType::I64],
            [extism::ValType::I64],
            move |plugin: &mut extism::CurrentPlugin, inputs, outputs, _| {
                let handle = plugin
                    .memory_from_val(&inputs[0])
                    .ok_or_else(|| extism::Error::msg("Invalid memory handle"))?;
                let app_key_str = plugin.memory_str(handle)?;
                let app_key = app_key_str.to_string();

                let caps_inner = caps.clone();
                let result = tokio::task::block_in_place(move || {
                    tokio::runtime::Handle::current().block_on(async {
                        caps_inner
                            .native_config
                            .get_app_ticket(None, &app_key)
                            .await
                    })
                });

                let out_str = match result {
                    Ok(Some(ticket)) => serde_json::to_string(&ticket).unwrap_or_default(),
                    _ => "".to_string(),
                };

                let mem = plugin.memory_new(out_str.as_bytes())?;
                outputs[0] = extism::Val::I64(mem.offset() as i64);
                Ok(())
            },
        );

        let caps_for_secret = ctx.capabilities.clone();
        let profile = ctx.profile.clone();
        builder.register(
            "read",
            "host_get_app_secret",
            [],
            [extism::ValType::I64],
            move |plugin: &mut extism::CurrentPlugin, _inputs, outputs, _| {
                let caps_inner = caps_for_secret.clone();
                let profile_inner = profile.clone();
                let secret = tokio::task::block_in_place(move || {
                    tokio::runtime::Handle::current().block_on(async {
                        caps_inner
                            .native_config
                            .get_app_secret(None, &profile_inner)
                            .await
                    })
                })
                .unwrap_or_default();

                let mem = plugin.memory_new(secret.as_bytes())?;
                outputs[0] = extism::Val::I64(mem.offset() as i64);
                Ok(())
            },
        );

        Ok(builder.build())
    }
}
