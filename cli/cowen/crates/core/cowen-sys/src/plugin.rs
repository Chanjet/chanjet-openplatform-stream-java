use cowen_plugin::{JsonRpcRequest, JsonRpcResponse};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};

use std::sync::Mutex;

pub struct PluginLoader {
    path: PathBuf,
    manifest: cowen_infra::pki::PluginManifest,
}

impl PluginLoader {
    pub fn new<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let p = path.as_ref();

        if !is_secure_plugin_path(p) {
            return Err(anyhow::anyhow!(
                "Plugin path is insecure (wrong owner or world-writable)"
            ));
        }

        cowen_infra::pki::verify_plugin_bundle(p)?;

        let is_dev = std::env::var("COWEN_DEV_MODE").unwrap_or_default() == "1";

        let manifest = if is_dev {
            let bundle_path = p.with_extension("bundle");
            if bundle_path.exists() {
                let bundle_str = std::fs::read_to_string(&bundle_path)?;
                let bundle: cowen_infra::pki::SignatureBundle = serde_json::from_str(&bundle_str)?;
                bundle.manifest
            } else {
                let stem = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let clean_stem = stem.replace("libcowen_", "").replace("cowen_", "");
                let camel = clean_stem
                    .split('_')
                    .flat_map(|s| s.split('-'))
                    .map(|s| {
                        let mut c = s.chars();
                        match c.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        }
                    })
                    .collect::<String>();
                let deduced_capability = format!("{}Provider", camel);

                let mut capabilities = vec![deduced_capability.clone()];
                let mut required_privileges = vec![];

                if stem.contains("search") || stem.contains("embedding") {
                    capabilities.push("SearchProvider".to_string());
                    required_privileges.push("sys.fs:cache_access".to_string());
                    required_privileges.push("sys.network:fetch_asset".to_string());
                    required_privileges.push("sys.cpu:compute_heavy".to_string());
                }

                cowen_infra::pki::PluginManifest {
                    name: stem,
                    version: "dev".to_string(),
                    binary_hash: String::new(),
                    transport: Some("stdio".to_string()),
                    capabilities,
                    required_privileges,
                    contributes: None,
                }
            }
        } else {
            let bundle_path = p.with_extension("bundle");
            let bundle_str = std::fs::read_to_string(&bundle_path)?;
            let bundle: cowen_infra::pki::SignatureBundle = serde_json::from_str(&bundle_str)?;
            bundle.manifest
        };

        Ok(Self {
            path: p.to_path_buf(),
            manifest,
        })
    }

    pub fn manifest(&self) -> &cowen_infra::pki::PluginManifest {
        &self.manifest
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Declarative check for supported traits in Phase 1 (0-FFI).
    pub fn supports_trait(&self, trait_name: &str) -> bool {
        // Strict blocking of wildcard "all" permission to prevent capability abuse
        if self.manifest.capabilities.iter().any(|p| p == "all") {
            tracing::warn!("⚠️  Blocking plugin '{}': wildcard 'all' permission is deprecated and strictly forbidden.", self.manifest.name);
            eprintln!("⚠️  Blocking plugin '{}': wildcard 'all' permission is deprecated and strictly forbidden. Please re-sign the plugin using specific capabilities.", self.manifest.name);
            return false;
        }

        // 1. Check legacy capabilities
        if self.manifest.capabilities.iter().any(|p| p == trait_name) {
            return true;
        }

        // 2. Check contributes.providers
        if let Some(contributes) = &self.manifest.contributes {
            if contributes.providers.iter().any(|p| {
                p.provider_type == trait_name
                    || (trait_name == "SearchProvider" && p.provider_type == "SearchEmbedding")
            }) {
                return true;
            }
        }

        // 3. Name signature fallback for legacy plugins without contributes
        if trait_name == "SearchProvider"
            && (self.manifest.name.contains("search") || self.manifest.name.contains("embedding"))
        {
            return true;
        }

        false
    }

    pub fn verify_identity(&self, target_slot: &str) -> bool {
        self.supports_trait(target_slot)
    }

    pub fn enforce_privilege(&self, privilege: &str) -> bool {
        if self
            .manifest
            .required_privileges
            .iter()
            .any(|p| p == privilege)
        {
            return true;
        }
        // Standard privilege auto-grant for SearchProvider in dev/legacy modes
        if (privilege == "sys.fs:cache_access"
            || privilege == "sys.network:fetch_asset"
            || privilege == "sys.cpu:compute_heavy")
            && self.supports_trait("SearchProvider")
        {
            return true;
        }
        false
    }
}

