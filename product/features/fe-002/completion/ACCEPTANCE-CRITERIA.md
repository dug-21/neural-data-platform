# FE-002: Domain Configuration Standardization - Acceptance Criteria

> **Feature:** FE-002 Domain Configuration Standardization
> **Version:** 1.0
> **Created:** 2026-02-05
> **Last Updated:** 2026-02-05

---

## Executive Summary

This document defines the detailed acceptance criteria for FE-002 Domain Configuration Standardization. The feature is considered DONE when both phases (A and B) pass all acceptance criteria with verified outputs matching expected results.

**Critical Gates:**
- **Phase A Gate:** DDL output from JSON config must be BYTE-FOR-BYTE identical to baseline from YAML config
- **Phase B Gate:** Domain validation produces clear, actionable error messages with JSONPath locations

---

## Phase A: YAML to JSON Migration (GAP-001)

### AC-A-001: domain.json Exists and is Valid JSON

**Description:** The domain configuration file has been converted from YAML to JSON format.

**Verification Command:**
```bash
# Check file exists
ls -la /workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.json

# Validate JSON syntax
jq . /workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.json > /dev/null && echo "VALID JSON" || echo "INVALID JSON"

# Pretty-print to verify structure
jq . /workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.json | head -20
```

**Expected Output:**
- File exists at `config/domains/indoor-air-quality/domain.json`
- `jq .` command exits with code 0
- JSON structure matches YAML structure semantically

**Pass Criteria:**
- [ ] File exists
- [ ] File is valid JSON (jq parse succeeds)
- [ ] Contains `domain` root object with `id`, `description`, `streams`, `alignment`, `objectives` keys

---

### AC-A-002: domain.yaml Has Been Deleted

**Description:** The original YAML file has been removed to enforce single source of truth.

**Verification Command:**
```bash
# Check YAML file does NOT exist
ls /workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.yaml 2>&1
```

**Expected Output:**
```
ls: cannot access '/workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.yaml': No such file or directory
```

**Pass Criteria:**
- [ ] File `domain.yaml` does not exist
- [ ] No other `.yaml` files in domain directory

---

### AC-A-003: loader.rs Uses serde_json

**Description:** The domain configuration loader has been updated to use JSON parser.

**Verification Command:**
```bash
# Check for serde_json usage
grep -n "serde_json" /workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/loader.rs

# Check path uses .json extension
grep -n "domain.json" /workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/loader.rs

# Verify NO serde_yaml usage
grep -c "serde_yaml" /workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/loader.rs
```

**Expected Output:**
- `serde_json::from_str` or `serde_json::from_reader` call present
- Path references `domain.json` not `domain.yaml`
- Zero occurrences of `serde_yaml`

**Pass Criteria:**
- [ ] Line ~80 uses `serde_json::from_str` for domain parsing
- [ ] Lines ~46-47 reference `domain.json` extension
- [ ] `grep -c "serde_yaml"` returns `0`

---

### AC-A-004: All DDL Outputs Match Baseline (Golden Master)

**Description:** DDL generated from JSON config produces byte-for-byte identical output to baseline captured from YAML config.

**Pre-Requisite:** Capture golden master BEFORE migration:
```bash
# Run BEFORE Phase A implementation - captures baseline
mkdir -p /workspaces/neural-data-platform/.test/golden-master/fe-002
ndp-gold-ddl generate --domain indoor-air-quality > /workspaces/neural-data-platform/.test/golden-master/fe-002/domain-ddl-baseline.sql
```

**Verification Command:**
```bash
# Generate DDL from JSON config
ndp-gold-ddl generate --domain indoor-air-quality > /tmp/domain-ddl-new.sql

# Compare with golden master
diff /workspaces/neural-data-platform/.test/golden-master/fe-002/domain-ddl-baseline.sql /tmp/domain-ddl-new.sql

# If diff produces output, test FAILED
```

**Expected Output:**
```
(empty - no differences)
```

**Pass Criteria:**
- [ ] `diff` command produces empty output
- [ ] Exit code is 0
- [ ] Generated SQL is identical character-by-character

