# fe-004 Alignment Report: Similarity Intelligence (V1.2 Phase 2)

> **Feature**: fe-004
> **Date**: 2026-02-15
> **Criteria source**: `product/vision/ALIGNMENT-CRITERIA.md`
> **Artifacts reviewed**: SCOPE.md, SPECIFICATION.md, TASK-DECOMPOSITION.md, ARCHITECTURE.md (ADRs 009-015), PSEUDOCODE.md

---

## Summary

| Principle | Status | Notes |
|-----------|--------|-------|
| 1. Edge-Only | PASS | All processing on Pi; no cloud dependencies |
| 2. Config-Driven | PASS | Intelligence config optional; env vars for runtime; objectives in JSON |
| 3. Domain-Portable | PASS | Factory pattern + trait abstraction; domain_id parameterized throughout |
| 4. Resource-Constrained | PASS | 256MB container limit; 2 connection pool; ~34MB estimated runtime |
| 5. Integration-First | PASS | Extends existing traits, crates, and deployment; no parallel systems |
| 6. Privacy by Architecture | PASS | Local-only; no telemetry; no network calls beyond local PostgreSQL |
| 7. Self-Learning | PASS | Predictions evaluated against outcomes; accuracy tracked; warmup builds statistics |

**Overall**: 7 PASS, 0 WARN, 0 VARIANCE, 0 FAIL.

---

## Detailed Analysis

### 1. Edge-Only

- All intelligence processing runs on the Raspberry Pi 5
- No cloud APIs, no external model downloads at runtime
- ruvector-core is compiled locally (no download-at-runtime)
- PostgreSQL/pgvector runs locally in Docker
- PG NOTIFY is local IPC, not network
- Internet not required for any intelligence operation

**Result**: PASS

### 2. Config-Driven

- Intelligence config block is `Option<IntelligenceConfig>` in DomainConfig -- existing configs without it continue to work unchanged
- Domain config loaded from etcd via `config-client` (consistent with stream config pattern)
- Embedding fields, search K, similarity threshold, prediction horizons all from JSON config in etcd
- Objective metrics (thresholds, direction) read from existing `objectives` in domain config (ADR-015)
- Runtime settings via environment variables (ADR-012): poll interval, pool size, warmup threshold, etcd endpoints
- No hardcoded field names, thresholds, or intervals in source code
- Timer interval configurable via `INTELLIGENCE_POLL_INTERVAL_SECS`

**Result**: PASS

### 3. Domain-Portable

- `SimilarityEngine` trait abstracts search backend -- works for any domain
- `MetricEmbedder` reads field list from config -- no air-quality-specific code
- `PredictionEngine` uses objective metrics from config -- no hardcoded metrics
- `IntelligenceService` parameterized by `domain_id` -- view name derived dynamically
- `sql_row_to_gold_row` reads columns dynamically -- no schema assumptions
- Factory function selects engine based on feature flags, not domain

**Result**: PASS

### 4. Resource-Constrained

- Container limit: 256MB (metric-only; no MiniLM model in Phase 2)
- Estimated runtime memory: ~34MB (binary + HNSW index for <10K vectors + connection pool)
- Connection pool: 2 pooled + 1 NOTIFY = 3 PostgreSQL connections (ADR-009)
- HNSW index: ~32D vectors * 4 bytes * 10K entries = ~1.3MB
- Batch processing: 100 rows per cycle (PSEUDOCODE section 7)
- ARM64 compilation: all dependencies compile for aarch64 (ruvector-core feature-gated)
- No banned dependencies (no DuckDB, no Polars, no jemalloc)
- deadpool-postgres is lightweight and ARM64-compatible

**Result**: PASS

### 5. Integration-First

- Extends existing `SimilarityEngine` trait (fe-003) -- no new abstraction
- Uses existing `StorageBackend` trait (fe-003) for pgvector read/write
- Uses existing `MetricEmbedder` (fe-003) for embedding generation
- Uses existing `Prediction` struct (fe-003) for storage
- Extends existing `ndp-intelligence` crate -- no new crate
- Extends existing `ndp-intelligence-app` -- replaces stubs with implementations
- Adds to existing `docker-compose.yml` and `deploy.sh` -- no new deployment system
- PgVectorSchemaGenerator (fe-003) already produces required DDL -- no new schema tool

**Result**: PASS

### 6. Privacy by Architecture

- All data stays on the Pi
- No telemetry, phone-home, or cloud analytics
- Predictions stored locally in PostgreSQL
- No network calls beyond local Docker network
- Open source; all code auditable

**Result**: PASS

### 7. Self-Learning

- System improves via observation: MetricEmbedder's `RunningStats` adapts z-score normalization over time
- Predictions tracked against outcomes: `OutcomeTracker` evaluates correctness after horizon elapses
- Accuracy logged: `EvaluationSummary` tracks correct/incorrect ratios
- 168-hour warmup ensures sufficient statistical baseline before predicting
- K-NN is inherently self-improving: more observations = better neighbor quality
- Warmup counter persists across restarts (ADR-013)

**Result**: PASS

---

## Scope Alignment

### Scope Coverage

All 17 deliverables from SCOPE.md (P2-01 through P2-17) are addressed in SPECIFICATION.md and PSEUDOCODE.md. All 12 acceptance criteria from SCOPE.md are mapped in SPECIFICATION.md Section 5.

### Out-of-Scope Adherence

The specification correctly excludes:
- Event intelligence / text embeddings (Phase 4)
- Granger causality (Phase 3)
- Anomaly detection (Phase 5)
- Grafana dashboards (Phase 5)
- SONA learning (V1.3)
- MCP query interface (V1.3)

No scope creep detected.

### Additions Beyond Scope

None. The specification delivers exactly what SCOPE.md requests.

### Simplifications

- **Timer as primary wake mechanism** (not NOTIFY): SCOPE.md says "PG NOTIFY listener." The specification makes timer primary and NOTIFY optional, because continuous aggregates cannot have triggers. This is technically correct and does not reduce functionality -- NOTIFY is still supported if available. Documented in ADR (prior ADR-004 in the existing brief).

---

## Version Discipline

- Target version: v1.2.0 (matches SCOPE.md)
- No v1.3 capabilities included (SONA, MCP, sysops)
- No v2.0 capabilities included (cross-domain)
- Roadmap alignment: SCOPE.md is Phase 2 subset of FEATURE-ROADMAPv1.2.md Track A + Track D

---

## Variances Requiring User Approval

None.
