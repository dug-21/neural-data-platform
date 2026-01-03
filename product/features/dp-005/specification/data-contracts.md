# DP-005: Bronze MCP Server - Data Contracts

**Document Type**: SPARC Specification
**Version**: 1.0.0
**Last Updated**: 2026-01-03
**Status**: Draft

---

## Overview

This document defines the data structures and contracts that the Bronze MCP Server reads from and exposes to clients. It covers etcd key patterns, Parquet schema, raw_payload JSON structure, and entity_schemas format.

---

## etcd Configuration Store

### Key Pattern

Stream configurations are stored in etcd with flattened YAML paths:

```
/streams/{stream_id}/{path}
```

Where `{path}` is the dot-separated YAML path converted to slash-separated keys.

### Key Examples

For stream `air-quality`:

| etcd Key | Value | Source YAML Path |
|----------|-------|------------------|
| `/streams/air-quality/stream_id` | `"air-quality"` | `stream_id` |
| `/streams/air-quality/description` | `"AirGradient sensor..."` | `description` |
| `/streams/air-quality/version` | `"1.0.0"` | `version` |
| `/streams/air-quality/enabled` | `true` | `enabled` |
| `/streams/air-quality/retention_days` | `365` | `retention_days` |
| `/streams/air-quality/sources/0/type` | `"mqtt"` | `sources[0].type` |
| `/streams/air-quality/sources/0/enabled` | `true` | `sources[0].enabled` |
| `/streams/air-quality/sources/0/ndp_id` | `"aq_airgradient_1"` | `sources[0].ndp_id` |
| `/streams/air-quality/sources/0/parser/parser_type` | `"flat_json"` | `sources[0].parser.parser_type` |
| `/streams/air-quality/entity_schemas/0/schema_name` | `"airgradient"` | `entity_schemas[0].schema_name` |
| `/streams/air-quality/entity_schemas/0/attributes/0/name` | `"pm25"` | `entity_schemas[0].attributes[0].name` |

### Stream Discovery Pattern

To list all streams:

```bash
# Get all stream IDs
etcdctl get --prefix /streams/ --keys-only | grep -E '/streams/[^/]+/stream_id$'
```

### Key Structure by Section

#### Stream Metadata
```
/streams/{stream_id}/stream_id         → string
/streams/{stream_id}/description       → string
/streams/{stream_id}/version           → string
/streams/{stream_id}/enabled           → boolean
/streams/{stream_id}/retention_days    → integer
```

#### Sources Array
```
/streams/{stream_id}/sources/{index}/type               → "mqtt" | "http_poll"
/streams/{stream_id}/sources/{index}/enabled            → boolean
/streams/{stream_id}/sources/{index}/ndp_id             → string
/streams/{stream_id}/sources/{index}/context/{key}      → any
/streams/{stream_id}/sources/{index}/parser/parser_type → string
/streams/{stream_id}/sources/{index}/parser/field_mappings/{index}/path        → string
/streams/{stream_id}/sources/{index}/parser/field_mappings/{index}/metric_name → string
/streams/{stream_id}/sources/{index}/parser/field_mappings/{index}/unit        → string
```

#### Entity Schemas Array
```
/streams/{stream_id}/entity_schemas/{index}/schema_name                     → string
/streams/{stream_id}/entity_schemas/{index}/description                     → string
/streams/{stream_id}/entity_schemas/{index}/device_class                    → string (optional)
/streams/{stream_id}/entity_schemas/{index}/attributes/{index}/name         → string
/streams/{stream_id}/entity_schemas/{index}/attributes/{index}/type         → string
/streams/{stream_id}/entity_schemas/{index}/attributes/{index}/unit         → string (optional)
/streams/{stream_id}/entity_schemas/{index}/attributes/{index}/description  → string
/streams/{stream_id}/entity_schemas/{index}/attributes/{index}/nullable     → boolean
/streams/{stream_id}/entity_schemas/{index}/attributes/{index}/range/{index}→ number (optional)
```

