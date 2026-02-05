# GAP-001: Domain Config YAML→JSON Migration - Scope Analysis

**Issue**: GitHub Issue #11 - Domain configuration in `ndp-gold-ddl` violates ADR-016-001 by using YAML format instead of JSON.

**Analysis Date**: 2026-02-05
**Analyst**: NDP Architect

---

## Executive Summary

The domain configuration subsystem in `ndp-gold-ddl` uses YAML files (`.yaml` extension, `serde_yaml` parsing) while ADR-016-001 mandates JSON as the platform-wide configuration format. This creates a standards violation and inconsistency.

**Estimated Complexity**: **LOW** ✓
**Estimated Effort**: **4-6 hours** (single developer)
**Risk Level**: **LOW** (well-contained change)
**Breaking Changes**: **None** (internal format only)

---

## Current State Analysis

### Files Affected by YAML Format

**Primary Implementation** (code that reads YAML):
1. `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/loader.rs`
   - **Lines 42-47**: `domain_config_path()` returns `domain.yaml` path
   - **Line 80**: `serde_yaml::from_str()` parses domain config
   - **Impact**: ConfigLoader trait implementation for FileSystemConfigLoader
   - **Lines 69-85**: `load_domain_config()` method loads and parses YAML

2. `/workspaces/neural-data-platform/tools/ndp-gold-ddl/Cargo.toml`
   - **Line 19**: `serde_yaml = "0.9"` dependency
   - **Status**: Can be removed after migration (only used for domain configs)

**Data Files** (configurations to convert):
3. `/workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.yaml`
   - **Lines 1-107**: Complete domain configuration for indoor-air-quality domain
   - **Size**: ~3KB, well-structured YAML
   - **References**: Comments reference ADRs (SPEC-C01, ADR-FE001-004)

**Type Definitions** (compatible with both formats):
4. `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/domain.rs`
   - **No changes needed**: `DomainConfig` and related structs use `#[derive(Serialize, Deserialize)]`
   - **Status**: Already compatible with JSON (demonstrated in tests using inline YAML)
   - **Tests (lines 310-371)**: Parse YAML inline; can be updated to JSON

**Tests Using Inline YAML** (hardcoded test data):
5. `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/domain.rs`
   - **Line 311-329**: `test_domain_config_deserialize()` - parses YAML string
   - **Line 342-350**: `test_stream_ref_with_null_handling_override()` - parses YAML string
   - **Line 354-370**: `test_objective_config_deserialize()` - parses YAML string
   - **Impact**: Tests must be updated to parse JSON instead (code change required)

6. `/workspaces/neural-data-platform/tools/ndp-gold-ddl/tests/fixtures/phase_c.rs`
   - **Lines 35-70**: `create_three_stream_domain()` - constructs DomainConfig programmatically
   - **Lines 77-100+**: Other fixture functions
   - **Status**: No changes needed (constructs structs directly, not parsing files)

**Test Coverage**:
7. `/workspaces/neural-data-platform/tools/ndp-gold-ddl/tests/aligned_view_tests.rs`
   - Status: Likely uses phase_c fixtures (verified as passing)

### Schema Validation

**Existing JSON Schema**:
- `/workspaces/neural-data-platform/config/schemas/domain.schema.json`
- **Status**: Comprehensive schema exists for domain configs
- **Lines 308-314**: Expects `{ "domain": { ... } }` structure
- **Note**: Domain config file should match `domain_content` definition (lines 267-303)

### Current Configuration Structure

The domain.yaml file (/workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.yaml) is a **flat structure** (not wrapped in `domain:` key):

```yaml
id: indoor-air-quality
description: "Maintain healthy indoor air quality"
streams:
  - stream_id: air-quality
    alias: indoor
    role: primary
alignment:
  view_name: indoor_air_quality_aligned
  granularity: "1 hour"
objectives: [...]
```

**Problem**: Current schema at line 308-314 expects:
```json
{
  "domain": { /* content here */ }
}
```

**Migration Decision**: The domain.json file should use the **flat structure** (to match DomainConfig struct), and the schema should be updated if the wrapper is not needed.

---

## Migration Path

