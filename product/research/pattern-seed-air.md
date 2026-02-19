# Pattern Seed: Air Series (air-001 through air-018)

**Generated:** 2026-02-19
**Sources:** product/features/air-001 through product/features/air-018 architecture docs, SCOPE.md files, ADRs
**Purpose:** AgentDB pattern seeding — high-value reusable knowledge for new developers, testers, and ML engineers

---

## How to Read This File

Each pattern is structured for direct import into AgentDB via `/save-pattern`. Patterns from higher-numbered features supersede older ones where noted. Deprecated entries are included specifically to prevent future agents from repeating known mistakes.

---

### Pattern: Hexagonal Architecture (Ports and Adapters)

- **taskType**: architecture:hexagonal-ports-adapters
- **approach**: The platform core (`core/`) is completely domain-agnostic — it knows only generic time-series concepts (`TimeSeriesPoint`, `Store`, `Source`, `Forecast` traits). Domain-specific logic lives in separate adapters. All dependencies point inward: adapters depend on core, never the reverse. Adding a new domain (e.g. energy monitoring) takes ~8 hours: create a `domains/{name}/` crate with `types.rs`, `parser.rs`, `adapter.rs`, `validation.rs`, and implement the `TimeSeriesPoint` trait to translate domain data to generic points. Verify isolation: `grep -r "pm25\|co2\|aqi" core/` should return nothing.
- **successRate**: 0.97
- **tags**: architecture, hexagonal, ports-adapters, domain-isolation, traits, rust
- **source**: air-001, air-002
- **status**: current

---

### Pattern: Source and Sink Trait Pattern

- **taskType**: architecture:source-sink-traits
- **approach**: Data ingestion uses the `Source` trait from `core/src/traits.rs`. Push sources (MQTT) and pull sources (HTTP polling) both implement this trait. The `RawStore` trait is used for Bronze layer writes. Key traits: `Source::fetch_raw_batch() -> CoreResult<Vec<RawDataPoint>>`, `RawStore::write_raw_batch()`. Sources are instantiated from stream config, passed to `BronzeSubscriber`. Never tie parsing logic directly to source implementations — sources return raw data, parsing happens in the ETL layer (Silver). As of air-011, parsers were decoupled from the HTTP polling source path; `HttpPollingSource` is raw-only.
- **successRate**: 0.95
- **tags**: traits, source, sink, ingestion, raw-data, bronze, rust
- **source**: air-001, air-005, air-011
- **status**: current

---

### Pattern: etcd Configuration with Watch API

- **taskType**: architecture:etcd-config-watch
- **approach**: All stream configuration is stored in etcd under `/streams/{stream_id}/config`. The `config-client` crate (260 LOC, `config-client/src/`) is a thin wrapper providing type-safe get/watch. ConfigSyncService reads YAML files from `config/base/streams/` at startup, syncs to etcd via `sync_all()`. Application code subscribes to etcd watches for hot-reload without restart. Config retrieval is <10ms p95. `ndp_id` and `silver_etl` config must be in etcd (not just YAML) to ensure consistent behavior — see air-013. Key: never access YAML files at runtime; etcd is the runtime source of truth.
- **successRate**: 0.93
- **tags**: etcd, configuration, watch, hot-reload, config-client, stream-registry
- **source**: air-003, air-004, air-013
- **status**: current

---

### Pattern: Multi-Stream Architecture with Independent Per-Stream Tables

- **taskType**: architecture:multi-stream-tables
- **approach**: Each stream gets its own typed Silver table (e.g. `silver.air_quality_readings`, `silver.state_events`). Rejected alternatives: single table with JSONB (poor query performance), separate microservices per stream (over-engineering for Pi), Redis/Kafka event bus (unnecessary infrastructure). Stream config stored in etcd under `/streams/{stream_id}/config` with schema, source type, retention, and silver_etl sections. `IngestionCoordinator` routes points by `stream_id`. Bronze uses per-stream directories: `/data/{stream_id}/YYYY-MM-DD_HH.parquet`.
- **successRate**: 0.92
- **tags**: multi-stream, schema, timescaledb, silver, bronze, routing
- **source**: air-004
- **status**: current

