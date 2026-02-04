# SPEC-A05: Objectives JSON Schema

> **Feature ID:** v11-A05
> **Priority:** High
> **Status:** Specification
> **Dependencies:** V1.0 schema validation pipeline
> **Blocks:** Phase C (Objectives Storage), Phase E (Threshold Crossing)

---

## User Story

**As a** platform operator,
**I want** a JSON Schema that validates objectives and constraints in domain configurations,
**So that** I can declaratively define what the system should optimize for and what limits apply.

---

## Goal

Define a JSON Schema for objectives configuration that:
1. Specifies measurable targets with thresholds
2. Defines constraints that limit actions
3. Enables threshold crossing event generation
4. Provides clear, type-safe objective definitions
5. Supports V1.2 pattern detection filtering

---

## Functional Requirements

### FR-A05-001: Objective Structure

Each objective SHALL define:

```yaml
objectives:
  - id: healthy_co2
    description: "Maintain CO2 below 800ppm for cognitive performance"
    target:
      stream: air-quality
      metric: co2
      condition: "<"
      threshold: 800
      unit: ppm
    priority: high
    tags: ["health", "air-quality"]
```

### FR-A05-002: Target Definition

The `target` object SHALL include:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `stream` | string | Yes | Reference to stream_id |
| `metric` | string | Yes | Field/metric name from stream |
| `condition` | enum | Yes | Comparison operator |
| `threshold` | number | Yes | Threshold value |
| `unit` | string | No | Unit of measurement |

### FR-A05-003: Condition Operators

Valid condition operators:

| Operator | Description | Example |
|----------|-------------|---------|
| `<` | Less than | CO2 < 800 |
| `<=` | Less than or equal | PM2.5 <= 12 |
| `>` | Greater than | Temperature > 18 |
| `>=` | Greater than or equal | Humidity >= 30 |
| `==` | Equal to | State == "on" |
| `!=` | Not equal to | State != "error" |
| `between` | Range (inclusive) | Temperature between 18-24 |

### FR-A05-004: Between Condition

For range conditions, use array threshold:

```yaml
target:
  stream: air-quality
  metric: temperature_c
  condition: between
  threshold: [18, 24]  # Array for range
  unit: celsius
```

### FR-A05-005: Priority Enum

Valid priority values:
- `critical` - Must always be met
- `high` - Strong preference
- `medium` - Normal importance (default)
- `low` - Nice to have

### FR-A05-006: Constraint Structure

Constraints define conditions that MUST be true before taking action:

```yaml
constraints:
  - id: outdoor_air_safe
    description: "Only open window if outdoor air quality is acceptable"
    condition:
      stream: outdoor-air-quality
      metric: pm25
      operator: "<"
      threshold: 35
    applies_to: ["open_window"]  # Optional: which actions this constrains
```

### FR-A05-007: Time-Based Objectives

Objectives MAY include time-based criteria:

```yaml
objectives:
  - id: night_quiet
    description: "Maintain quiet conditions at night"
    target:
      stream: home-assistant-state
      metric: noise_level
      condition: "<"
      threshold: 40
    time_window:
      start: "22:00"
      end: "07:00"
      timezone: "local"
```

### FR-A05-008: Objective Dependencies

Objectives MAY reference other objectives:

```yaml
objectives:
  - id: healthy_air_composite
    description: "Combined air quality objective"
    depends_on:
      - healthy_co2
      - healthy_pm25
      - acceptable_humidity
    aggregation: all  # "all" or "any"
```

### FR-A05-009: Unique Objective IDs

All `id` values within a domain SHALL be unique across both objectives and constraints.

### FR-A05-010: Metric Validation

The `metric` field SHALL reference a valid field in the target stream's configuration. This is semantic validation (not schema validation).

---

## Non-Functional Requirements

### NFR-A05-001: Schema Composability

The objectives schema SHALL be:
- Referenced from `domain.schema.json`
- Independently validatable
- Extensible for future objective types

### NFR-A05-002: Error Messages

Validation errors SHALL clearly indicate:
- Which objective has the issue
- What field is invalid
- Expected format/values

---

## Acceptance Criteria

