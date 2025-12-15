# AIR-004: Stream Registry Schema Definition

## Overview

This document defines the stream registry schema for the multi-stream data platform. The schema focuses on **descriptive metadata** - "what the data is" - rather than prescriptive processing rules. Field-level metadata provides informational context that downstream consumers can use for validation, display, and analysis.

---

## Design Philosophy

**Registry Purpose**: Describe the nature and structure of data streams

**What the Registry Contains**:
- Stream identity and lifecycle configuration
- Field schemas with type, units, and descriptive constraints
- Source connection configurations
- Partitioning and retention policies

**What the Registry Does NOT Contain**:
- Processing logic (thresholds, alerts, transformations)
- Business rules (what constitutes "bad" data)
- Application-specific behavior
- Dynamic runtime state

**Metadata as Documentation**: Field metadata (units, ranges, precision) serves as:
- Validation hints for ingestion
- Display guidance for dashboards
- Context for data scientists
- Contract for schema evolution

---

## 1. Extended Field Schema

### Field Definition Structure

```yaml
fields:
  - name: string              # Field identifier (snake_case, alphanumeric + underscore)
    type: float|int|string|bool|json  # Data type
    unit: string              # OPTIONAL: Physical unit (informational)
    description: string       # OPTIONAL: Human-readable description
    range: [min, max]         # OPTIONAL: Expected range (informational, not enforced)
    display_precision: int    # OPTIONAL: Decimal places for display
    nullable: bool            # Whether field can be null (default: true)
    default: any              # OPTIONAL: Default value if not provided
```

### Field Type Specifications

| Type | Storage | Examples | Notes |
|------|---------|----------|-------|
| `float` | 64-bit floating point | 23.4, -15.2, 1.5e-3 | Temperature, humidity, PM2.5 |
| `int` | 64-bit signed integer | 400, -10, 0 | CO2 ppm, counts |
| `string` | UTF-8 text | "open", "kitchen", "clear" | States, identifiers |
| `bool` | Boolean | true, false | Binary flags |
| `json` | JSON object | `{"key": "value"}` | Flexible metadata |

### Field Naming Conventions

- **Format**: `snake_case` (lowercase with underscores)
- **Length**: 1-64 characters
- **Pattern**: `^[a-z][a-z0-9_]*$`
- **Reserved**: Cannot use SQL keywords as field names without quoting

**Examples**:
- Valid: `pm25`, `co2`, `temperature`, `event_type`, `wind_speed_kmh`
- Invalid: `PM2.5` (not snake_case), `select` (SQL keyword), `temp-c` (hyphen)

### Unit Conventions

Units are **informational metadata** for downstream consumers. Common patterns:

| Measurement | Unit String | Example |
|-------------|-------------|---------|
| Temperature | `celsius`, `fahrenheit`, `kelvin` | `celsius` |
| Particulate Matter | `µg/m³`, `ug/m3` | `µg/m³` |
| Concentration | `ppm` (parts per million) | `ppm` |
| Pressure | `hPa`, `mbar`, `mmHg` | `hPa` |
| Humidity | `percent`, `%` | `percent` |
| Speed | `m/s`, `km/h`, `mph` | `m/s` |
| Percentage/Index | `index`, `percent`, `ratio` | `index` |

### Range and Precision

**`range`**: Informational expected bounds
- Not enforced at ingestion (allows outliers to pass through)
- Used for dashboard Y-axis scaling
- Aids in anomaly detection context

**`display_precision`**: Decimal places for UI rendering
- Example: `display_precision: 1` → 23.456 renders as "23.5"
- Does not affect stored precision

### Complete Field Example

```yaml
fields:
  - name: pm25
    type: float
    unit: µg/m³
    description: Particulate Matter 2.5 micrometers
    range: [0, 500]
    display_precision: 1
    nullable: false

  - name: temperature
    type: float
    unit: celsius
    description: Ambient air temperature
    range: [-40, 60]
    display_precision: 1
    nullable: true

  - name: event_type
    type: string
    description: Type of home event (window_state, cooking_state, etc.)
    nullable: false

  - name: metadata
    type: json
    description: Flexible event metadata
    nullable: true
    default: {}
```

---

## 2. Stream Configuration Schema

### Complete Stream Structure

