# ETL Genericity Assessment for Multi-Domain Data Platform

**Document**: 09-etl-genericity-assessment.md
**Version**: 1.0
**Date**: 2026-01-05
**Author**: NDP Analytics Engineer
**Status**: Complete - Ready for Review

---

## Executive Summary

This assessment evaluates the Neural Data Platform's config-driven Silver ETL design for multi-domain extensibility. While the current design works well for weather and air quality domains (time-series sensor data with JSON payloads over HTTP/MQTT), significant gaps exist for supporting diverse future domains.

### Key Findings

| Aspect | Current Capability | Multi-Domain Readiness | Priority |
|--------|-------------------|----------------------|----------|
| **Transform Types** | Linear unit conversion, range checks | Limited - needs delta, precision, event transforms | High |
| **Source Variety** | HTTP poll, MQTT only | Limited - needs file, streaming, batch sources | Medium |
| **Schema Flexibility** | Fixed typed columns | Limited - needs evolution, sparse/EAV patterns | Medium |
| **DQ Genericity** | Range, null, pattern | Limited - needs temporal, referential, domain-specific | High |

### Recommendation Summary

**Implement Now (DP-006)**:
1. Delta/difference transform for cumulative readings
2. Decimal precision transform for financial data
3. Temporal DQ rules (monotonicity, gap detection)
4. Schema evolution mechanism

**Implement Later (Future Phases)**:
1. Event deduplication and sparse data handling
2. File upload and batch source patterns
3. Streaming source integration (WebSocket, Kafka)
4. Referential integrity DQ checks

---

## 1. Transform Type Assessment

### 1.1 Current Transform Capabilities

The current design supports:

```yaml
transform:
  type: unit_conversion
  from: celsius
  to: celsius
  formula: { type: linear, scale: 1.0, offset: 0.0 }
```

This handles:
- **Linear transformations**: `output = input * scale + offset`
- **Unit conversions**: Kelvin to Celsius, m/s to km/h, Pa to hPa
- **Simple scaling**: Percentage normalization

### 1.2 Gap Analysis by Domain

#### Energy Domain (Cumulative Readings)

**Challenge**: Smart meters report cumulative kWh readings. Analytics needs power (kW) derived from delta between consecutive readings.

**Current Capability**: Cannot handle cumulative-to-delta transformation.

**Required Transform Type**:
```yaml
transform:
  type: delta
  time_column: observation_time
  partition_by: [ndp_id]
  order_by: observation_time
  output_unit: kW
  time_unit: hours  # Delta per hour
  handle_resets: wrap_around  # For meter rollovers at max value
```

**SQL Generation Pattern**:
```sql
SELECT
    observation_time,
    ndp_id,
    cumulative_kwh - LAG(cumulative_kwh) OVER (
        PARTITION BY ndp_id ORDER BY observation_time
    ) / NULLIF(
        EXTRACT(EPOCH FROM observation_time - LAG(observation_time) OVER (
            PARTITION BY ndp_id ORDER BY observation_time
        )) / 3600.0, 0
    ) AS power_kw
FROM bronze.energy_readings
```

**Implementation Complexity**: Medium (requires window functions in DuckDB, state tracking for resets)

#### Financial Domain (Decimal Precision)

**Challenge**: Financial data requires exact decimal precision (e.g., $1234.56, not floating point approximation).

**Current Capability**: Uses `DOUBLE PRECISION` which can introduce rounding errors.

**Required Transform Type**:
```yaml
transform:
  type: decimal_precision
  scale: 2        # Decimal places
  precision: 18   # Total digits
  rounding: half_even  # Banker's rounding
```

**Target Type**: PostgreSQL `NUMERIC(18,2)` or `DECIMAL(18,2)`

**SQL Generation Pattern**:
```sql
SELECT
    ROUND(raw_amount::NUMERIC, 2) AS amount,
    CAST(raw_price AS NUMERIC(18,8)) AS price  -- Crypto needs 8 decimals
FROM bronze.transactions
```