### Reconstructing Objects from etcd

The MCP server reconstructs objects by:

1. Prefix query for stream keys
2. Parse key paths to identify arrays (numeric segments)
3. Rebuild nested structure

**Algorithm:**
```rust
// Pseudocode for reconstruction
fn reconstruct(keys: Vec<(String, Value)>) -> Value {
    let mut root = json!({});
    for (key, value) in keys {
        let segments: Vec<&str> = key.split('/').collect();
        // segments[0] = "", segments[1] = "streams", segments[2] = stream_id, ...
        insert_at_path(&mut root, &segments[3..], value);
    }
    root
}

fn insert_at_path(obj: &mut Value, path: &[&str], value: Value) {
    if path.is_empty() { return; }
    if path.len() == 1 {
        obj[path[0]] = value;
        return;
    }
    // Check if next segment is numeric (array index)
    if path[1].parse::<usize>().is_ok() {
        // Ensure array exists
        if !obj[path[0]].is_array() {
            obj[path[0]] = json!([]);
        }
        // ... continue recursively
    }
}
```

---

## Bronze Layer Parquet Schema

### File Organization

Hive-style partitioning:
```
/data/raw/{stream_id}/
└── year=YYYY/
    └── month=MM/
        └── day=DD/
            └── data.parquet
```

**Example paths:**
```
/data/raw/air-quality/year=2026/month=01/day=03/data.parquet
/data/raw/outdoor-weather/year=2026/month=01/day=03/data.parquet
```

### Parquet Schema Definition

Based on `RawDataPoint` from `core/src/types/raw_data_point.rs`:

| Column | Parquet Type | Arrow Type | Nullable | Description |
|--------|--------------|------------|----------|-------------|
| `timestamp` | INT64 | Timestamp(Microsecond, UTC) | No | Ingestion timestamp |
| `source_id` | BYTE_ARRAY | Utf8 | No | Source identifier |
| `ndp_id` | BYTE_ARRAY | Utf8 | Yes | Platform-assigned ID |
| `context` | BYTE_ARRAY | Utf8 | Yes | JSON-serialized metadata |
| `raw_payload` | BYTE_ARRAY | Utf8 | No | JSON-serialized payload |
| `year` | INT32 | Int32 | No | Partition: year |
| `month` | INT32 | Int32 | No | Partition: month |
| `day` | INT32 | Int32 | No | Partition: day |

### Arrow Schema Definition

```rust
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};

pub fn bronze_schema() -> Schema {
    Schema::new(vec![
        Field::new("timestamp", DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())), false),
        Field::new("source_id", DataType::Utf8, false),
        Field::new("ndp_id", DataType::Utf8, true),
        Field::new("context", DataType::Utf8, true),  // JSON as string
        Field::new("raw_payload", DataType::Utf8, false),  // JSON as string
        // Partition columns added by storage layer
        Field::new("year", DataType::Int32, false),
        Field::new("month", DataType::Int32, false),
        Field::new("day", DataType::Int32, false),
    ])
}
```

### Parquet File Properties

| Property | Value |
|----------|-------|
| Compression | Snappy |
| Row Group Size | Default (varies) |
| Version | Parquet 2.6 |
| Created By | arrow-rs |

### Reading Parquet for Schema Discovery

```rust
use parquet::file::reader::{FileReader, SerializedFileReader};
use std::fs::File;

fn read_schema(path: &Path) -> Result<Schema> {
    let file = File::open(path)?;
    let reader = SerializedFileReader::new(file)?;
    let parquet_schema = reader.metadata().file_metadata().schema();
    let arrow_schema = parquet::arrow::parquet_to_arrow_schema(parquet_schema, None)?;
    Ok(arrow_schema)
}
```

---

## RawDataPoint Structure

### Rust Definition

From `/workspaces/neural-data-platform/core/src/types/raw_data_point.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawDataPoint {
    /// Ingestion timestamp (when NDP received the message)
    pub timestamp: DateTime<Utc>,

    /// Source identifier in format "{stream_id}-{source_type}"
    /// Examples: "air-quality-Mqtt", "outdoor-weather-Http"
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
```

