# DQ Evaluator Module - Pseudocode

**Feature**: dp-006 (Silver Layer Implementation)
**Module**: DQ Evaluator
**Version**: 1.0
**Created**: 2026-01-10
**Author**: SPARC Pseudocode Agent
**Status**: Draft

---

## 1. Overview

The DQ Evaluator module converts Data Quality rule configurations from YAML into SQL expressions for inline evaluation during ETL. It supports all 5 rule types (range_check, not_null, pattern, one_of, custom) and all 4 actions (flag, reject, clamp, drop).

### 1.1 Design Principles

1. **Config-driven**: All DQ logic derived from YAML configuration
2. **Transparency first**: `flag` is the default action - preserve data, enable investigation
3. **SQL generation**: Rules compile to SQL CASE expressions at ETL time
4. **Composable**: Flag expressions aggregate into `dq_flags TEXT[]` column

### 1.2 Module Dependencies

```
DQ Evaluator
├── Input: DqRule enum (from config/dq_rules.rs)
├── Input: FieldMapping (source_path, target_column)
├── Output: DqExpression (value_expr, flag_expr)
└── Output: Combined dq_flags SQL fragment
```

---

## 2. Type Definitions

### 2.1 DQ Rule Types

```
ENUM DqRuleType:
    RANGE_CHECK     // Numeric bounds validation
    NOT_NULL        // Required field validation
    PATTERN         // Regex string validation
    ONE_OF          // Enumeration validation
    CUSTOM          // Arbitrary SQL expression

ENUM DqAction:
    FLAG            // Keep value, add to dq_flags (DEFAULT)
    REJECT          // Set to NULL, add to dq_flags
    CLAMP           // Adjust to bounds, add to dq_flags
    DROP            // Exclude entire row from output
```

### 2.2 Core Data Structures

```
STRUCT DqRule:
    rule_type: DqRuleType
    field: String                   // Target column name (not source path)
    action: DqAction               // Default: FLAG

    // Rule-specific parameters
    min: Optional<Float>           // For RANGE_CHECK
    max: Optional<Float>           // For RANGE_CHECK
    regex: Optional<String>        // For PATTERN
    values: Optional<List<String>> // For ONE_OF
    case_sensitive: Boolean        // For ONE_OF/PATTERN, default: false
    name: Optional<String>         // For CUSTOM
    expression: Optional<String>   // For CUSTOM

STRUCT DqExpression:
    value_sql: String              // SQL for the output value
    flag_sql: String               // SQL for the flag expression
    is_row_filter: Boolean         // True if action=DROP (WHERE clause)

STRUCT DqEvaluationResult:
    expressions: List<DqExpression>
    drop_conditions: List<String>  // Conditions for WHERE clause
    flag_column_sql: String        // ARRAY aggregation SQL
```

---

## 3. Core Functions

### 3.1 Function: generate_dq_expression

Converts a single DQ rule into SQL expressions for value and flag.

```
FUNCTION generate_dq_expression(
    rule: DqRule,
    source_expr: String            // SQL expression for the source value
) -> DqExpression

INPUT:
    rule        - DQ rule configuration from YAML
    source_expr - SQL expression that extracts the source value
                  Example: "json_extract(raw_payload, '$.pm02')::FLOAT"

OUTPUT:
    DqExpression containing:
        - value_sql: SQL for the (possibly transformed) output value
        - flag_sql: SQL CASE expression that returns flag string or NULL
        - is_row_filter: Boolean indicating if row should be filtered

ALGORITHM:
BEGIN
    // Dispatch based on rule type
    SWITCH rule.rule_type:
        CASE RANGE_CHECK:
            RETURN generate_range_check_expression(rule, source_expr)

        CASE NOT_NULL:
            RETURN generate_not_null_expression(rule, source_expr)

        CASE PATTERN:
            RETURN generate_pattern_expression(rule, source_expr)

        CASE ONE_OF:
            RETURN generate_one_of_expression(rule, source_expr)

        CASE CUSTOM:
            RETURN generate_custom_expression(rule, source_expr)

        DEFAULT:
            RAISE Error("Unknown DQ rule type: " + rule.rule_type)
    END SWITCH
END

COMPLEXITY:
    Time: O(1) - constant dispatch
    Space: O(n) where n = length of generated SQL strings
```

