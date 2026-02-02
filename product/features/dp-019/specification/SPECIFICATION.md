# dp-019: Config Validation Pipeline - SPARC Specification

**Document Type**: SPARC Specification (Phase S)
**Feature**: dp-019 Config Validation Pipeline
**Version**: 1.0
**Date**: 2026-02-02
**Parent**: dp-016 Configuration Architecture Review
**Dependency**: dp-018 JSON Config Foundation

---

## 1. Executive Summary

This specification defines the requirements for implementing Phase 2 of the dp-016 Configuration Architecture roadmap. The goal is to establish a two-layer validation pipeline that catches configuration errors at deploy time rather than runtime.

### Key Outcomes

1. **Two-layer validation**: Schema validation (JSON Schema) + Semantic validation (Rust code)
2. **Documented NDP-supported values**: Research and document valid types, device_classes, transforms, DQ operators
3. **DDL generation research**: Type mapping, index strategy for dp-020
4. **Fail-fast deployment**: Bad configs block deployment
5. **Structured error output**: JSONPath-based error locations with actionable messages
6. **Runtime defense**: Defensive validation at application startup

### Core Architecture Principle

**Validate early, fail loudly, provide actionable errors**

Configuration errors should be caught at the earliest possible stage:
- **Layer 1 (Schema)**: Structure, types, required fields - caught during `ndp-validate`
- **Layer 2 (Semantic)**: Application rules, valid values, cross-references - caught during `ndp-validate --full`
- **Layer 3 (Runtime)**: Defense-in-depth at application startup

---

## 2. Requirements Analysis

### 2.1 Functional Requirements

#### Research Tasks

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-001** | Research NDP-supported field types | HIGH | Document all valid `fields[].type` values with their PostgreSQL mappings. Output: `docs/config/SUPPORTED-VALUES.md` | Task 2.0 |
| **FR-002** | Research device_class constraints | MEDIUM | Determine if device_class is constrained or freeform. Document valid values if constrained | Task 2.0 |
| **FR-003** | Research source types | HIGH | Document valid `sources[].type` values (mqtt, http, etc.) with required fields for each | Task 2.0 |
| **FR-004** | Research transform functions | HIGH | Document valid `silver_etl.field_mappings[].transform` values | Task 2.0 |
| **FR-005** | Research DQ operators | HIGH | Document valid DQ rule syntax and operators | Task 2.0 |
| **FR-006** | Research DDL type mapping | HIGH | Create type mapping table (JSON type -> PostgreSQL type). Output: `docs/config/DDL-GENERATION.md` | Task 2.0a |
| **FR-007** | Research index strategy | MEDIUM | Document default indexes (timestamp, ndp_id) and DQ-derived indexes | Task 2.0a |

#### Layer 1: JSON Schema Validation

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-010** | Create Validator CLI binary | HIGH | Rust binary `ndp-validate` with two-layer validation. Returns structured JSON errors | Task 2.1 |
| **FR-011** | JSON syntax validation | HIGH | Catch malformed JSON with line/column numbers. Clear error message showing exact location | Task 2.2 |
| **FR-012** | JSON Schema validation | HIGH | Validate against `stream-config.v1.schema.json` using `jsonschema` crate. Report all violations | Task 2.3 |
| **FR-013** | Unknown field detection | HIGH | Fail on unexpected fields. Schema uses `additionalProperties: false` at all levels | Task 2.4, P-007 |
| **FR-014** | Required field validation | HIGH | Fail if required fields are missing with clear message naming the field | Task 2.3 |
| **FR-015** | Type coercion rejection | MEDIUM | Reject wrong types (e.g., string where number expected) without silent coercion | Task 2.3 |

