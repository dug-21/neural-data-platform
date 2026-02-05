# AIR-014: Self-Healing Silver ETL

## Problem Statement

When TimescaleDB becomes unavailable while Bronze ingestion continues, the SilverSubscriber (embedded in air-quality-app) drops events without retry or recovery. When the database recovers, a data gap remains in the Silver layer that requires manual intervention to resolve.

**Current behavior (observed):**
- SilverSubscriber processes events from EventBus in real-time
- On write failure: logs error, drops event, continues
- No retry mechanism, no dead letter queue, no buffering
- Catch-up capability exists in code but is disabled (`NoBronzeReader` stub)
- Result: Unrecoverable gap in Silver data during any TimescaleDB outage

## Desired Behavior

Self-healing recovery that automatically catches up Silver with Bronze after a failure:

1. **Detect** sustained write failures (circuit breaker pattern)
2. **Preserve** knowledge of last successful write (watermark persistence)
3. **Recover** automatically when database becomes healthy
4. **Catch-up** from Bronze Parquet files to fill the gap
5. **Log** all recovery activity for operational transparency
6. **Resume** normal real-time processing seamlessly

## Scope

### In Scope

- **ParquetBronzeReader**: Implement `BronzeReader` trait using existing `ParquetStore`
- **Circuit Breaker**: Failure detection with configurable thresholds
- **Continuous Watermark Persistence**: Save progress periodically, not just on shutdown
- **Health-Triggered Recovery**: Automatic catch-up when DB health transitions to healthy
- **Configuration**: All thresholds and behaviors configurable via stream config
- **Observability**: Structured logging for all state transitions and recovery actions

### Out of Scope

- Changes to Bronze layer or EventBus
- New external dependencies (use existing tokio, bb8, etc.)
- Kafka/Redis message queue (keep it simple - Bronze Parquet is the source of truth)
- Multi-node coordination (single app instance recovery only)

## Success Criteria

1. **Zero manual intervention** required after TimescaleDB recovery
2. **No duplicate data** in Silver after catch-up (deduplication via UPSERT)
3. **Gap filled within** catch-up window (configurable, default 2 hours)
4. **Logs clearly show** circuit breaker state transitions and catch-up progress
5. **Existing tests pass** - no regression in normal operation

## Technical Approach

### Component Overview

```
SilverSubscriber (enhanced)
├── CircuitBreaker
│   ├── State: Closed → Open → HalfOpen → Closed
│   ├── Failure threshold (default: 5 consecutive)
│   └── Health check interval (default: 30s when open)
├── WatermarkPersistence
│   ├── File-based (already exists, enhance)
│   └── Persist every N records or T seconds
├── ParquetBronzeReader (new)
│   ├── Implements BronzeReader trait
│   └── Uses existing ParquetStore
└── CatchUpCoordinator
    ├── Triggered on circuit close
    ├── Reads from watermark to now
    └── Batch writes with backpressure
```

### Configuration Schema

```yaml
silver_etl:
  # ... existing config ...

  self_healing:
    enabled: true

    circuit_breaker:
      failure_threshold: 5        # consecutive failures to open
      success_threshold: 3        # consecutive successes to close
      health_check_interval_secs: 30

    watermark:
      persist_interval_secs: 60   # save watermark periodically
      persist_batch_count: 100    # or after N successful writes
      file_path: "/var/lib/ndp/watermarks/{stream_id}.txt"

    catch_up:
      enabled: true
      max_window_secs: 7200       # max 2 hours catch-up
      batch_size: 500             # records per batch
      batch_delay_ms: 100         # backpressure between batches
```

## Dependencies

- Existing `ParquetStore` (read capability)
- Existing `SilverSubscriber` infrastructure
- Existing `BronzeReader` trait definition
- Existing watermark file load/save methods

## Risks

| Risk | Mitigation |
|------|------------|
| Parquet files purged before catch-up | Configure Bronze retention > max catch-up window |
| Catch-up overwhelms recovering DB | Batch with backpressure, configurable delays |
| Watermark file corruption | Atomic write with temp file + rename |
| Race condition during recovery | Single-threaded catch-up, pause live processing |

## Estimation

- **Size**: Small-Medium (isolated to SilverSubscriber, uses existing infrastructure)
- **Components**: 4 new structs, 1 trait implementation, config extension
- **Testing**: Unit tests for circuit breaker state machine, integration test for recovery flow
