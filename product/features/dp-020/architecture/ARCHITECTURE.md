# dp-020: Declarative Deploy - Architecture

**Status**: Proposed
**Date**: 2026-02-02
**Decision Makers**: NDP Architecture Team
**Feature**: dp-020 Declarative Deploy
**Parent ADRs**: ADR-016-002 (Declarative Deploy), ADR-019-001 (Two-Layer Validation)

---

## Executive Summary

dp-020 implements a declarative deployment system where agents declare **what changed** in a manifest, and deploy.sh orchestrates the necessary actions. This inverts deployment responsibility: agents express intent, deploy handles execution order and idempotency.

---

## System Context

### Current State (8+ Manual Steps)

```
Agent/Operator                     Device (Pi)
      |                                |
      +-- Edit YAML config ----------->|
      +-- SSH to Pi ------------------>|
      +-- Run deploy.sh sync --------->|
      +-- Run deploy.sh init-streams ->|
      +-- Write Silver DDL manually -->|
      +-- Apply DDL to TimescaleDB --->|
      +-- Run sync-dictionary -------->|
      +-- Restart app ---------------->|
```

**Problems**: Order-dependent, error-prone, undocumented, requires deep system knowledge.

### Target State (Single Command)

```
Agent/Operator                     Device (Pi)
      |                                |
      +-- Edit config file             |
      +-- Update manifest.json         |
      +-- git push ---------------->   |
      |                            git pull
      |                            ./deploy.sh apply
      |                                |
      |   +--------------------------- + Validates all declarations
      |   |                            + Runs migrations (if any)
      |   |                            + Generates & applies DDL (if any)
      |   |                            + Syncs streams to etcd
      |   |                            + Syncs dictionary
      |   |                            + Syncs dimensions
      |   |                            + Reloads affected streams
      |   |                            + Updates device state
      |   +--------------------------- +
      |                                |
      <-- Deployment complete ---------|
```

---

## Component Architecture

```
+------------------------------------------------------------------+
|                         deploy.sh apply                           |
+------------------------------------------------------------------+
                                |
                                v
+------------------------------------------------------------------+
|                      Manifest Parser                              |
|  - Load .deploy/manifest.json                                     |
|  - Validate schema version                                        |
|  - Build change list                                              |
+------------------------------------------------------------------+
                                |
                                v
+------------------------------------------------------------------+
|                      Orchestrator                                 |
|  - Resolve dependencies                                           |
|  - Order actions correctly                                        |
|  - Execute handlers sequentially                                  |
|  - Track success/failure                                          |
+------------------------------------------------------------------+
     |         |         |         |         |         |         |
     v         v         v         v         v         v         v
+--------+ +--------+ +--------+ +--------+ +--------+ +--------+ +--------+
|Container| |Migration| | Silver | | Stream | |Dictionary| |Dimensions| |Container|
| Build  | | Handler | | Table  | | Handler| | Handler | | Handler | | Restart|
| Handler|  |         | | Handler| |        | |         | |         | | Handler|
|        |  | silver- | | DDL Gen| | sync to| | sync to | | sync CSV| |        |
| dc     |  | migrate | | ->apply| | etcd   | |TimescaleDB| to TSDB | | dc up  |
| build  |  |         | |        | |        | |         | |         | | -d     |
+--------+ +--------+ +--------+ +--------+ +--------+ +--------+ +--------+
     |             |              |             |             |
     v             v              v             v             v
+------------------------------------------------------------------+
|                    Infrastructure Layer                           |
|  - TimescaleDB (Silver tables, dictionary, dimensions)            |
|  - etcd (stream configuration)                                    |
|  - File system (/var/ndp/ device state)                           |
+------------------------------------------------------------------+
```

---

## Data Flow

### Manifest Structure

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

### Dependency Resolution

The orchestrator executes actions in dependency order:

```
Phase 1: Validation
    +-- Validate all declarations (dp-019 validator)
    +-- Fail fast if any invalid

Phase 2: Container Builds (code updates)
    +-- Build container images with new code
    +-- Optional --no-cache for clean builds
    +-- Note: Builds early so new code is available for later phases

Phase 3: Migrations (database schema changes)
    +-- Run SQL migrations in order
    +-- Idempotent (track applied migrations)

Phase 4: Silver Tables (DDL generation)
    +-- Generate DDL from config
    +-- Apply CREATE TABLE / ADD COLUMN
    +-- Idempotent (IF NOT EXISTS)

Phase 5: Streams (config sync)
    +-- Sync declared streams to etcd
    +-- Per-stream atomic updates

Phase 6: Dimensions (reference data sync)
    +-- Sync dimension CSVs to TimescaleDB
    +-- Truncate and load strategy

Phase 7: Dictionary (metadata sync)
    +-- Sync data dictionary from config
    +-- Refresh silver_tables, silver_columns

Phase 8: Container Restarts (service reload)
    +-- Restart containers to load new config
    +-- Wait for health checks to pass
    +-- Note: Restarts late so new config is loaded

Phase 9: Device State Update
    +-- Update /var/ndp/deployed-version
    +-- Update /var/ndp/deployed-at
```

**Container phase ordering rationale**: Container builds happen early (Phase 2) so new code is available for migrations and other phases. Container restarts happen late (Phase 8) so containers pick up all new configuration, schema changes, and dimension data.

---

## Handler Architecture

Each declaration type has a dedicated handler. Handlers follow a common interface pattern:

### Handler Interface (Shell Functions)

```bash
# Handler contract (implemented as shell functions)
#
# Parameters:
#   $1 - Declaration JSON (from manifest)
#
# Returns:
#   0 - Success
#   1 - Failure (stops deployment)
#
# Side effects:
#   - Log to stdout
#   - May call external tools (psql, etcdctl, etc.)

handle_stream() {
    local declaration="$1"
    # Implementation
}

handle_silver_table() {
    local declaration="$1"
    # Implementation
}

handle_migration() {
    local declaration="$1"
    # Implementation
}

handle_dimensions() {
    local declaration="$1"
    # Implementation
}

handle_dictionary() {
    local declaration="$1"
    # Implementation
}

handle_container_build() {
    local declaration="$1"
    # Implementation
}

handle_container_restart() {
    local declaration="$1"
    # Implementation
}
```

### Handler Responsibilities

| Handler | Input | Output | Dependencies |
|---------|-------|--------|--------------|
| `container-build` | target, no_cache | image built | Docker running |
| `container-restart` | target | container restarted | Docker running, image built |
| `stream` | stream config path | etcd key/value | etcd running |
| `silver-table` | stream config | DDL executed | TimescaleDB running, migrations complete |
| `migration` | SQL file path | SQL executed | TimescaleDB running |
| `dimensions` | dimension config | CSV loaded | TimescaleDB running |
| `dictionary` | stream configs | dict tables updated | TimescaleDB running |

---

## DDL Generation Component

### Architecture

