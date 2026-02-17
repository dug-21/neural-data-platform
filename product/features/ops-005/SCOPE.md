# ops-005: Config-Driven Lifecycle Edge Cases

## Vision

The platform principle is: **config drives behavior, not data lifecycle**. Adding config starts processing. Removing config stops processing. Neither creates nor destroys data. Data deletion requires explicit, intentional action.

This feature audits every lifecycle edge case across Bronze/Silver/Gold layers, determines what SHOULD happen, discovers what ACTUALLY happens today, and closes the gaps.

## Tracking

- GitHub Issue: #26 (tracking), #27-#35 (categories)

## Edge Cases

### Category 1: Config Removal

**EC-01: Remove Gold Domain**
- Current: Unknown
- Expected: Stop refresh jobs (CAs, aligned view, detection). Optionally drop CAs (derived, recreatable from Silver). Preserve `gold.events` (original detection data). Never drop tables without explicit action.

**EC-02: Remove Silver defs for a stream**
- Current: Unknown
- Expected: Stop SilverSubscriber. Preserve hypertable. Warn about Gold dependencies (CAs still refresh but get no new data). Config removal = stop writing, not delete data.

**EC-03: Remove stream config entirely**
- Current: Unknown
- Expected: Cascading stop — source stops polling/MQTT, Bronze stops WAL writes, Silver stops ETL, Gold loses a feed. Preserve all data at every layer. Re-adding config should resume seamlessly.

### Category 2: Config Addition (with backfill)

**EC-04: Add Gold streams or domain**
- Current: Unknown (CAs may auto-backfill via `refresh_continuous_aggregate(ca, NULL, NULL)`)
- Expected: Create CAs with auto-backfill from Silver. Refresh aligned view. Run `detect_events_for_range()` for historical coverage. Zero manual intervention.

**EC-05: Add Silver to a stream**
- Current: Unknown (HybridBronzeReader exists but catch-up flow untested for full backfill)
- Expected: Create hypertable. Run `BronzeReader::read_since(DateTime::MIN)` for full Bronze history. Then start live ETL. Retroactive analytics on historical Bronze data.

### Category 3: Config Mutation

**EC-06: Change field mappings in `silver_etl`**
- Current: Unknown
- Expected: New field added = ALTER TABLE ADD COLUMN, old rows get NULL. Field removed = stop populating, do NOT drop column. Additive-only schema evolution.

