# AIR-009: Source Identity and Context Configuration - System Design

**Version**: 1.0.0
**Date**: 2025-12-31
**Status**: Draft

---

## Executive Summary

This document describes the system architecture for implementing `ndp_id` (stable source identifier) and `context` (mutable attributes) across the Neural Data Platform. The design ensures that identity remains constant while context travels with every record, enabling point-in-time accuracy and device mobility.

---

## Data Flow Architecture

### End-to-End Flow Diagram

```
+-------------------+     +------------------+     +---------------------+
|  YAML Config      |     |      etcd        |     |  Application Load   |
|  (Git-managed)    | --> | (Runtime Store)  | --> |  (StreamRegistry)   |
+-------------------+     +------------------+     +---------------------+
                                                            |
        sources[]:                                          v
          ndp_id: "airgradient-office-001"         +-------------------+
          context:                                 |  SourceConfig     |
            location:                              |  - ndp_id         |
              coordinates: [lat, lon]              |  - context (raw)  |
              type: indoor                         +-------------------+
              path: home/upstairs/office                    |
            device_type: airgradient                        v
                                                   +-------------------+
                                                   |  Context Flattener|
                                                   |  (Ingestion Layer)|
                                                   +-------------------+
                                                            |
        Flattened Context:                                  v
          ndp_id: "airgradient-office-001"         +-------------------+
          location.coordinates: [lat, lon]         |  Bronze Layer     |
          location.type: "indoor"                  |  (Parquet)        |
          location.path: "home/upstairs/office"    +-------------------+
          device_type: "airgradient"                        |
                                                            v
                                                   +-------------------+
                                                   |  Silver Layer     |
                                                   |  (TimescaleDB)    |
                                                   +-------------------+
```

### Component Interaction Sequence

```
sequenceDiagram
    participant YAML as Config YAML
    participant Sync as ConfigSyncService
    participant etcd as etcd
    participant Registry as StreamRegistry
    participant Source as MqttSource/HttpPoller
    participant Flatten as ContextFlattener
    participant Bronze as ParquetStore
    participant Silver as TimescaleDB

    YAML->>Sync: Load on startup
    Sync->>Sync: Parse ndp_id + context
    Sync->>etcd: Store structured config
    etcd->>Registry: Watch/Load config
    Registry->>Source: Create with ndp_id + context

    loop Data Ingestion
        Source->>Source: Receive raw data
        Source->>Flatten: Attach context to record
        Flatten->>Flatten: Flatten nested keys
        Flatten->>Bronze: Write with ndp_id + flat context
        Bronze->>Silver: ETL with context preservation
    end
```

---

## Component Architecture

### 1. Configuration Layer

#### SourceConfig Extension

The `SourceConfig` struct in `core/src/types/stream_config.rs` will be extended:

```rust
pub struct SourceConfig {
    // Existing fields
    pub source_type: SourceType,
    pub enabled: bool,

    // NEW: Stable identifier (required)
    pub ndp_id: Option<String>,

    // NEW: Context (optional, written with every record)
    pub context: Option<serde_json::Value>,

    // Existing: Source-specific parameters
    pub params: HashMap<String, serde_json::Value>,
}
```

#### Context Structure (YAML)

```yaml
context:
  location:                    # Standardized location schema
    coordinates: [lat, lon]    # Tuple - NOT flattened
    type: indoor | outdoor     # Environment type
    path: house/zone/room      # Hierarchical path (any depth)
  device_type: airgradient     # Domain-specific (dynamic)
  model: ONE-V9                # Device model
  tags: [primary, calibrated]  # Filtering tags
```

### 2. Configuration Sync Layer

The `ConfigSyncService` handles YAML to etcd synchronization:

```
config/base/streams/{stream-id}/config.yaml
         |
         v
+-------------------+
| YAML Parser       |
| - Parse ndp_id    |
| - Parse context   |
| - Validate schema |
+-------------------+
         |
         v
+-------------------+
| etcd Writer       |
| - Nested JSON     |
| - Watch updates   |
+-------------------+
```

#### etcd Key Structure

```
/streams/{stream_id}/config                     # Full StreamConfig JSON
/streams/{stream_id}/sources/0/ndp_id           # "airgradient-office-001"
/streams/{stream_id}/sources/0/context          # JSON object
/streams/{stream_id}/sources/0/context/location/coordinates  # [29.958, -81.308]
/streams/{stream_id}/sources/0/context/location/type         # "indoor"
/streams/{stream_id}/sources/0/context/location/path         # "home/upstairs/office"
/streams/{stream_id}/sources/0/context/device_type           # "airgradient"
```

