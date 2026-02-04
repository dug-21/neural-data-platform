# SPEC-A03: Alignment JSON Schema

> **Feature ID:** v11-A03
> **Priority:** Critical
> **Status:** Specification
> **Dependencies:** V1.0 schema validation pipeline
> **Blocks:** v11-A04 (Alignment Interpreter)

---

## User Story

**As a** platform operator,
**I want** a JSON Schema that validates the alignment section of domain configurations,
**So that** I can declaratively specify how multiple streams should be joined for correlation analysis.

---

## Goal

Define a JSON Schema for domain alignment configuration that:
1. Specifies which streams to include in an aligned view
2. Defines join strategy and granularity
3. Configures NULL handling per stream type
4. Validates stream references exist
5. Integrates with the two-layer validation pipeline

---

## Functional Requirements

### FR-A03-001: Domain Config Structure

The schema SHALL define the complete domain configuration structure:

```yaml
domain:
  id: <domain_id>
  description: <human_readable_description>

  streams:
    - stream_id: <stream_id>
      alias: <short_alias>
      role: primary | context | actuator | constraint

  alignment:
    view_name: <view_name>
    granularity: "1 hour"
    join_strategy: full_outer | left | inner
    null_handling: preserve | interpolate | carry_forward
    timestamp_alignment: bucket_start | bucket_end

  objectives:
    - id: <objective_id>
      # ... (covered by SPEC-A05)
```

### FR-A03-002: Stream Reference

Each stream entry in `domain.streams[]` SHALL:
- Reference an existing stream by `stream_id`
- Define an optional `alias` for column naming
- Specify a `role` enum:
  - `primary` - The target variable being optimized
  - `context` - Environmental/contextual data
  - `actuator` - Potential causes/actions
  - `constraint` - Limits on actions

### FR-A03-003: Alignment Configuration

The `alignment` section SHALL define:

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `view_name` | string | Yes | - | Name of generated view |
| `granularity` | granularity | Yes | - | Time bucket size |
| `join_strategy` | enum | No | `full_outer` | How to join streams |
| `null_handling` | enum | No | `preserve` | Default NULL strategy |
| `timestamp_alignment` | enum | No | `bucket_start` | Bucket timestamp position |

### FR-A03-004: Join Strategy Enum

Valid join strategies:
- `full_outer` - Preserve all rows from all streams (default)
- `left` - Keep all rows from first (primary) stream
- `inner` - Only keep rows where all streams have data

### FR-A03-005: NULL Handling Enum

Valid NULL handling strategies:
- `preserve` - Keep NULLs as-is (default for observations, forecasts)
- `carry_forward` - Last Observation Carried Forward (LOCF) (default for state_event)
- `interpolate` - Linear interpolation (only for numeric fields)

### FR-A03-006: Stream-Specific NULL Override

Each stream reference MAY override the default NULL handling:

```yaml
streams:
  - stream_id: air-quality
    alias: indoor
    role: primary
    null_handling: preserve  # Override for this stream
```

### FR-A03-007: View Naming Pattern

The `view_name` SHALL follow the pattern:
- `{domain_id}_aligned` (recommended)
- Must be valid PostgreSQL identifier
- Maximum 63 characters

### FR-A03-008: Granularity Validation

Granularity SHALL use the same pattern as Gold ETL:
```regex
^\d+\s+(minute|hour|day)s?$
```

### FR-A03-009: Unique Aliases

All `alias` values within a domain SHALL be unique. This prevents column name collisions in the aligned view.

---

## Non-Functional Requirements

### NFR-A03-001: Error Message Quality

Validation errors SHALL include:
- The domain config file path
- The JSON path to the error
- For stream references: list of valid stream IDs if unknown

### NFR-A03-002: Schema Composability

The alignment schema SHALL be composable:
- Referenced from `domain.schema.json`
- Can be validated independently for testing
- Follows JSON Schema $ref patterns

---

## Acceptance Criteria

### AC-A03-001: Valid Domain Config Accepted

```gherkin
Scenario: Valid domain configuration passes validation
  Given a domain config file at config/domains/indoor-air-quality/domain.yaml
  And the alignment section specifies view_name, granularity, and streams
  When I run ndp-validate on the domain configuration
  Then validation SHALL pass with exit code 0
```

### AC-A03-002: Invalid Join Strategy Rejected

```gherkin
Scenario: Invalid join strategy is rejected
  Given a domain config with alignment.join_strategy = "outer"
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code ENUM_VIOLATION
  And the error message SHALL list valid strategies: full_outer, left, inner
```