---

### Pattern: Stream Config YAML Format

- **taskType**: conventions:stream-config-yaml
- **approach**: Stream configs live in `config/base/streams/{stream_id}/config.yaml`. Required fields: `stream_id`, `description`, `retention_days`, `enabled`, `fields[]`, `sources[]`. Field naming uses `snake_case` (`pm25`, `co2`, `temperature`). Field types: `float`, `int`, `string`, `bool`, `json`. Units are informational metadata (not enforced): `celsius`, `ppm`, `µg/m³`, `percent`. Each source must have `ndp_id` at the top level outside `context` — see air-009 ADR-001. `silver_etl` section is optional but when present must be synced to etcd (air-013). Example file: `config/base/streams/air-quality/config.yaml`.
- **successRate**: 0.90
- **tags**: config, yaml, stream-config, conventions, naming, ndp_id
- **source**: air-004, air-009, air-013
- **status**: current

---

### Pattern: ndp_id Stable Source Identity

- **taskType**: conventions:ndp-id-source-identity
- **approach**: Every data source requires a stable `ndp_id` in its config. Format: `{device-type}-{location-hint}-{sequence}` (lowercase alphanumeric + hyphens, 3-64 chars, starts with letter). Examples: `airgradient-office-001`, `owm-weather-home`, `nws-observations-ksgj`. `ndp_id` is immutable — device replacement, firmware updates, or location changes do not change it. Both Bronze (Parquet) and Silver (TimescaleDB) store `ndp_id` as a column. This decouples identity from mutable attributes (device type, location, model) stored in `context`. Query: `WHERE ndp_id = 'airgradient-office-001'` works across device replacements.
- **successRate**: 0.93
- **tags**: ndp_id, identity, conventions, source-config, data-lineage
- **source**: air-009
- **status**: current

---

### Pattern: Generic HTTP Polling with Config-Driven Parsers

- **taskType**: architecture:http-polling-config-driven
- **approach**: `GenericHttpPollingSource` (core/src/sources/) is driven entirely by stream config — no code changes needed for new HTTP APIs. Parser selection is also config-driven. Available parsers in `core/src/parsers/`: `FlatJsonParser` (flat key-value JSON), `JsonPathParser` (JSONPath extraction), `ArrayIteratorParser` (unwraps array responses like OWM `list[0]`), `ColumnOrientedParser` (NWS-style column-per-metric with time arrays). Parser trait: `parse(payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>>`. The old `ResponseParser` trait (hardcoded structs: `WeatherParser`, `AirPollutionParser`) was deleted in air-006 — do not use.
- **successRate**: 0.88
- **tags**: http-polling, parser, config-driven, json, airnow, nws, owm, generic
- **source**: air-005, air-006, air-007, air-008
- **status**: current

---

### Pattern: Column-Oriented Parser for NWS-Style APIs

- **taskType**: coding:column-oriented-parser
- **approach**: NWS gridpoints API uses column-oriented JSON: each metric has its own `values` array of `{validTime, value}` objects. Use `ColumnOrientedParser` from `core/src/parsers/column_oriented.rs`. Config-drive the `column_mappings` (path to values array, metric_name, unit, optional flag). Timestamp extraction from ISO 8601 interval strings: split on `/`, parse first part with `DateTime::parse_from_rfc3339`. Do NOT try to use `FlatJsonParser` or `ArrayIteratorParser` for this shape — they cannot handle one-array-per-metric.
- **successRate**: 0.88
- **tags**: parser, nws, column-oriented, iso8601, weather, config-driven
- **source**: air-007
- **status**: current

---

### Pattern: Silver Layer = Simple Facts, Gold Layer = Computed Features

