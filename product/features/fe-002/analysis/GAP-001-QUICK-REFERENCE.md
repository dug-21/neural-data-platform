# GAP-001: Quick Reference Summary

**Issue**: GitHub #11 - Domain config uses YAML instead of JSON (ADR-016-001 violation)

**Status**: ANALYSIS COMPLETE - Ready for Implementation

---

## At-a-Glance

| Aspect | Detail |
|--------|--------|
| **Complexity** | **LOW** |
| **Effort** | **4-6 hours** |
| **Risk** | **LOW** |
| **Files Changed** | 4 |
| **Production Impact** | None (config format only) |
| **Breaking Changes** | None |
| **Tests Affected** | 3 inline tests |
| **Dependencies** | 1 (serde_yaml - for removal) |

---

## What Needs to Change

### 1. Data File (1 file)
```
config/domains/indoor-air-quality/domain.yaml
↓
config/domains/indoor-air-quality/domain.json (NEW)
```

### 2. Rust Code (2 code changes in 1 file)
```
tools/ndp-gold-ddl/src/config/loader.rs
  Line 46:  "domain.yaml" → "domain.json"
  Line 80:  serde_yaml::from_str() → serde_json::from_str()
```

### 3. Tests (3 tests in 1 file)
```
tools/ndp-gold-ddl/src/config/domain.rs
  Line 331:  YAML test string → JSON test string
  Line 349:  YAML test string → JSON test string
  Line 367:  YAML test string → JSON test string
```

### 4. Dependencies (optional)
```
tools/ndp-gold-ddl/Cargo.toml
  Line 19:  Remove: serde_yaml = "0.9"
```

---

## Why This Change

**ADR-016-001 Requirement**: JSON is the platform-wide configuration format

| Benefit | Impact |
|---------|--------|
| **Agent Reliability** | No indentation errors in generated config |
| **MCP-native** | MCP speaks JSON natively |
| **Consistency** | Stream configs already JSON ✓ |
| **Schema Validation** | JSON Schema tooling is mature |
| **Tooling** | jq, JSONPath, IDE support |

---

## Implementation Phases

### Phase 1: Convert File (30 min)
- Convert domain.yaml → domain.json
- Use jq to validate JSON syntax
- Keep domain.yaml temporarily for reference

### Phase 2: Update Code (15 min)
- Edit loader.rs: 2 one-line changes
- Change serde_yaml to serde_json

### Phase 3: Update Tests (45 min)
- 3 inline test strings: YAML → JSON
- Keep assertions identical

### Phase 4: Cleanup (10 min)
- Remove serde_yaml from Cargo.toml
- Verify no other uses

### Phase 5: Validation (20 min)
- Run full test suite
- Manual smoke tests
- Regression testing

---

## Key Files Analyzed

| File | Lines | Purpose |
|------|-------|---------|
| loader.rs | 42-47, 69-85 | Config loading logic |
| domain.rs | 310-371 | Domain config types + tests |
| Cargo.toml | 19 | serde_yaml dependency |
| domain.yaml | 107 | Config to convert |
| domain.schema.json | - | Validation (already JSON) |

---

## Risk Mitigation

| Risk | Mitigation |
|------|-----------|
| Parse errors | Validate JSON before commit |
| Tests break | Full test suite: `cargo test` |
| Dependency issues | Grep for serde_yaml before removal |
| Behavioral change | Format change only, no logic changes |

---

## Success Criteria (Checklist)

- [ ] domain.json created with valid JSON
- [ ] `jq . domain.json` passes
- [ ] All tests pass: `cargo test -p ndp-gold-ddl`
- [ ] No serde_yaml references in code
- [ ] ndp-gold-ddl validate command works
- [ ] ndp-gold-ddl generate --domain works
- [ ] Stream configs still work (regression test)

---

## Testing Commands

```bash
# Unit tests
cargo test -p ndp-gold-ddl config::domain

# Integration tests
cargo test -p ndp-gold-ddl aligned_view

# Full suite
cargo test -p ndp-gold-ddl

# Manual validation
jq . config/domains/indoor-air-quality/domain.json
cargo run -p ndp-gold-ddl -- validate --domain indoor-air-quality --config-dir ./config
```

---

## Before/After Comparison

