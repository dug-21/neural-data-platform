# ALGO-stream-classification: Stream Type Classification Assignment

> **Algorithm ID:** B01
> **Feature:** v11-001, v11-002 (Stream Type Classification)
> **Phase:** B (First Stream - Reference Implementation)
> **Created:** 2026-02-04

---

## Purpose

Assign and propagate stream type classification from stream configuration to the data dictionary. Stream types (`observation`, `state_event`, `forecast`, `dimension`) determine NULL handling behavior in aligned views and enable the Gold layer to apply appropriate transformation logic.

---

## Algorithm: AssignStreamClassification

```
ALGORITHM: AssignStreamClassification
INPUT:
    stream_id: String
    stream_config: StreamConfig
OUTPUT: Result<ClassificationResult, ClassificationError>
REQUIRES:
    - stream_config has valid stream_type field or defaults to "observation"
    - data_dictionary schema exists

BEGIN
    // 1. Extract or default stream type
    stream_type <- stream_config.stream_type.unwrap_or("observation")

    // 2. Validate stream type against allowed values
    IF stream_type NOT IN ["observation", "state_event", "forecast", "dimension"] THEN
        RETURN Err(ClassificationError::InvalidStreamType {
            stream_id: stream_id,
            provided: stream_type,
            valid: ["observation", "state_event", "forecast", "dimension"]
        })
    END IF

    // 3. Derive correlation role from stream type and domain context
    correlation_role <- DeriveCorrelationRole(stream_type, stream_config.domain_role)

    // 4. Build classification result
    classification <- ClassificationResult {
        stream_id: stream_id,
        stream_type: stream_type,
        correlation_role: correlation_role,
        null_handling: DefaultNullHandling(stream_type)
    }

    RETURN Ok(classification)
END
```

---

## Algorithm: DeriveCorrelationRole

```
ALGORITHM: DeriveCorrelationRole
INPUT:
    stream_type: String
    domain_role: Option<String>    // "primary", "context", "actuator", "constraint"
OUTPUT: String

BEGIN
    // If explicit domain role provided, map to correlation role
    IF domain_role IS Some(role) THEN
        RETURN MATCH role WITH
            | "primary" => "effect"       // Primary stream is what we're optimizing
            | "context" => "context"      // Context provides environmental info
            | "actuator" => "cause"       // Actuator state changes cause effects
            | "constraint" => "metadata"  // Constraints are metadata
            | _ => "context"              // Default fallback
    END IF

    // Default correlation role based on stream type
    RETURN MATCH stream_type WITH
        | "observation" => "effect"       // Observations are typically effects
        | "state_event" => "cause"        // State events are potential causes
        | "forecast" => "context"         // Forecasts provide predictive context
        | "dimension" => "metadata"       // Dimensions are reference data
        | _ => "context"
END
```

---

## Algorithm: DefaultNullHandling

```
ALGORITHM: DefaultNullHandling
INPUT: stream_type: String
OUTPUT: NullHandling

BEGIN
    RETURN MATCH stream_type WITH
        | "observation" => NullHandling::Preserve      // Missing sensor readings stay NULL
        | "state_event" => NullHandling::CarryForward  // State persists until changed (LOCF)
        | "forecast" => NullHandling::Preserve         // Missing forecasts stay NULL
        | "dimension" => NullHandling::CarryForward    // Reference data persists
        | _ => NullHandling::Preserve                  // Default: don't fabricate data
END
```

---

## Algorithm: PropagateClassificationToDataDictionary

```
ALGORITHM: PropagateClassificationToDataDictionary
INPUT:
    classification: ClassificationResult
    db_executor: DatabaseExecutor
OUTPUT: Result<(), PropagationError>
REQUIRES:
    - data_dictionary.stream_classification table exists
    - db_executor has write permissions

BEGIN
    // 1. Generate UPSERT SQL
    sql <- format!(r#"
INSERT INTO data_dictionary.stream_classification (
    stream_id, stream_type, correlation_role, null_handling, updated_at
)
VALUES (
    '{stream_id}', '{stream_type}', '{correlation_role}', '{null_handling}', NOW()
)
ON CONFLICT (stream_id) DO UPDATE SET
    stream_type = EXCLUDED.stream_type,
    correlation_role = EXCLUDED.correlation_role,
    null_handling = EXCLUDED.null_handling,
    updated_at = NOW();
"#,
        stream_id = classification.stream_id,
        stream_type = classification.stream_type,
        correlation_role = classification.correlation_role,
        null_handling = classification.null_handling.to_string()
    )

    // 2. Execute SQL
    result <- db_executor.execute(sql)?

    // 3. Verify propagation
    IF result.rows_affected == 0 THEN
        RETURN Err(PropagationError::NoRowsAffected(classification.stream_id))
    END IF

    RETURN Ok(())
END
```

---

## Algorithm: SyncAllStreamClassifications

```
ALGORITHM: SyncAllStreamClassifications
INPUT:
    config_loader: ConfigLoader
    db_executor: DatabaseExecutor
OUTPUT: Result<SyncReport, SyncError>

BEGIN
    // 1. Load all stream configurations
    stream_configs <- config_loader.load_all_streams()?

    // 2. Process each stream
    successes <- Vec::new()
    failures <- Vec::new()

    FOR EACH (stream_id, config) IN stream_configs DO
        // Assign classification
        classification_result <- AssignStreamClassification(stream_id, config)

        MATCH classification_result WITH
            | Ok(classification) =>
                // Propagate to data dictionary
                propagation_result <- PropagateClassificationToDataDictionary(
                    classification, db_executor
                )
                MATCH propagation_result WITH
                    | Ok(_) => successes.push(stream_id)
                    | Err(e) => failures.push((stream_id, e))

            | Err(e) =>
                failures.push((stream_id, e))
    END FOR

    // 3. Build sync report
    report <- SyncReport {
        total_streams: stream_configs.len(),
        successful: successes.len(),
        failed: failures.len(),
        failures: failures
    }

    RETURN Ok(report)
END
```

