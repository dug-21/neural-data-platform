# DP-002 System Design Document

**Feature**: Online Data Dictionary and HomeAssistant Stream Preparation
**Date**: 2025-12-30
**Author**: NDP Architecture Team
**Status**: Proposed

---

## 1. Executive Summary

DP-002 introduces an **Online Data Dictionary** that provides queryable metadata about all NDP streams, enabling:

1. **Schema-driven Grafana dashboards** that auto-update when streams change
2. **Entity schema configuration** for HomeAssistant pattern-based ingestion
3. **Data quality monitoring** with freshness and volume metrics
4. **Documentation** for ML pipelines and analytics consumers

This document describes the end-to-end system design, component interactions, and deployment topology.

---

## 2. Component Diagram

```
+===========================================================================+
|                        NEURAL DATA PLATFORM (DP-002)                       |
+===========================================================================+

  CONFIG LAYER (Source of Truth)
  ==============================

  +------------------+          +------------------+
  | YAML Config      |          | etcd             |
  | config/base/     |  sync    | /streams/        |
  | streams/         |--------->| {stream-id}/     |
  |   config.yaml    |          | config           |
  +------------------+          +------------------+
         |                              |
         | deploy.sh sync               | watch API
         v                              v
  +------------------+          +------------------+
  | deploy.sh        |          | air-quality-app  |
  | (sync command)   |          | (config-client)  |
  +------------------+          +------------------+
         |
         | SQL transaction
         v

  SILVER LAYER (Queryable Storage)
  =================================

  +------------------------------------------------------------------+
  |                     TimescaleDB Container                         |
  |                        (pi5-timescaledb)                         |
  |                                                                   |
  |  +------------------------+    +------------------------+        |
  |  | data_dictionary schema |    | analytics schema       |        |
  |  |------------------------|    |------------------------|        |
  |  | streams                |    | air_quality            |        |
  |  | fields                 |    | home_events            |        |
  |  | sources                |    | weather (future)       |        |
  |  | entity_schemas         |    | (hypertables)          |        |
  |  | sync_status            |    |                        |        |
  |  +------------------------+    +------------------------+        |
  |                                                                   |
  +------------------------------------------------------------------+
         |
         | PostgreSQL queries
         v

  PRESENTATION LAYER (Dashboards)
  ================================

  +------------------------------------------------------------------+
  |                     Grafana Container                             |
  |                        (pi5-grafana)                             |
  |                                                                   |
  |  +------------------------+    +------------------------+        |
  |  | Data Quality Dashboard |    | Analytics Dashboards   |        |
  |  |------------------------|    |------------------------|        |
  |  | Stream Overview        |    | Air Quality            |        |
  |  | Field Details          |    | Home Events            |        |
  |  | Entity Schemas         |    | Cross-Stream           |        |
  |  | Freshness Metrics      |    | Correlation            |        |
  |  +------------------------+    +------------------------+        |
  |                                                                   |
  +------------------------------------------------------------------+

  BRONZE LAYER (Raw Storage)
  ===========================

  +------------------------------------------------------------------+
  |                     Parquet Files (Volume)                        |
  |                     data/bronze/{stream-id}/YYYY/MM/DD/           |
  |                                                                   |
  |  +------------------+  +------------------+  +------------------+ |
  |  | air-quality/     |  | home-events/     |  | weather/         | |
  |  | 2025/12/30/      |  | 2025/12/30/      |  | 2025/12/30/      | |
  |  | *.parquet        |  | *.parquet        |  | *.parquet        | |
  |  +------------------+  +------------------+  +------------------+ |
  |                                                                   |
  +------------------------------------------------------------------+
         ^
         | write
         |
  +------------------+
  | air-quality-app  |
  | (ingestion)      |
  +------------------+
         ^
         | ingest
         |
  +------------------+          +------------------+
  | MQTT (Mosquitto) |          | HTTP Polling     |
  | - AirGradient    |          | - NWS Weather    |
  | - HomeAssistant  |          | - Other APIs     |
  +------------------+          +------------------+
```

