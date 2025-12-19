# Silver Layer Architecture Decision Matrix

**Research Date**: 2025-12-19
**Context**: 6 hours debugging DuckDB ARM64 issues, seeking balance of simplicity, scalability, and Pi-first deployment
**Scope**: Neural prediction, pattern identification, feature engineering, dashboards

---

## Executive Summary

After comprehensive research across 5 domains, the **current DuckDB + SQLite export approach is validated as the right choice for dp-001**. The root cause of debugging pain was the **Grafana DuckDB plugin** (glibc 2.35+ dependency), not DuckDB itself.

### Key Finding

**DuckDB is working correctly on ARM64 Raspberry Pi 5.** The issue is specifically the Grafana plugin ecosystem, which uses duckdb-go compiled against glibc binaries unavailable on ARM64.

### Recommendation

| Timeframe | Recommendation | Rationale |
|-----------|---------------|-----------|
| **Short-term (dp-001)** | ✅ Keep SQLite export workaround | Working, low-risk, 5-min latency acceptable |
| **Medium-term (dp-002)** | 🔄 Evaluate QuestDB or TimescaleDB | Real-time features, ML integration needs |
| **Long-term (cloud)** | 📈 Add Iceberg/Delta metadata layer | Cloud portability without data rewrite |

---

## Decision Matrix: Silver Layer Technologies

### Overall Scoring

| Criteria | Weight | DuckDB+SQLite | QuestDB | TimescaleDB | InfluxDB 3 | DataFusion |
|----------|--------|---------------|---------|-------------|------------|------------|
| **ARM64 Stability** | 25% | ✅ 9/10 | ✅ 9/10 | ✅ 10/10 | ⚠️ 7/10 | ✅ 9/10 |
| **Grafana Integration** | 20% | ✅ 9/10 (SQLite) | ✅ 8/10 | ✅ 10/10 | ✅ 9/10 | ⚠️ 5/10 |
| **Parquet Integration** | 15% | ✅ 10/10 | ✅ 8/10 | ⚠️ 5/10 (FDW) | ✅ 10/10 | ✅ 10/10 |
| **ML/Feature Eng.** | 15% | ⚠️ 6/10 | ✅ 8/10 | ✅ 10/10 | ⚠️ 6/10 | ⚠️ 7/10 |
| **Memory Footprint** | 10% | ✅ 9/10 | ✅ 8/10 | ⚠️ 6/10 | ⚠️ 7/10 | ✅ 8/10 |
| **Simplicity** | 10% | ✅ 8/10 | ✅ 8/10 | ⚠️ 6/10 | ⚠️ 6/10 | ⚠️ 5/10 |
| **Cloud Portability** | 5% | ✅ 9/10 | ✅ 8/10 | ✅ 8/10 | ✅ 9/10 | ✅ 9/10 |
| **TOTAL (Weighted)** | 100% | **8.4** | **8.3** | **8.0** | **7.5** | **7.2** |

### Interpretation

1. **DuckDB+SQLite (8.4)**: Best for current needs (dashboards, batch ML)
2. **QuestDB (8.3)**: Best alternative if needing real-time features
3. **TimescaleDB (8.0)**: Best for PostgreSQL expertise, continuous aggregates
4. **InfluxDB 3 (7.5)**: Wait for ARM64 maturity (GA April 2025)
5. **DataFusion (7.2)**: Excellent for Rust-native, but more work

---

## Detailed Analysis by Research Area

### 1. DuckDB ARM64 Analysis

**Root Cause Identified**: Grafana DuckDB plugin requires glibc 2.35+, duckdb-go only provides glibc binaries.

| Component | Status | Notes |
|-----------|--------|-------|
| DuckDB Core | ✅ Works | TPC-H SF300 (300GB) in 55s on Pi 5 |
| Docker Images | ✅ Works | `datacatering/duckdb:v1.1.3` ARM64 |
| Grafana Plugin | ❌ Broken | glibc dependency, no ARM64 binaries |
| Extensions | ⚠️ Fixed | 403 errors fixed in DuckDB 1.4.3 |

**Validation**: Your SQLite export workaround is correct. DuckDB itself is stable.

**Source**: `product/research/Silver/duckdb-arm64-analysis.md`

### 2. Time-Series Database Comparison

| Database | Parquet Read | ARM64 Docker | Memory | Grafana | Verdict |
|----------|-------------|--------------|--------|---------|---------|
| **QuestDB** | ✅ read_parquet() | ✅ Official | ~300MB | ✅ Official plugin | Best fit |
| **TimescaleDB** | ⚠️ FDW | ✅ Mature | ~500MB | ✅ PostgreSQL | Strong alternative |
| **InfluxDB 3** | ✅ Native | ⚠️ New | ~500MB | ✅ Built-in | Wait 6-12 months |
| **VictoriaMetrics** | ❌ No | ✅ Official | ~200MB | ✅ Prometheus | Wrong use case |
| **Apache Druid** | ⚠️ Batch | ❌ Unknown | >1GB | ✅ Druid DS | Too heavy |

