# AIR-009: TDD Implementation Order

## Overview

This document provides the exact sequence of test-first implementation steps for Source Identity and Context Configuration. Each step follows the Red-Green-Refactor cycle.

**Amendment**: Updated to reflect **ADR-002-AMENDMENT-002** simple blob context storage approach.

---

## Simple Blob Approach (ADR-002-AMENDMENT-002)

**Key Change**: Maximum simplicity - NO flattening, NO promoted fields.

| Field | Storage |
|-------|---------|
| `ndp_id` | Dedicated column for fast indexed queries |
| `context` | Single JSON blob (Bronze: STRING, Silver: JSONB) |

**Implementation is trivial:**
```rust
let context_json = serde_json::to_string(&config.context)?;
record.context = Some(context_json);
```

---

## London School Approach: Outside-In

```
Phase 1: Acceptance Test (RED)      <- Start here, drives discovery
    |                                  Tests ndp_id + JSONB context queries
    v
Phase 2: Integration Tests (RED)    <- Discover component interfaces
    |                                  Tests parser context blob, Bronze/Silver schema
    v
Phase 3: Unit Tests (RED)           <- Implement smallest units
    |                                  Tests context serialization
    v
Phase 4: Implementation (GREEN)     <- Make tests pass bottom-up
    |                                  TRIVIAL: Just serde_json::to_string()
    v
Phase 5: Refactor                   <- Clean up, optimize
```

---

## Implementation Cycles

### Cycle 1: SourceConfig ndp_id Field

**RED**: Write failing test for ndp_id in SourceConfig

```rust
// core/src/types/stream_config.rs

#[test]
fn test_source_config_has_ndp_id_field() {
    let config = SourceConfig {
        source_type: SourceType::Mqtt,
        enabled: true,
        ndp_id: Some("test-device-001".to_string()),  // NEW
        context: None,  // NEW
        params: HashMap::new(),
    };

    assert_eq!(config.ndp_id, Some("test-device-001".to_string()));
}
```