### Phase 1: Convert Domain Config File (File Change)

**Scope**: Convert `/workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.yaml` → `domain.json`

**Action**:
1. Convert YAML to JSON (preserving all structure)
2. Add description fields from YAML comments
3. Place at: `/workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.json`
4. Keep domain.yaml temporarily for reference

**Example Output**:
```json
{
  "id": "indoor-air-quality",
  "description": "Maintain healthy indoor air quality",
  "streams": [
    {
      "stream_id": "air-quality",
      "alias": "indoor",
      "role": "primary"
    },
    {
      "stream_id": "outdoor-weather",
      "alias": "outdoor",
      "role": "context"
    }
  ],
  "alignment": {
    "view_name": "indoor_air_quality_aligned",
    "granularity": "1 hour",
    "join_strategy": "full_outer"
  },
  "objectives": [
    {
      "id": "healthy_co2",
      "description": "Keep CO2 below 800 ppm for cognitive performance",
      "target": {
        "stream": "air-quality",
        "metric": "co2",
        "condition": "<",
        "threshold": 800,
        "unit": "ppm"
      },
      "priority": "high"
    }
  ]
}
```

**Risks**: None (file change only)

### Phase 2: Update Loader (Code Change)

**Scope**: Update `FileSystemConfigLoader::load_domain_config()` to read `.json` instead of `.yaml`

**File**: `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/loader.rs`

**Changes**:
1. **Line 46**: Change `".join("domain.yaml")` → `.join("domain.json")`
2. **Line 80**: Change `serde_yaml::from_str()` → `serde_json::from_str()`
3. Update error messages if needed

**Before**:
```rust
fn domain_config_path(&self, domain_id: &str) -> PathBuf {
    self.config_dir
        .join("domains")
        .join(domain_id)
        .join("domain.yaml")  // CHANGE THIS
}

fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig> {
    let path = self.domain_config_path(domain_id);
    if !path.exists() {
        return Err(GoldDdlError::ConfigNotFound { path: path.display().to_string() });
    }
    let content = std::fs::read_to_string(&path)?;
    let config: DomainConfig =
        serde_yaml::from_str(&content)  // CHANGE THIS
            .map_err(|e| GoldDdlError::ConfigParseError { ... })?;
    Ok(config)
}
```

**After**:
```rust
fn domain_config_path(&self, domain_id: &str) -> PathBuf {
    self.config_dir
        .join("domains")
        .join(domain_id)
        .join("domain.json")  // JSON
}

fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig> {
    let path = self.domain_config_path(domain_id);
    if !path.exists() {
        return Err(GoldDdlError::ConfigNotFound { path: path.display().to_string() });
    }
    let content = std::fs::read_to_string(&path)?;
    let config: DomainConfig =
        serde_json::from_str(&content)  // JSON
            .map_err(|e| GoldDdlError::ConfigParseError { ... })?;
    Ok(config)
}
```

**Risks**:
- None if only production code affected
- Tests must be updated (Phase 3)

### Phase 3: Update Tests (Code Change)

**Scope**: Update hardcoded YAML strings in tests to JSON

**Files Affected**:
1. `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/domain.rs` (lines 310-371)
   - 3 tests using inline YAML

**Changes**:
1. **Line 311-329**: `test_domain_config_deserialize()` - convert YAML → JSON
2. **Line 342-350**: `test_stream_ref_with_null_handling_override()` - convert YAML → JSON
3. **Line 354-370**: `test_objective_config_deserialize()` - convert YAML → JSON
4. Keep `serde_yaml::from_str()` → `serde_json::from_str()`

**Before**:
```rust
#[test]
fn test_domain_config_deserialize() {
    let yaml = r#"
id: indoor-air-quality
description: Indoor air quality monitoring domain
streams:
  - stream_id: air-quality
    alias: indoor
    role: primary
...
"#;
    let config: DomainConfig = serde_yaml::from_str(yaml).unwrap();
    ...
}
```

