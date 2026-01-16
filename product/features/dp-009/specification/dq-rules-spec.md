# DP-009: DQ Rules Specification for Data Dictionary

**Document Type**: Specification
**Feature**: dp-009 (Config-Driven Silver Layer Data Dictionary)
**Created**: 2026-01-16
**Author**: NDP Data Quality Engineer

---

## 1. Overview

This specification defines how Data Quality (DQ) rules are exposed in the Silver layer data dictionary. DQ rules are applied during Bronze-to-Silver ETL transformations and must be queryable to enable:

- **Transparency**: Users can discover what validation is applied to their data
- **Debugging**: When data is flagged or rejected, users can understand why
- **Documentation**: Self-documenting data quality contracts per column/table
- **Governance**: Auditable record of data quality rules across all Silver tables

---

## 2. DQ Rule Types Inventory

The following DQ rule types are supported during Silver ETL transformation:

### 2.1 Column-Level Rules

These rules validate individual column values:

| Rule Type | Description | Config Location |
|-----------|-------------|-----------------|
| `range_check` | Validates numeric values within min/max bounds | `field_mappings[].dq_rules[]` |
| `not_null` | Ensures column is non-null (derived from `nullable: false`) | `field_mappings[].nullable` |
| `pattern` | Validates string values against regex pattern | `field_mappings[].dq_rules[]` |
| `enum_check` | Validates value against allowed set | `field_mappings[].dq_rules[]` |

### 2.2 Table-Level Rules

These rules validate relationships across columns or batch-level properties:

| Rule Type | Description | Config Location |
|-----------|-------------|-----------------|
| `cross_field_check` | SQL expression validating column relationships | `silver_etl.dq_rules[]` |
| `freshness_check` | Validates timestamp recency | `silver_etl.dq_rules[]` |
| `rate_of_change` | Detects spikes exceeding threshold | `silver_etl.dq_rules[]` |
| `completeness_check` | Validates batch-level non-null percentage | `silver_etl.dq_rules[]` |

---

## 3. Rule Parameters Schema (JSONB)

Each rule type has a specific parameter schema stored in the `rule_params` JSONB column:

### 3.1 `range_check`

Validates that a numeric value falls within specified bounds.

```json
{
  "min": 0.0,
  "max": 1000.0,
  "clamp_to_bounds": false
}
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `min` | number | Yes | Minimum allowed value (inclusive) |
| `max` | number | Yes | Maximum allowed value (inclusive) |
| `clamp_to_bounds` | boolean | No | If true with `action: clamp`, values are forced to bounds |

**Example from config**:
```yaml
dq_rules:
  - rule: range_check
    min: 0.0
    max: 1000.0
    action: flag
```

**Resulting rule_params**:
```json
{"min": 0.0, "max": 1000.0, "clamp_to_bounds": false}
```

---

### 3.2 `not_null`

Validates that a column value is non-null. This rule is implicitly created when `nullable: false` is set on a field mapping.

```json
{}
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| _(none)_ | - | - | No parameters needed |

**Derived from config**:
```yaml
field_mappings:
  - source_path: raw_payload.pm02Compensated
    target_column: pm25
    nullable: false  # Creates implicit not_null rule
```

**Resulting rule_params**:
```json
{}
```

---

### 3.3 `pattern`

Validates that a string value matches a regex pattern.

```json
{
  "regex": "^[A-Z]{3}$",
  "description": "Three-letter airport code"
}
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `regex` | string | Yes | Regular expression pattern (POSIX or PCRE) |
| `description` | string | No | Human-readable description of expected format |

**Example from config**:
```yaml
dq_rules:
  - rule: pattern
    regex: "^[A-Z]{3}$"
    description: "Three-letter airport code"
    action: flag
```

---

### 3.4 `enum_check`

Validates that a value is one of an allowed set.

```json
{
  "allowed_values": ["N", "S", "E", "W", "NE", "NW", "SE", "SW"],
  "case_sensitive": true
}
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `allowed_values` | array | Yes | List of valid values |
| `case_sensitive` | boolean | No | Whether comparison is case-sensitive (default: true) |

---

### 3.5 `cross_field_check`

