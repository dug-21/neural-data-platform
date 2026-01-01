# DP-004: Code Changes Specification

## Overview

This document specifies exact files to modify, the nature of changes, and dependencies between them.

**Approach**: Raw JSON storage as defined in ADR-001. Store exact source payloads in Bronze; move parsing to Silver ETL.

> **Simplified**: Platform is <1 week old. No backward compatibility required.
> No schema detection, no dual-write, no migration module needed.

---

## New Schema (ADR-001)

| Column | Type | Description |
|--------|------|-------------|
| `timestamp` | DateTime | Ingestion timestamp (when NDP received the message) |
| `source_id` | String | Source identifier (e.g., "air-quality-Http") |
| `ndp_id` | String? | Platform-assigned stable identifier |
| `context` | JSON? | Config-derived metadata snapshot |
| `raw_payload` | JSON | Exact payload from source, untransformed |

---

## Change Summary

| File | Type | LOC Est. | Phase | Risk | Test Req |
|------|------|----------|-------|------|----------|
| `core/src/types/raw_data_point.rs` | New | +80 | 1 | Low | TC-001 to TC-005 |
| `core/src/types/mod.rs` | Modify | +2 | 1 | Low | N/A |
| `core/src/traits.rs` | Modify | +10 | 1 | Low | N/A |
| `core/src/sources/mod.rs` | Modify | +15 | 2 | Low | TC-010, TC-011 |
| `core/src/sources/http_poll.rs` | Modify | +60 | 2 | Medium | TC-020 to TC-022 |
| `core/src/sources/mqtt.rs` | Modify | +50 | 2 | Medium | TC-023 |
| `core/src/storage/parquet.rs` | Modify | +120 | 3 | High | TC-030 to TC-034 |
| `core/src/parsers/mod.rs` | Modify | +5 | 4 | Low | N/A |
| `apps/.../pipeline/ingestion.rs` | Modify | +80 | 5 | Medium | TC-040, TC-041 |
| `apps/.../coordinator/*.rs` | Modify | +30 | 5 | Medium | N/A |
| `tests/fixtures/mod.rs` | New | +100 | 1 | Low | N/A |
| `tests/integration/test_raw_pipeline.rs` | New | +150 | 6 | Low | AT-001 to AT-004 |

**Total Estimated Changes:** ~700-800 LOC (including tests)

---

## Detailed File Changes

### 1. `core/src/types/raw_data_point.rs` (NEW)

**Phase:** 1
**Risk:** Low
**LOC:** +80
**Test Coverage:** TC-001 to TC-005

#### New File Content