#### Layer 2: Semantic Validation (Rust Code)

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-020** | Valid field type values | HIGH | Validate `fields[].type` against NDP-supported types enum. Reject unsupported types | Task 2.5, P-010 |
| **FR-021** | Valid device_class values | MEDIUM | Warn or error on unknown device_class values (behavior depends on FR-002 research) | Task 2.6 |
| **FR-022** | Source path cross-reference | CRITICAL | Validate `silver_etl.field_mappings[].source_path` references exist in `fields` section | Task 2.7, P-005 |
| **FR-023** | Silver table existence check | HIGH | Verify `silver_etl.target_table` exists in TimescaleDB (optional, requires DB connection) | Task 2.8, P-006 |
| **FR-024** | DQ rule syntax validation | HIGH | Validate DQ rule expressions against supported operators. Catch invalid expressions | Task 2.9 |
| **FR-025** | DQ rule column validation | MEDIUM | Validate DQ rule column references exist in field_mappings | Task 2.9, P-009 |
| **FR-026** | Source config validation | HIGH | Validate MQTT sources have `broker`, `topic`. HTTP sources have `url`, `interval` | Task 2.10 |
| **FR-027** | Transform function validation | MEDIUM | Validate `field_mappings[].transform` values against supported transforms | Task 2.5 |
| **FR-028** | Retention/compression validation | LOW | Validate `retention_days >= compression_after_days` | Task 2.5 |

#### Integration

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-030** | Integrate into deploy.sh | CRITICAL | `deploy.sh sync` calls `ndp-validate --all` before syncing. Bad config = deploy blocked | Task 2.11 |
| **FR-031** | Runtime startup validation | HIGH | `air-quality-app` runs validation on loaded config at startup. Fails loudly if invalid | Task 2.12 |
| **FR-032** | Structured error output | HIGH | All errors in JSON format with `layer`, `path`, `message`, `severity` fields | Error Format |
| **FR-033** | Exit code semantics | MEDIUM | `ndp-validate` returns 0 on success, 1 on validation errors, 2 on system errors | Task 2.1 |
| **FR-034** | Schema vs code decision | MEDIUM | Document which rules are JSON Schema enums vs Rust code after research | Task 2.13 |

### 2.2 Non-Functional Requirements

| ID | Category | Requirement | Measurement | Traces To |
|----|----------|-------------|-------------|-----------|
| **NFR-001** | Performance | Schema validation completes in <100ms for typical config | Benchmark single config validation | - |
| **NFR-002** | Performance | Full validation (with DB checks) completes in <500ms | Benchmark with mocked DB | - |
| **NFR-003** | Usability | Error messages are actionable | Operator can fix error based on message alone | P-018, P-019 |
| **NFR-004** | Reliability | Zero false positives | Valid configs never rejected | - |
| **NFR-005** | Reliability | Zero false negatives for critical rules | Invalid source_path always caught | P-005 |
| **NFR-006** | Maintainability | Validation rules are declarative where possible | Schema rules in JSON, not Rust | Task 2.13 |
| **NFR-007** | Portability | Validator runs on Raspberry Pi (ARM64) | Rust binary cross-compiles | Platform Constraints |
| **NFR-008** | Testability | All validation rules have unit tests | 100% coverage of rules | dp-017 |

---

## 3. Acceptance Criteria

### AC-001: Schema Validation Catches Malformed JSON

```gherkin
Feature: Malformed JSON Detection (Layer 1)

  Scenario: Syntax error in JSON
    Given a config file with content: '{"stream_id": "test", invalid}'
    When I run `ndp-validate config.json`
    Then exit code is 1
    And output contains error with layer "syntax"
    And error message contains line number
    And error message indicates "expected string or '}'"

  Scenario: Valid JSON structure
    Given a valid JSON config file
    When I run `ndp-validate --schema-only config.json`
    Then exit code is 0
    And output contains '"valid": true'
```

### AC-002: Schema Validation Catches Unknown Fields

```gherkin
Feature: Unknown Field Detection (Layer 1)

  Scenario: Typo in top-level field
    Given a config with field "silver_elt" (typo for silver_etl)
    When I run `ndp-validate config.json`
    Then exit code is 1
    And output contains error with layer "schema"
    And error path is "$.silver_elt"
    And error message contains "additional property" or "unexpected field"

  Scenario: Typo in nested field
    Given a config with "silver_etl.field_mapings" (typo)
    When I run `ndp-validate config.json`
    Then exit code is 1
    And error path contains "$.silver_etl.field_mapings"

  Scenario: Extra field in array element
    Given a config with "fields[0].unknown_attr"
    When I run `ndp-validate config.json`
    Then exit code is 1
    And error path contains "$.fields[0].unknown_attr"
```

### AC-003: Semantic Validation Catches Invalid Type Values