Validates a SQL expression across multiple columns. Applies to the entire row.

```json
{
  "expression": "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25",
  "message": "pm10_less_than_pm25",
  "description": "PM10 (larger particles) should always be >= PM2.5"
}
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `expression` | string | Yes | SQL boolean expression (must evaluate to TRUE for valid rows) |
| `message` | string | Yes | Short identifier added to dq_flags[] on violation |
| `description` | string | No | Human-readable explanation of the constraint |

**Example from config**:
```yaml
dq_rules:
  - rule: cross_field_check
    name: pm10_gte_pm25
    expression: "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25"
    message: "pm10_less_than_pm25"
    action: flag
```

**Resulting rule_params**:
```json
{
  "expression": "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25",
  "message": "pm10_less_than_pm25"
}
```

**Common Cross-Field Patterns**:

| Pattern | Expression | Use Case |
|---------|------------|----------|
| Column comparison | `col_a >= col_b` | PM10 >= PM2.5, gust >= sustained |
| Temporal ordering | `valid_time >= issue_time` | Forecast valid after issue |
| Derived bounds | `ABS(feels_like_c - temperature_c) <= 20` | Reasonable feels-like delta |
| Horizon limits | `EXTRACT(EPOCH FROM (valid_time - issue_time)) <= 604800` | 7-day forecast limit |
| Physical constraint | `dewpoint_c <= temperature_c` | Dewpoint cannot exceed temp |

---

### 3.6 `freshness_check`

Validates that a timestamp field is within acceptable age bounds.

```json
{
  "field": "observation_time",
  "max_age": "2 hours",
  "max_future": "5 minutes",
  "reference": "ingestion_time"
}
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `field` | string | Yes | Column name to validate |
| `max_age` | string | Yes | Maximum allowed age as PostgreSQL interval (e.g., "2 hours", "30 minutes") |
| `max_future` | string | No | Maximum allowed future offset (default: "0") |
| `reference` | string | No | Reference timestamp column (default: "ingestion_time") |

**Example from config**:
```yaml
dq_rules:
  - rule: freshness_check
    field: observation_time
    max_age: "2 hours"
    max_future: "5 minutes"
    reference: ingestion_time
    action: flag
```

**Resulting rule_params**:
```json
{
  "field": "observation_time",
  "max_age": "2 hours",
  "max_future": "5 minutes",
  "reference": "ingestion_time"
}
```

---

### 3.7 `rate_of_change`

Detects sudden spikes or changes that may indicate sensor malfunction or data errors.

```json
{
  "field": "pm25",
  "max_change_per_minute": 100.0,
  "partition_by": ["ndp_id"]
}
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `field` | string | Yes | Column name to monitor |
| `max_change_per_minute` | number | Yes | Maximum absolute change per minute |
| `partition_by` | array | No | Columns to partition by (rate calculated within each partition) |
| `min_interval_seconds` | number | No | Minimum time between readings to apply check |

**Example from config**:
```yaml
dq_rules:
  - rule: rate_of_change
    field: pm25
    max_change_per_minute: 100.0
    partition_by: [ndp_id]
    action: flag
```

**Resulting rule_params**:
```json
{
  "field": "pm25",
  "max_change_per_minute": 100.0,
  "partition_by": ["ndp_id"]
}
```

**Rate of Change Thresholds by Domain**:

| Domain | Field | Threshold | Rationale |
|--------|-------|-----------|-----------|
| Air Quality | pm25 | 100 ug/m3/min | Sudden spikes indicate sensor issues |
| Air Quality | temperature_c | 3.0 C/min | Indoor temp rarely changes faster |
| Weather | temperature_c | 2.0 C/min | Outdoor temp change limits |
| Weather | pressure_pa | 500 Pa/min | Rapid pressure = weather event |

---

### 3.8 `completeness_check`

Validates batch-level completeness (percentage of non-null values).

```json
{
  "level": "batch",
  "field": "pm25",
  "min_completeness": 0.95
}
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `level` | string | Yes | Must be "batch" for completeness checks |
| `field` | string | Yes | Column to check for completeness |
| `min_completeness` | number | Yes | Minimum required ratio (0.0-1.0) of non-null values |
| `time_window` | string | No | Optional time window (e.g., "1 hour") for windowed checks |

