# ALGO-objectives-sync: Domain Objectives Synchronization

> **Algorithm ID:** C03
> **Feature:** v11-007 (Objectives Storage)
> **Phase:** C (Cross-Stream + Alignment)
> **Created:** 2026-02-04

---

## Purpose

Synchronize domain objectives from `domain.yaml` configuration to the `data_dictionary.objectives` table. This algorithm enables V1.2 pattern detection to query objectives programmatically and Phase E threshold crossing detection to generate events when metrics cross objective thresholds.

---

## Algorithm: SyncDomainObjectives

```
ALGORITHM: SyncDomainObjectives
INPUT:
    domain_config: DomainConfig
    db_executor: DatabaseExecutor
OUTPUT: Result<SyncResult, SyncError>
REQUIRES:
    - data_dictionary.domains table exists
    - data_dictionary.objectives table exists
    - data_dictionary.constraints table exists
    - data_dictionary.domain_streams table exists

BEGIN
    domain_id <- domain_config.id

    // 1. Begin transaction for atomic sync
    tx <- db_executor.begin_transaction()?

    // 2. Upsert domain metadata
    UpsertDomain(domain_config, tx)?

    // 3. Sync domain-stream mappings
    SyncDomainStreams(domain_config, tx)?

    // 4. Sync objectives (delete-insert pattern for full sync)
    SyncObjectives(domain_id, domain_config.objectives, tx)?

    // 5. Sync constraints
    SyncConstraints(domain_id, domain_config.constraints, tx)?

    // 6. Commit transaction
    tx.commit()?

    // 7. Build sync result
    result <- SyncResult {
        domain_id: domain_id,
        objectives_synced: domain_config.objectives.len(),
        constraints_synced: domain_config.constraints.unwrap_or_default().len(),
        streams_synced: domain_config.streams.len()
    }

    RETURN Ok(result)
END
```

---

## Algorithm: UpsertDomain

```
ALGORITHM: UpsertDomain
INPUT:
    domain_config: DomainConfig
    tx: Transaction
OUTPUT: Result<(), SyncError>

BEGIN
    sql <- format!(r#"
INSERT INTO data_dictionary.domains (
    domain_id,
    description,
    stream_count,
    config_path,
    updated_at
)
VALUES (
    '{domain_id}',
    '{description}',
    {stream_count},
    '{config_path}',
    NOW()
)
ON CONFLICT (domain_id) DO UPDATE SET
    description = EXCLUDED.description,
    stream_count = EXCLUDED.stream_count,
    config_path = EXCLUDED.config_path,
    updated_at = NOW();
"#,
        domain_id = domain_config.id,
        description = EscapeSqlString(domain_config.description.unwrap_or("")),
        stream_count = domain_config.streams.len(),
        config_path = format!("config/domains/{}/domain.yaml", domain_config.id)
    )

    tx.execute(sql)?

    RETURN Ok(())
END
```

---

## Algorithm: SyncDomainStreams

```
ALGORITHM: SyncDomainStreams
INPUT:
    domain_config: DomainConfig
    tx: Transaction
OUTPUT: Result<(), SyncError>

BEGIN
    domain_id <- domain_config.id

    // 1. Delete existing mappings for this domain
    delete_sql <- format!(
        "DELETE FROM data_dictionary.domain_streams WHERE domain_id = '{}'",
        domain_id
    )
    tx.execute(delete_sql)?

    // 2. Insert new mappings
    FOR EACH stream_ref IN domain_config.streams DO
        insert_sql <- format!(r#"
INSERT INTO data_dictionary.domain_streams (
    domain_id,
    stream_id,
    alias,
    role
)
VALUES (
    '{domain_id}',
    '{stream_id}',
    '{alias}',
    '{role}'
);
"#,
            domain_id = domain_id,
            stream_id = stream_ref.stream_id,
            alias = stream_ref.alias,
            role = stream_ref.role
        )
        tx.execute(insert_sql)?
    END FOR

    RETURN Ok(())
END
```

