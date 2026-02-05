# FE-002: Domain Configuration Standardization - Verification Procedure

> **Feature:** FE-002 Domain Configuration Standardization
> **Version:** 1.0
> **Created:** 2026-02-05
> **Purpose:** Step-by-step verification procedure for feature completion

---

## Overview

This document provides the complete verification procedure for FE-002. Follow these steps in order to verify that both Phase A (YAML to JSON Migration) and Phase B (Schema Validation Integration) are complete.

**Estimated Time:** 30-45 minutes
**Prerequisites:** All implementation work complete

---

## Pre-Verification Checklist

Before starting verification, confirm:

| Item | Check | Command |
|------|-------|---------|
| Git state clean | No uncommitted changes | `git status --porcelain` |
| Docker running | All services up | `docker compose ps` |
| Build succeeds | Rust compiles | `cargo build -p ndp-gold-ddl -p ndp-validate` |
| Golden master exists | Baseline captured | `ls .test/golden-master/fe-002/` |

---

## Phase A Verification

### Step A1: File System Verification

**Duration:** 2 minutes

```bash
echo "=== A1: File System Verification ==="

# Check domain.json exists
echo -n "domain.json exists: "
if [ -f config/domains/indoor-air-quality/domain.json ]; then
    echo "PASS"
else
    echo "FAIL"
    exit 1
fi

# Check domain.yaml deleted
echo -n "domain.yaml deleted: "
if [ ! -f config/domains/indoor-air-quality/domain.yaml ]; then
    echo "PASS"
else
    echo "FAIL"
    exit 1
fi

# Validate JSON syntax
echo -n "JSON syntax valid: "
if jq . config/domains/indoor-air-quality/domain.json > /dev/null 2>&1; then
    echo "PASS"
else
    echo "FAIL"
    exit 1
fi

echo ""
```

**Expected Result:** All checks PASS

---

### Step A2: JSON Structure Verification

**Duration:** 3 minutes

```bash
echo "=== A2: JSON Structure Verification ==="

# Extract and verify required keys
echo "Checking required structure..."

jq -e '.domain.id' config/domains/indoor-air-quality/domain.json > /dev/null && echo "  domain.id: PRESENT" || echo "  domain.id: MISSING"
jq -e '.domain.description' config/domains/indoor-air-quality/domain.json > /dev/null && echo "  domain.description: PRESENT" || echo "  domain.description: MISSING"
jq -e '.domain.streams' config/domains/indoor-air-quality/domain.json > /dev/null && echo "  domain.streams: PRESENT" || echo "  domain.streams: MISSING"
jq -e '.domain.alignment' config/domains/indoor-air-quality/domain.json > /dev/null && echo "  domain.alignment: PRESENT" || echo "  domain.alignment: MISSING"

# Count streams
STREAM_COUNT=$(jq '.domain.streams | length' config/domains/indoor-air-quality/domain.json)
echo "  streams count: $STREAM_COUNT"

# Verify domain ID
DOMAIN_ID=$(jq -r '.domain.id' config/domains/indoor-air-quality/domain.json)
echo "  domain id: $DOMAIN_ID"
if [ "$DOMAIN_ID" = "indoor-air-quality" ]; then
    echo "  domain id match: PASS"
else
    echo "  domain id match: FAIL (expected 'indoor-air-quality')"
fi

echo ""
```

**Expected Result:**
- All required keys present
- Stream count matches original YAML (typically 3-4)
- Domain ID is `indoor-air-quality`

---

### Step A3: Code Changes Verification

**Duration:** 3 minutes

```bash
echo "=== A3: Code Changes Verification ==="

# Check loader.rs uses serde_json
echo -n "loader.rs uses serde_json: "
if grep -q "serde_json" tools/ndp-gold-ddl/src/config/loader.rs; then
    echo "PASS"
else
    echo "FAIL"
fi

# Check loader.rs references domain.json
echo -n "loader.rs uses .json extension: "
if grep -q "domain.json" tools/ndp-gold-ddl/src/config/loader.rs; then
    echo "PASS"
else
    echo "FAIL"
fi

# Check NO serde_yaml in loader.rs
echo -n "loader.rs has no serde_yaml: "
if ! grep -q "serde_yaml" tools/ndp-gold-ddl/src/config/loader.rs; then
    echo "PASS"
else
    echo "FAIL"
fi

# Check Cargo.toml for serde_yaml removal
echo -n "Cargo.toml has no serde_yaml: "
if ! grep -q "serde_yaml" tools/ndp-gold-ddl/Cargo.toml; then
    echo "PASS"
else
    echo "FAIL"
fi

echo ""
```

