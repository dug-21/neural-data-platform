# Changelog

All notable changes to the Neural Data Platform will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.0] - 2026-02-15

First end-to-end intelligence cycle: similarity search, breach predictions, and outcome tracking as a running daemon on the Raspberry Pi. Builds on the fe-003 library foundation (traits, MetricEmbedder, PostgresStorage, pgvector DDL) by wiring real implementations to those traits. After a 168-hour warmup, the system generates hourly predictions: "Given conditions like now, what happened next in the past?"

### Added

- **HnswEngine** (`crates/ndp-intelligence/src/similarity/hnsw.rs`) — ruvector-core HNSW wrapper, feature-gated behind `#[cfg(feature = "ruvector")]`
- **PgVectorEngine** (`crates/ndp-intelligence/src/similarity/pgvector.rs`) — SQL K-NN search fallback using pgvector `<=>` cosine distance operator
- **DualSimilarityEngine** (`crates/ndp-intelligence/src/similarity/dual.rs`) — HNSW wrapper for fast search with StorageBackend for durable pgvector writes
- **Factory function** (`create_similarity_engine`) — dispatches HNSW or pgvector based on feature flags
- **PredictionEngine** (`crates/ndp-intelligence/src/predictions/mod.rs`) — K-NN neighbor outcome lookup with confidence scoring (confidence = k_supporting / k_total)
- **OutcomeTracker** (`crates/ndp-intelligence/src/predictions/outcome.rs`) — evaluates pending predictions against Gold aligned view actual values
- **IntelligenceService** (`crates/ndp-intelligence/src/service.rs`) — full observe-embed-store-search-predict-evaluate orchestration cycle
- **PG NOTIFY listener** (`crates/ndp-intelligence/src/notify.rs`) — optional optimization wake with exponential backoff reconnection (1s-60s)
- **App modes** — daemon (timer + NOTIFY + graceful shutdown), one-shot (single cycle, exit 0), backfill (historical embed-only, batch 100), status
- **AppConfig** (`apps/ndp-intelligence-app/src/config.rs`) — domain config loaded from etcd via config-client; runtime config from environment variables
- **Intelligence config block** in `config/domains/indoor-air-quality/domain.json` — embedding fields, search K, prediction horizons
- **Docker container** (`docker/intelligence/Dockerfile`) — multi-stage build (rust:1.82 builder, debian:bookworm-slim runtime), 256MB memory limit
- **docker-compose service** (`ndp-intelligence`) — intelligence profile, depends on timescaledb + etcd
- **deploy.sh integration** — `intelligence-start`, `intelligence-stop`, `intelligence-logs`, `intelligence-status` commands; declarative `apply` support for `ndp-intelligence` container target
- **72 new tests** — 61 in ndp-intelligence (11 integration `#[ignore]`), 11 in ndp-intelligence-app

### Changed

- **`crates/ndp-intelligence/Cargo.toml`** — added deadpool-postgres dependency for connection pooling
- **`crates/ndp-intelligence/src/error.rs`** — added `Database` and `Shutdown` error variants
- **`apps/ndp-intelligence-app/Cargo.toml`** — added config-client, deadpool-postgres, tokio-postgres, chrono, serde, serde_json dependencies
- **`docs/procedures/DEPLOYMENT-DECLARATIVES.md`** — added `ndp-intelligence` to valid container targets

### Technical Notes

- Timer (20 min default) is the primary wake mechanism; PG NOTIFY is an optimization (continuous aggregates cannot have triggers)
- Backfill mode never generates predictions (embed-only)
- Observation count persisted by querying `gold.metric_embeddings` count on restart
- SimilarityEngine trait is synchronous; PgVectorEngine uses `block_on` internally
- 908 platform-core tests unchanged; 72 new intelligence tests added
- GitHub Issue: #18

## [1.1.26] - 2026-02-15

Remove vestigial periodic snapshot timer from Bronze subscriber. With the v1.1.25 WAL-only architecture, Parquet should only be written at day rollover (1/day). The old 30-minute snapshot timer was left in the select! loop, causing unnecessary 45 MiB RSS spikes, glibc arena ratcheting (~3.76 MiB/hr growth), and redundant I/O (Parquet overwritten 48x/day instead of 1x).

### Removed

- **`snapshot_interval_secs` config field** from `BronzeSubscriberConfig` — periodic Parquet snapshots are no longer needed in WAL-only design. Old configs with this field are silently ignored (serde backward compatibility)
- **Periodic snapshot timer** from `start()` select! loop — the `snapshot_timer.tick()` arm that replayed the entire WAL into memory every 30 minutes. `snapshot()` method retained (called by `day_rollover()`)

### Changed

- **`core/src/subscribers/bronze.rs`** — removed `snapshot_interval_secs` field, `default_snapshot_interval_secs()` function, snapshot timer setup and select! arm
- **`apps/air-quality-app/src/main.rs`** — removed `snapshot_interval_secs` from BronzeSubscriberConfig construction
- **`config/base/platform.yaml`** — removed `snapshot_interval_secs: 1800` line
- **Tests updated** — 3 tests reworked to test via `day_rollover()` and final-snapshot-on-shutdown instead of periodic timer. 1 backward-compatibility test added for old YAML with `snapshot_interval_secs`

### Fixed

- **RSS spikes every 30 min** — periodic snapshot replayed entire WAL into memory (45 MiB allocation), then wrote Parquet, but never truncated WAL. This allocation was pure overhead since day_rollover already handles Parquet writes
- **glibc arena ratcheting** — repeated 45 MiB allocations every 30 min caused glibc to retain arena pages, growing RSS by ~3.76 MiB/hr even after malloc_trim
- **Redundant Parquet I/O** — Parquet was overwritten 48x/day (every 30 min) instead of 1x/day (at day rollover)

### Technical Notes

- 908 platform-core lib tests passing (0 change in count -- 2 tests reworked, 1 renamed, 1 added)
- Pure removal — no new features, no new code paths
- Backward compatible — old platform.yaml and subscriber configs with `snapshot_interval_secs` are silently ignored

## [1.1.25] - 2026-02-14

BUG-004: WAL-only Bronze architecture — eliminate accumulator memory leak. Removes the in-memory Accumulator (636 lines), making WAL the single source of truth in the hot path. Parquet writes move from 48/day to 1/day via day rollover. HybridBronzeReader enables Silver catch-up from both Parquet (historical) and WAL (today). Projected RSS reduction from 467 MiB to ~75 MiB flat on Pi 5.

### Removed

