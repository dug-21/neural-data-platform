# fe-007 Test Plan: ndp-intelligence (Granger Module)

## Unit Tests

### OLS Regression (ols.rs)

| Test | Input | Expected | Assertion |
|------|-------|----------|-----------|
| `test_ols_simple_linear` | X=[1,2,3,4,5], y=[2.1,4.0,6.1,8.0,10.1] | beta ~= [0.05, 2.0] | Coefficients within 0.1 of true values |
| `test_ols_perfect_fit` | X, y on exact line y=3x+1 | RSS = 0.0, R^2 = 1.0 | RSS < 1e-10, R^2 > 0.9999 |
| `test_ols_multivariate` | X with 3 columns, known beta | Correct coefficients | Each coefficient within 0.05 of true |
| `test_ols_singular_matrix` | X with duplicate columns | GrangerError::SingularMatrix | Returns error, does not panic |
| `test_ols_single_observation` | n=1 | Error (insufficient data) | Returns InsufficientData |
| `test_ols_residuals_sum_zero` | Any well-conditioned X, y | sum(residuals) ~= 0 | Absolute sum < 1e-10 |

### F-distribution P-value (stats.rs)

| Test | Input | Expected | Assertion |
|------|-------|----------|-----------|
| `test_f_pvalue_known_values` | F=1.0, df1=2, df2=10 | p ~= 0.4019 | Within 0.001 of reference |
| `test_f_pvalue_large_f` | F=100.0, df1=5, df2=50 | p ~= 0.0 | p < 0.0001 |
| `test_f_pvalue_zero` | F=0.0, df1=3, df2=20 | p = 1.0 | Exact |
| `test_f_pvalue_negative` | F=-1.0 | p = 1.0 (or NaN) | Does not panic |
| `test_f_pvalue_symmetry` | Various F, df pairs | P(F>x) + P(F<=x) = 1 | Sum within 1e-6 of 1.0 |
| `test_incomplete_beta_boundary` | x=0, x=1 | 0.0, 1.0 respectively | Exact |
| `test_ln_gamma_known` | gamma(5) = 24 | ln(24) ~= 3.178 | Within 1e-8 |

### ADF Test (adf.rs)

| Test | Input | Expected | Assertion |
|------|-------|----------|-----------|
| `test_adf_stationary_series` | White noise (seeded) | is_stationary = true | Rejects null (unit root) |
| `test_adf_random_walk` | Cumulative sum of white noise | is_stationary = false | Fails to reject null |
| `test_adf_trend_stationary` | Linear trend + noise | is_stationary = true | Detects trend-stationarity |
| `test_adf_insufficient_data` | n=10 | Error | Returns InsufficientData |
| `test_adf_lag_selection` | n=200, known AR(1) | lags_used reasonable | lags_used > 0 and < n/3 |
| `test_adf_critical_values` | Known test statistic | Correct significance | Matches MacKinnon table |

### Preprocessing (preprocessing.rs)

| Test | Input | Expected | Assertion |
|------|-------|----------|-----------|
| `test_preprocessing_raw_stationary` | Stationary series | mode = Raw, series unchanged | Mode is Raw, length unchanged |
| `test_preprocessing_needs_difference` | Random walk | mode = Difference | Differenced series is stationary |
| `test_preprocessing_seasonal` | Seasonal + trend | mode = Seasonal | Seasonal-differenced is stationary |
| `test_preprocessing_override_raw` | Non-stationary, override="raw" | mode = Raw | Skips ADF, returns raw |
| `test_preprocessing_override_difference` | Any series, override="difference" | mode = Difference | Always differences |
| `test_first_difference_length` | n=100 | length = 99 | n-1 elements |
| `test_seasonal_difference_length` | n=100, period=24 | length = 76 | n-period elements |

### Classical F-test (classical.rs)

| Test | Input | Expected | Assertion |
|------|-------|----------|-----------|
| `test_classical_known_granger` | Synthetic pair with lag=2, strength=0.8 | p < 0.05, f_stat > 4.0 | Detects causality |
| `test_classical_no_granger` | Two independent AR(1) processes | p > 0.1 | Does not detect spurious causality |
| `test_classical_bidirectional` | A causes B but B does not cause A | A->B significant, B->A not | Correct directionality |
| `test_classical_insufficient_data` | n=5, lag=3 | Error | Returns InsufficientData |
| `test_classical_f_statistic_positive` | Any valid input | f_stat >= 0 | Non-negative |
| `test_classical_rss_ordering` | Any valid input | RSS_restricted >= RSS_unrestricted | Always true (more params = lower RSS) |

### Toda-Yamamoto (toda_yamamoto.rs)

