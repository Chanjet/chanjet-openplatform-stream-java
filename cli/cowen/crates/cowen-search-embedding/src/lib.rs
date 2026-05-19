use std::ffi::{c_char, CStr};
use std::sync::Mutex;
use once_cell::sync::Lazy;
use cowen_search::{SearchProvider, SearchDocument};
use cowen_ai::{ONNXEmbedder, SearchIndex};

static PROVIDER: Lazy<Mutex<Option<Box<dyn SearchProvider>>>> = Lazy::new(|| Mutex::new(None));

struct EmbeddingProvider {
    embedder: ONNXEmbedder,
    index: SearchIndex,
}

impl SearchProvider for EmbeddingProvider {
    fn name(&self) -> &str {
        "embedding_match"
    }

    fn search(&self, query: &str, top: usize) -> Vec<(f32, &cowen_search::SearchDocument)> {
        // dummy vector
        let vec: Vec<f32> = vec![0.0; 128];
        let results = self.index.search(&vec, query, top);
        
        // This is a stub: real implementation will require a shared Document definition
        // between cowen-ai and cowen-search. 
        // Returning empty for now to fix build.
        vec![]
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn v1_init(model_path: *const c_char, tokenizer_path: *const c_char) -> i32 {
    let model = unsafe { CStr::from_ptr(model_path).to_string_lossy() };
    let tokenizer = unsafe { CStr::from_ptr(tokenizer_path).to_string_lossy() };
    
    if let Ok(embedder) = ONNXEmbedder::new(&model, &tokenizer) {
        let provider = Box::new(EmbeddingProvider {
            embedder,
            index: SearchIndex::default(),
        });
        *PROVIDER.lock().unwrap() = Some(provider);
        0
    } else {
        1
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn v1_free() {
    *PROVIDER.lock().unwrap() = None;
}
