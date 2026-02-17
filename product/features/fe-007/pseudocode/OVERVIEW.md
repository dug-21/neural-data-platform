# fe-007 Pseudocode Overview: Granger Causality

## Component Interaction Map

```
                         Domain Config (etcd)
                              |
                              v
                    +-------------------+
                    | IntelligenceConfig |
                    | .granger: Option   |
                    +-------------------+
                              |
                              v
+------------------+    +-----------------------+    +---------------------+
| ndp-intelligence |    | ndp-intelligence-app  |    | ndp-cli             |
| (granger module) |    | (cycle integration)   |    | (DDL via ndp-lib)   |
|                  |<---| run_granger_scan()    |    |                     |
| GrangerScanner   |    | NDP_GRANGER_ENABLED   |    | CausalCandidates    |
| GrangerTest      |    | should_run_granger()  |    | Generator           |
| OLS engine       |    |                       |    |                     |
| ADF test         |    +-----------------------+    +---------------------+
| Preprocessing    |              |                           |
| BIC selection    |              v                           v
| FDR correction   |    +---------------------+    +-------------------+
| CandidateRegistry|    | gold.causal_         |    | CREATE TABLE      |
| EvidenceTracker  |--->| candidates           |<---| gold.causal_      |
| CandidateRanker  |    | (PostgreSQL)         |    | candidates        |
+------------------+    +---------------------+    +-------------------+
```

## Data Flow

1. **Trigger**: `gold_refresh` NOTIFY fires -> intelligence cycle runs
2. **Gate check**: `NDP_GRANGER_ENABLED` + `scan_interval_hours` elapsed?
3. **Candidate selection**: Top-K pairs from K-NN similarity results
4. **Time series extraction**: Query gold aligned view for each pair's columns
5. **Preprocessing**: ADF stationarity test -> adaptive differencing if needed
6. **Granger testing**: For each pair x each lag -> F-test or Toda-Yamamoto
7. **Lag optimization**: BIC selects optimal lag per pair
8. **FDR correction**: Benjamini-Hochberg across all tests in scan
9. **Registry update**: UPSERT results to gold.causal_candidates
10. **Evidence accumulation**: Update stability_score based on rolling window

## Component Files

| Component | Pseudocode File |
|-----------|----------------|
| ndp-intelligence (granger module) | `pseudocode/ndp-intelligence.md` |
| ndp-intelligence-app (cycle integration) | `pseudocode/ndp-intelligence-app.md` |
| ndp-cli (DDL via ndp-lib) | `pseudocode/ndp-cli.md` |
| domain-config (schema + config types) | `pseudocode/domain-config.md` |
