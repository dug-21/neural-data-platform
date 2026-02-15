# fe-004 Architecture: Similarity Intelligence

> **Feature**: fe-004
> **Date**: 2026-02-15
> **Prior ADRs**: AgentDB IDs 17 (dual-backend), 18 (PredictionEngine), 19 (IntelligenceService), 22 (Docker), 24 (async vs sync)

---

## Prior ADRs (Confirmed)

The following ADRs from prior planning cycles remain valid and are not repeated here. Reference them by AgentDB pattern ID during implementation.

| ADR | AgentDB ID | Decision |
|-----|-----------|----------|
| fe-004-001: SimilarityEngine Dual-Backend Dispatch | 17 | Dual-write to HNSW (fast) + pgvector (durable); reads prefer HNSW |
| fe-004-002: PredictionEngine Design | 18 | SQL per-neighbor outcome lookup; confidence = k_supporting/k_total |
| fe-004-003: IntelligenceService Orchestration | 19 | Single struct coordinates observe-embed-store-search-predict-evaluate cycle |
| fe-004-006: Docker Container Architecture | 22 | Separate container, 256MB limit, depends on timescaledb |
| fe-004-008: Async vs Sync SimilarityEngine Trait | 24 | Sync trait; PgVectorEngine uses block_on internally |

---

## New ADRs

### ADR-009: Connection Pool Strategy

#### Context

The intelligence service needs PostgreSQL connections for: reading Gold data, writing embeddings (via StorageBackend), reading embeddings (for HNSW rebuild), K-NN search (PgVectorEngine), outcome evaluation, and PG NOTIFY listening. On the Pi, connection count matters -- PostgreSQL default `max_connections` is 100, shared with the ingestion pipeline and Grafana.

#### Decision

Use `deadpool-postgres` as the connection pool. Configure with:
- **Pool size**: 2 connections (default), configurable via `INTELLIGENCE_POOL_SIZE` env var
- **Shared pool**: A single `Arc<Pool>` is passed to all components (StorageBackend, PgVectorEngine, PredictionEngine, OutcomeTracker)
- **NOTIFY connection**: Separate dedicated connection (not pooled), because LISTEN requires a persistent connection
- **Connection string**: From `DATABASE_URL` env var (same as used by ingestion pipeline)

Pool is created once in `main.rs` and passed to `IntelligenceService::new()`.

#### Consequences

- +: Minimal connection count (2 pooled + 1 NOTIFY = 3 total)
- +: Single configuration point for database connection
- +: deadpool-postgres is already production-proven and lightweight
- -: NOTIFY connection is outside the pool (manual reconnection logic needed)
- -: Pool size 2 means sequential access if both connections are checked out; acceptable for the intelligence cycle which is inherently sequential

---

### ADR-010: Gold Row Query Strategy

#### Context

The intelligence service must query the Gold aligned view for new rows. The aligned view is a materialized view (or continuous aggregate) that combines multiple streams into a single time-aligned row per hour. The intelligence service needs to:
1. Detect new rows (since last processed)
2. Read field values for embedding generation
3. Read field values for outcome evaluation

The aligned view schema is domain-specific (field names come from stream configuration). The intelligence service should not hardcode field names.

#### Decision

Query the Gold aligned view using dynamic SQL built from the intelligence config's field list. The query strategy:

1. **New row detection**: `SELECT * FROM gold.{domain}_aligned_hourly WHERE bucket > $1 ORDER BY bucket ASC LIMIT 100`
2. **Field extraction**: Read all columns returned by the query; map to `GoldRow.fields` by column name
3. **Domain view name**: Derived from `domain_id` config: `gold.{domain_id.replace('-', '_')}_aligned_hourly`
4. **Column discovery**: At startup, query `information_schema.columns` for the view to validate that configured embedding fields exist

The query returns `tokio_postgres::Row` objects. A helper function `sql_row_to_gold_row(row: &Row, domain_id: &str) -> GoldRow` converts each row to the `GoldRow` type used by `MetricEmbedder`.

#### Consequences

- +: No hardcoded field names; works for any domain's aligned view
- +: Column validation at startup catches config errors early
- +: Batch querying (LIMIT 100) prevents memory spikes
- -: Dynamic SQL construction requires care to prevent injection (use parameterized queries for values; view name is derived from config, not user input)
- -: Schema changes to the aligned view require config updates