- **taskType**: architecture:silver-gold-separation
- **approach**: Silver stores cleaned facts only — no SCD semantics, no `previous_state`, no computed duration columns. SCD (Slowly Changing Dimension) semantics and derived features (state periods, durations, point-in-time queries) are computed in Gold as materialized views. Example: `silver.state_events` stores `(event_time, ndp_id, state)` only. Gold computes `valid_from`/`valid_to` via `LEAD() OVER (PARTITION BY ndp_id ORDER BY event_time)`. Rationale: baking features into Silver makes schema iteration expensive (requires migrations). Gold materialized views can be refreshed or rebuilt cheaply. See air-012 ADR-001.
- **successRate**: 0.92
- **tags**: silver, gold, scd, timescaledb, materialized-views, architecture, data-modeling
- **source**: air-012
- **status**: current

---

### Pattern: Parquet Bronze Storage — Per-Flush Sidecar Files

- **taskType**: architecture:parquet-sidecar-files
- **approach**: Do NOT use read-modify-write for Parquet appends (reading entire file + deserializing all rows + rewriting — this is O(file_size) per flush and causes OOM on Pi). Instead, each flush writes a new small Parquet file with epoch-microsecond suffix: `readings_{epoch_us}.parquet`. Read path globs all files in the partition directory. Memory cost becomes O(batch_size) constant. Tradeoff: ~2,880 small files per stream per day (~23K files/day total); read path slightly slower due to globbing. This was resolved definitively in air-016/air-017/air-018 after multiple OOM incidents. See air-016 ADR-001.
- **successRate**: 0.90
- **tags**: parquet, bronze, memory, oom, sidecar, append-only, raspberry-pi
- **source**: air-016, air-017
- **status**: current

---

### Pattern: WAL Owned by BronzeSubscriber, Not ParquetStore

- **taskType**: architecture:wal-ownership
- **approach**: The Write-Ahead Log (WAL) belongs to `BronzeSubscriber`, not `ParquetStore`. Rationale: durability is a subscriber concern — BronzeSubscriber is the first component to receive data and must ensure crash safety before any further processing. Flow: event received → WAL append (immediate durability) → in-memory accumulator insert → flush timer fires → ParquetStore snapshot → WAL commit. `ParquetStore` is a pure archival backend with no internal state between calls. `RawStore` trait stays clean (no WAL-specific methods). WAL file lives under the Bronze data directory for consistency. See air-017 ADR-001.
- **successRate**: 0.88
- **tags**: wal, bronze, durability, subscriber, parquet, architecture
- **source**: air-017
- **status**: current

---

### Pattern: In-Memory Accumulator — HashMap<String, Vec<RawDataPoint>>

- **taskType**: coding:bronze-accumulator
- **approach**: The BronzeSubscriber in-memory accumulator is `HashMap<String, Vec<RawDataPoint>>` where key is `stream_id`. Chosen over `BTreeMap<(String, DateTime), RawDataPoint>` because: BTree has O(log n) insert overhead and ~80 bytes per node overhead (vs ~0 for Vec), and timestamp-based dedup is incorrect (two points can share same timestamp). Ring buffers rejected: data loss by design contradicts durability goal. The accumulator survives across flush cycles (cleared only on day rollover) and is seeded from existing Parquet during startup recovery. See air-017 ADR-002.
- **successRate**: 0.87
- **tags**: accumulator, bronze, memory, rust, data-structures, hashmap
- **source**: air-017
- **status**: current

---

### Pattern: Replace Polars with arrow-rs for Bronze Write Path

