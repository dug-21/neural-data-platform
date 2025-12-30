# ADR-001: TimescaleDB Schema Design for Data Dictionary

**Status**: Proposed
**Date**: 2025-12-30
**Decision Makers**: NDP Architecture Team
**Context**: DP-002 Online Data Dictionary and HomeAssistant Stream Preparation
**Supersedes**: None

---

## Context

DP-002 introduces an **Online Data Dictionary** that provides queryable metadata about all NDP streams, fields, entities, and their schemas. The Data Dictionary serves multiple purposes:

1. **Grafana Data Quality Dashboards**: Enable dashboard panels to dynamically query what streams/fields exist
2. **Schema Discovery**: Allow ML pipelines and analytics to understand available data
3. **Documentation**: Provide human-readable metadata for streams and fields
4. **HomeAssistant Preparation**: Store entity schemas for pattern-based home event ingestion

The data dictionary must be stored in the **Silver layer** for real-time queryability. The primary options are:

- **TimescaleDB**: Time-series optimized PostgreSQL (recommended for Silver layer per ADR-001 Multi-Stream)
- **DuckDB**: In-process OLAP database (current Silver layer for analytics)
- **PostgreSQL**: Standard relational database

### Technical Constraints

1. **Raspberry Pi 5 Resources**: Total memory budget ~1.7GB across all services
2. **Query Latency**: Sub-100ms for dashboard queries
3. **Update Frequency**: Schema changes are rare (minutes/hours, not seconds)
4. **Integration**: Must work with Grafana's PostgreSQL/TimescaleDB data source

---

## Decision

**Use TimescaleDB with normalized schema design for the Data Dictionary.**

### Schema Design

