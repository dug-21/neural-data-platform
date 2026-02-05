# GAP-003 Scope Analysis: Domain Schema Validation

**Issue**: Domain configurations lack Layer 1 JSON Schema validation, unlike stream configurations which have comprehensive two-layer validation.

**Analysis Date**: 2026-02-05
**Analyst**: NDP Architecture Team
**Related Feature**: FE-001 Gold Layer Foundation
**Related ADR**: ADR-FE001-002 (Domain-Centric Configuration)

---

## Executive Summary

| Aspect | Finding |
|--------|---------|
| **Scope** | Medium (~2-3 days for 1 developer) |
| **Complexity** | Low-to-Medium |
| **Risk** | Low (follows established dp-019 pattern) |
| **Reusability** | High (can reuse stream validation infrastructure) |
| **Files to Modify** | 5-6 files |
| **New Files** | 0-1 files |
| **Breaking Changes** | None (additive only) |

---

## Current State Analysis

### What Exists

1. **Domain Schema (COMPLETE)**
   - File: `/workspaces/neural-data-platform/config/schemas/domain.schema.json`
   - Status: Comprehensive JSON Schema Draft 7
   - Coverage: All domain concepts (streams, alignment, objectives, constraints)
   - Size: 316 lines
   - Definitions: 10 complex object types with nested validation rules

2. **Semantic Validation (COMPLETE)**
   - File: `/workspaces/neural-data-platform/tools/ndp-validate/src/semantic/domain.rs`
   - Status: Full Layer 2 implementation (27,229 bytes)
   - Validation Rules:
     - `validate_domain_stream_references()` - Stream existence checks
     - `validate_unique_aliases()` - Duplicate alias detection
     - `validate_has_primary()` - At least one primary stream required
     - `validate_alignment()` - Join strategy and granularity validation
     - `validate_objectives()` - Target stream/metric existence
     - `validate_constraints()` - Constraint stream/metric existence
   - Error Codes: `InvalidDomainStream`, `CircularDomainDependency`, `InvalidObjectiveCondition`, etc.

3. **Stream Schema Validation Pipeline (REFERENCE)**
   - Layer 1: `SchemaValidator` class in `/workspaces/neural-data-platform/tools/ndp-validate/src/schema.rs` (40,209 bytes)
   - Layer 2: `SemanticValidator` class in `/workspaces/neural-data-platform/tools/ndp-validate/src/semantic/mod.rs` (5,552 bytes)
   - Pattern established in dp-019 specification
   - Exit codes, error reporting, JSON/human output formats all standardized

4. **Test Infrastructure (COMPLETE)**
   - Stream schema validation: Comprehensive London TDD tests
   - Domain semantic validation: Tests embedded in `semantic/domain.rs`
   - Valid test data: `/workspaces/neural-data-platform/config/schemas/tests/valid-domain.json`

5. **CLI Infrastructure (COMPLETE)**
   - File: `/workspaces/neural-data-platform/tools/ndp-validate/src/cli.rs` (35,448 bytes)
   - Current flags: `--all`, `--schema-only`, `--check-tables`, `--format`, `--strict`, `--verbose`
   - Output formats: JSON and human-readable with colors
   - Exit codes: 0 (success), 1 (validation error), 2 (system error)
   - **Missing**: `--domain` flag and domain-specific config directory handling

### What's Missing (Gap)

| Component | Current | Gap |
|-----------|---------|-----|
| **Layer 1 Schema Validation** | ✓ For streams | ✗ For domains |
| **CLI Flag** | `--schema-only` | ✗ `--domain` flag |
| **Config Directory** | `config/base/streams/` | ✗ `config/domains/` handling |
| **Schema File Parameter** | Hardcoded for streams | ✗ Domain schema path option |
| **Integration** | Semantic validation exists | ✗ Not connected to CLI |
| **Error Handling** | Stream errors → formatted | ✗ Domain schema errors missing |
| **Test Coverage** | Stream schema tested | Partial for domain semantic |
| **Documentation** | CLI help/README | ✗ Domain validation not mentioned |

