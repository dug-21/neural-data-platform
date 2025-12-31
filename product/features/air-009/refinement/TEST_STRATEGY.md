# AIR-009: Test Strategy - London School TDD

## Overview

This document outlines the London School (mockist) TDD approach for implementing AIR-009: Source Identity and Context Configuration. The London School emphasizes:

1. **Outside-In Development**: Start from acceptance tests, work inward
2. **Mock Collaborators**: Isolate units by mocking dependencies
3. **Behavior Verification**: Test interactions, not internal state
4. **Contract Definition**: Establish interfaces through mock expectations

---

## Simple Blob Approach (ADR-002-AMENDMENT-002)

This test strategy reflects the **simple blob storage** decision:

| Aspect | Approach |
|--------|----------|
| **ndp_id** | Dedicated column for fast indexed queries |
| **context** | Single JSON blob (no flattening, no promoted fields) |
| **Query Strategy** | ndp_id for identity, JSONB operators for context fields |

**Schema:**
- Bronze (Parquet): `ndp_id` STRING, `context` STRING (JSON)
- Silver (TimescaleDB): `ndp_id` TEXT, `context` JSONB

**All context queries use JSONB operators:**
```sql
WHERE context->>'device_type' = 'airgradient'
WHERE context->'location'->>'type' = 'indoor'
```

---

## Test Pyramid for AIR-009

```
                    /\
                   /  \
                  / E2E \           <- 6 tests (ndp_id queries, JSONB queries)
                 /------\
                /        \
               / Integra- \         <- 10-15 tests (parser, Bronze, Silver)
              /   tion     \
             /--------------\
            /                \
           /    Unit Tests    \     <- 10-15 tests (serialization, SourceConfig)
          /--------------------\
```

### Layer Distribution

| Layer | Count | Focus | Execution Time |
|-------|-------|-------|----------------|
| Unit | 10-15 | Context JSON serialization, SourceConfig parsing | < 1 second |
| Integration | 10-15 | Parser context attachment, Bronze/Silver columns | 5-10 seconds |
| E2E/Acceptance | 6 | ndp_id queries, JSONB queries, index performance | 30-60 seconds |

---

## London School Principles Applied

### 1. Outside-In Development Flow

```
[Acceptance Test: Query by ndp_id]
            |
            v
[Integration: Bronze Writer includes ndp_id + context]
            |
            v
[Integration: Parser attaches context blob]
            |
            v
[Unit: Context JSON serialization]
            |
            v
[Unit: SourceConfig struct parsing]
```

We write the outermost test first (query by ndp_id returns expected record), then drill down to discover what collaborators we need.

### 2. Mock Boundaries

The London School identifies collaboration points to mock:

```
+-------------------+       +-------------------+       +-------------------+
|   ConfigClient    | <---> |   MockEtcdClient  |       |  Real: Internal   |
| (etcd operations) |       | (test double)     |       |  logic            |
+-------------------+       +-------------------+       +-------------------+
         |
         v
+-------------------+       +-------------------+
|   SourceConfig    | <---> | MockSourceConfig  |
| (config parsing)  |       | (stub data)       |
+-------------------+       +-------------------+
         |
         v
+-------------------+       +-------------------+
|   Parser          | <---> | MockContextProvider|
| (attach context)  |       | (provides context)|
+-------------------+       +-------------------+
         |
         v
+-------------------+       +-------------------+
|   ParquetWriter   | <---> | MockParquetWriter |
| (Bronze layer)    |       | (captures writes) |
+-------------------+       +-------------------+
         |
         v
+-------------------+       +-------------------+
|   TimescaleDB     | <---> | MockTimescaleClient|
| (Silver layer)    |       | (verify SQL)      |
+-------------------+       +-------------------+
```

### 3. Behavior Verification Focus

Instead of testing internal state:
```rust
// BAD: Testing internal state
assert_eq!(config.context.location.path, "home/office");
```

We test interactions:
```rust
// GOOD: Testing behavior/collaboration
verify!(mock_writer.write_was_called_with(
    contains_field("context", json_containing("location"))
));
```

### 4. Test Double Types

| Type | Purpose | Used For |
|------|---------|----------|
| **Mock** | Verify interactions | ParquetWriter, TimescaleClient |
| **Stub** | Provide canned answers | ConfigClient returning test configs |
| **Spy** | Record calls for later assertion | Context serialization call tracking |
| **Fake** | Simplified implementation | In-memory etcd |

---

## Component Test Boundaries

### SourceConfig (Unit Tests)

**System Under Test (SUT)**: `SourceConfig` struct
**Collaborators**: None (pure data structure)
**Mock Boundary**: N/A

```rust
// Test: Deserialization includes ndp_id and context
#[test]
fn source_config_deserializes_ndp_id_and_context() {
    let yaml = r#"
        type: mqtt
        ndp_id: airgradient-office-001
        context:
          location:
            coordinates: [29.958, -81.308]
            type: indoor
            path: home/office
    "#;

    let config: SourceConfig = serde_yaml::from_str(yaml).unwrap();

    assert_eq!(config.ndp_id, Some("airgradient-office-001".into()));
    assert!(config.context.is_some());
}
```

