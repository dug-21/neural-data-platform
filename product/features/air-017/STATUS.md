# AIR-017: Bronze Write-Ahead Architecture -- Status

| Field | Value |
|-------|-------|
| Feature ID | air-017 |
| Status | Implementation |
| Current Phase | Phase 1 Complete — All tests passing, ready for release |
| Depends On | air-016 Phase 1 (time buffer, not prerequisite) |
| Started | 2026-02-08 |
| Target | TBD |

## Phase Tracking

| Phase | SPARC Stage | Status | Notes |
|-------|------------|--------|-------|
| Phase 1 | Implementation | Complete | P1-01 through P1-10 done; 861 tests pass in 1.66s |
| Phase 2 | Not started | Pending | Day rollover + WAL watermarking |
| Phase 3 | Not started | Pending | Read path integration + Silver resilience |

## SPARC Artifacts

| Artifact | Status | Path |
|----------|--------|------|
| SCOPE.md | Complete | product/features/air-017/SCOPE.md |
| SPECIFICATION.md | Complete | product/features/air-017/specification/SPECIFICATION.md |
| TEST-PLAN.md | Complete | product/features/air-017/specification/TEST-PLAN.md |
| PSEUDOCODE.md | Complete | product/features/air-017/pseudocode/PSEUDOCODE.md |
| ARCHITECTURE.md | Complete | product/features/air-017/architecture/ARCHITECTURE.md |
| ADR-001 to ADR-005 | Complete | product/features/air-017/architecture/ADR-AIR017-*.md |
| REFINEMENT.md | Complete | product/features/air-017/refinement/REFINEMENT.md |
| COMPLETION.md | Complete | product/features/air-017/completion/COMPLETION.md |
| STATUS.md | Complete | product/features/air-017/STATUS.md |

## Progress

- [x] SCOPE.md created
- [x] SPARC Specification complete
- [x] SPARC Pseudocode complete
- [x] SPARC Architecture complete (5 ADRs)
- [x] SPARC Refinement complete
- [x] SPARC Completion plan complete
- [x] P1-01: WAL v2 (sequence numbers, watermark, commit_to) — 26 tests
- [x] P1-02: InMemoryAccumulator — 15 tests
- [x] P1-03: WAL moved to BronzeSubscriber event receipt
- [x] P1-04: Snapshot timer added to select! loop
- [x] P1-05: snapshot_to_parquet() implemented
- [x] P1-06: Startup recovery (Parquet seed + WAL replay) — 6 tests
- [x] P1-07: BronzeSubscriberConfig new fields (snapshot_interval_secs, day_rollover_utc_hour)
- [x] P1-08: ParquetStore WAL removed from write_raw_batch, append_to_raw_parquet deprecated
- [x] P1-09: Unit tests — 34 bronze + 79 storage + 64 traits passing
- [x] P1-10: Integration tests (full cycle with real ParquetStore) — 4 tests
- [x] Full test suite verification (`cargo test -p platform-core --lib`) — 861 tests, 1.66s
- [x] Coordinator test hang fix (3 tests used CancellationToken)
- [x] Phase 1 released (v1.2.0)
- [ ] Phase 2 implemented
- [ ] Phase 2 tests passing
- [ ] Phase 3 implemented
- [ ] Phase 3 tests passing
- [ ] Memory profiled on Pi (peak RSS < 150 MiB)
- [ ] Documentation updated
- [ ] Deployed to production

## Implementation Waves (Phase 1)

### Wave 1 — Foundation (Complete)
| Item | File | Tests |
|------|------|-------|
| P1-01 WAL v2 | `core/src/storage/wal.rs` | 26 |
| P1-02 Accumulator | `core/src/storage/accumulator.rs` (new) | 15 |
| P1-07 Config | `core/src/subscribers/bronze.rs` | 4 |
| P1-08 partial | `core/src/storage/parquet.rs`, `core/src/traits.rs` | existing |