```gherkin
Feature: Invalid Type Detection (Layer 2)

  Scenario: Unsupported field type
    Given a config with fields[0].type = "decimal"
    And "decimal" is not in NDP-supported types
    When I run `ndp-validate config.json`
    Then exit code is 1
    And output contains error with layer "semantic"
    And error path is "$.fields[0].type"
    And error message contains "must be one of: float, integer, string, boolean, timestamp, json"

  Scenario: Valid field type
    Given a config with fields[0].type = "float"
    When I run `ndp-validate config.json`
    Then validation passes (no type error)
```

### AC-004: Semantic Validation Catches Bad source_path References

```gherkin
Feature: Source Path Cross-Reference (Layer 2)

  Scenario: source_path references non-existent field
    Given a config with fields: [{"name": "pm25", ...}]
    And silver_etl.field_mappings[0].source_path = "raw_payload.pm2_5" (typo)
    When I run `ndp-validate config.json`
    Then exit code is 1
    And output contains error with layer "semantic"
    And error path is "$.silver_etl.field_mappings[0].source_path"
    And error message contains "not found in fields"
    And error message suggests "did you mean 'pm25'?" (optional fuzzy match)

  Scenario: All source_paths are valid
    Given a config where all field_mappings.source_path values match fields[].name
    When I run `ndp-validate config.json`
    Then no source_path errors are reported

  Scenario: source_path with raw_payload prefix
    Given fields[].name = "temperature"
    And field_mappings[].source_path = "raw_payload.temperature"
    When I run `ndp-validate config.json`
    Then no error (raw_payload. prefix is stripped for comparison)
```

### AC-005: Silver Table Existence Check Works

```gherkin
Feature: Silver Table Existence (Layer 2, Optional)

  Scenario: Table does not exist
    Given silver_etl.target_table = "silver.nonexistent_table"
    And the table does not exist in TimescaleDB
    When I run `ndp-validate --check-tables config.json`
    Then exit code is 1
    And output contains error with layer "semantic"
    And error path is "$.silver_etl.target_table"
    And error message contains "table 'silver.nonexistent_table' does not exist"

  Scenario: Table exists
    Given silver_etl.target_table = "silver.air_quality_readings"
    And the table exists in TimescaleDB
    When I run `ndp-validate --check-tables config.json`
    Then no table existence error is reported

  Scenario: Skip table check without flag
    Given silver_etl.target_table = "silver.nonexistent_table"
    When I run `ndp-validate config.json` (no --check-tables)
    Then no table existence error is reported (check skipped)
```

### AC-006: Deploy Blocked on Validation Failure

```gherkin
Feature: Deploy Integration

  Scenario: Deploy blocked by invalid config
    Given an invalid config file in config/base/streams/test/config.json
    When I run `./deploy.sh sync`
    Then deploy.sh returns non-zero exit code
    And output contains "Validation failed"
    And etcd is NOT updated with the invalid config

  Scenario: Deploy succeeds with valid configs
    Given all config files in config/base/streams/ are valid
    When I run `./deploy.sh sync`
    Then deploy.sh returns 0
    And all configs are synced to etcd

  Scenario: Partial failure lists all errors
    Given 3 config files, 2 valid and 1 invalid
    When I run `./deploy.sh sync`
    Then deploy.sh returns non-zero exit code
    And output shows which config failed
    And output shows validation errors for the failed config
    And valid configs are NOT synced (atomic behavior)
```

### AC-007: Structured Error Output with Paths

```gherkin
Feature: Error Output Format

  Scenario: Multiple errors in one config
    Given a config with multiple validation errors
    When I run `ndp-validate config.json`
    Then output is valid JSON
    And output has structure:
      {
        "valid": false,
        "config_path": "config/base/streams/test/config.json",
        "errors": [
          {
            "layer": "schema|semantic",
            "path": "$.some.json.path",
            "message": "human readable message",
            "severity": "error|warning"
          }
        ]
      }
    And each error has all required fields
    And paths use JSONPath notation

  Scenario: Warnings vs Errors
    Given a config with deprecated device_class value
    When I run `ndp-validate config.json`
    Then output contains warning with severity "warning"
    And exit code is 0 (warnings don't fail validation)

  Scenario: Human-readable output mode
    Given an invalid config
    When I run `ndp-validate --human config.json`
    Then output is formatted for terminal readability
    And each error shows file path, JSONPath, and message
    And errors are color-coded (if terminal supports)
```

