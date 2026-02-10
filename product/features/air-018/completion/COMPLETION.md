# AIR-018 Completion: Eliminate Polars from Bronze Write Path

> **Feature:** air-018
> **SPARC Phase:** Completion (C)
> **Author:** ndp-scrum-master
> **Date:** 2026-02-10
> **Status:** Ready for Implementation
> **Version Target:** v1.1.21

---

## 1. Pre-Implementation Verification

Complete these checks BEFORE starting any code changes.

| # | Check | Status | Notes |
|---|-------|--------|-------|
| 1 | SPARC S/P/A artifacts reviewed and approved | [ ] | SPECIFICATION.md, TEST-STRATEGY.md, PSEUDOCODE.md, ADR-001, DEPENDENCY-ANALYSIS.md |
| 2 | arrow + parquet crate availability confirmed | [ ] | Cargo.lock shows arrow 57.1.0 and parquet 57.1.0 as transitive deps via Polars. Use version "57" in Cargo.toml. |
| 3 | ARM64 cross-compilation tested | [ ] | `cargo check --target aarch64-unknown-linux-gnu -p platform-core` (after adding arrow+parquet deps) |
| 4 | Current test suite baseline captured | [ ] | Run `cargo test --workspace 2>&1 \| tail -5` and record pass count (874+ expected) |
| 5 | Binary size baseline captured | [ ] | `ls -la target/release/air-quality-app` (or ARM64 equivalent) |
| 6 | BUG-004 diagnostic logging present in bronze.rs | [ ] | Verify `malloc_trim(0)` and RSS logging at lines 249+ are intact |

### Crate Version Rationale

The SPECIFICATION states arrow v57 and parquet v57 with `default-features = false`. The ADR and PSEUDOCODE reference v54 (drafted before Cargo.lock was inspected). **Use version 57** -- this is the version already present as a transitive dependency via Polars 0.35 in the current Cargo.lock, ensuring schema compatibility and avoiding duplicate arrow crate compilations.

```toml
# Workspace Cargo.toml [workspace.dependencies]
arrow = { version = "57", default-features = false, features = ["chrono-tz"] }
parquet = { version = "57", default-features = false, features = ["snap"] }
```

---

## 2. Implementation Verification

Complete these checks DURING implementation. Each maps to a TDD cycle from the Pseudocode.

### 2.1 Dependency Changes

| # | Check | Status |
|---|-------|--------|
| 1 | `arrow` and `parquet` added to `[workspace.dependencies]` in root Cargo.toml | [ ] |
| 2 | `arrow = { workspace = true }` added to `core/Cargo.toml` `[dependencies]` | [ ] |
| 3 | `parquet = { workspace = true }` added to `core/Cargo.toml` `[dependencies]` | [ ] |
| 4 | `cargo check -p platform-core` passes with both polars AND arrow/parquet | [ ] |

### 2.2 Error Handling (core/src/error.rs)

| # | Check | Status |
|---|-------|--------|
| 5 | `CoreError::Polars` renamed to `CoreError::Arrow` | [ ] |
| 6 | `From<polars::error::PolarsError>` removed | [ ] |
| 7 | `From<arrow::error::ArrowError>` added | [ ] |
| 8 | `From<parquet::errors::ParquetError>` added | [ ] |
| 9 | No other crate matches on `CoreError::Polars` (verified via grep) | [ ] |

### 2.3 Write Path (P-01, P-02)