---

### ADR-011: Error Propagation Architecture

#### Context

The intelligence service has multiple error sources: database, embedding, similarity engine, configuration. Errors can be fatal (service must stop) or recoverable (skip one cycle, continue). The existing `IntelligenceError` enum in `error.rs` needs extension.

#### Decision

Extend `IntelligenceError` with new variants:

```rust
pub enum IntelligenceError {
    // Existing
    Storage(StorageError),
    Embedding(EmbeddingError),
    Similarity(SimilarityError),
    // New
    Database(String),       // Raw database errors (connection, query)
    Config(String),         // Configuration validation failures
    Shutdown,               // Graceful shutdown signal received
}
```

Error handling strategy per component:
- **Startup errors** (config, connection, HNSW rebuild): Fatal. Log ERROR, exit 1.
- **Cycle errors** (individual row embedding, search, prediction): Recoverable. Log WARN, skip that item, continue cycle.
- **Storage errors** (write failures): Recoverable per-item. Log ERROR, continue cycle.
- **Connection loss** (pool exhausted, DB down): Fatal after 3 retries. Log ERROR, exit 1.

All error paths include structured context: domain_id, bucket timestamp, component name.

#### Consequences

- +: Clear distinction between fatal and recoverable errors
- +: Structured error context aids debugging on headless Pi
- +: Service stays running through transient per-row failures
- -: 3-retry logic adds complexity to the main loop

---

### ADR-012: Intelligence Config Extension

#### Context

The existing `IntelligenceConfig` (in `ndp-lib/src/gold/embeddings/config.rs`) defines embedding and search settings. The intelligence daemon also needs runtime configuration: database URL, poll interval, warmup threshold, pool size. These runtime settings should not be in the domain config (which is about data shape, not deployment).

The platform uses etcd as the single source of truth for configuration. The `config-client` crate already provides `ConfigClient` with typed `get<T>()`, `watch()`, and env var override support. `StreamRegistry` already loads stream configs from etcd via this pattern. Domain config should follow the same path.

#### Decision

Two configuration layers:

1. **Domain config** (`DomainConfig` containing `IntelligenceConfig`): Embedding type, fields, search K, horizons, objectives. Loaded from etcd via `config-client` at key `/domains/{domain_id}/config`. The `config/domains/` directory contains the source-of-truth JSON files that `deploy.sh` seeds into etcd during deployment.

2. **Runtime config** (`AppConfig` in ndp-intelligence-app): Database URL, etcd endpoints, poll interval, pool size, warmup threshold. Read from environment variables.

```rust
// apps/ndp-intelligence-app/src/config.rs (binary-level, not library)
pub struct AppConfig {
    pub database_url: String,
    pub domain_id: String,
    pub etcd_endpoints: Vec<String>, // default: ["http://etcd:2379"]
    pub poll_interval_secs: u64,     // default: 1200 (20 min)
    pub pool_size: usize,            // default: 2
    pub warmup_threshold: usize,     // default: 168
}

impl AppConfig {
    pub fn from_env() -> Result<Self>;
}
```

Config loading flow in `main.rs`:
```rust
let app_config = AppConfig::from_env()?;
let config_client = ConfigClient::new(&app_config.etcd_endpoints).await?;
let domain_config: DomainConfig = config_client
    .get(&format!("/domains/{}/config", app_config.domain_id))
    .await?;
let intel_config = domain_config.intelligence
    .ok_or_else(|| ConfigError("intelligence block not found in domain config"))?;
```

Environment variables:
- `DATABASE_URL` (required)
- `INTELLIGENCE_DOMAIN` (required)
- `ETCD_ENDPOINTS` (default: `http://etcd:2379`)
- `INTELLIGENCE_POLL_INTERVAL_SECS` (default: 1200)
- `INTELLIGENCE_POOL_SIZE` (default: 2)
- `INTELLIGENCE_WARMUP_THRESHOLD` (default: 168)