### AC-008: Runtime Defensive Validation at Startup

```gherkin
Feature: Runtime Validation (Defense in Depth)

  Scenario: App starts with valid config
    Given etcd contains valid config for stream "air-quality"
    When air-quality-app starts
    Then app starts successfully
    And log contains "Config validation passed for air-quality"

  Scenario: App fails loudly with invalid config
    Given etcd contains config with invalid source_path for "air-quality"
    When air-quality-app starts
    Then app startup fails (or stream is disabled)
    And log contains ERROR "Config validation failed for air-quality"
    And log contains the validation error details
    And the invalid stream does NOT start processing

  Scenario: --skip-validation flag (emergency use)
    Given etcd contains config with validation warnings
    When air-quality-app starts with --skip-validation
    Then app starts (validation skipped)
    And log contains WARNING "Config validation skipped"
```

---

## 4. Validation Rules Matrix

### Layer 1: JSON Schema (Declarative)

| Rule ID | Field/Path | Validation | Schema Implementation |
|---------|------------|------------|----------------------|
| L1-001 | `stream_id` | Required, non-empty string, kebab-case pattern | `"required": ["stream_id"]`, `"pattern": "^[a-z][a-z0-9-]*$"` |
| L1-002 | `description` | Required string | `"required": ["description"]` |
| L1-003 | `fields` | Required array, min 1 element | `"required": ["fields"]`, `"minItems": 1` |
| L1-004 | `fields[].name` | Required string, snake_case | `"required": ["name"]`, `"pattern": "^[a-z_][a-z0-9_]*$"` |
| L1-005 | `fields[].type` | Required string, enum of valid types | `"enum": ["float", "integer", "string", "boolean", "timestamp", "json"]` |
| L1-006 | `sources` | Required array, min 1 element | `"required": ["sources"]`, `"minItems": 1` |
| L1-007 | `sources[].type` | Required string, enum of source types | `"enum": ["mqtt", "http", "file"]` |
| L1-008 | `silver_etl.enabled` | Boolean if present | `"type": "boolean"` |
| L1-009 | `silver_etl.target_table` | Required if silver_etl present, format "silver.{name}" | `"pattern": "^silver\\.[a-z_]+$"` |
| L1-010 | `silver_etl.field_mappings` | Required array if silver_etl present | `"required": ["field_mappings"]` |
| L1-011 | `silver_etl.field_mappings[].target_column` | Required string | `"required": ["target_column"]` |
| L1-012 | `silver_etl.field_mappings[].source_path` | Required string | `"required": ["source_path"]` |
| L1-013 | `silver_etl.field_mappings[].target_type` | Required string | `"required": ["target_type"]` |
| L1-014 | Root object | No additional properties | `"additionalProperties": false` |
| L1-015 | All nested objects | No additional properties | `"additionalProperties": false` at each level |
| L1-016 | `retention_days` | Integer >= 0 if present | `"type": "integer", "minimum": 0` |
| L1-017 | `compression_after_days` | Integer >= 0 if present | `"type": "integer", "minimum": 0` |
| L1-018 | `fields[].range` | Array of exactly 2 numbers if present | `"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2` |

### Layer 2: Semantic Validation (Rust Code)

