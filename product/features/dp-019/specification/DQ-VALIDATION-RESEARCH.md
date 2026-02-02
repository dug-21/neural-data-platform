# DP-019: DQ Rule Validation Research

**Document Type**: Research / Specification
**Feature**: dp-019 (Config Validation Pipeline)
**Task ID**: 2.9 - DQ rule syntax validation
**Created**: 2026-02-02
**Author**: NDP Data Quality Engineer

---

## Executive Summary

This document catalogs all supported DQ rule types, operators, severity levels, and validation requirements for the NDP configuration validation pipeline. The research is based on analysis of:

- `core/src/config/silver_etl.rs` - DQ rule type definitions
- `core/src/silver/dq_evaluator.rs` - Streaming DQ evaluation logic
- `apps/silver-etl/src/dq.rs` - SQL generation for DQ rules
- `schemas/stream-config.v1.1.schema.json` - JSON Schema definitions
- `product/features/dp-006/architecture/DQ-FRAMEWORK-DESIGN.md` - Framework design
- `config/base/streams/*/config.yaml` - Existing config examples

---

## 1. DQ Rule Types (Complete Inventory)

### 1.1 Rule Type Hierarchy

```
DQ Rule Types (11 total)
|
+-- Value-Level Rules (single field validation)
|   +-- range_check        - Numeric bounds validation
|   +-- null_check         - Required field validation
|   +-- enum_check         - Value in allowed set
|   +-- pattern_check      - Regex pattern validation
|
+-- Temporal Rules (time-based validation)
|   +-- freshness_check    - Timestamp within expected window
|   +-- monotonic_check    - Values increase/decrease monotonically
|   +-- rate_of_change     - Max delta between consecutive values
|
+-- Cross-Field Rules (multi-field validation)
|   +-- cross_field_check  - Relationship between fields (SQL expression)
|   +-- conditional_check  - Value depends on another field
|
+-- Batch-Level Rules (aggregate validation)
    +-- completeness_check - Minimum non-null percentage
    +-- cardinality_check  - Expected distinct value count
```

### 1.2 Rule Type Details

| Rule Type | Category | Scope | Requires Field | Expression Support |
|-----------|----------|-------|----------------|-------------------|
| `range_check` | Value | Row | Yes (can inherit) | No |
| `null_check` | Value | Row | Yes (can inherit) | No |
| `enum_check` | Value | Row | Yes (can inherit) | No |
| `pattern_check` | Value | Row | Yes (can inherit) | Regex |
| `freshness_check` | Temporal | Row | Yes (can inherit) | Interval strings |
| `monotonic_check` | Temporal | Row | Yes (can inherit) | No |
| `rate_of_change` | Temporal | Row | Yes (can inherit) | No |
| `cross_field_check` | Cross-Field | Row | No | SQL expression |
| `conditional_check` | Cross-Field | Row | No | SQL condition + nested rule |
| `completeness_check` | Batch | Batch | Yes (can inherit) | No |
| `cardinality_check` | Batch | Batch | Yes (can inherit) | No |

---

## 2. DQ Rule Structure by Type

### 2.1 range_check

```yaml
- rule: range_check
  field: <string>           # Required at global level; optional in field_mappings
  min: <number>             # Optional: minimum value (inclusive)
  max: <number>             # Optional: maximum value (inclusive)
  action: <action>          # Default: flag
  clamp_to_bounds: <bool>   # Default: false
```

**Validation Requirements**:
- At least one of `min` or `max` must be specified
- If both specified: `min` < `max`
- `field` must reference valid Silver column (or inherit from parent mapping)
- `clamp_to_bounds` only applies when `action: clamp`

**Example**:
```yaml
- rule: range_check
  field: pm25
  min: 0.0
  max: 1000.0
  action: flag
```

### 2.2 null_check

```yaml
- rule: null_check
  field: <string>           # Required at global level; optional in field_mappings
  action: <action>          # Default: reject
```

**Validation Requirements**:
- `field` must reference valid Silver column (or inherit from parent mapping)

**Example**:
```yaml
- rule: null_check
  field: observation_time
  action: reject
```

### 2.3 enum_check