---

## Algorithm: SyncObjectives

```
ALGORITHM: SyncObjectives
INPUT:
    domain_id: String
    objectives: Vec<Objective>
    tx: Transaction
OUTPUT: Result<usize, SyncError>

BEGIN
    // 1. Delete existing objectives for this domain (full sync)
    delete_sql <- format!(
        "DELETE FROM data_dictionary.objectives WHERE domain_id = '{}'",
        domain_id
    )
    tx.execute(delete_sql)?

    // 2. Insert objectives from config
    synced_count <- 0

    FOR EACH objective IN objectives DO
        // Validate objective
        validation_result <- ValidateObjective(objective)
        IF validation_result.has_errors() THEN
            RETURN Err(SyncError::InvalidObjective {
                objective_id: objective.id,
                errors: validation_result.errors
            })
        END IF

        // Generate insert SQL
        insert_sql <- GenerateObjectiveInsert(domain_id, objective)
        tx.execute(insert_sql)?

        synced_count <- synced_count + 1
    END FOR

    RETURN Ok(synced_count)
END
```

---

## Algorithm: GenerateObjectiveInsert

```
ALGORITHM: GenerateObjectiveInsert
INPUT:
    domain_id: String
    objective: Objective
OUTPUT: String

BEGIN
    target <- objective.target

    // Handle threshold (could be single value or array for between)
    threshold_value <- MATCH target.threshold WITH
        | Single(value) => value.to_string()
        | Range(min, max) => format!("ARRAY[{}, {}]::numeric[]", min, max)
    END

    sql <- format!(r#"
INSERT INTO data_dictionary.objectives (
    objective_id,
    domain_id,
    description,
    target_stream,
    target_metric,
    condition,
    threshold,
    unit,
    priority,
    created_at,
    updated_at
)
VALUES (
    '{objective_id}',
    '{domain_id}',
    '{description}',
    '{target_stream}',
    '{target_metric}',
    '{condition}',
    {threshold},
    '{unit}',
    '{priority}',
    NOW(),
    NOW()
);
"#,
        objective_id = objective.id,
        domain_id = domain_id,
        description = EscapeSqlString(objective.description.unwrap_or("")),
        target_stream = target.stream,
        target_metric = target.metric,
        condition = target.condition.to_sql_string(),
        threshold = threshold_value,
        unit = target.unit.unwrap_or(""),
        priority = objective.priority.to_string()
    )

    RETURN sql
END
```

---

## Algorithm: SyncConstraints

```
ALGORITHM: SyncConstraints
INPUT:
    domain_id: String
    constraints: Option<Vec<Constraint>>
    tx: Transaction
OUTPUT: Result<usize, SyncError>

BEGIN
    // 1. Delete existing constraints
    delete_sql <- format!(
        "DELETE FROM data_dictionary.constraints WHERE domain_id = '{}'",
        domain_id
    )
    tx.execute(delete_sql)?

    // 2. If no constraints, return early
    IF constraints IS None THEN
        RETURN Ok(0)
    END IF

    // 3. Insert constraints
    synced_count <- 0

    FOR EACH constraint IN constraints.unwrap() DO
        insert_sql <- format!(r#"
INSERT INTO data_dictionary.constraints (
    constraint_id,
    domain_id,
    description,
    constraint_stream,
    constraint_metric,
    condition,
    threshold,
    unit,
    created_at
)
VALUES (
    '{constraint_id}',
    '{domain_id}',
    '{description}',
    '{stream}',
    '{metric}',
    '{condition}',
    {threshold},
    '{unit}',
    NOW()
);
"#,
            constraint_id = constraint.id,
            domain_id = domain_id,
            description = EscapeSqlString(constraint.description.unwrap_or("")),
            stream = constraint.stream,
            metric = constraint.metric,
            condition = constraint.condition.to_sql_string(),
            threshold = constraint.threshold,
            unit = constraint.unit.unwrap_or("")
        )
        tx.execute(insert_sql)?

        synced_count <- synced_count + 1
    END FOR

    RETURN Ok(synced_count)
END
```

