#![cfg(feature = "ai")]
use anyhow::{anyhow, Result, Context};
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;
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
    pub fn save(&self, path: &PathBuf) -> Result<()> {
        let json = serde_json::to_string(self)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, json).context("Failed to write index file")?;
        Ok(())
    }

    pub fn load(path: &PathBuf) -> Result<Self> {
        let json = fs::read_to_string(path).context("Failed to read index file")?;
        let index = serde_json::from_str(&json).context("Failed to parse index JSON")?;
        Ok(index)
    }

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

        let mut results = Vec::new();
        for doc in &self.docs {
            let mut score = cosine_similarity(query_vec, &doc.vector);
            
            // Hybrid Boost: Check N-Gram matches in summary, description and ID
            if !n_grams.is_empty() {
                let mut hit_count = 0;
                let doc_text = format!("{} {} {}", doc.id, doc.summary, doc.description).to_lowercase();
                for gram in &n_grams {
                    if doc_text.contains(gram) {
                        hit_count += 1;
                    }
                }
                
                let boost = (hit_count as f32 / n_grams.len() as f32) * 0.98;
                if boost > 0.0 {
                    score += boost;
                }
            }

            if score > 0.3 {
                results.push((score, doc));
            }
        }

        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        results.into_iter().take(top).collect()
    }
}

/// Real ONNX Embedder using BGE-small-zh-v1.5 (ort 2.0 rc.12)
pub struct ONNXEmbedder {
    session: Session,
    tokenizer: Tokenizer,
}

impl ONNXEmbedder {
    pub fn new() -> Result<Self> {
        let app_dir = crate::core::config::get_app_dir();
        let model_dir = app_dir.join("models");
        fs::create_dir_all(&model_dir)?;

        let model_path = model_dir.join("model_quantized.onnx");
        let data_path = model_dir.join("model_quantized.onnx_data");
        let tokenizer_path = model_dir.join("tokenizer.json");

        // Extract embedded assets to local filesystem
        Self::ensure_asset(&model_path, include_bytes!("../../../../assets/search/models/model_quantized.onnx"))?;
        Self::ensure_asset(&data_path, include_bytes!("../../../../assets/search/models/model_quantized.onnx_data"))?;
        Self::ensure_asset(&tokenizer_path, include_bytes!("../../../../assets/search/models/tokenizer.json"))?;

        // Load Tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow!("Failed to load tokenizer: {}", e))?;

        // Load Model
        let session = Session::builder()?
            .commit_from_file(&model_path)?;

        Ok(Self { session, tokenizer })
    }

    fn ensure_asset(path: &PathBuf, bytes: &[u8]) -> Result<()> {
        if !path.exists() || fs::metadata(path)?.len() != bytes.len() as u64 {
            fs::write(path, bytes)?;
        }
        Ok(())
    }

    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        let encoding = self.tokenizer.encode(text, true)
            .map_err(|e| anyhow!("Tokenization failed: {}", e))?;

        let ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&id| id as i64).collect();
        let types: Vec<i64> = encoding.get_type_ids().iter().map(|&id| id as i64).collect();

        let seq_len = ids.len();
        let shape = vec![1i64, seq_len as i64];

        let input_ids = Tensor::from_array((shape.clone(), ids))?;
        let attention_mask = Tensor::from_array((shape.clone(), mask))?;
        let token_type_ids = Tensor::from_array((shape, types))?;

        let inputs = ort::inputs! {
            "input_ids" => input_ids,
            "attention_mask" => attention_mask,
            "token_type_ids" => token_type_ids,
        };

        let outputs = self.session.run(inputs)?;

        let (shape, data) = outputs[0].try_extract_tensor::<f32>()?;
        let dim = shape[2] as usize;

        let mut mean_vec = vec![0.0f32; dim];
        for i in 0..seq_len {
            for j in 0..dim {
                mean_vec[j] += data[i * dim + j];
            }
        }
        for v in mean_vec.iter_mut() {
            *v /= seq_len as f32;
        }

        let norm = mean_vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-6 {
            for v in mean_vec.iter_mut() {
                *v /= norm;
            }
        }

        Ok(mean_vec)
    }

    /// Rebuild the index from OpenAPI spec
    pub fn rebuild_index(&mut self, spec: &serde_json::Value) -> Result<SearchIndex> {
        let paths = spec["paths"].as_object().ok_or_else(|| anyhow!("Invalid spec: paths not found"))?;
        let mut index = SearchIndex::default();

        for (path, methods) in paths {
            if let Some(methods_obj) = methods.as_object() {
                for (method, op) in methods_obj {
                    let summary = op["summary"].as_str().unwrap_or("");
                    let desc = op["description"].as_str().unwrap_or("");
                    
                    // Embedding input text = summary + desc + path
                    let text = format!("{} {} {}", summary, desc, path).trim().to_string();
                    let vector = self.embed(&text)?;

                    index.docs.push(SearchDocument {
                        id: format!("{} {}", method.to_uppercase(), path),
                        summary: summary.to_string(),
                        description: desc.to_string(),
                        vector,
                    });
                }
            }
        }

        Ok(index)
    }
}

pub fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum()
}