### 3. Ingestion Pipeline

#### Context Flattener Module

New module: `core/src/ingestion/context_flattener.rs`

```
+---------------------+
|   Raw Context       |
|   (Nested JSON)     |
+---------------------+
          |
          v
+---------------------+
|  Flattening Rules   |
|  - Dot notation     |
|  - Preserve tuples  |
|  - Dynamic keys     |
+---------------------+
          |
          v
+---------------------+
|  Flat Context       |
|  (HashMap<K,V>)     |
+---------------------+
```

#### Flattening Rules

| Input Path | Output Key | Value Type | Notes |
|------------|-----------|------------|-------|
| `location.coordinates` | `location.coordinates` | `[f64, f64]` | Preserved as tuple |
| `location.type` | `location.type` | `String` | Flattened |
| `location.path` | `location.path` | `String` | Flattened |
| `device_type` | `device_type` | `String` | Top-level passthrough |
| `tags` | `tags` | `Vec<String>` | Preserved as array |

#### Integration Point

The flattener integrates at the source handler level:

```
MqttHandler.parse_message()
     |
     v
+-------------------+
| Create Point      |
| - timestamp       |
| - location_id     |
| - value           |
| - tags (metrics)  |
+-------------------+
     |
     v (NEW)
+-------------------+
| Attach Context    |
| - ndp_id          |
| - flat context    |
+-------------------+
     |
     v
+-------------------+
| Send to Channel   |
+-------------------+
```

### 4. Storage Layers

#### Bronze Layer (Parquet)

Extended Parquet schema:

```
+------------------+----------+------------------------+
| Column           | Type     | Description            |
+------------------+----------+------------------------+
| timestamp        | INT64    | Unix epoch millis      |
| location_id      | STRING   | Original sensor ID     |
| ndp_id           | STRING   | NEW: Stable identifier |
| value            | DOUBLE   | Metric value           |
| metric_name      | STRING   | Metric type            |
| location.coords  | LIST     | NEW: [lat, lon] tuple  |
| location.type    | STRING   | NEW: indoor/outdoor    |
| location.path    | STRING   | NEW: Hierarchical path |
| context.*        | STRING   | NEW: Dynamic context   |
+------------------+----------+------------------------+
```

#### Silver Layer (TimescaleDB)

See ADR-003 for schema decision. Recommended schema:

```sql
CREATE TABLE readings (
    time        TIMESTAMPTZ NOT NULL,
    ndp_id      TEXT NOT NULL,              -- Stable identifier
    stream_id   TEXT NOT NULL,              -- Stream this belongs to
    location_id TEXT,                       -- Original sensor ID
    context     JSONB,                      -- Flattened context
    -- Metric columns...
    temperature DOUBLE PRECISION,
    humidity    DOUBLE PRECISION,
    pm25        DOUBLE PRECISION,
    -- etc.
);

-- Hypertable for time-series optimization
SELECT create_hypertable('readings', 'time');

-- Index for source queries
CREATE INDEX idx_readings_ndp_id ON readings (ndp_id, time DESC);

-- GIN index for context queries
CREATE INDEX idx_readings_context ON readings USING GIN (context);
```

---

## Context Flattening Logic

### Algorithm

```
FUNCTION flatten_context(context: JSON, prefix: String) -> HashMap<String, Value>:
    result = HashMap::new()

    FOR (key, value) IN context:
        full_key = if prefix.is_empty() { key } else { prefix + "." + key }

        IF key == "coordinates":
            # SPECIAL CASE: Preserve as tuple
            result.insert(full_key, value)
        ELSE IF value.is_object():
            # RECURSIVE: Flatten nested objects
            nested = flatten_context(value, full_key)
            result.extend(nested)
        ELSE IF value.is_array() AND NOT value[0].is_object():
            # PRESERVE: Simple arrays (strings, numbers)
            result.insert(full_key, value)
        ELSE:
            # PASSTHROUGH: Scalar values
            result.insert(full_key, value)

    RETURN result
```

### Example Transformation