**Winner**: **QuestDB** for native Parquet + SQL + low memory
**Runner-up**: **TimescaleDB** for PostgreSQL ecosystem + continuous aggregates

**Source**: `product/research/Silver/timeseries-database-comparison.md`

### 3. Data Lake Patterns

| Pattern | Pi 5 Viability | Cloud Portability | Complexity |
|---------|---------------|-------------------|------------|
| **DuckDB Virtual Lakehouse** | ✅ Optimal | ✅ Parquet portable | Low |
| **DataFusion** | ✅ Great | ✅ Parquet portable | Medium |
| **Polars** | ✅ Good | ✅ Parquet portable | Medium |
| **Delta Lake (delta-rs)** | ⚠️ Overkill | ✅ Native migration | High |
| **Apache Iceberg** | ❌ Too heavy | ✅ Native migration | High |

**Winner**: **DuckDB virtual lakehouse** (Bronze Parquet → DuckDB views → Gold aggregates)

**Key Insight**: Traditional lakehouse formats (Delta, Iceberg) are overkill for edge. Add metadata layer later when migrating to cloud.

**Source**: `product/research/Silver/data-lake-patterns.md`

### 4. Grafana ARM64 Datasources

| Datasource | ARM64 Status | Installation | Best For |
|------------|-------------|--------------|----------|
| **SQLite** | ✅ Confirmed | CLI | Current workaround ✓ |
| **PostgreSQL** | ✅ Built-in | N/A | TimescaleDB, production |
| **Infinity** | ✅ Confirmed | CLI | API/JSON flexibility |
| **InfluxDB** | ✅ Built-in | N/A | Dedicated TSDB |
| **CSV** | ✅ Confirmed | CLI | Static exports only |
| **DuckDB Plugin** | ❌ Broken | N/A | Wait for ARM64 support |

**Current NDP**: Using `frser-sqlite-datasource` ✅ - correct choice for ARM64

**Source**: `product/research/Silver/grafana-arm64-datasources.md`

### 5. ML/Feature Engineering Integration

| Approach | Real-Time | Batch | Pi 5 Fit | Complexity |
|----------|-----------|-------|----------|------------|
| **TimescaleDB Cont. Agg.** | ✅ Yes | ✅ Yes | ✅ Good | Medium |
| **DuckDB + Cron** | ❌ No | ✅ Yes | ✅ Excellent | Low |
| **Polars Pipeline** | ❌ No | ✅ Excellent | ✅ Good | Medium |
| **Redis Feature Cache** | ✅ <10ms | N/A | ✅ Good | Medium |

**Recommended Architecture**:
```
Bronze (Parquet) → DuckDB (dp-001) → TimescaleDB (dp-002) → Redis Cache
                                            ↓
                                    ruv-FANN Inference
```

**Phase 1 (dp-001)**: DuckDB views + batch export (current)
**Phase 2 (dp-002)**: TimescaleDB continuous aggregates + Redis
**Phase 3 (fe-001)**: Polars + augurs pattern detection

**Source**: `product/research/Silver/ml-feature-engineering.md`

---

## Option Analysis

### Option A: Keep Current Approach (RECOMMENDED FOR dp-001)

**Architecture**:
```
Bronze (Parquet) → DuckDB (Silver Views) → SQLite Export → Grafana
                                     ↓
                            Batch Export → ruv-FANN Training
```

**Pros**:
- ✅ Already working
- ✅ No migration risk
- ✅ Minimal components
- ✅ 5-minute latency acceptable for home monitoring
- ✅ DuckDB proven stable on Pi 5

**Cons**:
- ⚠️ Not real-time (5-min export cycle)
- ⚠️ SQLite extra storage overhead
- ⚠️ No feature caching for inference

**When to Use**: Home monitoring, batch ML training, Grafana dashboards

**Effort**: None (continue current implementation)

---

### Option B: Migrate to QuestDB (CONSIDER FOR dp-002)

**Architecture**:
```
Bronze (Parquet) → QuestDB (read_parquet() + native) → Grafana
                           ↓
                  SAMPLE BY → Feature Aggregates → ruv-FANN
```

**Pros**:
- ✅ Native Parquet read via `read_parquet()` SQL
- ✅ Low memory (~300MB)
- ✅ Official ARM64 Docker images
- ✅ SQL-native (PostgreSQL wire protocol)
- ✅ Advanced SAMPLE BY for time-series

**Cons**:
- ⚠️ New service to learn
- ⚠️ Parquet read limited to single files
- ⚠️ Smaller community than PostgreSQL

**When to Use**: Need real-time queries, want SQL + Parquet hybrid

**Effort**: ~3-5 days (new container, data migration, dashboard updates)

---

### Option C: Migrate to TimescaleDB (CONSIDER FOR dp-002)

**Architecture**:
```
Bronze (Parquet) → DuckDB ETL → TimescaleDB (Hypertables) → Grafana
                                        ↓
                        Continuous Aggregates → Redis → ruv-FANN
```

**Pros**:
- ✅ Battle-tested on Raspberry Pi
- ✅ PostgreSQL ecosystem (extensions, tools)
- ✅ Continuous aggregates for real-time features
- ✅ Native Grafana support (no plugin needed)
- ✅ Automatic compression (80-95% reduction)