**Failure Action:** If any differences exist, investigate and fix before proceeding to Phase B.

---

### AC-A-005: All Existing Tests Pass

**Description:** The full ndp-gold-ddl test suite passes without modification to test logic.

**Verification Command:**
```bash
# Run all tests
cargo test -p ndp-gold-ddl

# Capture test count
cargo test -p ndp-gold-ddl 2>&1 | grep -E "test result|passed|failed"
```

**Expected Output:**
```
test result: ok. XX passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Pass Criteria:**
- [ ] All tests pass (0 failed)
- [ ] No tests ignored due to migration issues
- [ ] Test count unchanged from pre-migration

---

### AC-A-006: No serde_yaml References Remain

**Description:** The serde_yaml dependency has been completely removed from ndp-gold-ddl.

**Verification Command:**
```bash
# Check Cargo.toml
grep "serde_yaml" /workspaces/neural-data-platform/tools/ndp-gold-ddl/Cargo.toml

# Check all source files
grep -r "serde_yaml" /workspaces/neural-data-platform/tools/ndp-gold-ddl/src/

# Check Cargo.lock for transitive dependency (informational)
grep "serde_yaml" /workspaces/neural-data-platform/Cargo.lock | head -5
```

**Expected Output:**
- Cargo.toml grep: no output
- Source grep: no output
- Cargo.lock: may show serde_yaml if other crates use it (acceptable)

**Pass Criteria:**
- [ ] `grep "serde_yaml" Cargo.toml` returns empty
- [ ] `grep -r "serde_yaml" src/` returns empty
- [ ] `cargo build -p ndp-gold-ddl` succeeds

---

## Phase B: Schema Validation Integration (GAP-003)

### AC-B-001: ndp-validate --domain Works

**Description:** The `--domain` flag validates a single domain configuration file.

**Verification Command:**
```bash
# Validate valid domain config
ndp-validate --domain /workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.json

# Check exit code
echo "Exit code: $?"
```

**Expected Output:**
```
Validating domain config: config/domains/indoor-air-quality/domain.json
  Layer 1 (Schema): PASSED
  Layer 2 (Semantic): PASSED
Result: VALID

Exit code: 0
```

**Pass Criteria:**
- [ ] Command accepts `--domain` flag
- [ ] Reports Layer 1 and Layer 2 validation
- [ ] Returns exit code 0 for valid config
- [ ] Outputs "VALID" or equivalent success message

---

### AC-B-002: Layer 1 Errors Show JSONPath Locations

**Description:** Schema validation errors include precise JSONPath locations for debugging.

**Verification Command:**
```bash
# Create test file with invalid schema
cat > /tmp/invalid-domain.json << 'EOF'
{
  "domain": {
    "id": "test",
    "streams": [
      {
        "invalid_field": "not allowed"
      }
    ]
  }
}
EOF

# Validate and capture error
ndp-validate --domain /tmp/invalid-domain.json 2>&1 || true
```

**Expected Output:**
```
Validating domain config: /tmp/invalid-domain.json
  Layer 1 (Schema): FAILED
    - $.domain.streams[0]: missing required property 'stream_id'
    - $.domain.streams[0]: additional property 'invalid_field' not allowed
Result: INVALID

Exit code: 1
```

**Pass Criteria:**
- [ ] Error messages include JSONPath (e.g., `$.domain.streams[0]`)
- [ ] Missing required fields are identified
- [ ] Additional properties are flagged
- [ ] Line/character positions included if possible

---

### AC-B-003: Layer 2 Semantic Validation Runs

**Description:** Semantic validation runs after Layer 1 passes, catching logical errors.

**Verification Command:**
```bash
# Create test file with valid schema but invalid semantics
cat > /tmp/semantic-error-domain.json << 'EOF'
{
  "domain": {
    "id": "test-domain",
    "description": "Test domain",
    "streams": [
      {
        "stream_id": "nonexistent-stream",
        "alias": "test",
        "role": "primary"
      }
    ],
    "alignment": {
      "view_name": "test_aligned",
      "granularity": "1 hour",
      "join_strategy": "full_outer",
      "null_handling": "by_stream_type"
    }
  }
}
EOF

