use crate::{SearchDocument, SearchProvider};

pub struct StringMatchProvider {
    pub docs: Vec<SearchDocument>,
}

impl SearchProvider for StringMatchProvider {
    fn name(&self) -> &str {
        "string_match"
    }

    fn search(&self, query: &str, top: usize) -> Vec<(f32, &SearchDocument)> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<(f32, &SearchDocument)> = self.docs.iter()
            .map(|doc| {
                let mut score = 0.0;
                let content = format!("{} {}", doc.summary, doc.description).to_lowercase();
                
                if content.contains(&query_lower) {
                    score += 1.0;
                }
                
                // Simple partial match count
                let match_count = query_lower.chars()
                    .filter(|c| content.contains(*c))
                    .count();
                score += match_count as f32 * 0.01;
                
                (score, doc)
            })
            .collect();

        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        results.into_iter().take(top).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SearchDocument};

    #[test]
    fn test_string_match_provider() {
        let docs = vec![
            SearchDocument {
                id: "test1".to_string(),
                summary: "Order API".to_string(),
                description: "Create a new order".to_string(),
                vector: vec![],
            },
            SearchDocument {
                id: "test2".to_string(),
                summary: "User API".to_string(),
                description: "Get user info".to_string(),
                vector: vec![],
            },
        ];
        
        let provider = StringMatchProvider { docs };
        let results = provider.search("Order", 1);
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.id, "test1");
        assert!(results[0].0 > 0.0);
    }
}
