# Declarative Deployment Guide (dp-020)

This document describes the declarative deployment system for the Neural Data Platform. Instead of running manual commands, you declare **what changed** in a manifest file and run `./deploy.sh apply` to orchestrate the deployment.

## Table of Contents

- [Quick Start](#quick-start)
- [Manifest Structure](#manifest-structure)
- [Declaration Types](#declaration-types)
  - [stream](#stream)
  - [silver-table](#silver-table)
  - [tool](#tool)
  - [migration](#migration)
  - [gold-tables](#gold-tables)
  - [dimensions](#dimensions)
  - [dictionary](#dictionary)
  - [container](#container)
- [Execution Order](#execution-order)
- [Release Workflow](#release-workflow)
- [Examples](#examples)
- [Troubleshooting](#troubleshooting)

---

## Quick Start

```bash
# 1. Create a release manifest
cat > .deploy/releases/v1.0.0.manifest.json << 'EOF'
{
  "version": "1.0",
  "description": "Initial release: air-quality stream",
  "changes": [
    {"type": "stream", "id": "air-quality", "action": "create"},
    {"type": "silver-table", "stream_id": "air-quality", "action": "sync"},
    {"type": "dictionary", "action": "sync"}
  ]
}
EOF

# 2. Deploy
./deploy.sh apply .deploy/releases/v1.0.0.manifest.json
```

---

## Manifest Structure

```json
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.0",
  "description": "Human-readable description of this release",
  "changes": [
    { "type": "...", ... },
    { "type": "...", ... }
  ]
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `$schema` | string | No | JSON Schema reference for validation |
| `version` | string | **Yes** | Must be `"1.0"` |
| `description` | string | No | Human-readable release description |
| `changes` | array | **Yes** | Array of declaration objects |

---

## Declaration Types

### stream

Syncs a stream configuration to etcd.

```json
{
  "type": "stream",
  "id": "air-quality",
  "action": "update",
  "reload": "sources"
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `type` | string | **Yes** | - | Must be `"stream"` |
| `id` | string | **Yes** | - | Stream identifier (kebab-case, e.g., `air-quality`) |
| `action` | enum | No | `"update"` | `"create"`, `"update"`, or `"validate-only"` |
| `reload` | enum | No | `"none"` | `"sources"` (hot-reload), `"full"` (restart), `"none"` |

**Actions:**
- `create` - Fail if stream already exists in etcd
- `update` - Upsert (create or update)
- `validate-only` - Validate config without syncing

**Reload:**
- `sources` - Hot-reload source configurations (no restart)
- `full` - Full application restart
- `none` - No reload after sync

**Config file location:** `config/base/streams/{id}/config.json`

---

### silver-table

Generates and applies Silver layer DDL from stream configuration.

```json
{
  "type": "silver-table",
  "stream_id": "air-quality",
  "action": "sync"
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `type` | string | **Yes** | - | Must be `"silver-table"` |
| `stream_id` | string | **Yes** | - | Stream ID with `silver_etl` configuration |
| `action` | enum | No | `"sync"` | `"sync"` or `"validate-only"` |

**Actions:**
- `sync` - Generate DDL and apply to TimescaleDB
- `validate-only` - Generate DDL without applying

**Generated DDL includes:**
- `CREATE TABLE IF NOT EXISTS` with columns from `field_mappings`
- Indexes (composite, GIN for dq_flags, ingestion time)
- TimescaleDB hypertable conversion
- Compression policy (from `compression_after_days`)
- Retention policy (from `retention_days`)
- Permissions for `ndp_app` and `grafana_reader` roles

**Type mapping:**

| Config Type | PostgreSQL Type |
|-------------|-----------------|
| `float`, `double_precision` | `DOUBLE PRECISION` |
| `real` | `REAL` |
| `integer`, `int` | `INTEGER` |
| `smallint` | `SMALLINT` |
| `bigint` | `BIGINT` |
| `text`, `string` | `TEXT` |
| `varchar` | `VARCHAR` |
| `boolean`, `bool` | `BOOLEAN` |
| `timestamp`, `timestamptz` | `TIMESTAMPTZ` |
| `json`, `jsonb` | `JSONB` |
| `text[]` | `TEXT[]` |

**Prerequisite:** Stream config must have `silver_etl.enabled: true` and `silver_etl.target_table` defined.

---

### tool

Builds a Rust CLI tool required for deployment (fe-001 Phase B).

```json
{
  "type": "tool",
  "id": "ndp-gold-ddl",
  "action": "build",
  "profile": "release"
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `type` | string | **Yes** | - | Must be `"tool"` |
| `id` | string | **Yes** | - | Tool identifier (see supported tools) |
| `action` | enum | No | `"build"` | Only `"build"` is supported |
| `profile` | enum | No | `"release"` | Build profile: `"release"` or `"debug"` |

**Supported Tools:**

| Tool ID | Description | Source |
|---------|-------------|--------|
| `ndp-gold-ddl` | Gold layer DDL generator | `tools/ndp-gold-ddl/` |
| `ndp-validate` | Configuration validator | `tools/ndp-validate/` |

**Notes:**
- Tool builds run in Phase 2.5, after Container Builds but before Migrations
- Ensures tools are available before Gold Tables phase (which depends on ndp-gold-ddl)
- Uses `cargo build` with the specified profile

---

### migration

Executes a SQL migration file.

```json
{
  "type": "migration",
  "file": "deploy/pi/init-scripts/004_stream_classification.sql"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | string | **Yes** | Must be `"migration"` |
| `file` | string | **Yes** | Path to SQL file (relative to repository root) |

**Notes:**
- Migrations run in declaration order
- File path is relative to repository root
- Common locations: `migrations/`, `deploy/pi/init-scripts/`

---

### gold-tables

Generates and applies Gold layer DDL from stream configuration (fe-001).

```json
{
  "type": "gold-tables",
  "stream_id": "air-quality",
  "action": "sync"
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `type` | string | **Yes** | - | Must be `"gold-tables"` |
| `stream_id` | string | **Yes** | - | Stream ID with `gold_etl` configuration |
| `action` | enum | No | `"sync"` | `"sync"` or `"recreate"` |

**Actions:**
- `sync` - Create if not exists (idempotent)
- `recreate` - Drop and recreate

**Generated DDL includes:**
- `CREATE MATERIALIZED VIEW` with TimescaleDB continuous aggregate
- Time bucket expressions for each granularity
- Aggregate functions (mean, std, min, max, p95, etc.)
- Refresh policies

**Prerequisites:**
- `ndp-gold-ddl` tool must be built (use `tool` declaration)
- Stream config must have `gold_etl.enabled: true`

---

### dimensions

Syncs dimension CSV files to TimescaleDB.

```json
{
  "type": "dimensions",
  "action": "sync"
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `type` | string | **Yes** | - | Must be `"dimensions"` |
| `action` | enum | No | `"sync"` | Only `"sync"` is supported |

**Dimension files location:** `config/base/dimensions/`

---

### dictionary

Syncs data dictionary metadata from stream configurations.

```json
{
  "type": "dictionary",
  "action": "sync"
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `type` | string | **Yes** | - | Must be `"dictionary"` |
| `action` | enum | No | `"sync"` | Only `"sync"` is supported |

**Syncs to tables:**
- `data_dictionary.streams`
- `data_dictionary.fields`
- `data_dictionary.entity_schemas`
- `data_dictionary.entity_schema_attributes`

---

### container

Builds or restarts Docker containers.

```json
{
  "type": "container",
  "target": "air-quality-app",
  "action": "build",
  "no_cache": false
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `type` | string | **Yes** | - | Must be `"container"` |
| `target` | enum | **Yes** | - | Container to operate on |
| `action` | enum | **Yes** | - | `"build"` or `"restart"` |
| `no_cache` | boolean | No | `false` | Force rebuild without Docker cache (build only) |

**Valid targets:**

| Target | Service | Description |
|--------|---------|-------------|
| `air-quality-app` | Air Quality App | Main data ingestion application |
| `ndp-mcp-server` | MCP Server | AI agent integration server |
| `silver-etl` | Silver ETL | Bronze → Silver ETL processor |
| `grafana` | Grafana | Dashboard and visualization |

**Notes:**
- Container builds run in **Phase 2** (early, so new code is available)
- Container restarts run in **Phase 8** (late, after config changes)
- Build targets use the appropriate Docker Compose profile

---

## Execution Order

The `apply` command executes declarations in a fixed 12-phase order to ensure dependencies are met:

| Phase | Declarations | Description |
|-------|--------------|-------------|
| 1 | - | **Validation**: Validate manifest and check infrastructure |
| 2 | `container` (build) | **Container Builds**: Build new images |
| 2.5 | `tool` | **Tool Builds**: Build Rust CLI tools (fe-001) |
| 3 | `migration` | **Migrations**: Run SQL migrations |
| 4 | `silver-table` | **Silver Tables**: Generate and apply DDL |
| 5 | `gold-tables` | **Gold Tables**: Generate Gold layer continuous aggregates (fe-001) |
| 6 | `domains` | **Domains**: Generate cross-stream aligned views (fe-001) |
| 7 | `stream` | **Streams**: Sync configs to etcd |
| 8 | `dimensions` | **Dimensions**: Sync dimension CSVs |
| 9 | `dictionary` | **Dictionary**: Sync data dictionary |
| 10 | `container` (restart) | **Container Restarts**: Restart services |
| 11 | - | **Device State**: Update deployment tracking files |

**Rationale:**
- Builds happen early so new code is available for migrations
- Tool builds (2.5) happen after container builds but before migrations, ensuring Rust CLI tools are ready
- Migrations run before Silver/Gold tables (schema dependencies)
- Gold tables depend on tool builds completing first (ndp-gold-ddl)
- Streams sync before dictionary (dictionary reads stream configs)
- Restarts happen last so containers pick up all changes

---

## Release Workflow

### Manifest Naming Convention

```
.deploy/releases/v{MAJOR}.{MINOR}.{PATCH}.manifest.json
```

Examples:
- `v1.0.0.manifest.json` - Initial release
- `v1.1.0.manifest.json` - New feature (added stream)
- `v1.1.1.manifest.json` - Bug fix

### Standard Release Process

```bash
# 1. Create or modify stream configuration
vim config/base/streams/new-sensor/config.json

# 2. Create release manifest
cat > .deploy/releases/v1.2.0.manifest.json << 'EOF'
{
  "version": "1.0",
  "description": "Release v1.2.0: Add new-sensor stream with Silver table",
  "changes": [
    {"type": "stream", "id": "new-sensor", "action": "create"},
    {"type": "silver-table", "stream_id": "new-sensor", "action": "sync"},
    {"type": "dictionary", "action": "sync"}
  ]
}
EOF

# 3. Validate manifest
cat .deploy/releases/v1.2.0.manifest.json | jq .

# 4. Commit and tag
git add config/base/streams/new-sensor/ .deploy/releases/v1.2.0.manifest.json
git commit -m "feat: Add new-sensor stream (v1.2.0)"
git tag -a v1.2.0 -m "Release v1.2.0: Add new-sensor stream"
git push && git push --tags

# 5. Deploy on production (Pi)
ssh pi@your-pi
cd /path/to/neural-data-platform
git pull
./deploy.sh apply .deploy/releases/v1.2.0.manifest.json
```

### Directory Structure

```
.deploy/
├── manifest.json                    # Working manifest (optional)
└── releases/
    ├── v1.0.0.manifest.json         # Initial release
    ├── v1.1.0.manifest.json         # Feature release
    ├── v1.2.0.manifest.json         # Feature release
    └── v1.2.1.manifest.json         # Patch release
```

---

## Examples

### Example 1: Add New Stream with Silver Table

```json
{
  "version": "1.0",
  "description": "Add weather-station stream",
  "changes": [
    {
      "type": "stream",
      "id": "weather-station",
      "action": "create"
    },
    {
      "type": "silver-table",
      "stream_id": "weather-station",
      "action": "sync"
    },
    {
      "type": "dictionary",
      "action": "sync"
    }
  ]
}
```

### Example 2: Run Migration and Rebuild App

```json
{
  "version": "1.0",
  "description": "Add forecast accuracy table with app update",
  "changes": [
    {
      "type": "container",
      "target": "air-quality-app",
      "action": "build"
    },
    {
      "type": "migration",
      "file": "migrations/003-forecast-accuracy.sql"
    },
    {
      "type": "container",
      "target": "air-quality-app",
      "action": "restart"
    }
  ]
}
```

### Example 3: Full Stack Update

```json
{
  "version": "1.0",
  "description": "Major release: New streams, migrations, and app rebuild",
  "changes": [
    {
      "type": "container",
      "target": "air-quality-app",
      "action": "build",
      "no_cache": true
    },
    {
      "type": "migration",
      "file": "migrations/004-add-alerts-table.sql"
    },
    {
      "type": "stream",
      "id": "indoor-air-quality",
      "action": "create"
    },
    {
      "type": "stream",
      "id": "outdoor-air-quality",
      "action": "update"
    },
    {
      "type": "silver-table",
      "stream_id": "indoor-air-quality",
      "action": "sync"
    },
    {
      "type": "dimensions",
      "action": "sync"
    },
    {
      "type": "dictionary",
      "action": "sync"
    },
    {
      "type": "container",
      "target": "air-quality-app",
      "action": "restart"
    }
  ]
}
```

### Example 4: Validate Only (Dry Run)

```json
{
  "version": "1.0",
  "description": "Validation test - no changes applied",
  "changes": [
    {
      "type": "stream",
      "id": "test-stream",
      "action": "validate-only"
    },
    {
      "type": "silver-table",
      "stream_id": "test-stream",
      "action": "validate-only"
    }
  ]
}
```

---

## Troubleshooting

### Manifest Validation Errors

```
ERROR: Invalid or missing manifest version (expected: 1.0, got: null)
```
**Fix:** Ensure `"version": "1.0"` is in your manifest.

```
ERROR: Manifest has no changes to apply
```
**Fix:** Add at least one declaration to the `changes` array.

### Stream Errors

```
ERROR: Stream config not found: config/base/streams/my-stream/config.json
```
**Fix:** Create the stream configuration file before deploying.

### Silver Table Errors

```
SKIP: silver_etl not enabled for stream my-stream
```
**Fix:** Add `silver_etl.enabled: true` and `silver_etl.target_table` to stream config.

```
ERROR: DDL generator not loaded
```
**Fix:** Ensure `deploy/pi/ddl-generator.sh` exists and is valid bash.

### Infrastructure Errors

```
Waiting for TimescaleDB to be ready...
```
**Fix:** Start services first: `./deploy.sh start`

### Permission Errors

```
NOTICE: Role ndp_app does not exist, skipping permissions
```
**Info:** This is expected in development. Roles are optional for DDL to succeed.

---

## Schema Reference

The manifest schema is defined in `schemas/manifest.schema.json`. Validate your manifest:

```bash
# Using jq for syntax check
cat .deploy/releases/v1.0.0.manifest.json | jq .

# Using ajv for full schema validation (if installed)
npx ajv validate -s schemas/manifest.schema.json -d .deploy/releases/v1.0.0.manifest.json
```

---

## See Also

- [deploy/pi/README.md](../../deploy/pi/README.md) - Deployment commands
- [schemas/manifest.schema.json](../../schemas/manifest.schema.json) - JSON Schema
- [deploy/pi/ddl-generator.sh](../../deploy/pi/ddl-generator.sh) - DDL generation functions
- [product/features/dp-020/](../../product/features/dp-020/) - Feature documentation
