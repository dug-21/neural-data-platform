# fe-007 Pseudocode: ndp-intelligence (Granger Module)

## Module Structure

```
crates/ndp-intelligence/src/granger/
  mod.rs              -- GrangerTest trait, TestResult, GrangerError, re-exports
  ols.rs              -- OLS regression engine (pure ndarray)
  stats.rs            -- F-distribution p-value (incomplete beta function)
  adf.rs              -- ADF stationarity test
  preprocessing.rs    -- Adaptive stationarity preprocessing pipeline
  classical.rs        -- Classical F-test Granger implementation
  toda_yamamoto.rs    -- Toda-Yamamoto Granger implementation
  lag_selection.rs    -- BIC-based lag optimizer
  fdr.rs              -- Benjamini-Hochberg FDR correction
  candidates.rs       -- Candidate pair extraction from K-NN results
  data.rs             -- Time series extraction from gold aligned view
  registry.rs         -- UPSERT logic for gold.causal_candidates
  evidence.rs         -- Rolling window evidence accumulator
  ranker.rs           -- Composite score ranker
  scanner.rs          -- GrangerScanner orchestrator
```

## mod.rs -- Trait + Types

```pseudocode
pub struct TestResult {
    f_statistic: f64
    p_value: f64
    df1: usize  // numerator df (lag order)
    df2: usize  // denominator df
    rss_restricted: f64
    rss_unrestricted: f64
}

pub trait GrangerTest: Send + Sync {
    fn test(source: &[f64], target: &[f64], lag: usize) -> Result<TestResult>
    fn name() -> &str
}

pub struct GrangerResult {
    source_stream: String
    target_stream: String
    test_method: String
    optimal_lag: u32
    f_statistic: f64
    p_value: f64
    p_value_adjusted: Option<f64>
    is_significant: bool
    bic: f64
    preprocessing: PreprocessingMode
    all_lag_results: Vec<LagResult>
}

pub enum GrangerError {
    InsufficientData { required: usize, available: usize }
    SingularMatrix
    NumericalInstability(String)
    Database(String)
}
```

## ols.rs -- OLS Regression

```pseudocode
pub struct OlsResult {
    coefficients: Array1<f64>  // beta hat
    residuals: Array1<f64>     // y - X*beta
    rss: f64                   // sum of squared residuals
    r_squared: f64
    n: usize
    k: usize                   // number of parameters
}

fn ols_fit(X: &Array2<f64>, y: &ArrayView1<f64>) -> Result<OlsResult>:
    // beta = (X'X)^{-1} X'y
    let xtx = X.t().dot(X)           // k x k
    let xty = X.t().dot(y)           // k x 1

    // Use LU decomposition for inversion (stable for well-conditioned matrices)
    // If xtx is singular, return GrangerError::SingularMatrix
    let xtx_inv = lu_inverse(xtx)?

    let beta = xtx_inv.dot(xty)
    let y_hat = X.dot(beta)
    let residuals = y - y_hat
    let rss = residuals.dot(residuals)

    let y_mean = y.mean()
    let tss = y.iter().map(|yi| (yi - y_mean)^2).sum()
    let r_squared = 1.0 - rss / tss

    return OlsResult { coefficients: beta, residuals, rss, r_squared, n: y.len(), k: beta.len() }

fn lu_inverse(matrix: Array2<f64>) -> Result<Array2<f64>>:
    // LU decomposition with partial pivoting
    // For small matrices (typically 5x5 to 15x15), this is numerically stable
    // If pivot is near-zero (< 1e-12), return SingularMatrix error
    let n = matrix.nrows()
    let (L, U, P) = lu_decompose(matrix)
    let inv = solve_lu_system(L, U, P, identity(n))
    return inv

fn build_var_matrices(
    source: &[f64],
    target: &[f64],
    lag: usize,
    restricted: bool
) -> (Array2<f64>, Array1<f64>):
    // Build design matrix X and response vector y for VAR model
    // y = target[lag..n]
    // X columns:
    //   [1, target[lag-1], ..., target[0], source[lag-1], ..., source[0]]  (unrestricted)
    //   [1, target[lag-1], ..., target[0]]                                   (restricted)
    let n = target.len() - lag
    let k = if restricted { lag + 1 } else { 2 * lag + 1 }

    let mut X = Array2::zeros((n, k))
    let mut y = Array1::zeros(n)

    for t in 0..n:
        y[t] = target[t + lag]
        X[[t, 0]] = 1.0  // intercept
        for j in 0..lag:
            X[[t, j + 1]] = target[t + lag - 1 - j]
        if not restricted:
            for j in 0..lag:
                X[[t, lag + 1 + j]] = source[t + lag - 1 - j]

    return (X, y)
```

