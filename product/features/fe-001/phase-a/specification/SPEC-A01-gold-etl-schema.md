# SPEC-A01: Gold ETL JSON Schema

> **Feature ID:** v11-A01
> **Priority:** Critical
> **Status:** Specification
> **Dependencies:** V1.0 schema validation pipeline
> **Blocks:** v11-A02 (Gold DDL Tool), ndp-validate Gold validation

---

## User Story

**As a** platform operator,
**I want** a JSON Schema that validates the `gold_etl` section of stream configurations,
**So that** I receive immediate, helpful feedback when my Gold layer configuration is invalid.

---

## Goal

Define a JSON Schema for the `gold_etl` configuration section that:
1. Validates structure, types, and enums
2. Integrates with the existing two-layer validation pipeline
3. Provides helpful error messages for common mistakes
4. Enables config-driven Gold layer deployment

---

## Functional Requirements

### FR-A01-001: Schema Definition

The schema SHALL define the following structure:

```json
{
  "gold_etl": {
    "enabled": boolean,
    "description": string (optional),

    "aggregates": {
      "granularities": ["1 hour", "1 day", ...],
      "default_metrics": ["mean", "std", ...],
      "fields": {
        "<field_name>": {
          "metrics": ["mean", "std", "min", "max", "p95", "p99", "count"],
          "exclude_from_default": boolean (optional)
        }
      }
    },

    "features": {
      "lag": {
        "enabled": boolean,
        "lags_hours": [1, 6, 24, ...],
        "fields": ["field1", "field2", ...]
      },
      "rolling": {
        "enabled": boolean,
        "windows": ["4 hours", "24 hours", ...],
        "stats": ["mean", "std"],
        "fields": ["field1", ...]
      },
      "trend": {
        "enabled": boolean,
        "window": "4 hours",
        "fields": ["field1", ...]
      }
    },

    "transitions": {
      "enabled": boolean,
      "state_field": string,
      "entity_field": string,
      "track_duration": boolean,
      "include_in_alignment": boolean
    }
  }
}
```

### FR-A01-002: Granularity Pattern

The schema SHALL validate granularity strings using the pattern:
```regex
^\d+\s+(minute|hour|day)s?$
```

Valid examples: `"1 hour"`, `"15 minutes"`, `"1 day"`, `"7 days"`
Invalid examples: `"1hr"`, `"hourly"`, `"60m"`

### FR-A01-003: Metric Enum

The schema SHALL define allowed aggregate metrics:
- `mean` - Average value
- `std` - Standard deviation
- `min` - Minimum value
- `max` - Maximum value
- `count` - Number of samples
- `p95` - 95th percentile
- `p99` - 99th percentile
- `first` - First value in bucket
- `last` - Last value in bucket

### FR-A01-004: Feature Stats Enum

The schema SHALL define allowed feature statistics:
- `mean` - Rolling mean
- `std` - Rolling standard deviation
- `min` - Rolling minimum
- `max` - Rolling maximum

### FR-A01-005: Default Behavior

When `gold_etl.enabled` is `true`:
- If `aggregates.default_metrics` is not specified, default to `["mean", "count"]`
- If `features.lag.fields` is empty, apply to all numeric fields
- If `transitions.state_field` is not specified, default to `"state"`
- If `transitions.entity_field` is not specified, default to `"ndp_id"`

### FR-A01-006: Schema Integration

The schema SHALL be referenced from `stream-config.v2.schema.json`:
```json
{
  "properties": {
    "gold_etl": {
      "$ref": "gold-etl.schema.json#/definitions/gold_etl"
    }
  }
}
```

---

## Non-Functional Requirements

### NFR-A01-001: Error Message Quality

Error messages SHALL include:
- The exact JSON path where the error occurred (e.g., `$.gold_etl.aggregates.fields.pm25.metrics[0]`)
- What was expected vs what was found
- A suggestion for correction when possible

### NFR-A01-002: Schema Validation Performance

Schema validation SHALL complete in < 50ms for typical stream configurations.

### NFR-A01-003: Backward Compatibility

Stream configurations without `gold_etl` section SHALL continue to validate successfully. The `gold_etl` field is optional.

---

## Acceptance Criteria

### AC-A01-001: Valid Configuration Accepted

