use sha2::{Digest, Sha256};
use std::fs;

#[test]
fn test_model_checksum() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_path = std::path::Path::new(&manifest_dir);

    let models_dir = match find_models_dir(manifest_path) {
        Some(dir) => dir,
        None => {
            println!("Models directory not found. Skipping checksum test.");
            return;
        }
    };

    let model_path = models_dir.join("model_quantized.onnx");
    let checksum_path = models_dir.join("model_quantized.onnx.sha256");

    if !model_path.exists() || !checksum_path.exists() {
        println!(
            "Model or checksum file not found in {:?}. Skipping test.",
            models_dir
        );
        return;
    }
    let checksum_contents = fs::read_to_string(&checksum_path).expect("Failed to read checksum");
    let expected = checksum_contents
        .split_whitespace()
        .next()
        .expect("Invalid checksum format");
    let data = fs::read(&model_path).expect("Failed to read model file");
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let actual = format!("{:x}", hasher.finalize());
    assert_eq!(
        actual, expected,
        "Checksum mismatch: expected {}, got {}",
        expected, actual
    );
}

fn find_models_dir(start: &std::path::Path) -> Option<std::path::PathBuf> {
    let local_models = start.join("assets").join("models");
    if local_models.exists() {
        return Some(local_models);
    }
    None
}