**Example from config**:
```yaml
dq_rules:
  - rule: completeness_check
    level: batch
    field: pm25
    min_completeness: 0.95
    action: warn
```

**Resulting rule_params**:
```json
{
  "level": "batch",
  "field": "pm25",
  "min_completeness": 0.95
}
```

---

## 4. Column-Level vs Table-Level Rules

### 4.1 Column-Level Rules

Stored with `silver_column` populated:

```sql
INSERT INTO data_dictionary.silver_dq_rules
  (silver_table, silver_column, rule_name, rule_params, action)
VALUES
  ('air_quality_observations', 'pm25', 'range_check',
   '{"min": 0.0, "max": 1000.0}', 'flag'),
  ('air_quality_observations', 'pm25', 'not_null',
   '{}', 'reject');
```

**Sources in config**:
- `field_mappings[].dq_rules[]` - Explicit column rules
- `field_mappings[].nullable: false` - Implicit not_null rule

### 4.2 Table-Level Rules

Stored with `silver_column = NULL`:

```sql
INSERT INTO data_dictionary.silver_dq_rules
  (silver_table, silver_column, rule_name, rule_params, action)
VALUES
  ('air_quality_observations', NULL, 'pm10_gte_pm25',
   '{"expression": "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25", "message": "pm10_less_than_pm25"}',
   'flag'),
  ('air_quality_observations', NULL, 'freshness_check',
   '{"field": "observation_time", "max_age": "2 hours", "reference": "ingestion_time"}',
   'flag');
```

**Sources in config**:
- `silver_etl.dq_rules[]` - All rules in this array are table-level

---

## 5. Action Types

| Action | Behavior | Row Persists | Use When |
|--------|----------|--------------|----------|
| `flag` | Add rule name to `dq_flags[]` array | Yes | Suspicious but queryable |
| `reject` | Exclude row from Silver table | No | Logically impossible values |
| `clamp` | Adjust value to min/max bounds | Yes | Physical constraints (0-100%) |
| `set_null_and_flag` | Set value to NULL + add to `dq_flags[]` | Yes | Invalid but row is useful |
| `warn` | Log only (no row-level action) | Yes | Unusual but valid |

### 5.1 Action Selection Guidelines

| Scenario | Recommended Action | Rationale |
|----------|-------------------|-----------|
| Value outside physical bounds | `clamp` | Force to valid range (e.g., humidity 0-100%) |
| Value outside expected range | `flag` | May be valid extreme; preserve for analysis |
| Logically impossible | `reject` | Cannot be correct (e.g., future observation time) |
| Cross-field constraint violated | `flag` | Preserve row, flag for investigation |
| Batch completeness below threshold | `warn` | Log alert, don't affect individual rows |
| Rate of change exceeded | `flag` | May indicate real event or sensor issue |

### 5.2 Action Priority

When multiple rules apply to the same value, actions are processed in this order:

1. `reject` - If any rule triggers reject, row is excluded
2. `clamp` - Applied before range_check if clamp_to_bounds is true
3. `set_null_and_flag` - Set value to NULL
4. `flag` - Add to dq_flags array
5. `warn` - Log only

---

## 6. Query Examples

### 6.1 "What DQ rules apply to pm25?"

```sql
SELECT
    silver_table,
    rule_name,
    rule_params,
    action
FROM data_dictionary.silver_dq_rules
WHERE silver_column = 'pm25'
ORDER BY silver_table, rule_name;
```

**Expected Result**:
| silver_table | rule_name | rule_params | action |
|--------------|-----------|-------------|--------|
| air_quality_observations | not_null | {} | reject |
| air_quality_observations | range_check | {"min": 0.0, "max": 1000.0} | flag |

---

### 6.2 "Which columns use range_check?"

```sql
SELECT
    silver_table,
    silver_column,
    rule_params->>'min' AS min_value,
    rule_params->>'max' AS max_value,
    action
FROM data_dictionary.silver_dq_rules
WHERE rule_name = 'range_check'
  AND silver_column IS NOT NULL
ORDER BY silver_table, silver_column;
```