**Implementation Complexity**: Low (type casting with precision specification)

#### Smart Home Domain (Event Deduplication)

**Challenge**: Event-based data (door open/close, motion detected) may have:
- Duplicate events from network retries
- State change detection (not continuous time-series)
- Sparse data (hours between events)

**Current Capability**: Simple deduplication by `key_columns` with `upsert` strategy.

**Required Transform Type**:
```yaml
transform:
  type: event_deduplication
  dedup_window: 5s          # Events within 5s are duplicates
  state_column: state       # Track state changes
  emit_on: state_change     # Only emit when state changes
  sequence_field: event_id  # Optional ordering field
```

**SQL Generation Pattern**:
```sql
WITH ordered AS (
    SELECT *,
        LAG(state) OVER (PARTITION BY ndp_id ORDER BY event_time) AS prev_state,
        ROW_NUMBER() OVER (
            PARTITION BY ndp_id,
            time_bucket('5 seconds', event_time)
            ORDER BY event_time
        ) AS rn
    FROM bronze.smart_home_events
)
SELECT * FROM ordered
WHERE rn = 1  -- Take first in dedup window
  AND (state != prev_state OR prev_state IS NULL)  -- State changed
```

**Implementation Complexity**: High (requires window functions, state tracking)

### 1.3 Transform Types Needed for Genericity

| Transform Type | Priority | Use Cases | Complexity |
|---------------|----------|-----------|------------|
| `delta` | High | Energy, water meters, counters | Medium |
| `decimal_precision` | High | Financial, billing | Low |
| `event_deduplication` | Medium | Smart home, IoT events | High |
| `json_flatten` | Medium | Nested API responses | Medium |
| `array_explode` | Medium | Forecast periods, batch records | Medium |
| `string_parse` | Low | Legacy text formats, logs | Medium |
| `conditional` | Low | Domain-specific business rules | High |

### 1.4 Recommended Transform Extension

```yaml
# Extended transform configuration schema
transform:
  type: <transform_type>

  # For linear/unit_conversion (existing)
  formula: { type: linear, scale: 1.0, offset: 0.0 }

  # For delta (new)
  delta:
    time_column: observation_time
    partition_by: [ndp_id]
    time_unit: hours
    handle_resets: wrap_around | ignore | null

  # For decimal_precision (new)
  decimal:
    scale: 2
    precision: 18
    rounding: half_even | floor | ceil | truncate

  # For event_deduplication (new)
  dedup:
    window: 5s
    state_column: state
    emit_on: state_change | all | first
```

---

## 2. Source Variety Assessment

### 2.1 Current Source Capabilities

```rust
pub enum SourceType {
    Mqtt,
    HttpPoll,
    Webhook,
    FileWatch,
}
```

**Currently Implemented**: MQTT, HTTP Poll
**Declared but Not Implemented**: Webhook, FileWatch

### 2.2 Gap Analysis by Source Pattern

#### File Upload Sources

**Use Cases**:
- Daily CSV drops from legacy systems
- Batch uploads from spreadsheets
- Historical data imports

**Required Integration Pattern**:
```yaml
sources:
  - type: file_watch
    enabled: true
    watch_path: /data/uploads/energy/
    pattern: "*.csv"
    format: csv
    csv_options:
      delimiter: ","
      header: true
      date_format: "%Y-%m-%d %H:%M:%S"
    on_complete: move_to_processed | delete | archive
    poll_interval: 60s  # Check for new files every minute
```

**Bronze Layer Impact**:
- Files should be converted to Parquet with same schema as streaming sources
- Metadata includes `source_file`, `import_time`
- Deduplication by file + row hash

**Implementation Complexity**: Medium
- Requires file system monitoring (inotify or polling)
- CSV/Excel parsing library
- Batch-to-stream adapter

#### Streaming Sources (WebSocket, Kafka)

**Use Cases**:
- Real-time market data (WebSocket)
- Enterprise event streams (Kafka)
- High-frequency IoT (MQTT QoS 0 at scale)

