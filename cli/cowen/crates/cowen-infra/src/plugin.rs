use libloading::{Library, Symbol};
use std::path::{Path, PathBuf};
use std::fs;
use tracing::info;

pub struct PluginLoader {
    lib: Library,
}

impl PluginLoader {
    pub fn new<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let lib = unsafe { Library::new(path.as_ref())? };
        Ok(Self { lib })
    }

    pub unsafe fn get_symbol<T>(&self, name: &[u8]) -> anyhow::Result<Symbol<'_, T>> {
        Ok(self.lib.get(name)?)
    }
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
                    info!("Discovered plugin candidate: {:?}", path);
                    plugins.push(path);
                }
            }
        }
    }
    plugins
}
