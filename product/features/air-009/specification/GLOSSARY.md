# AIR-009: Source Identity and Context Configuration - Glossary

## Document Information

| Field | Value |
|-------|-------|
| Feature | AIR-009 |
| Version | 1.2.0 |
| Status | Draft |
| Last Updated | 2025-12-31 |
| Author | NDP Specification Agent |
| Amendment | ADR-002-AMENDMENT-002 (Simple Blob) |

---

## Overview

This glossary defines key terms used throughout the AIR-009 feature documentation. Terms are organized alphabetically within logical categories.

---

## Core Concepts

### ndp_id

**Definition:** A stable, NDP-assigned identifier for a data source that never changes, even if the source is reconfigured, moved, or its metadata is updated.

**Characteristics:**
- Immutable once assigned
- Format: lowercase alphanumeric with hyphens (e.g., `airgradient-office-001`)
- Unique within the NDP deployment
- Resides at source level in configuration, NOT inside context

**Purpose:** Enables linking all data from a source across time, providing reliable data lineage regardless of configuration changes.

**Example:**
```yaml
sources:
  - type: mqtt
    ndp_id: airgradient-office-001  # Stable forever
```

---

### Context

**Definition:** A JSON blob containing mutable attributes that are stored with every record from a data source. No flattening, no transformation - just a simple JSON blob.

**Characteristics:**
- Mutable (can be updated in configuration)
- Written at ingestion time (point-in-time values)
- Stored as-is: JSON string in Bronze, JSONB in Silver
- All fields are optional
- Dynamic keys allowed (no hardcoded schema)
- Queried via JSONB operators (no promoted columns)

**Purpose:** Provides queryable metadata about where, how, and when data was collected, while preserving historical accuracy and original structure.

**Example:**
```yaml
context:
  location:
    coordinates: [29.958, -81.308]
    type: indoor
    path: home/upstairs/office
  device_type: airgradient
  model: ONE-V9
```

---

### Context Blob

**Definition:** The simple approach of storing context as a single JSON blob with no transformation.

**Storage:**
| Layer | Column | Type |
|-------|--------|------|
| Bronze (Parquet) | `context` | STRING (JSON) |
| Silver (TimescaleDB) | `context` | JSONB |

**Query Examples:**
```sql
-- Query by device type
SELECT * FROM sensor_readings
WHERE context->>'device_type' = 'airgradient';

-- Query nested location
SELECT * FROM sensor_readings
WHERE context->'location'->>'type' = 'indoor';

-- Query coordinates (array access)
SELECT * FROM sensor_readings
WHERE (context->'location'->'coordinates'->>0)::float > 29.0;
```

**Rationale:** Maximum simplicity - one column, one format. GIN index on JSONB provides good query performance for most use cases. See ADR-002-AMENDMENT-002.

---

### Point-in-Time Accuracy

**Definition:** The property that historical records retain the context values that were active at the time the data was written.

**Example Scenario:**
1. Day 1: Sensor in "office", records with `location.path: home/office`
2. Day 30: Sensor moved to "bedroom", config updated
3. Day 30+: New records have `location.path: home/bedroom`
4. Query on Day 60: Day 1-29 records still show "office"

**Importance:** Enables accurate historical analysis even when physical or logical context changes over time.

---

## Data Architecture

### Bronze Layer

**Definition:** The first tier of the NDP data lake, storing raw, immutable data in Parquet format as it arrives from sources.

**Characteristics:**
- Parquet file format
- Append-only writes
- Partitioned by date (typically daily)
- Contains all original fields plus `ndp_id` and `context` JSON blob
- No transformations or aggregations

**Role in AIR-009:** Bronze layer receives `ndp_id` (STRING) and `context` (STRING containing JSON blob) directly from the ingestion pipeline. No flattening or promoted fields.

---

### Silver Layer

**Definition:** The second tier of the NDP data lake, containing cleansed, validated, and structured data in TimescaleDB.

**Characteristics:**
- TimescaleDB hypertables
- Time-series optimized with continuous aggregates
- Schema-enforced structure
- Contains `ndp_id` (TEXT) and `context` (JSONB) columns
- Indexed for efficient querying (B-tree on ndp_id, GIN on context)

