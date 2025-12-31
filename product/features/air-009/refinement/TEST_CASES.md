# AIR-009: Detailed Test Cases

## Test Case Format

Each test case follows Given-When-Then (GWT) format with London School annotations:

```
TC-XXX: <Test Name>
Type: Unit | Integration | Acceptance
SUT: System Under Test
Mocks: List of mocked collaborators
Given: Preconditions
When: Action performed
Then: Expected outcome
```

---

## Simple Blob Approach (ADR-002-AMENDMENT-002)

**Key Change**: Maximum simplicity - NO flattening, NO promoted fields.
1. **`ndp_id`** - Stored as dedicated column (fast indexed queries)
2. **`context`** - Stored as single JSON blob (Bronze: STRING, Silver: JSONB)

**Schema:**
| Layer | Columns |
|-------|---------|
| Bronze (Parquet) | `ndp_id` (STRING), `context` (STRING/JSON) |
| Silver (TimescaleDB) | `ndp_id` (TEXT), `context` (JSONB) |

**All context queries use JSONB operators in Silver layer.**

---

## Unit Tests: Context Serialization

### TC-001: Serialize context to JSON string

**Type**: Unit
**SUT**: Context serialization
**Mocks**: None (pure function)

```
Given: context = {"location": {"type": "indoor", "path": "home/office"}, "device_type": "airgradient"}
When: serde_json::to_string(context) is called
Then: result is valid JSON string containing all fields
```

```rust
#[test]
fn tc_001_serialize_context_to_json_string() {
    let context = json!({
        "location": {
            "type": "indoor",
            "path": "home/office"
        },
        "device_type": "airgradient"
    });

    let result = serde_json::to_string(&context).unwrap();

    // Verify it's valid JSON and contains all fields
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["location"]["type"], "indoor");
    assert_eq!(parsed["location"]["path"], "home/office");
    assert_eq!(parsed["device_type"], "airgradient");
}
```

---

### TC-002: Context preserves nested structure

**Type**: Unit
**SUT**: Context serialization
**Mocks**: None

```
Given: context with deeply nested calibration data
When: serialized to JSON string
Then: All nested structure is preserved exactly
```

```rust
#[test]
fn tc_002_context_preserves_nested_structure() {
    let context = json!({
        "location": {
            "type": "indoor",
            "path": "home/upstairs/office",
            "coordinates": [29.958, -81.308]
        },
        "calibration": {
            "sensor_a": {
                "offset": 0.5,
                "last_date": "2024-01-15"
            }
        },
        "tags": ["primary", "calibrated"]
    });

    let json_str = serde_json::to_string(&context).unwrap();
    let restored: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    // Verify nested structure preserved
    assert_eq!(restored["location"]["coordinates"][0], 29.958);
    assert_eq!(restored["calibration"]["sensor_a"]["offset"], 0.5);
    assert_eq!(restored["tags"][0], "primary");
}
```

---

### TC-003: Empty context serializes to empty object

**Type**: Unit
**SUT**: Context serialization
**Mocks**: None

```
Given: context = {}
When: serialized to JSON string
Then: result == "{}"
```

```rust
#[test]
fn tc_003_empty_context_serializes_to_empty_object() {
    let context = json!({});
    let result = serde_json::to_string(&context).unwrap();
    assert_eq!(result, "{}");
}
```

---

### TC-004: Null values preserved in context

**Type**: Unit
**SUT**: Context serialization
**Mocks**: None

```
Given: context = {"location": {"type": null, "path": "home/office"}}
When: serialized to JSON string
Then: null value is preserved in the JSON
```

```rust
#[test]
fn tc_004_null_values_preserved_in_context() {
    let context = json!({
        "location": {
            "type": null,
            "path": "home/office"
        }
    });

    let json_str = serde_json::to_string(&context).unwrap();
    let restored: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    assert!(restored["location"]["type"].is_null());
    assert_eq!(restored["location"]["path"], "home/office");
}
```

---

### TC-005: Arrays preserved in context

**Type**: Unit
**SUT**: Context serialization
**Mocks**: None