| Rule ID | Field/Path | Validation | Why Not Schema? |
|---------|------------|------------|-----------------|
| L2-001 | `fields[].type` | Value in NDP-supported types | Could be schema enum, but allows runtime extension |
| L2-002 | `fields[].device_class` | Warn if not in known list | Freeform with recommendations |
| L2-003 | `silver_etl.field_mappings[].source_path` | Must reference existing field in `fields` | Cross-reference between sections |
| L2-004 | `silver_etl.target_table` | Table must exist in TimescaleDB | Requires database query |
| L2-005 | `silver_etl.field_mappings[].transform` | Must be valid transform function | Dynamic list from Rust code |
| L2-006 | `silver_etl.dq_rules[].expression` | Valid DQ expression syntax | Requires expression parser |
| L2-007 | `silver_etl.dq_rules` column refs | Columns must exist in field_mappings | Cross-reference within silver_etl |
| L2-008 | `sources[].broker` (MQTT) | Required for MQTT sources | Conditional requirement |
| L2-009 | `sources[].topic` (MQTT) | Required for MQTT sources | Conditional requirement |
| L2-010 | `sources[].url` (HTTP) | Required for HTTP sources | Conditional requirement |
| L2-011 | `sources[].interval` (HTTP) | Required for HTTP sources | Conditional requirement |
| L2-012 | `retention_days` vs `compression_after_days` | retention >= compression | Cross-field comparison |
| L2-013 | `silver_etl.field_mappings[].target_type` | Valid PostgreSQL type mapping | Application-specific mapping |
| L2-014 | Duplicate field names | No duplicate names in `fields[]` | Array uniqueness logic |
| L2-015 | Duplicate target_column | No duplicates in `field_mappings[]` | Array uniqueness logic |

### Decision: Schema vs Code (Task 2.13)

After research, the following decisions apply:

| Rule | JSON Schema | Rust Code | Rationale |
|------|-------------|-----------|-----------|
| Valid `fields[].type` | **YES** (enum) | Fallback | List is small, stable, known at schema design |
| Valid `sources[].type` | **YES** (enum) | Fallback | Limited source types (mqtt, http, file) |
| Valid `device_class` | **NO** | Warning only | Freeform/extensible for Home Assistant compatibility |
| `source_path` exists | **NO** | **YES** | Cross-reference logic not possible in schema |
| Table exists | **NO** | **YES** | Requires database query |
| DQ syntax | **NO** | **YES** | Requires expression parser |
| Transform validity | **NO** | **YES** | Dynamic list in Rust code |
| Conditional required fields | **NO** | **YES** | MQTT vs HTTP have different required fields |

---

## 5. Error Message Standards

### 5.1 Error Format Specification

All validation errors MUST conform to this JSON structure:

```json
{
  "valid": false,
  "config_path": "config/base/streams/air-quality/config.json",
  "summary": {
    "total_errors": 3,
    "total_warnings": 1,
    "by_layer": {
      "syntax": 0,
      "schema": 2,
      "semantic": 1
    }
  },
  "errors": [
    {
      "layer": "schema",
      "code": "UNKNOWN_FIELD",
      "path": "$.silver_etl.field_mapings",
      "message": "Unknown field 'field_mapings'. Did you mean 'field_mappings'?",
      "severity": "error",
      "suggestion": "Rename to 'field_mappings'"
    },
    {
      "layer": "semantic",
      "code": "INVALID_SOURCE_PATH",
      "path": "$.silver_etl.field_mappings[2].source_path",
      "message": "source_path 'raw_payload.temperture' not found in fields",
      "severity": "error",
      "suggestion": "Did you mean 'temperature'?",
      "context": {
        "available_fields": ["pm25", "temperature", "humidity"]
      }
    }
  ],
  "warnings": [
    {
      "layer": "semantic",
      "code": "UNKNOWN_DEVICE_CLASS",
      "path": "$.fields[0].device_class",
      "message": "Unknown device_class 'air_quality'. This may be intentional.",
      "severity": "warning",
      "suggestion": "Known device classes: sensor, binary_sensor, switch"
    }
  ]
}
```

### 5.2 Path Notation (JSONPath)

All paths MUST use JSONPath notation:

| Path | Meaning |
|------|---------|
| `$` | Root object |
| `$.stream_id` | Top-level `stream_id` field |
| `$.fields[0]` | First element of `fields` array |
| `$.fields[0].name` | `name` property of first field |
| `$.silver_etl.field_mappings[2].source_path` | Third field_mapping's source_path |

### 5.3 Error Codes