---

## Algorithm: ValidateObjective

```
ALGORITHM: ValidateObjective
INPUT: objective: Objective
OUTPUT: ValidationResult

BEGIN
    errors <- Vec::new()

    // 1. Validate objective_id format
    IF objective.id.is_empty() THEN
        errors.push(ValidationError::EmptyObjectiveId)
    END IF

    // 2. Validate target stream
    IF objective.target.stream.is_empty() THEN
        errors.push(ValidationError::EmptyTargetStream {
            objective_id: objective.id
        })
    END IF

    // 3. Validate target metric
    IF objective.target.metric.is_empty() THEN
        errors.push(ValidationError::EmptyTargetMetric {
            objective_id: objective.id
        })
    END IF

    // 4. Validate condition
    valid_conditions <- ["<", ">", "<=", ">=", "==", "!=", "between"]
    IF objective.target.condition.to_sql_string() NOT IN valid_conditions THEN
        errors.push(ValidationError::Code408_InvalidObjectiveCondition {
            objective_id: objective.id,
            condition: objective.target.condition.to_string(),
            valid: valid_conditions
        })
    END IF

    // 5. Validate threshold for between condition
    IF objective.target.condition == Condition::Between THEN
        IF NOT objective.target.threshold.is_range() THEN
            errors.push(ValidationError::BetweenRequiresRange {
                objective_id: objective.id
            })
        END IF
    END IF

    // 6. Validate priority
    valid_priorities <- ["low", "medium", "high", "critical"]
    IF objective.priority.to_string() NOT IN valid_priorities THEN
        errors.push(ValidationError::InvalidPriority {
            objective_id: objective.id,
            priority: objective.priority.to_string()
        })
    END IF

    RETURN ValidationResult::new(errors)
END
```

---

## Algorithm: Condition to SQL String

```
ALGORITHM: ConditionToSqlString
INPUT: condition: Condition
OUTPUT: String

BEGIN
    RETURN MATCH condition WITH
        | Condition::LessThan => "<"
        | Condition::GreaterThan => ">"
        | Condition::LessOrEqual => "<="
        | Condition::GreaterOrEqual => ">="
        | Condition::Equal => "=="
        | Condition::NotEqual => "!="
        | Condition::Between => "between"
END
```

---

## Algorithm: Query Objectives for Threshold Crossing

```
ALGORITHM: GetObjectivesForStream
INPUT:
    domain_id: String
    stream_id: String
    db_executor: DatabaseExecutor
OUTPUT: Result<Vec<Objective>, QueryError>
DESCRIPTION: Used by Phase E threshold crossing generator

BEGIN
    sql <- format!(r#"
SELECT
    objective_id,
    domain_id,
    description,
    target_stream,
    target_metric,
    condition,
    threshold,
    unit,
    priority
FROM data_dictionary.objectives
WHERE domain_id = '{domain_id}'
  AND target_stream = '{stream_id}'
ORDER BY priority DESC, objective_id;
"#,
        domain_id = domain_id,
        stream_id = stream_id
    )

    rows <- db_executor.query(sql)?

    objectives <- Vec::new()
    FOR EACH row IN rows DO
        objective <- ParseObjectiveFromRow(row)?
        objectives.push(objective)
    END FOR

    RETURN Ok(objectives)
END
```

---

## Data Types