---

## SQL Template: Stream Classification Table

```sql
-- DDL for stream_classification table
CREATE TABLE IF NOT EXISTS data_dictionary.stream_classification (
    stream_id           TEXT PRIMARY KEY,
    stream_type         TEXT NOT NULL CHECK (
        stream_type IN ('observation', 'state_event', 'forecast', 'dimension')
    ),
    correlation_role    TEXT CHECK (
        correlation_role IS NULL OR
        correlation_role IN ('cause', 'effect', 'context', 'metadata')
    ),
    null_handling       TEXT NOT NULL DEFAULT 'preserve' CHECK (
        null_handling IN ('preserve', 'carry_forward', 'interpolate')
    ),
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    updated_at          TIMESTAMPTZ DEFAULT NOW()
);

-- Index for lookup by stream type
CREATE INDEX IF NOT EXISTS idx_stream_classification_type
    ON data_dictionary.stream_classification(stream_type);
```

---

## Data Types

```
STRUCT ClassificationResult:
    stream_id: String
    stream_type: String
    correlation_role: String
    null_handling: NullHandling

ENUM NullHandling:
    Preserve        // NULL values pass through unchanged
    CarryForward    // Use LAG() IGNORE NULLS to fill
    Interpolate     // Linear interpolation between values

STRUCT SyncReport:
    total_streams: usize
    successful: usize
    failed: usize
    failures: Vec<(String, Error)>
```

---

## Complexity Analysis

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| AssignStreamClassification | O(1) | O(1) |
| DeriveCorrelationRole | O(1) | O(1) |
| PropagateToDataDictionary | O(1) per stream | O(1) |
| SyncAllClassifications | O(n) | O(n) |

Where n = number of streams

---

## Error Handling

```
ENUM ClassificationError:
    // Invalid stream type value
    InvalidStreamType {
        stream_id: String,
        provided: String,
        valid: Vec<String>
    }

    // Stream config not found
    StreamConfigNotFound {
        stream_id: String
    }

ENUM PropagationError:
    // Database error
    DatabaseError {
        message: String
    }

    // No rows affected (unexpected)
    NoRowsAffected {
        stream_id: String
    }
```

---

## Invariants

1. **Valid Stream Types**: Only `observation`, `state_event`, `forecast`, `dimension` are valid
2. **Default Behavior**: Unspecified stream_type defaults to "observation"
3. **Idempotent Sync**: Running sync multiple times produces same result
4. **NULL Handling Consistency**: Stream type determines default NULL handling
5. **Correlation Role Derivation**: Role derived from stream type when not explicit

---

## Test Cases (London TDD)

```
TRAITS TO MOCK:
    - ConfigLoader: Returns predefined stream configs
    - DatabaseExecutor: Captures SQL statements, returns mock results

TEST: ClassifyObservationStream
    GIVEN stream_config with stream_type = "observation"
    WHEN AssignStreamClassification() is called
    THEN classification.stream_type = "observation"
    AND classification.null_handling = NullHandling::Preserve
    AND classification.correlation_role = "effect"

TEST: ClassifyStateEventStream
    GIVEN stream_config with stream_type = "state_event"
    WHEN AssignStreamClassification() is called
    THEN classification.stream_type = "state_event"
    AND classification.null_handling = NullHandling::CarryForward
    AND classification.correlation_role = "cause"

TEST: DefaultToObservation
    GIVEN stream_config with NO stream_type field
    WHEN AssignStreamClassification() is called
    THEN classification.stream_type = "observation"

TEST: RejectInvalidStreamType
    GIVEN stream_config with stream_type = "invalid_type"
    WHEN AssignStreamClassification() is called
    THEN Err(ClassificationError::InvalidStreamType) is returned

TEST: PropagationGeneratesUpsert
    GIVEN valid classification
    AND mock database executor
    WHEN PropagateClassificationToDataDictionary() is called
    THEN SQL contains "INSERT INTO data_dictionary.stream_classification"
    AND SQL contains "ON CONFLICT (stream_id) DO UPDATE"

TEST: SyncAllStreamsReportsFailures
    GIVEN 3 stream configs, 1 with invalid stream_type
    WHEN SyncAllStreamClassifications() is called
    THEN report.successful = 2
    AND report.failed = 1
    AND report.failures contains the invalid stream

TEST: DomainRoleOverridesDefault
    GIVEN stream_config with stream_type = "observation"
    AND domain_role = "actuator"
    WHEN DeriveCorrelationRole() is called
    THEN correlation_role = "cause" (not "effect")
```

---

## Integration with Phase B

This algorithm is called during:
1. **Stream config deployment**: When `deploy.sh` applies a stream config
2. **Domain config deployment**: When domain references streams with roles
3. **Validation**: When `ndp-gold-ddl validate` checks stream configs

The classification stored in `data_dictionary.stream_classification` is used by:
- Phase C aligned view generation (NULL handling strategy)
- Phase E unified events (stream context for events)
- V1.2 pattern detection (correlation role for causality)

---

## References

- [SPEC-B01-stream-type-classification.md](../specification/SPEC-B01-stream-type-classification.md)
- [SPEC-B02-classification-propagation.md](../specification/SPEC-B02-classification-propagation.md)
- [ADR-FE001-004](../../architecture/DECISIONS.md) - NULL handling by stream type