```
Given: context with arrays (coordinates, tags)
When: serialized to JSON string
Then: arrays are preserved with correct values and order
```

```rust
#[test]
fn tc_005_arrays_preserved_in_context() {
    let context = json!({
        "location": {
            "coordinates": [29.958, -81.308]
        },
        "tags": ["primary", "calibrated", "indoor"]
    });

    let json_str = serde_json::to_string(&context).unwrap();
    let restored: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    let coords = restored["location"]["coordinates"].as_array().unwrap();
    assert_eq!(coords.len(), 2);
    assert!((coords[0].as_f64().unwrap() - 29.958).abs() < 0.001);

    let tags = restored["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 3);
    assert_eq!(tags[0], "primary");
}
```

---

## Unit Tests: SourceConfig Parsing

### TC-010: SourceConfig deserializes ndp_id field

**Type**: Unit
**SUT**: `SourceConfig` deserialization
**Mocks**: None

```
Given: YAML with ndp_id: "airgradient-office-001"
When: serde_yaml::from_str() is called
Then: config.ndp_id == Some("airgradient-office-001")
```

```rust
#[test]
fn tc_010_source_config_deserializes_ndp_id() {
    let yaml = r#"
        type: mqtt
        enabled: true
        ndp_id: airgradient-office-001
        broker_url: mosquitto
    "#;

    let config: SourceConfig = serde_yaml::from_str(yaml).unwrap();

    assert_eq!(config.ndp_id, Some("airgradient-office-001".to_string()));
}
```

---

### TC-011: SourceConfig deserializes context object

**Type**: Unit
**SUT**: `SourceConfig` deserialization
**Mocks**: None

```
Given: YAML with context: {location: {type: indoor}}
When: serde_yaml::from_str() is called
Then: config.context["location"]["type"] == "indoor"
```

```rust
#[test]
fn tc_011_source_config_deserializes_context() {
    let yaml = r#"
        type: mqtt
        enabled: true
        ndp_id: test-001
        context:
          location:
            type: indoor
            path: home/office
          device_type: airgradient
    "#;

    let config: SourceConfig = serde_yaml::from_str(yaml).unwrap();

    let ctx = config.context.unwrap();
    assert_eq!(ctx["location"]["type"], "indoor");
    assert_eq!(ctx["location"]["path"], "home/office");
    assert_eq!(ctx["device_type"], "airgradient");
}
```

---

### TC-012: SourceConfig without ndp_id is valid (optional)

**Type**: Unit
**SUT**: `SourceConfig` deserialization
**Mocks**: None

```
Given: YAML without ndp_id field
When: serde_yaml::from_str() is called
Then: config.ndp_id == None (no error)
```

```rust
#[test]
fn tc_012_source_config_ndp_id_optional() {
    let yaml = r#"
        type: mqtt
        enabled: true
        broker_url: mosquitto
    "#;

    let config: SourceConfig = serde_yaml::from_str(yaml).unwrap();

    assert_eq!(config.ndp_id, None);
    assert_eq!(config.context, None);
}
```

---

### TC-013: SourceConfig round-trips through JSON

**Type**: Unit
**SUT**: `SourceConfig` serialization
**Mocks**: None

```
Given: SourceConfig with ndp_id and context
When: serialized to JSON and deserialized back
Then: original == deserialized
```

```rust
#[test]
fn tc_013_source_config_round_trips() {
    let config = SourceConfig {
        source_type: SourceType::Mqtt,
        enabled: true,
        ndp_id: Some("test-001".into()),
        context: Some(json!({"location": {"type": "indoor"}})),
        params: HashMap::new(),
    };

    let json = serde_json::to_string(&config).unwrap();
    let restored: SourceConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.ndp_id, restored.ndp_id);
    assert_eq!(config.context, restored.context);
}
```

---

## Integration Tests: ConfigSyncService (etcd)

### TC-020: Config sync writes ndp_id to etcd

**Type**: Integration
**SUT**: `ConfigSyncService`
**Mocks**: `MockEtcdClient`

```
Given: StreamConfig with source.ndp_id = "test-001"
When: ConfigSyncService.sync(config) is called
Then: etcd.put("/streams/{id}/sources/0/ndp_id", "test-001") was called
```