---

### 3.2 Function: generate_range_check_expression

Generates SQL for range validation with support for flag/reject/clamp/drop actions.

```
FUNCTION generate_range_check_expression(
    rule: DqRule,
    source_expr: String
) -> DqExpression

PRECONDITIONS:
    - rule.rule_type == RANGE_CHECK
    - At least one of rule.min or rule.max is defined

ALGORITHM:
BEGIN
    field := rule.field
    action := rule.action OR FLAG
    min_val := rule.min
    max_val := rule.max

    // Build condition that identifies violation
    violation_condition := build_range_violation_condition(
        source_expr, min_val, max_val
    )

    // Generate flag label
    flag_label := generate_flag_label(rule, field)

    // Generate SQL based on action
    SWITCH action:

        CASE FLAG:
            // Keep original value, generate flag when out of range
            value_sql := source_expr
            flag_sql := format_flag_case(
                violation_condition,
                flag_label
            )
            is_row_filter := false

        CASE REJECT:
            // Set to NULL when out of range, generate flag
            value_sql := format(
                "CASE WHEN {condition} THEN NULL ELSE {source} END",
                condition = violation_condition,
                source = source_expr
            )
            flag_sql := format_flag_case(
                violation_condition,
                flag_label + ":rejected"
            )
            is_row_filter := false

        CASE CLAMP:
            // Clamp to bounds, generate flag with original value
            value_sql := generate_clamp_sql(source_expr, min_val, max_val)
            flag_sql := generate_clamp_flag_sql(
                source_expr, min_val, max_val, flag_label
            )
            is_row_filter := false

        CASE DROP:
            // Value doesn't matter (row excluded), flag for logging
            value_sql := source_expr
            flag_sql := NULL  // No flag needed, row is dropped
            is_row_filter := true

    END SWITCH

    RETURN DqExpression {
        value_sql: value_sql,
        flag_sql: flag_sql,
        is_row_filter: is_row_filter
    }
END

SUBROUTINE build_range_violation_condition(
    source: String,
    min: Optional<Float>,
    max: Optional<Float>
) -> String
BEGIN
    conditions := []

    IF min IS NOT NULL THEN
        conditions.append(format("{source} < {min}", source=source, min=min))
    END IF

    IF max IS NOT NULL THEN
        conditions.append(format("{source} > {max}", source=source, max=max))
    END IF

    RETURN join(conditions, " OR ")
END

SUBROUTINE generate_clamp_sql(
    source: String,
    min: Optional<Float>,
    max: Optional<Float>
) -> String
BEGIN
    result := source

    IF min IS NOT NULL THEN
        result := format("GREATEST({result}, {min})", result=result, min=min)
    END IF

    IF max IS NOT NULL THEN
        result := format("LEAST({result}, {max})", result=result, max=max)
    END IF

    RETURN result
END

SUBROUTINE generate_clamp_flag_sql(
    source: String,
    min: Optional<Float>,
    max: Optional<Float>,
    base_label: String
) -> String
BEGIN
    // Generate flag with original -> clamped value for transparency
    cases := []

    IF min IS NOT NULL THEN
        cases.append(format(
            "WHEN {source} < {min} THEN '{label}:clamped:' || {source}::TEXT || '->{min}'",
            source=source, min=min, label=base_label
        ))
    END IF

    IF max IS NOT NULL THEN
        cases.append(format(
            "WHEN {source} > {max} THEN '{label}:clamped:' || {source}::TEXT || '->{max}'",
            source=source, max=max, label=base_label
        ))
    END IF

    RETURN format(
        "CASE {cases} ELSE NULL END",
        cases = join(cases, " ")
    )
END

COMPLEXITY:
    Time: O(1)
    Space: O(n) where n = SQL string length
```