| Test | Input | Expected | Assertion |
|------|-------|----------|-----------|
| `test_toda_yamamoto_known_granger` | Synthetic pair with cointegration | p < 0.05 | Detects causality without differencing |
| `test_toda_yamamoto_no_granger` | Independent series | p > 0.1 | Does not detect spurious |
| `test_toda_yamamoto_dmax_1` | I(1) series pair | Works without error | Augmented lag handles non-stationarity |
| `test_toda_yamamoto_matches_classical` | Stationary pair | Similar p-values | Within 0.1 of classical F-test |

### BIC Lag Selection (lag_selection.rs)

| Test | Input | Expected | Assertion |
|------|-------|----------|-----------|
| `test_bic_selects_true_lag` | Synthetic pair with lag=2 | optimal_lag = 2 | BIC minimum at true lag |
| `test_bic_all_results_stored` | 3 candidate lags | all_results.len() = 3 | Every lag tested |
| `test_bic_formula_known` | RSS=100.0, n=50, k=5 | BIC = 50*ln(2) + 5*ln(50) | Within 1e-6 of analytic |
| `test_bic_penalizes_complexity` | Two models, same RSS, different k | Higher k -> higher BIC | Monotonic in k |
| `test_bic_all_lags_fail` | Insufficient data for all lags | Error | Returns InsufficientData |

### FDR Correction (fdr.rs)

| Test | Input | Expected | Assertion |
|------|-------|----------|-----------|
| `test_fdr_single_pvalue` | [0.03] at alpha=0.05 | adjusted = 0.03, significant = true | No correction for m=1 |
| `test_fdr_known_example` | [0.01, 0.04, 0.03, 0.20] | Known BH-adjusted values | Match textbook example |
| `test_fdr_monotonicity` | Various p-values | adjusted[i] <= adjusted[i+1] for sorted | Monotone non-decreasing |
| `test_fdr_cap_at_one` | [0.8, 0.9] | all adjusted <= 1.0 | Never exceeds 1.0 |
| `test_fdr_preserves_order` | Original indices | result[i].original_index correct | Index tracking |
| `test_fdr_empty` | [] | [] | Empty input, empty output |
| `test_fdr_all_significant` | [0.001, 0.002, 0.003] | All significant at alpha=0.05 | All is_significant = true |
| `test_fdr_none_significant` | [0.5, 0.6, 0.7] | None significant at alpha=0.05 | All is_significant = false |

### Scanner (scanner.rs)

| Test | Input | Expected | Assertion |
|------|-------|----------|-----------|
| `test_scanner_empty_candidates` | No candidate pairs | pairs_tested = 0 | Handles empty gracefully |
| `test_scanner_skips_insufficient` | Pairs with n < min_observations | skipped_insufficient_data > 0 | Counts skipped |
| `test_scanner_selects_test_method` | config.test_method = "toda_yamamoto" | Uses TodaYamamotoTest | Correct dispatch |

### Candidate Extraction (candidates.rs)

| Test | Input | Expected | Assertion |
|------|-------|----------|-----------|
| `test_extract_top_k` | 20 K-NN results, candidate_count=10 | 10 pairs | Correct count |
| `test_extract_bidirectional` | 1 pair | Tests A->B and B->A | Both directions |
| `test_extract_fewer_than_k` | 3 K-NN results, candidate_count=10 | 3 pairs | Uses all available |

## Test Data Generators

```rust
/// Generate a pair of time series where source Granger-causes target.
fn make_granger_pair(n: usize, lag: usize, strength: f64, seed: u64) -> (Vec<f64>, Vec<f64>) {
    // Deterministic PRNG from seed
    // Source: AR(1) with phi=0.5
    //   x(t) = 0.5 * x(t-1) + epsilon_x(t)
    // Target: AR(1) + lagged source
    //   y(t) = 0.3 * y(t-1) + strength * x(t-lag) + epsilon_y(t)
}

/// Generate independent AR(1) processes (no Granger causality).
fn make_independent_pair(n: usize, seed: u64) -> (Vec<f64>, Vec<f64>) {
    // Two independent AR(1) processes
}

/// Generate a random walk (non-stationary).
fn make_random_walk(n: usize, seed: u64) -> Vec<f64> {
    // Cumulative sum of white noise
}

/// Generate stationary white noise.
fn make_white_noise(n: usize, seed: u64) -> Vec<f64> {
    // iid N(0,1) from seeded PRNG
}
```

## Deterministic PRNG

All test data uses a simple deterministic PRNG (e.g., xorshift64) seeded per test to avoid depending on the `rand` crate in test code. This ensures reproducibility across platforms and runs.
