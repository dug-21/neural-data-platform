# fe-004: Similarity Intelligence (V1.2 Phase 2)

## Vision

Deliver the first end-to-end intelligence cycle: a running daemon that reads Gold aligned view data, generates metric embeddings, performs K-NN similarity search, produces predictions with confidence scores, tracks outcomes, and evaluates accuracy. After a 168-hour warmup period, the system generates hourly predictions that answer: "Given conditions like now, what happened next in the past?"

This is the first phase that delivers user-visible value. It transforms the library foundation from fe-003 into a running intelligence service deployed on the Pi.

## Tracking

- Feature: fe-004
- GitHub Issue: https://github.com/dug-21/neural-data-platform/issues/18
- Parent feature: gold-002
- Parent roadmap: `product/features/gold-002/IMPLEMENTATION-ROADMAP.md` (Phase 2)
- Parent architecture: `product/features/gold-002/ARCHITECTURE.md`
- Predecessor: fe-003 (Phase 0+1, GH Issue #17)
- Version target: v1.2.0

## What fe-003 Delivered (Prerequisites)

All of these exist and are tested:

- `crates/ndp-intelligence/` — library crate with traits (StorageBackend, GraphStore, SimilarityEngine)
- `apps/ndp-intelligence-app/` — binary with clap CLI (daemon/one-shot/backfill/status subcommands, stubs only)
- `crates/ndp-lib/src/gold/embeddings/` — Embedder trait, MetricEmbedder, RunningStats, EmbeddingConfig
- `crates/ndp-lib/src/gold/generators/pgvector_schema.rs` — DDL generator for intelligence tables
- `crates/ndp-intelligence/src/storage/postgres.rs` — PostgresStorage (pgvector insert/load, predictions CRUD)
- `crates/ndp-intelligence/src/graph/sql.rs` — SqlGraphStore (SQL adjacency tables)
- `crates/ndp-intelligence/src/populator/embedding_writer.rs` — EmbeddingWriter
- 80+ unit tests passing, 11 integration tests (ignored, need TimescaleDB)

## Phase 2 Deliverables (from IMPLEMENTATION-ROADMAP.md)

| ID | Task | Description |
|----|------|-------------|
| P2-01 | SimilarityEngine trait | Already exists (fe-003). Wire implementations. |
| P2-02 | HnswEngine (ruvector-core) | Wrapper around ruvector-core VectorDB |
| P2-03 | PgVectorEngine (fallback) | pgvector SQL-based K-NN search |
| P2-04 | HNSW index rebuild from pgvector | On startup, load all embeddings from pgvector into HNSW |
| P2-05 | PredictionEngine | K-NN results → outcome lookup → prediction generation |
| P2-06 | Confidence scoring | Prediction confidence = supporting_neighbors / total_neighbors |
| P2-07 | Prediction storage | Write predictions to gold.predictions |
| P2-08 | Outcome tracker | Evaluate predictions after horizon elapsed, update correctness |
| P2-09 | IntelligenceService orchestrator | Coordinates: observe → embed → store → search → predict → evaluate |
| P2-10 | PG NOTIFY listener | LISTEN gold_refresh with reconnection logic |
| P2-11 | Timer fallback | If no NOTIFY within 20 min, poll on timer |
| P2-12 | Daemon mode | ndp-intelligence-app daemon — continuous loop |
| P2-13 | One-shot mode | ndp-intelligence-app one-shot — single cycle then exit |
| P2-14 | Backfill mode | ndp-intelligence-app backfill --since <date> |
| P2-15 | Warmup logic | Skip predictions during warmup (first 168 hours) |
| P2-16 | Docker container | docker/intelligence/Dockerfile + docker-compose service |
| P2-17 | deploy.sh integration | Add ndp-intelligence to deployment script |

## Constraints

- Must run on Raspberry Pi 5 (16GB RAM, 1TB NVMe)
- Intelligence container must be separate from ingestion (workload isolation)
- Container memory limit: 256 MB (Phase 2 is metric-only, no MiniLM)
- No model training required (K-NN is training-free)
- pgvector is the durable storage baseline; ruvector-core is acceleration layer
- Phase 0 outcome: SQL fallback chosen (ruvector-graph not compiled). SimilarityEngine must support both HNSW and pgvector-only paths
- Must NOT break existing ingestion pipeline
- Must NOT require schema migration beyond what PgVectorSchemaGenerator already produces
- Intelligence config block must be optional in DomainConfig (existing configs must continue to work)
- All Rust code must compile for both x86_64 (dev) and aarch64 (Pi)
- Feature-gated ruvector dependency: `ruvector-core` behind `ruvector` feature flag

## Acceptance Criteria

| Criterion | Target |
|-----------|--------|
| Embeddings generated | All aligned view hours embedded in gold.metric_embeddings |
| Predictions generated after warmup | At least 1 prediction per hour after 168-hour warmup |
| Search latency (HNSW) | <1ms p99 |
| Search latency (pgvector fallback) | <10ms p99 |
| Full cycle latency | <500ms total |
| Memory usage (daemon) | <100 MB actual, within 256 MB limit |
| Prediction accuracy baseline | Logged (no target — baseline establishment) |
| Daemon runs without crash | 10+ minutes continuous operation |
| One-shot mode works | Single cycle, produces predictions, exits 0 |
| Backfill mode works | Processes N historical hours, generates embeddings |
| Docker container builds | On both x86_64 and aarch64 |
| deploy.sh deploys intelligence | New service starts alongside existing services |

## Out of Scope

- Event intelligence / text embeddings (Phase 4, fe-006+)
- Granger causality (Phase 3, fe-005)
- Anomaly detection (Phase 5)
- Grafana dashboards (Phase 5)
- SONA learning (V1.3)
- MCP query interface (V1.3)
- Sysops domain (V1.3)

## Release

v1.2.0 — First intelligence release. Deploys to Pi. Begins warmup period.
