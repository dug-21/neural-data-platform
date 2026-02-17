# dp-023: Implementation Brief -- Text Field Pipeline (Bronze through Gold)

## SPARC Artifacts

| Artifact | Path |
|----------|------|
| Scope | product/features/dp-023/SCOPE.md |
| Specification | product/features/dp-023/specification/SPECIFICATION.md |
| Task Decomposition | product/features/dp-023/specification/TASK-DECOMPOSITION.md |
| Architecture (ADRs) | product/features/dp-023/architecture/ARCHITECTURE.md |
| Pseudocode Overview | product/features/dp-023/pseudocode/OVERVIEW.md |
| Pseudocode: platform-core | product/features/dp-023/pseudocode/platform-core.md |
| Pseudocode: ndp-lib | product/features/dp-023/pseudocode/ndp-lib.md |
| Pseudocode: deploy-sh | product/features/dp-023/pseudocode/deploy-sh.md |
| Pseudocode: ndp-validate | product/features/dp-023/pseudocode/ndp-validate.md |
| Pseudocode: config | product/features/dp-023/pseudocode/config.md |
| Test Plan Overview | product/features/dp-023/test-plan/OVERVIEW.md |
| Test Plan: platform-core | product/features/dp-023/test-plan/platform-core.md |
| Test Plan: ndp-lib | product/features/dp-023/test-plan/ndp-lib.md |
| Test Plan: deploy-sh | product/features/dp-023/test-plan/deploy-sh.md |
| Test Plan: ndp-validate | product/features/dp-023/test-plan/ndp-validate.md |
| Test Plan: config | product/features/dp-023/test-plan/config.md |
| Alignment Report | product/features/dp-023/ALIGNMENT-REPORT.md |
| Acceptance Map | product/features/dp-023/ACCEPTANCE-MAP.md |
| Launch Prompt | product/features/dp-023/LAUNCH-PROMPT.md |

## Component Map

| Component | Pseudocode | Test Plan |
|-----------|-----------|-----------|
| platform-core | pseudocode/platform-core.md | test-plan/platform-core.md |
| ndp-lib | pseudocode/ndp-lib.md | test-plan/ndp-lib.md |
| deploy-sh | pseudocode/deploy-sh.md | test-plan/deploy-sh.md |
| ndp-validate | pseudocode/ndp-validate.md | test-plan/ndp-validate.md |
| config | pseudocode/config.md | test-plan/config.md |

## Goal

Add non-numeric type support (`text` and `jsonb`) as a generic capability through the full Bronze-Silver-Gold pipeline. NWS forecast is the validation case; the design supports future text-bearing streams (syslog, alerts, etc.) without architectural changes. This is plumbing only -- once text reaches Gold, fe-005 handles embedding it. The feature enables the intelligence engine to learn from text data by surfacing it in Gold views queryable by Grafana and the intelligence app.

## Tracking

- GitHub Issue: https://github.com/dug-21/neural-data-platform/issues/37
- Version target: v1.2.x

## Resolved Decisions

| Decision | Resolution | Source | Pattern ID |
|----------|-----------|--------|-----------|
| JSONB coercion strategy | Explicit `"jsonb"` match arm in coerce_to_type() with JSON validation for strings | ADR-001 | 23 |
| TimescaleOutput binding | Type-aware placeholders: `$N::jsonb` for jsonb columns in build_upsert_query() | ADR-002 | 24 |
| Gold text mechanism | Per-domain VIEW (not MATERIALIZED VIEW) using DISTINCT ON, unpivoted schema | ADR-003 | 25 |
| NWS forecast config | Add stream_type, detailedForecast parser mapping, silver_etl with 7 numeric + 2 text fields | ADR-004 | 26 |
| Validation updates | Verify/update schema enum to include text, jsonb, varchar, boolean, text[] | ADR-005 | 27 |
| Data dictionary | No code changes -- existing sync handles text/jsonb types at deploy.sh lines 676, 679 | ADR-006 | 28 |

## Files to Create/Modify

### New Files

| File | Description |
|------|-------------|
| `crates/ndp-lib/src/gold/generators/text_view.rs` | TextViewGenerator -- per-domain VIEW over Silver text columns |

### Modified Files