---

### 3.3 Function: generate_not_null_expression

Generates SQL for required field validation.

```
FUNCTION generate_not_null_expression(
    rule: DqRule,
    source_expr: String
) -> DqExpression

ALGORITHM:
BEGIN
    field := rule.field
    action := rule.action OR REJECT  // Default for not_null is REJECT

    violation_condition := format("{source} IS NULL", source=source_expr)
    flag_label := format("not_null:{field}:missing", field=field)

    SWITCH action:

        CASE FLAG:
            // Keep NULL, add flag
            value_sql := source_expr
            flag_sql := format_flag_case(violation_condition, flag_label)
            is_row_filter := false

        CASE REJECT:
            // Value is already NULL, just add flag
            // (REJECT for not_null is same as FLAG since value is NULL)
            value_sql := source_expr
            flag_sql := format_flag_case(violation_condition, flag_label)
            is_row_filter := false

        CASE DROP:
            // Exclude rows with NULL values
            value_sql := source_expr
            flag_sql := NULL
            is_row_filter := true

    END SWITCH

    RETURN DqExpression {
        value_sql: value_sql,
        flag_sql: flag_sql,
        is_row_filter: is_row_filter
    }
END

NOTE: CLAMP action is not valid for not_null rule.
      If configured, treat as FLAG with warning.
```

---

### 3.4 Function: generate_pattern_expression

Generates SQL for regex pattern validation.

```
FUNCTION generate_pattern_expression(
    rule: DqRule,
    source_expr: String
) -> DqExpression

PRECONDITIONS:
    - rule.rule_type == PATTERN
    - rule.regex IS NOT NULL

ALGORITHM:
BEGIN
    field := rule.field
    action := rule.action OR FLAG
    regex := rule.regex
    case_sensitive := rule.case_sensitive OR false

    // PostgreSQL regex operator
    regex_op := IF case_sensitive THEN "~" ELSE "~*"

    // Violation: does NOT match pattern
    violation_condition := format(
        "NOT ({source} {op} '{pattern}')",
        source = source_expr,
        op = regex_op,
        pattern = escape_sql_string(regex)
    )

    flag_label := format("pattern:{field}:mismatch", field=field)

    SWITCH action:

        CASE FLAG:
            value_sql := source_expr
            flag_sql := format_flag_case(violation_condition, flag_label)
            is_row_filter := false

        CASE REJECT:
            value_sql := format(
                "CASE WHEN {condition} THEN NULL ELSE {source} END",
                condition = violation_condition,
                source = source_expr
            )
            flag_sql := format_flag_case(violation_condition, flag_label + ":rejected")
            is_row_filter := false

        CASE DROP:
            value_sql := source_expr
            flag_sql := NULL
            is_row_filter := true

    END SWITCH

    RETURN DqExpression {
        value_sql: value_sql,
        flag_sql: flag_sql,
        is_row_filter: is_row_filter
    }
END

NOTE: Regex must be valid PostgreSQL regex syntax.
      SQL injection prevented by parameterization or escaping.
```

---

### 3.5 Function: generate_one_of_expression

Generates SQL for enumeration validation.