### Loader Code
```diff
- fn domain_config_path(...) -> PathBuf {
-   ...join("domain.yaml")
- }
+ fn domain_config_path(...) -> PathBuf {
+   ...join("domain.json")
+ }

- serde_yaml::from_str(&content)
+ serde_json::from_str(&content)
```

### Test String (Sample)
```diff
- let yaml = r#"
- id: indoor-air-quality
- streams:
-   - stream_id: air-quality
- "#;
- let config: DomainConfig = serde_yaml::from_str(yaml).unwrap();

+ let json = r#"{
+   "id": "indoor-air-quality",
+   "streams": [
+     {"stream_id": "air-quality", ...}
+   ]
+ }"#;
+ let config: DomainConfig = serde_json::from_str(json).unwrap();
```

---

## Post-Implementation Verification

**One-time checks after deployment:**

```bash
# Verify JSON is valid
jq . config/domains/indoor-air-quality/domain.json > /dev/null && echo "✓ Valid JSON"

# Verify no YAML parser left
! grep -r "serde_yaml" tools/ndp-gold-ddl/ && echo "✓ No serde_yaml references"

# Verify tool works
ndp-gold-ddl validate --domain indoor-air-quality && echo "✓ Tool works with JSON"

# Verify no regressions
cargo test -p ndp-gold-ddl && echo "✓ All tests pass"
```

---

## Related Documentation

- **Full Analysis**: `GAP-001-YAML-to-JSON-scope-analysis.md`
- **Implementation Details**: `GAP-001-IMPLEMENTATION-DETAILS.md`
- **ADR-016-001**: `/product/features/dp-016/architecture/ADR-016-001-config-source-of-truth.md`
- **Domain Schema**: `/config/schemas/domain.schema.json`
- **Current Config**: `/config/domains/indoor-air-quality/domain.yaml`

---

## Slack Message Template

```
GAP-001 Analysis Complete ✓

Domain config YAML→JSON migration is ready for implementation.

📊 Scope:
  • Complexity: LOW
  • Effort: 4-6 hours
  • Files: 4
  • Risk: LOW

📝 Changes:
  • Convert domain.yaml → domain.json
  • Update loader.rs: 2 lines
  • Update tests: 3 test strings
  • Remove serde_yaml dependency

✅ Analysis docs in product/features/dp-016/analysis/
  - GAP-001-YAML-to-JSON-scope-analysis.md (full)
  - GAP-001-IMPLEMENTATION-DETAILS.md (code changes)
  - GAP-001-QUICK-REFERENCE.md (this)

Related: #11, ADR-016-001, dp-016
```

---

## Notes for Implementation

1. **JSON Conversion Tool**
   ```bash
   # Convert YAML to JSON (if needed)
   python3 -c "import yaml, json, sys; print(json.dumps(yaml.safe_load(sys.stdin), indent=2))"
   ```

2. **Validation**
   ```bash
   # Comprehensive JSON validation
   jq 'def validate: true; validate' domain.json
   ```

3. **Schema Check**
   ```bash
   # If jsonschema tool is available
   jsonschema -i domain.json config/schemas/domain.schema.json
   ```

4. **Diff Inspection**
   ```bash
   # Before removal, inspect serde_yaml changes
   git diff tools/ndp-gold-ddl/Cargo.toml
   ```

---

## FAQ

**Q: Will this break existing deployments?**
A: No. This is a configuration format change, not a logic change. Domain configs are read at startup.

**Q: Do I need to handle both YAML and JSON?**
A: No. Migrate to JSON only. YAML file can be deleted after verification.

**Q: What if the JSON is invalid?**
A: Tests will fail. Validate JSON with `jq .` before commit.

**Q: Can I run just the domain tests?**
A: Yes: `cargo test -p ndp-gold-ddl config::domain`

**Q: Is serde_yaml used elsewhere?**
A: Check with: `grep -r "serde_yaml" tools/ndp-gold-ddl/`
Currently: Only domain config loading uses it.

---

**Analysis Document**: GAP-001-YAML-to-JSON-scope-analysis.md
**Implementation Guide**: GAP-001-IMPLEMENTATION-DETAILS.md
**Issue**: GitHub #11
**Related ADR**: ADR-016-001 (Configuration Source of Truth)