- **taskType**: architecture:no-polars-in-bronze
- **approach**: Do NOT use Polars DataFrames in the Bronze write path (`core/src/storage/parquet.rs`). On Raspberry Pi 5 / Ubuntu 25.x (kernel 6.14+) / cgroup v2 / 512 MiB Docker, Polars causes a +4.5 MiB/cycle memory leak from heap fragmentation that glibc malloc cannot return to the OS. Use `arrow` (v54) + `parquet` (v54) crates directly. Write path: build `RecordBatch` with `Arc<dyn Array>` builders, use `ArrowWriter`. Read path: `ParquetRecordBatchReader` with array downcasts. Cargo: `arrow = { version = "54", default-features = false, features = ["chrono-tz"] }`, `parquet = { version = "54", default-features = false, features = ["snap", "arrow"] }`. IMPORTANT: `silver-etl` and `air-quality-app` may still use Polars in dev-dependencies — do not remove workspace-level Polars definition. See air-018 ADR-001.
- **successRate**: 0.95
- **tags**: polars, arrow-rs, parquet, memory, oom, raspberry-pi, deprecated-polars
- **source**: air-018
- **status**: current

---

### Pattern: DEPRECATED — Polars in Bronze Write Path

- **taskType**: deprecated:polars-bronze
- **approach**: DEPRECATED. Using Polars DataFrames (`Series::new` + `DataFrame::new` + `ParquetWriter`) in `core/src/storage/parquet.rs` causes an unrecoverable memory leak on Raspberry Pi 5 / kernel 6.14+ / cgroup v2. The `malloc_trim(0)` workaround only reclaims 91% per cycle, leaving +4.5 MiB/cycle residual. Alternative allocators (jemalloc, mimalloc) both crash on this platform. Use `arrow-rs` + `parquet` crates directly instead. Do NOT retry alternative allocators without verifying kernel/cgroup interaction on target Pi first.
- **successRate**: 0.0
- **tags**: polars, deprecated, memory-leak, oom, raspberry-pi, bronze
- **source**: air-018
- **status**: deprecated

---

### Pattern: DEPRECATED — ResponseParser Trait (Hardcoded HTTP Parsers)

- **taskType**: deprecated:response-parser-hardcoded
- **approach**: DEPRECATED. The `ResponseParser` trait (`core/src/sources/http_poll.rs`) with implementations `WeatherParser` and `AirPollutionParser` (hardcoded structs) was deleted in air-006. These required code changes to add new APIs. Use the config-driven `Parser` trait from `core/src/parsers/` instead — all parsing is YAML-configurable with no code changes required.
- **successRate**: 0.0
- **tags**: deprecated, parser, http, hardcoded, response-parser
- **source**: air-006
- **status**: deprecated

---

### Pattern: DEPRECATED — Parsers in HTTP Ingestion Path

- **taskType**: deprecated:parsers-in-ingestion
- **approach**: DEPRECATED. Calling parsers during HTTP polling (in `HttpPollingSource::fetch()`) causes memory exhaustion on Pi. Parsed `TimeSeriesPoint` results accumulated in channels that were never drained, causing Pi lockups after hours of operation. The fix (air-011): decouple parsers from ingestion entirely. Sources return raw JSON (`RawDataPoint`) only. Parsers are reserved for Silver ETL (feature-gated with `#[cfg(feature = "etl")]`). `HttpPollingSource::new()` no longer takes a parser argument.
- **successRate**: 0.0
- **tags**: deprecated, parser, ingestion, memory, pi-lockup
- **source**: air-011
- **status**: deprecated

---

### Pattern: London School TDD for Domain Layer

- **taskType**: testing:london-school-tdd
- **approach**: Domain layers (e.g. `domains/air-quality/`) are built using London School TDD: write tests first, verify behavior not implementation. Test structure: `types.rs` has 10 tests for struct construction and serde, `parser.rs` has 20 tests including edge cases (null values, partial payloads, type coercion), `validation.rs` has 27 tests (one per boundary condition per field). All sensor fields are `Option<T>` — tests must verify graceful handling of missing fields. Use hardware-spec ranges for validation bounds (not guesses): CO2 380-10,000 ppm (SenseAir S8), PM 0-500 µg/m³ (PMS5003), TVOC/NOx 1-500 (SGP41), temp -10 to 50°C (SHT40), humidity 0-100%.
- **successRate**: 0.88
- **tags**: testing, tdd, london-school, domain, validation, sensors
- **source**: air-001
- **status**: current