---

## 3. Data Flow

### 3.1 Configuration Flow

```
1. Developer edits YAML
   config/base/streams/home-events/config.yaml

2. Deploy sync command
   $ ./deploy.sh sync

3. YAML synced to etcd
   etcdctl put /streams/home-events/config "<yaml-content>"

4. YAML synced to TimescaleDB (NEW in DP-002)
   INSERT INTO data_dictionary.streams ...
   INSERT INTO data_dictionary.fields ...
   INSERT INTO data_dictionary.entity_schemas ...

5. Grafana queries Data Dictionary
   SELECT * FROM data_dictionary.stream_overview

6. Dashboard displays updated schema
   (No JSON edits required)
```

### 3.2 Ingestion Flow (Unchanged)

```
1. Sensor publishes to MQTT
   airgradient/readings/abc123 -> {"pm25": 12.5, "temperature": 22.3}

2. air-quality-app subscribes
   MqttSource receives message

3. Parser converts to TimeSeriesPoint
   HomeAssistantParser uses entity_schemas for matching (NEW)

4. Writer batches and stores
   ParquetStore.write_batch() -> data/bronze/air-quality/2025/12/30/

5. ETL to Silver (future DP-002 phase)
   Bronze Parquet -> TimescaleDB analytics tables
```

### 3.3 Dashboard Query Flow

```
1. User opens Data Quality Dashboard in Grafana

2. Grafana sends SQL to TimescaleDB
   SELECT * FROM data_dictionary.stream_overview

3. TimescaleDB returns results
   [{stream_id: "air-quality", field_count: 8, ...}, ...]

4. Grafana renders table panel
   Shows all streams with field counts

5. User selects stream from dropdown
   Variable: stream_id = "home-events"

6. Field panel queries with variable
   SELECT * FROM data_dictionary.fields WHERE stream_id = 'home-events'

7. Grafana renders field table
   Shows fields for selected stream
```

---

## 4. Component Specifications

### 4.1 TimescaleDB Container

| Property | Value |
|----------|-------|
| **Image** | timescale/timescaledb:latest-pg15 |
| **Container Name** | pi5-timescaledb |
| **Port** | 5432 (internal), optionally exposed |
| **Memory Limit** | 256MB |
| **Database** | ndp |
| **Schemas** | data_dictionary, analytics |
| **Volumes** | timescaledb_data:/var/lib/postgresql/data |

### 4.2 Data Dictionary Schema

See ADR-001 for full DDL. Key tables:

| Table | Purpose | Primary Key |
|-------|---------|-------------|
| `streams` | Stream metadata | stream_id |
| `fields` | Field definitions | id (serial) |
| `sources` | Source configurations | id (serial) |
| `entity_schemas` | Pattern-based entity configs | id (serial) |
| `sync_status` | Sync audit trail | id (serial) |

### 4.3 deploy.sh Sync Command

| Function | Purpose |
|----------|---------|
| `sync_to_etcd()` | Existing - sync YAML to etcd |
| `sync_to_data_dictionary()` | NEW - sync YAML to TimescaleDB |
| `generate_sync_sql()` | NEW - Generate SQL from YAML |
| `sql_escape()` | NEW - Escape strings for SQL |

### 4.4 Grafana Data Quality Dashboard

| Panel | Data Source | Query Type |
|-------|-------------|------------|
| Stream Overview | TimescaleDB | Table (stream_overview view) |
| Field Details | TimescaleDB | Table (fields filtered by variable) |
| Entity Schemas | TimescaleDB | Table (entity_schema_details view) |
| Data Freshness | DuckDB/TimescaleDB | Stat (max timestamp per stream) |
| Volume Metrics | DuckDB | Time series (hourly record counts) |

