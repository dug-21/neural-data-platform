# Silver Layer Configuration Research

## Executive Summary

This document details how Silver ETL is configured and how Silver tables are created in the Neural Data Platform. The research reveals a **configuration split problem** (documented in air-013) where Bronze and Silver layers load configuration from different sources.

---

## 1. Silver ETL Configuration

### 1.1 SilverEtlConfig Definition

**Location:** `/workspaces/neural-data-platform/core/src/config/silver_etl.rs` (lines 59-106)

```rust
pub struct SilverEtlConfig {
    pub enabled: bool,
    pub target_table: String,                    // e.g., "silver.air_quality_observations"
    pub target_schema: Option<String>,
    pub timestamp: TimestampMapping,
    pub valid_timestamp: Option<ValidTimestampMapping>,
    pub pre_transform: Option<PreTransformConfig>,
    pub identity_fields: Vec<IdentityField>,
    pub field_mappings: Vec<SilverFieldMapping>,
    pub dq_rules: Vec<DqRule>,
    pub dq_output: DqOutputConfig,
    pub deduplication: DeduplicationConfig,
    pub incremental: IncrementalConfig,
}
```

### 1.2 Key Sub-Structures

| Struct | Purpose | Key Fields |
|--------|---------|------------|
| `TimestampMapping` | Maps Bronze timestamp to Silver | `source_field`, `target_field`, `transform` |
| `SilverFieldMapping` | Maps Bronze JSON paths to Silver columns | `source_path`, `target_column`, `type`, `dq_rules` |
| `IdentityField` | Pass-through fields (e.g., ndp_id) | `source`, `target` |
| `DqRule` | Data quality rules (11 types) | Varies by rule type |
| `PreTransformConfig` | Array explosion for NWS forecasts | `ArrayExplosionConfig` |

### 1.3 Configuration Loading

**Two Loading Paths Exist:**

#### Path A: Batch Silver ETL (apps/silver-etl)
**File:** `/workspaces/neural-data-platform/apps/silver-etl/src/config.rs` (lines 33-129)

```rust
// ConfigLoader tries etcd first, then YAML files
impl ConfigLoader {
    pub async fn load_stream_config(&self, stream_id: &str) -> Result<SilverEtlConfig> {
        // Try etcd first
        match self.load_from_etcd(stream_id).await {
            Ok(config) => return Ok(config),
            Err(_) => { /* fallback to YAML */ }
        }
        // Fallback to YAML
        self.load_from_yaml(stream_id).await
    }
}
```

**etcd path:** `/streams/{stream_id}/silver_etl/*` (flattened keys)
**YAML path:** `{config_dir}/{stream_id}/config.yaml` or `{config_dir}/{stream_id}.yaml`

#### Path B: Streaming SilverSubscriber (air-quality-app)
**File:** `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs` (lines 601-629)

```rust
async fn load_silver_etl_config(
    config_dir: &str,
    stream_id: &str,
) -> Result<Option<SilverEtlConfig>, ...> {
    // Reads YAML directly - NO etcd fallback!
    let contents = tokio::fs::read_to_string(&yaml_path).await?;
    let config: StreamConfigWithSilver = serde_yaml::from_str(&contents)?;
    Ok(config.silver_etl)
}
```

---

## 2. SilverSubscriber Creation

### 2.1 Discovery Flow

**File:** `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs` (lines 500-598)

```
1. Registry lists streams from etcd:
   streams = registry.list_streams().await

2. For each stream, load YAML config:
   load_silver_etl_config(config_dir, stream_id)

3. If enabled, create SilverSubscriber:
   SilverSubscriber::new(config, timescale_output)
```

**The Problem (air-013):**
- `list_streams()` queries **etcd**
- `load_silver_etl_config()` reads **YAML files**
- If etcd sync fails, stream is NOT in list, so YAML is never read

### 2.2 SilverSubscriber Structure

