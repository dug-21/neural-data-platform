# dp-015: Config-Driven Silver Table Creation

## Problem Statement

When adding a new stream to NDP, the current workflow requires **manual DDL execution** to create Silver tables. This breaks the "config-driven" promise of the platform.

**Observed during air-012 (Home Assistant Integration):**

1. Created stream config YAML with `silver_etl` section defining target table, field mappings, DQ rules
2. Synced config to etcd via `deploy.sh sync`
3. Restarted app - Bronze layer started working immediately
4. Silver layer failed silently - table `silver.state_events` didn't exist
5. Had to manually run DDL: `cat *.sql | docker exec ... psql`

**The gap:** YAML config describes the Silver schema, but nothing creates it.

---

## Current Architecture

```
Stream Config YAML
├── stream_id, description, fields
├── sources[] (MQTT, HTTP, etc.)
├── silver_etl:
│   ├── target_table: silver.state_events
│   ├── field_mappings[]
│   ├── dq_rules[]
│   └── deduplication, incremental config
└── entity_schemas[]

                    ↓ sync to etcd

Bronze Layer: Works automatically (Parquet files created on first message)

Silver Layer: REQUIRES MANUAL DDL
              - Table must pre-exist
              - Indexes, compression, retention policies
              - Hypertable conversion
              - Permissions
```

---

## Desired Behavior

**Adding a new stream should be fully config-driven:**

1. Create/edit YAML config with `silver_etl` section
2. Run `deploy.sh sync` (or equivalent)
3. **System automatically:**
   - Creates Silver table if it doesn't exist
   - Adds/modifies columns based on field_mappings
   - Creates indexes for identity/timestamp columns
   - Converts to hypertable
   - Applies compression/retention policies
   - Grants permissions to ndp_app role
4. ETL starts flowing Bronze → Silver

---

## Design Questions

### 1. Schema Generation Strategy

| Option | Pros | Cons |
|--------|------|------|
| **A: Generate DDL from YAML** | Single source of truth, portable | Complex type mapping, migration handling |
| **B: Hybrid (YAML + DDL templates)** | Explicit control, familiar | Two files to maintain |
| **C: Schema inference from Bronze** | Zero config | Loses type precision, DQ rules |

### 2. When to Create/Modify Tables

| Trigger | Pros | Cons |
|---------|------|------|
| **At config sync time** | Immediate feedback | Requires DB connection during sync |
| **At ETL first run** | Lazy creation, no sync dependency | Silent failure if schema wrong |
| **Separate CLI command** | Explicit control | Extra step |

### 3. Schema Evolution

- What happens when field_mappings change?
- Add column vs. alter column vs. recreate?
- How to handle hypertable limitations (can't alter certain properties)?

### 4. Type Mapping

YAML config uses abstract types. Need mapping to PostgreSQL/TimescaleDB:

| YAML Type | PostgreSQL Type | Notes |
|-----------|-----------------|-------|
| `string` | `TEXT` | |
| `float` | `DOUBLE PRECISION` | |
| `int` | `BIGINT` or `INTEGER`? | |
| `timestamp` | `TIMESTAMPTZ` | |
| `boolean` | `BOOLEAN` | |
| `json` | `JSONB` | |

### 5. Index Strategy

- Auto-create index on `(timestamp, ndp_id)` for all streams?
- Parse DQ rules to determine additional indexes?
- GIN indexes for array columns (dq_flags)?

---

## Scope Options

### Option A: Minimal - CLI Command (Recommended for MVP)

Add `ndp schema apply <stream_id>` command that:
- Reads silver_etl config from etcd
- Generates and executes DDL
- Idempotent (IF NOT EXISTS everywhere)
- Reports what was created/skipped

**Workflow:**
```bash
./deploy.sh sync                    # Sync YAML to etcd
ndp schema apply home-assistant-state  # Create Silver table
./deploy.sh restart                 # Start ETL
```

### Option B: Medium - Auto-Create on ETL Start

ETL process checks if target_table exists before first run:
- If missing, generate and execute DDL
- Log schema creation
- Continue with ETL

**Risk:** Schema errors discovered at runtime, not config time.

### Option C: Full - Schema Migration System

Full migration tracking like Flyway/Alembic:
- Version-controlled schema changes
- Rollback support
- Diff detection between config and actual schema

**Likely overkill for current scale.**

---

## Out of Scope

- Modifying existing columns (breaking changes)
- Cross-stream schema dependencies
- Multi-database support
- Schema rollback/downgrade

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| silver_etl config format | Stable | Defined in dp-006 |
| etcd config sync | Working | Scripts exist |
| TimescaleDB connection | Working | Via ndp-etl binary |

---

## Success Criteria

1. New stream with silver_etl config can be added without writing SQL
2. Table creation is idempotent (safe to run multiple times)
3. Created tables match manually-written DDL quality (indexes, compression, etc.)
4. Clear error messages if config is invalid

---

## References

- **air-012**: Exposed this gap during Home Assistant integration
- **dp-006**: Defines silver_etl config format
- **dp-013**: Dimension tables (similar problem, solved differently with CSV source)

---

## Notes

This is a **research/design** feature. Implementation complexity depends on chosen scope option. Recommend starting with Option A (CLI command) to validate the approach before automating further.
