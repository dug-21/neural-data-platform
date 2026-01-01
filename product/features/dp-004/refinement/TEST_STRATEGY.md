# DP-004: Test Strategy - London School TDD

## Overview

This document outlines the London School (mockist) TDD approach for implementing DP-004: Bronze Layer Raw JSON Schema. The London School emphasizes:

1. **Outside-In Development**: Start from acceptance tests, work inward
2. **Mock Collaborators**: Isolate units by mocking dependencies
3. **Behavior Verification**: Test interactions, not internal state
4. **Contract Definition**: Establish interfaces through mock expectations

---

## Raw JSON Storage Approach (ADR-001)

This test strategy reflects the **raw JSON storage** decision from ADR-001:

| Aspect | Approach |
|--------|----------|
| **raw_payload** | Exact JSON from source, unmodified |
| **source_id** | Dedicated column for source identification |
| **ndp_id** | Platform-assigned stable identifier |
| **context** | Config-derived metadata snapshot |
| **Schema** | 5 columns: timestamp, source_id, ndp_id, context, raw_payload |

**Key Principle**: Bronze layer stores raw data; parsing moves to Silver ETL.

---

## Test Pyramid for DP-004

```
                    /\
                   /  \
                  / E2E \           <- 4-6 tests (write/read roundtrip, schema evolution)
                 /------\
                /        \
               / Integra- \         <- 12-15 tests (Parquet, source adapters, pipeline)
              /   tion     \
             /--------------\
            /                \
           /    Unit Tests    \     <- 15-20 tests (RawDataPoint, serialization, sources)
          /--------------------\
```

### Layer Distribution

| Layer | Count | Focus | Execution Time |
|-------|-------|-------|----------------|
| Unit | 15-20 | RawDataPoint construction, JSON serialization, metadata | < 1 second |
| Integration | 12-15 | Parquet write/read, source → pipeline flow | 5-10 seconds |
| E2E/Acceptance | 4-6 | Full pipeline, schema compatibility, DuckDB queries | 30-60 seconds |

---

## London School Principles Applied

### 1. Outside-In Development Flow

```
[Acceptance Test: RawDataPoint stored and queryable]
            |
            v
[Integration: ParquetStore writes 5-column schema]
            |
            v
[Integration: Source returns RawDataPoint]
            |
            v
[Unit: RawDataPoint construction and serialization]
            |
            v
[Unit: Source metadata extraction]
```

We write the outermost test first (raw payload stored and queryable), then drill down to discover what collaborators we need.

### 2. Mock Boundaries

The London School identifies collaboration points to mock:

```
+-------------------+       +-------------------+       +-------------------+
|   Source          | <---> |   MockHttpClient  |       |  Real: Internal   |
| (fetch raw data)  |       | (test double)     |       |  logic            |
+-------------------+       +-------------------+       +-------------------+
         |
         v
+-------------------+       +-------------------+
|   Pipeline        | <---> | MockChannel       |
| (route data)      |       | (capture sends)   |
+-------------------+       +-------------------+
         |
         v
+-------------------+       +-------------------+
|   ParquetStore    | <---> | TempDir           |
| (write Bronze)    |       | (isolated fs)     |
+-------------------+       +-------------------+
```

### 3. Behavior Verification Focus

Instead of testing internal state:
```rust
// BAD: Testing internal state
assert_eq!(raw_point.raw_payload["pm02"], 12.5);
```

We test interactions:
```rust
// GOOD: Testing behavior/collaboration
verify!(spy_store.write_was_called_with(
    has_source_id("air-quality-Mqtt"),
    has_raw_payload(json!({"pm02": 12.5}))
));
```

### 4. Test Double Types

| Type | Purpose | Used For |
|------|---------|----------|
| **Mock** | Verify interactions | ParquetWriter, Channel sender |
| **Stub** | Provide canned answers | HTTP responses, MQTT messages |
| **Spy** | Record calls for later assertion | Pipeline routing verification |
| **Fake** | Simplified implementation | In-memory storage, temp filesystem |

---

## Component Test Boundaries

### RawDataPoint (Unit Tests)

**System Under Test (SUT)**: `RawDataPoint` struct
**Collaborators**: None (pure data structure)
**Mock Boundary**: N/A

```rust
// Test: Construction with all fields
#[test]
fn raw_data_point_construction() {
    let point = RawDataPoint {
        timestamp: Utc::now(),
        source_id: "air-quality-Mqtt".to_string(),
        ndp_id: Some("airgradient-001".to_string()),
        context: Some(json!({"room": "office"})),
        raw_payload: json!({"pm02": 12.5, "rco2": 450}),
    };

    assert_eq!(point.source_id, "air-quality-Mqtt");
    assert!(point.ndp_id.is_some());
    assert_eq!(point.raw_payload["pm02"], 12.5);
}
```