---

## 5. Integration Points

### 5.1 Existing Components (No Changes Required)

| Component | Integration |
|-----------|-------------|
| `air-quality-app` | Continues to read config from etcd; no TimescaleDB dependency |
| `config-client` | Unchanged; used by app for runtime config |
| `ParquetStore` | Unchanged; writes Bronze layer |
| `MqttSource` | Unchanged; ingests sensor data |
| `HttpPollingSource` | Unchanged; polls external APIs |

### 5.2 New Components

| Component | Description |
|-----------|-------------|
| `TimescaleDB container` | New service in docker-compose |
| `sync_to_data_dictionary` | New function in deploy.sh |
| `Data Quality Dashboard` | New Grafana dashboard |
| `timescaledb.yaml` | New Grafana data source config |

### 5.3 Modified Components

| Component | Modification |
|-----------|--------------|
| `deploy.sh` | Add sync_to_data_dictionary call |
| `docker-compose.yml` | Add TimescaleDB service, remove DuckDB |
| `Grafana provisioning` | Add TimescaleDB data source |

---

## 6. Deployment Topology

### 6.1 Raspberry Pi 5 (Production)

```
Raspberry Pi 5 (8GB RAM)
|
+-- Docker Compose Stack
    |
    +-- pi5-mosquitto        (128MB)  - MQTT broker
    |
    +-- pi5-etcd             (256MB)  - Configuration store
    |
    +-- pi5-air-quality-app  (512MB)  - Ingestion application
    |
    +-- pi5-timescaledb      (256MB)  - Silver layer + Data Dictionary
    |
    +-- pi5-grafana          (256MB)  - Dashboards
    |
    +-- Volume: parquet_data         - Bronze layer storage
    +-- Volume: timescaledb_data     - TimescaleDB data
    +-- Volume: etcd_data            - etcd persistence
    +-- Volume: grafana_data         - Grafana state

Total Memory: ~1.4GB (within 1.7GB budget)
```

### 6.2 Development (Codespace/Local)

```
Development Container
|
+-- Docker Compose (dev profile)
    |
    +-- Same services as production
    |
    +-- Exposed ports for debugging:
        - 1883 (MQTT)
        - 2379 (etcd)
        - 5432 (TimescaleDB)
        - 3000 (Grafana)
        - 8080 (air-quality-app API)
```

---

## 7. Security Considerations

### 7.1 Database Access

| User | Purpose | Permissions |
|------|---------|-------------|
| postgres | Admin | Superuser (internal only) |
| ndp_app | Application | SELECT, INSERT on analytics |
| grafana_reader | Dashboards | SELECT on data_dictionary |

### 7.2 Network Isolation

```yaml
# docker-compose.yml
networks:
  neural-network:
    driver: bridge
    internal: false  # Allow external access for MQTT, Grafana

# Only expose necessary ports
services:
  timescaledb:
    ports:
      - "127.0.0.1:5432:5432"  # Localhost only in production
```

### 7.3 Secrets Management

| Secret | Storage | Usage |
|--------|---------|-------|
| `POSTGRES_PASSWORD` | .env file | TimescaleDB admin |
| `GRAFANA_TIMESCALE_PASSWORD` | .env file | Grafana read access |
| `HASS_ACCESS_TOKEN` | .env file | HomeAssistant (future) |

---

## 8. Error Handling

### 8.1 Sync Failures

| Failure Mode | Detection | Recovery |
|--------------|-----------|----------|
| YAML parse error | yq exit code | Log error, abort sync |
| SQL syntax error | psql exit code | Rollback transaction, log error |
| TimescaleDB unavailable | Connection timeout | Retry with backoff, alert if persistent |
| Partial sync | Transaction rollback | Automatic (atomic transaction) |

### 8.2 Query Failures