---

## Files Requiring Modification

### Core Implementation Files (6)

#### 1. **`/tools/ndp-validate/src/cli.rs`** - CLI Arguments [Priority: HIGH]

**Current State**:
- 1,099 lines (35,448 bytes)
- Defines `Cli` struct with stream-specific arguments
- `--all` flag defaults to `config/base/streams/`
- No domain configuration handling

**Required Changes**:
```rust
// Add new enum variant
pub enum ConfigType {
    Stream,
    Domain,
}

// Modify Cli struct:
pub struct Cli {
    pub config_path: Option<PathBuf>,
    pub config_type: ConfigType,  // NEW: stream|domain
    pub all: bool,
    pub domain_dir: PathBuf,      // NEW: config/domains directory
    pub stream_dir: PathBuf,      // RENAME from config_dir (backward compatible)
    // ... existing fields ...
}

// Add validation logic for domain context
pub fn validate_args(&self) -> Result<(), String> {
    // Validate domain-specific requirements
    if self.config_type == ConfigType::Domain && /* conditions */ {
        return Err("...".to_string());
    }
    // ... existing validation ...
}
```

**Scope**: 50-100 lines added/modified
**Tests**: Add 15-20 test cases for new flag combinations

#### 2. **`/tools/ndp-validate/src/schema.rs`** - Layer 1 Schema Validation [Priority: HIGH]

**Current State**:
- 40,209 bytes
- `SchemaValidator` class with stream schema (hardcoded default)
- Methods: `from_file()`, `validate_schema()`, `validate_json_syntax()`

**Required Changes**:
```rust
impl SchemaValidator {
    // NEW: Generic constructor that can load any schema file
    pub fn from_file_generic(path: &Path, schema_type: SchemaType) -> Result<Self, SchemaValidatorError> {
        // Generalize schema loading for both stream and domain schemas
    }

    // KEEP EXISTING: default_stream_schema() for backward compatibility
    pub fn default_stream_schema() -> Result<Self, SchemaValidatorError> { /* ... */ }

    // NEW: Load domain schema
    pub fn default_domain_schema() -> Result<Self, SchemaValidatorError> {
        let schema_value = serde_json::from_str(include_str!("../../schemas/domain.schema.json"))?;
        Self::new(schema_value)
    }
}
```

**Scope**: 30-50 lines added
**Complexity**: Low (reuse existing validation logic)
**Tests**: Add tests for domain schema loading and basic validation errors

#### 3. **`/tools/ndp-validate/src/main.rs`** - CLI Entry Point [Priority: MEDIUM]

**Current State**:
- 221 lines
- Single validation flow assumes stream config
- Hardcoded stream schema validator

**Required Changes**:
```rust
async fn run_validation(cli: &Cli) -> Result<ValidationResult, Box<dyn std::error::Error>> {
    // ... file existence checks ...

    // NEW: Branch on config type
    match cli.config_type {
        ConfigType::Stream => {
            let schema_validator = SchemaValidator::default_stream_schema()?;
            // ... existing stream validation ...
        }
        ConfigType::Domain => {
            let schema_validator = SchemaValidator::default_domain_schema()?;
            // Use domain schema for Layer 1
            let schema_errors = schema_validator.validate_schema(&value);
            for error in schema_errors { result.add_error(error); }

            // NEW: Call domain semantic validation (Layer 2)
            if !cli.schema_only {
                let available_streams = load_available_streams()?;  // NEW helper
                let semantic_errors = validate_domain(&value, &available_streams);
                for error in semantic_errors { result.add_error(error); }
            }
        }
    }
    // ...
}
```

**Scope**: 40-60 lines added
**Complexity**: Low (follows existing pattern)

#### 4. **`/tools/ndp-validate/src/semantic/mod.rs`** - Semantic Validator Orchestration [Priority: MEDIUM]