```rust
#[tokio::test]
async fn tc_020_config_sync_writes_ndp_id() {
    let mock_etcd = MockEtcdClient::new();
    mock_etcd.expect_put()
        .withf(|key, _| key.contains("/ndp_id"))
        .times(1)
        .returning(|_, _| Ok(()));

    let sync = ConfigSyncService::with_client(mock_etcd);
    sync.sync(test_config_with_ndp_id()).await.unwrap();

    mock_etcd.checkpoint(); // Verify expectations met
}
```

---

### TC-021: Config sync writes context as JSON blob

**Type**: Integration
**SUT**: `ConfigSyncService`
**Mocks**: `MockEtcdClient`

```
Given: StreamConfig with context = {location: {type: indoor}}
When: ConfigSyncService.sync(config) is called
Then: etcd.put("/streams/{id}/sources/0/context", JSON_BLOB) was called
```

```rust
#[tokio::test]
async fn tc_021_config_sync_writes_context_blob() {
    let mock_etcd = MockEtcdClient::new();
    mock_etcd.expect_put()
        .withf(|key, val| {
            key.contains("/context") &&
            // Verify it's a JSON blob, not flattened keys
            String::from_utf8_lossy(val).contains("{")
        })
        .times(1)
        .returning(|_, _| Ok(()));

    let sync = ConfigSyncService::with_client(mock_etcd);
    sync.sync(test_config_with_context()).await.unwrap();

    mock_etcd.checkpoint();
}
```

---

### TC-022: Config read from etcd includes ndp_id

**Type**: Integration
**SUT**: `ConfigClient`
**Mocks**: `MockEtcdClient` (stub)

```
Given: etcd has key "/streams/air-quality/sources/0/ndp_id" = "test-001"
When: ConfigClient.get_stream("air-quality") is called
Then: result.sources[0].ndp_id == "test-001"
```

```rust
#[tokio::test]
async fn tc_022_config_read_includes_ndp_id() {
    let mock_etcd = MockEtcdClient::new();
    mock_etcd.expect_get()
        .returning(|_| Ok(etcd_response_with_ndp_id()));

    let client = ConfigClient::with_client(mock_etcd);
    let config = client.get_stream("air-quality").await.unwrap();

    assert_eq!(config.sources[0].ndp_id, Some("test-001".into()));
}
```

---

### TC-023: Config round-trip through etcd preserves context blob

**Type**: Integration
**SUT**: `ConfigClient` + `ConfigSyncService`
**Mocks**: In-memory etcd fake

```
Given: StreamConfig with full context blob
When: sync(config) then get_stream(id) is called
Then: original.context == retrieved.context (full blob preserved)
```

```rust
#[tokio::test]
async fn tc_023_round_trip_preserves_context_blob() {
    let fake_etcd = FakeEtcdClient::new();

    let original = test_config_with_full_context();

    let sync = ConfigSyncService::with_client(fake_etcd.clone());
    sync.sync(original.clone()).await.unwrap();

    let client = ConfigClient::with_client(fake_etcd);
    let retrieved = client.get_stream(&original.stream_id).await.unwrap();

    // Full context blob preserved
    assert_eq!(original.sources[0].context, retrieved.sources[0].context);
}
```

---

## Integration Tests: Parser Context Attachment

### TC-030: Parser adds ndp_id to parsed records

**Type**: Integration
**SUT**: `FlatJsonParser`
**Mocks**: None (parser uses stub config)

```
Given: Parser configured with source.ndp_id = "test-001"
When: Parser.parse(mqtt_payload) is called
Then: result.ndp_id == "test-001"
```

```rust
#[test]
fn tc_030_parser_adds_ndp_id() {
    let config = SourceConfigBuilder::new()
        .ndp_id("test-001")
        .build();

    let parser = FlatJsonParser::with_source_config(config);
    let payload = r#"{"pm25": 12.5, "temperature": 22.3}"#;

    let record = parser.parse(payload.as_bytes()).unwrap();

    assert_eq!(record.ndp_id, Some("test-001".to_string()));
}
```

