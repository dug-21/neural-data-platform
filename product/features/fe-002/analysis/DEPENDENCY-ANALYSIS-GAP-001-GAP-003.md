# Dependency Analysis: GAP-001 vs GAP-003

**Date:** 2026-02-05
**Analysis By:** NDP Architect
**Status:** READY FOR DECISION

---

## Executive Summary

**GAP-001** (Domain Config YAML→JSON Migration) and **GAP-003** (JSON Schema Validation) have a **sequential dependency**: **GAP-001 must complete before GAP-003**.

**Recommendation:** Combine into single V1.2 feature with recommended execution order:
1. **Phase 1:** Migrate domain configs from YAML to JSON (GAP-001)
2. **Phase 2:** Add JSON Schema validation layer (GAP-003)

This prevents rework and maintains the principle of "JSON as authoritative configuration format" established by ADR-016-001.

---

## Issue Overview

### GAP-001: Domain Config Format Inconsistency [#11](https://github.com/dug-21/neural-data-platform/issues/11)

**Current State:**
- Stream configs: JSON (`config/base/streams/{id}/config.json`)
- Domain configs: YAML (`config/domains/{id}/domain.yaml`)
- Inconsistent parser: `serde_json` for streams, `serde_yaml` for domains

**Target State:**
- Unified JSON format across all configuration files
- Single deserialization path: `serde_json`
- Alignment with ADR-016-001 (JSON as authoritative)

**Scope of Changes:**
```
Files to Migrate:
- config/domains/indoor-air-quality/domain.yaml → domain.json
- [Future domains would use JSON by default]

Code to Update:
- tools/ndp-gold-ddl/src/config/loader.rs (line 46-47, 80)
  * Change domain_config_path() to return *.json
  * Change serde_yaml::from_str → serde_json::from_str

- tools/ndp-gold-ddl/src/config/domain.rs (line 331, 349)
  * Update test YAML → JSON format
```

**Complexity:** Low-Medium
- Path changes: 2 locations
- Parser swap: 1 location
- Test updates: 2 YAML→JSON conversions
- **Risk:** None (schema structure unchanged, just format)

---

### GAP-003: No JSON Schema Validation for Domains [#13](https://github.com/dug-21/neural-data-platform/issues/13)

**Current State:**
- Domain configs: Only Rust struct deserialization validation
- Stream configs: Two-layer validation (JSON Schema Layer 1 + Rust Layer 2)
- Schema exists: `config/schemas/domain.schema.json` (305 lines, comprehensive)

**Target State:**
- Layer 1: JSON Schema validation (before deserialization)
- Layer 2: Semantic validation (during deserialization)
- Consistent with stream config validation strategy (dp-019)

**Scope of Changes:**
```
Files to Create/Update:
- config/schemas/domain.schema.json (EXISTS - no changes needed)
- tools/ndp-gold-ddl/src/config/validator.rs (NEW)
  * Implement schema validation using jsonschema crate
  * Load schema from disk or embed

- tools/ndp-gold-ddl/src/config/loader.rs (UPDATE)
  * Call validator before deserializing
  * Return validation errors before parser errors

- tools/ndp-gold-ddl/Cargo.toml (UPDATE)
  * Add dependency: jsonschema = "0.17"

- tests/fixtures/domains/ (NEW - if not exist)
  * invalid_domain.json (missing required fields)
  * invalid_domain_bad_granularity.json (pattern mismatch)
  * valid_domain.json (baseline)
```

**Complexity:** Medium
- Schema validation library integration: 1
- Validator module: ~50-100 lines
- Loader integration: 10-15 lines
- Test fixtures: 5-8 new files
- **Risk:** Schema mismatch if schema not tested thoroughly

---

## Dependency Analysis

### Question 1: Can GAP-003 be done BEFORE GAP-001?

**Answer: NO - Not practically, and would be rework**

#### Technical Blockers

1. **Schema validates JSON format, but files are YAML**
   ```
   Schema says: "domain.json" (implicit)
   Current: "domain.yaml" exists
   Result: Schema validation would fail on every existing domain
   ```

2. **Validation would reject all current configs during migration**
   ```
   Before GAP-001 complete:
   - domain.yaml exists
   - Validator tries to load domain.json (path would need changing anyway)
   - File not found error or parse error
   ```

3. **Two parsers in flight = confusion**
   ```
   If we add schema validation first:
   - Validator expects JSON per schema
   - Loader still uses serde_yaml
   - Schema validation would be dead code until GAP-001
   ```

#### Architectural Violation

- **ADR-016-001:** "JSON files are the authoritative source for NDP configuration"
  - Adding schema validation before migration contradicts this principle
  - Validates non-authoritative format (YAML)

#### Rework Risk

