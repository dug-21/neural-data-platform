# ops-008 Task Decomposition

## Wave Structure

### Wave 1: New Init-Scripts (Core)
**Parallel — no dependencies between scripts**

| Task | File | Description | Effort |
|------|------|-------------|--------|
| W1-01 | `deploy/pi/init-scripts/001-extensions.sql` | CREATE EXTENSION timescaledb, vector. Verification check. | S |
| W1-02 | `deploy/pi/init-scripts/002-schemas.sql` | CREATE SCHEMA data_dictionary, silver, gold, analytics. All IF NOT EXISTS. | S |
| W1-03 | `deploy/pi/init-scripts/003-silver-functions.sql` | silver.linear_interpolate(), silver.calculate_aqi_pm25(), silver.calculate_mold_risk(). Copied from 001_silver_schema.sql Section 1. | S |
| W1-04 | `deploy/pi/init-scripts/004-roles.sql` | CREATE ROLE ndp_app (LOGIN), grafana_reader (LOGIN). Schema grants for data_dictionary to grafana_reader. | S |
| W1-05 | `deploy/pi/init-scripts/005-data-dictionary.sql` | Core tables: streams, fields, sources, entity_schemas, entity_schema_attributes, sync_status. Indexes. From current 01-create-data-dictionary.sql. | M |
| W1-06 | `deploy/pi/init-scripts/006-silver-dictionary.sql` | Silver metadata: silver_tables, silver_columns, silver_lineage, silver_dq_rules. Indexes. sync_status column additions. From current 003_silver_data_dictionary.sql. | M |
| W1-07 | `deploy/pi/init-scripts/007-classification.sql` | stream_classification, gold_tables. Indexes, functions. From current 004_stream_classification.sql. | M |
| W1-08 | `deploy/pi/init-scripts/008-domain-objectives.sql` | domains, domain_streams, objectives, constraints. Indexes, sync_status additions. From current 005_domain_objectives.sql. | M |
| W1-09 | `deploy/pi/init-scripts/009-dictionary-views.sql` | All data_dictionary views and functions consolidated: v_data_dictionary, stream_overview, v_complete_dictionary, v_silver_table_overview, v_lineage, v_dq_rules_summary, v_column_search, v_stream_classification_summary, v_correlation_candidates, v_domain_overview, v_objectives_with_context, v_high_priority_objectives. Plus functions: get_column_lineage, get_column_dq_rules, derive_correlation_role, derive_null_handling, sync_stream_classification, get_objectives_for_stream, check_objective_violation. | L |

### Wave 2: Cleanup & Migration
**Sequential — depends on Wave 1**

| Task | Description | Effort |
|------|-------------|--------|
| W2-01 | Delete all 10 old init-scripts from `deploy/pi/init-scripts/` | S |
| W2-02 | Create deploy.sh migration for analytics views (analytics.forecast_accuracy, analytics.indoor_outdoor_comparison, analytics.latest_readings) — runs after Phase 4 Silver table creation | M |
| W2-03 | Create deploy.sh migration for silver.dq_events hypertable + retention policy | S |
| W2-04 | Update docker-compose.integration.yml if init-scripts volume mapping needs changes | S |
| W2-05 | Verify `02-create-users.sql` grant on silver schema still works (silver schema now created by init-scripts, grants in 004-roles.sql) | S |

### Wave 3: Validation & Testing
**Sequential — depends on Wave 2**

| Task | Description | Effort |
|------|-------------|--------|
| W3-01 | Integration test: `docker compose down -v && up -d`, verify zero init errors | M |
| W3-02 | Integration test: `deploy.sh apply` on fresh DB, verify Silver/Gold/Intelligence creation | M |
| W3-03 | Integration test: Re-run init-scripts on existing DB (idempotency check) | S |
| W3-04 | Integration test: Run smoke testbed end-to-end | M |
| W3-05 | Verify C locale sort order: `LC_ALL=C ls deploy/pi/init-scripts/` | S |
| W3-06 | Verify no references to 001_silver_schema.sql in new scripts | S |
| W3-07 | Verify no Silver hypertable creation in init-scripts (grep for create_hypertable) | S |

