# dp-020: Declarative Deploy - SPARC Specification

**Document Type**: SPARC Specification (Phase S)
**Feature**: dp-020 Declarative Deploy
**Version**: 1.0
**Date**: 2026-02-02
**Parent**: dp-016 Configuration Architecture Review
**Dependencies**: dp-018 JSON Config Foundation, dp-019 Config Validation Pipeline

---

## 1. Executive Summary

This specification defines the requirements for implementing Phase 3 of the dp-016 Configuration Architecture roadmap. The goal is to establish a manifest-driven deployment system that reduces the 8+ manual deployment steps to a single `./deploy.sh apply` command.

### Key Outcomes

1. **Manifest-driven deployment** - Agents declare changes in `.deploy/manifest.json`; deploy.sh orchestrates all actions
2. **DDL generation** - Silver tables created/updated automatically from `silver_etl` configuration
3. **Schema evolution** - New columns added via `ALTER TABLE ADD COLUMN` when `field_mappings` change
4. **Extensible architecture** - Plugin/handler pattern for declaration types; new types easily added
5. **Device state tracking** - `/var/ndp/` files track deployed version and timestamp
6. **Idempotent execution** - Safe to run multiple times without side effects

### Core Architecture Principle

**Declare intent, deploy executes.**

Agents declare what changed in the manifest. The deployment system figures out what actions to execute and in what order.

---

## 2. Requirements Analysis

### 2.1 Functional Requirements

#### Manifest Parsing and Orchestration

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-001** | Define manifest schema | HIGH | JSON Schema `manifest.schema.json` validates all declaration types with `additionalProperties: false` | Task 3.1 |
| **FR-002** | Parse manifest JSON | HIGH | Rust parser produces typed `Manifest` struct; invalid JSON returns structured error | Task 3.2 |
| **FR-003** | Validate manifest structure | HIGH | Schema validation catches unknown fields, missing required fields, invalid enums | Task 3.2 |
| **FR-004** | Determine execution order | HIGH | Dependencies resolved: migrations before silver-tables, silver-tables before streams | Task 3.9 |
| **FR-005** | Track device state | HIGH | Write `/var/ndp/deployed-version` (git SHA), `/var/ndp/deployed-at` (ISO timestamp), `/var/ndp/manifest-applied` (hash) | Task 3.10 |

#### Declaration Type: `stream`

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-010** | Validate stream config | CRITICAL | Call `ndp-validate` (dp-019) on declared stream; block on validation failure | Task 3.3 |
| **FR-011** | Sync stream to etcd | HIGH | Write JSON blob to `/streams/{stream_id}/config` in etcd atomically | Task 3.3 |
| **FR-012** | Support reload types | MEDIUM | `reload: "sources"` triggers hot-reload; `reload: "full"` triggers app restart; `reload: "none"` skips reload | Task 3.8 |
| **FR-013** | Per-stream isolation | HIGH | Syncing one stream does not affect other streams in etcd | Task 3.3 |

#### Declaration Type: `silver-table`

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-020** | Generate CREATE TABLE DDL | HIGH | Generate DDL from `silver_etl.target_table` and `field_mappings`; use type mapping | Task 3.4a |
| **FR-021** | Generate standard columns | HIGH | Include `timestamp TIMESTAMPTZ NOT NULL`, `ndp_id TEXT NOT NULL`, `dq_flags TEXT[]`, `_bronze_id UUID`, `_ingested_at TIMESTAMPTZ DEFAULT NOW()` | Task 3.4a |
| **FR-022** | Generate indexes | HIGH | Create index on `(timestamp, ndp_id)`; create GIN index on `dq_flags` | Task 3.4b |
| **FR-023** | Generate hypertable | HIGH | Call `create_hypertable()` with `chunk_time_interval => INTERVAL '1 day'` and `if_not_exists => TRUE` | Task 3.4c |
| **FR-024** | Generate compression policy | MEDIUM | Call `add_compression_policy()` with `INTERVAL '7 days'` and `if_not_exists => TRUE` | Task 3.4d |
| **FR-025** | Generate retention policy | MEDIUM | Call `add_retention_policy()` with configured `retention_days` or default 90 days | Task 3.4d |
| **FR-026** | Generate permissions | HIGH | GRANT SELECT, INSERT to `ndp_app`; GRANT SELECT to `grafana_reader` | Task 3.4e |
| **FR-027** | Idempotent execution | CRITICAL | Use `IF NOT EXISTS` for CREATE TABLE, CREATE INDEX; use `if_not_exists => TRUE` for hypertable/policies | Task 3.4f |
| **FR-028** | Generate ADD COLUMN DDL | HIGH | For existing tables, generate `ALTER TABLE ADD COLUMN` for new `field_mappings` entries not in current schema | Task 3.4g |
| **FR-029** | Type mapping | HIGH | Map config types to PostgreSQL types per mapping table (see Section 5) | Task 3.4a |

#### Declaration Type: `migration`

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-030** | Execute SQL migration file | HIGH | Run SQL file in transaction; track in `silver.migrations` table | Task 3.5 |
| **FR-031** | Track applied migrations | HIGH | Record migration filename, SHA256 hash, applied_at timestamp | Task 3.5 |
| **FR-032** | Skip already-applied | HIGH | If migration hash matches previously applied, skip without error | Task 3.5 |
| **FR-033** | Rollback on failure | MEDIUM | If migration fails, rollback transaction; report error with filename and line number | Task 3.5 |