```
FUNCTION generate_one_of_expression(
    rule: DqRule,
    source_expr: String
) -> DqExpression

PRECONDITIONS:
    - rule.rule_type == ONE_OF
    - rule.values IS NOT NULL AND length(rule.values) > 0

ALGORITHM:
BEGIN
    field := rule.field
    action := rule.action OR FLAG
    values := rule.values
    case_sensitive := rule.case_sensitive OR false

    // Build IN clause
    IF case_sensitive THEN
        // Case-sensitive comparison
        in_list := join(
            [format("'{v}'", v=escape_sql_string(v)) FOR v IN values],
            ", "
        )
        violation_condition := format(
            "{source} NOT IN ({in_list})",
            source = source_expr,
            in_list = in_list
        )
    ELSE
        // Case-insensitive comparison
        in_list := join(
            [format("'{v}'", v=escape_sql_string(upper(v))) FOR v IN values],
            ", "
        )
        violation_condition := format(
            "UPPER({source}) NOT IN ({in_list})",
            source = source_expr,
            in_list = in_list
        )
    END IF

    flag_label := format("one_of:{field}:invalid_value", field=field)

    SWITCH action:

        CASE FLAG:
            value_sql := source_expr
            flag_sql := format_flag_case(violation_condition, flag_label)
            is_row_filter := false

        CASE REJECT:
            value_sql := format(
                "CASE WHEN {condition} THEN NULL ELSE {source} END",
                condition = violation_condition,
                source = source_expr
            )
            flag_sql := format_flag_case(violation_condition, flag_label + ":rejected")
            is_row_filter := false

        CASE DROP:
            value_sql := source_expr
            flag_sql := NULL
            is_row_filter := true

    END SWITCH

    RETURN DqExpression {
        value_sql: value_sql,
        flag_sql: flag_sql,
        is_row_filter: is_row_filter
    }
END
```

---

### 3.6 Function: generate_custom_expression

Generates SQL for arbitrary custom validation expressions.

```
FUNCTION generate_custom_expression(
    rule: DqRule,
    source_expr: String
) -> DqExpression

PRECONDITIONS:
    - rule.rule_type == CUSTOM
    - rule.expression IS NOT NULL
    - rule.name IS NOT NULL

ALGORITHM:
BEGIN
    field := rule.field
    action := rule.action OR FLAG
    name := rule.name
    expression := rule.expression

    // Custom expression should return boolean
    // Violation when expression is FALSE
    violation_condition := format("NOT ({expr})", expr=expression)

    flag_label := format("custom:{name}", name=name)

    SWITCH action:

        CASE FLAG:
            value_sql := source_expr
            flag_sql := format_flag_case(violation_condition, flag_label)
            is_row_filter := false

        CASE REJECT:
            value_sql := format(
                "CASE WHEN {condition} THEN NULL ELSE {source} END",
                condition = violation_condition,
                source = source_expr
            )
            flag_sql := format_flag_case(violation_condition, flag_label + ":rejected")
            is_row_filter := false

        CASE DROP:
            value_sql := source_expr
            flag_sql := NULL
            is_row_filter := true

    END SWITCH

    RETURN DqExpression {
        value_sql: value_sql,
        flag_sql: flag_sql,
        is_row_filter: is_row_filter
    }
END

SECURITY NOTE:
    Custom expressions should be validated before use.
    Only allow safe SQL constructs (no DDL, DML, subqueries).
    Consider allowlist of functions and operators.
```

---

## 4. Value Generation with Action

### 4.1 Function: generate_value_with_action

Generates the final SQL expression for a field value, applying the DQ action.

```
FUNCTION generate_value_with_action(
    rule: DqRule,
    source_expr: String
) -> String

INPUT:
    rule        - DQ rule configuration
    source_expr - SQL expression for source value extraction

OUTPUT:
    SQL expression for the output value (possibly transformed by action)

ALGORITHM:
BEGIN
    dq_expr := generate_dq_expression(rule, source_expr)
    RETURN dq_expr.value_sql
END

EXAMPLES:

    // Example 1: FLAG action (value unchanged)
    Input:
        rule = { rule_type: RANGE_CHECK, field: "pm25", min: 0, max: 1000, action: FLAG }
        source_expr = "json_extract(raw_payload, '$.pm02')::FLOAT"

    Output:
        "json_extract(raw_payload, '$.pm02')::FLOAT"

    // Example 2: REJECT action (NULL on violation)
    Input:
        rule = { rule_type: RANGE_CHECK, field: "pm25", min: 0, max: 1000, action: REJECT }
        source_expr = "json_extract(raw_payload, '$.pm02')::FLOAT"

    Output:
        "CASE WHEN json_extract(raw_payload, '$.pm02')::FLOAT < 0 OR
              json_extract(raw_payload, '$.pm02')::FLOAT > 1000
         THEN NULL
         ELSE json_extract(raw_payload, '$.pm02')::FLOAT END"

    // Example 3: CLAMP action (bounded value)
    Input:
        rule = { rule_type: RANGE_CHECK, field: "humidity_pct", min: 0, max: 100, action: CLAMP }
        source_expr = "json_extract(raw_payload, '$.rhum')::FLOAT"

    Output:
        "LEAST(GREATEST(json_extract(raw_payload, '$.rhum')::FLOAT, 0.0), 100.0)"
```

