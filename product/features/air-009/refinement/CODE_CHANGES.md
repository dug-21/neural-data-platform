# AIR-009: Code Changes Specification

## Overview

This document specifies exact files to modify, the nature of changes, and dependencies between them.

**Amendment**: Updated to reflect **ADR-002-AMENDMENT-002** simple blob context storage approach.

---

## Simple Blob Approach (ADR-002-AMENDMENT-002)

**Key Change**: Maximum simplicity - NO flattening, NO promoted fields.

| Field | Storage |
|-------|---------|
| `ndp_id` | Dedicated column (Bronze: STRING, Silver: TEXT with index) |
| `context` | JSON blob (Bronze: STRING, Silver: JSONB with GIN index) |

**Queries use JSONB operators:**
```sql
WHERE context->>'device_type' = 'airgradient'
WHERE context->'location'->>'type' = 'indoor'
```

---

## Change Summary

| File | Type | LOC Est. | Phase | Risk |
|------|------|----------|-------|------|
| `core/src/types/stream_config.rs` | Modify | +30 | 1 | Low |
| `core/src/lib.rs` | Modify | +2 | 1 | Low |
| `core/src/parsers/traits.rs` | Modify | +15 | 2 | Low |
| `core/src/parsers/flat_json.rs` | Modify | +25 | 2 | Low |
| `core/src/parsers/json_path.rs` | Modify | +25 | 2 | Low |
| `core/src/parsers/column_oriented.rs` | Modify | +25 | 2 | Low |
| `core/src/parsers/array_iterator.rs` | Modify | +25 | 2 | Low |
| `core/src/parsers/factory.rs` | Modify | +10 | 2 | Low |
| `core/src/storage/parquet.rs` | Modify | +50 | 3 | Medium |
| `apps/.../config_sync/service.rs` | Modify | +15 | 2 | Low |
| `config/base/streams/*/config.yaml` | Modify | +10 each | 4 | Low |
| `docs/data-dictionary/schema.sql` | New/Modify | +30 | 5 | Low |

**Total Estimated Changes:** ~350-400 LOC (including tests)

**Significant Reduction**: Simple JSON serialization vs complex flattening/promotion logic.

---

## Detailed File Changes

### 1. `core/src/types/stream_config.rs`

**Phase:** 1
**Risk:** Low
**LOC:** +30

#### Current State (Lines 190-203)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceConfig {
    #[serde(rename = "type")]
    pub source_type: SourceType,

    #[serde(default = "default_enabled")]
    pub enabled: bool,

    #[serde(flatten)]
    pub params: HashMap<String, serde_json::Value>,
}
```

#### Required Changes

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceConfig {
    #[serde(rename = "type")]
    pub source_type: SourceType,

    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Stable NDP-assigned identifier (never changes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ndp_id: Option<String>,

    /// Context attributes written with every record (stored as JSON blob)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,

    #[serde(flatten)]
    pub params: HashMap<String, serde_json::Value>,
}
```

#### Validation Function to Add

```rust
/// Validate ndp_id format (lowercase alphanumeric + hyphens, 3-64 chars)
fn is_valid_ndp_id(id: &str) -> bool {
    let len = id.len();
    if len < 3 || len > 64 {
        return false;
    }
    // Must start with lowercase letter
    if !id.chars().next().map_or(false, |c| c.is_ascii_lowercase()) {
        return false;
    }
    // Only lowercase letters, digits, and hyphens
    id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}
```

#### Test Additions

```rust
#[test]
fn test_source_config_with_ndp_id() {
    let config = SourceConfig {
        source_type: SourceType::Mqtt,
        enabled: true,
        ndp_id: Some("airgradient-office-001".to_string()),
        context: None,
        params: HashMap::new(),
    };
    assert_eq!(config.ndp_id, Some("airgradient-office-001".to_string()));
}

#[test]
fn test_ndp_id_validation() {
    assert!(is_valid_ndp_id("airgradient-office-001"));
    assert!(is_valid_ndp_id("nws-ksgj-001"));
    assert!(!is_valid_ndp_id("AB")); // too short
    assert!(!is_valid_ndp_id("UPPERCASE")); // must be lowercase
    assert!(!is_valid_ndp_id("123-start")); // must start with letter
}
```

---

### 2. `core/src/parsers/traits.rs`

**Phase:** 2
**Risk:** Low
**LOC:** +15

#### Add ParseContext Structure (Simple Blob)

