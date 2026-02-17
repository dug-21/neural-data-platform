# fe-007 Task Decomposition: Granger Causality

## Wave Structure

### Wave 1: Core Statistical Library + Table DDL

Foundation layer: all pure computation, no integration dependencies. Can be developed and tested independently.

| Task | Description | Files | Dependencies |
|------|-------------|-------|-------------|
| T-01 | OLS regression engine | `crates/ndp-intelligence/src/granger/ols.rs` | ndarray |
| T-02 | ADF stationarity test | `crates/ndp-intelligence/src/granger/adf.rs` | T-01 |
| T-03 | Adaptive preprocessing pipeline | `crates/ndp-intelligence/src/granger/preprocessing.rs` | T-02 |
| T-04 | GrangerTest trait + Classical F-test | `crates/ndp-intelligence/src/granger/mod.rs`, `crates/ndp-intelligence/src/granger/classical.rs` | T-01 |
| T-05 | Toda-Yamamoto test | `crates/ndp-intelligence/src/granger/toda_yamamoto.rs` | T-01 |
| T-06 | BIC lag selection | `crates/ndp-intelligence/src/granger/lag_selection.rs` | T-04 |
| T-07 | Benjamini-Hochberg FDR correction | `crates/ndp-intelligence/src/granger/fdr.rs` | None |
| T-08 | F-distribution p-value (incomplete beta) | `crates/ndp-intelligence/src/granger/stats.rs` | None |
| T-09 | causal_candidates DDL generator | `crates/ndp-lib/src/gold/generators/causal_candidates.rs` | None |
| T-10 | Granger config types | `crates/ndp-lib/src/gold/embeddings/config.rs` (extend IntelligenceConfig) | None |

**Exit criteria**: All unit tests pass. OLS produces correct beta coefficients on synthetic data. ADF correctly identifies stationary vs non-stationary series. F-test produces correct F-statistic and p-value on known datasets. BIC selects correct lag. FDR correction matches expected adjusted p-values.

### Wave 2: Scanner Integration + Evidence Accumulation

Integration layer: wires the statistical library into the intelligence cycle.

| Task | Description | Files | Dependencies |
|------|-------------|-------|-------------|
| T-11 | GrangerScanner orchestrator | `crates/ndp-intelligence/src/granger/scanner.rs` | T-01 through T-08 |
| T-12 | Candidate extraction from K-NN results | `crates/ndp-intelligence/src/granger/candidates.rs` | T-11 |
| T-13 | Time series extraction from gold aligned view | `crates/ndp-intelligence/src/granger/data.rs` | T-11 |
| T-14 | Candidate registry (UPSERT logic) | `crates/ndp-intelligence/src/granger/registry.rs` | T-09 |
| T-15 | Evidence accumulator | `crates/ndp-intelligence/src/granger/evidence.rs` | T-14 |
| T-16 | Candidate ranker | `crates/ndp-intelligence/src/granger/ranker.rs` | T-14 |
| T-17 | Integration into IntelligenceService.run_cycle() | `crates/ndp-intelligence/src/service.rs` | T-11 through T-16 |
| T-18 | CycleSummary extension | `crates/ndp-intelligence/src/service.rs` | T-17 |

**Exit criteria**: Scanner produces correct results on test data. Integration tests with mock database verify end-to-end flow. Evidence accumulator correctly tracks stability across simulated scans.

### Wave 3: Config + Deployment

Configuration and deployment: schema updates, feature flag, deploy.sh changes.

| Task | Description | Files | Dependencies |
|------|-------------|-------|-------------|
| T-19 | Domain config schema update | `config/schemas/domain.schema.json` | None |
| T-20 | GrangerConfig deserialization | `apps/ndp-intelligence-app/src/config.rs` | T-10, T-19 |
| T-21 | Feature flag (NDP_GRANGER_ENABLED) | `apps/ndp-intelligence-app/src/main.rs` | T-17 |
| T-22 | Deploy.sh causal_candidates DDL | `deploy/pi/deploy.sh` (Phase 6) | T-09 |
| T-23 | Integration domain.json update | `tests/integration/config/domains/indoor-air-quality/domain.json` | T-19 |
| T-24 | Module exports and lib.rs update | `crates/ndp-intelligence/src/lib.rs` | T-01 through T-18 |

**Exit criteria**: Config schema validates with granger block. Feature flag prevents all Granger execution when false. deploy.sh creates causal_candidates table. Integration domain config includes granger settings.

## Implementation Agent Assignment Guidance

| Agent | Waves | Components |
|-------|-------|-----------|
| ndp-rust-dev (statistical) | Wave 1 | T-01 through T-08: OLS, ADF, F-test, Toda-Yamamoto, BIC, FDR, stats |
| ndp-rust-dev (integration) | Wave 2 | T-11 through T-18: scanner, candidates, registry, evidence, cycle integration |
| ndp-rust-dev (DDL + config) | Wave 1 + Wave 3 | T-09, T-10, T-19 through T-24: DDL generator, config types, schema, deploy |

## Risk Items

1. **F-distribution p-value accuracy**: Implementing the incomplete beta function in pure Rust. The regularized incomplete beta function requires careful numerical computation. For the initial implementation, a series expansion with sufficient terms should be adequate for the range of F-statistics we expect (F < 100, df1 and df2 < 50).

2. **OLS numerical stability**: Small matrix inversions (typically 5x5 to 15x15) should be stable with f64 precision. Use QR decomposition rather than direct inverse if conditioning is poor.

3. **Toda-Yamamoto complexity**: Requires determining integration order (d_max) first, which needs multiple ADF tests. If d_max determination is unreliable, default to d_max=1 for environmental data (typically I(0) or I(1)).

4. **Gold aligned view column naming**: Streams in the aligned view use `{alias}_{field}` naming. The Granger scanner must parse these compound names correctly to identify source/target streams.