### Source Adapters (Unit Tests)

**SUT**: HttpPollingSource, MqttSource
**Collaborators**: HTTP client, MQTT client
**Mock Boundary**: External clients

**Key Behaviors to Test**:
1. Source produces `RawDataPoint` (not parsed metrics)
2. `source_id` matches configuration
3. `raw_payload` contains exact source response

```rust
// Test: Source returns RawDataPoint with unmodified payload
#[tokio::test]
async fn source_returns_raw_data_point() {
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/current"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "pm25": 12.5,
            "temp": 22.3,
            "model": "ONE-V9"
        })))
        .mount(&mock_server)
        .await;

    let source = create_test_source(&mock_server.uri());
    let result = source.fetch_raw().await.unwrap();

    assert_eq!(result.source_id, "test-source-Http");
    assert_eq!(result.raw_payload["pm25"], 12.5);
    assert_eq!(result.raw_payload["model"], "ONE-V9");  // Non-numeric preserved
}
```

### ParquetStore (Integration Tests)

**SUT**: `ParquetStore` (Bronze writer)
**Collaborators**: File system
**Mock Boundary**: `tempfile::TempDir`

**Schema to Test**:
- `timestamp`: DateTime
- `source_id`: String
- `ndp_id`: String (nullable)
- `context`: String/JSON (nullable)
- `raw_payload`: String/JSON

```rust
// Test: Parquet writes 5-column schema
#[tokio::test]
async fn parquet_writes_raw_data_point_schema() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let point = RawDataPoint {
        timestamp: Utc::now(),
        source_id: "test-source".to_string(),
        ndp_id: Some("test-001".to_string()),
        context: Some(json!({"room": "office"})),
        raw_payload: json!({"pm25": 12.5, "state": "active"}),
    };

    store.write_raw(point).await.unwrap();

    // Read back and verify schema
    let df = read_parquet(temp_dir.path().join("*.parquet")).unwrap();
    assert!(df.column_names().contains(&"timestamp"));
    assert!(df.column_names().contains(&"source_id"));
    assert!(df.column_names().contains(&"ndp_id"));
    assert!(df.column_names().contains(&"context"));
    assert!(df.column_names().contains(&"raw_payload"));
}
```

### Pipeline Integration (Integration Tests)

**SUT**: Ingestion pipeline
**Collaborators**: Sources, Channels, Storage
**Mock Boundary**: Source responses stubbed

```rust
// Test: Pipeline routes RawDataPoint to storage
#[tokio::test]
async fn pipeline_routes_raw_data_to_storage() {
    let spy_store = SpyParquetStore::new();
    let pipeline = create_pipeline_with_store(spy_store.clone());

    let raw_point = create_test_raw_point();
    pipeline.ingest(raw_point.clone()).await.unwrap();

    let written = spy_store.get_written_points();
    assert_eq!(written.len(), 1);
    assert_eq!(written[0].source_id, raw_point.source_id);
    assert_eq!(written[0].raw_payload, raw_point.raw_payload);
}
```

---

## Acceptance Test Structure

### AT-001: RawDataPoint Stored and Queryable

This is our "walking skeleton" - the outermost test that drives all implementation.

**Key Verification Points**:
1. Source emits `RawDataPoint`
2. Pipeline routes to Bronze storage
3. Parquet file has correct schema
4. DuckDB can query raw_payload fields

```rust
#[tokio::test]
async fn acceptance_raw_data_point_stored_and_queryable() {
    // GIVEN: A configured source
    let env = TestEnvironment::new().await;
    let source_config = SourceConfig {
        source_type: SourceType::Http,
        source_id: "test-source-Http".into(),
        ndp_id: Some("test-device-001".into()),
        context: Some(json!({"room": "lab", "floor": 2})),
        ..default()
    };

    // AND: Source returns raw data
    env.stub_source_response(json!({
        "pm25": 15.3,
        "co2": 580,
        "status": "healthy",  // Non-numeric preserved
        "firmware": "v2.1.3"
    }));

    // WHEN: Pipeline runs ingestion
    env.run_ingestion(&source_config).await.unwrap();

    // THEN: Parquet file contains raw data
    let results = env.query_parquet(
        "SELECT source_id, raw_payload, context FROM bronze WHERE source_id = 'test-source-Http'"
    ).await.unwrap();

    assert!(!results.is_empty());
    let row = &results[0];

    // raw_payload exact match
    let payload: serde_json::Value = serde_json::from_str(&row.raw_payload).unwrap();
    assert_eq!(payload["pm25"], 15.3);
    assert_eq!(payload["status"], "healthy");
    assert_eq!(payload["firmware"], "v2.1.3");

    // context preserved
    let ctx: serde_json::Value = serde_json::from_str(&row.context).unwrap();
    assert_eq!(ctx["room"], "lab");
}
```