#### Declaration Type: `dimensions`

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-040** | Sync dimension CSV | HIGH | Load CSV from `config/base/dimensions/{id}/data.csv` to configured target table | Task 3.6 |
| **FR-041** | Truncate and load | HIGH | Default strategy: TRUNCATE then COPY | Task 3.6 |
| **FR-042** | Schema validation | MEDIUM | Validate CSV columns match dimension schema definition | Task 3.6 |

#### Declaration Type: `dictionary`

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-050** | Sync data dictionary | HIGH | Generate and execute data dictionary sync SQL from all stream configs | Task 3.7 |
| **FR-051** | Sync silver metadata | HIGH | Populate `silver_tables`, `silver_columns`, `silver_lineage`, `silver_dq_rules` | Task 3.7 |

#### Declaration Type: `container`

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-070** | Container actions | HIGH | Container declaration type SHALL support `build` and `restart` actions | Task 3.11 |
| **FR-071** | Container targets | HIGH | Container targets SHALL include: air-quality-app, ndp-mcp-server, silver-etl, grafana | Task 3.11 |
| **FR-072** | Build no_cache flag | MEDIUM | Build action SHALL support optional `no_cache` flag (default: false) | Task 3.11 |
| **FR-073** | Build phase ordering | HIGH | Build actions SHALL execute before config sync (early phase, after validation) | Task 3.11 |
| **FR-074** | Restart phase ordering | HIGH | Restart actions SHALL execute after all config changes applied (late phase) | Task 3.11 |
| **FR-075** | Build command | HIGH | Build action SHALL use `docker compose build [--no-cache] <target>` | Task 3.11 |
| **FR-076** | Restart command | HIGH | Restart action SHALL use `docker compose up -d <target>` (recreates container) | Task 3.11 |

#### Orchestration

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-060** | Single command | CRITICAL | `./deploy.sh apply` reads manifest and executes all actions | Task 3.9 |
| **FR-061** | Execution order | HIGH | Order: validate all -> container builds -> migrations -> silver-tables -> streams -> dictionary -> dimensions -> container restarts -> reload | Task 3.9 |
| **FR-062** | Fail-fast | HIGH | If any validation fails, abort before any mutations; report all errors | Task 3.9 |
| **FR-063** | Atomic per-declaration | MEDIUM | Each declaration type executes atomically; failure in one type does not corrupt others | Task 3.9 |
| **FR-064** | Progress reporting | MEDIUM | Log each step with clear status: `[DEPLOY] Validating stream air-quality... OK` | Task 3.9 |

### 2.2 Non-Functional Requirements

| ID | Category | Requirement | Measurement | Traces To |
|----|----------|-------------|-------------|-----------|
| **NFR-001** | Extensibility | New declaration type can be added by implementing handler interface | Handler interface documented; template handler provided | Architecture |
| **NFR-002** | Extensibility | Manifest schema supports versioning via `version` field | Schema version 1.0 defined; upgrade path documented | FR-001 |
| **NFR-003** | Idempotency | Running `deploy.sh apply` twice produces same state | Integration test runs apply twice; no errors on second run | FR-027 |
| **NFR-004** | Error Handling | All errors include context: declaration type, stream ID, specific field | Error format documented; actionable messages | Error Format |
| **NFR-005** | Performance | DDL generation completes in <1 second per table | Benchmark with 10 streams | Performance |
| **NFR-006** | Performance | Full deploy completes in <30 seconds (excluding Docker operations) | Benchmark with 5 streams, 2 migrations | Performance |
| **NFR-007** | Testability | All declaration handlers testable in isolation | Unit tests for each handler | Testing |
| **NFR-008** | Testability | Full deploy testable in docker-compose.integration.yml | Integration test suite passes | dp-017 |
| **NFR-009** | Portability | deploy.sh runs on Raspberry Pi (Ubuntu 25.04 ARM64) | Tested on Pi | Platform |
| **NFR-010** | Portability | DDL generator is pure Rust, no runtime dependencies | No Python, Node.js required | Platform |

---

## 3. Acceptance Criteria

### AC-001: Manifest Validation

```gherkin
Feature: Manifest Validation

  Scenario: Valid manifest accepted
    Given a manifest file with valid stream and silver-table declarations
    When I run `./deploy.sh apply`
    Then deploy proceeds to execution phase
    And log shows "Manifest validated successfully"

  Scenario: Invalid manifest rejected - unknown declaration type
    Given a manifest with a declaration type "unknown-type"
    When I run `./deploy.sh apply`
    Then deploy exits with code 1
    And error message contains "Unknown declaration type: unknown-type"

  Scenario: Invalid manifest rejected - missing required field
    Given a manifest with a stream declaration missing "id" field
    When I run `./deploy.sh apply`
    Then deploy exits with code 1
    And error message contains "Missing required field 'id'"
```

### AC-002: Stream Sync

