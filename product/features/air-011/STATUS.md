# AIR-011 Status

## Current Phase: COMPLETION

## Status: IMPLEMENTED

## Summary
Eliminate duplicative parser processing that causes Pi lockups. Parsers are being invoked during ingestion but their output is never used - Bronze layer stores raw JSON responses.

## Root Cause Analysis
- `source.start()` spawns background `polling_loop()` that parses responses
- `source_manager` has separate loop calling `fetch_raw_batch()` (no parsing)
- Both loops poll the same endpoints
- Parser results accumulate in unbounded channels, never consumed
- Memory pressure causes lockup after hours of operation

## SPARC Progress

| Phase | Status | Artifacts |
|-------|--------|-----------|
| Specification | Complete | REQUIREMENTS.md |
| Pseudocode | Complete | ALGORITHM.md |
| Architecture | Complete | ADR-001-parser-archive.md, SYSTEM_DESIGN.md |
| Refinement | Complete | IMPLEMENTATION.md, TEST_PLAN.md |
| Completion | Pending | VERIFICATION.md |

## Key Decisions Made

### Pseudocode Phase
1. **Approach**: Option A - Remove `source.start()` calls (minimal risk, 10 lines changed)
2. **Implementation**: Modify `run_http_polling_source` and `run_generic_http_polling_source`

### Architecture Phase (ADR-001)
1. **Parser Archive Strategy**: Option C - In-Place Decoupling
   - Keep parsers in `core/src/parsers/` (no file moves)
   - Feature-gate with `#[cfg(feature = "etl")]` for future Silver ETL
   - Zero breaking changes to public API
2. **Source Modification**: Modify existing sources vs creating new ones
   - Remove parser from constructors
   - Remove `start()` / `polling_loop()`
   - Retain `fetch_raw_batch()` as primary interface

## Architecture Artifacts

| Artifact | Description |
|----------|-------------|
| `architecture/ADR-001-parser-archive.md` | Parser archive strategy decision with options analysis |
| `architecture/SYSTEM_DESIGN.md` | Detailed system design with component diagrams, data flows, migration plan |

## Algorithm Summary (from ALGORITHM.md)
- Remove `source.start()` call - eliminates background polling_loop that parses JSON
- Remove `source.stop()` call - nothing to stop anymore
- Use `config.poll_interval` instead of hardcoded 1 second interval
- Continue using `fetch_raw_batch()` which returns raw JSON without parsing

## Refinement Artifacts

| Artifact | Description |
|----------|-------------|
| `refinement/IMPLEMENTATION.md` | Detailed TDD implementation plan with 5 phases |
| `refinement/TEST_PLAN.md` | Comprehensive test strategy including Pi stability tests |

### Implementation Phases (from IMPLEMENTATION.md)
1. **Phase 1**: Remove `source.start()` calls in SourceManager (lowest risk)
2. **Phase 2**: Remove parser dependency from source constructors
3. **Phase 3**: Remove parser creation from SourceManager
4. **Phase 4**: Archive parsers behind feature flag
5. **Phase 5**: Memory optimization - remove unused channels

### Test Coverage (from TEST_PLAN.md)
- Unit tests for raw-only source creation
- Integration tests for Bronze layer ingestion
- Memory stability tests (Pi hardware)
- 24-hour stability test protocol with pass/fail criteria

## Implementation Summary

### Phase 1 Changes Applied (2026-01-01)

**File Modified**: `apps/air-quality-app/src/coordinator/source_manager.rs`

#### run_http_polling_source (lines 401-483)
- ✅ Removed `source.start()` call
- ✅ Removed `source.stop()` call
- ✅ Changed from hardcoded 1s to `config.poll_interval`
- ✅ Added AIR-011 documentation comments

#### run_generic_http_polling_source (lines 817-885)
- ✅ Removed `source.start()` call
- ✅ Removed `source.stop()` call
- ✅ Changed from hardcoded 1s to `config.poll_interval`
- ✅ Added AIR-011 documentation comments
- ✅ Removed `mut` from source variable

### Build Status
- ✅ `cargo build -p air-quality-app` - SUCCESS (no errors, no warnings)
- ✅ `cargo test -p air-quality-app --lib` - **124 tests passed**
- ✅ `cargo test -p platform-core --lib` - **407 tests passed**
- ✅ `cargo clippy` - No new warnings from AIR-011 changes

### Architecture Patterns Saved
1. **Double-Polling Prevention** - Never call source.start() with fetch_raw_batch()
2. **Raw-Only Source Pattern** - Bronze layer uses only RawSource interface
3. **Poll Interval Configuration** - Use config.poll_interval, not hardcoded values

## Next Steps
1. ✅ Deploy changes to Pi
2. Run 4-hour stability test
3. Verify memory stability
4. If stable, proceed to 24-hour test

## Future Work (Out of Scope for AIR-011)
- Phase 2 (AIR-012): Remove parser dependency from source constructors
- Phase 3 (AIR-013): Archive parsers behind feature flag
- Phase 4 (AIR-014): Memory optimization - remove unused channels

## Blocked By
None

## Last Updated
2026-01-01
