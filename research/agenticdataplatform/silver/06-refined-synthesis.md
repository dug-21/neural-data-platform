# Silver Layer Research - Refined Synthesis

**Date**: 2026-01-05
**Status**: Complete
**Research Swarm**: Silver Layer Scope Refinement (Resumed)

---

## Executive Summary

This document synthesizes findings from the resumed research swarm, incorporating new constraints and considerations:

1. **DuckDB container was removed** from Pi - external DuckDB is not available
2. **Config-driven patterns** should extend through Silver layer (GitOps approach)
3. **Generic parser types** and data dictionary may inform ETL
4. **Integration vs separation** decision for Silver ETL component

### Revised Recommendations

| Decision Area | Original Recommendation | Revised Recommendation |
|--------------|------------------------|----------------------|
| **ETL Engine** | DuckDB SQL script (external) | **duckdb-rs embedded** (in Rust binary) |
| **Architecture** | Single ETL container | **Separate silver-etl binary** (start separated, can integrate later) |
| **Configuration** | Separate ETL SQL | **Config-driven** (`silver_etl` section in stream YAML) |
| **DQ Integration** | Not specified | **Config-driven DQ rules** with `dq_flags` transparency |

---

## 1. ETL Engine: duckdb-rs Embedded

### Why duckdb-rs (Not External Container)

| Factor | External DuckDB | duckdb-rs Embedded |
|--------|-----------------|-------------------|
| Deployment | Requires Docker container | Single Rust binary |
| PostgreSQL writes | Via postgres extension | Via postgres extension |
| ARM64 support | Pre-built binaries | Pre-built binaries |
| Memory | Separate 512MB container | Embedded in ~200MB binary |
| Configuration | SQL files | Rust + SQL generation |

### Key Findings

1. **DuckDB postgres extension supports writes** - Can INSERT/UPDATE/DELETE to PostgreSQL (TimescaleDB)
2. **Pi 5 16GB is proven** - DuckDB successfully processed 300GB TPC-H on Pi 5
3. **Polars lacks database connectors** - Feature request #24148 still open; requires manual tokio-postgres integration
4. **Code reduction** - SQL-based ETL reduces ~200+ lines (Polars + tokio-postgres) to ~50 lines

### Recommended Cargo.toml Addition

```toml
[dependencies]
duckdb = { version = "1.1", features = ["bundled", "parquet", "json"] }
```

### ETL Pattern

```rust
// silver-etl/src/etl.rs
use duckdb::{Connection, params};

pub fn run_etl(config: &SilverEtlConfig) -> Result<usize, Error> {
    let conn = Connection::open_in_memory()?;

    // Load extensions
    conn.execute("INSTALL postgres; LOAD postgres;", [])?;
    conn.execute("INSTALL parquet; LOAD parquet;", [])?;

    // Attach TimescaleDB
    conn.execute(&format!(
        "ATTACH 'host={} dbname={} user={}' AS pg (TYPE postgres)",
        config.pg_host, config.pg_dbname, config.pg_user
    ), [])?;

    // Generate and execute ETL SQL from config
    let sql = generate_etl_sql(config)?;
    let rows = conn.execute(&sql, [])?;

    Ok(rows)
}
```

---

## 2. Architecture: Separate silver-etl Binary

### Recommended Approach: Start Separated

Per existing ADRs (`arch-data-lake-layers`):
> "Bronze must succeed. Silver is best-effort and can be rebuilt from Bronze."

**Separation protects Bronze reliability while allowing Silver iteration.**

### Architecture Diagram

```
┌─────────────────────────────────────┐
│         air-quality-app             │
│  Sources → Channel → ParquetStore   │
│         (Bronze layer)              │
└─────────────────────────────────────┘
              │
              │ writes Parquet files
              ▼
┌─────────────────────────────────────┐
│     silver-etl (separate binary)    │
│                                     │
│  ┌───────────────────────────────┐ │
│  │  ConfigLoader (etcd/YAML)     │ │
│  │  - StreamConfig.silver_etl    │ │
│  └─────────────┬─────────────────┘ │
│                ▼                    │
│  ┌───────────────────────────────┐ │
│  │  DuckDB (embedded)            │ │
│  │  - read_parquet()             │ │
│  │  - Transform + DQ rules       │ │
│  │  - postgres extension         │ │
│  └─────────────┬─────────────────┘ │
│                ▼                    │
│  ┌───────────────────────────────┐ │
│  │  TimescaleDB (Silver)         │ │
│  └───────────────────────────────┘ │
└─────────────────────────────────────┘
```