**Expected Result**:
| silver_table | silver_column | min_value | max_value | action |
|--------------|---------------|-----------|-----------|--------|
| air_quality_observations | co2 | 380 | 10000 | flag |
| air_quality_observations | humidity_pct | 0.0 | 100.0 | clamp |
| air_quality_observations | pm25 | 0.0 | 1000.0 | flag |
| weather_forecasts | temperature_c | -50.0 | 60.0 | flag |
| ... | ... | ... | ... | ... |

---

### 6.3 "What are all table-level rules for weather_observations?"

```sql
SELECT
    rule_name,
    rule_params,
    action
FROM data_dictionary.silver_dq_rules
WHERE silver_table = 'weather_observations'
  AND silver_column IS NULL
ORDER BY rule_name;
```

**Expected Result**:
| rule_name | rule_params | action |
|-----------|-------------|--------|
| feels_like_reasonable | {"expression": "feels_like_c IS NULL OR ABS(feels_like_c - temperature_c) <= 20", "message": "feels_like_unreasonable"} | flag |
| freshness_check | {"field": "observation_time", "max_age": "3 hours", "max_future": "10 minutes", "reference": "ingestion_time"} | flag |
| temperature_rate_of_change | {"field": "temperature_c", "max_change_per_minute": 2.0, "partition_by": ["ndp_id"]} | flag |
| wind_gust_gte_speed | {"expression": "wind_gust_kmh IS NULL OR wind_gust_kmh >= wind_speed_kmh", "message": "gust_less_than_sustained"} | flag |

---

### 6.4 "Show all DQ rules with their full context"

```sql
SELECT
    r.silver_table,
    COALESCE(r.silver_column, '(table-level)') AS applies_to,
    r.rule_name,
    r.action,
    c.data_type,
    c.unit,
    r.rule_params
FROM data_dictionary.silver_dq_rules r
LEFT JOIN data_dictionary.silver_columns c
    ON r.silver_table = c.table_name
   AND r.silver_column = c.column_name
ORDER BY r.silver_table, r.silver_column NULLS LAST, r.rule_name;
```

---

### 6.5 "What percentage of rows are flagged by each rule?"

This query joins with DQ transparency data (runtime):

```sql
SELECT
    dq.rule_name,
    dq.violation_type,
    SUM(dq.row_count) AS violations,
    SUM(dq.row_count)::float /
        (SELECT COUNT(*) FROM silver.air_quality_observations
         WHERE observation_time > NOW() - INTERVAL '24 hours') * 100 AS pct
FROM silver.dq_transparency dq
WHERE dq.check_time > NOW() - INTERVAL '24 hours'
  AND dq.stream_id = 'air-quality'
GROUP BY dq.rule_name, dq.violation_type
ORDER BY violations DESC;
```

---

### 6.6 "Find all clamp rules and their bounds"

```sql
SELECT
    silver_table,
    silver_column,
    rule_params->>'min' AS clamp_min,
    rule_params->>'max' AS clamp_max
FROM data_dictionary.silver_dq_rules
WHERE action = 'clamp'
ORDER BY silver_table, silver_column;
```

---

### 6.7 "Which fields have freshness constraints?"

```sql
SELECT
    silver_table,
    rule_params->>'field' AS checked_field,
    rule_params->>'max_age' AS max_age,
    rule_params->>'max_future' AS max_future,
    rule_params->>'reference' AS reference_field
FROM data_dictionary.silver_dq_rules
WHERE rule_name = 'freshness_check';
```

---

## 7. Sync Logic

### 7.1 Extraction Algorithm

The sync process extracts DQ rules from two config locations:

#### From `field_mappings[].dq_rules[]` (Column-Level)

```python
for stream_config in stream_configs:
    if not stream_config.silver_etl:
        continue

    table_name = stream_config.silver_etl.target_table.split('.')[-1]

    for mapping in stream_config.silver_etl.field_mappings:
        column_name = mapping.target_column

        # Implicit not_null from nullable: false
        if mapping.nullable == False:
            emit_rule(
                silver_table=table_name,
                silver_column=column_name,
                rule_name='not_null',
                rule_params={},
                action='reject'  # Default action for not_null
            )

        # Explicit dq_rules
        for rule in mapping.dq_rules or []:
            params = extract_params(rule)
            emit_rule(
                silver_table=table_name,
                silver_column=column_name,
                rule_name=rule.rule,
                rule_params=params,
                action=rule.action
            )
```

