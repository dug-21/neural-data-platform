# DP-006 Specification: Silver Layer - Config-Driven ETL to TimescaleDB

**Feature ID**: dp-006
**Phase**: Specification
**Version**: 1.0
**Created**: 2026-01-10
**Author**: NDP Scrum Master
**Status**: Draft

---

## 1. Overview

### 1.1 Purpose

This specification defines the functional and non-functional requirements for implementing the Silver layer of the Neural Data Platform. The Silver layer transforms raw Bronze Parquet data into clean, typed TimescaleDB hypertables optimized for analytics, dashboards, and the primary use case: determining optimal window opening times.

### 1.2 Business Context

**Primary Use Case**: "When should I open/close the window for optimal indoor air quality?"

This requires:
- Indoor air quality data (PM2.5, CO2, temperature, humidity) from AirGradient sensors
- Outdoor air quality data (PM2.5, ozone, AQI) from OpenWeatherMap
- Outdoor weather data (temperature, humidity, wind) from NWS/OWM
- Weather forecasts (1-6 hour predictions) from NWS gridpoints

### 1.3 Scope Reference

See `SCOPE.md` for full scope definition including:
- In-scope deliverables
- Out-of-scope items (Gold layer, Home Assistant, continuous aggregates)
- Principles (config-driven, Bronze reliability, DQ transparency)
- ADR proposals

---

## 2. Functional Requirements

### 2.1 ETL Binary Requirements

#### FR-001: Separate Silver ETL Binary

**Requirement**: The system SHALL provide a separate Rust binary (`silver-etl`) for Bronze-to-Silver transformation.

**Rationale**: Process isolation ensures Bronze reliability is not compromised by Silver failures (per ADR-006-002 recommendation).

**Acceptance Criteria**:
- [ ] Binary compiles independently: `cargo build -p silver-etl`
- [ ] Binary runs without air-quality-app dependency
- [ ] Binary can be started/stopped independently via systemd
- [ ] Failure in silver-etl does not affect Bronze ingestion

#### FR-002: DuckDB-rs ETL Engine

**Requirement**: The system SHALL use duckdb-rs embedded as the ETL engine for reading Parquet and writing to PostgreSQL.

**Rationale**: Single binary deployment, proven PostgreSQL writes, ARM64/Pi-compatible (per ADR-006-001 recommendation).

**Acceptance Criteria**:
- [ ] DuckDB reads all Bronze Parquet files (glob pattern support)
- [ ] DuckDB writes to TimescaleDB via postgres extension
- [ ] Memory usage stays within 300MB limit on Pi
- [ ] ETL completes within 60 seconds for hourly batch

#### FR-003: Config-Driven Transforms

**Requirement**: The system SHALL generate ETL SQL from YAML configuration without requiring Rust code changes for new streams.

**Rationale**: Extensibility principle - adding a new stream should require YAML changes only.

**Acceptance Criteria**:
- [ ] ETL reads `silver_etl` section from stream config
- [ ] Field mappings drive SELECT clause generation
- [ ] Transform definitions drive conversion expressions
- [ ] DQ rules drive validation expressions
- [ ] New stream can be added with config-only changes (verified by test)

### 2.2 Configuration Requirements

#### FR-004: Silver ETL Config Schema

**Requirement**: The system SHALL extend stream configuration with a `silver_etl` section.

**Config Schema**:
```yaml
silver_etl:
  enabled: boolean              # Enable/disable Silver ETL for stream
  target_table: string          # Qualified table name (silver.table_name)
  target_schema: string         # Schema version reference
  timestamp:                    # Timestamp mapping
    source_field: string        # Bronze column
    target_field: string        # Silver column
    transform: enum             # microseconds_to_timestamp | iso8601 | unix_seconds
  identity_fields: array        # Passthrough fields
  field_mappings: array         # Transform definitions
  dq_output:                    # DQ transparency config
    enabled: boolean
    target_column: string
  deduplication:                # Upsert strategy
    enabled: boolean
    key_columns: array
    strategy: enum              # upsert | skip | replace
  incremental:                  # Incremental load
    enabled: boolean
    watermark_column: string
    lag_interval: string
```

**Acceptance Criteria**:
- [ ] Config types defined in `core/src/config/silver_etl.rs`
- [ ] Serde deserialize/serialize tests pass
- [ ] Invalid config produces clear error messages
- [ ] Config hot-reloads via etcd watch

