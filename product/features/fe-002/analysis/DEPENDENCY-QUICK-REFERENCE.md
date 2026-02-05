# GAP-001 vs GAP-003: Quick Reference

## One-Sentence Summary

**GAP-001 (YAML→JSON) MUST complete before GAP-003 (Schema Validation) because the validator requires JSON format.**

---

## Dependency Flow

```
┌──────────────────────────────────────────────────────────────────┐
│ V1.2: Domain Configuration Standardization Feature               │
└──────────────────────────────────────────────────────────────────┘

Phase 1: GAP-001 (Format Migration)
═════════════════════════════════════════════════════════════════════
  Step 1a: Migrate config/domains/indoor-air-quality/
           domain.yaml → domain.json

  Step 1b: Update loader.rs
           - Line 46-47: domain_config_path() → "domain.json"
           - Line 80: serde_yaml → serde_json

  Step 1c: Update tests
           - Line 331: YAML snippet → JSON
           - Line 349: YAML snippet → JSON

  Result:  ✓ format consistency across all configs
           ✓ enables Phase 2

           ↓ (conditional: proceed to Phase 2 only if Phase 1 passes)

Phase 2: GAP-003 (Schema Validation)
═════════════════════════════════════════════════════════════════════
  Step 2a: Create validator.rs module
           - JSON Schema validation logic
           - Error handling and messages

  Step 2b: Integrate into loader.rs
           - Call validator() before deserializer()
           - Error precedence: schema errors before parse errors

  Step 2c: Add test fixtures
           - valid_complete.json
           - invalid_missing_id.json
           - invalid_bad_granularity.json
           - [4-5 more edge cases]

  Result:  ✓ two-layer validation (schema + struct)
           ✓ catches config errors early
           ✓ consistent with stream configs (dp-019)
```

---

## Can They Run In Parallel?

| Scenario | Answer | Why |
|----------|--------|-----|
| **GAP-003 before GAP-001?** | ❌ NO | Schema validates JSON; YAML files won't exist |
| **GAP-001 before GAP-003?** | ✅ YES | Migration prepares format for validation |
| **Both simultaneously?** | ⚠️ RISKY | Would create merge conflicts, overlapping code changes in loader.rs |
| **One after the other?** | ✅ YES (RECOMMENDED) | Natural progression, no conflicts |

---

## Shared Code: loader.rs

### Current State
```rust
// tools/ndp-gold-ddl/src/config/loader.rs

fn domain_config_path(&self, domain_id: &str) -> PathBuf {
    self.config_dir.join("domains").join(domain_id)
        .join("domain.yaml")  // ← INCONSISTENT with streams
}

fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig> {
    let path = self.domain_config_path(domain_id);
    if !path.exists() { return Err(...); }
    let content = std::fs::read_to_string(&path)?;

    let config: DomainConfig =
        serde_yaml::from_str(&content)?  // ← Uses YAML parser

    Ok(config)
}
```

### After GAP-001
```rust
// ← Same function name, different path + parser
fn domain_config_path(&self, domain_id: &str) -> PathBuf {
    self.config_dir.join("domains").join(domain_id)
        .join("domain.json")  // ✓ Consistent with streams
}

fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig> {
    let path = self.domain_config_path(domain_id);
    if !path.exists() { return Err(...); }
    let content = std::fs::read_to_string(&path)?;

    let config: DomainConfig =
        serde_json::from_str(&content)?  // ✓ Uses JSON parser

    Ok(config)
}
```

### After GAP-003 (Depends on GAP-001)
```rust
// ← Validator integrates into same function
fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig> {
    let path = self.domain_config_path(domain_id);
    if !path.exists() { return Err(...); }
    let content = std::fs::read_to_string(&path)?;

    // ← NEW: Layer 1 validation (only works on JSON)
    validate_domain_json(&content)?;

    // Layer 2 validation (deserialization)
    let config: DomainConfig =
        serde_json::from_str(&content)?  // ← Still JSON parser

    Ok(config)
}
```

**Key Insight:** Validator integrates cleanly into the same function ONLY after GAP-001 changes the format to JSON.

---

## Impact Matrix

| Component | GAP-001 | GAP-003 | Notes |
|-----------|---------|---------|-------|
| **Config Files** | ✓ Migrate | - | domain.yaml → domain.json |
| **loader.rs** | ✓ Update | ✓ Update | Path change + Parser swap + Validator call |
| **domain.rs** | ✓ Update tests | - | YAML snippets → JSON |
| **Cargo.toml** | - | ✓ Update | Add jsonschema crate |
| **New Files** | - | ✓ Create | validator.rs + test fixtures |
| **Schema File** | - | - | Exists; needs format verification |

---

## Risk: Schema-Struct Mismatch

**Current Schema** (config/schemas/domain.schema.json):
```json
{
  "type": "object",
  "properties": {
    "domain": {           // ← WRAPPER KEY
      "type": "object",
      "properties": {
        "id": {...},
        "streams": {...},
        "alignment": {...}
      }
    }
  }
}
```

