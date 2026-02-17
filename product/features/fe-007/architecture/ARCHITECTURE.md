# fe-007 Architecture: Granger Causality (Statistical Validation)

## ADR-001: Granger Test Strategy Trait

### Context

The scope defines two test methods (classical F-test, Toda-Yamamoto) with extensibility for future Transfer Entropy. The similarity module in `crates/ndp-intelligence/src/similarity/` uses a trait-based design (`SimilarityEngine` trait with `PgVectorEngine` and `HnswEngine` implementations). Granger testing needs the same pluggability.

The key difference from similarity: Granger tests are stateless pure functions (no index to maintain), so the trait is simpler. Each test method takes two time series and a lag order, and returns test statistics.

### Decision

Define a `GrangerTest` trait in `crates/ndp-intelligence/src/granger/mod.rs`:

```rust
/// Result of a single Granger causality test.
pub struct TestResult {
    pub f_statistic: f64,
    pub p_value: f64,
    pub df1: usize,        // numerator degrees of freedom (lag order p)
    pub df2: usize,        // denominator degrees of freedom (n - 2p - 1)
    pub rss_restricted: f64,
    pub rss_unrestricted: f64,
}

/// Trait for Granger causality test implementations.
pub trait GrangerTest: Send + Sync {
    /// Test whether `source` Granger-causes `target` at the given lag.
    fn test(&self, source: &[f64], target: &[f64], lag: usize) -> Result<TestResult>;

    /// Human-readable name for this test method.
    fn name(&self) -> &str;
}
```

Two implementations:
- `ClassicalFTest` -- restricted vs unrestricted VAR model comparison via OLS
- `TodaYamamotoTest` -- augmented VAR with Wald test on source coefficients

Selection via domain config `test_method` field: `"classical"` (default) or `"toda_yamamoto"`.

### Consequences

- **Enables**: Drop-in addition of Transfer Entropy or other test methods by implementing `GrangerTest`
- **Follows**: Established project pattern (SimilarityEngine trait in same crate)
- **Cost**: One level of indirection for test dispatch -- negligible for compute-heavy statistical tests
- **Rules out**: Nothing -- trait is additive

---

## ADR-002: Stationarity Pipeline Design

### Context

Environmental sensor data (temperature, humidity, PM2.5) exhibits diurnal cycles, weather-regime shifts, and seasonal drift. Classical Granger causality assumes both time series are stationary (constant mean and variance). Applying Granger to non-stationary data produces spurious results.

The ADF (Augmented Dickey-Fuller) test detects non-stationarity by testing for a unit root. The test requires OLS regression, which we already need for the Granger F-test itself.

### Decision

Implement a three-stage stationarity pipeline in `crates/ndp-intelligence/src/granger/preprocessing.rs`:

```rust
pub enum PreprocessingMode {
    Raw,
    Difference,
    Seasonal,
}

pub struct PreprocessingResult {
    pub series: Vec<f64>,
    pub mode: PreprocessingMode,
    pub adf_statistic: f64,
    pub adf_p_value: f64,
}

/// Ensure stationarity through adaptive preprocessing.
pub fn ensure_stationary(
    series: &[f64],
    mode_override: Option<PreprocessingMode>,
    seasonal_period: usize, // 24 for hourly data
) -> Result<PreprocessingResult>
```

Pipeline:
1. If `mode_override` is set, apply that mode directly (skip ADF)
2. Otherwise, run ADF on raw data (5% significance)
3. If stationary -> return raw
4. If non-stationary -> first-difference, re-test
5. If still non-stationary -> seasonal-difference (period=24 for hourly)
6. Record which mode was applied

ADF implementation in `crates/ndp-intelligence/src/granger/adf.rs`:
- Test regression: delta_y(t) = alpha + beta*t + gamma*y(t-1) + sum(delta_i * delta_y(t-i)) + epsilon
- T-statistic on gamma coefficient
- Critical values: hardcoded MacKinnon (1996) table for 1%, 5%, 10% at sample sizes [25, 50, 100, 250, 500, inf]
- Interpolate between table entries for intermediate sample sizes
- ADF lag order: min(floor(12 * (n/100)^(1/4)), n/3) -- Schwert (1989) rule

### Consequences

- **Enables**: Reliable Granger testing on real-world sensor data without manual preprocessing
- **Cost**: Up to 3 ADF tests per series (raw, differenced, seasonal) -- ~1ms each at n=168
- **Trade-off**: Seasonal differencing loses 24 data points -- acceptable for n >= 48 (min_observations)
- **Interpretability**: Each causal candidate records its preprocessing mode, so users know what transformation was applied

---

## ADR-003: causal_candidates Table Schema

### Context