#### FR-005: Field Mapping Configuration

**Requirement**: Each field mapping SHALL specify source path, target column, type, transforms, and DQ rules.

**Field Mapping Schema**:
```yaml
field_mappings:
  - source_path: string         # JSON path in raw_payload
    target_column: string       # Silver column name
    type: string                # PostgreSQL type
    nullable: boolean           # NULL allowed
    transform:                  # Optional transformation
      type: enum                # unit_conversion | expression | lookup | json_extract | timestamp | computed
      # Type-specific parameters
    dq_rules: array             # Validation rules
```

**Acceptance Criteria**:
- [ ] All 6 transform types implemented
- [ ] Transform generates correct SQL expression
- [ ] Type coercion handles type mismatches gracefully
- [ ] Missing source fields produce NULL (not errors)

#### FR-006: DQ Rule Configuration

**Requirement**: The system SHALL support configurable data quality rules with actions.

**Supported Rules**:
| Rule | Parameters | Description |
|------|------------|-------------|
| `range_check` | min, max | Numeric bounds validation |
| `not_null` | - | NULL rejection |
| `pattern` | regex | String pattern match |
| `one_of` | values[] | Enumeration check |
| `custom` | name, expr | Custom SQL expression |

**Supported Actions**:
| Action | Behavior |
|--------|----------|
| `flag` | Keep value, add rule name to dq_flags |
| `reject` | Set NULL, add to dq_flags |
| `clamp` | Clamp to bounds, add to dq_flags |
| `drop` | Drop entire row |

**Acceptance Criteria**:
- [ ] All 5 rule types generate correct SQL
- [ ] All 4 actions behave as specified
- [ ] dq_flags array populated correctly
- [ ] `drop` action excludes row from INSERT

### 2.3 Data Model Requirements

#### FR-007: Four Initial Silver Tables

**Requirement**: The system SHALL create and populate four Silver tables.

| Table | Source Streams | Primary Key |
|-------|----------------|-------------|
| `silver.air_quality_observations` | air-quality | (observation_time, ndp_id) |
| `silver.weather_observations` | nws-observations, outdoor-weather | (observation_time, ndp_id) |
| `silver.weather_forecasts` | nws-forecast-hourly, nws-gridpoints-forecast | (issue_time, valid_time, ndp_id) |
| `silver.outdoor_air_quality` | outdoor-air-quality | (observation_time, ndp_id) |

**Acceptance Criteria**:
- [ ] All four tables created as TimescaleDB hypertables
- [ ] Schemas match data dictionary (03-data-dictionary.md)
- [ ] Primary keys enforce uniqueness
- [ ] Appropriate indexes created

#### FR-008: Weather Observations Merge

**Requirement**: The system SHALL merge NWS observations and OWM weather into a single `weather_observations` table with `source_provider` distinction.

**Rationale**: Domain-driven schema - both are outdoor weather observations, provider is metadata.

**Acceptance Criteria**:
- [ ] Both streams write to same table
- [ ] `source_provider` column indicates 'nws' or 'owm'
- [ ] Column naming aligned across providers
- [ ] Unit normalization applied (OWM Kelvin to Celsius, etc.)

#### FR-009: Forecast Domain Model

**Requirement**: The system SHALL implement forecast tables with issue_time, valid_time, and lead_time_hours.

**Rationale**: Lead time is critical dimension for forecast accuracy analysis.

**Acceptance Criteria**:
- [ ] `issue_time` captures when forecast was generated
- [ ] `valid_time` captures when forecast applies
- [ ] `lead_time_hours` is computed column: (valid_time - issue_time) / 3600
- [ ] Index on (lead_time_hours, valid_time) for accuracy queries

#### FR-010: Hypertable Configuration

**Requirement**: All Silver tables SHALL be TimescaleDB hypertables.

**Hypertable Parameters**:
| Table | Time Column | Chunk Interval |
|-------|-------------|----------------|
| air_quality_observations | observation_time | 1 day |
| weather_observations | observation_time | 1 day |
| weather_forecasts | valid_time | 1 day |
| outdoor_air_quality | observation_time | 1 day |