---

### Pattern: Silver Layer — Simple Event Log Schema

- **taskType**: architecture:silver-event-log-schema
- **approach**: For state/event data (e.g. window open/closed from Home Assistant), Silver stores a minimal event log, not a computed history. Schema: `(event_time TIMESTAMPTZ NOT NULL, ndp_id TEXT NOT NULL, state TEXT NOT NULL, dq_flags TEXT[], PRIMARY KEY (event_time, ndp_id))`. Create as TimescaleDB hypertable: `SELECT create_hypertable('silver.state_events', 'event_time')`. No computed columns (`previous_state`, duration). Entity metadata (category, friendly_name, location) lives in a separate `entity_context` dimension table — JOIN at query time. SCD semantics computed in Gold via `LEAD() OVER (PARTITION BY ndp_id ORDER BY event_time)`. See air-012 ADR-001.
- **successRate**: 0.90
- **tags**: silver, timescaledb, hypertable, event-log, schema, state-events
- **source**: air-012
- **status**: current

---

### Pattern: Silver ETL Config Must Be in etcd (Not YAML at Runtime)

- **taskType**: architecture:silver-etl-config-in-etcd
- **approach**: `silver_etl` config must be part of `StreamConfig` and stored in etcd — not read from YAML files at runtime. The failure mode without this: `ConfigSyncService.sync_all()` fails (e.g. validation error) → stream not in etcd → `list_streams()` doesn't return it → `load_silver_etl_config()` never called → `SilverSubscriber` silently not created. Fix: add `silver_etl: Option<SilverEtlConfig>` to `StreamConfig`, include in `ConfigSyncService.sync_all()`. At startup: for each stream in etcd, create `SilverSubscriber` if `config.silver_etl.enabled`. Remove YAML file mounts for Silver ETL. See air-013 SCOPE.md.
- **successRate**: 0.90
- **tags**: silver-etl, etcd, config, silent-failure, stream-config
- **source**: air-013
- **status**: current

---

### Pattern: Self-Healing Silver ETL with Circuit Breaker

- **taskType**: architecture:silver-self-healing
- **approach**: SilverSubscriber must handle TimescaleDB outages without data loss. Pattern: circuit breaker (Closed → Open → HalfOpen → Closed), watermark persistence, and Bronze catch-up. On sustained failures (default: 5 consecutive), circuit opens. On recovery, `ParquetBronzeReader` catches up from last watermark to now. Config in `silver_etl.self_healing`: `circuit_breaker.failure_threshold`, `circuit_breaker.health_check_interval_secs`, `watermark.persist_interval_secs`, `catch_up.max_window_secs`, `catch_up.batch_size`. Watermark file: `/var/lib/ndp/watermarks/{stream_id}.txt`. Use UPSERT for catch-up writes to prevent duplicates. See air-014 SCOPE.md.
- **successRate**: 0.85
- **tags**: silver, circuit-breaker, self-healing, watermark, catch-up, timescaledb
- **source**: air-014
- **status**: current

---

### Pattern: Config Directory Semantics and Lifecycle

- **taskType**: conventions:config-directory-lifecycle
- **approach**: `config/base/` = production (deployed to Pi, stable only). `config/domains/` = production domain configs (Gold layer). `config/schemas/` = platform schemas (environment-agnostic). `config/grafana/` = production Grafana. `config/overlays/{development,integration,production}/` = environment-specific overrides merged at deploy time. `config/samples/` = documentation only. `config/duckdb/` = DEPRECATED, remove it. New features must use `enabled: false` in production configs until deployment-ready. Tests use `tests/fixtures/` only — never modify `config/base/` during testing. Config lifecycle: Draft (fixtures) → Validated (config/base/ after CI passes) → Deployed (target env). See air-015 SCOPE.md.
- **successRate**: 0.87
- **tags**: config, conventions, directory, lifecycle, duckdb-deprecated, overlays
- **source**: air-015
- **status**: current