```rust
//! Bronze layer raw data point - stores exact source payloads without transformation.
//!
//! This struct implements the raw JSON storage model from ADR-001. Key principles:
//! - `raw_payload` is sacred: exactly what the source sent
//! - `context` is a snapshot: config-derived metadata frozen at ingestion time
//! - No parsing in Bronze: field extraction happens in Silver layer

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Bronze layer record - raw JSON storage
///
/// # Schema
/// | Column | Type | Description |
/// |--------|------|-------------|
/// | `timestamp` | DateTime | Ingestion timestamp |
/// | `source_id` | String | Source identifier (e.g., "air-quality-Http") |
/// | `ndp_id` | String? | Platform-assigned stable identifier |
/// | `context` | JSON? | Config-derived metadata snapshot |
/// | `raw_payload` | JSON | Exact payload from source |
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawDataPoint {
    /// Ingestion timestamp (when NDP received the message)
    pub timestamp: DateTime<Utc>,

    /// Source identifier in format "{stream_id}-{source_type}"
    /// Examples: "air-quality-Http", "outdoor-weather-Mqtt"
    pub source_id: String,

    /// Platform-assigned stable identifier (from config ndp_id field)
    /// Example: "airgradient-office-001"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ndp_id: Option<String>,

    /// Config-derived metadata snapshot at ingestion time
    /// Stored as JSON blob; queried via DuckDB/JSONB operators
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,

    /// Exact payload from source, untransformed
    /// Contains all fields, types, and nested structures as received
    pub raw_payload: Value,
}

impl Default for RawDataPoint {
    fn default() -> Self {
        Self {
            timestamp: Utc::now(),
            source_id: String::new(),
            ndp_id: None,
            context: None,
            raw_payload: Value::Null,
        }
    }
}

impl RawDataPoint {
    /// Create a new RawDataPoint with required fields
    pub fn new(source_id: impl Into<String>, raw_payload: Value) -> Self {
        Self {
            timestamp: Utc::now(),
            source_id: source_id.into(),
            ndp_id: None,
            context: None,
            raw_payload,
        }
    }

    /// Set custom timestamp
    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Set ndp_id metadata
    pub fn with_ndp_id(mut self, ndp_id: impl Into<String>) -> Self {
        self.ndp_id = Some(ndp_id.into());
        self
    }

    /// Set context metadata
    pub fn with_context(mut self, context: Value) -> Self {
        self.context = Some(context);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_construction_all_fields() {
        let point = RawDataPoint {
            timestamp: Utc::now(),
            source_id: "test-Http".to_string(),
            ndp_id: Some("device-001".to_string()),
            context: Some(json!({"room": "office"})),
            raw_payload: json!({"pm25": 12.5, "status": "active"}),
        };

        assert_eq!(point.source_id, "test-Http");
        assert_eq!(point.ndp_id, Some("device-001".to_string()));
        assert_eq!(point.raw_payload["pm25"], 12.5);
        assert_eq!(point.raw_payload["status"], "active");
    }

    #[test]
    fn test_builder_pattern() {
        let point = RawDataPoint::new("test-Http", json!({"value": 42}))
            .with_ndp_id("test-001")
            .with_context(json!({"room": "lab"}));

        assert_eq!(point.source_id, "test-Http");
        assert_eq!(point.ndp_id, Some("test-001".to_string()));
        assert_eq!(point.context.unwrap()["room"], "lab");
    }

    #[test]
    fn test_preserves_non_numeric_types() {
        let point = RawDataPoint::new("test", json!({
            "string": "hello",
            "boolean": true,
            "null": null,
            "array": [1, "two", false],
            "object": {"nested": "value"}
        }));

        assert_eq!(point.raw_payload["string"], "hello");
        assert_eq!(point.raw_payload["boolean"], true);
        assert!(point.raw_payload["null"].is_null());
        assert_eq!(point.raw_payload["array"][1], "two");
        assert_eq!(point.raw_payload["object"]["nested"], "value");
    }

    #[test]
    fn test_serialization_round_trip() {
        let original = RawDataPoint::new("test", json!({"value": 42}))
            .with_ndp_id("test-001")
            .with_context(json!({"key": "value"}));

        let json_str = serde_json::to_string(&original).unwrap();
        let restored: RawDataPoint = serde_json::from_str(&json_str).unwrap();

        assert_eq!(original, restored);
    }
}
```

---

### 2. `core/src/types/mod.rs`

**Phase:** 1
**Risk:** Low
**LOC:** +2

#### Changes

```rust
// Add new module
mod raw_data_point;

// Add to public exports
pub use raw_data_point::RawDataPoint;
```

---

### 3. `core/src/traits.rs`

**Phase:** 1
**Risk:** Low
**LOC:** +10

#### Changes

Add RawDataStore trait alongside existing Store trait:

```rust
use crate::types::RawDataPoint;

/// Store trait for raw JSON data (Bronze layer)
#[async_trait]
pub trait RawDataStore: Send + Sync {
    /// Write a single raw data point
    async fn write_raw(&self, point: RawDataPoint) -> Result<(), CoreError>;

    /// Write a batch of raw data points
    async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> Result<(), CoreError>;

    /// Query raw data points by time range and optional source filter
    async fn query_raw(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        source_filter: Option<&str>,
    ) -> Result<Vec<RawDataPoint>, CoreError>;
}
```

---

### 4. `core/src/sources/mod.rs`

**Phase:** 2
**Risk:** Low
**LOC:** +15
**Test Coverage:** TC-010, TC-011

#### Add Source ID Generation

```rust
use crate::types::SourceType;

/// Generate source ID in format "{stream_id}-{type_suffix}"
pub fn generate_source_id(stream_id: &str, source_type: &SourceType) -> String {
    let type_suffix = match source_type {
        SourceType::HttpPolling => "Http",
        SourceType::Mqtt => "Mqtt",
        SourceType::Webhook => "Webhook",
    };
    format!("{}-{}", stream_id, type_suffix)
}

/// Generate source ID with index for multi-source streams
pub fn generate_source_id_indexed(
    stream_id: &str,
    source_type: &SourceType,
    index: usize,
) -> String {
    format!("{}-{}", generate_source_id(stream_id, source_type), index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_source_id() {
        assert_eq!(
            generate_source_id("air-quality", &SourceType::HttpPolling),
            "air-quality-Http"
        );
        assert_eq!(
            generate_source_id("sensors", &SourceType::Mqtt),
            "sensors-Mqtt"
        );
    }

    #[test]
    fn test_generate_source_id_indexed() {
        assert_eq!(
            generate_source_id_indexed("air-quality", &SourceType::Mqtt, 0),
            "air-quality-Mqtt-0"
        );
        assert_eq!(
            generate_source_id_indexed("air-quality", &SourceType::Mqtt, 1),
            "air-quality-Mqtt-1"
        );
    }
}
```

