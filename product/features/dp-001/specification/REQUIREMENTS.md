# DP-001 Requirements Specification
## DuckDB Analytics Layer + Grafana Dashboards

**Status**: Draft
**Created**: 2025-12-18
**Author**: NDP Architect
**Feature**: dp-001

---

## 1. Overview

This specification defines the requirements for adding an analytics and visualization layer to the Neural Data Platform using DuckDB and Grafana. The solution provides read-only analytical access to existing Parquet files without modifying the current Rust ingestion pipeline.

### 1.1 Goals

- Enable SQL-based analytics on Bronze layer Parquet files
- Provide interactive dashboards for air quality monitoring
- Support cross-stream analysis (indoor + outdoor correlation)
- Maintain resource efficiency on Raspberry Pi 5 hardware

### 1.2 Non-Goals

- Modifying existing Rust ingestion pipeline
- Writing or transforming Parquet files
- User authentication/authorization
- Real-time streaming analytics
- External API exposure

---

## 2. Functional Requirements

### FR-001: DuckDB Parquet File Access

**Priority**: P0 (Critical)

DuckDB MUST be able to read Parquet files from all three existing data streams:

- `/data/air-quality/*.parquet` - Indoor sensor data
  - Fields: `timestamp`, `pm25`, `pm10`, `co2`, `temperature`, `humidity`, `tvoc`
- `/data/outdoor-weather/*.parquet` - OpenWeatherMap data
  - Fields: `timestamp`, `temperature`, `feels_like`, `pressure`, `humidity`, `wind_speed`, `clouds`
- `/data/outdoor-air-quality/*.parquet` - OpenWeatherMap AQI data
  - Fields: `timestamp`, `aqi`, `pm2_5`, `pm10`, `no2`, `o3`, `co`, `so2`

**Acceptance Criteria**:
- DuckDB can execute `SELECT * FROM read_parquet('/data/air-quality/*.parquet')` successfully
- All three streams are accessible via wildcard patterns
- Column names and types are correctly inferred from Parquet schema
- Partition structure (daily files) is handled transparently

---

### FR-002: Virtual Silver Views

**Priority**: P0 (Critical)

DuckDB MUST provide virtual views with data quality transformations:

**View 1: `silver_indoor`**
```sql
CREATE VIEW silver_indoor AS
SELECT
    timestamp,
    CASE WHEN pm25 BETWEEN 0 AND 500 THEN pm25 END AS pm25,
    CASE WHEN pm10 BETWEEN 0 AND 500 THEN pm10 END AS pm10,
    CASE WHEN co2 BETWEEN 400 AND 5000 THEN co2 END AS co2,
    CASE WHEN temperature BETWEEN -40 AND 85 THEN temperature END AS temperature,
    CASE WHEN humidity BETWEEN 0 AND 100 THEN humidity END AS humidity,
    CASE WHEN tvoc BETWEEN 0 AND 60000 THEN tvoc END AS tvoc
FROM read_parquet('/data/air-quality/*.parquet')
WHERE timestamp IS NOT NULL;
```

**View 2: `silver_outdoor_weather`**
```sql
CREATE VIEW silver_outdoor_weather AS
SELECT
    timestamp,
    CASE WHEN temperature BETWEEN -100 AND 100 THEN temperature END AS temperature,
    CASE WHEN feels_like BETWEEN -100 AND 100 THEN feels_like END AS feels_like,
    CASE WHEN pressure BETWEEN 800 AND 1200 THEN pressure END AS pressure,
    CASE WHEN humidity BETWEEN 0 AND 100 THEN humidity END AS humidity,
    CASE WHEN wind_speed BETWEEN 0 AND 200 THEN wind_speed END AS wind_speed,
    CASE WHEN clouds BETWEEN 0 AND 100 THEN clouds END AS clouds
FROM read_parquet('/data/outdoor-weather/*.parquet')
WHERE timestamp IS NOT NULL;
```

**View 3: `silver_outdoor_aqi`**
```sql
CREATE VIEW silver_outdoor_aqi AS
SELECT
    timestamp,
    CASE WHEN aqi BETWEEN 1 AND 5 THEN aqi END AS aqi,
    CASE WHEN pm2_5 >= 0 THEN pm2_5 END AS pm2_5,
    CASE WHEN pm10 >= 0 THEN pm10 END AS pm10,
    CASE WHEN no2 >= 0 THEN no2 END AS no2,
    CASE WHEN o3 >= 0 THEN o3 END AS o3,
    CASE WHEN co >= 0 THEN co END AS co,
    CASE WHEN so2 >= 0 THEN so2 END AS so2
FROM read_parquet('/data/outdoor-air-quality/*.parquet')
WHERE timestamp IS NOT NULL;
```