# Validate
ndp-validate --domain /tmp/semantic-error-domain.json 2>&1 || true
```

**Expected Output:**
```
Validating domain config: /tmp/semantic-error-domain.json
  Layer 1 (Schema): PASSED
  Layer 2 (Semantic): FAILED
    - Stream 'nonexistent-stream' not found in stream registry
Result: INVALID

Exit code: 1
```

**Pass Criteria:**
- [ ] Layer 1 passes before Layer 2 runs
- [ ] Semantic errors are clearly differentiated
- [ ] Reference validation catches invalid stream_ids
- [ ] Cross-reference validation catches inconsistencies

---

### AC-B-004: Invalid Configs Produce Clear Errors

**Description:** All validation errors are actionable with clear remediation guidance.

**Verification Commands:**
```bash
# Test 1: Empty file
echo '{}' | ndp-validate --domain /dev/stdin 2>&1 || true

# Test 2: Malformed JSON
echo 'not json' > /tmp/bad.json && ndp-validate --domain /tmp/bad.json 2>&1 || true

# Test 3: Wrong root structure
echo '{"wrong": "structure"}' | ndp-validate --domain /dev/stdin 2>&1 || true
```

**Expected Outputs:**

Test 1 (Empty):
```
Error: Missing required property 'domain' at root ($)
```

Test 2 (Malformed):
```
Error: Failed to parse JSON: expected value at line 1 column 1
```

Test 3 (Wrong structure):
```
Error: Missing required property 'domain' at root ($)
       Additional property 'wrong' not allowed
```

**Pass Criteria:**
- [ ] Empty files produce clear "missing required" error
- [ ] Malformed JSON shows parse error with location
- [ ] Wrong structure identifies missing and extra properties
- [ ] All errors are non-technical and actionable

---

### AC-B-005: deploy.sh Validates Domains Before Deployment

**Description:** The deployment script validates domain configs before applying changes.

**Verification Command:**
```bash
# Check deploy.sh includes domain validation
grep -n "ndp-validate.*domain" /workspaces/neural-data-platform/deploy/pi/deploy.sh

# Test with dry-run
./deploy/pi/deploy.sh apply --dry-run --domain indoor-air-quality 2>&1 | head -20
```

**Expected Output:**
```
Phase 1: Validating configuration...
  - Validating domain: indoor-air-quality... OK
```

**Pass Criteria:**
- [ ] `deploy.sh` calls `ndp-validate --domain` during Phase 1
- [ ] Validation failure prevents deployment
- [ ] Validation success continues to next phase

---

### AC-B-006: 30+ New Tests Pass

**Description:** Comprehensive test coverage for domain validation functionality.

**Verification Command:**
```bash
# Run ndp-validate tests
cargo test -p ndp-validate

# Count domain-related tests
cargo test -p ndp-validate 2>&1 | grep -c "domain"

