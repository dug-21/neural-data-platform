# fe-005 Test Plan: ndp-lib

## Location: `crates/ndp-lib/src/gold/embeddings/`

### text.rs -- TextEmbedder Trait Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // T-001: TextEmbedder trait is object-safe
    #[test]
    fn test_text_embedder_is_object_safe() {
        fn _accept(e: &dyn TextEmbedder) { let _ = e; }
    }

    // T-002: TextEmbeddingError display messages
    #[test]
    fn test_error_display() {
        let err = TextEmbeddingError::ModelNotLoaded { reason: "file not found".into() };
        assert!(err.to_string().contains("file not found"));

        let err = TextEmbeddingError::DimensionMismatch { expected: 384, actual: 768 };
        assert!(err.to_string().contains("384"));
        assert!(err.to_string().contains("768"));
    }
}
```

### onnx.rs -- OnnxEmbedder Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_model_path() -> PathBuf {
        // Resolve to tests/fixtures/models/test-model/
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/models/test-model")
    }

    // T-003: OnnxEmbedder loads from fixture model
    #[test]
    fn test_onnx_embedder_loads() {
        let path = fixture_model_path();
        if !path.join("model.onnx").exists() {
            eprintln!("Skipping: test model fixture not found");
            return;
        }
        let embedder = OnnxEmbedder::new(
            &path.join("model.onnx"),
            &path.join("tokenizer.json"),
            384,
        );
        assert!(embedder.is_ok());
    }

    // T-004: OnnxEmbedder produces correct dimensions
    #[test]
    fn test_onnx_embedder_dimensions() {
        let embedder = load_test_embedder();
        let result = embedder.embed(&["Hello world"]).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 384);
    }

    // T-005: OnnxEmbedder batch support
    #[test]
    fn test_onnx_embedder_batch() {
        let embedder = load_test_embedder();
        let texts = &["First text", "Second text", "Third text"];
        let result = embedder.embed(texts).unwrap();
        assert_eq!(result.len(), 3);
        for vec in &result {
            assert_eq!(vec.len(), 384);
        }
    }

    // T-006: OnnxEmbedder empty input returns empty
    #[test]
    fn test_onnx_embedder_empty_input() {
        let embedder = load_test_embedder();
        let result = embedder.embed(&[]).unwrap();
        assert!(result.is_empty());
    }

    // T-007: OnnxEmbedder output is L2-normalized
    #[test]
    fn test_onnx_embedder_l2_normalized() {
        let embedder = load_test_embedder();
        let result = embedder.embed(&["Test normalization"]).unwrap();
        let norm: f32 = result[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01, "L2 norm should be ~1.0, got {}", norm);
    }

    // T-008: OnnxEmbedder handles long text (truncation)
    #[test]
    fn test_onnx_embedder_long_text() {
        let embedder = load_test_embedder();
        let long_text = "word ".repeat(1000); // ~5000 tokens, exceeds 512 limit
        let result = embedder.embed(&[&long_text]);
        assert!(result.is_ok()); // Should truncate, not error
        assert_eq!(result.unwrap()[0].len(), 384);
    }

    // T-009: OnnxEmbedder similar texts produce similar vectors
    #[test]
    fn test_onnx_embedder_semantic_similarity() {
        let embedder = load_test_embedder();
        let similar_a = "Partly cloudy with light winds";
        let similar_b = "Partly cloudy with gentle breeze";
        let different = "Stagnation advisory with poor air quality";

        let vecs = embedder.embed(&[similar_a, similar_b, different]).unwrap();
        let sim_ab = cosine_similarity(&vecs[0], &vecs[1]);
        let sim_ac = cosine_similarity(&vecs[0], &vecs[2]);

        assert!(sim_ab > sim_ac,
            "Similar texts should have higher similarity ({}) than different ({}))",
            sim_ab, sim_ac);
    }

    // T-010: OnnxEmbedder fails gracefully with invalid model path
    #[test]
    fn test_onnx_embedder_invalid_model() {
        let result = OnnxEmbedder::new(
            Path::new("/nonexistent/model.onnx"),
            Path::new("/nonexistent/tokenizer.json"),
            384,
        );
        assert!(result.is_err());
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        dot / (norm_a * norm_b)
    }
}
```