## stats.rs -- F-distribution P-value

```pseudocode
/// Compute p-value from F-distribution: P(F > f_stat | df1, df2)
fn f_distribution_p_value(f_stat: f64, df1: usize, df2: usize) -> f64:
    if f_stat <= 0.0: return 1.0
    if f_stat.is_nan() or f_stat.is_infinite(): return NaN

    // P(F > x) = 1 - I_x(df1/2, df2/2)
    // where I_x is the regularized incomplete beta function
    // and x = df1 * f_stat / (df1 * f_stat + df2)

    let x = (df1 as f64 * f_stat) / (df1 as f64 * f_stat + df2 as f64)
    let a = df1 as f64 / 2.0
    let b = df2 as f64 / 2.0

    return 1.0 - regularized_incomplete_beta(x, a, b)

/// Regularized incomplete beta function I_x(a, b)
/// Uses continued fraction expansion (Lentz's algorithm)
fn regularized_incomplete_beta(x: f64, a: f64, b: f64) -> f64:
    if x <= 0.0: return 0.0
    if x >= 1.0: return 1.0

    // Use symmetry: if x > (a+1)/(a+b+2), compute 1 - I_{1-x}(b, a)
    let symmetry_threshold = (a + 1.0) / (a + b + 2.0)
    if x > symmetry_threshold:
        return 1.0 - regularized_incomplete_beta(1.0 - x, b, a)

    // Continued fraction via Lentz's modified algorithm
    // Reference: Numerical Recipes, Chapter 6.4
    let prefix = exp(
        a * ln(x) + b * ln(1.0 - x) - ln_beta(a, b) - ln(a)
    )

    let cf = continued_fraction_beta(x, a, b, max_iterations=200, epsilon=1e-12)
    return prefix * cf

fn ln_beta(a: f64, b: f64) -> f64:
    return ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b)

fn ln_gamma(x: f64) -> f64:
    // Lanczos approximation (g=7, 9 coefficients)
    // Accurate to ~15 digits for x > 0.5
    // For x < 0.5: use reflection formula
```

## adf.rs -- ADF Stationarity Test

```pseudocode
pub struct AdfResult {
    test_statistic: f64      // t-statistic on gamma
    p_value: f64             // interpolated from critical value table
    lags_used: usize         // number of augmenting lags
    is_stationary: bool      // at specified significance level
}

/// MacKinnon (1996) critical values for ADF test with constant + trend
const ADF_CRITICAL_VALUES: table indexed by (sample_size, significance_level)
    // sample sizes: [25, 50, 100, 250, 500, inf]
    // significance: [0.01, 0.05, 0.10]
    // values for constant + trend model

fn adf_test(series: &[f64], max_lag: Option<usize>, significance: f64) -> Result<AdfResult>:
    let n = series.len()
    if n < 20: return Err(InsufficientData)

    // Determine number of augmenting lags (Schwert 1989 rule)
    let default_max_lag = min(floor(12 * (n as f64 / 100.0).powf(0.25)), n / 3)
    let lags = max_lag.unwrap_or(default_max_lag)

    // Compute first differences: delta_y(t) = y(t) - y(t-1)
    let delta_y = diff(series)

    // Build ADF regression: delta_y(t) = alpha + beta*t + gamma*y(t-1) + sum(delta_i * delta_y(t-i))
    let effective_n = delta_y.len() - lags
    let k = lags + 3  // intercept + trend + y(t-1) + lags augmenting terms

    let mut X = Array2::zeros((effective_n, k))
    let mut y = Array1::zeros(effective_n)

    for t in 0..effective_n:
        let idx = t + lags
        y[t] = delta_y[idx]
        X[[t, 0]] = 1.0                          // intercept (alpha)
        X[[t, 1]] = (idx + 1) as f64             // trend (beta)
        X[[t, 2]] = series[idx]                   // y(t-1) (gamma)
        for j in 0..lags:
            X[[t, 3 + j]] = delta_y[idx - 1 - j] // delta_y(t-j)

    let ols = ols_fit(&X, &y.view())?
    let gamma = ols.coefficients[2]

    // Standard error of gamma
    let sigma2 = ols.rss / (effective_n - k) as f64
    let xtx_inv = lu_inverse(X.t().dot(&X))?
    let se_gamma = sqrt(sigma2 * xtx_inv[[2, 2]])

    let t_stat = gamma / se_gamma

    // Interpolate p-value from MacKinnon critical value table
    let p_value = interpolate_adf_pvalue(t_stat, effective_n)
    let is_stationary = t_stat < critical_value(significance, effective_n)

    return AdfResult { test_statistic: t_stat, p_value, lags_used: lags, is_stationary }
```

