# Data Platform Lifecycle Stages

## Overview

Data domains go through distinct lifecycle phases, each with different needs for data access, schema stability, and DQ rigor.

## Lifecycle Phases

```
┌─────────────────────────────────────────────────────────────────────┐
│                     DATA PLATFORM LIFECYCLE                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────┐      ┌───────────┐      ┌──────────┐                   │
│  │  EARLY  │ ───► │ DEV STAGE │ ───► │  STABLE  │                   │
│  └─────────┘      └───────────┘      └──────────┘                   │
│                                                                     │
│  Domain model     Model being       Production                      │
│  undefined        tested            dashboards                      │
│                                                                     │
│  Raw data         Schema            Feature                         │
│  exploration      iteration         engineering                     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Phase Details

### Early Phase: Domain Exploration

**Goal**: Understand the data before committing to schemas.

**Characteristics**:
- Domain model undefined
- Need easy access to raw data
- Exploratory queries, notebooks, ad-hoc analysis
- No production dependencies

**Infrastructure**:
```
Bronze (Parquet)
    │
    └──► DuckDB (ad-hoc queries on raw JSON)
            │
            └──► Jupyter/notebooks for exploration
```

**Silver Layer**: Not created yet, or read-only views on Bronze.

**DQ Rigor**: Minimal - just reject malformed payloads.

**Activities**:
- Query raw payloads with DuckDB
- Understand field meanings, value ranges
- Identify relationships between entities
- Draft domain model
- Document findings

**Example Query (exploring NWS data)**:
```sql
-- DuckDB query directly on Bronze Parquet
SELECT
    timestamp,
    raw_payload->'properties'->'temperature'->'values' as temp_forecast,
    raw_payload->'properties'->'updateTime' as issue_time
FROM read_parquet('/data/bronze/nws-gridpoints-forecast/**/*.parquet')
LIMIT 10;
```

---

### DevStage Phase: Schema Iteration

**Goal**: Test Silver schemas, build prototype dashboards.

**Characteristics**:
- Domain model drafted, being validated
- Silver schemas evolving (expect changes)
- Test dashboards for validation
- May need to rebuild Silver from Bronze

**Infrastructure**:
```
Bronze (Parquet)
    │
    ├──► DuckDB (still available for debugging)
    │
    └──► ETL ──► Silver (TimescaleDB)
                    │
                    └──► Test Dashboards (Grafana)
```

**Silver Layer**: Active development, schema changes expected.

**DQ Rigor**: Medium - validate ranges, flag anomalies, but flexible.

**Activities**:
- Implement Bronze → Silver ETL
- Create Silver schema (expect iterations)
- Build test dashboards
- Validate domain model against real data
- Refine DQ rules based on observed data

**Key Principle**: Silver can be rebuilt from Bronze at any time. This enables schema iteration without data loss.

---

### Stable Phase: Production

**Goal**: Locked schemas, full DQ, feature engineering.

**Characteristics**:
- Domain model finalized
- Silver schemas locked (changes require migration)
- Production dashboards
- Gold layer (ML features) depends on Silver
- SLAs for data freshness and quality

**Infrastructure**:
```
Bronze (Parquet)
    │
    └──► ETL ──► Silver (TimescaleDB) ──► Gold (Features)
                    │                         │
                    └──► Production           └──► ML Models
                         Dashboards
```

**Silver Layer**: Schema locked. Changes require:
1. Migration plan
2. Backward compatibility consideration
3. Downstream impact analysis (dashboards, Gold, ML)

**DQ Rigor**: Full - reject bad data, alert on anomalies, audit trail.

**Activities**:
- Operate production pipelines
- Monitor DQ metrics
- Feature engineering in Gold layer
- ML model training and inference

---

## Lifecycle Comparison Table

| Aspect | Early | DevStage | Stable |
|--------|-------|----------|--------|
| **Bronze** | Write | Write | Write |
| **Silver** | None or read-only | Write (evolving) | Write (locked) |
| **Gold** | None | Experimental | Production |
| **DQ Rigor** | Minimal | Medium | Full |
| **Schema Stability** | N/A | Iterating | Locked |
| **Dashboards** | Ad-hoc | Test | Production |
| **Rebuild Silver?** | N/A | Common | Rare (migration) |

## Transitioning Between Phases

### Early → DevStage

**Trigger**: Domain model drafted, ready to test schema.

**Actions**:
1. Create `config/silver/streams/{stream}/` directory
2. Define initial schema.yaml
3. Implement ETL pipeline
4. Build first test dashboard

### DevStage → Stable

**Trigger**: Schema validated, dashboards approved, DQ rules tuned.

**Actions**:
1. Lock schema (version it)
2. Migrate test dashboards to production
3. Enable full DQ alerting
4. Document schema for downstream users
5. Plan Gold layer features

## Benefits of This Approach

1. **No premature optimization**: Don't design schemas without understanding data
2. **Safe iteration**: Bronze always has raw data for rebuild
3. **Clear expectations**: Each phase has defined capabilities and constraints
4. **Gradual hardening**: DQ and schema stability increase with maturity