| Code | Layer | Description |
|------|-------|-------------|
| `SYNTAX_ERROR` | syntax | Malformed JSON |
| `MISSING_REQUIRED` | schema | Required field not present |
| `INVALID_TYPE` | schema | Wrong JSON type (e.g., string where number expected) |
| `UNKNOWN_FIELD` | schema | Unexpected field (additionalProperties violation) |
| `PATTERN_MISMATCH` | schema | String doesn't match regex pattern |
| `ENUM_VIOLATION` | schema | Value not in allowed enum |
| `ARRAY_BOUNDS` | schema | Array has too few/many items |
| `INVALID_FIELD_TYPE` | semantic | `fields[].type` not NDP-supported |
| `UNKNOWN_DEVICE_CLASS` | semantic | device_class not recognized (warning) |
| `INVALID_SOURCE_PATH` | semantic | source_path doesn't reference valid field |
| `TABLE_NOT_FOUND` | semantic | Silver table doesn't exist |
| `INVALID_TRANSFORM` | semantic | Transform function not recognized |
| `INVALID_DQ_SYNTAX` | semantic | DQ rule expression parse error |
| `INVALID_DQ_COLUMN` | semantic | DQ rule references unknown column |
| `MISSING_SOURCE_CONFIG` | semantic | Source missing required config (broker, topic, etc.) |
| `DUPLICATE_NAME` | semantic | Duplicate field name or target_column |
| `CONSTRAINT_VIOLATION` | semantic | retention_days < compression_after_days |

### 5.4 Severity Levels

| Severity | Exit Code Impact | Description |
|----------|------------------|-------------|
| `error` | Causes non-zero exit | Must fix before deploy |
| `warning` | Does NOT cause non-zero exit | Should review, may be intentional |

---

## 6. Integration Requirements

### 6.1 deploy.sh Integration

**Location**: `deploy/pi/deploy.sh`

**Modification**: Add validation gate before `sync` action.

```bash
# In deploy.sh sync action
sync_configs() {
    echo "Validating configs..."

    # Run validation on all configs
    if ! ndp-validate --all --format human; then
        echo "ERROR: Config validation failed. Deploy aborted."
        exit 1
    fi

    echo "Validation passed. Syncing to etcd..."
    # Existing sync logic...
}
```

**Behavior**:
- Validation runs BEFORE any etcd writes
- Single failure blocks entire sync (atomic)
- All errors displayed before abort
- Exit code 1 if any config invalid

### 6.2 Runtime Startup Validation

**Location**: `apps/air-quality-app/src/main.rs` (or config loading module)

**Behavior**:
- After loading config from etcd via StreamRegistry
- Before creating subscribers
- Run semantic validation (schema already validated at sync time)
- Focus on runtime-relevant checks (table existence, source connectivity)

```rust
// Pseudocode for runtime validation
async fn validate_config_at_runtime(config: &StreamConfig, pool: &PgPool) -> Result<()> {
    // Check table exists
    if let Some(silver_etl) = &config.silver_etl {
        let table_exists = check_table_exists(pool, &silver_etl.target_table).await?;
        if !table_exists {
            return Err(anyhow!(
                "Config validation failed: table '{}' does not exist",
                silver_etl.target_table
            ));
        }
    }

    // Additional runtime checks...
    Ok(())
}
```

### 6.3 Validator CLI Interface

**Binary**: `tools/ndp-validate/` or `apps/ndp-validate/`

```bash
# Usage
ndp-validate [OPTIONS] [CONFIG_PATH]

# Options
  --all                 Validate all configs in config/base/streams/
  --schema-only         Skip semantic validation (fast mode)
  --check-tables        Include table existence check (requires DB)
  --check-source-paths  Include source_path cross-reference check
  --format <FORMAT>     Output format: json (default), human
  --strict              Treat warnings as errors
  --verbose             Show validation progress

# Examples
ndp-validate config/base/streams/air-quality/config.json
ndp-validate --all --format human
ndp-validate --all --check-tables --format json > validation-report.json
ndp-validate --schema-only config.json  # Fast, no DB needed
```

---

## 7. Constraints

### 7.1 Technical Constraints

| Constraint | Impact | Mitigation |
|------------|--------|------------|
| **Rust binary for Pi** | Must cross-compile for ARM64 | Use existing cross-compilation setup |
| **No Python dependency** | Can't use Python validators | Rust `jsonschema` crate |
| **etcd not always available** | Schema validation must work offline | `--schema-only` mode, DB checks optional |
| **dp-018 dependency** | JSON configs and ConfigSyncService must exist | Sequence: dp-018 before dp-019 |

### 7.2 Architectural Constraints