## preprocessing.rs -- Adaptive Pipeline

```pseudocode
pub enum PreprocessingMode { Raw, Difference, Seasonal }

pub struct PreprocessingResult {
    series: Vec<f64>
    mode: PreprocessingMode
    adf_statistic: f64
    adf_p_value: f64
    original_len: usize
}

fn ensure_stationary(
    series: &[f64],
    mode_override: Option<&str>,  // from domain config
    significance: f64,
    seasonal_period: usize        // 24 for hourly
) -> Result<PreprocessingResult>:

    match mode_override:
        Some("raw"):
            return PreprocessingResult { series: series.to_vec(), mode: Raw, ... }
        Some("difference"):
            return PreprocessingResult { series: first_difference(series), mode: Difference, ... }
        Some("seasonal"):
            return PreprocessingResult { series: seasonal_difference(series, seasonal_period), mode: Seasonal, ... }
        _ => // "adaptive" or None -- run the pipeline

    // Stage 1: test raw
    let adf_raw = adf_test(series, None, significance)?
    if adf_raw.is_stationary:
        return PreprocessingResult { series: series.to_vec(), mode: Raw, adf_statistic: adf_raw.test_statistic, ... }

    // Stage 2: first-difference
    let diffed = first_difference(series)
    let adf_diff = adf_test(&diffed, None, significance)?
    if adf_diff.is_stationary:
        return PreprocessingResult { series: diffed, mode: Difference, adf_statistic: adf_diff.test_statistic, ... }

    // Stage 3: seasonal-difference
    let seasonal = seasonal_difference(series, seasonal_period)
    let adf_seasonal = adf_test(&seasonal, None, significance)?
    // Use seasonal regardless (best effort)
    return PreprocessingResult { series: seasonal, mode: Seasonal, adf_statistic: adf_seasonal.test_statistic, ... }

fn first_difference(series: &[f64]) -> Vec<f64>:
    series.windows(2).map(|w| w[1] - w[0]).collect()

fn seasonal_difference(series: &[f64], period: usize) -> Vec<f64>:
    series[period..].iter().zip(series.iter()).map(|(a, b)| a - b).collect()
```

## classical.rs -- Classical F-test

```pseudocode
pub struct ClassicalFTest;

impl GrangerTest for ClassicalFTest:
    fn test(source: &[f64], target: &[f64], lag: usize) -> Result<TestResult>:
        let n = target.len()
        if n <= 2 * lag + 1:
            return Err(InsufficientData)

        // Restricted model: target ~ intercept + target_lags
        let (X_r, y) = build_var_matrices(source, target, lag, restricted=true)
        let ols_r = ols_fit(&X_r, &y.view())?

        // Unrestricted model: target ~ intercept + target_lags + source_lags
        let (X_u, _) = build_var_matrices(source, target, lag, restricted=false)
        let ols_u = ols_fit(&X_u, &y.view())?

        let p = lag  // number of restrictions
        let df1 = p
        let df2 = n - lag - 2 * p - 1  // effective observations minus parameters

        // F = ((RSS_r - RSS_u) / p) / (RSS_u / df2)
        let f_stat = ((ols_r.rss - ols_u.rss) / p as f64) / (ols_u.rss / df2 as f64)

        if f_stat.is_nan() or f_stat < 0.0:
            return Err(NumericalInstability("F-statistic is invalid"))

        let p_value = f_distribution_p_value(f_stat, df1, df2)

        return TestResult {
            f_statistic: f_stat,
            p_value,
            df1, df2,
            rss_restricted: ols_r.rss,
            rss_unrestricted: ols_u.rss,
        }

    fn name() -> &str: "classical"
```

## toda_yamamoto.rs -- Toda-Yamamoto Test

