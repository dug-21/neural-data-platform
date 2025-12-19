# Data Lake Patterns for Raspberry Pi 5 Edge Deployment

**Research Date**: 2025-12-19
**Focus**: Viable lakehouse architectures for edge deployment (Raspberry Pi 5, 16GB RAM, ARM64)
**Goal**: Pattern that scales from Pi to cloud without architectural rewrites

---

## Executive Summary

After researching modern data lake patterns for edge deployment, the clear winner for Raspberry Pi 5 is **DuckDB-based virtual lakehouse** (DuckLake pattern), with **Apache DataFusion** as a strong alternative for Rust-native integration.

**Key Finding**: Traditional lakehouse formats (Delta Lake, Iceberg) are overkill for edge deployment. The "virtual data layer" approach using query engines over Parquet provides 80-90% cost savings, 10x better performance on small hardware, and seamless cloud migration.

### Recommended Pattern

```
Bronze (Parquet files) → DuckDB/DataFusion (virtual Silver) → Gold (aggregates)
```

This pattern:
- Runs natively on ARM64 with minimal overhead
- Processes datasets 10GB-100TB (the "forgotten middle")
- Uses standard Parquet (cloud-portable)
- Requires no Java/JVM dependencies
- Provides ACID-like guarantees through query engine features

---

## 1. Lakehouse Architecture Options

### 1.1 Traditional Lakehouse Formats

#### Delta Lake
**Verdict**: ❌ Not viable for Pi edge deployment

**Research Findings**:
- Original Delta Lake requires Java/Spark (too heavy for Pi)
- `delta-rs` (Rust implementation) exists but adds complexity
- Designed for distributed systems, not single-node edge
- 2M+ PyPI downloads/month shows popularity, but for different use cases

**delta-rs Details**:
- Native Rust library with Python bindings
- No Java/Spark dependencies (major advantage)
- Supports ACID transactions, schema evolution
- However: adds metadata layer complexity for minimal edge benefit

**When to Use**: Cloud migration phase, not initial Pi deployment