**Acceptance Criteria**:
- [ ] `create_hypertable()` called for each table
- [ ] Chunk interval set to 1 day
- [ ] Hypertable compression policies documented (future)

### 2.4 Transform Requirements

#### FR-011: Unit Normalization

**Requirement**: The system SHALL normalize units to SI standards during ETL.

**Standard Units**:
| Measurement | Silver Unit | Suffix |
|-------------|-------------|--------|
| Temperature | Celsius | _c |
| Humidity | Percent | _pct |
| Pressure | Pascals | _pa |
| Wind Speed | km/h | _kmh |
| Direction | Degrees | _deg |
| Visibility | Meters | _m |
| Precipitation | mm | _mm |

**Conversions Required**:
| Source | Field | From | To | Formula |
|--------|-------|------|----|---------|
| OWM Weather | temperature | Kelvin | Celsius | K - 273.15 |
| OWM Weather | wind.speed | m/s | km/h | m/s * 3.6 |
| OWM Weather | pressure | hPa | Pa | hPa * 100 |

**Acceptance Criteria**:
- [ ] All conversions documented in config
- [ ] Conversion formulas generate correct SQL
- [ ] Unit metadata stored (column comments or data dictionary)
- [ ] NWS data passes through (already SI units)

#### FR-012: Timestamp Handling

**Requirement**: The system SHALL correctly transform timestamps from various source formats.

**Supported Formats**:
| Format | Source | Transform |
|--------|--------|-----------|
| Microseconds | Bronze timestamp | to_timestamp(ts / 1000000) |
| ISO8601 | NWS properties.timestamp | Direct parse |
| Unix seconds | OWM list[0].dt | to_timestamp(dt) |
| NWS duration | valid duration | INTERVAL parse |

**Acceptance Criteria**:
- [ ] All timestamp formats parsed correctly
- [ ] Output is TIMESTAMPTZ in UTC
- [ ] Invalid timestamps produce NULL (not errors)
- [ ] Timezone handling documented

### 2.5 DQ Transparency Requirements

#### FR-013: DQ Flags Column

**Requirement**: All Silver tables SHALL include a `dq_flags TEXT[]` column for transparency.

**Flag Format**: `{rule}:{column}:{violation_type}`

**Examples**:
- `range_check:temperature_c:exceeded_max`
- `range_check:humidity_pct:clamped`
- `not_null:pm25:was_null`

**Acceptance Criteria**:
- [ ] dq_flags column on all Silver tables
- [ ] Flags populated when DQ rules trigger
- [ ] Flag format is parseable (colon-delimited)
- [ ] GIN index on dq_flags for querying

#### FR-014: DQ Monitoring Dashboard

**Requirement**: The system SHALL provide visibility into DQ flag patterns.

**Acceptance Criteria**:
- [ ] Query to count flags by rule/column/day
- [ ] Grafana panel showing DQ trends (future, out of scope)
- [ ] Documentation of expected flag patterns

### 2.6 Scheduling Requirements

#### FR-015: Systemd Timer Scheduling

**Requirement**: The system SHALL execute ETL via systemd timer.

**Schedule**: Hourly at 5 minutes past the hour

**Configuration**:
```ini
# silver-etl.timer
[Unit]
Description=Silver ETL Timer

[Timer]
OnCalendar=*-*-* *:05:00
Persistent=true

[Install]
WantedBy=timers.target
```

**Acceptance Criteria**:
- [ ] Timer unit file created
- [ ] Service unit file created
- [ ] `Persistent=true` handles missed runs
- [ ] Logs written to journald

#### FR-016: Incremental Processing

**Requirement**: The system SHALL process only new data since last ETL run.

**Implementation**:
1. Query max(observation_time) from Silver table
2. Read Bronze where timestamp > watermark - lag_interval
3. Insert/upsert into Silver

**Acceptance Criteria**:
- [ ] Only new data processed (not full reload)
- [ ] Watermark persisted between runs
- [ ] Late arrivals handled via lag_interval
- [ ] Full reload available via flag (--full-reload)

### 2.7 Deployment Requirements

#### FR-017: TimescaleDB Container

**Requirement**: The system SHALL deploy TimescaleDB as a Docker container.

**Container Configuration**:
| Setting | Value | Rationale |
|---------|-------|-----------|
| Memory limit | 256MB | Pi resource budget |
| Port | 5432 | Standard PostgreSQL |
| Volume | /data/silver | Persistent storage |
| Image | timescale/timescaledb:latest-pg15 | Latest stable |