Granger results need persistent storage. The scope specifies a regular table (not hypertable, not continuous aggregate) because:
1. Row count is small (max ~300 per domain: 15 pairs * 2 directions * ~10 lags)
2. No time-series aggregation needed
3. UPSERT semantics required (update existing results on re-scan)

The table must be domain-scoped (multi-tenancy) and created by the existing Gold DDL pipeline.

Existing Gold DDL generators in `crates/ndp-lib/src/gold/generators/` follow a pattern: each generator implements a trait or function that produces SQL strings from domain config. Examples: `AlignedViewGenerator`, `EventsGenerator`, `ContinuousAggregateGenerator`.

### Decision

Create `crates/ndp-lib/src/gold/generators/causal_candidates.rs` with a `CausalCandidatesGenerator`:

```rust
pub struct CausalCandidatesGenerator;

impl CausalCandidatesGenerator {
    /// Generate DDL for the gold.causal_candidates table.
    /// Unlike other generators, this is a global table (not per-domain view)
    /// but with domain_id column for multi-tenancy.
    pub fn generate_ddl() -> String {
        // CREATE TABLE IF NOT EXISTS gold.causal_candidates (...)
        // CREATE INDEX IF NOT EXISTS ...
    }
}
```

Table schema (see SPECIFICATION.md FR-05 for full DDL):
- Primary key: BIGSERIAL id
- Unique constraint: (domain_id, source_stream, target_stream, lag_hours)
- Indexes: domain_id, (domain_id, is_significant) partial index
- Regular table, not hypertable -- no time_bucket, no continuous aggregates

DDL execution: via deploy.sh Phase 6 (Gold DDL) using `ndp gold` subcommand, similar to how intelligence DDL is generated.

### Consequences

- **Enables**: UPSERT-based evidence accumulation across scans
- **Follows**: Existing generator pattern in ndp-lib
- **Cost**: One new generator file, one new `ndp gold` subcommand or flag
- **Trade-off**: Global table (not per-domain) means all domains share the same table -- domain_id column provides isolation. This is consistent with `gold.predictions` and `gold.metric_embeddings`.

---

## ADR-004: Intelligence Cycle Integration

### Context

The existing intelligence cycle in `IntelligenceService::run_cycle()` (service.rs) follows this sequence:
1. OBSERVE (query gold rows)
2. WARMUP (running stats)
3. EMBED (generate embedding)
4. STORE (write to pgvector)
5. INDEX (HNSW insert)
6. SEARCH (K-NN similarity)
7. PREDICT (generate predictions)
8. EVALUATE (check outcomes)

Granger must be added as an optional step after SEARCH, running at a lower cadence than the hourly cycle. The scope specifies gating by `scan_interval_hours` (default 24h).

### Decision

Add step 6.5 "GRANGER" between SEARCH and PREDICT in `run_cycle()`:

```rust
// 6.5. GRANGER: validate causal relationships (gated by interval)
if self.granger_enabled && self.should_run_granger() {
    match self.run_granger_scan(&neighbors).await {
        Ok(granger_summary) => {
            summary.granger_pairs_tested = granger_summary.pairs_tested;
            summary.granger_significant = granger_summary.significant_count;
            self.last_granger_run = Some(Utc::now());
        }
        Err(e) => {
            warn!("Granger scan failed (non-fatal): {}", e);
        }
    }
}
```

New fields on `IntelligenceService`:
- `granger_enabled: bool` -- from NDP_GRANGER_ENABLED env var
- `last_granger_run: Option<DateTime<Utc>>` -- in-memory timestamp
- `granger_config: Option<GrangerConfig>` -- from domain config

`should_run_granger()` checks:
1. `granger_enabled` is true
2. Domain config has `intelligence.granger` block
3. `last_granger_run` is None OR elapsed time >= `scan_interval_hours`

Granger failures are non-fatal (WARN log) -- the rest of the cycle continues.

### Consequences

- **Enables**: Granger runs in the existing cycle with no separate scheduler
- **Follows**: Integration-first mandate (extend existing code path)
- **Cost**: Additional ~5-30s every `scan_interval_hours` (default daily)
- **Risk**: If Granger takes >30s, it blocks the current cycle. Mitigated by the 30s budget constraint and by running daily, not hourly.
- **Trade-off**: `last_granger_run` is in-memory -- lost on restart. On restart, Granger will run on the next cycle. Acceptable because Granger is idempotent (UPSERT).

---

## ADR-005: Feature Flag Mechanism

### Context

The scope explicitly requires a deployment-level feature flag (`NDP_GRANGER_ENABLED` env var), NOT domain config. This is because:
1. Granger is computationally expensive -- operators need a kill switch
2. Domain config hot-reload could accidentally enable Granger mid-cycle
3. Feature flags should be deployment-level for staged rollout (test on one Pi, then enable globally)