### Wave 2 — BronzeSubscriber Integration (Complete)
| Item | File | Tests |
|------|------|-------|
| P1-03 WAL on receipt | `core/src/subscribers/bronze.rs` | 28 total |
| P1-04 Snapshot timer | `core/src/subscribers/bronze.rs` | (included) |
| P1-05 snapshot_to_parquet | `core/src/subscribers/bronze.rs` | (included) |

### Wave 3 — Recovery + Cleanup (Complete)
| Item | File | Tests |
|------|------|-------|
| P1-06 Startup recovery | `core/src/subscribers/bronze.rs` | 34 total (+6 new) |
| P1-08 ParquetStore cleanup | `core/src/storage/parquet.rs` | 79 storage pass |

## Files Modified

| File | Change |
|------|--------|
| `core/src/storage/wal.rs` | Rewritten: v2 API with WalEntry, sequence numbers, watermark, commit_to |
| `core/src/storage/accumulator.rs` | New: HashMap-based accumulator with dedup, memory estimation |
| `core/src/storage/mod.rs` | Added pub mod accumulator, exports |
| `core/src/storage/parquet.rs` | WAL removed from write_raw_batch; append_to_raw_parquet deprecated |
| `core/src/subscribers/bronze.rs` | Rewritten: WAL on receipt, accumulator, snapshot, recovery |
| `core/src/traits.rs` | Added write_raw_snapshot to RawStore trait |
| `apps/air-quality-app/src/main.rs` | Updated BronzeSubscriber constructor call |
| `config/base/platform.yaml` | Added snapshot_interval_secs, day_rollover_utc_hour |

## Active Work

Phase 1 implementation complete (P1-01 through P1-10). All 861 platform-core lib tests pass in 1.66s.
Integration tests cover: full ingest-snapshot cycle, crash recovery, snapshot overwrite, multi-stream isolation.
Coordinator test hang fixed via CancellationToken (3 tests previously blocked forever).
Ready for release per RELEASE-POLICY.md.

## Bugs

| ID | Status | Summary |
|----|--------|---------|
| (none yet) | | |

## Decisions Log

| ADR | Decision |
|-----|----------|
| ADR-AIR017-001 | WAL moves from ParquetStore to BronzeSubscriber |
| ADR-AIR017-002 | Accumulator uses HashMap&lt;String, Vec&lt;RawDataPoint&gt;&gt; |
| ADR-AIR017-003 | Sequence-numbered WAL with watermark-based truncation |
| ADR-AIR017-004 | Full Parquet overwrite from accumulator (no read) |
| ADR-AIR017-005 | Keep Polars, deprecate read-modify-write methods |

## Implementation Decisions (during coding)

| Decision | Rationale |
|----------|-----------|
| WAL failure = point not added to accumulator | Pseudocode ADR: durability before memory |
| Single WAL file (not per-stream) | Phase 1 simplicity; per-stream can be Phase 2 |
| BronzeSubscriber owns WAL directly (not via RawStore) | WAL is a local subscriber concern, not a storage trait |
| Partition path replicated in BronzeSubscriber | Avoids coupling to ParquetStore internals |
| Recovery: non-fatal errors for Parquet read | WAL replay still works if Parquet is corrupt |

## Blockers

- None currently

## Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Accumulator memory exceeds budget | Low | Medium | 22 MiB for current volumes; monitor; add eviction if needed |
| Day rollover timer drift | Low | Low | Recompute next midnight on each tick; use wall clock |
| WAL grows unbounded if snapshots fail | Medium | Medium | Cap WAL size; alert on failure; retry |
| Silver catch-up reads stale Parquet | High (given air-017) | Medium | Phase 3 option A or forced snapshot before exit |
| Pre-existing Silver data-loss on DB downtime | Medium | Medium | Separate feature or Phase 3 |

## Branch

`main` (trunk-based development)

## Last Updated

2026-02-08 by ndp-scrum-master
