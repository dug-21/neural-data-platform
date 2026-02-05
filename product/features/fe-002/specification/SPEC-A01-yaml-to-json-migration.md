# SPEC-A01: YAML to JSON Migration

> **Feature:** FE-002 Domain Configuration Standardization
> **Phase:** A - YAML to JSON Migration (GAP-001)
> **Version:** 1.0
> **Status:** Draft
> **Created:** 2026-02-05

---

## 1. Introduction

### 1.1 Purpose

This specification defines requirements for migrating domain configuration from YAML to JSON format, resolving GAP-001 (ADR-016-001 violation).

### 1.2 Scope

- Convert `domain.yaml` to `domain.json`
- Update `ndp-gold-ddl` loader to use JSON
- Remove `serde_yaml` dependency
- Preserve all existing functionality with zero behavioral change

### 1.3 Critical Constraint

**DDL Output Preservation Guarantee:**
```
YAML Config --> ndp-gold-ddl --> DDL Output (baseline)
JSON Config --> ndp-gold-ddl --> DDL Output (MUST BE IDENTICAL)
```

This is a non-negotiable requirement. Any change in DDL output constitutes a failure.

---

## 2. Functional Requirements

### 2.1 Configuration File Conversion

#### REQ-A01-001: Create domain.json File

**Description:** Convert `config/domains/indoor-air-quality/domain.yaml` to `domain.json` preserving all data.

**Acceptance Criteria:**
- AC1: File `config/domains/indoor-air-quality/domain.json` exists
- AC2: All fields from YAML are present in JSON with equivalent values
- AC3: Numeric values maintain precision (e.g., `threshold: 800` stays `800`, not `800.0`)
- AC4: String values are properly quoted
- AC5: Array order is preserved
- AC6: Nested structures maintain hierarchy

**Verification Method:** Inspection, automated comparison

**Priority:** High

---

#### REQ-A01-002: Preserve YAML Comments as Description Fields

**Description:** YAML comments that document intent should be preserved as `description` fields where the schema supports them.

**Current YAML Comments to Preserve:**

| Location | Comment | Target Field |
|----------|---------|--------------|
| File header | "Cross-stream alignment for correlation analysis" | Already in `description` |
| Stream `air-quality` | "NULL handling: preserve (observation stream - default)" | Document in README |
| Stream `home-assistant-state` | "State persists until changed (state_event stream)" | Document in README |
| Stream `outdoor-air-quality` | "Phase D Fast-Follower: Added outdoor-air-quality as 4th stream" | Document in README |
| Alignment | "null_handling: by_stream_type - resolved from stream configs per ADR-FE001-004" | Document in README |

**Acceptance Criteria:**
- AC1: Essential design intent documented in `domain.json` `description` fields where supported
- AC2: Implementation notes documented in domain README or inline JSON comments (if tooling supports)
- AC3: No loss of architectural context

**Verification Method:** Manual review

**Priority:** Medium

---

#### REQ-A01-003: Delete Original YAML File

**Description:** Remove `domain.yaml` after successful migration to prevent configuration drift.

**Acceptance Criteria:**
- AC1: File `config/domains/indoor-air-quality/domain.yaml` does not exist
- AC2: No other YAML domain config files exist in `config/domains/`

**Verification Method:** Inspection, `find config/domains -name "*.yaml"` returns empty

**Priority:** High

---

### 2.2 Loader Code Updates

#### REQ-A01-004: Update Domain Config Path Extension

**Description:** Change the file path in `loader.rs` from `.yaml` to `.json`.

**Current Code (loader.rs:42-47):**
```rust
fn domain_config_path(&self, domain_id: &str) -> PathBuf {
    self.config_dir
        .join("domains")
        .join(domain_id)
        .join("domain.yaml")  // <-- Change to "domain.json"
}
```

**Acceptance Criteria:**
- AC1: Method returns path ending in `domain.json`
- AC2: Path structure `{config_dir}/domains/{domain_id}/domain.json` is preserved

**Verification Method:** Code review, unit test

**Priority:** High

---

#### REQ-A01-005: Update Parser to serde_json

**Description:** Change the parser from `serde_yaml` to `serde_json`.

**Current Code (loader.rs:79-82):**
```rust
let config: DomainConfig =
    serde_yaml::from_str(&content).map_err(|e| GoldDdlError::ConfigParseError {
        message: format!("Failed to parse {}: {}", path.display(), e),
    })?;
```

**Target Code:**
```rust
let config: DomainConfig =
    serde_json::from_str(&content).map_err(|e| GoldDdlError::ConfigParseError {
        message: format!("Failed to parse {}: {}", path.display(), e),
    })?;
```

**Acceptance Criteria:**
- AC1: Parser uses `serde_json::from_str`
- AC2: Error messages still include file path and parse error details
- AC3: Error handling behavior is consistent with YAML parser

**Verification Method:** Code review, unit test

**Priority:** High

