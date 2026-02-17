# fe-007 Test Plan: ndp-intelligence-app (Cycle Integration)

## Unit Tests

| Test | Input | Expected | Assertion |
|------|-------|----------|-----------|
| `test_granger_disabled_by_default` | No NDP_GRANGER_ENABLED env | granger_enabled = false | Default is disabled |
| `test_granger_enabled_true` | NDP_GRANGER_ENABLED=true | granger_enabled = true | Parses "true" |
| `test_granger_enabled_one` | NDP_GRANGER_ENABLED=1 | granger_enabled = true | Parses "1" |
| `test_granger_enabled_false` | NDP_GRANGER_ENABLED=false | granger_enabled = false | Explicit false |
| `test_should_run_granger_disabled` | granger_enabled=false | false | Skips immediately |
| `test_should_run_granger_no_config` | enabled but no granger config | false | Needs config block |
| `test_should_run_granger_first_run` | enabled, config present, last_run=None | true | Runs on first cycle |
| `test_should_run_granger_interval_not_elapsed` | last_run 1h ago, interval=24h | false | Respects interval |
| `test_should_run_granger_interval_elapsed` | last_run 25h ago, interval=24h | true | Runs after interval |

## Integration Tests

These require a running PostgreSQL + TimescaleDB instance (integration test environment).

| Test | Setup | Action | Assertion |
|------|-------|--------|-----------|
| `test_granger_disabled_zero_overhead` | NDP_GRANGER_ENABLED=false, populate gold data | Run cycle | summary.granger_pairs_tested = 0, no causal_candidates rows |
| `test_granger_enabled_populates_candidates` | NDP_GRANGER_ENABLED=true, populate gold aligned view with 7 days hourly data (168 rows), insert K-NN similarity results | Run cycle | summary.granger_pairs_tested > 0, gold.causal_candidates has rows |
| `test_granger_respects_scan_interval` | Run two cycles 1 minute apart | Second cycle | summary.granger_pairs_tested = 0 on second (interval not elapsed) |
| `test_granger_scan_under_30s` | Realistic data: 6 streams, 168 time points | Time the scan | elapsed < 30s |
| `test_granger_memory_under_50mb` | Same realistic data | Measure peak allocation | peak_alloc < 50MB |
| `test_granger_handles_missing_data` | Gold view with NULL values in some columns | Run scan | Skips pairs with insufficient non-null data, does not crash |
| `test_granger_cycle_summary_fields` | Enable granger, run cycle | Check CycleSummary | granger_pairs_tested and granger_significant populated |
| `test_granger_hot_reload_config` | Change granger.significance_level via etcd | Run cycle after reload | New significance_level applied |

## Feature Flag Verification

The feature flag test is critical: it must verify that when `NDP_GRANGER_ENABLED=false`:

1. No SQL queries to gold.causal_candidates
2. No GrangerScanner instantiation
3. No time series extraction
4. No OLS computation
5. CycleSummary.granger_pairs_tested remains 0

This is tested by:
- Checking that `granger_scanner` is `None` when disabled
- Running a full cycle and verifying no Granger-related log messages at any level
- Querying `gold.causal_candidates` before and after to verify no writes
