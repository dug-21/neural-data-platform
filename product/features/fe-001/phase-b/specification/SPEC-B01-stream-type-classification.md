# SPEC-B01: Stream Type Classification

> **Feature ID:** v11-001
> **Priority:** High
> **Status:** Specification
> **Dependencies:** V1.0 stream config, v11-A01 (Gold ETL Schema)
> **Blocks:** v11-002 (Classification Propagation), Phase C (correlation roles)

---

## User Story

**As a** platform architect,
**I want** each stream to declare its classification type (observation, state_event, forecast, dimension),
**So that** the Gold layer can apply type-appropriate aggregation and alignment strategies.

---

## Goal

Add a `stream_type` field to stream configurations that:
1. Classifies streams for correlation analysis (causes vs effects)
2. Determines NULL handling strategy in aligned views
3. Enables type-specific Gold layer features (e.g., transitions for state_event)
4. Is validated by the existing schema validation pipeline

---

## Background: Stream Type Classification

Per SCOPE.md and DECISIONS.md, streams are classified by their data characteristics:

| Type | Description | Correlation Role | NULL Handling | Examples |
|------|-------------|------------------|---------------|----------|
| `observation` | Continuous numeric readings | Effect (target) | Preserve NULL | PM2.5, CO2, temperature |
| `state_event` | Binary/discrete state changes | Cause (source) | Carry Forward (LOCF) | Window open/close, door |
| `forecast` | Future predictions from external source | Context | Preserve NULL | NWS weather forecast |
| `dimension` | Slowly changing reference data | Metadata | Carry Forward | Entity context, locations |

### Air-Quality Classification

The air-quality stream is classified as `observation`:
- Produces continuous numeric sensor readings
- Represents potential effects (what we're trying to optimize)
- Missing readings should be preserved as NULL (not interpolated)
- Aggregates are statistical (mean, std, min, max)

---

## Functional Requirements

### FR-B01-001: Stream Type Field

The stream configuration SHALL include a `stream_type` field:

```yaml
stream_id: "air-quality"
stream_type: "observation"    # NEW: Required classification
description: "AirGradient sensor readings from MQTT"
```

### FR-B01-002: Stream Type Enum

The `stream_type` field SHALL accept only these values:
- `observation` - Continuous numeric readings
- `state_event` - Discrete state changes
- `forecast` - Future predictions
- `dimension` - Slowly changing reference data

### FR-B01-003: Schema Validation

The JSON Schema SHALL validate `stream_type`:

```json
{
  "properties": {
    "stream_type": {
      "type": "string",
      "enum": ["observation", "state_event", "forecast", "dimension"],
      "description": "Classification of stream data for correlation analysis"
    }
  },
  "required": ["stream_id", "stream_type"]
}
```

### FR-B01-004: Default Behavior

If `stream_type` is not specified:
- Validation SHALL fail with error code 409 (MISSING_STREAM_TYPE)
- Error message SHALL list valid stream types

### FR-B01-005: Semantic Validation

The semantic validation layer SHALL enforce:
1. If `gold_etl.transitions.enabled = true`, `stream_type` MUST be `state_event`
2. If `stream_type = forecast`, stream SHOULD have `issued_at` timestamp field
3. Warning if `stream_type = observation` but fields are all discrete/boolean

---

## Non-Functional Requirements

### NFR-B01-001: Backward Compatibility

Existing stream configurations without `stream_type` SHALL:
- Fail validation with a helpful error message
- Not break Silver layer operations (Bronze -> Silver still works)
- Require explicit addition of `stream_type` for Gold layer enablement

### NFR-B01-002: Documentation

Each stream type SHALL be documented with:
- Definition and characteristics
- Appropriate Gold layer features
- NULL handling strategy
- Example streams

---

## Acceptance Criteria

### AC-B01-001: Valid Stream Type Accepted

```gherkin
Scenario: Valid stream_type passes validation
  Given a stream configuration with stream_type = "observation"
  When I run ndp-validate on the configuration
  Then validation SHALL pass with exit code 0
  And no errors SHALL be reported
```

### AC-B01-002: Invalid Stream Type Rejected

```gherkin
Scenario: Invalid stream_type is rejected
  Given a stream configuration with stream_type = "sensor"
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code ENUM_VIOLATION
  And the error message SHALL list valid stream types
```

### AC-B01-003: Missing Stream Type Rejected

```gherkin
Scenario: Missing stream_type is rejected
  Given a stream configuration without stream_type field
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code MISSING_REQUIRED (409)
  And the error path SHALL be "$.stream_type"
```

### AC-B01-004: Transition on Non-State Stream Rejected

```gherkin
Scenario: Transitions enabled on observation stream is rejected
  Given a stream configuration with stream_type = "observation"
  And gold_etl.transitions.enabled = true
  When I run ndp-validate on the configuration
  Then validation SHALL fail with exit code 1
  And the error SHALL include code INVALID_STREAM_TYPE (401)
  And the error message SHALL explain "transitions apply to state_event streams"
```

### AC-B01-005: Air-Quality Classified as Observation

```gherkin
Scenario: Air-quality stream is correctly classified
  Given the air-quality stream configuration
  When I verify the stream_type field
  Then stream_type SHALL equal "observation"
  And validation SHALL pass
```

---

## Air-Quality Config Change

### Current Config (V1.0)

```yaml
stream_id: "air-quality"
description: "AirGradient sensor readings from MQTT"
version: "1.0.0"
enabled: true
```

### Target Config (V1.1 - Phase B)

```yaml
stream_id: "air-quality"
stream_type: "observation"          # NEW: Classification
description: "AirGradient sensor readings from MQTT"
version: "1.1.0"                    # Version bump
enabled: true
```

---

## Schema Extension

### stream-config.v2.schema.json

Add `stream_type` to the stream configuration schema:

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "stream-config.v2.schema.json",
  "type": "object",
  "properties": {
    "stream_id": {
      "type": "string",
      "pattern": "^[a-z][a-z0-9-]*$",
      "description": "Unique identifier for the stream"
    },
    "stream_type": {
      "type": "string",
      "enum": ["observation", "state_event", "forecast", "dimension"],
      "description": "Classification for correlation analysis"
    },
    "description": {
      "type": "string"
    }
  },
  "required": ["stream_id", "stream_type"]
}
```

---

## Error Code Addition

Add error code 409 to ndp-validate:

| Code | Name | Description |
|------|------|-------------|
| 409 | MISSING_STREAM_TYPE | stream_type field is required for V1.1 streams |

---

## Integration Test Requirements

### Test: Schema Validation

```bash
# Test valid classification
echo '{"stream_id": "test", "stream_type": "observation"}' | ndp-validate --schema-only
# Expected: Exit 0