```
STRUCT Objective:
    id: String
    description: Option<String>
    target: ObjectiveTarget
    priority: Priority

STRUCT ObjectiveTarget:
    stream: String
    metric: String
    condition: Condition
    threshold: Threshold
    unit: Option<String>

ENUM Condition:
    LessThan        // <
    GreaterThan     // >
    LessOrEqual     // <=
    GreaterOrEqual  // >=
    Equal           // ==
    NotEqual        // !=
    Between         // between [min, max]

ENUM Threshold:
    Single(f64)
    Range(f64, f64)

ENUM Priority:
    Low
    Medium
    High
    Critical

STRUCT Constraint:
    id: String
    description: Option<String>
    stream: String
    metric: String
    condition: Condition
    threshold: f64
    unit: Option<String>

STRUCT SyncResult:
    domain_id: String
    objectives_synced: usize
    constraints_synced: usize
    streams_synced: usize
```

---

## SQL Templates

### Data Dictionary Tables

```sql
-- Domains table
CREATE TABLE IF NOT EXISTS data_dictionary.domains (
    domain_id TEXT PRIMARY KEY,
    description TEXT,
    stream_count INTEGER,
    config_path TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Domain-stream mapping
CREATE TABLE IF NOT EXISTS data_dictionary.domain_streams (
    domain_id TEXT NOT NULL REFERENCES data_dictionary.domains(domain_id) ON DELETE CASCADE,
    stream_id TEXT NOT NULL,
    alias TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('primary', 'context', 'actuator', 'constraint')),
    PRIMARY KEY (domain_id, stream_id)
);

-- Objectives table
CREATE TABLE IF NOT EXISTS data_dictionary.objectives (
    objective_id TEXT NOT NULL,
    domain_id TEXT NOT NULL REFERENCES data_dictionary.domains(domain_id) ON DELETE CASCADE,
    description TEXT,
    target_stream TEXT NOT NULL,
    target_metric TEXT NOT NULL,
    condition TEXT NOT NULL CHECK (condition IN ('<', '>', '<=', '>=', '==', '!=', 'between')),
    threshold NUMERIC NOT NULL,
    unit TEXT,
    priority TEXT DEFAULT 'medium' CHECK (priority IN ('low', 'medium', 'high', 'critical')),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (domain_id, objective_id)
);

-- Constraints table
CREATE TABLE IF NOT EXISTS data_dictionary.constraints (
    constraint_id TEXT NOT NULL,
    domain_id TEXT NOT NULL REFERENCES data_dictionary.domains(domain_id) ON DELETE CASCADE,
    description TEXT,
    constraint_stream TEXT NOT NULL,
    constraint_metric TEXT NOT NULL,
    condition TEXT NOT NULL CHECK (condition IN ('<', '>', '<=', '>=', '==', '!=')),
    threshold NUMERIC NOT NULL,
    unit TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (domain_id, constraint_id)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_objectives_domain ON data_dictionary.objectives(domain_id);
CREATE INDEX IF NOT EXISTS idx_objectives_stream ON data_dictionary.objectives(target_stream);
CREATE INDEX IF NOT EXISTS idx_constraints_domain ON data_dictionary.constraints(domain_id);
CREATE INDEX IF NOT EXISTS idx_domain_streams_domain ON data_dictionary.domain_streams(domain_id);
```

---

## Query Examples

### Get All Objectives for Domain

```sql
SELECT
    objective_id,
    target_stream,
    target_metric,
    condition,
    threshold,
    unit,
    priority
FROM data_dictionary.objectives
WHERE domain_id = 'indoor-air-quality'
ORDER BY priority DESC, objective_id;
```

### Get High Priority Objectives

```sql
SELECT
    o.objective_id,
    o.target_stream,
    o.target_metric,
    o.condition || ' ' || o.threshold::TEXT || ' ' || COALESCE(o.unit, '') AS condition_display
FROM data_dictionary.objectives o
WHERE o.domain_id = 'indoor-air-quality'
  AND o.priority IN ('high', 'critical')
ORDER BY o.priority DESC;
```

### Get Domain with Stream Mappings

```sql
SELECT
    d.domain_id,
    d.description,
    ds.stream_id,
    ds.alias,
    ds.role
FROM data_dictionary.domains d
JOIN data_dictionary.domain_streams ds ON d.domain_id = ds.domain_id
WHERE d.domain_id = 'indoor-air-quality'
ORDER BY ds.role;
```

