# Catalogued Pain Points: NDP Configuration System

**Document Type:** Problem Analysis
**Feature:** dp-016 Configuration Architecture Review
**Last Updated:** 2026-02-01
**Source:** Swarm research + air-012 retrospective

---

## Executive Summary

The research identified **23 distinct pain points** across 6 categories. The most critical issues cause **silent failures** where data appears to flow but Silver ETL never runs.

| Category | Count | Severity |
|----------|-------|----------|
| Dual Source of Truth | 4 | Critical |
| Validation Gaps | 7 | High |
| Manual Steps | 5 | Medium |
| Silent Failures | 4 | Critical |
| Observability | 2 | Medium |
| Documentation | 1 | Low |

---

## Category 1: Dual Source of Truth (Critical)

### P-001: etcd vs YAML Split (air-013)

**Description:** Bronze config loads from etcd, Silver config loads from YAML files directly.

**Code Evidence:**
```rust
// Bronze: apps/air-quality-app/src/main.rs
let streams = registry.list_streams().await;  // Reads etcd

// Silver: same file, different function
load_silver_etl_config(&config_dir, stream_id)  // Reads YAML directly
```

**Impact:** If etcd sync fails (validation error), stream isn't listed, so `load_silver_etl_config()` is never called. Silver ETL silently doesn't start.

**Severity:** Critical
**Proposed Fix:** air-013 - Add `silver_etl` to `StreamConfig` in etcd

---

### P-002: Dimension Data in Separate CSV

**Description:** Dimension entries are in `data/dimensions/entity_context.csv`, separate from stream config YAML.

**Impact:**
- Easy to add stream but forget dimensions
- No validation that ndp_id references exist
- Two files to maintain in sync

**Severity:** Medium
**Proposed Fix:** Consider inline dimension definitions in stream config

---

### P-003: Data Dictionary vs Dimension Tables

**Description:** Two separate metadata systems:
- **Dimension tables** (`silver.entity_context`) - enriches data via JOINs
- **Data dictionary** (`data_dictionary.*`) - documents columns for MCP tools

**Impact:** Confusion about where metadata belongs, duplicate maintenance.

**Severity:** Medium
**Proposed Fix:** Clarify boundaries, potentially unify or auto-sync

---

### P-004: Batch vs Streaming Config Loaders

**Description:** Two separate config loading implementations:
- `apps/silver-etl/src/config.rs` - batch ETL (etcd first, YAML fallback)
- `apps/air-quality-app/src/main.rs:load_silver_etl_config()` - streaming (YAML only)

**Impact:** Different behavior between batch and streaming modes.

**Severity:** Medium
**Proposed Fix:** Unified config loader used by both

---

## Category 2: Validation Gaps (High)

### P-005: No source_path Validation

**Description:** `silver_etl.field_mappings.source_path` values are not validated against:
- The `fields` section of the config
- Actual Bronze payload structure

**Code Location:** `core/src/config/silver_etl.rs` - `FieldMapping::validate()` only checks column type

**Impact:** Typos in source paths cause NULL values in Silver with no warning.

**Severity:** High
**Proposed Fix:** Cross-validate source_path against fields section

---

### P-006: No Silver Table Existence Check

**Description:** No validation at startup that `target_table` exists in TimescaleDB.

**Code Location:** `apps/air-quality-app/src/main.rs:500-600` - creates SilverSubscriber without table check

**Impact:** First INSERT attempt fails at runtime. Data may be lost.

**Severity:** High
**Proposed Fix:** Validate table existence at startup, fail loudly

---

### P-007: Unknown Fields Silently Captured

**Description:** `#[serde(flatten)]` captures unknown YAML fields into `extra` HashMap without warning.

**Code Location:** `core/src/types/stream_config.rs`
```rust
#[serde(flatten)]
pub extra: HashMap<String, serde_json::Value>,
```

**Impact:** Typos like `silver_elt` instead of `silver_etl` are silently ignored.

**Severity:** High
**Proposed Fix:** Log warning for non-empty `extra` HashMap

---

### P-008: No Cross-Schema Reference Validation