### AC-A03-003: Unknown Stream Reference Rejected

```gherkin
Scenario: Reference to non-existent stream is rejected
  Given a domain config with streams[].stream_id = "unknown-stream"
  And no stream config exists for "unknown-stream"
  When I run ndp-validate --semantic on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code UNKNOWN_ALIGNMENT_STREAM (402)
  And the suggestion SHALL list available stream IDs
```

### AC-A03-004: Duplicate Alias Rejected

```gherkin
Scenario: Duplicate stream aliases are rejected
  Given a domain config with two streams having alias = "indoor"
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code DUPLICATE_NAME
  And the error message SHALL identify the duplicate alias
```

### AC-A03-005: Missing View Name Rejected

```gherkin
Scenario: Missing required view_name is rejected
  Given a domain config without alignment.view_name
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code MISSING_REQUIRED
  And the error path SHALL be "$.domain.alignment.view_name"
```

### AC-A03-006: Primary Role Required

```gherkin
Scenario: Domain requires at least one primary stream
  Given a domain config where no stream has role = "primary"
  When I run ndp-validate --semantic on the configuration
  Then validation SHALL fail with exit code 1
  And the error message SHALL indicate "Domain requires at least one stream with role: primary"
```

---

## Schema Location

**Files:**
- `config/schemas/domain.schema.json` - Main domain schema
- `config/schemas/alignment.schema.json` - Alignment definitions (referenced)

### alignment.schema.json

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "alignment.schema.json",
  "title": "NDP Domain Alignment Configuration",
  "description": "Schema for cross-stream alignment in domain configurations",

  "definitions": {
    "granularity": {
      "type": "string",
      "pattern": "^\\d+\\s+(minute|hour|day)s?$",
      "description": "Time bucket granularity"
    },

    "stream_role": {
      "type": "string",
      "enum": ["primary", "context", "actuator", "constraint"],
      "description": "Role of stream in the domain"
    },

    "join_strategy": {
      "type": "string",
      "enum": ["full_outer", "left", "inner"],
      "default": "full_outer",
      "description": "SQL join strategy"
    },

    "null_handling": {
      "type": "string",
      "enum": ["preserve", "carry_forward", "interpolate"],
      "default": "preserve",
      "description": "NULL handling strategy"
    },

    "timestamp_alignment": {
      "type": "string",
      "enum": ["bucket_start", "bucket_end"],
      "default": "bucket_start",
      "description": "Whether bucket timestamp is start or end of period"
    },

    "stream_reference": {
      "type": "object",
      "properties": {
        "stream_id": {
          "type": "string",
          "description": "Reference to existing stream configuration"
        },
        "alias": {
          "type": "string",
          "pattern": "^[a-z][a-z0-9_]*$",
          "maxLength": 20,
          "description": "Short alias for column naming"
        },
        "role": {
          "$ref": "#/definitions/stream_role"
        },
        "null_handling": {
          "$ref": "#/definitions/null_handling",
          "description": "Override default NULL handling for this stream"
        }
      },
      "required": ["stream_id", "role"],
      "additionalProperties": false
    },

    "alignment": {
      "type": "object",
      "properties": {
        "view_name": {
          "type": "string",
          "pattern": "^[a-z][a-z0-9_]*$",
          "maxLength": 63,
          "description": "Name for the aligned view"
        },
        "granularity": {
          "$ref": "#/definitions/granularity"
        },
        "join_strategy": {
          "$ref": "#/definitions/join_strategy"
        },
        "null_handling": {
          "$ref": "#/definitions/null_handling",
          "description": "Default NULL handling for all streams"
        },
        "timestamp_alignment": {
          "$ref": "#/definitions/timestamp_alignment"
        }
      },
      "required": ["view_name", "granularity"],
      "additionalProperties": false
    }
  }
}
```

### domain.schema.json (referencing alignment)

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "domain.schema.json",
  "title": "NDP Domain Configuration",
  "description": "Schema for domain configurations combining streams, alignment, and objectives",

  "type": "object",
  "properties": {
    "domain": {
      "type": "object",
      "properties": {
        "id": {
          "type": "string",
          "pattern": "^[a-z][a-z0-9-]*$",
          "description": "Unique domain identifier"
        },
        "description": {
          "type": "string",
          "description": "Human-readable domain description"
        },
        "streams": {
          "type": "array",
          "items": {
            "$ref": "alignment.schema.json#/definitions/stream_reference"
          },
          "minItems": 1,
          "description": "Streams included in this domain"
        },
        "alignment": {
          "$ref": "alignment.schema.json#/definitions/alignment"
        },
        "objectives": {
          "$ref": "objectives.schema.json#/definitions/objectives_list"
        },
        "constraints": {
          "$ref": "objectives.schema.json#/definitions/constraints_list"
        }
      },
      "required": ["id", "streams", "alignment"],
      "additionalProperties": false
    }
  },
  "required": ["domain"],
  "additionalProperties": false
}
```