**Required Integration Pattern**:
```yaml
sources:
  - type: websocket
    enabled: true
    url: "wss://stream.example.com/v1/data"
    auth:
      type: bearer
      token_env: WS_API_TOKEN
    reconnect_delay_secs: 5
    heartbeat_interval_secs: 30

  - type: kafka
    enabled: true
    bootstrap_servers: ["kafka:9092"]
    topic: "sensor-events"
    group_id: "ndp-consumer"
    offset_reset: earliest
    batch_size: 1000
    commit_interval_ms: 5000
```

**Bronze Layer Impact**:
- Higher throughput requirements (may need Parquet streaming write)
- Offset/checkpoint management for replay
- Backpressure handling

**Implementation Complexity**: High
- Kafka: `rdkafka` crate, offset management, rebalancing
- WebSocket: `tokio-tungstenite`, reconnection, heartbeat

#### Batch Sources (Daily Drops)

**Use Cases**:
- Third-party data vendor daily feeds
- Regulatory reporting (end-of-day snapshots)
- Partner data exchange

**Required Integration Pattern**:
```yaml
sources:
  - type: scheduled_batch
    enabled: true
    schedule: "0 6 * * *"  # Daily at 6 AM
    source_type: sftp
    sftp:
      host: vendor.example.com
      path: /outbound/daily/
      pattern: "data_*.csv.gz"
      credentials_env: VENDOR_SFTP_CREDS
    processing:
      decompress: gzip
      format: csv
      date_column: report_date
      full_replace: false  # Append, don't replace
```

**Bronze Layer Impact**:
- Idempotent processing (same file = same output)
- Full vs incremental load tracking
- Historical vs current partition handling

**Implementation Complexity**: Medium
- Scheduling: systemd timer or internal scheduler
- SFTP: `ssh2` crate
- Decompression: `flate2` crate

### 2.3 Source Integration Roadmap

| Source Type | Priority | Timeline | Dependencies |
|-------------|----------|----------|--------------|
| File Watch (CSV) | High | DP-007 | File system monitoring |
| Webhook (push) | Medium | DP-008 | HTTP server integration |
| Scheduled Batch | Medium | DP-009 | Scheduler, SFTP |
| WebSocket | Low | Future | Async streaming |
| Kafka | Low | Future | Enterprise use cases |

---

## 3. Schema Flexibility Assessment

### 3.1 Current Schema Approach

Fixed typed columns per entity:
```sql
CREATE TABLE silver.air_quality_observations (
    observation_time    TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,
    pm25                DOUBLE PRECISION,
    co2                 SMALLINT,
    temperature_c       DOUBLE PRECISION,
    -- Fixed column set
);
```

### 3.2 Schema Evolution Challenges

#### Adding Columns

**Scenario**: New sensor firmware adds `voc_index` field.

**Current Handling**:
1. Modify config YAML to add field mapping
2. ALTER TABLE to add column
3. Backfill if historical data exists

**Gap**: No versioned schema migration tracking.

**Recommended Solution**:
```yaml
schema_version: "2.0.0"
migrations:
  - version: "2.0.0"
    changes:
      - type: add_column
        column: voc_index
        datatype: smallint
        nullable: true
        after: nox_index
    backfill:
      enabled: true
      source_path: raw_payload.vocIndex
```

#### Semi-Structured Data

**Scenario**: Smart home events have varying fields per device type.

**Current Handling**: Would require separate tables per device type.

**Recommended Solutions**:

**Option A: JSONB Overflow Column**
```sql
CREATE TABLE silver.smart_home_events (
    event_time      TIMESTAMPTZ NOT NULL,
    ndp_id          TEXT NOT NULL,
    event_type      TEXT NOT NULL,
    -- Common fields as columns
    device_type     TEXT,
    state           TEXT,
    -- Device-specific fields in JSONB
    attributes      JSONB DEFAULT '{}'
);
```

