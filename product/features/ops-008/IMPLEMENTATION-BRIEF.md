# Implementation Brief: ops-008 Database Bootstrap & Init-Script Consolidation

## SPARC Artifacts

| Artifact | Path |
|----------|------|
| Scope | product/features/ops-008/SCOPE.md |
| Specification | product/features/ops-008/specification/SPECIFICATION.md |
| Task Decomposition | product/features/ops-008/specification/TASK-DECOMPOSITION.md |
| Architecture (ADRs) | product/features/ops-008/architecture/ARCHITECTURE.md |
| Pseudocode | product/features/ops-008/pseudocode/PSEUDOCODE.md |
| Alignment Report | product/features/ops-008/ALIGNMENT-REPORT.md |
| Acceptance Map | product/features/ops-008/ACCEPTANCE-MAP.md |

## Goal

Replace the 10 broken init-scripts in `deploy/pi/init-scripts/` with 9 correctly-ordered, self-contained SQL scripts that deterministically bootstrap a blank PostgreSQL database for NDP. Establish a clean two-layer bootstrap architecture: Layer 0 (init-scripts) provides foundational infrastructure (extensions, schemas, roles, data_dictionary tables, utility functions), while Layer 1 (deploy.sh apply) creates all config-driven objects (Silver hypertables, Gold CAs, intelligence tables, dimensions, dictionary data). This unblocks ops-007 (integration testbed framework) and enables deploying NDP to new Pi devices without manual database intervention.

## Tracking

GitHub Issue: https://github.com/dug-21/neural-data-platform/issues/22

## Resolved Decisions

| Decision | Resolution | Source | Pattern ID |
|----------|-----------|--------|------------|
| Two-layer bootstrap boundary | Init-scripts = structural foundation; deploy.sh = config-driven DDL | ADR-001 | 26 |
| Init-script naming convention | NNN-description.sql (three-digit, hyphen separator) | ADR-002 | 27 |
| Dimension table creation | Remove from init-scripts; activate ensure_table() in ndp dimension sync | ADR-003 | 28 |
| silver.dq_events placement | deploy.sh migration, not init-scripts (hypertable with policies = Layer 1) | ADR-004 | 29 |
| silver.schema_version | Drop entirely; manifests + git tags handle versioning | ADR-005 | 30 |
| Silver continuous aggregates | Deferred; existing silver-etl migrate pathway continues | ADR-006 | 31 |
| Analytics views migration | analytics schema in init-scripts; views in deploy.sh migration post-Phase-4 | ADR-007 | 32 |

## Files to Create

| Path | Description |
|------|-------------|
| `deploy/pi/init-scripts/001-extensions.sql` | CREATE EXTENSION timescaledb, vector with verification |
| `deploy/pi/init-scripts/002-schemas.sql` | CREATE SCHEMA data_dictionary, silver, gold, analytics |
| `deploy/pi/init-scripts/003-silver-functions.sql` | silver.linear_interpolate, silver.calculate_aqi_pm25, silver.calculate_mold_risk |
| `deploy/pi/init-scripts/004-roles.sql` | CREATE ROLE ndp_app, grafana_reader + schema grants + default privileges |
| `deploy/pi/init-scripts/005-data-dictionary.sql` | Core tables: streams, fields, sources, entity_schemas, entity_schema_attributes, sync_status + indexes |
| `deploy/pi/init-scripts/006-silver-dictionary.sql` | Silver metadata: silver_tables, silver_columns, silver_lineage, silver_dq_rules + indexes |
| `deploy/pi/init-scripts/007-classification.sql` | stream_classification, gold_tables + indexes |
| `deploy/pi/init-scripts/008-domain-objectives.sql` | domains, domain_streams, objectives, constraints + indexes |
| `deploy/pi/init-scripts/009-dictionary-views.sql` | All data_dictionary views and functions consolidated from 5 current scripts |
| `deploy/pi/migrations/001-analytics-views.sql` | analytics.forecast_accuracy, indoor_outdoor_comparison, latest_readings (post-Silver) |
| `deploy/pi/migrations/002-dq-events.sql` | silver.dq_events hypertable with retention policy |

## Files to Delete

| Path | Reason |
|------|--------|
| `deploy/pi/init-scripts/00-create-extensions.sql` | Replaced by 001-extensions.sql |
| `deploy/pi/init-scripts/002_state_events_schema.sql` | Redundant with config-driven Phase 4 |
| `deploy/pi/init-scripts/003_silver_data_dictionary.sql` | Replaced by 006-silver-dictionary.sql |
| `deploy/pi/init-scripts/004_stream_classification.sql` | Replaced by 007-classification.sql |
| `deploy/pi/init-scripts/005_domain_objectives.sql` | Replaced by 008-domain-objectives.sql |
| `deploy/pi/init-scripts/006_pgvector_extension.sql` | Merged into 001-extensions.sql |
| `deploy/pi/init-scripts/01-create-data-dictionary.sql` | Replaced by 005-data-dictionary.sql |
| `deploy/pi/init-scripts/02-create-users.sql` | Replaced by 004-roles.sql |
| `deploy/pi/init-scripts/03-add-computed-columns.sql` | No-op (entirely commented out) |
| `deploy/pi/init-scripts/04-dimension-tables.sql` | Dimensions moved to ndp dimension sync ensure_table |