**Acceptance Criteria**:
- [ ] docker-compose.yml updated
- [ ] Memory limit enforced
- [ ] Data persisted across restarts
- [ ] Health check configured

#### FR-018: Grafana Datasource

**Requirement**: The system SHALL provision TimescaleDB as a Grafana datasource.

**Acceptance Criteria**:
- [ ] Datasource provisioning file created
- [ ] Connection tested from Grafana
- [ ] Variables for common filters (ndp_id, time range)
- [ ] Sample dashboard query works

---

## 3. Non-Functional Requirements

### 3.1 Performance Requirements

#### NFR-001: ETL Latency

**Requirement**: Hourly ETL batch SHALL complete within 60 seconds.

**Measurement**: Time from ETL start to completion for hourly batch.

**Target**: < 60 seconds (p95)

#### NFR-002: Data Freshness

**Requirement**: Silver data SHALL lag Bronze by no more than 5 minutes during normal operation.

**Measurement**: max(Silver.ingestion_time) - max(Bronze.timestamp)

**Target**: < 5 minutes (excluding scheduled batch delay)

#### NFR-003: Memory Usage

**Requirement**: silver-etl process SHALL not exceed 300MB peak memory.

**Measurement**: RSS memory during ETL execution.

**Target**: < 300MB peak

### 3.2 Reliability Requirements

#### NFR-004: Bronze Independence

**Requirement**: Silver ETL failures SHALL NOT impact Bronze ingestion.

**Verification**: Bronze continues writing during Silver downtime.

#### NFR-005: Idempotent ETL

**Requirement**: Re-running ETL on same time range SHALL produce identical results.

**Verification**: Run ETL twice, compare Silver state.

#### NFR-006: Recovery from Failure

**Requirement**: System SHALL resume ETL after process restart.

**Verification**: Kill ETL mid-run, restart, verify completion.

### 3.3 Maintainability Requirements

#### NFR-007: Config-Only Stream Addition

**Requirement**: Adding a new stream to Silver SHALL require only YAML configuration changes.

**Verification**: Document procedure, verify no Rust code changes needed.

#### NFR-008: Schema Evolution

**Requirement**: Schema changes SHALL be applied via migration without data loss.

**Verification**: Add column to schema, verify existing data preserved.

### 3.4 Observability Requirements

#### NFR-009: ETL Metrics

**Requirement**: System SHALL expose metrics for monitoring.

**Metrics**:
| Metric | Type | Description |
|--------|------|-------------|
| `silver_etl_rows_processed` | Counter | Rows processed per stream |
| `silver_etl_duration_seconds` | Histogram | ETL execution time |
| `silver_etl_errors` | Counter | ETL errors by type |
| `silver_etl_dq_flags` | Counter | DQ flags by rule |

**Target**: Metrics available in Prometheus format.

#### NFR-010: Logging

**Requirement**: System SHALL log ETL progress and errors.

**Log Levels**:
- INFO: ETL start/complete, rows processed
- WARN: DQ flags triggered, retries
- ERROR: ETL failures, connection errors

---

## 4. Interface Requirements

### 4.1 Configuration Interface

#### IR-001: Config Loading

**Input**: Stream configuration from etcd or YAML files
**Output**: Validated SilverEtlConfig struct

**Error Handling**:
- Invalid config: Log error, skip stream, continue with others
- Missing required field: Clear error message with field path

### 4.2 Data Interfaces

#### IR-002: Bronze Data Input

**Format**: Parquet files
**Location**: `/data/raw/{stream-id}/**/*.parquet`
**Schema**: timestamp, source_id, ndp_id, context (JSON), raw_payload (JSON)

#### IR-003: Silver Data Output

**Format**: TimescaleDB tables
**Location**: `silver.{table_name}` schema
**Schema**: Per data dictionary (03-data-dictionary.md)

### 4.3 External Interfaces

#### IR-004: etcd Configuration

**Endpoints**: etcd cluster (2379)
**Keys**: `/streams/{stream-id}/config`
**Watch**: Config changes trigger reload

#### IR-005: TimescaleDB Connection

**Protocol**: PostgreSQL (libpq)
**Port**: 5432
**Authentication**: Password-based (from env)