```rust
/// Context passed to parsers for enrichment
#[derive(Debug, Clone, Default)]
pub struct ParseContext {
    /// Stable NDP source identifier
    pub ndp_id: Option<String>,

    /// Context as JSON string blob
    pub context: Option<String>,
}

impl ParseContext {
    /// Create ParseContext from SourceConfig
    pub fn from_source_config(config: &SourceConfig) -> Self {
        Self {
            ndp_id: config.ndp_id.clone(),
            context: config.context.as_ref()
                .map(|c| serde_json::to_string(c).unwrap_or_default()),
        }
    }
}
```

---

### 3. `core/src/parsers/flat_json.rs`

**Phase:** 2
**Risk:** Low
**LOC:** +25

#### Modifications (Simple Blob)

```rust
impl FlatJsonParser {
    /// Parse with context injection
    /// - ndp_id attached as dedicated field
    /// - context serialized as JSON blob
    pub fn parse_with_context(
        &self,
        payload: &[u8],
        parse_context: &ParseContext,
    ) -> Result<Vec<TimeSeriesPoint>, ParserError> {
        let mut points = self.parse(payload)?;

        // Inject ndp_id and context into all points
        for point in &mut points {
            // Set ndp_id
            if let Some(ref ndp_id) = parse_context.ndp_id {
                point.ndp_id = Some(ndp_id.clone());
            }

            // Set context as JSON blob
            if let Some(ref context) = parse_context.context {
                point.context = Some(context.clone());
            }
        }

        Ok(points)
    }
}
```

---

### 4. `core/src/storage/parquet.rs`

**Phase:** 3
**Risk:** Medium
**LOC:** +50

#### Schema Updates (Simple Blob - ADR-002-AMENDMENT-002)

Add to `write_parquet()`:

```rust
use arrow::datatypes::{DataType, Field, Schema};
use arrow::array::StringArray;
use std::sync::Arc;

// Build schema with simple blob columns
fn build_schema() -> Schema {
    Schema::new(vec![
        // Existing columns
        Field::new("timestamp", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("location_id", DataType::Utf8, false),
        Field::new("fields", DataType::Utf8, true),
        Field::new("tags", DataType::Utf8, true),

        // AIR-009: ndp_id
        Field::new("ndp_id", DataType::Utf8, true),

        // AIR-009: context as JSON blob
        Field::new("context", DataType::Utf8, true),
    ])
}

// Extract columns from TimeSeriesPoint
impl ParquetStore {
    fn extract_columns(&self, points: &[TimeSeriesPoint]) -> Result<RecordBatch, StorageError> {
        // ndp_id column
        let ndp_ids: Vec<Option<&str>> = points.iter()
            .map(|p| p.ndp_id.as_deref())
            .collect();

        // context column (JSON blob)
        let contexts: Vec<Option<&str>> = points.iter()
            .map(|p| p.context.as_deref())
            .collect();

        RecordBatch::try_new(
            Arc::new(self.schema.clone()),
            vec![
                // ... existing columns
                Arc::new(StringArray::from(ndp_ids)),
                Arc::new(StringArray::from(contexts)),
            ],
        )
    }
}
```

---

### 5. `apps/air-quality-app/src/config_sync/service.rs`

**Phase:** 2
**Risk:** Low
**LOC:** +15

#### Update SourceYaml

```rust
#[derive(Debug, Clone, Deserialize)]
struct SourceYaml {
    #[serde(rename = "type")]
    source_type: String,

    #[serde(default = "default_enabled")]
    enabled: bool,

    /// Stable NDP identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    ndp_id: Option<String>,

    /// Context attributes (stored as blob)
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<serde_yaml::Value>,

    #[serde(flatten)]
    params: std::collections::HashMap<String, serde_yaml::Value>,
}
```

#### Update to_stream_config()

```rust
// In the sources array processing:
let ndp_id = source_yaml.ndp_id.clone();

let context = source_yaml.context.as_ref()
    .map(|c| yaml_to_json(c))
    .transpose()?;

sources.push(SourceConfig {
    source_type,
    enabled: source_yaml.enabled,
    ndp_id,     // NEW
    context,    // NEW (as serde_json::Value)
    params,
});
```

---

### 6. Stream Configuration Files

**Phase:** 4
**Risk:** Low
**LOC:** +10 each (6 files)

#### `config/base/streams/air-quality/config.yaml`

Add after `enabled: true`:

