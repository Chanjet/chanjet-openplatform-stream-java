use std::path::{Path, PathBuf};
use std::fs;
use std::process::{Command as StdCommand, Child, Stdio};
use std::io::{BufRead, BufReader, Write};
use std::sync::Mutex;

pub struct PluginLoader {
    path: PathBuf,
    manifest: cowen_infra::pki::PluginManifest,
}

impl PluginLoader {
    pub fn new<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let p = path.as_ref();
        
        if !is_secure_plugin_path(p) {
            return Err(anyhow::anyhow!("Plugin path is insecure (wrong owner or world-writable)"));
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
                cowen_infra::pki::PluginManifest {
                    name: p.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string(),
                    version: "dev".to_string(),
                    binary_hash: String::new(),
                    permissions: vec!["all".to_string()],
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
        if trait_name == "SearchProvider" {
            self.manifest.name.contains("search") 
                || self.manifest.name.contains("embedding") 
                || self.manifest.permissions.iter().any(|p| p == "all" || p == "SearchProvider")
        } else {
            false
        }
    }
}

/// Generic Stdio JSON-RPC Client for Rust standalone Sidecar processes (Phase 2)
pub struct RpcPluginClient {
    child: Mutex<Option<Child>>,
    binary_path: PathBuf,
    tenant_id: String,
    bridge_token: String,
}

impl RpcPluginClient {
    pub fn new(binary_path: PathBuf, tenant_id: String) -> Self {
        // Generate a cryptographically secure-ish random协商 Token
        let bridge_token = format!("{:x}", uuid::Uuid::new_v4().simple());
        
        Self {
            child: Mutex::new(None),
            binary_path,
            tenant_id,
            bridge_token,
        }
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    /// Sends a JSON-RPC method call to the Sidecar process via Stdio.
    pub fn call_tool(&self, method: &str, params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let mut child_guard = self.child.lock().unwrap();
        
        // Spawn the Rust Sidecar process on demand if not already running
        if child_guard.is_none() {
            tracing::info!(target: "sys", "Spawning Rust Sidecar: {:?}", self.binary_path);
            let child = StdCommand::new(&self.binary_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                // Safe credential injection in-memory (Option A requirement)
                .env("COWEN_BRIDGE_TOKEN", &self.bridge_token)
                .spawn()
                .map_err(|e| anyhow::anyhow!("Failed to spawn Sidecar child process: {}", e))?;
            *child_guard = Some(child);
        }

        let child = child_guard.as_mut().unwrap();

        // 1. Write request line to Stdio
        let stdin = child.stdin.as_mut().ok_or_else(|| anyhow::anyhow!("Sidecar stdin pipeline closed"))?;
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let request_str = serde_json::to_string(&request)?;
        stdin.write_all(request_str.as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;

        // 2. Read single line response from Stdio
        let stdout = child.stdout.as_mut().ok_or_else(|| anyhow::anyhow!("Sidecar stdout pipeline closed"))?;
        let mut reader = BufReader::new(stdout);
        let mut response_line = String::new();
        reader.read_line(&mut response_line)?;

        if response_line.trim().is_empty() {
            // Child process might have died or EOF. Kill and clear child cache.
            let _ = child.kill();
            *child_guard = None;
            return Err(anyhow::anyhow!("Sidecar returned empty stdout/EOF"));
        }

        let response: serde_json::Value = serde_json::from_str(&response_line)?;
        if let Some(err) = response.get("error") {
            return Err(anyhow::anyhow!("Sidecar JSON-RPC Error: {:?}", err));
        }

        Ok(response.get("result").cloned().unwrap_or(serde_json::Value::Null))
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
        tracing::warn!("Plugin file or its parent directory {:?} is insecure (wrong owner or world-writable)", path);
    }
    secure
}

pub fn discover_plugins<P: AsRef<Path>>(dir: P) -> Vec<PathBuf> {
    let mut plugins = Vec::new();
    let supported_exts = crate::get_supported_plugin_extensions();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if supported_exts.contains(&ext) {
                    if is_secure_plugin_path(&path) {
                        if cowen_infra::pki::verify_plugin_bundle(&path).is_ok() {
                            tracing::info!("Discovered plugin candidate: {:?}", path);
                            plugins.push(path);
                        } else {
                            tracing::error!("Skipping plugin with invalid or missing signature: {:?}", path);
                        }
                    } else {
                        tracing::error!("Skipping insecure plugin candidate: {:?}", path);
                    }
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
}