```gherkin
Feature: Stream Configuration Sync

  Scenario: New stream synced to etcd
    Given a valid stream config at config/base/streams/new-sensor/config.json
    And a manifest declaring {"type": "stream", "id": "new-sensor", "action": "create"}
    When I run `./deploy.sh apply`
    Then etcd key "/streams/new-sensor/config" contains the JSON config
    And log shows "Stream new-sensor synced to etcd"

  Scenario: Updated stream synced to etcd
    Given an existing stream "air-quality" in etcd
    And an updated config/base/streams/air-quality/config.json
    And a manifest declaring {"type": "stream", "id": "air-quality", "action": "update"}
    When I run `./deploy.sh apply`
    Then etcd key "/streams/air-quality/config" contains the updated JSON
    And log shows "Stream air-quality updated in etcd"

  Scenario: Invalid stream blocks sync
    Given a stream config with invalid source_path reference
    And a manifest declaring that stream
    When I run `./deploy.sh apply`
    Then deploy exits with code 1
    And error contains "Validation failed for stream"
    And etcd is NOT updated
```

### AC-003: Silver Table DDL Generation

```gherkin
Feature: Silver Table DDL Generation

  Scenario: CREATE TABLE for new stream
    Given a stream "new-sensor" with silver_etl configuration
    And field_mappings: [{target_column: "pm25", target_type: "float"}, {target_column: "temp", target_type: "float"}]
    And a manifest declaring {"type": "silver-table", "stream_id": "new-sensor", "action": "sync"}
    When I run `./deploy.sh apply`
    Then TimescaleDB has table silver.new_sensor_readings
    And table has columns: timestamp, ndp_id, pm25, temp, dq_flags, _bronze_id, _ingested_at
    And column pm25 has type DOUBLE PRECISION
    And index idx_new_sensor_readings_time_id exists
    And table is a hypertable with chunk_time_interval = 1 day

  Scenario: ADD COLUMN for existing stream
    Given existing table silver.air_quality_readings with columns pm25, temperature
    And stream config adds new field_mapping: {target_column: "humidity", target_type: "float"}
    And a manifest declaring {"type": "silver-table", "stream_id": "air-quality", "action": "sync"}
    When I run `./deploy.sh apply`
    Then table silver.air_quality_readings has new column humidity
    And column humidity has type DOUBLE PRECISION
    And existing data is preserved (no data loss)

  Scenario: Idempotent execution - no error on re-run
    Given silver table already exists with all columns
    And same manifest as previous run
    When I run `./deploy.sh apply` twice
    Then second run completes without error
    And log shows "Table silver.xxx already exists, checking for column additions"
```

### AC-004: Migration Execution

```gherkin
Feature: Migration Execution

  Scenario: New migration applied
    Given a migration file migrations/002-add-forecast-table.sql
    And a manifest declaring {"type": "migration", "file": "migrations/002-add-forecast-table.sql"}
    When I run `./deploy.sh apply`
    Then migration SQL is executed in a transaction
    And silver.migrations table has entry for 002-add-forecast-table.sql
    And log shows "Migration 002-add-forecast-table.sql applied"

  Scenario: Already-applied migration skipped
    Given migration 002 was previously applied (in silver.migrations table)
    And same manifest as before
    When I run `./deploy.sh apply`
    Then migration is NOT re-executed
    And log shows "Migration 002-add-forecast-table.sql already applied, skipping"

  Scenario: Failed migration rolls back
    Given a migration file with invalid SQL
    When I run `./deploy.sh apply`
    Then transaction is rolled back
    And deploy exits with code 1
    And error contains migration filename and SQL error
    And database state is unchanged
```

### AC-005: Device State Tracking

```gherkin
Feature: Device State Tracking

  Scenario: State files updated after successful deploy
    Given current git commit is abc123
    When I run `./deploy.sh apply` successfully
    Then /var/ndp/deployed-version contains "abc123"
    And /var/ndp/deployed-at contains ISO 8601 timestamp within last minute
    And /var/ndp/manifest-applied contains SHA256 hash of manifest.json

  Scenario: State files NOT updated on failed deploy
    Given a manifest with invalid stream
    When I run `./deploy.sh apply` (fails)
    Then /var/ndp/deployed-version is unchanged
    And /var/ndp/deployed-at is unchanged
```

### AC-006: Container Operations

```gherkin
Feature: Container Operations

  Scenario: Container build before config sync
    Given a manifest with container build declaration:
      | type      | container         |
      | target    | air-quality-app   |
      | action    | build             |
      | no_cache  | false             |
    When I run `./deploy.sh apply`
    Then Docker image for air-quality-app is rebuilt
    And build occurs before any config sync operations
    And log shows "Building container air-quality-app..."

  Scenario: Container build with no_cache flag
    Given a manifest with container build declaration:
      | type      | container         |
      | target    | silver-etl        |
      | action    | build             |
      | no_cache  | true              |
    When I run `./deploy.sh apply`
    Then Docker build runs with --no-cache flag
    And log shows "Building container silver-etl (no-cache)..."

  Scenario: Container restart after config changes
    Given a manifest with stream update and container restart declarations
    When I run `./deploy.sh apply`
    Then stream config is synced to etcd first
    Then container is restarted after config sync
    And log shows "Restarting container air-quality-app..."
    And container picks up new configuration

  Scenario: Invalid container target rejected
    Given a manifest with container declaration for "unknown-service"
    When I run `./deploy.sh apply`
    Then deploy exits with code 1
    And error message contains "Invalid container target: unknown-service"
    And error lists valid targets: air-quality-app, ndp-mcp-server, silver-etl, grafana
```

### AC-007: Integration Test Workflow