The `ndp-intelligence` library crate does NOT depend on `config-client`. It accepts parsed `&IntelligenceConfig` structs. Only the `ndp-intelligence-app` binary depends on `config-client` for config loading. This follows the established pattern: "ndp-lib functions take parsed structs, not file paths" — the intelligence library is source-agnostic.

#### Consequences

- +: Consistent with platform config strategy (etcd as single source of truth)
- +: Reuses existing `config-client` crate — no new config loading code
- +: Domain config changes propagate via etcd without redeployment (future: watch support)
- +: Library crate remains config-source-agnostic (accepts parsed structs)
- +: Existing stream configs continue to work unchanged (intelligence block is optional)
- -: Intelligence container depends on etcd being available at startup
- -: deploy.sh must seed domain config into etcd before starting intelligence

---

### ADR-013: Observation Count Persistence

#### Context

The warmup threshold is 168 observations. If the daemon restarts, it must not lose its observation count -- otherwise warmup restarts from zero and predictions are delayed by another 168 hours.

#### Decision

Persist observation count by querying existing data. On startup:

```sql
SELECT count(*) FROM gold.metric_embeddings WHERE domain_id = $1
```

This count is loaded into `IntelligenceService.observation_count`. Since every observed+embedded row is written to `gold.metric_embeddings`, the count in the database IS the observation count. No separate state file or counter table needed.

Additionally, `MetricEmbedder` running stats are rebuilt during startup by replaying historical embeddings. The `rebuild_from_storage` call that loads HNSW also feeds `embedder.observe()` for each loaded row, rebuilding the z-score statistics.

#### Consequences

- +: Zero additional storage; database is the source of truth
- +: Restarts are seamless; no state files to manage
- +: Works correctly with backfill (backfilled rows increment the count)
- -: First startup after fresh deploy requires a COUNT query (fast with hypertable index)
- -: Running stats rebuild adds O(n) startup cost; acceptable for <100K rows

---

### ADR-014: DualSimilarityEngine Write Path

#### Context

ADR-001 (AgentDB ID 17) established dual-write to HNSW + pgvector. The question is: does DualSimilarityEngine handle the pgvector write, or does the caller handle it separately?

#### Decision

DualSimilarityEngine does NOT write to pgvector. The write path is:

1. `IntelligenceService` calls `StorageBackend::store_embedding()` -- this writes to pgvector
2. `IntelligenceService` calls `SimilarityEngine::insert()` on DualSimilarityEngine -- this writes to HNSW only

DualSimilarityEngine wraps only HnswEngine for writes. For reads (search), it delegates to HnswEngine. PgVectorEngine is used only as a standalone fallback (no ruvector feature).

This is simpler than having DualSimilarityEngine manage both backends, because the StorageBackend already handles pgvector writes. Duplicating that in DualSimilarityEngine would create two write paths to the same table.

#### Consequences

- +: No duplicate pgvector writes
- +: DualSimilarityEngine is thin (just HNSW + StorageBackend ref for rebuild)
- +: Clear responsibility: StorageBackend owns pgvector; SimilarityEngine owns HNSW
- -: The name "DualSimilarityEngine" is slightly misleading (it's really "HNSW with pgvector rebuild source"); consider renaming to `HnswWithRebuild` during implementation

---

### ADR-015: Objective Metrics Configuration

#### Context

PredictionEngine needs to know WHICH metrics to predict and what thresholds constitute a "breach." This is domain-specific: air quality cares about PM2.5 > 35 ug/m3 and CO2 > 1000 ppm. Other domains will have different metrics and thresholds.

#### Decision

Add an `objectives` section to the intelligence config:

```json
{
  "intelligence": {
    "objectives": [
      {
        "field": "pm25_mean",
        "threshold": 35.0,
        "direction": "above",
        "label": "PM2.5 unhealthy"
      },
      {
        "field": "co2_mean",
        "threshold": 1000.0,
        "direction": "above",
        "label": "CO2 high"
      }
    ]
  }
}
```

PredictionEngine reads this list and generates predictions for each objective metric. If no objectives are configured, predictions are disabled (warn log).

#### Consequences

- +: Config-driven; no hardcoded thresholds
- +: Domain-portable; new domains define their own objectives
- +: Labels improve prediction logging readability
- -: Requires at least one objective to produce meaningful predictions