#### From `silver_etl.dq_rules[]` (Table-Level)

```python
for stream_config in stream_configs:
    if not stream_config.silver_etl:
        continue

    table_name = stream_config.silver_etl.target_table.split('.')[-1]

    for rule in stream_config.silver_etl.dq_rules or []:
        params = extract_params(rule)

        # Use rule.name if present, else generate from rule type
        rule_name = rule.name if hasattr(rule, 'name') else f"{rule.rule}_{rule.field}"

        emit_rule(
            silver_table=table_name,
            silver_column=None,  # Table-level rule
            rule_name=rule_name,
            rule_params=params,
            action=rule.action
        )
```

### 7.2 Parameter Extraction

```python
def extract_params(rule: DQRule) -> dict:
    """Extract rule-specific parameters into JSONB-ready dict."""

    params = {}

    if rule.rule == 'range_check':
        params['min'] = rule.min
        params['max'] = rule.max
        if hasattr(rule, 'clamp_to_bounds'):
            params['clamp_to_bounds'] = rule.clamp_to_bounds

    elif rule.rule == 'cross_field_check':
        params['expression'] = rule.expression
        params['message'] = rule.message

    elif rule.rule == 'freshness_check':
        params['field'] = rule.field
        params['max_age'] = rule.max_age
        if hasattr(rule, 'max_future'):
            params['max_future'] = rule.max_future
        if hasattr(rule, 'reference'):
            params['reference'] = rule.reference

    elif rule.rule == 'rate_of_change':
        params['field'] = rule.field
        params['max_change_per_minute'] = rule.max_change_per_minute
        if hasattr(rule, 'partition_by'):
            params['partition_by'] = rule.partition_by

    elif rule.rule == 'completeness_check':
        params['level'] = rule.level
        params['field'] = rule.field
        params['min_completeness'] = rule.min_completeness

    elif rule.rule == 'pattern':
        params['regex'] = rule.regex
        if hasattr(rule, 'description'):
            params['description'] = rule.description

    elif rule.rule == 'enum_check':
        params['allowed_values'] = rule.allowed_values
        if hasattr(rule, 'case_sensitive'):
            params['case_sensitive'] = rule.case_sensitive

    return params
```

### 7.3 Upsert Strategy

The sync uses PostgreSQL upsert to maintain idempotency:

```sql
INSERT INTO data_dictionary.silver_dq_rules
    (silver_table, silver_column, rule_name, rule_params, action)
VALUES
    ($1, $2, $3, $4::jsonb, $5)
ON CONFLICT (silver_table, silver_column, rule_name)
DO UPDATE SET
    rule_params = EXCLUDED.rule_params,
    action = EXCLUDED.action;
```

**Note**: The unique constraint handles NULL silver_column correctly:
```sql
UNIQUE(silver_table, silver_column, rule_name)
-- Uses NULLS NOT DISTINCT for proper NULL handling
```

### 7.4 Handling Multiple Streams to Same Table

When multiple streams feed the same Silver table (e.g., `outdoor-weather` and `nws-observations` both feeding `weather_observations`), rules are merged:

1. Column-level rules are deduplicated by (table, column, rule_name)
2. If same rule exists with different params, last sync wins
3. Table-level rules use `name` attribute for uniqueness

---

## 8. Complete Table Schema

### 8.1 Final DDL

