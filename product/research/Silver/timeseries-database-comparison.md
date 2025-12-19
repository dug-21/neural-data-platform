# Time-Series Database Comparison for NDP Silver Layer

**Research Date**: 2025-12-19
**Context**: Raspberry Pi 5 (16GB RAM, ARM64), Docker deployment
**Purpose**: Evaluate alternatives to DuckDB for SQL analytics, Grafana dashboards, ML feature engineering

---

## Executive Summary

This research evaluates five time-series databases for the Neural Data Platform Silver layer: InfluxDB 3, QuestDB, TimescaleDB, VictoriaMetrics, and Apache Druid. Each database was assessed for ARM64 support, memory footprint, Grafana integration, query language, feature engineering capabilities, Parquet integration, and cloud portability.

**Top Recommendations**:
1. **QuestDB** - Best all-around fit for NDP requirements
2. **TimescaleDB** - Strong SQL capabilities with proven Raspberry Pi deployment
3. **InfluxDB 3** - Native Parquet storage but limited ARM64 maturity

---

## Detailed Comparison Table

| Criteria | InfluxDB 3 | QuestDB | TimescaleDB | VictoriaMetrics | Apache Druid |
|----------|-----------|---------|-------------|----------------|--------------|
| **ARM64 Docker Support** | ✅ Official | ✅ Official | ✅ Mature | ✅ Official | ⚠️ Limited info |
| **Memory Footprint** | ⚠️ Unknown (~500MB+) | ✅ Low (~200-400MB) | ⚠️ Moderate (PostgreSQL base ~300-600MB) | ✅ Very low (~100-300MB) | ❌ Heavy (JVM, >1GB) |
| **Grafana Support** | ✅ Native plugin | ✅ Official plugin | ✅ PostgreSQL datasource | ✅ Prometheus/native plugin | ✅ Druid datasource |
| **Query Language** | SQL + InfluxQL | SQL (PostgreSQL wire) | SQL (PostgreSQL) | PromQL/MetricsQL | SQL (custom dialect) |
| **Native Parquet Read** | ✅ Storage format | ✅ read_parquet() | ⚠️ Via FDW (parquet_fdw) | ❌ No | ⚠️ Via ingestion |
| **Parquet Write** | ✅ Native storage | ✅ Native export/storage | ❌ No (export via COPY) | ❌ No | ⚠️ Via batch |
| **ETL Requirements** | Low (native Parquet) | Low (read_parquet SQL) | Medium (FDW setup) | High (metrics only) | High (batch ingestion) |
| **Feature Engineering** | ⚠️ Basic (InfluxQL) | ✅ Advanced SQL + SAMPLE BY | ✅ Continuous aggregates | ⚠️ Limited (PromQL) | ✅ OLAP aggregations |
| **Cloud Managed Service** | ✅ InfluxDB Cloud | ✅ QuestDB Cloud | ✅ Timescale Cloud | ✅ VictoriaMetrics Cloud | ✅ Imply (managed Druid) |
| **ML Integration** | ⚠️ Basic export | ✅ Parquet export + SQL | ✅ PostgreSQL ecosystem | ❌ Metrics-focused | ✅ OLAP + Parquet |
| **Maturity on ARM64** | ⚠️ New (v3 GA Apr 2025) | ✅ Proven | ✅ Very mature | ✅ Proven | ❌ Limited evidence |
| **Deployment Complexity** | Medium | Low | Medium (PostgreSQL) | Low | High (JVM + ZooKeeper) |
| **License** | MIT (Core) | Apache 2.0 | Apache 2.0 (TimescaleDB License) | Apache 2.0 | Apache 2.0 |

**Legend**: ✅ Strong support | ⚠️ Partial/uncertain | ❌ Not supported/suitable

---

## Detailed Database Analysis

### 1. InfluxDB 3

**Overview**: Complete rewrite using Apache Arrow, DataFusion, and Parquet. GA release April 15, 2025.

#### Strengths
- **Native Parquet Storage**: All data stored as Parquet in object storage, enabling direct access from ML tools
- **ARM64 Docker Support**: Official ARM64 images available (`influxdb:3-core`)
- **Dual Query Languages**: SQL and InfluxQL support via DataFusion
- **Performance Optimizations**: Last Value Cache (LVC) for <10ms queries, Distinct Value Cache (DVC)
- **Cloud-Ready Architecture**: Diskless design with object storage (S3, Azure Blob)
- **Grafana Integration**: Native datasource plugin