**Option B: Entity-Attribute-Value (EAV)**
```sql
CREATE TABLE silver.smart_home_event_attributes (
    event_time      TIMESTAMPTZ NOT NULL,
    ndp_id          TEXT NOT NULL,
    attribute_name  TEXT NOT NULL,
    attribute_value TEXT,  -- Or JSONB for typed values
    PRIMARY KEY (event_time, ndp_id, attribute_name)
);
```

**Trade-offs**:
| Approach | Query Performance | Flexibility | Storage | TimescaleDB Support |
|----------|------------------|-------------|---------|---------------------|
| Fixed columns | Excellent | Low | Optimal | Full |
| JSONB overflow | Good | High | Moderate | Full |
| EAV | Poor | Very High | High | Limited |

**Recommendation**: Use JSONB overflow column for device-specific attributes, with promoted columns for frequently-queried fields.

### 3.3 Schema Flexibility Configuration

```yaml
schema:
  version: "1.0.0"
  evolution_strategy: backward_compatible

  columns:
    # Fixed columns (always present)
    - name: observation_time
      type: timestamptz
      nullable: false

    - name: ndp_id
      type: text
      nullable: false

    # Typed columns (extracted from payload)
    - name: temperature_c
      type: double_precision
      source_path: raw_payload.temp
      nullable: true

    # Overflow column for semi-structured data
    - name: extra_attributes
      type: jsonb
      source: remaining_fields
      exclude: [observation_time, ndp_id, temperature_c]

  promotion_rules:
    # Auto-promote fields that appear frequently
    - field_pattern: "raw_payload.*"
      occurrence_threshold: 0.9  # 90% of records have this field
      query_frequency: high       # Used in dashboards/alerts
      action: promote_to_column
```

---

## 4. Data Quality Genericity Assessment

### 4.1 Current DQ Capabilities

```yaml
dq_rules:
  - rule: range_check
    min: -50.0
    max: 60.0
    action: flag  # flag | reject | clamp | drop

  - rule: not_null
    action: reject

  - rule: pattern
    regex: "^[A-Z]{4}$"
    action: flag
```

### 4.2 Missing DQ Patterns

#### Temporal DQ Rules

**Use Cases**:
- Energy meters must be monotonically increasing (except resets)
- Time-series should have no large gaps
- Timestamps must be in expected order

**Required Rules**:
```yaml
dq_rules:
  # Monotonicity check
  - rule: monotonic
    column: cumulative_kwh
    direction: increasing  # increasing | decreasing | strict_increasing
    partition_by: [ndp_id]
    allow_reset: true
    reset_threshold: 1000000  # Values below this after high value = reset
    action: flag

  # Gap detection
  - rule: time_gap
    time_column: observation_time
    partition_by: [ndp_id]
    expected_interval: 1m
    max_gap: 5m
    action: flag  # Flag when gap > 5 minutes

  # Sequence validation
  - rule: sequence_order
    sequence_column: event_sequence
    partition_by: [session_id]
    action: reject  # Reject out-of-order events
```

**SQL Generation for Monotonicity**:
```sql
SELECT *,
    CASE
        WHEN cumulative_kwh < LAG(cumulative_kwh) OVER (
            PARTITION BY ndp_id ORDER BY observation_time
        )
        AND cumulative_kwh > 1000000  -- Not a reset
        THEN 'monotonic:cumulative_kwh:decreased'
        ELSE NULL
    END AS dq_monotonic_flag
FROM bronze.energy_readings
```

#### Referential DQ Rules

**Use Cases**:
- `ndp_id` should exist in device registry
- `location_id` should match valid locations
- `category_code` should be in lookup table

**Required Rules**:
```yaml
dq_rules:
  - rule: foreign_key
    column: ndp_id
    reference_table: data_dictionary.sources
    reference_column: ndp_id
    action: flag  # Flag unknown devices

  - rule: lookup
    column: category_code
    valid_values: [A, B, C, D]  # Inline list
    # Or: lookup_table: reference.categories
    action: reject
```

**Implementation Note**: Referential checks require access to lookup tables during ETL. Consider:
1. Pre-loading lookup tables into DuckDB memory
2. Using PostgreSQL foreign data wrapper for live checks
3. Caching lookup values in config

