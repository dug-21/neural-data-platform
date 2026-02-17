# fe-007: Granger Causality (Statistical Validation)

## Vision

K-NN similarity search finds correlations. Granger causality tests whether those correlations are predictive — does stream A's past values improve prediction of stream B's future values beyond what stream B's own history provides? This separates genuine lead-lag relationships ("outdoor humidity predicts indoor PM2.5 with 2-hour lag") from coincidental correlation.

Feature-flagged at the deployment level (`NDP_GRANGER_ENABLED` environment variable). When disabled, the intelligence cycle skips all Granger computation with zero overhead.

## Tracking

- Feature: fe-007
- GitHub Issue: https://github.com/dug-21/neural-data-platform/issues/38
- Parent roadmap: `product/features/gold-001/FEATURE-ROADMAPv1.2.md` (Track B: v12-G01 through G07)
- Predecessor: fe-004 (K-NN search provides candidate pairs)
- Version target: v1.2.x

## What fe-004 Delivers (Prerequisites)

- K-NN similarity search returning nearest neighbors
- Gold aligned view with cross-stream hourly data
- `gold.predictions` with outcome tracking

## Deliverables

| ID | Task | Description |
|----|------|-------------|
| G-01 | Feature flag | `NDP_GRANGER_ENABLED` env var (default: false). When false, all Granger code is skipped at runtime |
| G-02 | Similarity-guided candidates | Top 10 K-NN pairs per scan. Test both directions (A→B and B→A) for each pair (20 tests total) |
| G-03 | Granger causality scanner | Pairwise Granger test on candidate pairs. Pure Rust, ndarray. No external stats libraries. Pluggable test strategy (see Statistical Strategy) |
| G-04 | Lag optimizer | For each validated relationship, find optimal lag from domain-configured set (default: 1h, 2h, 4h). Selection via BIC. Store results for all tested lags |
| G-05 | Candidate registry | `gold.causal_candidates` table — regular table (not hypertable), no aggregates. Domain-scoped. Table creation: verify during planning whether init-script or ndp-cli gold DDL (leaning ndp-cli since table is domain-dependent) |
| G-06 | Candidate ranker | Rank by strength x relevance to domain objectives |
| G-07 | Evidence accumulator | Rolling window (default 7 days, domain-configurable). Track relationship stability — how consistently does Granger confirm the relationship across scans? |

## Statistical Strategy

### Stationarity — Adaptive Preprocessing Pipeline

Environmental sensor data is inherently non-stationary (diurnal cycles, weather regimes, seasonal drift). Classical Granger assumes stationarity. Pipeline:

1. Run ADF test (Augmented Dickey-Fuller) on windowed data — OLS regression, pure ndarray
2. If stationary → use raw values
3. If non-stationary → first-difference (Δx(t) = x(t) - x(t-1)), re-test
4. If still non-stationary → seasonal-difference (x(t) - x(t-24) for hourly data)
5. Record which preprocessing was applied per pair (interpretability)

Preprocessing mode is domain-configurable: `adaptive` (default), `raw`, `difference`, `seasonal`.

### Test Methods — Pluggable Strategy Trait

| Method | Strengths | When |
|--------|-----------|------|
| Classical F-test (default) | Fast, well-understood, interpretable | Data passes stationarity check |
| Toda-Yamamoto | Works on non-stationary/cointegrated series without differencing | Fallback when stationarity is borderline |

Both are OLS-based (restricted vs unrestricted VAR model comparison). Extensible — Transfer Entropy (nonlinear, model-free) can be added later without changing the pipeline.

### Multiple Comparisons

Benjamini-Hochberg FDR correction (controls false discovery rate). Store both raw and adjusted p-values. Significance level domain-configurable (default 0.05).

### Minimum Observations

48 data points (2 days of hourly data) required before Granger will attempt a test on any pair. Domain-configurable.

## Domain Configuration

Feature gate: `NDP_GRANGER_ENABLED` env var (deployment-level). Tuning: domain config under `intelligence.granger` (hot-reloadable via config-client).

```json
{
  "intelligence": {
    "embedding": { "..." : "..." },
    "search": { "..." : "..." },
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

Note: Requires domain schema update (`additionalProperties: false` — see v1.2.3 lesson).

## Integration

Granger runs inside the existing intelligence cycle at a lower cadence:

1. `gold_refresh` NOTIFY fires (hourly) → embeddings → K-NN → predictions (existing, every cycle)
2. Check `last_granger_run` against `scan_interval_hours` (default 24h)
3. If interval elapsed: pull top-K candidates from K-NN → run Granger → update `gold.causal_candidates`
4. Validated Granger relationships feed `gold.predictions` (a confirmed causal link IS a prediction)
5. If interval not elapsed: skip Granger step entirely

No separate scheduler or trigger — gated step in the existing pipeline.

## Constraints

- Feature-flagged via environment variable, NOT domain config
- When disabled: zero computation, zero database queries, zero overhead
- Pure Rust implementation — no R, no Python, no external stats packages
- Granger tests only run on candidate pairs identified by K-NN (not exhaustive pairwise)
- Must handle missing data gracefully (NWS observations with null fields)
- Computation budget: Granger scan must complete within 30s for current stream count
- No dependency on fe-005/fe-006 — works with metric-only K-NN results

## Acceptance Criteria

| Criterion | Target |
|-----------|--------|
| Feature flag off = zero overhead | No Granger queries when NDP_GRANGER_ENABLED=false |
| Feature flag on = candidates tested | Top 10 K-NN pairs tested bidirectionally with Granger |
| Validated relationships found | >3 significant relationships (p < 0.05 after FDR correction) |
| Optimal lags identified | Each relationship has BIC-selected lag estimate |
| Candidate registry populated | gold.causal_candidates has entries with metadata |
| Evidence accumulation works | Rolling window tracks stability over multiple scans |
| Pi resource budget | Granger scan <30s, <50MB additional memory |
| Stationarity handled | ADF test + adaptive preprocessing pipeline functional |
| Both test methods work | Classical F-test and Toda-Yamamoto selectable via config |
| Domain config hot-reload | Changing granger config updates behavior without restart |

## Out of Scope

- PC algorithm / full causal graph discovery (future)
- Cross-domain causal analysis (future)
- Transfer Entropy test method (future extension via strategy trait)
- Dashboards (fe-008)
- SONA learning from causal relationships (fe-009)

## Release

v1.2.x — Statistical validation of similarity-discovered relationships. Feature-flagged.