### Scheduling

```ini
# /etc/systemd/system/ndp-silver-etl.timer
[Timer]
OnCalendar=*:05:00        # 5 minutes past each hour
Persistent=true           # Catch up after downtime
RandomizedDelaySec=60     # Prevent thundering herd
```

### Memory Impact

```
silver-etl process:
  Base Rust runtime:        ~20MB
  duckdb-rs embedded:       ~100MB
  Parquet reader buffers:   ~50MB
  PostgreSQL client:        ~30MB
  ─────────────────────────────────
  Total:                   ~200MB
```

---

## 3. Config-Driven Silver ETL

### Design Document Created

Full design at: `docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md`

### Key Pattern: Extend Stream Config

Add `silver_etl` section to existing stream YAML:

```yaml
# config/base/streams/air-quality/config.yaml
stream_id: air-quality
# ... existing Bronze config ...

silver_etl:
  enabled: true
  target_table: silver.air_quality_observations

  field_mappings:
    - source_path: raw_payload.pm02
      target_column: pm25
      type: double_precision
      nullable: false
      transform: null
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag

    - source_path: raw_payload.atmp
      target_column: temperature_c
      type: double_precision
      transform:
        type: unit_conversion
        from: celsius
        to: celsius
        formula: { type: linear, scale: 1.0, offset: 0.0 }
      dq_rules:
        - rule: range_check
          min: -40.0
          max: 85.0
          action: flag

  dq_output:
    enabled: true
    target_column: dq_flags

  deduplication:
    key_columns: [observation_time, ndp_id]
    strategy: upsert
```

### Reusing Existing Patterns

| Existing Pattern | Silver Extension |
|-----------------|------------------|
| `FieldMapping` (parser) | `SilverFieldMapping` with DQ rules |
| `UnitConversion` | Same type, reused directly |
| `ConversionFormula::Linear` | Same type for transforms |
| `SchemaField.range` | Becomes `dq_rules[].range_check` |
| etcd config sync | Same GitOps workflow |

### SQL Generation from Config

The config drives SQL generation:

```rust
pub trait SqlGenerator {
    fn generate_etl_sql(&self, config: &SilverEtlConfig) -> Result<String>;
    fn generate_transform(&self, transform: &TransformConfig) -> String;
    fn generate_dq_check(&self, rule: &DqRule) -> String;
}
```

Generated SQL example:
```sql
INSERT INTO pg.silver.air_quality_observations (...)
SELECT
    to_timestamp(timestamp / 1000000) as observation_time,
    ndp_id,
    -- DQ: range_check with flag action
    CASE
        WHEN json_extract(raw_payload, '$.pm02')::FLOAT NOT BETWEEN 0 AND 1000
        THEN NULL
        ELSE json_extract(raw_payload, '$.pm02')::FLOAT
    END as pm25,
    -- Collect DQ flags
    ARRAY_AGG(...) as dq_flags
FROM read_parquet('/data/raw/air-quality/**/*.parquet')
WHERE ...
```

---

## 4. DQ Integration

### DQ Rules in Config

```yaml
dq_rules:
  - rule: range_check
    min: -50.0
    max: 60.0
    action: flag  # flag | reject | clamp | drop

  - rule: not_null
    action: reject

  - rule: pattern
    regex: "^[A-Z]{4}$"
    action: flag
```

### DQ Actions

| Action | Behavior | dq_flags Entry |
|--------|----------|----------------|
| `flag` | Keep value, add flag | `range_check:temperature_c:exceeded` |
| `reject` | Set NULL, add flag | `range_check:temperature_c:rejected` |
| `clamp` | Clamp to bounds, add flag | `range_check:humidity:clamped:105→100` |
| `drop` | Drop entire row | (row not inserted) |

### Transparency Output