### AC-A05-001: Valid Objectives Accepted

```gherkin
Scenario: Valid objectives configuration passes validation
  Given a domain config with objectives section
  And each objective has id, target with stream, metric, condition, threshold
  When I run ndp-validate on the configuration
  Then validation SHALL pass with exit code 0
```

### AC-A05-002: Invalid Condition Rejected

```gherkin
Scenario: Invalid condition operator is rejected
  Given an objective with target.condition = "less_than"
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code ENUM_VIOLATION
  And the error message SHALL list valid operators: <, <=, >, >=, ==, !=, between
```

### AC-A05-003: Between Condition Requires Array

```gherkin
Scenario: Between condition with single value is rejected
  Given an objective with target.condition = "between"
  And target.threshold = 20 (not an array)
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error message SHALL indicate "between condition requires array threshold [min, max]"
```

### AC-A05-004: Invalid Metric Reference

```gherkin
Scenario: Reference to non-existent metric is rejected
  Given an objective targeting metric "nonexistent_field"
  And the stream does not have a field named "nonexistent_field"
  When I run ndp-validate --semantic on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code INVALID_GOLD_FIELD (400)
```

### AC-A05-005: Duplicate Objective ID Rejected

```gherkin
Scenario: Duplicate objective IDs are rejected
  Given two objectives with id = "healthy_co2"
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code DUPLICATE_NAME
```

### AC-A05-006: Missing Required Fields Rejected

```gherkin
Scenario: Objective without threshold is rejected
  Given an objective without target.threshold
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code MISSING_REQUIRED
  And the error path SHALL be "$.domain.objectives[0].target.threshold"
```

### AC-A05-007: Time Window Validation

```gherkin
Scenario: Invalid time window format is rejected
  Given an objective with time_window.start = "10pm"
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error message SHALL indicate expected format "HH:MM"
```

---

## Schema Location

**File:** `config/schemas/objectives.schema.json`

