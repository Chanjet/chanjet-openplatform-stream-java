use crate::{SearchDocument, SearchProvider};
use cowen_infra::PluginLoader;
use std::ffi::c_char;
use tracing::warn;

pub struct DynamicSearchProvider {
    name: String,
    _loader: PluginLoader,
    _init_fn: libloading::Symbol<'static, unsafe extern "C" fn(*const c_char, *const c_char) -> i32>,
    _index_fn: libloading::Symbol<'static, unsafe extern "C" fn(*const c_char) -> i32>,
    _search_fn: libloading::Symbol<'static, unsafe extern "C" fn(*const c_char, usize) -> *const c_char>,
    _free_fn: libloading::Symbol<'static, unsafe extern "C" fn()>,
}

impl DynamicSearchProvider {
    pub unsafe fn new<P: AsRef<std::path::Path>>(name: &str, path: P) -> anyhow::Result<Self> {
        let loader = PluginLoader::new(path.as_ref())?;

        let init_fn: libloading::Symbol<'static, unsafe extern "C" fn(*const c_char, *const c_char) -> i32> = 
            unsafe { std::mem::transmute(loader.get_symbol::<unsafe extern "C" fn(*const c_char, *const c_char) -> i32>(b"v1_init")?) };

        let index_fn: libloading::Symbol<'static, unsafe extern "C" fn(*const c_char) -> i32> = 
            unsafe { std::mem::transmute(loader.get_symbol::<unsafe extern "C" fn(*const c_char) -> i32>(b"v1_index")?) };

        let search_fn: libloading::Symbol<'static, unsafe extern "C" fn(*const c_char, usize) -> *const c_char> = 
            unsafe { std::mem::transmute(loader.get_symbol::<unsafe extern "C" fn(*const c_char, usize) -> *const c_char>(b"v1_search")?) };

        let free_fn: libloading::Symbol<'static, unsafe extern "C" fn()> = 
            unsafe { std::mem::transmute(loader.get_symbol::<unsafe extern "C" fn()> (b"v1_free")?) };

        // 🚀 LIFE CYCLE: Explicitly initialize the plugin
        // Pass null pointers to let the plugin use its default asset paths
        unsafe { (init_fn)(std::ptr::null(), std::ptr::null()); }

        Ok(Self { name: name.to_string(), _loader: loader, _init_fn: init_fn, _index_fn: index_fn, _search_fn: search_fn, _free_fn: free_fn })
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

    fn search(&self, query: &str, top: usize) -> Vec<(f32, SearchDocument)> {
        let c_query = std::ffi::CString::new(query).unwrap();
        let ptr = unsafe { (self._search_fn)(c_query.as_ptr(), top) };
        if ptr.is_null() { return vec![]; }

        let json = unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy() };
        serde_json::from_str::<(Vec<(f32, SearchDocument)>)>(&json).unwrap_or_default()
    }

    fn update_index(&self, docs: &[SearchDocument]) {
        if let Ok(json) = serde_json::to_string(docs) {
            let c_json = std::ffi::CString::new(json).unwrap();
            unsafe { (self._index_fn)(c_json.as_ptr()); }
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
