# ADR-002: Database Connection Management for ETL Run Persistence

## Status

Accepted

## Context

The silver-etl application needs to persist run statistics to `silver.etl_runs` in TimescaleDB. We need to decide how to manage database connections for this persistence:

### Current Architecture

```
EtlRunner (etl.rs)
├── conn: DuckDB Connection (in-memory)
├── pg_conn_str: Option<String>
└── postgres_attached: bool (DuckDB ATTACH for data writes)

RealEtlExecutor (daemon.rs)
├── runner: EtlRunner
├── config_loader: ConfigLoader
└── bronze_dir: String
```

**Current data flow**: DuckDB reads Bronze Parquet, transforms data, writes to PostgreSQL via DuckDB's `ATTACH ... (TYPE postgres)`.

### Options

**Option A**: Reuse DuckDB's PostgreSQL attachment
- Run statistics INSERT via DuckDB SQL: `INSERT INTO pg.silver.etl_runs ...`
- Pros: No new dependencies, single connection path
- Cons: DuckDB's postgres extension has limitations, stats writes mixed with data writes

**Option B**: Separate direct PostgreSQL connection (tokio-postgres)
- New connection specifically for run persistence
- Pros: Better transaction control, failure isolation, async-native
- Cons: New dependency, connection pooling overhead

**Option C**: Hybrid - DuckDB for data, tokio-postgres for stats
- Use DuckDB attachment for ETL data writes (current)
- Use tokio-postgres for run statistics (new)
- Pros: Best of both, failure isolation between concerns
- Cons: Two connection strategies to maintain

## Decision

**Adopt Option C: Hybrid Connection Strategy**

### Rationale

1. **Separation of Concerns**: ETL data writes are batch/bulk operations; stats writes are single-row CRUD
2. **Failure Isolation**: If stats persistence fails, ETL data writes should continue unaffected
3. **Async Compatibility**: tokio-postgres is async-native, better suited for daemon context
4. **Transaction Semantics**: Stats need simple INSERT/UPDATE; data needs bulk COPY semantics

### Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        silver-etl daemon                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────┐          ┌──────────────────────────────────┐ │
│  │   EtlRunner      │          │     EtlRunPersistence            │ │
│  │                  │          │     (tokio-postgres)             │ │
│  │ ┌──────────────┐ │          │                                  │ │
│  │ │ DuckDB conn  │ │          │  ┌───────────────────────────┐  │ │
│  │ │ + ATTACH pg  │──────┬────▶│  │ PostgreSQL Pool (bb8)    │  │ │
│  │ └──────────────┘ │    │     │  │ - Shared connection pool  │  │ │
│  │                  │    │     │  │ - Max 2 connections       │  │ │
│  └──────────────────┘    │     │  │ - Connection timeout 5s   │  │ │
│         │                │     │  └───────────────────────────┘  │ │
│         │ Bulk INSERT    │     │                                  │ │
│         │ (ETL data)     │     │  - start_run() INSERT           │ │
│         ▼                │     │  - complete_run() UPDATE        │ │
│  ┌──────────────────┐    │     │  - fail_run() UPDATE            │ │
│  │ silver.*        │    │     │                                  │ │
│  │ (data tables)   │    │     └─────────────┬────────────────────┘ │
│  └──────────────────┘    │                   │                      │
│                          │                   │ Single-row ops       │
│                          │                   ▼                      │
│                          │     ┌──────────────────────────────────┐ │
│                          └────▶│ silver.etl_runs                  │ │
│                                │ (run statistics)                 │ │
│                                └──────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────┘
```

### Connection Pool Configuration

```rust
// Recommended pool settings for Pi 5
let pool_config = bb8_postgres::PostgresConnectionPool::builder()
    .max_size(2)                    // Low: only for stats, not bulk data
    .min_idle(1)                    // Keep one warm
    .connection_timeout(Duration::from_secs(5))
    .idle_timeout(Some(Duration::from_secs(600)))  // 10 min idle
    .build(pg_config)
    .await?;
