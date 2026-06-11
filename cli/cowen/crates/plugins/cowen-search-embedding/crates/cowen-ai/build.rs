use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
fn main() {
    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-lib=dylib=stdc++");
        println!("cargo:rustc-link-lib=dylib=atomic");
    }

    // Reconstruction logic for model chunks
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

    // Search upwards to find repository root and models directory
    if let Some(models_dir) = find_models_dir(&manifest_dir) {
        let chunks_dir = models_dir.join("chunks");

        if chunks_dir.exists() {
            // Reconstruct model_quantized.onnx
            let onnx_file = models_dir.join("model_quantized.onnx");
            let onnx_chunks = find_chunks(&chunks_dir, "model_quantized.onnx.chunk");
            if !onnx_chunks.is_empty() {
                for chunk in &onnx_chunks {
                    println!("cargo:rerun-if-changed={}", chunk.display());
                }
                if is_rebuild_needed(&onnx_file, &onnx_chunks) {
                    reconstruct_file(&onnx_file, &onnx_chunks);
                    verify_checksum(&onnx_file, &models_dir.join("model_quantized.onnx.sha256"));
                }
            }

            // Reconstruct model_quantized.onnx_data
            let onnx_data_file = models_dir.join("model_quantized.onnx_data");
            let onnx_data_chunks = find_chunks(&chunks_dir, "model_quantized.onnx_data.chunk");
            if !onnx_data_chunks.is_empty() {
                for chunk in &onnx_data_chunks {
                    println!("cargo:rerun-if-changed={}", chunk.display());
                }
                // Ensure the reconstructed files are available via env vars
                println!("cargo:rustc-env=MODEL_ONNX={}", onnx_file.display());
                println!(
                    "cargo:rustc-env=MODEL_ONNX_DATA={}",
                    onnx_data_file.display()
                );
                if is_rebuild_needed(&onnx_data_file, &onnx_data_chunks) {
                    reconstruct_file(&onnx_data_file, &onnx_data_chunks);
                    verify_checksum(
                        &onnx_data_file,
                        &models_dir.join("model_quantized.onnx_data.sha256"),
                    );
                }
            }
        }
    }
}

fn find_models_dir(start: &Path) -> Option<PathBuf> {
    let local_models = start.join("assets").join("models");
    if local_models.exists() {
        return Some(local_models);
    }
    None
}

fn find_chunks(chunks_dir: &Path, prefix: &str) -> Vec<PathBuf> {
    let mut chunks = Vec::new();
    if let Ok(entries) = fs::read_dir(chunks_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                    if filename.starts_with(prefix) {
                        chunks.push(path);
                    }
                }
            }
        }
    }
    chunks.sort();
    chunks
}

fn is_rebuild_needed(output_file: &Path, chunks: &[PathBuf]) -> bool {
    if !output_file.exists() {
        return true;
    }
    let total_chunks_size: u64 = chunks
        .iter()
        .map(|p| fs::metadata(p).map(|m| m.len()).unwrap_or(0))
        .sum();
    let output_size = fs::metadata(output_file).map(|m| m.len()).unwrap_or(0);
    output_size != total_chunks_size
}

fn reconstruct_file(output_file: &Path, chunks: &[PathBuf]) {
    let filename = output_file
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    println!("cargo:warning=Reconstructing {} from chunks...", filename);
    let mut output_writer = fs::File::create(output_file)
        .unwrap_or_else(|_| panic!("Failed to create reconstructed file {:?}", output_file));

    use std::io::Write;
    for chunk_path in chunks {
        let chunk_data = fs::read(chunk_path)
            .unwrap_or_else(|_| panic!("Failed to read chunk: {:?}", chunk_path));
        output_writer
            .write_all(&chunk_data)
            .unwrap_or_else(|_| panic!("Failed to write chunk data from {:?}", chunk_path));
    }
    output_writer
        .flush()
        .expect("Failed to flush reconstructed file data");
    println!(
        "cargo:warning=Successfully reconstructed {} from {} chunks",
        filename,
        chunks.len()
    );
}

fn verify_checksum(output: &Path, checksum_path: &Path) {
    if !checksum_path.exists() {
        println!(
            "cargo:warning=Checksum file not found for {}, skipping verification",
            output.display()
        );
        return;
    }
    // Read expected checksum (first token before whitespace)
    let checksum_contents =
        fs::read_to_string(checksum_path).expect("Failed to read checksum file");
    let expected = checksum_contents
        .split_whitespace()
        .next()
        .expect("Invalid checksum file format");

    // Compute actual SHA-256 of the output file
    let mut hasher = Sha256::new();
    let data = fs::read(output).expect("Failed to read rebuilt model file");
    hasher.update(&data);
    let actual = format!("{:x}", hasher.finalize());

    if actual != expected {
        panic!(
            "Checksum mismatch for {}: expected {}, got {}",
            output.display(),
            expected,
            actual
        );
    } else {
        println!("cargo:warning=Checksum verified for {}", output.display());
    }
}
