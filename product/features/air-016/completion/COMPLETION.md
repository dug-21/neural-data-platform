# AIR-016 Phase 1 Completion: Allocator-Level Memory Management

> **Feature:** AIR-016 Parquet Append-Only Writes
> **Phase:** 1 of 2 (Allocator Fix + Explicit Memory Management)
> **Release:** v1.1.14 (PATCH -- bugfix, no API changes)
> **Date:** 2026-02-07

---

## Acceptance Criteria

All criteria must pass before the release commit is created.

### Build and Test

- [ ] `cargo build -p air-quality-app` succeeds with `tikv-jemallocator`
- [ ] `cargo test --workspace` -- all existing tests pass, zero regressions
- [ ] `cargo clippy --workspace` -- no new warnings (the `unsafe` block for `malloc_trim` is a well-known FFI call)
- [ ] `cargo fmt --all --check` -- no formatting violations
- [ ] Binary size increase < 500 KB (jemalloc typically adds ~200 KB)

### Functional Correctness

- [ ] Parquet read/write round-trip produces identical results to pre-change output
- [ ] WAL continues to function unchanged
- [ ] Store trait interface has zero signature changes
- [ ] Silver ETL reads Bronze Parquet files without errors
- [ ] MCP server reads Bronze Parquet files without errors

### Memory Behavior (post-deploy)

- [ ] RSS stays below 200 MiB after 24h sustained operation on Pi
- [ ] RSS does not grow linearly with time (plateau within first 2 hours)
- [ ] No OOM kills observed over 48h observation window

---

## Files Modified

| File | Change | Lines |
|------|--------|-------|
| `apps/air-quality-app/Cargo.toml` | Add `tikv-jemallocator = "0.6"` to `[dependencies]` | +1 |
| `apps/air-quality-app/src/main.rs` | Global allocator declaration (`#[global_allocator]`) at top of file | +5 |
| `core/Cargo.toml` | Add `libc = "0.2"` to `[dependencies]` | +1 |
| `core/src/storage/parquet.rs` | `drop(points)` in `write_parquet()` after column extraction loop | +1 |
| `core/src/storage/parquet.rs` | `drop(points)` in `write_raw_parquet()` after column extraction loop | +1 |
| `core/src/storage/parquet.rs` | `malloc_trim(0)` at end of `append_to_parquet()` | +4, -1 |
| `core/src/storage/parquet.rs` | `malloc_trim(0)` at end of `append_to_raw_parquet()` | +4, -1 |

**Total: 4 files, ~15 lines added, 2 lines modified. Zero trait changes. Zero read-path changes.**

---

## Verification Commands

### 1. Build

```bash
cargo build -p air-quality-app
```

### 2. Tests

```bash
# Core parquet tests
cargo test -p platform-core -- parquet

# App tests
cargo test -p air-quality-app

# Full workspace (confirm no regressions)
cargo test --workspace
```

### 3. Lint

```bash
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
```

### 4. Binary Size Delta

```bash
# Before (build from main without changes, record size)
# After (build with air-016 changes, compare)
ls -la target/debug/air-quality-server

# Acceptable: increase < 500 KB
```

### 5. Cross-Compile Check (if available)

```bash
# Verify tikv-jemallocator compiles for the Pi target
cargo build -p air-quality-app --target aarch64-unknown-linux-gnu
```

If cross-compilation toolchain is not available on the dev machine, this is verified during the Docker build on the Pi itself.

---

## Deployment Checklist

Following `docs/procedures/RELEASE-POLICY.md`. This is a PATCH release (backwards-compatible bugfix, no API changes).

### Pre-Release

- [ ] All changes tested locally (`cargo test --workspace`)
- [ ] Stream configs validated: `./tools/ndp-validate/ndp-validate.sh --all`
- [ ] No uncommitted changes: `git status` is clean
- [ ] On `main` branch

### Create Release

- [ ] Version: `v1.1.14`
- [ ] Create manifest: `.deploy/releases/v1.1.14.manifest.json`
- [ ] Manifest contents:
  ```json
  {
    "$schema": "../../schemas/manifest.schema.json",
    "version": "1.0",
    "release_version": "1.1.14",
    "description": "Release v1.1.14: jemalloc allocator + explicit memory management (air-016 Phase 1)",
    "changes": [
      {"type": "container", "target": "air-quality-app", "action": "build"},
      {"type": "container", "target": "air-quality-app", "action": "restart"}
    ]
  }
  ```