| File | Description |
|------|-------------|
| `core/src/silver/transform.rs` | Add `"jsonb"` branch to `coerce_to_type()` (~15 lines, after line 653) |
| `core/src/silver/outputs/timescale.rs` | Type-aware placeholders in `build_upsert_query()` (~10 lines modified at line 196) |
| `config/base/streams/nws-forecast-hourly/config.json` | Add `stream_type`, `detailedForecast` parser mapping, `silver_etl` section |
| `crates/ndp-lib/src/gold/generators/mod.rs` | Add `pub mod text_view` and re-export |
| `tools/ndp-gold-ddl/src/lib.rs` | Add re-export of TextViewGenerator |
| `deploy/pi/deploy.sh` | Add `handle_gold_text_view()` to Phase 6 |
| `config/schemas/stream.schema.json` (if exists) | Verify/update type enum to include text, jsonb, etc. |

### Verified Unchanged

| File | Why |
|------|-----|
| `deploy/pi/ddl-generator.sh` | `map_type()` already handles text/jsonb/varchar -- no changes needed |
| `deploy/pi/deploy.sh` (dictionary sync) | `_sync_to_data_dictionary_bash()` already maps text->TEXT, jsonb->JSONB |
| `core/src/types/stream_config.rs` | `SilverFieldType` already has Text, Jsonb, Boolean, Varchar, TextArray |
| `apps/silver-etl/` | DEPRECATED -- do not touch |

## Data Structures

### TextFieldInfo (new, in text_view.rs)

```rust
struct TextFieldInfo {
    stream_id: String,          // "nws-forecast-hourly"
    silver_table: String,       // "silver.nws_forecast_hourly"
    column_name: String,        // "short_forecast"
    field_type: String,         // "text" or "jsonb"
    timestamp_column: String,   // "observation_time"
}
```

### TextViewGenerator (new, in text_view.rs)

```rust
pub struct TextViewGenerator<L: ConfigLoader> {
    config_loader: L,
}

impl<L: ConfigLoader> TextViewGenerator<L> {
    pub fn new(config_loader: L) -> Self;
    pub fn generate(&self, domain_id: &str, action: Action) -> Result<String>;
    fn discover_text_fields(&self, domain_config: &DomainConfig) -> Vec<TextFieldInfo>;
}
```

## Function Signatures

### coerce_to_type() -- new jsonb branch (transform.rs)

```rust
// Add to existing match arms in coerce_to_type():
"jsonb" => match value {
    Value::Object(_) | Value::Array(_) => Ok(value.clone()),
    Value::String(s) => serde_json::from_str::<Value>(s)
        .map_err(|_| TransformError::TypeConversion { ... }),
    Value::Null => Ok(Value::Null),
    Value::Number(_) | Value::Bool(_) => Ok(value.clone()),
},
```

### build_upsert_query() -- type-aware placeholders (timescale.rs)

```rust
// Modified field iteration in build_upsert_query():
for name in &field_names {
    columns.push(name.clone());
    let col_type = etl_config.field_mappings.iter()
        .find(|m| m.target_column == *name)
        .map(|m| m.column_type.as_str())
        .unwrap_or("text");
    if col_type == "jsonb" {
        placeholders.push(format!("${}::jsonb", param_index));
    } else {
        placeholders.push(format!("${}", param_index));
    }
    param_index += 1;
}
```

### TextViewGenerator::generate() (text_view.rs)

```rust
pub fn generate(&self, domain_id: &str, action: Action) -> Result<String> {
    // 1. Load domain config
    // 2. discover_text_fields() -- scan streams for text/jsonb field_mappings
    // 3. If none, return comment-only SQL
    // 4. Build UNION ALL subqueries per text field
    // 5. Wrap in CREATE OR REPLACE VIEW gold.{domain}_text AS SELECT DISTINCT ON ...
    // 6. Add COMMENT ON VIEW
}
```

## Test Expectations

### Unit Tests (est. 14 new)