```sql
-- DQ rules per Silver column (or table-level if silver_column IS NULL)
CREATE TABLE data_dictionary.silver_dq_rules (
    id                  SERIAL PRIMARY KEY,
    silver_table        TEXT NOT NULL,
    silver_column       TEXT,           -- NULL for table-level rules
    rule_name           TEXT NOT NULL,  -- 'range_check', 'cross_field_check', etc.
    rule_params         JSONB NOT NULL DEFAULT '{}',
    action              TEXT NOT NULL,  -- 'flag', 'reject', 'clamp', 'warn'
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    updated_at          TIMESTAMPTZ DEFAULT NOW(),

    -- Unique constraint with proper NULL handling
    CONSTRAINT uq_silver_dq_rules
        UNIQUE NULLS NOT DISTINCT (silver_table, silver_column, rule_name),

    -- Validate action values
    CONSTRAINT chk_action
        CHECK (action IN ('flag', 'reject', 'clamp', 'set_null_and_flag', 'warn')),

    -- Validate rule_name values
    CONSTRAINT chk_rule_name
        CHECK (rule_name IN (
            'range_check', 'not_null', 'pattern', 'enum_check',
            'cross_field_check', 'freshness_check', 'rate_of_change',
            'completeness_check'
        ) OR rule_name LIKE '%_%')  -- Allow custom named rules
);

-- Index for column lookups
CREATE INDEX idx_dq_rules_column
ON data_dictionary.silver_dq_rules (silver_column)
WHERE silver_column IS NOT NULL;

-- Index for rule type analysis
CREATE INDEX idx_dq_rules_rule_name
ON data_dictionary.silver_dq_rules (rule_name);

-- Index for table lookups
CREATE INDEX idx_dq_rules_table
ON data_dictionary.silver_dq_rules (silver_table);

-- GIN index for JSONB queries
CREATE INDEX idx_dq_rules_params
ON data_dictionary.silver_dq_rules USING gin (rule_params);

-- Trigger for updated_at
CREATE TRIGGER trg_dq_rules_updated
    BEFORE UPDATE ON data_dictionary.silver_dq_rules
    FOR EACH ROW
    EXECUTE FUNCTION data_dictionary.update_timestamp();
```

### 8.2 Helper Views

```sql
-- Column-level rules with column metadata
CREATE VIEW data_dictionary.v_column_dq_rules AS
SELECT
    r.silver_table,
    r.silver_column,
    c.data_type,
    c.unit,
    r.rule_name,
    r.rule_params,
    r.action,
    CASE r.rule_name
        WHEN 'range_check' THEN
            format('Value must be between %s and %s',
                   r.rule_params->>'min', r.rule_params->>'max')
        WHEN 'not_null' THEN 'Value cannot be NULL'
        WHEN 'pattern' THEN
            format('Value must match pattern: %s', r.rule_params->>'regex')
        ELSE r.rule_name
    END AS rule_description
FROM data_dictionary.silver_dq_rules r
JOIN data_dictionary.silver_columns c
    ON r.silver_table = c.table_name
   AND r.silver_column = c.column_name
WHERE r.silver_column IS NOT NULL
ORDER BY r.silver_table, r.silver_column, r.rule_name;

-- Table-level rules (cross-field, batch, etc.)
CREATE VIEW data_dictionary.v_table_dq_rules AS
SELECT
    r.silver_table,
    r.rule_name,
    r.rule_params,
    r.action,
    CASE
        WHEN r.rule_params ? 'expression' THEN
            format('Cross-field: %s', r.rule_params->>'expression')
        WHEN r.rule_params ? 'max_age' THEN
            format('Freshness: max_age=%s', r.rule_params->>'max_age')
        WHEN r.rule_params ? 'max_change_per_minute' THEN
            format('Rate of change: max=%s/min on %s',
                   r.rule_params->>'max_change_per_minute',
                   r.rule_params->>'field')
        WHEN r.rule_params ? 'min_completeness' THEN
            format('Completeness: min=%s%% for %s',
                   (r.rule_params->>'min_completeness')::float * 100,
                   r.rule_params->>'field')
        ELSE 'Table-level rule'
    END AS rule_description
FROM data_dictionary.silver_dq_rules r
WHERE r.silver_column IS NULL
ORDER BY r.silver_table, r.rule_name;

-- All rules with human-readable summary
CREATE VIEW data_dictionary.v_all_dq_rules AS
SELECT
    silver_table,
    COALESCE(silver_column, '-- TABLE LEVEL --') AS column_or_level,
    rule_name,
    action,
    rule_params,
    CASE
        WHEN silver_column IS NULL THEN 'table'
        ELSE 'column'
    END AS rule_scope
FROM data_dictionary.silver_dq_rules
ORDER BY silver_table, silver_column NULLS LAST, rule_name;
```

