use crate::{SearchDocument, SearchProvider};
use cowen_infra::PluginLoader;
use std::ffi::c_char;
use tracing::warn;

pub struct DynamicSearchProvider {
    name: String,
    _loader: PluginLoader,
    _init_fn: libloading::Symbol<'static, unsafe extern "C-unwind" fn(*const c_char, *const c_char) -> i32>,
    _index_fn: libloading::Symbol<'static, unsafe extern "C-unwind" fn(*const c_char) -> i32>,
    _search_fn: libloading::Symbol<'static, unsafe extern "C-unwind" fn(*const c_char, usize) -> *const c_char>,
    _free_fn: libloading::Symbol<'static, unsafe extern "C-unwind" fn()>,
}

impl DynamicSearchProvider {
    /// Loads a dynamic search provider plugin.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the plugin library at the given path is trusted,
    /// as loading it will execute arbitrary initialization code and FFI calls.
    pub unsafe fn new<P: AsRef<std::path::Path>>(name: &str, path: P) -> anyhow::Result<Self> {
        let loader = PluginLoader::new(path.as_ref())?;

        let init_fn: libloading::Symbol<'static, unsafe extern "C-unwind" fn(*const c_char, *const c_char) -> i32> = 
            unsafe { std::mem::transmute(loader.get_symbol::<unsafe extern "C-unwind" fn(*const c_char, *const c_char) -> i32>(b"v1_init")?) };

        let index_fn: libloading::Symbol<'static, unsafe extern "C-unwind" fn(*const c_char) -> i32> = 
            unsafe { std::mem::transmute(loader.get_symbol::<unsafe extern "C-unwind" fn(*const c_char) -> i32>(b"v1_index")?) };

        let search_fn: libloading::Symbol<'static, unsafe extern "C-unwind" fn(*const c_char, usize) -> *const c_char> = 
            unsafe { std::mem::transmute(loader.get_symbol::<unsafe extern "C-unwind" fn(*const c_char, usize) -> *const c_char>(b"v1_search")?) };

        let free_fn: libloading::Symbol<'static, unsafe extern "C-unwind" fn()> = 
            unsafe { std::mem::transmute(loader.get_symbol::<unsafe extern "C-unwind" fn()> (b"v1_free")?) };

        // 🚀 LIFE CYCLE: Explicitly initialize the plugin
        // Pass null pointers to let the plugin use its default asset paths
        let init_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            unsafe { (init_fn)(std::ptr::null(), std::ptr::null()) }
        }));
        
        match init_res {
            Ok(0) => {},
            Ok(code) => return Err(anyhow::anyhow!("Plugin initialization failed with code: {}", code)),
            Err(_) => return Err(anyhow::anyhow!("Plugin initialization panicked")),
        }

        Ok(Self { name: name.to_string(), _loader: loader, _init_fn: init_fn, _index_fn: index_fn, _search_fn: search_fn, _free_fn: free_fn })
    }
}

impl Drop for DynamicSearchProvider {
    fn drop(&mut self) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            unsafe { (self._free_fn)(); }
        }));
    }
}

impl SearchProvider for DynamicSearchProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn search(&self, query: &str, top: usize) -> Vec<(f32, SearchDocument)> {
        let c_query = std::ffi::CString::new(query).unwrap();
        
        let search_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            unsafe { (self._search_fn)(c_query.as_ptr(), top) }
        }));

        let ptr = match search_res {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(target: "sys", "Plugin search panicked: {:?}", e);
                return vec![];
            }
        };

        if ptr.is_null() {
            return vec![];
        }

        let parse_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned() }
        }));

        let json = match parse_res {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(target: "sys", "Failed to parse plugin search result pointer: {:?}", e);
                return vec![];
            }
        };

        serde_json::from_str::<Vec<(f32, SearchDocument)>>(&json).unwrap_or_default()
    }

    fn update_index(&self, docs: &[SearchDocument]) {
        if let Ok(json) = serde_json::to_string(docs) {
            let c_json = std::ffi::CString::new(json).unwrap();
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                unsafe { (self._index_fn)(c_json.as_ptr()); }
            }));
        }
    }
}

pub struct FallbackProvider {
    pub primary: Option<Box<dyn SearchProvider>>,
    pub fallback: Box<dyn SearchProvider>,
}

impl SearchProvider for FallbackProvider {
    fn name(&self) -> &str {
        "fallback_search"
    }

    fn search(&self, query: &str, top: usize) -> Vec<(f32, SearchDocument)> {
        if let Some(ref primary) = self.primary {
            let res = primary.search(query, top);
            if !res.is_empty() {
                return res;
            }
            warn!("Primary search provider returned no results, falling back.");
        }
        self.fallback.search(query, top)
    }

    fn update_index(&self, docs: &[SearchDocument]) {
        if let Some(ref primary) = self.primary {
            primary.update_index(docs);
        }
        self.fallback.update_index(docs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    use std::process::Command as SysCommand;

    #[test]
    fn test_dynamic_search_provider_panic_safety() {
        // 1. Create temp directory
        let dir = tempdir().unwrap();
        let src_path = dir.path().join("mock_plugin.rs");
        
        let dylib_name = if cfg!(target_os = "windows") {
            "mock_plugin.dll"
        } else if cfg!(target_os = "macos") {
            "libmock_plugin.dylib"
        } else {
            "libmock_plugin.so"
        };
        let dylib_path = dir.path().join(dylib_name);

        // 2. Write panicking plugin source code
        let src = r#"
            #![allow(unused)]
            use std::ffi::c_char;

            #[no_mangle]
            pub unsafe extern "C-unwind" fn v1_init(model_path: *const c_char, tokenizer_path: *const c_char) -> i32 {
                0
            }

            #[no_mangle]
            pub unsafe extern "C-unwind" fn v1_index(docs_json: *const c_char) -> i32 {
                0
            }

            #[no_mangle]
            pub unsafe extern "C-unwind" fn v1_search(query_ptr: *const c_char, top: usize) -> *const c_char {
                let res = std::panic::catch_unwind(|| {
                    panic!("Deliberate FFI panic!");
                });
                match res {
                    Ok(ptr) => ptr,
                    Err(_) => std::ptr::null(),
                }
            }

            #[no_mangle]
            pub unsafe extern "C-unwind" fn v1_free() {}
        "#;
        fs::write(&src_path, src).unwrap();

        // 3. Compile the cdylib dynamically using rustc
        let output = SysCommand::new("rustc")
            .arg("--crate-type=cdylib")
            .arg(&src_path)
            .arg("-o")
            .arg(&dylib_path)
            .output()
            .expect("Failed to execute rustc to compile mock plugin");

        if !output.status.success() {
            panic!(
                "Failed to compile mock plugin cdylib:\nSTDOUT:\n{}\nSTDERR:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // 4. Load the compiled dynamic plugin
        let provider = unsafe {
            DynamicSearchProvider::new("mock_plugin", &dylib_path)
                .expect("Failed to load mock plugin")
        };

        // 5. Test search which will panic.
        // If panic safety is NOT implemented, this will crash the whole test run or thread.
        // We assert that the panic is caught and an empty list is returned instead of crashing!
        let results = provider.search("anything", 10);
        assert!(results.is_empty(), "Expected empty results due to panic catching, but got: {:?}", results);
    }
}