```
+------------------------------------------------------------------+
|                     DDL Generator                                 |
+------------------------------------------------------------------+
|                                                                   |
|  Input: Stream Config (silver_etl section)                        |
|                                                                   |
|  +------------------------------------------------------------+  |
|  | 1. Extract table metadata                                   |  |
|  |    - target_table: "silver.air_quality_readings"            |  |
|  |    - timestamp: observation_time (TIMESTAMPTZ)              |  |
|  |    - identity: ndp_id (TEXT)                                |  |
|  +------------------------------------------------------------+  |
|                              |                                    |
|                              v                                    |
|  +------------------------------------------------------------+  |
|  | 2. Type Mapping                                             |  |
|  |    - double_precision -> DOUBLE PRECISION                   |  |
|  |    - smallint -> SMALLINT                                   |  |
|  |    - text -> TEXT                                           |  |
|  |    - (see ADR-020-002 for full mapping)                     |  |
|  +------------------------------------------------------------+  |
|                              |                                    |
|                              v                                    |
|  +------------------------------------------------------------+  |
|  | 3. DDL Template Assembly                                    |  |
|  |    - CREATE TABLE IF NOT EXISTS                             |  |
|  |    - CREATE INDEX IF NOT EXISTS                             |  |
|  |    - SELECT create_hypertable(..., if_not_exists => TRUE)   |  |
|  |    - SELECT add_compression_policy(..., if_not_exists)      |  |
|  |    - GRANT statements                                       |  |
|  +------------------------------------------------------------+  |
|                              |                                    |
|                              v                                    |
|  +------------------------------------------------------------+  |
|  | 4. ADD COLUMN Detection (for existing tables)               |  |
|  |    - Query information_schema.columns                       |  |
|  |    - Compare config columns to existing                     |  |
|  |    - Generate ALTER TABLE ADD COLUMN for new columns        |  |
|  +------------------------------------------------------------+  |
|                                                                   |
|  Output: SQL DDL script                                           |
|                                                                   |
+------------------------------------------------------------------+
```

### Type Mapping Table

| Config Type | PostgreSQL DDL | Example |
|-------------|----------------|---------|
| `double_precision` | `DOUBLE PRECISION` | pm25, temperature |
| `real` | `REAL` | Lower-precision floats |
| `integer` | `INTEGER` | Count values |
| `bigint` | `BIGINT` | Large integers |
| `smallint` | `SMALLINT` | CO2 ppm, AQI |
| `text` | `TEXT` | String fields |
| `boolean` | `BOOLEAN` | Flags |
| `timestamptz` | `TIMESTAMPTZ` | Timestamps |
| `jsonb` | `JSONB` | Nested data |
| `text[]` | `TEXT[]` | Arrays (dq_flags) |

### Standard Columns (Always Generated)

| Column | Type | Purpose |
|--------|------|---------|
| `timestamp` | `TIMESTAMPTZ NOT NULL` | Primary time column (from config) |
| `ndp_id` | `TEXT NOT NULL` | Device/entity identifier |
| `dq_flags` | `TEXT[]` | Data quality flags |
| `_bronze_id` | `UUID` | Link to Bronze record |
| `_ingested_at` | `TIMESTAMPTZ DEFAULT NOW()` | Ingestion timestamp |

---

## Integration with Existing deploy.sh

### Current Commands (Preserved)

| Command | Status | Notes |
|---------|--------|-------|
| `deploy` | Preserved | Full deploy (build + start) |
| `start` | Preserved | Start services |
| `stop` | Preserved | Stop services |
| `sync` | Preserved | Sync config to etcd |
| `sync-dictionary` | Preserved | Sync data dictionary |
| `sync-dimensions` | Preserved | Sync dimensions |
| `silver-migrate` | Preserved | Run migrations |

### New Command

| Command | Purpose |
|---------|---------|
| `apply` | Execute manifest-driven deployment |

### Implementation Approach

```bash
# deploy.sh addition

apply() {
    log "Starting manifest-driven deployment..."

    local manifest_file="${REPO_ROOT}/.deploy/manifest.json"

    if [ ! -f "$manifest_file" ]; then
        error "No manifest found at $manifest_file"
        exit 1
    fi

    # Phase 1: Validation
    log "Phase 1: Validating declarations..."
    validate_manifest "$manifest_file"
    validate_all_configs

    # Phase 2: Container Builds (early - new code available for later phases)
    log "Phase 2: Building container images..."
    process_container_builds "$manifest_file"

    # Phase 3: Migrations
    log "Phase 3: Running migrations..."
    process_migrations "$manifest_file"

    # Phase 4: Silver Tables
    log "Phase 4: Processing silver-table declarations..."
    process_silver_tables "$manifest_file"

    # Phase 5: Streams
    log "Phase 5: Syncing streams to etcd..."
    process_streams "$manifest_file"

    # Phase 6: Dimensions
    log "Phase 6: Syncing dimensions..."
    process_dimensions "$manifest_file"

    # Phase 7: Dictionary
    log "Phase 7: Syncing dictionary..."
    process_dictionary "$manifest_file"

    # Phase 8: Container Restarts (late - picks up new config)
    log "Phase 8: Restarting containers..."
    process_container_restarts "$manifest_file"

    # Phase 9: State Update
    log "Phase 9: Updating device state..."
    update_device_state

    log "Deployment complete!"
}
```

