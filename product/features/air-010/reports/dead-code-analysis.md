# Dead Code Analysis Report

**Project:** Neural Data Platform
**Analyzed Paths:**
- `/workspaces/neural-data-platform/core/src/`
- `/workspaces/neural-data-platform/apps/air-quality-app/src/`

**Analysis Date:** 2026-01-01
**Total Lines Analyzed:** ~42,000 LOC

---

## Executive Summary

| Category | Count | Est. LOC Removable | Priority |
|----------|-------|-------------------|----------|
| Commented-out Modules | 1 | ~880 | High |
| Unused Struct Fields | 2 | ~10 | Medium |
| Unused Functions/Methods | 3 | ~50 | Medium |
| Unused Imports | 2 | ~5 | Low |
| Unused Variables | 4 | ~10 | Low |
| Duplicate/Redundant Code | 2 | ~30 | Medium |
| **Total** | **14** | **~985** | |

**Estimated Reduction Potential:** ~985 LOC (2.3% of total)

---

## 1. Commented-out Modules (HIGH PRIORITY)

### 1.1 Forecast Module - Commented Out

**Location:** `/workspaces/neural-data-platform/core/src/lib.rs`
```rust
// Line 3: pub mod forecast;
// Line 12: pub use forecast::{FannForecaster, ModelType};
```

**Files Affected:**
- `core/src/forecast/mod.rs` (9 lines)
- `core/src/forecast/fann_adapter.rs` (680 lines)
- `core/src/forecast/features.rs` (359 lines)
- `core/src/forecast/scaler.rs` (146 lines)

**Total:** ~1,194 LOC in the forecast module

**Confidence:** HIGH