The existing intelligence app reads env vars in `AppConfig::from_env()` (service.rs).

### Decision

Add `granger_enabled` to `AppConfig`:

```rust
// In AppConfig::from_env():
let granger_enabled = std::env::var("NDP_GRANGER_ENABLED")
    .map(|v| v == "true" || v == "1")
    .unwrap_or(false);
```

Pass to `IntelligenceService::new()`:

```rust
pub async fn new(
    app_config: &AppConfig,
    intelligence_config: &IntelligenceConfig,
    objectives: Vec<ObjectiveMetric>,
    pool: Arc<Pool>,
    storage: Arc<dyn StorageBackend>,
    primary_alias: String,
) -> Result<Self> {
    // ... existing init ...
    Ok(Self {
        // ... existing fields ...
        granger_enabled: app_config.granger_enabled,
        last_granger_run: None,
        granger_config: intelligence_config.granger.clone(),
    })
}
```

When `granger_enabled` is false, the `run_cycle()` method skips the Granger block entirely -- no config loading, no database queries, no memory allocation for Granger structures.

### Consequences

- **Enables**: Zero-overhead disable -- operators can deploy the code without enabling Granger
- **Follows**: Existing AppConfig pattern (same as INTELLIGENCE_POLL_INTERVAL_SECS)
- **Cost**: One env var read at startup
- **Rules out**: Dynamic enable/disable without restart -- acceptable for a feature flag

---

## ADR-006: Domain Config Schema Update

### Context

The domain config schema at `config/schemas/domain.schema.json` has `additionalProperties: false` on the `intelligence` definition. This was a v1.2.3 lesson -- adding new fields to intelligence requires schema update.

Current intelligence definition has: `enabled`, `embedding`, `search`, `anomaly`. We need to add `granger`.

### Decision

Add `granger` to the intelligence definition in domain.schema.json:

```json
{
  "intelligence": {
    "properties": {
      "granger": {
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "candidate_count": { "type": "integer", "minimum": 1, "default": 10 },
          "lag_hours": {
            "type": "array",
            "items": { "type": "integer", "minimum": 1 },
            "minItems": 1,
            "default": [1, 2, 4]
          },
          "significance_level": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.05 },
          "test_method": { "type": "string", "enum": ["classical", "toda_yamamoto"], "default": "classical" },
          "preprocessing": { "type": "string", "enum": ["adaptive", "raw", "difference", "seasonal"], "default": "adaptive" },
          "evidence_window_days": { "type": "integer", "minimum": 1, "default": 7 },
          "scan_interval_hours": { "type": "integer", "minimum": 1, "default": 24 },
          "min_observations": { "type": "integer", "minimum": 10, "default": 48 }
        }
      }
    }
  }
}
```

