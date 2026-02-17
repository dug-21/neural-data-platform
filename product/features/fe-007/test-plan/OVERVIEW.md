# fe-007 Test Plan Overview: Granger Causality

## Test Strategy

Granger causality involves complex statistical computation that must be validated against known results. The test strategy emphasizes:

1. **Known-answer tests**: Use synthetic time series with analytically known Granger relationships to verify correctness
2. **Numerical accuracy**: Compare OLS, ADF, F-distribution results against published tables and reference implementations
3. **Integration boundaries**: Verify the intelligence cycle correctly gates, executes, and stores Granger results
4. **Zero-overhead verification**: Confirm feature flag off = no computation, no queries

## Test Distribution

| Component | Unit Tests | Integration Tests | Notes |
|-----------|-----------|------------------|-------|
| ndp-intelligence (granger) | ~45 | 0 | Pure computation, all testable in isolation |
| ndp-intelligence-app | ~5 | ~8 | Cycle integration, feature flag, config loading |
| ndp-cli (DDL via ndp-lib) | ~3 | ~2 | DDL generation, table existence check |
| domain-config | ~5 | ~2 | Schema validation, deserialization |
| **Total** | **~58** | **~12** | |

## Integration Surface

| Boundary | What to Test |
|----------|-------------|
| Service -> GrangerScanner | Scanner receives correct candidates and config |
| GrangerScanner -> PostgreSQL | Time series extraction queries correct view/columns |
| GrangerScanner -> causal_candidates | UPSERT writes correct rows, evidence accumulates |
| Config -> GrangerConfig | Deserialization from etcd JSON, hot-reload |
| Feature flag -> cycle | NDP_GRANGER_ENABLED=false skips everything |
| DDL generator -> PostgreSQL | Table creation is idempotent (IF NOT EXISTS) |

## Component Test Plans

| Component | Test Plan File |
|-----------|---------------|
| ndp-intelligence (granger module) | `test-plan/ndp-intelligence.md` |
| ndp-intelligence-app (cycle integration) | `test-plan/ndp-intelligence-app.md` |
| ndp-cli (DDL via ndp-lib) | `test-plan/ndp-cli.md` |
| domain-config (schema + types) | `test-plan/domain-config.md` |

## Pi Resource Budget Validation

The 30s/50MB constraints are tested via:
1. **Timing assertion**: Integration test measures scan duration on synthetic data (6 streams, 168 time points)
2. **Memory bound**: Use `std::alloc::System` global allocator with tracking wrapper in test to measure peak allocation during scan
3. **Realistic data**: Generate synthetic data matching production characteristics (hourly, 7 days, 6 streams with realistic correlations)

## Synthetic Test Data

All statistical tests use reproducible synthetic data:

```
// Deterministic seed for reproducibility
fn make_granger_pair(n: usize, lag: usize, strength: f64, seed: u64) -> (Vec<f64>, Vec<f64>)
    // Source: AR(1) process with known parameters
    // Target: AR(1) + lagged source with known coefficient
    // strength=0.0: no Granger causality
    // strength=1.0: strong Granger causality
```

This ensures tests are deterministic and do not depend on random number generation.
