# fe-004 Task Decomposition: Similarity Intelligence

> **Feature**: fe-004
> **Date**: 2026-02-15

---

## Wave Structure

### Wave 1: SimilarityEngine Implementations (no external dependencies beyond fe-003)

| Task | Deliverable | Files | Complexity | Dependencies |
|------|-------------|-------|-----------|-------------|
| 1.1 | P2-02: HnswEngine | `crates/ndp-intelligence/src/similarity/hnsw.rs` | Medium | ruvector-core crate |
| 1.2 | P2-03: PgVectorEngine | `crates/ndp-intelligence/src/similarity/pgvector.rs` | Medium | tokio-postgres pool |
| 1.3 | P2-04: HNSW rebuild | Part of `hnsw.rs` | Low | 1.1 |
| 1.4 | P2-01: Factory + DualSimilarityEngine | `crates/ndp-intelligence/src/similarity/mod.rs`, `crates/ndp-intelligence/src/similarity/dual.rs` | Medium | 1.1, 1.2 |
| 1.5 | Update Cargo.toml | `crates/ndp-intelligence/Cargo.toml` | Low | -- |

**Wave 1 exit criteria**: `cargo test -p ndp-intelligence` passes with unit tests for all three engine types.

---

### Wave 2: Prediction Pipeline

| Task | Deliverable | Files | Complexity | Dependencies |
|------|-------------|-------|-----------|-------------|
| 2.1 | P2-05/P2-06: PredictionEngine + confidence | `crates/ndp-intelligence/src/predictions/mod.rs` | High | Wave 1 (SearchResult type) |
| 2.2 | P2-07: Prediction storage wiring | Part of `predictions/mod.rs` | Low | 2.1 |
| 2.3 | P2-08: OutcomeTracker | `crates/ndp-intelligence/src/predictions/outcome.rs` | Medium | 2.1 |
| 2.4 | P2-15: Warmup logic | Part of service.rs (Wave 3) | Low | -- |

**Wave 2 exit criteria**: Unit tests pass for PredictionEngine (mock neighbors), OutcomeTracker (mock storage), confidence scoring.

---

### Wave 3: Orchestration

| Task | Deliverable | Files | Complexity | Dependencies |
|------|-------------|-------|-----------|-------------|
| 3.1 | P2-09: IntelligenceService | `crates/ndp-intelligence/src/service.rs` | High | Wave 1, Wave 2 |
| 3.2 | P2-10: PG NOTIFY listener | `crates/ndp-intelligence/src/notify.rs` | Medium | tokio-postgres |
| 3.3 | P2-11: Timer fallback | Part of `service.rs` or app main | Low | 3.1 |
| 3.4 | Runtime config (env vars + etcd) | `apps/ndp-intelligence-app/src/config.rs` | Medium | config-client crate |

**Wave 3 exit criteria**: IntelligenceService::run_cycle() compiles and unit tests pass. NotifyListener compiles.

---

### Wave 4: App Modes

| Task | Deliverable | Files | Complexity | Dependencies |
|------|-------------|-------|-----------|-------------|
| 4.1 | P2-12: Daemon mode | `apps/ndp-intelligence-app/src/main.rs` | Medium | Wave 3 |
| 4.2 | P2-13: One-shot mode | `apps/ndp-intelligence-app/src/main.rs` | Low | Wave 3 |
| 4.3 | P2-14: Backfill mode | `apps/ndp-intelligence-app/src/main.rs` | Medium | Wave 3 |
| 4.4 | App Cargo.toml updates | `apps/ndp-intelligence-app/Cargo.toml` | Low | -- |

**Wave 4 exit criteria**: `cargo build -p ndp-intelligence-app` succeeds. CLI subcommands are functional (not stubs).

---

### Wave 5: Deployment