- **`core/src/storage/accumulator.rs`** (636 lines) — in-memory HashMap that grew monotonically, cloned every 30-min snapshot, caused glibc arena fragmentation and RSS growth of ~7.6 MiB/hr. Root cause of BUG-005 (GH Issue #16)

### Changed

- **`core/src/subscribers/bronze.rs`** — complete rewrite of data flow:
  - `handle_point()`: WAL append only (no accumulator staging)
  - `snapshot()`: replays WAL from disk, groups by source_id with move semantics (no clone), writes Parquet per source
  - `day_rollover()`: final snapshot then WAL truncate (safety: WAL NOT truncated if snapshot fails)
  - `start()` select! loop: new day rollover timer branch alongside heartbeat and event processing
  - `duration_until_next_rollover()`: computes time to next rollover based on `day_rollover_utc_hour` config
- **`core/src/diagnostics/memory.rs`** — decoupled from Accumulator, uses `wal_file_bytes` for memory diagnostics
- **`core/src/storage/mod.rs`** — removed `pub mod accumulator` and `pub use accumulator::Accumulator`
- **`core/src/lib.rs`** — re-exported HybridBronzeReader, ParquetBronzeReader, RawStore
- **`apps/air-quality-app/src/main.rs`** — replaced `NoBronzeReader` with `HybridBronzeReader` for real Silver catch-up

### Added

- **`core/src/subscribers/bronze_reader.rs`** — three BronzeReader implementations:
  - `ParquetBronzeReader`: reads historical Parquet files for past days
  - `WalBronzeReader`: opens read-only WAL instance, replays and filters by timestamp/stream
  - `HybridBronzeReader`: combines Parquet + WAL with deduplication (WAL authoritative, keyed on source_id + timestamp)
  - 15 unit tests covering all reader types and dedup logic
- **`core/src/storage/wal.rs`** — `entry_count()` and `truncate()` methods (renamed from `commit()`)
- **10 new bronze subscriber tests** replacing 6 removed accumulator-dependent recovery tests

### Fixed

- **BUG-004** (GH Issue #16): Accumulator memory leak causing RSS growth of ~7.6 MiB/hr from glibc arena fragmentation. Eliminated by removing Accumulator entirely — WAL is now the single source of truth
- **Silver catch-up disabled**: `NoBronzeReader` was hardcoded in air-quality-app, meaning Silver could never recover from missed events. `HybridBronzeReader` now provides real catch-up from both Parquet and WAL

### Technical Notes

- 908 platform-core lib tests passing (was 904, +10 new bronze reader tests, -6 removed recovery tests)
- WAL-only hot path: zero heap allocation per event beyond the WAL append
- Parquet writes: 48/day (every 30 min) reduced to 1/day (at configurable `day_rollover_utc_hour`)
- RawDataPoint is the universal type across EventBus, WAL, and Silver — zero format conversion
- Net code change: -634 lines (530 added, 1164 removed)

## [1.1.24] - 2026-02-13

ops-004 Phase 2b: BUG-005 dual offensive/defensive memory strategy. Overnight v1.1.23 data showed glibc's dynamic mmap threshold ratcheting arena back to ~192 MiB despite MALLOC_ARENA_MAX=2. Two additional strategies:

### Added

- **`core/src/diagnostics/memory.rs`** — `MemoryWatchdog` struct: monitors RSS on every heartbeat (30s), triggers graceful process restart when `NDP_MEMORY_RESTART_THRESHOLD_MIB` exceeded (default 400 MiB, 0=disabled). Final snapshot completes before `exit(0)`, WAL guarantees zero data loss, Docker `restart: unless-stopped` brings process back in ~2-3s
- **`core/src/subscribers/bronze.rs`** — watchdog integration: startup log at INFO, threshold breach at WARN ("MEMORY WATCHDOG: RSS exceeds restart threshold"), post-snapshot exit confirmation at WARN
- **9 unit tests** (T-23 through T-31): threshold parsing, enable/disable, boundary conditions, env var reading

### Changed

- **`deploy/pi/docker-compose.yml`** — added two env vars:
  - `MALLOC_MMAP_THRESHOLD_=131072` — disables glibc dynamic mmap threshold ratchet, forces allocations >= 128 KiB through `mmap()` (cleanly returned to OS on free)
  - `NDP_MEMORY_RESTART_THRESHOLD_MIB=400` — memory watchdog threshold (set to 0 to disable)

### Technical Notes

- Dual strategy: offensive (glibc tuning slows growth) + defensive (watchdog guarantees operations)
- For high-rate ingestion domains: raise `MALLOC_MMAP_THRESHOLD_` to 1048576 (1 MiB) to avoid mmap syscall overhead
- 904 platform-core lib tests passing (was 895, +9 watchdog tests)
- All env vars tunable without code changes — no recompile needed for threshold adjustment

## [1.1.23] - 2026-02-12

ops-004 Phase 2: BUG-005 mitigation — glibc arena tuning to reduce RSS fragmentation. Overnight v1.1.22 instrumentation confirmed glibc malloc fragmentation as root cause: `fordblks` at 97 MiB (freed-but-unreturned), `arena` at 195 MiB across up to 32 independent arenas on 4-core Pi 5. Alternative allocators (jemalloc, mimalloc) crash on Pi 5 / kernel 6.14+ / cgroup v2.

### Changed

- **`deploy/pi/docker-compose.yml`** — added glibc tuning env vars to air-quality-app:
  - `MALLOC_ARENA_MAX=2` — reduce from 32 default arenas to 2, consolidating freed pages so `malloc_trim` can return them to OS
  - `MALLOC_TRIM_THRESHOLD_=65536` — lower threshold (64 KiB vs 128 KiB default) for more aggressive page return
- **`core/src/subscribers/bronze.rs`** — `malloc_trim(0)` now called every heartbeat (30s) via `do_malloc_trim()`, not just after hourly snapshot
- **`core/src/subscribers/bronze.rs`** — snapshot `malloc_trim` replaced inline `extern "C"` with shared `do_malloc_trim()` function
- **`core/src/diagnostics/memory.rs`** — new `do_malloc_trim()` function using `dlsym` pattern (consistent with `read_mallinfo2`)

### Technical Notes

- Zero new crate dependencies — `do_malloc_trim()` uses same `dlsym(RTLD_DEFAULT, ...)` pattern as existing `read_mallinfo2()`
- `#[cfg(target_os = "linux")]` guard with no-op fallback on non-Linux
- 895 platform-core lib tests passing, 1,617 total across workspace
- GitHub Issue #16 updated with overnight instrumentation findings
- 48h soak test required post-deploy to validate RSS plateau < 256 MiB

## [1.1.22] - 2026-02-11

ops-004: Memory instrumentation for BUG-005 RSS drift investigation. Adds diagnostics module with glibc mallinfo2 FFI, /proc/self/smaps parser, accumulator capacity tracking, and enhanced heartbeat/snapshot logging to attribute the ~121 MiB unaccounted RSS gap observed during 13.5h production runs on Pi 5.

### Added

- **`core/src/diagnostics/mod.rs`** — new diagnostics module root with re-exports
- **`core/src/diagnostics/memory.rs`** — central memory diagnostics:
  - `MemoryDiagnostics` struct with `collect(&Accumulator)` for single-snapshot metrics
  - `SmapsSummary` — RSS decomposition by mapping type (heap, stack, anonymous, file-backed)
  - `MallocStats` — glibc mallinfo2 allocator statistics via `dlsym` FFI (runtime graceful degradation on glibc < 2.33)
  - `MemoryTrend` — ring buffer for RSS growth rate computation (bytes/hour)
  - Pure parse functions: `parse_kb_value`, `parse_proc_status_rss_bytes`, `parse_proc_smaps_summary`
  - Read functions: `read_proc_status_rss_bytes`, `read_proc_smaps_summary`, `read_mallinfo2`
  - Helper: `format_opt_mib` for "X.Y" MiB / "N/A" formatting
- **Accumulator capacity methods**: `hash_capacity()`, `vec_capacity()`, `vec_len()` on `Accumulator` struct
- **Enhanced heartbeat**: allocator stats (`arena_mib`, `fordblks_mib`, `hblkhd_mib`), smaps decomposition (`heap_rss_mib`, `anon_rss_mib`), accumulator capacity (`accum_hash_cap`, `accum_vec_cap`, `accum_vec_len`), unaccounted gap (`unaccounted_mib`)
- **Enhanced snapshot**: allocator stats before/after writes, per-source RSS delta tracking (debug level), fordblks fragmentation delta
- **Memory trend summary**: every 10 snapshots, logs RSS growth rate (MiB/hour) from ring buffer
- **Subsystem memory attribution**: RSS delta > 1 MiB around `handle_point()` logged at debug level
- 32 new unit tests (27 diagnostics + 5 accumulator capacity)

### Changed

- `read_process_rss_mib()` moved from `bronze.rs` to `diagnostics::memory` (consolidation)
- `Accumulator::clear()` now uses `HashMap::new()` instead of `HashMap::clear()` to release bucket capacity (correct behavior for day rollover memory reclamation)
- Heartbeat log line: all existing fields preserved (backward compatible), new fields appended

### Technical Notes

- No new crate dependencies — uses raw `extern "C"` FFI via `dlsym` for mallinfo2, `std::fs` for procfs
- `#[cfg(target_os = "linux")]` guards with `None` fallback on non-Linux
- 895 platform-core lib tests passing (32 new + 863 existing)
- GitHub Issue #16 (BUG-005) tracks the investigation

## [1.1.21] - 2026-02-10

air-018: Replace Polars with arrow-rs + parquet crate in Bronze write/read path. Fixes BUG-004 OOM — Polars DataFrame create/drop leaked ~4.5 MiB per 30-min snapshot cycle on Pi 5, exhausting the 512 MiB container within 24-36 hours. Alternative allocators (jemalloc, mimalloc) crash on Pi 5 / kernel 6.14+ / cgroup v2.

### Changed

- **`core/src/storage/parquet.rs`** — all Polars API replaced with `arrow` v57 + `parquet` v57 crate direct usage
  - Write path: `Series`/`DataFrame`/`ParquetWriter` replaced with `Int64Array`/`StringArray`/`Float64Array`/`RecordBatch`/`ArrowWriter`
  - Read path: `ParquetReader` replaced with `ParquetRecordBatchReaderBuilder` + column downcast pattern
  - Query filter: Polars lazy `.filter(col("timestamp").gt_eq(...))` replaced with manual microsecond comparison
- **`core/src/error.rs`** — `CoreError::Polars` renamed to `CoreError::Arrow`; `From<ArrowError>` and `From<ParquetError>` impls replace `From<PolarsError>`
- **`core/Cargo.toml`** — `polars` removed from `[dependencies]`, `arrow` + `parquet` added
- **`Cargo.toml`** (workspace) — `arrow` v57 + `parquet` v57 added to `[workspace.dependencies]`

### Added

- **`test_timeseries_parquet_schema_metadata`** — AC-02 verification: 6-column schema names, types, nullable flags, Snappy compression
- **`test_raw_parquet_schema_metadata`** — AC-02 verification: 5-column schema names, types, nullable flags, Snappy compression
- Schema helper functions: `timeseries_schema()`, `raw_data_schema()`, `read_nullable_string()`, `read_nullable_json()`

### Fixed

- **BUG-004**: Bronze Parquet write no longer allocates/drops Polars DataFrames, eliminating the 4.5 MiB/cycle chunk leak that caused container OOM within 24-36 hours on Pi 5

### Technical Notes

- Parquet file schema identical — zero downstream impact to Silver ETL, MCP server, Grafana
- 863 platform-core tests passing (+ 2 new schema metadata tests)
- `polars` retained in workspace deps for `silver-etl` and `air-quality-app` (out of scope)
- BUG-004 diagnostic logging in `bronze.rs` preserved for post-deploy verification
- `malloc_trim(0)` call remains (harmless, confirms fix via near-zero `polars_delta`)

## [1.1.20] - 2026-02-09

ops-003 Phase 3: Shared constants, cross-cutting validation, ConfigValidator unification, NoOpDbClient dedup, YAML retirement. Internal consolidation — zero deploy.sh changes, zero new CLI flags.

### Added

- **`ndp_lib::constants` module** — single source of truth for `VALID_METRICS`, `VALID_ROLLING_STATS`, `GOLD_SCHEMA`, `SILVER_SCHEMA`, `NDP_ENTITY_COLUMN`
- **Cross-cutting validation** — `gold::sync_stream()` now validates config before DDL generation by default (`SyncOptions.validate = true`)
- **`--no-validate` wiring** — existing CLI flag now connected to `SyncOptions.validate` for opt-out
- **4 gap checks ported to `validate_gold_etl()`** — lag lags_hours non-empty, lag hours >= 1, rolling windows non-empty, trend window non-empty
- **4 NoOpDbClient tests** in ndp-lib verifying contract
- **7 constants tests** verifying exact values and counts

### Changed

- **Constants deduplicated** — `gold::config::types` and `validate::semantic::gold` now import from `ndp_lib::constants` instead of defining locally
- **`gold::generators::constants`** re-exports from `ndp_lib::constants`
- **ConfigValidator removed** — all validation now through `validate::semantic::gold::validate_gold_etl()` (4 gap checks ported first)
- **NoOpDbClient deduplicated** — 3 CLI copies (unreachable!()) replaced with import from `ndp_lib::db::NoOpDbClient` (Ok(()))
- **ndp-cli dependencies trimmed** — removed `async-trait` and `tokio-postgres` (no longer needed after NoOpDbClient dedup)

### Removed

- **7 `config.yaml` files** renamed to `.yaml.bak` under `config/base/streams/` (all configs JSON since v1.1.8)
- **`core/tests/nws_config_compatibility_test.rs`** — obsolete YAML deserialization tests
- **`gold::validation::ConfigValidator` struct** — replaced by unified `validate_gold_etl()`

### Technical Notes

- 874 tests passing across ndp-lib (554), ndp-validate (65), ndp-gold-ddl (15), ndp-cli (4+), ndp-types (88+), platform-core (861)
- Clippy clean with `-D warnings` on all modified packages
- Re-serialization approach (typed struct -> JSON -> validate) avoids dual validation paths
- Utility functions `parse_granularity`, `parse_window`, `granularity_to_suffix` kept in `gold::validation` (used by generators)

## [1.1.19] - 2026-02-09

BUG-004: Diagnostic logging for Bronze memory leak investigation. Process RSS, accumulator size, and WAL file size are now logged on every heartbeat (5s) and snapshot cycle (30min). `malloc_trim(0)` called after Polars/Arrow Parquet writes to test whether glibc page retention is the cause.

### Added

- **Memory diagnostics on snapshot** — logs `rss_before_mib`, `rss_after_writes_mib`, `rss_after_trim_mib`, `polars_delta_mib`, `trim_reclaimed_mib`, `net_delta_mib`
- **Memory diagnostics on heartbeat** — logs `accumulator_mib`, `wal_mib`, `rss_mib` every 5 seconds at `info` level
- **`malloc_trim(0)` after Parquet writes** — forces glibc to return freed pages to OS (Linux only)
- **`WriteAheadLog::file_size_bytes()`** — helper for WAL size logging
- **BUG-004 planning artifacts** — SPARC spec and test plan under `product/features/air-017/bugs/`

### Fixed

- Updated air-017 STATUS.md to reflect Phase 1 production deployment status

## [1.1.18] - 2026-02-08

AIR-017 Phase 1: Bronze write-ahead architecture. Separates durability (WAL) from archival (Parquet) in the Bronze layer. WAL provides immediate durability on event receipt; Parquet is a periodic snapshot from an in-memory accumulator. Read-modify-write is eliminated entirely.

### Added

- **WAL v2** (`core/src/storage/wal.rs`) — sequence-numbered entries, watermark-based truncation via `commit_to()`, replaces delete-all `commit()`
- **InMemoryAccumulator** (`core/src/storage/accumulator.rs`) — HashMap-based accumulator with dedup by `(source_id, timestamp, value_hash)`, memory estimation, per-stream drain
- **WAL on event receipt** — BronzeSubscriber writes WAL immediately on event arrival (millisecond durability, was up to 30s)
- **Periodic Parquet snapshots** — configurable `snapshot_interval_secs` (default 1800s), full overwrite from accumulator (no read-modify-write)
- **Startup crash recovery** — seed accumulator from today's Parquet, replay WAL entries after last snapshot watermark
- **`write_raw_snapshot()`** on RawStore trait — bulk write from Vec without WAL (used by snapshot path)
- **`snapshot_interval_secs`** and **`day_rollover_utc_hour`** config fields in `platform.yaml`
- **4 integration tests** — full ingest-snapshot cycle, crash recovery, snapshot overwrite, multi-stream isolation
- **CancellationToken fix** for coordinator tests — 3 previously-hanging tests now complete in <1s

### Changed

- **BronzeSubscriber** — rewritten with WAL-on-receipt, accumulator, snapshot timer in `select!` loop
- **ParquetStore** — WAL removed from `write_raw_batch()`, `append_to_raw_parquet()` deprecated
- **BronzeSubscriber constructor** — now takes `snapshot_interval_secs` and `day_rollover_utc_hour` parameters

### Technical Notes

- 861 platform-core lib tests passing in 1.66s (was hanging indefinitely due to coordinator test bug)
- 1,690 total tests across platform-core (861), ndp-lib (675+), ndp-validate (65+)
- Parquet write frequency: ~2,880/day → ~24-48/day
- Parquet write cost: O(file_size) read+write → O(day's data) write only
- Durability latency: up to 30s → milliseconds
- Memory: stable O(day's data) in accumulator (~22 MiB for current volumes)

## [1.1.17] - 2026-02-08

Validate migration to ndp-lib (ops-003 phase 2). All validation logic now lives in `crates/ndp-lib/src/validate/`, the `ndp validate` CLI command replaces the standalone `ndp-validate` binary, and deploy.sh uses no-fallback dispatch.

### Added

- **`ndp validate` CLI command** with flat flags for all validation operations
  - `--stream <path>`, `--all`, `--domain <path>`, `--domain-all` for config validation
  - `--schema --generate`, `--schema --verify <path>` for JSON Schema operations
  - `--schema-only`, `--check-tables`, `--format json|human`, `--strict`
  - Exit codes: 0 (pass), 1 (validation error), 2 (system error) per dp-019
- **`crates/ndp-lib/src/validate/` module** — 12 files migrated from ndp-validate
  - `error.rs`, `result.rs`, `schema.rs`, `schema_gen.rs`, `semantic/{mod,sources,source_path,dq_rules,gold,domain,table_exists}.rs`
  - Public API: `validate_stream()`, `validate_all_streams()`, `validate_domain_config()`, `validate_all_domains()`, `generate_schema()`, `verify_schema()`
  - `ValidateOptions` struct for all validation configuration
- **`is_valid_granularity()` deduplication** — shared implementation in `semantic/mod.rs`, called by both `gold.rs` and `domain.rs`

### Changed

- **deploy.sh** — 2 validate dispatch sites now use `ndp validate` with no-fallback policy (error + return 1)
  - `validate_domain_configs()`: `ndp validate --domain <path> --config-dir $CONFIG_STREAMS_DIR --format human`
  - `handle_domain()`: same pattern, consolidated with Gold dispatch
- **ndp-validate** — now a thin wrapper re-exporting from `ndp_lib::validate`
- **ndp-validate Cargo.toml** — `serde_yaml` dependency removed (all configs are JSON since v1.1.8)
- **ndp-validate main.rs** — YAML format auto-detection code stripped

### Technical Notes

- 740 tests passing (675 ndp-lib + 65 ndp-validate), 0 failures
- Output parity verified: `ndp validate` matches `ndp-validate` for stream, domain, schema operations
- Integration-tested: 4 streams + 1 domain validated in integration environment
- deploy.sh: `bash -n` syntax check clean, zero `ndp-validate` dispatch references remaining

## [1.1.16] - 2026-02-08

Fix aligned view `ndp_id` fan-out that caused `detect_events` to fail with "more than one row returned by a subquery used as an expression".

### Fixed

- **Aligned view produced multiple rows per bucket** — underlying CAs group by `(bucket, ndp_id)` but the aligned view joined on `bucket` only. With multiple Home Assistant entities per bucket, the FULL OUTER JOIN created a cartesian product. The `detect_events` context subquery (scalar) then failed on multi-row results.
- **Permanent fix in `join_builder.rs`** — each CA source is now wrapped in a `(SELECT bucket, AGG(col) ... GROUP BY bucket)` subquery that collapses `ndp_id` before joining. Aggregate functions derived from column naming convention: `_mean`→AVG, `_min`→MIN, `_max`→MAX, `_count`→SUM, `_p95`→MAX, `sample_count`→SUM.

- **deploy.sh ignored manifest `action` for domains** — `handle_domain()` parsed `$action` from the manifest but never passed `--action "$action"` to `ndp gold generate`. All domain DDL defaulted to `sync` mode regardless of manifest. Fixed both aligned view and events DDL generation calls.

### Technical Notes

- 729 tests passing (361 ndp-lib + 31 aligned-view + 15 golden-master + 15 ndp-gold-ddl + 217 ndp-validate + other), 0 failures
- Integration-tested: aligned view produces exactly 0 duplicate buckets, `detect_events` executes cleanly
- Deployment uses `recreate` action — aligned view must be dropped/recreated to pick up subquery structure
- 5 new unit tests for bucket subquery mechanics and aggregate function mapping

## [1.1.15] - 2026-02-07

Fix Gold events detection and aligned view refresh — three bugs that caused Grafana dashboards to show no data for events and aligned views.

### Fixed

- **`detect_events` referenced nonexistent table** — `derive_gold_ca_table()` derived Gold CA name from Silver table (`gold.air_quality_observations_hourly`) instead of stream_id (`gold.air_quality_hourly`). Introduced in v1.1.11 (ops-002 config-driven rewrite). Threshold crossings were silently failing.
- **Aligned view had no auto-refresh** — `gold.indoor_air_quality_aligned` is a regular materialized view with no scheduled refresh. Added TimescaleDB job (`refresh_indoor_air_quality_aligned`, every 15 min) generated as part of domain DDL.
- **`detect_events` PL/pgSQL `job_id` ambiguity** — parameter name `job_id` conflicted with `timescaledb_information.job_stats.job_id` column. Fully qualified the column reference.

### Technical Notes

- 492 ndp-lib tests, 15 ndp-gold-ddl tests, 0 failures
- Integration-tested against live TimescaleDB: detect_events executes cleanly, refresh job schedules correctly
- 68 existing events on Pi preserved (no schema changes to gold.events)

## [1.1.14] - 2026-02-07

Gold module migration to ndp-lib and `ndp gold` CLI commands (ops-003 phase 1). Gold DDL generation now lives in the shared library. deploy.sh dispatches through `ndp gold` instead of `ndp-gold-ddl`.

### Added

- **`ndp gold generate|sync|recreate` CLI commands** — full parity with ndp-gold-ddl binary
  - `--stream`, `--domain`, `--transitions`, `--events`, `--dry-run`, `--validate-only`, `--no-validate` flags
  - `--events requires --domain` guard matches old binary behavior
  - `--validate-only` validates config without generating DDL
- **`crates/ndp-lib/src/gold/` module** — 29 files migrated from ndp-gold-ddl with shared imports
  - Config (loader, domain, types), generators (8), planner, registry (4), validation, error, db
  - Convenience API: `generate_stream()`, `generate_domain()`, `sync_stream()`, `sync_domain()`, `recreate_stream()`
  - `GenerateOptions` struct for transitions/events/verbose flags
- **Golden master tests** — 15 tests (14 DDL comparisons + 1 checksum verification) in ndp-lib
- **DbClient trait unification** — `PostgresCaChecker` uses `ndp_lib::db::DbClient`

### Changed

- **deploy.sh** — 3 gold dispatch sites now use `ndp gold` commands with no fallback (error + return 1)
  - `handle_gold_table`: `ndp gold sync --stream`
  - `handle_domain_declaration`: `ndp gold sync --domain` and `ndp gold generate --domain --events`

### Fixed

- **deploy.sh `--config-dir` path** (BUG-004) — dispatch sites passed `$REPO_ROOT/config` but ndp CLI expects `config/base` (uses `.parent()` to reach config root). Fixed 3 sites.

### Technical Notes

- 491 ndp-lib tests (355 unit + 121 integration + 15 golden master), 0 failures, 1 ignored
- 13 live integration tests against TimescaleDB (12 pass, 1 pre-existing config issue)
- ndp-gold-ddl retained as thin wrapper (re-exports ndp-lib)

## [1.1.13] - 2026-02-06

Fix CA refresh policy: events_hourly showed no data because start_offset was hardcoded to 3 hours and if_not_exists prevented updates on redeploy (BUG-003, ops-002).

### Fixed

- **events_hourly empty after deploy** (BUG-003) — CA refresh policy used `start_offset => INTERVAL '3 hours'` with `if_not_exists => TRUE`, causing historical events to never materialize and preventing offset changes on redeploy
- Replaced `if_not_exists` with remove+add pattern for idempotent redeployment
- Added `CALL refresh_continuous_aggregate(..., NULL, NULL)` to backfill data outside the rolling window

### Changed

- **`refresh_start_offset_days`** — new optional config field in `EventsConfig` (default 365). Controls how far back the CA refresh policy extends. Existing configs without the field use the default.
- **ndp-validate schema** — added `refresh_start_offset_days` (integer, minimum 1) to events schema

## [1.1.12] - 2026-02-06

Fix domain objectives sync: migrate ~200 lines of dead Bash (YAML-based, never worked with JSON configs) to Rust CLI command `ndp domain sync` (BUG-002, ops-002).

### Fixed

- **Domain objectives sync not migrated** (BUG-002) — `sync_domains_to_data_dictionary()` in deploy.sh contained ~200 lines of dead Bash code that parsed `domain.yaml` (eliminated in v1.1.8). Domains, objectives, constraints, and stream mappings were never synced to the data dictionary after the YAML→JSON migration.

### Added

- **`ndp domain sync` CLI command** — syncs domain configs from `config/domains/` to `data_dictionary` schema
  - Parameterized SQL (zero string concatenation) for domains, domain_streams, objectives, constraints
  - UPSERT for domains, DELETE+INSERT for children (idempotent)
  - Transaction-wrapped (BEGIN/COMMIT)
  - `--dry-run` support, `--domains-dir` override
  - Per-domain error collection (non-fatal, continues to next domain)
- **`crates/ndp-lib/src/domain/` module** — sync engine with 18 London TDD tests
  - `types.rs` — DomainSyncEntry, StreamMappingEntry, ObjectiveSyncEntry, ConstraintSyncEntry
  - `sql.rs` — 7 parameterized SQL constants with float8→numeric casts
  - `mod.rs` — `sync_domains()` function with MockDbClient tests
- **ConfigLoader extension** — `load_domain_configs()` with `FileSystemConfigLoader.with_domains_dir()` builder
- **Convert function** — `domain_config_to_sync_entry()` flattens nested ObjectiveTargetConfig

### Changed

- **deploy.sh** — replaced `sync_domains_to_data_dictionary()` body with `command -v ndp` fallback pattern (~200 lines → ~47 lines)

### Technical Notes

- 710+ tests passing (94 ndp-lib, 399 ndp-gold-ddl, 217 ndp-validate)
- 23 new tests in ndp-lib (18 domain sync + 4 config + 3 convert - offset by test module reorganization)
- E2E verified against integration TimescaleDB: 1 domain, 4 streams, 6 objectives, 0 constraints
- Idempotent: second sync produces identical results

## [1.1.11] - 2026-02-06

Fix duplicate CTE names in events detection procedure. Config-driven event generators (ops-002).

### Fixed

- **Duplicate CTE names in detection procedure** — objectives sharing the same metric (e.g., `comfortable_humidity_min` and `comfortable_humidity_max` both targeting `humidity_pct`) produced duplicate `{metric}_crossings` CTEs, causing PostgreSQL to reject the procedure. CTE names now use `{objective_id}_crossings` for uniqueness.
- **Detection procedure union** — all 6 objectives now referenced in `all_crossings` UNION (previously 2 were silently dropped by deduplication)

### Changed

- **Config-driven event generators (ops-002)** — eliminated 50+ hardcoded domain-specific references from events, state_transitions, and aligned_view generators
  - `EventsGenerator` reads objectives, streams, transitions from `DomainConfig` and `StreamConfig` via `ConfigLoader` trait
  - `StateTransitionsGenerator` reads device type mapping and states from `TransitionConfig`
  - `AlignedViewGenerator` reads `stream_type` from `StreamConfig` with heuristic fallback
  - New `constants.rs` module for `NDP_ENTITY_COLUMN`, `GOLD_SCHEMA`, `SILVER_SCHEMA`
- **153 new tests** — London TDD with `MockConfigLoader`, fictional "energy-monitoring" domain proves generators are truly generic

## [1.1.10] - 2026-02-06

Wire events generator into ndp-gold-ddl CLI and deployment pipeline.

### Added

- **`--events` CLI flag** for `ndp-gold-ddl generate --domain <id> --events`
  - Generates events hypertable, unified view, hourly CAs, detection procedure, and scheduled job
  - Config-driven via `events` section in `domain.json`
- **`events_hourly_by_entity` continuous aggregate** — per-entity/stream hourly event rollup
  - Groups by `entity_id`, `stream_id` with same 15-minute refresh schedule
  - Compound index on `(entity_id, bucket DESC)`
- **Events section in `domain.json`** — `enabled`, `chunk_interval`, `retention`, `detection_schedule`
- **deploy.sh events DDL generation** — `handle_domain()` now generates and applies events DDL
  - Uses `ON_ERROR_STOP=1` for reliable error detection (fixes v1.1.6 silent failure root cause)

### Changed

- **Manifest format** — replaced migration-based deployment with domain declaration + tool build
- **`EventsGenerator::from_domain_config()`** — now reads `events` config from domain when present

### Removed

- **008_events_hourly_refresh_policies.sql** — generator handles refresh policies natively

## [1.1.9] - 2026-02-06

Deployment Tooling Foundation (ops-001). Extracts dictionary and dimension sync from Bash into Rust, with London TDD.

### Added

- **ndp-lib crate** (`crates/ndp-lib/`)
  - `sync_dictionary()` - Replaces ~460 lines of Bash SQL generation with parameterized queries
  - `sync_dimension()` - CSV-to-Silver dimension import with batch INSERT
  - `DbClient` trait (query/execute/batch_execute) for mockable database access
  - `ConfigLoader` trait for source-agnostic config loading (files today, etcd later)
  - `SyncReport` structured output from all sync operations
  - 57 London TDD tests with MockDbClient

- **ndp-cli tool** (`tools/ndp-cli/`)
  - `ndp dictionary sync` - Sync data dictionary from stream configs
  - `ndp dimension sync <id>` - Sync dimension table from CSV
  - Entity/verb CLI structure designed for V1.1-V2.0 journey
  - `--dry-run` support on all commands
  - Environment-aware config resolution (DEPLOY_ENV=integration|pi)

- **Dimension config JSON migration** (ops-001-12)
  - `config/base/dimensions/entity_context.json` (YAML preserved as fallback)

- **deploy.sh integration** (ops-001-09)
  - `command -v ndp` fallback wrapper around `sync_to_data_dictionary()`
  - `command -v ndp` fallback wrapper around `sync_dimension()`
  - `ndp-cli` added to supported tool builds
  - JSON-priority glob for dimension configs (`*.json` before `*.yaml`)

### Fixed

- **Stale data dictionary bug** (ops-001-13) - Bash `sync_to_data_dictionary()` read legacy `.yaml` files instead of authoritative `.json` configs; Rust implementation reads `.json` only

### Technical Notes

- Functions take parsed structs, not file paths (source-agnostic for files/etcd)
- Parameterized SQL ($1, $2) replaces Bash string concatenation
- Bronze layer: DELETE + INSERT; Silver layer: UPSERT
- 600+ tests passing across all crates (ndp-lib: 57, ndp-types: 88, ndp-gold-ddl: 338, ndp-validate: 217)
- No changes to existing crate code - purely additive

## [1.1.8] - 2026-02-05

Domain Configuration Standardization (FE-002). Migrates domain configs from YAML to JSON, fixes schema format, and adds CLI validation.

### Changed

- **Domain Config Format** (GAP-001)
  - Migrated `domain.yaml` → `domain.json` with FLAT format
  - Updated `ndp-gold-ddl` loader to use `serde_json`
  - Removed `serde_yaml` dependency from ndp-gold-ddl

- **Schema Format Standardization** (GAP-004)
  - Fixed `domain.schema.json` to use FLAT format (no wrapper object)
  - Updated semantic validator to expect FLAT format
  - All NDP configs now consistently use flat format

### Added

- **Domain Validation CLI** (GAP-003)
  - `ndp-validate --domain <path>` - Validate single domain config
  - `ndp-validate --domain-all` - Validate all domain configs
  - Layer 1 (JSON Schema) + Layer 2 (Semantic) validation
  - 38+ new validation tests (217 total in ndp-validate)

- **Golden Master Tests**
  - 13 golden master tests for DDL output verification
  - 12 SQL baseline fixtures with SHA256 checksums
  - Normalized SQL comparison for non-deterministic ordering

- **Deploy.sh Integration**
  - `validate_domain_configs()` function in Phase 1
  - Domain config validation before deployment

### Fixed

- Non-deterministic column ordering in aligned view generator
  - Fields now sorted alphabetically before DDL generation

### Removed

- `config/domains/indoor-air-quality/domain.yaml` (replaced by domain.json)

### Technical Notes

- Resolves GitHub issues #11 (GAP-001) and #13 (GAP-003)
- 556 total tests passing (339 ndp-gold-ddl + 217 ndp-validate)
- Implements FE-002 SPARC specification

## [1.1.7] - 2026-02-05

Patch release fixing event detection procedure column mismatch.

### Fixed

- **Event Detection Procedure** - Column name mismatch in `gold.get_event_context()`
  - Fixed `state_last_state` → `state_state_last` to match aligned view schema
  - Events now correctly capture window state context

- **Dashboard Column Names** - Fixed queries using wrong column names
  - Aligned view: `outdoor_aqi_pm25_mean`, `outdoor_temperature_c_mean`
  - Weather context: `temperature_c_mean`, `humidity_pct_mean`, `pressure_pa_mean`
  - State timeline: `ndp_id`, `state_last`

- **ndp-gold-ddl Events Generator** - Fixed column references in generated SQL
  - `outdoor_temperature_mean` → `outdoor_temperature_c_mean`
  - `outdoor_aqi_mean` → `outdoor_aqi_pm25_mean`
  - Added `window_state` with correct `state_state_last` column

### Migration Notes

- Re-applies `004_detect_events_procedure.sql` (idempotent via CREATE OR REPLACE)
- Run `CALL gold.detect_events();` after deployment to detect historical events

## [1.1.6] - 2026-02-05

Gold Layer Foundation - Phase E: Unified Event Abstraction (FE-001). Events hypertable, threshold crossings, and Gold Layer Dashboard.

### Added

- **Events Hypertable** (v11-013)
  - `gold.events` TimescaleDB hypertable for event storage
  - 7-day chunk interval, 1-year retention, 30-day compression
  - All event types: state_transition, threshold_crossing
  - Context snapshot at event time for V1.2 correlation
  - V1.2 query pattern indexes (time, type+time, entity, objective, GIN on context/details)

- **Unified Events View** (v11-013)
  - `gold.events_unified` view for V1.2 API compatibility
  - JSONB details with backward-compatible schema
  - `gold.indoor_air_quality_events` domain-scoped view

- **Events Continuous Aggregates** (v11-013)
  - `gold.events_hourly` - hourly event counts by type
  - `gold.events_hourly_by_entity` - per-entity breakdown
  - 15-minute refresh policy

- **Event Detection Procedure** (v11-013)
  - `gold.detect_events()` procedure with idempotent detection
  - `gold.get_event_context()` helper for context capture
  - `gold.detect_events_for_range()` for manual backfill
  - TimescaleDB scheduled job (15-minute interval)
  - Management functions: pause, resume, set_interval

- **Threshold Crossing Generator** (v11-012)
  - `gold.detect_threshold_crossings()` function
  - All condition types: <, <=, >, >=, between
  - Crossing directions: rising, falling, entering_range, exiting_range_low, exiting_range_high
  - Dynamic objective lookup from `data_dictionary.objectives`
  - Monitoring views for oscillation analysis

- **Gold Layer Dashboard** (v11-014)
  - `config/grafana/dashboards/gold-layer-overview.json`
  - 20 panels across 4 rows
  - Row 1: Air Quality Metrics (PM2.5, CO2, Temp/Humidity)
  - Row 2: Cross-Stream Alignment (multi-stream overlay)
  - Row 3: Events & Objectives (timeline, gauges, tables)
  - Row 4: Data Quality & Volume
  - Threshold lines at 12 µg/m³ (PM2.5) and 800 ppm (CO2)
  - Dashboard variables for filtering

- **ndp-gold-ddl EventsGenerator**
  - `generators/events.rs` with 5 generation methods
  - `EventsConfig` struct for configuration
  - 34 new unit tests (311 total)

- **Acceptance Tests**
  - `acceptance_events_hypertable.sql` - hypertable schema tests
  - `acceptance_threshold_crossings.sql` - all condition type tests
  - `acceptance_unified_events.sql` - V1.2 query pattern tests
  - `acceptance_detection_job.sql` - job scheduling tests
  - `run_acceptance_tests.sh` - test runner script

### Changed

- STATUS.md: Phase E complete (95% overall progress)
- ndp-gold-ddl: Added events module to generators

### Technical Notes

- Events are stored in hypertable (not view) per Decision 12 in DECISIONS.md
- Enables continuous aggregates on events (impossible with UNION ALL view)
- Context captured from `gold.indoor_air_quality_aligned` at event time
- Detection job uses `last_successful_finish` for idempotency
- V1.2 Pattern Detection Engine can consume events via `gold.events_unified`

### Migration Notes

- No breaking changes from v1.1.x
- New tables/views are additive
- Detection job requires Silver layer tables to generate events
- Dashboard requires Grafana datasource configuration

## [1.1.2] - 2026-02-05

Gold Layer Foundation - Phase C (FE-001). Cross-stream alignment with 3 streams and objectives storage.

### Added

- **Cross-Stream Aligned View** (v11-005)
  - `gold.indoor_air_quality_aligned` materialized view
  - FULL OUTER JOIN across air-quality, outdoor-weather, home-assistant-state
  - NULL handling by stream type (ADR-FE001-004)
    - observation streams: preserve NULL
    - state_event streams: LOCF (Last Observation Carried Forward)
  - Column aliasing by stream alias (indoor_, outdoor_, state_)
  - COALESCE bucket across all streams

- **State Transition Materializer** (v11-006)
  - `gold.state_events_transitions` view
  - LAG window function for state change detection
  - `is_actual_transition` boolean for noise filtering
  - Duration in previous state calculation
  - Partitioned by entity (ndp_id)
  - Transition direction mapping (opening/closing)
  - 22 unit tests

- **Objectives Storage** (v11-007)
  - `data_dictionary.domains` table
  - `data_dictionary.domain_streams` mapping table
  - `data_dictionary.objectives` table with all condition types (<, >, <=, >=, ==, !=, between)
  - `data_dictionary.constraints` table for V1.3+ action framework
  - MCP queryable views: v_domain_overview, v_high_priority_objectives
  - `check_objective_violation()` function
  - deploy.sh commands: `sync-domains`, `list-domains`

- **Additional Stream Aggregates** (v11-003 extended)
  - `gold.outdoor_weather_hourly` - temperature, humidity, wind, pressure
  - `gold.outdoor_weather_daily` - daily aggregates
  - `gold.state_events_hourly` - state counts and changes
  - gold_etl configurations for outdoor-weather and home-assistant-state

- **Domain Configuration**
  - `config/domains/indoor-air-quality/domain.yaml`
  - 3 streams: air-quality (primary), outdoor-weather (context), home-assistant-state (actuator)
  - 4 objectives: healthy_co2, healthy_pm25, comfortable_humidity, comfortable_temperature

- **Unit Tests**
  - 87 new Phase C tests (279 total in ndp-gold-ddl)
  - aligned_view_tests.rs (31 tests)
  - state_transitions_tests.rs (27 tests)
  - objectives_tests.rs (29 tests)
  - Test fixtures with MockConfigLoader

### Changed

- outdoor-weather stream config: Added `stream_type: "observation"`, `gold_etl` section
- home-assistant-state stream config: Added `stream_type: "state_event"`, `gold_etl` section with transitions
- ndp-gold-ddl: Added `--transitions` CLI flag
- ndp-gold-ddl: State transitions generator module
- deploy.sh: Added sync-domains and list-domains commands

### Notes

- outdoor-air-quality stream deliberately **excluded** (reserved for Phase D fast-follower test)
- 3 streams in Gold layer aligned view
- Reference implementation for domain-centric configuration pattern
- V1.1.2 pattern detection ready with queryable aligned view

## [1.1.1] - 2026-02-04

Gold Layer Foundation - Phase B (FE-001). First Gold stream deployment with air-quality.

### Added

- **Stream Type Classification** (v11-001)
  - StreamType enum: observation, state_event, forecast, dimension
  - Correlation role derivation (effect, cause, context, metadata)
  - NULL handling strategy (preserve, carry_forward)
  - 14 unit tests

- **Classification Propagation** (v11-002)
  - SQL migration `004_stream_classification.sql`
  - `data_dictionary.stream_classification` table
  - `data_dictionary.gold_tables` catalog
  - Helper functions: derive_correlation_role(), derive_null_handling()
  - 17 unit tests

- **Per-Stream Continuous Aggregates** (v11-003)
  - `gold.air_quality_hourly` - Hourly aggregates
  - `gold.air_quality_daily` - Daily aggregates
  - 22 metrics across 7 fields (pm25, pm10, co2, temperature_c, humidity_pct, tvoc_index, nox_index)
  - Idempotent DDL generation with IF NOT EXISTS pattern

- **Aggregate Refresh Policy** (v11-004)
  - Granularity-aware defaults (hourly: 15min, daily: 1hr)
  - Idempotent policy creation with if_not_exists => TRUE
  - 38 unit tests

### Changed

- Air quality stream config: `gold_etl.enabled: true`
- Air quality stream config: Added `stream_type: "observation"`
- ndp-gold-ddl: 152 total tests (up from 113)

### Notes

- First **active** Gold layer deployment
- Reference implementation for future Gold stream rollouts
- Continuous aggregates refresh automatically on configured schedule

## [1.1.0] - 2026-02-04

Gold Layer Foundation - Phase A (FE-001). Tooling and schemas for Gold layer, no active deployment.

### Added

- **Gold DDL Tool** (`tools/ndp-gold-ddl/`)
  - Rust CLI for generating TimescaleDB continuous aggregate DDL
  - Aligned view generator with FULL OUTER JOIN strategy
  - Feature registry with extensible trait system (lag, rolling, trend)
  - 113 unit tests

- **Gold Layer JSON Schemas** (`config/schemas/`)
  - `gold-etl.schema.json` - Gold ETL configuration validation
  - `alignment.schema.json` - Cross-stream alignment rules
  - `objectives.schema.json` - Domain objectives and constraints
  - `domain.schema.json` - Domain configuration with embedded objectives

- **Gold Validation Extensions** (`tools/ndp-validate/`)
  - 9 Gold-specific error codes (400-408)
  - Semantic validation for `gold_etl` sections
  - Domain configuration validation
  - 179 total tests (up from 134)

- **Deploy.sh Gold Handlers**
  - New declaration types: `gold-tables`, `domains`
  - `handle_gold_table()` - Stream-level Gold DDL generation
  - `handle_domain()` - Domain-level aligned view DDL
  - Phases 5 (Gold Tables) and 6 (Domains) added to orchestration
  - Dry-run support for testing

- **Test Fixtures** (`tests/fixtures/`)
  - 27 test configuration files (valid + invalid)
  - Reference domain config at `config/domains/indoor-air-quality/`

### Changed

- Air quality stream config updated with `gold_etl` section (disabled until Phase B)
- Deploy.sh now has 11-phase orchestration (was 9)

### Documentation

- FE-001 Phase A SPARC artifacts (74 files)
- 5 Architecture Decision Records (ADR-FE001-001 through ADR-FE001-005)

### Notes

- Gold layer is **not active** in this release (`gold_etl.enabled: false`)
- Phase B (v1.1.1) will enable first Gold stream deployment

## [1.0.0] - 2026-02-02

First formal release establishing declarative deployment and release methodology.

### Added

- **Declarative Deployment System** (dp-020)
  - Manifest-driven deployment with `./deploy.sh apply`
  - 9-phase orchestration: validation, builds, migrations, silver tables, streams, dimensions, dictionary, restarts, device state
  - DDL generation from stream configuration (no manual SQL)
  - Device state tracking in `/var/ndp/`

- **Config Lifecycle & Release Management** (dp-021)
  - Hot-reload support for source configurations
  - Semantic versioning standard for NDP
  - Release manifest naming convention (`vX.Y.Z.manifest.json`)
  - Git tag to manifest alignment
  - Config schema migration v1.1 to v2.0

- **Configuration Validation Pipeline** (dp-019)
  - Two-layer validation (JSON Schema + Rust semantic)
  - 134 validation tests

### Changed

- Stream configs migrated to `config_version: 2` (removed deprecated `entity_schemas`)
- Air quality app rebuilt with hot-reload capability

### Documentation

- `docs/procedures/RELEASE-POLICY.md` - Versioning and release workflow
- `docs/procedures/DEPLOYMENT-DECLARATIVES.md` - Manifest format and declaration types
- `CLAUDE.md` - Added Release Methodology section for agents

---

[Unreleased]: https://github.com/dug-21/neural-data-platform/compare/v1.1.26...HEAD
[1.1.26]: https://github.com/dug-21/neural-data-platform/compare/v1.1.25...v1.1.26
[1.1.25]: https://github.com/dug-21/neural-data-platform/compare/v1.1.24...v1.1.25
[1.1.24]: https://github.com/dug-21/neural-data-platform/compare/v1.1.23...v1.1.24
[1.1.23]: https://github.com/dug-21/neural-data-platform/compare/v1.1.22...v1.1.23
[1.1.22]: https://github.com/dug-21/neural-data-platform/compare/v1.1.21...v1.1.22
[1.1.21]: https://github.com/dug-21/neural-data-platform/compare/v1.1.20...v1.1.21
[1.1.20]: https://github.com/dug-21/neural-data-platform/compare/v1.1.19...v1.1.20
[1.1.19]: https://github.com/dug-21/neural-data-platform/compare/v1.1.18...v1.1.19
[1.1.18]: https://github.com/dug-21/neural-data-platform/compare/v1.1.17...v1.1.18
[1.1.17]: https://github.com/dug-21/neural-data-platform/compare/v1.1.16...v1.1.17
[1.1.16]: https://github.com/dug-21/neural-data-platform/compare/v1.1.15...v1.1.16
[1.1.15]: https://github.com/dug-21/neural-data-platform/compare/v1.1.14...v1.1.15
[1.1.14]: https://github.com/dug-21/neural-data-platform/compare/v1.1.13...v1.1.14
[1.1.13]: https://github.com/dug-21/neural-data-platform/compare/v1.1.12...v1.1.13
[1.1.12]: https://github.com/dug-21/neural-data-platform/compare/v1.1.11...v1.1.12
[1.1.11]: https://github.com/dug-21/neural-data-platform/compare/v1.1.10...v1.1.11
[1.1.10]: https://github.com/dug-21/neural-data-platform/compare/v1.1.9...v1.1.10
[1.1.9]: https://github.com/dug-21/neural-data-platform/compare/v1.1.8...v1.1.9
[1.1.8]: https://github.com/dug-21/neural-data-platform/compare/v1.1.7...v1.1.8
[1.1.7]: https://github.com/dug-21/neural-data-platform/compare/v1.1.6...v1.1.7
[1.1.6]: https://github.com/dug-21/neural-data-platform/compare/v1.1.2...v1.1.6
[1.1.2]: https://github.com/dug-21/neural-data-platform/compare/v1.1.1...v1.1.2
[1.1.1]: https://github.com/dug-21/neural-data-platform/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/dug-21/neural-data-platform/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/dug-21/neural-data-platform/releases/tag/v1.0.0