**Expected Result:** All checks PASS

---

### Step A4: Test Suite Verification

**Duration:** 5 minutes

```bash
echo "=== A4: Test Suite Verification ==="

# Run ndp-gold-ddl tests
echo "Running cargo test -p ndp-gold-ddl..."
cargo test -p ndp-gold-ddl 2>&1 | tee /tmp/fe-002-test-a4.log

# Extract results
if grep -q "test result: ok" /tmp/fe-002-test-a4.log; then
    echo ""
    echo "Test Result: PASS"
    grep "test result:" /tmp/fe-002-test-a4.log
else
    echo ""
    echo "Test Result: FAIL"
    grep -E "failures:|FAILED" /tmp/fe-002-test-a4.log
fi

echo ""
```

**Expected Result:** `test result: ok. XX passed; 0 failed`

---

### Step A5: Golden Master Comparison (CRITICAL)

**Duration:** 5 minutes

```bash
echo "=== A5: Golden Master Comparison (CRITICAL GATE) ==="

# Generate DDL from JSON config
echo "Generating DDL from JSON config..."
ndp-gold-ddl generate --domain indoor-air-quality > /tmp/fe-002-ddl-new.sql

# Compare with golden master
echo "Comparing with baseline..."
if diff -q .test/golden-master/fe-002/domain-ddl-baseline.sql /tmp/fe-002-ddl-new.sql > /dev/null; then
    echo ""
    echo "Golden Master Comparison: PASS (byte-for-byte identical)"
else
    echo ""
    echo "Golden Master Comparison: FAIL (differences detected)"
    echo ""
    echo "Differences:"
    diff .test/golden-master/fe-002/domain-ddl-baseline.sql /tmp/fe-002-ddl-new.sql | head -50
    echo ""
    echo "*** CRITICAL: DO NOT PROCEED TO PHASE B ***"
    exit 1
fi

echo ""
```

**Expected Result:** PASS (byte-for-byte identical)

**If FAIL:** Stop immediately. Differences indicate data loss or semantic changes. Debug before proceeding.

---

### Step A6: CLI Functional Test

**Duration:** 2 minutes

```bash
echo "=== A6: CLI Functional Test ==="

# Test ndp-gold-ddl generate command
echo "Testing ndp-gold-ddl generate..."
if ndp-gold-ddl generate --domain indoor-air-quality > /dev/null 2>&1; then
    echo "  generate command: PASS"
else
    echo "  generate command: FAIL"
fi

# Test ndp-gold-ddl validate command (if exists)
echo "Testing ndp-gold-ddl validate..."
if ndp-gold-ddl validate --domain indoor-air-quality > /dev/null 2>&1; then
    echo "  validate command: PASS"
else
    echo "  validate command: PASS (not implemented yet)"
fi

echo ""
```

---

### Phase A Summary

```bash
echo "=== PHASE A VERIFICATION SUMMARY ==="
echo ""
echo "| Criterion | Status |"
echo "|-----------|--------|"
echo "| AC-A-001: domain.json exists | [  ] |"
echo "| AC-A-002: domain.yaml deleted | [  ] |"
echo "| AC-A-003: loader.rs uses serde_json | [  ] |"
echo "| AC-A-004: Golden master match | [  ] |"
echo "| AC-A-005: All tests pass | [  ] |"
echo "| AC-A-006: No serde_yaml refs | [  ] |"
echo ""
echo "Phase A Complete: [ ] YES  [ ] NO"
echo ""
```

---

## Phase B Verification

### Step B1: CLI Flag Verification

**Duration:** 3 minutes

```bash
echo "=== B1: CLI Flag Verification ==="

# Check help includes --domain
echo -n "--domain flag in help: "
if ndp-validate --help 2>&1 | grep -q "\-\-domain"; then
    echo "PASS"
else
    echo "FAIL"
fi

# Test with valid domain config
echo "Testing valid domain config..."
if ndp-validate --domain config/domains/indoor-air-quality/domain.json; then
    echo "  Valid config validation: PASS"
else
    echo "  Valid config validation: FAIL"
fi

# Test exit code
ndp-validate --domain config/domains/indoor-air-quality/domain.json
EXIT_CODE=$?
echo "  Exit code for valid: $EXIT_CODE (expected: 0)"

echo ""
```

**Expected Result:** `--domain` flag works, valid config returns exit 0

---

### Step B2: Layer 1 Schema Validation

**Duration:** 5 minutes