**Acceptance Criteria**:
- Views filter out NULL timestamps
- Range validation removes outliers (returns NULL for out-of-range values)
- Views are queryable from Grafana
- Query performance meets NFR-002 targets

---

### FR-003: Cross-Stream JOIN Capability

**Priority**: P1 (High)

DuckDB MUST support time-based JOINs across streams for correlation analysis.

**Example Use Case**:
```sql
SELECT
    i.timestamp,
    i.temperature AS indoor_temp,
    o.temperature AS outdoor_temp,
    i.pm25 AS indoor_pm25,
    a.pm2_5 AS outdoor_pm25
FROM silver_indoor i
LEFT JOIN silver_outdoor_weather o
    ON date_trunc('minute', i.timestamp) = date_trunc('minute', o.timestamp)
LEFT JOIN silver_outdoor_aqi a
    ON date_trunc('minute', i.timestamp) = date_trunc('minute', a.timestamp)
WHERE i.timestamp >= now() - INTERVAL 7 DAYS;
```

**Acceptance Criteria**:
- JOINs complete within performance budget (NFR-002)
- Timestamp alignment handles different polling intervals (1min indoor, 5min outdoor)
- LEFT JOINs preserve indoor data even if outdoor missing
- Time-bucketing functions (`date_trunc`) work correctly

---

### FR-004: Grafana DuckDB Connectivity

**Priority**: P0 (Critical)

Grafana MUST connect to DuckDB using a supported data source plugin.

**Options Evaluated**:
1. **DuckDB HTTP API** (via `duckdb-wasm` or `duckdb-http-server`)
2. **PostgreSQL wire protocol** (DuckDB compatibility mode)
3. **CSV/JSON export** (static queries)

**Selected Approach**: PostgreSQL wire protocol (DuckDB v0.9.0+ supports `pg_wire` extension)

**Acceptance Criteria**:
- Grafana can execute queries against DuckDB views
- Connection is stable across container restarts
- Query builder and raw SQL modes both work
- Time-series data is correctly interpreted

---

### FR-005: Dashboard Provisioning (GitOps)

**Priority**: P1 (High)

Grafana dashboards MUST be provisioned from version-controlled configuration files.

**Directory Structure**:
```
/deploy/grafana/
├── provisioning/
│   ├── datasources/
│   │   └── duckdb.yaml
│   └── dashboards/
│       ├── dashboards.yaml
│       └── air-quality-overview.json
```

**Dashboard Requirements**:
- **Air Quality Overview** - 7-day default view
  - Panel 1: Indoor PM2.5 time series
  - Panel 2: Indoor vs Outdoor temperature comparison
  - Panel 3: CO2 levels with threshold markers (800, 1000, 1500 ppm)
  - Panel 4: Humidity comparison (indoor/outdoor)
  - Panel 5: AQI gauge (outdoor)
  - Panel 6: TVOC histogram

**Acceptance Criteria**:
- Dashboards load automatically on Grafana startup
- Changes to JSON files are reflected after container restart
- No manual configuration required post-deployment
- Dashboard versioning tracked in git

---

### FR-006: Dashboard Editing in Grafana UI

**Priority**: P2 (Medium)

Users MUST be able to edit dashboards in Grafana UI and save changes.

**Acceptance Criteria**:
- Dashboard edit mode is accessible
- Changes can be saved within Grafana
- Save location is persistent across restarts
- Warning displayed if provisioned dashboard is edited (overwrite risk)

**Implementation Note**: Use Grafana's default SQLite database for user edits, separate from provisioned dashboards.

---

### FR-007: Time Range Selection

**Priority**: P1 (High)

Dashboards MUST support configurable time range selection.

**Default Ranges**:
- **Default**: Last 7 days
- **Extended**: Last 30 days
- **Custom**: User-defined via Grafana UI

**Performance Constraints**:
- 7-day queries: <5 seconds (NFR-002)
- 30-day queries: <15 seconds (NFR-002)

**Acceptance Criteria**:
- Time range picker is visible on all dashboards
- Queries adjust dynamically based on selection
- Performance meets NFR-002 targets for both ranges
- No timeout errors within supported ranges

---

## 3. Non-Functional Requirements

### NFR-001: Resource Limits

**Priority**: P0 (Critical)

**DuckDB Container**:
- Memory limit: 512MB (`--memory=512m`)
- Memory reservation: 256MB (`--memory-reservation=256m`)
- CPU limit: 1 core (`--cpus=1`)

**Grafana Container**:
- Memory limit: 256MB (`--memory=256m`)
- Memory reservation: 128MB (`--memory-reservation=128m`)
- CPU limit: 0.5 cores (`--cpus=0.5`)

**Acceptance Criteria**:
- Containers respect memory limits (no OOM kills)
- Combined additional memory usage ≤768MB
- Total platform memory usage <2GB (including existing services)
- Resource monitoring via Docker stats