```sql
-- =============================================================================
-- DATA DICTIONARY SCHEMA (Silver Layer - TimescaleDB)
-- =============================================================================

-- 1. STREAMS TABLE
-- Stores stream-level metadata
CREATE TABLE data_dictionary.streams (
    stream_id           TEXT PRIMARY KEY,
    description         TEXT,
    version             TEXT NOT NULL DEFAULT '1.0.0',
    enabled             BOOLEAN NOT NULL DEFAULT true,
    retention_days      INTEGER DEFAULT 90,
    partitioning_strategy TEXT DEFAULT 'daily',
    compression_after_days INTEGER DEFAULT 7,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata            JSONB
);

-- 2. FIELDS TABLE
-- Stores field-level schema definitions
CREATE TABLE data_dictionary.fields (
    id                  SERIAL PRIMARY KEY,
    stream_id           TEXT NOT NULL REFERENCES data_dictionary.streams(stream_id) ON DELETE CASCADE,
    field_name          TEXT NOT NULL,
    field_type          TEXT NOT NULL,  -- String, Int, Float, Bool, Json
    nullable            BOOLEAN NOT NULL DEFAULT true,
    unit                TEXT,
    description         TEXT,
    validation_min      DOUBLE PRECISION,
    validation_max      DOUBLE PRECISION,
    validation_pattern  TEXT,  -- Regex pattern for string validation
    sort_order          INTEGER NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(stream_id, field_name)
);

-- 3. SOURCES TABLE
-- Stores source configurations per stream
CREATE TABLE data_dictionary.sources (
    id                  SERIAL PRIMARY KEY,
    stream_id           TEXT NOT NULL REFERENCES data_dictionary.streams(stream_id) ON DELETE CASCADE,
    source_id           TEXT NOT NULL,
    source_type         TEXT NOT NULL,  -- mqtt, http_poll, home_assistant
    enabled             BOOLEAN NOT NULL DEFAULT true,
    config              JSONB NOT NULL,  -- Source-specific configuration
    parser_type         TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(stream_id, source_id)
);

-- 4. ENTITY SCHEMAS TABLE (NEW for DP-002)
-- Stores entity-level schema definitions for pattern-based streams (HomeAssistant)
CREATE TABLE data_dictionary.entity_schemas (
    id                  SERIAL PRIMARY KEY,
    stream_id           TEXT NOT NULL REFERENCES data_dictionary.streams(stream_id) ON DELETE CASCADE,
    entity_pattern      TEXT NOT NULL,  -- Glob pattern: "binary_sensor.*_window*"
    entity_domain       TEXT NOT NULL,  -- binary_sensor, sensor, switch, etc.
    device_class        TEXT,           -- window, door, motion, temperature, etc.
    unit_of_measurement TEXT,
    state_mapping       JSONB,          -- {"on": 1.0, "off": 0.0} for numeric conversion
    description         TEXT,
    protocol            TEXT,           -- matter_thread, zigbee, zwave, wifi
    enabled             BOOLEAN NOT NULL DEFAULT true,
    priority            INTEGER NOT NULL DEFAULT 0,  -- Higher priority patterns match first
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 5. SYNC STATUS TABLE
-- Tracks synchronization state between etcd and TimescaleDB
CREATE TABLE data_dictionary.sync_status (
    id                  SERIAL PRIMARY KEY,
    sync_type           TEXT NOT NULL,  -- 'full', 'incremental'
    started_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at        TIMESTAMPTZ,
    status              TEXT NOT NULL DEFAULT 'running',  -- 'running', 'success', 'failed'
    streams_synced      INTEGER DEFAULT 0,
    fields_synced       INTEGER DEFAULT 0,
    entities_synced     INTEGER DEFAULT 0,
    error_message       TEXT,
    etcd_revision       BIGINT  -- etcd revision number for incremental sync
);

-- =============================================================================
-- INDEXES
-- =============================================================================

-- Primary query patterns for Grafana dashboards
CREATE INDEX idx_fields_stream_id ON data_dictionary.fields(stream_id);
CREATE INDEX idx_sources_stream_id ON data_dictionary.sources(stream_id);
CREATE INDEX idx_entity_schemas_stream_id ON data_dictionary.entity_schemas(stream_id);
CREATE INDEX idx_entity_schemas_pattern ON data_dictionary.entity_schemas(entity_pattern);
CREATE INDEX idx_entity_schemas_domain ON data_dictionary.entity_schemas(entity_domain);

-- GIN index for JSONB queries (entity_schema attributes, source config)
CREATE INDEX idx_sources_config ON data_dictionary.sources USING GIN (config);
CREATE INDEX idx_entity_schemas_state_mapping ON data_dictionary.entity_schemas USING GIN (state_mapping);

-- =============================================================================
-- VIEWS FOR COMMON QUERIES
-- =============================================================================

-- Complete stream overview with field counts
CREATE VIEW data_dictionary.stream_overview AS
SELECT
    s.stream_id,
    s.description,
    s.version,
    s.enabled,
    s.retention_days,
    COUNT(DISTINCT f.id) AS field_count,
    COUNT(DISTINCT src.id) AS source_count,
    COUNT(DISTINCT e.id) AS entity_schema_count,
    s.created_at,
    s.updated_at
FROM data_dictionary.streams s
LEFT JOIN data_dictionary.fields f ON s.stream_id = f.stream_id
LEFT JOIN data_dictionary.sources src ON s.stream_id = src.stream_id
LEFT JOIN data_dictionary.entity_schemas e ON s.stream_id = e.stream_id
GROUP BY s.stream_id, s.description, s.version, s.enabled,
         s.retention_days, s.created_at, s.updated_at;

-- Field details with stream context
CREATE VIEW data_dictionary.field_details AS
SELECT
    s.stream_id,
    s.description AS stream_description,
    s.enabled AS stream_enabled,
    f.field_name,
    f.field_type,
    f.nullable,
    f.unit,
    f.description AS field_description,
    f.validation_min,
    f.validation_max,
    f.sort_order
FROM data_dictionary.streams s
JOIN data_dictionary.fields f ON s.stream_id = f.stream_id
ORDER BY s.stream_id, f.sort_order, f.field_name;

-- Entity schemas with matching info
CREATE VIEW data_dictionary.entity_schema_details AS
SELECT
    e.id,
    s.stream_id,
    e.entity_pattern,
    e.entity_domain,
    e.device_class,
    e.unit_of_measurement,
    e.state_mapping,
    e.protocol,
    e.enabled,
    e.priority,
    e.description
FROM data_dictionary.entity_schemas e
JOIN data_dictionary.streams s ON e.stream_id = s.stream_id
ORDER BY s.stream_id, e.priority DESC, e.entity_pattern;
```

### TimescaleDB Specific Configuration

The Data Dictionary tables are **not hypertables** because they don't contain time-series data. However, we leverage TimescaleDB for:

1. **Existing Infrastructure**: TimescaleDB is already planned for Silver layer analytics
2. **Grafana Integration**: Native PostgreSQL/TimescaleDB data source
3. **JSON Support**: Full PostgreSQL JSONB with GIN indexes for flexible metadata

### Memory Budget

```
TimescaleDB container: 256MB (shared with analytics hypertables)
Data Dictionary tables: ~10-50MB estimated for 10-20 streams
Index overhead: ~5-10MB
```

---

## Rationale

### Why Normalized Tables Over Single Denormalized Table

**Considered Alternative**: Single `stream_metadata` table with all data in JSONB