/// Generic Stdio JSON-RPC Client for Rust standalone Sidecar processes (Phase 2)
pub struct RpcPluginClient {
    child: Mutex<Option<Child>>,
    binary_path: PathBuf,
    tenant_id: String,
    bridge_token: String,
    ipc_token: Option<String>,
}

impl RpcPluginClient {
    pub fn new(binary_path: PathBuf, tenant_id: String, ipc_token: Option<String>) -> Self {
        // Generate a cryptographically secure-ish random协商 Token
        let bridge_token = format!("{:x}", uuid::Uuid::new_v4().simple());

        Self {
            child: Mutex::new(None),
            binary_path,
            tenant_id,
            bridge_token,
            ipc_token,
        }
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    /// Sends a JSON-RPC method call to the Sidecar process via Stdio.
    pub fn call_tool(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let mut child_guard = self.child.lock().unwrap();

        // Spawn the Rust Sidecar process on demand if not already running
        if child_guard.is_none() {
            tracing::info!(target: "sys", "Spawning Rust Sidecar: {:?}", self.binary_path);

            // Safe privilege audit
            let mut compute_heavy = false;
            if let Ok(loader) = PluginLoader::new(&self.binary_path) {
                compute_heavy = loader.enforce_privilege("ComputeHeavy");
            }

            let mut allowed_roots = vec![];
            if let Ok(workspace) = std::env::var("COWEN_WORKSPACE") {
                allowed_roots.push(PathBuf::from(workspace));
            } else if let Ok(cwd) = std::env::current_dir() {
                allowed_roots.push(cwd);
            }

            let mut cmd = crate::create_sandboxed_command(
                &self.binary_path,
                &cowen_infra::path::get_app_dir(),
                &allowed_roots,
            );
            cmd.stdin(Stdio::piped())
                .stdout(Stdio::piped())
                // Safe credential injection in-memory (Option A requirement)
                .env("COWEN_BRIDGE_TOKEN", &self.bridge_token);

            // Inject JWT for TCP IPC (Phase 1 iteration)
            if let Some(token) = &self.ipc_token {
                cmd.env("COWEN_PLUGIN_IPC_TOKEN", token);
            }

            if compute_heavy {
                #[cfg(unix)]
                {
                    use std::os::unix::process::CommandExt;
                    unsafe {
                        cmd.pre_exec(|| {
                            // nice value 15 (lower scheduling priority) to avoid starving host gateway hot path threads
                            libc::setpriority(libc::PRIO_PROCESS, 0, 15);
                            Ok(())
                        });
                    }
                }
            }

            let child = cmd
                .spawn()
                .map_err(|e| anyhow::anyhow!("Failed to spawn Sidecar child process: {}", e))?;
            *child_guard = Some(child);
        }

        let child = child_guard.as_mut().unwrap();

        // 1. Write request line to Stdio
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Sidecar stdin pipeline closed"))?;
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::json!(1)),
            method: method.to_string(),
            params: Some(params),
        };

        let request_str = serde_json::to_string(&request)?;
        stdin.write_all(request_str.as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;

        // 2. Read single line response from Stdio
        let stdout = child
            .stdout
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Sidecar stdout pipeline closed"))?;
        let mut reader = BufReader::new(stdout);
        let mut response_line = String::new();
        reader.read_line(&mut response_line)?;

        if response_line.trim().is_empty() {
            // Child process might have died or EOF. Kill and clear child cache.
            let _ = child.kill();
            *child_guard = None;
            return Err(anyhow::anyhow!("Sidecar returned empty stdout/EOF"));
        }

        let response: JsonRpcResponse = serde_json::from_str(&response_line)?;
        if let Some(err) = response.error {
            return Err(anyhow::anyhow!(
                "Sidecar JSON-RPC Error: code={}, message={}",
                err.code,
                err.message
            ));
        }

        Ok(response.result.unwrap_or(serde_json::Value::Null))
    }
}

impl Drop for RpcPluginClient {
    fn drop(&mut self) {
        if let Ok(mut child_guard) = self.child.lock() {
            if let Some(mut child) = child_guard.take() {
                let _ = child.kill();
            }
        }
    }
}

pub fn is_secure_plugin_path(path: &Path) -> bool {
    let secure = crate::fs::is_file_secure(path);
    if !secure {
        tracing::warn!(
            "Plugin file or its parent directory {:?} is insecure (wrong owner or world-writable)",
            path
        );
    }
    secure
}

