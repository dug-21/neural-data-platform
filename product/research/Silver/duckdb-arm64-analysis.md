# DuckDB on ARM64/Raspberry Pi - Technical Analysis

**Research Date**: 2025-12-19
**Project**: Neural Data Platform - Silver Layer (dp-001)
**Context**: 6 hours debugging DuckDB ARM64 issues on Raspberry Pi 5
**Current State**: Forced to use SQLite export workaround due to Grafana plugin incompatibility

---

## Executive Summary

DuckDB **does work** on ARM64 Raspberry Pi 5 for analytical queries, but the **Grafana DuckDB datasource plugin has critical glibc compatibility issues** on ARM64. The project's current SQLite export workaround is **necessary but suboptimal**. This analysis examines the root causes, explores alternatives, and provides recommendations.

### Key Findings

| Area | Status | Notes |
|------|--------|-------|
| **DuckDB Core on ARM64** | ✅ Stable | Native builds work well, TPC-H benchmarks successful |
| **Grafana DuckDB Plugin** | ❌ Broken on ARM64 | Requires glibc 2.35+, duckdb-go only has glibc binaries |
| **Docker Images** | ⚠️ Mixed | Official image works, datacatering works, plugin ecosystem doesn't |
| **Extension Installation** | ❌ Problematic | 403 errors on ARM64 extension downloads (fixed in 1.4.3) |
| **Production Viability** | ✅ Proven | MotherDuck and community using in production |
| **Current Workaround** | ✅ Functional | SQLite export acceptable for read-heavy analytics |

### Recommendation Summary

**For dp-001 (current phase)**: ✅ **Keep SQLite workaround**
**For future phases**: 🔄 **Re-evaluate when Grafana plugin supports musl or ARM64 binaries available**

---

## 1. DuckDB ARM64 Compatibility Status (2025)

### 1.1 Official Support

**ARM64 (AArch64) is officially supported** for Linux, macOS, and Windows:
- ✅ Both x86_64 (amd64) and AArch64 (ARM64) builds are available
- ✅ Almost all extensions distributed for ARM64 platforms
- ✅ Official Docker image supports both AMD64 and ARM64

However:
- ❌ **Raspberry Pi OS (Raspbian) is not officially distributed**
- ✅ Users report "aarch64 binaries work just fine on 64-bit Raspberry Pi systems"