```yaml
- rule: enum_check
  field: <string>           # Required at global level; optional in field_mappings
  allowed_values: [<values>] # Required: list of valid string values
  case_sensitive: <bool>    # Default: false
  action: <action>          # Default: flag
```

**Validation Requirements**:
- `allowed_values` must be non-empty array
- `field` must reference valid Silver column (or inherit from parent mapping)

**Example**:
```yaml
- rule: enum_check
  field: wind_direction_cardinal
  allowed_values: [N, NE, E, SE, S, SW, W, NW]
  case_sensitive: false
  action: flag
```

### 2.4 pattern_check

```yaml
- rule: pattern_check
  field: <string>           # Required at global level; optional in field_mappings
  pattern: <regex>          # Required: POSIX regex pattern
  action: <action>          # Default: flag
```

**Validation Requirements**:
- `pattern` must be valid regex (validated by Rust `regex` crate)
- `field` must reference valid Silver column (or inherit from parent mapping)

**Example**:
```yaml
- rule: pattern_check
  field: device_serial
  pattern: "^[A-Z0-9]{8,12}$"
  action: flag
```

### 2.5 freshness_check

```yaml
- rule: freshness_check
  field: <string>           # Required at global level; optional in field_mappings
  max_age: <interval>       # Optional: e.g., "2 hours"
  max_future: <interval>    # Optional: e.g., "10 minutes"
  reference: <string>       # Default: "ingestion_time"
  action: <action>          # Default: flag
```

**Validation Requirements**:
- At least one of `max_age` or `max_future` should be specified
- `max_age` and `max_future` must be valid PostgreSQL interval strings
- `field` must reference valid timestamp column
- `reference` must be a valid timestamp column or `"NOW()"`

**Valid Interval Formats**:
```
"2 hours"
"30 minutes"
"1 day"
"5 minutes"
"1 hour 30 minutes"
```

**Example**:
```yaml
- rule: freshness_check
  field: observation_time
  max_age: "2 hours"
  max_future: "5 minutes"
  reference: ingestion_time
  action: flag
```

### 2.6 monotonic_check

```yaml
- rule: monotonic_check
  field: <string>                    # Required at global level; optional in field_mappings
  direction: <direction>             # Required: increasing|decreasing|strict_increasing
  partition_by: [<columns>]          # Required: grouping columns
  allow_reset: <bool>                # Default: false
  reset_threshold: <number>          # Optional: value indicating reset
  action: <action>                   # Default: flag
```

**Validation Requirements**:
- `direction` must be one of: `increasing`, `decreasing`, `strict_increasing`
- `partition_by` must be non-empty array of valid column names
- `field` must reference valid numeric column

**Example**:
```yaml
- rule: monotonic_check
  field: cumulative_rainfall
  direction: increasing
  partition_by: [ndp_id]
  allow_reset: true
  reset_threshold: 1000.0
  action: flag
```

### 2.7 rate_of_change

```yaml
- rule: rate_of_change
  field: <string>                    # Required at global level; optional in field_mappings
  max_change_per_minute: <number>    # Required: positive number
  partition_by: [<columns>]          # Required: grouping columns
  action: <action>                   # Default: flag
```

**Validation Requirements**:
- `max_change_per_minute` must be positive number
- `partition_by` must be non-empty array of valid column names
- `field` must reference valid numeric column

**Example**:
```yaml
- rule: rate_of_change
  field: temperature_c
  max_change_per_minute: 2.0
  partition_by: [ndp_id]
  action: flag
```

### 2.8 cross_field_check

```yaml
- rule: cross_field_check
  name: <string>            # Required: unique rule identifier
  expression: <sql_expr>    # Required: SQL boolean expression
  message: <string>         # Optional: custom flag message (defaults to name)
  action: <action>          # Default: flag
```

**Validation Requirements**:
- `name` must be unique within the config
- `expression` must be valid SQL boolean expression
- Column references in `expression` must be valid Silver columns

**Expression Operators** (see Section 3 for full details):
- Comparison: `>`, `<`, `>=`, `<=`, `=`, `!=`
- Logical: `AND`, `OR`, `NOT`
- Null checks: `IS NULL`, `IS NOT NULL`
- SQL functions: `ABS()`, `COALESCE()`, `EXTRACT()`

