# dp-020: Declarative Deploy

## Parent Initiative

This feature implements **Phase 3** of [dp-016: Configuration Architecture Review](../dp-016/IMPLEMENTATION-ROADMAP.md).

**Absorbs**: dp-015 (Config-Driven Silver Table Creation)

---

## Problem Statement

Deploying configuration changes requires 8+ manual steps in a specific order:

1. Edit YAML config file
2. SSH to Pi
3. Run `deploy.sh sync` (config to etcd)
4. Run `deploy.sh init-streams` (if new stream)
5. Manually write Silver DDL
6. Apply DDL to TimescaleDB
7. Run `deploy.sh sync-dictionary`
8. Restart app (if schema changed)

This is error-prone, order-dependent, and undocumented. Agents and operators must remember which commands to run and in what order.

These issues were documented in dp-016's pain points (P-010, P-011, P-012, P-013).

---

## Goals

1. **Single command deployment** - `./deploy.sh apply` executes everything
2. **Declarative manifest** - Agents declare what changed, deploy figures out actions
3. **DDL generation** - Silver tables created/updated from config (no manual SQL)
4. **Schema evolution** - New columns added automatically when field_mappings change
5. **Correct ordering** - Dependencies resolved automatically
6. **Device state tracking** - Pi knows what's deployed

---

## Scope

### In Scope

**Manifest and Orchestration**

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 3.1 | Define manifest schema | `manifest.schema.json` with all declaration types | Schema validates all patterns |
| 3.2 | Create manifest parser | Parse and validate `.deploy/manifest.json` | Typed Rust structs |
| 3.9 | Create deploy.sh v2 | Orchestrates all actions from manifest | Single command deployment |
| 3.10 | Add device state tracking | `/var/ndp/deployed-version`, `/var/ndp/deployed-at` | Device knows what's deployed |

**Action: Stream Sync**

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 3.3 | Implement stream sync | Sync declared streams to etcd | Per-stream atomic updates |

**Action: Silver Table DDL Generation (dp-015)**

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 3.4 | Implement silver-table action | Generate DDL from silver_etl config | Creates tables from config |
| 3.4a | DDL generator: CREATE TABLE | Generate column definitions using type mapping | Correct PostgreSQL types |
| 3.4b | DDL generator: Indexes | Generate (timestamp, ndp_id) + DQ-derived indexes | Standard + custom indexes |
| 3.4c | DDL generator: Hypertable | Convert to hypertable with chunk_time_interval | Compression-ready |
| 3.4d | DDL generator: Policies | Apply compression and retention policies | Matches existing tables |
| 3.4e | DDL generator: Permissions | Grant to ndp_app, grafana_reader roles | Consistent permissions |
| 3.4f | Idempotent execution | IF NOT EXISTS everywhere | Safe to re-run |
| 3.4g | DDL generator: ADD COLUMN | Add new columns to existing tables | Schema evolution support |

**Action: Other Declarations**

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 3.5 | Implement migration action | Run SQL migrations in order | Tracks applied migrations |
| 3.6 | Implement dimensions action | Sync dimension CSV to TimescaleDB | Dimensions updated atomically |
| 3.7 | Implement dictionary action | Sync data dictionary from config | Dictionary reflects config |
| 3.8 | Implement reload logic | Hot-reload sources or full restart | Respects declared reload type |

### Out of Scope

- **Modify column type** - ALTER TABLE ALTER COLUMN (breaking change)
- **Remove column** - ALTER TABLE DROP COLUMN (breaking change)
- **Rename column** - Requires data migration
- Hot-reload implementation details (dp-021)
- Schema migration tool v1.1→v2.0 (dp-021)
- MCP write tools (dp-021)

*Note: Adding new columns IS supported (3.4g). Only destructive schema changes are excluded.*

---

## Technical Context

### Manifest Schema

```json
{
  "$schema": "./schemas/manifest.schema.json",
  "version": "1.0",
  "changes": [
    {
      "type": "stream",
      "id": "air-quality",
      "action": "update",
      "reload": "sources"
    },
    {
      "type": "silver-table",
      "stream_id": "air-quality",
      "action": "sync"
    },
    {
      "type": "migration",
      "file": "migrations/002-add-forecast-table.sql"
    },
    {
      "type": "dimensions",
      "action": "sync"
    },
    {
      "type": "dictionary",
      "action": "sync"
    }
  ]
}
```

### Declaration Types

| Type | Actions | Description |
|------|---------|-------------|
| `stream` | validate → sync → reload | Stream config changed |
| `silver-table` | generate DDL → apply | Create table or add columns from config |
| `migration` | apply SQL file | Run database migration |
| `dimensions` | sync CSV → TimescaleDB | Update dimension data |
| `dictionary` | sync config → data_dictionary | Refresh data dictionary |

### DDL Generation Example