### objectives.schema.json

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "objectives.schema.json",
  "title": "NDP Objectives Configuration",
  "description": "Schema for objectives and constraints in domain configurations",

  "definitions": {
    "condition_operator": {
      "type": "string",
      "enum": ["<", "<=", ">", ">=", "==", "!=", "between"],
      "description": "Comparison operator for threshold"
    },

    "priority": {
      "type": "string",
      "enum": ["critical", "high", "medium", "low"],
      "default": "medium",
      "description": "Objective priority level"
    },

    "time_24h": {
      "type": "string",
      "pattern": "^([01]?[0-9]|2[0-3]):[0-5][0-9]$",
      "description": "Time in 24-hour format (HH:MM)"
    },

    "time_window": {
      "type": "object",
      "properties": {
        "start": { "$ref": "#/definitions/time_24h" },
        "end": { "$ref": "#/definitions/time_24h" },
        "timezone": {
          "type": "string",
          "enum": ["local", "utc"],
          "default": "local"
        },
        "days": {
          "type": "array",
          "items": {
            "type": "string",
            "enum": ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
          },
          "description": "Days when this window applies"
        }
      },
      "required": ["start", "end"]
    },

    "target": {
      "type": "object",
      "properties": {
        "stream": {
          "type": "string",
          "description": "Reference to stream_id"
        },
        "metric": {
          "type": "string",
          "description": "Field name from stream"
        },
        "condition": {
          "$ref": "#/definitions/condition_operator"
        },
        "threshold": {
          "oneOf": [
            { "type": "number" },
            {
              "type": "array",
              "items": { "type": "number" },
              "minItems": 2,
              "maxItems": 2,
              "description": "Range for 'between' condition [min, max]"
            },
            { "type": "string", "description": "For state comparisons" }
          ],
          "description": "Threshold value or range"
        },
        "unit": {
          "type": "string",
          "description": "Unit of measurement"
        }
      },
      "required": ["stream", "metric", "condition", "threshold"],
      "additionalProperties": false,
      "allOf": [
        {
          "if": {
            "properties": { "condition": { "const": "between" } }
          },
          "then": {
            "properties": {
              "threshold": {
                "type": "array",
                "items": { "type": "number" },
                "minItems": 2,
                "maxItems": 2
              }
            }
          }
        }
      ]
    },

    "objective": {
      "type": "object",
      "properties": {
        "id": {
          "type": "string",
          "pattern": "^[a-z][a-z0-9_]*$",
          "maxLength": 50,
          "description": "Unique objective identifier"
        },
        "description": {
          "type": "string",
          "description": "Human-readable description"
        },
        "target": {
          "$ref": "#/definitions/target"
        },
        "priority": {
          "$ref": "#/definitions/priority"
        },
        "time_window": {
          "$ref": "#/definitions/time_window",
          "description": "Optional time-based applicability"
        },
        "tags": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Categorization tags"
        },
        "depends_on": {
          "type": "array",
          "items": { "type": "string" },
          "description": "References to other objective IDs"
        },
        "aggregation": {
          "type": "string",
          "enum": ["all", "any"],
          "default": "all",
          "description": "How to combine dependent objectives"
        }
      },
      "required": ["id", "target"],
      "additionalProperties": false
    },

    "constraint_condition": {
      "type": "object",
      "properties": {
        "stream": { "type": "string" },
        "metric": { "type": "string" },
        "operator": { "$ref": "#/definitions/condition_operator" },
        "threshold": {
          "oneOf": [
            { "type": "number" },
            { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 },
            { "type": "string" }
          ]
        }
      },
      "required": ["stream", "metric", "operator", "threshold"]
    },

    "constraint": {
      "type": "object",
      "properties": {
        "id": {
          "type": "string",
          "pattern": "^[a-z][a-z0-9_]*$",
          "maxLength": 50,
          "description": "Unique constraint identifier"
        },
        "description": {
          "type": "string",
          "description": "Human-readable description"
        },
        "condition": {
          "$ref": "#/definitions/constraint_condition"
        },
        "applies_to": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Actions this constraint applies to"
        }
      },
      "required": ["id", "condition"],
      "additionalProperties": false
    },

    "objectives_list": {
      "type": "array",
      "items": { "$ref": "#/definitions/objective" },
      "description": "List of objectives"
    },

    "constraints_list": {
      "type": "array",
      "items": { "$ref": "#/definitions/constraint" },
      "description": "List of constraints"
    }
  }
}
```

---

## Example Configuration

```yaml
domain:
  id: indoor-air-quality
  description: "Optimize indoor air quality"

  streams:
    - stream_id: air-quality
      alias: indoor
      role: primary
    - stream_id: outdoor-air-quality
      alias: outdoor_aqi
      role: constraint

  alignment:
    view_name: indoor_air_quality_aligned
    granularity: "1 hour"

  objectives:
    - id: healthy_co2
      description: "Maintain CO2 below 800ppm for cognitive performance"
      target:
        stream: air-quality
        metric: co2
        condition: "<"
        threshold: 800
        unit: ppm
      priority: high
      tags: ["health", "cognitive"]

    - id: healthy_pm25
      description: "Keep PM2.5 below EPA 24-hour standard"
      target:
        stream: air-quality
        metric: pm25
        condition: "<"
        threshold: 12
        unit: µg/m³
      priority: high
      tags: ["health", "respiratory"]

    - id: comfortable_temp
      description: "Maintain comfortable temperature range"
      target:
        stream: air-quality
        metric: temperature_c
        condition: between
        threshold: [20, 24]
        unit: celsius
      priority: medium
      tags: ["comfort"]

    - id: night_quiet
      description: "Reduce ventilation noise at night"
      target:
        stream: home-assistant-state
        metric: fan_speed
        condition: "<="
        threshold: 2
      priority: low
      time_window:
        start: "22:00"
        end: "07:00"
        timezone: local

  constraints:
    - id: outdoor_air_safe
      description: "Only ventilate when outdoor air is acceptable"
      condition:
        stream: outdoor-air-quality
        metric: pm25
        operator: "<"
        threshold: 35
      applies_to: ["open_window", "increase_ventilation"]

    - id: not_raining
      description: "Don't open window when raining"
      condition:
        stream: outdoor-weather
        metric: precipitation
        operator: "=="
        threshold: 0
      applies_to: ["open_window"]