---

## 9. Current Config Inventory

### 9.1 Air Quality Stream (`air-quality`)

**Target Table**: `silver.air_quality_observations`

**Column-Level Rules**:
| Column | Rule | Params | Action |
|--------|------|--------|--------|
| pm25 | not_null | {} | reject |
| pm25 | range_check | {"min": 0.0, "max": 1000.0} | flag |
| pm10 | range_check | {"min": 0.0, "max": 2000.0} | flag |
| co2 | range_check | {"min": 380, "max": 10000} | flag |
| temperature_c | range_check | {"min": -40.0, "max": 85.0} | flag |
| humidity_pct | range_check | {"min": 0.0, "max": 100.0, "clamp_to_bounds": true} | clamp |
| tvoc_index | range_check | {"min": 1, "max": 500, "clamp_to_bounds": true} | clamp |
| nox_index | range_check | {"min": 1, "max": 500, "clamp_to_bounds": true} | clamp |

**Table-Level Rules**:
| Rule Name | Params | Action |
|-----------|--------|--------|
| pm10_gte_pm25 | {"expression": "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25", "message": "pm10_less_than_pm25"} | flag |
| freshness_check | {"field": "observation_time", "max_age": "2 hours", "max_future": "5 minutes", "reference": "ingestion_time"} | flag |
| rate_of_change_pm25 | {"field": "pm25", "max_change_per_minute": 100.0, "partition_by": ["ndp_id"]} | flag |
| rate_of_change_temperature | {"field": "temperature_c", "max_change_per_minute": 3.0, "partition_by": ["ndp_id"]} | flag |
| completeness_pm25 | {"level": "batch", "field": "pm25", "min_completeness": 0.95} | warn |

---

### 9.2 Outdoor Weather Stream (`outdoor-weather`)

**Target Table**: `silver.weather_observations`

**Column-Level Rules**:
| Column | Rule | Params | Action |
|--------|------|--------|--------|
| temperature_c | not_null | {} | reject |
| temperature_c | range_check | {"min": -60.0, "max": 60.0} | flag |
| feels_like_c | range_check | {"min": -60.0, "max": 60.0} | flag |
| humidity_pct | range_check | {"min": 0.0, "max": 100.0, "clamp_to_bounds": true} | clamp |
| pressure_pa | range_check | {"min": 80000.0, "max": 120000.0} | flag |
| wind_speed_kmh | range_check | {"min": 0.0, "max": 400.0} | flag |
| wind_gust_kmh | range_check | {"min": 0.0, "max": 500.0} | flag |
| wind_direction_deg | range_check | {"min": 0.0, "max": 360.0, "clamp_to_bounds": true} | clamp |
| cloud_cover_pct | range_check | {"min": 0.0, "max": 100.0, "clamp_to_bounds": true} | clamp |
| visibility_m | range_check | {"min": 0.0, "max": 50000.0} | flag |
| precipitation_mm | range_check | {"min": 0.0, "max": 500.0} | flag |

**Table-Level Rules**:
| Rule Name | Params | Action |
|-----------|--------|--------|
| wind_gust_gte_speed | {"expression": "wind_gust_kmh IS NULL OR wind_gust_kmh >= wind_speed_kmh", "message": "gust_less_than_sustained"} | flag |
| feels_like_reasonable | {"expression": "feels_like_c IS NULL OR ABS(feels_like_c - temperature_c) <= 20", "message": "feels_like_unreasonable"} | flag |
| freshness_check | {"field": "observation_time", "max_age": "3 hours", "max_future": "10 minutes", "reference": "ingestion_time"} | flag |
| rate_of_change_temperature | {"field": "temperature_c", "max_change_per_minute": 2.0, "partition_by": ["ndp_id"]} | flag |
| rate_of_change_pressure | {"field": "pressure_pa", "max_change_per_minute": 500.0, "partition_by": ["ndp_id"]} | flag |
| completeness_temperature | {"level": "batch", "field": "temperature_c", "min_completeness": 0.98} | warn |