**Input (Nested):**
```yaml
context:
  location:
    coordinates: [29.958, -81.308]
    type: indoor
    path: home/upstairs/office
  device_type: airgradient
  model: ONE-V9
  tags: [primary, calibrated]
```

**Output (Flattened):**
```json
{
  "location.coordinates": [29.958, -81.308],
  "location.type": "indoor",
  "location.path": "home/upstairs/office",
  "device_type": "airgradient",
  "model": "ONE-V9",
  "tags": ["primary", "calibrated"]
}
```

---

## Record Structure

### Post-Ingestion Record

```json
{
  "timestamp": "2025-01-15T10:30:00Z",
  "stream_id": "air-quality",
  "ndp_id": "airgradient-office-001",
  "location_id": "AG:12345",

  "location.coordinates": [29.958, -81.308],
  "location.type": "indoor",
  "location.path": "home/upstairs/office",
  "device_type": "airgradient",
  "model": "ONE-V9",

  "pm25": 12.5,
  "temperature": 22.3,
  "humidity": 45.0,
  "co2": 650
}
```

### Key Properties

| Property | Source | Mutability | Purpose |
|----------|--------|------------|---------|
| `ndp_id` | Config | Immutable | Stable identity across all records |
| `location_id` | Payload | Per-message | Original device/sensor ID |
| `context.*` | Config | Mutable (per write) | Point-in-time attributes |

---

## Deployment Architecture

### Service Dependencies

```
+-------------------+     +-------------------+     +-------------------+
|    YAML Files     |     |       etcd        |     |    Application    |
|   (Git Volume)    |<--->|  (Config Store)   |<--->|   (Rust Binary)   |
+-------------------+     +-------------------+     +-------------------+
                                                            |
                                   +------------------------+
                                   |
                          +--------v--------+     +-------------------+
                          |  Bronze Layer   |     |   Silver Layer    |
                          |   (Parquet)     |---->|  (TimescaleDB)    |
                          +-----------------+     +-------------------+
```

### Resource Impact

| Component | Memory Delta | CPU Delta | Notes |
|-----------|-------------|-----------|-------|
| ConfigSyncService | +10MB | Negligible | Context parsing |
| SourceConfig | +5MB | Negligible | In-memory context |
| Context Flattener | +20MB | +2% | Per-record processing |
| Parquet Schema | +15% file size | Negligible | Additional columns |
| TimescaleDB | +20% storage | +5% query | JSONB + indexes |

---

## Security Considerations

### Context Validation

1. **Size Limits**: Context JSON must not exceed 64KB
2. **Key Validation**: Only alphanumeric + underscores + dots allowed
3. **No Secrets**: Context must not contain sensitive data (API keys, passwords)
4. **Sanitization**: HTML/script injection prevention for string values

### Access Control

- `ndp_id` is read-only after first write (enforced by convention)
- Context modifications require config file change + redeploy
- Historical records are immutable (append-only Bronze layer)

---

## Migration Strategy

### Phase 1: Schema Extension (Non-Breaking)

1. Add `ndp_id` and `context` fields to `SourceConfig` as `Option<T>`
2. Update ConfigSyncService to parse new fields
3. Existing configs continue to work (fields are optional)

### Phase 2: Ingestion Updates

1. Implement `ContextFlattener` module
2. Integrate with `MqttHandler` and `HttpPollingSource`
3. Records without context continue to work (empty context)

### Phase 3: Storage Layer Updates

1. Extend Parquet schema with new columns
2. Add `ndp_id` and `context` columns to TimescaleDB
3. Create indexes for efficient querying

### Phase 4: Config Population

1. Add `ndp_id` and `context` to all 6 active stream configs
2. Sync to etcd via ConfigSyncService
3. Validate via queries

---

## Related ADRs

| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | ndp_id Design | Proposed |
| ADR-002 | Context Flattening Approach | Proposed |
| ADR-003 | Silver Layer Schema Choice | Proposed |

---

## References

- [SCOPE.md](../SCOPE.md) - Feature scope and requirements
- [PLATFORM_ARCHITECTURE_OVERVIEW.md](/workspaces/neural-data-platform/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md) - Platform architecture
- [AIR-005 ADR Summary](/workspaces/neural-data-platform/docs/architecture/AIR-005_ADR_SUMMARY.md) - Previous architecture decisions
- Sample configs: `config/samples/mqtt_stream.yaml`, `config/samples/http_stream.yaml`

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-31 | ndp-architect | Initial system design |