```gherkin
Feature: Integration Test Workflow

  Scenario: Full deployment in integration environment
    Given docker-compose.integration.yml is running
    And config/base/streams/air-quality/config.json exists
    And .deploy/manifest.json declares stream and silver-table
    When I run `DEPLOY_ENV=integration ./deploy.sh apply`
    Then deploy completes successfully
    And etcd contains /streams/air-quality/config
    And TimescaleDB has silver.air_quality_readings table
    And table is a hypertable

  Scenario: Test new stream end-to-end
    Given integration environment running
    And new stream config created at config/base/streams/_test-dp020/config.json
    And manifest updated with stream and silver-table declarations
    When I run `DEPLOY_ENV=integration ./deploy.sh apply`
    Then silver._test_dp020_readings table exists
    And psql query `\d silver._test_dp020_readings` shows all expected columns
```

---

## 4. Declaration Types Matrix

### Declaration Type Summary

| Type | Required Fields | Optional Fields | Actions Executed |
|------|-----------------|-----------------|------------------|
| `stream` | `id` | `action`, `reload` | validate -> sync to etcd -> reload |
| `silver-table` | `stream_id` | `action` | generate DDL -> apply to TimescaleDB |
| `migration` | `file` | - | apply SQL file in transaction |
| `dimensions` | - | `action` | sync CSV -> TimescaleDB |
| `dictionary` | - | `action` | sync config -> data_dictionary schema |
| `container` | `target`, `action` | `no_cache` | build or restart Docker container |

### Execution Order

```
1. VALIDATE phase (fail-fast)
   ├── Validate manifest schema
   ├── Validate all declared stream configs (dp-019 validator)
   ├── Validate migration files exist
   └── Validate container targets exist

2. CONTAINER BUILD phase (early)
   └── 2.1 container builds (docker compose build)

3. MUTATIONS phase (ordered)
   ├── 3.1 migrations (SQL files in declared order)
   ├── 3.2 silver-tables (DDL for each stream)
   ├── 3.3 streams (sync to etcd)
   ├── 3.4 dimensions (sync to TimescaleDB)
   └── 3.5 dictionary (sync to data_dictionary)

4. CONTAINER RESTART phase (late)
   └── 4.1 container restarts (docker compose up -d)

5. RELOAD phase
   ├── Hot-reload sources (for reload: "sources")
   ├── Full restart (for reload: "full")
   └── Skip (for reload: "none")

6. FINALIZE phase
   ├── Update /var/ndp/deployed-version
   ├── Update /var/ndp/deployed-at
   └── Update /var/ndp/manifest-applied
```

**Orchestration Order Summary:**
1. Validation
2. Container builds (NEW)
3. Migrations
4. Silver tables
5. Streams
6. Dimensions
7. Dictionary
8. Container restarts (NEW)
9. Device state

---

## 5. Data Model Specification

### 5.1 Type Mapping: Config to PostgreSQL

| Config Type (`target_type`) | PostgreSQL DDL Type | Information Schema `udt_name` | Notes |
|-----------------------------|---------------------|------------------------------|-------|
| `float` | `DOUBLE PRECISION` | `float8` | Default for numeric fields |
| `double_precision` | `DOUBLE PRECISION` | `float8` | Explicit double |
| `real` | `REAL` | `float4` | Lower precision float |
| `int` | `INTEGER` | `int4` | Standard integer |
| `integer` | `INTEGER` | `int4` | Alias |
| `smallint` | `SMALLINT` | `int2` | Small integer |
| `bigint` | `BIGINT` | `int8` | Large integer |
| `string` | `TEXT` | `text` | Variable length string |
| `text` | `TEXT` | `text` | Alias |
| `varchar` | `VARCHAR` | `varchar` | Variable with limit |
| `bool` | `BOOLEAN` | `bool` | Boolean |
| `boolean` | `BOOLEAN` | `bool` | Alias |
| `timestamp` | `TIMESTAMPTZ` | `timestamptz` | Always timezone-aware |
| `timestamptz` | `TIMESTAMPTZ` | `timestamptz` | Alias |
| `json` | `JSONB` | `jsonb` | Binary JSON |
| `jsonb` | `JSONB` | `jsonb` | Alias |
| `text[]` | `TEXT[]` | `_text` | Text array |

### 5.2 Standard Columns (Always Generated)

| Column | Type | Constraint | Purpose |
|--------|------|------------|---------|
| `timestamp` | `TIMESTAMPTZ` | `NOT NULL` | Hypertable time dimension |
| `ndp_id` | `TEXT` | `NOT NULL` | Device/entity identifier |
| `dq_flags` | `TEXT[]` | - | Data quality flags from DQ rules |
| `_bronze_id` | `UUID` | - | Provenance link to Bronze record |
| `_ingested_at` | `TIMESTAMPTZ` | `DEFAULT NOW()` | Ingestion timestamp |

### 5.3 Standard Indexes

| Index Name Pattern | Columns | Type | Purpose |
|--------------------|---------|------|---------|
| `idx_{table}_time_id` | `(timestamp, ndp_id)` | BTREE | Primary query pattern |
| `idx_{table}_dq_flags` | `(dq_flags)` | GIN | DQ flag queries |