**Example**:
```yaml
- rule: cross_field_check
  name: pm10_gte_pm25
  expression: "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25"
  message: "pm10_less_than_pm25"
  action: flag
```

### 2.9 conditional_check

```yaml
- rule: conditional_check
  name: <string>            # Required: unique rule identifier
  condition: <sql_expr>     # Required: SQL boolean expression (when true, apply then_rule)
  then_rule: <dq_rule>      # Required: nested DQ rule to apply
  action: <action>          # Default: flag
```

**Validation Requirements**:
- `name` must be unique within the config
- `condition` must be valid SQL boolean expression
- `then_rule` must be a valid DQ rule (recursive validation)

**Example**:
```yaml
- rule: conditional_check
  name: rain_requires_precip
  condition: "weather_condition = 'rain'"
  then_rule:
    rule: range_check
    field: precipitation_mm
    min: 0.1
    max: 500.0
  action: flag
```

### 2.10 completeness_check

```yaml
- rule: completeness_check
  level: batch              # Required: must be "batch"
  field: <string>           # Required at global level; optional in field_mappings
  min_completeness: <float> # Required: 0.0-1.0
  action: <action>          # Default: warn
```

**Validation Requirements**:
- `level` must be `"batch"`
- `min_completeness` must be between 0.0 and 1.0 (inclusive)
- `field` must reference valid Silver column

**Example**:
```yaml
- rule: completeness_check
  level: batch
  field: pm25
  min_completeness: 0.95
  action: warn
```

### 2.11 cardinality_check

```yaml
- rule: cardinality_check
  level: batch              # Required: must be "batch"
  field: <string>           # Required at global level; optional in field_mappings
  expected_range: [<min>, <max>]  # Required: tuple of integers
  action: <action>          # Default: warn
```

**Validation Requirements**:
- `level` must be `"batch"`
- `expected_range` must be array of exactly 2 integers
- `expected_range[0]` <= `expected_range[1]`
- `field` must reference valid Silver column

**Example**:
```yaml
- rule: cardinality_check
  level: batch
  field: ndp_id
  expected_range: [1, 10]
  action: warn
```

---

## 3. DQ Expression Operators

### 3.1 Comparison Operators (Streaming Evaluator)

The streaming evaluator (`dq_evaluator.rs`) supports these operators in `cross_field_check` expressions:

| Operator | Symbol | Description | Example |
|----------|--------|-------------|---------|
| Greater Than | `>` | Strict greater | `pm10 > pm25` |
| Less Than | `<` | Strict less | `temperature_c < 40` |
| Greater or Equal | `>=` | Greater or equal | `pm10 >= pm25` |
| Less or Equal | `<=` | Less or equal | `dewpoint_c <= temperature_c` |
| Equal | `=` | Equality | `status = 1` |
| Not Equal | `!=` | Inequality | `category != 0` |

**Implementation Note**: The streaming evaluator parses operators in this order to avoid prefix conflicts:
1. `>=`, `<=`, `!=` (two-character operators first)
2. `>`, `<`, `=` (single-character operators)

### 3.2 Logical Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `AND` | Logical AND (short-circuit) | `a > 0 AND b > 0` |
| `OR` | Logical OR (short-circuit) | `a IS NULL OR a > 0` |
| `NOT` | Logical negation | `NOT (a > b)` |

**Precedence**: `NOT` > `AND` > `OR`
**Parentheses**: Supported for grouping

### 3.3 Null Handling Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `IS NULL` | Check for null | `pm10 IS NULL` |
| `IS NOT NULL` | Check for non-null | `temperature_c IS NOT NULL` |

**Pattern**: Always check for NULL before comparisons in cross-field checks:
```sql
pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25
```

### 3.4 SQL Functions (SQL Generator)

The SQL generator (`dq.rs`) supports these functions in generated SQL:

| Function | Category | Description | Example |
|----------|----------|-------------|---------|
| `ABS()` | Math | Absolute value | `ABS(a - b) <= 10` |
| `GREATEST()` | Math | Maximum of values | `GREATEST(value, 0.0)` |
| `LEAST()` | Math | Minimum of values | `LEAST(value, 100.0)` |
| `NULLIF()` | Null | Return NULL if equal | `NULLIF(divisor, 0)` |
| `COALESCE()` | Null | First non-null | `COALESCE(a, b, 0)` |
| `EXTRACT()` | Temporal | Extract from timestamp | `EXTRACT(EPOCH FROM ts)` |
| `UPPER()` | String | Uppercase | `UPPER(status)` |
| `LAG()` | Window | Previous row value | `LAG(value) OVER (...)` |

### 3.5 Interval Expressions

For `freshness_check`, intervals must be PostgreSQL-compatible:

```
INTERVAL 'N unit'
```

**Valid Units**:
- `seconds`, `second`, `sec`, `s`
- `minutes`, `minute`, `min`, `m` (except when 'm' means month)
- `hours`, `hour`, `h`
- `days`, `day`, `d`
- `weeks`, `week`, `w`
- `months`, `month`
- `years`, `year`

**Examples**:
```yaml
max_age: "2 hours"
max_age: "30 minutes"
max_age: "1 day"
max_age: "1 hour 30 minutes"
```

---

## 4. DQ Action Types (Severity Levels)

### 4.1 Action Enumeration

```rust
pub enum DqAction {
    Flag,   // Default - Keep value, add to dq_flags
    Reject, // Set value to NULL, add to dq_flags
    Clamp,  // Adjust to bounds, add to dq_flags
    Drop,   // Filter row in WHERE clause (not in Silver)
    Warn,   // Log warning (batch-level only)
}
```

### 4.2 Action Details

| Action | Value Written | Row Persists | dq_flags Entry | Use Case |
|--------|---------------|--------------|----------------|----------|
| `flag` | Original | Yes | Yes | Suspicious but possible value |
| `reject` | NULL | Yes | Yes | Invalid value that breaks queries |
| `clamp` | Clamped value | Yes | Yes | Physical constraints (0-100%) |
| `drop` | N/A | No | In transparency | Catastrophically invalid row |
| `warn` | Original | Yes | No | Batch-level informational |

### 4.3 Action Validation Rules

| Rule Type | Valid Actions | Default Action |
|-----------|---------------|----------------|
| `range_check` | flag, reject, clamp | flag |
| `null_check` | flag, reject | reject |
| `enum_check` | flag, reject | flag |
| `pattern_check` | flag, reject | flag |
| `freshness_check` | flag, reject | flag |
| `monotonic_check` | flag | flag |
| `rate_of_change` | flag | flag |
| `cross_field_check` | flag, reject | flag |
| `conditional_check` | flag, reject | flag |
| `completeness_check` | warn, flag | warn |
| `cardinality_check` | warn, flag | warn |

---

## 5. Validation Requirements Summary

### 5.1 Structural Validation (JSON Schema)

| Check | Location | Description |
|-------|----------|-------------|
| Required `rule` field | `dq_rules[].rule` | Must be valid rule type |
| Valid action | `dq_rules[].action` | Must be in action enum |
| Type constraints | Various | Numbers must be numbers, etc. |
| Array constraints | `allowed_values`, `partition_by` | Non-empty arrays |

### 5.2 Semantic Validation (Rust Code)

| Check | Description | Error Example |
|-------|-------------|---------------|
| Field existence | `field` must be valid Silver column | "Field 'typo_field' not found" |
| Range validity | `min` < `max` for range_check | "min (100) must be less than max (50)" |
| Completeness range | 0.0 <= `min_completeness` <= 1.0 | "min_completeness must be 0-1" |
| Cardinality range | `expected_range[0]` <= `expected_range[1]` | "Range start > end" |
| Regex validity | `pattern` must compile | "Invalid regex: ..." |
| Interval validity | Interval strings must be valid | "Invalid interval: '2 hoursss'" |
| Expression syntax | SQL expressions must parse | "Invalid expression: missing operator" |
| Column references | Columns in expressions exist | "Unknown column: 'typo_col'" |
| Unique rule names | `name` must be unique | "Duplicate rule name: 'my_check'" |

### 5.3 Cross-Reference Validation