## Files to Modify

| Path | Change |
|------|--------|
| `deploy/pi/deploy.sh` | Add migrations directory execution (Phase 3); handle analytics views and dq_events migrations |

## SQL Structure (Key Scripts)

### 001-extensions.sql
```sql
CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;
CREATE EXTENSION IF NOT EXISTS vector;
-- Verification DO block
```

### 002-schemas.sql
```sql
CREATE SCHEMA IF NOT EXISTS data_dictionary;
CREATE SCHEMA IF NOT EXISTS silver;
CREATE SCHEMA IF NOT EXISTS gold;
CREATE SCHEMA IF NOT EXISTS analytics;
```

### 004-roles.sql
```sql
-- ndp_app: LOGIN role for applications
-- grafana_reader: READ-ONLY role for dashboards
-- Grants: USAGE on all schemas, DEFAULT PRIVILEGES for future tables
-- Key: ALTER DEFAULT PRIVILEGES for auto-grant on new tables
```

### 009-dictionary-views.sql (consolidated)
```sql
-- 12 views consolidated from 5 current scripts:
-- v_data_dictionary, stream_overview (from 01-)
-- v_complete_dictionary, v_silver_table_overview, v_lineage, v_dq_rules_summary, v_column_search (from 003_)
-- v_stream_classification_summary, v_correlation_candidates (from 004_)
-- v_domain_overview, v_objectives_with_context, v_high_priority_objectives (from 005_)
--
-- 7 functions consolidated:
-- get_column_lineage, get_column_dq_rules (from 003_)
-- derive_correlation_role, derive_null_handling, sync_stream_classification (from 004_)
-- get_objectives_for_stream, check_objective_violation (from 005_)
```

## Test Expectations

### Unit Tests
No Rust unit tests needed -- ops-008 is pure SQL and deploy.sh scripting.

### Integration Tests
| Test | Method | Expected |
|------|--------|----------|
| Clean-slate init | `docker compose down -v && up -d` | Zero errors in TimescaleDB logs |
| Schema verification | psql queries on pg_namespace | 4 schemas: data_dictionary, silver, gold, analytics |
| Role verification | psql queries on pg_roles | 2 roles: ndp_app, grafana_reader |
| Table count | information_schema.tables query | 16 data_dictionary tables |
| Function verification | pg_proc query | 3 silver functions |
| No hypertables at init | timescaledb_information.hypertables | 0 silver hypertables |
| deploy.sh apply | Full deploy | Silver/Gold/Intelligence objects created |
| Idempotency | Re-run init-scripts | Zero errors |
| Smoke test | run-testbed.sh smoke | End-to-end pass |

## Constraints

- **No Rust code changes** for init-script work (apps already assume DDL exists)
- **Minor Rust change**: Activate `ensure_table()` feature flag in dimension sync (if behind feature gate)
- **Integration environment first**: All testing on docker-compose.integration.yml before Pi
- **Same scripts for Pi and integration**: No environment-specific SQL
- **NNN-description.sql naming**: Three-digit zero-padded, hyphen separator
- **No hardcoded DDL for streams**: All stream-specific DDL is deploy.sh's responsibility
- **ARM64 compatible**: SQL only, no architecture constraints
- **PG15/PG16 compatible**: Standard PostgreSQL + TimescaleDB + pgvector syntax

## Dependencies

- TimescaleDB extension available in PostgreSQL image
- pgvector extension available in PostgreSQL image
- deploy.sh Phase 3 migrations mechanism (existing)
- deploy.sh Phase 4 ddl-generator.sh (existing, no changes)
- silver-etl migrate for Silver CAs (existing, no changes)

## NOT in Scope

- Production Dockerfile fix (`docker/timescaledb/Dockerfile` Alpine/apt-get bug)
- PG15 vs PG16 version alignment
- Neural-trader vestigial artifacts in `docker/timescaledb/`
- Making Silver CAs config-driven (follow-up feature)
- Changes to Rust applications (air-quality-app, ndp-intelligence-app)
- Dimension sync ensure_table Rust implementation (only activation of existing feature flag)

## Wave Structure

| Wave | Tasks | Parallel? | Est. Effort |
|------|-------|-----------|-------------|
| Wave 1 | Write 9 new init-scripts | Yes (all parallel) | M |
| Wave 2 | Delete 10 old scripts + create migrations + update deploy.sh | Mostly parallel | M |
| Wave 3 | Integration testing + validation | Sequential then parallel | M |

## Alignment Status

**Overall: PASS** (no variances requiring user approval)

One WARN noted: Silver utility functions (AQI, mold risk) are domain-specific in a generic bootstrap. Accepted because they are small, stable, and needed by Silver CAs/Gold views. See ALIGNMENT-REPORT.md for details.

## Pattern IDs for Implementation

Planning input patterns: 3 (deploy-sh-ndp-dispatch), 18 (ADR ops-007-002 etcd sync), 21 (ADR ops-007-005 clean slate), 22 (ADR ops-007-006 manifest-per-testbed), 25 (integration-testbed-usage)

ADR patterns (from this planning swarm): 26 (two-layer-bootstrap), 27 (naming-convention), 28 (dimension-ensure-table), 29 (dq-events-migration), 30 (schema-version-removal), 31 (silver-cas-deferral), 32 (analytics-views-migration)