```yaml
sources:
  - type: mqtt
    enabled: true
    ndp_id: airgradient-office-001
    context:
      location:
        coordinates: [29.95838, -81.30878]
        type: indoor
        path: home/upstairs/office
      device_type: airgradient
      model: ONE-V9
      tags:
        - primary
        - calibrated
    # ... rest of existing config
```

#### Similar changes for:

- `config/base/streams/outdoor-weather/config.yaml` (ndp_id: `owm-home-001`)
- `config/base/streams/outdoor-air-quality/config.yaml` (ndp_id: `owm-air-home-001`)
- `config/base/streams/nws-observations/config.yaml` (ndp_id: `nws-ksgj-001`)
- `config/base/streams/nws-forecast-hourly/config.yaml` (ndp_id: `nws-ksgj-forecast-001`)
- `config/base/streams/nws-gridpoints-forecast/config.yaml` (ndp_id: `nws-ksgj-grid-001`)

---

### 7. Silver Layer Schema

**Phase:** 5
**Risk:** Low
**LOC:** +30

#### Migration Script

```sql
-- AIR-009: Source Identity and Context Configuration (Simple Blob)

-- Add ndp_id column
ALTER TABLE sensor_readings
ADD COLUMN IF NOT EXISTS ndp_id TEXT;

-- Add context JSONB column
ALTER TABLE sensor_readings
ADD COLUMN IF NOT EXISTS context JSONB;

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_readings_ndp_id
ON sensor_readings(ndp_id);

CREATE INDEX IF NOT EXISTS idx_readings_context
ON sensor_readings USING GIN (context);

-- Documentation comments
COMMENT ON COLUMN sensor_readings.ndp_id IS
    'Stable NDP-assigned source identifier (immutable)';

COMMENT ON COLUMN sensor_readings.context IS
    'Full context as JSONB blob - query with JSONB operators';
```

---

## Dependency Graph (Simple Blob)

```
stream_config.rs (Phase 1)
         |
         ├──────────────────┐
         v                  v
   parsers/traits.rs    config_sync/service.rs
     (Phase 2)              (Phase 2)
         |                    |
         v                    v
   parsers/*.rs          Stream Configs
     (Phase 2)              (Phase 4)
    [Simple context blob]
         |
         v
   storage/parquet.rs
     (Phase 3)
    [ndp_id + context columns]
         |
         v
   Silver Schema
     (Phase 5)
    [ndp_id TEXT + context JSONB + indexes]
```

---

## Build Verification Commands

```bash
# After each phase, run:
cargo fmt --check
cargo clippy -- -D warnings
cargo test

# Full integration test:
cargo test --workspace

# Specific test files:
cargo test --package neural-core stream_config
cargo test --package neural-core parsers
cargo test --package neural-core storage
cargo test --package air-quality-app config_sync
```

---

## Risk Assessment

### Medium Risk

| File | Risk | Mitigation |
|------|------|------------|
| `storage/parquet.rs` | Schema changes could break reads | Add column presence checks; handle missing cols as NULL |

### Low Risk (Simplified)

| File | Risk | Mitigation |
|------|------|------------|
| `stream_config.rs` | Additive changes only | Optional fields with defaults |
| `parsers/*.rs` | Simple string assignment | Unit tests for context attachment |
| `config_sync/service.rs` | YAML to JSON conversion | Test parse before deploy |
| `config/*.yaml` | Validation might fail | Test parse before deploy |
| `lib.rs` | Module export | Simple re-export |

**Note**: Risk significantly reduced from original plan because:
- No `process_context()` function needed
- No `ProcessedContext` struct
- No promoted field extraction logic
- Just simple JSON serialization: `serde_json::to_string(&context)`

---

## Implementation is Now Trivial

The core context handling is just:

```rust
// In parser
if let Some(ref context) = config.context {
    let context_json = serde_json::to_string(context)?;
    point.context = Some(context_json);
}
```

That's it. No flattening, no promoted fields, no complex processing.

---

## Testing Strategy

### Unit Tests (Phase 1-2)

- Test SourceConfig serialization/deserialization
- Test context JSON round-trip
- Test ndp_id validation

### Integration Tests (Phase 3)

- Round-trip: YAML -> etcd -> application
- Write/read Parquet with new columns
- Query by ndp_id

### End-to-End Tests (Phase 4-5)

- Deploy updated configs
- Ingest sample data
- Query from TimescaleDB using JSONB operators
- Verify context in records