### Context Serialization (Unit Tests)

**SUT**: JSON serialization
**Collaborators**: None (pure function)
**Mock Boundary**: N/A

**Key Point**: With the simple blob approach, there's no processing - just serialize the context as-is.

```rust
// Test: Context serializes to JSON string preserving all structure
#[test]
fn context_serializes_to_json_blob() {
    let context = json!({
        "location": {
            "coordinates": [29.958, -81.308],
            "type": "indoor",
            "path": "home/office"
        },
        "device_type": "airgradient",
        "model": "ONE-V9"
    });

    let json_str = serde_json::to_string(&context).unwrap();
    let restored: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    // Everything preserved exactly
    assert_eq!(restored["location"]["type"], "indoor");
    assert_eq!(restored["device_type"], "airgradient");
    assert_eq!(restored["model"], "ONE-V9");
}
```

### ConfigSyncService (Integration Tests)

**SUT**: `ConfigSyncService`
**Collaborators**: `ConfigClient` (etcd)
**Mock Boundary**: `etcd_client::Client`

```rust
// Test: ndp_id and context sync to correct etcd keys
#[tokio::test]
async fn config_sync_writes_ndp_id_to_etcd() {
    let mock_etcd = MockEtcdClient::new();
    mock_etcd.expect_put()
        .with(eq("/streams/air-quality/sources/0/ndp_id"), any())
        .times(1)
        .returning(|_, _| Ok(()));

    let sync_service = ConfigSyncService::new(mock_etcd);
    sync_service.sync(test_stream_config()).await.unwrap();

    mock_etcd.verify();
}
```

### Parser Context Attachment (Integration Tests)

**SUT**: Parser implementations
**Collaborators**: `SourceConfig` (provides context)
**Mock Boundary**: `SourceConfig` stubbed with test context

**Key Behaviors to Test**:
1. ndp_id attached to record
2. Full context serialized as JSON string blob

```rust
// Test: Parsed records include ndp_id and context blob
#[test]
fn parser_attaches_ndp_id_and_context_blob() {
    let stub_config = SourceConfigBuilder::new()
        .ndp_id("test-sensor-001")
        .context(json!({
            "location": {"type": "indoor", "path": "home/office"},
            "device_type": "airgradient"
        }))
        .build();

    let parser = FlatJsonParser::with_config(stub_config);
    let record = parser.parse(raw_mqtt_payload).unwrap();

    // ndp_id attached
    assert_eq!(record.ndp_id, Some("test-sensor-001".into()));

    // Context as JSON blob
    let ctx: serde_json::Value = serde_json::from_str(&record.context.unwrap()).unwrap();
    assert_eq!(ctx["location"]["type"], "indoor");
    assert_eq!(ctx["device_type"], "airgradient");
}
```

### Bronze Layer Writer (Integration Tests)

**SUT**: `ParquetStore`
**Collaborators**: File system
**Mock Boundary**: `tempfile` for isolated file system

**Schema to Test** (Simple Blob):
- `ndp_id`: STRING
- `context`: STRING (JSON blob)

```rust
// Test: Written records include ndp_id and context columns
#[tokio::test]
async fn parquet_writer_includes_context_blob() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let point = TimeSeriesPoint::builder()
        .ndp_id("test-sensor-001")
        .context(r#"{"location":{"type":"indoor"},"device":"airgradient"}"#)
        .build();

    store.write(point).await.unwrap();

    // Read back and verify both columns exist
    let df = read_parquet(temp_dir.path().join("...")).unwrap();
    assert!(df.column_names().contains(&"ndp_id"));
    assert!(df.column_names().contains(&"context"));
}
```

### Silver Layer Schema (Integration Tests)

**SUT**: TimescaleDB migration/ETL
**Collaborators**: PostgreSQL/TimescaleDB
**Mock Boundary**: `testcontainers` or embedded PG

**Schema to Test** (Simple Blob):
- `ndp_id`: TEXT with B-tree index
- `context`: JSONB with GIN index

```rust
// Test: Silver layer has ndp_id + JSONB context + indexes
#[tokio::test]
async fn silver_layer_has_simple_blob_schema() {
    let mock_client = MockTimescaleClient::new();

    // Expect ndp_id column
    mock_client.expect_execute()
        .withf(|sql| sql.contains("ndp_id") && sql.contains("TEXT"))
        .times(1)
        .returning(|_| Ok(()));

    // Expect JSONB column
    mock_client.expect_execute()
        .withf(|sql| sql.contains("context") && sql.contains("JSONB"))
        .times(1)
        .returning(|_| Ok(()));

    // Expect GIN index on JSONB
    mock_client.expect_execute()
        .withf(|sql| sql.contains("CREATE INDEX") && sql.contains("GIN"))
        .times(1)
        .returning(|_| Ok(()));

    run_migration(mock_client).await.unwrap();
    mock_client.verify();
}
```

---

## Acceptance Test Structure

### AT-001: Query Records by ndp_id with Context Blob

This is our "walking skeleton" - the outermost test that drives all implementation.