```pseudocode
pub struct TodaYamamotoTest {
    d_max: usize  // maximum integration order (default 1)
}

impl GrangerTest for TodaYamamotoTest:
    fn test(source: &[f64], target: &[f64], lag: usize) -> Result<TestResult>:
        // Toda-Yamamoto: fit VAR(p + d_max), but only test coefficients on lags 1..p
        // This avoids pre-testing for unit roots
        let augmented_lag = lag + self.d_max
        let n = target.len()
        if n <= 2 * augmented_lag + 1:
            return Err(InsufficientData)

        // Unrestricted: target ~ intercept + target_lags(1..p+d) + source_lags(1..p+d)
        let (X_u, y) = build_var_matrices(source, target, augmented_lag, restricted=false)
        let ols_u = ols_fit(&X_u, &y.view())?

        // Restricted: target ~ intercept + target_lags(1..p+d) + source_lags(p+1..p+d)
        // i.e., zero out source coefficients for lags 1..p, keep lags p+1..p+d
        let (X_r, _) = build_toda_restricted_matrix(source, target, lag, self.d_max)
        let ols_r = ols_fit(&X_r, &y.view())?

        let p = lag  // number of restrictions (source lags 1..p)
        let df1 = p
        let df2 = y.len() - X_u.ncols()

        let f_stat = ((ols_r.rss - ols_u.rss) / p as f64) / (ols_u.rss / df2 as f64)
        let p_value = f_distribution_p_value(f_stat, df1, df2)

        return TestResult { f_statistic: f_stat, p_value, df1, df2, rss_restricted: ols_r.rss, rss_unrestricted: ols_u.rss }

    fn name() -> &str: "toda_yamamoto"
```

## lag_selection.rs -- BIC Optimizer

```pseudocode
fn compute_bic(rss: f64, n: usize, k: usize) -> f64:
    n as f64 * (rss / n as f64).ln() + k as f64 * (n as f64).ln()

fn select_optimal_lag(
    source: &[f64],
    target: &[f64],
    test: &dyn GrangerTest,
    lag_candidates: &[u32],
) -> Result<LagSelectionResult>:

    let mut results = Vec::new()
    let mut best_bic = f64::INFINITY
    let mut best_lag = lag_candidates[0]

    for &lag in lag_candidates:
        match test.test(source, target, lag as usize):
            Ok(tr) =>
                let k = 2 * lag as usize + 1  // unrestricted model parameters
                let n = target.len() - lag as usize
                let bic = compute_bic(tr.rss_unrestricted, n, k)
                if bic < best_bic:
                    best_bic = bic
                    best_lag = lag
                results.push(LagResult { lag, bic, test_result: tr })
            Err(e) =>
                warn!("Lag {} failed: {}", lag, e)
                // skip this lag

    if results.is_empty():
        return Err(InsufficientData)

    return LagSelectionResult { optimal_lag: best_lag, optimal_bic: best_bic, all_results: results }
```

## fdr.rs -- Benjamini-Hochberg

```pseudocode
fn benjamini_hochberg(p_values: &[f64], alpha: f64) -> Vec<AdjustedPValue>:
    let m = p_values.len()
    if m == 0: return vec![]

    // Create indexed pairs, sort by p-value ascending
    let mut indexed: Vec<(usize, f64)> = p_values.iter().copied().enumerate().collect()
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap())

    // Compute adjusted p-values
    let mut adjusted = vec![0.0; m]
    adjusted[m - 1] = indexed[m - 1].1  // last one stays as-is (rank m: p * m/m = p)

    for i in (0..m-1).rev():
        let rank = i + 1  // 1-indexed rank
        let raw_adjusted = indexed[i].1 * m as f64 / rank as f64
        adjusted[i] = raw_adjusted.min(adjusted[i + 1]).min(1.0)  // monotonicity + cap at 1.0

    // Build result in original order
    let mut result = vec![AdjustedPValue::default(); m]
    for (sorted_idx, (orig_idx, p_val)) in indexed.iter().enumerate():
        result[*orig_idx] = AdjustedPValue {
            original_index: *orig_idx,
            p_value: *p_val,
            p_value_adjusted: adjusted[sorted_idx],
            is_significant: adjusted[sorted_idx] < alpha,
        }

    return result
```

## scanner.rs -- GrangerScanner Orchestrator