---

## Complexity Analysis

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| Upsert domain | O(1) | O(1) |
| Sync streams | O(s) | O(s) |
| Sync objectives | O(o) | O(o) |
| Sync constraints | O(c) | O(c) |
| Total | O(s + o + c) | O(s + o + c) |

Where: s = streams, o = objectives, c = constraints

---

## Error Handling

```
ENUM SyncError:
    InvalidObjective { objective_id, errors }
    InvalidConstraint { constraint_id, errors }
    DatabaseError { message }
    TransactionFailed { stage, message }

ENUM ValidationError:
    EmptyObjectiveId
    EmptyTargetStream { objective_id }
    EmptyTargetMetric { objective_id }
    Code408_InvalidObjectiveCondition { objective_id, condition, valid }
    BetweenRequiresRange { objective_id }
    InvalidPriority { objective_id, priority }
```

---

## Invariants

1. **Atomic Sync**: All changes in single transaction
2. **Full Sync**: Delete-then-insert ensures config is source of truth
3. **Cascade Delete**: Removing domain removes all objectives/constraints
4. **Valid Conditions**: Only allowed condition operators accepted
5. **Priority Required**: Defaults to "medium" if not specified

---

## Test Cases (London TDD)

```
TRAITS TO MOCK:
    - DatabaseExecutor: Capture SQL, return mock results
    - ConfigLoader: Return test domain configs

TEST: SyncObjectivesDeletesExisting
    GIVEN existing objectives for domain "test-domain"
    WHEN SyncDomainObjectives() is called with new objectives
    THEN DELETE FROM data_dictionary.objectives WHERE domain_id = 'test-domain'
    AND new objectives are inserted

TEST: UpsertDomainMetadata
    GIVEN domain config with description "Test Domain"
    WHEN UpsertDomain() is called
    THEN SQL contains "ON CONFLICT (domain_id) DO UPDATE"
    AND description is updated

TEST: ValidateInvalidCondition
    GIVEN objective with condition = "invalid"
    WHEN ValidateObjective() is called
    THEN ValidationError::Code408_InvalidObjectiveCondition is returned

TEST: ValidateBetweenRequiresRange
    GIVEN objective with condition = "between"
    AND threshold is single value
    WHEN ValidateObjective() is called
    THEN ValidationError::BetweenRequiresRange is returned

TEST: SyncStreamsWithRoles
    GIVEN domain with streams having roles [primary, context, actuator]
    WHEN SyncDomainStreams() is called
    THEN all three stream mappings are inserted
    AND roles are correct

TEST: TransactionRollbackOnError
    GIVEN valid domain config
    AND database error on objective insert
    WHEN SyncDomainObjectives() is called
    THEN transaction is rolled back
    AND no partial data remains

TEST: ConstraintsOptional
    GIVEN domain config with no constraints
    WHEN SyncDomainObjectives() is called
    THEN constraints_synced = 0
    AND no error is returned

TEST: GetObjectivesForStream
    GIVEN objectives for stream "air-quality"
    WHEN GetObjectivesForStream() is called
    THEN only air-quality objectives returned
    AND sorted by priority DESC
```

---

## Integration with Phase E

Objectives stored via this algorithm are used by:
1. **Threshold Crossing Generator (v11-012)**: Queries objectives to generate crossing detection SQL
2. **Unified Events View (v11-013)**: References objectives in event details
3. **Dashboard Annotations**: Objectives provide threshold lines for charts

---

## References

- [SPEC-C03-objectives-storage.md](../specification/SPEC-C03-objectives-storage.md)
- [ADR-FE001-002](../../architecture/DECISIONS.md) - Domain-centric configuration
- [SPEC-A05](../../phase-a/specification/SPEC-A05-objectives-schema.md) - Objectives JSON schema
