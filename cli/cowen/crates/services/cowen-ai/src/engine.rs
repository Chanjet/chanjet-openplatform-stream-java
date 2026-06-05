use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub metadata: String,
    pub description: String,
    pub vector: Vec<Float32>, // Using float32 for consistency with Go
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub summary: String,
    pub description: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Engine {
    pub docs: Vec<Document>,
}

impl Engine {
    pub fn new(docs: Vec<Document>) -> Self {
        Self { docs }
    }

    pub fn search(&self, query_vector: &[f32], query_text: &str, limit: usize) -> Vec<SearchResult> {
        if query_vector.is_empty() {
            return Vec::new();
        }

        let query_text_lower = query_text.to_lowercase();
        let query_runes: Vec<char> = query_text_lower.chars().collect();
        let mut n_grams = Vec::new();

        for window_size in 2..=4 {
            if query_runes.len() < window_size {
                continue;
            }
            for i in 0..=(query_runes.len() - window_size) {
                n_grams.push(query_runes[i..(i + window_size)].iter().collect::<String>());
            }
        }

        let mut results = Vec::new();
        for doc in &self.docs {
            let mut score = cosine_similarity(query_vector, &doc.vector);
            
            let metadata_lower = doc.metadata.to_lowercase();
            let desc_lower = doc.description.to_lowercase();
            let id_lower = doc.id.to_lowercase();

            if !n_grams.is_empty() {
                let mut hit_count = 0;
                for gram in &n_grams {
                    if metadata_lower.contains(gram) || desc_lower.contains(gram) || id_lower.contains(gram) {
                        hit_count += 1;
                    }
                }
                
                let boost = (hit_count as f32) / (n_grams.len() as f32) * 0.98;
                if boost > 0.0 {
                    score += boost;
                }
            }

            if score > 0.3 {
                results.push(SearchResult {
                    id: doc.id.clone(),
                    summary: doc.metadata.clone(),
                    description: doc.description.clone(),
                    score: score as f64,
                });
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        if results.len() > limit {
            results.truncate(limit);
        }
        results
    }
}

pub fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }

    let mut dot_product = 0.0;
    let mut mag1 = 0.0;
    let mut mag2 = 0.0;

    for i in 0..v1.len() {
        dot_product += v1[i] * v2[i];
        mag1 += v1[i] * v1[i];
        mag2 += v2[i] * v2[i];
    }

    if mag1 == 0.0 || mag2 == 0.0 {
        return 0.0;
    }

    dot_product / (mag1.sqrt() * mag2.sqrt())
}

// Support type alias since f32 is float32 in Go but f32 in Rust.
// But some crates like ndarray or ort might prefer f32.
type Float32 = f32;