```

---

## Integration Test Requirements

### Test: Objective Schema Validation

```rust
#[test]
fn test_valid_objectives() {
    let config = json!({
        "objectives": [{
            "id": "healthy_co2",
            "target": {
                "stream": "air-quality",
                "metric": "co2",
                "condition": "<",
                "threshold": 800
            },
            "priority": "high"
        }]
    });

    let result = validate_objectives_schema(&config);
    assert!(result.is_ok());
}

#[test]
fn test_between_requires_array() {
    let config = json!({
        "objectives": [{
            "id": "temp_range",
            "target": {
                "stream": "air-quality",
                "metric": "temperature",
                "condition": "between",
                "threshold": 20  // Should be [min, max]
            }
        }]
    });

    let result = validate_objectives_schema(&config);
    assert!(result.is_err());
}
```

### Test: Metric Reference Validation

```rust
#[test]
fn test_invalid_metric_reference() {
    let config = create_test_domain_with_objective("nonexistent_metric");
    let streams = create_mock_streams();  // Without "nonexistent_metric"

    let errors = semantic_validate(&config, &streams);

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, ErrorCode::InvalidGoldField);
}
```

---

## London TDD Interfaces

### Trait: ObjectiveValidator

```rust
pub trait ObjectiveValidator {
    fn validate_schema(&self, objectives: &Value) -> Vec<ValidationError>;
    fn validate_references(&self, objectives: &[Objective], resolver: &dyn StreamResolver) -> Vec<ValidationError>;
}
```

### Trait: ThresholdCrossingDetector (for Phase E)

```rust
pub trait ThresholdCrossingDetector {
    fn detect_crossings(&self, objective: &Objective, data: &[DataPoint]) -> Vec<ThresholdCrossing>;
}

pub struct ThresholdCrossing {
    pub event_time: DateTime<Utc>,
    pub objective_id: String,
    pub direction: CrossingDirection,  // Rising, Falling
    pub previous_value: f64,
    pub current_value: f64,
    pub threshold: f64,
}
```

---

## Semantic Validation Rules

### Rule: INVALID_OBJECTIVE_CONDITION (408)

```rust
fn validate_between_threshold(objective: &Objective) -> Option<ValidationError> {
    if objective.target.condition == "between" {
        match &objective.target.threshold {
            Threshold::Range(min, max) if min < max => None,
            Threshold::Range(min, max) => Some(ValidationError::semantic_error(
                ErrorCode::InvalidObjectiveCondition,
                &format!("$.objectives[{}].target.threshold", objective.id),
                format!("Between range must have min < max, got [{}, {}]", min, max)
            )),
            _ => Some(ValidationError::semantic_error(
                ErrorCode::InvalidObjectiveCondition,
                &format!("$.objectives[{}].target.threshold", objective.id),
                "Between condition requires array threshold [min, max]"
            ))
        }
    } else {
        None
    }
}
```

### Rule: Unique IDs Across Objectives and Constraints

```rust
fn validate_unique_ids(domain: &DomainConfig) -> Vec<ValidationError> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut errors = Vec::new();

    for (idx, obj) in domain.objectives.iter().enumerate() {
        if !seen.insert(obj.id.clone()) {
            errors.push(ValidationError::semantic_error(
                ErrorCode::DuplicateName,
                &format!("$.domain.objectives[{}].id", idx),
                format!("Duplicate objective ID '{}'", obj.id)
            ));
        }
    }

    for (idx, constraint) in domain.constraints.iter().enumerate() {
        if !seen.insert(constraint.id.clone()) {
            errors.push(ValidationError::semantic_error(
                ErrorCode::DuplicateName,
                &format!("$.domain.constraints[{}].id", idx),
                format!("Duplicate constraint ID '{}' (conflicts with objective or another constraint)", constraint.id)
            ));
        }
    }

    errors
}
```

---

## References

- [SCOPE.md](../../SCOPE.md) - Objectives framework description
- [DECISIONS.md](../../architecture/DECISIONS.md) - Decision 6: Domain-Centric Configuration
- [SPEC-A03](./SPEC-A03-alignment-schema.md) - Domain schema (references objectives)
