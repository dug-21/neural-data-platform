# Implementation Brief: fe-007 Granger Causality (Statistical Validation)

## Goal

Add Granger causality testing to the ndp-intelligence crate to statistically validate whether K-NN-discovered correlations represent genuine lead-lag relationships. The Granger scanner runs inside the existing intelligence cycle at a configurable cadence (default daily), tests top K-NN candidate pairs bidirectionally using classical F-test or Toda-Yamamoto methods, applies Benjamini-Hochberg FDR correction, and stores validated relationships in a new `gold.causal_candidates` table with rolling evidence accumulation. Feature-flagged via `NDP_GRANGER_ENABLED` env var (default: false) for staged rollout.

## SPARC Artifact Links

| Artifact | Path |
|----------|------|
| Scope | product/features/fe-007/SCOPE.md |
| Specification | product/features/fe-007/specification/SPECIFICATION.md |
| Task Decomposition | product/features/fe-007/specification/TASK-DECOMPOSITION.md |
| Architecture (ADRs) | product/features/fe-007/architecture/ARCHITECTURE.md |
| Pseudocode Overview | product/features/fe-007/pseudocode/OVERVIEW.md |
| Pseudocode: ndp-intelligence | product/features/fe-007/pseudocode/ndp-intelligence.md |
| Pseudocode: ndp-intelligence-app | product/features/fe-007/pseudocode/ndp-intelligence-app.md |
| Pseudocode: ndp-cli | product/features/fe-007/pseudocode/ndp-cli.md |
| Pseudocode: domain-config | product/features/fe-007/pseudocode/domain-config.md |
| Test Plan Overview | product/features/fe-007/test-plan/OVERVIEW.md |
| Test Plan: ndp-intelligence | product/features/fe-007/test-plan/ndp-intelligence.md |
| Test Plan: ndp-intelligence-app | product/features/fe-007/test-plan/ndp-intelligence-app.md |
| Test Plan: ndp-cli | product/features/fe-007/test-plan/ndp-cli.md |
| Test Plan: domain-config | product/features/fe-007/test-plan/domain-config.md |
| Alignment Report | product/features/fe-007/ALIGNMENT-REPORT.md |
| Acceptance Map | product/features/fe-007/ACCEPTANCE-MAP.md |

## Component Map

| Component | Pseudocode | Test Plan |
|-----------|-----------|-----------|
| ndp-intelligence | pseudocode/ndp-intelligence.md | test-plan/ndp-intelligence.md |
| ndp-intelligence-app | pseudocode/ndp-intelligence-app.md | test-plan/ndp-intelligence-app.md |
| ndp-cli | pseudocode/ndp-cli.md | test-plan/ndp-cli.md |
| domain-config | pseudocode/domain-config.md | test-plan/domain-config.md |

## Resolved Decisions

| Decision | Resolution | Source | Pattern ID |
|----------|-----------|--------|-----------|
| Test method pluggability | GrangerTest trait with ClassicalFTest + TodaYamamotoTest impls | ADR-001 | 34 |
| Stationarity handling | 3-stage adaptive pipeline: ADF -> difference -> seasonal | ADR-002 | 35 |
| Table schema | Regular table gold.causal_candidates, UNIQUE(domain_id, source, target, lag) | ADR-003 | 36 |
| Cycle integration | Step 6.5 in run_cycle(), gated by env var + interval | ADR-004 | 37 |
| Feature flag | NDP_GRANGER_ENABLED env var, default false, zero overhead | ADR-005 | 38 |
| Config schema | Add granger to intelligence definition, GrangerConfig struct | ADR-006 | 39 |
| Lag selection | BIC minimization across candidate lags | ADR-007 | 40 |
| Multiple comparisons | Benjamini-Hochberg FDR, store raw + adjusted p-values | ADR-008 | 41 |

## GitHub Issue

https://github.com/dug-21/neural-data-platform/issues/38

## Files to Create/Modify

### New Files