```yaml
# Stream metadata
stream_id: string              # Unique identifier (kebab-case)
description: string            # Human-readable description
version: string                # Schema version (semver, default: "1.0.0")
enabled: bool                  # Whether stream is active (default: true)

# Storage and retention
retention_days: int            # Days to retain data (0 = infinite)
compression_after_days: int    # Days before compression (TimescaleDB)
partitioning_strategy: string  # Partition key (default: "daily")

# Schema definition
fields:
  - name: ...
    type: ...
    # (see Field Schema above)

# Source configurations
sources:
  - type: mqtt|http_poll|webhook|file_watch
    # (see Source Configuration below)
```

### Partitioning Strategies

| Strategy | Description | Key Pattern | Best For |
|----------|-------------|-------------|----------|
| `daily` | One partition per day | `2025-12-15` | Default, most use cases |
| `hourly` | One partition per hour | `2025-12-15-14` | High-volume streams |
| `weekly` | One partition per week | `2025-W50` | Low-volume, long retention |
| `monthly` | One partition per month | `2025-12` | Archival data |

### Stream ID Conventions

- **Format**: `kebab-case` (lowercase with hyphens)
- **Length**: 3-64 characters
- **Pattern**: `^[a-z][a-z0-9-]*$`
- **Examples**: `air-quality`, `home-events`, `weather`, `power-usage`

---

## 3. Source Configuration Schemas

### Common Source Fields

All sources include:
```yaml
type: mqtt|http_poll|webhook|file_watch
enabled: bool                  # Whether source is active (default: true)
health_check_interval: string  # How often to check health (e.g., "30s", "5m")
```

### 3.1 MQTT Source

```yaml
sources:
  - type: mqtt
    broker_url: string         # mqtt://host:port or mqtts://
    topic: string              # MQTT topic pattern (supports wildcards)
    qos: int                   # Quality of Service (0, 1, 2)
    client_id: string          # OPTIONAL: MQTT client identifier
    username: string           # OPTIONAL: Authentication username
    password_env: string       # OPTIONAL: Env var for password
    keep_alive: string         # OPTIONAL: Keep-alive interval (default: "60s")
    clean_session: bool        # OPTIONAL: Clean session flag (default: true)
```

**Example**:
```yaml
sources:
  - type: mqtt
    broker_url: mqtt://mosquitto:1883
    topic: airgradient/readings/#
    qos: 1
    client_id: ingestion-air-quality
    keep_alive: 60s
```

### 3.2 HTTP Polling Source

```yaml
sources:
  - type: http_poll
    url: string                # Full HTTP/HTTPS URL
    interval: string           # Poll interval (e.g., "5m", "1h")
    method: string             # HTTP method (default: "GET")
    headers: map               # OPTIONAL: HTTP headers
    timeout: string            # Request timeout (default: "30s")
    auth:                      # OPTIONAL: Authentication config
      type: api_key|bearer|basic
      # (see Auth Configuration below)
```

**Example**:
```yaml
sources:
  - type: http_poll
    url: https://api.openweathermap.org/data/2.5/weather?lat=40.7128&lon=-74.0060
    interval: 5m
    timeout: 30s
    auth:
      type: api_key
      key_param: appid
      key_env: OPENWEATHERMAP_API_KEY
```

### 3.3 Webhook Source

```yaml
sources:
  - type: webhook
    path: string               # HTTP path (e.g., "/api/events")
    port: int                  # OPTIONAL: Port override (default: from main config)
    auth:                      # OPTIONAL: Authentication config
      type: bearer|api_key|none
      # (see Auth Configuration below)
    allowed_ips: list          # OPTIONAL: IP whitelist
```

**Example**:
```yaml
sources:
  - type: webhook
    path: /api/events
    auth:
      type: bearer
      token_env: WEBHOOK_BEARER_TOKEN
    allowed_ips:
      - 192.168.1.0/24
      - 10.0.0.5
```

### 3.4 File Watch Source

```yaml
sources:
  - type: file_watch
    directory: string          # Directory to watch
    pattern: string            # File glob pattern (e.g., "*.csv")
    recursive: bool            # Watch subdirectories (default: false)
    mode: append|replace       # How to handle file changes
    parser:                    # File parser configuration
      type: csv|json|parquet
      # (parser-specific config)
```

**Example**:
```yaml
sources:
  - type: file_watch
    directory: /data/imports
    pattern: "sensor_*.csv"
    recursive: false
    mode: append
    parser:
      type: csv
      delimiter: ","
      has_header: true
      timestamp_column: timestamp
      timestamp_format: "%Y-%m-%d %H:%M:%S"
```

### Authentication Configuration