**Expected by Struct** (tools/ndp-gold-ddl/src/config/domain.rs line 10):
```rust
pub struct DomainConfig {
    pub id: String,         // ← NO WRAPPER - at root level
    pub description: String,
    pub streams: Vec<StreamRef>,
    pub alignment: AlignmentConfig,
    // ...
}
```

**Resolution during GAP-001:**
1. Migrate domain.yaml to domain.json
2. Test deserialization: `cargo test test_domain_config_deserialize`
3. If test fails → **Update schema** to remove wrapper before GAP-003

---

## Decision Points

### After GAP-001 Phase 1
**Checkpoint:** Does cargo test pass?
- ✅ YES → Proceed to GAP-003
- ❌ NO → Debug format/parser issue, fix before GAP-003

### After GAP-001 Phase 2
**Checkpoint:** Does ndp-gold-ddl validate --domain indoor-air-quality pass?
- ✅ YES → Ready for Phase 2 (GAP-003)
- ❌ NO → Fix config or schema before proceeding

### After GAP-003 Phase 2a/2b
**Checkpoint:** Do validator tests pass?
- ✅ YES → Proceed to Phase 2c
- ❌ NO → Debug validator logic

### After GAP-003 Phase 2c
**Checkpoint:** All fixtures covered?
- ✅ YES → Merge & deploy
- ❌ NO → Add more edge cases

---

## Why NOT Separate?

If we split this into two separate V1.2 features:

```
Feature A (V1.2.1): Domain Config Format Migration
├─ Migrate YAML → JSON
├─ Update parser
└─ Result: JSON format, but no validation
          (inconsistent with goal: "standardize format")

THEN later...

Feature B (V1.2.2): Add Schema Validation
├─ Create validator
├─ Add jsonschema crate
└─ Integrate into loader.rs
   (all the same changes as if combined)

❌ Problem: Intermediate state (JSON without validation) is awkward
❌ Problem: GAP-003 must wait anyway (true dependency)
❌ Problem: Two separate reviews of similar code
```

**Better:** Combine as one V1.2 "Domain Configuration" epic with two clearly sequenced phases.

---

## Execution Estimate

| Phase | Task | Estimate | Notes |
|-------|------|----------|-------|
| **GAP-001** | Migrate domain.yaml → domain.json | 30 min | Simple config change |
| **GAP-001** | Update loader.rs (2 changes) | 15 min | Mechanical code change |
| **GAP-001** | Update tests (2 snippets) | 15 min | YAML → JSON conversion |
| **GAP-001** | Testing & verification | 30 min | cargo test + manual validation |
| **GAP-003** | Create validator.rs (~50 lines) | 60 min | Schema integration logic |
| **GAP-003** | Create 6-8 test fixtures | 45 min | Edge cases + baselines |
| **GAP-003** | Integration testing | 30 min | Positive + negative tests |
| **Docs** | Update procedures | 30 min | VALIDATION-PROCEDURE.md + guide |
| | **TOTAL** | ~4.5-5 hours | 2h (GAP-001) + 3h (GAP-003) |

---

## Commands to Verify

### After GAP-001
```bash
# Verify format migration
file config/domains/indoor-air-quality/domain.json

# Verify loader sees new path
cargo test test_load_domain_config_success

# Verify validation works
cargo run -p ndp-gold-ddl -- validate --domain indoor-air-quality

# Should output: "Domain 'indoor-air-quality' configuration is valid"
```

### After GAP-003
```bash
# Verify validator catches errors
cargo test test_invalid_domain_missing_id
cargo test test_invalid_domain_bad_granularity

# Verify positive cases still pass
cargo test test_valid_domain_complete
```

---

## Recommendation for Team

| Stakeholder | Recommendation |
|-------------|-----------------|
| **Sprint Planner** | Schedule as single V1.2 epic with two phases (~5h) |
| **Architecture** | Combine features; update ADR-016-001 to reference both |
| **Rust Dev** | Execute Phase 1 (GAP-001) first; pause for checkpoint; then Phase 2 (GAP-003) |
| **Tester** | Prepare test fixtures now; they're independent of code changes |
| **Docs** | Update VALIDATION-PROCEDURE.md only after GAP-001 complete |

---

## Related Documentation

- **Full Analysis:** DEPENDENCY-ANALYSIS-GAP-001-GAP-003.md (this directory)
- **Issue #11:** GAP-001 - Domain Config Format Inconsistency
- **Issue #13:** GAP-003 - No JSON Schema Validation
- **ADR-016-001:** JSON Configuration Standard
- **Fast-Follower Report:** phase-d/reports/FAST-FOLLOWER-REPORT.md
- **Schema File:** config/schemas/domain.schema.json
- **Code:** tools/ndp-gold-ddl/src/config/loader.rs

---

**TL;DR:** Do GAP-001 first (YAML→JSON), then GAP-003 (add validation). They're sequential, not parallel. Combined as single V1.2 feature (~5h total).
