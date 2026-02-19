# fe-005: Task Decomposition

## Wave Structure

### Wave 1: Library Foundation (parallel tasks, no runtime dependencies)

| Task | Deliverable | Complexity | Description |
|------|-------------|-----------|-------------|
| W1-T1 | E-01 | M | TextEmbedder trait + error types in `crates/ndp-lib/src/gold/embeddings/text.rs` |
| W1-T2 | E-08 | S | TextPreprocessor trait + PassthroughPreprocessor in `crates/ndp-lib/src/gold/embeddings/preprocessing.rs` |
| W1-T3 | E-01 | L | OnnxEmbedder implementation in `crates/ndp-lib/src/gold/embeddings/onnx.rs` -- ort crate, tokenizer, batch inference, INT8 support |
| W1-T4 | E-02 | M | Model manager in `crates/ndp-lib/src/gold/embeddings/model_manager.rs` -- volume path resolution, download-on-first-use, model registry |
| W1-T5 | E-09 | S | Text embedding config types in `crates/ndp-lib/src/gold/embeddings/text_config.rs` -- TextEmbeddingConfig, PreprocessingConfig deserialization |

**Dependencies**: W1-T3 depends on W1-T1 (trait definition). W1-T3 depends on W1-T4 (model loading). W1-T5 is independent. W1-T2 is independent.

### Wave 2: Infrastructure (depends on Wave 1 library types)

| Task | Deliverable | Complexity | Description |
|------|-------------|-----------|-------------|
| W2-T1 | E-05 | M | gold.text_embeddings DDL -- table, HNSW index, hypertable. Either via ndp-gold-ddl generator or init-script |
| W2-T2 | E-09 | S | Domain schema update -- add `embedding` block to `config/schemas/domain.schema.json` |
| W2-T3 | E-07 | M | New crate: `apps/ndp-embedder/` -- Cargo.toml, main.rs skeleton, AppConfig, service loop |
| W2-T4 | E-07 | S | Dockerfile at `docker/embedder/Dockerfile` following intelligence Dockerfile pattern |

**Dependencies**: W2-T1 needs text_config types from W1-T5. W2-T3 needs all Wave 1 library work. W2-T2 and W2-T4 are Wave 1-independent but logically grouped here.

### Wave 3: Integration (depends on Wave 2)

| Task | Deliverable | Complexity | Description |
|------|-------------|-----------|-------------|
| W3-T1 | E-07 | M | ndp-embedder service implementation -- Gold text view query, preprocessing, embedding, storage write cycle |
| W3-T2 | E-07 | S | Compose entry in `deploy/pi/docker-compose.yml` -- service definition, volume mount, profile |
| W3-T3 | E-07 | S | deploy.sh integration -- Phase 6 text embedding DDL execution, model volume creation |
| W3-T4 | -- | M | Integration testing -- end-to-end pipeline with test text data |

**Dependencies**: W3-T1 needs W2-T3 (crate skeleton) + W2-T1 (DDL). W3-T2 needs W2-T4 (Dockerfile). W3-T3 needs W2-T1 (DDL).

## Task-to-Deliverable Map

| Deliverable | Tasks | Status |
|-------------|-------|--------|
| E-01: TextEmbedder trait + OnnxEmbedder | W1-T1, W1-T3 | Wave 1 |
| E-02: Model management | W1-T4 | Wave 1 |
| E-05: gold.text_embeddings table | W2-T1 | Wave 2 |
| E-07: ndp-embedder container | W2-T3, W2-T4, W3-T1, W3-T2, W3-T3 | Wave 2-3 |
| E-08: Preprocessing pipeline | W1-T2 | Wave 1 |
| E-09: Domain schema update | W1-T5, W2-T2 | Wave 1-2 |

## Complexity Summary

| Complexity | Count |
|-----------|-------|
| S (Small, <1 day) | 5 |
| M (Medium, 1-2 days) | 6 |
| L (Large, 2-3 days) | 1 |
| **Total** | **12 tasks** |

## Critical Path

```
W1-T1 (TextEmbedder trait)
  -> W1-T3 (OnnxEmbedder) + W1-T4 (model manager)
    -> W2-T3 (ndp-embedder crate)
      -> W3-T1 (service implementation)
        -> W3-T4 (integration testing)
```

The critical path runs through the trait definition, ONNX implementation, and service integration. Preprocessing (W1-T2), config types (W1-T5), DDL (W2-T1), schema update (W2-T2), Dockerfile (W2-T4), compose (W3-T2), and deploy.sh (W3-T3) are all parallelizable off the critical path within their waves.

## Risk Items

1. **ort crate ARM64 compatibility**: The `ort` crate wraps ONNX Runtime. Must verify it builds and runs on aarch64. Fallback: use `tract` crate (pure Rust ONNX, no C++ dependency) at the cost of performance.
2. **Model download on Pi**: Pi may have limited bandwidth. First-use download of ~33MB model must handle timeouts and retries.
3. **Tokenizer crate size**: The `tokenizers` crate from HuggingFace is large. May need to evaluate lighter alternatives or embed a pre-built vocabulary.
4. **Memory budget**: 512MB must cover model (~33MB INT8) + tokenizer + inference runtime. Should be fine but needs validation on Pi.