```gherkin
Scenario: Valid gold_etl configuration passes validation
  Given a stream configuration file with a valid gold_etl section
  When I run ndp-validate on the configuration
  Then validation SHALL pass with exit code 0
  And no errors or warnings SHALL be reported
```

### AC-A01-002: Invalid Metric Rejected

```gherkin
Scenario: Invalid metric type is rejected
  Given a stream configuration with gold_etl.aggregates.fields.pm25.metrics = ["mean", "average"]
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code ENUM_VIOLATION
  And the error path SHALL be "$.gold_etl.aggregates.fields.pm25.metrics[1]"
  And the error message SHALL list valid metric values
```

### AC-A01-003: Invalid Granularity Rejected

```gherkin
Scenario: Invalid granularity format is rejected
  Given a stream configuration with gold_etl.aggregates.granularities = ["1 hour", "hourly"]
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code PATTERN_MISMATCH
  And the error message SHALL show the expected pattern
  And the suggestion SHALL include valid examples like "1 hour"
```

### AC-A01-004: Missing Required Field Rejected

```gherkin
Scenario: Enabled gold_etl without aggregates is rejected
  Given a stream configuration with gold_etl.enabled = true
  And gold_etl.aggregates is not present
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code MISSING_REQUIRED
  And the error path SHALL be "$.gold_etl"
```

### AC-A01-005: Transition on Non-State Stream Warning

```gherkin
Scenario: Transitions config on observation stream raises semantic warning
  Given a stream configuration with stream_type = "observation"
  And gold_etl.transitions.enabled = true
  When I run ndp-validate on the configuration
  Then validation SHALL pass with warnings
  And the warning SHALL include code INVALID_STREAM_TYPE (401)
  And the warning message SHALL explain transitions apply to state_event streams
```

### AC-A01-006: Helpful Error for Typos

```gherkin
Scenario: Unknown field with similar name suggests correction
  Given a stream configuration with gold_etl.agregates instead of gold_etl.aggregates
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code UNKNOWN_FIELD
  And the suggestion SHALL be "Did you mean 'aggregates'?"
```

---

## Schema Location

**File:** `config/schemas/gold-etl.schema.json`