**Current State**:
- 148 lines
- `SemanticValidator::validate()` called for all configs (assumes stream)
- Domain validation exists but not integrated

**Required Changes**:
```rust
pub struct SemanticValidator;

impl SemanticValidator {
    pub fn new() -> Self { Self }

    // EXISTING: Stream validation
    pub fn validate(&self, config: &Value) -> Vec<ValidationError> { /* ... */ }

    // NEW: Domain-specific validation
    pub fn validate_domain(&self, config: &Value, available_streams: &HashSet<String>)
        -> Vec<ValidationError>
    {
        // Wrapper around domain::validate_domain() that also validates
        // stream existence by loading available streams from config directory
        validate_domain(config, available_streams)
    }
}
```

**Scope**: 10-20 lines added
**Complexity**: Minimal (mostly delegation)

#### 5. **`/tools/ndp-validate/src/lib.rs`** - Public API Exports [Priority: LOW]

**Current State**:
- 1,837 bytes (54 lines)
- Exports stream validation components

**Required Changes**:
```rust
// Add public re-export
pub use semantic::validate_domain;
pub use schema::SchemaValidator;
```

**Scope**: 2-5 lines added
**Complexity**: Trivial

#### 6. **`/tools/ndp-validate/Cargo.toml`** - Dependency Management [Priority: LOW]

**Current State**:
- All required dependencies present (jsonschema, serde_json, tokio, etc.)

**Required Changes**:
- None - all dependencies already included

---

## Infrastructure & Support Files

### 7. **`/tools/ndp-validate/src/error.rs`** - Error Types [REVIEW ONLY]

**Current State**:
- 15,023 bytes
- Error codes defined for Layer 1 (Schema) and Layer 2 (Semantic)
- Domain-specific error codes already defined: `InvalidDomainStream` (404), `CircularDomainDependency` (407), `InvalidObjectiveCondition` (408)

**Required Changes**: None (error codes already exist)

---

## What Already Exists and Can Be Reused

| Component | Location | Status | Reusable For Domains |
|-----------|----------|--------|----------------------|
| JSON Schema infrastructure | `schema.rs` | Complete | Yes - just load different schema |
| Error types | `error.rs` | Complete | Yes - domain codes exist |
| Semantic validation rules | `semantic/domain.rs` | Complete | Yes - just integrate |
| Output formatting | `cli.rs` | Complete | Yes - same format for both |
| Exit codes | `cli.rs` | Complete | Yes - same codes apply |
| Test framework | `cli.rs` tests | Complete | Yes - adapt test patterns |
| YAML support | `main.rs` | Complete | Yes - domain configs are YAML |

---

## Detailed Work Breakdown

### Phase 1: CLI & Main Flow (3-4 hours)
1. Add `ConfigType` enum to distinguish stream vs domain configs
2. Add `--domain` flag and config directory arguments to `Cli` struct
3. Add validation for flag combinations
4. Update `main.rs` to branch on config type
5. Write 20+ test cases for new CLI paths

### Phase 2: Schema Validation Layer (2-3 hours)
1. Add `default_domain_schema()` method to `SchemaValidator`
2. Create generic schema loading helper (optional refactoring)
3. Update `main.rs` to use correct schema validator based on config type
4. Write tests for domain schema validation errors
5. Test with valid domain configuration

### Phase 3: Semantic Integration (2-3 hours)
1. Add domain-specific validator method to `SemanticValidator`
2. Implement stream availability loader (load streams from `config/base/streams/`)
3. Call domain semantic validation in main flow
4. Update error reporting for domain-specific errors
5. Write tests for semantic validation errors

### Phase 4: Documentation & Polish (1-2 hours)
1. Update CLI help text and README
2. Add examples in help for `--domain` flag
3. Update main.rs docstring
4. Write integration test for end-to-end domain validation
5. Document error codes for domain validation

---

## Implementation Strategy

### Why Low Complexity?

