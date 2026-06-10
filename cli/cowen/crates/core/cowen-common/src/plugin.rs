use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmInterceptorContribution {
    pub name: String,
    #[serde(default)]
    pub app_modes: Vec<String>,
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub requested_permissions: Vec<String>,
    #[serde(default)]
    pub required_capabilities: std::collections::HashMap<String, String>,
    pub allowed_commands: HashSet<String>,
    #[serde(default)]
    pub wasm_interceptors: Vec<WasmInterceptorContribution>,
}

impl PluginManifest {
    pub fn load_from_json(plugin_name: &str, json_path: &std::path::Path) -> anyhow::Result<Self> {
        let mut manifest_val: Option<serde_json::Value> = None;
        if json_path.exists() {
            if let Ok(json_str) = std::fs::read_to_string(json_path) {
                manifest_val = serde_json::from_str::<serde_json::Value>(&json_str).ok();
            }
        }
        Self::parse_manifest(plugin_name, manifest_val)
    }

    pub fn load(plugin_name: &str) -> anyhow::Result<Self> {
        let plugins_dir = crate::config::get_app_dir().join("plugins");
        let expected_path = if cfg!(target_os = "windows") {
            plugins_dir.join(format!("{}.exe", plugin_name))
        } else {
            plugins_dir.join(plugin_name)
        };

        // For Wasm, it could just be .wasm, but the metadata is usually in .json anyway
        // or embedded in the bundle.
        let bundle_path = expected_path.with_extension("bundle");
        let json_path = plugins_dir.join(format!("{}.json", plugin_name));

        let mut manifest_val: Option<serde_json::Value> = None;

        if bundle_path.exists() {
            if let Ok(bundle_str) = std::fs::read_to_string(&bundle_path) {
                if let Ok(bundle) = serde_json::from_str::<serde_json::Value>(&bundle_str) {
                    manifest_val = bundle.get("manifest").cloned();
                }
            }
        } else if json_path.exists() {
            if let Ok(json_str) = std::fs::read_to_string(&json_path) {
                manifest_val = serde_json::from_str::<serde_json::Value>(&json_str).ok();
            }
        }

        Self::parse_manifest(plugin_name, manifest_val)
    }

    fn parse_permissions(m: &serde_json::Value, scopes: &mut Vec<String>) {
        if let Some(perms) = m.get("requested_permissions").and_then(|p| p.as_object()) {
            for (k, v) in perms {
                if v.as_bool().unwrap_or(false) {
                    scopes.push(k.clone());
                }
            }
        } else if let Some(privs) = m.get("required_privileges").and_then(|p| p.as_array()) {
            for p in privs {
                if let Some(s) = p.as_str() {
                    scopes.push(s.to_string());
                }
            }
        }
    }

    fn parse_capabilities(m: &serde_json::Value, required_capabilities: &mut std::collections::HashMap<String, String>) {
        if let Some(caps) = m.get("required_capabilities").and_then(|c| c.as_object()) {
            for (k, v) in caps {
                if let Some(s) = v.as_str() {
                    required_capabilities.insert(k.clone(), s.to_string());
                }
            }
        }
    }

    fn parse_contributes(
        m: &serde_json::Value,
        allowed_commands: &mut HashSet<String>,
        wasm_interceptors: &mut Vec<WasmInterceptorContribution>,
    ) {
        if let Some(contributes) = m.get("contributes").and_then(|c| c.as_object()) {
            if let Some(cmds) = contributes.get("cli_commands").and_then(|c| c.as_array()) {
                for cmd in cmds {
                    if let Some(name) = cmd.get("name").and_then(|n| n.as_str()) {
                        allowed_commands.insert(name.to_string());
                    }
                }
            }
            if let Some(interceptors) = contributes
                .get("wasm_interceptors")
                .and_then(|c| c.as_array())
            {
                for interceptor in interceptors {
                    if let Ok(contribution) = serde_json::from_value::<
                        WasmInterceptorContribution,
                    >(interceptor.clone())
                    {
                        wasm_interceptors.push(contribution);
                    }
                }
            }
        }
    }

    fn parse_manifest(
        plugin_name: &str,
        manifest_val: Option<serde_json::Value>,
    ) -> anyhow::Result<Self> {
        let mut scopes = vec![];
        let mut required_capabilities = std::collections::HashMap::new();
        let mut allowed_commands = HashSet::new();
        let mut wasm_interceptors = Vec::new();

        if let Some(m) = &manifest_val {
            Self::parse_permissions(m, &mut scopes);
            Self::parse_capabilities(m, &mut required_capabilities);
            Self::parse_contributes(m, &mut allowed_commands, &mut wasm_interceptors);
        } else {
            // No manifest found, default empty permissions
            tracing::warn!("No plugin.json or bundle found for {}", plugin_name);
        }

        Ok(Self {
            name: plugin_name.to_string(),
            requested_permissions: scopes,
            required_capabilities,
            allowed_commands,
            wasm_interceptors,
        })
    }
}