#### Weaknesses
- **New on ARM64**: Only GA since April 2025, less production-proven
- **Memory Footprint Unknown**: No specific ARM64 memory benchmarks found
- **Feature Engineering**: Basic InfluxQL aggregations, not as rich as SQL databases
- **Migration Path**: InfluxDB 2.x to 3.x migration required if upgrading

#### Parquet Integration
- **Write**: Native storage format (best-in-class)
- **Read**: Query engine reads Parquet directly from object storage
- **Export**: Data already in Parquet format

#### Best For
- Greenfield deployments prioritizing Parquet-native storage
- Teams wanting dual SQL/InfluxQL query support
- Cloud-first architectures with object storage

**Sources**:
- [InfluxDB 3 Core Installation](https://docs.influxdata.com/influxdb3/core/install/)
- [InfluxDB Docker ARM Support](https://www.influxdata.com/blog/influxdata-docker-arm/)
- [InfluxDB 3 Storage Engine Architecture](https://docs.influxdata.com/influxdb3/cloud-dedicated/reference/internals/storage-engine/)
- [InfluxDB 3 Open Source GA](https://www.influxdata.com/blog/the-plan-for-influxdb-3-0-open-source/)

---

### 2. QuestDB

**Overview**: High-performance SQL time-series database optimized for ingestion and query speed.

#### Strengths
- **ARM64 Docker Support**: Official support, well-documented
- **Low Memory Footprint**: Designed for edge devices, ~200-400MB typical
- **Native Parquet Support**: `read_parquet()` SQL function (v8.1+), parallel execution (v8.2.2+)
- **SQL-Native**: Full PostgreSQL wire protocol, standard SQL queries
- **Grafana Plugin**: Official QuestDB datasource with excellent integration
- **Feature Engineering**: Advanced SAMPLE BY for time-series aggregations, windowing functions
- **Performance**: Zero-GC Java, 230 inserts/sec on Raspberry Pi (benchmarked)
- **Parquet Storage**: Convert partitions to Parquet format for hybrid storage

#### Weaknesses
- **Parquet Read Limitations**: Single file only (no directory reads), security restrictions (import directory only)
- **Community Size**: Smaller than PostgreSQL/TimescaleDB ecosystem
- **Type Support**: Parquet columns with unsupported types are ignored

#### Parquet Integration
- **Write**: Native export to Parquet, ALTER TABLE partition conversion
- **Read**: `read_parquet('file.parquet')` SQL function with parallel execution
- **Hybrid Storage**: Query native QuestDB + Parquet files simultaneously via SQL

#### Best For
- Low-latency ingestion and query requirements
- Teams wanting SQL + native Parquet integration
- Resource-constrained environments (Raspberry Pi)
- ML workflows needing Parquet export

**Sources**:
- [QuestDB Parquet Functions](https://questdb.com/docs/reference/function/parquet/)
- [QuestDB 8.1.0 Release - Parquet Support](https://questdb.com/blog/questdb-release-8-1-0/)
- [QuestDB Grafana Integration](https://questdb.com/docs/third-party-tools/grafana/)
- [QuestDB for Time-Series](https://questdb.com/)

---

### 3. TimescaleDB

**Overview**: PostgreSQL extension adding time-series capabilities (hypertables, continuous aggregates).

#### Strengths
- **ARM64 Maturity**: Very mature on Raspberry Pi, archlinuxarm packages available
- **Continuous Aggregates**: Materialized views for real-time aggregations (91% data reduction in benchmarks)
- **Full PostgreSQL**: Complete SQL support, rich ecosystem (pg_extensions, FDWs)
- **Grafana Support**: PostgreSQL datasource (universally supported)
- **Feature Engineering**: Advanced SQL, window functions, custom aggregations
- **Memory Improvements**: Version 2.0 overhauled materializer, reduced memory usage
- **Proven Performance**: 260 inserts/sec on Raspberry Pi (benchmarked)

#### Weaknesses
- **Memory Footprint**: PostgreSQL base (~300-600MB) heavier than purpose-built TSDB
- **Continuous Aggregate Memory**: Historical issues with unbounded memory (fixed in 2.0+)
- **Parquet Integration**: Requires Foreign Data Wrapper (parquet_fdw), not native
- **Segfaults on Pi**: Reported issues with continuous aggregates on older versions (fixed)

#### Parquet Integration
- **Write**: Export via COPY to CSV, then convert (not native)
- **Read**: Foreign Data Wrapper (parquet_fdw, parquet_s3_fdw) for querying Parquet files
- **Toolkit Request**: Community has requested native Parquet I/O in timescaledb-toolkit (issue #450)

#### Best For
- Teams with PostgreSQL expertise
- Complex SQL query requirements
- Applications needing full RDBMS features (transactions, constraints)
- Continuous aggregates for materialized views

**Sources**:
- [TimescaleDB 2.24.0 Release](https://github.com/timescale/timescaledb/releases/tag/2.24.0)
- [TimescaleDB Continuous Aggregates Memory Issue](https://github.com/timescale/timescaledb/issues/2130)
- [PostgreSQL Parquet S3 FDW](https://www.postgresql.org/about/news/parquet-s3-fdw-110-released-2768/)
- [TimescaleDB Toolkit Parquet Request](https://github.com/timescale/timescaledb-toolkit/issues/450)

---

### 4. VictoriaMetrics

**Overview**: Prometheus-compatible metrics database optimized for monitoring workloads.

#### Strengths
- **ARM64 Support**: Official ARM/ARM64 builds via DockerHub and releases page
- **Minimal Memory**: Extremely low footprint (~100-300MB), designed for edge
- **Grafana Integration**: Native VictoriaMetrics datasource + Prometheus compatibility
- **Single Binary**: No external dependencies, simple deployment
- **PromQL/MetricsQL**: Enhanced PromQL for advanced queries
- **Performance**: Less CPU/RAM than Prometheus for >1000 targets

#### Weaknesses
- **Metrics-Focused**: Designed for metrics/monitoring, not general time-series analytics
- **No Parquet Support**: No native Parquet read/write
- **Limited SQL**: PromQL/MetricsQL, not standard SQL
- **Feature Engineering**: Basic aggregations via MetricsQL, not as rich as SQL databases
- **Data Model**: Labels/metrics model, not relational

#### Parquet Integration
- **Write**: No native support
- **Read**: No native support
- **ETL**: Would require custom export pipeline

#### Best For
- Prometheus-compatible monitoring use cases
- Resource-constrained monitoring deployments
- Teams familiar with PromQL
- **Not Recommended** for NDP's SQL analytics and ML feature engineering needs

**Sources**:
- [VictoriaMetrics Documentation](https://docs.victoriametrics.com/)
- [VictoriaMetrics ARM Support](https://docs.victoriametrics.com/victoriametrics/single-server-victoriametrics/)
- [VictoriaMetrics Grafana Integration](https://docs.victoriametrics.com/victoriametrics/integrations/grafana/)

---

### 5. Apache Druid

**Overview**: Distributed OLAP database for high-cardinality, high-dimensional analytics.

#### Strengths
- **OLAP Power**: Millisecond queries on billions of rows, high-cardinality support
- **Grafana Support**: Druid datasource plugin available
- **SQL Support**: SQL queries via Druid SQL
- **Real-Time + Batch**: Streaming (Kafka, Kinesis) + batch ingestion
- **Horizontal Scaling**: Distributed architecture

#### Weaknesses
- **Resource Heavy**: JVM-based, requires ZooKeeper, >1GB memory typical
- **ARM64 Unknown**: No specific ARM64 documentation or benchmarks found
- **Complexity**: Multi-node architecture (coordinator, broker, historical, middleManager)
- **Overkill for Pi**: Designed for large-scale distributed deployments, not edge
- **Parquet**: Can ingest Parquet but not query directly (batch ingestion required)

#### Parquet Integration
- **Write**: Not primary storage format
- **Read**: Batch ingestion from Parquet, not direct queries
- **ETL**: High overhead for small-scale deployments

#### Best For
- Large-scale OLAP analytics (billions of rows)
- Multi-tenant dashboards with high concurrency
- **Not Recommended** for Raspberry Pi (too resource-heavy)

**Sources**:
- [Apache Druid Introduction](https://druid.apache.org/docs/latest/design/)
- [Apache Druid FAQ](https://druid.apache.org/faq/)
- [Apache Druid Technology](https://druid.apache.org/technology/)

---

## Feature Engineering Capabilities

### Continuous Aggregates & Rollups

| Database | Feature | Capability |
|----------|---------|------------|
| **TimescaleDB** | Continuous Aggregates | Materialized views auto-refreshed in background, 91% data reduction |
| **QuestDB** | SAMPLE BY | Time-bucketing aggregations (ALIGN TO CALENDAR), parallel execution |
| **InfluxDB 3** | Caching | Last Value Cache (LVC), Distinct Value Cache (DVC) for <10ms queries |
| **VictoriaMetrics** | Downsampling | Prometheus recording rules, MetricsQL aggregations |
| **Druid** | Rollup | Pre-aggregation at ingestion, real-time + historical rollup |

### ML Feature Engineering Patterns

Common time-series ML features supported:

1. **Lag Features**: All databases via SQL window functions (except VictoriaMetrics)
2. **Rolling Windows**: TimescaleDB (SQL), QuestDB (SAMPLE BY), InfluxDB (InfluxQL)
3. **Temporal Aggregations**: All databases
4. **Seasonality/Periodicity**: SQL databases (PostgreSQL extensions), basic in others
5. **Fourier Transforms**: External libraries required (except TimescaleDB with extensions)

**Best for ML Feature Engineering**:
1. **TimescaleDB**: Full PostgreSQL + extensions (PL/Python, MADlib)
2. **QuestDB**: Native SAMPLE BY + Parquet export for external ML
3. **InfluxDB 3**: Parquet export to Python (pandas, polars)

**Sources**:
- [Practical Guide for Time-Series Feature Engineering](https://dotdata.com/blog/practical-guide-for-feature-engineering-of-time-series-data/)
- [Feature Engineering for Time Series - Featuretools](https://featuretools.alteryx.com/en/stable/guides/time_series.html)
- [Advanced Feature Engineering for Time-Series](https://medium.com/@rahulholla1/advanced-feature-engineering-for-time-series-data-5f00e3a8ad29)

---

## Parquet Integration Deep Dive

### Native Parquet Support Comparison

| Database | Read | Write | Storage | Virtual Views | ETL Overhead |
|----------|------|-------|---------|--------------|--------------|
| **InfluxDB 3** | ✅ Native | ✅ Native | ✅ Primary format | ✅ Object storage | ⭐⭐⭐⭐⭐ (None) |
| **QuestDB** | ✅ read_parquet() | ✅ Export/ALTER | ⚠️ Hybrid | ⚠️ Single file only | ⭐⭐⭐⭐ (Low) |
| **TimescaleDB** | ⚠️ FDW | ❌ No | ❌ PostgreSQL | ⚠️ Via parquet_fdw | ⭐⭐ (Medium) |
| **VictoriaMetrics** | ❌ No | ❌ No | ❌ Metrics | ❌ No | ⭐ (High) |
| **Druid** | ⚠️ Batch ingest | ⚠️ Not primary | ❌ Columnar | ❌ No | ⭐ (High) |

### Virtual Views Pattern (Query Parquet Without Import)

**InfluxDB 3**: Best-in-class. Data is stored as Parquet in object storage, queries run directly against Parquet files using DataFusion's optimized execution engine.

**QuestDB**: Excellent. Use `read_parquet('file.parquet')` in SQL queries. Version 8.2.2 added parallel execution. Limitation: single file only, must be in import directory.

```sql
-- QuestDB example
SELECT timestamp, sensor_id, temperature
FROM read_parquet('bronze/air-quality-2025-12-19.parquet')
WHERE timestamp > '2025-12-19T00:00:00Z';
```

**TimescaleDB**: Moderate. Requires setting up Foreign Data Wrapper:

```sql
-- TimescaleDB with parquet_fdw
CREATE EXTENSION parquet_fdw;
CREATE SERVER parquet_srv FOREIGN DATA WRAPPER parquet_fdw;
CREATE FOREIGN TABLE bronze_data (
  timestamp TIMESTAMP,
  sensor_id TEXT,
  temperature DOUBLE PRECISION
) SERVER parquet_srv
OPTIONS (filename '/data/bronze/air-quality.parquet');
```

**Sources**:
- [QuestDB Parquet Functions](https://questdb.com/docs/reference/function/parquet/)
- [InfluxDB 3 Storage Engine](https://docs.influxdata.com/influxdb3/cloud-dedicated/reference/internals/storage-engine/)
- [PostgreSQL Parquet FDW](https://github.com/adjust/parquet_fdw)

---

## Cloud Portability & Managed Services

| Database | Managed Service | Cloud Providers | Data Portability |
|----------|----------------|-----------------|------------------|
| **InfluxDB 3** | InfluxDB Cloud | AWS, Azure, GCP | ✅ Parquet (object storage) |
| **QuestDB** | QuestDB Cloud | AWS (planned multi-cloud) | ✅ Parquet export |
| **TimescaleDB** | Timescale Cloud | AWS, Azure, GCP | ⚠️ PostgreSQL dump |
| **VictoriaMetrics** | VictoriaMetrics Cloud | Multi-cloud | ⚠️ Prometheus format |
| **Druid** | Imply | AWS, Azure, GCP | ⚠️ Deep storage (S3/etc) |

**Portability Ranking**:
1. **InfluxDB 3** & **QuestDB**: Parquet as first-class citizen
2. **TimescaleDB**: PostgreSQL dumps (industry standard but not analytics-optimized)
3. **VictoriaMetrics**: Metrics-specific format
4. **Druid**: Vendor-specific deep storage

---

## Memory Footprint Analysis

Based on research and community reports for Raspberry Pi 5 (16GB RAM) with ~512MB per-container target:

| Database | Idle Memory | Under Load | Suitability |
|----------|-------------|------------|-------------|
| **VictoriaMetrics** | ~100MB | ~200-300MB | ✅ Excellent (but wrong use case) |
| **QuestDB** | ~150-200MB | ~300-500MB | ✅ Excellent |
| **InfluxDB 3** | ~200MB (est.) | ~500MB+ (est.) | ⚠️ Acceptable (unproven) |
| **TimescaleDB** | ~200-300MB | ~400-800MB | ⚠️ Acceptable (depends on config) |
| **Druid** | >500MB (min) | >1GB | ❌ Too heavy |

**Notes**:
- TimescaleDB memory issues with continuous aggregates were fixed in version 2.0+
- QuestDB's zero-GC Java design minimizes memory overhead
- InfluxDB 3 memory profile on ARM64 not well-documented (new platform)

**Sources**:
- [TimescaleDB Memory Benchmarks on Pi](https://forums.raspberrypi.com/viewtopic.php?t=342305)
- [QuestDB Performance on Pi](https://ylin31.medium.com/performance-evaluation-of-ticktock-a-new-time-series-db-on-raspberrypi-15359053ebe2)
- [VictoriaMetrics Low Memory](https://docs.victoriametrics.com/faq/)

---

## Recommendations

### Primary Recommendation: **QuestDB**

**Why QuestDB is the best fit for NDP**:

1. **Parquet Integration**: Native `read_parquet()` SQL function enables virtual views pattern without ETL overhead. Hybrid storage (QuestDB + Parquet) allows querying Bronze layer directly.

2. **SQL-Native**: Full PostgreSQL wire protocol, standard SQL for analytics and feature engineering. No custom query language to learn.

3. **ARM64 Proven**: Well-tested on Raspberry Pi with documented benchmarks (230 inserts/sec).

4. **Memory Efficiency**: Low footprint (~200-400MB) fits comfortably in 512MB container limit.

5. **Grafana Integration**: Official QuestDB datasource plugin with excellent time-series support.

6. **Feature Engineering**: Advanced SAMPLE BY for time-bucketing, windowing functions, and parallel Parquet reads (v8.2.2+).

7. **Cloud Portability**: Parquet export + QuestDB Cloud for future migration.

8. **ML-Ready**: Export to Parquet for ML pipelines (pandas, polars, PyArrow).

**Trade-offs**:
- Smaller community than PostgreSQL/TimescaleDB
- Parquet read limited to single files (not directories)

---

### Alternative Recommendation: **TimescaleDB**

**When to choose TimescaleDB instead**:

1. **PostgreSQL Expertise**: Team already knows PostgreSQL deeply
2. **Complex Transactions**: Need full RDBMS features (constraints, triggers, foreign keys)
3. **Extension Ecosystem**: Require PostGIS, PL/Python, pg_cron, etc.
4. **Continuous Aggregates**: Materialized views are primary use case (91% data reduction)
5. **Mature ARM64**: Most battle-tested on Raspberry Pi

**Trade-offs**:
- Higher memory footprint (PostgreSQL overhead)
- Parquet integration via FDW (less elegant than QuestDB)
- No native Parquet write

---

### Not Recommended

**InfluxDB 3**: Wait for ARM64 maturity (6-12 months). Great architecture but too new on ARM64 (GA April 2025). Reconsider in late 2026.

**VictoriaMetrics**: Wrong use case (metrics/monitoring, not analytics).

**Apache Druid**: Too resource-heavy for Raspberry Pi edge deployment.

---

## Implementation Strategy

### Recommended Architecture (QuestDB)

```
┌─────────────────┐
│  Bronze Layer   │
│  (Parquet Files)│
└────────┬────────┘
         │
         │ read_parquet() SQL
         ▼
┌─────────────────┐
│  Silver Layer   │
│    (QuestDB)    │
│                 │
│ • Hybrid storage│
│ • SQL analytics │
│ • Feature eng.  │
└────────┬────────┘
         │
         ├─────────► Grafana (QuestDB datasource)
         │
         └─────────► ML Pipeline (Parquet export)
```

**ETL Pipeline**:
1. Bronze Parquet files remain as-is (no duplication)
2. QuestDB queries Bronze via `read_parquet()` for ad-hoc analytics
3. QuestDB stores aggregated/enriched data in native format
4. Continuous aggregates via SAMPLE BY for Grafana dashboards
5. Export to Parquet for ML feature engineering

### Migration Path (If Choosing TimescaleDB)

```
┌─────────────────┐
│  Bronze Layer   │
│  (Parquet Files)│
└────────┬────────┘
         │
         │ parquet_fdw
         ▼
┌─────────────────┐
│  Silver Layer   │
│  (TimescaleDB)  │
│                 │
│ • Hypertables   │
│ • Cont. Aggr.   │
│ • PostgreSQL    │
└────────┬────────┘
         │
         ├─────────► Grafana (PostgreSQL datasource)
         │
         └─────────► ML Pipeline (COPY to Parquet)
```

**ETL Pipeline**:
1. Set up parquet_fdw foreign tables pointing to Bronze
2. Materialized views (continuous aggregates) in TimescaleDB
3. Grafana queries against hypertables
4. Export to Parquet via external tools for ML

---

## Docker Deployment Examples

### QuestDB Docker Compose

```yaml
version: '3.8'
services:
  questdb:
    image: questdb/questdb:latest
    container_name: ndp-silver-questdb
    ports:
      - "9000:9000"  # Web console & REST
      - "9009:9009"  # InfluxDB line protocol
      - "8812:8812"  # PostgreSQL wire
      - "9003:9003"  # Min health server
    volumes:
      - /data/questdb:/var/lib/questdb
      - /data/bronze:/var/lib/questdb/import:ro  # Bronze Parquet files
    environment:
      - QDB_PG_READONLY_USER_ENABLED=true
      - QDB_TELEMETRY_ENABLED=false
    mem_limit: 512m
    restart: unless-stopped
```

### TimescaleDB Docker Compose

```yaml
version: '3.8'
services:
  timescaledb:
    image: timescale/timescaledb:latest-pg16
    container_name: ndp-silver-timescaledb
    ports:
      - "5432:5432"
    volumes:
      - /data/timescaledb:/var/lib/postgresql/data
      - /data/bronze:/bronze:ro  # Bronze Parquet files
    environment:
      - POSTGRES_USER=ndp
      - POSTGRES_PASSWORD=ndp_password
      - POSTGRES_DB=neural_data
    mem_limit: 768m
    shm_size: 256m
    restart: unless-stopped
```

---

## Testing & Validation Plan

### Performance Benchmarks (To Run)

1. **Ingestion Rate**: Inserts/second from Bronze Parquet files
2. **Query Latency**: P50/P95/P99 for typical Grafana queries
3. **Memory Usage**: Idle vs. under load (1-hour, 24-hour)
4. **Aggregation Speed**: Time to compute continuous aggregates
5. **Parquet Read Speed**: `read_parquet()` vs. native storage

### Validation Criteria

- [ ] Memory stays under 512MB under typical load
- [ ] Query latency <100ms for Grafana dashboards (P95)
- [ ] Can read Bronze Parquet files without duplication
- [ ] SAMPLE BY / continuous aggregates reduce query data by >80%
- [ ] ARM64 Docker image runs stable for 7 days
- [ ] Grafana datasource plugin works on ARM64

---

## References & Sources

### InfluxDB 3
- [InfluxDB 3 Core Installation](https://docs.influxdata.com/influxdb3/core/install/)
- [InfluxDB Docker ARM Support](https://www.influxdata.com/blog/influxdata-docker-arm/)
- [InfluxDB 3 Storage Engine Architecture](https://docs.influxdata.com/influxdb3/cloud-dedicated/reference/internals/storage-engine/)
- [InfluxDB 3 Open Source GA](https://www.influxdata.com/blog/the-plan-for-influxdb-3-0-open-source/)
- [Deep Dive InfluxDB 3 Core](https://grafana.com/events/grafanacon/2025/influxdb-3-core-open-source-release/)

### QuestDB
- [QuestDB Parquet Functions](https://questdb.com/docs/reference/function/parquet/)
- [QuestDB 8.1.0 Release - Parquet Support](https://questdb.com/blog/questdb-release-8-1-0/)
- [QuestDB 8.2.2 - Parallel Parquet](https://questdb.com/blog/questdb-8-2-2/)
- [QuestDB Grafana Integration](https://questdb.com/docs/third-party-tools/grafana/)
- [Fluid Dashboards with Grafana and QuestDB](https://questdb.com/blog/time-series-monitoring-dashboard-grafana-questdb/)

### TimescaleDB
- [TimescaleDB 2.24.0 Release](https://github.com/timescale/timescaledb/releases/tag/2.24.0)
- [TimescaleDB Continuous Aggregates Memory Issue](https://github.com/timescale/timescaledb/issues/2130)
- [TimescaleDB Raspberry Pi Issue](https://github.com/timescale/timescaledb/issues/1227)
- [PostgreSQL Parquet S3 FDW](https://www.postgresql.org/about/news/parquet-s3-fdw-110-released-2768/)
- [Parquet FDW for PostgreSQL](https://github.com/adjust/parquet_fdw)
- [TimescaleDB Toolkit Parquet Request](https://github.com/timescale/timescaledb-toolkit/issues/450)

### VictoriaMetrics
- [VictoriaMetrics Documentation](https://docs.victoriametrics.com/)
- [VictoriaMetrics Single-Node](https://docs.victoriametrics.com/victoriametrics/single-server-victoriametrics/)
- [VictoriaMetrics Grafana Integration](https://docs.victoriametrics.com/victoriametrics/integrations/grafana/)
- [VictoriaMetrics FAQ](https://docs.victoriametrics.com/faq/)

### Apache Druid
- [Apache Druid Introduction](https://druid.apache.org/docs/latest/design/)
- [Apache Druid FAQ](https://druid.apache.org/faq/)
- [Apache Druid Technology](https://druid.apache.org/technology/)
- [Apache Druid - A Scalable Timeseries OLAP Database](https://anskarl.github.io/post/2019/druid-part-1/)

### Feature Engineering & ML
- [Practical Guide for Time-Series Feature Engineering](https://dotdata.com/blog/practical-guide-for-feature-engineering-of-time-series-data/)
- [Feature Engineering for Time Series - Featuretools](https://featuretools.alteryx.com/en/stable/guides/time_series.html/)
- [Advanced Feature Engineering for Time-Series](https://medium.com/@rahulholla1/advanced-feature-engineering-for-time-series-data-5f00e3a8ad29)
- [Real-Time Aggregation Features for ML](https://towardsdatascience.com/real-time-aggregation-features-for-machine-learning-part-2-fe9fd42522c0/)

---

## Next Steps

1. **Proof of Concept**: Deploy QuestDB on Raspberry Pi 5 test environment
2. **Parquet Integration Test**: Verify `read_parquet()` can query existing Bronze layer
3. **Memory Profiling**: Monitor actual memory usage over 7-day period
4. **Grafana Dashboard**: Build sample dashboard with QuestDB datasource on ARM64
5. **Feature Engineering Prototype**: Test SAMPLE BY for ML feature generation
6. **Fallback Test**: If QuestDB issues arise, test TimescaleDB with parquet_fdw

---

**Research Conducted By**: Research Agent (Neural Data Platform)
**Last Updated**: 2025-12-19