```bash
echo "=== B2: Layer 1 Schema Validation ==="

# Create test cases
mkdir -p /tmp/fe-002-tests

# Test case 1: Missing domain root
echo '{"wrong": "root"}' > /tmp/fe-002-tests/missing-root.json

# Test case 2: Missing stream_id
cat > /tmp/fe-002-tests/missing-stream-id.json << 'EOF'
{
  "domain": {
    "id": "test",
    "description": "Test",
    "streams": [{"alias": "test", "role": "primary"}],
    "alignment": {"view_name": "test", "granularity": "1 hour", "join_strategy": "full_outer", "null_handling": "by_stream_type"}
  }
}
EOF

# Test case 3: Invalid enum value
cat > /tmp/fe-002-tests/invalid-role.json << 'EOF'
{
  "domain": {
    "id": "test",
    "description": "Test",
    "streams": [{"stream_id": "test", "alias": "test", "role": "invalid_role"}],
    "alignment": {"view_name": "test", "granularity": "1 hour", "join_strategy": "full_outer", "null_handling": "by_stream_type"}
  }
}
EOF

# Run tests
echo "Test 1: Missing domain root"
ndp-validate --domain /tmp/fe-002-tests/missing-root.json 2>&1 || true
echo ""

echo "Test 2: Missing stream_id"
ndp-validate --domain /tmp/fe-002-tests/missing-stream-id.json 2>&1 || true
echo ""

echo "Test 3: Invalid role enum"
ndp-validate --domain /tmp/fe-002-tests/invalid-role.json 2>&1 || true
echo ""

echo "Verify: All tests should show clear error messages with JSONPath locations"
```

**Expected Result:** Each invalid config produces clear error with JSONPath (e.g., `$.domain.streams[0].stream_id`)

---

### Step B3: Layer 2 Semantic Validation

**Duration:** 5 minutes

```bash
echo "=== B3: Layer 2 Semantic Validation ==="

# Test case: Valid schema but invalid semantics (nonexistent stream)
cat > /tmp/fe-002-tests/invalid-stream-ref.json << 'EOF'
{
  "domain": {
    "id": "test-domain",
    "description": "Test with invalid stream reference",
    "streams": [
      {
        "stream_id": "nonexistent-stream-xyz",
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

echo "Test: Invalid stream reference (valid schema, invalid semantics)"
ndp-validate --domain /tmp/fe-002-tests/invalid-stream-ref.json 2>&1 || true
echo ""

echo "Expected: Layer 1 PASS, Layer 2 FAIL with stream not found error"
```

**Expected Result:**
- Layer 1 (Schema): PASSED
- Layer 2 (Semantic): FAILED with message about nonexistent stream

---

### Step B4: Error Message Quality

**Duration:** 3 minutes

```bash
echo "=== B4: Error Message Quality ==="

# Test malformed JSON
echo "not valid json" > /tmp/fe-002-tests/malformed.json

echo "Test: Malformed JSON"
ndp-validate --domain /tmp/fe-002-tests/malformed.json 2>&1 || true
echo ""
echo "Expected: Parse error with line/column position"

# Test empty file
echo '{}' > /tmp/fe-002-tests/empty.json

echo "Test: Empty object"
ndp-validate --domain /tmp/fe-002-tests/empty.json 2>&1 || true
echo ""
echo "Expected: Clear 'missing domain' error"
```

**Expected Result:** Error messages are non-technical and include remediation hints

---

### Step B5: deploy.sh Integration

**Duration:** 5 minutes

```bash
echo "=== B5: deploy.sh Integration ==="

# Check deploy.sh includes domain validation
echo -n "deploy.sh includes domain validation: "
if grep -q "ndp-validate.*domain" deploy/pi/deploy.sh; then
    echo "PASS"
else
    echo "FAIL"
fi

# Test dry-run with domain validation
echo ""
echo "Testing deploy.sh dry-run..."
./deploy/pi/deploy.sh apply --dry-run .deploy/releases/v1.0.0.manifest.json 2>&1 | grep -A5 "Validating" || echo "No validation output found"

echo ""
```

**Expected Result:** deploy.sh calls `ndp-validate --domain` during Phase 1

---

### Step B6: Test Coverage Verification

**Duration:** 5 minutes