### Schema Structure

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "gold-etl.schema.json",
  "title": "NDP Gold ETL Configuration Schema",
  "description": "Schema for the gold_etl section of NDP stream configurations",

  "definitions": {
    "granularity": {
      "type": "string",
      "pattern": "^\\d+\\s+(minute|hour|day)s?$",
      "description": "Time bucket granularity (e.g., '1 hour', '15 minutes', '1 day')"
    },

    "metric": {
      "type": "string",
      "enum": ["mean", "std", "min", "max", "count", "p95", "p99", "first", "last"],
      "description": "Aggregate metric type"
    },

    "stat": {
      "type": "string",
      "enum": ["mean", "std", "min", "max"],
      "description": "Feature statistic type"
    },

    "aggregates": {
      "type": "object",
      "properties": {
        "granularities": {
          "type": "array",
          "items": { "$ref": "#/definitions/granularity" },
          "minItems": 1,
          "description": "Time bucket sizes for aggregation"
        },
        "default_metrics": {
          "type": "array",
          "items": { "$ref": "#/definitions/metric" },
          "default": ["mean", "count"],
          "description": "Default metrics applied to all fields"
        },
        "fields": {
          "type": "object",
          "additionalProperties": {
            "type": "object",
            "properties": {
              "metrics": {
                "type": "array",
                "items": { "$ref": "#/definitions/metric" },
                "description": "Metrics to compute for this field"
              },
              "exclude_from_default": {
                "type": "boolean",
                "default": false,
                "description": "Exclude this field from default metrics"
              }
            }
          },
          "description": "Per-field metric configuration"
        }
      },
      "required": ["granularities"]
    },

    "lag_features": {
      "type": "object",
      "properties": {
        "enabled": { "type": "boolean", "default": false },
        "lags_hours": {
          "type": "array",
          "items": { "type": "integer", "minimum": 1 },
          "description": "Lag offsets in hours"
        },
        "fields": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Fields to compute lags for"
        }
      }
    },

    "rolling_features": {
      "type": "object",
      "properties": {
        "enabled": { "type": "boolean", "default": false },
        "windows": {
          "type": "array",
          "items": { "$ref": "#/definitions/granularity" },
          "description": "Rolling window sizes"
        },
        "stats": {
          "type": "array",
          "items": { "$ref": "#/definitions/stat" },
          "description": "Statistics to compute"
        },
        "fields": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Fields to compute rolling stats for"
        }
      }
    },

    "trend_features": {
      "type": "object",
      "properties": {
        "enabled": { "type": "boolean", "default": false },
        "window": {
          "$ref": "#/definitions/granularity",
          "description": "Window for trend calculation"
        },
        "fields": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Fields to compute trends for"
        }
      }
    },

    "transitions": {
      "type": "object",
      "properties": {
        "enabled": { "type": "boolean", "default": false },
        "state_field": {
          "type": "string",
          "default": "state",
          "description": "Field containing state value"
        },
        "entity_field": {
          "type": "string",
          "default": "ndp_id",
          "description": "Field identifying the entity"
        },
        "track_duration": {
          "type": "boolean",
          "default": true,
          "description": "Track duration in previous state"
        },
        "include_in_alignment": {
          "type": "boolean",
          "default": true,
          "description": "Include transitions in aligned view"
        }
      }
    },

    "gold_etl": {
      "type": "object",
      "properties": {
        "enabled": {
          "type": "boolean",
          "default": false,
          "description": "Enable Gold layer for this stream"
        },
        "description": {
          "type": "string",
          "description": "Human-readable description of Gold transformation"
        },
        "aggregates": { "$ref": "#/definitions/aggregates" },
        "features": {
          "type": "object",
          "properties": {
            "lag": { "$ref": "#/definitions/lag_features" },
            "rolling": { "$ref": "#/definitions/rolling_features" },
            "trend": { "$ref": "#/definitions/trend_features" }
          },
          "additionalProperties": false
        },
        "transitions": { "$ref": "#/definitions/transitions" }
      },
      "additionalProperties": false,
      "if": {
        "properties": { "enabled": { "const": true } },
        "required": ["enabled"]
      },
      "then": {
        "required": ["aggregates"]
      }
    }
  }
}
```

---

## Integration Test Requirements

### Test: Schema Validation Pipeline Integration

```rust
#[test]
fn test_gold_etl_schema_integration() {
    // Load the schema
    let schema = load_schema("config/schemas/gold-etl.schema.json");

    // Test valid configuration
    let valid_config = json!({
        "gold_etl": {
            "enabled": true,
            "aggregates": {
                "granularities": ["1 hour"],
                "fields": {
                    "pm25": { "metrics": ["mean", "std", "max"] }
                }
            }
        }
    });
    assert!(validate(&schema, &valid_config).is_ok());

    // Test invalid metric
    let invalid_config = json!({
        "gold_etl": {
            "enabled": true,
            "aggregates": {
                "granularities": ["1 hour"],
                "fields": {
                    "pm25": { "metrics": ["average"] }  // Invalid
                }
            }
        }
    });
    let errors = validate(&schema, &invalid_config).unwrap_err();
    assert_eq!(errors[0].code, ErrorCode::EnumViolation);
}
```

### Test: ndp-validate Integration

```bash
# Test valid config
ndp-validate config/base/streams/air-quality/config.json
# Expected: Exit 0, no errors

# Test invalid config
echo '{"gold_etl": {"enabled": true}}' > /tmp/invalid.json
ndp-validate /tmp/invalid.json
# Expected: Exit 1, MISSING_REQUIRED error for aggregates
```

---

## London TDD Interfaces

### Mock: SchemaLoader

```rust
trait SchemaLoader {
    fn load_schema(&self, path: &str) -> Result<JsonSchema, SchemaLoadError>;
}

// Production implementation loads from filesystem
// Test implementation returns pre-defined schemas
```

### Mock: ValidationReporter

```rust
trait ValidationReporter {
    fn report_error(&self, error: ValidationError);
    fn report_warning(&self, warning: ValidationError);
    fn has_errors(&self) -> bool;
}
```

---

## References

- [DECISIONS.md](../../architecture/DECISIONS.md) - Decision 2: Schema Validation
- [ndp-validate](../../../../tools/ndp-validate/) - Existing validation tool
- [Two-Layer Validation Pattern](../../architecture/schema-validation-patterns.md)