**File:** `/workspaces/neural-data-platform/core/src/subscribers/silver.rs` (lines 161-175)

```rust
pub struct SilverSubscriber<O, B> {
    config: SilverSubscriberConfig,
    output: Arc<O>,               // TimescaleOutput
    bronze_reader: Option<Arc<B>>,
    state: SubscriberState,
    high_water_mark: Option<DateTime<Utc>>,
    // ... metrics fields
}
```

**SilverSubscriberConfig** (lines 50-76):
```rust
pub struct SilverSubscriberConfig {
    pub subscriber_id: String,
    pub stream_filter: HashSet<String>,
    pub etl_configs: HashMap<String, SilverEtlConfig>,  // stream_id -> config
    pub catch_up: CatchUpConfig,
    pub batch_size: usize,
    pub flush_interval_secs: u64,
}
```

### 2.3 Table Mapping Configuration

**File:** `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs` (lines 513-537)

```rust
// Build table_mapping from ETL configs
for stream_id in &streams {
    if let Ok(Some(silver_config)) = load_silver_etl_config(&config_dir, stream_id).await {
        if silver_config.enabled {
            // Use target_table from config (e.g., "silver.air_quality_observations")
            table_mapping.insert(stream_id.clone(), silver_config.target_table.clone());
        }
    }
}
```

**Result:**
- `table_mapping["air-quality"] = "silver.air_quality_observations"`
- `table_mapping["nws-observations"] = "silver.weather_observations"`

---

## 3. Silver Table DDL

### 3.1 Table Creation Location

**Init Script:** `/workspaces/neural-data-platform/deploy/timescaledb/init/001_silver_schema.sql`
**Migration:** `/workspaces/neural-data-platform/deploy/timescaledb/migrations/001_silver_schema.sql`

### 3.2 Current Tables (Manually Created)

| Table | Lines | Primary Key | Chunk Interval |
|-------|-------|-------------|----------------|
| `silver.air_quality_observations` | 101-148 | `(observation_time, ndp_id)` | 1 day |
| `silver.weather_observations` | 172-229 | `(observation_time, ndp_id)` | 1 day |
| `silver.weather_forecasts` | 249-308 | `(valid_time, issue_time, ndp_id)` | 1 day |
| `silver.outdoor_air_quality` | 347-382 | `(observation_time, ndp_id)` | 1 day |

### 3.3 Schema Pattern

```sql
CREATE TABLE silver.air_quality_observations (
    -- Audit/Metadata
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_time    TIMESTAMPTZ NOT NULL,
    source_stream       TEXT NOT NULL DEFAULT 'air-quality',
    ndp_id              TEXT NOT NULL,

    -- Device Context
    device_serial       TEXT,
    location_path       TEXT,

    -- Core Metrics (from field_mappings)
    co2                 SMALLINT,
    pm25                DOUBLE PRECISION,
    pm10                DOUBLE PRECISION,
    temperature_c       DOUBLE PRECISION,
    humidity_pct        DOUBLE PRECISION,
    -- ...

    -- DQ Transparency
    dq_flags            TEXT[],

    PRIMARY KEY (observation_time, ndp_id)
);

-- Convert to hypertable
SELECT create_hypertable('silver.air_quality_observations',
    'observation_time',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);
```

### 3.4 Compression and Retention

**File:** `/workspaces/neural-data-platform/deploy/timescaledb/init/001_silver_schema.sql` (lines 400-471)

```sql
-- Compression after 7 days
ALTER TABLE silver.air_quality_observations SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'ndp_id',
    timescaledb.compress_orderby = 'observation_time DESC'
);
SELECT add_compression_policy('silver.air_quality_observations', INTERVAL '7 days');

-- Retention: 90 days raw data
SELECT add_retention_policy('silver.air_quality_observations', INTERVAL '90 days');
```

### 3.5 Table Creation is MANUAL