---

### 5. `core/src/sources/http_poll.rs`

**Phase:** 2
**Risk:** Medium
**LOC:** +60
**Test Coverage:** TC-020 to TC-022, TC-024

#### Current State (Approximate)

```rust
impl HttpPollingSource {
    pub async fn fetch(&self) -> Result<Vec<TimeSeriesPoint>, SourceError> {
        // Current: Fetches and parses to TimeSeriesPoint
    }
}
```

#### Required Additions

```rust
use crate::types::RawDataPoint;
use crate::sources::generate_source_id;

impl HttpPollingSource {
    /// Configuration for raw data mode
    pub fn new_raw(
        stream_id: &str,
        base_url: &str,
        path: &str,
        ndp_id: Option<String>,
        context: Option<serde_json::Value>,
    ) -> Self {
        let source_id = generate_source_id(stream_id, &SourceType::HttpPolling);
        Self {
            source_id,
            base_url: base_url.to_string(),
            path: path.to_string(),
            ndp_id,
            context,
            // ... other fields
        }
    }

    /// Fetch raw data without parsing
    ///
    /// Returns RawDataPoint with exact JSON response as raw_payload.
    /// No field extraction or type conversion is performed.
    pub async fn fetch_raw(&self) -> Result<RawDataPoint, SourceError> {
        let url = format!("{}{}", self.base_url, self.path);

        let response = self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| SourceError::HttpError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(SourceError::HttpError(format!(
                "HTTP {} from {}",
                response.status(),
                url
            )));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| SourceError::ParseError(format!(
                "Failed to parse JSON from {}: {}",
                url, e
            )))?;

        Ok(RawDataPoint {
            timestamp: Utc::now(),
            source_id: self.source_id.clone(),
            ndp_id: self.ndp_id.clone(),
            context: self.context.clone(),
            raw_payload: json,
        })
    }
}
```

#### Add New Fields to Struct

```rust
pub struct HttpPollingSource {
    // Existing fields
    client: reqwest::Client,
    base_url: String,
    path: String,
    // ...

    // New fields for raw mode
    source_id: String,
    ndp_id: Option<String>,
    context: Option<serde_json::Value>,
}
```

---

### 6. `core/src/sources/mqtt.rs`

**Phase:** 2
**Risk:** Medium
**LOC:** +50
**Test Coverage:** TC-023

#### Required Additions

```rust
use crate::types::RawDataPoint;
use crate::sources::generate_source_id;

impl MqttSource {
    /// Create source configured for raw data mode
    pub fn new_raw(
        stream_id: &str,
        client: impl MqttClient,
        ndp_id: Option<String>,
        context: Option<serde_json::Value>,
    ) -> Self {
        let source_id = generate_source_id(stream_id, &SourceType::Mqtt);
        Self {
            source_id,
            client,
            ndp_id,
            context,
            // ... other fields
        }
    }

    /// Receive raw data without parsing
    ///
    /// Returns RawDataPoint with exact MQTT message payload as raw_payload.
    pub async fn receive_raw(&mut self) -> Result<RawDataPoint, SourceError> {
        let message = self.client.receive().await?;

        let json: serde_json::Value = serde_json::from_slice(&message.payload)
            .map_err(|e| SourceError::ParseError(format!(
                "Failed to parse JSON from MQTT message on topic {}: {}",
                message.topic, e
            )))?;

        Ok(RawDataPoint {
            timestamp: Utc::now(),
            source_id: self.source_id.clone(),
            ndp_id: self.ndp_id.clone(),
            context: self.context.clone(),
            raw_payload: json,
        })
    }
}
```

---

### 7. `core/src/storage/parquet.rs`

**Phase:** 3
**Risk:** High
**LOC:** +120
**Test Coverage:** TC-030 to TC-034

This is the highest-risk change as it modifies the Bronze layer schema.

#### Current State

