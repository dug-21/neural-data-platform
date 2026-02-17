# fe-007 Specification: Granger Causality (Statistical Validation)

## Problem Statement

K-NN similarity search (fe-004) discovers correlations between streams by finding periods where environmental states resemble each other. However, correlation does not imply causation or even directionality. Two streams may be similar simply because both follow the same diurnal cycle, or because a confounding third variable drives both.

Granger causality testing determines whether one stream's past values improve prediction of another stream's future values beyond what the target stream's own history provides. This separates genuine lead-lag relationships ("outdoor humidity Granger-causes indoor PM2.5 with 2-hour lag") from coincidental correlation. The output is a curated registry of statistically validated causal candidates that downstream prediction and action systems can rely on with quantified confidence.

## Functional Requirements

### FR-01: Feature Flag (G-01)

The `NDP_GRANGER_ENABLED` environment variable gates all Granger functionality at the deployment level:

- Default: `false` (disabled)
- When `false`: the intelligence cycle skips the entire Granger step -- zero database queries, zero computation, zero memory allocation beyond the flag check
- When `true`: the Granger scanner runs according to `scan_interval_hours` gating
- The flag is read once at startup from `std::env::var("NDP_GRANGER_ENABLED")`
- Stored as a field on `IntelligenceService` or passed to `run_cycle()` context
- NOT in domain config -- this is a deployment-level kill switch

### FR-02: Similarity-Guided Candidate Selection (G-02)

Granger tests only candidate pairs identified by K-NN similarity search:

- After each K-NN search cycle, extract the top `candidate_count` pairs (default 10) from similarity results
- Each pair is tested bidirectionally: A->B and B->A (20 tests total for 10 pairs)
- Candidate pairs are identified by stream alias (e.g., "indoor", "outdoor", "nws") and field name (e.g., "pm25_mean", "humidity_mean")
- Pairs are extracted from the gold aligned view columns, which use `{alias}_{field}` naming
- If K-NN returns fewer than `candidate_count` pairs, test all available pairs

### FR-03: Granger Causality Scanner (G-03)

A pairwise Granger causality test engine, pure Rust with ndarray:

