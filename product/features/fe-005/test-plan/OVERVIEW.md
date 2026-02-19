# fe-005: Test Plan Overview

## Test Strategy

fe-005 introduces a new ONNX inference pipeline and a new container. Testing must cover four layers:

1. **Unit tests**: Trait implementations, preprocessing, config deserialization, model manager logic
2. **Integration tests**: End-to-end embedding pipeline with real ONNX model (or fixture)
3. **Container tests**: ndp-embedder startup, config loading, graceful degradation
4. **Schema tests**: Domain schema validation, DDL correctness

## Test Environment Requirements

### ONNX Model Fixtures

The integration test environment needs ONNX model fixtures for deterministic testing. Two strategies:

**Strategy A (Preferred): Tiny test model**
- Create a minimal ONNX model (~1MB) that accepts tokenized input and produces 384D vectors
- Store in `tests/fixtures/models/test-model/model.onnx` + `tokenizer.json`
- This model produces random but deterministic vectors (seeded)
- Advantage: tests run without network, fast, deterministic
- Creation: use Python `torch.nn.Linear(128, 384)` exported to ONNX

**Strategy B: Mock trait implementation**
- Create `MockTextEmbedder` implementing `TextEmbedder` that returns fixed vectors
- Use for service-level tests where ONNX inference quality doesn't matter
- Advantage: no ONNX dependency in test binary

**Recommendation**: Use Strategy A for OnnxEmbedder unit tests (validates real ONNX inference), Strategy B for service-level integration tests (validates the pipeline around the embedder).

### Database Fixtures

Integration tests need:
- pgvector extension enabled (existing from fe-004 test infrastructure)
- `gold.text_embeddings` table created (from init-script `004-text-embeddings.sql`)
- Test Gold text view: `gold.test_domain_text_latest` with sample NWS forecast text

```sql
-- Test fixture: create a fake Gold text view for testing
CREATE VIEW gold.test_domain_text_latest AS
SELECT
    '2026-02-17 12:00:00+00'::timestamptz AS bucket,
    'nws-forecast-hourly'::text AS stream_id,
    'detailed_forecast'::text AS column_name,
    'Partly cloudy with a chance of showers. Winds from the south at 10 mph.'::text AS text_value
UNION ALL
SELECT
    '2026-02-17 13:00:00+00'::timestamptz,
    'nws-forecast-hourly',
    'short_forecast',
    'Chance Showers'
;
```

### Text Data Fixtures

Sample NWS forecast texts for embedding quality validation:

```rust
const TEST_TEXTS: &[&str] = &[
    "Partly cloudy with a chance of showers. Winds from the south at 10 mph.",
    "Clear skies and light winds. High near 75.",
    "Stagnation advisory in effect. Air quality may be unhealthy for sensitive groups.",
    "Gusty winds will transport smoke from nearby wildfires.",
];
```

## Integration Surfaces

| Surface | Test Type | What We Verify |
|---------|-----------|----------------|
| TextEmbedder trait | Unit | Contract: embed() returns correct dimensions, handles empty input |
| OnnxEmbedder | Unit + Integration | ONNX inference produces 384D vectors, batch support works |
| TextPreprocessor | Unit | Passthrough returns unchanged text, factory handles unknown types |
| ModelManager | Unit + Integration | Volume path resolution, download retry logic |
| EmbeddingService | Integration | Full cycle: query Gold text view -> preprocess -> embed -> store |
| gold.text_embeddings DDL | Schema | Table exists, HNSW index exists, hypertable created |
| Domain schema | Unit | text_embedding block validates, backward compatible |
| Container startup | Container | Config loading, model loading, graceful shutdown |

## Test Count Estimates

| Component | Unit Tests | Integration Tests | Total |
|-----------|-----------|------------------|-------|
| ndp-lib (text, onnx, preprocessing, model_manager, config) | ~35 | ~5 | ~40 |
| ndp-embedder (service, config) | ~10 | ~5 | ~15 |
| deploy (DDL, schema) | ~5 | ~3 | ~8 |
| **Total** | **~50** | **~13** | **~63** |

## CI Considerations

- ONNX Runtime must be available in CI (cargo dependency handles this via ort crate)
- Tiny test model fixture must be checked into the repo (~1MB)
- Integration tests require running TimescaleDB with pgvector (existing CI infrastructure)
- Model download tests should be marked `#[ignore]` (require network access)