**After**:
```rust
#[test]
fn test_domain_config_deserialize() {
    let json = r#"{
  "id": "indoor-air-quality",
  "description": "Indoor air quality monitoring domain",
  "streams": [
    {
      "stream_id": "air-quality",
      "alias": "indoor",
      "role": "primary"
    }
  ],
  "alignment": { ... }
}"#;
    let config: DomainConfig = serde_json::from_str(json).unwrap();
    ...
}
```

**Risks**: None (test changes only)

### Phase 4: Cleanup (Optional)

**Scope**: Remove unused `serde_yaml` dependency if not used elsewhere

**File**: `/workspaces/neural-data-platform/tools/ndp-gold-ddl/Cargo.toml`

**Action**:
1. Check if `serde_yaml` used anywhere else in ndp-gold-ddl
2. If not used: Remove `serde_yaml = "0.9"` from Cargo.toml line 19
3. Run `cargo build` to verify no breakage

**Findings**:
- Only domain config loading uses `serde_yaml` in ndp-gold-ddl
- Stream configs already use `serde_json`
- Safe to remove after verifying no other uses

**Risks**: Low (dependency removal, well-scoped)

---

## Complete File Modification List

| File | Change Type | Scope | Lines | Complexity |
|------|-------------|-------|-------|------------|
| `/config/domains/indoor-air-quality/domain.yaml` | Convert to JSON | Create new `.json` file | - | Low |
| `/config/domains/indoor-air-quality/domain.json` | CREATE | New domain config file | 110+ | Low |
| `tools/ndp-gold-ddl/src/config/loader.rs` | Code change | 2 small edits | 46, 80 | Low |
| `tools/ndp-gold-ddl/src/config/domain.rs` | Code change | 3 tests | 331, 349, 367 | Low |
| `tools/ndp-gold-ddl/Cargo.toml` | Dependency | Remove serde_yaml | 19 | Low |

---

## Risk Assessment

### Low-Risk Factors ✓

1. **Well-isolated change**
   - Only affects domain config subsystem
   - Stream configs already use JSON
   - No cross-module dependencies

2. **No behavioral changes**
   - DomainConfig struct unchanged
   - Deserialize logic identical (serde handles format)
   - File path change is internal

3. **Comprehensive test coverage**
   - 3 inline tests for domain parsing
   - Phase C fixtures (programmatic) don't need changes
   - Can verify with existing test suite

4. **Schema already exists**
   - JSON schema defined (domain.schema.json)
   - Validation can be added if needed

5. **Single domain in production**
   - Only `indoor-air-quality` domain exists
   - Migration scope limited to one file

### Risk Mitigation

| Risk | Mitigation |
|------|-----------|
| Parsing errors on migration | Validate JSON before deploying using `jq` or JSON Schema validator |
| Tests break | Run full test suite: `cargo test` |
| Dependency issues | Verify no other code uses serde_yaml before removal |
| File not found errors | Keep domain.yaml temporarily, test both paths work |

---

## Dependencies & Blockers

### Pre-Migration Requirements
- None (ADR-016-001 already approved)
- JSON schema (domain.schema.json) already exists

### Post-Migration Tasks
- None (clean migration)

### Related Issues
- **ADR-016-001**: Configuration Source of Truth (already approved)
- **dp-016 feature**: Declarative Deploy Architecture (parent feature)

---

## Testing Strategy

### Unit Tests (Existing)
```bash
# Run domain config tests
cargo test -p ndp-gold-ddl config::domain
```

**Coverage**:
- DomainConfig deserialization ✓
- StreamRef with null_handling override ✓
- ObjectiveConfig deserialization ✓

### Integration Tests (Phase C)
```bash
# Run Phase C aligned view tests
cargo test -p ndp-gold-ddl aligned_view
```

**Coverage**:
- Uses fixtures (no file reads)
- Fixtures construct structs programmatically
- No changes needed

### Manual Validation
```bash
# Test JSON schema validation
jq -f config/schemas/domain.schema.json config/domains/indoor-air-quality/domain.json

# Test ndp-gold-ddl can load domain
./ndp-gold-ddl validate --domain indoor-air-quality --config-dir ./config
```

---

## Effort Breakdown