**Description:** Three config sections that should agree are validated independently:
- `fields` (Bronze schema)
- `entity_schemas` (Data dictionary)
- `silver_etl.field_mappings` (Silver transformation)

**Impact:** Inconsistencies not detected until runtime or never.

**Severity:** Medium
**Proposed Fix:** Add cross-reference validation

---

### P-009: DQ Rule Column Names Not Validated

**Description:** DQ rules reference column names that are not validated to exist.

**Impact:** DQ rules silently apply to NULL (column not found) or cause runtime errors.

**Severity:** Medium
**Proposed Fix:** Validate DQ rule columns against field_mappings.target_column

---

### P-010: No Type Consistency Check

**Description:** YAML `field_mappings.type` (e.g., `double_precision`) not validated against actual Silver DDL column type.

**Impact:** Type mismatch causes runtime INSERT errors.

**Severity:** Medium
**Proposed Fix:** Schema inference or DDL generation from config

---

### P-011: Enum Values Not Validated

**Description:** When field type is `enum`, the allowed values in config are not validated against actual data.

**Impact:** Unexpected values may slip through or be flagged incorrectly.

**Severity:** Low
**Proposed Fix:** Runtime validation with reporting

---

## Category 3: Manual Steps (Medium)

### P-012: Manual DDL Required (dp-015)

**Description:** Silver tables must be manually created via SQL scripts. YAML config describes schema but nothing creates the table.

**Current Workflow:**
1. Write DDL file
2. Run `./deploy.sh silver-migrate`

**Impact:** "Config-driven" promise broken. Easy to forget, causes silent failures.

**Severity:** High
**Proposed Fix:** dp-015 - Generate DDL from SilverEtlConfig

---

### P-013: Manual Data Dictionary Sync

**Description:** After adding stream config, must manually run `./deploy.sh sync-dictionary`.

**Impact:** MCP tools show stale metadata until manually synced.

**Severity:** Medium
**Proposed Fix:** Auto-sync on config change or app startup

---

### P-014: Manual Dimension Sync

**Description:** After editing dimension CSV, must manually run `./deploy.sh sync-dimensions`.

**Impact:** JOINs return NULL until dimensions synced.

**Severity:** Medium
**Proposed Fix:** Auto-sync or inline dimensions in stream config

---

### P-015: Manual Pi Deployment

**Description:** Multiple SSH commands required to deploy changes:
```bash
git pull
./deploy.sh silver-migrate
./deploy.sh sync
./deploy.sh sync-dictionary
./deploy.sh sync-dimensions
docker restart air-quality-app
```

**Impact:** Error-prone, easy to miss steps, no single-command deploy.

**Severity:** Medium
**Proposed Fix:** Single `./deploy.sh update` that does all steps

---

### P-016: No Hot Reload

**Description:** Config changes require container restart to take effect.

**Impact:** Downtime for config updates, no dynamic stream addition.

**Severity:** Low
**Proposed Fix:** etcd watch + dynamic reload (future feature)

---

## Category 4: Silent Failures (Critical)

### P-017: etcd Sync Failure Logged as Warning

**Description:** `sync_service.sync_all()` failures are logged as WARN, application continues.

**Code Location:** `apps/air-quality-app/src/main.rs`
```rust
Err(e) => {
    tracing::warn!("Config sync failed: {}", e);
    // Application continues with stale config!
}
```

**Impact:** App runs with stale/missing etcd config. Components reading etcd see old data.

**Severity:** Critical
**Proposed Fix:** Promote to ERROR, add `--strict` mode, health endpoint

---

### P-018: Validation Errors Skip Stream Silently

**Description:** If one stream's config fails validation, it's skipped and others continue.

**Code Location:** `apps/air-quality-app/src/config_sync/service.rs:216-249`

**Impact:** One typo in YAML causes that stream to silently not exist. Other streams work fine, masking the problem.

**Severity:** High
**Proposed Fix:** Clear logging, health endpoint showing skipped streams

---

### P-019: Silver ETL Doesn't Start - No Clear Error

**Description:** When Silver ETL doesn't start for a stream, there's no clear indication.

**Current Behavior:**
- No "SilverSubscriber created" log = silent failure
- Must grep logs to notice absence

