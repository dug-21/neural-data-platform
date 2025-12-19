# Grafana ARM64 Datasource Research for Raspberry Pi 5

**Research Date**: 2025-12-19
**Context**: DP-001 Silver Layer - DuckDB plugin has ARM64 glibc compatibility issues
**Target Platform**: Raspberry Pi 5 (ARM64/aarch64), Grafana OSS 11.4.0
**Objective**: Identify reliable datasource alternatives for time-series visualization

---

## Executive Summary

### What Definitely Works on ARM64

| Datasource | Status | Installation | Complexity | Best For |
|-----------|--------|--------------|------------|----------|
| **SQLite** (frser-sqlite-datasource) | ✅ CONFIRMED | CLI | Low | Current workaround, simple queries |
| **PostgreSQL** (built-in) | ✅ CONFIRMED | Built-in | Low | TimescaleDB, production time-series |
| **Infinity** (yesoreyeram) | ✅ CONFIRMED | CLI + ARM64 binaries | Medium | JSON/CSV/API flexibility |
| **InfluxDB** (built-in) | ✅ CONFIRMED | Built-in | Medium | Dedicated TSDB if migrating storage |
| **CSV** (marcusolsson) | ✅ CONFIRMED | CLI + ARM64 binaries | Low | Static datasets, exports |

### Current Implementation Status

**NDP currently uses**: SQLite datasource (frser-sqlite-datasource) as a workaround
- **Why**: DuckDB plugin has ARM64 glibc 2.29+ dependency issues
- **Approach**: DuckDB exports aggregated views to SQLite (`/var/duckdb/grafana.db`) every 5 minutes
- **Trade-off**: 5-minute staleness vs native DuckDB queries

---

## Detailed Analysis

### 1. SQLite Datasource (frser-sqlite-datasource)

#### ARM64 Compatibility: ✅ EXCELLENT

**Official Support**:
- Raspberry Pi 3, 4, 5 (64-bit ARMv8) supported via Grafana CLI
- ARMv6 build available for Raspberry Pi Zero
- ARMv7 (Raspberry Pi 2 Model B) requires manual installation
- Uses SQLite 3.50.1 as of version 3.7.0

**Installation**:
```bash
grafana-cli plugins install frser-sqlite-datasource
```

**Current NDP Usage**:
```yaml
# config/grafana/provisioning/datasources/duckdb.yaml
datasources:
  - name: NDP-SQLite
    type: frser-sqlite-datasource
    uid: duckdb-ndp
    access: proxy
    jsonData:
      path: /duckdb/grafana.db
```

**Pros**:
- Zero compilation needed - works out of the box
- Lightweight (no backend service needed)
- Standard SQL queries
- Good for read-heavy workloads
- Currently deployed and working in NDP

**Cons**:
- No native time-series functions (requires SQL tricks)
- Limited query optimization compared to dedicated TSDB
- File-based (must mount volume or use network filesystem)
- Not ideal for high-frequency inserts (but NDP uses it read-only)

**Performance Characteristics**:
- Query latency: 100ms-2s for typical time-series queries
- Memory footprint: Minimal (queries run in-process)
- Concurrent users: Good (SQLite handles multiple readers)

**Recommendation**:
✅ **Keep using for current workaround** - It's working, low-risk, and the 5-minute export cycle is acceptable for home monitoring. Consider upgrading to PostgreSQL/TimescaleDB for production scale.

---

### 2. PostgreSQL Datasource (Built-in)

#### ARM64 Compatibility: ✅ EXCELLENT

**Official Support**:
- Core Grafana datasource (not a plugin)
- Grafana provides official ARM64 builds since v5.2.0
- PostgreSQL fully supports ARM64 architecture
- TimescaleDB extension available for ARM64

**Installation**:
```bash
# Already built into Grafana - no plugin needed
# Just configure as a datasource
```

**Docker Setup Example**:
```yaml
# docker-compose.yml
timescaledb:
  image: timescale/timescaledb:latest-pg16
  environment:
    POSTGRES_DB: neural_platform
    POSTGRES_USER: grafana
    POSTGRES_PASSWORD: ${DB_PASSWORD}
  volumes:
    - timescale-data:/var/lib/postgresql/data
  ports:
    - "5432:5432"

grafana:
  depends_on:
    - timescaledb
  # Built-in PostgreSQL datasource, no plugin needed
```