---

#### REQ-A01-006: Remove serde_yaml Dependency

**Description:** Remove `serde_yaml` from `ndp-gold-ddl` Cargo.toml.

**Acceptance Criteria:**
- AC1: `serde_yaml` not in `tools/ndp-gold-ddl/Cargo.toml` dependencies
- AC2: No `use serde_yaml` statements in ndp-gold-ddl crate
- AC3: `cargo build -p ndp-gold-ddl` succeeds

**Verification Method:** `grep -r "serde_yaml" tools/ndp-gold-ddl/` returns nothing

**Priority:** Medium

---

### 2.3 Test Fixture Updates

#### REQ-A01-007: Convert Test Fixtures to JSON

**Description:** Update inline YAML test fixtures in `domain.rs` to JSON format.

**Affected Tests (domain.rs):**
1. `test_domain_config_deserialize` (lines 310-338)
2. `test_stream_ref_with_null_handling_override` (lines 341-351)
3. `test_objective_config_deserialize` (lines 354-371)

**Example Conversion:**

**Before (YAML):**
```rust
let yaml = r#"
id: indoor-air-quality
description: Indoor air quality monitoring domain
streams:
  - stream_id: air-quality
    alias: indoor
    role: primary
"#;
let config: DomainConfig = serde_yaml::from_str(yaml).unwrap();
```

**After (JSON):**
```rust
let json = r#"{
    "id": "indoor-air-quality",
    "description": "Indoor air quality monitoring domain",
    "streams": [
        {
            "stream_id": "air-quality",
            "alias": "indoor",
            "role": "primary"
        }
    ]
}"#;
let config: DomainConfig = serde_json::from_str(json).unwrap();
```

**Acceptance Criteria:**
- AC1: All test fixtures use JSON format
- AC2: All tests pass with `cargo test -p ndp-gold-ddl`
- AC3: Test coverage maintained (same assertions)

**Verification Method:** Test execution, code review

**Priority:** High

---

## 3. Non-Functional Requirements

### 3.1 DDL Output Preservation

#### REQ-A01-008: Zero DDL Output Change (Critical)

**Description:** The generated DDL must be byte-identical before and after migration.

**Verification Procedure:**
```bash
# Step 1: Capture baseline BEFORE any changes
ndp-gold-ddl generate --domain indoor-air-quality > /tmp/baseline.sql

# Step 2: After migration, capture new output
ndp-gold-ddl generate --domain indoor-air-quality > /tmp/migrated.sql

# Step 3: Compare (must be identical)
diff /tmp/baseline.sql /tmp/migrated.sql
# Exit code MUST be 0 (no differences)
```

**Acceptance Criteria:**
- AC1: `diff` returns exit code 0
- AC2: No warnings or changes in output formatting
- AC3: All SQL statements identical

**Verification Method:** Automated diff comparison

**Priority:** Critical (Blocking)

---

### 3.2 Data Type Preservation

#### REQ-A01-009: Numeric Precision Preservation

**Description:** Numeric values must maintain their precision and type during conversion.

**Examples:**
| YAML Value | JSON Value | Correct |
|------------|------------|---------|
| `threshold: 800` | `"threshold": 800` | Yes |
| `threshold: 800` | `"threshold": 800.0` | No |
| `threshold: 12` | `"threshold": 12` | Yes |
| `threshold: 20.5` | `"threshold": 20.5` | Yes |

**Acceptance Criteria:**
- AC1: Integer values remain integers
- AC2: Float values maintain precision
- AC3: No unintended type coercion

**Verification Method:** Inspection, JSON schema validation

**Priority:** High

---

#### REQ-A01-010: String Encoding Preservation

**Description:** All string values must be properly encoded in JSON.

**Acceptance Criteria:**
- AC1: Special characters properly escaped
- AC2: UTF-8 encoding preserved
- AC3: No data corruption

**Verification Method:** `jq . domain.json` succeeds without errors

**Priority:** High

---

### 3.3 Backward Compatibility

#### REQ-A01-011: CLI Interface Unchanged

**Description:** The `ndp-gold-ddl` CLI interface must remain unchanged.

**Acceptance Criteria:**
- AC1: `ndp-gold-ddl generate --domain indoor-air-quality` works
- AC2: `ndp-gold-ddl generate --all` works (if applicable)
- AC3: Error messages for missing configs are helpful

**Verification Method:** CLI testing

**Priority:** High

---

## 4. Data Preservation Requirements

### 4.1 Field Mapping

#### REQ-A01-012: Complete Field Mapping

**Description:** All fields from YAML must map to JSON equivalents.

**Field Inventory:**

