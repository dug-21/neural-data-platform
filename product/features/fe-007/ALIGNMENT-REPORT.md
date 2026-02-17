# fe-007 Alignment Report: Granger Causality (Statistical Validation)

## Summary

**Overall: PASS** -- fe-007 is well-aligned with the product vision. One WARN for version targeting (v1.2.x vs roadmap v1.3 for "causal validation"). No FAIL or VARIANCE items.

## Alignment Check Results

### 1. Edge-Only: PASS

All Granger computation runs on-device in the ndp-intelligence Rust crate. No cloud dependencies, no external API calls, no data leaving the device. OLS regression and statistical tests are pure ndarray computation. The incomplete beta function for F-distribution p-values is implemented in Rust, not delegated to an external service.

- No network calls during Granger scan
- All time series data comes from local PostgreSQL (gold aligned view)
- Results stored locally in gold.causal_candidates

### 2. Config-Driven: PASS

All tunable parameters are in the domain config `intelligence.granger` block:
- candidate_count, lag_hours, significance_level, test_method, preprocessing, evidence_window_days, scan_interval_hours, min_observations
- Hot-reloadable via config-client etcd watch
- No hardcoded thresholds in Rust code -- all defaults are in serde default functions and domain schema
- Feature flag (NDP_GRANGER_ENABLED) is deployment-level env var, which is appropriate for a kill switch

### 3. Domain-Portable: PASS

Granger testing operates on generic time series extracted from the gold aligned view. It does not contain air-quality-specific logic:
- Candidate pairs are identified by column names from the view (any domain's streams)
- Statistical tests work on `Vec<f64>` -- domain-agnostic
- Results stored with domain_id column for multi-tenancy
- Configuration is per-domain (intelligence.granger block in domain config)
- Any domain with numeric streams in a gold aligned view can use Granger testing

### 4. Resource-Constrained: PASS

Explicit Pi budget constraints:
- Computation: <30s for Granger scan (tested in test plan)
- Memory: <50MB additional allocation during scan (tested in test plan)
- Runs daily by default (scan_interval_hours=24), not hourly
- Pure Rust + ndarray -- no C dependencies that might fail on ARM64
- ndarray already compiles for aarch64-unknown-linux-gnu in the workspace
- Small matrix operations (typically 5x5 to 15x15) -- well within Pi 5 capability

### 5. Integration-First: PASS

Granger integrates into the existing intelligence cycle:
- Added as step 6.5 in IntelligenceService::run_cycle() -- extends existing method
- Uses existing K-NN similarity results as candidate source
- Reuses existing connection pool and database infrastructure
- GrangerConfig extends existing IntelligenceConfig (adds optional field)
- DDL generator follows existing ndp-lib generator pattern
- No new containers, no new schedulers, no new triggers
- Feature flag follows AppConfig::from_env() pattern

### 6. Privacy by Architecture: PASS

No data leaves the device. Granger results are stored locally. No telemetry, no external reporting. The feature-flagged nature means the computation does not even run unless explicitly enabled.

### 7. Self-Learning: PASS

This is a core self-learning feature:
- Granger causality discovers genuine lead-lag relationships from the system's own data
- Evidence accumulation tracks relationship stability over time (compounding intelligence)
- Validated relationships feed predictions (confirmed causal links become prediction signals)
- The system improves its understanding of inter-stream dynamics the longer it runs

## Version Targeting: WARN

The product roadmap (FEATURE-ROADMAPv1.2.md, ALIGNMENT-CRITERIA.md) lists:
- v1.2: "Discovery engine -- automatic correlation detection" (IN PROGRESS)
- v1.3: "Prediction & actions -- causal validation, model selection" (PLANNED)

fe-007 (Granger causality = "causal validation") could be argued as v1.3 scope. However:
- The SCOPE.md explicitly targets v1.2.x
- fe-007 extends the v1.2 discovery engine (validates correlations found by K-NN)
- fe-004 (K-NN similarity) is already deployed in v1.2.6
- Granger testing is a natural complement to similarity search, not a separate prediction system
- The scope is feature-flagged (NDP_GRANGER_ENABLED=false by default), allowing staged rollout

**Recommendation**: Accept as v1.2.x. The feature flag mitigates any risk of premature capability. This is discovery validation, not prediction/action.

## Scope Coverage

All 10 acceptance criteria from SCOPE.md are addressed in the specification:

| AC | Status |
|----|--------|
| Feature flag off = zero overhead | FR-01, tested |
| Feature flag on = candidates tested | FR-02 + FR-03, tested |
| Validated relationships found | FR-03 + FR-10 (FDR correction) |
| Optimal lags identified | FR-04 (BIC selection) |
| Candidate registry populated | FR-05 (causal_candidates table) |
| Evidence accumulation works | FR-07 (rolling window) |
| Pi resource budget | NFR-01 (<30s, <50MB) |
| Stationarity handled | FR-08 + FR-09 (ADF + adaptive) |
| Both test methods work | FR-03 (GrangerTest trait) |
| Domain config hot-reload | FR-12 (config-client) |

No scope gaps. No out-of-scope additions.

## Technical Constraint Compliance

| Constraint | Status |
|------------|--------|
| ARM64 (Pi 5) | PASS -- pure Rust + ndarray, no C deps |
| Banned: DuckDB | PASS -- not used |
| Banned: Polars | PASS -- not used |
| Database: TimescaleDB | PASS -- uses existing gold aligned view |
| Config-driven | PASS -- intelligence.granger block |
| Deployment: Docker on Pi | PASS -- runs inside existing ndp-intelligence container |