#### API Key Authentication
```yaml
auth:
  type: api_key
  key_param: string            # Query param or header name
  key_env: string              # Environment variable containing key
  location: query|header       # Where to send key (default: query)
```

#### Bearer Token Authentication
```yaml
auth:
  type: bearer
  token_env: string            # Environment variable containing token
```

#### Basic Authentication
```yaml
auth:
  type: basic
  username: string
  password_env: string         # Environment variable containing password
```

---

## 4. etcd Key Structure

### Hierarchy

```
streams/
├── {stream-id}/
│   ├── config           # Stream-level configuration
│   ├── schema           # Field definitions
│   └── sources          # Source configurations
```

### Key Specifications

| Key Path | Content Type | Purpose | Watch Pattern |
|----------|-------------|---------|---------------|
| `streams/{stream-id}/config` | YAML | Stream metadata, retention, partitioning | `streams/{stream-id}/` |
| `streams/{stream-id}/schema` | YAML | Field definitions array | `streams/{stream-id}/` |
| `streams/{stream-id}/sources` | YAML | Source configurations array | `streams/{stream-id}/` |

### Watch Patterns for Hot-Reload

**Watch all streams**:
```rust
etcd_client.watch("streams/", WatchOptions::default().with_prefix())
```

**Watch specific stream**:
```rust
etcd_client.watch("streams/air-quality/", WatchOptions::default().with_prefix())
```

**Event Types**:
- `PUT`: Stream created or updated → Reload configuration
- `DELETE`: Stream removed → Stop ingestion, cleanup tasks

### GitOps Sync Pattern

```bash
# Sync from Git to etcd
etcdctl put streams/air-quality/config < config/streams/air-quality/config.yaml
etcdctl put streams/air-quality/schema < config/streams/air-quality/schema.yaml
etcdctl put streams/air-quality/sources < config/streams/air-quality/sources.yaml
```

**Directory Structure in Git**:
```
config/
└── streams/
    ├── air-quality/
    │   ├── config.yaml
    │   ├── schema.yaml
    │   └── sources.yaml
    ├── home-events/
    │   ├── config.yaml
    │   ├── schema.yaml
    │   └── sources.yaml
    └── weather/
        ├── config.yaml
        ├── schema.yaml
        └── sources.yaml
```

---

## 5. Example Stream Definitions

### 5.1 Air Quality Stream (Migrated from AIR-001/003)

#### config.yaml
```yaml
stream_id: air-quality
description: Indoor air quality measurements from AirGradient sensors
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: daily
```

#### schema.yaml
```yaml
fields:
  - name: pm25
    type: float
    unit: µg/m³
    description: Particulate Matter 2.5 micrometers
    range: [0, 500]
    display_precision: 1
    nullable: false

  - name: pm10
    type: float
    unit: µg/m³
    description: Particulate Matter 10 micrometers
    range: [0, 500]
    display_precision: 1
    nullable: true

  - name: pm01
    type: float
    unit: µg/m³
    description: Particulate Matter 1 micrometer
    range: [0, 500]
    display_precision: 1
    nullable: true

  - name: co2
    type: int
    unit: ppm
    description: Carbon dioxide concentration
    range: [400, 5000]
    nullable: false

  - name: voc
    type: int
    unit: index
    description: Volatile organic compounds index
    range: [0, 500]
    nullable: true

  - name: temperature
    type: float
    unit: celsius
    description: Ambient air temperature
    range: [-10, 50]
    display_precision: 1
    nullable: true

  - name: humidity
    type: float
    unit: percent
    description: Relative humidity
    range: [0, 100]
    display_precision: 1
    nullable: true

  - name: nox_index
    type: int
    unit: index
    description: Nitrogen oxides index
    range: [0, 500]
    nullable: true

  - name: sensor_id
    type: string
    description: Unique identifier of the reporting sensor
    nullable: false
```

#### sources.yaml
```yaml
sources:
  - type: mqtt
    broker_url: mqtt://mosquitto:1883
    topic: airgradient/readings/#
    qos: 1
    client_id: ingestion-air-quality
    keep_alive: 60s
```

---

### 5.2 Home Events Stream (New)

#### config.yaml
```yaml
stream_id: home-events
description: Discrete home activity events (windows, cooking, occupancy)
version: "1.0.0"
enabled: true
retention_days: 730
compression_after_days: 30
partitioning_strategy: daily
```