**Role in AIR-009:** Silver layer stores:
- `ndp_id` as a dedicated indexed TEXT column
- `context` as JSONB with GIN index for flexible queries via JSONB operators

---

### Gold Layer

**Definition:** The third tier of the NDP data lake, containing aggregated, feature-engineered data ready for analytics and ML.

**Note:** Gold layer is not directly affected by AIR-009, though it inherits source identity from Silver layer.

---

### Data Dictionary

**Definition:** A catalog of data structures, field definitions, and schemas used throughout the NDP platform.

**Role in AIR-009:** The data dictionary is updated to include `ndp_id` and context column definitions in the Silver layer schema.

---

## Infrastructure

### etcd

**Definition:** A distributed key-value store used by NDP for service configuration and discovery.

**Role in AIR-009:** Stores synchronized stream configurations, including `ndp_id` and context fields, with hierarchical key structure:
```
/streams/{stream_id}/sources/{index}/ndp_id
/streams/{stream_id}/sources/{index}/context/...
```

---

### ConfigSyncService

**Definition:** The NDP component responsible for synchronizing YAML stream configurations to etcd.

**Role in AIR-009:** Must correctly handle nested context structures and sync them to etcd with proper key hierarchy.

---

### TimescaleDB

**Definition:** A PostgreSQL extension optimized for time-series data, used for the NDP Silver layer.

**Key Features:**
- Hypertables for automatic time partitioning
- Continuous aggregates for pre-computed rollups
- JSONB support for flexible schemas
- Full SQL compatibility

**Role in AIR-009:** Hosts the `ndp_id` column and `context` JSONB column for queryable source identification.

---

### Parquet

**Definition:** A columnar file format optimized for analytical workloads, used for NDP Bronze layer storage.

**Characteristics:**
- Efficient compression
- Column pruning for fast queries
- Schema evolution support
- Compatible with Arrow, Spark, DuckDB

**Role in AIR-009:** Stores `ndp_id` as a string column and `context` as a JSON string column. No promoted fields, no flattening.

---

## Source Types

### MQTT Source

**Definition:** A data source that subscribes to MQTT topics and ingests messages in real-time.

**AIR-009 Affected Streams:**
- `air-quality` (AirGradient sensors)

**Configuration Example:**
```yaml
sources:
  - type: mqtt
    ndp_id: airgradient-office-001
    context:
      location:
        type: indoor
        path: home/upstairs/office
    broker_url: mosquitto
    topic_pattern: "airgradient/readings/+"
```

---

### HTTP Poll Source

**Definition:** A data source that periodically fetches data from HTTP/REST APIs.

**AIR-009 Affected Streams:**
- `nws-observations`
- `nws-forecast-hourly`
- `nws-gridpoints-forecast`
- `outdoor-air-quality`
- `outdoor-weather`

**Configuration Example:**
```yaml
sources:
  - type: http_poll
    ndp_id: nws-ksgj-observations
    context:
      location:
        coordinates: [29.959, -81.339]
        type: outdoor
      station_id: KSGJ
    poll_interval_secs: 300
```

---

## Location Schema

### Coordinates

**Definition:** GPS location expressed as a `[latitude, longitude]` tuple.

**Format:** Array of two floating-point numbers
- First element: Latitude (-90 to 90)
- Second element: Longitude (-180 to 180)

**Storage:** Coordinates remain as a tuple/array within the context JSON blob. Query via JSONB array access: `context->'location'->'coordinates'->>0`.

**Example:** `[29.95838, -81.30878]` (St. Augustine, FL)

---

### Location Type

**Definition:** The environment classification of a data source's location.

**Values:**
| Value | Description |
|-------|-------------|
| `indoor` | Inside a building or enclosed space |
| `outdoor` | Outside, exposed to weather |

**Usage:** Enables filtering and grouping data by environment type for analysis.

---

### Location Path

**Definition:** A hierarchical path describing the logical location of a data source.

**Format:** Forward-slash separated segments of arbitrary depth.

**Examples:**
| Path | Description |
|------|-------------|
| `home/upstairs/office` | Home office on second floor |
| `home/downstairs/kitchen` | Kitchen on first floor |
| `building-a/floor-3/room-301` | Office building room |
| `st-augustine/ksgj` | Weather station location |