```pseudocode
pub struct GrangerScanner {
    pool: Arc<Pool>
    config: GrangerConfig
    test: Box<dyn GrangerTest>
}

pub struct ScanSummary {
    pairs_tested: usize
    significant_count: usize
    skipped_insufficient_data: usize
    skipped_errors: usize
    duration: Duration
}

impl GrangerScanner:
    fn new(pool: Arc<Pool>, config: &GrangerConfig) -> Self:
        let test: Box<dyn GrangerTest> = match config.test_method.as_str():
            "toda_yamamoto" => Box::new(TodaYamamotoTest { d_max: 1 })
            _ => Box::new(ClassicalFTest)
        Self { pool, config: config.clone(), test }

    async fn run_scan(
        &self,
        domain_id: &str,
        candidates: &[CandidatePair],
        view_name: &str,
    ) -> Result<ScanSummary>:
        let start = Instant::now()
        let client = self.pool.get().await?

        let mut all_results: Vec<GrangerResult> = Vec::new()
        let mut skipped_data = 0
        let mut skipped_errors = 0

        // For each candidate pair, test both directions
        for pair in candidates:
            for (source, target) in [(pair.stream_a, pair.stream_b), (pair.stream_b, pair.stream_a)]:
                // Extract time series from gold aligned view
                let (source_data, target_data) = extract_time_series(
                    &client, view_name, source, target, self.config.min_observations
                ).await?

                if source_data.len() < self.config.min_observations:
                    warn!("Skipping {}->{}: insufficient data ({} < {})", source, target, source_data.len(), self.config.min_observations)
                    skipped_data += 1
                    continue

                // Preprocess for stationarity
                let source_prep = ensure_stationary(&source_data, Some(&self.config.preprocessing), 0.05, 24)?
                let target_prep = ensure_stationary(&target_data, Some(&self.config.preprocessing), 0.05, 24)?

                // Check sufficient data after preprocessing
                let min_len = source_prep.series.len().min(target_prep.series.len())
                if min_len < self.config.min_observations:
                    skipped_data += 1
                    continue

                // Truncate to same length
                let effective_len = min_len
                let src = &source_prep.series[..effective_len]
                let tgt = &target_prep.series[..effective_len]

                // Lag optimization via BIC
                match select_optimal_lag(src, tgt, self.test.as_ref(), &self.config.lag_hours):
                    Ok(lag_result) =>
                        all_results.push(GrangerResult {
                            source_stream: source.to_string(),
                            target_stream: target.to_string(),
                            test_method: self.test.name().to_string(),
                            optimal_lag: lag_result.optimal_lag,
                            f_statistic: lag_result.all_results[optimal_idx].test_result.f_statistic,
                            p_value: lag_result.all_results[optimal_idx].test_result.p_value,
                            p_value_adjusted: None,  // set after FDR
                            is_significant: false,     // set after FDR
                            bic: lag_result.optimal_bic,
                            preprocessing: source_prep.mode,
                            all_lag_results: lag_result.all_results,
                        })
                    Err(e) =>
                        warn!("Granger test failed for {}->{}: {}", source, target, e)
                        skipped_errors += 1

        // Apply FDR correction across all results
        let p_values: Vec<f64> = all_results.iter().map(|r| r.p_value).collect()
        let adjusted = benjamini_hochberg(&p_values, self.config.significance_level)
        for (result, adj) in all_results.iter_mut().zip(adjusted.iter()):
            result.p_value_adjusted = Some(adj.p_value_adjusted)
            result.is_significant = adj.is_significant

        // UPSERT to gold.causal_candidates
        let significant_count = all_results.iter().filter(|r| r.is_significant).count()
        upsert_candidates(&client, domain_id, &all_results).await?

        // Update evidence and stability
        update_evidence(&client, domain_id, &all_results, self.config.evidence_window_days).await?

        // Rank candidates
        rank_candidates(&client, domain_id).await?

        return ScanSummary {
            pairs_tested: all_results.len(),
            significant_count,
            skipped_insufficient_data: skipped_data,
            skipped_errors,
            duration: start.elapsed(),
        }
```

## candidates.rs -- K-NN Pair Extraction

```pseudocode
pub struct CandidatePair {
    stream_a: String   // column name from gold aligned view
    stream_b: String   // column name from gold aligned view
    similarity: f64    // K-NN similarity score
}

fn extract_candidates_from_knn(
    neighbors: &[SearchResult],
    candidate_count: usize,
) -> Vec<CandidatePair>:
    // K-NN results are embedding-level (whole-vector similarity)
    // We need stream-level pairs for Granger testing
    //
    // Strategy: use the gold aligned view columns directly
    // Each column represents a stream+field (e.g., "indoor_pm25_mean")
    // Extract top-K unique column pairs by correlation strength

    // For initial implementation: pair all columns against each other
    // and use the K-NN similarity as a proxy for which pairs to test
    // This will be refined when we have per-field similarity scores

    let mut pairs: Vec<CandidatePair> = Vec::new()
    // Use similarity results to identify which time periods are most similar
    // Then compute pairwise correlations on those periods
    // Select top candidate_count pairs by absolute correlation

    pairs.truncate(candidate_count)
    return pairs
```