#### schema.yaml
```yaml
fields:
  - name: event_type
    type: string
    description: Type of event (window_state, cooking_state, occupancy, etc.)
    nullable: false

  - name: target
    type: string
    description: Target of event (e.g., "front_window", "kitchen", "bedroom")
    nullable: false

  - name: state
    type: string
    description: State value (e.g., "open", "closed", "active", "idle")
    nullable: true

  - name: source
    type: string
    description: Source system that generated the event
    nullable: true
    default: "manual"

  - name: metadata
    type: json
    description: Flexible event metadata (tags, attributes, context)
    nullable: true
    default: {}
```

#### sources.yaml
```yaml
sources:
  - type: mqtt
    broker_url: mqtt://mosquitto:1883
    topic: home/events/#
    qos: 1
    client_id: ingestion-home-events

  - type: webhook
    path: /api/events
    auth:
      type: bearer
      token_env: WEBHOOK_BEARER_TOKEN
    allowed_ips:
      - 192.168.1.0/24
```

---

### 5.3 Weather Stream (New)

#### config.yaml
```yaml
stream_id: weather
description: Outdoor weather conditions from OpenWeatherMap
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 30
partitioning_strategy: daily
```

#### schema.yaml
```yaml
fields:
  - name: temperature
    type: float
    unit: celsius
    description: Outdoor air temperature
    range: [-40, 60]
    display_precision: 1
    nullable: false

  - name: feels_like
    type: float
    unit: celsius
    description: Perceived temperature
    range: [-50, 70]
    display_precision: 1
    nullable: true

  - name: humidity
    type: float
    unit: percent
    description: Relative humidity
    range: [0, 100]
    display_precision: 0
    nullable: false

  - name: pressure
    type: float
    unit: hPa
    description: Atmospheric pressure
    range: [900, 1100]
    display_precision: 0
    nullable: true

  - name: wind_speed
    type: float
    unit: m/s
    description: Wind speed
    range: [0, 50]
    display_precision: 1
    nullable: true

  - name: wind_direction
    type: int
    unit: degrees
    description: Wind direction (0-360 degrees from north)
    range: [0, 360]
    nullable: true

  - name: cloud_cover
    type: int
    unit: percent
    description: Cloud coverage percentage
    range: [0, 100]
    nullable: true

  - name: visibility
    type: int
    unit: meters
    description: Visibility distance
    range: [0, 10000]
    nullable: true

  - name: conditions
    type: string
    description: Weather condition description (e.g., "clear sky", "light rain")
    nullable: true

  - name: location
    type: string
    description: Location identifier or name
    nullable: false
```

#### sources.yaml
```yaml
sources:
  - type: http_poll
    url: https://api.openweathermap.org/data/2.5/weather?lat=40.7128&lon=-74.0060&units=metric
    interval: 5m
    timeout: 30s
    auth:
      type: api_key
      key_param: appid
      key_env: OPENWEATHERMAP_API_KEY
```

---

## 6. Schema Validation Rules

### Required Fields

**Every stream MUST have**:
- `stream_id`: Non-empty string matching `^[a-z][a-z0-9-]*$`
- `description`: Non-empty human-readable string
- At least one field in `schema`
- At least one source in `sources`

**Every field MUST have**:
- `name`: Non-empty string matching `^[a-z][a-z0-9_]*$`
- `type`: One of `float`, `int`, `string`, `bool`, `json`

### Type Constraints

| Type | Additional Constraints |
|------|----------------------|
| `float` | Range values must be numeric |
| `int` | Range values must be integers; `display_precision` invalid |
| `string` | Range and `display_precision` invalid |
| `bool` | Range and `display_precision` invalid |
| `json` | Range and `display_precision` invalid |

### Naming Conventions

**Stream IDs**:
- Pattern: `^[a-z][a-z0-9-]*$`
- Length: 3-64 characters
- Examples: `air-quality`, `home-events`

**Field Names**:
- Pattern: `^[a-z][a-z0-9_]*$`
- Length: 1-64 characters
- No SQL reserved keywords without quoting
- Examples: `pm25`, `event_type`, `wind_speed`

### Validation Logic

