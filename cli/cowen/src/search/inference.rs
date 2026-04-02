use ort::{Session, inputs, GraphOptimizationLevel};
use tokenizers::Tokenizer;
use anyhow::{Result, anyhow};
use ndarray::{Array2};

pub struct ONNXEmbedder {
    session: Session,
    tokenizer: Tokenizer,
    dimension: usize,
    max_len: usize,
}

impl ONNXEmbedder {
    pub fn new(model_path: &str, tokenizer_path: &str) -> Result<Self> {
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow!("Failed to load tokenizer from {}: {}", tokenizer_path, e))?;
        
        // Initialize ONNX Session (ort 2.0-rc.9 API)
        let session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(1)?
            .commit_from_file(model_path)?;

        Ok(Self {
            session,
            tokenizer,
            dimension: 512, // From Go implementation
            max_len: 512,   // From Go implementation
        })
    }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let encoding = self.tokenizer.encode(text, true)
            .map_err(|e| anyhow!("Tokenization failed: {}", e))?;
        
        let ids = encoding.get_ids();
        let mask = encoding.get_attention_mask();
        let types = encoding.get_type_ids();

        let mut input_ids = Array2::<i64>::zeros((1, self.max_len));
        let mut attention_mask = Array2::<i64>::zeros((1, self.max_len));
        let mut token_type_ids = Array2::<i64>::zeros((1, self.max_len));

        let len = std::cmp::min(ids.len(), self.max_len);
        for i in 0..len {
            input_ids[[0, i]] = ids[i] as i64;
            attention_mask[[0, i]] = mask[i] as i64;
            token_type_ids[[0, i]] = types[i] as i64;
        }

        // Run session (ort 2.0-rc.9 API)
        let outputs = self.session.run(inputs![
            "input_ids" => input_ids,
            "attention_mask" => attention_mask,
            "token_type_ids" => token_type_ids,
        ]?)?;
        
        // Output shape is [1, 512, 512]
        let output = outputs["last_hidden_state"]
            .try_extract_tensor::<f32>()?;
        
        let view = output.view();
        
        // Extract CLS token ([0, 0, :])
        let mut vec = Vec::with_capacity(self.dimension);
        for i in 0..self.dimension {
            vec.push(view[[0, 0, i]]);
        }

        // L2 Normalization
        let mut norm = 0.0;
        for v in &vec {
            norm += v * v;
        }
        norm = norm.sqrt();
        if norm > 0.0 {
            for v in &mut vec {
                *v /= norm;
            }
        }

        Ok(vec)
    }
}
