# fe-003: Intelligence Foundation — Phase 0 + Phase 1

> **Parent**: `product/features/gold-002/IMPLEMENTATION-ROADMAP.md`
> **Architecture**: `product/features/gold-002/ARCHITECTURE.md`
> **Version target**: v1.2.0 (Phase 0 + Phase 1 are pre-release, library-only)
> **Created**: 2026-02-14

---

## Goal

Implement Phase 0 (Go/No-Go Gate) and Phase 1 (Foundation) of the V1.2 Intelligence Foundation. Phase 0 proves ruvector-core and ruvector-graph compile on aarch64. Phase 1 builds the crate structure, database schema, Embedder trait, MetricEmbedder, and storage infrastructure that all subsequent intelligence phases depend on.

---

## Phase 0: Go/No-Go Gate (1 day)

Prove ruvector-core and ruvector-graph compile and run on aarch64 (Pi 5).

### Deliverables

| ID | Task | Exit Criterion |
|----|------|---------------|
| P0-01 | Minimal Rust project with `ruvector-core = "2.0.1"` + `ruvector-graph = "0.1"` | Compiles with `cargo build --release` |
| P0-02 | Run on Pi 5 (native) or cross-compile for aarch64 | Binary executes without crash |
| P0-03 | Smoke test ruvector-core: insert 100 vectors, search, verify results | Correct K-NN results returned |
| P0-04 | Smoke test ruvector-graph: add nodes + edges, traverse neighbors | Correct traversal results |
| P0-05 | Measure: memory usage, search latency, build time | Documented in go/no-go report |

### Decision Gate

| Component | Outcome | Action |
|-----------|---------|--------|
| ruvector-core | Compiles and works | Primary HNSW backend |
| ruvector-core | SimSIMD fails, scalar fallback works | Proceed with `default-features = false, features = ["storage", "hnsw", "parallel"]` |
| ruvector-core | Fails entirely | pgvector-only mode (slower, functionally equivalent) |
| ruvector-graph | Compiles and works | Primary graph backend |
| ruvector-graph | Fails | SQL adjacency tables (`gold.graph_nodes`, `gold.graph_edges`) |

### Artifact

`product/features/fe-003/reports/phase0-go-no-go.md`

---

## Phase 1: Foundation (2-3 weeks)

Establish crate structure, database schema, Embedder trait, and MetricEmbedder. No runtime intelligence yet.

### Deliverables

| ID | Task | Description | Exit Criterion |
|----|------|-------------|---------------|
| P1-01 | Create `crates/ndp-intelligence` crate | Cargo.toml, lib.rs, module structure, workspace member | Compiles, `cargo test` passes |
| P1-02 | Create `apps/ndp-intelligence-app` crate | Cargo.toml, main.rs stub with clap CLI, workspace member | Compiles, `--help` works |
| P1-03 | Embedder trait + GoldRow types | `ndp-lib::gold::embeddings::mod.rs` — trait, GoldRow, Embedding | Unit tests pass |
| P1-04 | MetricEmbedder implementation | Z-score normalization, temporal encoding, NULL handling | Unit tests with known inputs/outputs |
| P1-05 | RunningStats for z-score | Exponential decay mean/std tracker | Statistical accuracy tests |
| P1-06 | EmbeddingConfig types | Config structs for embedding, deserialization from domain JSON | Deserialize tests with sample config |
| P1-07 | DomainConfig `intelligence` extension | Add optional `intelligence: Option<IntelligenceConfig>` to DomainConfig | Existing tests still pass, new field deserializes |
| P1-08 | pgvector schema DDL generator | `PgVectorSchemaGenerator` in ndp-lib — generates tables for metric_embeddings, predictions, graph (SQL fallback), reasoning_bank (V1.3 prep, empty) | DDL generation tests |
| P1-09 | pgvector extension in TimescaleDB Docker | Add `postgresql-15-pgvector` to Docker image, init script | Extension loads |
| P1-10 | StorageBackend trait + PostgresStorage | Trait definition + pgvector INSERT/SELECT for embeddings and predictions | Integration test against TimescaleDB |
| P1-11 | GraphStore trait + backend | `ndp-intelligence::graph` — trait + ruvector-graph or SQL adjacency backend (per Phase 0 outcome) | Node/edge CRUD + traversal tests |
| P1-12 | EmbeddingWriter populator | `ndp-lib::gold::populator::embedding_writer` — writes Embedding to PostgresStorage | Integration test: write + read round-trip |
| P1-13 | ndp-cli `gold intelligence` subcommand | `ndp gold intelligence schema` — generates pgvector + graph DDL | CLI outputs valid SQL |

### Architecture Constraints

- P1-03 through P1-05 are pure Rust with no database dependencies
- P1-07 must NOT break existing DomainConfig deserialization (`intelligence` field is `Option`)
- P1-08 follows the same pattern as existing `ContinuousAggregateGenerator`
- P1-10 requires the integration environment (TimescaleDB running)
- No code changes outside of new crates and `ndp-lib::gold` extensions
- Phase 1 does NOT deploy to Pi. Library-only. Tests run in integration environment.

### Config Change

`config/domains/indoor-air-quality/domain.json` gains an `intelligence` block (see ARCHITECTURE.md section 6). Existing fields unchanged.

---

## Non-Goals (explicitly out of scope)

- Runtime intelligence cycle (Phase 2)
- SimilarityEngine implementation (Phase 2)
- PredictionEngine (Phase 2)
- Docker container for intelligence daemon (Phase 2)
- EventEmbedder / MiniLM text pipeline (Phase 4)
- Granger causality (Phase 3)
- Anomaly detection (Phase 5)
- Grafana dashboards (Phase 5)
- SONA / ruv-fann integration (V1.3)

---

## Tracking

GitHub Issue: #17 — https://github.com/dug-21/neural-data-platform/issues/17
