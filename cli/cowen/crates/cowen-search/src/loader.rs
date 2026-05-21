use crate::{SearchDocument, SearchProvider};
use cowen_infra::PluginLoader;
use std::ffi::c_char;
use tracing::warn;

pub struct DynamicSearchProvider {
    name: String,
    _loader: PluginLoader,
    _init_fn: libloading::Symbol<'static, unsafe extern "C" fn(*const c_char, *const c_char) -> i32>,
    _free_fn: libloading::Symbol<'static, unsafe extern "C" fn()>,
}

impl DynamicSearchProvider {
    pub unsafe fn new<P: AsRef<std::path::Path>>(name: &str, path: P) -> anyhow::Result<Self> {
        let loader = PluginLoader::new(path.as_ref())?;
        let init_fn: libloading::Symbol<'static, unsafe extern "C" fn(*const c_char, *const c_char) -> i32> = 
            unsafe { std::mem::transmute(loader.get_symbol::<unsafe extern "C" fn(*const c_char, *const c_char) -> i32>(b"v1_init")?) };
        let free_fn: libloading::Symbol<'static, unsafe extern "C" fn()> = 
            unsafe { std::mem::transmute(loader.get_symbol::<unsafe extern "C" fn()>(b"v1_free")?) };
        
        Ok(Self { name: name.to_string(), _loader: loader, _init_fn: init_fn, _free_fn: free_fn })
    }
}

impl Drop for DynamicSearchProvider {
    fn drop(&mut self) {
        unsafe { (self._free_fn)(); }
    }
}

impl SearchProvider for DynamicSearchProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn search(&self, _query: &str, _top: usize) -> Vec<(f32, &SearchDocument)> {
        vec![]
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

    fn search(&self, query: &str, top: usize) -> Vec<(f32, &SearchDocument)> {
        if let Some(ref primary) = self.primary {
            let res = primary.search(query, top);
            if !res.is_empty() {
                return res;
            }
            warn!("Primary search provider returned no results, falling back.");
        }
        self.fallback.search(query, top)
    }
}