# Test invalid classification
echo '{"stream_id": "test", "stream_type": "sensor"}' | ndp-validate --schema-only
# Expected: Exit 1, ENUM_VIOLATION

# Test missing classification
echo '{"stream_id": "test"}' | ndp-validate --schema-only
# Expected: Exit 1, MISSING_REQUIRED
```

### Test: Semantic Validation

```rust
#[test]
fn test_transitions_require_state_event_stream() {
    let config = StreamConfig {
        stream_id: "test".into(),
        stream_type: StreamType::Observation,
        gold_etl: Some(GoldEtlConfig {
            transitions: Some(TransitionsConfig { enabled: true, ..Default::default() }),
            ..Default::default()
        }),
        ..Default::default()
    };

    let result = validate_semantic(&config);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code, 401); // INVALID_STREAM_TYPE
}
```

### Test: Air-Quality Config

```bash
# Verify air-quality config includes stream_type
grep "stream_type" config/base/streams/air-quality/config.yaml
# Expected: stream_type: "observation"

# Validate complete config
ndp-validate config/base/streams/air-quality/config.yaml
# Expected: Exit 0
```

---

## London TDD Interfaces

### Trait: StreamClassifier

```rust
/// Determines stream classification and validates compatibility
trait StreamClassifier {
    /// Get the correlation role for a stream type
    fn get_correlation_role(&self, stream_type: &StreamType) -> CorrelationRole;

    /// Get the NULL handling strategy for a stream type
    fn get_null_handling(&self, stream_type: &StreamType) -> NullHandling;

    /// Validate that Gold ETL config is compatible with stream type
    fn validate_gold_compatibility(
        &self,
        stream_type: &StreamType,
        gold_etl: &GoldEtlConfig
    ) -> Result<(), ValidationError>;
}
```

### Enum: StreamType

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamType {
    Observation,
    StateEvent,
    Forecast,
    Dimension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrelationRole {
    Effect,   // observation - what we optimize
    Cause,    // state_event - what triggers changes
    Context,  // forecast - environmental context
    Metadata, // dimension - reference data
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullHandling {
    Preserve,     // observation, forecast
    CarryForward, // state_event, dimension (LOCF)
}
```

### Mock: StreamClassifier

```rust
mock! {
    pub StreamClassifier {}

    impl StreamClassifier for StreamClassifier {
        fn get_correlation_role(&self, stream_type: &StreamType) -> CorrelationRole;
        fn get_null_handling(&self, stream_type: &StreamType) -> NullHandling;
        fn validate_gold_compatibility(
            &self,
            stream_type: &StreamType,
            gold_etl: &GoldEtlConfig
        ) -> Result<(), ValidationError>;
    }
}
```

---

## References

- [SCOPE.md](../../SCOPE.md) - Stream Type Classification section
- [DECISIONS.md](../../architecture/DECISIONS.md) - Decision 10: NULL Handling by Stream Type
- [SPEC-A01](../phase-a/specification/SPEC-A01-gold-etl-schema.md) - Gold ETL Schema
- [ndp-validate](../../../../tools/ndp-validate/) - Validation tool

---

*Specification created: 2026-02-04*
