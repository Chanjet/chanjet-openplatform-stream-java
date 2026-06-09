
use std::sync::Arc;
use extism::Function;
use cowen_common::config::Config;

pub mod native_config;
pub mod native_auth;

pub struct CapabilityContext {
    pub profile: String,
    pub config: Config,
    pub capabilities: Arc<cowen_capabilities::CapabilityRegistry>,
}

pub trait HostCapabilityProvider: Send + Sync {
    /// Domain, e.g., "sys.vault" or "sys.http"
    fn domain(&self) -> &'static str;
    
    /// The versions of the capability contract this provider supports generating Host Functions for
    fn supported_versions(&self) -> Vec<&'static str> {
        vec!["1.0.0"] // default to 1.0.0
    }

    /// Check if the requested version satisfies any of the supported versions using semver.
    fn check_version(&self, req_version: &str) -> anyhow::Result<()> {
        let req = semver::VersionReq::parse(req_version)
            .map_err(|e| anyhow::anyhow!("Invalid version requirement '{}': {}", req_version, e))?;
        for &sup in self.supported_versions().iter() {
            if let Ok(ver) = semver::Version::parse(sup) {
                if req.matches(&ver) {
                    return Ok(());
                }
            }
        }
        anyhow::bail!("Unsupported {} version: {}. Supported versions: {:?}", self.domain(), req_version, self.supported_versions());
    }
    
    /// Create Extism host functions for the given version and specific permissions
    fn create_functions(
        &self, 
        version: &str, 
        permissions: &[String], 
        context: &CapabilityContext
    ) -> anyhow::Result<Vec<Function>>;
}

pub fn registry_supported_versions() -> std::collections::HashMap<&'static str, Vec<&'static str>> {
    let mut map = std::collections::HashMap::new();
    let providers: Vec<Box<dyn HostCapabilityProvider>> = vec![
        Box::new(SysBaseProvider),
        Box::new(native_config::NativeConfigProvider),
        Box::new(native_auth::NativeAuthProvider),
    ];
    for p in providers {
        map.insert(p.domain(), p.supported_versions());
    }
    
    // The Wasm facade must align with the gRPC facade to pass the FacadeManifest check.
    // Since Wasm currently doesn't implement these directly as Extism host functions,
    // we declare their versions here to maintain contract alignment.
    map.insert("native.api.registry", vec!["1.0.0"]);
    map.insert("native.system", vec!["1.0.0"]);
    map.insert("native.dlq", vec!["1.0.0"]);
    map.insert("native.search", vec!["1.0.0"]);
    
    map
}

pub struct SysBaseProvider;

impl HostCapabilityProvider for SysBaseProvider {
    fn domain(&self) -> &'static str {
        "sys.base"
    }

    fn create_functions(
        &self,
        version: &str,
        permissions: &[String],
        context: &CapabilityContext,
    ) -> anyhow::Result<Vec<Function>> {
        self.check_version(version)?;

        let mut builder = WasmHostFunctionBuilder::new(self.domain(), permissions);

        let profile_clone = context.profile.clone();
        builder.register(
            "",
            "host_get_profile",
            [],
            [extism::ValType::I64],
            move |plugin: &mut extism::CurrentPlugin, _inputs, outputs, _| {
                let mem = plugin.memory_new(profile_clone.as_bytes())?;
                outputs[0] = plugin.memory_to_val(mem);
                Ok(())
            },
        );

        let app_key_clone = context.config.app_key.clone();
        builder.register(
            "",
            "host_get_app_key",
            [],
            [extism::ValType::I64],
            move |plugin: &mut extism::CurrentPlugin, _inputs, outputs, _| {
                let mem = plugin.memory_new(app_key_clone.as_bytes())?;
                outputs[0] = plugin.memory_to_val(mem);
                Ok(())
            },
        );

        Ok(builder.build())
    }
}

pub struct WasmHostFunctionBuilder<'a> {
    domain: &'static str,
    permissions: &'a [String],
    funcs: Vec<extism::Function>,
}

impl<'a> WasmHostFunctionBuilder<'a> {
    pub fn new(domain: &'static str, permissions: &'a [String]) -> Self {
        Self {
            domain,
            permissions,
            funcs: vec![],
        }
    }

    pub fn register<I, O, F>(
        &mut self,
        action: &str,
        name: impl Into<String>,
        inputs: I,
        outputs: O,
        f: F,
    ) where
        I: IntoIterator<Item = extism::ValType>,
        O: IntoIterator<Item = extism::ValType>,
        F: Fn(&mut extism::CurrentPlugin, &[extism::Val], &mut [extism::Val], extism::UserData<()>) -> Result<(), extism::Error> + Send + Sync + 'static,
    {
        // For sys.base, we skip permission checks (or we could define it properly)
        // Actually, if we pass "" as action, has_policy_permission will allow it if empty policy
        if cowen_capabilities::rbac::has_policy_permission(self.permissions, self.domain, action) {
            self.funcs.push(extism::Function::new(
                name,
                inputs,
                outputs,
                extism::UserData::new(()),
                f,
            ));
        }
    }

    pub fn build(self) -> Vec<extism::Function> {
        self.funcs
    }
}