| # | Check | Status |
|---|-------|--------|
| 10 | `write_parquet()` rewritten with RecordBatch + ArrowWriter | [ ] |
| 11 | `write_parquet()` passes existing tests (#2, #3) | [ ] |
| 12 | `write_raw_parquet()` rewritten with RecordBatch + ArrowWriter | [ ] |
| 13 | `write_raw_parquet()` passes existing tests (#26, #29, #30, #31) | [ ] |
| 14 | T-NEW-01: 6-column schema metadata verified (names, types, Snappy) | [ ] |
| 15 | T-NEW-02: 5-column schema metadata verified (names, types, Snappy) | [ ] |
| 16 | ArrowWriter `.close()` called in every write path (MANDATORY -- flushes footer) | [ ] |

### 2.4 Read Path (P-03, P-04)

| # | Check | Status |
|---|-------|--------|
| 17 | `append_to_parquet()` rewritten with ParquetRecordBatchReaderBuilder | [ ] |
| 18 | `append_to_parquet()` passes existing tests (#2, #11, #12) | [ ] |
| 19 | `append_to_raw_parquet()` rewritten with ParquetRecordBatchReaderBuilder | [ ] |
| 20 | `append_to_raw_parquet()` passes existing tests (#26, #27, #28) | [ ] |
| 21 | T-NEW-03: Nullable column round-trip (mixed Some/None) verified | [ ] |
| 22 | `#[deprecated]` attribute preserved on `append_to_raw_parquet` | [ ] |

### 2.5 Query Path (P-05, P-06)

| # | Check | Status |
|---|-------|--------|
| 23 | `query()` rewritten with row-level timestamp filter | [ ] |
| 24 | `query()` passes existing tests (#4, #5) | [ ] |
| 25 | `query_raw()` rewritten with ParquetRecordBatchReaderBuilder | [ ] |
| 26 | `query_raw()` passes existing tests (#27, #33) | [ ] |

### 2.6 Test Migration and New Tests

| # | Check | Status |
|---|-------|--------|
| 27 | `test_raw_parquet_schema_has_5_columns` (#25) rewritten with arrow-rs reader | [ ] |
| 28 | T-NEW-04: Empty batch handling (no file created) | [ ] |
| 29 | T-NEW-05: Large batch stress test (10,000 points) | [ ] |
| 30 | T-NEW-06: Cross-read compatibility (manual or dev-dep) | [ ] |

### 2.7 Dependency Removal

| # | Check | Status |
|---|-------|--------|
| 31 | `polars` removed from `core/Cargo.toml` `[dependencies]` | [ ] |
| 32 | No `use polars::` in any file under `core/src/` | [ ] |
| 33 | `cargo build -p platform-core` succeeds without polars | [ ] |
| 34 | Workspace-level polars entry preserved (used by silver-etl, air-quality-app dev-deps) | [ ] |

---

## 3. Post-Implementation Verification

Complete these checks AFTER all code changes and before release.

### 3.1 Test Suite

| # | Check | Command | Status |
|---|-------|---------|--------|
| 1 | Core unit tests | `cargo test -p platform-core -- storage::parquet::tests` | [ ] |
| 2 | Core integration tests | `cargo test -p platform-core -- subscribers::bronze::integration_tests` | [ ] |
| 3 | Bronze unit tests | `cargo test -p platform-core -- subscribers::bronze::tests` | [ ] |
| 4 | Full workspace | `cargo test --workspace` | [ ] |
| 5 | Clippy clean | `cargo clippy -p platform-core -- -D warnings` | [ ] |
| 6 | Pass count >= 874 | Record actual count | [ ] |

### 3.2 Schema Compatibility

| # | Check | Status |
|---|-------|--------|
| 7 | TimeSeriesPoint Parquet: 6 columns, correct names, correct types, Snappy | [ ] |
| 8 | RawDataPoint Parquet: 5 columns, correct names, correct types, Snappy | [ ] |
| 9 | Nullable columns: ndp_id and context null bitmaps correct | [ ] |
| 10 | Column order matches: timestamp, location_id/source_id, metric/ndp_id, ... | [ ] |

### 3.3 Binary Size

| Metric | Before (Polars) | After (arrow-rs) | Delta |
|--------|----------------|-------------------|-------|
| `air-quality-app` binary | _____ MB | _____ MB | _____ MB |
| Core crate compile time | _____ s | _____ s | _____ s |
| Crates compiled for core | _____ | _____ | _____ |

Expected: 15-20 MB binary size reduction (Polars pulls ~40-50 transitive crates).

### 3.4 Changelog Entry (Draft)

```markdown
## [1.1.21] - 2026-XX-XX

Eliminate Polars dependency from Bronze storage layer (air-018, BUG-004 fix). Replace all Polars DataFrame usage in `core/src/storage/parquet.rs` with direct arrow-rs RecordBatch + parquet ArrowWriter. Fixes memory leak that caused OOM within 24-36 hours on Raspberry Pi 5.

### Changed

- **core/src/storage/parquet.rs** -- All write methods (`write_parquet`, `write_raw_parquet`) now use `arrow::record_batch::RecordBatch` + `parquet::arrow::ArrowWriter` instead of Polars `DataFrame` + `ParquetWriter`
- **core/src/storage/parquet.rs** -- All read methods (`append_to_parquet`, `append_to_raw_parquet`, `query`, `query_raw`) now use `parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder` instead of Polars `ParquetReader`
- **core/src/error.rs** -- `CoreError::Polars` renamed to `CoreError::Arrow`; `From<PolarsError>` replaced with `From<ArrowError>` and `From<ParquetError>`
- **core/Cargo.toml** -- `polars` removed from `[dependencies]`, `arrow` v57 and `parquet` v57 added

### Fixed

- **BUG-004: Bronze memory leak** -- Polars DataFrame create/drop cycle leaked ~4.5 MiB per 30-min snapshot cycle due to glibc malloc fragmentation. Direct RecordBatch construction uses simpler allocation patterns that do not accumulate residual heap pages.

### Added

- 5-6 new tests: schema metadata verification, nullable column round-trip, empty batch handling, large batch stress test
- `read_nullable_string()` and `read_nullable_json()` helper functions in parquet.rs

### Technical Notes

- Parquet file schema is identical (column names, types, nullability, Snappy compression)
- Trait signatures (`Store`, `RawStore`) unchanged -- zero downstream API impact
- `malloc_trim(0)` and BUG-004 diagnostic logging in bronze.rs preserved for production verification
- ARM64 binary size reduction: ~15-20 MB (Polars transitive deps eliminated from production binary)
- 874+ tests passing (all existing tests unchanged except test #25 rewritten)
```

---

## 4. Release Procedure

Follow `docs/procedures/RELEASE-POLICY.md`. This is a **PATCH** version bump: bug fix (BUG-004 OOM) with identical Parquet schema output.

### 4.1 Version

**Proposed version:** v1.1.21 (next patch after v1.1.20)

**Rationale:** PATCH bump -- this is fundamentally a bug fix (BUG-004 memory leak causing OOM). While the internal implementation changes significantly (Polars → arrow-rs), the external behavior is identical: same Parquet schema, same trait signatures, same API. No new user-facing features are added.

### 4.2 Release Artifacts

Three artifacts required:

#### Artifact 1: Release Manifest

**File:** `.deploy/releases/v1.1.21.manifest.json`

```json
{
  "$schema": "../../schemas/manifest.schema.json",
  "version": "1.0",
  "release_version": "1.1.21",
  "description": "Release v1.1.21: Eliminate Polars from Bronze storage, fix BUG-004 memory leak",
  "changes": [
    {
      "type": "container",
      "target": "air-quality-app",
      "action": "build",
      "no_cache": true
    },
    {
      "type": "container",
      "target": "air-quality-app",
      "action": "restart"
    }
  ]
}
```

**Notes:**
- `no_cache: true` is CRITICAL -- cargo incremental cache can ship stale binary after dependency changes (pattern ID 21).
- No stream, silver-table, migration, or dictionary changes -- this is a code-only release.
- Only the `air-quality-app` container needs rebuild and restart.

#### Artifact 2: Git Tag

```bash
git tag -a v1.1.21 -m "Release v1.1.21: Eliminate Polars from Bronze storage (air-018, BUG-004 fix)"
```

#### Artifact 3: Changelog Entry

Add the draft from Section 3.4 to `CHANGELOG.md` under `## [Unreleased]`, then move to `## [1.1.21] - {date}` at release time.

### 4.3 Commit Message Format

```
feat: replace Polars with arrow-rs in Bronze Parquet I/O (air-018)

Eliminate Polars DataFrame usage from core/src/storage/parquet.rs.
All write and read methods now use arrow RecordBatch + parquet ArrowWriter
directly. Fixes BUG-004 memory leak (~4.5 MiB/cycle on Pi 5).

- core/Cargo.toml: Remove polars, add arrow v57 + parquet v57
- core/src/error.rs: CoreError::Polars -> CoreError::Arrow
- core/src/storage/parquet.rs: Full rewrite of 6 methods
- 5-6 new tests, 1 test rewritten, 38 tests unchanged
- Parquet schema unchanged (zero downstream impact)
```

---

## 5. Deployment Procedure

### 5.1 Pre-Deploy (Pattern ID 22: Pre-Deploy Baseline Sampling)

On the Pi, BEFORE deploying:

```bash
# Capture current data baseline
ndp sample --all

# Record current container RSS
docker stats --no-stream air-quality-app

# Record current version
cat /var/ndp/deployed-version
# Expected: v1.1.20
```

### 5.2 Build (Pattern ID 21: Docker Cache Verification)

On the Pi:

```bash
# Pull latest code
cd /path/to/neural-data-platform
git pull

# Verify tag
git describe --tags --exact-match
# Expected: v1.1.21

# Build with --no-cache (CRITICAL after dependency changes)
docker compose build --no-cache air-quality-app
```

**Why `--no-cache`:** Pattern ID 21 documents that cargo incremental cache inside Docker can ship a stale binary when dependencies change. Since air-018 removes polars and adds arrow+parquet, the entire dependency tree changes. `--no-cache` forces a clean build.

### 5.3 Deploy

```bash
./deploy/pi/deploy.sh apply .deploy/releases/v1.1.21.manifest.json
```

Or manually:

```bash
docker compose down air-quality-app
docker compose up -d air-quality-app
```

### 5.4 Verify

| Time | Check | Expected | Command |
|------|-------|----------|---------|
| T+0 | Container starts | Running, no crash | `docker ps` |
| T+0 | Version deployed | v1.1.21 | `cat /var/ndp/deployed-version` |
| T+1m | MQTT data flowing | Messages received | `docker logs air-quality-app \| grep "mqtt"` |
| T+5m | First heartbeat RSS | < 140 MiB | `docker stats --no-stream air-quality-app` |
| T+30m | First snapshot cycle | RSS spike + recovery | Check bronze.rs diagnostic logs |
| T+30m | polars_delta | Near zero (was +4.5 MiB) | `docker logs air-quality-app \| grep polars_delta` |
| T+30m | net_delta | Near zero (was +4.5 MiB) | `docker logs air-quality-app \| grep net_delta` |
| T+2h | RSS stability | No unbounded growth | `docker stats --no-stream air-quality-app` |
| T+24h | Container alive | Still running, RSS stable | `docker ps` |
| T+24h | Silver ETL | Reads Bronze Parquet without error | `docker logs silver-etl \| grep error` |

### 5.5 Post-Deploy (Pattern ID 22: Post-Deploy Verification)

```bash
# Verify data flow unchanged
ndp sample --all

# Compare with pre-deploy baseline
# Same streams, same data schema, same Silver tables
```

### 5.6 BUG-004 Specific Verification

The diagnostic logging added in v1.1.19 remains in `bronze.rs`. After deployment, look for these log entries during a snapshot cycle:

```
# BEFORE air-018 (v1.1.20):
rss_before_mib=XX, rss_after_writes_mib=XX+46, rss_after_trim_mib=XX+4.5, polars_delta_mib=46, trim_reclaimed_mib=41.5, net_delta_mib=4.5

# AFTER air-018 (v1.1.21) -- EXPECTED:
rss_before_mib=XX, rss_after_writes_mib=XX+small, rss_after_trim_mib=XX+~0, polars_delta_mib=small, trim_reclaimed_mib=small, net_delta_mib=~0
```

The `polars_delta_mib` field name in the log is slightly misleading after air-018 (no Polars involved), but it measures the same thing: RSS delta during Parquet writes. It should drop from ~46 MiB to a much smaller value since RecordBatch construction uses less heap than DataFrame construction.

**Success criteria:** After 24+ hours, the container is still running and RSS has not grown unboundedly. This is the definitive BUG-004 fix validation.

---

## 6. Rollback Plan

If issues are detected after deployment:

### 6.1 Immediate Rollback

```bash
# On Pi
cd /path/to/neural-data-platform
git checkout v1.1.20

# MUST use --no-cache (pattern ID 21)
docker compose build --no-cache air-quality-app
docker compose down air-quality-app
docker compose up -d air-quality-app

# Verify rollback
cat /var/ndp/deployed-version
# Expected: v1.1.20
```

### 6.2 Data Compatibility

Parquet files written by v1.1.21 (arrow-rs) are standard Apache Parquet files with identical schema to v1.1.20 (Polars). Rolling back to v1.1.20 will read these files without issue because:

- Column names, types, and nullability are identical
- Compression is Snappy in both versions
- Polars' `ParquetReader` reads standard Apache Parquet files

No data migration is needed in either direction.

### 6.3 Bug Reporting

If rollback is triggered, file a bug report:

**File:** `product/features/air-018/bugs/BUG-001-{slug}.md`

Use the standard bug template from the scrum-master playbook. Include:
- Docker logs from the failed deployment
- RSS metrics from diagnostic logging
- Exact failure mode (crash, data corruption, performance regression, etc.)

---

## 7. Patterns Applied

| Pattern ID | Name | How Applied |
|------------|------|-------------|
| 3 | `deployment:deploy-sh-ndp-dispatch` | Manifest uses container build+restart declarations |
| 21 | `deployment:docker-cache-verification` | `--no-cache` required for dependency change deployment |
| 22 | `procedure:pre-deploy-baseline-sampling` | Pre/post deploy `ndp sample` comparison |
| 29 | `conventions:feature-dir-structure` | SPARC directory structure followed for air-018 |

---

## 8. Open Questions

| # | Question | Status | Resolution |
|---|----------|--------|------------|
| 1 | Arrow version: v54 (ADR/Pseudocode) vs v57 (Specification/Cargo.lock)? | Resolved | Use v57 -- matches actual transitive dep in Cargo.lock |
| 2 | Keep polars in core `[dev-dependencies]` for cross-read tests? | Open | Decision deferred to implementor. T-NEW-06 can be manual or use a temporary dev-dep. |
| 3 | Rename `polars_delta_mib` log field in bronze.rs? | Deferred | Out of scope. Field name is cosmetic; behavior is what matters. |
| 4 | Remove `malloc_trim(0)` after BUG-004 confirmed fixed? | Deferred | Separate cleanup after 1+ week of stable production. |