```rust
// Pseudo-code for validation
fn validate_stream_config(config: StreamConfig) -> Result<()> {
    // Stream ID
    if !config.stream_id.matches(r"^[a-z][a-z0-9-]*$") {
        return Err("Invalid stream_id format");
    }

    // Fields
    if config.fields.is_empty() {
        return Err("Stream must have at least one field");
    }

    for field in config.fields {
        // Field name
        if !field.name.matches(r"^[a-z][a-z0-9_]*$") {
            return Err(format!("Invalid field name: {}", field.name));
        }

        // Type-specific constraints
        match field.type {
            FieldType::String | FieldType::Bool | FieldType::Json => {
                if field.range.is_some() {
                    return Err(format!("Field {} cannot have range", field.name));
                }
            }
            FieldType::Int => {
                if field.display_precision.is_some() {
                    return Err(format!("Int field {} cannot have display_precision", field.name));
                }
            }
            _ => {}
        }

        // Range validation
        if let Some(range) = field.range {
            if range.len() != 2 || range[0] >= range[1] {
                return Err(format!("Invalid range for {}", field.name));
            }
        }
    }

    // Sources
    if config.sources.is_empty() {
        return Err("Stream must have at least one source");
    }

    Ok(())
}
```

### Schema Evolution

**Adding Fields**:
- New fields must be `nullable: true` or have `default` value
- Existing data will have `NULL` for new fields

**Removing Fields**:
- Mark as deprecated in description
- Remove from schema after data migration/cleanup
- Bronze layer retains historical schema

**Changing Field Types**:
- NOT supported for existing fields
- Add new field with new type, migrate data, deprecate old field

**Version Tracking**:
```yaml
version: "1.1.0"  # Bump on schema changes
```

---

## 7. Common Envelope Fields

All streams automatically include these fields (do not define in schema):

| Field | Type | Description | Source |
|-------|------|-------------|--------|
| `timestamp` | timestamptz | Event timestamp | Source or ingestion time |
| `stream_id` | string | Stream identifier | Registry |
| `source_type` | string | Source type (mqtt, http_poll, etc.) | Source config |
| `ingestion_time` | timestamptz | Time data entered platform | Ingestion layer |

These fields are **automatically added** by the ingestion layer and should not be included in the stream schema definition.

---

## 8. Rust Type Mappings

### Field Type → Rust Type

| Schema Type | Rust Type | Serde | SQL Type |
|-------------|-----------|-------|----------|
| `float` | `f64` | `#[serde(rename = "...")]` | `DOUBLE PRECISION` |
| `int` | `i64` | `#[serde(rename = "...")]` | `BIGINT` |
| `string` | `String` | `#[serde(rename = "...")]` | `TEXT` |
| `bool` | `bool` | `#[serde(rename = "...")]` | `BOOLEAN` |
| `json` | `serde_json::Value` | `#[serde(rename = "...")]` | `JSONB` |

### Generated Rust Struct Example

From `air-quality` schema:

```rust
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirQualityRecord {
    // Envelope fields (auto-added)
    pub timestamp: DateTime<Utc>,
    pub stream_id: String,
    pub source_type: String,
    pub ingestion_time: DateTime<Utc>,

    // Schema-defined fields
    pub pm25: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pm10: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pm01: Option<f64>,
    pub co2: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voc: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub humidity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nox_index: Option<i64>,
    pub sensor_id: String,
}
```

---

## 9. TimescaleDB Schema Generation

### Auto-Generated DDL

From stream schema:

```sql
-- Generated from streams/air-quality/schema
CREATE TABLE air_quality (
    -- Envelope columns
    timestamp TIMESTAMPTZ NOT NULL,
    stream_id TEXT NOT NULL,
    source_type TEXT NOT NULL,
    ingestion_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Schema-defined columns
    pm25 DOUBLE PRECISION NOT NULL,
    pm10 DOUBLE PRECISION,
    pm01 DOUBLE PRECISION,
    co2 BIGINT NOT NULL,
    voc BIGINT,
    temperature DOUBLE PRECISION,
    humidity DOUBLE PRECISION,
    nox_index BIGINT,
    sensor_id TEXT NOT NULL,

    PRIMARY KEY (timestamp, sensor_id)
);

-- Convert to hypertable
SELECT create_hypertable('air_quality', 'timestamp');

-- Compression policy (from config: compression_after_days: 7)
ALTER TABLE air_quality SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sensor_id'
);

SELECT add_compression_policy('air_quality', INTERVAL '7 days');

-- Retention policy (from config: retention_days: 365)
SELECT add_retention_policy('air_quality', INTERVAL '365 days');
```

---

## 10. Parquet Schema Generation

### Auto-Generated Parquet Schema