If we did GAP-003 first:
1. Add schema validator expecting JSON format
2. Tests would mock or skip validation (dead code)
3. Complete GAP-001 (format migration)
4. Enable validation in production (breaking change in test phase)
5. **Result:** All work done twice, no clean progression

---

### Question 2: Can GAP-001 be done BEFORE GAP-003?

**Answer: YES - Strongly recommended**

#### Technical Benefits

1. **GAP-001 removes the format ambiguity**
   ```
   After migration:
   - domain.json exists (matches schema expectation)
   - No path changes needed in validator
   - Validator can immediately validate existing configs
   ```

2. **Validation is straightforward on JSON**
   ```
   Current: serde_yaml → DomainConfig (implicit validation)
   After:   JSON Schema → serde_json → DomainConfig (explicit validation)
   ```

3. **No parser inconsistency**
   ```
   Single path: load domain.json → validate → deserialize with serde_json
   ```

#### Testing is Cleaner

1. **Test fixtures are already JSON-focused**
   ```
   domain.schema.json references JSON structure
   Test data templates would be JSON
   No YAML↔JSON conversion in tests
   ```

2. **Validation tests run on actual stored format**
   ```
   Test: Load config/domains/indoor-air-quality/domain.json
   Validate: Against config/schemas/domain.schema.json
   Result: Tests match production workflow
   ```

#### Implementation Path

```
Phase 1 (GAP-001): ~2 hours
├─ Migrate domain.yaml → domain.json
├─ Update loader.rs paths (2 lines)
├─ Update parser (1 line: serde_yaml → serde_json)
├─ Update tests (2 YAML test snippets → JSON)
└─ Verify: cargo test passes

Phase 2 (GAP-003): ~3 hours
├─ Add jsonschema crate to Cargo.toml
├─ Create validator module (Enum + errors)
├─ Integrate into loader.rs (call validator before deserialize)
├─ Create test fixtures (5-8 files covering schema edge cases)
└─ Verify: cargo test passes + schema validation catches errors
```

---

### Question 3: Are there shared changes?

**Answer: YES - All shared changes are in loader.rs**

#### Shared File: `tools/ndp-gold-ddl/src/config/loader.rs`

| Change | GAP-001 | GAP-003 | Type |
|--------|---------|---------|------|
| `domain_config_path()` | Rename `.yaml` → `.json` | Use new path | Path |
| Line 46-47 | Required | Reads from | Direct |
| `load_domain_config()` | Change parser | Add validator call | Sequence |
| Line 69-85 | Parser change | Validation hook | Direct |

#### GAP-001 Changes (Required First)

```rust
// BEFORE (loader.rs line 42-47)
fn domain_config_path(&self, domain_id: &str) -> PathBuf {
    self.config_dir
        .join("domains")
        .join(domain_id)
        .join("domain.yaml")  // ← CHANGE TO domain.json
}

// BEFORE (loader.rs line 80)
let config: DomainConfig = serde_yaml::from_str(&content)?  // ← CHANGE TO serde_json

// AFTER
fn domain_config_path(&self, domain_id: &str) -> PathBuf {
    self.config_dir
        .join("domains")
        .join(domain_id)
        .join("domain.json")  // ✓ Changed
}

// Line 80
let config: DomainConfig = serde_json::from_str(&content)?  // ✓ Changed
```

#### GAP-003 Changes (Depend on GAP-001)

```rust
// NEW in loader.rs line 70-78 (after file read, before deserialization)
use crate::config::validator::validate_domain_json;

fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig> {
    let path = self.domain_config_path(domain_id);
    // ... file existence check ...
    let content = std::fs::read_to_string(&path)?;

    // ← INSERT: Layer 1 validation (JSON Schema)
    validate_domain_json(&content)?;  // NEW - only works if JSON format

    // Layer 2 validation (Rust deserialization)
    let config: DomainConfig = serde_json::from_str(&content)?;
    Ok(config)
}
```

#### Cascade Effect

```
Change A: domain.yaml → domain.json (GAP-001)
  └─ Enables: validator to find correct file path
  └─ Enables: serde_json parser to work on JSON

Change B: Add validator (GAP-003)
  └─ Requires: File to be JSON (from GAP-001)
  └─ Requires: Path to point to .json (from GAP-001)
  └─ Integrates: Into same load_domain_config() function
```

**Conclusion:** GAP-003 validator cannot integrate cleanly until GAP-001 format migration is complete.

---

## Configuration Schema Status

### Current State