**Sources**:
- [delta-rs GitHub](https://github.com/delta-io/delta-rs)
- [delta-rs Documentation](https://delta-io.github.io/delta-rs/)
- [Lessons learned building delta-rs](https://www.buoyantdata.com/blog/2025-03-09-lessons-learned-building-delta-rs.html)

#### Apache Iceberg
**Verdict**: ❌ Not viable for Pi edge deployment

**Research Findings**:
- Designed for distributed cloud architectures
- Heavy metadata management overhead
- No ARM64-specific optimizations found
- Moving toward "modern runtimes, cloud native storage" (not edge-first)

**2025 Developments**:
- V3 spec with binary deletion vectors (Roaring bitmaps)
- Default column values for schema evolution
- Scan planning endpoints, materialized views
- All optimized for cloud-scale, not edge-scale

**When to Use**: Cloud lakehouse after proven edge pattern

**Sources**:
- [10 Future Apache Iceberg Developments (2025)](https://medium.com/data-engineering-with-dremio/10-future-apache-iceberg-developments-to-look-forward-to-in-2025-7292a2a2101d)
- [Apache Iceberg 2025 Guide](https://dev.to/alexmercedcoder/the-2025-comprehensive-guide-to-apache-iceberg-2g22)
- [What's new in Iceberg v3](https://opensource.googleblog.com/2025/08/whats-new-in-iceberg-v3.html)

---

## 2. Virtual Data Layer Patterns (RECOMMENDED)

### 2.1 DuckDB (Top Recommendation)

**Verdict**: ✅ **OPTIMAL for Pi edge deployment**

#### Why DuckDB Wins

**Architecture**:
- Embedded analytics database (like SQLite for OLAP)
- Columnar, vectorized execution
- Native Parquet/Arrow support
- Zero-dependency single binary

**ARM64 Performance** (January 2025 Benchmarks):
- Raspberry Pi 5 (16GB RAM, NVMe SSD)
- SF300 dataset (300GB): 55.2 second geometric mean
- 3x faster with NVMe vs microSD
- Memory requirement: 1-4GB per thread (minimum 125MB/thread)

**Memory Management**:
- Streaming execution engine (small chunks)
- Automatic spilling to disk for larger-than-memory workloads
- Works with both persistent and in-memory databases
- Built-in out-of-core processing

**DuckLake Pattern** (2025 Innovation):
- New open table format using SQL database for metadata
- Standard Parquet files for storage (not custom format)
- Metadata in standard SQL tables (PostgreSQL, SQLite, MySQL, or DuckDB)
- Simpler than JSON/Avro metadata layers
- Status: Experimental (June 2025), production-ready expected late 2025

**Performance vs Competitors**:
- Generally fastest for embedded analytics
- 10x faster than managed lakehouse solutions on same hardware
- 80-90% cheaper than traditional data warehouses
- Beats DataFusion and Polars in most benchmarks

**Edge-Specific Benefits**:
- Perfect for "forgotten middle" (10GB-100TB datasets)
- Single-file database philosophy
- Runs in-process (no server needed)
- Local development, CI/CD, production-ready

**Sources**:
- [TPC-H on Raspberry Pi](https://duckdb.org/2025/01/17/raspberryi-pi-tpch)
- [DuckDB Memory Management](https://duckdb.org/2024/07/09/memory-management)
- [Quacking at the Edge: DuckDB on Raspberry Pi](https://motherduck.com/blog/duckdb-on-edge-raspberry-pi/)
- [DuckLake: Lakehouse Architecture Reimagined](https://endjin.com/blog/2025/06/introducing-ducklake-lakehouse-architecture-reimagined-modern-era)
- [DuckLake Step-by-Step Guide](https://medium.com/@aya.space/ducklake-step-by-step-build-a-full-lakehouse-with-just-parquet-files-and-duckdb-f755bdb76389)
- [DuckDB Medallion Architecture Guide](https://medium.com/@datatomas/duckdb-medallion-architecture-a-complete-local-lakehouse-guide-0f1944b6bcdf)

### 2.2 Apache DataFusion (Strong Alternative)

**Verdict**: ✅ **EXCELLENT for Rust-native integration**

#### Why DataFusion is Compelling

**Architecture**:
- Extensible query engine written in Rust
- Apache Arrow in-memory format
- Designed for building custom databases/query engines
- Columnar, streaming, multi-threaded execution

**Performance**:
- Fastest single-node engine for Parquet queries (ClickBench)
- First Rust-based engine to beat C/C++ engines (DuckDB, ClickHouse)
- 1.5x improvement for TPC-H-style queries
- Most sophisticated open-source Parquet reader

**Parquet Optimizations**:
- Projection pushdown
- Predicate pushdown (row group metadata, page index, bloom filters)
- Limit pushdown
- Parallel reading
- Interleaved I/O
- Late materialized filtering

**2025 Enhancements**:
- DataFusion 51.0.0: faster Parquet metadata parsing (Arrow Rust 57.0.0)
- Encrypted Parquet support (modular encryption)
- Benefits workloads with many small files
- Low latency, fast startup time

**Integration Benefits**:
- Pure Rust (matches NDP stack)
- Easily embeddable in neural-core library
- Extensible for custom analytics
- Native ARM64 support (Rust toolchain)

**Trade-offs vs DuckDB**:
- Slightly slower in some benchmarks (but competitive)
- More complex to integrate (requires building, not just embedding)
- Better for custom query engines than ad-hoc analytics
- Uses Tokio async runtime (not optimized for compute-heavy workloads)

**When to Use**:
- Rust-native integration is priority
- Building custom analytics engine
- Need extensibility over simplicity
- Want to avoid any C++ dependencies

**Sources**:
- [Apache DataFusion GitHub](https://github.com/apache/datafusion)
- [DataFusion 51.0.0 Release](https://datafusion.apache.org/blog/2025/11/25/datafusion-51.0.0/)
- [DataFusion: Fastest Single Node Parquet Engine](https://datafusion.apache.org/blog/2024/11/18/datafusion-fastest-single-node-parquet-clickbench/)
- [Beyond Postgres and DuckDB: Composable Query Engines](https://thinhdanggroup.github.io/composable-query-engines-with-polars-and-datafusion/)

### 2.3 Polars

**Verdict**: ⚠️ **Good, but not optimal for NDP use case**

#### Why Polars is Interesting

**Architecture**:
- OLAP query engine with DataFrame API
- Written in Rust with Python bindings
- Apache Arrow memory representation
- Multi-threaded execution (Rayon backend)

**Performance**:
- 30x faster than pandas
- 3-7x faster with new streaming engine
- Can process datasets with only 512MB RAM
- "Could trade laptop for Raspberry Pi" efficiency claim

**Streaming Capabilities**:
- Processes larger-than-memory datasets
- Lazy API with query optimization
- Projection and predicate pushdown

**Edge Viability**:
- ARM64 support via `pip install polars[rtcompat]` (no AVX instructions)
- Efficient on resource-constrained hardware
- Proven on minimal VMs (€4/month with 512MB RAM)

**Trade-offs**:
- DataFrame API vs SQL (less familiar for analytics)
- Slower than DuckDB in most benchmarks
- Varied core utilization (not full saturation like DuckDB)
- Better for Python data pipelines than embedded analytics

**When to Use**:
- Python-first development
- DataFrame API preferred over SQL
- Data pipeline orchestration

**Sources**:
- [Polars Official Site](https://pola.rs/)
- [Polars Benchmarks (May 2025)](https://pola.rs/posts/benchmarks/)
- [Working with Large Datasets on Tiny Machine](https://r-brink.medium.com/working-with-large-datasets-300m-on-a-tiny-machine-512mb-ram-1-core-6d1553e474df)
- [DuckDB vs Polars Performance](https://www.codecentric.de/en/knowledge-hub/blog/duckdb-vs-dataframe-libraries)

---

## 3. Medallion Architecture for Edge

### 3.1 Pattern Overview

**Standard Medallion Layers**:

```
Bronze (Raw)     → Parquet files, append-only, unmodified data
Silver (Refined) → Cleaned, structured, validated, "enterprise view"
Gold (Curated)   → Aggregated, denormalized, reporting-optimized
```

**Origin**: Databricks invention, adopted by Microsoft OneLake

**Sources**:
- [Medallion Architecture (Azure Databricks)](https://learn.microsoft.com/en-us/azure/databricks/lakehouse/medallion)
- [What is Medallion Architecture?](https://www.databricks.com/glossary/medallion-architecture)
- [Medallion Architecture 101 (2025)](https://www.chaosgenius.io/blog/medallion-architecture/)

### 3.2 Edge Adaptation (NDP Pattern)

**Recommended Minimal Implementation**:

```
Bronze:  Parquet files (existing NDP implementation ✓)
         - Source: Sensors, external APIs
         - Format: Columnar Parquet (ARM64-optimized)
         - Storage: Local NVMe SSD on Pi

Silver:  DuckDB virtual layer (RECOMMENDED)
         - Query engine over Bronze Parquet
         - Transformations via SQL views
         - Optional: Materialized views for hot paths
         - No data duplication (query-time processing)

Gold:    Aggregated Parquet or DuckDB tables
         - Pre-computed rollups (hourly, daily)
         - Denormalized for Grafana queries
         - Small footprint (aggregates only)
```

**Why Virtual Silver?**

1. **Memory Efficiency**: No data duplication, process on-demand
2. **ARM64 Performance**: DuckDB optimized for streaming
3. **Simplicity**: No ETL jobs, just SQL transformations
4. **Cloud Portability**: Parquet files move unchanged to cloud
5. **Cost**: 80-90% cheaper than traditional approaches

**2025 Best Practices**:
- Use framework as guideline, not rulebook
- Adapt to team expertise and constraints
- Don't dogmatically apply Bronze/Silver/Gold
- Consider adding Feature layer for ML
- Minimize environmental sprawl (DEV/QA/STG/PROD)

**Sources**:
- [Beyond Bronze, Silver, Gold: Evolving for AI Era](https://medium.com/@vishal.dutt.data.architect/beyond-bronze-silver-gold-evolving-the-medallion-architecture-for-the-ai-era-77d3cca78745)
- [Medallion Architecture Best Practices](https://bix-tech.com/medallion-architecture-explained-how-bronzesilvergold-layers-supercharge-your-data-lakehouse-mesh-and-data-quality/)

---

## 4. Edge-Specific Considerations

### 4.1 Raspberry Pi 5 Capabilities (2025)

**Hardware**:
- CPU: Quad-core ARM Cortex-A76 @ 2.4 GHz
- RAM: Up to 16GB LPDDR4 (NDP has 16GB ✓)
- Storage: M.2 NVMe SSD support
- Docker: ARM64 support for containerized workloads
- TPU: Optional Coral TPU for acceleration

**Performance**:
- Can run 4-7B parameter LLM models (4-bit quantization)
- DuckDB SF300 (300GB): 55 second queries with NVMe
- Proven for edge computing, home labs, lightweight production

**Sources**:
- [Raspberry Pi Ecosystem 2025](https://www.ics.com/blog/look-back-raspberry-pi-ecosystem-2025)
- [Edge LLM Deployment 2025](https://kodekx-solutions.medium.com/edge-llm-deployment-on-small-devices-the-2025-guide-2eafb7c59d07)

### 4.2 Edge Security Requirements

**Critical for Production**:
- Filesystem encryption (MUST HAVE)
- Hardware security module or secure element
- Physically accessible flash storage is major attack vector
- Keys must not be on local storage

**Why This Matters**:
- Edge devices in distributed, unsecured environments
- Easy to perform physical attacks
- Can escalate to complete device/fleet compromise

**Sources**:
- [Raspberry Pi Deployment Considerations](https://www.zymbit.com/2025/03/10/things-to-consider-before-deploying-raspberry-pi/)

### 4.3 IoT Time Series Scale

**Context**:
- 75+ billion IoT devices by 2025 (estimated)
- 175 zettabytes annual IoT data volume (IBM estimate)
- Need for real-time edge analytics
- Minimize cloud data transmission (bandwidth/cost)

**Edge Processing Benefits**:
- Latency: Milliseconds for predictive maintenance
- Cost: Process locally, send only anomalies/aggregates
- Resilience: Continues with intermittent connectivity
- Privacy: Minimize sensitive data movement

**Lightweight Lakehouse Pattern**:
- Edge devices monitor sensor streams
- Local inference detects anomalies
- Flag events sync to central lakehouse when connected
- Hybrid sync: materialize results locally, sync when able

**Sources**:
- [IoT Time Series Lakehouse](https://dview.io/blog/iot-data-management-lakehouse-integration)
- [Minimal Lakehouse Edge Computing](https://celerdata.com/glossary/key-data-lake-innovations-to-watch-in-2025/)

---

## 5. Cloud Migration Path

### 5.1 Parquet Portability

**Why Parquet is Key**:
- Open standard, vendor-neutral format
- Native support in all cloud platforms
- Works with Spark, Presto, Trino, BigQuery, Athena, Redshift
- No lock-in, easy migration

**Migration Pattern**:

```
Phase 1 (Edge - Raspberry Pi):
  Bronze: Parquet files on NVMe
  Silver: DuckDB query engine
  Gold: Parquet aggregates

Phase 2 (Hybrid - Pi + Cloud):
  Bronze: Sync Parquet to S3/GCS/Azure
  Silver: DuckDB (Pi) + cloud query engine
  Gold: Replicate to cloud for dashboards

Phase 3 (Cloud - Scaled):
  Bronze: S3 Parquet with Iceberg/Delta metadata
  Silver: DataFusion/Spark/Presto
  Gold: Cloud data warehouse or lakehouse
```

**Zero Breaking Changes**:
- Parquet files move unchanged
- SQL queries mostly portable
- Add metadata layer (Iceberg/Delta) without rewriting data
- DuckDB SQL ≈ PostgreSQL SQL (minimal dialect changes)

### 5.2 Engine Independence (2025 Trend)

**Key Insight**:
"By 2025, market converged: Databricks ships Iceberg GA alongside Delta, Snowflake supports both, IOMETE is Iceberg-native. Real shift is engine independence."

**What This Means**:
- Standard formats matter more than specific engines
- Can swap DuckDB → DataFusion → Spark without data migration
- Open table formats (Iceberg, Delta) provide interoperability
- But start simple (Parquet + DuckDB), add complexity later

**Sources**:
- [Data Lakehouse Architecture 2025](https://iomete.com/resources/blog/datalakehouse-architecture-in-2025)
- [2025 Guide to Lakehouse Ecosystem](https://dev.to/alexmercedcoder/the-2025-2026-ultimate-guide-to-the-data-lakehouse-and-the-data-lakehouse-ecosystem-dig)

---

## 6. Comparison Matrix

| Feature | DuckDB | DataFusion | Polars | Delta Lake | Iceberg |
|---------|--------|------------|--------|------------|---------|
| **ARM64 Native** | ✅ Yes | ✅ Yes (Rust) | ✅ Yes | ⚠️ delta-rs only | ❌ No |
| **Memory Footprint** | ✅ Low (125MB/thread) | ✅ Low | ✅ Low | ⚠️ Medium | ❌ High |
| **Java Required** | ✅ No | ✅ No | ✅ No | ❌ Yes (original) | ❌ Yes |
| **Parquet Native** | ✅ Yes | ✅ Best-in-class | ✅ Yes | ✅ Yes | ✅ Yes |
| **Embedded Use** | ✅ Perfect | ✅ Great | ⚠️ Good | ⚠️ Complex | ❌ Not designed |
| **SQL Support** | ✅ Full | ✅ Full | ⚠️ DataFrame | ✅ Yes | ✅ Yes |
| **Rust Integration** | ⚠️ Via FFI | ✅ Native | ✅ Native | ⚠️ delta-rs | ❌ No |
| **Cloud Portability** | ✅ Parquet + SQL | ✅ Parquet | ✅ Parquet | ✅ Delta format | ✅ Iceberg format |
| **Pi Performance** | ✅ Proven (55s SF300) | ✅ Expected good | ✅ Proven efficient | ⚠️ Untested | ❌ Too heavy |
| **Maturity (Edge)** | ✅ Production-ready | ✅ Production-ready | ✅ Production-ready | ⚠️ Experimental | ❌ Not edge-focused |
| **Learning Curve** | ✅ Low (SQL) | ⚠️ Medium (Rust) | ⚠️ Medium (API) | ❌ High | ❌ High |
| **Edge → Cloud** | ✅ Seamless | ✅ Seamless | ✅ Good | ✅ Native migration | ✅ Native migration |

**Legend**:
- ✅ Excellent / Optimal
- ⚠️ Acceptable / Trade-offs
- ❌ Poor fit / Not recommended

---

## 7. Recommendations for NDP

### 7.1 Phase 1: Silver Layer Implementation (Current)

**Recommended Architecture**:

```rust
Bronze Layer (existing):
  - Parquet writer ✓
  - Write-Ahead Log ✓
  - Local NVMe storage ✓

Silver Layer (new):
  - DuckDB embedded database
  - SQL views over Bronze Parquet
  - Rust integration via duckdb-rs crate
  - In-memory or persistent mode

Gold Layer (future):
  - Pre-computed aggregations
  - Hourly/daily rollups for Grafana
  - Stored as Parquet or DuckDB tables
```

**Why This Pattern**:
1. **Zero Rewrites**: Bronze Parquet files unchanged
2. **ARM64 Optimal**: DuckDB proven on Pi 5 with 16GB RAM
3. **Rust Compatible**: `duckdb-rs` crate for native integration
4. **SQL Familiar**: Team knows SQL, low learning curve
5. **Cloud Ready**: Parquet files portable to S3 + Iceberg later

**Implementation Steps**:
1. Add `duckdb-rs` dependency to neural-core
2. Create DuckDB instance pointing to Bronze Parquet directory
3. Define SQL views for transformations (Silver schema)
4. Expose query interface for Grafana/APIs
5. Optional: Materialize hot paths to DuckDB tables

### 7.2 Alternative: DataFusion for Rust Purity

**If Rust-native is critical**:

```rust
Bronze Layer:
  - Parquet files ✓

Silver Layer:
  - Apache DataFusion query engine
  - Native Rust integration (no FFI)
  - Build custom analytics functions
  - Extensible for domain-specific optimizations

Gold Layer:
  - DataFusion-computed aggregations
  - Write to Parquet for Grafana
```

**Trade-offs**:
- ✅ Pure Rust stack (no C++ DuckDB dependency)
- ✅ Fastest Parquet reader (benchmarks)
- ✅ Highly extensible for custom analytics
- ❌ More code to write (less "batteries included")
- ❌ Slightly slower than DuckDB in some benchmarks
- ⚠️ Smaller ecosystem than DuckDB

**When to Choose DataFusion**:
- Rust purity is architectural requirement
- Need custom query extensions
- Building domain-specific analytics engine
- Want to avoid any C++ dependencies

### 7.3 NOT Recommended for Phase 1

**Delta Lake / Iceberg**:
- Wait until cloud migration phase
- Add metadata layer to existing Parquet
- Don't complicate Pi deployment with distributed system patterns

**Polars**:
- Good library, but DuckDB/DataFusion are better fits
- Consider for Python-based tooling/notebooks
- Not optimal for embedded analytics server

---

## 8. Minimal Viable Lakehouse (Edge Edition)

### 8.1 Definition

"A lakehouse succeeds when it clearly defines layers, each with its own role but working together as a cohesive whole." (2025 consensus)

**NDP Minimal Lakehouse**:

```
┌─────────────────────────────────────────────────┐
│  Grafana Dashboards (Gold queries)              │
└─────────────────────┬───────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────┐
│  DuckDB Query Engine (Silver transformations)   │
│  - SQL views for cleaning/validation            │
│  - Joins across Bronze tables                   │
│  - Aggregations (hourly, daily)                 │
│  - Feature engineering queries                  │
└─────────────────────┬───────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────┐
│  Parquet Files (Bronze raw data)                │
│  - Air quality sensors                          │
│  - External APIs (AQI, weather)                 │
│  - Append-only, immutable                       │
│  - Columnar, compressed                         │
└─────────────────────────────────────────────────┘
           Local NVMe SSD (Raspberry Pi 5)
```

**Key Characteristics**:
- **Single node**: No distributed system complexity
- **Embedded**: DuckDB runs in-process with neural-core
- **Standard formats**: Parquet (portable)
- **SQL interface**: Familiar, powerful, portable
- **Resource efficient**: 125MB-4GB per thread
- **Cloud ready**: Add S3 + Iceberg metadata later

### 8.2 What Makes It "Lakehouse"?

**Traditional Requirements**:
- ✅ Separation of storage and compute
- ✅ Standard open formats (Parquet)
- ✅ ACID-like guarantees (DuckDB transactions)
- ✅ Schema enforcement (DuckDB type system)
- ✅ Time travel (Parquet file versioning)
- ⚠️ Unified batch/streaming (streaming via continuous queries)
- ⚠️ Multi-engine support (Parquet portable, but not multi-tenant)

**Edge Adaptations**:
- **Storage = compute location**: Same Pi, but logically separated
- **ACID**: Via DuckDB, not distributed transactions
- **Governance**: File-level, not enterprise catalog
- **Scale**: Single-node, not petabyte

**Is it "really" a lakehouse?**
By 2025 standards: **Yes, at edge scale**.
It follows the architectural pattern, uses open standards, provides analytics capabilities, and scales to cloud without rewrites.

---

## 9. Open Questions & Future Research

### 9.1 Answered Questions

✅ **Can Delta Lake or Iceberg run on ARM64 with minimal Java overhead?**
- Delta: Yes via delta-rs, but unnecessary complexity for Pi
- Iceberg: Not optimized for edge, cloud-focused

✅ **Is Apache DataFusion viable?**
- Yes, excellent choice for Rust-native integration
- Fastest Parquet reader, proven performance
- Good alternative to DuckDB

✅ **How does Polars compare for analytics workloads?**
- Great for Python pipelines, efficient on Pi
- Slower than DuckDB, DataFrame API vs SQL
- Not optimal for embedded server analytics

✅ **What's the minimal viable "lakehouse" for edge deployment?**
- Parquet (Bronze) + DuckDB (Silver) + Parquet aggregates (Gold)
- Proven on Pi 5, 80-90% cost savings vs cloud
- Cloud migration via S3 + Iceberg (no data rewrites)

✅ **How do these patterns translate when moving to cloud?**
- Parquet portability is key (vendor-neutral)
- Add metadata layer (Iceberg/Delta) without rewriting data
- Swap engines (DuckDB → Spark) without format changes
- SQL queries mostly portable

### 9.2 Outstanding Questions

⚠️ **DuckDB Rust FFI performance overhead**:
- How much slower than native Rust (DataFusion)?
- Worth measuring in prototype phase

⚠️ **Materialized views refresh strategy**:
- DuckDB supports materialized views
- How to trigger refreshes (time-based, event-based)?
- Balance freshness vs compute resources

⚠️ **Concurrent query performance**:
- Multiple Grafana dashboards querying simultaneously
- DuckDB WAL mode vs journal mode
- Connection pooling strategy

⚠️ **Parquet file organization**:
- Partitioning strategy (by date, sensor, location?)
- File size optimization for Pi NVMe
- Compaction/cleanup policies

⚠️ **Gold layer storage format**:
- DuckDB tables vs Parquet files?
- Trade-offs for Grafana query performance
- Disk space vs query speed

### 9.3 Next Steps

1. **Prototype Phase**:
   - Implement DuckDB Silver layer over existing Bronze Parquet
   - Benchmark query performance on Pi 5 hardware
   - Test concurrent Grafana dashboard queries
   - Measure memory footprint under load

2. **Schema Design**:
   - Define Silver views for air quality analytics
   - Design Gold aggregation tables
   - Document Grafana query patterns

3. **Integration Testing**:
   - Rust duckdb-rs crate integration
   - Connection lifecycle management
   - Error handling and recovery
   - Resource cleanup

4. **Documentation**:
   - ADR for Silver layer architecture choice
   - Operational runbook for DuckDB maintenance
   - Grafana query examples
   - Cloud migration guide (future)

---

## 10. Sources

### DuckDB
- [TPC-H on Raspberry Pi](https://duckdb.org/2025/01/17/raspberryi-pi-tpch)
- [DuckDB Memory Management](https://duckdb.org/2024/07/09/memory-management)
- [Quacking at the Edge: DuckDB on Raspberry Pi](https://motherduck.com/blog/duckdb-on-edge-raspberry-pi/)
- [DuckLake: Lakehouse Architecture Reimagined](https://endjin.com/blog/2025/06/introducing-ducklake-lakehouse-architecture-reimagined-modern-era)
- [DuckLake Step-by-Step Guide](https://medium.com/@aya.space/ducklake-step-by-step-build-a-full-lakehouse-with-just-parquet-files-and-duckdb-f755bdb76389)
- [DuckDB Medallion Architecture Guide](https://medium.com/@datatomas/duckdb-medallion-architecture-a-complete-local-lakehouse-guide-0f1944b6bcdf)
- [DuckDB flips lakehouse model](https://www.theregister.com/2025/05/28/duckdb_flips_lakehouse_model_with/)
- [Save 90% on Data Warehouse with DuckDB](https://medium.com/@klaushofenbitzer/save-up-to-90-on-your-data-warehouse-lakehouse-with-an-in-process-database-duckdb-63892e76676e)

### Apache DataFusion
- [Apache DataFusion GitHub](https://github.com/apache/datafusion)
- [DataFusion 51.0.0 Release](https://datafusion.apache.org/blog/2025/11/25/datafusion-51.0.0/)
- [DataFusion: Fastest Single Node Parquet Engine](https://datafusion.apache.org/blog/2024/11/18/datafusion-fastest-single-node-parquet-clickbench/)
- [Beyond Postgres and DuckDB: Composable Query Engines](https://thinhdanggroup.github.io/composable-query-engines-with-polars-and-datafusion/)

### Polars
- [Polars Official Site](https://pola.rs/)
- [Polars Benchmarks (May 2025)](https://pola.rs/posts/benchmarks/)
- [Working with Large Datasets on Tiny Machine](https://r-brink.medium.com/working-with-large-datasets-300m-on-a-tiny-machine-512mb-ram-1-core-6d1553e474df)
- [DuckDB vs Polars Performance](https://www.codecentric.de/en/knowledge-hub/blog/duckdb-vs-dataframe-libraries)
- [Ibis benchmarking: DuckDB, DataFusion, Polars](https://ibis-project.org/posts/ibis-bench/)

### Delta Lake & Iceberg
- [delta-rs GitHub](https://github.com/delta-io/delta-rs)
- [delta-rs Documentation](https://delta-io.github.io/delta-rs/)
- [Lessons learned building delta-rs](https://www.buoyantdata.com/blog/2025-03-09-lessons-learned-building-delta-rs.html)
- [10 Future Apache Iceberg Developments (2025)](https://medium.com/data-engineering-with-dremio/10-future-apache-iceberg-developments-to-look-forward-to-in-2025-7292a2a2101d)
- [Apache Iceberg 2025 Guide](https://dev.to/alexmercedcoder/the-2025-comprehensive-guide-to-apache-iceberg-2g22)
- [What's new in Iceberg v3](https://opensource.googleblog.com/2025/08/whats-new-in-iceberg-v3.html)

### Medallion Architecture
- [Medallion Architecture (Azure Databricks)](https://learn.microsoft.com/en-us/azure/databricks/lakehouse/medallion)
- [What is Medallion Architecture?](https://www.databricks.com/glossary/medallion-architecture)
- [Medallion Architecture 101 (2025)](https://www.chaosgenius.io/blog/medallion-architecture/)
- [Beyond Bronze, Silver, Gold: Evolving for AI Era](https://medium.com/@vishal.dutt.data.architect/beyond-bronze-silver-gold-evolving-the-medallion-architecture-for-the-ai-era-77d3cca78745)
- [Medallion Architecture Best Practices](https://bix-tech.com/medallion-architecture-explained-how-bronzesilvergold-layers-supercharge-your-data-lakehouse-mesh-and-data-quality/)

### Edge Computing & IoT
- [Raspberry Pi Ecosystem 2025](https://www.ics.com/blog/look-back-raspberry-pi-ecosystem-2025)
- [Edge LLM Deployment 2025](https://kodekx-solutions.medium.com/edge-llm-deployment-on-small-devices-the-2025-guide-2eafb7c59d07)
- [Raspberry Pi Deployment Considerations](https://www.zymbit.com/2025/03/10/things-to-consider-before-deploying-raspberry-pi/)
- [IoT Time Series Lakehouse](https://dview.io/blog/iot-data-management-lakehouse-integration)
- [Minimal Lakehouse Edge Computing](https://celerdata.com/glossary/key-data-lake-innovations-to-watch-in-2025/)
- [ARM CPU for Apache Kafka at Edge](https://www.kai-waehner.de/blog/2024/02/22/apache-kafka-arm-cpu-edge-hybrid-cloud/)

### Cloud Migration & Lakehouse Trends
- [Data Lakehouse Architecture 2025](https://iomete.com/resources/blog/datalakehouse-architecture-in-2025)
- [2025 Guide to Lakehouse Ecosystem](https://dev.to/alexmercedcoder/the-2025-2026-ultimate-guide-to-the-data-lakehouse-and-the-data-lakehouse-ecosystem-dig)
- [Cheap OpenTelemetry Lakehouses](https://clay.fyi/blog/cheap-opentelemetry-lakehouses-parquet-duckdb-iceberg)

---

## Conclusion

**For Neural Data Platform on Raspberry Pi 5**:

The **DuckDB-based virtual lakehouse** pattern is the clear winner:
- Proven performance on Pi 5 (55s for 300GB queries)
- Minimal overhead (125MB-4GB per thread)
- SQL interface (team familiarity)
- Parquet portability (cloud-ready)
- 80-90% cost savings vs traditional approaches

**Runner-up**: Apache DataFusion for Rust-native purity, with trade-offs in ecosystem maturity.

**Avoid**: Delta Lake and Iceberg until cloud migration phase. These add complexity without edge benefits.

**Architecture**:
```
Bronze (Parquet) → DuckDB (Silver SQL views) → Gold (Parquet aggregates) → Grafana
```

This pattern scales from Pi to cloud by adding S3 storage and Iceberg metadata layer, without rewriting data or changing query logic.

**Next action**: Prototype DuckDB integration with existing Bronze Parquet layer, measure performance, validate architecture assumptions.
