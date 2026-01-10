# ADR-006-004: DQ Rule Actions

**Feature**: dp-006 (Silver Layer Implementation)
**Status**: Accepted
**Date**: 2026-01-10
**Author**: NDP Architect
**Supersedes**: None

---

## Context

Data Quality (DQ) rules validate incoming data during Bronze-to-Silver ETL. When a rule triggers, the system must decide how to handle the violation. This decision affects:

1. **Data completeness** - How much data reaches Silver layer
2. **Data quality** - How clean the Silver data is
3. **Transparency** - How violations are tracked and auditable
4. **Downstream impact** - How dashboards and analytics handle bad data

### Guiding Principle

From SCOPE.md:
> "DQ Transparency Over Rejection: Prefer flagging bad data over rejecting it. Transparency enables investigation."

This principle favors keeping data visible while marking quality issues, rather than silently dropping records.

### DQ Rule Types (Implemented)

| Rule | Description | Example |
|------|-------------|---------|
| `range_check` | Value within min/max bounds | Temperature -50 to 60 |
| `not_null` | Value must be present | Primary keys |
| `pattern` | Regex match | Serial number format |
| `one_of` | Value in allowed set | Status codes |

---

## Decision

**Support four DQ actions**, with `flag` as the default:

| Action | Behavior | Value Kept | Row Kept | dq_flags Entry |
|--------|----------|------------|----------|----------------|
| `flag` | Keep value, add flag | Yes | Yes | Yes |
| `reject` | Set to NULL, add flag | No (NULL) | Yes | Yes |
| `clamp` | Clamp to bounds, add flag | Yes (clamped) | Yes | Yes |
| `drop` | Drop entire row | - | No | - |

### YAML Configuration

```yaml
dq_rules:
  # Flag: Keep value, record violation
  - rule: range_check
    min: -50.0
    max: 60.0
    action: flag  # DEFAULT

  # Reject: Set to NULL, record violation
  - rule: not_null
    action: reject

  # Clamp: Adjust value to bounds, record adjustment
  - rule: range_check
    min: 0.0
    max: 100.0
    action: clamp

  # Drop: Remove entire row (use sparingly)
  - rule: range_check
    min: 0.0
    max: 1000000.0
    action: drop
```

---

## Consequences

### Positive

1. **Transparency by default** - `flag` keeps data visible for investigation
2. **Flexible per-field** - Different actions for different data criticality
3. **Bounded values** - `clamp` useful for percentages, indices
4. **Safety valve** - `drop` prevents catastrophically invalid data
5. **Audit trail** - All actions (except drop) recorded in `dq_flags`

### Negative

1. **Complexity** - Four actions vs simple pass/fail
2. **Action choice** - Developers must decide appropriate action per rule
3. **Clamp precision** - Clamped values may mask sensor issues

### Neutral

1. **No partial rejection** - Cannot reject some fields while keeping others in same row
2. **No conditional actions** - Action is static per rule, not value-dependent

---

## Action Specifications

### Action: `flag` (Default)

**Behavior**: Keep original value, append flag to `dq_flags` array.

**Use when**:
- Value is suspicious but potentially valid
- Want to investigate anomalies later
- Downstream can handle outliers

**SQL Pattern**:
```sql
-- Value passes through unchanged
json_extract(raw_payload, '$.temperature')::FLOAT AS temperature_c,

-- Flag collected separately
CASE
    WHEN json_extract(raw_payload, '$.temperature')::FLOAT NOT BETWEEN -50 AND 60
    THEN 'range_check:temperature_c:out_of_range'
    ELSE NULL
END AS _flag_temperature
```

**dq_flags entry**: `range_check:temperature_c:out_of_range`

---

### Action: `reject`

**Behavior**: Set value to NULL, append flag to `dq_flags` array.

**Use when**:
- Value would break calculations (division by zero, invalid enum)
- Value is clearly invalid (negative count, future timestamp)
- Downstream cannot handle outliers

**SQL Pattern**:
```sql
-- Value nullified if violation
CASE
    WHEN json_extract(raw_payload, '$.co2')::INT BETWEEN 380 AND 10000
    THEN json_extract(raw_payload, '$.co2')::INT
    ELSE NULL
END AS co2,

-- Flag always recorded
CASE
    WHEN json_extract(raw_payload, '$.co2')::INT NOT BETWEEN 380 AND 10000
    THEN 'range_check:co2:rejected'
    ELSE NULL
END AS _flag_co2
```

**dq_flags entry**: `range_check:co2:rejected`

---

### Action: `clamp`

**Behavior**: Adjust value to nearest bound, append flag with original and clamped values.

**Use when**:
- Value has hard physical limits (humidity 0-100%)
- Bounded indices (AQI 0-500)
- Sensor occasionally reports impossible values

