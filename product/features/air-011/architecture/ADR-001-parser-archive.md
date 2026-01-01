# ADR-001: Parser Archive Strategy for AIR-011

**Status**: Proposed
**Date**: 2026-01-01
**Decision Makers**: NDP Architecture Team
**Supersedes**: None

---

## Context

### Problem Statement

The current ingestion pipeline executes parsers during HTTP polling, but their output is never consumed. The Bronze layer stores raw JSON responses directly via `RawSource::fetch_raw_batch()`, making the parser-produced `TimeSeriesPoint` data redundant.

This creates:
1. **Wasted CPU cycles** - Parsers process ~100KB JSON responses into 1000+ TimeSeriesPoints per poll
2. **Memory pressure** - Parsed results accumulate in channels that are never drained
3. **Pi lockups** - After hours of operation, memory exhaustion causes system instability

### Parser Inventory

The following parsers exist in `core/src/parsers/`:

| Parser | Purpose | Future Use |
|--------|---------|------------|
| `flat_json.rs` | Flat key-value JSON extraction | Silver ETL |
| `json_path.rs` | JSONPath-based field extraction | Silver ETL |
| `array_iterator.rs` | Array-based data with metadata | Silver ETL |
| `column_oriented.rs` | Column-oriented data formats | Silver ETL |
| `factory.rs` | Parser instantiation from config | Silver ETL |
| `config.rs` | Parser configuration types | Silver ETL |
| `traits.rs` | Parser trait definitions | Silver ETL |

### Constraints

1. Parsers MUST remain accessible for future Silver layer ETL (DP-00x features)
2. No breaking changes to public `neural-core` API
3. Minimize code churn to reduce risk
4. Maintain testability of parser logic

---

## Decision

**We will keep parsers in their current location (`core/src/parsers/`) but decouple them from the ingestion path.**

### Chosen Option: C - In-Place Decoupling

Keep parsers in `core/src/parsers/` but:
1. Remove parser instantiation from source constructors
2. Remove parser invocation from `Source::fetch()` implementations
3. Add `#[cfg(feature = "etl")]` feature gate for Silver layer use
4. Document parsers as "Reserved for Silver ETL" in module docs

### Implementation Approach

```
BEFORE (current):
  HttpPollingSource::new(config, parser) -> Source
  source.start() -> spawns polling_loop() -> calls parser.parse()
  source.fetch() -> returns TimeSeriesPoints (from parser)
  source.fetch_raw_batch() -> returns RawDataPoint (raw JSON)

AFTER (AIR-011):
  HttpPollingSource::new(config) -> RawSource  (no parser)
  source.start() -> no longer spawns polling_loop
  source.fetch_raw_batch() -> returns RawDataPoint (raw JSON)
  Source trait deprecated for raw-only sources
```

---

## Options Considered

### Option A: Archive to `core/archive/parsers/`

**Description**: Move all parser files to a new `core/archive/` directory.

**Pros**:
- Clear visual separation of archived code
- Easy to find "inactive" code

**Cons**:
- Breaks existing imports and tests
- Requires module restructuring
- Git history becomes harder to follow
- Risk of breakage during migration

**Verdict**: Rejected - Too much churn for marginal benefit

### Option B: Create `domains/etl/parsers/`

**Description**: Move parsers to a new `domains/etl/` crate for future ETL use.

**Pros**:
- Clean domain separation
- Dedicated ETL module for Silver layer
- Independent versioning possible

**Cons**:
- Premature abstraction (Silver layer not yet designed)
- Creates additional crate complexity
- Parsers currently depend on `neural-core` types
- Would require significant refactoring

**Verdict**: Rejected - Premature; revisit when Silver ETL is designed (DP-00x)

### Option C: In-Place Decoupling (SELECTED)

**Description**: Keep parsers where they are, but remove them from the ingestion code path.

**Pros**:
- Zero file moves - minimal git churn
- Parsers remain testable and maintained
- Public API unchanged (just deprecated paths)
- Easy to re-enable for Silver ETL
- No new crates or dependencies

**Cons**:
- Parsers "look active" but aren't called
- Requires clear documentation
- Feature flags add conditional compilation

**Verdict**: Selected - Best balance of simplicity and future-proofing

---

## Consequences

### Positive

1. **Immediate stability improvement** - No parser CPU/memory usage during ingestion
2. **Zero breaking changes** - `neural-core` public API remains stable
3. **Preserved investment** - Parser code remains available for Silver ETL
4. **Minimal risk** - No file moves, no new dependencies
5. **Clear upgrade path** - Feature gate enables Silver ETL when ready

### Negative

1. **Documentation burden** - Must clearly document parser status
2. **Potential confusion** - New developers may wonder why parsers exist but aren't used
3. **Dead code perception** - Parsers may appear unused in code coverage

### Neutral

1. **Feature flag complexity** - `#[cfg(feature = "etl")]` adds conditional compilation
2. **Module structure unchanged** - Neither better nor worse than before

---

## Implementation Notes

### Phase 1: Remove Parser from Source Constructors

```rust
// BEFORE: HttpPollingSource requires parser
pub fn new(config: HttpPollingConfig, parser: Box<dyn Parser>) -> CoreResult<Self>

// AFTER: HttpPollingSource is raw-only
pub fn new(config: HttpPollingConfig) -> CoreResult<Self>
```

### Phase 2: Deprecate Source::fetch() for Raw Sources

```rust
// Add deprecation warning
#[deprecated(since = "0.5.0", note = "Use fetch_raw_batch() for Bronze layer ingestion")]
async fn fetch(&self) -> CoreResult<Vec<TimeSeriesPoint>>;
```

### Phase 3: Add ETL Feature Gate

```toml
# core/Cargo.toml
[features]
default = []
etl = []  # Enables Silver layer parser functionality
```

```rust
// core/src/lib.rs
#[cfg(feature = "etl")]
pub mod parsers;

// When etl feature not enabled, parsers module is not compiled
// This prevents accidental use in ingestion path
```

### Phase 4: Update SourceManager

```rust
// BEFORE: SourceManager creates parser from config
fn create_parser_from_params(&self, params: &HashMap<String, Value>) -> CoreResult<Box<dyn Parser>>

// AFTER: Remove parser creation entirely
// SourceManager spawns raw-only sources
```

---

## Verification Criteria

1. `cargo test` passes without parser-related test failures
2. `cargo build` succeeds without `etl` feature
3. `cargo build --features etl` compiles parsers
4. Pi deployment runs 24+ hours without memory accumulation
5. No parser code executed during HTTP polling (verified via tracing)

---

## Related Documents

- [AIR-011 SCOPE.md](../SCOPE.md) - Feature scope and success criteria
- [AIR-011 SYSTEM_DESIGN.md](./SYSTEM_DESIGN.md) - Detailed system design
- [PLATFORM_ARCHITECTURE_OVERVIEW.md](/workspaces/neural-data-platform/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md) - Platform context
- [AIR-005_INGESTION_COORDINATOR_DESIGN.md](/workspaces/neural-data-platform/docs/architecture/AIR-005_INGESTION_COORDINATOR_DESIGN.md) - Coordinator architecture

---

## Decision Record

| Date | Author | Action |
|------|--------|--------|
| 2026-01-01 | NDP Architecture Agent | Initial proposal |