**Key Verification Points**:
1. Query by `ndp_id` returns records
2. Full context preserved in `context` JSONB
3. All fields accessible via JSONB operators

```rust
#[tokio::test]
async fn acceptance_query_by_ndp_id_returns_context_blob() {
    // GIVEN: A stream configured with ndp_id and full context
    let config = StreamConfig {
        stream_id: "air-quality".into(),
        sources: vec![SourceConfig {
            ndp_id: Some("airgradient-office-001".into()),
            context: Some(json!({
                "location": {
                    "type": "indoor",
                    "path": "home/office",
                    "coordinates": [29.958, -81.308]
                },
                "device_type": "airgradient",
                "model": "ONE-V9"
            })),
            ..default()
        }],
        ..default()
    };

    // AND: Records have been ingested through the pipeline
    let pipeline = setup_test_pipeline(config).await;
    pipeline.ingest(test_mqtt_payload()).await.unwrap();

    // WHEN: We query by ndp_id
    let results = pipeline
        .query("SELECT * FROM sensor_readings WHERE ndp_id = 'airgradient-office-001'")
        .await
        .unwrap();

    // THEN: We get records with full context JSONB
    assert!(!results.is_empty());
    for record in results {
        assert_eq!(record.ndp_id, "airgradient-office-001");

        // All context in JSONB blob
        let ctx: serde_json::Value = record.context;
        assert_eq!(ctx["device_type"], "airgradient");
        assert_eq!(ctx["model"], "ONE-V9");
        assert_eq!(ctx["location"]["type"], "indoor");
    }
}
```

### AT-002: Query by Context Field (JSONB Operators)

```rust
#[tokio::test]
async fn acceptance_query_by_context_field() {
    // GIVEN: Multiple sources with different device types
    let config = create_multi_source_config();
    let pipeline = setup_test_pipeline(config).await;

    // AND: Records ingested from both sources
    pipeline.ingest_from("airgradient-001", 10).await;
    pipeline.ingest_from("purpleair-001", 10).await;

    // WHEN: We query by context field using JSONB operators
    let results = pipeline
        .query("SELECT * FROM sensor_readings WHERE context->>'device_type' = 'airgradient'")
        .await
        .unwrap();

    // THEN: Only matching records returned
    assert_eq!(results.len(), 10);
    for record in results {
        let ctx: serde_json::Value = record.context;
        assert_eq!(ctx["device_type"], "airgradient");
    }
}
```

### AT-003: Query Nested Context Field (JSONB Operators)

```rust
#[tokio::test]
async fn acceptance_query_nested_context_field() {
    // GIVEN: Records with location.type in context
    let pipeline = setup_test_pipeline(test_config()).await;
    pipeline.ingest(test_payload()).await;

    // WHEN: We query using JSONB nested access
    let results = pipeline
        .query("SELECT * FROM sensor_readings WHERE context->'location'->>'type' = 'indoor'")
        .await
        .unwrap();

    // THEN: Records filtered correctly
    assert!(!results.is_empty());
    for record in results {
        let ctx: serde_json::Value = record.context;
        assert_eq!(ctx["location"]["type"], "indoor");
    }
}
```

---

## Test Execution Strategy

### Phase 1: Red (Failing Tests)

1. Write acceptance test AT-001 (will fail - no ndp_id/context support)
2. Write integration tests for each component (will fail)
3. Write unit tests for serialization (will fail)

### Phase 2: Green (Make Tests Pass)

1. Add `ndp_id` and `context` to `SourceConfig`
2. Implement simple JSON serialization (trivial - just `serde_json::to_string`)
3. Update parsers to attach ndp_id and context blob
4. Update Bronze writer to include `ndp_id` + `context` columns
5. Create Silver layer schema with `ndp_id` TEXT + `context` JSONB + indexes

### Phase 3: Refactor

1. Add validation for ndp_id format
2. Consider caching serialized context if needed
3. Add helper methods for common JSONB queries

---

## Continuous Verification

```bash
# Run unit tests (fast feedback)
cargo test --lib source_config serialization

# Run integration tests (medium feedback)
cargo test --test integration_*

# Run acceptance tests (slow, comprehensive)
cargo test --test acceptance_*

# Full suite with coverage
cargo tarpaulin --out Html --all-features
```

---

## Simplicity Benefits

The simple blob approach significantly reduces test complexity:

| Aspect | Hybrid Approach (Old) | Simple Blob (New) |
|--------|----------------------|-------------------|
| Unit tests | 15-20 (promoted fields, ProcessedContext) | 10 (just serialization) |
| Integration tests | 20+ (promoted columns, context_raw) | 10-15 (just ndp_id + context) |
| Schema columns | 5+ (ndp_id, ctx_location_*, context_raw) | 2 (ndp_id, context) |
| Query patterns | 2 (column access, JSONB) | 1 (JSONB only) |
| Test complexity | High | Low |

---

## Next Steps

1. See `TEST_CASES.md` for detailed test cases
2. See `MOCK_DEFINITIONS.md` for mock object specifications
3. See `TDD_IMPLEMENTATION_ORDER.md` for Red-Green-Refactor sequence