**Grafana Provisioning**:
```yaml
# config/grafana/provisioning/datasources/timescaledb.yaml
apiVersion: 1
datasources:
  - name: TimescaleDB
    type: postgres
    url: timescaledb:5432
    database: neural_platform
    user: grafana
    secureJsonData:
      password: ${DB_PASSWORD}
    jsonData:
      sslmode: disable
      postgresVersion: 1600  # PostgreSQL 16
      timescaledb: true
```

**Pros**:
- Excellent time-series support via TimescaleDB
- Continuous aggregates (automatic rollups)
- Data retention policies (automatic cleanup)
- Full SQL support with window functions
- ACID guarantees (unlike DuckDB's current setup)
- Built-in compression (10x+ reduction)
- Battle-tested production database

**Cons**:
- Requires PostgreSQL service (more infrastructure)
- More complex setup than SQLite
- Needs data migration from Parquet to PostgreSQL
- Higher resource usage (RAM, storage)

**Performance Characteristics**:
- Query latency: 50ms-500ms for typical time-series queries
- Memory footprint: ~256MB minimum for PostgreSQL
- Concurrent users: Excellent (handles 100+ concurrent connections)
- Hypertable partitioning provides 10x+ speedup on time-series queries

**Migration Path for NDP**:
```sql
-- Create hypertable from DuckDB view
CREATE TABLE indoor_air_readings (
  time TIMESTAMPTZ NOT NULL,
  pm25 REAL,
  temperature REAL,
  humidity REAL,
  co2 INTEGER
);

SELECT create_hypertable('indoor_air_readings', 'time');

-- Create continuous aggregate (hourly rollup)
CREATE MATERIALIZED VIEW indoor_air_hourly
WITH (timescaledb.continuous) AS
SELECT
  time_bucket('1 hour', time) AS hour,
  AVG(pm25) AS pm25_avg,
  AVG(temperature) AS temp_avg,
  AVG(humidity) AS humidity_avg
FROM indoor_air_readings
GROUP BY hour;

-- Auto-refresh policy
SELECT add_continuous_aggregate_policy('indoor_air_hourly',
  start_offset => INTERVAL '3 hours',
  end_offset => INTERVAL '1 hour',
  schedule_interval => INTERVAL '1 hour');
```

**Recommendation**:
✅ **BEST LONG-TERM SOLUTION** - If NDP evolves to need real-time queries, alerting, or multi-user dashboards, TimescaleDB is the right architecture. Migration would involve replacing the DuckDB export step with direct writes from the ingestion pipeline to PostgreSQL.

---

### 3. InfluxDB Datasource (Built-in)

#### ARM64 Compatibility: ✅ GOOD

**Official Support**:
- Core Grafana datasource (built-in)
- InfluxDB 2.x confirmed working on Raspberry Pi 4 (container-based)
- Requires 2GB+ RAM (4GB recommended)

**Installation**:
```bash
# InfluxDB container
docker run -d \
  --name influxdb \
  -p 8086:8086 \
  -v influxdb-data:/var/lib/influxdb2 \
  influxdb:2.7

# Grafana datasource (built-in, no plugin)
```

**Grafana Configuration**:
```yaml
# config/grafana/provisioning/datasources/influxdb.yaml
apiVersion: 1
datasources:
  - name: InfluxDB
    type: influxdb
    url: http://influxdb:8086
    jsonData:
      version: Flux  # Or InfluxQL for v1.8+
      organization: neural-platform
      defaultBucket: air-quality
    secureJsonData:
      token: ${INFLUX_TOKEN}
```

**Pros**:
- Purpose-built for time-series data
- Native downsampling and retention policies
- Flux query language (powerful transformations)
- Built-in data collection (Telegraf)
- Good compression (time-structured merge tree)

**Cons**:
- Requires dedicated InfluxDB service
- Different query language (Flux, not SQL)
- Higher resource usage than SQLite
- Migration from Parquet requires ETL
- InfluxDB v1.x vs v2.x compatibility differences (breaking changes)

**Performance Characteristics**:
- Query latency: 100ms-1s for typical time-series queries
- Memory footprint: ~256MB minimum for InfluxDB
- Write throughput: 50k+ points/second (single node)

**Recommendation**:
⚠️ **CONSIDER IF MIGRATING STORAGE** - If NDP decides to move away from Parquet storage entirely, InfluxDB is a solid choice. However, the SQL-to-Flux migration is non-trivial, and TimescaleDB offers similar benefits while staying in the SQL ecosystem.

---

### 4. Infinity Datasource (yesoreyeram-infinity-datasource)

#### ARM64 Compatibility: ✅ EXCELLENT

**Official Support**:
- Now maintained by Grafana Labs
- Official ARM64 binaries since v3.6.0 (September 2025)
- Backend compiled with Go 1.22.3

**ARM64 Builds Available**:
```
yesoreyeram-infinity-datasource-3.6.0.linux_arm64.zip (14.2 MB)
yesoreyeram-infinity-datasource-3.6.0.darwin_arm64.zip (14.8 MB)
yesoreyeram-infinity-datasource-3.6.0.linux_arm.zip (14.7 MB)
```

**Installation**:
```bash
grafana-cli plugins install yesoreyeram-infinity-datasource
```

**Use Cases**:
- Query JSON APIs (REST endpoints)
- Parse CSV files (local or remote)
- XML/GraphQL endpoints
- Backend operations (alerting, enterprise caching)

**Example: Query DuckDB via HTTP API**:
```json
{
  "type": "json",
  "url": "http://duckdb-api:8080/query",
  "method": "POST",
  "body": {
    "sql": "SELECT * FROM readings_hourly WHERE time > $__timeFrom"
  },
  "parser": "backend",
  "source": "url",
  "format": "table"
}
```

**Pros**:
- Flexible (JSON, CSV, XML, GraphQL)
- Supports backend alerting
- OAuth2, JWT, digest authentication
- Can query HTTP APIs (potential DuckDB HTTP wrapper)
- Enterprise query caching

**Cons**:
- Requires backend API if querying databases
- More complex setup than direct database connections
- Not optimized for high-volume time-series

**Recommendation**:
✅ **GOOD FOR API INTEGRATION** - If NDP builds an HTTP API wrapper around DuckDB (instead of SQLite export), Infinity datasource could query it directly. Also useful for integrating external data sources (weather APIs, etc.).

---

### 5. CSV Datasource (marcusolsson-csv-datasource)

#### ARM64 Compatibility: ✅ CONFIRMED

**Official Support**:
- Maintained by Grafana Labs
- ARM64 binary: `marcusolsson-csv-datasource-0.7.1.linux_arm64.zip` (September 2025)

**Installation**:
```bash
grafana-cli plugins install marcusolsson-csv-datasource
```

**Use Cases**:
- Static CSV datasets
- Exported reports
- Testing/development with sample data

**Configuration**:
```yaml
# config/grafana/provisioning/datasources/csv.yaml
apiVersion: 1
datasources:
  - name: CSV-Export
    type: marcusolsson-csv-datasource
    jsonData:
      path: /data/exports/hourly_rollup.csv
```

**Pros**:
- Simple file-based storage
- No backend service needed
- Human-readable data format
- Good for periodic exports

**Cons**:
- No query optimization
- Full file scan on every query
- Not suitable for large datasets (>100MB)
- No time-series indexing

**Recommendation**:
⚠️ **LIMITED USE CASE** - Only suitable for small, static datasets or ad-hoc exports. Not recommended for primary time-series storage.

---

### 6. JSON API Datasources

#### Deprecated: SimpleJSON (simpod-json-datasource)
- ❌ **Deprecated** - EOL June 2024
- Migrate to Infinity datasource instead

#### Current: JSON API (marcusolsson-json-datasource)
- ⚠️ **Maintenance Mode** - No new features
- ⚠️ ARM64 support unconfirmed in official docs (user reports suggest it works)
- Use Infinity datasource for new projects

---

## Alternative Approaches

### A. HTTP Proxy / Query Runner Pattern

Instead of direct database plugins, run a query service that Grafana queries via HTTP.

**Architecture**:
```
Grafana → HTTP API → DuckDB (or any database)
         (Infinity    (Query runner service)
          datasource)
```

**Example Implementation**:
```rust
// Simple DuckDB HTTP query service
#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/query", post(execute_query));

    axum::Server::bind(&"0.0.0.0:8080".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn execute_query(Json(req): Json<QueryRequest>) -> Json<QueryResponse> {
    let conn = Connection::open("/var/duckdb/neural_platform.db").unwrap();
    let mut stmt = conn.prepare(&req.sql).unwrap();
    let rows = stmt.query_map([], |row| { /* map rows */ }).unwrap();
    Json(QueryResponse { data: rows })
}
```

**Pros**:
- Bypasses plugin compatibility issues entirely
- Works with any database backend
- Can add caching, rate limiting, authentication
- Uses Infinity datasource (fully ARM64 compatible)

**Cons**:
- Additional service to maintain
- Network latency on every query
- Must implement security (SQL injection prevention)

**Recommendation**:
✅ **VIABLE LONG-TERM** - If DuckDB plugin never gets ARM64 support, this is a cleaner solution than SQLite export. The query runner can implement advanced features like query caching, prepared statements, and monitoring.

---

### B. Grafana Backend Plugin (Custom)

Build a custom Grafana backend plugin specifically for NDP's use case.

**Pros**:
- Full control over query execution
- Native Grafana integration
- Can optimize for NDP's specific query patterns

**Cons**:
- High development effort (Go + Grafana SDK)
- Maintenance burden
- Must compile for ARM64 and distribute

**Recommendation**:
❌ **NOT RECOMMENDED** - Overkill for NDP's scale. Use existing solutions.

---

## Comparison Matrix

| Feature | SQLite | PostgreSQL | InfluxDB | Infinity | CSV |
|---------|--------|------------|----------|----------|-----|
| **ARM64 Support** | ✅ Excellent | ✅ Excellent | ✅ Good | ✅ Excellent | ✅ Confirmed |
| **Setup Complexity** | Low | Medium | Medium | Low | Low |
| **Query Language** | SQL | SQL | Flux/InfluxQL | JSON/API | N/A |
| **Time-Series Functions** | Limited | Excellent (TimescaleDB) | Excellent | N/A | N/A |
| **Concurrent Users** | Good (read) | Excellent | Excellent | Good | Limited |
| **Memory Footprint** | ~50MB | ~256MB | ~256MB | ~100MB | ~50MB |
| **ACID Guarantees** | ✅ | ✅ | ❌ | N/A | N/A |
| **Data Retention** | Manual | Automatic | Automatic | N/A | Manual |
| **Compression** | Limited | Excellent | Good | N/A | None |
| **Backend Alerting** | ❌ | ✅ | ✅ | ✅ | ❌ |
| **Query Latency** | 100ms-2s | 50ms-500ms | 100ms-1s | Varies | 1s-10s |
| **Best For** | Simple queries | Production TSDB | Dedicated TSDB | API integration | Static data |

---

## Recommendations by Use Case

### Current NDP Setup (Home Monitoring, Single User)
**Recommendation**: ✅ **Keep SQLite datasource**
- Already deployed and working
- Low complexity, low resource usage
- 5-minute staleness is acceptable for home monitoring
- DuckDB export approach is a clean workaround

**Migration Path**: None needed immediately. Monitor for:
- Query performance degradation (if data grows beyond 1-2 years)
- Need for real-time queries (< 5 minutes)
- Multi-user access requirements

---

### Future Scale (Multi-User, Real-Time, Production)
**Recommendation**: ✅ **Migrate to PostgreSQL + TimescaleDB**

**Why**:
- Industry-standard production TSDB
- Excellent ARM64 support
- Built-in Grafana datasource (no plugin needed)
- Automatic rollups, retention, compression
- ACID guarantees for data integrity
- Supports backend alerting

**Migration Steps**:
1. Add TimescaleDB container to `docker-compose.yml`
2. Create hypertables matching current DuckDB views
3. Update ingestion pipeline to write directly to TimescaleDB (bypass Parquet)
4. Set up continuous aggregates for hourly/daily rollups
5. Configure retention policies (e.g., raw data 30 days, hourly 1 year, daily forever)
6. Update Grafana datasource from SQLite to PostgreSQL
7. Test queries and dashboards
8. Migrate data (if needed) or start fresh

**Estimated Effort**: 2-3 days
**Resource Impact**: +256MB RAM for PostgreSQL

---

### Alternative: API-First Architecture
**Recommendation**: ✅ **HTTP Query Service + Infinity Datasource**

**When to Use**:
- DuckDB is the long-term storage choice (not moving to PostgreSQL)
- Want to avoid SQLite export complexity
- Need more control over query execution (caching, rate limiting)
- Plan to expose data to external consumers (not just Grafana)

**Architecture**:
```
Bronze (Parquet) → DuckDB → HTTP Query API → Grafana (Infinity)
                            ↓
                         External Consumers
```

**Implementation**:
```rust
// apps/query-api/src/main.rs
use axum::{Router, Json, extract::Query};
use duckdb::{Connection, params};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct QueryRequest {
    sql: String,
    #[serde(default)]
    params: Vec<String>,
}

#[derive(Serialize)]
struct QueryResponse {
    columns: Vec<String>,
    rows: Vec<Vec<serde_json::Value>>,
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/query", post(execute_query))
        .route("/health", get(|| async { "OK" }));

    axum::Server::bind(&"0.0.0.0:8090".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn execute_query(Json(req): Json<QueryRequest>) -> Json<QueryResponse> {
    // TODO: Add SQL injection prevention
    // TODO: Add query allowlist
    // TODO: Add rate limiting
    let conn = Connection::open("/var/duckdb/neural_platform.db").unwrap();
    // Execute and return results
    // ...
}
```

**Grafana Configuration**:
```yaml
# config/grafana/provisioning/datasources/query-api.yaml
apiVersion: 1
datasources:
  - name: DuckDB-API
    type: yesoreyeram-infinity-datasource
    url: http://query-api:8090
```

**Pros**:
- Clean separation of concerns
- Can add caching layer (Redis)
- Supports external API consumers
- Full control over query execution

**Cons**:
- Additional service to maintain
- Network latency on queries
- Must implement security carefully

**Estimated Effort**: 3-4 days
**Resource Impact**: +128MB RAM for query API service

---

## Known Issues & Workarounds

### Issue: DuckDB Plugin ARM64 Incompatibility
**Error**: `version 'GLIBC_2.29' not found`
**Root Cause**: DuckDB plugin compiled for newer glibc than available on Raspberry Pi OS
**Workaround**: Export DuckDB views to SQLite (current NDP approach)
**Long-Term Fix**: Wait for plugin ARM64 build, or migrate to PostgreSQL

### Issue: Grafana Plugin ARM64 Builds
**Symptom**: Plugin installs but fails to start backend
**Cause**: Some plugins don't provide ARM64 binaries
**Workaround**: Check plugin releases for `linux_arm64.zip` before installing
**Verified ARM64 Plugins**:
- frser-sqlite-datasource ✅
- yesoreyeram-infinity-datasource ✅
- marcusolsson-csv-datasource ✅

### Issue: InfluxDB Version Confusion
**Symptom**: Queries fail with "invalid syntax"
**Cause**: InfluxDB v1.x uses InfluxQL, v2.x uses Flux (incompatible)
**Workaround**: Verify version before configuring Grafana datasource
**Recommendation**: Use InfluxDB 2.x (Flux) for new deployments

---

## Testing Checklist

Before deploying any datasource change to production:

- [ ] **ARM64 Binary Confirmed**: Check plugin releases for `linux_arm64.zip`
- [ ] **Resource Usage**: Monitor RAM/CPU during typical queries
- [ ] **Query Latency**: Verify < 5s for 7-day range, < 15s for 30-day range
- [ ] **Concurrent Access**: Test with 2-3 simultaneous dashboard loads
- [ ] **Health Checks**: Verify datasource health endpoint responds
- [ ] **Backup/Restore**: Test data export and restore procedures
- [ ] **Grafana Restart**: Verify datasource survives Grafana container restart
- [ ] **Data Persistence**: Verify data survives database container restart
- [ ] **Dashboard Queries**: Test all existing dashboard panels
- [ ] **Alert Rules**: Verify alerting works (if using backend-capable datasource)

---

## References & Sources

### SQLite Datasource
- [SQLite plugin for Grafana | Grafana Labs](https://grafana.com/grafana/plugins/frser-sqlite-datasource/)
- [GitHub - fr-ser/grafana-sqlite-datasource](https://github.com/fr-ser/grafana-sqlite-datasource)
- [Releases · fr-ser/grafana-sqlite-datasource](https://github.com/fr-ser/grafana-sqlite-datasource/releases)
- [Would SQLite works well with Grafana - Raspberry Pi Forums](https://forums.raspberrypi.com/viewtopic.php?t=375594)

### PostgreSQL/TimescaleDB
- [Configure the PostgreSQL data source | Grafana documentation](https://grafana.com/docs/grafana/latest/datasources/postgres/configure/)
- [Download Grafana | Grafana Labs](https://grafana.com/grafana/download?platform=arm)
- [Storing and visualizing time-series data from a Raspberry Pi | Timescale](https://www.timescale.com/blog/storing-and-visualizing-time-series-data-from-a-raspberry-pi)
- [How to set up self-hosted TimescaleDB to work with Grafana?](https://community.grafana.com/t/how-to-set-up-self-hosted-timescaledb-to-work-with-grafana-what-schema-to-use/55636)

### InfluxDB
- [Setting up InfluxDB and Grafana on the Raspberry Pi 4](https://sandyjmacdonald.github.io/2021/12/29/setting-up-influxdb-and-grafana-on-the-raspberry-pi-4/)
- [Raspberry Pi 4B, 64-Bit Raspberry OS, Grafana and InfluxDB2 support](https://community.grafana.com/t/raspberry-pi-4b-64-bit-raspberry-os-grafana-and-influxdb2-support/102931)
- [Get started with Grafana and InfluxDB | Grafana documentation](https://grafana.com/docs/grafana/latest/fundamentals/getting-started/first-dashboards/get-started-grafana-influxdb/)

### Infinity Datasource
- [Infinity data source plugin for Grafana | Grafana Plugins documentation](https://grafana.com/docs/plugins/yesoreyeram-infinity-datasource/latest/)
- [Infinity plugin for Grafana | Grafana Labs](https://grafana.com/grafana/plugins/yesoreyeram-infinity-datasource/)
- [Releases · grafana/grafana-infinity-datasource](https://github.com/yesoreyeram/grafana-infinity-datasource/releases)
- [GitHub - grafana/grafana-infinity-datasource](https://github.com/grafana/grafana-infinity-datasource)

### CSV Datasource
- [Releases · grafana/grafana-csv-datasource](https://github.com/grafana/grafana-csv-datasource/releases)
- [CSV plugin for Grafana | Grafana Labs](https://grafana.com/grafana/plugins/marcusolsson-csv-datasource/)

### JSON Datasources
- [JSON API plugin for Grafana | Grafana Labs](https://grafana.com/grafana/plugins/marcusolsson-json-datasource/)
- [GitHub - grafana/grafana-json-datasource](https://github.com/grafana/grafana-json-datasource)
- [JSON plugin for Grafana | Grafana Labs](https://grafana.com/grafana/plugins/simpod-json-datasource/)

### Grafana ARM64 Support
- [GitHub - fg2it/grafana-on-raspberry](https://github.com/fg2it/grafana-on-raspberry)
- [Raspberry Pi 5 and OS supported? - Installation](https://community.grafana.com/t/raspberry-pi-5-and-os-supported/129125)
- [Supported platforms | Grafana Alloy documentation](https://grafana.com/docs/alloy/latest/introduction/supported-platforms/)

---

## Conclusion

For NDP's current home monitoring use case:
✅ **Keep the SQLite datasource approach** - It's working, low-risk, and appropriate for the scale.

For future production scale:
✅ **Plan migration to PostgreSQL + TimescaleDB** - Industry-standard, excellent ARM64 support, purpose-built for time-series data.

Alternative if staying with DuckDB long-term:
✅ **Build HTTP Query API + Infinity datasource** - Cleaner than SQLite export, more control over query execution.

**Avoid**:
- ❌ InfluxDB (unless migrating storage away from Parquet entirely)
- ❌ Custom Grafana plugins (overkill for NDP scale)
- ❌ CSV datasource (only for static/test data)
- ❌ Deprecated JSON plugins (use Infinity instead)

---

**Next Steps**:
1. Continue using SQLite datasource for DP-001 deployment
2. Monitor query performance as data volume grows
3. Evaluate PostgreSQL migration if requirements change (real-time, multi-user)
4. Document decision in Architecture Decision Record (ADR)
