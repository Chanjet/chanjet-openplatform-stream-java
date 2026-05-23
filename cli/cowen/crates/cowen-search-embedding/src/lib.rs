use std::ffi::{c_char, CStr};
use std::sync::{Mutex, LazyLock};
use cowen_search::SearchProvider;
use cowen_ai::{ONNXEmbedder, SearchIndex};

static PROVIDER: LazyLock<Mutex<Option<Box<dyn SearchProvider>>>> = LazyLock::new(|| Mutex::new(None));

use std::cell::RefCell;

thread_local! {
    static JSON_CACHE: RefCell<std::ffi::CString> = RefCell::new(std::ffi::CString::new("").unwrap());
}

struct EmbeddingProvider {
    _embedder: Mutex<ONNXEmbedder>,
    index: Mutex<SearchIndex>,
}

impl SearchProvider for EmbeddingProvider {
    fn name(&self) -> &str {
        "embedding_match"
    }

    fn search(&self, query: &str, top: usize) -> Vec<(f32, cowen_search::SearchDocument)> {
        let mut embedder = self._embedder.lock().unwrap();
        if let Ok(query_vector) = embedder.embed(query) {
            let index = self.index.lock().unwrap();
            let results = index.search(&query_vector, query, top);
            
            results.into_iter().map(|(score, ai_doc)| {
                (score, cowen_search::SearchDocument {
                    id: ai_doc.id.clone(),
                    summary: ai_doc.summary.clone(),
                    description: ai_doc.description.clone(),
                    vector: ai_doc.vector.clone(),
                })
            }).collect()
        } else {
            vec![]
        }
    }

    fn update_index(&self, docs: &[cowen_search::SearchDocument]) {
        let mut embedder = self._embedder.lock().unwrap();
        let mut index = self.index.lock().unwrap();
        for doc in docs {
            let text = format!("{} {}", doc.summary, doc.description);
            if let Ok(vector) = embedder.embed(&text) {
                // Bridge types
                let ai_doc = cowen_ai::SearchDocument {
                    id: doc.id.clone(),
                    summary: doc.summary.clone(),
                    description: doc.description.clone(),
                    vector,
                };
                index.push(ai_doc);
            }
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn v1_init(model_path: *const c_char, tokenizer_path: *const c_char) -> i32 {
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let model = if !model_path.is_null() {
            unsafe { CStr::from_ptr(model_path).to_string_lossy().to_string() }
        } else {
            String::new()
        };
        
        let tokenizer = if !tokenizer_path.is_null() {
            unsafe { CStr::from_ptr(tokenizer_path).to_string_lossy().to_string() }
        } else {
            String::new()
        };
        
        // Use default paths if pointers are null or empty
        let app_dir = cowen_common::config::get_app_dir();
        let default_model = app_dir.join("search").join("models").join("model_quantized.onnx");
        let default_tokenizer = app_dir.join("search").join("models").join("tokenizer.json");

        // 🚀 Ensure AI assets are available locally for the plugin when initialized
        if let Err(e) = cowen_ai::SearchIndex::ensure_assets(&app_dir) {
            eprintln!("⚠️  Failed to prepare AI assets from plugin: {}", e);
        }

        let final_model_path = if model.is_empty() { default_model.to_string_lossy().to_string() } else { model };
        let final_tokenizer_path = if tokenizer.is_empty() { default_tokenizer.to_string_lossy().to_string() } else { tokenizer };

        if let Ok(embedder) = ONNXEmbedder::new(&final_model_path, &final_tokenizer_path) {
            let provider = Box::new(EmbeddingProvider {
                _embedder: Mutex::new(embedder),
                index: Mutex::new(SearchIndex::default()),
            });
            *PROVIDER.lock().unwrap() = Some(provider);
            0
        } else {
            1
        }
    }));

    match res {
        Ok(code) => code,
        Err(_) => {
            eprintln!("⚠️ Panic occurred during v1_init");
            1
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn v1_index(docs_json: *const c_char) -> i32 {
    if docs_json.is_null() {
        return 1;
    }

    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let json = unsafe { CStr::from_ptr(docs_json).to_string_lossy() };
        if let Ok(docs) = serde_json::from_str::<Vec<cowen_search::SearchDocument>>(&json)
            && let Some(ref provider) = *PROVIDER.lock().unwrap() {
                provider.update_index(&docs);
                return 0;
            }
        1
    }));

    match res {
        Ok(code) => code,
        Err(_) => {
            eprintln!("⚠️ Panic occurred during v1_index");
            1
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn v1_search(query_ptr: *const c_char, top: usize) -> *const c_char {
    if query_ptr.is_null() {
        return std::ptr::null();
    }

    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let query = unsafe { CStr::from_ptr(query_ptr).to_string_lossy() };
        if let Some(ref provider) = *PROVIDER.lock().unwrap() {
            let results = provider.search(&query, top);
            if let Ok(json) = serde_json::to_string(&results) {
                let c_str = std::ffi::CString::new(json).unwrap();
                return JSON_CACHE.with(|cache| {
                    *cache.borrow_mut() = c_str;
                    cache.borrow().as_ptr()
                });
            }
        }
        std::ptr::null()
    }));

    match res {
        Ok(ptr) => ptr,
        Err(_) => {
            eprintln!("⚠️ Panic occurred during v1_search");
            std::ptr::null()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn v1_free() {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        *PROVIDER.lock().unwrap() = None;
    }));
}

#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn v1_name() -> *const c_char {
    c"AI Embedding Matcher".as_ptr()
}

#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn v1_desc() -> *const c_char {
    c"使用向量模型检索 api".as_ptr()
}

#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn v1_trait() -> *const c_char {
    cowen_search::plugin_trait_search_provider!().as_ptr()
}