---

## 5. Flag Label Generation

### 5.1 Function: generate_flag_label

Generates the structured flag label for a DQ violation.

```
FUNCTION generate_flag_label(
    rule: DqRule,
    column: String
) -> String

FORMAT: {rule_type}:{column}:{reason}

ALGORITHM:
BEGIN
    rule_name := lowercase(rule.rule_type.to_string())

    SWITCH rule.rule_type:
        CASE RANGE_CHECK:
            reason := "out_of_bounds"
        CASE NOT_NULL:
            reason := "missing"
        CASE PATTERN:
            reason := "mismatch"
        CASE ONE_OF:
            reason := "invalid_value"
        CASE CUSTOM:
            // Use the custom rule name as the label
            RETURN format("custom:{name}", name=rule.name)
    END SWITCH

    RETURN format("{rule}:{column}:{reason}",
        rule = rule_name,
        column = column,
        reason = reason
    )
END

EXAMPLES:
    - range_check:pm25:out_of_bounds
    - not_null:observation_time:missing
    - pattern:device_serial:mismatch
    - one_of:source_provider:invalid_value
    - custom:freshness_check
```

---

## 6. DQ Flags Array Construction

### 6.1 Function: combine_dq_flags

Aggregates all individual flag expressions into the dq_flags TEXT[] column.

```
FUNCTION combine_dq_flags(
    mappings: List<FieldMapping>
) -> String

INPUT:
    mappings - List of field mappings, each with optional dq_rules

OUTPUT:
    SQL expression that produces TEXT[] of all triggered flags

ALGORITHM:
BEGIN
    flag_expressions := []

    FOR EACH mapping IN mappings DO
        IF mapping.dq_rules IS NOT NULL THEN
            FOR EACH rule IN mapping.dq_rules DO
                // Generate source expression for this field
                source_expr := generate_source_expression(mapping)

                // Generate DQ expression
                dq_expr := generate_dq_expression(rule, source_expr)

                IF dq_expr.flag_sql IS NOT NULL THEN
                    flag_expressions.append(dq_expr.flag_sql)
                END IF
            END FOR
        END IF
    END FOR

    IF flag_expressions IS EMPTY THEN
        RETURN "'{}'::TEXT[]"  // Empty array
    END IF

    // Combine all flags into array, removing NULLs
    RETURN format(
        "ARRAY_REMOVE(ARRAY[{flags}], NULL)",
        flags = join(flag_expressions, ", ")
    )
END

EXAMPLE OUTPUT:
    ARRAY_REMOVE(ARRAY[
        CASE WHEN pm25 < 0.0 OR pm25 > 1000.0
             THEN 'range_check:pm25:out_of_bounds'
             ELSE NULL END,
        CASE WHEN observation_time IS NULL
             THEN 'not_null:observation_time:missing'
             ELSE NULL END,
        CASE WHEN humidity_pct < 0.0
             THEN 'range_check:humidity_pct:clamped:' || humidity_pct::TEXT || '->0.0'
             WHEN humidity_pct > 100.0
             THEN 'range_check:humidity_pct:clamped:' || humidity_pct::TEXT || '->100.0'
             ELSE NULL END
    ], NULL) AS dq_flags
```

---

### 6.2 Function: generate_drop_conditions

Collects all DROP action conditions for WHERE clause.

