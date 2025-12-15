# AIR-004 Implementation Constraints - Quick Reference

## NON-NEGOTIABLE RULES

### Rule 1: Preserve Existing Functionality
**The MQTT AirGradient ingestion pipeline MUST continue working throughout AIR-004**

Current working components:
- `core/src/sources/mqtt.rs` (MqttSource)
- `core/src/storage/parquet.rs` (ParquetStore)
- `config-client/src/lib.rs` (ConfigClient)
- etcd namespace: `/air-quality/*`

**Action**: Do NOT refactor, rewrite, or break these components

---

### Rule 2: Backward Compatible Configuration
**Existing etcd keys MUST remain valid**

Current keys:
```
/air-quality/server/{host,port,graceful_shutdown_timeout_secs}
/air-quality/mqtt/{broker_url,port,client_id,topic_pattern,qos,...}
/air-quality/storage/{base_path,wal_enabled,batch_size,...}
/air-quality/alerts/{enabled,thresholds...}
/air-quality/logging/{level,format}
```

**Action**: Add new `streams/*` namespace WITHOUT touching `/air-quality/*`

---

### Rule 3: No Performance Regression
**New features MUST NOT slow down existing functionality**

Current baselines:
- Config reads: <10ms
- MQTT ingestion: 1+ msg/sec sustained
- Parquet writes: 10k records/sec batch
- Reconnect: exponential backoff 1s → 30s

**Action**: Benchmark before/after each phase, fail if regression >10%

---

### Rule 4: Data Continuity
**Existing Parquet data MUST remain queryable**

Current structure:
```
data/{location}/year={YYYY}/month={MM}/day={DD}/readings.parquet
```

**Action**: New streams can use different paths, but preserve existing path for air-quality

---

### Rule 5: Additive Implementation
**Add features alongside existing code, don't replace**

**WRONG Approach**:
```rust
// Phase 1: Refactor MqttSource to generic Source
impl GenericSource for MqttSource { ... } // BREAKS EXISTING CODE
```

**RIGHT Approach**:
```rust
// Phase 1: Create stream coordinator, leave MqttSource alone
struct StreamCoordinator {
    sources: Vec<Box<dyn Source>>  // MqttSource already implements Source trait
}

// Phase 2: Add air-quality to coordinator via feature flag
if config.use_multi_stream {
    coordinator.add_stream(mqtt_source);
} else {
    mqtt_source.start();  // Legacy path still works
}
```

---

## Implementation Phases Constraints

### Phase 0: Baseline Verification
**Constraint**: MUST create passing regression tests before writing any new code

Deliverables:
- [ ] Integration test: MQTT → Parquet end-to-end
- [ ] Benchmark: Config read performance
- [ ] Benchmark: MQTT ingestion throughput
- [ ] Benchmark: Parquet write performance
- [ ] Verification: Query existing Parquet data

**Gate**: Phase 1 cannot start until all tests pass

---

### Phase 1: Stream Registry
**Constraint**: MUST NOT modify `/air-quality/*` namespace

Allowed:
- ✅ Create `streams/*` namespace
- ✅ Add ConfigClient methods for stream registry
- ✅ Add backward compat layer mapping `/air-quality/*` → `streams/air-quality/*`

Forbidden:
- ❌ Rename `/air-quality/*` keys
- ❌ Change config-client API (breaking changes)
- ❌ Modify MqttSource or ParquetStore

**Gate**: Regression tests from Phase 0 MUST still pass

---

### Phase 2: Multi-Stream Coordinator
**Constraint**: Feature flag for rollback, air-quality works via BOTH paths

Allowed:
- ✅ Create StreamCoordinator struct
- ✅ Add air-quality to coordinator (opt-in)
- ✅ Integrate HttpPollingSource from `core/src/sources/http_poll.rs`

Required Testing:
- [ ] Air-quality via legacy path (no coordinator)
- [ ] Air-quality via coordinator path
- [ ] Verify identical behavior between paths
- [ ] Load test: 2 streams don't interfere

**Gate**: Can run air-quality without coordinator (rollback works)

---

### Phase 3: Schema & Storage
**Constraint**: Air-quality schema auto-inferred, no manual migration