| Constraint | Description | Source |
|------------|-------------|--------|
| **JSON Schema v1.1** | Validator must work with v1.1 schema (supports both patterns) | dp-018 |
| **StreamConfig struct** | Validation operates on same struct as runtime | dp-018 ADR-018-001 |
| **Existing error format** | Must integrate with existing logging/error patterns | Codebase consistency |
| **Two layers are distinct** | Schema validation is offline; semantic may need resources | Design decision |

### 7.3 Business Constraints

| Constraint | Description |
|------------|-------------|
| **dp-018 prerequisite** | JSON configs must exist before validation can be built |
| **dp-017 prerequisite** | Integration environment needed for testing validation |
| **Research before implementation** | SUPPORTED-VALUES.md must be complete before enum validation |

---

## 8. Data Model Specification

### 8.1 Validation Result Structure

```yaml
entities:
  ValidationResult:
    attributes:
      - valid: boolean (true if no errors)
      - config_path: string (path to validated config)
      - summary: ValidationSummary
      - errors: array<ValidationError>
      - warnings: array<ValidationError>

  ValidationSummary:
    attributes:
      - total_errors: integer
      - total_warnings: integer
      - by_layer: map<string, integer>

  ValidationError:
    attributes:
      - layer: enum [syntax, schema, semantic]
      - code: string (error code from standard list)
      - path: string (JSONPath to error location)
      - message: string (human-readable description)
      - severity: enum [error, warning]
      - suggestion: string (optional, actionable fix)
      - context: object (optional, additional context)
```

### 8.2 Supported Values Registry (Research Output)

```yaml
entities:
  FieldTypeRegistry:
    attributes:
      - type_name: string (e.g., "float")
      - postgresql_type: string (e.g., "DOUBLE PRECISION")
      - parquet_type: string (e.g., "FLOAT64")
      - description: string
      - example: any

  SourceTypeRegistry:
    attributes:
      - source_type: string (e.g., "mqtt")
      - required_fields: array<string>
      - optional_fields: array<string>
      - description: string

  TransformRegistry:
    attributes:
      - transform_name: string (e.g., "to_float")
      - input_type: string
      - output_type: string
      - description: string

  DqOperatorRegistry:
    attributes:
      - operator: string (e.g., "range", "not_null")
      - syntax: string (expression pattern)
      - description: string
```

---

## 9. Interface Specification

### 9.1 Validator Library API (Rust)

```rust
/// Main validation entry point
pub struct Validator {
    schema: CompiledSchema,
    supported_types: Vec<String>,
    db_pool: Option<PgPool>,
}

impl Validator {
    /// Create validator with compiled JSON Schema
    pub fn new(schema_path: &Path) -> Result<Self>;

    /// Validate a single config
    pub async fn validate(&self, config_path: &Path, options: ValidationOptions) -> ValidationResult;

    /// Validate all configs in directory
    pub async fn validate_all(&self, base_dir: &Path, options: ValidationOptions) -> Vec<ValidationResult>;
}

pub struct ValidationOptions {
    /// Run schema validation only (fast, no DB)
    pub schema_only: bool,
    /// Check table existence in TimescaleDB
    pub check_tables: bool,
    /// Check source_path references
    pub check_source_paths: bool,
    /// Treat warnings as errors
    pub strict: bool,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            schema_only: false,
            check_tables: false,
            check_source_paths: true, // On by default
            strict: false,
        }
    }
}
```

### 9.2 CLI Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Validation passed (may have warnings) |
| 1 | Validation failed (has errors) |
| 2 | System error (file not found, DB connection failed, etc.) |

---

## 10. Validation Checklist

Before completing dp-019:

**Research Phase**:
- [ ] NDP-supported field types documented in SUPPORTED-VALUES.md
- [ ] Source type required fields documented
- [ ] Transform functions documented
- [ ] DQ operators documented
- [ ] DDL type mapping documented in DDL-GENERATION.md
- [ ] Index strategy documented

**Layer 1 (Schema)**:
- [ ] JSON Schema updated with `additionalProperties: false`
- [ ] Field type enum in schema
- [ ] Source type enum in schema
- [ ] Pattern validation for stream_id, field names
- [ ] Schema validates v1.1 configs