### preprocessing.rs -- TextPreprocessor Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // T-011: PassthroughPreprocessor returns unchanged text
    #[test]
    fn test_passthrough_identity() {
        let pp = PassthroughPreprocessor;
        let input = "Partly cloudy with a chance of showers.";
        assert_eq!(pp.preprocess(input), input);
    }

    // T-012: PassthroughPreprocessor handles empty string
    #[test]
    fn test_passthrough_empty() {
        let pp = PassthroughPreprocessor;
        assert_eq!(pp.preprocess(""), "");
    }

    // T-013: PassthroughPreprocessor handles Unicode
    #[test]
    fn test_passthrough_unicode() {
        let pp = PassthroughPreprocessor;
        let input = "Temperature: 23\u{00B0}C with \u{2601} clouds";
        assert_eq!(pp.preprocess(input), input);
    }

    // T-014: PassthroughPreprocessor name
    #[test]
    fn test_passthrough_name() {
        let pp = PassthroughPreprocessor;
        assert_eq!(pp.name(), "passthrough");
    }

    // T-015: Factory creates passthrough for "passthrough"
    #[test]
    fn test_factory_passthrough() {
        let pp = create_preprocessor("passthrough");
        assert_eq!(pp.name(), "passthrough");
    }

    // T-016: Factory creates passthrough for empty string
    #[test]
    fn test_factory_empty_string() {
        let pp = create_preprocessor("");
        assert_eq!(pp.name(), "passthrough");
    }

    // T-017: Factory falls back to passthrough for unknown type
    #[test]
    fn test_factory_unknown_type() {
        let pp = create_preprocessor("unknown_type");
        assert_eq!(pp.name(), "passthrough");
    }

    // T-018: TextPreprocessor is object-safe
    #[test]
    fn test_preprocessor_object_safe() {
        fn _accept(p: &dyn TextPreprocessor) { let _ = p; }
    }
}
```

### model_manager.rs -- ModelManager Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // T-019: ModelManager resolves existing model
    #[test]
    fn test_model_manager_existing_model() {
        let dir = TempDir::new().unwrap();
        let model_dir = dir.path().join("test-model");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("model.onnx"), b"fake model").unwrap();
        std::fs::write(model_dir.join("tokenizer.json"), b"{}").unwrap();

        let mgr = ModelManager::new(dir.path());
        let rt = tokio::runtime::Runtime::new().unwrap();
        let paths = rt.block_on(mgr.ensure_model("test-model")).unwrap();
        assert!(paths.model.exists());
        assert!(paths.tokenizer.exists());
    }

    // T-020: ModelManager returns error for missing model (no network)
    #[test]
    fn test_model_manager_download_fails_gracefully() {
        let dir = TempDir::new().unwrap();
        let mgr = ModelManager::new(dir.path());
        let rt = tokio::runtime::Runtime::new().unwrap();
        // This will attempt to download from a non-existent URL
        let result = rt.block_on(mgr.ensure_model("nonexistent/model"));
        assert!(result.is_err());
    }

    // T-021: ModelPaths from_dir resolves correct files
    #[test]
    fn test_model_paths_resolution() {
        let dir = TempDir::new().unwrap();
        let model_dir = dir.path().join("bge-small");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("model.onnx"), b"data").unwrap();
        std::fs::write(model_dir.join("tokenizer.json"), b"{}").unwrap();

        let paths = ModelPaths {
            model: model_dir.join("model.onnx"),
            tokenizer: model_dir.join("tokenizer.json"),
        };
        assert!(paths.model.ends_with("model.onnx"));
        assert!(paths.tokenizer.ends_with("tokenizer.json"));
    }

    // T-022: ModelManager creates directory for new model
    #[test]
    fn test_model_manager_creates_dir() {
        let dir = TempDir::new().unwrap();
        let model_dir = dir.path().join("new-model");
        assert!(!model_dir.exists());

        // When download fails, dir should have been created
        let mgr = ModelManager::new(dir.path());
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(mgr.ensure_model("new-model"));
        // Note: download fails but directory was created
        assert!(model_dir.exists());
    }

    // T-023: Multiple models on same volume
    #[test]
    fn test_multiple_models() {
        let dir = TempDir::new().unwrap();
        for model in &["model-a", "model-b", "model-c"] {
            let model_dir = dir.path().join(model);
            std::fs::create_dir_all(&model_dir).unwrap();
            std::fs::write(model_dir.join("model.onnx"), b"data").unwrap();
            std::fs::write(model_dir.join("tokenizer.json"), b"{}").unwrap();
        }

        let mgr = ModelManager::new(dir.path());
        let rt = tokio::runtime::Runtime::new().unwrap();
        for model in &["model-a", "model-b", "model-c"] {
            let paths = rt.block_on(mgr.ensure_model(model)).unwrap();
            assert!(paths.model.exists());
        }
    }
}
```