**Current State:** Tables are created manually via SQL scripts.
**No automation:** There is no code that generates CREATE TABLE from `SilverEtlConfig`.

---

## 4. Field Mappings

### 4.1 Configuration in YAML

**File:** `/workspaces/neural-data-platform/config/base/streams/air-quality/config.yaml` (lines 150-256)

```yaml
silver_etl:
  enabled: true
  target_table: silver.air_quality_observations

  timestamp:
    source_field: timestamp
    target_field: observation_time
    transform: microseconds_to_timestamp

  identity_fields:
    - source: ndp_id
      target: ndp_id

  field_mappings:
    - source_path: raw_payload.pm02Compensated
      target_column: pm25
      type: double_precision
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag

    - source_path: raw_payload.rco2
      target_column: co2
      type: smallint
      dq_rules:
        - rule: range_check
          min: 380
          max: 10000
          action: flag
```

### 4.2 Field Mapping Application

**Streaming (SilverSubscriber):**
**File:** `/workspaces/neural-data-platform/core/src/silver/transform.rs` (lines 28-81)

```rust
pub fn transform_to_silver(raw: &RawDataPoint, config: &SilverEtlConfig) -> Result<SilverRecord, ...> {
    // 1. Transform timestamp
    let timestamp = transform_timestamp(raw, &config.timestamp)?;

    // 2. Extract identity fields
    for identity_field in &config.identity_fields {
        if let Some(value) = extract_json_path(&identity_field.source, raw) {
            record.identity_fields.insert(identity_field.target.clone(), value);
        }
    }

    // 3. Apply field mappings
    for mapping in &config.field_mappings {
        match apply_field_mapping(raw, mapping) {
            Ok(Some(value)) => {
                record.fields.insert(mapping.target_column.clone(), value);
            }
            // ...
        }
    }
}
```

**Batch (silver-etl):**
**File:** `/workspaces/neural-data-platform/apps/silver-etl/src/etl.rs` (lines 711-833)

Generates SQL dynamically from config:
```rust
// Add field mappings
for mapping in &config.field_mappings {
    columns.push(mapping.target_column.clone());
    let expr = sql_gen.generate_select_expr(mapping);
    select_exprs.push(expr);
}
```

### 4.3 JSON Path Extraction

**File:** `/workspaces/neural-data-platform/core/src/silver/transform.rs` (lines 436-465)

```rust
fn extract_json_path(path: &str, raw: &RawDataPoint) -> Option<Value> {
    let parts: Vec<&str> = path.split('.').collect();

    match parts[0] {
        "raw_payload" => {
            let mut current = &raw.raw_payload;
            for part in &parts[1..] {
                current = navigate_json_part(current, part)?;
            }
            Some(current.clone())
        }
        "context" => { /* ... */ }
        "ndp_id" => raw.ndp_id.clone().map(Value::String),
        "timestamp" => { /* ... */ }
        _ => raw.raw_payload.get(parts[0]).cloned()
    }
}
```

Supports array indexing: `raw_payload.list[0].components.pm2_5`

---

## 5. The air-013 Problem: etcd vs YAML Split

### 5.1 Root Cause

**Bronze Config:** Loaded from etcd (`StreamRegistry.load_stream()`)
**Silver Config:** Loaded from YAML files (`load_silver_etl_config()`)

### 5.2 Failure Mode

```
YAML config exists with valid silver_etl section
        |
        v
ConfigSyncService.sync_all() fails (e.g., validation error)
        |
        v
/streams/{stream_id}/config key NOT created in etcd
        |
        v
registry.list_streams() doesn't return the stream
        |
        v
load_silver_etl_config() never called for that stream
        |
        v
SilverSubscriber not created - SILENT FAILURE
```

### 5.3 Code Evidence