**Input** (from stream config):
```json
{
  "silver_etl": {
    "target_table": "silver.air_quality_readings",
    "field_mappings": [
      {"target_column": "pm25", "source_path": "raw_payload.pm25", "target_type": "float"},
      {"target_column": "temperature", "source_path": "raw_payload.temp", "target_type": "float"}
    ]
  }
}
```

**Output** (generated DDL):
```sql
-- 3.4a: CREATE TABLE
CREATE TABLE IF NOT EXISTS silver.air_quality_readings (
    timestamp TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,
    pm25 DOUBLE PRECISION,
    temperature DOUBLE PRECISION,
    dq_flags TEXT[],
    _bronze_id UUID,
    _ingested_at TIMESTAMPTZ DEFAULT NOW()
);

-- 3.4b: Indexes
CREATE INDEX IF NOT EXISTS idx_air_quality_readings_time_id
    ON silver.air_quality_readings (timestamp, ndp_id);
CREATE INDEX IF NOT EXISTS idx_air_quality_readings_dq_flags
    ON silver.air_quality_readings USING GIN (dq_flags);

-- 3.4c: Hypertable
SELECT create_hypertable('silver.air_quality_readings', 'timestamp',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE);

-- 3.4d: Policies
SELECT add_compression_policy('silver.air_quality_readings',
    INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_retention_policy('silver.air_quality_readings',
    INTERVAL '90 days', if_not_exists => TRUE);

-- 3.4e: Permissions
GRANT SELECT, INSERT ON silver.air_quality_readings TO ndp_app;
GRANT SELECT ON silver.air_quality_readings TO grafana_reader;

-- 3.4g: ADD COLUMN (for existing tables with new field_mappings)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'silver'
        AND table_name = 'air_quality_readings'
        AND column_name = 'humidity'
    ) THEN
        ALTER TABLE silver.air_quality_readings ADD COLUMN humidity DOUBLE PRECISION;
    END IF;
END $$;
```

### Deploy Flow

```bash
# Development workflow
vi config/base/streams/new-sensor/config.json
vi .deploy/manifest.json
git add . && git commit -m "feat: add new-sensor"
git push

# Device (webhook or manual)
git pull
./deploy.sh apply

# deploy.sh apply executes:
# 1. Validate all declared changes (dp-019 validator)
# 2. Run migrations (if any)
# 3. Create silver tables (if any)
# 4. Sync streams to etcd
# 5. Sync dictionary
# 6. Sync dimensions
# 7. Reload affected streams
# 8. Update /var/ndp/deployed-version
```

### Device State Files

| File | Content | Purpose |
|------|---------|---------|
| `/var/ndp/deployed-version` | Git commit SHA | Track what's deployed |
| `/var/ndp/deployed-at` | ISO timestamp | Track when deployed |
| `/var/ndp/manifest-applied` | Last manifest hash | Detect drift |

---

## Deliverables

| Deliverable | Location | Description |
|-------------|----------|-------------|
| Manifest schema | `schemas/manifest.schema.json` | Declaration types and structure |
| Manifest template | `.deploy/manifest.json` | Example manifest |
| DDL generator | `tools/ndp-ddl-gen/` or inline in deploy | Generates Silver table SQL |
| deploy.sh v2 | `deploy/pi/deploy.sh` | Manifest-driven deployment |
| Device state | `/var/ndp/` | Deployment tracking files |
| **Deployment Guide** | `docs/procedures/DEPLOYMENT-DECLARATIVES.md` | Reference for all declaration types |
| **AgentDB Pattern** | Stored via `save-pattern` | Points agents to deployment guide |

### Deployment Guide Requirements

The `DEPLOYMENT-DECLARATIVES.md` must document:

1. **Each declaration type** with:
   - Purpose and when to use
   - Required and optional fields
   - Example manifest entry
   - What actions deploy.sh executes
   - How to verify success

2. **Declaration types to document:**
   - `stream` - Config sync to etcd
   - `silver-table` - DDL generation (CREATE TABLE, ADD COLUMN)
   - `migration` - SQL file execution
   - `dimensions` - CSV sync to TimescaleDB
   - `dictionary` - Data dictionary sync

3. **Common workflows:**
   - Adding a new stream end-to-end
   - Adding a column to existing stream
   - Running a database migration
   - Full deployment sequence

### AgentDB Pattern Requirement

After completing dp-020, store a pattern via `/save-pattern`:

```
taskType: "deployment:manifest-declaratives"
approach: "When deploying changes to NDP, agents MUST:
  1. Read docs/procedures/DEPLOYMENT-DECLARATIVES.md for available declaration types
  2. Create/update .deploy/manifest.json with required changes
  3. Run DEPLOY_ENV=integration ./deploy.sh apply to test locally
  4. Verify changes before production deployment

  Declaration types: stream, silver-table, migration, dimensions, dictionary

  Reference: docs/procedures/DEPLOYMENT-DECLARATIVES.md"
tags: ["deployment", "manifest", "declarative", "deploy.sh"]
```

This ensures future agents searching for "how to deploy" or "add stream" find the authoritative reference.

---

## Success Criteria