### 5.4 Manifest Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "manifest.schema.json",
  "title": "NDP Deployment Manifest",
  "type": "object",
  "required": ["version", "changes"],
  "additionalProperties": false,
  "properties": {
    "version": {
      "type": "string",
      "enum": ["1.0"],
      "description": "Manifest schema version"
    },
    "changes": {
      "type": "array",
      "items": {
        "oneOf": [
          {"$ref": "#/$defs/stream-declaration"},
          {"$ref": "#/$defs/silver-table-declaration"},
          {"$ref": "#/$defs/migration-declaration"},
          {"$ref": "#/$defs/dimensions-declaration"},
          {"$ref": "#/$defs/dictionary-declaration"},
          {"$ref": "#/$defs/container-declaration"}
        ]
      }
    }
  },
  "$defs": {
    "stream-declaration": {
      "type": "object",
      "required": ["type", "id"],
      "additionalProperties": false,
      "properties": {
        "type": {"const": "stream"},
        "id": {"type": "string", "pattern": "^[a-z][a-z0-9-]*$"},
        "action": {"enum": ["create", "update", "validate-only"], "default": "update"},
        "reload": {"enum": ["sources", "full", "none"], "default": "none"}
      }
    },
    "silver-table-declaration": {
      "type": "object",
      "required": ["type", "stream_id"],
      "additionalProperties": false,
      "properties": {
        "type": {"const": "silver-table"},
        "stream_id": {"type": "string", "pattern": "^[a-z][a-z0-9-]*$"},
        "action": {"enum": ["sync", "validate-only"], "default": "sync"}
      }
    },
    "migration-declaration": {
      "type": "object",
      "required": ["type", "file"],
      "additionalProperties": false,
      "properties": {
        "type": {"const": "migration"},
        "file": {"type": "string", "pattern": "^migrations/.*\\.sql$"}
      }
    },
    "dimensions-declaration": {
      "type": "object",
      "required": ["type"],
      "additionalProperties": false,
      "properties": {
        "type": {"const": "dimensions"},
        "action": {"enum": ["sync"], "default": "sync"}
      }
    },
    "dictionary-declaration": {
      "type": "object",
      "required": ["type"],
      "additionalProperties": false,
      "properties": {
        "type": {"const": "dictionary"},
        "action": {"enum": ["sync"], "default": "sync"}
      }
    },
    "container-declaration": {
      "type": "object",
      "required": ["type", "target", "action"],
      "additionalProperties": false,
      "properties": {
        "type": {"const": "container"},
        "target": {"enum": ["air-quality-app", "ndp-mcp-server", "silver-etl", "grafana"]},
        "action": {"enum": ["build", "restart"]},
        "no_cache": {"type": "boolean", "default": false}
      }
    }
  }
}
```

### 5.5 Device State Files

| File | Content | Format | Example |
|------|---------|--------|---------|
| `/var/ndp/deployed-version` | Git commit SHA | Plain text | `abc123def456...` |
| `/var/ndp/deployed-at` | Deployment timestamp | ISO 8601 | `2026-02-02T14:30:00Z` |
| `/var/ndp/manifest-applied` | Manifest hash | SHA256 | `a1b2c3d4e5f6...` |

---

## 6. Interface Specification

### 6.1 deploy.sh Commands

```bash
# Apply manifest (primary command)
./deploy.sh apply

# Validate manifest only (no mutations)
./deploy.sh apply --dry-run

# Validate specific stream
./deploy.sh validate <stream-id>

# Generate DDL only (no execution)
./deploy.sh ddl <stream-id>

# Show device state
./deploy.sh state
```

### 6.2 Environment Variables

| Variable | Purpose | Default | Example |
|----------|---------|---------|---------|
| `DEPLOY_ENV` | Target environment | `pi` | `integration` |
| `NDP_MANIFEST` | Manifest file path | `.deploy/manifest.json` | `/path/to/manifest.json` |
| `NDP_CONFIG_DIR` | Stream config directory | `config/base/streams` | `/etc/ndp/streams` |
| `NDP_TIMESCALE_URL` | TimescaleDB connection | (from env) | `postgresql://user:pass@host:5432/ndp` |

### 6.3 DDL Output Format

Generated DDL follows this structure:

```sql
-- =============================================================================
-- NDP Silver Table DDL
-- Generated: 2026-02-02T14:30:00Z
-- Stream: air-quality
-- Target: silver.air_quality_readings
-- =============================================================================

-- 1. CREATE TABLE
CREATE TABLE IF NOT EXISTS silver.air_quality_readings (
    timestamp TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,
    pm25 DOUBLE PRECISION,
    temperature DOUBLE PRECISION,
    humidity DOUBLE PRECISION,
    dq_flags TEXT[],
    _bronze_id UUID,
    _ingested_at TIMESTAMPTZ DEFAULT NOW()
);

-- 2. INDEXES
CREATE INDEX IF NOT EXISTS idx_air_quality_readings_time_id
    ON silver.air_quality_readings (timestamp, ndp_id);
CREATE INDEX IF NOT EXISTS idx_air_quality_readings_dq_flags
    ON silver.air_quality_readings USING GIN (dq_flags);

-- 3. HYPERTABLE
SELECT create_hypertable('silver.air_quality_readings', 'timestamp',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE);

-- 4. POLICIES
SELECT add_compression_policy('silver.air_quality_readings',
    INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_retention_policy('silver.air_quality_readings',
    INTERVAL '90 days', if_not_exists => TRUE);

-- 5. PERMISSIONS
GRANT SELECT, INSERT ON silver.air_quality_readings TO ndp_app;
GRANT SELECT ON silver.air_quality_readings TO grafana_reader;

-- 6. ADD COLUMN (if table exists with missing columns)
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

### 6.4 Interface Contracts

#### Container Targets Enum

Valid container targets for the `container` declaration type:

| Target | Service | Description |
|--------|---------|-------------|
| `air-quality-app` | Primary application | Rust-based air quality data ingestion |
| `ndp-mcp-server` | MCP server | Model Context Protocol server for AI agents |
| `silver-etl` | ETL service | Bronze to Silver data transformation |
| `grafana` | Visualization | Grafana dashboard service |

#### Container Actions Enum

| Action | Docker Command | Phase | Description |
|--------|----------------|-------|-------------|
| `build` | `docker compose build [--no-cache] <target>` | Early (after validation) | Rebuild container image |
| `restart` | `docker compose up -d <target>` | Late (after config sync) | Recreate container with new config |

#### Container Declaration Examples

```json
// Build action (early phase)
{
  "type": "container",
  "target": "air-quality-app",
  "action": "build",
  "no_cache": false
}