**air-quality-app/src/main.rs:**
```rust
// Line 512: Lists streams from etcd
let streams = registry.list_streams().await.unwrap_or_default();

// Line 516-527: For each stream, loads YAML
for stream_id in &streams {
    if let Ok(Some(silver_config)) = load_silver_etl_config(&config_dir, stream_id).await {
        // Only reached if stream exists in etcd AND YAML exists
    }
}
```

### 5.4 Proposed Solution (air-013)

Add `silver_etl` to `StreamConfig` and store in etcd:

```rust
// core/src/types/stream_config.rs (proposed)
pub struct StreamConfig {
    // ... existing fields ...
    pub silver_etl: Option<SilverEtlConfig>,
}
```

Then both Bronze and Silver load from etcd, ensuring consistency.

---

## 6. Code Path Summary

### 6.1 From Config to Running ETL

```
1. YAML File (config/base/streams/air-quality/config.yaml)
   |
   +-- ConfigSyncService.sync_all() --> etcd (Bronze fields only)
   |
   +-- load_silver_etl_config() --> SilverSubscriber (Silver fields)

2. Air Quality App Startup (main.rs)
   |
   +-- registry.list_streams() --> etcd query
   |
   +-- For each stream:
       |
       +-- load_silver_etl_config() --> YAML file
       |
       +-- If enabled:
           |
           +-- SilverSubscriber::new(config, timescale_output)
           |
           +-- subscriber_coordinator.register(subscriber)

3. Event Processing
   |
   +-- EventBus receives RawDataPoint
   |
   +-- SilverSubscriber.process_event()
       |
       +-- transform_to_silver(raw, config)
       |
       +-- output.write(record, config)
```

### 6.2 Silver vs Bronze Config Consumption

| Aspect | Bronze | Silver |
|--------|--------|--------|
| **Config Source** | etcd (`StreamRegistry`) | YAML files (direct) |
| **Discovery** | `registry.list_streams()` | Iterates etcd streams, reads YAML |
| **Sync** | `ConfigSyncService` syncs YAML to etcd | No sync, reads YAML directly |
| **Failure Mode** | Explicit error if etcd unavailable | Silent skip if YAML missing |
| **Config Struct** | `StreamConfig` | `SilverEtlConfig` |

---

## 7. Key File Locations

| Component | File | Key Lines |
|-----------|------|-----------|
| SilverEtlConfig | `core/src/config/silver_etl.rs` | 59-106 |
| Config Module | `core/src/config/mod.rs` | 1-33 |
| ConfigLoader (batch) | `apps/silver-etl/src/config.rs` | 16-228 |
| SilverSubscriber | `core/src/subscribers/silver.rs` | 161-667 |
| Transform | `core/src/silver/transform.rs` | 28-81, 96-277 |
| App Main | `apps/air-quality-app/src/main.rs` | 500-629 |
| Silver DDL | `deploy/timescaledb/init/001_silver_schema.sql` | All |
| Migration DDL | `deploy/timescaledb/migrations/001_silver_schema.sql` | All |
| Air Quality Config | `config/base/streams/air-quality/config.yaml` | 150-318 |

---

## 8. Conclusions

### Why air-013 Problem Exists

1. **Historical Evolution:** Bronze layer was built first with etcd. Silver layer added later with direct YAML access.
2. **Incremental Development:** Features dp-006 through dp-012 added Silver layer piecemeal.
3. **Two Binaries:** `silver-etl` (batch) and `air-quality-app` (streaming) evolved separately.

### Current Limitations

1. **No Config-Driven Table Creation:** Tables must be manually created in SQL.
2. **Schema Drift Risk:** YAML field_mappings can diverge from actual SQL schema.
3. **Silent Failures:** If sync fails, Silver ETL silently skips streams.
4. **Duplicate Loading Logic:** Both batch and streaming have separate config loaders.

### Recommendations for dp-016

1. Implement air-013 first (unified config source)
2. Add schema validation (YAML field_mappings vs SQL table schema)
3. Consider config-driven table creation (generate DDL from SilverEtlConfig)
4. Add health checks for config consistency
