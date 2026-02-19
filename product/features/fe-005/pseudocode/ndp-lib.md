# fe-005 Pseudocode: ndp-lib

## Location: `crates/ndp-lib/src/gold/embeddings/`

### New Files

#### `text.rs` -- TextEmbedder trait + error types

```rust
// crates/ndp-lib/src/gold/embeddings/text.rs

use anyhow::Result;

/// Error types specific to text embedding
#[derive(Debug, thiserror::Error)]
pub enum TextEmbeddingError {
    #[error("Model not loaded: {reason}")]
    ModelNotLoaded { reason: String },

    #[error("Inference failed: {reason}")]
    InferenceFailed { reason: String },

    #[error("Tokenization failed: {reason}")]
    TokenizationFailed { reason: String },

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}

/// Model-agnostic trait for text -> vector embedding.
/// No ONNX types in the interface.
pub trait TextEmbedder: Send + Sync {
    /// Embed texts into vectors. Returns one Vec<f32> per input.
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;

    /// Output dimensionality.
    fn dimensions(&self) -> usize;
}
```

#### `onnx.rs` -- OnnxEmbedder implementation

```rust
// crates/ndp-lib/src/gold/embeddings/onnx.rs

use ort::{Session, SessionBuilder};
use tokenizers::Tokenizer;
use ndarray::{Array2, Axis};
use std::path::Path;

pub struct OnnxEmbedder {
    session: Session,
    tokenizer: Tokenizer,
    dimensions: usize,
    max_length: usize,
}

impl OnnxEmbedder {
    pub fn new(model_path: &Path, tokenizer_path: &Path, dimensions: usize) -> Result<Self> {
        // 1. Create ONNX session with 2 intra-op threads (Pi 5 has 4 cores)
        let session = SessionBuilder::new()?
            .with_intra_threads(2)?
            .commit_from_file(model_path)?;

        // 2. Load tokenizer from JSON file
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Tokenizer load failed: {}", e))?;

        Ok(Self {
            session,
            tokenizer,
            dimensions,
            max_length: 512,
        })
    }

    fn tokenize_batch(&self, texts: &[&str]) -> Result<TokenizedBatch> {
        // For each text:
        //   encoding = tokenizer.encode(text, add_special_tokens=true)
        //   truncate to max_length
        //   collect input_ids, attention_mask, token_type_ids
        // Pad all sequences to max length in batch
        // Return as 2D arrays: [batch_size, seq_len]

        let encodings = self.tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let batch_size = encodings.len();
        let max_len = encodings.iter()
            .map(|e| e.get_ids().len().min(self.max_length))
            .max()
            .unwrap_or(0);

        // Build padded arrays
        let mut input_ids = Array2::<i64>::zeros((batch_size, max_len));
        let mut attention_mask = Array2::<i64>::zeros((batch_size, max_len));

        for (i, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let len = ids.len().min(max_len);
            for j in 0..len {
                input_ids[[i, j]] = ids[j] as i64;
                attention_mask[[i, j]] = mask[j] as i64;
            }
        }

        Ok(TokenizedBatch { input_ids, attention_mask })
    }

    fn mean_pool(hidden_state: &Array2<f32>, attention_mask: &Array2<i64>) -> Vec<Vec<f32>> {
        // For each row in batch:
        //   mask_expanded = attention_mask broadcasted to hidden_state dims
        //   sum = sum(hidden_state * mask_expanded, axis=seq_len)
        //   count = sum(mask_expanded, axis=seq_len)
        //   pooled = sum / count
        //   normalized = pooled / ||pooled||  (L2 normalize)
        // Return Vec of normalized vectors

        let batch_size = hidden_state.nrows();
        let dims = hidden_state.ncols();
        let mut results = Vec::with_capacity(batch_size);

        for i in 0..batch_size {
            let mut sum = vec![0.0f32; dims];
            let mut count = 0.0f32;

            // Note: hidden_state shape is [batch*seq_len, dims] after reshape
            // This pseudocode assumes per-token iteration
            for j in 0..attention_mask.ncols() {
                if attention_mask[[i, j]] == 1 {
                    // Add hidden state for this token
                    count += 1.0;
                    for d in 0..dims {
                        // sum[d] += hidden_state[i*seq_len + j][d]
                    }
                }
            }

            // Divide by count
            if count > 0.0 {
                for d in 0..dims {
                    sum[d] /= count;
                }
            }

            // L2 normalize
            let norm: f32 = sum.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 1e-12 {
                for d in 0..dims {
                    sum[d] /= norm;
                }
            }

            results.push(sum);
        }

        results
    }
}

impl TextEmbedder for OnnxEmbedder {
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        // 1. Tokenize
        let batch = self.tokenize_batch(texts)?;

        // 2. Run ONNX inference
        //    Inputs: input_ids, attention_mask (both i64 arrays)
        //    Output: last_hidden_state [batch, seq_len, dims]
        let outputs = self.session.run(ort::inputs![
            batch.input_ids.clone(),
            batch.attention_mask.clone(),
        ]?)?;

        // 3. Extract output tensor
        let output_tensor = outputs[0].try_extract_tensor::<f32>()?;

        // 4. Mean pooling over sequence dimension with attention mask
        let pooled = Self::mean_pool(&output_tensor, &batch.attention_mask);

        // 5. Verify dimensions
        for vec in &pooled {
            if vec.len() != self.dimensions {
                return Err(TextEmbeddingError::DimensionMismatch {
                    expected: self.dimensions,
                    actual: vec.len(),
                }.into());
            }
        }

        Ok(pooled)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

struct TokenizedBatch {
    input_ids: Array2<i64>,
    attention_mask: Array2<i64>,
}
```