**Impact:** Data flows to Bronze but not Silver. Discovered only when querying empty tables.

**Severity:** Critical
**Proposed Fix:** Explicit "SilverSubscriber NOT created for stream X because Y" logging

---

### P-020: Missing Table Not Caught Until INSERT

**Description:** If Silver table doesn't exist, error only appears on first INSERT attempt.

**Impact:** May lose initial data batch, error buried in logs.

**Severity:** High
**Proposed Fix:** Validate table existence at startup

---

## Category 5: Observability (Medium)

### P-021: No Config Health Endpoint

**Description:** `/health` endpoint only checks HTTP server is running. No config validation status.

**Current State:**
```json
{"status": "ok"}
```

**Desired State:**
```json
{
  "status": "ok",
  "config": {
    "streams": {
      "air-quality": {"etcd_synced": true, "silver_enabled": true, "silver_table_exists": true},
      "my-broken-stream": {"etcd_synced": false, "error": "validation failed"}
    },
    "last_sync": "2026-01-31T10:00:00Z"
  }
}
```

**Impact:** Operators must grep logs to find config issues.

**Severity:** Medium
**Proposed Fix:** Add `/health/config` endpoint

---

### P-022: No Config Metrics

**Description:** No Prometheus metrics for config state:
- Number of synced streams
- Number of failed streams
- Last sync timestamp
- Validation error count

**Impact:** No alerting on config sync failures.

**Severity:** Medium
**Proposed Fix:** Add config-related metrics

---

## Category 6: Documentation (Low)

### P-023: No "Add Stream" Runbook

**Description:** Until this document, there was no step-by-step guide for adding a new stream.

**Impact:** Agents and operators had to discover process by trial and error.

**Severity:** Low (now addressed)
**Status:** Resolved by AS-IS-PROCESS.md

---

## Pain Point Priority Matrix

| ID | Pain Point | Severity | Effort | Priority |
|----|------------|----------|--------|----------|
| P-001 | etcd vs YAML split | Critical | Medium | **1** |
| P-017 | Sync failure as warning | Critical | Low | **2** |
| P-019 | Silent Silver ETL failure | Critical | Low | **3** |
| P-007 | Unknown fields silent | High | Low | **4** |
| P-012 | Manual DDL | High | Medium | **5** |
| P-006 | No table existence check | High | Low | **6** |
| P-005 | No source_path validation | High | Medium | **7** |
| P-018 | Validation skips silently | High | Low | **8** |
| P-020 | Missing table not caught | High | Low | **9** |
| P-021 | No config health endpoint | Medium | Medium | **10** |

---

## Relationship to Existing Features

| Pain Point | Related Feature | Status |
|------------|-----------------|--------|
| P-001 | air-013: Unified Config Source | Scoped |
| P-012 | dp-015: Config-Driven Silver Tables | Scoped |
| P-002, P-014 | Part of dp-016 architecture | In Review |
| P-005 - P-011 | dp-016 → dp-017: Config Validation | TBD |
| P-015, P-016 | ops-001: Deployment Automation | TBD |
| P-021, P-022 | dp-016 → dp-018: Config Observability | TBD |

---

## Recommended Implementation Order

Based on severity, effort, and dependencies:

### Phase 1: Stop the Bleeding (Quick Wins)
1. P-017: Promote sync failure to ERROR
2. P-007: Log warning for non-empty `extra`
3. P-019: Explicit "SilverSubscriber NOT created" logging
4. P-018: List skipped streams at startup

### Phase 2: Foundation Fix (air-013)
5. P-001: Unified config source in etcd

### Phase 3: Validation Framework
6. P-006: Table existence check
7. P-020: Prerequisite validation at startup
8. P-005: Cross-validate source_path
9. P-008: Cross-schema reference validation

### Phase 4: Automation (dp-015)
10. P-012: Generate DDL from config

### Phase 5: Observability
11. P-021: Config health endpoint
12. P-022: Config metrics

### Phase 6: Polish
13. P-015: Single-command deployment
14. P-016: Hot reload (future)

---

*This catalogue serves as the definitive list of configuration system issues for the dp-016 architecture review.*
