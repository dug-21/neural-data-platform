# SPEC-B01: Schema Validation Integration

> **Feature:** FE-002 Domain Configuration Standardization
> **Phase:** B - Schema Validation Integration (GAP-003)
> **Version:** 1.0
> **Status:** Draft
> **Created:** 2026-02-05
> **Depends On:** SPEC-A01 (Phase A must complete first)

---

## 1. Introduction

### 1.1 Purpose

This specification defines requirements for integrating JSON Schema validation into the domain configuration pipeline, resolving GAP-003 (dp-019 two-layer validation violation).

### 1.2 Scope

- Add `--domain` flag to `ndp-validate` CLI
- Implement Layer 1 (Schema) validation for domain configs
- Connect existing Layer 2 (Semantic) validation to CLI
- Integrate domain validation into `deploy.sh` workflow

### 1.3 Background

The dp-019 specification defines two-layer validation:

| Layer | Purpose | Implementation |
|-------|---------|----------------|
| **Layer 1** | Structural validation | JSON Schema |
| **Layer 2** | Business rule validation | Rust semantic checks |

Stream configurations already implement both layers. Domain configurations only have Layer 2 (semantic validation in `semantic/domain.rs`). This phase adds Layer 1.

---

## 2. Functional Requirements

### 2.1 CLI Interface

#### REQ-B01-001: Add --domain Flag

**Description:** Add a `--domain` flag to `ndp-validate` CLI for validating domain configuration files.

**CLI Syntax:**
```bash
# Validate single domain config
ndp-validate --domain config/domains/indoor-air-quality/domain.json

# Validate all domain configs
ndp-validate --all --domain

# Schema-only validation (Layer 1 only)
ndp-validate --schema-only --domain config/domains/indoor-air-quality/domain.json
```

**Acceptance Criteria:**
- AC1: `--domain <path>` validates a single domain config file
- AC2: `--all --domain` discovers and validates all domain configs in `config/domains/*/domain.json`
- AC3: `--domain` is mutually exclusive with positional config path (for stream configs)
- AC4: Help text explains domain validation mode

**Verification Method:** CLI testing, `--help` inspection

**Priority:** High

---

#### REQ-B01-002: Domain Config Discovery

**Description:** When using `--all --domain`, automatically discover all domain configurations.

**Discovery Path:** `{config_dir}/domains/*/domain.json`

**Acceptance Criteria:**
- AC1: Discovers all `domain.json` files in domain subdirectories
- AC2: Reports count of configs found
- AC3: Continues validation if some configs fail (reports all errors)
- AC4: Returns appropriate exit code (0=all pass, 1=any fail)

**Verification Method:** Integration test with multiple domain configs

**Priority:** Medium

---

#### REQ-B01-003: Config Type Detection

**Description:** Automatically detect whether a config file is a stream config or domain config based on content.

**Detection Logic:**
```rust
// If file contains "stream_id" at root level -> stream config
// If file contains "streams" array with stream references -> domain config
// If file contains "domain" wrapper -> domain config (wrapped format)
```

**Acceptance Criteria:**
- AC1: Correctly identifies domain configs by structure
- AC2: Correctly identifies stream configs by structure
- AC3: Provides clear error if config type cannot be determined

**Verification Method:** Unit tests with various config formats

**Priority:** Low (Nice-to-have for future `--auto` mode)

---

### 2.2 Layer 1 (Schema) Validation

#### REQ-B01-004: Load Domain Schema

**Description:** Load `domain.schema.json` for Layer 1 validation.

**Schema Location:** `config/schemas/domain.schema.json`

**Acceptance Criteria:**
- AC1: Schema loads successfully from default path
- AC2: Schema path can be overridden with `--domain-schema <path>`
- AC3: Clear error if schema file not found
- AC4: Schema compilation errors reported clearly

**Verification Method:** Unit test, integration test

**Priority:** High

---

#### REQ-B01-005: Schema Validation Execution

**Description:** Validate domain config against JSON Schema.

**Acceptance Criteria:**
- AC1: Valid configs pass with no errors
- AC2: Invalid configs fail with descriptive errors
- AC3: Multiple schema errors collected and reported
- AC4: Validation completes even if early errors found

**Verification Method:** Unit tests with valid/invalid configs

**Priority:** High

---

#### REQ-B01-006: Schema Format Compatibility

**Description:** Handle the schema format discrepancy identified in SPEC-A01.

**Current State:**
- `domain.schema.json` expects wrapped format: `{ "domain": { ... } }`
- Phase A creates flat format: `{ "id": "...", "streams": [...] }`

**Options:**
1. **Update schema** to accept flat format (recommended)
2. **Wrap config** before validation
3. **Create new schema** `domain-flat.schema.json`