---

### TC-031: Parser adds context as JSON blob

**Type**: Integration
**SUT**: `FlatJsonParser`
**Mocks**: None

```
Given: Parser configured with context = {location: {type: indoor}, device_type: airgradient}
When: Parser.parse(mqtt_payload) is called
Then: record.context contains full JSON blob
```

```rust
#[test]
fn tc_031_parser_adds_context_blob() {
    let config = SourceConfigBuilder::new()
        .context(json!({
            "location": {
                "type": "indoor",
                "path": "home/office"
            },
            "device_type": "airgradient"
        }))
        .build();

    let parser = FlatJsonParser::with_source_config(config);
    let payload = r#"{"pm25": 12.5}"#;

    let record = parser.parse(payload.as_bytes()).unwrap();

    // Context is stored as JSON string blob
    let context = record.context.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&context).unwrap();
    assert_eq!(parsed["location"]["type"], "indoor");
    assert_eq!(parsed["device_type"], "airgradient");
}
```

---

### TC-032: Parser preserves complex nested context

**Type**: Integration
**SUT**: `FlatJsonParser`
**Mocks**: None

```
Given: Parser configured with deeply nested context
When: Parser.parse(mqtt_payload) is called
Then: All nested structure preserved in context blob
```

```rust
#[test]
fn tc_032_parser_preserves_nested_context() {
    let config = SourceConfigBuilder::new()
        .context(json!({
            "location": {
                "type": "indoor",
                "path": "home/office",
                "coordinates": [29.958, -81.308]
            },
            "device_type": "airgradient",
            "model": "ONE-V9",
            "calibration": {
                "offset": 0.5
            }
        }))
        .build();

    let parser = FlatJsonParser::with_source_config(config);
    let payload = r#"{"pm25": 12.5}"#;

    let record = parser.parse(payload.as_bytes()).unwrap();

    let context: serde_json::Value = serde_json::from_str(&record.context.unwrap()).unwrap();
    assert_eq!(context["calibration"]["offset"], 0.5);
    assert_eq!(context["location"]["coordinates"][0], 29.958);
}
```

---

## Integration Tests: Bronze Layer Writer

### TC-040: ParquetStore writes ndp_id column

**Type**: Integration
**SUT**: `ParquetStore`
**Mocks**: `tempfile::TempDir`

```
Given: TimeSeriesPoint with ndp_id = "test-001"
When: ParquetStore.write(point) is called
Then: Parquet file contains ndp_id column with value "test-001"
```

```rust
#[tokio::test]
async fn tc_040_parquet_writes_ndp_id_column() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let mut point = create_test_point();
    point.ndp_id = Some("test-001".to_string());

    store.write(point.clone()).await.unwrap();

    // Read parquet and verify column exists
    let path = store.partition_path("test-stream", point.timestamp);
    let df = ParquetReader::new(File::open(path).unwrap())
        .finish()
        .unwrap();

    assert!(df.column_names().contains(&"ndp_id"));
}
```

---

### TC-041: ParquetStore writes context as STRING column

**Type**: Integration
**SUT**: `ParquetStore`
**Mocks**: `tempfile::TempDir`

```
Given: TimeSeriesPoint with context JSON blob
When: ParquetStore.write(point) is called
Then: Parquet file contains context STRING column with JSON blob
```

```rust
#[tokio::test]
async fn tc_041_parquet_writes_context_blob() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let context = json!({
        "location": {"type": "indoor"},
        "device_type": "airgradient"
    });

    let mut point = create_test_point();
    point.context = Some(serde_json::to_string(&context).unwrap());

    store.write(point.clone()).await.unwrap();

    // Read parquet and verify context column
    let path = store.partition_path("test-stream", point.timestamp);
    let df = ParquetReader::new(File::open(path).unwrap())
        .finish()
        .unwrap();

    assert!(df.column_names().contains(&"context"));

    // Verify the content is valid JSON blob
    let context_col = df.column("context").unwrap();
    let first_value = context_col.get(0).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&first_value.to_string()).unwrap();
    assert_eq!(parsed["device_type"], "airgradient");
}
```