**SQL Pattern**:
```sql
-- Value clamped to bounds
LEAST(GREATEST(
    json_extract(raw_payload, '$.humidity')::FLOAT,
    0.0
), 100.0) AS humidity_pct,

-- Flag with original value
CASE
    WHEN json_extract(raw_payload, '$.humidity')::FLOAT < 0.0
    THEN 'range_check:humidity_pct:clamped:' ||
         json_extract(raw_payload, '$.humidity')::TEXT || '->0.0'
    WHEN json_extract(raw_payload, '$.humidity')::FLOAT > 100.0
    THEN 'range_check:humidity_pct:clamped:' ||
         json_extract(raw_payload, '$.humidity')::TEXT || '->100.0'
    ELSE NULL
END AS _flag_humidity
```

**dq_flags entry**: `range_check:humidity_pct:clamped:105.3->100.0`

---

### Action: `drop`

**Behavior**: Exclude entire row from Silver table. No `dq_flags` entry (row doesn't exist).

**Use when**:
- Data is catastrophically invalid (timestamp from 1970)
- Row would corrupt downstream aggregates
- Privacy/compliance requires removal

**SQL Pattern**:
```sql
-- Filter in WHERE clause
SELECT ...
FROM bronze_data
WHERE
    -- Drop rows with invalid timestamps
    to_timestamp(timestamp / 1000000) > '2020-01-01'::TIMESTAMP
    -- Drop rows with impossible values
    AND json_extract(raw_payload, '$.pm25')::FLOAT >= 0
```

**Note**: Dropped rows are still in Bronze for audit. Consider logging drop counts.

---

## dq_flags Column Design

### Column Definition

```sql
dq_flags TEXT[] DEFAULT NULL
```

### Flag Format

```
{rule_name}:{column_name}:{violation_type}[:{details}]
```

Examples:
- `range_check:temperature_c:exceeded_max`
- `range_check:humidity_pct:clamped:105.3->100.0`
- `not_null:ndp_id:null_value`
- `pattern:serial:invalid_format`

### Aggregation in SQL

```sql
-- Collect all flags for a row
ARRAY_REMOVE(ARRAY[
    _flag_temperature,
    _flag_humidity,
    _flag_co2,
    _flag_pm25
], NULL) AS dq_flags
```

### Querying Flags

```sql
-- Find all rows with any DQ violations
SELECT * FROM silver.air_quality_observations
WHERE dq_flags IS NOT NULL AND array_length(dq_flags, 1) > 0;

-- Find rows with specific violation type
SELECT * FROM silver.air_quality_observations
WHERE 'range_check:temperature_c:exceeded_max' = ANY(dq_flags);

-- Count violations by type
SELECT
    unnest(dq_flags) AS flag,
    COUNT(*) AS occurrences
FROM silver.air_quality_observations
WHERE dq_flags IS NOT NULL
GROUP BY 1
ORDER BY 2 DESC;
```

---

## Alternatives Considered

### Alternative 1: Binary Pass/Fail

**Description**: Simple accept or reject per row.

**Rejected because**: Loses nuance. Some bad values don't invalidate entire row. No visibility into flagged-but-kept data.

### Alternative 2: Separate DQ Table

**Description**: Store all violations in separate table, keep Silver clean.

```sql
CREATE TABLE silver.dq_violations (
    observation_time TIMESTAMPTZ,
    ndp_id TEXT,
    column_name TEXT,
    rule_name TEXT,
    original_value TEXT,
    action_taken TEXT
);
```

**Rejected because**: Requires join to understand data quality. Complicates queries. Adds ETL complexity.

**Note**: May add this as optional enhancement later for detailed audit trails.

### Alternative 3: Configurable Flag Format

**Description**: Allow custom flag format in config.

```yaml
dq_output:
  format: "{rule}:{column}:{status}"  # Configurable template
```

**Rejected because**: Over-engineering. Standard format is sufficient. Consistency aids tooling.

---

## Default Action Recommendations

| Rule Type | Recommended Default | Rationale |
|-----------|---------------------|-----------|
| `range_check` | `flag` | Outliers often valid |
| `not_null` | `reject` | Missing data breaks joins |
| `pattern` | `flag` | Format variations common |
| `one_of` | `reject` | Invalid enum breaks logic |

### Per-Domain Guidance

| Domain | Stricter | More Lenient |
|--------|----------|--------------|
| **Air Quality** | `not_null` on PM2.5 | `clamp` on humidity |
| **Weather** | `reject` on temperature > 100C | `flag` on wind direction |
| **Financial** | `drop` on negative amounts | - |
| **Events** | `drop` on invalid timestamp | `flag` on missing metadata |

---

## References

1. SCOPE.md: "DQ Transparency Over Rejection" principle
2. Pattern: `arch-config-driven-silver-etl` - DQ config schema
3. Research: `research/agenticdataplatform/silver/09-etl-genericity-assessment.md`
4. Research: `docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md` Section 4

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP Architect | Initial decision |