---

### Pattern: DEPRECATED — DuckDB as ETL Engine or Gold Layer

- **taskType**: deprecated:duckdb
- **approach**: DEPRECATED. DuckDB was eliminated from the architecture entirely. Do not use DuckDB as an ETL engine, Gold layer storage, or for any production data processing. Use TimescaleDB continuous aggregates for Silver aggregations and materialized views for Gold layer. If you see `config/duckdb/` in the repo, remove it. Any reference to DuckDB in architecture discussions is outdated.
- **successRate**: 0.0
- **tags**: deprecated, duckdb, etl, gold, architecture
- **source**: air-015, CLAUDE.md
- **status**: deprecated

---

### Pattern: DEPRECATED — Polars Streaming for ETL

- **taskType**: deprecated:polars-streaming
- **approach**: DEPRECATED. Polars with streaming was eliminated from ETL. Use TimescaleDB continuous aggregates for aggregations, not Polars lazy evaluation or streaming. Polars itself is still used in `silver-etl` dev-dependencies for testing only — but not in the production write path of `core/`.
- **successRate**: 0.0
- **tags**: deprecated, polars, streaming, etl, timescaledb
- **source**: CLAUDE.md, air-018
- **status**: deprecated

---

### Pattern: Docker Deployment on Raspberry Pi 5 with Memory Constraints

- **taskType**: architecture:pi-deployment-constraints
- **approach**: Air quality app container hard limit: 512 MiB. Design for <250 MiB steady-state RSS. Memory pressure events to avoid: Polars DataFrame create/drop cycles (4.5 MiB/cycle leak), read-modify-write Parquet appends (O(file_size) spike), alternative allocators (jemalloc/mimalloc both fail on Pi 5/kernel 6.14+/cgroup v2). Safe approaches: arrow-rs RecordBatch construction (O(batch_size) constant), per-flush sidecar Parquet files, jemalloc global allocator is NOT safe on this platform. Container uses tikv-jemallocator was tested and failed. Diagnostic: add RSS logging with `malloc_trim(0)` after each write cycle to track leaks. Use `docker stats` to monitor container memory.
- **successRate**: 0.93
- **tags**: raspberry-pi, docker, memory, oom, deployment, constraints, 512mb
- **source**: air-016, air-017, air-018
- **status**: current

---

## Summary Statistics

- Total patterns: 22
- Current patterns: 17
- Deprecated patterns: 5
- Features covered: air-001 through air-018
- Key themes: hexagonal architecture, Bronze/Silver/Gold data lake, config-driven ingestion, memory management on Pi, Parquet storage evolution

## Deprecated Approaches (Quick Reference)

| Approach | Replacement | Source |
|----------|-------------|--------|
| DuckDB for ETL/Gold | TimescaleDB continuous aggregates + materialized views | air-015, CLAUDE.md |
| Polars streaming ETL | TimescaleDB aggregates | CLAUDE.md |
| Polars in Bronze write path | arrow-rs + parquet crates directly | air-018 |
| ResponseParser trait (hardcoded) | Config-driven Parser trait | air-006 |
| Parsers in HTTP ingestion path | Sources return raw JSON, parsers in Silver ETL only | air-011 |
| Read-modify-write Parquet appends | Per-flush sidecar files + WAL in BronzeSubscriber | air-016, air-017 |
| SCD in Silver layer | Simple event log in Silver, Gold computes derived features | air-012 |
| silver_etl config from YAML at runtime | silver_etl in etcd via StreamConfig | air-013 |
| WAL in ParquetStore | WAL in BronzeSubscriber (durability = subscriber concern) | air-017 |
| jemalloc/mimalloc on Pi 5/kernel 6.14+ | glibc + malloc_trim (not perfect but functional) | air-018 |
