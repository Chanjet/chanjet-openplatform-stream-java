use ort::inputs;
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

pub struct ONNXEmbedder {
    session: Session,
    tokenizer: Tokenizer,
}

static ORT_INIT: std::sync::OnceLock<Result<(), String>> = std::sync::OnceLock::new();

fn ensure_ort_initialized() -> Result<(), String> {
    ORT_INIT
        .get_or_init(|| {
            if let Ok(path) = std::env::var("ORT_DYLIB_PATH") {
                if let Ok(mut builder) = ort::init_from(&path) {
                    builder = builder.with_name("cowen_search");
                    let _ = builder.commit();
                    return Ok(());
                }
            }

            if let Ok(exe_path) = std::env::current_exe() {
                let lib_name = match std::env::consts::OS {
                    "macos" => "libonnxruntime.dylib",
                    "windows" => "onnxruntime.dll",
                    _ => "libonnxruntime.so",
                };

                let mut current = exe_path.as_path();
                let mut found_path = None;

                while let Some(parent) = current.parent() {
                    let candidates = vec![
                        parent.join(lib_name),
                        parent.join("deps").join(lib_name),
                        parent.join(".libs").join(lib_name),
                        parent
                            .join("target")
                            .join("debug")
                            .join("deps")
                            .join(".libs")
                            .join(lib_name),
                        parent
                            .join("target")
                            .join("release")
                            .join("deps")
                            .join(".libs")
                            .join(lib_name),
                        parent
                            .join("target")
                            .join("llvm-cov-target")
                            .join("debug")
                            .join(".libs")
                            .join(lib_name),
                    ];
                    for candidate in &candidates {
                        if candidate.exists() {
                            found_path = Some(candidate.clone());
                            break;
                        }
                    }
                    if found_path.is_some() {
                        break;
                    }
                    current = parent;
                }

                if let Some(path) = found_path {
                    if let Ok(mut builder) = ort::init_from(&path) {
                        builder = builder.with_name("cowen_search");
                        let _ = builder.commit();
                        Ok(())
                    } else {
                        Err(format!(
                            "[ONNX-INIT] Found library at {:?} but init_from failed!",
                            path
                        ))
                    }
                } else {
                    Err(format!(
                        "[ONNX-INIT] Could not find {} in any parent directory of {:?}",
                        lib_name, exe_path
                    ))
                }
            } else {
                Err("[ONNX-INIT] Failed to get current_exe and ORT_DYLIB_PATH not set".to_string())
            }
        })
        .clone()
}

impl ONNXEmbedder {
    pub fn new(model_path: &str, tokenizer_path: &str) -> anyhow::Result<Self> {
        ensure_ort_initialized().map_err(|e| anyhow::anyhow!(e))?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        let session = Session::builder()
            .map_err(|e| anyhow::anyhow!("Failed to create session builder: {}", e))?
            .commit_from_file(model_path)
            .map_err(|e| anyhow::anyhow!("Failed to load model: {}", e))?;

        Ok(Self { session, tokenizer })
    }

    pub fn embed(&mut self, text: &str) -> anyhow::Result<Vec<f32>> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let mask: Vec<i64> = encoding
            .get_attention_mask()
            .iter()
            .map(|&id| id as i64)
            .collect();
        let types: Vec<i64> = encoding
            .get_type_ids()
            .iter()
            .map(|&id| id as i64)
            .collect();

        let seq_len = ids.len();
        let shape = vec![1i64, seq_len as i64];

        let input_ids = Tensor::from_array((shape.clone(), ids))
            .map_err(|e| anyhow::anyhow!("Failed to create input_ids tensor: {}", e))?;
        let attention_mask = Tensor::from_array((shape.clone(), mask))
            .map_err(|e| anyhow::anyhow!("Failed to create attention_mask tensor: {}", e))?;
        let token_type_ids = Tensor::from_array((shape, types))
            .map_err(|e| anyhow::anyhow!("Failed to create token_type_ids tensor: {}", e))?;

        let inputs = inputs! {
            "input_ids" => input_ids,
            "attention_mask" => attention_mask,
            "token_type_ids" => token_type_ids,
        };

        let outputs = self
            .session
            .run(inputs)
            .map_err(|e| anyhow::anyhow!("Inference failed: {}", e))?;

        let (shape, data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow::anyhow!("Extraction failed: {}", e))?;

        let dim = shape[2] as usize;
        let mut mean_vec = vec![0.0f32; dim];
        for i in 0..dim {
            let mut sum = 0.0f32;
            for j in 0..seq_len {
                sum += data[j * dim + i];
            }
            mean_vec[i] = sum / (seq_len as f32);
        }

        let mut norm: f32 = 0.0;
        for v in &mean_vec {
            norm += v * v;
        }
        norm = norm.sqrt();

        if norm > 0.0 {
            for v in &mut mean_vec {
                *v /= norm;
            }
        }

        Ok(mean_vec)
    }

    pub fn rebuild_index(
        &mut self,
        spec: &serde_json::Value,
    ) -> anyhow::Result<crate::index::SearchIndex> {
        let paths = spec["paths"]
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("Invalid spec: paths not found"))?;
        let mut index = crate::index::SearchIndex::default();

        for (path, methods) in paths {
            if let Some(methods_obj) = methods.as_object() {
                for (method, op) in methods_obj {
                    let summary = op["summary"].as_str().unwrap_or("");
                    let desc = op["description"].as_str().unwrap_or("");

                    let text = format!("{} {} {}", summary, desc, path).trim().to_string();
                    let vector = self.embed(&text)?;

                    index.docs.push(crate::index::SearchDocument {
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