| Task | Time | Notes |
|------|------|-------|
| Convert domain.yaml → domain.json | 30 min | Manual conversion + validation |
| Update loader.rs (2 lines) | 15 min | Straightforward code changes |
| Update domain.rs tests (3 tests) | 45 min | Convert YAML to JSON in test strings |
| Run full test suite | 10 min | Verify no breakage |
| Remove serde_yaml dependency | 10 min | Check for other uses, update Cargo.toml |
| Documentation + review | 30 min | Update this analysis, prepare PR notes |
| **TOTAL** | **2.5 hours** | |

**With contingency** (QA, review cycles): **4-6 hours**

---

## Success Criteria

### Pre-Deployment
- [ ] domain.json created with valid JSON (passes `jq .` and JSON schema validation)
- [ ] All tests pass: `cargo test -p ndp-gold-ddl`
- [ ] Loader correctly reads domain.json instead of domain.yaml
- [ ] No serde_yaml dependency left in Cargo.toml

### Post-Deployment
- [ ] ndp-gold-ddl validate command works with JSON config
- [ ] ndp-gold-ddl generate --domain indoor-air-quality works
- [ ] No runtime errors parsing domain config
- [ ] Existing stream configs still work (no regression)

### Standards Compliance
- [ ] ADR-016-001 compliance verified (JSON as source of truth)
- [ ] Configuration follows schema definition
- [ ] Consistent with stream config format (both now JSON)

---

## Implementation Notes

### JSON Conversion Gotchas

1. **YAML comments → JSON descriptions**
   - Comments in YAML are preserved in the domain.yaml comments
   - Convert these to `description` fields in JSON
   - Example:
     ```yaml
     # NULL handling: preserve (observation stream - default)
     ```
     Becomes (in description field):
     ```json
     "description": "NULL handling: preserve (observation stream - default)"
     ```

2. **Flat vs. Wrapped Structure**
   - Current domain.yaml is flat (no `domain:` wrapper)
   - DomainConfig struct expects flat structure
   - Keep it flat in domain.json (don't wrap)

3. **String vs. Number Types**
   - Objective thresholds should be numbers, not strings
   - Example: `"threshold": 800` not `"threshold": "800"`

4. **Enum Serialization**
   - Role, priority, join_strategy are enums in Rust
   - Serialize as snake_case strings: `"role": "primary"` ✓
   - serde already handles this with `#[serde(rename_all = "snake_case")]`

### Validation Before Commit

```bash
# 1. Validate JSON syntax
jq . config/domains/indoor-air-quality/domain.json

# 2. Validate against schema
jq --arg domainFile "$(cat config/domains/indoor-air-quality/domain.json)" \
   '.domain = ($domainFile | fromjson) | . as $input |
    . as $schema | $input | . == ($schema | @base64d | fromjson)' \
   config/schemas/domain.schema.json

# Or simpler: just parse it
cargo test -p ndp-gold-ddl config::domain::tests::test_domain_config_deserialize
```

---

## References

- **ADR-016-001**: Configuration Source of Truth
  - Location: `/workspaces/neural-data-platform/product/features/dp-016/architecture/ADR-016-001-config-source-of-truth.md`
  - Mandates: JSON as platform-wide configuration format

- **Current Domain Config**:
  - Location: `/workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.yaml`
  - Lines: 107 (well-structured, comprehensive)

- **Domain Schema**:
  - Location: `/workspaces/neural-data-platform/config/schemas/domain.schema.json`
  - Status: Comprehensive, production-ready

- **Loader Implementation**:
  - Location: `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/loader.rs`
  - Lines: 42-47 (path), 69-85 (loading logic)

---

## Approval Checklist

- [ ] Scope analysis reviewed and approved
- [ ] Risk assessment acceptable
- [ ] Testing strategy sufficient
- [ ] No blockers identified
- [ ] Ready for implementation phase

---

## Next Steps

1. **Immediate**: Use this analysis to schedule work
2. **Phase 1**: Create domain.json from domain.yaml
3. **Phase 2**: Update loader.rs to use serde_json
4. **Phase 3**: Update tests
5. **Phase 4**: Remove serde_yaml dependency
6. **Validation**: Run full test suite + manual verification
7. **Deployment**: Include in next release with ADR-016-001 compliance note