```
FUNCTION generate_drop_conditions(
    mappings: List<FieldMapping>
) -> List<String>

INPUT:
    mappings - List of field mappings with dq_rules

OUTPUT:
    List of SQL conditions that, when TRUE, should DROP the row

ALGORITHM:
BEGIN
    drop_conditions := []

    FOR EACH mapping IN mappings DO
        IF mapping.dq_rules IS NOT NULL THEN
            FOR EACH rule IN mapping.dq_rules DO
                IF rule.action == DROP THEN
                    source_expr := generate_source_expression(mapping)
                    dq_expr := generate_dq_expression(rule, source_expr)

                    // Invert the condition (keep rows that DON'T violate)
                    keep_condition := get_keep_condition(rule, source_expr)
                    drop_conditions.append(keep_condition)
                END IF
            END FOR
        END IF
    END FOR

    RETURN drop_conditions
END

SUBROUTINE get_keep_condition(
    rule: DqRule,
    source_expr: String
) -> String
BEGIN
    SWITCH rule.rule_type:
        CASE RANGE_CHECK:
            conditions := []
            IF rule.min IS NOT NULL THEN
                conditions.append(format("{source} >= {min}", source=source_expr, min=rule.min))
            END IF
            IF rule.max IS NOT NULL THEN
                conditions.append(format("{source} <= {max}", source=source_expr, max=rule.max))
            END IF
            RETURN join(conditions, " AND ")

        CASE NOT_NULL:
            RETURN format("{source} IS NOT NULL", source=source_expr)

        CASE PATTERN:
            op := IF rule.case_sensitive THEN "~" ELSE "~*"
            RETURN format("{source} {op} '{pattern}'",
                source=source_expr, op=op, pattern=rule.regex)

        CASE ONE_OF:
            // ... similar to generate_one_of_expression

        CASE CUSTOM:
            RETURN rule.expression
    END SWITCH
END

USAGE IN ETL:
    drop_conditions := generate_drop_conditions(mappings)
    where_clause := "WHERE " + join(drop_conditions, " AND ")
```

---

## 7. Helper Functions

### 7.1 format_flag_case

```
FUNCTION format_flag_case(
    condition: String,
    flag_label: String
) -> String

BEGIN
    RETURN format(
        "CASE WHEN {condition} THEN '{label}' ELSE NULL END",
        condition = condition,
        label = escape_sql_string(flag_label)
    )
END
```

### 7.2 escape_sql_string

```
FUNCTION escape_sql_string(value: String) -> String

BEGIN
    // Replace single quotes with escaped quotes
    escaped := replace(value, "'", "''")
    // Handle backslashes
    escaped := replace(escaped, "\\", "\\\\")
    RETURN escaped
END
```

### 7.3 generate_source_expression

```
FUNCTION generate_source_expression(mapping: FieldMapping) -> String

BEGIN
    // Build JSON extraction based on source_path
    parts := split(mapping.source_path, ".")

    IF parts[0] == "raw_payload" THEN
        json_path := "$." + join(parts[1:], ".")
        base_expr := format(
            "json_extract(raw_payload, '{path}')",
            path = json_path
        )
    ELSE
        // Direct column reference
        base_expr := mapping.source_path
    END IF

    // Apply type cast
    type_cast := get_type_cast(mapping.type)
    RETURN format("{expr}::{type}", expr=base_expr, type=type_cast)
END
```

---

## 8. Complete Example: Air Quality PM2.5

### 8.1 Configuration Input

```yaml
# From config/base/streams/air-quality/config.yaml

silver_etl:
  field_mappings:
    - source_path: raw_payload.pm02
      target_column: pm25
      type: double_precision
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag

        - rule: not_null
          action: flag  # PM2.5 is critical but sensor may be warming up
```

### 8.2 Generated SQL Components

**Source Expression:**
```sql
json_extract(raw_payload, '$.pm02')::DOUBLE PRECISION
```

**Value SQL (no transformation, FLAG action):**
```sql
json_extract(raw_payload, '$.pm02')::DOUBLE PRECISION AS pm25
```