#### Statistical DQ Rules

**Use Cases**:
- Detect outliers beyond N standard deviations
- Identify sudden value spikes
- Rate of change limits

**Required Rules**:
```yaml
dq_rules:
  - rule: statistical_outlier
    column: temperature_c
    method: zscore  # zscore | iqr | mad
    threshold: 3.0  # 3 standard deviations
    window: 24h     # Rolling window for statistics
    partition_by: [ndp_id]
    action: flag

  - rule: rate_of_change
    column: temperature_c
    max_change_per_minute: 2.0  # Max 2 degrees per minute
    partition_by: [ndp_id]
    action: flag
```

### 4.3 DQ Rule Extension Schema

```yaml
dq_rules:
  # Existing rules
  - rule: range_check | not_null | pattern
    # ... existing config

  # Temporal rules (new)
  - rule: monotonic | time_gap | sequence_order
    # ... temporal config

  # Referential rules (new)
  - rule: foreign_key | lookup
    # ... referential config

  # Statistical rules (new)
  - rule: statistical_outlier | rate_of_change
    # ... statistical config

  # Domain-specific rules (extensible)
  - rule: custom
    sql: |
      CASE
        WHEN pm25 > 35 AND humidity > 80
        THEN 'domain:high_pm_high_humidity'
        ELSE NULL
      END
```

### 4.4 DQ Rule Priority Matrix

| Rule Type | Priority | Implementation | Dependencies |
|-----------|----------|----------------|--------------|
| Range check | Implemented | - | - |
| Not null | Implemented | - | - |
| Pattern | Implemented | - | - |
| Monotonic | High | DP-006 | Window functions |
| Time gap | High | DP-006 | Window functions |
| Rate of change | High | DP-006 | Window functions |
| Foreign key | Medium | Future | Lookup table loading |
| Statistical | Low | Future | Rolling statistics |
| Custom SQL | Low | Future | SQL injection safety |

---

## 5. Implementation Recommendations

### 5.1 What to Implement Now (DP-006)

These additions have high value and low-to-medium complexity:

#### Transform Extensions

1. **Delta Transform**
   - Essential for energy domain
   - Uses standard SQL window functions
   - Config-driven, no code changes needed per stream

2. **Decimal Precision Transform**
   - Essential for financial domain
   - Simple type casting in SQL generation
   - Prevents floating-point errors

3. **Array Explode Transform** (already partially designed for forecasts)
   - Needed for NWS forecast periods
   - Reusable for other array payloads

#### DQ Extensions

1. **Monotonicity Check**
   - Catches counter resets, data corruption
   - Standard window function pattern

2. **Time Gap Detection**
   - Identifies data collection issues
   - Standard window function pattern

3. **Rate of Change Limit**
   - Catches sensor malfunctions
   - Simple calculation with LAG()

#### Schema Extensions

1. **JSONB Overflow Column**
   - Add `extra_attributes JSONB` to Silver tables
   - Store unmapped Bronze fields for future access

2. **Schema Version Tracking**
   - Add `schema_version` to stream config
   - Migration tracking table in data_dictionary

### 5.2 What to Implement Later

These can wait for specific domain requirements:

#### Source Extensions (DP-007+)

- File watch source for CSV/Excel imports
- Webhook source for push-based ingestion
- Scheduled batch source for vendor feeds

#### Advanced Transforms (Future)

- Event deduplication with state tracking
- Complex conditional transforms
- String parsing for legacy formats

#### Advanced DQ (Future)

- Referential integrity checks
- Statistical outlier detection
- Custom SQL rules with safety validation

### 5.3 Architecture Changes for Genericity

#### 1. Transform Registry Pattern