---

### TC-042: ParquetStore handles missing optional columns

**Type**: Integration
**SUT**: `ParquetStore`
**Mocks**: `tempfile::TempDir`

```
Given: TimeSeriesPoint with ndp_id but no context
When: ParquetStore.write(point) is called
Then: Parquet file has ndp_id populated, context is NULL
```

```rust
#[tokio::test]
async fn tc_042_parquet_handles_missing_optional_columns() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let mut point = create_test_point();
    point.ndp_id = Some("test-001".to_string());
    point.context = None;  // No context

    store.write(point.clone()).await.unwrap();

    let results = store.query(
        &point.location_id,
        point.timestamp - Duration::hours(1),
        point.timestamp + Duration::hours(1),
        None
    ).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].ndp_id, Some("test-001".to_string()));
    assert!(results[0].context.is_none());
}
```

---

## Integration Tests: Silver Layer Schema

### TC-050: Migration creates ndp_id column

**Type**: Integration
**SUT**: Schema migration
**Mocks**: `MockTimescaleClient`

```
Given: Fresh TimescaleDB database
When: Migration is run
Then: sensor_readings table has ndp_id TEXT column
```

```rust
#[tokio::test]
async fn tc_050_migration_creates_ndp_id_column() {
    let mock_client = MockTimescaleClient::new();
    mock_client.expect_execute()
        .withf(|sql| {
            sql.contains("ALTER TABLE") &&
            sql.contains("ndp_id") &&
            sql.contains("TEXT")
        })
        .times(1)
        .returning(|_| Ok(0));

    run_air_009_migration(&mock_client).await.unwrap();
    mock_client.checkpoint();
}
```

---

### TC-051: Migration creates ndp_id index

**Type**: Integration
**SUT**: Schema migration
**Mocks**: `MockTimescaleClient`

```
Given: sensor_readings table exists
When: Migration is run
Then: Index idx_readings_ndp_id is created on ndp_id column
```

```rust
#[tokio::test]
async fn tc_051_migration_creates_ndp_id_index() {
    let mock_client = MockTimescaleClient::new();
    mock_client.expect_execute()
        .withf(|sql| {
            sql.contains("CREATE INDEX") &&
            sql.contains("ndp_id")
        })
        .times(1)
        .returning(|_| Ok(0));

    run_air_009_migration(&mock_client).await.unwrap();
    mock_client.checkpoint();
}
```

---

### TC-052: Migration creates context JSONB column

**Type**: Integration
**SUT**: Schema migration
**Mocks**: `MockTimescaleClient`

```
Given: sensor_readings table exists
When: Migration is run
Then: Table has context JSONB column
```

```rust
#[tokio::test]
async fn tc_052_migration_creates_context_jsonb() {
    let mock_client = MockTimescaleClient::new();
    mock_client.expect_execute()
        .withf(|sql| {
            sql.contains("context") &&
            sql.contains("JSONB")
        })
        .times(1)
        .returning(|_| Ok(0));

    run_air_009_migration(&mock_client).await.unwrap();
    mock_client.checkpoint();
}
```

---

### TC-053: Migration creates GIN index on context JSONB

**Type**: Integration
**SUT**: Schema migration
**Mocks**: `MockTimescaleClient`

```
Given: sensor_readings table exists
When: Migration is run
Then: GIN index created on context column for JSONB queries
```

```rust
#[tokio::test]
async fn tc_053_migration_creates_context_gin_index() {
    let mock_client = MockTimescaleClient::new();
    mock_client.expect_execute()
        .withf(|sql| {
            sql.contains("CREATE INDEX") &&
            sql.contains("GIN") &&
            sql.contains("context")
        })
        .times(1)
        .returning(|_| Ok(0));

    run_air_009_migration(&mock_client).await.unwrap();
    mock_client.checkpoint();
}
```

---

## Acceptance Tests: End-to-End

### AT-001: Query all records by ndp_id

**Type**: Acceptance (E2E)
**SUT**: Full pipeline (Config -> Ingest -> Query)
**Mocks**: Real components, isolated test environment