**Purpose:** Supports hierarchical filtering and grouping without requiring fixed schema depth.

---

## Processing Concepts

### Ingestion Pipeline

**Definition:** The data processing path from source connection through parsing to storage.

**Stages:**
1. **Source Connection** - MQTT subscription or HTTP polling
2. **Message Parsing** - Extract fields from raw payload
3. **Context Attachment** - Add `ndp_id` and serialize `context` as JSON string
4. **Bronze Write** - Persist to Parquet files

**Role in AIR-009:** Modified to attach `ndp_id` and `context` JSON blob. No flattening, no promoted fields - just simple serialization.

---

### ETL (Extract, Transform, Load)

**Definition:** The process of moving and transforming data between layers.

**Bronze to Silver ETL:**
1. **Extract** - Read Parquet files from Bronze layer
2. **Transform** - Validate, parse `context` JSON string to JSONB
3. **Load** - Insert into TimescaleDB hypertables

**Role in AIR-009:** Must map `ndp_id` directly and parse `context` JSON string to JSONB column. Simple pass-through with JSON parsing.

---

### SourceConfig

**Definition:** The Rust struct representing a parsed source configuration.

**AIR-009 Additions:**
```rust
pub struct SourceConfig {
    // ... existing fields
    pub ndp_id: String,
    pub context: Option<HashMap<String, serde_json::Value>>,
}
```

---

## Query Patterns

### Source Query

**Definition:** A database query that filters records by source identity.

**Example:**
```sql
SELECT * FROM sensor_readings
WHERE ndp_id = 'airgradient-office-001'
ORDER BY time DESC
LIMIT 100;
```

---

### Context Query

**Definition:** A database query that filters or groups records by context attributes using JSONB operators.

**Examples:**
```sql
-- Query top-level context field
SELECT * FROM sensor_readings
WHERE context->>'device_type' = 'airgradient';

-- Query nested context field
SELECT * FROM sensor_readings
WHERE context->'location'->>'type' = 'indoor';

-- Query coordinates (array access)
SELECT * FROM sensor_readings
WHERE (context->'location'->'coordinates'->>0)::float > 29.0;

-- Group by device type (JSONB path)
SELECT context->>'device_type', AVG(pm25)
FROM sensor_readings
GROUP BY context->>'device_type';

-- Check for tag existence
SELECT * FROM sensor_readings
WHERE context->'tags' ? 'calibrated';
```

---

## Related Patterns

### Event Envelope Pattern

**Definition:** A design pattern (inspired by Amazon) where metadata/context travels alongside the event payload.

**Application in AIR-009:** Context attributes are "enveloped" with each data record at write time, ensuring the record carries its full attribution.

---

### Domain Adapter Pattern

**Definition:** NDP's hexagonal architecture pattern using Source and Store traits for pluggable data connectors.

**Relation to AIR-009:** `ndp_id` and context are part of the Source configuration, flowing through the Domain Adapter interface.

---

## Acronyms

| Acronym | Expansion |
|---------|-----------|
| NDP | Neural Data Platform |
| MQTT | Message Queuing Telemetry Transport |
| HTTP | Hypertext Transfer Protocol |
| API | Application Programming Interface |
| GPS | Global Positioning System |
| JSONB | JSON Binary (PostgreSQL type) |
| ETL | Extract, Transform, Load |
| SQL | Structured Query Language |
| YAML | YAML Ain't Markup Language |
| ADR | Architecture Decision Record |
| TDD | Test-Driven Development |
| SPARC | Specification, Pseudocode, Architecture, Refinement, Completion |

---

## See Also

- [REQUIREMENTS.md](./REQUIREMENTS.md) - Functional and non-functional requirements
- [ACCEPTANCE_CRITERIA.md](./ACCEPTANCE_CRITERIA.md) - Testable criteria for each requirement
- [USER_STORIES.md](./USER_STORIES.md) - User stories and sprint planning
- [SCOPE.md](../SCOPE.md) - Feature scope definition
- [ADR-002-AMENDMENT-002](../architecture/ADR-002-AMENDMENT-002-simple-blob.md) - Simple blob context storage decision
