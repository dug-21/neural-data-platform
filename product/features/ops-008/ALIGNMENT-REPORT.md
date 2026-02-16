# ops-008 Alignment Report

**Feature**: ops-008 Database Bootstrap & Init-Script Consolidation
**Date**: 2026-02-16
**Artifacts Reviewed**: SPECIFICATION.md, TASK-DECOMPOSITION.md, ARCHITECTURE.md (7 ADRs), PSEUDOCODE.md

## Alignment Summary

| # | Principle | Result | Notes |
|---|-----------|--------|-------|
| 1 | Edge-Only | PASS | All processing is local SQL, no cloud dependencies |
| 2 | Config-Driven | PASS | Init-scripts are structural only; all config-driven DDL delegated to deploy.sh |
| 3 | Domain-Portable | PASS | Init-scripts are domain-agnostic; Silver utility functions are the only domain-specific content (WARN noted below) |
| 4 | Resource-Constrained | PASS | No new runtime processes; SQL scripts run once at init; no memory impact |
| 5 | Integration-First | PASS | Extends existing deploy.sh phases; no parallel systems created |
| 6 | Privacy by Architecture | PASS | No telemetry, no external calls, local-only database bootstrap |
| 7 | Self-Learning | N/A | Infrastructure/ops feature -- no learning component |

**Overall**: PASS

## Detailed Analysis

### 1. Edge-Only
All 9 init-scripts and deploy.sh migrations run locally on the device. No internet connectivity required. No cloud service dependencies. The init-scripts work identically on Pi production and local integration environments.

### 2. Config-Driven
The core innovation of ops-008 is enforcing the config-driven boundary: init-scripts create ONLY structural infrastructure, while deploy.sh creates all objects derived from stream/domain JSON configs. No hardcoded retention periods, thresholds, or refresh intervals in init-scripts. The `grafana_reader` password is hardcoded but this is existing behavior (not introduced by ops-008) and follows the current convention.

### 3. Domain-Portable
Init-scripts are domain-agnostic with one exception: `003-silver-functions.sql` contains three air-quality-domain functions (AQI calculation, mold risk). These functions exist because Silver CAs and Gold views reference them, and they must be available before those objects are created.

**WARN**: `silver.calculate_aqi_pm25()` and `silver.calculate_mold_risk()` are domain-specific functions in a generic bootstrap. This is acceptable because: (a) they are small, stable, and immutable; (b) breaking them out adds complexity without clear benefit; (c) future domains can add their own functions to init-scripts or deploy.sh migrations.

### 4. Resource-Constrained
No new runtime processes. Init-scripts run once at database creation (typically once per device lifetime). The 9 SQL scripts are small (estimated total < 2000 lines). No impact on ongoing memory, CPU, or storage.

### 5. Integration-First
ops-008 extends the existing deploy.sh framework rather than creating a new system:
- Uses existing Phase 3 migrations mechanism for analytics views and dq_events
- Uses existing Phase 4/5/6 for Silver/Gold/Intelligence creation
- Uses existing Phase 8 dimension sync (activating ensure_table)
- No new CLI commands or tools introduced

### 6. Privacy by Architecture
Database bootstrap is entirely local. No external network calls. No telemetry.

### 7. Self-Learning
Not applicable -- this is an infrastructure/operations feature with no learning component.

## Variances Requiring User Approval

None. All checks pass. The WARN on domain-specific Silver functions is documented but does not require user approval -- it matches the existing convention and the SCOPE.md explicitly recommends this approach.

## Scope Alignment

| Check | Result |
|-------|--------|
| All SCOPE.md requirements addressed | PASS -- all 5 open questions resolved with ADRs |
| No out-of-scope additions | PASS -- no features beyond what SCOPE.md defines |
| No scope gaps | PASS -- every acceptance criterion in SCOPE.md maps to a specification AC |
| Simplifications documented | PASS -- Silver CAs deferred (ADR-006), dimension sync functions deferred to ensure_table |

## Version Discipline

ops-008 targets the current deployment infrastructure. No version-specific features. Compatible with both PG15 (production) and PG16 (integration) -- all SQL used is standard PostgreSQL + TimescaleDB + pgvector syntax compatible with both versions.

## Technical Constraints Check

| Constraint | Status |
|-----------|--------|
| ARM64 (Pi 5) | N/A -- SQL scripts, no compiled code |
| Banned: DuckDB | PASS -- not referenced |
| Banned: Polars | PASS -- not referenced |
| Database: TimescaleDB | PASS -- uses TimescaleDB extensions correctly |
| Config-driven | PASS -- Layer 0/Layer 1 boundary enforces this |
| Deployment: Docker on Pi | PASS -- init-scripts integrate with Docker entrypoint |