```rust
pub trait Transform: Send + Sync {
    fn name(&self) -> &str;
    fn generate_sql(&self, config: &TransformConfig, context: &SqlContext) -> Result<String>;
    fn validate_config(&self, config: &TransformConfig) -> Result<()>;
}

pub struct TransformRegistry {
    transforms: HashMap<String, Box<dyn Transform>>,
}

impl TransformRegistry {
    pub fn register(&mut self, transform: Box<dyn Transform>) {
        self.transforms.insert(transform.name().to_string(), transform);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Transform> {
        self.transforms.get(name).map(|t| t.as_ref())
    }
}
```

#### 2. DQ Rule Registry Pattern

```rust
pub trait DqRule: Send + Sync {
    fn name(&self) -> &str;
    fn generate_check_sql(&self, config: &DqRuleConfig, context: &SqlContext) -> Result<String>;
    fn generate_flag_sql(&self, config: &DqRuleConfig, column: &str) -> Result<String>;
}

pub struct DqRuleRegistry {
    rules: HashMap<String, Box<dyn DqRule>>,
}
```

#### 3. Source Abstraction

```rust
pub trait SourceAdapter: Send + Sync {
    fn source_type(&self) -> SourceType;
    async fn connect(&mut self, config: &SourceConfig) -> Result<()>;
    async fn fetch_batch(&mut self) -> Result<Vec<StreamRecord>>;
    async fn checkpoint(&mut self, position: SourcePosition) -> Result<()>;
}
```

### 5.4 Config Schema Evolution

**Current** (v1.0):
```yaml
silver_etl:
  field_mappings:
    - source_path: raw_payload.pm02
      target_column: pm25
      transform: null
      dq_rules: [...]
```

**Proposed** (v2.0):
```yaml
silver_etl:
  schema_version: "2.0.0"

  field_mappings:
    - source_path: raw_payload.pm02
      target_column: pm25
      type: double_precision
      transform:
        type: unit_conversion  # Explicit type
        formula: { type: linear, scale: 1.0, offset: 0.0 }
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag
        - rule: rate_of_change  # New rule type
          max_change_per_minute: 50.0
          action: flag

    - source_path: raw_payload.cumulative_kwh
      target_column: power_kw
      type: double_precision
      transform:
        type: delta  # New transform type
        delta:
          time_column: observation_time
          partition_by: [ndp_id]
          time_unit: hours
      dq_rules:
        - rule: monotonic  # New rule type
          direction: increasing
          allow_reset: true
          action: flag

  schema_evolution:
    strategy: backward_compatible
    overflow_column: extra_attributes
    auto_promote: false
```

---

## 6. Summary

### 6.1 Readiness Assessment

| Domain | Current Readiness | With DP-006 Extensions | Notes |
|--------|-------------------|----------------------|-------|
| **Weather/Air Quality** | Ready | Ready | Current design sufficient |
| **Energy** | Not Ready | Ready | Needs delta transform, monotonicity |
| **Smart Home** | Partial | Partial | Basic events work, state tracking deferred |
| **Financial** | Not Ready | Ready | Needs decimal precision |
| **Industrial** | Partial | Partial | Batch sources deferred |

### 6.2 Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Transform complexity grows | High | Medium | Registry pattern, clear interfaces |
| DQ rules become domain-specific | High | Low | Custom SQL escape hatch |
| Schema evolution breaks downstream | Medium | High | Version tracking, backward compatibility |
| Source variety fragments codebase | Medium | Medium | Adapter pattern, common interfaces |

### 6.3 Success Criteria for Generic Platform

1. **New domain onboarding**: Can add new domain (e.g., energy) with config-only changes in <4 hours
2. **Transform extensibility**: Can add new transform type with <200 LOC in Rust
3. **DQ extensibility**: Can add new DQ rule type with <100 LOC in Rust
4. **Schema flexibility**: Can add columns without downtime, with automatic backfill option
5. **Source variety**: Can add new source type with consistent Bronze output format

---

## Appendix A: Domain-Specific Transform Examples

### Energy Domain