## registry.rs -- UPSERT Logic

```pseudocode
async fn upsert_candidates(
    client: &Client,
    domain_id: &str,
    results: &[GrangerResult],
) -> Result<()>:
    let stmt = "
        INSERT INTO gold.causal_candidates (
            domain_id, source_stream, target_stream, test_method,
            lag_hours, f_statistic, p_value, p_value_adjusted,
            is_significant, bic, preprocessing, evidence_count,
            stability_score, first_seen, last_seen, metadata
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 1, NULL, now(), now(), $12)
        ON CONFLICT (domain_id, source_stream, target_stream, lag_hours)
        DO UPDATE SET
            test_method = EXCLUDED.test_method,
            f_statistic = EXCLUDED.f_statistic,
            p_value = EXCLUDED.p_value,
            p_value_adjusted = EXCLUDED.p_value_adjusted,
            is_significant = EXCLUDED.is_significant,
            bic = EXCLUDED.bic,
            preprocessing = EXCLUDED.preprocessing,
            evidence_count = gold.causal_candidates.evidence_count + 1,
            last_seen = now(),
            metadata = EXCLUDED.metadata
    "
    for result in results:
        let metadata = serde_json::json!({
            "all_lags": result.all_lag_results.iter().map(|lr| {
                { "lag": lr.lag, "bic": lr.bic, "f_stat": lr.test_result.f_statistic, "p_value": lr.test_result.p_value }
            }).collect::<Vec<_>>()
        })
        client.execute(stmt, &[
            &domain_id, &result.source_stream, &result.target_stream,
            &result.test_method, &(result.optimal_lag as i32),
            &result.f_statistic, &result.p_value, &result.p_value_adjusted,
            &result.is_significant, &result.bic, &result.preprocessing.as_str(),
            &metadata
        ]).await?
```

## evidence.rs -- Evidence Accumulator

```pseudocode
async fn update_evidence(
    client: &Client,
    domain_id: &str,
    results: &[GrangerResult],
    window_days: u32,
) -> Result<()>:
    // Compute stability_score = evidence_count / total_scans_in_window
    // total_scans_in_window is approximated from first_seen and scan_interval
    let stmt = "
        UPDATE gold.causal_candidates
        SET stability_score = CASE
            WHEN EXTRACT(EPOCH FROM (now() - first_seen)) / 3600 / scan_interval_hours_est > 0
            THEN evidence_count::float / GREATEST(1,
                FLOOR(EXTRACT(EPOCH FROM (now() - first_seen)) / 3600 / 24)::int)
            ELSE 1.0
        END
        WHERE domain_id = $1
          AND last_seen >= now() - ($2 || ' days')::interval
    "
    client.execute(stmt, &[&domain_id, &window_days.to_string()]).await?

    // Mark stale candidates (not seen in this scan) as not significant
    // Only within the evidence window
    let source_targets: HashSet<(String, String)> = results.iter()
        .map(|r| (r.source_stream.clone(), r.target_stream.clone()))
        .collect()

    // This is handled by the UPSERT -- candidates not in this scan keep their old is_significant
    // The stability_score naturally decays as evidence_count / total_scans shrinks
```

## ranker.rs -- Candidate Ranker

```pseudocode
async fn rank_candidates(client: &Client, domain_id: &str) -> Result<()>:
    // Update metadata.rank for all significant candidates
    // Score = -log10(p_value_adjusted) * relevance_weight
    // relevance_weight: 1.0 if stream is referenced by domain objective, 0.5 otherwise
    let stmt = "
        UPDATE gold.causal_candidates
        SET metadata = metadata || jsonb_build_object(
            'rank_score', -log(p_value_adjusted) / log(10.0) *
                CASE WHEN source_stream = ANY($2) OR target_stream = ANY($2)
                     THEN 1.0 ELSE 0.5 END
        )
        WHERE domain_id = $1 AND is_significant = true AND p_value_adjusted > 0
    "
    // $2 = array of objective-referenced stream fields
    client.execute(stmt, &[&domain_id, &objective_fields]).await?
```