```
Given: Stream configured with ndp_id = "airgradient-office-001"
And: Full context blob
And: Multiple records ingested from that source
When: SQL query "SELECT * FROM readings WHERE ndp_id = 'airgradient-office-001'"
Then: All records from that source are returned
And: Full context preserved in context JSONB column
```

```rust
#[tokio::test]
async fn at_001_query_by_ndp_id() {
    let env = TestEnvironment::new().await;

    // Configure stream with ndp_id and full context
    let config = StreamConfig {
        stream_id: "air-quality".into(),
        sources: vec![SourceConfig {
            source_type: SourceType::Mqtt,
            ndp_id: Some("airgradient-office-001".into()),
            context: Some(json!({
                "location": {
                    "type": "indoor",
                    "path": "home/office",
                    "coordinates": [29.958, -81.308]
                },
                "device_type": "airgradient",
                "model": "ONE-V9",
                "tags": ["primary", "calibrated"]
            })),
            ..default()
        }],
        ..default()
    };

    env.sync_config(config).await;

    // Ingest test records
    for i in 0..5 {
        env.ingest_mqtt_payload(json!({
            "pm25": 10.0 + i as f64,
            "temperature": 22.0
        })).await;
    }

    env.wait_for_silver_layer().await;

    // Query by ndp_id
    let results = env.query_sql(
        "SELECT * FROM sensor_readings WHERE ndp_id = 'airgradient-office-001'"
    ).await.unwrap();

    assert_eq!(results.len(), 5);
    for record in &results {
        assert_eq!(record.ndp_id, "airgradient-office-001");

        // Full context preserved in JSONB
        let context: serde_json::Value = record.context.clone();
        assert_eq!(context["device_type"], "airgradient");
        assert_eq!(context["location"]["type"], "indoor");
        assert_eq!(context["model"], "ONE-V9");
    }
}
```

---

### AT-002: Query by context field using JSONB operators

**Type**: Acceptance (E2E)
**SUT**: Silver layer JSONB query
**Mocks**: Real components

```
Given: Records with device_type in context
When: SQL query using JSONB extraction: context->>'device_type' = 'airgradient'
Then: Records filtered correctly
```

```rust
#[tokio::test]
async fn at_002_query_by_context_field() {
    let env = TestEnvironment::new().await;

    let config = StreamConfig {
        stream_id: "air-quality".into(),
        sources: vec![
            SourceConfig {
                ndp_id: Some("airgradient-001".into()),
                context: Some(json!({
                    "location": {"type": "indoor"},
                    "device_type": "airgradient"
                })),
                ..default()
            },
            SourceConfig {
                ndp_id: Some("purpleair-001".into()),
                context: Some(json!({
                    "location": {"type": "indoor"},
                    "device_type": "purpleair"
                })),
                ..default()
            },
        ],
        ..default()
    };

    env.sync_config(config).await;
    env.ingest_from_source("airgradient-001", 3).await;
    env.ingest_from_source("purpleair-001", 3).await;
    env.wait_for_silver_layer().await;

    // Query using JSONB extraction
    let airgradient_results = env.query_sql(
        "SELECT * FROM sensor_readings WHERE context->>'device_type' = 'airgradient'"
    ).await.unwrap();

    assert_eq!(airgradient_results.len(), 3);
    for record in &airgradient_results {
        let ctx: serde_json::Value = record.context.clone();
        assert_eq!(ctx["device_type"], "airgradient");
    }
}
```

---

### AT-003: Query nested context field using JSONB operators

**Type**: Acceptance (E2E)
**SUT**: Silver layer JSONB query
**Mocks**: Real components

```
Given: Records with location.type in context
When: SQL query: context->'location'->>'type' = 'indoor'
Then: Records filtered correctly by nested field
```

