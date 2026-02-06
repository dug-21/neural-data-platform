# OPS-002: Eliminate Hardcoded Silver/Gold Table References from Generators

> **Feature ID:** ops-002
> **Created:** 2026-02-06
> **Status:** Scoping
> **Phase:** ops (Infrastructure / Deployment)

---

## Problem Statement

The Gold layer generators in `ndp-gold-ddl` hardcode Silver and Gold table names, column names, stream identifiers, and threshold values directly in Rust string templates. When the actual database schema doesn't match these assumptions, generated SQL fails silently or errors at deploy time.

### Concrete Failures (v1.1.10)

The `detect_events` procedure (job 1026) failed on production Pi with 2/2 runs failing because:

1. **Wrong table name**: Procedure references `silver.home_assistant_state` but the actual Silver table is `silver.state_events`
2. **Hardcoded Gold CA name**: Procedure references `gold.air_quality_hourly` — correct today, but fragile
3. **Hardcoded column names**: `co2_mean`, `pm25_mean`, `ndp_id`, `state` are baked into the SQL template
4. **Hardcoded stream literals**: `'air-quality'::TEXT` and `'home-assistant-state'` appear as string constants
5. **Hardcoded thresholds**: `800` (CO2) and `12` (PM2.5) are duplicated from objectives config into SQL

### Scope of the Problem

The hardcoded references exist in `tools/ndp-gold-ddl/src/generators/events.rs`, specifically in `generate_detection_procedure()`. The procedure has two sections:

| Section | Hardcoded References |
|---------|---------------------|
| State Transitions | `silver.state_events` (was `silver.home_assistant_state`), column names `ndp_id`, `state`, `event_time` |
| Threshold Crossings | `gold.air_quality_hourly`, columns `co2_mean`, `pm25_mean`, `ndp_id`, `bucket`; thresholds `800`, `12`; stream literal `'air-quality'` |
| Context Enrichment | `gold.{domain}_aligned` view name (this one IS config-driven already) |

### Why This Matters

- **No mapping exists** from `stream_id` (e.g. `home-assistant-state`) to Silver table name (e.g. `silver.state_events`). The naming convention is inconsistent and cannot be derived programmatically.
- **Objectives already declare** streams, metrics, thresholds, and conditions in `domain.json` — but the generator ignores them and duplicates the values as literals.
- **Adding a new domain** would require editing Rust code rather than just writing config files, which breaks the platform's config-driven design principle.

### What Needs to Change

The detection procedure generator should read from configuration to determine:

1. Which Silver table holds state events for a given stream
2. Which Gold CA holds hourly aggregates for a given stream
3. What columns/metrics to query (from stream field definitions)
4. What thresholds to check (from domain objectives)
5. What stream_id literals to use (from domain stream references)