# Get test summary
cargo test -p ndp-validate 2>&1 | grep -E "test result|passed|failed"
```

**Expected Output:**
```
test result: ok. XX passed; 0 failed; 0 ignored
```
Where XX >= 30 new tests plus existing tests.

**Pass Criteria:**
- [ ] All tests pass
- [ ] At least 30 new domain validation tests
- [ ] Tests cover:
  - [ ] Layer 1 schema validation (10+ tests)
  - [ ] Layer 2 semantic validation (10+ tests)
  - [ ] CLI flag parsing (5+ tests)
  - [ ] Error message formatting (5+ tests)

---

## Test Categories Required

### Layer 1 (Schema) Tests

| Test | Description |
|------|-------------|
| `test_valid_minimal_domain` | Minimal valid domain config passes |
| `test_valid_full_domain` | Full domain config with all optional fields passes |
| `test_missing_domain_root` | Error when `domain` key missing |
| `test_missing_stream_id` | Error when stream missing `stream_id` |
| `test_missing_stream_alias` | Error when stream missing `alias` |
| `test_missing_stream_role` | Error when stream missing `role` |
| `test_invalid_role_value` | Error when role not in enum |
| `test_invalid_granularity` | Error when granularity format wrong |
| `test_invalid_join_strategy` | Error when join_strategy not in enum |
| `test_additional_properties` | Error when unknown properties present |

### Layer 2 (Semantic) Tests

| Test | Description |
|------|-------------|
| `test_stream_exists` | Validates stream_id references existing stream |
| `test_stream_not_exists` | Error when stream_id not found |
| `test_duplicate_aliases` | Error when two streams have same alias |
| `test_primary_role_exists` | Warning if no primary role stream |
| `test_alignment_streams_match` | All streams in alignment exist in streams array |
| `test_objectives_reference_valid` | Objective metrics reference valid streams |
| `test_valid_interval_format` | Granularity interval is valid PostgreSQL interval |
| `test_view_name_valid_identifier` | view_name is valid SQL identifier |
| `test_gold_etl_enabled_for_streams` | Streams have gold_etl if used in domain |
| `test_domain_id_matches_directory` | Domain ID matches directory name |

### CLI Tests

| Test | Description |
|------|-------------|
| `test_domain_flag_exists` | `--domain` flag is recognized |
| `test_domain_flag_with_path` | `--domain <path>` validates file |
| `test_domain_flag_with_all` | `--all --domain` validates all domains |
| `test_schema_only_flag` | `--schema-only` skips Layer 2 |
| `test_exit_code_success` | Exit 0 on valid config |
| `test_exit_code_failure` | Exit 1 on invalid config |

---

## Integration Verification

### Full Stack Test

**Verification Command:**
```bash
# End-to-end: config -> validate -> generate -> deploy
./scripts/fe-002-integration-test.sh
```

**Test Script:**
```bash
#!/bin/bash
set -e

echo "=== FE-002 Integration Test ==="

# 1. Validate domain config
echo "Step 1: Validating domain config..."
ndp-validate --domain config/domains/indoor-air-quality/domain.json
echo "  OK"

# 2. Generate DDL
echo "Step 2: Generating DDL..."
ndp-gold-ddl generate --domain indoor-air-quality > /tmp/fe-002-ddl.sql
echo "  OK"

# 3. Compare with golden master
echo "Step 3: Comparing with golden master..."
diff .test/golden-master/fe-002/domain-ddl-baseline.sql /tmp/fe-002-ddl.sql
echo "  OK (byte-for-byte match)"

# 4. Deploy dry-run
echo "Step 4: Deploy dry-run..."
./deploy/pi/deploy.sh apply --dry-run --domain indoor-air-quality
echo "  OK"

echo ""
echo "=== All Integration Tests PASSED ==="
```

---

## Summary Checklist

### Phase A Completion

- [ ] AC-A-001: domain.json exists and is valid JSON
- [ ] AC-A-002: domain.yaml deleted
- [ ] AC-A-003: loader.rs uses serde_json
- [ ] AC-A-004: DDL matches golden master (CRITICAL GATE)
- [ ] AC-A-005: All existing tests pass
- [ ] AC-A-006: No serde_yaml references

### Phase B Completion

- [ ] AC-B-001: `ndp-validate --domain` works
- [ ] AC-B-002: Layer 1 errors show JSONPath
- [ ] AC-B-003: Layer 2 semantic validation runs
- [ ] AC-B-004: Clear, actionable error messages
- [ ] AC-B-005: deploy.sh validates domains
- [ ] AC-B-006: 30+ new tests pass

### Feature Complete

- [ ] All Phase A criteria met
- [ ] All Phase B criteria met
- [ ] GitHub Issue #11 closed
- [ ] GitHub Issue #13 closed
- [ ] STATUS.md updated to "done"

---

## References

- [SCOPE.md](../SCOPE.md) - Feature scope and requirements
- [STATUS.md](../STATUS.md) - Current progress tracking
- [FE-001-DONE-DEFINITION.md](../../fe-001/completion/FE-001-DONE-DEFINITION.md) - Pattern reference
- [RELEASE-POLICY.md](../../../../docs/procedures/RELEASE-POLICY.md) - Release requirements

---

*Acceptance Criteria created: 2026-02-05 by ndp-scrum-master (SPARC Completion)*
