# fe-004 Acceptance Criteria Map

> **Feature**: fe-004 — Similarity Intelligence (V1.2 Phase 2)
> **Source**: SCOPE.md Acceptance Criteria table
> **Date**: 2026-02-15

| AC-ID | Description | Verification Method | Verification Detail | Status |
|-------|-------------|--------------------|--------------------|--------|
| AC-01 | Embeddings generated — all aligned view hours embedded in gold.metric_embeddings | shell | `docker exec timescaledb psql -U postgres -d ndp -c "SELECT count(*) FROM gold.metric_embeddings WHERE domain_id = 'indoor-air-quality'" | grep -v '0'` | PENDING |
| AC-02 | Predictions generated after warmup — at least 1 prediction per hour after 168-hour warmup | shell | `docker exec timescaledb psql -U postgres -d ndp -c "SELECT count(*) FROM gold.predictions WHERE domain_id = 'indoor-air-quality' AND bucket > (SELECT MIN(bucket) + INTERVAL '168 hours' FROM gold.metric_embeddings WHERE domain_id = 'indoor-air-quality')"` | PENDING |
| AC-03 | Search latency HNSW < 1ms p99 | test | `cargo test -p ndp-intelligence test_hnsw_search_latency -- --nocapture` (insert 1000+ vectors, measure p99) | PENDING |
| AC-04 | Search latency pgvector fallback < 10ms p99 | test | `cargo test -p ndp-intelligence test_pgvector_search_latency --ignored -- --nocapture` (integration test against TimescaleDB) | PENDING |
| AC-05 | Full cycle latency < 500ms | test | `cargo test -p ndp-intelligence test_cycle_latency --ignored -- --nocapture` (verify CycleSummary.duration < 500ms) | PENDING |
| AC-06 | Memory usage daemon < 100 MB actual within 256 MB limit | shell | `docker stats ndp-intelligence --no-stream --format '{{.MemUsage}}'` (verify under 100MB after 10 min run) | PENDING |
| AC-07 | Prediction accuracy baseline logged (no target) | grep | `docker logs ndp-intelligence 2>&1 | grep 'EvaluationSummary'` (verify evaluated/correct/incorrect logged) | PENDING |
| AC-08 | Daemon runs without crash 10+ minutes | shell | `timeout 600 docker logs -f ndp-intelligence 2>&1 | tail -1` (no panic/crash in 10 min; exit code 0 from timeout) | PENDING |
| AC-09 | One-shot mode works — single cycle, produces predictions, exits 0 | shell | `docker exec ndp-intelligence ndp-intelligence one-shot --domain indoor-air-quality; echo $?` (verify exit code 0) | PENDING |
| AC-10 | Backfill mode works — processes N historical hours, generates embeddings | shell | `docker exec ndp-intelligence ndp-intelligence backfill --domain indoor-air-quality --since 2026-01-01T00:00:00Z; echo $?` (verify exit 0 and embedding count increased) | PENDING |
| AC-11 | Docker container builds on x86_64 and aarch64 | shell | `docker build -f docker/intelligence/Dockerfile .` (x86_64) and `deploy/pi/deploy.sh` (aarch64 on Pi) | PENDING |
| AC-12 | deploy.sh deploys intelligence — new service starts alongside existing | shell | `ssh pi 'docker ps --filter name=ndp-intelligence --format "{{.Status}}"'` (verify "Up" status) | PENDING |
