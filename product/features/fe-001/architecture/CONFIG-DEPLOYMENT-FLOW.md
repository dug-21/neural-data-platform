# Config-to-Production Deployment Flow

> **Created:** 2026-02-04
> **Purpose:** Document every component that touches configuration during declarative deployment
> **Scope:** From files on Pi through `deploy.sh apply` to operational database
> **Use Case:** Guide Gold layer implementation - identifies every touchpoint requiring modification

---

## Executive Summary

This document traces configuration from file to production, identifying every component and transformation. For Gold layer (V1.1), virtually every step needs minor modifications. Understanding this flow prevents missed integration points.

**Key Insight**: There are **12 distinct components** that touch configuration. Gold layer must integrate with all of them.

---

## The Complete Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                    DECLARATIVE DEPLOYMENT FLOW                                   │
│                    deploy.sh apply <manifest>                                    │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                  │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │ PHASE 0: MANIFEST PARSING                                                │    │
│  │ Component: deploy.sh                                                     │    │
│  │ Input: .deploy/releases/vX.Y.Z.manifest.json                            │    │
│  │ Action: Parse JSON, extract declaration arrays by type                   │    │
│  │ Output: Typed arrays (etcd_configs, silver_tables, streams, etc.)       │    │
│  └────────────────────────────────────────┬────────────────────────────────┘    │
│                                           │                                      │
│                                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │ PHASE 1: ETCD CONFIG SYNC                                                │    │
│  │ Component: deploy.sh → handle_etcd_config()                              │    │
│  │ Input: Declaration { stream_id, path }                                   │    │
│  │ Action: Read JSON file, PUT to etcd                                      │    │
│  │ Output: /streams/{stream_id}/config in etcd                             │    │
│  │                                                                          │    │
│  │ ┌─────────────────────────────────────────────────────────────────────┐ │    │
│  │ │ SUB-COMPONENT: etcdctl (via docker exec)                            │ │    │
│  │ │ Action: etcdctl put /streams/{id}/config < config.json              │ │    │
│  │ └─────────────────────────────────────────────────────────────────────┘ │    │
│  └────────────────────────────────────────┬────────────────────────────────┘    │
│                                           │                                      │
│                                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │ PHASE 2: DIMENSION TABLES                                                │    │
│  │ Component: deploy.sh → handle_dimension()                                │    │
│  │ Input: Declaration { dimension_id, csv_path }                           │    │
│  │ Action: Generate DDL, load CSV data                                      │    │
│  │ Output: dimension.{table} in TimescaleDB                                │    │
│  └────────────────────────────────────────┬────────────────────────────────┘    │
│                                           │                                      │
│                                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │ PHASE 3: DATA DICTIONARY SYNC                                            │    │
│  │ Component: deploy.sh → sync_to_data_dictionary()                         │    │
│  │ Input: All stream configs from config/base/streams/*/                   │    │
│  │ Action: Parse YAML/JSON, INSERT/UPDATE metadata tables                   │    │
│  │ Output: data_dictionary.streams, .fields, .sources populated            │    │
│  └────────────────────────────────────────┬────────────────────────────────┘    │
│                                           │                                      │
│                                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │ PHASE 4: SILVER TABLES                                                   │    │
│  │ Component: deploy.sh → handle_silver_table()                             │    │
│  │ Input: Declaration { stream_id, action: "sync" }                        │    │
│  │                                                                          │    │
│  │ ┌─────────────────────────────────────────────────────────────────────┐ │    │
│  │ │ SUB-COMPONENT: ddl-generator.sh → generate_silver_ddl()             │ │    │
│  │ │ Input: config/base/streams/{id}/config.json                         │ │    │
│  │ │ Action: Read silver_etl section, generate SQL strings               │ │    │
│  │ │ Output: CREATE TABLE, indexes, hypertable, policies DDL             │ │    │
│  │ └─────────────────────────────────────────────────────────────────────┘ │    │
│  │                                                                          │    │
│  │ ┌─────────────────────────────────────────────────────────────────────┐ │    │
│  │ │ SUB-COMPONENT: psql (via docker exec)                               │ │    │
│  │ │ Action: echo "$ddl" | dcx timescaledb psql -U postgres -d ndp       │ │    │
│  │ └─────────────────────────────────────────────────────────────────────┘ │    │
│  └────────────────────────────────────────┬────────────────────────────────┘    │
│                                           │                                      │
│                                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │ PHASE 5: GOLD TABLES (NEW - V1.1)                                        │    │
│  │ Component: deploy.sh → handle_gold_table()                               │    │
│  │ Input: Declaration { stream_id, action: "sync" }                        │    │
│  │                                                                          │    │
│  │ ┌─────────────────────────────────────────────────────────────────────┐ │    │
│  │ │ SUB-COMPONENT: ndp-gold-ddl (Rust CLI - NEW)                        │ │    │
│  │ │ Input: config/base/streams/{id}/config.json (gold_etl section)      │ │    │
│  │ │ Action: Read gold_etl, generate continuous aggregate SQL            │ │    │
│  │ │ Output: CREATE MATERIALIZED VIEW, policies DDL                      │ │    │
│  │ └─────────────────────────────────────────────────────────────────────┘ │    │
│  │                                                                          │    │
│  │ ┌─────────────────────────────────────────────────────────────────────┐ │    │
│  │ │ SUB-COMPONENT: psql (via docker exec)                               │ │    │
│  │ │ Action: echo "$ddl" | dcx timescaledb psql -U postgres -d ndp       │ │    │
│  │ └─────────────────────────────────────────────────────────────────────┘ │    │
│  └────────────────────────────────────────┬────────────────────────────────┘    │
│                                           │                                      │
│                                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │ PHASE 6: DOMAIN CONFIG (NEW - V1.1)                                      │    │
│  │ Component: deploy.sh → handle_domain()                                   │    │
│  │ Input: Declaration { domain_id, action: "sync" }                        │    │
│  │                                                                          │    │
│  │ ┌─────────────────────────────────────────────────────────────────────┐ │    │
│  │ │ SUB-COMPONENT: etcdctl (domain config sync)                         │ │    │
│  │ │ Action: etcdctl put /domains/{id}/config < domain.yaml              │ │    │
│  │ └─────────────────────────────────────────────────────────────────────┘ │    │
│  │                                                                          │    │
│  │ ┌─────────────────────────────────────────────────────────────────────┐ │    │
│  │ │ SUB-COMPONENT: ndp-gold-ddl --domain (Rust CLI - NEW)               │ │    │
│  │ │ Input: config/domains/{id}/domain.yaml                              │ │    │
│  │ │ Action: Generate aligned view, unified events DDL                   │ │    │
│  │ │ Output: CREATE VIEW gold.{domain}_aligned, gold.events_unified      │ │    │
│  │ └─────────────────────────────────────────────────────────────────────┘ │    │
│  └────────────────────────────────────────┬────────────────────────────────┘    │
│                                           │                                      │
│                                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │ PHASE 7: STREAMS (Application Enable)                                    │    │
│  │ Component: deploy.sh → handle_stream()                                   │    │
│  │ Input: Declaration { stream_id, action: "enable" }                      │    │
│  │ Action: Start/restart ingestion for stream                               │    │
│  │ Output: Stream actively collecting data                                  │    │
│  └────────────────────────────────────────┬────────────────────────────────┘    │
│                                           │                                      │
│                                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │ PHASE 8: GRAFANA DASHBOARDS                                              │    │
│  │ Component: deploy.sh → handle_dashboard()                                │    │
│  │ Input: Declaration { dashboard_id, path }                               │    │
│  │ Action: POST to Grafana API                                              │    │
│  │ Output: Dashboard available in Grafana                                   │    │
│  └────────────────────────────────────────┬────────────────────────────────┘    │
│                                           │                                      │
│                                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────────────────────┐    │
│  │ PHASE 9: SUMMARY                                                         │    │
│  │ Component: deploy.sh                                                     │    │
│  │ Action: Print deployment summary                                         │    │
│  └─────────────────────────────────────────────────────────────────────────┘    │
│                                                                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## Component Inventory

### 1. Manifest File

| Attribute | Value |
|-----------|-------|
| **Location** | `.deploy/releases/vX.Y.Z.manifest.json` |
| **Format** | JSON |
| **Purpose** | Declares what should exist in production |
| **Read By** | deploy.sh |

**Current Structure**:
```json
{
  "version": "1.0.0",
  "description": "Release description",
  "declarations": {
    "etcd-config": [...],
    "dimensions": [...],
    "silver-tables": [...],
    "streams": [...],
    "dashboards": [...]
  }
}
```

**Gold Layer Additions**:
```json
{
  "declarations": {
    "gold-tables": [
      { "stream_id": "air-quality", "action": "sync" }
    ],
    "domains": [
      { "domain_id": "indoor-air-quality", "action": "sync" }
    ]
  }
}
```

---

### 2. Stream Config Files

| Attribute | Value |
|-----------|-------|
| **Location** | `config/base/streams/{stream_id}/config.json` |
| **Format** | JSON |
| **Purpose** | Complete stream definition (source, schema, ETL) |
| **Read By** | sync-streams-to-etcd.sh, ddl-generator.sh, ndp-validate, ndp-gold-ddl |

**Current Sections**:
- `stream_id`, `description`, `version`, `enabled`
- `fields[]` - Schema definition
- `sources[]` - Ingestion configuration
- `silver_etl` - Silver layer transformation

**Gold Layer Addition**:
```json
{
  "gold_etl": {
    "enabled": true,
    "aggregates": {
      "granularities": ["1 hour", "1 day"],
      "fields": {
        "pm25": { "metrics": ["mean", "std", "min", "max", "p95"] }
      }
    },
    "features": {
      "lag": { "enabled": true, "lags_hours": [1, 6, 24] },
      "rolling": { "enabled": true, "windows": ["4 hours"] }
    }
  }
}
```

---

### 3. Domain Config Files (NEW)

| Attribute | Value |
|-----------|-------|
| **Location** | `config/domains/{domain_id}/domain.yaml` |
| **Format** | YAML |
| **Purpose** | Cross-stream alignment, objectives |
| **Read By** | ndp-gold-ddl, sync-domains-to-etcd.sh (new) |

**Structure**:
```yaml
domain:
  id: indoor-air-quality
  description: "Maintain healthy indoor air quality"
  streams:
    - stream_id: air-quality
      role: primary
    - stream_id: outdoor-weather
      role: context
  alignment:
    view_name: indoor_air_quality_aligned
    granularity: "1 hour"
    join_strategy: full_outer
  objectives:
    - id: healthy_co2
      target: { stream: air-quality, metric: co2, condition: "<", threshold: 800 }
```

---

### 4. JSON Schema Files

| Attribute | Value |
|-----------|-------|
| **Location** | `config/schemas/*.schema.json` |
| **Format** | JSON Schema Draft-07 |
| **Purpose** | Structural validation of config files |
| **Read By** | ndp-validate |

**Current Schemas**:
- `stream-config.schema.json` - Stream configuration
- `silver-etl.schema.json` - Silver ETL section

**Gold Layer Additions**:
- `gold-etl.schema.json` - Gold ETL section
- `domain.schema.json` - Domain configuration
- `objectives.schema.json` - Objectives definition

---

### 5. deploy.sh

| Attribute | Value |
|-----------|-------|
| **Location** | `deploy/pi/deploy.sh` |
| **Language** | Bash |
| **Purpose** | Orchestrate declarative deployment |
| **Calls** | ddl-generator.sh, etcdctl, psql, ndp-gold-ddl (new) |

**Current Functions**:
| Function | Purpose |
|----------|---------|
| `apply()` | Main entry - parse manifest, orchestrate phases |
| `handle_etcd_config()` | Sync config JSON to etcd |
| `handle_dimension()` | Load dimension tables |
| `sync_to_data_dictionary()` | Populate metadata |
| `handle_silver_table()` | Generate and apply Silver DDL |
| `handle_stream()` | Enable/disable stream ingestion |
| `handle_dashboard()` | Deploy Grafana dashboards |

**Gold Layer Additions**:
| Function | Purpose |
|----------|---------|
| `handle_gold_table()` | Call ndp-gold-ddl, apply DDL |
| `handle_domain()` | Sync domain config, generate aligned views |

---

### 6. ddl-generator.sh

| Attribute | Value |
|-----------|-------|
| **Location** | `deploy/pi/ddl-generator.sh` |
| **Language** | Bash |
| **Purpose** | Generate Silver layer DDL from config |
| **Called By** | deploy.sh |

**Key Functions**:
| Function | Output |
|----------|--------|
| `generate_silver_ddl()` | Complete Silver DDL for stream |
| `generate_create_table_ddl()` | CREATE TABLE statement |
| `generate_indexes_ddl()` | CREATE INDEX statements |
| `generate_hypertable_ddl()` | create_hypertable() call |
| `generate_policies_ddl()` | Compression/retention policies |
| `generate_permissions_ddl()` | GRANT statements |
| `map_type()` | Config type → PostgreSQL type |

**Gold Layer**: No changes. Silver DDL stays in Bash.

---

### 7. ndp-gold-ddl (NEW)

| Attribute | Value |
|-----------|-------|
| **Location** | `tools/ndp-gold-ddl/` |
| **Language** | Rust |
| **Purpose** | Generate Gold layer DDL from config |
| **Called By** | deploy.sh |

**CLI Commands**:
```bash
ndp-gold-ddl generate --stream <id> [--mode full|evolve]
ndp-gold-ddl generate --domain <id>
ndp-gold-ddl validate --stream <id>
ndp-gold-ddl validate --domain <id>
```

**Generators**:
| Generator | Output |
|-----------|--------|
| `continuous_aggregate.rs` | CREATE MATERIALIZED VIEW ... WITH (timescaledb.continuous) |
| `features.rs` | Lag, rolling, trend column expressions |
| `aligned_view.rs` | Cross-stream JOIN view |
| `events.rs` | State transitions, threshold crossings, unified events |
| `policies.rs` | add_continuous_aggregate_policy() |

---

### 8. ndp-validate

| Attribute | Value |
|-----------|-------|
| **Location** | `tools/ndp-validate/` |
| **Language** | Rust |
| **Purpose** | Two-layer config validation |
| **Called By** | CI/CD, pre-deploy validation |

**Current Validation**:
- Layer 1: JSON Schema validation
- Layer 2: Semantic validation (source_path refs, table existence)

**Gold Layer Additions**:
| Error Code | Rule |
|------------|------|
| 400 | `InvalidGoldField` - gold_etl references field not in stream |
| 401 | `InvalidStreamType` - transitions on non-state_event stream |
| 402 | `UnknownAlignmentStream` - alignment references unknown stream |
| 403 | `InvalidAggregateMetric` - unknown metric type |
| 404 | `InvalidDomainStream` - domain references non-existent stream |
| 405 | `CircularDomainDependency` - domain references itself |

**Files to Modify**:
- `src/schema.rs` - Add gold_etl to default schema
- `src/semantic/mod.rs` - Add gold validation module
- `src/semantic/gold.rs` - NEW: Gold-specific semantic rules
- `src/semantic/domain.rs` - NEW: Domain validation rules

---

### 9. sync-streams-to-etcd.sh

| Attribute | Value |
|-----------|-------|
| **Location** | `scripts/sync-streams-to-etcd.sh` |
| **Language** | Bash |
| **Purpose** | Sync stream configs from files to etcd |
| **Output** | `/streams/{stream_id}/config` keys in etcd |

**Gold Layer**: No changes needed if gold_etl is embedded in stream config. Config is synced as complete JSON blob.

---

### 10. sync-domains-to-etcd.sh (NEW)

| Attribute | Value |
|-----------|-------|
| **Location** | `scripts/sync-domains-to-etcd.sh` |
| **Language** | Bash |
| **Purpose** | Sync domain configs from files to etcd |
| **Output** | `/domains/{domain_id}/config` keys in etcd |

**Pattern**: Follow sync-streams-to-etcd.sh exactly.

---

### 11. config-client (StreamRegistry)

| Attribute | Value |
|-----------|-------|
| **Location** | `config-client/src/stream/registry.rs` |
| **Language** | Rust |
| **Purpose** | Load config from etcd at runtime |
| **Used By** | air-quality-app, future Gold services |

**Current Methods**:
- `load_stream()` - Load single stream config
- `load_all_streams()` - Load all stream configs
- `list_streams()` - List stream IDs

**Gold Layer Additions**:
- Extend `StreamConfig` struct with `gold_etl: Option<GoldEtlConfig>`
- Potentially add `DomainRegistry` for domain configs

---

### 12. Data Dictionary Sync

| Attribute | Value |
|-----------|-------|
| **Location** | `deploy.sh` → `sync_to_data_dictionary()` function |
| **Language** | Bash (generates SQL, pipes to psql) |
| **Purpose** | Populate queryable metadata tables from config |
| **Output** | TimescaleDB `data_dictionary.*` tables |

**How It Works Today**:
```bash
# Iterates config files, generates INSERT statements
for config_dir in "$CONFIG_DIR"/*/; do
    local config_file="$config_dir/config.yaml"

    # Reads specific sections with yaml_get helpers
    local description=$(yaml_get "$config_file" "description" "")

    # Generates SQL
    echo "INSERT INTO data_dictionary.streams ..."
