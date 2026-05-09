use cowen_common::CowenResult;
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;

/// A vectorized API document
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchDocument {
    pub id: String,          // e.g. "GET /v1/orders"
    pub summary: String,     // summary
    pub description: String, // description
    pub vector: Vec<f32>,    // high-dimensional vector
}

/// Persistent Search Index
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SearchIndex {
    pub docs: Vec<SearchDocument>,
}

impl SearchIndex {
    /// Hybrid search: Vector similarity + N-Gram boost for Chinese
    pub fn search(&self, query_vec: &[f32], query_text: &str, top: usize) -> Vec<(f32, &SearchDocument)> {
        let query_lower = query_text.to_lowercase();
        let query_runes: Vec<char> = query_lower.chars().collect();
        
        // Generate N-Grams (2, 3, 4 chars) for boosting
        let mut n_grams = Vec::new();
        for window_size in 2..=4 {
            if query_runes.len() >= window_size {
                for i in 0..=(query_runes.len() - window_size) {
                    let gram: String = query_runes[i..i+window_size].iter().collect();
                    n_grams.push(gram);
                }
            }
        }

        let mut results: Vec<(f32, &SearchDocument)> = self.docs.iter()
            .map(|doc| {
                // 1. Vector cosine similarity
                let mut similarity = 0.0;
                for i in 0..doc.vector.len().min(query_vec.len()) {
                    similarity += doc.vector[i] * query_vec[i];
                }

                // 2. Keyword boost (N-Gram match)
                let doc_content = format!("{} {}", doc.summary, doc.description).to_lowercase();
                let mut boost = 0.0;
                for gram in &n_grams {
                    if doc_content.contains(gram) {
                        boost += 0.05; // 5% boost per n-gram match
                    }
                }

                (similarity + boost, doc)
            })
            .collect();

        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        results.into_iter().take(top).collect()
    }

    /// Ensure embedding model assets are available locally.
    /// This extracts embedded models into the local application directory.
    pub fn ensure_assets(target_dir: &std::path::Path) -> CowenResult<()> {
        let models_dir = target_dir.join("search").join("models");
        if !models_dir.exists() {
            fs::create_dir_all(&models_dir)?;
        }

        let model_path = models_dir.join("model_quantized.onnx");
        let data_path = models_dir.join("model_quantized.onnx_data");
        let tokenizer_path = models_dir.join("tokenizer.json");

        Self::ensure_asset(&model_path, include_bytes!("../../../../../assets/search/models/model_quantized.onnx"))?;
        Self::ensure_asset(&data_path, include_bytes!("../../../../../assets/search/models/model_quantized.onnx_data"))?;
        Self::ensure_asset(&tokenizer_path, include_bytes!("../../../../../assets/search/models/tokenizer.json"))?;

        Ok(())
    }

    fn ensure_asset(path: &PathBuf, content: &[u8]) -> CowenResult<()> {
        if !path.exists() || fs::metadata(path)?.len() != content.len() as u64 {
            fs::write(path, content)?;
        }
        Ok(())
    }

    pub fn get_asset_paths(target_dir: &std::path::Path) -> (String, String) {
        let models_dir = target_dir.join("search").join("models");
        let model_path = models_dir.join("model_quantized.onnx");
        let tokenizer_path = models_dir.join("tokenizer.json");
        (
            model_path.to_string_lossy().to_string(),
            tokenizer_path.to_string_lossy().to_string()
        )
    }
}