```rust
// Current schema: tall format with parsed metrics
fn build_schema() -> Schema {
    Schema::new(vec![
        Field::new("timestamp", DataType::Timestamp(...), false),
        Field::new("location_id", DataType::Utf8, false),
        Field::new("metric", DataType::Utf8, false),
        Field::new("value", DataType::Float64, false),
        Field::new("tags", DataType::Utf8, true),
        Field::new("ndp_id", DataType::Utf8, true),
        Field::new("context", DataType::Utf8, true),
    ])
}
```

#### Required Changes

```rust
use crate::types::RawDataPoint;
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::array::{StringArray, TimestampMillisecondArray};

impl ParquetStore {
    /// Build schema for raw data storage (5 columns)
    fn build_raw_schema() -> Schema {
        Schema::new(vec![
            Field::new("timestamp", DataType::Timestamp(TimeUnit::Millisecond, None), false),
            Field::new("source_id", DataType::Utf8, false),
            Field::new("ndp_id", DataType::Utf8, true),      // nullable
            Field::new("context", DataType::Utf8, true),      // JSON as string, nullable
            Field::new("raw_payload", DataType::Utf8, false), // JSON as string
        ])
    }

    /// Get raw data schema
    pub fn get_raw_schema(&self) -> &Schema {
        &self.raw_schema
    }

    /// Write a single raw data point
    pub async fn write_raw(&self, point: RawDataPoint) -> Result<(), StorageError> {
        self.write_raw_batch(vec![point]).await
    }

    /// Write batch of raw data points
    pub async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> Result<(), StorageError> {
        if points.is_empty() {
            return Ok(());
        }

        // Convert to column arrays
        let timestamps: Vec<i64> = points.iter()
            .map(|p| p.timestamp.timestamp_millis())
            .collect();

        let source_ids: Vec<&str> = points.iter()
            .map(|p| p.source_id.as_str())
            .collect();

        let ndp_ids: Vec<Option<&str>> = points.iter()
            .map(|p| p.ndp_id.as_deref())
            .collect();

        let contexts: Vec<Option<String>> = points.iter()
            .map(|p| p.context.as_ref().map(|c| c.to_string()))
            .collect();

        let payloads: Vec<String> = points.iter()
            .map(|p| p.raw_payload.to_string())
            .collect();

        // Create record batch
        let batch = RecordBatch::try_new(
            Arc::new(self.raw_schema.clone()),
            vec![
                Arc::new(TimestampMillisecondArray::from(timestamps)),
                Arc::new(StringArray::from(source_ids)),
                Arc::new(StringArray::from(
                    ndp_ids.iter().map(|o| *o).collect::<Vec<_>>()
                )),
                Arc::new(StringArray::from(
                    contexts.iter().map(|o| o.as_deref()).collect::<Vec<_>>()
                )),
                Arc::new(StringArray::from(
                    payloads.iter().map(|s| s.as_str()).collect::<Vec<_>>()
                )),
            ],
        ).map_err(|e| StorageError::WriteError(e.to_string()))?;

        // Write to partitioned path
        let partition_path = self.get_raw_partition_path(&points[0].timestamp);
        self.write_batch_to_path(batch, &partition_path).await
    }

    /// Query raw data points
    pub async fn query_raw(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        source_filter: Option<&str>,
    ) -> Result<Vec<RawDataPoint>, StorageError> {
        // Find relevant partition files
        let partitions = self.find_partitions(start, end, "raw")?;

        let mut results = Vec::new();

        for partition_path in partitions {
            let file = File::open(&partition_path)?;
            let reader = ParquetRecordBatchReader::try_new(file, 1024)?;

            for batch in reader {
                let batch = batch?;
                let points = self.batch_to_raw_points(&batch)?;

                for point in points {
                    // Apply time filter
                    if point.timestamp < start || point.timestamp > end {
                        continue;
                    }

                    // Apply source filter
                    if let Some(filter) = source_filter {
                        if point.source_id != filter {
                            continue;
                        }
                    }

                    results.push(point);
                }
            }
        }

        Ok(results)
    }

    /// Convert record batch to RawDataPoint vector
    fn batch_to_raw_points(&self, batch: &RecordBatch) -> Result<Vec<RawDataPoint>, StorageError> {
        let timestamps = batch
            .column(0)
            .as_any()
            .downcast_ref::<TimestampMillisecondArray>()
            .ok_or(StorageError::SchemaError("timestamp column mismatch".into()))?;

        let source_ids = batch
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(StorageError::SchemaError("source_id column mismatch".into()))?;

        let ndp_ids = batch
            .column(2)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(StorageError::SchemaError("ndp_id column mismatch".into()))?;

        let contexts = batch
            .column(3)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(StorageError::SchemaError("context column mismatch".into()))?;

        let payloads = batch
            .column(4)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(StorageError::SchemaError("raw_payload column mismatch".into()))?;

        let mut points = Vec::with_capacity(batch.num_rows());

        for i in 0..batch.num_rows() {
            let timestamp = DateTime::from_timestamp_millis(timestamps.value(i))
                .ok_or(StorageError::ParseError("invalid timestamp".into()))?;

            let source_id = source_ids.value(i).to_string();

            let ndp_id = if ndp_ids.is_null(i) {
                None
            } else {
                Some(ndp_ids.value(i).to_string())
            };

            let context = if contexts.is_null(i) {
                None
            } else {
                Some(serde_json::from_str(contexts.value(i))?)
            };

            let raw_payload: serde_json::Value = serde_json::from_str(payloads.value(i))?;

            points.push(RawDataPoint {
                timestamp,
                source_id,
                ndp_id,
                context,
                raw_payload,
            });
        }

        Ok(points)
    }

    /// Get partition path for raw data
    fn get_raw_partition_path(&self, timestamp: &DateTime<Utc>) -> PathBuf {
        let date = timestamp.format("%Y-%m-%d").to_string();
        self.base_path.join("raw").join(date).join(format!(
            "data_{}.parquet",
            timestamp.timestamp_millis()
        ))
    }
}
```