**Recommended Solution:** Update `domain.schema.json` to use `oneOf` for both formats:
```json
{
  "oneOf": [
    { "$ref": "#/definitions/domain_content" },
    {
      "type": "object",
      "required": ["domain"],
      "properties": {
        "domain": { "$ref": "#/definitions/domain_content" }
      }
    }
  ]
}
```

**Acceptance Criteria:**
- AC1: Flat format domain configs validate successfully
- AC2: Wrapped format domain configs still validate (backward compatibility)
- AC3: Schema change documented in ADR or decision log

**Verification Method:** Schema validation tests with both formats

**Priority:** High

---

### 2.3 Layer 2 (Semantic) Validation

#### REQ-B01-007: Wire Existing Semantic Validation

**Description:** Connect existing `semantic/domain.rs` validation to CLI flow.

**Existing Validation Rules:**
| Rule | Error Code | Description |
|------|------------|-------------|
| Stream exists | InvalidDomainStream | All referenced streams must exist |
| Valid role | InvalidDomainStream | Role must be primary/context/actuator/constraint |
| Unique aliases | DuplicateName | No duplicate aliases in domain |
| Has primary | InvalidDomainStream | At least one stream must be primary (warning) |
| Valid join strategy | InvalidDomainStream | full_outer/left/inner |
| Valid granularity | InvalidGranularity | Format: "N minute|hour|day(s)" |
| Valid condition | InvalidObjectiveCondition | <, >, <=, >=, ==, != |
| Objective stream exists | InvalidDomainStream | Target stream in domain |

**Acceptance Criteria:**
- AC1: All existing semantic rules execute via CLI
- AC2: Semantic validation runs AFTER schema validation passes
- AC3: Semantic errors include JSONPath locations
- AC4: `--schema-only` skips semantic validation

**Verification Method:** Integration tests

**Priority:** High

---

#### REQ-B01-008: Stream Existence Validation

**Description:** Validate that streams referenced in domain config exist.

**Validation Logic:**
1. Load list of available streams from `config/base/streams/*/config.json`
2. For each `stream_id` in domain config, verify it exists
3. Report missing streams with suggestions

**Acceptance Criteria:**
- AC1: Missing streams reported with error
- AC2: Error includes available stream list (truncated if long)
- AC3: Suggestion provided for typos (Levenshtein distance)

**Verification Method:** Unit test with missing stream

**Priority:** High

---

### 2.4 Error Message Format

#### REQ-B01-009: JSONPath Error Locations

**Description:** All errors must include JSONPath to the problematic field.

**Format Examples:**
```
$.streams[0].stream_id
$.alignment.granularity
$.objectives[2].target.condition
```

**Acceptance Criteria:**
- AC1: Schema errors include JSONPath from schema validator
- AC2: Semantic errors include manually constructed JSONPath
- AC3: Paths are consistent with stream config error format

**Verification Method:** Error message inspection

**Priority:** High

---

#### REQ-B01-010: Human-Readable Error Messages

**Description:** Error messages must be clear and actionable.

**Required Components:**
| Component | Description |
|-----------|-------------|
| Layer | `[Schema]` or `[Semantic]` |
| Path | JSONPath to field |
| Message | What is wrong |
| Suggestion | How to fix (when applicable) |

**Example Output (Human Format):**
```
[FAIL] config/domains/indoor-air-quality/domain.json

  ERRORS:
    [Schema] $.streams[0].stream_id
      Required field missing
      Suggestion: Add "stream_id" field to stream reference

    [Semantic] $.objectives[0].target.stream
      Stream 'air-qualitty' not found. Available streams: air-quality, outdoor-weather
      Suggestion: Did you mean 'air-quality'?
```

**Acceptance Criteria:**
- AC1: Human format shows layer, path, message, suggestion
- AC2: JSON format includes same fields in structured form
- AC3: Suggestions provided for common errors (typos, invalid enums)

**Verification Method:** Output inspection, user testing

**Priority:** Medium

---

#### REQ-B01-011: JSON Output Format

**Description:** JSON output must match existing stream validation format.

**Expected Structure:**
```json
{
  "valid": false,
  "config_path": "config/domains/indoor-air-quality/domain.json",
  "summary": {
    "total_errors": 2,
    "total_warnings": 1,
    "by_layer": {
      "schema": 1,
      "semantic": 1
    }
  },
  "errors": [
    {
      "layer": "Schema",
      "code": "MissingRequired",
      "path": "$.streams[0].stream_id",
      "message": "Required field missing",
      "severity": "Error",
      "suggestion": "Add \"stream_id\" field"
    }
  ],
  "warnings": []
}
```