| Check | Source | Target | Error |
|-------|--------|--------|-------|
| Field in Silver schema | `dq_rules[].field` | `silver_etl.field_mappings[].target_column` | "DQ rule references unknown field" |
| Partition columns | `partition_by[]` | Valid column names | "Unknown partition column" |
| Reference column | `freshness_check.reference` | Valid timestamp column | "Unknown reference column" |

---

## 6. Expression Validation Rules

### 6.1 Cross-Field Expression Parsing

The validator must check that `expression` in `cross_field_check`:

1. **Parses successfully** as SQL boolean expression
2. **References valid columns** from Silver schema
3. **Uses supported operators** (see Section 3)
4. **Has balanced parentheses**
5. **Does not contain subqueries** (not supported in streaming)

### 6.2 Expression Grammar (Simplified)

```bnf
expression     ::= or_expression
or_expression  ::= and_expression ( 'OR' and_expression )*
and_expression ::= comparison ( 'AND' comparison )*
comparison     ::= term comparator term
                 | term 'IS' 'NULL'
                 | term 'IS' 'NOT' 'NULL'
                 | '(' expression ')'
                 | 'NOT' comparison
comparator     ::= '>' | '<' | '>=' | '<=' | '=' | '!='
term           ::= column_name | literal
column_name    ::= [a-z_][a-z0-9_]*
literal        ::= number | string
```

### 6.3 Common Expression Patterns

| Pattern | Example | Validation |
|---------|---------|------------|
| Null-safe comparison | `a IS NULL OR b IS NULL OR a >= b` | Columns a, b exist |
| Absolute difference | `ABS(a - b) <= threshold` | Columns a, b exist |
| Temporal ordering | `valid_time >= issue_time` | Both are timestamps |
| Range check | `value >= 0 AND value <= 100` | Column value exists |
| Physical constraint | `dewpoint_c <= temperature_c` | Both columns exist |

---

## 7. Field Inheritance Rules

### 7.1 Where Rules Can Appear

```yaml
silver_etl:
  # Global DQ rules - field is REQUIRED
  dq_rules:
    - rule: range_check
      field: pm25           # Required - no parent to inherit from
      min: 0.0
      max: 1000.0

  field_mappings:
    - source_path: raw_payload.pm25
      target_column: pm25
      # Per-field DQ rules - field is OPTIONAL (inherits from parent)
      dq_rules:
        - rule: range_check    # field defaults to "pm25" from parent
          min: 0.0
          max: 1000.0
```

### 7.2 Inheritance Validation

| Rule Location | Field Required | Inherited From |
|---------------|----------------|----------------|
| `silver_etl.dq_rules[]` | Yes | N/A |
| `field_mappings[].dq_rules[]` | No | `field_mappings[].target_column` |

### 7.3 Exception: Cross-Field Rules

`cross_field_check` and `conditional_check` do NOT have a `field` attribute - they operate on multiple fields via `expression`.

---

## 8. Validator Implementation Recommendations

### 8.1 Validation Order

```
1. JSON syntax validation (parsing)
2. JSON Schema validation (structure)
3. DQ rule type validation (enum check)
4. DQ rule parameter validation (per-type checks)
5. Expression parsing validation (for cross_field_check)
6. Cross-reference validation (field existence)
7. Uniqueness validation (rule names)
```

### 8.2 Error Message Format

```json
{
  "layer": "semantic",
  "path": "$.silver_etl.dq_rules[2].expression",
  "rule_type": "cross_field_check",
  "message": "Unknown column 'typo_col' in expression",
  "severity": "error",
  "suggestion": "Did you mean 'temperature_c'?"
}
```

### 8.3 Validation Functions Needed

```rust
// From dp-019 validator
fn validate_dq_rules(
    rules: &[DqRule],
    silver_columns: &HashSet<String>,
) -> Vec<ValidationError>;

fn validate_expression(
    expression: &str,
    available_columns: &HashSet<String>,
) -> Result<(), ExpressionError>;

fn validate_interval(interval: &str) -> Result<(), IntervalError>;

fn validate_regex(pattern: &str) -> Result<(), RegexError>;
```

---

## 9. Test Cases for Validator

### 9.1 Valid Cases