---

## Example Domain Config

```yaml
domain:
  id: indoor-air-quality
  description: "Maintain healthy indoor air quality through ventilation"

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
    null_handling: preserve
    timestamp_alignment: bucket_start

  objectives:
    - id: healthy_co2
      target:
        stream: air-quality
        metric: co2
        condition: "<"
        threshold: 800
        unit: ppm
      priority: high
```

---

## Integration Test Requirements

### Test: Valid Domain Validation

```rust
#[test]
fn test_valid_domain_config() {
    let config = load_test_domain("indoor-air-quality");
    let validator = DomainValidator::new();

    let result = validator.validate(&config);

    assert!(result.is_ok());
    assert!(result.unwrap().errors.is_empty());
}
```

### Test: Stream Reference Validation

```rust
#[test]
fn test_unknown_stream_reference() {
    let config = json!({
        "domain": {
            "id": "test",
            "streams": [
                { "stream_id": "nonexistent", "role": "primary" }
            ],
            "alignment": {
                "view_name": "test_aligned",
                "granularity": "1 hour"
            }
        }
    });

    let errors = semantic_validate(&config, &available_streams);

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, ErrorCode::UnknownAlignmentStream);
}
```

---

## London TDD Interfaces

### Trait: StreamResolver

```rust
pub trait StreamResolver {
    fn stream_exists(&self, stream_id: &str) -> bool;
    fn list_streams(&self) -> Vec<String>;
    fn get_stream_type(&self, stream_id: &str) -> Option<StreamType>;
}

// Production: EtcdStreamResolver (loads from etcd)
// Test: MockStreamResolver with predefined streams
```

### Trait: DomainSchemaValidator

```rust
pub trait DomainSchemaValidator {
    fn validate_schema(&self, config: &Value) -> Vec<ValidationError>;
    fn validate_semantic(&self, config: &Value, resolver: &dyn StreamResolver) -> Vec<ValidationError>;
}
```

---

## Semantic Validation Rules

### Rule: UNKNOWN_ALIGNMENT_STREAM (402)

```rust
fn validate_stream_references(config: &DomainConfig, resolver: &dyn StreamResolver) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    for (idx, stream) in config.streams.iter().enumerate() {
        if !resolver.stream_exists(&stream.stream_id) {
            let available = resolver.list_streams().join(", ");
            errors.push(ValidationError::semantic_error(
                ErrorCode::UnknownAlignmentStream,
                &format!("$.domain.streams[{}].stream_id", idx),
                format!("Stream '{}' not found", stream.stream_id)
            ).with_suggestion(&format!("Available streams: {}", available)));
        }
    }

    errors
}
```

### Rule: Duplicate Alias Detection

```rust
fn validate_unique_aliases(config: &DomainConfig) -> Vec<ValidationError> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut errors = Vec::new();

    for (idx, stream) in config.streams.iter().enumerate() {
        let alias = stream.alias.clone().unwrap_or(stream.stream_id.clone());
        if !seen.insert(alias.clone()) {
            errors.push(ValidationError::semantic_error(
                ErrorCode::DuplicateName,
                &format!("$.domain.streams[{}].alias", idx),
                format!("Duplicate alias '{}'", alias)
            ));
        }
    }

    errors
}
```

### Rule: Primary Role Required

```rust
fn validate_has_primary(config: &DomainConfig) -> Vec<ValidationError> {
    let has_primary = config.streams.iter().any(|s| s.role == StreamRole::Primary);

    if !has_primary {
        vec![ValidationError::semantic_error(
            ErrorCode::ConstraintViolation,
            "$.domain.streams",
            "Domain requires at least one stream with role: primary"
        )]
    } else {
        vec![]
    }
}
```

---

## References

- [DECISIONS.md](../../architecture/DECISIONS.md) - Decision 6: Domain-Centric Configuration
- [DECISIONS.md](../../architecture/DECISIONS.md) - Decision 10: NULL Handling by Stream Type
- [SCOPE.md](../../SCOPE.md) - Domain config structure