- [ ] Verify manifest: `cat .deploy/releases/v1.1.14.manifest.json | jq .`
- [ ] Update `CHANGELOG.md`:
  ```markdown
  ## [1.1.14] - YYYY-MM-DD

  Fix memory growth in air-quality-app: RSS grew from 96 MiB to 490 MiB over days due to glibc malloc fragmentation, risking OOM at the 512 MiB Docker limit (air-016 Phase 1).

  ### Fixed

  - **Memory never returned to OS** -- switched global allocator to jemalloc (`tikv-jemallocator`) which uses `madvise(MADV_DONTNEED)` to release freed pages
  - **Peak memory doubled during Parquet writes** -- added explicit `drop(points)` after column extraction in `write_parquet()` and `write_raw_parquet()` to free input data before DataFrame construction
  - **Fallback for glibc-allocated memory** -- added `malloc_trim(0)` after each append cycle to reclaim any pages allocated through glibc (no-op under jemalloc, safety net for Polars internals)
  ```
- [ ] Commit: `git commit -m "release: v1.1.14 -- jemalloc allocator + memory management (air-016)"`
- [ ] Tag: `git tag -a v1.1.14 -m "Release v1.1.14: jemalloc allocator + explicit memory management (air-016 Phase 1)"`
- [ ] Push: `git push && git push origin v1.1.14`

### Deploy to Pi

- [ ] On Pi: `git pull`
- [ ] Verify tag: `git describe --tags --exact-match` shows `v1.1.14`
- [ ] Deploy: `./deploy.sh apply .deploy/releases/v1.1.14.manifest.json`
- [ ] Verify device state: `cat /var/ndp/deployed-version` shows `v1.1.14`
- [ ] Verify services: `./deploy.sh status`
- [ ] Smoke test: confirm data flow (MQTT ingestion, Parquet writes, Silver ETL)

---

## Monitoring After Deploy

### Immediate (first 2 hours)

```bash
# Watch container RSS in real time
docker stats air-quality-app --no-stream

# Confirm data is being written
ls -lt /path/to/bronze/air-quality/$(date +%Y-%m-%d)/
```

- RSS should stabilize within the first 1-2 hours
- Parquet files should appear at the expected 30s cadence
- No error logs in `docker logs air-quality-app`

### Short-term (24 hours)

```bash
# Record RSS at regular intervals
docker stats air-quality-app --no-stream --format "{{.MemUsage}}"
```

- RSS must stay below 200 MiB
- RSS should not grow linearly (compare hour-1 to hour-12 to hour-24)
- All four MQTT streams should show continuous data

### Baseline Comparison

| Metric | Before (glibc, v1.1.13) | Target (jemalloc, v1.1.14) |
|--------|--------------------------|----------------------------|
| RSS after 1 hour | ~150 MiB (growing) | < 120 MiB (stable) |
| RSS after 12 hours | ~350 MiB (growing) | < 150 MiB (stable) |
| RSS after 24 hours | ~490 MiB (OOM risk) | < 200 MiB (stable) |
| Daily RSS growth | ~20-30 MiB/day | < 5 MiB/day |

---

## Rollback Plan

If Phase 1 causes unexpected issues:

1. Deploy previous version:
   ```bash
   ./deploy.sh apply .deploy/releases/v1.1.13.manifest.json
   ```
2. The rollback removes jemalloc and the `drop()`/`malloc_trim` calls. The app returns to its previous memory behavior (growing RSS, but functional).
3. No data migration needed. No file format changes. No schema changes.

---

## Phase 2 Trigger Criteria

Phase 2 (per-flush sidecar files, architecture in `architecture/ADR-001-sidecar-files.md`) should be implemented if ANY of the following conditions are observed after Phase 1 has been deployed for at least 48 hours:

| Condition | Threshold | Measurement |
|-----------|-----------|-------------|
| Sustained RSS too high | RSS exceeds 250 MiB after 48h | `docker stats` |
| Linear growth continues | Daily RSS growth rate > 10 MiB/day | Compare 24h snapshots |
| OOM kill occurs | Any OOM event on air-quality-app | `dmesg` or Docker events |

If none of these conditions are met after 7 days of observation, Phase 1 is considered sufficient and Phase 2 is deferred indefinitely. The ADR and architecture analysis are preserved for future reference.

---

## What Does NOT Change

Confirmed invariants -- these are unchanged by Phase 1:

- Parquet file naming: `readings.parquet` / `data.parquet`
- Parquet file format: single daily file, same schema
- Directory structure: `bronze/{stream}/{date}/`
- Read path: `query()`, `query_raw()`, `find_partitions()`
- Write path logic: read-modify-write (only memory management around it changes)
- Store trait interface: zero signature changes
- Silver ETL: reads same files in same format
- MCP server: reads same files in same format
- WAL: completely separate path, untouched
- MQTT ingestion: unchanged
- Configuration: no new config fields