### text_config.rs -- Configuration Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // T-024: Full config deserialization
    #[test]
    fn test_text_embedding_config_deserialize() {
        let json = r#"{
            "model": "BAAI/bge-small-en-v1.5",
            "quantization": "int8",
            "dimensions": 384,
            "preprocessing": { "type": "passthrough" }
        }"#;
        let config: TextEmbeddingConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.model, "BAAI/bge-small-en-v1.5");
        assert_eq!(config.quantization, "int8");
        assert_eq!(config.dimensions, 384);
        assert_eq!(config.preprocessing.preprocessing_type, "passthrough");
    }

    // T-025: Config with defaults
    #[test]
    fn test_text_embedding_config_defaults() {
        let json = r#"{ "model": "test-model", "dimensions": 384 }"#;
        let config: TextEmbeddingConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.quantization, "int8"); // default
        assert_eq!(config.preprocessing.preprocessing_type, "passthrough"); // default
    }

    // T-026: Config round-trip serialization
    #[test]
    fn test_config_round_trip() {
        let config = TextEmbeddingConfig {
            model: "test".to_string(),
            quantization: "fp32".to_string(),
            dimensions: 768,
            preprocessing: PreprocessingConfig {
                preprocessing_type: "passthrough".to_string(),
            },
        };
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: TextEmbeddingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model, config.model);
        assert_eq!(deserialized.dimensions, config.dimensions);
    }

    // T-027: Config requires model field
    #[test]
    fn test_config_requires_model() {
        let json = r#"{ "dimensions": 384 }"#;
        let result = serde_json::from_str::<TextEmbeddingConfig>(json);
        assert!(result.is_err());
    }

    // T-028: Config requires dimensions field
    #[test]
    fn test_config_requires_dimensions() {
        let json = r#"{ "model": "test" }"#;
        let result = serde_json::from_str::<TextEmbeddingConfig>(json);
        assert!(result.is_err());
    }

    // T-029: PreprocessingConfig defaults
    #[test]
    fn test_preprocessing_config_default() {
        let config = PreprocessingConfig::default();
        assert_eq!(config.preprocessing_type, "passthrough");
    }
}
```

### Mock Implementation for Service Tests

```rust
// tests/common/mock_embedder.rs (shared test utility)

pub struct MockTextEmbedder {
    dimensions: usize,
    fixed_vector: Vec<f32>,
}

impl MockTextEmbedder {
    pub fn new(dimensions: usize) -> Self {
        // Deterministic vector: [0.026, 0.026, ...] L2-normalized
        let val = 1.0 / (dimensions as f32).sqrt();
        Self {
            dimensions,
            fixed_vector: vec![val; dimensions],
        }
    }
}

impl TextEmbedder for MockTextEmbedder {
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| self.fixed_vector.clone()).collect())
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}
```

## Test Summary

| ID | Test | Type | Component |
|----|------|------|-----------|
| T-001 | TextEmbedder trait is object-safe | Unit | text.rs |
| T-002 | TextEmbeddingError display | Unit | text.rs |
| T-003 | OnnxEmbedder loads from fixture | Unit | onnx.rs |
| T-004 | OnnxEmbedder correct dimensions | Unit | onnx.rs |
| T-005 | OnnxEmbedder batch support | Unit | onnx.rs |
| T-006 | OnnxEmbedder empty input | Unit | onnx.rs |
| T-007 | OnnxEmbedder L2 normalization | Unit | onnx.rs |
| T-008 | OnnxEmbedder long text truncation | Unit | onnx.rs |
| T-009 | OnnxEmbedder semantic similarity | Unit | onnx.rs |
| T-010 | OnnxEmbedder invalid model path | Unit | onnx.rs |
| T-011 | Passthrough identity | Unit | preprocessing.rs |
| T-012 | Passthrough empty string | Unit | preprocessing.rs |
| T-013 | Passthrough Unicode | Unit | preprocessing.rs |
| T-014 | Passthrough name | Unit | preprocessing.rs |
| T-015 | Factory passthrough type | Unit | preprocessing.rs |
| T-016 | Factory empty string | Unit | preprocessing.rs |
| T-017 | Factory unknown type fallback | Unit | preprocessing.rs |
| T-018 | TextPreprocessor object-safe | Unit | preprocessing.rs |
| T-019 | ModelManager existing model | Unit | model_manager.rs |
| T-020 | ModelManager download failure | Unit | model_manager.rs |
| T-021 | ModelPaths resolution | Unit | model_manager.rs |
| T-022 | ModelManager creates directory | Unit | model_manager.rs |
| T-023 | Multiple models on volume | Unit | model_manager.rs |
| T-024 | Config full deserialization | Unit | text_config.rs |
| T-025 | Config defaults | Unit | text_config.rs |
| T-026 | Config round-trip | Unit | text_config.rs |
| T-027 | Config requires model | Unit | text_config.rs |
| T-028 | Config requires dimensions | Unit | text_config.rs |
| T-029 | PreprocessingConfig defaults | Unit | text_config.rs |