**`config/schemas/domain.schema.json`** (EXISTS - line 1-315)

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "domain": {
      "type": "object",
      "properties": {
        "id": { "type": "string", "pattern": "^[a-z][a-z0-9-]*$" },
        "streams": { "type": "array", ... },
        "alignment": { "type": "object", ... },
        "objectives": { "type": "array", ... }
      }
    }
  }
}
```

**Issue:** Schema wraps content in `"domain"` key, but `DomainConfig` struct expects flat deserialization.

### For GAP-001

After migration, test if struct matches:
```json
{
  "id": "indoor-air-quality",      // NOT wrapped in "domain": {...}
  "description": "...",
  "streams": [...],
  "alignment": {...},
  "objectives": [...]
}
```

**Action:** Remove wrapper from schema or update struct. See domain.rs line 9-27.

### For GAP-003

Validator uses schema as-is:
```rust
let schema = jsonschema::JSONSchema::compile(schema_json)?;
schema.validate(config_json)?;  // Enforces schema structure
```

---

## Risk & Mitigation

### Risk 1: Schema-Struct Mismatch (Discovered During GAP-001)

**Scenario:** Migrate to JSON, then discover schema wrapper doesn't match struct.

**Mitigation:**
- In GAP-001, **test deserialization immediately**
  ```bash
  cargo test test_load_domain_config_success
  ```
- If test fails, update schema before proceeding to GAP-003
- Document decision: wrapper or flat?

### Risk 2: Breaking Change During Validation Integration (GAP-003)

**Scenario:** Adding schema validation rejects previously-valid configs.

**Mitigation:**
- Schema should already be comprehensive (it is - reviewed line-by-line)
- Run validation on all existing domains before production
- If validation fails, update config (not schema) to match

### Risk 3: Test Fixtures Coverage

**Scenario:** Schema validation layer catches edge cases Rust deserialization misses.

**Mitigation:**
- Create fixtures for:
  - ✓ Valid: All required fields present, all constraints met
  - ✓ Invalid: Missing `id` field
  - ✓ Invalid: Bad `granularity` pattern (e.g., "1 days" not "1 day")
  - ✓ Invalid: `join_strategy` not in enum
  - ✓ Invalid: stream role not in enum
  - ✓ Invalid: Stream ID pattern violation
  - ✓ Invalid: Objective condition not in enum

---

## Combined Feature Specification

### V1.2 Domain Configuration Migration

**Feature:** Unified Domain Configuration Format with Validation

**Goals:**
1. Migrate domain configs from YAML to JSON (consistency)
2. Add JSON Schema validation layer (robustness)
3. Maintain backward compatibility via migration guide

**Phases:**

#### Phase 1: Format Migration (GAP-001)
- **Scope:** Convert domain.yaml to domain.json
- **Changes:**
  - Migrate `config/domains/indoor-air-quality/domain.yaml` → `domain.json`
  - Update `loader.rs`: path and parser
  - Update tests
- **Tests:** Existing tests still pass, new path verified
- **Deployment:** None (no Pi impact - config only)
- **Risk:** Low (same structure, different format)

#### Phase 2: Schema Validation (GAP-003)
- **Scope:** Add Layer 1 JSON Schema validation
- **Changes:**
  - Create `validator.rs` with schema validation logic
  - Integrate into `loader.rs` before deserialization
  - Create test fixtures
  - Add `jsonschema` crate dependency
- **Tests:** Positive + negative test cases
- **Deployment:** None (validation only - no data change)
- **Risk:** Low-Medium (schema must match struct)

#### Phase 3: Documentation & Support
- **Scope:** Update procedures and guides
- **Changes:**
  - Update VALIDATION-PROCEDURE.md (reference JSON format)
  - Update domain config contribution guide
  - Add validation error reference
- **Tests:** Manual verification
- **Deployment:** None (docs only)
- **Risk:** None

---

## Alternative: Separate vs. Combined

### Option A: Combine (RECOMMENDED)

**Rationale:**
- Same root cause: domain config handling
- Both affect `loader.rs`
- Sequential dependency naturally enforced
- Single feature story: "Standardize domain configuration"

**Pros:**
- Clear narrative: YAML→JSON + validation
- Shared testing framework
- Single review+deployment cycle
- Avoids intermediate state (JSON without validation)

**Cons:**
- Slightly longer feature timeline (~5 hours)
- More scope in single PR

**Estimate:** 5 hours (2 hr GAP-001 + 3 hr GAP-003)

---

### Option B: Separate Features

**Rationale:**
- GAP-001 is immediate (format consistency)
- GAP-003 can wait for utility validation

**Pros:**
- Faster GAP-001 delivery (~2 hours)
- Smaller PRs

**Cons:**
- Intermediate state: JSON format without validation (confusing)
- Duplicated validation discussions in two feature docs
- GAP-003 must wait for GAP-001 anyway (true dependency)
- Validation framework (validator.rs) created twice in analysis

**Estimate:** 2 hours (GAP-001) + 3 hours (GAP-003 later)

---

## Recommendation: Execute Combined, Sequenced Phases

**Why Combined:**
1. Natural progression: format standardization → validation enforcement
2. Avoids intermediate state (JSON without schema validation)
3. Shared code ownership (loader.rs)
4. Validates ADR-016-001 principle end-to-end

**Why Sequenced:**
1. Phase 1 (GAP-001) validates format migration works
2. Phase 2 (GAP-003) leverages Phase 1's JSON structure
3. Clear decision points between phases
4. Easier debugging if issues arise

**Execution Order:**
```
1. GAP-001: Migrate domain.yaml → domain.json
   ✓ Test: cargo test passes
   ✓ Verify: loader.rs reads new path