// Build with cache disabled
{
  "type": "container",
  "target": "silver-etl",
  "action": "build",
  "no_cache": true
}

// Restart action (late phase)
{
  "type": "container",
  "target": "air-quality-app",
  "action": "restart"
}
```

### 6.5 Error Output Format

All errors follow structured format for consistency with dp-019 validator:

```json
{
  "success": false,
  "phase": "validate",
  "declaration": {
    "type": "stream",
    "id": "air-quality"
  },
  "errors": [
    {
      "code": "VALIDATION_FAILED",
      "path": "$.silver_etl.field_mappings[2].source_path",
      "message": "source_path 'raw_payload.typo' not found in fields",
      "severity": "error",
      "suggestion": "Did you mean 'raw_payload.temperature'?"
    }
  ]
}
```

---

## 7. Extensibility Architecture

### 7.1 Declaration Handler Interface

The system uses a handler pattern for extensibility. Each declaration type is processed by a handler implementing this interface:

```rust
/// Handler for a specific declaration type
pub trait DeclarationHandler: Send + Sync {
    /// Unique identifier for this declaration type (e.g., "stream", "silver-table")
    fn declaration_type(&self) -> &'static str;

    /// Validate the declaration (fail-fast phase)
    async fn validate(&self, declaration: &Declaration, ctx: &DeployContext) -> Result<(), DeployError>;

    /// Execute the declaration (mutation phase)
    async fn execute(&self, declaration: &Declaration, ctx: &DeployContext) -> Result<ExecutionResult, DeployError>;

    /// Priority for execution ordering (lower = earlier)
    fn priority(&self) -> u32;
}

/// Registry of all declaration handlers
pub struct HandlerRegistry {
    handlers: HashMap<String, Box<dyn DeclarationHandler>>,
}

impl HandlerRegistry {
    pub fn register(&mut self, handler: Box<dyn DeclarationHandler>) {
        self.handlers.insert(handler.declaration_type().to_string(), handler);
    }

    pub fn get(&self, declaration_type: &str) -> Option<&dyn DeclarationHandler> {
        self.handlers.get(declaration_type).map(|h| h.as_ref())
    }
}
```

### 7.2 Adding a New Declaration Type

To add a new declaration type (e.g., `alerts`):

1. **Define schema** - Add to `manifest.schema.json`:
   ```json
   "alerts-declaration": {
     "type": "object",
     "required": ["type"],
     "properties": {
       "type": {"const": "alerts"},
       "action": {"enum": ["sync"]}
     }
   }
   ```

2. **Implement handler** - Create `AlertsHandler`:
   ```rust
   pub struct AlertsHandler;

   impl DeclarationHandler for AlertsHandler {
       fn declaration_type(&self) -> &'static str { "alerts" }
       fn priority(&self) -> u32 { 60 } // After dictionary

       async fn validate(&self, decl: &Declaration, ctx: &DeployContext) -> Result<()> {
           // Validate alert configs exist
       }

       async fn execute(&self, decl: &Declaration, ctx: &DeployContext) -> Result<ExecutionResult> {
           // Sync alerts to TimescaleDB
       }
   }
   ```

3. **Register handler** - In deploy initialization:
   ```rust
   registry.register(Box::new(AlertsHandler));
   ```

### 7.3 Schema Versioning

The manifest schema supports forward evolution:

| Version | Changes |
|---------|---------|
| `1.0` | Initial: stream, silver-table, migration, dimensions, dictionary |
| `1.1` | (Future) Add: alerts, continuous-aggregates |
| `2.0` | (Future) Breaking: Restructure declaration format |

Version upgrade path:
- `1.x` versions are backward compatible (additive only)
- `2.0` requires migration tool (like config schema v1.1 -> v2.0)

---

## 8. Error Scenarios and Handling

### 8.1 Validation Phase Errors

| Error | Cause | Handling | User Message |
|-------|-------|----------|--------------|
| Manifest syntax error | Invalid JSON | Abort | `Manifest parse error at line X: {details}` |
| Unknown declaration type | Typo or unsupported type | Abort | `Unknown declaration type: '{type}'. Supported: stream, silver-table, ...` |
| Missing required field | Incomplete declaration | Abort | `Declaration {type} missing required field: {field}` |
| Stream validation failed | dp-019 validator error | Abort | `Stream {id} validation failed: {validator_errors}` |
| Migration file not found | File path incorrect | Abort | `Migration file not found: {path}` |

### 8.2 Execution Phase Errors

| Error | Cause | Handling | User Message |
|-------|-------|----------|--------------|
| etcd connection failed | etcd not running | Abort | `Cannot connect to etcd: {details}. Is etcd running?` |
| TimescaleDB connection failed | DB not running | Abort | `Cannot connect to TimescaleDB: {details}` |
| DDL execution failed | SQL error | Abort, rollback | `DDL execution failed for {table}: {sql_error}` |
| Migration failed | SQL error in migration | Abort, rollback | `Migration {file} failed at line {line}: {error}` |
| Permission denied | File/directory access | Abort | `Permission denied writing {path}` |

### 8.3 Partial Failure Handling

- **Validation phase**: All errors collected, reported together, no mutations made
- **Execution phase**: Each declaration type is atomic; failure in one does not roll back others
- **State tracking**: Device state files only updated on complete success

---

## 9. Testing Strategy

### 9.1 Unit Tests

| Component | Test Focus |
|-----------|------------|
| Manifest parser | Parse valid/invalid JSON, schema validation |
| DDL generator | Type mapping, column generation, idempotency |
| Stream handler | Validation calls, etcd sync |
| Migration handler | File reading, hash tracking, skip logic |

### 9.2 Integration Tests

| Test ID | Scenario | Verification |
|---------|----------|--------------|
| INT-001 | New stream CREATE TABLE | `\d silver.{table}` shows all columns |
| INT-002 | Existing stream ADD COLUMN | New column appears, existing data intact |
| INT-003 | Idempotent re-run | No errors on second `deploy.sh apply` |
| INT-004 | Type mapping correctness | Column types match config |
| INT-005 | Index creation | `\di silver.*` shows indexes |
| INT-006 | Hypertable conversion | `timescaledb_information.hypertables` has table |
| INT-007 | Compression policy | `timescaledb_information.jobs` has compression job |
| INT-008 | Retention policy | `timescaledb_information.jobs` has retention job |
| INT-009 | Permissions | `ndp_app` can SELECT and INSERT |
| INT-010 | Device state | `/var/ndp/deployed-version` matches git HEAD |

### 9.3 Test Infrastructure

```bash
# Start integration environment
./scripts/integration-test.sh start