| Task | Deliverable | Files | Complexity | Dependencies |
|------|-------------|-------|-----------|-------------|
| 5.1 | P2-16: Dockerfile | `docker/intelligence/Dockerfile` | Medium | Wave 4 |
| 5.2 | P2-16: docker-compose service | `docker-compose.yml`, `docker-compose.integration.yml` | Low | 5.1 |
| 5.3 | P2-17: deploy.sh integration | `deploy/pi/deploy.sh` | Low | 5.1 |
| 5.4 | Domain config update + etcd seeding | `config/domains/indoor-air-quality/domain.json`, `deploy/pi/deploy.sh` | Low | config-client |
| 5.5 | Integration tests | `crates/ndp-intelligence/tests/` | High | Wave 1-4, 5.2 |

**Wave 5 exit criteria**: Docker container builds on x86_64. Integration tests pass against TimescaleDB. deploy.sh has intelligence service.

---

## File-Level Change Summary

### New Files (10)

| File | Wave | Description |
|------|------|-------------|
| `crates/ndp-intelligence/src/similarity/hnsw.rs` | 1 | HnswEngine: ruvector-core wrapper |
| `crates/ndp-intelligence/src/similarity/pgvector.rs` | 1 | PgVectorEngine: SQL K-NN |
| `crates/ndp-intelligence/src/similarity/dual.rs` | 1 | DualSimilarityEngine: dual-write |
| `crates/ndp-intelligence/src/predictions/mod.rs` | 2 | PredictionEngine + confidence |
| `crates/ndp-intelligence/src/predictions/outcome.rs` | 2 | OutcomeTracker |
| `crates/ndp-intelligence/src/service.rs` | 3 | IntelligenceService orchestrator |
| `crates/ndp-intelligence/src/notify.rs` | 3 | PG NOTIFY listener |
| `apps/ndp-intelligence-app/src/config.rs` | 3 | Runtime config (env vars, etcd loading, AppConfig) |
| `docker/intelligence/Dockerfile` | 5 | Multi-stage build |
| `config/domains/indoor-air-quality/domain.json` | 5 | Add intelligence config block (file already exists, extend it) |

### Modified Files (8)

| File | Wave | Change |
|------|------|--------|
| `crates/ndp-intelligence/src/lib.rs` | 1 | Add `pub mod predictions; pub mod service; pub mod notify;` |
| `crates/ndp-intelligence/src/similarity/mod.rs` | 1 | Add submodule declarations + factory function |
| `crates/ndp-intelligence/Cargo.toml` | 1 | Add runtime deps (deadpool-postgres, tokio features) |
| `apps/ndp-intelligence-app/src/main.rs` | 4 | Replace stubs with real implementations |
| `apps/ndp-intelligence-app/Cargo.toml` | 4 | Add deps: config-client, signal handling |
| `docker-compose.yml` | 5 | Add ndp-intelligence service |
| `docker-compose.integration.yml` | 5 | Add ndp-intelligence for testing |
| `deploy/pi/deploy.sh` | 5 | Add intelligence service deployment |

---

## Estimated Effort

| Wave | Tasks | Complexity | Estimated Effort |
|------|-------|-----------|-----------------|
| Wave 1 | 5 | Medium | 1-2 days |
| Wave 2 | 4 | Medium-High | 1-2 days |
| Wave 3 | 4 | High | 1-2 days |
| Wave 4 | 4 | Medium | 1 day |
| Wave 5 | 5 | Medium-High | 1-2 days |
| **Total** | **22** | | **5-9 days** |

---

## Risk Items

| Risk | Wave | Mitigation |
|------|------|-----------|
| ruvector-core API differences from expected | 1 | Feature-gated; PgVectorEngine always works as fallback |
| Gold aligned view schema assumptions | 3 | Query schema from information_schema at startup |
| Connection pool sizing for Pi | 3 | Default pool size 2, configurable via env |
| Backfill memory usage with large history | 4 | Batch processing (100 rows at a time) |
| Docker aarch64 cross-compilation | 5 | Build on Pi directly via deploy.sh |