2. GAP-003: Add JSON Schema validation
   ✓ Test: positive & negative fixtures pass
   ✓ Verify: schema catches invalid configs

3. Documentation: Update procedures
   ✓ Verify: new engineers can add domains
```

---

## Implementation Checklist

### GAP-001: YAML→JSON Migration

- [ ] Migrate `config/domains/indoor-air-quality/domain.yaml` → `domain.json`
- [ ] Update `tools/ndp-gold-ddl/src/config/loader.rs` line 46-47
- [ ] Update `tools/ndp-gold-ddl/src/config/loader.rs` line 80
- [ ] Update `tools/ndp-gold-ddl/src/config/domain.rs` test YAML (line 331, 349)
- [ ] Run `cargo test` - all pass
- [ ] Verify with: `ndp-gold-ddl validate --domain indoor-air-quality`
- [ ] Update CHANGELOG.md: migration note

### GAP-003: JSON Schema Validation

- [ ] Create `tools/ndp-gold-ddl/src/config/validator.rs`
- [ ] Add `jsonschema = "0.17"` to `Cargo.toml`
- [ ] Implement `validate_domain_json(content: &str) -> Result<()>`
- [ ] Integrate into `loader.rs` before deserialization
- [ ] Create test fixtures:
  - [ ] `tests/fixtures/domains/valid_complete.json`
  - [ ] `tests/fixtures/domains/invalid_missing_id.json`
  - [ ] `tests/fixtures/domains/invalid_bad_granularity.json`
  - [ ] `tests/fixtures/domains/invalid_bad_join_strategy.json`
- [ ] Write integration tests: positive + negative cases
- [ ] Run `cargo test` - all pass
- [ ] Verify error messages are helpful

### Documentation

- [ ] Update `docs/procedures/VALIDATION-PROCEDURE.md`: reference JSON format
- [ ] Add section: "Adding a New Domain Configuration"
- [ ] Add section: "Common Validation Errors"
- [ ] Update CHANGELOG.md: feature notes

---

## Evidence & References

### Current Implementation

- **loader.rs:** `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/loader.rs`
  - Line 42-47: `domain_config_path()` - hardcoded to `.yaml`
  - Line 80: `serde_yaml::from_str()` - YAML parser

- **domain.rs:** `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/domain.rs`
  - Line 9-27: `DomainConfig` struct
  - Line 331, 349: Test YAML snippets

- **Schema:** `/workspaces/neural-data-platform/config/schemas/domain.schema.json`
  - 305 lines, comprehensive, already prepared
  - **Issue:** Wraps in `"domain"` key (needs verification against struct)

- **Config:** `/workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.yaml`
  - 107 lines, no wrapper at root level (already fixed per GAP-002)

### Related Issues & Reports

- GAP-001: Domain Config Format Inconsistency [#11](https://github.com/dug-21/neural-data-platform/issues/11)
- GAP-003: No JSON Schema Validation for Domains [#13](https://github.com/dug-21/neural-data-platform/issues/13)
- ADR-016-001: JSON Configuration Standard
- Fast-Follower Report: `/workspaces/neural-data-platform/product/features/fe-001/phase-d/reports/FAST-FOLLOWER-REPORT.md`

---

## Decision Summary

| Aspect | Decision | Rationale |
|--------|----------|-----------|
| **Execute Combined?** | YES | Natural progression, shared code, avoids intermediate state |
| **Sequence?** | GAP-001 → GAP-003 | Format first, validation second |
| **Blocking?** | GAP-003 blocks on GAP-001 | Schema validates JSON format; YAML files don't exist after migration |
| **Shared Code?** | `loader.rs` | Both affect domain loading pipeline |
| **Risk Level** | Low-Medium | Schema-struct mismatch is main risk |
| **Estimate** | 5 hours | 2h (GAP-001) + 3h (GAP-003) |

---

**Prepared by:** NDP Architect
**Date:** 2026-02-05
**Status:** Ready for Sprint Planning (V1.2)