**EC-07: Change Gold aggregation granularity** (e.g., hourly to 15min)
- Current: Unknown (CAs can't be ALTERed)
- Expected: `recreate` action drops old CA, creates new, auto-backfills from Silver. No data loss.

**EC-08: Change retention policies**
- Current: Unknown
- Expected: Update policy in-place. Warn if lowering retention (irreversible data loss).

**EC-09: Change domain objectives** (thresholds)
- Current: Unknown
- Expected: Update `data_dictionary.objectives`. Regenerate detection procedure. Optionally re-run detection for historical data.

**EC-10: Add/remove stream from existing domain**
- Current: Unknown
- Expected: Regenerate aligned view with new JOIN shape. Refresh.

### Category 4: Soft Lifecycle (enable/disable)

**EC-11: Disable stream** (`enabled: false`)
- Current: Unknown (no `enabled` field exists?)
- Expected: Stop source, stop Bronze, stop Silver. Gold CAs keep refreshing (stale but valid). Re-enable resumes. Distinct from remove.

**EC-12: Disable Silver ETL** (`silver_etl.enabled: false`)
- Current: Partially implemented (field exists in config)
- Expected: Source continues to Bronze. Silver stops. Pending WAL question: drain first?

**EC-13: Disable Gold** (`gold_etl.enabled: false`)
- Current: Partially implemented (field exists in config)
- Expected: Stop CA refresh jobs, stop detection job. Keep tables/views intact.

### Category 5: Data Recovery / Rebuild

**EC-14: Silver data corrupted — drop/recreate from Bronze**
- Current: Unknown
- Expected: Drop Silver hypertable. Recreate schema. Full backfill from Bronze via HybridBronzeReader (all Parquet history + WAL today). Question: do Gold CAs need drop/recreate? (Yes — CAs reference the underlying hypertable. If the hypertable is dropped, CAs are orphaned and must be recreated too.)
- Cascade: Silver drop -> Gold CAs must be dropped/recreated -> Aligned views regenerated -> Detection re-run for historical range
- Command: something like `ndp silver rebuild <stream-id>` or `ndp silver rebuild --all`

**EC-15: Gold data corrupted — recreate by stream, aggregate, or domain**
- Current: `ndp gold recreate --stream` exists but scope unclear
- Expected: Three granularities of rebuild:
  - **By stream**: Drop + recreate all CAs for one stream (hourly, daily). Auto-backfill from Silver.
  - **By aggregate**: Drop + recreate one specific CA (e.g., `gold.air_quality_hourly`). Auto-backfill.
  - **By domain**: Drop + recreate aligned view, all domain CAs, events CAs, detection procedure. Re-run detection.
- Principle: Gold is 100% derived from Silver. Any Gold artifact can be destroyed and perfectly recreated. Events table is the exception — detection results depend on point-in-time thresholds.
- Command: `ndp gold recreate --stream <id>`, `ndp gold recreate --domain <id>`, `ndp gold recreate --table <name>`

### Category 6: Infrastructure Failures

**EC-16: TimescaleDB goes offline**
- Current: Unknown (Silver ETL likely panics or logs errors)
- Expected: Bronze continues (WAL + Parquet are filesystem only). Silver ETL logs errors. WAL accumulates. On DB return, BronzeReader catch-up fills the gap. No data loss.

**EC-17: Disk full**
- Current: Unknown
- Expected: WAL can't append, Parquet can't write. Log CRITICAL. Silver live path still works if TimescaleDB has space. Graceful degradation, not crash.

**EC-18: Source goes offline** (API 5xx, MQTT broker down)
- Current: Retry with backoff (implemented)
- Expected: Log warnings, no data corruption, auto-resume. No config change needed.

### Category 7: Data Consistency

**EC-19: Orphaned data** — Bronze has data for stream_id with no config
- Current: Data sits on disk, never read
- Expected: `ndp status` should flag orphaned streams. No automatic deletion.

**EC-20: Late-arriving data** (timestamp older than Silver's last processed)
- Current: Unknown (UPSERT vs INSERT behavior)
- Expected: Silver handles via UPSERT. CAs auto-refresh catches within window.

**EC-21: Duplicate stream IDs across configs**
- Current: ndp-validate may catch this
- Expected: Validation error at deploy time, before any processing starts.

**EC-22: Multiple instances** (accidental double-start)
- Current: Unknown (WAL file locking?)
- Expected: File locks prevent WAL corruption. Silver UPSERT handles duplicate inserts. Detect and warn.

### Category 8: Operations / Rollback

**EC-23: Git rollback to previous config version**
- Current: Unknown
- Expected: DB schema changes persist (append-only). Old configs work because DDL uses IF NOT EXISTS. Column additions not reversed. Principle: DB schema is append-only unless explicit migration.

### Category 9: Performance Testing

**EC-24: End-to-end performance baseline**
- Current: No performance testing framework exists
- Expected: With Gold functional, establish baseline metrics for:
  - **Ingestion throughput**: events/sec from source to Bronze WAL
  - **Silver ETL latency**: time from event receipt to Silver hypertable row
  - **Gold refresh latency**: CA refresh duration, aligned view refresh duration
  - **Detection latency**: time from threshold crossing to event row in `gold.events`
  - **Query performance**: Gold CA queries, aligned view queries, event queries
  - **Memory profile**: RSS over 24h, 48h, 7d under sustained load
  - **Disk growth**: WAL size, Parquet size, TimescaleDB size per day per stream
  - **Recovery time**: full Bronze-to-Silver backfill time, Gold recreate time
- Approach: synthetic load generator that simulates N streams at M events/sec. Run against integration environment. Capture metrics. Establish baselines before adding intelligence layer (fe-004+).
- Tools: could use `ndp perf` CLI command, or standalone test harness

## Research Plan

Phase 1: Deep codebase analysis — for each edge case, trace the code path and document what ACTUALLY happens today.

Phase 2: Gap analysis — compare actual vs expected behavior. Categorize gaps as:
- P0: Data loss risk (fix immediately)
- P1: Silent failure (fix before production hardening)
- P2: Missing feature (implement when needed)
- P3: Nice-to-have (backlog)

Phase 3: Implementation — close gaps per priority.