```

### Trait Design (London TDD)

```rust
/// Trait for ETL run persistence - enables mocking in tests
#[async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait EtlRunPersistence: Send + Sync {
    /// Insert a new run record (status = 'running')
    /// Returns the run ID for later updates
    async fn start_run(
        &self,
        stream_id: &str,
        run_mode: EtlRunMode,
        daemon_cycle_id: Option<Uuid>,
    ) -> Result<Uuid, PersistenceError>;

    /// Update run with success status and final stats
    async fn complete_run(
        &self,
        id: Uuid,
        stats: &EtlStats,
    ) -> Result<(), PersistenceError>;

    /// Update run with failure status and error details
    async fn fail_run(
        &self,
        id: Uuid,
        error_message: &str,
        error_context: Option<serde_json::Value>,
    ) -> Result<(), PersistenceError>;
}
```

### Implementation Structure

```
apps/silver-etl/src/
├── persistence/
│   ├── mod.rs           # Module exports
│   ├── traits.rs        # EtlRunPersistence trait
│   ├── postgres.rs      # PostgresRunPersistence implementation
│   └── error.rs         # PersistenceError type
└── daemon.rs            # Updated to use persistence
```

## Consequences

### Benefits

1. **Clean separation** - Data writes via DuckDB, metadata writes via tokio-postgres
2. **Testability** - Mock `EtlRunPersistence` trait for unit tests
3. **Resilience** - Stats persistence failure doesn't block ETL execution
4. **Async-native** - tokio-postgres fits daemon's async runtime

### Costs

1. **New dependency** - `tokio-postgres` + `bb8` for connection pooling
2. **Two connection configs** - Need to manage both DuckDB attach and postgres pool
3. **Slightly more complexity** - But well-isolated to persistence module

### Resource Impact (Pi 5)

| Resource | Impact |
|----------|--------|
| Memory | +2-4MB for connection pool |
| CPU | Negligible (single-row ops) |
| Connections | +2 max to PostgreSQL |
| Latency | ~5ms per start/complete call |

### Dependencies to Add

```toml
# Cargo.toml additions
[dependencies]
tokio-postgres = { version = "0.7", features = ["with-uuid-1", "with-chrono-0_4", "with-serde_json-1"] }
bb8 = "0.8"
bb8-postgres = "0.8"
```

## Alternatives Considered

### Option A: DuckDB Only

```sql
-- Would work but mixing concerns
INSERT INTO pg.silver.etl_runs (stream_id, started_at, status)
VALUES ('air-quality', NOW(), 'running');
```

**Rejected**: DuckDB's PostgreSQL extension is optimized for bulk operations, not transactional single-row updates. Error handling is also awkward (need to parse DuckDB error messages for PostgreSQL errors).

### Option B: tokio-postgres for Everything

**Considered**: Replace DuckDB's ATTACH with direct tokio-postgres for data writes too.

**Rejected**: Would require rewriting ETL SQL generation, losing DuckDB's excellent Parquet reading and in-memory transforms. Significant scope creep.

### Shared Connection via RealEtlExecutor

**Considered**: Pass postgres pool through RealEtlExecutor to EtlRunner.

**Rejected**: Violates single responsibility - EtlRunner does transforms, not persistence metadata. Better to have daemon orchestrate persistence around EtlRunner calls.

## Implementation Notes

### Error Handling Strategy

```rust
// In daemon.rs run_cycle()
let run_id = match persistence.start_run(&stream_id, run_mode, cycle_id).await {
    Ok(id) => Some(id),
    Err(e) => {
        warn!(stream_id = %stream_id, error = %e, "Failed to record run start");
        None  // Continue ETL even if stats recording fails
    }
};

// After ETL execution
if let Some(id) = run_id {
    if let Err(e) = persistence.complete_run(id, &stats).await {
        error!(run_id = %id, error = %e, "Failed to record run completion");
    }
}
```

### Configuration

```bash
# Environment variables (same as existing)
TIMESCALE_URL=postgresql://ndp:secret@localhost:5432/ndp

# Or component-wise
NDP_TIMESCALE_HOST=localhost
NDP_TIMESCALE_PORT=5432
NDP_TIMESCALE_DB=ndp
NDP_TIMESCALE_USER=ndp
NDP_TIMESCALE_PASSWORD=secret
```

Reuse existing environment variable pattern from `EtlRunner::from_env()`.

## Related ADRs

- ADR-001: Persistence Strategy (what we persist)
- ADR-003: Run Lifecycle (when we persist)

## References

- [tokio-postgres documentation](https://docs.rs/tokio-postgres)
- [bb8 connection pool](https://docs.rs/bb8)
- [Current EtlRunner connection code](../../../../apps/silver-etl/src/etl.rs#L150-L174)
