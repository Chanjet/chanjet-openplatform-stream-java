use ort::session::Session;
use ort::value::Tensor;
use ort::inputs;
use tokenizers::Tokenizer;
use anyhow::{Result, anyhow};

pub struct ONNXEmbedder {
    session: Session,
    tokenizer: Tokenizer,
}

impl ONNXEmbedder {
    pub fn new(model_path: &str, tokenizer_path: &str) -> Result<Self> {
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow!("Failed to load tokenizer: {}", e))?;
        
        let session = Session::builder()
            .map_err(|e| anyhow!("Failed to create session builder: {}", e))?
            .commit_from_file(model_path)
            .map_err(|e| anyhow!("Failed to load model: {}", e))?;

        Ok(Self {
            session,
            tokenizer,
        })
    }

    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        let encoding = self.tokenizer.encode(text, true)
            .map_err(|e| anyhow!("Tokenization failed: {}", e))?;
        
        let ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&id| id as i64).collect();
        let types: Vec<i64> = encoding.get_type_ids().iter().map(|&id| id as i64).collect();

        let seq_len = ids.len();
        let shape = vec![1i64, seq_len as i64];

        let input_ids = Tensor::from_array((shape.clone(), ids))
            .map_err(|e| anyhow!("Failed to create input_ids tensor: {}", e))?;
        let attention_mask = Tensor::from_array((shape.clone(), mask))
            .map_err(|e| anyhow!("Failed to create attention_mask tensor: {}", e))?;
        let token_type_ids = Tensor::from_array((shape, types))
            .map_err(|e| anyhow!("Failed to create token_type_ids tensor: {}", e))?;

        let inputs = inputs! {
            "input_ids" => input_ids,
            "attention_mask" => attention_mask,
            "token_type_ids" => token_type_ids,
        };

        let outputs = self.session.run(inputs)
            .map_err(|e| anyhow!("Inference failed: {}", e))?;
        
        let (shape, data) = outputs[0].try_extract_tensor::<f32>()
            .map_err(|e| anyhow!("Extraction failed: {}", e))?;
        
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

    pub fn rebuild_index(&mut self, spec: &serde_json::Value) -> Result<crate::index::SearchIndex> {
        let paths = spec["paths"].as_object().ok_or_else(|| anyhow!("Invalid spec: paths not found"))?;
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