**Monitoring**:
```bash
watch -n 5 'docker stats --no-stream --format "table {{.Name}}\t{{.MemUsage}}\t{{.CPUPerc}}"'
```

---

### NFR-002: Query Performance

**Priority**: P0 (Critical)

**Performance Targets**:

| Query Type | Time Range | Target Latency | Max Latency |
|------------|------------|----------------|-------------|
| Simple SELECT | 7 days | <2s | <5s |
| Simple SELECT | 30 days | <5s | <15s |
| JOIN (2 streams) | 7 days | <3s | <8s |
| JOIN (3 streams) | 7 days | <5s | <15s |
| Aggregation (hourly) | 7 days | <3s | <10s |

**Acceptance Criteria**:
- 95th percentile queries meet target latency
- No query exceeds max latency
- Performance does not degrade over time (constant file count)
- Cache hit rate >80% for repeated queries

**Optimization Strategies**:
- DuckDB in-memory column caching
- Partition pruning (daily files)
- Aggregate pushdown to Parquet readers

---

### NFR-003: Security & Access Control

**Priority**: P1 (High)

**Authentication**: None required (home network deployment)

**Network Isolation**:
- DuckDB: Internal Docker network only (no host exposure)
- Grafana: Host port 3000 exposed (read-only access)

**File System Access**:
- Parquet files: Read-only mount (`ro` flag)
- No write permissions to `/data/` directory
- Grafana config: Read-write for dashboard persistence

**Acceptance Criteria**:
- DuckDB cannot modify Parquet files
- Grafana accessible on `http://<pi-ip>:3000`
- No external network exposure beyond LAN
- Docker volumes use appropriate permissions

---

### NFR-004: Container Health Checks

**Priority**: P1 (High)

**DuckDB Health Check**:
```bash
healthcheck:
  test: ["CMD", "duckdb", "-c", "SELECT 1"]
  interval: 30s
  timeout: 5s
  retries: 3
  start_period: 10s
```

**Grafana Health Check**:
```bash
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:3000/api/health"]
  interval: 30s
  timeout: 5s
  retries: 3
  start_period: 20s
```

**Acceptance Criteria**:
- Health checks pass under normal operation
- Unhealthy containers restart automatically
- Health status visible in `docker ps`
- Logs capture health check failures

---

### NFR-005: Graceful Degradation

**Priority**: P2 (Medium)

System MUST degrade gracefully if DuckDB is unavailable.

**Behavior**:
- Existing ingestion pipeline continues unaffected
- Grafana shows "Data source unavailable" error
- No impact on MQTT, etcd, or Parquet writing
- DuckDB restart does not require app restart

**Acceptance Criteria**:
- Air quality data ingestion continues during DuckDB downtime
- Parquet files are not corrupted by concurrent access
- Grafana reconnects automatically when DuckDB recovers
- No cascading failures across containers

---

## 4. Constraints

### C-001: No Rust Code Modifications

**Rationale**: Minimize risk and scope; focus on additive analytics layer.

**Implications**:
- No changes to `core/` or `apps/` directories
- No modifications to existing data models
- Parquet schema is fixed (defined by current implementation)
- Configuration changes limited to Docker and Grafana

**Verification**:
- Git diff shows no changes to `*.rs` files
- `cargo build` output identical before/after feature

---

### C-002: Read-Only Parquet Access

**Rationale**: Prevent data corruption from concurrent access.

**Implementation**:
- Docker volume mount: `/data:/data:ro`
- DuckDB configuration: No write operations enabled
- File system permissions: No write access for DuckDB user

**Verification**:
- Attempt to write fails with permission error
- Parquet file checksums unchanged after queries
- No `.tmp` or `.lock` files created in `/data/`

---

### C-003: Docker-Based Deployment

**Rationale**: Consistent with existing deployment strategy.

**Requirements**:
- All services run in Docker containers
- Managed via `docker-compose.yml`
- Orchestration via `deploy.sh` scripts
- No manual service installation on host

**Verification**:
- `docker ps` shows all services running
- Services start/stop via `deploy.sh` commands
- No host-level systemd services added

---

### C-004: ARM64 Compatibility

**Rationale**: Raspberry Pi 5 uses ARM64 architecture.

**Image Requirements**:
- DuckDB: Official ARM64 image or multi-arch build
- Grafana: Official `grafana/grafana:latest` (supports ARM64)
- No x86-only dependencies

**Verification**:
```bash
docker inspect <image> | grep Architecture
# Output: "Architecture": "arm64"
```

---

## 5. Integration Points

### INT-001: Parquet File Structure

**Source**: Existing Rust ingestion pipeline (apps/air-quality-app)

**Directory Layout**:
```
/data/
├── air-quality/
│   ├── 2025-12-18.parquet
│   ├── 2025-12-17.parquet
│   └── ...
├── outdoor-weather/
│   ├── 2025-12-18.parquet
│   └── ...
└── outdoor-air-quality/
    ├── 2025-12-18.parquet
    └── ...
```

