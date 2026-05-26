use libloading::{Library, Symbol};
use std::path::{Path, PathBuf};
use std::fs;

pub struct PluginLoader {
    lib: Library,
}

impl PluginLoader {
    pub fn new<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let p = path.as_ref();
        
        if !is_secure_plugin_path(p) {
            return Err(anyhow::anyhow!("Plugin path is insecure (wrong owner or world-writable)"));
        }
        
        crate::pki::verify_plugin_bundle(p)?;
        
        let lib = unsafe { Library::new(p)? };
        Ok(Self { lib })
    }

    /// Retrieves a symbol from the loaded plugin library.
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    /// - The generic type `T` accurately represents the function signature or variable type of the loaded symbol.
    /// - The plugin library is trusted, as executing arbitrary code can lead to undefined behavior or compromise the system.
    pub unsafe fn get_symbol<T>(&self, name: &[u8]) -> anyhow::Result<Symbol<'_, T>> {
        Ok(self.lib.get(name)?)
    }
}

pub fn is_secure_plugin_path(path: &Path) -> bool {
    let secure = crate::sys::fs::is_file_secure(path);
    if !secure {
        tracing::warn!("Plugin file or its parent directory {:?} is insecure (wrong owner or world-writable)", path);
    }
    secure
}

pub fn discover_plugins<P: AsRef<Path>>(dir: P) -> Vec<PathBuf> {
    let mut plugins = Vec::new();
    let supported_exts = if cfg!(target_os = "windows") {
        vec!["dll"]
    } else if cfg!(target_os = "macos") {
        vec!["dylib", "so"]
    } else {
        vec!["so"]
    };

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if supported_exts.contains(&ext) {
                    if is_secure_plugin_path(&path) {
                        if crate::pki::verify_plugin_bundle(&path).is_ok() {
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