### JSON Serialized Format

When stored in Parquet (as UTF-8 strings):

**context column:**
```json
{
  "device_type": "airgradient",
  "location": {
    "coordinates": [29.95838, -81.30878],
    "type": "indoor",
    "path": "/beachhouse/livingroom"
  },
  "environment": "indoor"
}
```

**raw_payload column (AirGradient example):**
```json
{
  "pm25": 12,
  "pm10": 15,
  "co2": 650,
  "temperature": 23.5,
  "humidity": 55,
  "tvoc": 120,
  "nox": 45,
  "serialno": "abc123",
  "firmware": "3.1.0",
  "model": "ONE"
}
```

**raw_payload column (OpenWeatherMap example):**
```json
{
  "coord": {"lon": -81.3088, "lat": 29.9584},
  "weather": [
    {"id": 803, "main": "Clouds", "description": "broken clouds", "icon": "04n"}
  ],
  "base": "stations",
  "main": {
    "temp": 19.72,
    "feels_like": 19.78,
    "temp_min": 18.29,
    "temp_max": 21.15,
    "pressure": 1020,
    "humidity": 76,
    "sea_level": 1020,
    "grnd_level": 1019
  },
  "visibility": 10000,
  "wind": {"speed": 5.66, "deg": 220, "gust": 8.23},
  "clouds": {"all": 75},
  "dt": 1767452400,
  "sys": {
    "type": 2,
    "id": 2010624,
    "country": "US",
    "sunrise": 1767435600,
    "sunset": 1767472800
  },
  "timezone": -18000,
  "id": 4151440,
  "name": "Crescent Beach",
  "cod": 200
}
```

### source_id Format

Pattern: `{stream_id}-{source_type}`

| Stream | Source Type | source_id |
|--------|-------------|-----------|
| air-quality | mqtt | `air-quality-Mqtt` |
| outdoor-weather | http_poll | `outdoor-weather-Http` |
| nws-observations | http_poll | `nws-observations-Http` |

---

## Entity Schemas Format

### YAML Structure

From stream configuration:

```yaml
entity_schemas:
  - schema_name: airgradient
    description: AirGradient indoor air quality sensors (AG One, AG Pro)
    device_class: air_quality
    attributes:
      - name: pm25
        type: float
        unit: ug/m3
        description: Particulate Matter 2.5 micrometers
        nullable: false
        range: [0, 1000]
      - name: temperature
        type: float
        unit: celsius
        description: Ambient temperature
        nullable: true
        range: [-40, 85]
```

### Attribute Field Definitions

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Attribute identifier (snake_case) |
| `type` | string | Yes | Data type: float, int, string, bool, json, timestamp |
| `unit` | string | No | Measurement unit |
| `description` | string | Yes | Human-readable description |
| `nullable` | boolean | No | Whether null values allowed (default: true) |
| `range` | [number, number] | No | Valid value range for numeric types |

### Supported Data Types

| Type | Description | JSON Representation |
|------|-------------|---------------------|
| `float` | 64-bit floating point | number |
| `int` | 64-bit integer | number |
| `string` | UTF-8 text | string |
| `bool` | Boolean | true/false |
| `json` | Nested JSON object/array | object/array |
| `timestamp` | ISO 8601 datetime | string |

### Standard Units

| Category | Units |
|----------|-------|
| Temperature | celsius, fahrenheit, kelvin |
| Concentration | ug/m3, ppm, ppb |
| Percentage | percent |
| Pressure | hpa, pa, mbar |
| Speed | m/s, km/h, mph |
| Distance | meters, km, miles |
| Direction | degrees |
| Time | seconds, milliseconds, epoch_seconds |
| Indices | index, 1-5_scale, aqi_scale |

---

## Field Mappings Format

### Parser Configuration

From `sources[].parser.field_mappings`:

```yaml
parser:
  parser_type: json_path
  field_mappings:
    - path: main.temp
      metric_name: temperature
      unit: celsius
    - path: main.humidity
      metric_name: humidity
      unit: percent
    - path: wind.speed
      metric_name: wind_speed
      unit: m/s
```

### Field Mapping Structure

| Field | Type | Description |
|-------|------|-------------|
| `path` | string | JSON path in raw_payload (dot-separated) |
| `metric_name` | string | Target field name (matches entity_schema attribute) |
| `unit` | string | Unit of measurement |

### Parser Types

| Type | Description | Path Syntax |
|------|-------------|-------------|
| `json_path` | Extract values from nested JSON | Dot notation: `main.temp` |
| `flat_json` | Direct field access | Field name: `pm25` |
| `array_iterator` | Extract from arrays | Array access: `periods[0].temperature` |

---

## Data Flow Contracts

### Ingestion Path

```
Source API → RawDataPoint → ParquetStore → /data/raw/{stream_id}/...
```

### MCP Query Path

```
MCP Request → etcd (config) + Parquet (data) → MCP Response
```

### Configuration Sync Path

```
config/base/streams/{stream_id}/config.yaml
    → scripts/sync-config-to-etcd.sh
    → etcd /streams/{stream_id}/*
```

---

## Validation Contracts

### Stream ID Format

```regex
^[a-z][a-z0-9-]*$
```

- Starts with lowercase letter
- Contains lowercase letters, digits, hyphens
- Length: 1-64 characters

**Valid:** `air-quality`, `nws-observations`, `outdoor-weather`
**Invalid:** `Air-Quality`, `123-stream`, `stream_name`

### Partition Path Format

```regex
^year=\d{4}/month=\d{2}/day=\d{2}$
```

**Valid:** `year=2026/month=01/day=03`
**Invalid:** `2026/01/03`, `year=26/month=1/day=3`

### Timestamp Format

- Parquet: INT64 microseconds since Unix epoch
- JSON response: INT64 microseconds since Unix epoch
- ISO 8601 for file_modified: `2026-01-03T14:54:00Z`

---

## Schema Evolution

### Backward Compatibility

The Bronze schema is stable. Changes require:

1. New columns added as nullable
2. No column type changes
3. No column removals

### raw_payload Evolution

Since `raw_payload` is opaque JSON:
- Source API changes are transparent
- Field additions don't require schema changes
- Field removals detected by `validate_config`

### Entity Schema Versioning

Streams include `version` field for config changes:
- Semantic versioning: MAJOR.MINOR.PATCH
- MAJOR: Breaking schema changes
- MINOR: New attributes added
- PATCH: Description/metadata updates

---

## Query Patterns

### DuckDB raw_payload Access

```sql
-- Access nested field
SELECT
    timestamp,
    json_extract_string(raw_payload, '$.main.temp') as temperature
FROM '/data/raw/outdoor-weather/**/*.parquet';

-- Access nested object
SELECT
    timestamp,
    json_extract(raw_payload, '$.main') as main_block
FROM read_parquet('/data/raw/outdoor-weather/**/*.parquet');
```

### Parquet File Discovery

```rust
// Find latest partition
fn find_latest_partition(base_path: &Path) -> Option<PathBuf> {
    let mut years: Vec<_> = fs::read_dir(base_path).ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with("year="))
        .collect();
    years.sort_by_key(|e| e.file_name());
    let latest_year = years.last()?;

    // Continue with months, days...
    // Return path to data.parquet
}
```

---

## References

- [DP-004 Requirements](/workspaces/neural-data-platform/product/features/dp-004/specification/REQUIREMENTS.md) - Bronze schema design
- [Entity Schema Format](/workspaces/neural-data-platform/product/features/dp-002/specification/ENTITY_SCHEMA_FORMAT.md) - Entity schema specification
- [RawDataPoint Source](/workspaces/neural-data-platform/core/src/types/raw_data_point.rs) - Rust type definition

---

*This document is part of the SPARC Specification phase for DP-005.*
