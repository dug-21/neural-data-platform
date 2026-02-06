# OPS-002 Hardcoding Audit Report

> **Generated:** 2026-02-06
> **Scope:** ndp-gold-ddl, ndp-validate, ndp-lib, deploy scripts
> **Finding:** 50+ hardcoded domain-specific values across 13 source files

---

## Priority Classification

### P0 - CRITICAL (Domain logic hardcoded in SQL generators)

#### 1. `events.rs` :: `generate_detection_procedure()` (25+ hardcoded values)

| Lines | Hardcoded Value | Should Read From |
|-------|----------------|-----------------|
| 475 | `'home-assistant-state'` stream literal | `domain.streams[role=actuator].stream_id` |
| 476 | `s.ndp_id` entity column | `TransitionConfig.entity_field` |
| 478 | `s.state` state column | `TransitionConfig.state_field` |
| 481 | `silver.state_events` table | `stream.silver_etl.target_table` |
| 505-510 | 6 context enrichment fields (`indoor_co2_mean`, `indoor_pm25_mean`, etc.) | Derive from `domain.streams[].gold_etl.aggregates` |
| 530 | `'air-quality'::TEXT` stream literal | `domain.objectives[].target.stream` |
| 531 | `co2_mean` column name | `stream.gold_etl.aggregates.fields` naming convention |
| 533 | `pm25_mean` column name | Same |
| 535 | `gold.air_quality_hourly` Gold table | Derive from stream config like `AlignedViewGenerator` |
| 544 | `'co2' AS metric` | `domain.objectives[].target.metric` |
| 545 | `800.0 AS threshold_value` | `domain.objectives[].target.threshold` |
| 547-548, 558-559 | `800` CO2 threshold (used 8x) | Same |
| 552 | `'healthy_co2' AS objective_id` | `domain.objectives[].id` |
| 568 | `'pm25' AS metric` | `domain.objectives[].target.metric` |
| 569 | `12.0 AS threshold_value` | `domain.objectives[].target.threshold` |
| 571-572, 582-583 | `12` PM2.5 threshold (used 8x) | Same |
| 576 | `'healthy_pm25' AS objective_id` | `domain.objectives[].id` |
| 621 | `WHEN 'co2' THEN 'ppm' ELSE 'ug/m3'` | `domain.objectives[].target.unit` |

#### 2. `state_transitions.rs` :: Device type inference

| Lines | Hardcoded Value | Should Read From |
|-------|----------------|-----------------|
| 296-297 | `'off'`/`'on'` binary state values | TransitionConfig `direction_mapping` or `states` config |
| 309-312 | `'door_%'`, `'window_%'`, `'motion_%'`, `'light_%'` | `device_type_mapping` config or entity schema |

#### 3. `aligned_view.rs` :: Stream type inference

| Lines | Hardcoded Value | Should Read From |
|-------|----------------|-----------------|
| 122-127 | String matching on stream_id to infer type (`"forecast"`, `"state"`, `"event"`, `"dimension"`) | Stream config `stream_type` field |

---

### P1 - SHOULD FIX (Cross-cutting magic strings)

#### 4. `"ndp_id"` entity column (5 files)

- `continuous_aggregate.rs:46`
- `state_transitions.rs:67`
- `events.rs:476`
- `main.rs:313`
- `ndp-validate/src/semantic/gold.rs:410`

**Fix:** Single `const NDP_ENTITY_COLUMN: &str = "ndp_id"` or read from stream config.

#### 5. `"gold"` schema name (6+ locations)

- `events.rs:104`
- `continuous_aggregate.rs:67`
- `aligned_view.rs` (throughout)
- `sync.rs:201, 219`
- `state_transitions.rs:333, 372`

**Fix:** Single `const GOLD_SCHEMA: &str = "gold"` or configurable.

#### 6. `join_builder.rs:123` - `f.issued_at` forecast timestamp

**Fix:** Read from stream config `silver_etl.timestamp` for forecast streams.

---

### P2 - LOW PRIORITY (Acceptable defaults, keep consistent)

- `"observation_time"` default timestamp (4 files) - make a shared constant
- Refresh policy defaults (`"15 minutes"`, `"4 hours"`) - documented, overridable
- `90` day retention / WHERE clause - should be configurable
- `"/opt/ndp/config"` Pi path - acceptable edge fallback

---

## Impact Assessment

| Severity | Count | Risk |
|----------|-------|------|
| P0 Critical | 30+ values in 3 files | Deploy fails for any non-air-quality domain |
| P1 Should Fix | 12+ values in 8 files | Fragile, inconsistent, maintenance burden |
| P2 Low | 8+ values in 5 files | Acceptable defaults but could be cleaner |

## Root Cause

The Gold layer generators were built with air-quality as the reference implementation. Domain-specific values leaked into SQL templates instead of being derived from the existing config structures (DomainConfig, StreamConfig, objectives).

## Config Sources That Already Exist

| Config | Location | Contains |
|--------|----------|----------|
| `DomainConfig` | `config/domains/air-quality/domain.json` | streams, objectives, thresholds |
| `StreamConfig` | `config/base/streams/*.json` | stream_id, silver_etl, gold_etl, fields |
| `TransitionConfig` | Embedded in StreamConfig gold_etl | state_field, entity_field |
| `AlignedViewConfig` | `config/domains/*/alignment.json` | streams, aliases, granularity |

The fix is NOT adding new config -- it's making the generators READ from config that already exists.