---

### 8. `apps/air-quality-app/src/pipeline/ingestion.rs`

**Phase:** 5
**Risk:** Medium
**LOC:** +80
**Test Coverage:** TC-040, TC-041

#### Required Changes

```rust
use neural_core::types::RawDataPoint;
use neural_core::traits::RawDataStore;

impl IngestionPipeline {
    /// Channel for raw data ingestion
    raw_sender: mpsc::Sender<RawDataPoint>,
    raw_receiver: Option<mpsc::Receiver<RawDataPoint>>,

    /// Create pipeline with raw data support
    pub fn new_with_raw_store(
        store: Arc<dyn RawDataStore>,
        buffer_size: usize,
    ) -> Self {
        let (raw_sender, raw_receiver) = mpsc::channel(buffer_size);
        Self {
            // ... existing fields
            raw_sender,
            raw_receiver: Some(raw_receiver),
            raw_store: store,
        }
    }

    /// Submit raw data point for ingestion
    pub async fn ingest_raw(&self, point: RawDataPoint) -> Result<(), PipelineError> {
        self.raw_sender.send(point).await
            .map_err(|_| PipelineError::ChannelClosed)
    }

    /// Start raw data writer task
    async fn start_raw_writer(&mut self) -> JoinHandle<()> {
        let rx = self.raw_receiver.take()
            .expect("raw_receiver already taken");
        let store = self.raw_store.clone();

        tokio::spawn(Self::raw_writer_task(rx, store))
    }

    /// Background task that batches and writes raw data
    async fn raw_writer_task(
        mut rx: mpsc::Receiver<RawDataPoint>,
        store: Arc<dyn RawDataStore>,
    ) {
        let batch_size = 100;
        let flush_interval = Duration::from_secs(5);
        let mut batch = Vec::with_capacity(batch_size);
        let mut last_flush = Instant::now();

        loop {
            tokio::select! {
                point = rx.recv() => {
                    match point {
                        Some(p) => {
                            batch.push(p);
                            if batch.len() >= batch_size {
                                Self::flush_batch(&store, &mut batch).await;
                                last_flush = Instant::now();
                            }
                        }
                        None => {
                            // Channel closed, flush remaining and exit
                            if !batch.is_empty() {
                                Self::flush_batch(&store, &mut batch).await;
                            }
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep_until(Instant::now() + flush_interval) => {
                    if !batch.is_empty() && last_flush.elapsed() >= flush_interval {
                        Self::flush_batch(&store, &mut batch).await;
                        last_flush = Instant::now();
                    }
                }
            }
        }

        tracing::info!("Raw writer task shutting down");
    }

    async fn flush_batch(
        store: &Arc<dyn RawDataStore>,
        batch: &mut Vec<RawDataPoint>,
    ) {
        if batch.is_empty() {
            return;
        }

        let points: Vec<RawDataPoint> = batch.drain(..).collect();
        let count = points.len();

        match store.write_raw_batch(points).await {
            Ok(_) => {
                tracing::debug!("Flushed {} raw data points", count);
            }
            Err(e) => {
                tracing::error!("Failed to write raw batch: {}", e);
            }
        }
    }
}
```