#### `preprocessing.rs` -- TextPreprocessor trait + PassthroughPreprocessor

```rust
// crates/ndp-lib/src/gold/embeddings/preprocessing.rs

pub trait TextPreprocessor: Send + Sync {
    fn preprocess(&self, text: &str) -> String;
    fn name(&self) -> &str;
}

pub struct PassthroughPreprocessor;

impl TextPreprocessor for PassthroughPreprocessor {
    fn preprocess(&self, text: &str) -> String {
        text.to_string()
    }
    fn name(&self) -> &str {
        "passthrough"
    }
}

/// Factory function: config type string -> preprocessor instance
pub fn create_preprocessor(preprocessing_type: &str) -> Box<dyn TextPreprocessor> {
    match preprocessing_type {
        "passthrough" | "" => Box::new(PassthroughPreprocessor),
        other => {
            tracing::warn!("Unknown preprocessing type '{}', using passthrough", other);
            Box::new(PassthroughPreprocessor)
        }
    }
}
```

#### `model_manager.rs` -- Model loading and download

```rust
// crates/ndp-lib/src/gold/embeddings/model_manager.rs

use std::path::{Path, PathBuf};

pub struct ModelPaths {
    pub model: PathBuf,      // model.onnx
    pub tokenizer: PathBuf,  // tokenizer.json
}

pub struct ModelManager {
    volume_path: PathBuf,
}

impl ModelManager {
    pub fn new(volume_path: &Path) -> Self {
        Self { volume_path: volume_path.to_path_buf() }
    }

    pub async fn ensure_model(&self, model_id: &str) -> Result<ModelPaths> {
        let model_dir = self.volume_path.join(model_id);
        let model_file = model_dir.join("model.onnx");
        let tokenizer_file = model_dir.join("tokenizer.json");

        if model_file.exists() && tokenizer_file.exists() {
            info!("Model '{}' found on volume", model_id);
            return Ok(ModelPaths { model: model_file, tokenizer: tokenizer_file });
        }

        info!("Model '{}' not found, downloading...", model_id);
        std::fs::create_dir_all(&model_dir)?;

        // Download model.onnx from HuggingFace
        self.download_file(
            &format!("https://huggingface.co/{model_id}/resolve/main/model.onnx"),
            &model_file,
        ).await?;

        // Download tokenizer.json
        self.download_file(
            &format!("https://huggingface.co/{model_id}/resolve/main/tokenizer.json"),
            &tokenizer_file,
        ).await?;

        Ok(ModelPaths { model: model_file, tokenizer: tokenizer_file })
    }

    async fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        // Use reqwest with retry (3 attempts, exponential backoff)
        // Stream to file to avoid OOM on large models
        // Log progress: "Downloading {url} -> {dest} ({bytes}/{total})"
        let client = reqwest::Client::new();
        let mut retries = 3;
        let mut backoff = std::time::Duration::from_secs(1);

        loop {
            match client.get(url).send().await {
                Ok(response) if response.status().is_success() => {
                    let bytes = response.bytes().await?;
                    std::fs::write(dest, &bytes)?;
                    info!("Downloaded {} ({} bytes)", dest.display(), bytes.len());
                    return Ok(());
                }
                Ok(response) => {
                    warn!("Download failed: HTTP {}", response.status());
                }
                Err(e) => {
                    warn!("Download error: {}", e);
                }
            }

            retries -= 1;
            if retries == 0 {
                return Err(anyhow::anyhow!("Failed to download {} after 3 attempts", url));
            }
            tokio::time::sleep(backoff).await;
            backoff *= 2;
        }
    }
}
```

#### `text_config.rs` -- Configuration types

```rust
// crates/ndp-lib/src/gold/embeddings/text_config.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEmbeddingConfig {
    pub model: String,
    #[serde(default = "default_quantization")]
    pub quantization: String,
    pub dimensions: usize,
    #[serde(default)]
    pub preprocessing: PreprocessingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PreprocessingConfig {
    #[serde(rename = "type", default = "default_passthrough")]
    pub preprocessing_type: String,
}

fn default_quantization() -> String { "int8".to_string() }
fn default_passthrough() -> String { "passthrough".to_string() }
```

### Modified File: `mod.rs`

```rust
// Add to crates/ndp-lib/src/gold/embeddings/mod.rs

pub mod text;
pub mod onnx;
pub mod preprocessing;
pub mod model_manager;
pub mod text_config;
```