```rust
#[tokio::test]
async fn at_003_query_nested_context_field() {
    let env = TestEnvironment::new().await;

    // Configure two sources: indoor and outdoor
    let config = StreamConfig {
        stream_id: "air-quality".into(),
        sources: vec![
            SourceConfig {
                ndp_id: Some("indoor-sensor-001".into()),
                context: Some(json!({
                    "location": {"type": "indoor", "path": "home/office"}
                })),
                ..default()
            },
            SourceConfig {
                ndp_id: Some("outdoor-sensor-001".into()),
                context: Some(json!({
                    "location": {"type": "outdoor", "path": "backyard"}
                })),
                ..default()
            },
        ],
        ..default()
    };

    env.sync_config(config).await;
    env.ingest_from_source("indoor-sensor-001", 3).await;
    env.ingest_from_source("outdoor-sensor-001", 3).await;
    env.wait_for_silver_layer().await;

    // Query by nested field using JSONB operators
    let indoor_results = env.query_sql(
        "SELECT * FROM sensor_readings WHERE context->'location'->>'type' = 'indoor'"
    ).await.unwrap();

    assert_eq!(indoor_results.len(), 3);
    for record in &indoor_results {
        let ctx: serde_json::Value = record.context.clone();
        assert_eq!(ctx["location"]["type"], "indoor");
    }
}
```

---

### AT-004: Context changes reflected in new records only

**Type**: Acceptance (E2E)
**SUT**: Full pipeline with config update
**Mocks**: Real components, isolated test environment

```
Given: Source with context.location.path = "home/office"
And: 3 records ingested
When: Context updated to location.path = "home/bedroom"
And: 3 more records ingested
Then: First 3 records have context with path = "home/office"
And: Last 3 records have context with path = "home/bedroom"
```

```rust
#[tokio::test]
async fn at_004_context_changes_in_new_records() {
    let env = TestEnvironment::new().await;

    // Initial context
    let mut config = create_test_config();
    config.sources[0].context = Some(json!({
        "location": {"type": "indoor", "path": "home/office"}
    }));
    env.sync_config(config.clone()).await;

    // Ingest first batch
    for _ in 0..3 {
        env.ingest_record().await;
    }

    // Update context (change path)
    config.sources[0].context = Some(json!({
        "location": {"type": "indoor", "path": "home/bedroom"}
    }));
    env.sync_config(config).await;

    // Ingest second batch
    for _ in 0..3 {
        env.ingest_record().await;
    }

    env.wait_for_silver_layer().await;

    // Query using JSONB path extraction
    let office = env.query_sql(
        "SELECT * FROM sensor_readings WHERE context->'location'->>'path' = 'home/office'"
    ).await.unwrap();

    let bedroom = env.query_sql(
        "SELECT * FROM sensor_readings WHERE context->'location'->>'path' = 'home/bedroom'"
    ).await.unwrap();

    assert_eq!(office.len(), 3);
    assert_eq!(bedroom.len(), 3);
}
```

---

### AT-005: ndp_id query uses index efficiently

**Type**: Acceptance (Performance)
**SUT**: Silver layer query
**Mocks**: Real TimescaleDB

```
Given: 100,000 records across 10 different ndp_ids
When: Query by specific ndp_id is executed
Then: Query uses idx_readings_ndp_id index (EXPLAIN shows Index Scan)
And: Query completes in < 100ms
```

```rust
#[tokio::test]
async fn at_005_ndp_id_query_uses_index() {
    let env = TestEnvironment::new().await;

    // Setup: Insert many records (can be stubbed for speed)
    env.seed_test_data(100_000, 10).await;

    // Check query plan
    let plan = env.explain_sql(
        "SELECT * FROM sensor_readings WHERE ndp_id = 'test-001'"
    ).await.unwrap();

    assert!(
        plan.contains("Index Scan") || plan.contains("idx_readings_ndp_id"),
        "Query should use ndp_id index"
    );

    // Time the query
    let start = Instant::now();
    let _ = env.query_sql(
        "SELECT * FROM sensor_readings WHERE ndp_id = 'test-001'"
    ).await.unwrap();
    let elapsed = start.elapsed();

    assert!(elapsed < Duration::from_millis(100));
}
```

---

### AT-006: JSONB GIN index supports context queries

**Type**: Acceptance (Performance)
**SUT**: Silver layer query
**Mocks**: Real TimescaleDB

```
Given: 100,000 records with context
When: Query by context field
Then: GIN index is used for efficient access
```