**GREEN**: Add ndp_id field to SourceConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceConfig {
    #[serde(rename = "type")]
    pub source_type: SourceType,

    #[serde(default = "default_enabled")]
    pub enabled: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ndp_id: Option<String>,  // NEW

    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,  // NEW

    #[serde(flatten)]
    pub params: HashMap<String, serde_json::Value>,
}
```

**REFACTOR**: Add validation helper

```rust
fn is_valid_ndp_id(id: &str) -> bool {
    let len = id.len();
    len >= 3 && len <= 64 &&
    id.chars().next().map_or(false, |c| c.is_ascii_lowercase()) &&
    id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
```

---

### Cycle 2: SourceConfig context Field

**RED**: Write failing test for context deserialization

```rust
#[test]
fn test_source_config_deserializes_context() {
    let yaml = r#"
        type: mqtt
        enabled: true
        ndp_id: test-device-001
        context:
          location:
            type: indoor
            path: home/office
          device_type: airgradient
    "#;

    let config: SourceConfig = serde_yaml::from_str(yaml).unwrap();

    assert!(config.context.is_some());
    let ctx = config.context.unwrap();
    assert_eq!(ctx["location"]["type"], "indoor");
    assert_eq!(ctx["device_type"], "airgradient");
}
```

**GREEN**: The `context: Option<serde_json::Value>` added in Cycle 1 already handles this!

**REFACTOR**: None needed - serde handles nested JSON/YAML naturally.

---

### Cycle 3: Context JSON Serialization (Simple Blob)

**RED**: Write test for context blob serialization

```rust
#[test]
fn test_context_serializes_to_json_blob() {
    let context = json!({
        "location": {
            "type": "indoor",
            "path": "home/office",
            "coordinates": [29.958, -81.308]
        },
        "device_type": "airgradient",
        "model": "ONE-V9"
    });

    let json_str = serde_json::to_string(&context).unwrap();
    let restored: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    // All structure preserved exactly
    assert_eq!(restored["location"]["type"], "indoor");
    assert_eq!(restored["device_type"], "airgradient");
    assert_eq!(restored["location"]["coordinates"][0], 29.958);
}
```

**GREEN**: This test passes immediately - serde_json handles everything!

**REFACTOR**: None needed - the simple blob approach means NO processing code.

---

### Cycle 4: ParseContext Struct

**RED**: Write test for ParseContext creation

```rust
// core/src/parsers/traits.rs

#[test]
fn test_parse_context_from_source_config() {
    let config = SourceConfig {
        source_type: SourceType::Mqtt,
        enabled: true,
        ndp_id: Some("test-001".into()),
        context: Some(json!({"location": {"type": "indoor"}})),
        params: HashMap::new(),
    };

    let parse_ctx = ParseContext::from_source_config(&config);

    assert_eq!(parse_ctx.ndp_id, Some("test-001".into()));
    assert!(parse_ctx.context.is_some());
    let ctx_str = parse_ctx.context.unwrap();
    assert!(ctx_str.contains("indoor"));
}
```

**GREEN**: Implement ParseContext

```rust
#[derive(Debug, Clone, Default)]
pub struct ParseContext {
    pub ndp_id: Option<String>,
    pub context: Option<String>,  // JSON blob
}

impl ParseContext {
    pub fn from_source_config(config: &SourceConfig) -> Self {
        Self {
            ndp_id: config.ndp_id.clone(),
            context: config.context.as_ref()
                .map(|c| serde_json::to_string(c).unwrap_or_default()),
        }
    }
}
```

**REFACTOR**: Add helper method for checking if context is present

---

### Cycle 5: Parser Injects ndp_id

**RED**: Write test for parser injecting ndp_id

```rust
// core/src/parsers/flat_json.rs

#[test]
fn test_parser_injects_ndp_id() {
    let config = SourceConfigBuilder::new()
        .ndp_id("test-sensor-001")
        .build();

    let parse_ctx = ParseContext::from_source_config(&config);
    let parser = FlatJsonParser::new();
    let payload = r#"{"pm25": 12.5, "temperature": 22.3}"#;

    let points = parser.parse_with_context(payload.as_bytes(), &parse_ctx).unwrap();

    assert_eq!(points[0].ndp_id, Some("test-sensor-001".to_string()));
}
```

**GREEN**: Add parse_with_context method

```rust
impl FlatJsonParser {
    pub fn parse_with_context(
        &self,
        payload: &[u8],
        parse_context: &ParseContext,
    ) -> Result<Vec<TimeSeriesPoint>, ParserError> {
        let mut points = self.parse(payload)?;

        for point in &mut points {
            if let Some(ref ndp_id) = parse_context.ndp_id {
                point.ndp_id = Some(ndp_id.clone());
            }
        }

        Ok(points)
    }
}
```

**REFACTOR**: Consider extracting context injection to a shared trait method

---

### Cycle 6: Parser Injects Context Blob

**RED**: Write test for parser injecting context blob

```rust
#[test]
fn test_parser_injects_context_blob() {
    let config = SourceConfigBuilder::new()
        .ndp_id("test-001")
        .context(json!({
            "location": {"type": "indoor"},
            "device_type": "airgradient"
        }))
        .build();

    let parse_ctx = ParseContext::from_source_config(&config);
    let parser = FlatJsonParser::new();
    let payload = r#"{"pm25": 12.5}"#;

    let points = parser.parse_with_context(payload.as_bytes(), &parse_ctx).unwrap();

    // Context as JSON blob
    let ctx_str = points[0].context.as_ref().unwrap();
    let ctx: serde_json::Value = serde_json::from_str(ctx_str).unwrap();
    assert_eq!(ctx["location"]["type"], "indoor");
    assert_eq!(ctx["device_type"], "airgradient");
}
```

**GREEN**: Extend parse_with_context

```rust
pub fn parse_with_context(
    &self,
    payload: &[u8],
    parse_context: &ParseContext,
) -> Result<Vec<TimeSeriesPoint>, ParserError> {
    let mut points = self.parse(payload)?;

    for point in &mut points {
        if let Some(ref ndp_id) = parse_context.ndp_id {
            point.ndp_id = Some(ndp_id.clone());
        }

        if let Some(ref context) = parse_context.context {
            point.context = Some(context.clone());  // Simple clone!
        }
    }

    Ok(points)
}
```

**REFACTOR**: None needed - implementation is minimal.

---

### Cycle 7: TimeSeriesPoint Fields

**RED**: Write test that TimeSeriesPoint has ndp_id and context

```rust
// core/src/types/mod.rs

#[test]
fn test_time_series_point_has_identity_fields() {
    let point = TimeSeriesPoint {
        timestamp: Utc::now(),
        location_id: "test".into(),
        fields: HashMap::new(),
        tags: HashMap::new(),
        ndp_id: Some("test-001".into()),
        context: Some(r#"{"device_type":"airgradient"}"#.into()),
    };

    assert_eq!(point.ndp_id, Some("test-001".into()));
    assert!(point.context.is_some());
}
```

**GREEN**: Add fields to TimeSeriesPoint

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub location_id: String,
    pub fields: HashMap<String, serde_json::Value>,
    pub tags: HashMap<String, String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ndp_id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,  // JSON blob
}
```

**REFACTOR**: Update Default impl if needed

---

### Cycle 8: Parquet Writer ndp_id Column

**RED**: Write test for ndp_id in Parquet

```rust
// core/src/storage/parquet.rs

#[tokio::test]
async fn test_parquet_writer_includes_ndp_id() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let mut point = create_test_point();
    point.ndp_id = Some("test-001".to_string());

    store.write(point.clone()).await.unwrap();

    // Read back and verify column exists
    let path = store.partition_path("test-stream", point.timestamp);
    let df = ParquetReader::new(File::open(path).unwrap())
        .finish()
        .unwrap();

    assert!(df.column_names().contains(&"ndp_id"));
}
```

**GREEN**: Add ndp_id to schema and writer

```rust
fn build_schema() -> Schema {
    Schema::new(vec![
        // ... existing fields
        Field::new("ndp_id", DataType::Utf8, true),
    ])
}
```

**REFACTOR**: Ensure nullable handling is consistent

---

### Cycle 9: Parquet Writer context Column

**RED**: Write test for context blob in Parquet

```rust
#[tokio::test]
async fn test_parquet_writer_includes_context_blob() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let context = json!({"location": {"type": "indoor"}, "device_type": "airgradient"});

    let mut point = create_test_point();
    point.context = Some(serde_json::to_string(&context).unwrap());

    store.write(point.clone()).await.unwrap();

    // Read back and verify
    let df = read_parquet(&store.partition_path("test-stream", point.timestamp)).unwrap();

    assert!(df.column_names().contains(&"context"));

    // Verify it's valid JSON in the column
    let context_col = df.column("context").unwrap();
    let first_val = context_col.get(0).unwrap().to_string();
    let parsed: serde_json::Value = serde_json::from_str(&first_val).unwrap();
    assert_eq!(parsed["device_type"], "airgradient");
}
```

**GREEN**: Add context column to schema

```rust
fn build_schema() -> Schema {
    Schema::new(vec![
        // ... existing fields
        Field::new("ndp_id", DataType::Utf8, true),
        Field::new("context", DataType::Utf8, true),  // JSON blob as string
    ])
}
```

**REFACTOR**: None needed

---

### Cycle 10: ConfigSync YAML Parsing

**RED**: Write test for ConfigSync parsing ndp_id and context

```rust
// apps/air-quality-app/src/config_sync/mod.rs

#[test]
fn test_config_sync_parses_ndp_id_and_context() {
    let yaml = r#"
        stream_id: air-quality
        sources:
          - type: mqtt
            enabled: true
            ndp_id: airgradient-office-001
            context:
              location:
                type: indoor
                path: home/office
              device_type: airgradient
            broker_url: mqtt://mosquitto
            topic: airgradient/readings
    "#;

    let config = parse_stream_config(yaml).unwrap();

    assert_eq!(
        config.sources[0].ndp_id,
        Some("airgradient-office-001".into())
    );

    let ctx = config.sources[0].context.as_ref().unwrap();
    assert_eq!(ctx["location"]["type"], "indoor");
}
```

**GREEN**: Update SourceYaml struct in config_sync

```rust
#[derive(Debug, Clone, Deserialize)]
struct SourceYaml {
    #[serde(rename = "type")]
    source_type: String,

    #[serde(default)]
    enabled: bool,

    ndp_id: Option<String>,

    context: Option<serde_yaml::Value>,

    #[serde(flatten)]
    params: HashMap<String, serde_yaml::Value>,
}
```

**REFACTOR**: Add YAML to JSON conversion for context

---

### Cycle 11: Silver Layer Migration (Mock)

**RED**: Write test that migration creates correct schema

```rust
#[tokio::test]
async fn test_migration_creates_simple_blob_schema() {
    let mock = MockTimescaleClient::new();

    // Expect ndp_id TEXT column
    mock.expect_execute()
        .withf(|sql| sql.contains("ndp_id") && sql.contains("TEXT"))
        .times(1)
        .returning(|_| Ok(0));

    // Expect context JSONB column
    mock.expect_execute()
        .withf(|sql| sql.contains("context") && sql.contains("JSONB"))
        .times(1)
        .returning(|_| Ok(0));

    // Expect GIN index
    mock.expect_execute()
        .withf(|sql| sql.contains("GIN") && sql.contains("context"))
        .times(1)
        .returning(|_| Ok(0));

    run_air_009_migration(mock).await.unwrap();
    mock.checkpoint();
}
```

**GREEN**: Create migration SQL

```sql
ALTER TABLE sensor_readings ADD COLUMN IF NOT EXISTS ndp_id TEXT;
ALTER TABLE sensor_readings ADD COLUMN IF NOT EXISTS context JSONB;
CREATE INDEX IF NOT EXISTS idx_readings_ndp_id ON sensor_readings(ndp_id);
CREATE INDEX IF NOT EXISTS idx_readings_context ON sensor_readings USING GIN (context);
```

**REFACTOR**: None needed

---

### Cycle 12: End-to-End Query Test

**RED**: Write acceptance test for query by ndp_id with full context

```rust
#[tokio::test]
async fn test_e2e_query_by_ndp_id_returns_context_blob() {
    let env = TestEnvironment::new().await;

    // Setup config
    let config = create_test_config_with_full_context();
    env.sync_config(config).await;

    // Ingest
    env.ingest_mqtt_payload(test_payload()).await;

    // Query by ndp_id
    let results = env.query_sql(
        "SELECT * FROM sensor_readings WHERE ndp_id = 'airgradient-office-001'"
    ).await.unwrap();

    // Verify
    assert!(!results.is_empty());
    let ctx: serde_json::Value = results[0].context.clone();
    assert_eq!(ctx["device_type"], "airgradient");
    assert_eq!(ctx["location"]["type"], "indoor");
}

#[tokio::test]
async fn test_e2e_query_by_context_field() {
    let env = TestEnvironment::new().await;
    env.sync_config(create_test_config_with_full_context()).await;
    env.ingest_mqtt_payload(test_payload()).await;

    // Query by context field using JSONB operator
    let results = env.query_sql(
        "SELECT * FROM sensor_readings WHERE context->>'device_type' = 'airgradient'"
    ).await.unwrap();

    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_e2e_query_by_nested_context_field() {
    let env = TestEnvironment::new().await;
    env.sync_config(create_test_config_with_full_context()).await;
    env.ingest_mqtt_payload(test_payload()).await;

    // Query by nested context field using JSONB operator
    let results = env.query_sql(
        "SELECT * FROM sensor_readings WHERE context->'location'->>'type' = 'indoor'"
    ).await.unwrap();

    assert!(!results.is_empty());
}
```

**GREEN**: All previous cycles make this pass!

**REFACTOR**: None needed - integration of all components

---

## Test Execution Summary

| Cycle | Component | Test Type | Complexity |
|-------|-----------|-----------|------------|
| 1 | SourceConfig.ndp_id | Unit | Trivial |
| 2 | SourceConfig.context | Unit | Trivial |
| 3 | Context Serialization | Unit | Trivial (serde) |
| 4 | ParseContext | Unit | Simple |
| 5 | Parser ndp_id | Unit | Simple |
| 6 | Parser context blob | Unit | Simple |
| 7 | TimeSeriesPoint | Unit | Trivial |
| 8 | Parquet ndp_id | Integration | Simple |
| 9 | Parquet context | Integration | Simple |
| 10 | ConfigSync | Unit | Simple |
| 11 | Silver Schema | Integration (mock) | Simple |
| 12 | E2E Query | Acceptance | Medium |

---

## Simplicity Wins

The simple blob approach eliminates several cycles that would have been needed with the hybrid approach:

**Removed (not needed with simple blob):**
- ProcessedContext struct and tests
- Promoted field extraction tests
- `ctx_location_type`, `ctx_location_path`, `ctx_location_coordinates` column tests
- `context_raw` column tests
- Hybrid query pattern tests
- Context flattening logic tests
- PROMOTED_FIELDS constant tests
- apply_processed_context() tests

**Total effort saved: ~30-40% fewer test cycles**

---

## Implementation Checklist (Simple Blob)

### Unit Tests

- [ ] TC-001: SourceConfig deserializes ndp_id
- [ ] TC-002: SourceConfig deserializes context
- [ ] TC-003: SourceConfig ndp_id optional
- [ ] TC-004: SourceConfig round-trips with new fields
- [ ] TC-005: Context serializes to JSON string preserving all structure
- [ ] TC-006: Empty context serializes to "{}"
- [ ] TC-007: Nested context preserved in serialization
- [ ] TC-008: Arrays preserved in context serialization

### Integration Tests

- [ ] TC-020: Config sync writes ndp_id to etcd
- [ ] TC-021: Config sync writes context blob to etcd
- [ ] TC-022: Config read includes ndp_id
- [ ] TC-023: Round-trip preserves context structure
- [ ] TC-030: Parser adds ndp_id to point
- [ ] TC-031: Parser adds context as JSON string blob
- [ ] TC-040: Parquet writes ndp_id column
- [ ] TC-041: Parquet writes context STRING column
- [ ] TC-050: Migration creates ndp_id TEXT column
- [ ] TC-051: Migration creates ndp_id B-tree index
- [ ] TC-052: Migration creates context JSONB column
- [ ] TC-053: Migration creates context GIN index

### Acceptance Tests

- [ ] AT-001: Query by ndp_id returns all records
- [ ] AT-002: Query by context field via JSONB works
- [ ] AT-003: Query nested context field via JSONB works
- [ ] AT-004: Context changes in new records only
- [ ] AT-005: ndp_id query uses index (EXPLAIN shows index scan)
- [ ] AT-006: JSONB containment query uses GIN index

---

## Verification Commands

After each cycle:

```bash
# Run the specific test
cargo test <test_name>

# Run all tests for the module
cargo test --package <package> <module>

# Check formatting and linting
cargo fmt --check
cargo clippy -- -D warnings
```

Full suite after all cycles:

```bash
cargo test --workspace
cargo tarpaulin --out Html  # Coverage report
```

---

## Definition of Done

AIR-009 is complete when:

1. All unit tests pass (`cargo test --lib`)
2. All integration tests pass (`cargo test --test integration_*`)
3. All acceptance tests pass (`cargo test --test acceptance -- --include-ignored`)
4. Code coverage > 80% for new code
5. Simple blob approach verified:
   - `ndp_id` column populated in Bronze/Silver
   - `context` blob contains ALL original fields
   - JSONB queries work for context fields
6. Query performance verified:
   - Query by `ndp_id` uses index
   - Query by JSONB context fields works
7. Documentation updated
8. Sample configs include ndp_id and context
9. PR approved and merged