**Flag SQL for range_check:**
```sql
CASE
    WHEN json_extract(raw_payload, '$.pm02')::DOUBLE PRECISION < 0.0
         OR json_extract(raw_payload, '$.pm02')::DOUBLE PRECISION > 1000.0
    THEN 'range_check:pm25:out_of_bounds'
    ELSE NULL
END
```

**Flag SQL for not_null:**
```sql
CASE
    WHEN json_extract(raw_payload, '$.pm02')::DOUBLE PRECISION IS NULL
    THEN 'not_null:pm25:missing'
    ELSE NULL
END
```

**Combined dq_flags Column:**
```sql
ARRAY_REMOVE(ARRAY[
    CASE
        WHEN json_extract(raw_payload, '$.pm02')::DOUBLE PRECISION < 0.0
             OR json_extract(raw_payload, '$.pm02')::DOUBLE PRECISION > 1000.0
        THEN 'range_check:pm25:out_of_bounds'
        ELSE NULL
    END,
    CASE
        WHEN json_extract(raw_payload, '$.pm02')::DOUBLE PRECISION IS NULL
        THEN 'not_null:pm25:missing'
        ELSE NULL
    END
], NULL) AS dq_flags
```

### 8.3 Full ETL Query Fragment

```sql
SELECT
    -- Identity/timestamp columns
    to_timestamp(timestamp / 1000000) AS observation_time,
    ndp_id,

    -- Field with DQ evaluation
    json_extract(raw_payload, '$.pm02')::DOUBLE PRECISION AS pm25,

    -- Other fields...
    json_extract(raw_payload, '$.rco2')::SMALLINT AS co2,
    json_extract(raw_payload, '$.atmp')::DOUBLE PRECISION AS temperature_c,
    LEAST(GREATEST(
        json_extract(raw_payload, '$.rhum')::DOUBLE PRECISION, 0.0
    ), 100.0) AS humidity_pct,

    -- DQ flags array
    ARRAY_REMOVE(ARRAY[
        -- PM2.5 range check
        CASE
            WHEN json_extract(raw_payload, '$.pm02')::DOUBLE PRECISION < 0.0
                 OR json_extract(raw_payload, '$.pm02')::DOUBLE PRECISION > 1000.0
            THEN 'range_check:pm25:out_of_bounds'
            ELSE NULL
        END,
        -- PM2.5 null check
        CASE
            WHEN json_extract(raw_payload, '$.pm02')::DOUBLE PRECISION IS NULL
            THEN 'not_null:pm25:missing'
            ELSE NULL
        END,
        -- Humidity clamp flag
        CASE
            WHEN json_extract(raw_payload, '$.rhum')::DOUBLE PRECISION < 0.0
            THEN 'range_check:humidity_pct:clamped:' ||
                 json_extract(raw_payload, '$.rhum')::TEXT || '->0.0'
            WHEN json_extract(raw_payload, '$.rhum')::DOUBLE PRECISION > 100.0
            THEN 'range_check:humidity_pct:clamped:' ||
                 json_extract(raw_payload, '$.rhum')::TEXT || '->100.0'
            ELSE NULL
        END
    ], NULL) AS dq_flags

FROM bronze_air_quality
WHERE
    -- Drop conditions (if any rules have action: drop)
    timestamp > :watermark
```

---

## 9. SQL Generation Patterns Summary

### 9.1 By Rule Type

| Rule Type | Violation Condition Pattern |
|-----------|----------------------------|
| `range_check` | `{source} < {min} OR {source} > {max}` |
| `not_null` | `{source} IS NULL` |
| `pattern` | `NOT ({source} ~ '{regex}')` |
| `one_of` | `{source} NOT IN ('{v1}', '{v2}', ...)` |
| `custom` | `NOT ({expression})` |

### 9.2 By Action Type