| File | Description |
|------|-------------|
| `crates/ndp-intelligence/src/granger/mod.rs` | GrangerTest trait, TestResult, GrangerError, module re-exports |
| `crates/ndp-intelligence/src/granger/ols.rs` | OLS regression engine (pure ndarray) |
| `crates/ndp-intelligence/src/granger/stats.rs` | F-distribution p-value via incomplete beta function |
| `crates/ndp-intelligence/src/granger/adf.rs` | ADF stationarity test with MacKinnon critical values |
| `crates/ndp-intelligence/src/granger/preprocessing.rs` | Adaptive stationarity preprocessing pipeline |
| `crates/ndp-intelligence/src/granger/classical.rs` | Classical F-test Granger implementation |
| `crates/ndp-intelligence/src/granger/toda_yamamoto.rs` | Toda-Yamamoto Granger implementation |
| `crates/ndp-intelligence/src/granger/lag_selection.rs` | BIC-based lag optimizer |
| `crates/ndp-intelligence/src/granger/fdr.rs` | Benjamini-Hochberg FDR correction |
| `crates/ndp-intelligence/src/granger/candidates.rs` | Candidate pair extraction from K-NN results |
| `crates/ndp-intelligence/src/granger/data.rs` | Time series extraction from gold aligned view |
| `crates/ndp-intelligence/src/granger/registry.rs` | UPSERT logic for gold.causal_candidates |
| `crates/ndp-intelligence/src/granger/evidence.rs` | Rolling window evidence accumulator |
| `crates/ndp-intelligence/src/granger/ranker.rs` | Composite score candidate ranker |
| `crates/ndp-intelligence/src/granger/scanner.rs` | GrangerScanner orchestrator |
| `crates/ndp-lib/src/gold/generators/causal_candidates.rs` | DDL generator for gold.causal_candidates |

### Modified Files

| File | Change |
|------|--------|
| `crates/ndp-intelligence/src/lib.rs` | Add `pub mod granger;` |
| `crates/ndp-intelligence/src/service.rs` | Add granger fields to IntelligenceService, step 6.5 in run_cycle(), extend CycleSummary |
| `crates/ndp-lib/src/gold/embeddings/config.rs` | Add GrangerConfig struct, add granger: Option<GrangerConfig> to IntelligenceConfig |
| `crates/ndp-lib/src/gold/generators/mod.rs` | Add `pub mod causal_candidates;` |
| `tools/ndp-cli/src/gold.rs` | Add causal-candidates subcommand invoking CausalCandidatesGenerator |
| `apps/ndp-intelligence-app/src/main.rs` | Add granger_enabled to AppConfig::from_env() |
| `config/schemas/domain.schema.json` | Add granger to intelligence properties |
| `config/integration/domains/indoor-air-quality/domain.json` | Add granger block to intelligence |

## Data Structures

```rust
// GrangerTest trait (crates/ndp-intelligence/src/granger/mod.rs)
pub struct TestResult {
    pub f_statistic: f64,
    pub p_value: f64,
    pub df1: usize,
    pub df2: usize,
    pub rss_restricted: f64,
    pub rss_unrestricted: f64,
}

pub trait GrangerTest: Send + Sync {
    fn test(&self, source: &[f64], target: &[f64], lag: usize) -> Result<TestResult>;
    fn name(&self) -> &str;
}

// GrangerConfig (crates/ndp-lib/src/gold/embeddings/config.rs)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrangerConfig {
    pub candidate_count: usize,      // default 10
    pub lag_hours: Vec<u32>,          // default [1, 2, 4]
    pub significance_level: f64,      // default 0.05
    pub test_method: String,          // "classical" | "toda_yamamoto"
    pub preprocessing: String,        // "adaptive" | "raw" | "difference" | "seasonal"
    pub evidence_window_days: u32,    // default 7
    pub scan_interval_hours: u32,     // default 24
    pub min_observations: usize,      // default 48
}

// GrangerResult (crates/ndp-intelligence/src/granger/mod.rs)
pub struct GrangerResult {
    pub source_stream: String,
    pub target_stream: String,
    pub test_method: String,
    pub optimal_lag: u32,
    pub f_statistic: f64,
    pub p_value: f64,
    pub p_value_adjusted: Option<f64>,
    pub is_significant: bool,
    pub bic: f64,
    pub preprocessing: PreprocessingMode,
    pub all_lag_results: Vec<LagResult>,
}

// Extended CycleSummary (crates/ndp-intelligence/src/service.rs)
pub struct CycleSummary {
    // ... existing fields ...
    pub granger_pairs_tested: usize,
    pub granger_significant: usize,
}
```

