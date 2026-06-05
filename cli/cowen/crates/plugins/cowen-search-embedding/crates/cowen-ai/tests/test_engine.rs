use cowen_ai::engine::{Engine, Document, cosine_similarity};

#[test]
fn test_cosine_similarity() {
    let v1 = vec![1.0, 0.0, 0.0];
    let v2 = vec![1.0, 0.0, 0.0];
    assert!((cosine_similarity(&v1, &v2) - 1.0).abs() < 1e-6);

    let v3 = vec![0.0, 1.0, 0.0];
    assert!((cosine_similarity(&v1, &v3) - 0.0).abs() < 1e-6);

    let v4 = vec![-1.0, 0.0, 0.0];
    assert!((cosine_similarity(&v1, &v4) - (-1.0)).abs() < 1e-6);
}

#[test]
fn test_engine_search() {
    let docs = vec![
        Document {
            id: "doc1".to_string(),
            metadata: "metadata 1".to_string(),
            description: "description of document one".to_string(),
            vector: vec![1.0, 0.0, 0.0],
        },
        Document {
            id: "doc2".to_string(),
            metadata: "metadata 2".to_string(),
            description: "another document about rust".to_string(),
            vector: vec![0.0, 1.0, 0.0],
        },
    ];
    let engine = Engine::new(docs);

    // Search by vector
    let query_vector = vec![1.0, 0.1, 0.0];
    let results = engine.search(&query_vector, "document", 10);
    assert!(!results.is_empty());
    assert_eq!(results[0].id, "doc1");

    // Search by text boost
    let query_vector_empty = vec![0.0, 0.0, 1.0]; // No vector match
    let results_text = engine.search(&query_vector_empty, "rust", 10);
    assert!(!results_text.is_empty());
    assert_eq!(results_text[0].id, "doc2");
}