## Dependencies

```
Wave 1 (all parallel)
  |
  v
Wave 2 (W2-01 first, then W2-02..W2-05 parallel)
  |
  v
Wave 3 (W3-01 first, then rest parallel)
```

## Effort Scale

- S (Small): < 30 min, straightforward copy/adapt
- M (Medium): 30-60 min, requires careful extraction and testing
- L (Large): 60-120 min, consolidation of multiple sources

## Files Created

| Path | Source |
|------|--------|
| `deploy/pi/init-scripts/001-extensions.sql` | New (from 00-create-extensions.sql + 006_pgvector_extension.sql) |
| `deploy/pi/init-scripts/002-schemas.sql` | New |
| `deploy/pi/init-scripts/003-silver-functions.sql` | New (from 001_silver_schema.sql Section 1) |
| `deploy/pi/init-scripts/004-roles.sql` | New (from 02-create-users.sql + 001_silver_schema.sql Section 11) |
| `deploy/pi/init-scripts/005-data-dictionary.sql` | Adapted from 01-create-data-dictionary.sql (tables only, views moved to 009) |
| `deploy/pi/init-scripts/006-silver-dictionary.sql` | Adapted from 003_silver_data_dictionary.sql (tables only, views moved to 009) |
| `deploy/pi/init-scripts/007-classification.sql` | Adapted from 004_stream_classification.sql (tables only, views/functions moved to 009) |
| `deploy/pi/init-scripts/008-domain-objectives.sql` | Adapted from 005_domain_objectives.sql (tables only, views/functions moved to 009) |
| `deploy/pi/init-scripts/009-dictionary-views.sql` | Consolidated from all current scripts' views and functions |

## Files Deleted

| Path | Reason |
|------|--------|
| `deploy/pi/init-scripts/00-create-extensions.sql` | Replaced by 001-extensions.sql |
| `deploy/pi/init-scripts/002_state_events_schema.sql` | Redundant — state_events created by deploy.sh Phase 4 from config |
| `deploy/pi/init-scripts/003_silver_data_dictionary.sql` | Replaced by 006-silver-dictionary.sql |
| `deploy/pi/init-scripts/004_stream_classification.sql` | Replaced by 007-classification.sql |
| `deploy/pi/init-scripts/005_domain_objectives.sql` | Replaced by 008-domain-objectives.sql |
| `deploy/pi/init-scripts/006_pgvector_extension.sql` | Merged into 001-extensions.sql |
| `deploy/pi/init-scripts/01-create-data-dictionary.sql` | Replaced by 005-data-dictionary.sql |
| `deploy/pi/init-scripts/02-create-users.sql` | Replaced by 004-roles.sql |
| `deploy/pi/init-scripts/03-add-computed-columns.sql` | Entirely commented out, no-op — removed |
| `deploy/pi/init-scripts/04-dimension-tables.sql` | Dimension tables moved to ndp dimension sync ensure_table |

## Files Modified

| Path | Change |
|------|--------|
| `deploy/pi/deploy.sh` | Add analytics views migration step (after Phase 4); add dq_events migration; adjust Phase 8 dimension sync to use ensure_table |

## Open Questions Resolved

| Question | Resolution | ADR |
|----------|-----------|-----|
| Silver CAs | Deferred — silver-etl migrate handles them, not in ops-008 scope | ADR-006 |
| silver.dq_events | deploy.sh migration, not init-scripts | ADR-004 |
| silver.schema_version | Dropped — manifests handle versioning | ADR-005 |
| Dimension tables | ndp dimension sync ensure_table (idempotent) | ADR-003 |
| Analytics views | deploy.sh migration after Phase 4 | ADR-007 |