| YAML Path | JSON Path | Type | Required |
|-----------|-----------|------|----------|
| `id` | `id` | string | Yes |
| `description` | `description` | string | No |
| `streams` | `streams` | array | Yes |
| `streams[].stream_id` | `streams[].stream_id` | string | Yes |
| `streams[].alias` | `streams[].alias` | string | No |
| `streams[].role` | `streams[].role` | enum | Yes |
| `streams[].null_handling` | `streams[].null_handling` | enum | No |
| `alignment` | `alignment` | object | Yes |
| `alignment.view_name` | `alignment.view_name` | string | Yes |
| `alignment.granularity` | `alignment.granularity` | string | Yes |
| `alignment.join_strategy` | `alignment.join_strategy` | enum | No |
| `objectives` | `objectives` | array | No |
| `objectives[].id` | `objectives[].id` | string | Yes |
| `objectives[].description` | `objectives[].description` | string | No |
| `objectives[].target` | `objectives[].target` | object | Yes |
| `objectives[].target.stream` | `objectives[].target.stream` | string | Yes |
| `objectives[].target.metric` | `objectives[].target.metric` | string | Yes |
| `objectives[].target.condition` | `objectives[].target.condition` | enum | Yes |
| `objectives[].target.threshold` | `objectives[].target.threshold` | number | Yes |
| `objectives[].target.unit` | `objectives[].target.unit` | string | No |
| `objectives[].priority` | `objectives[].priority` | enum | No |

**Acceptance Criteria:**
- AC1: All fields present in source YAML exist in target JSON
- AC2: No extra fields added
- AC3: No fields missing

**Verification Method:** Automated field comparison, manual review

**Priority:** High

---

### 4.2 Current Configuration Snapshot

#### REQ-A01-013: Document Current State

**Description:** The current `domain.yaml` content for reference:

```yaml
# Domain: Indoor Air Quality
id: indoor-air-quality
description: "Maintain healthy indoor air quality"

streams:
  - stream_id: air-quality
    alias: indoor
    role: primary

  - stream_id: outdoor-weather
    alias: outdoor
    role: context

  - stream_id: home-assistant-state
    alias: state
    role: actuator
    null_handling: carry_forward

  - stream_id: outdoor-air-quality
    alias: outdoor_aqi
    role: constraint

alignment:
  view_name: indoor_air_quality_aligned
  granularity: "1 hour"
  join_strategy: full_outer

objectives:
  - id: healthy_co2
    description: "Keep CO2 below 800 ppm for cognitive performance"
    target:
      stream: air-quality
      metric: co2
      condition: "<"
      threshold: 800
      unit: ppm
    priority: high

  # ... (6 total objectives)
```

**Expected JSON Output:**
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
    },
    {
      "stream_id": "home-assistant-state",
      "alias": "state",
      "role": "actuator",
      "null_handling": "carry_forward"
    },
    {
      "stream_id": "outdoor-air-quality",
      "alias": "outdoor_aqi",
      "role": "constraint"
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

---

## 5. Verification Commands

### 5.1 Phase A Verification Checklist

```bash
# 1. Verify JSON file exists and is valid
jq . config/domains/indoor-air-quality/domain.json

# 2. Verify YAML file removed
[ ! -f config/domains/indoor-air-quality/domain.yaml ] && echo "PASS: YAML removed"

# 3. Verify no serde_yaml in ndp-gold-ddl
grep -r "serde_yaml" tools/ndp-gold-ddl/ && echo "FAIL" || echo "PASS: No serde_yaml"

# 4. Run tests
cargo test -p ndp-gold-ddl

# 5. Test CLI
ndp-gold-ddl generate --domain indoor-air-quality

# 6. CRITICAL: DDL comparison (must pass)
# Pre-capture baseline BEFORE making changes
# Then compare after changes
diff /tmp/baseline.sql /tmp/migrated.sql && echo "PASS: DDL identical"
```

---

## 6. Schema Format Decision

### 6.1 Format Recommendation

**Recommendation:** Use **flat format** (no `"domain":` wrapper)

**Rationale:**
1. Current `DomainConfig` struct expects flat format
2. Minimizes code changes
3. Direct YAML to JSON conversion

**Required Schema Update:**
The current `domain.schema.json` expects wrapped format:
```json
{
  "required": ["domain"],
  "properties": {
    "domain": { "$ref": "#/definitions/domain_content" }
  }
}
```

For Phase B, either:
- Create `domain-flat.schema.json` that validates flat format, OR
- Update `domain.schema.json` to accept flat format (using `oneOf`)

This decision is documented here for Phase B implementation.

---

## 7. Traceability

| Requirement | SCOPE.md Reference | GitHub Issue |
|-------------|-------------------|--------------|
| REQ-A01-001 | AC-A1 | #11 |
| REQ-A01-003 | AC-A3 | #11 |
| REQ-A01-004 | AC-A4 | #11 |
| REQ-A01-005 | AC-A4 | #11 |
| REQ-A01-006 | AC-A7 | #11 |
| REQ-A01-007 | AC-A5 | #11 |
| REQ-A01-008 | New (Critical) | #11 |

---

## 8. Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-02-05 | Specification Agent | Initial specification |