---

### 9. Test Fixtures (`tests/fixtures/mod.rs`) (NEW)

**Phase:** 1
**Risk:** Low
**LOC:** +100

See MOCK_DEFINITIONS.md for full content.

---

### 10. Integration Tests (`tests/integration/test_raw_pipeline.rs`) (NEW)

**Phase:** 6
**Risk:** Low
**LOC:** +150

See TEST_CASES.md for AT-001 to AT-004.

---

## Dependency Graph

```
raw_data_point.rs (Phase 1)
         |
         ├─────────────────────────────────────────┐
         v                                         v
   sources/mod.rs (Phase 2)              storage/parquet.rs (Phase 3)
         |                                         |
         ├─────────────────┐                       |
         v                 v                       |
   http_poll.rs       mqtt.rs                      |
     (Phase 2)        (Phase 2)                    |
         |                 |                       |
         └────────┬────────┘                       |
                  |                                |
                  v                                v
           pipeline/ingestion.rs (Phase 5) <──────┘
                  |
                  v
           coordinator/*.rs (Phase 5)
                  |
                  v
           Integration Tests (Phase 6)
```

---

## Build Verification Commands

```bash
# After Phase 1 (RawDataPoint)
cargo test --package neural-core raw_data_point
cargo test --package neural-core types::

# After Phase 2 (Sources)
cargo test --package neural-core sources::
cargo test --package neural-core http_poll
cargo test --package neural-core mqtt

# After Phase 3 (Storage)
cargo test --package neural-core storage::parquet

# After Phase 5 (Pipeline)
cargo test --package air-quality-app pipeline

# Full integration
cargo test --workspace

# Lint and format
cargo fmt --check
cargo clippy -- -D warnings
```

---

## Risk Assessment

### High Risk

| File | Risk | Mitigation |
|------|------|------------|
| `storage/parquet.rs` | Schema changes could break reads | Add schema detection; support both old and new schema |

### Medium Risk

| File | Risk | Mitigation |
|------|------|------------|
| `sources/http_poll.rs` | New method alongside existing | Keep existing `fetch()` method; add `fetch_raw()` |
| `sources/mqtt.rs` | New method alongside existing | Keep existing method; add `receive_raw()` |
| `pipeline/ingestion.rs` | New channel and task | Add raw channel separate from existing; gradual rollout |

### Low Risk

| File | Risk | Mitigation |
|------|------|------------|
| `types/raw_data_point.rs` | New file, no changes to existing | Additive change only |
| `types/mod.rs` | Export only | Simple re-export |
| `traits.rs` | New trait | Additive change, no breaks |
| `sources/mod.rs` | Helper functions | Additive change only |

---

## Backward Compatibility Strategy

### Schema Detection

```rust
impl ParquetStore {
    /// Detect schema version from Parquet file
    fn detect_schema_version(path: &Path) -> SchemaVersion {
        let file = File::open(path)?;
        let reader = ParquetReader::new(file);
        let schema = reader.schema();

        if schema.field("raw_payload").is_some() {
            SchemaVersion::RawJson // New DP-004 schema
        } else if schema.field("metric").is_some() {
            SchemaVersion::TallParsed // Old AIR-* schema
        } else {
            SchemaVersion::Unknown
        }
    }

    /// Query with automatic schema handling
    pub async fn query_any(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<QueryResult, StorageError> {
        // Check each partition's schema and handle appropriately
    }
}
```

### Dual-Write Period (Optional)

During transition, can write to both old and new schemas:

```rust
pub async fn write_dual(
    &self,
    point: RawDataPoint,
    parsed_points: Vec<TimeSeriesPoint>,
) -> Result<(), StorageError> {
    // Write raw to new schema
    self.write_raw(point).await?;

    // Write parsed to old schema (for compatibility)
    self.write_batch(parsed_points).await?;

    Ok(())
}
```

---

## Simplification Benefits

The raw JSON storage approach eliminates complexity:

| Removed | Reason |
|---------|--------|
| Parser field extraction | Deferred to Silver ETL |
| Type coercion logic | JSON preserves all types |
| Complex TimeSeriesPoint building | Simple JSON pass-through |
| Per-source parser customization | Generic JSON handling |

**Net Result**: Sources become simpler; Bronze storage becomes simpler; complexity moves to Silver ETL (separate feature).