1. **No new concepts** - Follows exact dp-019 two-layer pattern
2. **Schema already exists** - `domain.schema.json` is complete and tested
3. **Semantic validation exists** - `semantic/domain.rs` is fully implemented
4. **Reusable infrastructure** - Can directly reuse `SchemaValidator`, error types, output formatting
5. **Similar to streams** - Domain validation follows same layer 1 → layer 2 flow
6. **No database changes** - No schema migrations required
7. **No breaking changes** - All new code is additive; existing stream validation unchanged

### Why Not Medium/High?

- Would be Medium if we had to implement semantic validation from scratch (we don't)
- Would be High if we had to redesign CLI (we don't - just add flags)
- Would be High if we had to create schema (we don't - it exists)
- The work is mostly **integration**, not **development**

---

## Test Coverage Requirements

### Unit Tests (New - 30-40 test cases)

```rust
// CLI tests
#[test] fn test_parse_domain_flag() { /* ... */ }
#[test] fn test_parse_domain_config_path() { /* ... */ }
#[test] fn test_domain_requires_available_streams() { /* ... */ }
#[test] fn test_invalid_domain_config_path() { /* ... */ }

// Schema tests
#[test] fn test_load_domain_schema() { /* ... */ }
#[test] fn test_validate_domain_schema_valid() { /* ... */ }
#[test] fn test_validate_domain_schema_missing_required() { /* ... */ }
#[test] fn test_validate_domain_schema_invalid_role() { /* ... */ }
#[test] fn test_validate_domain_schema_invalid_granularity() { /* ... */ }

// Semantic tests (integrate existing domain.rs tests)
#[test] fn test_domain_semantic_stream_not_found() { /* ... */ }
#[test] fn test_domain_semantic_duplicate_alias() { /* ... */ }
#[test] fn test_domain_semantic_no_primary_stream() { /* ... */ }
```

### Integration Tests (3-5 test cases)

```rust
// Full flow tests
#[test] async fn test_validate_domain_config_end_to_end() { /* ... */ }
#[test] async fn test_domain_validation_json_output() { /* ... */ }
#[test] async fn test_domain_validation_human_output() { /* ... */ }
#[test] async fn test_domain_strict_mode() { /* ... */ }
```

### Test Data

Use existing files:
- Valid domain: `/workspaces/neural-data-platform/config/schemas/tests/valid-domain.json`
- Real domain: `/workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.yaml`
- Invalid schemas: Create 3-5 files for common errors

---

## Files Modified Summary

| File | Type | Lines | Complexity |
|------|------|-------|-----------|
| `src/cli.rs` | Core | +80-120 | Low |
| `src/main.rs` | Core | +50-80 | Low |
| `src/schema.rs` | Core | +30-50 | Low |
| `src/semantic/mod.rs` | Support | +15-25 | Minimal |
| `src/lib.rs` | Support | +2-5 | Trivial |
| `Cargo.toml` | Config | 0 | N/A |
| **Total** | | **+177-280** | Low-Medium |

---

## Risk Assessment

### Low-Risk Factors
1. No breaking changes (additive only)
2. Error infrastructure already exists
3. Semantic validation already complete
4. Schema already complete
5. Follows established dp-019 pattern
6. All test infrastructure in place

### Medium-Risk Factors
1. Need to load available streams from filesystem (robustness needed)
2. YAML/JSON format handling (but infrastructure exists)
3. Flag validation must prevent invalid combinations

### Mitigation Strategies
1. Create helper function `load_available_streams()` with error handling
2. Validate YAML parsing follows same pattern as streams
3. Add comprehensive CLI validation tests

---

## Dependencies & Blockers

### Required Before Implementation
- None - all infrastructure exists

### Post-Implementation
- CI/CD update to run domain validation (separate PR)
- Documentation update for users (can be same PR)
- Optional: GitHub Actions workflow update

---

## Estimation Summary

| Task | Hours | Notes |
|------|-------|-------|
| CLI modifications | 3-4 | Add flags, validation, branching logic |
| Schema validation layer | 2-3 | Add domain schema loading |
| Semantic integration | 2-3 | Wire up domain validation |
| Tests | 4-5 | Unit + integration tests |
| Documentation | 1-2 | Help text, examples, README |
| Code review | 1 | Architecture team review |
| **Total** | **13-18 hours** | **~1.5-2 days for 1 developer** |

### Confidence Level: HIGH (85-90%)
- All infrastructure exists
- Clear pattern to follow
- Low integration complexity
- Well-defined scope

---

## Success Criteria

### Acceptance Tests

1. **CLI Flag Works**
   - `ndp-validate config/domains/indoor-air-quality/domain.yaml` validates domain
   - `ndp-validate --domain config/domains/indoor-air-quality/domain.yaml` (explicit flag)
   - `ndp-validate --all --domain` validates all domains from `config/domains/`

2. **Layer 1 Schema Validation**
   - Invalid domain JSON rejected with clear error message
   - Missing required fields caught (e.g., `id`, `streams`, `alignment`)
   - Invalid enum values caught (e.g., invalid role)
   - Pattern validation works (e.g., `id` kebab-case)

3. **Layer 2 Semantic Validation**
   - Non-existent stream references caught
   - Duplicate aliases caught
   - Missing primary stream caught
   - Invalid objective streams caught

4. **Output Formats**
   - JSON output includes errors with layer, code, path, message
   - Human output shows colored errors with suggestions
   - Exit code 0 on success, 1 on error, 2 on system error

5. **Integration**
   - Works with `--schema-only` flag (skips semantic)
   - Works with `--format json` and `--format human`
   - Works with `--strict` flag (treats warnings as errors)
   - Works with `--verbose` flag (shows progress)

6. **Backward Compatibility**
   - Existing stream validation still works unchanged
   - No breaking changes to CLI
   - New flags are optional

---

## Next Steps (Post-Approval)

1. Create feature branch: `feature/gap-003-domain-schema-validation`
2. Implement Phase 1 (CLI changes)
3. Implement Phase 2 (Schema validation)
4. Implement Phase 3 (Semantic integration)
5. Write comprehensive test suite
6. Update documentation and help text
7. Submit PR with detailed description
8. Architecture team review
9. Merge and update CI/CD

---

## References

| Document | Link | Relevance |
|----------|------|-----------|
| dp-019 | `docs/procedures/DP-019-TWO-LAYER-VALIDATION.md` | Foundation - validation pattern |
| ADR-FE001-002 | `product/features/fe-001/architecture/ADR-FE001-002-domain-centric-config.md` | Domain configuration design |
| GitHub #13 | Issue GAP-003 | Original gap description |
| Domain Schema | `config/schemas/domain.schema.json` | Reference for validation |
| Valid Domain | `config/schemas/tests/valid-domain.json` | Test data |
| Semantic Domain Validation | `tools/ndp-validate/src/semantic/domain.rs` | Existing layer 2 implementation |

---

## Appendix: Example Commands Post-Implementation

```bash
# Validate single domain config
ndp-validate config/domains/indoor-air-quality/domain.yaml

# Validate domain (explicit flag)
ndp-validate --domain config/domains/indoor-air-quality/domain.yaml

# Validate all domains
ndp-validate --all --domain

# Schema validation only
ndp-validate --schema-only config/domains/indoor-air-quality/domain.yaml

# Strict mode (warnings as errors)
ndp-validate --strict config/domains/indoor-air-quality/domain.yaml

# Verbose output
ndp-validate -v config/domains/indoor-air-quality/domain.yaml

# JSON output (for scripting)
ndp-validate --format json config/domains/indoor-air-quality/domain.yaml | jq .

# Human-readable output
ndp-validate --format human config/domains/indoor-air-quality/domain.yaml
```

---

**Report Created**: 2026-02-05
**Analysis Confidence**: HIGH (85-90%)
**Ready for Implementation**: YES
