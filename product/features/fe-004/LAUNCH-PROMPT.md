# Implementation Launch Prompt: fe-004

## Proposed Prompt

> Implement fe-004: Similarity Intelligence (V1.2 Phase 2)
> GitHub Issue: #18
> Brief: product/features/fe-004/IMPLEMENTATION-BRIEF.md
> Pattern IDs from planning: 17 (dual-backend), 18 (PredictionEngine), 19 (IntelligenceService), 22 (Docker), 24 (async-vs-sync), 39 (connection-pool), 40 (gold-query), 41 (error-propagation), 42 (config-extension), 43 (observation-persistence), 44 (dual-write-path), 45 (objective-metrics)
> Constraints: 256MB container limit, ARM64 (aarch64), ruvector feature-gated, intelligence config optional, no ndarray, no text/event embeddings
> Wave structure: Wave 1 (SimilarityEngine backends) -> Wave 2 (Prediction pipeline) -> Wave 3 (Orchestration) -> Wave 4 (App modes) -> Wave 5 (Deployment)

## Reminders for User

- Review ALIGNMENT-REPORT.md -- all 7 principles PASS, 0 variances
- SCOPE.md has 12 acceptance criteria; ACCEPTANCE-MAP.md maps each to verification commands
- fe-003 delivered the library foundation (traits, MetricEmbedder, PostgresStorage, pgvector DDL) -- fe-004 wires implementations to those traits
- The ndp-intelligence-app stubs (daemon, one-shot, backfill, status) need to be replaced with real implementations
- Existing 908 tests must continue passing

## Gotchas Discovered During Planning

- **Timer is primary, not NOTIFY**: CAs (continuous aggregates) cannot have triggers. PG NOTIFY is an optimization only. Timer fallback (20 min default) is the reliable wake mechanism. Do not depend on NOTIFY for correctness.
- **PgVectorEngine insert is a no-op**: Embeddings are already written via StorageBackend::store_embedding(). PgVectorEngine only reads for search. Do not duplicate writes.
- **Observation count persistence**: On restart, query gold.metric_embeddings count to restore warmup state. Do not use a state file. Also replay Gold rows to rebuild MetricEmbedder running stats.
- **SimilarityEngine trait is synchronous**: PgVectorEngine must use tokio::runtime::Handle::current().block_on() for async SQL in sync trait methods (ADR-008, AgentDB ID 24). This works because the trait is called from within an async context.
- **Backfill never predicts**: set_backfill_mode(true) skips search/predict/evaluate regardless of warmup state. Backfill is embed-only.
- **Domain view name derivation**: gold.{domain_id.replace('-', '_')}_aligned_hourly. Hyphens become underscores. Validate at startup.
- **ndp-intelligence-app test compilation**: Known pre-existing failure due to missing stream_type field. Do not attempt to fix in fe-004; work around by testing ndp-intelligence crate directly.
- **DualSimilarityEngine naming**: Despite the name, it only wraps HNSW. pgvector writes happen separately via StorageBackend. Consider renaming to HnswWithRebuild during implementation (ADR-014).
- **deadpool-postgres vs tokio-postgres**: The crate currently uses tokio-postgres directly. Add deadpool-postgres for connection pooling. StorageBackend (PostgresStorage) may need to accept a pool instead of a single client.
- **Domain config from etcd, not files**: Domain config (IntelligenceConfig + objectives) is loaded from etcd via `config-client`, NOT from local files. The `config/domains/` JSON files are source-of-truth that `deploy.sh` seeds into etcd. The intelligence library crate accepts parsed structs (no config-client dependency). Only the app binary uses config-client. `ETCD_ENDPOINTS` env var defaults to `http://etcd:2379`. The intelligence container `depends_on` etcd.
- **Objectives already exist in domain.json**: `config/domains/indoor-air-quality/domain.json` already has an `objectives` array with field/threshold/condition. PredictionEngine should read these from the domain config loaded via etcd, not from a separate intelligence-specific config block.