**Schema Validation**:
- DuckDB MUST handle schema evolution (new columns added)
- Missing columns return NULL (backward compatibility)
- Type mismatches logged but do not fail queries

---

### INT-002: Docker Network

**Network Name**: `ndp-network` (internal)

**Services**:
- `mosquitto` (MQTT broker)
- `etcd` (configuration store)
- `air-quality-app` (Rust ingestion)
- `duckdb` (analytics) - NEW
- `grafana` (visualization) - NEW

**Ports**:
- Grafana: `3000:3000` (host:container)
- DuckDB: Internal only (no host exposure)

**DNS Resolution**:
- Services resolve each other by container name
- Grafana connects to `duckdb:5432` (PostgreSQL wire protocol)

---

### INT-003: Grafana Provisioning

**Mount Points**:
```yaml
volumes:
  - ./deploy/grafana/provisioning:/etc/grafana/provisioning:ro
  - grafana-data:/var/lib/grafana
```

**Provisioning Flow**:
1. Grafana starts
2. Reads `/etc/grafana/provisioning/datasources/*.yaml`
3. Configures DuckDB data source
4. Reads `/etc/grafana/provisioning/dashboards/*.json`
5. Loads dashboards into UI

**Configuration Management**:
- Datasource config: Version controlled in git
- Dashboard JSON: Version controlled in git
- User edits: Stored in `grafana-data` volume (not version controlled)

---

## 6. Acceptance Criteria Summary

**Feature Complete When**:
- [ ] DuckDB container running with <512MB memory
- [ ] Grafana container running with <256MB memory
- [ ] All three Parquet streams queryable via SQL
- [ ] Virtual Silver views return filtered data
- [ ] Cross-stream JOINs execute successfully
- [ ] Grafana connects to DuckDB data source
- [ ] Air Quality Overview dashboard renders
- [ ] 7-day queries complete in <5s
- [ ] 30-day queries complete in <15s
- [ ] Health checks pass for both containers
- [ ] No modifications to Rust codebase
- [ ] Deployment via `deploy.sh` commands
- [ ] Documentation updated in SPARC structure

---

## 7. Out of Scope

**Explicitly NOT included in DP-001**:

- User authentication/authorization
- External API exposure (Grafana API, DuckDB HTTP)
- Alerting/notifications (future feature: AL-xxx)
- Machine learning features (future feature: FE-xxx)
- Data retention policies (handled by existing Parquet rotation)
- Multi-user access control
- SSL/TLS encryption (home network deployment)
- Cloud deployment (Pi-only for now)
- Mobile app/PWA
- Real-time streaming dashboards (<1min latency)

---

## 8. Dependencies

**External**:
- Docker Engine 24.0+
- Docker Compose 2.20+
- Raspberry Pi OS (64-bit)

**Internal**:
- Existing Parquet files in `/data/`
- Docker network `ndp-network`
- `deploy.sh` infrastructure

**Optional**:
- Reverse proxy (future: nginx for HTTPS)
- Backup solution (future: automated Parquet archival)

---

## 9. Risks & Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Memory limit exceeded | Medium | High | Implement aggressive caching limits, monitor with `docker stats` |
| Query timeout (30-day range) | Medium | Medium | Add query optimization, consider pre-aggregated views |
| DuckDB ARM64 compatibility | Low | High | Verify multi-arch support before implementation |
| Parquet file locking | Low | High | Use read-only mounts, test concurrent access |
| Dashboard complexity (performance) | Medium | Medium | Limit panel count, use efficient queries |

---

## 10. Success Metrics

**KPI-001: Query Performance**
- Metric: P95 query latency for 7-day range
- Target: <5 seconds
- Measurement: Grafana query inspector

**KPI-002: Resource Efficiency**
- Metric: Combined memory usage (DuckDB + Grafana)
- Target: <768MB
- Measurement: `docker stats`

**KPI-003: Dashboard Usability**
- Metric: Time to first meaningful visualization
- Target: <10 seconds from page load
- Measurement: Browser performance tools

**KPI-004: System Stability**
- Metric: Container uptime
- Target: >99% over 30 days
- Measurement: Docker logs and health checks

---

## 11. References

- **Architecture**: `/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md`
- **ADRs**: `/docs/architecture/AIR-005_ADR_SUMMARY.md`
- **Parquet Schema**: Inferred from `core/src/models.rs` (`TimeSeriesPoint`)
- **DuckDB Docs**: https://duckdb.org/docs/
- **Grafana Provisioning**: https://grafana.com/docs/grafana/latest/administration/provisioning/

---

**Next Steps**: Proceed to SPARC Pseudocode phase (DP-001/pseudocode/)