**Layer 2 (Semantic)**:
- [ ] source_path cross-reference validation works
- [ ] Table existence check works (optional mode)
- [ ] DQ rule syntax validation works
- [ ] Source config validation (MQTT/HTTP requirements)
- [ ] Transform validation works

**Integration**:
- [ ] `ndp-validate` binary built and tested
- [ ] deploy.sh calls validator before sync
- [ ] Runtime validation at app startup
- [ ] Structured JSON error output
- [ ] Human-readable error output mode
- [ ] All acceptance criteria pass

**Testing**:
- [ ] Unit tests for each validation rule
- [ ] Integration tests with valid configs
- [ ] Integration tests with invalid configs (all error types)
- [ ] Performance benchmark (<100ms schema, <500ms full)

---

## 11. Glossary

| Term | Definition |
|------|------------|
| **Layer 1 (Schema Validation)** | JSON Schema-based validation checking structure, types, required fields, and unknown field detection. Does not require database or application context |
| **Layer 2 (Semantic Validation)** | Rust code-based validation checking application rules, cross-references, and database existence. May require database connection |
| **JSONPath** | Standard notation for identifying specific values in JSON documents (e.g., `$.fields[0].name`) |
| **additionalProperties** | JSON Schema keyword that rejects unknown fields when set to `false` |
| **source_path** | Field in `silver_etl.field_mappings` that references a Bronze field for ETL transformation |
| **NDP-supported types** | The set of field types that the Neural Data Platform can process (float, integer, string, boolean, timestamp, json) |
| **DQ rules** | Data Quality rules that validate data during Silver ETL (range checks, not_null, etc.) |
| **ndp-validate** | The CLI tool implementing the validation pipeline |

---

## 12. Dependencies and Prerequisites

| Dependency | Type | Status | Notes |
|------------|------|--------|-------|
| dp-018: JSON Config Foundation | REQUIRED | Must complete first | JSON configs and v1.1 schema required |
| dp-017: Integration Environment | REQUIRED | Must be available | Testing validation requires integration env |
| `jsonschema` crate | Dev tool | Available | Rust JSON Schema validation |
| TimescaleDB | Optional | Available | Only for `--check-tables` mode |

---

## 13. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Research delays implementation | Medium | Medium | Timeboxed research phase, implement known rules first |
| False positives block valid configs | Low | High | Comprehensive testing, `--skip-validation` escape hatch |
| Performance too slow for CI | Low | Medium | Benchmark early, optimize schema validation |
| Missing validation rule causes runtime error | Medium | Medium | Runtime validation as defense-in-depth |
| enum list becomes stale | Low | Low | Document where enums come from, create update process |

---

## 14. Success Metrics

| Metric | Current State | After dp-019 | Measurement |
|--------|---------------|--------------|-------------|
| Unknown field detection | None (P-007) | 100% | Test with typo configs |
| source_path validation | None (P-005) | 100% | Test with bad references |
| Table existence validation | None (P-006) | Available | Test with --check-tables |
| Deploy blocked on bad config | No | Yes | Attempt deploy with invalid config |
| Structured error output | N/A | Yes | Inspect validation JSON output |
| Validation latency (schema) | N/A | <100ms | Benchmark |
| Validation latency (full) | N/A | <500ms | Benchmark |
| SUPPORTED-VALUES.md | Does not exist | Complete | File exists with all values |

---

## 15. References

| Document | Path | Relevance |
|----------|------|-----------|
| dp-019 SCOPE.md | `product/features/dp-019/SCOPE.md` | Feature scope definition |
| dp-018 SPECIFICATION.md | `product/features/dp-018/specification/SPECIFICATION.md` | JSON config foundation |
| dp-016 IMPLEMENTATION-ROADMAP.md | `product/features/dp-016/IMPLEMENTATION-ROADMAP.md` | Phase 2 details |
| dp-016 PAIN-POINTS.md | `product/features/dp-016/specification/PAIN-POINTS.md` | P-005, P-006, P-007 validation gaps |
| ADR-018-001 | `product/features/dp-018/architecture/ADR-018-001-config-loader-design.md` | JSON pass-through architecture |

---

*Specification created: 2026-02-02*
*SPARC Phase: Specification (S)*
*Next Phase: Pseudocode (P)*