```sql
-- REJECTED: Single denormalized table
CREATE TABLE data_dictionary.metadata (
    stream_id TEXT PRIMARY KEY,
    config JSONB,  -- Everything in one blob
    created_at TIMESTAMPTZ
);
```

**Rejected because**:
1. **Query Inefficiency**: Cannot use indexes for field-level queries
2. **No Referential Integrity**: Cannot enforce FK relationships
3. **Update Complexity**: Full document replacement for any change
4. **Dashboard Queries**: Grafana needs flat relational data for Table panels

**Normalized Benefits**:
1. **Efficient Field Queries**: Direct `SELECT * FROM fields WHERE stream_id = ...`
2. **Referential Integrity**: Cascade deletes, FK constraints
3. **Partial Updates**: Update single field without rewriting entire config
4. **Standard SQL**: Works with any SQL tool, not just JSONB-aware

### Why TimescaleDB Over DuckDB

| Criterion | TimescaleDB | DuckDB |
|-----------|-------------|--------|
| **Write Latency** | <10ms | Good (file-based) |
| **Concurrent Access** | Full ACID | Limited |
| **Grafana Support** | Native data source | Requires plugin |
| **Real-time Updates** | Immediate | Requires refresh |
| **Memory Footprint** | ~256MB shared | ~512MB |

**Decision**: TimescaleDB is already planned for Silver layer analytics (ADR-001 Multi-Stream). Data Dictionary shares this instance.

---

## Consequences

### Positive

1. **Query Performance**: Indexed normalized tables enable <10ms lookups
2. **Grafana Compatibility**: PostgreSQL data source works out-of-box
3. **Schema Evolution**: Add columns with migrations, not schema redesign
4. **Referential Integrity**: Database enforces stream-field relationships
5. **Entity Pattern Support**: Dedicated table for HomeAssistant entity schemas

### Negative

1. **DDL Migrations**: Schema changes require SQL migrations
2. **Sync Complexity**: Need to transform YAML to relational inserts
3. **Memory Overhead**: TimescaleDB shared memory adds ~256MB

### Risks

1. **Resource Contention**: Data Dictionary queries could impact analytics
   - **Mitigation**: Separate schema (`data_dictionary`) with monitoring
2. **Sync Drift**: etcd and TimescaleDB could diverge
   - **Mitigation**: Sync status table, periodic full sync

---

## Alternatives Considered

### Alternative 1: DuckDB-Only (No TimescaleDB)

Store Data Dictionary as Parquet files read by DuckDB.

**Pros**:
- No additional database
- Simpler deployment

**Cons**:
- Poor concurrent write support
- Grafana requires DuckDB plugin
- Not suitable for frequent updates

**Verdict**: Rejected - Grafana integration critical

### Alternative 2: PostgreSQL (No TimescaleDB Extension)

Use standard PostgreSQL without TimescaleDB.

**Pros**:
- Slightly lower memory
- Simpler extension management

**Cons**:
- Loses hypertable capability for analytics
- No continuous aggregates for metrics
- Would need two databases (PG + something for time-series)

**Verdict**: Rejected - TimescaleDB already planned

### Alternative 3: SQLite

Embedded database for Data Dictionary.

**Pros**:
- Zero configuration
- Minimal memory

**Cons**:
- No concurrent writes from multiple processes
- No native Grafana data source
- Separate from analytics layer

**Verdict**: Rejected - Integration limitations

---

## Implementation Impact

### Files to Create

- `deploy/pi/init-scripts/01-create-data-dictionary.sql` - Schema DDL
- `deploy/pi/init-scripts/02-seed-existing-streams.sql` - Initial data

### Files to Modify

- `deploy/pi/docker-compose.yml` - Add TimescaleDB container (per DP-002 plan)
- `deploy/pi/deploy.sh` - Add sync command

### Migration Strategy

1. **Phase 1**: Create schema on TimescaleDB startup (init script)
2. **Phase 2**: Run full sync from existing YAML configs
3. **Phase 3**: Enable incremental sync via deploy.sh

---

## Related Decisions

- **ADR-001 (AIR-004)**: Multi-Stream Architecture Foundation
- **ADR-002 (DP-002)**: Entity Schema Format (this series)
- **ADR-003 (DP-002)**: Sync Mechanism (this series)

---

## References

- [TimescaleDB Schema Design](https://docs.timescale.com/timescaledb/latest/overview/core-concepts/hypertables/)
- [PostgreSQL JSONB Indexing](https://www.postgresql.org/docs/current/datatype-json.html#JSON-INDEXING)
- [Grafana PostgreSQL Data Source](https://grafana.com/docs/grafana/latest/datasources/postgres/)

---

**Last Updated**: 2025-12-30
**Next Review**: After Phase 1 implementation (sync command deployed)