# Run dp-020 specific tests
DEPLOY_ENV=integration ./deploy.sh apply

# Verify results
docker exec integration-timescaledb psql -U postgres -d ndp -c "\d silver.air_quality_readings"

# Clean up
./scripts/integration-test.sh clean
```

---

## 10. Validation Checklist

Before completing dp-020:

**Manifest and Schema**:
- [ ] manifest.schema.json created with all declaration types
- [ ] Schema uses additionalProperties: false at all levels
- [ ] Schema validates against test manifests
- [ ] Manifest parser produces typed structs

**Declaration Handlers**:
- [ ] stream handler validates via dp-019 validator
- [ ] stream handler syncs to etcd
- [ ] silver-table handler generates correct DDL
- [ ] silver-table handler handles ADD COLUMN
- [ ] migration handler tracks applied migrations
- [ ] dimensions handler syncs CSV data
- [ ] dictionary handler syncs metadata

**DDL Generation**:
- [ ] Type mapping covers all supported types
- [ ] Standard columns always included
- [ ] Indexes created correctly
- [ ] Hypertable created with correct settings
- [ ] Policies created with if_not_exists
- [ ] Permissions granted to correct roles

**Orchestration**:
- [ ] deploy.sh apply executes in correct order
- [ ] Validation phase is fail-fast
- [ ] Device state files updated on success
- [ ] Device state files NOT updated on failure

**Testing**:
- [ ] All INT-xxx tests pass in integration environment
- [ ] Idempotent execution verified
- [ ] Error messages are actionable

**Documentation**:
- [ ] DEPLOYMENT-DECLARATIVES.md created in docs/procedures/
- [ ] AgentDB pattern stored via save-pattern skill
- [ ] deploy.sh help updated with new commands

---

## 11. Dependencies and Prerequisites

| Dependency | Type | Status | Notes |
|------------|------|--------|-------|
| dp-018: JSON Config Foundation | REQUIRED | Must complete first | JSON configs, ConfigLoader |
| dp-019: Config Validation Pipeline | REQUIRED | Must complete first | ndp-validate binary |
| dp-017: Integration Environment | REQUIRED | Must be available | docker-compose.integration.yml |
| TimescaleDB | Runtime | Available | Silver layer database |
| etcd | Runtime | Available | Stream config storage |

---

## 12. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| DDL generation produces incorrect SQL | Medium | High | Extensive type mapping tests; --dry-run mode |
| Idempotency breaks on edge cases | Medium | Medium | Comprehensive IF NOT EXISTS tests |
| Handler interface limits future types | Low | Medium | Design review; prototype 2+ handlers first |
| Performance slow for many streams | Low | Low | Benchmark early; parallelize if needed |
| Device state corruption on crash | Low | Medium | Atomic write (temp file + rename) |

---

## 13. Success Metrics

| Metric | Current State | After dp-020 | Measurement |
|--------|---------------|--------------|-------------|
| Manual deploy steps | 8+ | 1 | Count commands to deploy change |
| Manual DDL writing | Required | None | DDL generated from config |
| Schema evolution support | None | ADD COLUMN | New columns added automatically |
| Deploy validation | None | Fail-fast | Bad config blocked before mutations |
| Device state tracking | None | Git SHA + timestamp | /var/ndp/ files exist |
| Declaration extensibility | N/A | Handler interface | New type added in <100 LOC |

---

## 14. Data Flow Diagrams

### 14.1 Overall Deploy Flow

```
.deploy/manifest.json
        │
        ▼