pub fn discover_plugins<P: AsRef<Path>>(dir: P) -> Vec<PathBuf> {
    let mut plugins = Vec::new();
    let supported_exts = crate::get_supported_plugin_extensions();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if supported_exts.contains(&ext) {
                // Ignore .bundle, .md5, .sha256 files etc. when matching empty extension on Unix
                if ext.is_empty() {
                    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if file_name.starts_with('.') || file_name.contains('.') {
                        continue;
                    }
                }
                if is_secure_plugin_path(&path) {
                    if cowen_infra::pki::verify_plugin_bundle(&path).is_ok() {
                        tracing::info!("Discovered plugin candidate: {:?}", path);
                        plugins.push(path);
                    } else {
                        tracing::error!(
                            "Skipping plugin with invalid or missing signature: {:?}",
                            path
                        );
                    }
                } else {
                    tracing::error!("Skipping insecure plugin candidate: {:?}", path);
                }
            }
        }
    }
    plugins
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    #[test]
    fn test_is_secure_plugin_path() {
        let dir = std::env::temp_dir().join(format!("cowen_test_plugin_{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let plugin_path = dir.join("test_plugin.so");
        File::create(&plugin_path).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            // Set normal permissions, should be secure if owned by current user
            let mut perms = fs::metadata(&plugin_path).unwrap().permissions();
            perms.set_mode(0o644);
            fs::set_permissions(&plugin_path, perms).unwrap();

            assert!(is_secure_plugin_path(&plugin_path));

            // Set world-writable permissions on file, should be insecure
            let mut perms = fs::metadata(&plugin_path).unwrap().permissions();
            perms.set_mode(0o777);
            fs::set_permissions(&plugin_path, perms).unwrap();

            assert!(!is_secure_plugin_path(&plugin_path));

            // Restore file permissions, but set directory world-writable
            let mut perms = fs::metadata(&plugin_path).unwrap().permissions();
            perms.set_mode(0o644);
            fs::set_permissions(&plugin_path, perms).unwrap();

            let mut dir_perms = fs::metadata(&dir).unwrap().permissions();
            dir_perms.set_mode(0o777);
            fs::set_permissions(&dir, dir_perms).unwrap();
            assert!(!is_secure_plugin_path(&plugin_path));
        }

        #[cfg(not(unix))]
        {
            assert!(is_secure_plugin_path(&plugin_path));
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_supports_trait_strict_blocking_of_all() {
        let manifest_with_all = cowen_infra::pki::PluginManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            binary_hash: String::new(),
            transport: Some("stdio".to_string()),
            capabilities: vec!["all".to_string()],
            required_privileges: vec![],
            contributes: None,
        };

        let loader = PluginLoader {
            path: std::path::PathBuf::from("/tmp/test_plugin"),
            manifest: manifest_with_all,
        };

        // Even if we query for "SearchProvider", "all" must be blocked!
        assert!(
            !loader.supports_trait("SearchProvider"),
            "Wildcard 'all' permission must be blocked and rejected!"
        );
        assert!(
            !loader.supports_trait("SomeOtherCapability"),
            "Wildcard 'all' permission must be blocked and rejected!"
        );

        let manifest_with_valid = cowen_infra::pki::PluginManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            binary_hash: String::new(),
            transport: Some("stdio".to_string()),
            capabilities: vec!["SearchProvider".to_string()],
            required_privileges: vec![],
            contributes: None,
        };

        let loader_valid = PluginLoader {
            path: std::path::PathBuf::from("/tmp/test_plugin"),
            manifest: manifest_with_valid,
        };

        assert!(
            loader_valid.supports_trait("SearchProvider"),
            "Valid SearchProvider capability should be accepted"
        );
    }

    #[test]
    fn test_verify_identity_and_enforce_privilege() {
        let manifest = cowen_infra::pki::PluginManifest {
            name: "cowen_search_embedding".to_string(),
            version: "0.4.0".to_string(),
            binary_hash: String::new(),
            transport: None,
            capabilities: vec!["SearchProvider".to_string()],
            required_privileges: vec![
                "sys.fs:cache_access".to_string(),
                "sys.network:fetch_asset".to_string(),
            ],
            contributes: None,
        };

        let loader = PluginLoader {
            path: std::path::PathBuf::from("/tmp/test_plugin"),
            manifest,
        };

        assert!(loader.verify_identity("SearchProvider"));
        assert!(!loader.verify_identity("AuthProvider"));

        assert!(loader.enforce_privilege("sys.fs:cache_access"));
        assert!(loader.enforce_privilege("sys.network:fetch_asset"));
        assert!(!loader.enforce_privilege("sys.db:write_access"));
    }
}
