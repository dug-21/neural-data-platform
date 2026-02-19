# fe-005 Test Plan: deploy

## DDL Tests

```rust
// T-048: gold.text_embeddings table exists after init-script
#[tokio::test]
#[ignore] // Requires running TimescaleDB
async fn test_text_embeddings_table_exists() {
    let client = get_test_client().await;
    let row = client.query_one(
        "SELECT EXISTS (
            SELECT FROM pg_tables
            WHERE schemaname = 'gold' AND tablename = 'text_embeddings'
        )",
        &[],
    ).await.unwrap();
    let exists: bool = row.get(0);
    assert!(exists, "gold.text_embeddings table should exist");
}

// T-049: text_embeddings is a hypertable
#[tokio::test]
#[ignore]
async fn test_text_embeddings_is_hypertable() {
    let client = get_test_client().await;
    let row = client.query_one(
        "SELECT EXISTS (
            SELECT FROM timescaledb_information.hypertables
            WHERE hypertable_schema = 'gold'
            AND hypertable_name = 'text_embeddings'
        )",
        &[],
    ).await.unwrap();
    let exists: bool = row.get(0);
    assert!(exists, "gold.text_embeddings should be a hypertable");
}

// T-050: HNSW index exists
#[tokio::test]
#[ignore]
async fn test_hnsw_index_exists() {
    let client = get_test_client().await;
    let row = client.query_one(
        "SELECT EXISTS (
            SELECT FROM pg_indexes
            WHERE schemaname = 'gold'
            AND tablename = 'text_embeddings'
            AND indexname = 'idx_text_embeddings_hnsw'
        )",
        &[],
    ).await.unwrap();
    let exists: bool = row.get(0);
    assert!(exists, "HNSW index should exist");
}

// T-051: Insert and retrieve embedding
#[tokio::test]
#[ignore]
async fn test_insert_and_retrieve() {
    let client = get_test_client().await;
    let embedding_str = format!("[{}]", vec!["0.1"; 384].join(","));
    client.execute(
        "INSERT INTO gold.text_embeddings \
         (bucket, domain_id, source_stream, source_column, source_text, embedding, model_id) \
         VALUES ($1, $2, $3, $4, $5, $6::vector, $7)",
        &[
            &chrono::Utc::now(),
            &"test-domain",
            &"test-stream",
            &"test-column",
            &"Test text for embedding",
            &embedding_str,
            &"test-model",
        ],
    ).await.unwrap();

    let row = client.query_one(
        "SELECT source_text, model_id FROM gold.text_embeddings \
         WHERE domain_id = 'test-domain' ORDER BY created_at DESC LIMIT 1",
        &[],
    ).await.unwrap();
    let text: String = row.get("source_text");
    assert_eq!(text, "Test text for embedding");
}

// T-052: retention_tier is nullable
#[tokio::test]
#[ignore]
async fn test_retention_tier_nullable() {
    let client = get_test_client().await;
    let row = client.query_one(
        "SELECT is_nullable FROM information_schema.columns \
         WHERE table_schema = 'gold' AND table_name = 'text_embeddings' \
         AND column_name = 'retention_tier'",
        &[],
    ).await.unwrap();
    let nullable: String = row.get(0);
    assert_eq!(nullable, "YES", "retention_tier should be nullable");
}
```

## Domain Schema Tests

```rust
// T-053: Domain schema accepts text_embedding block
#[test]
fn test_schema_with_text_embedding() {
    let schema = load_domain_schema();
    let domain = serde_json::json!({
        "id": "test-domain",
        "streams": [{"stream_id": "test", "role": "primary"}],
        "alignment": {"view_name": "test_aligned", "granularity": "1 hour"},
        "text_embedding": {
            "model": "BAAI/bge-small-en-v1.5",
            "dimensions": 384
        }
    });
    assert!(validate_json(&schema, &domain).is_ok());
}

// T-054: Domain schema valid without text_embedding (backward compat)
#[test]
fn test_schema_without_text_embedding() {
    let schema = load_domain_schema();
    let domain = serde_json::json!({
        "id": "test-domain",
        "streams": [{"stream_id": "test", "role": "primary"}],
        "alignment": {"view_name": "test_aligned", "granularity": "1 hour"}
    });
    assert!(validate_json(&schema, &domain).is_ok());
}

// T-055: text_embedding requires model and dimensions
#[test]
fn test_schema_text_embedding_requires_fields() {
    let schema = load_domain_schema();
    let domain = serde_json::json!({
        "id": "test-domain",
        "streams": [{"stream_id": "test", "role": "primary"}],
        "alignment": {"view_name": "test_aligned", "granularity": "1 hour"},
        "text_embedding": {
            "quantization": "int8"
        }
    });
    assert!(validate_json(&schema, &domain).is_err());
}
```

## Container Tests

```bash
# T-056: Dockerfile builds successfully
docker build -f docker/embedder/Dockerfile -t ndp-embedder-test .
# Assert: exit code 0

# T-057: Container starts and shows help
docker run --rm ndp-embedder-test --help
# Assert: shows "ndp-embedder" usage

# T-058: Container exits gracefully without config
docker run --rm \
  -e DATABASE_URL=postgresql://fake:5432/ndp \
  -e EMBEDDER_DOMAIN=nonexistent \
  ndp-embedder-test daemon
# Assert: logs "No text_embedding config" and exits 0
```

## Test Summary

| ID | Test | Type | Component |
|----|------|------|-----------|
| T-048 | Table exists | DDL | init-script |
| T-049 | Is hypertable | DDL | init-script |
| T-050 | HNSW index exists | DDL | init-script |
| T-051 | Insert and retrieve | DDL | init-script |
| T-052 | retention_tier nullable | DDL | init-script |
| T-053 | Schema with text_embedding | Schema | domain.schema.json |
| T-054 | Schema without text_embedding | Schema | domain.schema.json |
| T-055 | Schema requires fields | Schema | domain.schema.json |
| T-056 | Dockerfile builds | Container | Dockerfile |
| T-057 | Container help | Container | Dockerfile |
| T-058 | Container graceful exit | Container | Dockerfile |