### AT-002: Multiple Source Types Produce RawDataPoint

```rust
#[tokio::test]
async fn acceptance_multiple_source_types() {
    let env = TestEnvironment::new().await;

    // HTTP source
    env.run_http_source(json!({"temp": 22.5})).await;

    // MQTT source
    env.run_mqtt_source(json!({"humidity": 65})).await;

    // Both produce RawDataPoint with correct source_id
    let results = env.query_parquet("SELECT source_id, raw_payload FROM bronze").await.unwrap();
    assert_eq!(results.len(), 2);

    let source_ids: Vec<_> = results.iter().map(|r| &r.source_id).collect();
    assert!(source_ids.contains(&&"http-source-Http".to_string()));
    assert!(source_ids.contains(&&"mqtt-source-Mqtt".to_string()));
}
```

> **Note**: AT-003 (Schema Backward Compatibility) removed - platform is <1 week old.
> Clean cutover approach: no need to read old schema files.

### AT-004: DuckDB JSON Extraction Works

```rust
#[tokio::test]
async fn acceptance_duckdb_json_extraction() {
    let env = TestEnvironment::new().await;

    // Write data with nested JSON
    env.write_raw_point(RawDataPoint {
        timestamp: Utc::now(),
        source_id: "nested-source".into(),
        raw_payload: json!({
            "sensors": {
                "pm25": 12.5,
                "co2": 450
            },
            "meta": {
                "version": "1.2.3"
            }
        }),
        ..default()
    }).await;

    // DuckDB can extract nested fields
    let results = env.query_parquet(r#"
        SELECT
            raw_payload->>'$.sensors.pm25' as pm25,
            raw_payload->>'$.meta.version' as version
        FROM bronze
    "#).await.unwrap();

    assert_eq!(results[0]["pm25"], "12.5");
    assert_eq!(results[0]["version"], "1.2.3");
}
```

---

## Test Execution Strategy

### Phase 1: Red (Failing Tests)

1. Write acceptance test AT-001 (will fail - no RawDataPoint)
2. Write integration tests for Parquet schema (will fail)
3. Write unit tests for RawDataPoint (will fail)

### Phase 2: Green (Make Tests Pass)

1. Add `RawDataPoint` struct to `core/src/traits.rs`
2. Update `ParquetStore` schema to 5 columns
3. Update sources to return `RawDataPoint`
4. Simplify parsers to metadata extraction only
5. Update pipeline to handle `RawDataPoint`

### Phase 3: Refactor

1. Extract common metadata injection
2. Add builder pattern for RawDataPoint
3. Add validation for source_id format

---

## Testing Phases Aligned with Implementation

| Phase | Component | Test Focus |
|-------|-----------|------------|
| 1 | RawDataPoint struct | Unit: construction, serialization |
| 2 | ParquetStore schema | Integration: write/read 5 columns |
| 3 | Source adapters | Integration: fetch → RawDataPoint |
| 4 | Parser simplification | Unit: metadata extraction only |
| 5 | Pipeline integration | E2E: full flow |
| 6 | Backward compatibility | E2E: old + new schema |

---

## Continuous Verification

```bash
# Run unit tests (fast feedback)
cargo test --lib raw_data_point source_adapter

# Run integration tests (medium feedback)
cargo test --test integration_* parquet

# Run acceptance tests (slow, comprehensive)
cargo test --test acceptance_*

# Full suite with coverage
cargo tarpaulin --out Html --all-features
```

---

## Simplicity Benefits

The raw JSON storage approach significantly reduces test complexity:

| Aspect | Current (Parsed) | New (Raw JSON) |
|--------|-----------------|----------------|
| Parser tests | Complex field extraction | Minimal metadata only |
| Type handling | Float/Int/String variants | JSON blob (any type) |
| Schema columns | 7+ columns | 5 columns |
| Source tests | Verify parsed values | Verify payload passthrough |
| ETL burden | Ingestion time | Silver layer (deferred) |

---

## Next Steps

1. See `TEST_CASES.md` for detailed test cases
2. See `MOCK_DEFINITIONS.md` for mock object specifications
3. See `TDD_IMPLEMENTATION_ORDER.md` for Red-Green-Refactor sequence
4. See `CODE_CHANGES.md` for file-by-file implementation details