```rust
#[tokio::test]
async fn at_006_context_gin_index_used() {
    let env = TestEnvironment::new().await;

    env.seed_test_data_with_context(100_000).await;

    let plan = env.explain_sql(
        "SELECT * FROM sensor_readings WHERE context @> '{\"device_type\": \"airgradient\"}'"
    ).await.unwrap();

    assert!(
        plan.contains("Bitmap Index Scan") || plan.contains("idx_readings_context"),
        "Query should use context GIN index"
    );
}
```

---

## Test Data Fixtures

### Fixture: Test Stream Config

```rust
fn create_test_stream_config() -> StreamConfig {
    StreamConfig {
        stream_id: "air-quality".into(),
        description: "Test air quality stream".into(),
        version: "1.0.0".into(),
        enabled: true,
        retention_days: 365,
        compression_after_days: 7,
        partitioning_strategy: "daily".into(),
        fields: vec![
            SchemaField::new("pm25".into(), FieldType::Float),
            SchemaField::new("temperature".into(), FieldType::Float),
        ],
        sources: vec![SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: Some("airgradient-office-001".into()),
            context: Some(json!({
                "location": {
                    "coordinates": [29.958, -81.308],
                    "type": "indoor",
                    "path": "home/upstairs/office"
                },
                "device_type": "airgradient",
                "model": "ONE-V9",
                "tags": ["primary", "calibrated"]
            })),
            params: HashMap::new(),
        }],
        storage: None,
    }
}
```

### Fixture: Test MQTT Payload

```rust
fn create_test_mqtt_payload() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "serial_number": "d83bda1cd074",
        "pm25": 12.5,
        "pm10": 18.2,
        "temperature": 22.3,
        "humidity": 45.0,
        "co2": 650,
        "wifi_rssi": -65
    })).unwrap()
}
```

---

## Test Execution Matrix

| Test ID | Component | Type | Priority | Dependencies |
|---------|-----------|------|----------|--------------|
| TC-001 | Context Serialization | Unit | P0 | None |
| TC-002 | Context Serialization | Unit | P0 | None |
| TC-003 | Context Serialization | Unit | P0 | None |
| TC-004 | Context Serialization | Unit | P1 | None |
| TC-005 | Context Serialization | Unit | P1 | None |
| TC-010 | SourceConfig | Unit | P0 | None |
| TC-011 | SourceConfig | Unit | P0 | TC-010 |
| TC-012 | SourceConfig | Unit | P0 | TC-010 |
| TC-013 | SourceConfig | Unit | P1 | TC-010 |
| TC-020 | ConfigSync | Integration | P1 | TC-010, TC-011 |
| TC-021 | ConfigSync | Integration | P1 | TC-020 |
| TC-022 | ConfigSync | Integration | P1 | TC-020 |
| TC-023 | ConfigSync | Integration | P1 | TC-020 |
| TC-030 | Parser | Integration | P0 | TC-001 |
| TC-031 | Parser | Integration | P0 | TC-030 |
| TC-032 | Parser | Integration | P1 | TC-030 |
| TC-040 | ParquetStore (ndp_id) | Integration | P0 | TC-030 |
| TC-041 | ParquetStore (context) | Integration | P0 | TC-040 |
| TC-042 | ParquetStore (nullable) | Integration | P1 | TC-041 |
| TC-050 | Silver Layer (ndp_id) | Integration | P0 | TC-040 |
| TC-051 | Silver Layer (index) | Integration | P1 | TC-050 |
| TC-052 | Silver Layer (JSONB) | Integration | P0 | TC-050 |
| TC-053 | Silver Layer (GIN idx) | Integration | P1 | TC-052 |
| AT-001 | Full Pipeline | Acceptance | P0 | All above |
| AT-002 | Query by context field | Acceptance | P0 | AT-001 |
| AT-003 | Query nested context | Acceptance | P0 | AT-001 |
| AT-004 | Context Changes | Acceptance | P1 | AT-001 |
| AT-005 | ndp_id Index Performance | Acceptance | P2 | AT-001 |
| AT-006 | GIN Index Performance | Acceptance | P2 | AT-002 |