Also update the `IntelligenceConfig` Rust struct in `crates/ndp-lib/src/gold/embeddings/config.rs`:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct IntelligenceConfig {
    pub enabled: bool,
    pub embedding: EmbeddingConfig,
    pub search: SearchConfig,
    #[serde(default)]
    pub anomaly: Option<AnomalyConfig>,
    #[serde(default)]
    pub granger: Option<GrangerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GrangerConfig {
    #[serde(default = "default_candidate_count")]
    pub candidate_count: usize,
    #[serde(default = "default_lag_hours")]
    pub lag_hours: Vec<u32>,
    #[serde(default = "default_significance")]
    pub significance_level: f64,
    #[serde(default = "default_test_method")]
    pub test_method: String,
    #[serde(default = "default_preprocessing")]
    pub preprocessing: String,
    #[serde(default = "default_evidence_window")]
    pub evidence_window_days: u32,
    #[serde(default = "default_scan_interval")]
    pub scan_interval_hours: u32,
    #[serde(default = "default_min_observations")]
    pub min_observations: usize,
}
```

### Consequences

- **Enables**: Hot-reload of Granger parameters without code changes
- **Follows**: Established pattern (embedding, search, anomaly already in intelligence block)
- **Cost**: Schema migration -- existing domain configs without `granger` block are valid (it is optional)
- **Requires**: Update integration test domain.json to include granger block

---

## ADR-007: BIC Lag Selection

### Context

The scope defines lag optimization via BIC (Bayesian Information Criterion). For each candidate pair, we test multiple lags from the domain config (default [1, 2, 4] hours) and select the lag that best balances model fit vs complexity.

BIC penalizes model complexity more heavily than AIC, making it appropriate for small-n settings (typical n=168 for 7 days of hourly data).

### Decision

Implement BIC computation in `crates/ndp-intelligence/src/granger/lag_selection.rs`:

```rust
/// Compute BIC for a VAR model.
/// BIC = n * ln(RSS/n) + k * ln(n)
/// where k = number of estimated parameters, n = number of observations
pub fn compute_bic(rss: f64, n: usize, k: usize) -> f64 {
    let n_f = n as f64;
    n_f * (rss / n_f).ln() + (k as f64) * n_f.ln()
}

/// Select optimal lag from candidates using BIC.
pub fn select_optimal_lag(
    source: &[f64],
    target: &[f64],
    test: &dyn GrangerTest,
    lag_candidates: &[u32],
) -> Result<LagSelectionResult> {
    // For each lag: run Granger test, compute BIC, track minimum
    // Return optimal lag with all results for storage
}

pub struct LagSelectionResult {
    pub optimal_lag: u32,
    pub optimal_bic: f64,
    pub all_results: Vec<LagResult>,
}

pub struct LagResult {
    pub lag: u32,
    pub bic: f64,
    pub test_result: TestResult,
}
```

The unrestricted model has k = 2p + 1 parameters (p source lags + p target lags + intercept). The restricted model has k = p + 1 parameters (p target lags + intercept). BIC is computed on the unrestricted model for model selection.

### Consequences

- **Enables**: Automated optimal lag detection without user intervention
- **Cost**: Runs GrangerTest once per candidate lag (3 tests per pair with default [1,2,4])
- **Trade-off**: BIC may over-penalize for very small n. For n=48 (minimum), AIC might be better. We use BIC because it is consistent (selects true model as n->inf) and our typical n=168 is sufficient.

---

## ADR-008: Multiple Comparison Correction

### Context

When testing 20 pairs (10 K-NN candidates * 2 directions), the family-wise error rate inflates: at alpha=0.05, we expect 1 false positive by chance. Benjamini-Hochberg FDR (False Discovery Rate) controls the expected proportion of false discoveries among rejected hypotheses, which is more appropriate than Bonferroni for exploratory analysis.

### Decision

Implement BH-FDR in `crates/ndp-intelligence/src/granger/fdr.rs`:

```rust
/// Apply Benjamini-Hochberg FDR correction to a set of p-values.
/// Returns adjusted p-values in the same order as input.
pub fn benjamini_hochberg(p_values: &[f64], alpha: f64) -> Vec<AdjustedPValue> {
    let m = p_values.len();
    // 1. Create index-value pairs, sort by p-value ascending
    // 2. For rank i (1-indexed): adjusted_p = p * m / i
    // 3. Enforce monotonicity from bottom up: adjusted_p[i] = min(adjusted_p[i], adjusted_p[i+1])
    // 4. Cap at 1.0
    // 5. Return in original order
}

pub struct AdjustedPValue {
    pub original_index: usize,
    pub p_value: f64,
    pub p_value_adjusted: f64,
    pub is_significant: bool,
}
```

FDR correction is applied once per scan across all pairs tested in that scan. Both raw and adjusted p-values are stored in `gold.causal_candidates`.

### Consequences

- **Enables**: Controlled false discovery rate across multiple comparisons
- **Follows**: Standard statistical practice for exploratory hypothesis testing
- **Cost**: O(m log m) for sorting -- negligible for m <= 20
- **Trade-off**: BH-FDR is less conservative than Bonferroni (more discoveries, higher FDR). This is appropriate because we want to identify potential causal relationships for further validation, not make definitive causal claims.

---

## Integration Surface

### Crate Dependencies

```
ndp-intelligence (new granger module)
  <- ndarray (already in workspace)
  <- ndp-lib::gold::embeddings::config::GrangerConfig (new type)

ndp-intelligence-app
  <- ndp-intelligence::granger (new module)
  <- NDP_GRANGER_ENABLED env var (new)

ndp-lib (gold generators)
  <- CausalCandidatesGenerator (new generator)

ndp-cli
  <- ndp-lib::gold::generators (CausalCandidatesGenerator, invoked via ndp gold subcommand)
```

### Database Schema

- New table: `gold.causal_candidates` (regular table, not hypertable)
- No changes to existing tables (`gold.metric_embeddings`, `gold.predictions`)

### Config Schema

- `config/schemas/domain.schema.json`: add `granger` to intelligence definition
- `tests/integration/config/domains/indoor-air-quality/domain.json`: add granger block

### Deployment

- `deploy/pi/deploy.sh`: Phase 6 creates causal_candidates table via `ndp gold` subcommand
- Docker compose: no new containers (Granger runs inside ndp-intelligence-app)
- New env var: `NDP_GRANGER_ENABLED=false` in compose file (default disabled)