## Function Signatures

```rust
// OLS
fn ols_fit(x: &Array2<f64>, y: &ArrayView1<f64>) -> Result<OlsResult>;
fn build_var_matrices(source: &[f64], target: &[f64], lag: usize, restricted: bool) -> (Array2<f64>, Array1<f64>);

// Statistics
fn f_distribution_p_value(f_stat: f64, df1: usize, df2: usize) -> f64;
fn regularized_incomplete_beta(x: f64, a: f64, b: f64) -> f64;

// ADF
fn adf_test(series: &[f64], max_lag: Option<usize>, significance: f64) -> Result<AdfResult>;

// Preprocessing
fn ensure_stationary(series: &[f64], mode_override: Option<&str>, significance: f64, seasonal_period: usize) -> Result<PreprocessingResult>;

// Lag selection
fn select_optimal_lag(source: &[f64], target: &[f64], test: &dyn GrangerTest, lag_candidates: &[u32]) -> Result<LagSelectionResult>;
fn compute_bic(rss: f64, n: usize, k: usize) -> f64;

// FDR
fn benjamini_hochberg(p_values: &[f64], alpha: f64) -> Vec<AdjustedPValue>;

// Scanner
impl GrangerScanner {
    fn new(pool: Arc<Pool>, config: &GrangerConfig) -> Self;
    async fn run_scan(&self, domain_id: &str, candidates: &[CandidatePair], view_name: &str) -> Result<ScanSummary>;
}

// DDL
impl CausalCandidatesGenerator {
    fn generate_ddl() -> String;
    async fn table_exists(client: &dyn DbClient) -> Result<bool>;
}
```

## Test Expectations

- **Unit tests**: ~58 new tests across granger module (OLS: 6, stats: 7, ADF: 6, preprocessing: 7, classical: 6, toda_yamamoto: 4, BIC: 5, FDR: 8, scanner: 3, candidates: 3, domain-config: 5, DDL: 3)
- **Integration tests**: ~12 new tests (cycle integration: 8, DDL: 2, config: 2)
- **Total**: ~70 new tests
- **Existing tests**: Must not regress (platform-core 908, ndp-intelligence 61, ndp-lib 606)

## Constraints

- Pure Rust + ndarray only -- no external stats libraries (no statrs, no linfa)
- ARM64 Pi compatible (aarch64-unknown-linux-gnu)
- Computation budget: Granger scan < 30s, < 50MB additional memory
- Feature flag NDP_GRANGER_ENABLED=false means zero overhead
- Domain config `additionalProperties: false` requires schema update
- No dependency on fe-005/fe-006 -- works with metric-only K-NN results
- Must handle missing data gracefully (NWS observations with null fields)

## Dependencies

- `ndarray` (already in workspace)
- No new crate dependencies

## NOT in Scope

- PC algorithm / full causal graph discovery
- Cross-domain causal analysis
- Transfer Entropy test method (future via GrangerTest trait)
- Dashboards for causal relationships (fe-008)
- SONA learning from causal relationships (fe-009)
- Cleanup of stale causal_candidates entries

## Wave Structure

| Wave | Tasks | Agent Assignment | Exit Criteria |
|------|-------|-----------------|---------------|
| Wave 1 | T-01 to T-10: Core statistical library + DDL generator + config types | ndp-rust-dev (statistical) + ndp-rust-dev (DDL/config) | All unit tests pass, OLS/ADF/F-test/BIC/FDR correct on synthetic data |
| Wave 2 | T-11 to T-18: Scanner integration + evidence accumulation + cycle integration | ndp-rust-dev (integration) | Scanner produces correct results, integration tests verify end-to-end |
| Wave 3 | T-19 to T-24: Config schema + feature flag + deploy.sh + integration domain.json | ndp-rust-dev (config/deploy) | Schema validates, flag works, deploy.sh creates table |

## Alignment Status

**PASS** -- All 7 alignment principles satisfied. One WARN on version targeting (v1.2.x vs roadmap v1.3 for causal validation). Recommendation: accept as v1.2.x because feature is discovery validation (extends v1.2 K-NN), not prediction/action, and is feature-flagged. See ALIGNMENT-REPORT.md.
