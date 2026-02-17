# fe-007 Acceptance Criteria Map

| AC-ID | Description | Verification Method | Verification Detail | Status |
|-------|-------------|--------------------|--------------------|--------|
| AC-01 | Feature flag off = zero overhead | test | `test_granger_disabled_zero_overhead`: NDP_GRANGER_ENABLED=false, run cycle, assert granger_pairs_tested=0, no causal_candidates rows | PENDING |
| AC-02 | Feature flag on = candidates tested | test | `test_granger_enabled_populates_candidates`: NDP_GRANGER_ENABLED=true, 7 days hourly data, assert granger_pairs_tested > 0 | PENDING |
| AC-03 | Validated relationships found | test | `test_classical_known_granger`: Synthetic pair with lag=2 strength=0.8, assert p < 0.05 after FDR; integration test confirms >3 significant in realistic data | PENDING |
| AC-04 | Optimal lags identified | test | `test_bic_selects_true_lag`: Synthetic pair with true lag=2, assert optimal_lag=2; all tested lags stored in all_results | PENDING |
| AC-05 | Candidate registry populated | test | `test_granger_enabled_populates_candidates`: After scan, query gold.causal_candidates WHERE domain_id=test, assert row count > 0 with metadata | PENDING |
| AC-06 | Evidence accumulation works | test | `test_evidence_accumulator`: Run two scans, assert evidence_count incremented, stability_score computed, rolling window respected | PENDING |
| AC-07 | Pi resource budget | test | `test_granger_scan_under_30s`: 6 streams, 168 points, assert elapsed < 30s; `test_granger_memory_under_50mb`: assert peak_alloc < 50MB | PENDING |
| AC-08 | Stationarity handled | test | `test_preprocessing_needs_difference`: Random walk input, assert mode=Difference and output is stationary; `test_adf_stationary_series` and `test_adf_random_walk` | PENDING |
| AC-09 | Both test methods work | test | `test_classical_known_granger` + `test_toda_yamamoto_known_granger`: Both detect causality on synthetic data; config selects method | PENDING |
| AC-10 | Domain config hot-reload | test | `test_granger_hot_reload_config`: Change significance_level via etcd, run cycle, assert new value applied | PENDING |