```rust
use parquet::schema::types::{Type, TypePtr};
use parquet::basic::{Repetition, Type as PhysicalType};

fn generate_parquet_schema(stream_schema: &StreamSchema) -> TypePtr {
    let mut fields = vec![
        // Envelope fields
        Type::primitive_type_builder("timestamp", PhysicalType::INT64)
            .with_converted_type(ConvertedType::TIMESTAMP_MILLIS)
            .with_repetition(Repetition::REQUIRED)
            .build()
            .unwrap(),
        Type::primitive_type_builder("stream_id", PhysicalType::BYTE_ARRAY)
            .with_converted_type(ConvertedType::UTF8)
            .with_repetition(Repetition::REQUIRED)
            .build()
            .unwrap(),
        // ... other envelope fields
    ];

    // Schema-defined fields
    for field in &stream_schema.fields {
        let repetition = if field.nullable {
            Repetition::OPTIONAL
        } else {
            Repetition::REQUIRED
        };

        let field_type = match field.type {
            FieldType::Float => {
                Type::primitive_type_builder(&field.name, PhysicalType::DOUBLE)
                    .with_repetition(repetition)
                    .build()
                    .unwrap()
            }
            FieldType::Int => {
                Type::primitive_type_builder(&field.name, PhysicalType::INT64)
                    .with_repetition(repetition)
                    .build()
                    .unwrap()
            }
            FieldType::String => {
                Type::primitive_type_builder(&field.name, PhysicalType::BYTE_ARRAY)
                    .with_converted_type(ConvertedType::UTF8)
                    .with_repetition(repetition)
                    .build()
                    .unwrap()
            }
            FieldType::Bool => {
                Type::primitive_type_builder(&field.name, PhysicalType::BOOLEAN)
                    .with_repetition(repetition)
                    .build()
                    .unwrap()
            }
            FieldType::Json => {
                Type::primitive_type_builder(&field.name, PhysicalType::BYTE_ARRAY)
                    .with_converted_type(ConvertedType::UTF8)
                    .with_repetition(repetition)
                    .build()
                    .unwrap()
            }
        };

        fields.push(field_type);
    }

    Arc::new(
        Type::group_type_builder("schema")
            .with_fields(&mut fields)
            .build()
            .unwrap()
    )
}
```

---

## 11. Implementation Checklist

### Phase 1: Registry Schema Definition
- [x] Define field schema structure
- [x] Define stream configuration schema
- [x] Define source configuration schemas
- [x] Document etcd key hierarchy
- [x] Create example stream definitions
- [ ] Implement schema validation in Rust
- [ ] Create etcd helper library for CRUD operations

### Phase 2: Code Generation
- [ ] Implement Rust struct generator from schema
- [ ] Implement TimescaleDB DDL generator
- [ ] Implement Parquet schema generator
- [ ] Create CLI tool for schema operations

### Phase 3: Migration
- [ ] Migrate air-quality config to new schema
- [ ] Test hot-reload with etcd watch
- [ ] Validate backward compatibility

### Phase 4: Documentation
- [ ] API documentation for schema access
- [ ] Schema evolution guidelines
- [ ] Troubleshooting guide

---

## 12. Open Questions and Future Considerations

### Schema Evolution
**Question**: How to handle non-backward-compatible changes?
**Options**:
1. Versioned streams (`air-quality-v2`)
2. Schema migration scripts with dual-write period
3. Breaking change policy (major version bump, deprecation notice)

**Recommendation**: Use versioned streams for breaking changes, schema migrations for additive changes.

### Field Constraints
**Question**: Should we support enum types for strings?
**Example**:
```yaml
- name: event_type
  type: string
  enum: [window_state, cooking_state, occupancy]
```

**Consideration**: Adds validation complexity. Current approach: document valid values in `description`, validate in application layer.

### Units Standardization
**Question**: Enforce unit validation (e.g., reject `temp: "C"` in favor of `temp: "celsius"`)?
**Recommendation**: Document conventions, provide linter, but don't enforce (allows flexibility).

### Derived Fields
**Question**: Support computed fields in schema (e.g., `pm_total = pm25 + pm10`)?
**Recommendation**: No. Derived fields are analytics concern, not data schema concern. Compute in continuous aggregates or queries.

---

## Related Documents

- `/workspaces/neural-data-platform/product/features/air-004/architecture/PLATFORM_ARCHITECTURE.md` - Platform architecture overview
- `/workspaces/neural-data-platform/apps/air-quality-app/src/config/mod.rs` - Existing config implementation (AIR-003)
- `/workspaces/neural-data-platform/core/src/storage/parquet.rs` - Parquet storage implementation

---

**Status**: Draft Complete
**Last Updated**: 2025-12-15
**Next Steps**: Implement schema validation in Rust, create code generators