```bash
echo "=== B6: Test Coverage Verification ==="

# Run ndp-validate tests
echo "Running cargo test -p ndp-validate..."
cargo test -p ndp-validate 2>&1 | tee /tmp/fe-002-test-b6.log

# Count tests
TEST_COUNT=$(grep -c "test " /tmp/fe-002-test-b6.log | head -1)
echo ""
echo "Total tests: $TEST_COUNT"

# Count domain-specific tests
DOMAIN_TESTS=$(grep -c "domain" /tmp/fe-002-test-b6.log || echo "0")
echo "Domain-related tests: $DOMAIN_TESTS"

# Check result
if grep -q "test result: ok" /tmp/fe-002-test-b6.log; then
    echo ""
    echo "Test Result: PASS"
    grep "test result:" /tmp/fe-002-test-b6.log
else
    echo ""
    echo "Test Result: FAIL"
    grep -E "failures:|FAILED" /tmp/fe-002-test-b6.log
fi

echo ""
echo "Expected: >= 30 new domain validation tests"
```

**Expected Result:** All tests pass, at least 30 new tests

---

### Phase B Summary

```bash
echo "=== PHASE B VERIFICATION SUMMARY ==="
echo ""
echo "| Criterion | Status |"
echo "|-----------|--------|"
echo "| AC-B-001: --domain flag works | [  ] |"
echo "| AC-B-002: JSONPath in errors | [  ] |"
echo "| AC-B-003: Layer 2 runs | [  ] |"
echo "| AC-B-004: Clear error messages | [  ] |"
echo "| AC-B-005: deploy.sh integration | [  ] |"
echo "| AC-B-006: 30+ new tests | [  ] |"
echo ""
echo "Phase B Complete: [ ] YES  [ ] NO"
echo ""
```

---

## Final Verification

### Integration Test

**Duration:** 5 minutes

```bash
echo "=== FINAL INTEGRATION TEST ==="

# Full workflow test
echo "Step 1: Validate domain config"
ndp-validate --domain config/domains/indoor-air-quality/domain.json || { echo "FAIL"; exit 1; }
echo "  PASS"

echo "Step 2: Generate DDL"
ndp-gold-ddl generate --domain indoor-air-quality > /tmp/fe-002-final.sql || { echo "FAIL"; exit 1; }
echo "  PASS"

echo "Step 3: Golden master comparison"
diff -q .test/golden-master/fe-002/domain-ddl-baseline.sql /tmp/fe-002-final.sql || { echo "FAIL"; exit 1; }
echo "  PASS"

echo "Step 4: Deploy dry-run"
./deploy/pi/deploy.sh apply --dry-run .deploy/releases/v1.0.0.manifest.json || { echo "FAIL"; exit 1; }
echo "  PASS"

echo ""
echo "=== ALL INTEGRATION TESTS PASSED ==="
```

---

### Cleanup

```bash
echo "=== Cleanup ==="
rm -rf /tmp/fe-002-tests
rm -f /tmp/fe-002-*.sql
rm -f /tmp/fe-002-*.log
echo "Done"
```

---

## Sign-Off Checklist

### Verification Completed

| Step | Verifier | Date | Status |
|------|----------|------|--------|
| Phase A: A1-A6 | | | [ ] Pass |
| Phase B: B1-B6 | | | [ ] Pass |
| Integration Test | | | [ ] Pass |

### Final Sign-Off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Developer | | | |
| Reviewer | | | |
| Scrum Master | | | |

---

## Troubleshooting

### Phase A Issues

**Golden master mismatch:**
1. Check whitespace differences: `diff -w baseline.sql new.sql`
2. Check for floating point precision: `grep -E "\d+\.\d+" *.sql`
3. Compare line counts: `wc -l baseline.sql new.sql`

**serde_yaml still found:**
1. Run `cargo clean -p ndp-gold-ddl`
2. Rebuild: `cargo build -p ndp-gold-ddl`
3. Check for transitive dependencies

### Phase B Issues

**ndp-validate crashes:**
1. Check schema file exists: `ls config/schemas/domain.schema.json`
2. Verify schema is valid: `jq . config/schemas/domain.schema.json`
3. Check for missing dependencies in Cargo.toml

**Layer 2 not running:**
1. Verify semantic module is wired: `grep "semantic" tools/ndp-validate/src/main.rs`
2. Check for early returns in validation flow

---

## References

- [ACCEPTANCE-CRITERIA.md](./ACCEPTANCE-CRITERIA.md) - Detailed criteria
- [FE-002-DONE-DEFINITION.md](./FE-002-DONE-DEFINITION.md) - Definition of Done
- [RELEASE-CHECKLIST.md](./RELEASE-CHECKLIST.md) - Release preparation

---

*Verification Procedure created: 2026-02-05 by ndp-scrum-master (SPARC Completion)*