```yaml
# Test: Valid range_check with all fields
- rule: range_check
  field: pm25
  min: 0.0
  max: 1000.0
  action: flag

# Test: Valid cross_field_check with null handling
- rule: cross_field_check
  name: pm10_gte_pm25
  expression: "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25"
  action: flag

# Test: Valid freshness_check with intervals
- rule: freshness_check
  field: observation_time
  max_age: "2 hours"
  max_future: "10 minutes"
  reference: ingestion_time
```

### 9.2 Invalid Cases

```yaml
# Error: range_check with min > max
- rule: range_check
  field: pm25
  min: 1000.0
  max: 0.0           # Invalid: min > max

# Error: range_check with neither min nor max
- rule: range_check
  field: pm25        # Invalid: needs at least min or max

# Error: Unknown field reference
- rule: range_check
  field: typo_field  # Invalid: field doesn't exist
  min: 0.0

# Error: Invalid regex
- rule: pattern_check
  field: serial
  pattern: "[invalid(regex"  # Invalid: unclosed bracket

# Error: Invalid interval
- rule: freshness_check
  field: observation_time
  max_age: "2 hoursss"       # Invalid: typo

# Error: Unknown column in expression
- rule: cross_field_check
  name: bad_check
  expression: "typo_col >= 0"  # Invalid: unknown column

# Error: Invalid expression syntax
- rule: cross_field_check
  name: bad_syntax
  expression: "pm25 >= AND pm10"  # Invalid: missing operand

# Error: completeness_check out of range
- rule: completeness_check
  level: batch
  field: pm25
  min_completeness: 1.5       # Invalid: > 1.0
```

---

## 10. References

### 10.1 Source Files

| File | Purpose |
|------|---------|
| `core/src/config/silver_etl.rs` | DqRule enum definition |
| `core/src/silver/dq_evaluator.rs` | Streaming DQ evaluation |
| `apps/silver-etl/src/dq.rs` | SQL generation |
| `schemas/stream-config.v1.1.schema.json` | JSON Schema |
| `product/features/dp-006/architecture/DQ-FRAMEWORK-DESIGN.md` | Framework design |
| `product/features/dp-009/specification/dq-rules-spec.md` | DQ rules spec |

### 10.2 Related Features

- **dp-006**: Silver Layer Implementation (DQ framework)
- **dp-009**: Config-Driven Silver Layer Data Dictionary
- **dp-016**: Configuration Architecture Review
- **dp-018**: JSON Config Foundation
- **dp-019**: Config Validation Pipeline (this feature)

---

## Appendix A: Complete Rule Type Reference

| Rule Type | Required Fields | Optional Fields | Inheritable Field |
|-----------|-----------------|-----------------|-------------------|
| `range_check` | (min or max) | min, max, action, clamp_to_bounds | field |
| `null_check` | | action | field |
| `enum_check` | allowed_values | case_sensitive, action | field |
| `pattern_check` | pattern | action | field |
| `freshness_check` | (max_age or max_future) | max_age, max_future, reference, action | field |
| `monotonic_check` | direction, partition_by | allow_reset, reset_threshold, action | field |
| `rate_of_change` | max_change_per_minute, partition_by | action | field |
| `cross_field_check` | name, expression | message, action | N/A |
| `conditional_check` | name, condition, then_rule | action | N/A |
| `completeness_check` | level, min_completeness | action | field |
| `cardinality_check` | level, expected_range | action | field |

---

## Appendix B: Action Compatibility Matrix

| Rule Type | flag | reject | clamp | drop | warn |
|-----------|------|--------|-------|------|------|
| `range_check` | Yes | Yes | Yes | No | No |
| `null_check` | Yes | Yes | No | No | No |
| `enum_check` | Yes | Yes | No | No | No |
| `pattern_check` | Yes | Yes | No | No | No |
| `freshness_check` | Yes | Yes | No | No | No |
| `monotonic_check` | Yes | No | No | No | No |
| `rate_of_change` | Yes | No | No | No | No |
| `cross_field_check` | Yes | Yes | No | No | No |
| `conditional_check` | Yes | Yes | No | No | No |
| `completeness_check` | Yes | No | No | No | Yes |
| `cardinality_check` | Yes | No | No | No | Yes |

---

*Research completed: 2026-02-02*
*Author: NDP Data Quality Engineer*