| Test | Component | AC |
|------|-----------|-----|
| coerce_jsonb_object | platform-core | AC-04 |
| coerce_jsonb_array | platform-core | AC-04 |
| coerce_jsonb_string_valid | platform-core | AC-04 |
| coerce_jsonb_string_invalid | platform-core | AC-04 |
| coerce_jsonb_null | platform-core | AC-04 |
| coerce_jsonb_number | platform-core | AC-04 |
| coerce_jsonb_boolean | platform-core | AC-04 |
| build_upsert_query_jsonb_cast | platform-core | AC-03 |
| build_upsert_query_text_no_cast | platform-core | AC-03 |
| build_raw_query_text_value | platform-core | AC-05 |
| build_raw_query_jsonb_value | platform-core | AC-05 |
| build_raw_query_text_with_quotes | platform-core | AC-05 |
| generate_single_text_field | ndp-lib | AC-06, AC-07 |
| generate_mixed_numeric_text | ndp-lib | AC-07 |

### Regression Tests

- `cargo test -p platform-core` -- 908 existing tests
- `cargo test -p ndp-lib` -- 606 existing tests
- `ndp validate` on all existing stream configs

### Integration Tests

- DDL generator produces TEXT/JSONB columns for NWS forecast config
- Data dictionary sync populates silver_columns with TEXT entries
- Gold text view SQL is syntactically valid

## Wave Structure

### Wave 1: Core Silver Changes (parallel)

| Task | Files | Complexity |
|------|-------|-----------|
| W1-01: Add jsonb branch to coerce_to_type() | core/src/silver/transform.rs | Low |
| W1-02: Fix TimescaleOutput JSONB parameter binding | core/src/silver/outputs/timescale.rs | Medium |
| W1-03: Verify text value flow through TimescaleOutput | core/src/silver/outputs/timescale.rs | Low |

### Wave 2: Configuration + DDL + Gold (after Wave 1)

| Task | Files | Complexity |
|------|-------|-----------|
| W2-01: Add silver_etl to NWS forecast config | config/base/streams/nws-forecast-hourly/config.json | Medium |
| W2-02: Verify DDL generator handles text/jsonb | deploy/pi/ddl-generator.sh | Low |
| W2-03: Create Gold text view generator | crates/ndp-lib/src/gold/generators/text_view.rs + mod.rs | High |
| W2-04: Wire Gold text view into deploy.sh | deploy/pi/deploy.sh | Low |

### Wave 3: Validation + Dictionary + Integration (after Wave 2)

| Task | Files | Complexity |
|------|-------|-----------|
| W3-01: Validate ndp-validate accepts text/jsonb | tools/ndp-validate/, config/schemas/ | Low |
| W3-02: Verify data dictionary sync | deploy/pi/deploy.sh | Low |
| W3-03: Integration test -- full pipeline | tests/integration/ | High |
| W3-04: Existing stream regression test | existing test suites | Low |

## Constraints

- ARM64 (Raspberry Pi 5) -- all dependencies must compile for aarch64
- Config-driven -- no hardcoded DDL, view names, or type mappings
- No DuckDB, no Polars -- use TimescaleDB
- `apps/silver-etl/` is DEPRECATED -- do not reference or modify
- No new NOTIFY triggers -- intelligence reads Gold text view on existing `gold_refresh`
- No text processing/NLP/embedding (fe-005 territory)
- Version target: v1.2.x

## Dependencies

### Crate Dependencies (no new)

No new crate dependencies. TextViewGenerator uses existing `ndp-lib` infrastructure (ConfigLoader trait, gold::config types, gold::error types).

### Feature Dependencies

- dp-020 (declarative deployment) -- COMPLETE
- ops-002 (config-driven generators) -- COMPLETE
- ops-008 (database bootstrap) -- COMPLETE (init scripts for data_dictionary tables)

## NOT in Scope

- Text embedding / MiniLM / EventEmbedder (fe-005)
- Text feature extraction tables (fe-005)
- Template caching (fe-005)
- Composite embeddings (fe-006)
- Text preprocessing / templating / filtering
- Syslog domain -- future consumer
- Retention configuration -- handled by existing Silver policies
- Any NLP or text processing
- DQ rules for text fields (deferred)
- MATERIALIZED VIEW upgrade for Gold text (performance optimization, deferred)

## Alignment Status

**Overall: PASS** -- All 7 alignment principles satisfied. Self-Learning is WARN-expected (infrastructure feature). No variances requiring user approval. See ALIGNMENT-REPORT.md.