```yaml
# config/base/streams/energy-consumption/config.yaml
stream_id: energy-consumption
description: "Smart meter cumulative readings"

silver_etl:
  enabled: true
  target_table: silver.energy_readings

  field_mappings:
    - source_path: raw_payload.cumulative_kwh
      target_column: cumulative_kwh
      type: double_precision
      dq_rules:
        - rule: range_check
          min: 0
          max: 1000000000
          action: flag
        - rule: monotonic
          direction: increasing
          allow_reset: true
          reset_threshold: 1000000
          action: flag

    - source_path: raw_payload.cumulative_kwh
      target_column: power_kw
      type: double_precision
      transform:
        type: delta
        delta:
          time_column: observation_time
          partition_by: [ndp_id]
          time_unit: hours
          handle_resets: wrap_around
      dq_rules:
        - rule: range_check
          min: 0
          max: 100  # Max 100 kW for residential
          action: flag
```

### Financial Domain

```yaml
# config/base/streams/transactions/config.yaml
stream_id: transactions
description: "Financial transaction records"

silver_etl:
  enabled: true
  target_table: silver.transactions

  field_mappings:
    - source_path: raw_payload.amount
      target_column: amount
      type: numeric(18,2)
      transform:
        type: decimal_precision
        decimal:
          precision: 18
          scale: 2
          rounding: half_even
      dq_rules:
        - rule: not_null
          action: reject

    - source_path: raw_payload.currency
      target_column: currency
      type: text
      dq_rules:
        - rule: lookup
          valid_values: [USD, EUR, GBP, JPY]
          action: reject
```

---

## Appendix B: DQ Rule SQL Generation Templates

### Monotonicity Check

```sql
-- Monotonicity DQ check template
WITH lagged AS (
    SELECT *,
        LAG({{column}}) OVER (
            PARTITION BY {{partition_by}}
            ORDER BY {{time_column}}
        ) AS prev_value
    FROM {{source_table}}
)
SELECT *,
    CASE
        WHEN {{column}} < prev_value
        {{#if allow_reset}}
        AND prev_value < {{reset_threshold}}
        {{/if}}
        THEN 'monotonic:{{column}}:decreased'
        ELSE NULL
    END AS dq_monotonic_flag
FROM lagged
```

### Time Gap Detection

```sql
-- Time gap DQ check template
WITH lagged AS (
    SELECT *,
        LAG({{time_column}}) OVER (
            PARTITION BY {{partition_by}}
            ORDER BY {{time_column}}
        ) AS prev_time
    FROM {{source_table}}
)
SELECT *,
    CASE
        WHEN {{time_column}} - prev_time > INTERVAL '{{max_gap}}'
        THEN 'time_gap:{{time_column}}:exceeded:' ||
             EXTRACT(EPOCH FROM {{time_column}} - prev_time)::TEXT || 's'
        ELSE NULL
    END AS dq_gap_flag
FROM lagged
```

### Rate of Change

```sql
-- Rate of change DQ check template
WITH lagged AS (
    SELECT *,
        LAG({{column}}) OVER (
            PARTITION BY {{partition_by}}
            ORDER BY {{time_column}}
        ) AS prev_value,
        LAG({{time_column}}) OVER (
            PARTITION BY {{partition_by}}
            ORDER BY {{time_column}}
        ) AS prev_time
    FROM {{source_table}}
)
SELECT *,
    CASE
        WHEN ABS({{column}} - prev_value) /
             NULLIF(EXTRACT(EPOCH FROM {{time_column}} - prev_time) / 60.0, 0)
             > {{max_change_per_minute}}
        THEN 'rate_of_change:{{column}}:exceeded'
        ELSE NULL
    END AS dq_rate_flag
FROM lagged
```

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-05 | NDP Analytics Engineer | Initial assessment |

---

## References

1. `research/agenticdataplatform/silver/06-refined-synthesis.md` - Config-driven ETL design
2. `research/agenticdataplatform/silver/03-data-dictionary.md` - Field mappings, DQ rules
3. `research/agenticdataplatform/silver/02-etl-alternatives.md` - ETL approach comparison
4. `core/src/types/stream_config.rs` - Current config schema implementation
5. `config/base/streams/air-quality/config.yaml` - Example stream configuration