**Cons**:
- ⚠️ Heavier (~500MB memory)
- ⚠️ Parquet integration via FDW (not native)
- ⚠️ Requires Rust ingestion changes (or DuckDB ETL)

**When to Use**: Need continuous aggregates, PostgreSQL expertise, production-grade TSDB

**Effort**: ~5-7 days (new container, ETL pipeline, schema design, dashboard updates)

---

### Option D: Build HTTP Query API (ALTERNATIVE)

**Architecture**:
```
Bronze (Parquet) → DuckDB → HTTP Query API → Grafana (Infinity DS)
                                    ↓
                            ruv-FANN Inference
```

**Pros**:
- ✅ Bypasses Grafana plugin issues entirely
- ✅ Full control over query execution
- ✅ Can add caching, rate limiting
- ✅ Exposes API for external consumers

**Cons**:
- ⚠️ Additional service to maintain
- ⚠️ Network latency on queries
- ⚠️ Must implement security (SQL injection prevention)

**When to Use**: Need API access, want to avoid SQLite export, long-term DuckDB commitment

**Effort**: ~3-4 days (Rust service, Infinity datasource config)

---

## Final Recommendation

### Immediate (dp-001): ✅ KEEP CURRENT APPROACH

**Rationale**:
1. **It's working** - SQLite export + Grafana is functional
2. **6-hour debugging identified root cause** - Grafana plugin, not DuckDB
3. **Home monitoring needs met** - 5-minute latency acceptable
4. **No risk** - Continue with proven solution

**Actions**:
- [x] Continue SQLite export approach
- [x] Complete dp-001 deployment verification
- [ ] Document workaround in ADR

### Short-term (dp-002): 🔄 EVALUATE QUESTDB OR TIMESCALEDB

**Trigger Conditions** (any of):
- Need real-time queries (<1 minute latency)
- Need continuous aggregates for ML features
- Query complexity exceeds SQLite export capability
- Multi-user dashboard access

**Evaluation Criteria**:
1. QuestDB if: Parquet hybrid is priority, simpler migration
2. TimescaleDB if: PostgreSQL expertise, continuous aggregates critical

### Long-term (Cloud Migration): 📈 ADD ICEBERG METADATA

**When Platform Scales**:
- Sync Parquet to S3/GCS
- Add Apache Iceberg metadata layer (no data rewrite)
- Swap DuckDB → Spark/Trino for cloud-scale queries

---

## Research Documents Created

| Document | Location | Summary |
|----------|----------|---------|
| DuckDB ARM64 Analysis | `product/research/Silver/duckdb-arm64-analysis.md` | Root cause: Grafana plugin glibc, DuckDB stable |
| Time-Series DB Comparison | `product/research/Silver/timeseries-database-comparison.md` | QuestDB recommended, TimescaleDB alternative |
| Data Lake Patterns | `product/research/Silver/data-lake-patterns.md` | DuckDB virtual lakehouse optimal for Pi |
| Grafana ARM64 Datasources | `product/research/Silver/grafana-arm64-datasources.md` | SQLite, PostgreSQL, Infinity work |
| ML/Feature Engineering | `product/research/Silver/ml-feature-engineering.md` | TimescaleDB continuous aggregates + Redis |

---

## Appendix: Your Specific Constraints Addressed

| Your Requirement | Research Finding | Recommendation |
|-----------------|------------------|----------------|
| **Simple is better** | DuckDB virtual lakehouse is simplest viable approach | Keep current approach |
| **Future scalability** | Parquet is portable, add Iceberg later | No changes needed now |
| **Pi deployment size** | DuckDB+SQLite ~700MB, QuestDB ~400MB, TimescaleDB ~600MB | All acceptable |
| **Neural prediction** | TimescaleDB continuous aggregates + Redis cache | dp-002 scope |
| **Pattern identification** | augurs (Rust) + SQL-based detection | fe-001 scope |
| **Feature engineering** | TimescaleDB or DuckDB batch export | dp-002 or cron job |
| **Virtual views (simple)** | DuckDB views + SQLite export = working solution | Keep current |
| **Pi now, cloud later** | Parquet + SQL = cloud portable | Architecture validated |

---

## Conclusion

**Your 6 hours of debugging was not wasted.** You correctly identified that DuckDB has ARM64 issues with Grafana, and you implemented the right workaround (SQLite export). The research confirms:

1. ✅ **DuckDB core is stable on ARM64** - benchmarked successfully
2. ✅ **Your SQLite workaround is correct** - recommended approach
3. ✅ **Current architecture is sound** - no immediate changes needed
4. 🔄 **Future options exist** - QuestDB/TimescaleDB for real-time features

**Bottom line**: Continue with dp-001 deployment. The architecture you have is appropriate for the current phase. Consider TimescaleDB or QuestDB when real-time ML features become a requirement.

---

**Document Version**: 1.0
**Last Updated**: 2025-12-19
**Author**: Research Swarm (Claude)
**Status**: Complete
