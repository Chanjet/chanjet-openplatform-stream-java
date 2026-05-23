use serde::{Serialize, Deserialize};

pub mod provider;
pub mod loader;
pub use provider::StringMatchProvider;

/// API Document model
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchDocument {
    pub id: String,
    pub summary: String,
    pub description: String,
    pub vector: Vec<f32>,
}

/// SPI for search providers
pub trait SearchProvider: Send + Sync {
    fn search(&self, query: &str, top: usize) -> Vec<(f32, SearchDocument)>;
    fn update_index(&self, docs: &[SearchDocument]);
    fn name(&self) -> &str;
}

#[macro_export]
macro_rules! plugin_trait_search_provider {
    () => { c"SearchProvider" }
}