| Failure Mode | Detection | User Experience |
|--------------|-----------|-----------------|
| TimescaleDB down | Grafana error | Panel shows error message |
| Slow query | Timeout | Panel shows timeout error |
| Empty result | Query returns 0 rows | Panel shows "No data" |

---

## 9. Monitoring

### 9.1 Sync Metrics

| Metric | Source | Alert Threshold |
|--------|--------|-----------------|
| Last sync time | sync_status table | > 24 hours since successful sync |
| Sync duration | sync_status timestamps | > 60 seconds |
| Sync failures | sync_status.status = 'failed' | Any failure |

### 9.2 Dashboard Health

| Metric | Source | Alert Threshold |
|--------|--------|-----------------|
| Stream count | stream_overview | Unexpected change |
| Field count per stream | stream_overview | Unexpected decrease |
| Data freshness | Last timestamp query | > 15 minutes stale |

---

## 10. Performance Considerations

### 10.1 Query Performance

| Query | Expected Latency | Index Used |
|-------|------------------|------------|
| stream_overview | < 10ms | Primary keys |
| fields by stream_id | < 5ms | idx_fields_stream_id |
| entity_schemas by pattern | < 10ms | idx_entity_schemas_pattern |

### 10.2 Resource Usage

| Operation | Memory | CPU | Duration |
|-----------|--------|-----|----------|
| Full sync (10 streams) | ~50MB | Low | < 5s |
| Dashboard refresh | ~10MB | Low | < 1s |
| TimescaleDB idle | ~100MB | Minimal | Continuous |

---

## 11. Testing Strategy

### 11.1 Unit Tests

| Component | Test Type |
|-----------|-----------|
| SQL generation | Shell function tests |
| YAML parsing | yq output validation |
| SQL syntax | Dry run with psql |

### 11.2 Integration Tests

| Test | Description |
|------|-------------|
| End-to-end sync | YAML -> etcd -> TimescaleDB -> Grafana |
| Dashboard queries | Verify panels return expected data |
| Schema changes | Add field, verify dashboard updates |

### 11.3 Acceptance Criteria

| Criterion | Verification |
|-----------|--------------|
| No JSON edits | Add stream via YAML only, verify dashboard shows it |
| < 10ms query latency | Measure Grafana panel load time |
| Atomic sync | Verify no partial state on failure |

---

## 12. Migration Path

### 12.1 Phase 1: TimescaleDB Deployment

1. Add TimescaleDB to docker-compose.yml
2. Remove DuckDB container (replaced by TimescaleDB)
3. Create data_dictionary schema on startup
4. Verify container health

### 12.2 Phase 2: Sync Implementation

1. Add sync_to_data_dictionary to deploy.sh
2. Run initial sync
3. Verify data in TimescaleDB

### 12.3 Phase 3: Dashboard Deployment

1. Add TimescaleDB data source to Grafana
2. Deploy Data Quality dashboard
3. Verify panels display correctly

### 12.4 Phase 4: Validation

1. Add new stream via YAML
2. Run deploy.sh sync
3. Verify dashboard shows new stream
4. Confirm no JSON edits were needed

---

## 13. Related Documents

| Document | Description |
|----------|-------------|
| [ADR-001-TIMESCALEDB-SCHEMA.md](./ADR-001-TIMESCALEDB-SCHEMA.md) | Database schema design |
| [ADR-002-ENTITY-SCHEMA-FORMAT.md](./ADR-002-ENTITY-SCHEMA-FORMAT.md) | Entity schema configuration |
| [ADR-003-SYNC-MECHANISM.md](./ADR-003-SYNC-MECHANISM.md) | Sync implementation details |
| [ADR-004-DQ-DASHBOARD.md](./ADR-004-DQ-DASHBOARD.md) | Dashboard architecture |
| [DOCKER_CHANGES.md](./DOCKER_CHANGES.md) | Docker configuration changes |

---

**Last Updated**: 2025-12-30
**Next Review**: After Phase 1 deployment