**Acceptance Criteria:**
- AC1: Structure matches `ValidationResult` from `cli.rs`
- AC2: Exit codes follow dp-019: 0=pass, 1=fail, 2=system error
- AC3: JSON is valid and parseable

**Verification Method:** JSON schema validation of output

**Priority:** High

---

### 2.5 deploy.sh Integration

#### REQ-B01-012: Add Domain Validation to Deploy Workflow

**Description:** Add domain config validation to `deploy.sh` sync workflow.

**Integration Point:** Before DDL generation in sync command

**Workflow:**
```bash
# In deploy.sh sync command:
1. Validate stream configs (existing)
2. Validate domain configs (NEW)
3. Generate Silver DDL (existing)
4. Generate Gold DDL (existing)
5. Sync to etcd (existing)
```

**Acceptance Criteria:**
- AC1: Domain validation runs during `./deploy.sh sync`
- AC2: Deployment aborts if domain validation fails
- AC3: Clear error message indicates which domain failed
- AC4: Existing deployment workflow unchanged on success

**Verification Method:** Integration test

**Priority:** High

---

#### REQ-B01-013: Validation Command for Deploy Script

**Description:** Provide validation command for use in deploy scripts.

**Command:**
```bash
ndp-validate --all --domain --format json
```

**Exit Codes:**
| Code | Meaning | Deploy Action |
|------|---------|---------------|
| 0 | All valid | Continue |
| 1 | Validation failed | Abort with error |
| 2 | System error | Abort with error |

**Acceptance Criteria:**
- AC1: Command returns correct exit codes
- AC2: Output parseable by shell scripts
- AC3: Quick execution (< 2 seconds for typical configs)

**Verification Method:** Integration test in deploy.sh

**Priority:** High

---

## 3. Non-Functional Requirements

### 3.1 Performance

#### REQ-B01-014: Validation Performance

**Description:** Domain validation must complete quickly.

**Acceptance Criteria:**
- AC1: Single domain config validates in < 500ms
- AC2: All domain configs validate in < 2 seconds
- AC3: Schema compilation cached for repeated validations

**Verification Method:** Benchmark test

**Priority:** Low

---

### 3.2 Error Handling

#### REQ-B01-015: Graceful Error Handling

**Description:** Handle edge cases gracefully.

**Edge Cases:**
| Case | Expected Behavior |
|------|-------------------|
| File not found | Exit 2, "Config file not found: {path}" |
| Invalid JSON | Exit 1, syntax error with line/column |
| Schema not found | Exit 2, "Schema file not found: {path}" |
| No domain configs | Exit 0, "No domain configs found" (with --all) |
| Empty config | Exit 1, schema error for missing required fields |

**Acceptance Criteria:**
- AC1: All edge cases handled without panic
- AC2: Error messages are helpful
- AC3: Exit codes are correct

**Verification Method:** Unit tests for each edge case

**Priority:** Medium

---

### 3.3 IDE Integration

#### REQ-B01-016: JSON Schema IDE Support

**Description:** Enable IDE autocomplete and validation for domain configs.

**Method:** Add `$schema` reference to domain config files:
```json
{
  "$schema": "../../../schemas/domain.schema.json",
  "id": "indoor-air-quality",
  ...
}
```

**Acceptance Criteria:**
- AC1: VS Code shows autocomplete for domain config fields
- AC2: VS Code shows validation errors inline
- AC3: Schema reference uses relative path (works in repo)

**Verification Method:** Manual verification in VS Code

**Priority:** Low (Nice-to-have)

---

## 4. Test Requirements

### 4.1 Unit Test Coverage

#### REQ-B01-017: Layer 1 Validation Tests

**Test Cases:**

| Test ID | Description | Expected Result |
|---------|-------------|-----------------|
| L1-001 | Valid domain config | Pass, no errors |
| L1-002 | Missing required field (id) | Fail, MissingRequired |
| L1-003 | Invalid field type (streams not array) | Fail, InvalidType |
| L1-004 | Unknown field (additionalProperties) | Fail, UnknownField |
| L1-005 | Invalid enum value (role: "invalid") | Fail, InvalidEnum |
| L1-006 | Invalid pattern (id: "Invalid-ID") | Fail, PatternMismatch |
| L1-007 | Invalid granularity format | Fail, PatternMismatch |
| L1-008 | Empty streams array | Fail, minItems |
| L1-009 | Nested validation (target missing fields) | Fail, MissingRequired |
| L1-010 | Multiple errors | Report all errors |

**Acceptance Criteria:**
- AC1: All test cases implemented
- AC2: Tests use mocked schema where appropriate
- AC3: Tests verify error codes and paths

**Verification Method:** `cargo test -p ndp-validate`

**Priority:** High

---

#### REQ-B01-018: Layer 2 Validation Tests