done

# Pipes generated SQL to psql
dcx timescaledb psql -U postgres -d ndp < "$SQL_FILE"
```

**Current Tables Populated**:
| Table | Source Config Section |
|-------|----------------------|
| `streams` | Top-level metadata |
| `entity_schemas` | `entity_schemas[]` |
| `entity_schema_attributes` | `entity_schemas[].attributes[]` |
| `silver_tables` | `silver_etl.target_table` |
| `silver_columns` | `silver_etl.field_mappings[]` |
| `silver_lineage` | `silver_etl.field_mappings[].source_path` |
| `silver_dq_rules` | `silver_etl.dq_checks[]` |

**Gold Layer: Function Must Be Extended**

The current `sync_to_data_dictionary()` does NOT read `gold_etl` sections. Must add:

```bash
# NEW: Read gold_etl section
local gold_enabled=$(yaml_get "$config_file" "gold_etl.enabled" "false")
if [ "$gold_enabled" = "true" ]; then
    local gold_granularities=$(yaml_get "$config_file" "gold_etl.aggregates.granularities" "[]")
    # ... generate INSERT INTO data_dictionary.gold_tables ...
fi

# NEW: Read stream_type for classification
local stream_type=$(yaml_get "$config_file" "stream_type" "observation")
echo "INSERT INTO data_dictionary.stream_classification ..."
```

**New Tables to Create + Populate**:
| Table | Source | Populated From |
|-------|--------|----------------|
| `gold_tables` | Stream config | `gold_etl.aggregates.granularities` |
| `gold_columns` | Stream config | `gold_etl.aggregates.fields.*` |
| `stream_classification` | Stream config | `stream_type` field |
| `domains` | Domain config | `config/domains/*/domain.yaml` |
| `objectives` | Domain config | `domain.objectives[]` |

**Note**: Domain configs are in a separate directory (`config/domains/`), so `sync_to_data_dictionary()` needs a second loop to process those files.

---

## Validation Points

### Pre-Deployment Validation

| Stage | Component | What's Validated |
|-------|-----------|------------------|
| 1 | `ndp-validate --schema-only` | JSON Schema structure |
| 2 | `ndp-validate` | Semantic rules (cross-refs, types) |
| 3 | `ndp-gold-ddl validate` | Gold-specific expressions |
| 4 | CI/CD | All above + tests pass |

### Deployment-Time Validation

| Stage | Component | What's Validated |
|-------|-----------|------------------|
| 1 | `deploy.sh` | Manifest JSON parseable |
| 2 | `handle_etcd_config()` | Config file exists, valid JSON |
| 3 | `ddl-generator.sh` | Config has required sections |
| 4 | `ndp-gold-ddl` | Gold config valid, expressions valid |
| 5 | `psql` | DDL executes without error |

### Runtime Validation

| Stage | Component | What's Validated |
|-------|-----------|------------------|
| 1 | `config-client` | Config deserializes to Rust struct |
| 2 | `StreamConfig::validate()` | Business rules (has fields, has sources) |

---

## Gold Layer Integration Checklist

Every Gold layer feature must touch these components:

### Config Files
- [ ] Add to `config/base/streams/{id}/config.json` (gold_etl section)
- [ ] Create `config/domains/{id}/domain.yaml` (if cross-stream)
- [ ] Add to `config/schemas/gold-etl.schema.json`
- [ ] Add to `config/schemas/domain.schema.json`

### Validation
- [ ] Extend `ndp-validate` JSON schema
- [ ] Add semantic validation rules
- [ ] Add `ndp-gold-ddl validate` checks

### Sync to etcd
- [ ] Verify stream config syncs (includes gold_etl)
- [ ] Create/update `sync-domains-to-etcd.sh`

### DDL Generation
- [ ] Implement in `ndp-gold-ddl` generators
- [ ] Add unit tests for SQL output

### Deployment
- [ ] Add declaration type to manifest schema
- [ ] Implement `handle_gold_table()` in deploy.sh
- [ ] Implement `handle_domain()` in deploy.sh
- [ ] Add Phase 5, Phase 6 to apply() orchestration

### Runtime
- [ ] Extend `StreamConfig` struct
- [ ] Create `DomainConfig` struct (if needed)
- [ ] Update `config-client` registry

### Metadata (sync_to_data_dictionary)
- [ ] Create `gold_tables` table DDL
- [ ] Create `gold_columns` table DDL
- [ ] Create `stream_classification` table DDL
- [ ] Create `domains` table DDL
- [ ] Create `objectives` table DDL
- [ ] Extend `sync_to_data_dictionary()` to read `gold_etl` section
- [ ] Extend `sync_to_data_dictionary()` to read `stream_type` field
- [ ] Add second loop in `sync_to_data_dictionary()` for domain configs

---

## File Modification Summary

| File | Modification Type | Gold Layer Change |
|------|-------------------|-------------------|
| `.deploy/releases/*.manifest.json` | Extend | Add gold-tables, domains arrays |
| `config/base/streams/*/config.json` | Extend | Add gold_etl section, stream_type field |
| `config/domains/*/domain.yaml` | **Create** | New domain config files |
| `config/schemas/gold-etl.schema.json` | **Create** | Gold ETL JSON schema |
| `config/schemas/domain.schema.json` | **Create** | Domain JSON schema |
| `deploy/pi/deploy.sh` | Extend | Add handle_gold_table(), handle_domain(), Phase 5-6 |
| `deploy/pi/deploy.sh` | Extend | Extend sync_to_data_dictionary() for gold_etl, stream_type, domains |
| `tools/ndp-gold-ddl/` | **Create** | Entire Rust tool |
| `tools/ndp-validate/src/schema.rs` | Extend | Add gold_etl to schema |
| `tools/ndp-validate/src/semantic/` | Extend | Add gold.rs, domain.rs |
| `scripts/sync-domains-to-etcd.sh` | **Create** | Domain sync script |
| `config-client/src/stream/registry.rs` | Extend | Add GoldEtlConfig to StreamConfig |
| `core/src/types/stream_config.rs` | Extend | Add gold_etl field, stream_type field |
| `deploy/pi/init-timescaledb.sql` (or similar) | Extend | Add gold_*, stream_classification, domains, objectives tables |

---

## Deployment Sequence Diagram

```
User                   deploy.sh              etcd              ndp-gold-ddl         TimescaleDB
  │                        │                    │                     │                   │
  │  deploy.sh apply       │                    │                     │                   │
  │───────────────────────>│                    │                     │                   │
  │                        │                    │                     │                   │
  │                        │ Phase 1: etcd-config                     │                   │
  │                        │───────────────────>│                     │                   │
  │                        │   PUT /streams/*/config                  │                   │
  │                        │<───────────────────│                     │                   │
  │                        │                    │                     │                   │
  │                        │ Phase 4: silver-tables                   │                   │
  │                        │────────────────────────────────────────────────────────────>│
  │                        │   ddl-generator.sh | psql                │                   │
  │                        │<────────────────────────────────────────────────────────────│
  │                        │                    │                     │                   │
  │                        │ Phase 5: gold-tables                     │                   │
  │                        │────────────────────────────────────────>│                   │
  │                        │   ndp-gold-ddl generate --stream        │                   │
  │                        │<────────────────────────────────────────│                   │
  │                        │                    │                     │  SQL output       │
  │                        │────────────────────────────────────────────────────────────>│
  │                        │   psql                                   │                   │
  │                        │<────────────────────────────────────────────────────────────│
  │                        │                    │                     │                   │
  │                        │ Phase 6: domains   │                     │                   │
  │                        │───────────────────>│                     │                   │
  │                        │   PUT /domains/*/config                  │                   │
  │                        │<───────────────────│                     │                   │
  │                        │────────────────────────────────────────>│                   │
  │                        │   ndp-gold-ddl generate --domain        │                   │
  │                        │<────────────────────────────────────────│                   │
  │                        │────────────────────────────────────────────────────────────>│
  │                        │   psql (aligned view)                    │                   │
  │                        │<────────────────────────────────────────────────────────────│
  │                        │                    │                     │                   │
  │  ✓ Deployment complete │                    │                     │                   │
  │<───────────────────────│                    │                     │                   │
```

---

## References

- [DECISIONS.md](./DECISIONS.md) - Architecture decisions including ADR-FE001-001
- [SCOPE.md](../SCOPE.md) - Full V1.1 scope definition
- [deploy.sh](../../../../deploy/pi/deploy.sh) - Deployment orchestrator
- [ddl-generator.sh](../../../../deploy/pi/ddl-generator.sh) - Silver DDL generator
- [ndp-validate](../../../../tools/ndp-validate/) - Validation tool
- [sync-streams-to-etcd.sh](../../../../scripts/sync-streams-to-etcd.sh) - etcd sync