---

## Device State Tracking

### State Files

| File | Content | Purpose |
|------|---------|---------|
| `/var/ndp/deployed-version` | Git commit SHA or tag | Track what's deployed |
| `/var/ndp/deployed-at` | ISO 8601 timestamp | Track when deployed |
| `/var/ndp/manifest-applied` | Manifest content hash | Detect drift |

### State Update Logic

```bash
update_device_state() {
    local state_dir="/var/ndp"
    mkdir -p "$state_dir"

    # Record current git version
    git -C "$REPO_ROOT" rev-parse HEAD > "$state_dir/deployed-version"

    # Record deployment timestamp
    date -Iseconds > "$state_dir/deployed-at"

    # Record manifest hash for drift detection
    sha256sum "${REPO_ROOT}/.deploy/manifest.json" | cut -d' ' -f1 > "$state_dir/manifest-applied"

    log "Device state updated: $(cat $state_dir/deployed-version)"
}
```

---

## Error Handling

### Failure Modes

| Phase | Failure | Recovery |
|-------|---------|----------|
| Validation | Invalid config | Stop, show errors, no changes made |
| Migration | SQL error | Stop, transaction rolled back |
| DDL | Table creation fails | Stop, partial state possible |
| Stream sync | etcd error | Stop, retry safe |
| Dictionary | TimescaleDB error | Stop, retry safe |
| Reload | App restart fails | Manual intervention needed |

### Idempotency Guarantees

| Operation | Idempotent? | Mechanism |
|-----------|-------------|-----------|
| Migrations | Yes | Migration tracking table |
| CREATE TABLE | Yes | `IF NOT EXISTS` |
| ADD COLUMN | Yes | `IF NOT EXISTS` check |
| Indexes | Yes | `IF NOT EXISTS` |
| Hypertable | Yes | `if_not_exists => TRUE` |
| Policies | Yes | `if_not_exists => TRUE` |
| etcd sync | Yes | PUT overwrites |
| Dictionary | Yes | DELETE + INSERT transaction |
| Dimensions | Yes | TRUNCATE + COPY |

---

## Security Considerations

### Permissions

| Component | Required Permissions |
|-----------|---------------------|
| deploy.sh | Execute on Pi, docker access |
| DDL Generator | `ndp_admin` role (CREATE, ALTER, GRANT) |
| etcd sync | etcd write access |
| State files | Write to /var/ndp/ |

### Validation Gates

1. **Manifest schema validation** - Reject malformed manifests
2. **Config validation (dp-019)** - Reject invalid stream configs
3. **SQL injection prevention** - Use parameterized queries in DDL

---

## Related ADRs

- **ADR-020-001**: Extensible Handler Architecture
- **ADR-020-002**: DDL Generation Strategy
- **ADR-020-003**: Manifest Schema Versioning
- **ADR-016-002**: Declarative Deploy (parent decision)
- **ADR-019-001**: Two-Layer Validation (validation architecture)

---

## References

- `/workspaces/neural-data-platform/deploy/pi/deploy.sh` - Current deployment script
- `/workspaces/neural-data-platform/product/features/dp-016/architecture/ADR-016-002-declarative-deploy.md` - Parent ADR
- `/workspaces/neural-data-platform/product/features/dp-019/specification/SUPPORTED-VALUES-RESEARCH.md` - Type mapping
- `/workspaces/neural-data-platform/product/features/dp-020/SCOPE.md` - Feature requirements

---

*Architecture document created: 2026-02-02*
*Feature: dp-020 Declarative Deploy*