```sql
dq_flags TEXT[] -- Array of violation flags
```

Example:
```sql
['range_check:temperature_c:exceeded_max',
 'range_check:humidity_pct:clamped']
```

---

## 5. Implementation Roadmap

### Phase 1: Foundation (Week 1)

| Task | Effort | Output |
|------|--------|--------|
| Create `apps/silver-etl/` crate | 2h | New binary |
| Add duckdb-rs dependency | 1h | Cargo.toml |
| Define `SilverEtlConfig` types | 4h | `core/src/config/silver_etl.rs` |
| Basic DuckDB ETL runner | 4h | Working prototype |

### Phase 2: Config-Driven (Week 2)

| Task | Effort | Output |
|------|--------|--------|
| SQL generator trait | 4h | Transform → SQL |
| DQ rule generator | 4h | DQ checks → SQL |
| etcd config loader | 4h | Hot-reload support |
| Stream config extension | 2h | `silver_etl` section |

### Phase 3: Integration (Week 3)

| Task | Effort | Output |
|------|--------|--------|
| Systemd timer | 2h | Hourly execution |
| Monitoring dashboard | 4h | Grafana panels |
| Error handling/retry | 4h | Robust pipeline |
| Documentation | 4h | Procedures |

### Phase 4: All Streams (Week 4)

| Task | Effort | Output |
|------|--------|--------|
| air-quality config | 2h | YAML + test |
| outdoor-weather config | 2h | YAML + test |
| outdoor-air-quality config | 2h | YAML + test |
| nws-* streams config | 4h | YAML + test |
| Backfill validation | 4h | Historical data |

---

## 6. Files to Create/Modify

### New Files

| Path | Purpose |
|------|---------|
| `apps/silver-etl/Cargo.toml` | ETL binary crate |
| `apps/silver-etl/src/main.rs` | Entry point |
| `apps/silver-etl/src/config.rs` | Config loader |
| `apps/silver-etl/src/etl.rs` | DuckDB ETL runner |
| `apps/silver-etl/src/sql_gen.rs` | SQL generator |
| `core/src/config/silver_etl.rs` | Config types |
| `deploy/pi/systemd/silver-etl.timer` | Scheduler |
| `deploy/pi/systemd/silver-etl.service` | Service unit |

### Modified Files

| Path | Change |
|------|--------|
| `core/Cargo.toml` | Add duckdb feature flag |
| `config/base/streams/*/config.yaml` | Add `silver_etl` section |
| `deploy/pi/docker-compose.yml` | Add silver-etl service |
| `docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md` | Update diagrams |

---

## 7. Summary of Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **ETL Engine** | duckdb-rs embedded | Single binary, proven PostgreSQL writes, Pi 5 compatible |
| **Architecture** | Separate binary | Protects Bronze reliability, follows ADR |
| **Configuration** | Extend stream YAML | GitOps workflow, hot-reload, reuse existing patterns |
| **DQ Integration** | Config-driven rules | Transparent, auditable, flexible actions |
| **Scheduling** | systemd timer (hourly) | Persistent catch-up, standard Linux tooling |
| **Transforms** | Reuse `UnitConversion` | Existing infrastructure, proven patterns |

---

## 8. Related Documents

| Document | Location |
|----------|----------|
| Config-Driven Silver ETL Design | `docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md` |
| Original Silver Synthesis | `research/agenticdataplatform/silver/05-synthesis.md` |
| Scope Definition | `research/agenticdataplatform/silver/01-scope-definition.md` |
| ETL Alternatives | `research/agenticdataplatform/silver/02-etl-alternatives.md` |
| Data Dictionary | `research/agenticdataplatform/silver/03-data-dictionary.md` |
| Dashboard Integration | `research/agenticdataplatform/silver/04-dashboard-integration.md` |

---

## 9. Pattern Saved

The config-driven Silver ETL design has been saved as AgentDB pattern:
- **Name**: `arch-config-driven-silver-etl`
- **Tags**: `dp-006, silver, etl, config-driven, architecture`

---

*Refined Synthesis: 2026-01-05*
*Research Swarm (Resumed): 3 NDP agents (researcher, ndp-architect ×2)*