┌───────────────────┐
│  Manifest Parser  │
│  (JSON Schema)    │
└─────────┬─────────┘
          │ Typed Manifest
          ▼
┌───────────────────┐
│  Handler Registry │
│  (route by type)  │
└─────────┬─────────┘
          │
    ┌─────┴─────┬─────────┬──────────┬───────────┐
    ▼           ▼         ▼          ▼           ▼
┌───────┐  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐
│stream │  │silver- │ │migra-  │ │dimen-  │ │diction-│
│handler│  │table   │ │tion    │ │sions   │ │ary     │
└───┬───┘  │handler │ │handler │ │handler │ │handler │
    │      └────┬───┘ └────┬───┘ └────┬───┘ └────┬───┘
    │           │          │          │          │
    ▼           ▼          ▼          ▼          ▼
  etcd     TimescaleDB TimescaleDB TimescaleDB TimescaleDB
```

### 14.2 DDL Generation Flow

```
Stream Config (JSON)
├── silver_etl.target_table: "silver.air_quality_readings"
├── silver_etl.field_mappings[]
│   ├── {target_column: "pm25", target_type: "float"}
│   └── {target_column: "temp", target_type: "float"}
└── silver_etl.dq_rules[]
        │
        ▼
┌───────────────────┐
│   DDL Generator   │
│   (type_mapping)  │
└─────────┬─────────┘
          │
          ▼
┌───────────────────────────────────────────┐
│ CREATE TABLE IF NOT EXISTS                │
│   silver.air_quality_readings (           │
│     timestamp TIMESTAMPTZ NOT NULL,       │
│     ndp_id TEXT NOT NULL,                 │
│     pm25 DOUBLE PRECISION,                │ ◀── float → DOUBLE PRECISION
│     temp DOUBLE PRECISION,                │
│     dq_flags TEXT[],                      │
│     _bronze_id UUID,                      │
│     _ingested_at TIMESTAMPTZ DEFAULT NOW()│
│   );                                      │
│ CREATE INDEX IF NOT EXISTS ...            │
│ SELECT create_hypertable(...);            │
│ SELECT add_compression_policy(...);       │
│ GRANT SELECT, INSERT ...                  │
└───────────────────────────────────────────┘
          │
          ▼
     TimescaleDB
```

### 14.3 ADD COLUMN Flow

```
Existing Table Schema          New Config field_mappings
┌─────────────────────┐        ┌─────────────────────┐
│ pm25                │        │ pm25                │
│ temperature         │        │ temperature         │
│                     │        │ humidity ◀── NEW    │
└─────────────────────┘        └─────────────────────┘
          │                              │
          └──────────┬───────────────────┘
                     ▼
            ┌────────────────┐
            │ Column Differ  │
            │ (set diff)     │
            └────────┬───────┘
                     │ missing: [humidity]
                     ▼
            ┌────────────────────────────┐
            │ DO $$                      │
            │ BEGIN                      │
            │   IF NOT EXISTS (...) THEN │
            │     ALTER TABLE ... ADD    │
            │       COLUMN humidity ...  │
            │   END IF;                  │
            │ END $$;                    │
            └────────────────────────────┘
```

---

## 15. Glossary

| Term | Definition |
|------|------------|
| **Declaration** | A single entry in the manifest describing a change to deploy |
| **Declaration Type** | Category of declaration: stream, silver-table, migration, dimensions, dictionary |
| **Handler** | Component that processes a specific declaration type |
| **Manifest** | JSON file (`.deploy/manifest.json`) declaring all changes for a deployment |
| **Device State** | Files in `/var/ndp/` tracking what version is deployed |
| **Idempotent** | Safe to run multiple times with same result |
| **DDL** | Data Definition Language (CREATE TABLE, ALTER TABLE, etc.) |
| **Hypertable** | TimescaleDB table optimized for time-series data |

---

## 16. References

| Document | Path | Relevance |
|----------|------|-----------|
| dp-020 SCOPE.md | `product/features/dp-020/SCOPE.md` | Feature scope definition |
| dp-016 IMPLEMENTATION-ROADMAP.md | `product/features/dp-016/IMPLEMENTATION-ROADMAP.md` | Phase 3 details |
| ADR-016-002 Declarative Deploy | `product/features/dp-016/architecture/ADR-016-002-declarative-deploy.md` | Architecture decision |
| dp-019 SPECIFICATION.md | `product/features/dp-019/specification/SPECIFICATION.md` | Validation requirements |
| dp-019 SUPPORTED-VALUES-RESEARCH.md | `product/features/dp-019/specification/SUPPORTED-VALUES-RESEARCH.md` | Type mapping source |
| dp-019 SILVER-VALIDATION-RESEARCH.md | `product/features/dp-019/specification/SILVER-VALIDATION-RESEARCH.md` | Schema validation patterns |
| deploy.sh | `deploy/pi/deploy.sh` | Current deployment script |

---

*Specification created: 2026-02-02*
*SPARC Phase: Specification (S)*
*Next Phase: Pseudocode (P)*
