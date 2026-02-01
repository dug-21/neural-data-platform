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
3. **DDL generation** - Silver tables created from config (no manual SQL)
4. **Correct ordering** - Dependencies resolved automatically
5. **Device state tracking** - Pi knows what's deployed

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

**Action: Other Declarations**

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 3.5 | Implement migration action | Run SQL migrations in order | Tracks applied migrations |
| 3.6 | Implement dimensions action | Sync dimension CSV to TimescaleDB | Dimensions updated atomically |
| 3.7 | Implement dictionary action | Sync data dictionary from config | Dictionary reflects config |
| 3.8 | Implement reload logic | Hot-reload sources or full restart | Respects declared reload type |

### Out of Scope

- Hot-reload implementation details (dp-021)
- Schema migration tool v1.1→v2.0 (dp-021)
- MCP write tools (dp-021)

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
      "action": "create"
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
| `silver-table` | generate DDL → apply | Create Silver table from config |
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

---

## Success Criteria

1. **Single command** - `./deploy.sh apply` handles all deployment
2. **DDL generated** - Silver tables created from config without manual SQL
3. **Correct ordering** - Migrations before tables, tables before data
4. **Idempotent** - Safe to run multiple times
5. **State tracked** - Device knows deployed version and timestamp

### Verification Commands

```bash
# Create manifest for new stream
cat > .deploy/manifest.json << 'EOF'
{
  "version": "1.0",
  "changes": [
    {"type": "stream", "id": "new-sensor", "action": "create"},
    {"type": "silver-table", "stream_id": "new-sensor", "action": "create"},
    {"type": "dictionary", "action": "sync"}
  ]
}
EOF

# Deploy (integration environment)
DEPLOY_ENV=integration ./deploy.sh apply

# Verify
DEPLOY_ENV=integration ./deploy.sh status
cat /var/ndp/deployed-version
psql -c "\d silver.new_sensor_readings"
```

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
*Parent: dp-016 Configuration Architecture Review*