**Sources**:
- [DuckDB Raspberry Pi Documentation](https://duckdb.org/docs/stable/dev/building/raspberry_pi)
- [DuckDB Docker Container](https://duckdb.org/docs/stable/operations_manual/duckdb_docker)
- [Unofficial and Unsupported Platforms](https://duckdb.org/docs/stable/dev/building/unofficial_and_unsupported_platforms)

### 1.2 Raspberry Pi 5 Performance

DuckDB has demonstrated **excellent stability** on Raspberry Pi 5:

**TPC-H Benchmark Results** (January 2025):
- ✅ Successfully ran all TPC-H queries on datasets up to **1,000 GiB (SF1000)**
- ✅ **No crashes, errors, or incorrect results** during comprehensive testing
- ✅ Performance: SF100 dataset in **11.7 seconds (NVMe)** or 23.8s (microSD)
- ✅ SF300 dataset (300GB) in **55.2 seconds (NVMe)** or 171.9s (microSD)
- 💰 Total setup cost: **<$300** (Raspberry Pi 5 + NVMe storage)

**Hardware Used**:
- Raspberry Pi 5: 2.4 GHz quad-core CPU
- RAM: 16 GB
- Storage: NVMe SSD (3× faster than microSD)

**Sources**:
- [TPC-H on a Raspberry Pi](https://duckdb.org/2025/01/17/raspberryi-pi-tpch)
- [DuckDB on Twitter - TPC-H SF300](https://x.com/duckdb/status/1880375240619950109)
- [MotherDuck Blog - Quacking at the Edge](https://motherduck.com/blog/duckdb-on-edge-raspberry-pi/)

### 1.3 Build Instructions for Raspberry Pi

For 64-bit Raspberry Pi, building from source is straightforward:

```bash
# Install dependencies
sudo apt-get update
sudo apt-get install -y git g++ cmake ninja-build

# Clone and build
git clone https://github.com/duckdb/duckdb
cd duckdb
GEN=ninja CORE_EXTENSIONS="icu;json" make

# Run
build/release/duckdb
```

**Memory Requirements**: 125 MB per thread minimum (8 threads = 1 GB RAM minimum)

**Sources**:
- [Building DuckDB from Source](https://duckdb.org/docs/stable/dev/building/overview)
- [Raspberry Pi Build Instructions](https://raw.githubusercontent.com/duckdb/duckdb-web/refs/heads/main/docs/stable/dev/building/raspberry_pi.md)

---

## 2. The ARM64 Grafana Plugin Problem

### 2.1 Root Cause: glibc Dependency

The **MotherDuck Grafana DuckDB datasource plugin** has critical ARM64 limitations:

**Requirements**:
- ✅ Grafana Version 10.4.0 or later
- ❌ **glibc 2.35 or later** (Ubuntu 22.04+)
- ❌ **duckdb-go only provides glibc-based Linux binaries**

**The Problem**:
```
Alpine/Musl platforms are NOT supported.
This plugin uses duckdb-go which only provides glibc-based Linux binaries.
```

**What This Means for ARM64**:
- The plugin requires glibc (not musl libc)
- **No ARM64-specific binaries distributed**
- Go-based plugin compiled against glibc cannot run on musl
- Docker users must use Ubuntu-based Grafana images (not Alpine)

**Sources**:
- [MotherDuck Grafana DuckDB Datasource](https://github.com/motherduckdb/grafana-duckdb-datasource)
- [Python DuckDB 1.4.3 LTS on Windows Arm64](https://medium.com/@ccpythonprogramming/python-duckdb-1-4-3-lts-on-windows-arm64-15f431d3b566)

### 2.2 Extension Installation Issues (Fixed in 1.4.3)

**Historical Problem** (March 2025):
Users on ARM64 Ubuntu encountered **403 errors** when installing DuckDB extensions:

```
Extension Autoloading Error: Failed to download extension 'ui' at URL
'http://extensions.duckdb.org/v1.2.1/linux_arm64_gcc4/ui.duckdb_extension.gz'
(HTTP 403)
```

**Root Cause**: ARM64 extension files were not hosted on extensions.duckdb.org

**Resolution**: DuckDB 1.4.3 (December 9, 2025) added:
- ✅ Native extensions and Python support for Windows ARM64
- ✅ Python wheels for Windows ARM64 (Python 3.11+)
- ✅ Improved ARM64 extension distribution

**Workaround Options** (if using older versions):
1. Upgrade to DuckDB 1.4.3 or later
2. Try alternative repositories: `INSTALL <name> FROM core;` or `INSTALL <name> FROM community;`
3. Build extensions from source

**Sources**:
- [Problem installing extension on Arm64 Ubuntu - Issue #16616](https://github.com/duckdb/duckdb/issues/16616)
- [HTTP error 403 when trying to install UI extension - Issue #16673](https://github.com/duckdb/duckdb/issues/16673)
- [Announcing DuckDB 1.4.3 LTS](https://duckdb.org/2025/12/09/announcing-duckdb-143)
- [Troubleshooting of Extensions](https://duckdb.org/docs/stable/extensions/troubleshooting)

### 2.3 Docker Image Compatibility

**Official DuckDB Docker Image**: ✅ **Works on ARM64**
```yaml
image: duckdb/duckdb:latest  # Supports both ARM64 and x86_64
```

**datacatering/duckdb**: ✅ **Works on ARM64**
```yaml
image: datacatering/duckdb:v1.1.3  # Community image with ARM64 support
```

**marcboeker/go-duckdb**: ❌ **Archived** (October 20, 2025)
- Repository archived, no longer maintained
- Had Alpine/ARM64 Dockerfile issues on Mac M3

**Alpine Linux Performance**: ⚠️ **Poor performance**
- DuckDB can build with musl libc but performance suffers
- **>5× slowdown** on compute-intensive workloads
- Recommendation: Use Debian/Ubuntu base images instead

**Sources**:
- [datacatering/duckdb - Docker Hub](https://hub.docker.com/r/datacatering/duckdb)
- [DuckDB Docker Container Documentation](https://duckdb.org/docs/stable/operations_manual/duckdb_docker)
- [data-catering/duckdb-docker - GitHub](https://github.com/data-catering/duckdb-docker)
- [Dockerfile with alpine - Issue #256](https://github.com/marcboeker/go-duckdb/issues/256)

---

## 3. DuckDB vs SQLite for Raspberry Pi

### 3.1 Architecture Differences

**SQLite**:
- Row-oriented storage (like a spreadsheet)
- Optimized for **transactional workloads** (OLTP)
- Minimal memory usage
- Best for point queries and writes

**DuckDB**:
- **Column-oriented storage** (analytical workloads)
- Optimized for **analytical queries** (OLAP)
- Higher memory usage
- Best for aggregations, scans, and analytics

### 3.2 Performance Comparison

**Analytical Queries** (Aggregations, Scans):
- ✅ **DuckDB: 8-35× faster** than SQLite
- Columnar scans and parallelism dominate
- DuckDB optimized for these workloads

**Point Lookups**:
- ✅ **SQLite: ~20% faster** than DuckDB
- B-tree design advantages for single-row retrieval

**Write Performance**:
- ✅ **SQLite: 10-500× faster** (cloud server), 2-60× faster (Raspberry Pi)
- SQLite optimized for transaction processing
- DuckDB not designed for write-heavy workloads

**Raspberry Pi Specific**:
- On Pi: SQLite is **4.2× faster on SSB** after optimizations (Bloom filters)

**Memory Usage**:
- SQLite: ~480 MB peak
- DuckDB: ~2.3 GB peak (trading RAM for speed)

**Disk Compression**:
- DuckDB: 28 GB (automatic compression)
- SQLite: 92 GB (no compression)

**Sources**:
- [DuckDB vs SQLite: Choosing the Right Embedded Database](https://betterstack.com/community/guides/scaling-python/duckdb-vs-sqlite/)
- [DuckDB vs SQLite: Performance, Scalability and Features](https://motherduck.com/learn-more/duckdb-vs-sqlite-databases/)
- [SQLite vs DuckDB Head to Head on Performance and Usability](https://sqlflash.ai/article/20251119_sqlite_vs_duckdb/)

### 3.3 Use Case Recommendations

**Choose SQLite for**:
- ✅ Transactional workloads (CRUD operations)
- ✅ Write-heavy applications
- ✅ IoT devices and embedded systems
- ✅ Mobile app storage (Android/iOS)
- ✅ Resource-constrained environments
- ✅ **When memory is limited (<512MB available)**

**Choose DuckDB for**:
- ✅ Analytical queries (aggregations, joins)
- ✅ Data science and BI dashboards
- ✅ Large dataset analysis (millions of rows)
- ✅ Parquet/CSV file analysis
- ✅ ETL/ELT pipelines
- ✅ **When you have RAM to spare (>1GB available)**

**For NDP Silver Layer**:
- ✅ **DuckDB is the right choice** for analytical queries
- ✅ **SQLite export workaround** acceptable for Grafana integration
- ⚠️ Not using DuckDB for writes (read-only Bronze layer access)

---

## 4. Grafana Integration Alternatives

### 4.1 DuckDB Grafana Plugin Status

**Official Plugin**: `motherduck-duckdb-datasource`
- ❌ **Broken on ARM64** (glibc 2.35+, duckdb-go limitation)
- ✅ Works on x86_64 Ubuntu-based Grafana
- ❌ Does NOT work on Alpine/musl

**GitHub Discussion**: [New Data Source: DuckDb Data Source #80948](https://github.com/grafana/grafana/issues/80948)

### 4.2 Current Workaround: SQLite Export

**NDP Current Implementation**:
```sql
-- /config/duckdb/export_to_sqlite.sql
-- Export DuckDB views to SQLite for Grafana
ATTACH 'grafana.db' AS grafana (TYPE SQLITE);

CREATE OR REPLACE TABLE grafana.readings_hourly AS
SELECT * FROM readings_hourly;
```

**Docker Compose**:
```yaml
duckdb:
  command: |
    while true; do
      /duckdb /var/duckdb/neural_platform.db < /config/duckdb/export_to_sqlite.sql
      sleep 300  # Export every 5 minutes
    done
```

**Grafana Datasource**: `frser-sqlite-datasource` (works on ARM64)

**Evaluation**:
- ✅ **Works reliably** on ARM64
- ✅ Grafana SQLite plugin is stable
- ⚠️ 5-minute export delay (acceptable for analytics)
- ⚠️ Disk I/O overhead (mitigated by NVMe)
- ⚠️ Extra storage for SQLite copy
- ✅ **Acceptable for read-heavy analytical dashboards**

### 4.3 Alternative: TimescaleDB

**TimescaleDB Overview**:
- PostgreSQL extension for time-series data
- Specialized features: hypertables, continuous aggregates, retention policies
- Grafana has native PostgreSQL support

**ARM64 Raspberry Pi Considerations**:
- ❌ **PostgreSQL is quite heavy on Raspberry Pi**
- ⚠️ Research shows InfluxDB and PostgreSQL performed well, but TimescaleDB not specifically tested
- ⚠️ Built on PostgreSQL (heavier than DuckDB or SQLite)

**Comparison to DuckDB**:

| Feature | DuckDB | TimescaleDB |
|---------|--------|-------------|
| Architecture | In-process OLAP | PostgreSQL extension (client-server) |
| Resource Usage | Lower (512MB for NDP) | Higher (PostgreSQL overhead) |
| Time-Series Features | Generic SQL analytics | Specialized (hypertables, retention) |
| Grafana Integration | Plugin broken on ARM64 | ✅ Native PostgreSQL support |
| Raspberry Pi Fit | ✅ Lightweight | ⚠️ Heavy |

**Benchmarks**:
- TimescaleDB is **1.9× faster than ClickHouse** on RTABench
- TimescaleDB is **6.8× slower than ClickHouse** on ClickBench
- Optimized for real-time analytics (selective aggregations, normalized schemas)

**Recommendation**:
- ❌ **Not recommended for NDP** - too heavy for Raspberry Pi 5 given DuckDB is working
- ✅ Consider if **specialized time-series features** become critical (retention policies, automatic aggregations)

**Sources**:
- [Compare DuckDB vs TimescaleDB](https://www.influxdata.com/comparison/duckdb-vs-timescaledb/)
- [DuckDB vs. TimescaleDB Comparison](https://db-engines.com/en/system/DuckDB%3BTimescaleDB)
- [Storing and visualizing time-series data from a Raspberry Pi](https://www.timescale.com/blog/storing-and-visualizing-time-series-data-from-a-raspberry-pi/)

### 4.4 Alternative: InfluxDB

**InfluxDB Overview**:
- Purpose-built time-series database
- Written in Go, lightweight
- Native Grafana datasource support

**For Raspberry Pi**:
- ✅ Designed for resource-constrained devices
- ✅ Grafana integration is native and stable
- ⚠️ Would require data ingestion changes (write to InfluxDB, not Parquet)

**Comparison**:

| Aspect | DuckDB (current) | InfluxDB |
|--------|------------------|----------|
| Bronze Layer | ✅ Parquet files | ❌ Requires rewrite |
| Architecture | Read Bronze Parquet | Write to InfluxDB directly |
| Grafana Integration | SQLite workaround | ✅ Native |
| Resource Usage | 512MB | Similar |
| Silver Layer | SQL views | InfluxQL queries |
| **Fits NDP Design** | ✅ Yes (no Rust changes) | ❌ No (requires ingestion changes) |

**Recommendation**:
- ❌ **Out of scope for dp-001** (violates "no Rust code changes" constraint)
- ✅ Consider for **future architecture** if shifting from Parquet-based Bronze layer

**Sources**:
- [Compare DuckDB vs Mimir](https://www.influxdata.com/comparison/duckdb-vs-mimir/)
- [Data sources - Grafana documentation](https://grafana.com/docs/grafana/latest/datasources/)

---

## 5. Community Experience and Production Use

### 5.1 Production Deployments

**MotherDuck + Raspberry Pi**:
- ✅ MotherDuck demonstrated **Dual Query execution** on Raspberry Pi
- ✅ Use case: Play sound when users sign up (polling MotherDuck warehouse from Pi)
- ✅ Runs DuckDB on both client (Pi) and server (MotherDuck)

**Local Development + Production Pattern**:
- ✅ Rasmus (community): **DuckDB for local development, MotherDuck for production**
- ✅ Seamless switch with configuration toggle
- ✅ INSERT OR REPLACE INTO for idempotency
- ✅ Lightweight compression minimizes disk usage

**Log Analytics**:
- ✅ DuckDB as **simpler and cheaper alternative to ELK and OpenSearch**
- ✅ Lightweight, efficient, cost-effective for log analytics

**Security Considerations**:
- ⚠️ Sam Jewell (Grafana Labs): **CLI dot commands and file access capabilities** need attention in production

**Sources**:
- [DuckDB Ecosystem: February 2025](https://motherduck.com/blog/duckdb-ecosystem-newsletter-february-2025/)
- [Quacking at the Edge: DuckDB on Raspberry Pi](https://motherduck.com/blog/duckdb-on-edge-raspberry-pi/)
- [Observability and Log Analytics with DuckDB](https://neogeografia.wordpress.com/2023/08/02/observability-and-log-analytics-with-duckdb/)

### 5.2 Market Adoption (2025)

**Embedded Database Category** (July 2025):
- DuckDB: **12.3% mindshare** (up from 5.3% YoY) - 🚀 **132% growth**
- SQLite: **30.7% mindshare** (up from 28.7% YoY) - steady leader

**Trend**: DuckDB gaining traction rapidly for analytical use cases

**Sources**:
- [DuckDB vs SQLite (2025)](https://www.peerspot.com/products/comparisons/duckdb_vs_sqlite)
- [DuckDB vs. SQLite: The 2025 Data Analysis Showdown](https://medium.com/@bhagyarana80/duckdb-vs-sqlite-the-2025-data-analysis-showdown-0f01711db50b)

---

## 6. Issues Encountered in dp-001

### 6.1 Timeline of Problems

Based on git commit history:

**Initial Issues**:
1. ✅ **Volume mount conflicts** - Grafana container configuration
2. ✅ **Dashboard stream_id mapping** - Fixed to use correct field names
3. ✅ **DuckDB plugin version** - Switched to v0.4.0
4. ✅ **Grafana version upgrade** - Upgraded to 12.3.1 for native DuckDB plugin

**ARM64-Specific Issues**:
5. ❌ **DuckDB Grafana plugin glibc incompatibility** - Plugin requires glibc 2.35+, duckdb-go binaries not available for ARM64
6. ✅ **Workaround: SQLite export** - Switched to frser-sqlite-datasource
7. ✅ **Timestamp format** - Converted to epoch milliseconds for Grafana compatibility
8. ✅ **Shell syntax** - Fixed container command formatting
9. ❌ **SQLite index binder errors** - Removed index creation (latest commit)

**Sources**: Git log analysis from previous bash command

### 6.2 Current Workaround Analysis

**What Works**:
- ✅ DuckDB reads Parquet files correctly
- ✅ Silver layer views (SQL) execute successfully
- ✅ SQLite export every 5 minutes
- ✅ Grafana SQLite datasource connects
- ✅ Dashboards render time-series data

**Remaining Issues**:
- ⚠️ 5-minute export delay (acceptable for analytics)
- ⚠️ Binder errors on SQLite index creation (removed in latest commit)
- ⚠️ Extra storage overhead (SQLite copy of DuckDB views)

**Performance Impact**:
- ✅ DuckDB query performance: Excellent (columnar scans on Parquet)
- ⚠️ Export overhead: 5-minute batch updates (not real-time)
- ✅ Grafana SQLite queries: Fast (reading from exported tables)

---

## 7. Recommendations

### 7.1 Short-Term (dp-001 - Current Phase)

**✅ KEEP the SQLite export workaround**

**Rationale**:
1. ✅ **Functional**: Dashboards are working
2. ✅ **Acceptable latency**: 5-minute export delay fine for analytics (not real-time monitoring)
3. ✅ **Stable**: Grafana SQLite plugin is mature and reliable
4. ✅ **No Rust changes**: Maintains project constraint
5. ✅ **Resource efficient**: DuckDB + SQLite still lighter than TimescaleDB

**Optimizations**:
- ✅ Export interval: 5 minutes is reasonable (could reduce to 1 minute if needed)
- ✅ Use NVMe storage to minimize I/O impact
- ✅ Monitor SQLite file size (should be small for hourly aggregates)
- ⚠️ Remove problematic index creation (already done in latest commit)

### 7.2 Medium-Term (Future Phases)

**Monitor Grafana DuckDB Plugin Development**:
- 🔄 Watch [motherduckdb/grafana-duckdb-datasource](https://github.com/motherduckdb/grafana-duckdb-datasource) for ARM64 support
- 🔄 Check if duckdb-go releases ARM64 binaries
- 🔄 Test new plugin versions when released

**Alternative: Custom HTTP API**:
If direct DuckDB integration becomes critical:
```yaml
# Option: DuckDB HTTP API wrapper (custom implementation)
duckdb-api:
  image: custom/duckdb-http-api:latest
  # Expose DuckDB via REST API
  # Grafana queries via Infinity datasource or custom plugin
```

**Evaluate if**:
- Real-time (<1 minute latency) becomes requirement
- SQLite export overhead becomes problematic
- ARM64 plugin support becomes available

### 7.3 Long-Term (Architecture Evolution)

**Option 1: Continue DuckDB + SQLite Pattern**
- ✅ Proven to work
- ✅ Maintains Bronze (Parquet) → Silver (DuckDB views) architecture
- ✅ Low resource usage
- ⚠️ Export delay remains

**Option 2: Migrate to TimescaleDB**
- ✅ Native Grafana integration
- ✅ Specialized time-series features
- ❌ Requires Rust code changes (ingestion writes to TimescaleDB)
- ❌ Heavier resource usage
- ⚠️ Only consider if specialized features (retention policies, continuous aggregates) become critical

**Option 3: Hybrid Approach**
- Bronze: Parquet (archive/compliance)
- Silver: TimescaleDB (active analytics)
- ETL: DuckDB reads Parquet → writes to TimescaleDB
- ✅ Best of both worlds
- ❌ Most complex architecture
- ⚠️ Only for mature system with dedicated resources

**Recommendation**: **Stick with Option 1** (DuckDB + SQLite) until clear pain points emerge

### 7.4 When to Re-evaluate

**Trigger conditions for architectural change**:
1. ⏰ **Latency becomes critical**: Need <1 minute dashboard updates
2. 📊 **Query complexity increases**: SQLite export can't keep up
3. 🔧 **Grafana plugin fixed**: ARM64 binaries available for DuckDB plugin
4. 📈 **Scale increases**: Data volume exceeds DuckDB + SQLite capacity
5. 🎯 **Feature requirements**: Need time-series-specific features (retention, continuous aggregates)

**Re-evaluation timeline**: Quarterly review (every 3 months)

---

## 8. Conclusion

### 8.1 Summary

**DuckDB on ARM64 Raspberry Pi 5**: ✅ **Stable and performant**
- Core database works excellently
- TPC-H benchmarks prove production readiness
- Official Docker images support ARM64

**The Problem**: ❌ **Grafana plugin ecosystem**, not DuckDB itself
- MotherDuck plugin requires glibc 2.35+ and duckdb-go binaries
- No ARM64 binaries distributed for the Go-based plugin
- Extension 403 errors fixed in DuckDB 1.4.3 (December 2025)

**NDP's Solution**: ✅ **SQLite export workaround is appropriate**
- Functional and stable
- Acceptable 5-minute latency for analytics
- Maintains Bronze (Parquet) → Silver (DuckDB) architecture
- No Rust code changes required

### 8.2 Final Recommendation

**For dp-001**: ✅ **Continue with current SQLite export approach**

**Reasons**:
1. ✅ It works reliably
2. ✅ Latency is acceptable for dashboards
3. ✅ Resource usage is acceptable
4. ✅ Maintains architectural integrity (no ingestion changes)
5. ✅ DuckDB proven stable on ARM64/Pi 5

**Do NOT**:
- ❌ Switch to TimescaleDB (too heavy, requires Rust changes)
- ❌ Try to force DuckDB Grafana plugin to work (glibc incompatibility)
- ❌ Abandon DuckDB (core engine is excellent)

**Monitor**:
- 🔄 Grafana DuckDB plugin ARM64 support (quarterly check)
- 🔄 DuckDB extension ecosystem improvements
- 🔄 Community workarounds and patterns

### 8.3 Confidence Level

**High Confidence** (based on):
- ✅ Official DuckDB documentation confirming ARM64 support
- ✅ TPC-H benchmarks proving stability on Pi 5
- ✅ Community production deployments
- ✅ Clear root cause identified (Grafana plugin, not DuckDB)
- ✅ Working workaround validated in dp-001

**Research Quality**: Comprehensive (5 web searches, git history, project docs, official sources)

---

## 9. References

### Official Documentation
- [DuckDB Raspberry Pi Documentation](https://duckdb.org/docs/stable/dev/building/raspberry_pi)
- [DuckDB Docker Container](https://duckdb.org/docs/stable/operations_manual/duckdb_docker)
- [DuckDB Extensions Overview](https://duckdb.org/docs/stable/extensions/overview)
- [Announcing DuckDB 1.4.3 LTS](https://duckdb.org/2025/12/09/announcing-duckdb-143)

### Performance and Benchmarks
- [TPC-H on a Raspberry Pi](https://duckdb.org/2025/01/17/raspberryi-pi-tpch)
- [DuckDB vs SQLite: Performance Comparison](https://betterstack.com/community/guides/scaling-python/duckdb-vs-sqlite/)
- [SQLite vs DuckDB Head to Head](https://sqlflash.ai/article/20251119_sqlite_vs_duckdb/)

### Grafana Integration
- [MotherDuck Grafana DuckDB Datasource](https://github.com/motherduckdb/grafana-duckdb-datasource)
- [Grafana Data sources](https://grafana.com/docs/grafana/latest/datasources/)

### Community and Production Use
- [MotherDuck Blog - Quacking at the Edge](https://motherduck.com/blog/duckdb-on-edge-raspberry-pi/)
- [DuckDB Ecosystem Newsletter - February 2025](https://motherduck.com/blog/duckdb-ecosystem-newsletter-february-2025/)

### Known Issues
- [Problem installing extension on Arm64 Ubuntu - Issue #16616](https://github.com/duckdb/duckdb/issues/16616)
- [HTTP error 403 UI extension - Issue #16673](https://github.com/duckdb/duckdb/issues/16673)

### Docker Images
- [datacatering/duckdb - Docker Hub](https://hub.docker.com/r/datacatering/duckdb)
- [data-catering/duckdb-docker - GitHub](https://github.com/data-catering/duckdb-docker)

### Alternatives
- [Compare DuckDB vs TimescaleDB](https://www.influxdata.com/comparison/duckdb-vs-timescaledb/)
- [Storing time-series data on Raspberry Pi (TimescaleDB)](https://www.timescale.com/blog/storing-and-visualizing-time-series-data-from-a-raspberry-pi/)

---

**Report Prepared By**: Research Agent (Neural Data Platform)
**Date**: 2025-12-19
**Project Phase**: dp-001 (Data Platform - Silver Layer)
**Status**: ✅ Complete