1. **Single command** - `./deploy.sh apply` handles all deployment
2. **DDL generated** - Silver tables created from config without manual SQL
3. **Schema evolution** - New columns added to existing tables automatically
4. **Correct ordering** - Migrations before tables, tables before data
5. **Idempotent** - Safe to run multiple times
6. **State tracked** - Device knows deployed version and timestamp
7. **Documented** - All declaration types documented in DEPLOYMENT-DECLARATIVES.md
8. **Discoverable** - AgentDB pattern stored so future agents find deployment guide

### Verification Commands

```bash
# Scenario 1: New stream (CREATE TABLE)
cat > .deploy/manifest.json << 'EOF'
{
  "version": "1.0",
  "changes": [
    {"type": "stream", "id": "new-sensor", "action": "create"},
    {"type": "silver-table", "stream_id": "new-sensor", "action": "sync"},
    {"type": "dictionary", "action": "sync"}
  ]
}
EOF

DEPLOY_ENV=integration ./deploy.sh apply
psql -c "\d silver.new_sensor_readings"  # Table exists

# Scenario 2: Add column to existing stream (ALTER TABLE ADD COLUMN)
# Edit config/base/streams/air-quality/config.json to add new field_mapping
cat > .deploy/manifest.json << 'EOF'
{
  "version": "1.0",
  "changes": [
    {"type": "stream", "id": "air-quality", "action": "update"},
    {"type": "silver-table", "stream_id": "air-quality", "action": "sync"},
    {"type": "dictionary", "action": "sync"}
  ]
}
EOF

DEPLOY_ENV=integration ./deploy.sh apply
psql -c "\d silver.air_quality_observations"  # New column exists
```

---

## Testing Expectations

All dp-020 functionality MUST be tested in the local Docker integration environment before deployment to Pi.

### Infrastructure

| Component | Location | Notes |
|-----------|----------|-------|
| Docker stack | `docker-compose.integration.yml` | etcd, TimescaleDB, MQTT, app, MCP, Grafana |
| Test runner | `scripts/integration-test.sh` | start/stop/clean/status |
| Deploy automation | `DEPLOY_ENV=integration ./deploy.sh` | Same script, different target |

### Test Scenarios

| ID | Scenario | Verification Command |
|----|----------|---------------------|
| T1 | New stream → CREATE TABLE | `psql -c "\d silver.{table}"` |
| T2 | Add field_mapping → ADD COLUMN | `psql -c "\d silver.{table}"` shows new column |
| T3 | Idempotent (run twice) | No errors on second run |
| T4 | Type mapping | Column types match config |
| T5 | Indexes created | `psql -c "\di silver.*"` |
| T6 | Hypertable conversion | `SELECT * FROM timescaledb_information.hypertables` |
| T7 | Compression policy | `SELECT * FROM timescaledb_information.jobs WHERE proc_name = 'policy_compression'` |
| T8 | Retention policy | `SELECT * FROM timescaledb_information.jobs WHERE proc_name = 'policy_retention'` |
| T9 | Permissions | `psql -U ndp_app -c "SELECT 1 FROM silver.{table} LIMIT 1"` |
| T10 | Device state | `cat /var/ndp/deployed-version` |

### Test Workflow

```bash
# 1. Start integration environment
./scripts/integration-test.sh start

# 2. Test new stream (T1, T4-T9)
DEPLOY_ENV=integration ./deploy.sh apply

# 3. Verify table created
docker exec integration-timescaledb psql -U postgres -d ndp -c "\d silver.air_quality_observations"

# 4. Test add column (T2) - modify config, re-apply
DEPLOY_ENV=integration ./deploy.sh apply

# 5. Test idempotent (T3) - run again
DEPLOY_ENV=integration ./deploy.sh apply  # Should succeed with no changes

# 6. Clean up
./scripts/integration-test.sh clean
```

### Test Artifacts

Test-specific configs (if needed) go in:
- `config/base/streams/_test-dp020/config.json` - Underscore prefix = excluded from prod
- `.deploy/manifest.json` - Test manifest

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-018 | **REQUIRED** | JSON configs, ConfigLoader |
| dp-019 | **REQUIRED** | Validation pipeline, type mapping research |
| dp-017 | **REQUIRED** | Integration environment |

---

## References

- [dp-016 IMPLEMENTATION-ROADMAP.md](../dp-016/IMPLEMENTATION-ROADMAP.md) - Phase 3 details
- [dp-016 ADR-016-002: Declarative Deploy](../dp-016/architecture/ADR-016-002-declarative-deploy.md)
- [dp-015 SCOPE.md](../dp-015/SCOPE.md) - Absorbed feature (DDL generation)
- [dp-019 DDL-GENERATION.md](../dp-019/) - Type mapping and index strategy (research output)

---

*Scope created: 2026-02-01*
*Scope updated: 2026-02-02 - Added ADD COLUMN support (3.4g), testing expectations, documentation requirements*
*Parent: dp-016 Configuration Architecture Review*