---

### 9.3 NWS Gridpoints Forecast Stream (`nws-gridpoints-forecast`)

**Target Table**: `silver.weather_forecasts`

**Column-Level Rules**:
| Column | Rule | Params | Action |
|--------|------|--------|--------|
| temperature_c | range_check | {"min": -50.0, "max": 60.0} | flag |
| dewpoint_c | range_check | {"min": -50.0, "max": 60.0} | flag |
| apparent_temp_c | range_check | {"min": -60.0, "max": 70.0} | flag |
| heat_index_c | range_check | {"min": -50.0, "max": 70.0} | flag |
| wind_chill_c | range_check | {"min": -70.0, "max": 30.0} | flag |
| wind_speed_kmh | range_check | {"min": 0.0, "max": 300.0} | flag |
| wind_direction_deg | range_check | {"min": 0.0, "max": 360.0, "clamp_to_bounds": true} | clamp |
| wind_gust_kmh | range_check | {"min": 0.0, "max": 400.0} | flag |
| precip_probability_pct | range_check | {"min": 0.0, "max": 100.0, "clamp_to_bounds": true} | clamp |
| precip_amount_mm | range_check | {"min": 0.0, "max": 500.0} | flag |
| humidity_pct | range_check | {"min": 0.0, "max": 100.0, "clamp_to_bounds": true} | clamp |
| sky_cover_pct | range_check | {"min": 0.0, "max": 100.0, "clamp_to_bounds": true} | clamp |
| visibility_m | range_check | {"min": 0.0, "max": 50000.0} | flag |

**Table-Level Rules**:
| Rule Name | Params | Action |
|-----------|--------|--------|
| wind_gust_gte_speed | {"expression": "wind_gust_kmh IS NULL OR wind_speed_kmh IS NULL OR wind_gust_kmh >= wind_speed_kmh", "message": "gust_less_than_sustained"} | flag |
| valid_time_after_issue | {"expression": "valid_time >= issue_time", "message": "valid_time_before_issue"} | flag |
| forecast_horizon_reasonable | {"expression": "EXTRACT(EPOCH FROM (valid_time - issue_time)) <= 604800", "message": "forecast_horizon_exceeds_7_days"} | flag |
| dewpoint_lte_temp | {"expression": "dewpoint_c IS NULL OR temperature_c IS NULL OR dewpoint_c <= temperature_c", "message": "dewpoint_exceeds_temperature"} | flag |

---

## 10. Implementation Notes

### 10.1 Backward Compatibility

New config fields are optional:
- Existing configs without explicit DQ rules continue to work
- `nullable: false` already implies not_null constraint
- No breaking changes to existing ETL

### 10.2 Performance Considerations

- DQ rules dictionary is small (hundreds of rows) - no performance concern
- JSONB indexes enable efficient parameter queries
- Views are simple joins - no materialization needed

### 10.3 Future Extensions

| Extension | Description | Priority |
|-----------|-------------|----------|
| Rule versioning | Track changes to DQ rules over time | Low |
| Severity levels | Add severity (critical, warning, info) to rules | Medium |
| Documentation links | Add URL field linking to rule documentation | Low |
| Rule groups | Group related rules for bulk enable/disable | Low |

---

## 11. References

- [04-LAYERED-DQ-STRATEGY.md](/workspaces/neural-data-platform/product/research/analyticplatforminfrastructure/04-LAYERED-DQ-STRATEGY.md) - DQ layer architecture
- [dp-009 SCOPE.md](/workspaces/neural-data-platform/product/features/dp-009/SCOPE.md) - Feature scope
- [air-quality/config.yaml](/workspaces/neural-data-platform/config/base/streams/air-quality/config.yaml) - Air quality stream config
- [outdoor-weather/config.yaml](/workspaces/neural-data-platform/config/base/streams/outdoor-weather/config.yaml) - Weather stream config
- [nws-gridpoints-forecast/config.yaml](/workspaces/neural-data-platform/config/base/streams/nws-gridpoints-forecast/config.yaml) - Forecast stream config

---

*Specification complete: 2026-01-16*