**Analysis:**
- The entire `forecast` module is commented out in `lib.rs`
- Module files still exist but are not compiled into the crate
- Contains mock implementations and incomplete ruv-FANN integration
- Tests in `fann_adapter.rs` use outdated `TimeSeriesPoint` structure (has `source` and `metric` fields that don't exist in current struct)

**Recommendation:**
1. **SHORT TERM:** Keep commented out (current state) - module is incomplete
2. **LONG TERM:** Either complete the implementation or delete the files
3. If deleting: **~880 LOC** reduction (excluding tests that might be preserved)

---

## 2. Unused Struct Fields (MEDIUM PRIORITY)

### 2.1 SourceInfo.source_id Never Read

**Location:** `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs`
```rust
// Lines 49-50
struct SourceInfo {
    source_id: String,  // WARNING: field is never read
    ...
}
```

**Confidence:** HIGH (Compiler warning)

**Analysis:**
- The `source_id` field is set but never accessed
- Internal tracking uses HashMap key instead
- Debug derive is present but field is explicitly noted as unused

**Recommendation:** REMOVE or prefix with underscore (`_source_id`)
- **LOC Reduction:** ~5 lines (field + 3 assignment sites)

### 2.2 RedisConfigStore Fields Never Read

**Location:** `/workspaces/neural-data-platform/config-store/src/stores/redis.rs`
```rust
// Lines 17-20
pub struct RedisConfigStore {
    environment: String,
    redis_url: String,     // WARNING: never read
    client: Client,        // WARNING: never read
}
```

**Confidence:** HIGH (Compiler warning)

**Analysis:**
- Part of config-store package (outside primary scope)
- Fields stored but never accessed after initialization
- RedisConfigStore appears to be an incomplete/stub implementation

**Recommendation:** REFACTOR - either complete implementation or mark fields with `_` prefix
- **LOC Reduction:** ~5 lines

---

## 3. Unused Functions/Methods (MEDIUM PRIORITY)

### 3.1 check_rate_limit Method

**Location:** `/workspaces/neural-data-platform/config-store/src/stores/secure_in_memory.rs`
```rust
// Line 80
fn check_rate_limit(&self, client_id: Option<&str>) -> Result<(), ConfigError>
```

**Confidence:** HIGH (Compiler warning)

**Analysis:**
- Private method that is never called
- Part of SecureInMemoryConfigStore implementation
- Rate limiting feature not integrated

**Recommendation:** REMOVE if rate limiting not planned, otherwise integrate
- **LOC Reduction:** ~25 lines

### 3.2 GenericHttpPollingSource::with_default_parsers (Deprecated)

**Location:** `/workspaces/neural-data-platform/core/src/sources/http_poll.rs`
```rust
// Lines 877-894
#[deprecated(since = "0.1.0", note = "Use new() with parser injection instead")]
pub fn with_default_parsers(config: GenericHttpPollingConfig) -> CoreResult<Self>
```

**Confidence:** MEDIUM

**Analysis:**
- Explicitly marked as deprecated
- Should be using `new()` with parser injection
- Still compiled but generates warning if used

**Recommendation:** REMOVE after confirming no external callers
- **LOC Reduction:** ~18 lines

### 3.3 Forecast Trait predict() Signature Mismatch

**Location:** `/workspaces/neural-data-platform/core/src/forecast/fann_adapter.rs` (commented out)
```rust
async fn predict(
    &self,
    _source: &str,        // Parameter unused
    _metric: &str,        // Parameter unused
    horizon: usize,
) -> CoreResult<Vec<ForecastedPoint>>
```

**Confidence:** MEDIUM

**Analysis:**
- Trait method signature doesn't match current Forecast trait in traits.rs
- Current trait: `predict(&self, location_id: &str, horizon: usize)`
- Implementation has extra unused parameters
- Part of commented-out module

**Recommendation:** FIX if module is reactivated
- **LOC Reduction:** 0 (already commented out)

---

## 4. Unused Imports (LOW PRIORITY)

### 4.1 StreamConfigError Import

**Location:** `/workspaces/neural-data-platform/config-client/src/stream/registry.rs`
```rust
// Line 2
use neural_core::{StreamConfig, StreamConfigError};
                                 ^^^^^^^^^^^^^^^^^ unused
```

**Confidence:** HIGH (Compiler warning)

**Recommendation:** REMOVE unused import
- **LOC Reduction:** 1 line (or remove from multi-import)

### 4.2 FieldType Import

**Location:** `/workspaces/neural-data-platform/apps/air-quality-app/src/config_sync/service.rs`
```rust
// Line 349
use neural_core::{FieldType, SchemaField, SourceConfig, SourceType, StorageConfig};
                  ^^^^^^^^^ unused
```

**Confidence:** HIGH (Compiler warning)

**Recommendation:** REMOVE unused import
- **LOC Reduction:** 1 line (or remove from multi-import)

---

## 5. Unused Variables (LOW PRIORITY)

### 5.1 source_id Parameter - Multiple Locations

**Location:** `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs`
```rust
// Line 392
source_id: String,  // unused in function body

// Line 726
source_id: String,  // unused in function body

// Line 806
source_id: String,  // unused in function body
```

**Confidence:** HIGH (Compiler warning)

**Analysis:**
- Parameter passed but never used in function body
- Functions: `run_http_polling_source`, `run_mqtt_source`, `run_generic_http_polling_source`
- Likely intended for logging/tracing but not implemented

**Recommendation:** Either use the parameter or prefix with `_`
- **LOC Reduction:** 0 (just prefix fix)

---

## 6. Duplicate/Redundant Code (MEDIUM PRIORITY)

### 6.1 Default Implementations Can Be Derived

**Location:** `/workspaces/neural-data-platform/core/src/sources/http_poll.rs`
```rust
// Lines 40-44
impl Default for AuthMethod {
    fn default() -> Self {
        AuthMethod::None
    }
}
```

**Confidence:** HIGH (Clippy warning)

**Recommendation:** Replace with `#[derive(Default)]` and `#[default]` variant attribute
- **LOC Reduction:** ~3 lines

**Location:** `/workspaces/neural-data-platform/config-store/src/configs/feature_flags.rs`
```rust
// Lines 20-27
impl Default for FeatureFlags { ... }
```

**Recommendation:** Replace with `#[derive(Default)]`
- **LOC Reduction:** ~6 lines

### 6.2 Vec Push Patterns

**Locations:**
- `core/src/sources/parsers/air_pollution.rs` (lines 103-184)
- `core/src/sources/parsers/weather.rs` (lines 108-159)

**Analysis:**
- Multiple `push` calls immediately after `Vec::new()`
- Could be simplified using `vec![]` macro

**Recommendation:** Refactor to `vec![...]` initialization
- **LOC Reduction:** ~20 lines (consolidation, not removal)

---

## 7. Dead Code Paths (LOW PRIORITY)

### 7.1 MockSource/MockForecast in main.rs

**Location:** `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`
```rust
// Lines 348-399
struct MockSource;
impl Source for MockSource { ... }

struct MockForecast;
impl Forecast for MockForecast { ... }
```

**Confidence:** LOW

**Analysis:**
- Mock implementations used for health endpoint placeholder
- Comment indicates "to be replaced in future tasks"
- Currently in use but represents technical debt

**Recommendation:** Track as technical debt; replace when real implementations ready
- **LOC Reduction:** ~50 lines (when replaced)

---

## 8. Feature Flag Conditional Compilation

### 8.1 MCP Feature Module

**Location:** `/workspaces/neural-data-platform/apps/air-quality-app/src/lib.rs`
```rust
// Lines 12-13
#[cfg(feature = "mcp")]
pub mod mcp;
```

**Confidence:** LOW

**Analysis:**
- MCP module (~987 lines total) conditionally compiled
- If `mcp` feature not enabled, code is not compiled
- Not dead code per se, but feature-gated

**Recommendation:** Verify MCP feature is used in production builds
- **Potential LOC Reduction:** ~987 lines if feature never enabled

---

## 9. Code Quality Issues (INFORMATIONAL)

### 9.1 Parameter Only Used in Recursion

**Locations:**
- `core/src/storage/parquet.rs:638` - `&self` parameter in `collect_partition_files`
- `apps/air-quality-app/src/config_sync/service.rs:168` - `&'a self` in recursive function

**Analysis:**
- Self parameter could be passed as function parameter instead of method
- Style issue, not dead code

### 9.2 Too Many Arguments

**Location:** `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs:804`
```rust
async fn run_generic_http_polling_source(
    stream_id: String,
    source_id: String,
    config: GenericHttpPollingConfig,
    parser_config: ParserConfig,
    ingestion_sender: mpsc::Sender<RawDataPoint>,
    cancel_token: CancellationToken,
    ndp_id: Option<String>,
    context: Option<serde_json::Value>,
) -> Result<(), SourceManagerError>
```

**Analysis:**
- 8 parameters exceeds Clippy's default of 7
- Consider grouping into struct

---

## Summary of Actionable Items

### Immediate Actions (Low Risk)
1. Remove unused imports (`StreamConfigError`, `FieldType`)
2. Prefix unused `source_id` parameters with `_`
3. Apply `#[derive(Default)]` where applicable

### Short-term Actions (Medium Risk)
4. Prefix or remove unused `source_id` field in `SourceInfo`
5. Remove deprecated `with_default_parsers` method
6. Refactor `Vec::new()` + multiple `push` to `vec![]`

### Long-term Actions (Requires Planning)
7. Either complete forecast module or delete it entirely (~880 LOC)
8. Replace MockSource/MockForecast with real implementations
9. Complete or remove Redis config store stub
10. Decide on MCP feature usage

---

## Appendix: File Line Counts

### Core Library (`core/src/`)
| File | Lines | Notes |
|------|-------|-------|
| lib.rs | 32 | |
| traits.rs | 1,571 | Includes extensive tests |
| error.rs | 43 | |
| types/mod.rs | 23 | |
| types/air_quality.rs | 124 | |
| types/raw_data_point.rs | 305 | |
| types/stream_config.rs | 756 | |
| types/stream_record.rs | 328 | |
| sources/mod.rs | 168 | |
| sources/http_poll.rs | ~1,500+ | Large file |
| sources/merge.rs | ~200 | |
| sources/mqtt/mod.rs | 1,278 | |
| sources/mqtt/router.rs | 472 | |
| sources/mqtt/subscription.rs | 476 | |
| sources/parsers/* | ~655 | |
| parsers/* | ~1,000+ | |
| storage/mod.rs | 6 | |
| storage/parquet.rs | ~800+ | |
| storage/wal.rs | ~300+ | |
| coordinator/* | ~500+ | |
| **forecast/* (commented out)** | **~1,194** | **NOT COMPILED** |
| **Total (compiled)** | **~17,000** | |

### Air Quality App (`apps/air-quality-app/src/`)
| File | Lines | Notes |
|------|-------|-------|
| main.rs | 409 | |
| lib.rs | 19 | |
| config.rs | ~200 | |
| config_etcd.rs | ~100 | |
| error.rs | ~80 | |
| response.rs | ~50 | |
| stream_integration.rs | ~150 | |
| coordinator/* | ~3,000+ | |
| pipeline/* | ~680 | |
| ingestion/* | ~200 | |
| api/* | ~1,300+ | |
| config_sync/* | ~1,350 | |
| mcp/* (feature-gated) | ~987 | |
| **Total** | **~8,500** | |

---

*Report generated by ndp-rust-dev agent*
