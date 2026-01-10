# ADR-006-001: ETL Engine Selection

**Feature**: dp-006 (Silver Layer Implementation)
**Status**: Accepted
**Date**: 2026-01-10
**Author**: NDP Architect
**Supersedes**: None

---

## Context

The Neural Data Platform requires an ETL engine to transform raw Bronze layer data (Parquet files) into typed Silver layer tables in TimescaleDB. This decision is foundational as it affects:

1. **Development complexity** - How much code to write and maintain
2. **Resource usage** - Memory and CPU on Raspberry Pi 5 (16GB)
3. **Performance** - ETL completion time for hourly batch
4. **Maintainability** - SQL vs code-based transforms
5. **Ecosystem fit** - Integration with existing NDP patterns

### Current State

- Bronze layer: 7 streams writing to Parquet files (~550KB/day)
- DuckDB container: Removed from Pi deployment (was 512MB container)
- TimescaleDB: Deployed with 256MB limit
- Config infrastructure: etcd-based GitOps proven

### Constraints

- Memory budget: <300MB peak for silver-etl process
- ETL latency: <60 seconds for hourly batch
- ARM64 (aarch64): Must work on Raspberry Pi 5
- Config-driven: Transforms should be YAML-configurable

---

## Decision

**Use duckdb-rs embedded in a Rust binary** as the ETL engine.

```toml
# Cargo.toml
[dependencies]
duckdb = { version = "1.1", features = ["bundled", "parquet", "json"] }
```

The ETL binary will:
1. Load stream configs from etcd
2. Generate SQL from config-driven field mappings
3. Execute transforms via DuckDB in-memory
4. Write results to TimescaleDB via DuckDB's postgres extension

---

## Consequences

### Positive

1. **Single binary deployment** - No separate DuckDB container needed
2. **Proven PostgreSQL writes** - DuckDB postgres extension supports INSERT/UPDATE
3. **Native Parquet support** - Zero-copy reads from Arrow format
4. **SQL-based transforms** - Reuse existing SQL patterns, lower complexity
5. **Memory efficient** - Embedded DuckDB uses ~100-200MB peak
6. **ARM64 support** - Pre-built binaries available for aarch64
7. **Pi 5 proven** - DuckDB successfully tested on Pi 5 with TPC-H benchmark

### Negative

1. **New dependency** - Adds duckdb-rs to Cargo dependencies
2. **Extension installation** - postgres extension requires runtime `INSTALL postgres`
3. **Limited error handling** - SQL-based recovery is basic
4. **External watermark tracking** - Must query target for incremental position

### Neutral

1. **SQL generation required** - Config-driven approach generates SQL at runtime
2. **No streaming support** - Batch-only processing (acceptable for hourly cadence)

---

## Alternatives Considered

### Alternative 1: Polars + tokio-postgres

**Description**: Use Polars DataFrame library for Parquet reading and transforms, tokio-postgres for database writes.

| Factor | Assessment |
|--------|------------|
| Memory | ~50-100MB (lowest) |
| Complexity | High (~500-1000 LOC) |
| Performance | Good |
| DB Integration | Manual - no direct connector |

**Rejected because**: Polars lacks database connectors (feature request #24148 still open). Would require significant manual code for PostgreSQL integration, increasing maintenance burden.

### Alternative 2: Python Polars/DuckDB

**Description**: Python script using Polars for transforms and ADBC driver for PostgreSQL writes.

| Factor | Assessment |
|--------|------------|
| Memory | ~200-400MB (highest) |
| Complexity | Low (~100 LOC) |
| Performance | Good |
| DB Integration | Excellent via ADBC |

**Rejected because**: Adds Python runtime dependency to Pi deployment. Higher memory footprint. No compile-time type checking.

### Alternative 3: pg_parquet FDW

**Description**: Use PostgreSQL Foreign Data Wrapper to query Parquet files directly from TimescaleDB.

| Factor | Assessment |
|--------|------------|
| Memory | ~50MB (uses existing TimescaleDB) |
| Complexity | Low (SQL only) |
| Performance | Fair (no columnar pushdown) |
| DB Integration | Native |

**Rejected because**: Extension may not be available in TimescaleDB Docker image. Limited ARM64 support. No columnar pushdown reduces performance.

### Alternative 4: Rust Native (arrow-rs + sqlx)

**Description**: Pure Rust implementation using arrow-rs for Parquet and sqlx for PostgreSQL.

| Factor | Assessment |
|--------|------------|
| Memory | ~50-100MB |
| Complexity | High (~800+ LOC) |
| Performance | Excellent |
| DB Integration | sqlx (known performance issues) |

**Rejected because**: Significantly higher development effort. sqlx known to be slower than alternatives. All transform logic must be hand-coded.

---

## Comparison Matrix

| Factor | duckdb-rs | Polars+pg | Python | FDW | Rust Native |
|--------|-----------|-----------|--------|-----|-------------|
| Memory Peak | 200MB | 110MB | 300MB | 50MB | 110MB |
| Dev Effort | Low | High | Low | Low | High |
| Performance | Excellent | Good | Good | Fair | Excellent |
| Pi Suitability | Excellent | Excellent | Good | Good | Excellent |
| Maintainability | High | Medium | Medium | High | Low |
| **Recommendation** | **Selected** | Fallback | No | No | No |

---

## Implementation Notes

### ETL Pattern

```rust
use duckdb::{Connection, Result};

pub fn run_etl(config: &SilverEtlConfig) -> Result<usize> {
    let conn = Connection::open_in_memory()?;

    // Load extensions
    conn.execute_batch("
        INSTALL postgres;
        LOAD postgres;
        INSTALL parquet;
        LOAD parquet;
    ")?;

    // Attach TimescaleDB
    conn.execute(&format!(
        "ATTACH 'host={} port={} dbname={} user={} password={}' AS pg (TYPE postgres)",
        config.pg_host, config.pg_port, config.pg_dbname, config.pg_user, config.pg_password
    ), [])?;

    // Generate and execute ETL SQL from config
    let sql = generate_etl_sql(config)?;
    let result = conn.execute(&sql, [])?;

    Ok(result)
}
```

### Memory Budget

| Component | Memory |
|-----------|--------|
| Rust runtime | ~20MB |
| duckdb-rs | ~100MB |
| Parquet buffers | ~50MB |
| Query execution | ~30MB |
| **Total Peak** | **~200MB** |

### Fallback Plan

If duckdb-rs postgres extension proves unreliable on ARM64:
1. Fall back to tokio-postgres for writes
2. Use DuckDB only for Parquet reading and transforms
3. Export as Arrow → insert via UNNEST batching

---

## References

1. Research: `research/agenticdataplatform/silver/02-etl-alternatives.md`
2. Research: `research/agenticdataplatform/silver/06-refined-synthesis.md`
3. DuckDB Performance Guide: https://duckdb.org/docs/stable/guides/performance/overview
4. DuckDB PostgreSQL Extension: https://motherduck.com/blog/postgres-duckdb-options/
5. Pattern: `arch-config-driven-silver-etl` in AgentDB

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP Architect | Initial decision |