- **Input**: Two time series (source, target) as `Vec<f64>` extracted from the gold aligned view
- **Output**: `GrangerResult` containing f_statistic, p_value, optimal_lag, test_method used, preprocessing applied
- **Test trait**: `GrangerTest` trait with `fn test(&self, source: &[f64], target: &[f64], lag: usize) -> Result<TestResult>` -- implementations for classical F-test and Toda-Yamamoto
- **Classical F-test**: Compare RSS of restricted model (target ~ target_lags) vs unrestricted model (target ~ target_lags + source_lags). F = ((RSS_r - RSS_u) / p) / (RSS_u / (n - 2p - 1))
- **Toda-Yamamoto**: Augment VAR by d_max (integration order) extra lags, then Wald test on source coefficients only. Works without requiring stationarity.
- **OLS implementation**: Pure ndarray matrix operations: beta = (X'X)^(-1) X'y using LU decomposition or QR from ndarray-linalg (already in workspace via ndarray)
- **P-value from F-distribution**: Implement the regularized incomplete beta function for F-distribution CDF. No external stats library.

### FR-04: Lag Optimizer (G-04)

For each validated causal relationship, find the optimal lag:

- Test all lags from domain config `lag_hours` (default: [1, 2, 4])
- For each lag, run Granger test and compute BIC = n * ln(RSS/n) + k * ln(n) where k = number of parameters
- Select the lag that minimizes BIC
- Store results for ALL tested lags (not just the optimal) for interpretability
- If no lag produces a significant result, the pair is not a causal candidate

### FR-05: Candidate Registry (G-05)

`gold.causal_candidates` table -- regular table (NOT hypertable), domain-scoped:

```sql
CREATE TABLE IF NOT EXISTS gold.causal_candidates (
    id              BIGSERIAL PRIMARY KEY,
    domain_id       TEXT NOT NULL,
    source_stream   TEXT NOT NULL,   -- e.g., "outdoor_humidity_mean"
    target_stream   TEXT NOT NULL,   -- e.g., "indoor_pm25_mean"
    test_method     TEXT NOT NULL,   -- "classical" or "toda_yamamoto"
    lag_hours       INTEGER NOT NULL,
    f_statistic     DOUBLE PRECISION NOT NULL,
    p_value         DOUBLE PRECISION NOT NULL,
    p_value_adjusted DOUBLE PRECISION,  -- after FDR correction
    is_significant  BOOLEAN NOT NULL,
    bic             DOUBLE PRECISION,
    preprocessing   TEXT NOT NULL,   -- "raw", "difference", "seasonal"
    evidence_count  INTEGER NOT NULL DEFAULT 1,
    stability_score DOUBLE PRECISION,
    first_seen      TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen       TIMESTAMPTZ NOT NULL DEFAULT now(),
    scan_window_start TIMESTAMPTZ,
    scan_window_end   TIMESTAMPTZ,
    metadata        JSONB DEFAULT '{}'::jsonb,
    UNIQUE (domain_id, source_stream, target_stream, lag_hours)
);

CREATE INDEX idx_causal_candidates_domain ON gold.causal_candidates (domain_id);
CREATE INDEX idx_causal_candidates_significant ON gold.causal_candidates (domain_id, is_significant) WHERE is_significant = true;
```

- DDL generated by ndp-lib (new `CausalCandidatesGenerator`), deployed via ndp-cli `ndp gold` subcommand
- UPSERT on (domain_id, source_stream, target_stream, lag_hours) -- updates f_statistic, p_value, evidence_count, last_seen, stability_score on conflict
- Domain-scoped: each domain has its own causal candidates

### FR-06: Candidate Ranker (G-06)

Rank validated candidates by a composite score:

- Score = -log10(p_value_adjusted) * relevance_weight
- `relevance_weight`: 1.0 if either source or target is a field referenced by a domain objective, 0.5 otherwise
- Ranking is computed after each scan and stored in `metadata` JSON
- Top-ranked candidates are the ones most likely to be useful for prediction

### FR-07: Evidence Accumulator (G-07)

Track relationship stability across multiple Granger scans:

- Rolling window: default 7 days, configurable via `evidence_window_days`
- `evidence_count`: number of scans where this relationship was significant
- `stability_score`: evidence_count / total_scans_in_window (0.0 to 1.0)
- On each scan: increment evidence_count if significant, update stability_score
- Relationships that drop below significance in a scan: do NOT remove from table, but mark `is_significant = false` for that scan and let stability_score decay
- Old entries (not seen within 2x evidence_window_days) can be cleaned by a future maintenance task (out of scope)

## Stationarity Pipeline

### FR-08: ADF Stationarity Test

Augmented Dickey-Fuller test for unit root detection:

- Test model: delta_y(t) = alpha + beta*t + gamma*y(t-1) + sum(delta_i * delta_y(t-i)) + epsilon
- Null hypothesis: gamma = 0 (unit root, non-stationary)
- Test statistic: t-statistic on gamma coefficient
- Critical values: hardcoded lookup table (MacKinnon 1996) for sample sizes 25, 50, 100, 250, 500, inf at 1%, 5%, 10% significance
- Lag selection for ADF: use AIC on the delta_y regression (separate from Granger lag selection)
- Pure ndarray OLS implementation

### FR-09: Adaptive Preprocessing

When ADF rejects stationarity:

1. Test raw data with ADF
2. If stationary -> use raw values, record `preprocessing: "raw"`
3. If non-stationary -> first-difference: delta_x(t) = x(t) - x(t-1), re-test with ADF
4. If still non-stationary -> seasonal-difference: x(t) - x(t-24) for hourly data
5. Record which preprocessing was applied per pair
6. Domain-configurable mode override: `preprocessing` field in config ("adaptive", "raw", "difference", "seasonal")

### FR-10: Multiple Comparison Correction

Benjamini-Hochberg FDR correction:

1. Collect all p-values from a single scan (up to 20 bidirectional tests)
2. Sort p-values in ascending order
3. For rank i out of m tests: adjusted_p = p * m / i
4. Enforce monotonicity: adjusted_p[i] = min(adjusted_p[i], adjusted_p[i+1])
5. Significance: adjusted_p < significance_level (default 0.05)
6. Store both raw p_value and p_value_adjusted

## Integration

### FR-11: Intelligence Cycle Gating

Granger runs inside the existing intelligence cycle at a lower cadence:

1. `gold_refresh` NOTIFY fires (hourly) -> existing cycle: embeddings -> K-NN -> predictions
2. After predictions step: check `NDP_GRANGER_ENABLED`
3. If disabled: skip entirely
4. If enabled: check `last_granger_run` timestamp against `scan_interval_hours` (default 24h)
5. If interval not elapsed: skip
6. If interval elapsed: run Granger scanner on top-K K-NN candidates
7. Write results to `gold.causal_candidates` via UPSERT
8. Update `last_granger_run` timestamp
9. Validated Granger relationships feed `gold.predictions` (a confirmed causal link IS a prediction signal)

### FR-12: Domain Configuration

Configuration under `intelligence.granger` in domain config:

```json
{
  "intelligence": {
    "granger": {
      "candidate_count": 10,
      "lag_hours": [1, 2, 4],
      "significance_level": 0.05,
      "test_method": "classical",
      "preprocessing": "adaptive",
      "evidence_window_days": 7,
      "scan_interval_hours": 24,
      "min_observations": 48
    }
  }
}
```

- Hot-reload via config-client etcd watch
- Requires domain schema update (add `granger` to intelligence definition)
- `additionalProperties: false` on intelligence block must be updated to allow `granger`
- Integration domain config at `config/integration/domains/indoor-air-quality/domain.json`

### FR-13: Minimum Observations Guard

- 48 data points (2 days of hourly data) required before Granger will attempt a test on any pair
- Configurable via `min_observations`
- If insufficient data for a pair, skip that pair (do not fail the scan)
- Log at WARN level when pairs are skipped due to insufficient data

## Non-Functional Requirements

### NFR-01: Performance Budget
- Granger scan must complete within 30 seconds for current stream count (~6 streams, 15 unique pairs)
- Memory: <50MB additional allocation during Granger scan
- OLS operations use stack-allocated arrays where possible (ndarray `Array2<f64>` with small dimensions)

### NFR-02: ARM64 Compatibility
- Pure Rust + ndarray -- no C dependencies that might fail on ARM64
- ndarray already compiles for aarch64-unknown-linux-gnu in the workspace

### NFR-03: Graceful Degradation
- Missing data (NWS observations with null temp): skip that time point, reduce effective n
- If effective n < min_observations after null filtering: skip pair
- If OLS matrix is singular (perfect multicollinearity): report as inconclusive, do not crash
- If F-statistic computation yields NaN/Inf: skip pair with WARN log

### NFR-04: Observability
- Log at INFO: scan start/end, number of pairs tested, significant results found
- Log at DEBUG: per-pair test results (f_stat, p_value, lag, preprocessing)
- Log at WARN: pairs skipped (insufficient data, singular matrix)
- CycleSummary extended with granger_pairs_tested, granger_significant fields

## Out of Scope

- PC algorithm / full causal graph discovery
- Cross-domain causal analysis
- Transfer Entropy test method (future extension via strategy trait)
- Dashboards for causal relationships (fe-008)
- SONA learning from causal relationships (fe-009)
- Cleanup of stale causal_candidates entries