**Test Cases:**

| Test ID | Description | Expected Result |
|---------|-------------|-----------------|
| L2-001 | Valid domain with existing streams | Pass |
| L2-002 | Stream reference not found | Fail, InvalidDomainStream |
| L2-003 | Duplicate aliases | Fail, DuplicateName |
| L2-004 | Missing primary role | Warning |
| L2-005 | Invalid join strategy | Fail, InvalidDomainStream |
| L2-006 | Invalid objective condition | Fail, InvalidObjectiveCondition |
| L2-007 | Objective references non-domain stream | Fail, InvalidDomainStream |
| L2-008 | Typo in stream name (suggestion) | Fail with suggestion |

**Acceptance Criteria:**
- AC1: All existing tests in `semantic/domain.rs` pass
- AC2: New CLI integration tests added
- AC3: Tests verify suggestion messages

**Verification Method:** `cargo test -p ndp-validate`

**Priority:** High

---

### 4.2 Integration Test Coverage

#### REQ-B01-019: CLI Integration Tests

**Test Cases:**

| Test ID | Description | Command |
|---------|-------------|---------|
| CLI-001 | Validate single valid config | `ndp-validate --domain valid.json` |
| CLI-002 | Validate single invalid config | `ndp-validate --domain invalid.json` |
| CLI-003 | Validate all domains | `ndp-validate --all --domain` |
| CLI-004 | Schema-only mode | `ndp-validate --schema-only --domain config.json` |
| CLI-005 | Human output format | `ndp-validate --format human --domain config.json` |
| CLI-006 | JSON output format | `ndp-validate --format json --domain config.json` |
| CLI-007 | Strict mode with warnings | `ndp-validate --strict --domain config.json` |

**Acceptance Criteria:**
- AC1: All CLI integration tests pass
- AC2: Exit codes verified
- AC3: Output format verified

**Verification Method:** Integration test suite

**Priority:** High

---

## 5. Implementation Notes

### 5.1 Code Changes Summary

| File | Change Type | Description |
|------|-------------|-------------|
| `cli.rs` | Modify | Add `--domain`, `--domain-schema` flags |
| `main.rs` | Modify | Add domain validation flow |
| `schema.rs` | Modify | Add `DomainSchemaValidator` |
| `semantic/mod.rs` | Modify | Export domain validation |
| `lib.rs` | Modify | Re-export domain types |
| `deploy.sh` | Modify | Add domain validation step |
| `domain.schema.json` | Modify | Support flat format |

### 5.2 Existing Code Reuse

The following existing code should be reused:

| Component | Location | Reuse For |
|-----------|----------|-----------|
| `ValidationResult` | `cli.rs` | Output structure |
| `ValidationError` | `error.rs` | Error representation |
| `SchemaValidator` | `schema.rs` | Base pattern |
| `validate_domain()` | `semantic/domain.rs` | Layer 2 rules |
| `output_json()` | `cli.rs` | JSON formatting |
| `output_human()` | `cli.rs` | Human formatting |

---

## 6. Verification Commands

### 6.1 Phase B Verification Checklist

```bash
# 1. Validate single domain config
ndp-validate --domain config/domains/indoor-air-quality/domain.json
echo "Exit code: $?"  # Should be 0

# 2. Validate all domain configs
ndp-validate --all --domain
echo "Exit code: $?"  # Should be 0

# 3. Schema-only validation
ndp-validate --schema-only --domain config/domains/indoor-air-quality/domain.json

# 4. Human-readable output
ndp-validate --format human --domain config/domains/indoor-air-quality/domain.json

# 5. JSON output
ndp-validate --format json --domain config/domains/indoor-air-quality/domain.json | jq .

# 6. Test invalid config (should fail)
echo '{"id": "invalid"}' > /tmp/bad-domain.json
ndp-validate --domain /tmp/bad-domain.json
echo "Exit code: $?"  # Should be 1

# 7. Run all tests
cargo test -p ndp-validate

# 8. Test deploy.sh integration
./deploy.sh sync --dry-run  # Should include domain validation
```

---

## 7. Traceability

| Requirement | SCOPE.md Reference | GitHub Issue |
|-------------|-------------------|--------------|
| REQ-B01-001 | AC-B1 | #13 |
| REQ-B01-002 | AC-B2 | #13 |
| REQ-B01-004 | Phase B | #13 |
| REQ-B01-007 | AC-B4 | #13 |
| REQ-B01-009 | AC-B3 | #13 |
| REQ-B01-010 | AC-B5 | #13 |
| REQ-B01-012 | AC-B7 | #13 |
| REQ-B01-016 | AC-B8 | #13 |

---

## 8. Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-02-05 | Specification Agent | Initial specification |