| Action | Value SQL Pattern | Flag SQL Pattern | WHERE Clause |
|--------|-------------------|------------------|--------------|
| `flag` | `{source}` | `CASE WHEN {violation} THEN '{label}' ELSE NULL END` | - |
| `reject` | `CASE WHEN {violation} THEN NULL ELSE {source} END` | `CASE WHEN {violation} THEN '{label}:rejected' ELSE NULL END` | - |
| `clamp` | `LEAST(GREATEST({source}, {min}), {max})` | `CASE WHEN {below_min} THEN '{label}:clamped:{val}->{min}' WHEN {above_max} THEN '{label}:clamped:{val}->{max}' ELSE NULL END` | - |
| `drop` | `{source}` | (no flag, row excluded) | `{source} BETWEEN {min} AND {max}` |

---

## 10. Complexity Analysis

### 10.1 Time Complexity

| Function | Complexity | Notes |
|----------|------------|-------|
| `generate_dq_expression` | O(1) | Dispatch + string formatting |
| `generate_range_check_expression` | O(1) | Fixed number of conditions |
| `generate_one_of_expression` | O(v) | v = number of allowed values |
| `combine_dq_flags` | O(m * r) | m = mappings, r = avg rules per mapping |
| `generate_drop_conditions` | O(m * r) | Same as above |

### 10.2 Space Complexity

| Function | Complexity | Notes |
|----------|------------|-------|
| `generate_dq_expression` | O(s) | s = SQL string length |
| `combine_dq_flags` | O(m * r * s) | Total SQL size |

### 10.3 Generated SQL Size

For a typical stream with 10 fields and 2 DQ rules per field:
- ~20 CASE expressions in dq_flags array
- ~2KB of SQL per stream
- Acceptable for ETL query generation

---

## 11. Error Handling

### 11.1 Configuration Errors

```
ENUM DqConfigError:
    MISSING_FIELD           // Rule references undefined field
    INVALID_REGEX           // Pattern regex doesn't compile
    EMPTY_VALUES            // one_of has no values
    INVALID_BOUNDS          // min > max for range_check
    MISSING_EXPRESSION      // custom rule without expression
    INVALID_ACTION          // Action not supported for rule type
```

### 11.2 Error Recovery

```
FUNCTION validate_dq_rules(
    mappings: List<FieldMapping>
) -> List<DqConfigError>

BEGIN
    errors := []

    FOR EACH mapping IN mappings DO
        IF mapping.dq_rules IS NOT NULL THEN
            FOR EACH rule IN mapping.dq_rules DO
                // Validate rule configuration
                rule_errors := validate_single_rule(rule, mapping)
                errors.extend(rule_errors)
            END FOR
        END IF
    END FOR

    RETURN errors
END

BEHAVIOR ON ERROR:
    - Log warning with rule details
    - Skip invalid rule (don't generate SQL)
    - Continue with other rules
    - Return partial DQ evaluation
```

---

## 12. Future Considerations

### 12.1 Temporal Rules (Not in Scope)

The architecture supports future addition of temporal rules:

```
// Future: rate_of_change
- rule: rate_of_change
  field: pm25
  max_change_per_minute: 100.0
  partition_by: [ndp_id]
  action: flag

// Requires window function in SQL:
CASE
    WHEN ABS(pm25 - LAG(pm25) OVER w) /
         NULLIF(EXTRACT(EPOCH FROM observation_time - LAG(observation_time) OVER w) / 60.0, 0)
         > 100.0
    THEN 'rate_of_change:pm25:exceeded'
    ELSE NULL
END
```

### 12.2 Batch-Level Rules (Not in Scope)

Post-ETL validation rules:

```
// Future: completeness_check
- rule: completeness_check
  level: batch
  field: pm25
  min_completeness: 0.95
  action: warn
```

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | SPARC Pseudocode Agent | Initial pseudocode |

---

## References

1. `product/features/dp-006/architecture/DQ-FRAMEWORK-DESIGN.md` - DQ framework design
2. `product/features/dp-006/architecture/ADR-006-004-dq-rule-actions.md` - DQ actions ADR
3. `product/features/dp-006/specification/SPECIFICATION.md` - Feature specification
4. `config/base/streams/air-quality/config.yaml` - Air quality config example