---

## 5. Acceptance Criteria Summary

### 5.1 Deliverable Acceptance Criteria

| Deliverable | Criteria |
|-------------|----------|
| **silver-etl binary** | Compiles, runs independently, processes all 4 tables |
| **Config types** | Serde tests pass, validation errors clear |
| **Stream configs** | 4 streams have silver_etl sections |
| **TimescaleDB schema** | All tables created, hypertables configured |
| **Systemd units** | Timer runs hourly, service logs correctly |
| **Grafana datasource** | Connection works, sample query succeeds |

### 5.2 Integration Test Scenarios

| Scenario | Description | Pass Criteria |
|----------|-------------|---------------|
| **Happy Path** | Process 1 hour of data | All rows in Silver, no errors |
| **DQ Violations** | Process data with out-of-range values | dq_flags populated, values handled per action |
| **Late Arrivals** | Data arrives after watermark | Lag interval catches late data |
| **Recovery** | Kill and restart ETL | Resumes from watermark |
| **New Stream** | Add stream via config only | Data appears in Silver |
| **Schema Evolution** | Add column to config | Migration applied, data preserved |

### 5.3 Performance Test Scenarios

| Scenario | Input | Pass Criteria |
|----------|-------|---------------|
| **Hourly Batch** | 1 hour of 7 streams | < 60 seconds |
| **Memory Limit** | Full ETL run | < 300MB peak |
| **Backfill 24h** | 24 hours of data | Completes without OOM |

---

## 6. Dependencies

### 6.1 External Dependencies

| Dependency | Version | Notes |
|------------|---------|-------|
| duckdb-rs | ^1.1 | bundled feature for ARM64 |
| TimescaleDB | 2.x on PG15 | Docker image |
| etcd | 3.x | Existing deployment |

### 6.2 Internal Dependencies

| Component | Dependency Type | Notes |
|-----------|-----------------|-------|
| config-client | Library | etcd access |
| Bronze Parquet files | Data | Input source |
| stream configs | Configuration | Extended with silver_etl |

### 6.3 Documentation Dependencies

| Document | Dependency Type | Notes |
|----------|-----------------|-------|
| 03-data-dictionary.md | Schema reference | Column types, DQ rules |
| CONFIG_DRIVEN_SILVER_ETL_DESIGN.md | Architecture reference | Config schema design |
| Stream config files | Configuration reference | Existing field mappings |

---

## 7. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| duckdb-rs postgres extension issues on ARM64 | Low | High | Fallback: Polars + tokio-postgres |
| TimescaleDB memory pressure on Pi | Low | Medium | 256MB limit, monitoring |
| Config schema changes break Bronze | Low | High | Separate config section, versioning |
| ETL takes too long for hourly cadence | Low | Medium | Optimize queries, increase interval |

---

## 8. Open Questions for Architecture Phase

| Question | Owner | Target ADR |
|----------|-------|------------|
| Confirm duckdb-rs as ETL engine | ndp-architect | ADR-006-001 |
| Confirm separate binary architecture | ndp-architect | ADR-006-002 |
| Finalize schema naming convention | ndp-architect | ADR-006-003 |
| Confirm DQ rule action defaults | ndp-dq-engineer | ADR-006-004 |
| Confirm systemd timer vs embedded scheduler | ndp-architect | ADR-006-005 |
| Confirm stream_type field for future events | ndp-architect | ADR-006-006 |

---

## 9. Glossary

| Term | Definition |
|------|------------|
| **Bronze** | Raw data layer (Parquet), schema-on-read |
| **Silver** | Clean data layer (TimescaleDB), schema-on-write |
| **Gold** | Feature/ML layer, derived from Silver |
| **DQ** | Data Quality |
| **ETL** | Extract, Transform, Load |
| **Hypertable** | TimescaleDB time-partitioned table |
| **Watermark** | High-water mark for incremental processing |
| **Lead time** | Hours between forecast issue and valid time |

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP Scrum Master | Initial specification |

---

## References

1. SCOPE.md - Feature scope definition
2. 03-data-dictionary.md - Silver schema definitions
3. CONFIG_DRIVEN_SILVER_ETL_DESIGN.md - Config schema design
4. AgentDB patterns - arch-config-driven-silver-etl, arch-data-lake-layers
