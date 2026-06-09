use std::sync::Arc;
use extism::Function;
use cowen_common::vault::Vault;
use cowen_common::config::Config;

pub mod sys_vault;
pub mod sys_http;

pub struct CapabilityContext {
    pub vault: Arc<dyn Vault>,
    pub profile: String,
    pub config: Config,
}

pub trait HostCapabilityProvider: Send + Sync {
    /// Domain, e.g., "sys.vault" or "sys.http"
    fn domain(&self) -> &str;
    
    /// Create Extism host functions for the given version and specific permissions
    fn create_functions(
        &self, 
        version: &str, 
        permissions: &[String], 
        context: &CapabilityContext
    ) -> anyhow::Result<Vec<Function>>;
}

pub struct SysBaseProvider;

impl HostCapabilityProvider for SysBaseProvider {
    fn domain(&self) -> &str {
        "sys.base"
    }

    fn create_functions(
        &self,
        _version: &str,
        _permissions: &[String],
        context: &CapabilityContext,
    ) -> anyhow::Result<Vec<Function>> {
        let mut funcs = vec![];

        let profile_clone = context.profile.clone();
        let get_profile_fn = extism::Function::new(
            "host_get_profile",
            [],
            [extism::ValType::I64],
            extism::UserData::new(()),
            move |plugin: &mut extism::CurrentPlugin,
                  _inputs: &[extism::Val],
                  outputs: &mut [extism::Val],
                  _user_data: extism::UserData<()>|
                  -> Result<(), extism::Error> {
                let mem = plugin.memory_new(profile_clone.as_bytes())?;
                outputs[0] = plugin.memory_to_val(mem);
                Ok(())
            },
        );
        funcs.push(get_profile_fn);

        let app_key_clone = context.config.app_key.clone();
        let get_app_key_fn = extism::Function::new(
            "host_get_app_key",
            [],
            [extism::ValType::I64],
            extism::UserData::new(()),
            move |plugin: &mut extism::CurrentPlugin,
                  _inputs: &[extism::Val],
                  outputs: &mut [extism::Val],
                  _user_data: extism::UserData<()>|
                  -> Result<(), extism::Error> {
                let mem = plugin.memory_new(app_key_clone.as_bytes())?;
                outputs[0] = plugin.memory_to_val(mem);
                Ok(())
            },
        );
        funcs.push(get_app_key_fn);

        Ok(funcs)
    }
}