Allowed:
- ✅ Add `streams/{id}/schema` etcd namespace
- ✅ Infer air-quality schema from AirGradientReading struct
- ✅ Route new streams to separate Parquet paths

Forbidden:
- ❌ Require manual schema definition for air-quality
- ❌ Change air-quality Parquet path structure
- ❌ Break backward compat with existing Parquet files

**Gate**: Can query air-quality data written before AND after this phase

---

### Phase 4: Webhook & Observability
**Constraint**: Metrics MUST include single-stream baseline comparison

Required Metrics:
- `ingestion_records_per_second{stream="air-quality"}` (compare to Phase 0 baseline)
- `config_read_duration_seconds` (compare to Phase 0 baseline)
- `storage_write_duration_seconds{stream="air-quality"}` (compare to Phase 0 baseline)

**Gate**: No metric shows >10% regression vs Phase 0

---

### Phase 5: Validation
**Constraint**: Production-readiness proven by air-quality stream stability

Required Tests:
- [ ] Air-quality runs for 24 hours without intervention
- [ ] MQTT broker restart: auto-reconnect works
- [ ] Config change: hot-reload works without data loss
- [ ] Performance: matches Phase 0 baseline ±10%
- [ ] Rollback: can disable multi-stream, revert to legacy

**Gate**: All Phase 0 regression tests pass, performance within ±10%

---

## Code Review Checklist

Before merging any AIR-004 PR:

- [ ] Does this PR modify `core/src/sources/mqtt.rs`? → Reject unless critical bugfix
- [ ] Does this PR modify `core/src/storage/parquet.rs`? → Reject unless critical bugfix
- [ ] Does this PR modify `config-client/src/*.rs`? → Only if additive (no breaking changes)
- [ ] Does this PR change etcd keys under `/air-quality/*`? → Reject
- [ ] Does this PR add feature flags for rollback? → Approve
- [ ] Does this PR include regression tests? → Require before merge
- [ ] Does this PR benchmark performance vs Phase 0? → Require before merge

---

## Rollback Plan

If AIR-004 breaks air-quality monitoring:

### Immediate Rollback (< 5 minutes)
```bash
# Disable multi-stream feature flag
etcdctl put /air-quality/features/multi_stream false

# Restart service
docker-compose restart air-quality-app

# Verify MQTT ingestion resumed
docker logs -f air-quality-app | grep "Connected to MQTT"
```

### Full Rollback (< 30 minutes)
```bash
# Revert to git commit before AIR-004
git checkout <commit-before-air-004>

# Rebuild and deploy
docker-compose build air-quality-app
docker-compose up -d air-quality-app

# Verify etcd config still valid
etcdctl get --prefix /air-quality/
```

**Key**: `/air-quality/*` config never changed, so rollback is safe

---

## Success Criteria

AIR-004 is complete when:

1. ✅ Air-quality MQTT ingestion still works (regression tests pass)
2. ✅ Can add new stream (weather) via `streams/*` config
3. ✅ Both streams write to Parquet independently
4. ✅ Performance within ±10% of Phase 0 baseline
5. ✅ Rollback feature flag works (can disable multi-stream)
6. ✅ Documentation includes migration guide
7. ✅ Grafana dashboards show both streams

**Final Gate**: Run air-quality for 7 days, verify no regressions

---

## Red Flags During Implementation

**STOP and re-evaluate if you see:**

- "Refactoring MqttSource to support..."
- "Migrating /air-quality/* to streams/*..."
- "Breaking change in config-client API..."
- "Parquet path structure changed..."
- "Need to manually migrate existing data..."
- "Air-quality temporarily broken during..."

**These indicate deviation from additive approach**

---

## Contact / Escalation

If unclear whether a change violates constraints:

1. Check this document first
2. Run Phase 0 regression tests
3. Benchmark performance vs baseline
4. If still uncertain, prefer NOT making the change

**Principle**: When in doubt, preserve existing functionality

---

*Document Purpose*: Quick reference for developers implementing AIR-004
*Last Updated*: 2025-12-15
*Related*: SPECIFICATION.md v1.1.0, REVISION_SUMMARY.md
