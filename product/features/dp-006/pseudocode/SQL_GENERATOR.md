# SQL Generator Module - Pseudocode Specification

**Feature**: dp-006 (Silver Layer Implementation)
**Phase**: Pseudocode
**Version**: 1.0
**Date**: 2026-01-10
**Author**: NDP Pseudocode Specialist
**Status**: Draft

---

## 1. Overview

This document specifies the algorithmic design for the SQL Generator module, which transforms `SilverEtlConfig` YAML configuration into executable DuckDB SQL statements for Bronze-to-Silver ETL operations.

### 1.1 Design Goals

1. **Config-Driven**: Generate SQL entirely from YAML configuration
2. **Transform Support**: Handle all 6 transform types defined in ADR-006-001
3. **DQ Integration**: Generate DQ rule expressions with all 4 action types
4. **Idempotent Output**: Same config always produces identical SQL
5. **Type Safety**: Proper type coercion for PostgreSQL target types

### 1.2 Module Responsibilities

```
┌─────────────────────────────────────────────────────────────────┐
│                     SQL Generator Module                         │
├─────────────────────────────────────────────────────────────────┤
│  INPUT:  SilverEtlConfig (from YAML/etcd)                       │
│  OUTPUT: Complete DuckDB SQL INSERT...SELECT statement           │
├─────────────────────────────────────────────────────────────────┤
│  RESPONSIBILITIES:                                               │
│  1. Generate SELECT clause from field_mappings                   │
│  2. Generate transform expressions for each field                │
│  3. Generate DQ CASE expressions for validation                  │
│  4. Generate dq_flags array aggregation                          │
│  5. Generate FROM clause with Parquet glob                       │
│  6. Generate WHERE clause for incremental/watermark              │
│  7. Generate ON CONFLICT for upsert strategy                     │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. Data Structures

### 2.1 Input Types

```
TYPE SilverEtlConfig:
    enabled: Boolean
    target_table: String                    # e.g., "silver.air_quality_observations"
    target_schema: String                   # Schema version reference
    timestamp: TimestampMapping
    identity_fields: Array<IdentityField>
    field_mappings: Array<SilverFieldMapping>
    dq_output: DqOutputConfig
    deduplication: DeduplicationConfig
    incremental: IncrementalConfig

TYPE TimestampMapping:
    source_field: String                    # Bronze column (e.g., "timestamp")
    target_field: String                    # Silver column (e.g., "observation_time")
    transform: TimestampTransform           # microseconds_to_timestamp | iso8601 | unix_seconds

TYPE IdentityField:
    source: String                          # JSON path in Bronze (e.g., "context.source_type.provider")
    target: String                          # Silver column name (e.g., "source_provider")

TYPE SilverFieldMapping:
    source_path: String                     # JSON path (e.g., "raw_payload.pm02")
    target_column: String                   # Silver column (e.g., "pm25")
    type: String                            # PostgreSQL type (e.g., "double_precision")
    nullable: Boolean
    transform: Optional<TransformConfig>
    dq_rules: Array<DqRule>

TYPE TransformConfig:
    VARIANT UnitConversion:
        from: String                        # Source unit (e.g., "kelvin")
        to: String                          # Target unit (e.g., "celsius")
        formula: ConversionFormula
    VARIANT Expression:
        expr: String                        # SQL expression
    VARIANT Lookup:
        table: Map<String, String>          # Value mapping table
    VARIANT JsonExtract:
        path: String                        # Additional JSON path
    VARIANT Timestamp:
        format: TimestampTransform
    VARIANT Computed:
        depends_on: Array<String>           # Column dependencies
        expr: String                        # Computation expression

TYPE ConversionFormula:
    VARIANT Linear:
        scale: Float                        # Multiplier
        offset: Float                       # Additive offset
    VARIANT Custom:
        code: String                        # Custom expression

TYPE DqRule:
    VARIANT RangeCheck:
        field: String
        min: Optional<Float>
        max: Optional<Float>
        action: DqAction
    VARIANT NullCheck:
        field: String
        action: DqAction
    VARIANT EnumCheck:
        field: String
        allowed_values: Array<String>
        case_sensitive: Boolean
        action: DqAction
    VARIANT PatternCheck:
        field: String
        pattern: String                     # Regex pattern
        action: DqAction
    VARIANT CrossFieldCheck:
        name: String
        expression: String                  # SQL boolean expression
        message: Optional<String>
        action: DqAction
    VARIANT Custom:
        name: String
        expr: String
        action: DqAction

TYPE DqAction:
    Flag        # Keep value, add to dq_flags
    Reject      # Set to NULL, add to dq_flags
    Clamp       # Clamp to bounds, add to dq_flags
    Drop        # Exclude entire row

TYPE DqOutputConfig:
    enabled: Boolean
    target_column: String                   # Default: "dq_flags"
    include_rules: Boolean
    include_values: Boolean

TYPE DeduplicationConfig:
    enabled: Boolean
    key_columns: Array<String>
    strategy: DeduplicationStrategy         # upsert | skip | replace

TYPE IncrementalConfig:
    enabled: Boolean
    watermark_column: String
    lag_interval: String                    # e.g., "5 minutes"
```

### 2.2 Output Types

```
TYPE GeneratedSql:
    insert_statement: String                # Complete INSERT...SELECT
    target_columns: Array<String>           # Column names for INSERT
    select_expressions: Array<String>       # Expressions for SELECT
    dq_flag_expressions: Array<String>      # DQ flag CASE expressions
    where_clauses: Array<String>            # Filter conditions
    conflict_clause: Optional<String>       # ON CONFLICT for upsert

TYPE TransformResult:
    expression: String                      # SQL expression for value
    source_reference: String                # How to reference source value
    type_cast: String                       # PostgreSQL type cast
```

---

## 3. Algorithm Specifications

### 3.1 Main Entry Point: generate_etl_sql

```
ALGORITHM: generate_etl_sql
INPUT: config (SilverEtlConfig), stream_id (String), parquet_path (String)
OUTPUT: sql (String)

PRECONDITIONS:
    - config.enabled = true
    - config.target_table is non-empty
    - config.field_mappings is non-empty

BEGIN
    // Step 1: Build target column list
    columns <- []
    columns.append("ingestion_time")
    columns.append(config.timestamp.target_field)

    FOR EACH identity IN config.identity_fields DO
        columns.append(identity.target)
    END FOR

    FOR EACH mapping IN config.field_mappings DO
        columns.append(mapping.target_column)
    END FOR

    IF config.dq_output.enabled THEN
        columns.append(config.dq_output.target_column)
    END IF

    // Step 2: Generate SELECT clause
    select_clause <- generate_select_clause(config)

    // Step 3: Generate FROM clause with Parquet glob
    from_clause <- generate_from_clause(parquet_path, config.incremental)

    // Step 4: Generate WHERE clause for incremental + drop rules
    where_clause <- generate_where_clause(config)

    // Step 5: Generate ON CONFLICT clause for deduplication
    conflict_clause <- generate_conflict_clause(config.deduplication, columns)

    // Step 6: Assemble final SQL
    sql <- FORMAT(
        "INSERT INTO {target_table} ({column_list})\n{select_clause}\n{from_clause}\n{where_clause}\n{conflict_clause}",
        target_table = config.target_table,
        column_list = JOIN(columns, ", "),
        select_clause = select_clause,
        from_clause = from_clause,
        where_clause = where_clause,
        conflict_clause = conflict_clause
    )

    RETURN sql
END

COMPLEXITY:
    Time: O(n) where n = number of field mappings
    Space: O(n) for storing generated expressions
```

### 3.2 SELECT Clause Generation

```
ALGORITHM: generate_select_clause
INPUT: config (SilverEtlConfig)
OUTPUT: select_sql (String)

BEGIN
    expressions <- []

    // 1. Ingestion timestamp (always current_timestamp)
    expressions.append("current_timestamp AS ingestion_time")

    // 2. Observation timestamp
    ts_expr <- generate_timestamp_expr(config.timestamp)
    expressions.append(FORMAT("{expr} AS {target}",
        expr = ts_expr,
        target = config.timestamp.target_field))

    // 3. Identity fields (passthrough with JSON extraction)
    FOR EACH identity IN config.identity_fields DO
        expr <- generate_identity_expr(identity)
        expressions.append(FORMAT("{expr} AS {target}",
            expr = expr,
            target = identity.target))
    END FOR

    // 4. Transformed fields with DQ rules applied
    FOR EACH mapping IN config.field_mappings DO
        expr <- generate_field_expr(mapping, config.dq_output)
        expressions.append(FORMAT("{expr} AS {target}",
            expr = expr,
            target = mapping.target_column))
    END FOR

    // 5. DQ flags array (if enabled)
    IF config.dq_output.enabled THEN
        dq_flags_expr <- generate_dq_flags_array(config.field_mappings, config.dq_output)
        expressions.append(FORMAT("{expr} AS {target}",
            expr = dq_flags_expr,
            target = config.dq_output.target_column))
    END IF

    RETURN "SELECT\n  " + JOIN(expressions, ",\n  ")
END

COMPLEXITY:
    Time: O(n * r) where n = mappings, r = avg rules per mapping
    Space: O(n) for expression storage
```

### 3.3 Transform Expression Generation

```
ALGORITHM: generate_transform_expr
INPUT: transform (TransformConfig), source_path (String), target_type (String)
OUTPUT: sql_expr (String)

BEGIN
    // Build source reference (JSON extraction from raw_payload)
    source_ref <- build_source_reference(source_path)

    MATCH transform WITH
        // ─────────────────────────────────────────────────────────
        // Type 1: Unit Conversion
        // ─────────────────────────────────────────────────────────
        UnitConversion { from, to, formula } =>
            MATCH formula WITH
                Linear { scale, offset } =>
                    // Generate: (source * scale + offset)
                    IF scale = 1.0 AND offset = 0.0 THEN
                        // No-op conversion
                        RETURN source_ref
                    ELSE IF scale = 1.0 THEN
                        RETURN FORMAT("({source} + {offset})",
                            source = source_ref,
                            offset = format_number(offset))
                    ELSE IF offset = 0.0 THEN
                        RETURN FORMAT("({source} * {scale})",
                            source = source_ref,
                            scale = format_number(scale))
                    ELSE
                        RETURN FORMAT("({source} * {scale} + {offset})",
                            source = source_ref,
                            scale = format_number(scale),
                            offset = format_number(offset))
                    END IF

                Custom { code } =>
                    // Substitute {value} placeholder in custom expression
                    RETURN REPLACE(code, "{value}", source_ref)
            END MATCH

        // ─────────────────────────────────────────────────────────
        // Type 2: Expression
        // ─────────────────────────────────────────────────────────
        Expression { expr } =>
            // Direct SQL expression with source substitution
            RETURN REPLACE(expr, "{value}", source_ref)

        // ─────────────────────────────────────────────────────────
        // Type 3: Lookup
        // ─────────────────────────────────────────────────────────
        Lookup { table } =>
            // Generate CASE expression for value mapping
            cases <- []
            FOR EACH (key, value) IN table DO
                cases.append(FORMAT("WHEN {source} = '{key}' THEN '{value}'",
                    source = source_ref,
                    key = escape_sql_string(key),
                    value = escape_sql_string(value)))
            END FOR
            RETURN FORMAT("CASE {cases} ELSE NULL END",
                cases = JOIN(cases, " "))

        // ─────────────────────────────────────────────────────────
        // Type 4: JSON Extract
        // ─────────────────────────────────────────────────────────
        JsonExtract { path } =>
            // Extract nested JSON value
            full_path <- concatenate_json_paths(source_path, path)
            RETURN build_source_reference(full_path)

        // ─────────────────────────────────────────────────────────
        // Type 5: Timestamp
        // ─────────────────────────────────────────────────────────
        Timestamp { format } =>
            MATCH format WITH
                MicrosecondsToTimestamp =>
                    RETURN FORMAT("to_timestamp({source} / 1000000.0)",
                        source = source_ref)
                UnixSeconds =>
                    RETURN FORMAT("to_timestamp({source})",
                        source = source_ref)
                Iso8601 =>
                    RETURN FORMAT("{source}::TIMESTAMPTZ",
                        source = source_ref)
            END MATCH

        // ─────────────────────────────────────────────────────────
        // Type 6: Computed
        // ─────────────────────────────────────────────────────────
        Computed { depends_on, expr } =>
            // Direct expression referencing other computed columns
            // Note: Computed columns must be ordered correctly in config
            RETURN expr

        // ─────────────────────────────────────────────────────────
        // No Transform (passthrough)
        // ─────────────────────────────────────────────────────────
        NULL =>
            RETURN source_ref
    END MATCH
END

SUBROUTINE: build_source_reference
INPUT: source_path (String)
OUTPUT: sql_expr (String)

BEGIN
    // Parse source path to determine extraction method
    IF source_path STARTS_WITH "raw_payload." THEN
        // JSON extraction from raw_payload column
        json_path <- SUBSTRING(source_path, LENGTH("raw_payload."))
        RETURN FORMAT("json_extract(raw_payload, '$.{path}')",
            path = json_path)
    ELSE IF source_path STARTS_WITH "context." THEN
        // JSON extraction from context column
        json_path <- SUBSTRING(source_path, LENGTH("context."))
        RETURN FORMAT("json_extract(context, '$.{path}')",
            path = json_path)
    ELSE
        // Direct column reference
        RETURN source_path
    END IF
END

SUBROUTINE: format_number
INPUT: value (Float)
OUTPUT: formatted (String)

BEGIN
    // Format with sufficient precision, avoiding scientific notation
    IF value = FLOOR(value) THEN
        RETURN FORMAT("{value}.0", value = INTEGER(value))
    ELSE
        RETURN FORMAT("{value}", value = ROUND(value, 10))
    END IF
END

COMPLEXITY:
    Time: O(k) where k = lookup table size (worst case)
    Space: O(1) for simple transforms, O(k) for lookups
```

### 3.4 DQ Rule CASE Expression Generation

```
ALGORITHM: generate_dq_case
INPUT: rules (Array<DqRule>), source_path (String), target_column (String), target_type (String)
OUTPUT: case_expr (String)

PRECONDITIONS:
    - rules is non-empty
    - source_path is valid JSON path

BEGIN
    source_ref <- build_source_reference(source_path)
    typed_ref <- apply_type_cast(source_ref, target_type)

    // Separate rules by action type for processing order
    reject_rules <- FILTER(rules, r => r.action = Reject)
    clamp_rules <- FILTER(rules, r => r.action = Clamp)
    flag_rules <- FILTER(rules, r => r.action = Flag)

    // Build the CASE expression based on action priority
    // Priority: Reject > Clamp > Flag > Pass

    case_parts <- []

    // Process REJECT rules first (set to NULL if triggered)
    FOR EACH rule IN reject_rules DO
        condition <- generate_violation_condition(rule, typed_ref)
        case_parts.append(FORMAT("WHEN {condition} THEN NULL",
            condition = condition))
    END FOR

    // Process CLAMP rules (adjust value to bounds)
    FOR EACH rule IN clamp_rules DO
        IF rule IS RangeCheck THEN
            // Generate clamping expression
            clamped_value <- generate_clamp_expr(rule, typed_ref)
            condition <- generate_violation_condition(rule, typed_ref)
            case_parts.append(FORMAT("WHEN {condition} THEN {clamped}",
                condition = condition,
                clamped = clamped_value))
        END IF
    END FOR

    // FLAG rules don't modify value, so they're handled in dq_flags generation
    // The value passes through unchanged

    // Default case: return original value (possibly transformed)
    case_parts.append(FORMAT("ELSE {value}",
        value = typed_ref))

    IF LENGTH(case_parts) = 1 THEN
        // No rules that modify value, just return typed reference
        RETURN typed_ref
    ELSE
        RETURN FORMAT("CASE\n    {parts}\n  END",
            parts = JOIN(case_parts, "\n    "))
    END IF
END

SUBROUTINE: generate_violation_condition
INPUT: rule (DqRule), source_ref (String)
OUTPUT: condition (String)

BEGIN
    MATCH rule WITH
        RangeCheck { min, max, ... } =>
            conditions <- []
            IF min IS NOT NULL THEN
                conditions.append(FORMAT("{source} < {min}",
                    source = source_ref,
                    min = format_number(min)))
            END IF
            IF max IS NOT NULL THEN
                conditions.append(FORMAT("{source} > {max}",
                    source = source_ref,
                    max = format_number(max)))
            END IF
            IF LENGTH(conditions) = 0 THEN
                RETURN "FALSE"  // No bounds specified
            ELSE
                RETURN JOIN(conditions, " OR ")
            END IF

        NullCheck { ... } =>
            RETURN FORMAT("{source} IS NULL", source = source_ref)

        EnumCheck { allowed_values, case_sensitive, ... } =>
            IF case_sensitive THEN
                values_list <- JOIN(MAP(allowed_values, v => "'" + escape_sql_string(v) + "'"), ", ")
                RETURN FORMAT("{source} NOT IN ({values})",
                    source = source_ref,
                    values = values_list)
            ELSE
                values_list <- JOIN(MAP(allowed_values, v => "'" + UPPER(escape_sql_string(v)) + "'"), ", ")
                RETURN FORMAT("UPPER({source}) NOT IN ({values})",
                    source = source_ref,
                    values = values_list)
            END IF

        PatternCheck { pattern, ... } =>
            RETURN FORMAT("{source} !~ '{pattern}'",
                source = source_ref,
                pattern = escape_sql_string(pattern))

        CrossFieldCheck { expression, ... } =>
            RETURN FORMAT("NOT ({expr})", expr = expression)

        Custom { expr, ... } =>
            RETURN FORMAT("NOT ({expr})", expr = expr)
    END MATCH
END

SUBROUTINE: generate_clamp_expr
INPUT: rule (RangeCheck), source_ref (String)
OUTPUT: clamped_expr (String)

BEGIN
    // Generate LEAST(GREATEST(value, min), max) pattern
    min_val <- rule.min
    max_val <- rule.max

    IF min_val IS NOT NULL AND max_val IS NOT NULL THEN
        RETURN FORMAT("LEAST(GREATEST({source}, {min}), {max})",
            source = source_ref,
            min = format_number(min_val),
            max = format_number(max_val))
    ELSE IF min_val IS NOT NULL THEN
        RETURN FORMAT("GREATEST({source}, {min})",
            source = source_ref,
            min = format_number(min_val))
    ELSE IF max_val IS NOT NULL THEN
        RETURN FORMAT("LEAST({source}, {max})",
            source = source_ref,
            max = format_number(max_val))
    ELSE
        RETURN source_ref  // No bounds to clamp
    END IF
END

COMPLEXITY:
    Time: O(r) where r = number of rules
    Space: O(r) for condition strings
```

### 3.5 DQ Flags Array Generation

```
ALGORITHM: generate_dq_flags_array
INPUT: mappings (Array<SilverFieldMapping>), dq_config (DqOutputConfig)
OUTPUT: array_expr (String)

PRECONDITIONS:
    - dq_config.enabled = true

BEGIN
    flag_expressions <- []

    FOR EACH mapping IN mappings DO
        source_ref <- build_source_reference(mapping.source_path)
        typed_ref <- apply_type_cast(source_ref, mapping.type)

        FOR EACH rule IN mapping.dq_rules DO
            // Skip DROP rules - they're handled in WHERE clause
            IF rule.action = Drop THEN
                CONTINUE
            END IF

            flag_expr <- generate_single_flag_expr(
                rule,
                typed_ref,
                mapping.target_column,
                dq_config.include_values
            )
            flag_expressions.append(flag_expr)
        END FOR
    END FOR

    IF LENGTH(flag_expressions) = 0 THEN
        // No DQ rules defined, return empty array
        RETURN "ARRAY[]::TEXT[]"
    END IF

    // Use array_filter to remove NULL entries
    RETURN FORMAT(
        "array_filter(ARRAY[\n      {expressions}\n    ], x -> x IS NOT NULL)",
        expressions = JOIN(flag_expressions, ",\n      ")
    )
END

SUBROUTINE: generate_single_flag_expr
INPUT: rule (DqRule), source_ref (String), column (String), include_values (Boolean)
OUTPUT: case_expr (String)

BEGIN
    condition <- generate_violation_condition(rule, source_ref)
    flag_string <- generate_flag_string(rule, column, include_values, source_ref)

    RETURN FORMAT(
        "CASE WHEN {condition} THEN '{flag}' END",
        condition = condition,
        flag = flag_string
    )
END

SUBROUTINE: generate_flag_string
INPUT: rule (DqRule), column (String), include_values (Boolean), source_ref (String)
OUTPUT: flag (String)

BEGIN
    rule_name <- get_rule_name(rule)

    MATCH rule WITH
        RangeCheck { min, max, action, ... } =>
            // Determine specific violation type
            IF action = Clamp THEN
                reason <- "clamped"
            ELSE IF min IS NOT NULL AND max IS NOT NULL THEN
                reason <- "out_of_bounds"
            ELSE IF min IS NOT NULL THEN
                reason <- "below_min"
            ELSE
                reason <- "above_max"
            END IF

            IF include_values AND action = Clamp THEN
                // Include original->clamped value
                // Note: This requires runtime evaluation, so we embed expression
                RETURN FORMAT("{rule}:{column}:{reason}:' || CAST({source} AS TEXT) || '->' || CAST({clamped} AS TEXT)",
                    rule = rule_name,
                    column = column,
                    reason = reason,
                    source = source_ref,
                    clamped = generate_clamp_expr(rule, source_ref))
            ELSE IF include_values THEN
                RETURN FORMAT("{rule}:{column}:{reason}:' || CAST({source} AS TEXT)",
                    rule = rule_name,
                    column = column,
                    reason = reason,
                    source = source_ref)
            ELSE
                RETURN FORMAT("{rule}:{column}:{reason}",
                    rule = rule_name,
                    column = column,
                    reason = reason)
            END IF

        NullCheck { ... } =>
            RETURN FORMAT("{rule}:{column}:missing",
                rule = rule_name,
                column = column)

        EnumCheck { ... } =>
            IF include_values THEN
                RETURN FORMAT("{rule}:{column}:invalid_value:' || COALESCE(CAST({source} AS TEXT), 'NULL')",
                    rule = rule_name,
                    column = column,
                    source = source_ref)
            ELSE
                RETURN FORMAT("{rule}:{column}:invalid_value",
                    rule = rule_name,
                    column = column)
            END IF

        PatternCheck { ... } =>
            RETURN FORMAT("{rule}:{column}:pattern_mismatch",
                rule = rule_name,
                column = column)

        CrossFieldCheck { name, message, ... } =>
            violation_msg <- message OR name
            RETURN FORMAT("{rule}:{msg}",
                rule = rule_name,
                msg = violation_msg)

        Custom { name, ... } =>
            RETURN FORMAT("custom:{name}:violated",
                name = name)
    END MATCH
END

SUBROUTINE: get_rule_name
INPUT: rule (DqRule)
OUTPUT: name (String)

BEGIN
    MATCH rule WITH
        RangeCheck => RETURN "range_check"
        NullCheck => RETURN "null_check"
        EnumCheck => RETURN "enum_check"
        PatternCheck => RETURN "pattern_check"
        CrossFieldCheck => RETURN "cross_field_check"
        Custom => RETURN "custom"
    END MATCH
END

COMPLEXITY:
    Time: O(m * r) where m = mappings, r = avg rules per mapping
    Space: O(m * r) for flag expressions
```

### 3.6 Complete Field Expression Generation

```
ALGORITHM: generate_field_expr
INPUT: mapping (SilverFieldMapping), dq_config (DqOutputConfig)
OUTPUT: expr (String)

BEGIN
    // Step 1: Get source reference
    source_ref <- build_source_reference(mapping.source_path)

    // Step 2: Apply transform if specified
    IF mapping.transform IS NOT NULL THEN
        transformed <- generate_transform_expr(
            mapping.transform,
            mapping.source_path,
            mapping.type
        )
    ELSE
        transformed <- source_ref
    END IF

    // Step 3: Apply type cast
    typed_expr <- apply_type_cast(transformed, mapping.type)

    // Step 4: Wrap with DQ rules if any exist
    IF LENGTH(mapping.dq_rules) > 0 THEN
        final_expr <- generate_dq_case(
            mapping.dq_rules,
            mapping.source_path,
            mapping.target_column,
            mapping.type
        )
        // Replace source reference in DQ case with transformed value
        final_expr <- REPLACE(final_expr, source_ref, typed_expr)
    ELSE
        final_expr <- typed_expr
    END IF

    // Step 5: Handle NULL for non-nullable fields
    IF NOT mapping.nullable THEN
        // Add COALESCE or error handling for non-nullable fields
        // Note: This is informational; actual enforcement is in Silver schema
        final_expr <- FORMAT("/* NOT NULL: {col} */ {expr}",
            col = mapping.target_column,
            expr = final_expr)
    END IF

    RETURN final_expr
END

SUBROUTINE: apply_type_cast
INPUT: expr (String), pg_type (String)
OUTPUT: casted (String)

BEGIN
    // Map PostgreSQL types to DuckDB cast expressions
    MATCH pg_type WITH
        "double_precision", "float8", "real", "float4" =>
            RETURN FORMAT("({expr})::DOUBLE", expr = expr)

        "integer", "int4" =>
            RETURN FORMAT("({expr})::INTEGER", expr = expr)

        "smallint", "int2" =>
            RETURN FORMAT("({expr})::SMALLINT", expr = expr)

        "bigint", "int8" =>
            RETURN FORMAT("({expr})::BIGINT", expr = expr)

        "text", "varchar" =>
            RETURN FORMAT("({expr})::TEXT", expr = expr)

        "boolean", "bool" =>
            RETURN FORMAT("({expr})::BOOLEAN", expr = expr)

        "timestamptz", "timestamp with time zone" =>
            RETURN FORMAT("({expr})::TIMESTAMPTZ", expr = expr)

        "timestamp", "timestamp without time zone" =>
            RETURN FORMAT("({expr})::TIMESTAMP", expr = expr)

        "jsonb", "json" =>
            RETURN FORMAT("({expr})::JSON", expr = expr)

        _ =>
            // Unknown type, return as-is with comment
            RETURN FORMAT("/* type: {type} */ ({expr})",
                type = pg_type,
                expr = expr)
    END MATCH
END

COMPLEXITY:
    Time: O(r) where r = number of DQ rules
    Space: O(1) for expression building
```

### 3.7 WHERE Clause Generation (Incremental + Drop Rules)

```
ALGORITHM: generate_where_clause
INPUT: config (SilverEtlConfig)
OUTPUT: where_sql (String)

BEGIN
    conditions <- []

    // 1. Incremental watermark condition
    IF config.incremental.enabled THEN
        watermark_condition <- generate_watermark_condition(
            config.timestamp,
            config.incremental,
            config.target_table
        )
        conditions.append(watermark_condition)
    END IF

    // 2. DROP action rules (exclude entire rows)
    FOR EACH mapping IN config.field_mappings DO
        FOR EACH rule IN mapping.dq_rules DO
            IF rule.action = Drop THEN
                source_ref <- build_source_reference(mapping.source_path)
                typed_ref <- apply_type_cast(source_ref, mapping.type)
                violation <- generate_violation_condition(rule, typed_ref)
                // Negate: keep rows where violation is FALSE
                conditions.append(FORMAT("NOT ({violation})", violation = violation))
            END IF
        END FOR
    END FOR

    IF LENGTH(conditions) = 0 THEN
        RETURN ""  // No WHERE clause needed
    ELSE
        RETURN "WHERE " + JOIN(conditions, "\n  AND ")
    END IF
END

SUBROUTINE: generate_watermark_condition
INPUT: ts (TimestampMapping), incr (IncrementalConfig), target_table (String)
OUTPUT: condition (String)

BEGIN
    // Generate timestamp transformation for source
    ts_expr <- generate_timestamp_expr(ts)

    // Subquery to get current watermark from Silver
    watermark_query <- FORMAT(
        "(SELECT COALESCE(MAX({col}), '1970-01-01'::TIMESTAMPTZ) FROM {table})",
        col = incr.watermark_column,
        table = target_table
    )

    // Parse lag interval
    lag <- parse_interval(incr.lag_interval)

    // Build condition: timestamp > watermark - lag AND timestamp <= now - lag
    RETURN FORMAT(
        "{ts_expr} > {watermark} - INTERVAL '{lag}'\n  AND {ts_expr} <= current_timestamp - INTERVAL '{lag}'",
        ts_expr = ts_expr,
        watermark = watermark_query,
        lag = lag
    )
END

SUBROUTINE: generate_timestamp_expr
INPUT: ts (TimestampMapping)
OUTPUT: expr (String)

BEGIN
    source <- ts.source_field

    MATCH ts.transform WITH
        MicrosecondsToTimestamp =>
            RETURN FORMAT("to_timestamp({source} / 1000000.0)", source = source)
        UnixSeconds =>
            RETURN FORMAT("to_timestamp({source})", source = source)
        Iso8601 =>
            RETURN FORMAT("{source}::TIMESTAMPTZ", source = source)
    END MATCH
END

COMPLEXITY:
    Time: O(m * r) for scanning DROP rules
    Space: O(d) where d = number of DROP rules
```

### 3.8 FROM Clause Generation

```
ALGORITHM: generate_from_clause
INPUT: parquet_path (String), incremental (IncrementalConfig)
OUTPUT: from_sql (String)

BEGIN
    // Build glob pattern for Parquet files
    glob_pattern <- FORMAT("{path}/**/*.parquet", path = parquet_path)

    // DuckDB read_parquet function
    RETURN FORMAT("FROM read_parquet('{pattern}')", pattern = glob_pattern)
END

COMPLEXITY:
    Time: O(1)
    Space: O(1)
```

### 3.9 ON CONFLICT Clause Generation

```
ALGORITHM: generate_conflict_clause
INPUT: dedup (DeduplicationConfig), columns (Array<String>)
OUTPUT: conflict_sql (String)

BEGIN
    IF NOT dedup.enabled THEN
        RETURN ""  // No deduplication
    END IF

    key_cols <- JOIN(dedup.key_columns, ", ")

    MATCH dedup.strategy WITH
        Upsert =>
            // Generate UPDATE SET for all non-key columns
            update_cols <- FILTER(columns, c => c NOT IN dedup.key_columns)
            set_clauses <- []
            FOR EACH col IN update_cols DO
                set_clauses.append(FORMAT("{col} = EXCLUDED.{col}", col = col))
            END FOR

            RETURN FORMAT(
                "ON CONFLICT ({keys}) DO UPDATE SET\n  {sets}",
                keys = key_cols,
                sets = JOIN(set_clauses, ",\n  ")
            )

        Skip =>
            RETURN FORMAT("ON CONFLICT ({keys}) DO NOTHING", keys = key_cols)

        Replace =>
            // Delete + Insert (requires different approach)
            // For DuckDB -> PostgreSQL, use upsert pattern
            // Note: True REPLACE requires DELETE first
            update_cols <- FILTER(columns, c => c NOT IN dedup.key_columns)
            set_clauses <- []
            FOR EACH col IN update_cols DO
                set_clauses.append(FORMAT("{col} = EXCLUDED.{col}", col = col))
            END FOR

            RETURN FORMAT(
                "ON CONFLICT ({keys}) DO UPDATE SET\n  {sets}",
                keys = key_cols,
                sets = JOIN(set_clauses, ",\n  ")
            )
    END MATCH
END

COMPLEXITY:
    Time: O(c) where c = number of columns
    Space: O(c) for SET clauses
```

---

## 4. Edge Case Handling

### 4.1 NULL Source Values

```
ALGORITHM: handle_null_source
INPUT: source_ref (String), mapping (SilverFieldMapping)
OUTPUT: safe_expr (String)

STRATEGY:
    - JSON extraction returns NULL for missing paths (safe)
    - Type casts on NULL return NULL (safe)
    - DQ rules handle NULL via null_check rule
    - Non-nullable fields: NULL values should trigger DQ violation

BEGIN
    // DuckDB json_extract returns NULL for missing paths
    // This is the desired behavior - no special handling needed

    // If null_check rule exists with Reject action, value becomes NULL anyway
    // If null_check rule exists with Flag action, NULL passes through with flag

    RETURN source_ref  // JSON extraction handles NULL gracefully
END
```

### 4.2 Type Mismatch Handling

```
ALGORITHM: handle_type_mismatch
INPUT: source_ref (String), expected_type (String)
OUTPUT: safe_expr (String)

STRATEGY:
    - Use TRY_CAST where available (DuckDB supports this)
    - Invalid casts become NULL rather than errors
    - DQ rules can flag NULL results from failed casts

BEGIN
    // For numeric types, use TRY_CAST for safe conversion
    IF expected_type IN ["double_precision", "integer", "smallint", "bigint"] THEN
        RETURN FORMAT("TRY_CAST({source} AS {type})",
            source = source_ref,
            type = map_to_duckdb_type(expected_type))
    ELSE
        // String/timestamp types: use regular cast (more permissive)
        RETURN FORMAT("({source})::{type}",
            source = source_ref,
            type = map_to_duckdb_type(expected_type))
    END IF
END
```

### 4.3 Empty Field Mappings

```
ALGORITHM: validate_config
INPUT: config (SilverEtlConfig)
OUTPUT: validation_result (Result<Unit, Error>)

BEGIN
    errors <- []

    IF NOT config.enabled THEN
        RETURN Error("Silver ETL is disabled for this stream")
    END IF

    IF config.target_table IS EMPTY THEN
        errors.append("target_table is required")
    END IF

    IF LENGTH(config.field_mappings) = 0 THEN
        errors.append("At least one field_mapping is required")
    END IF

    IF config.timestamp.source_field IS EMPTY THEN
        errors.append("timestamp.source_field is required")
    END IF

    // Validate each field mapping
    FOR i, mapping IN ENUMERATE(config.field_mappings) DO
        IF mapping.source_path IS EMPTY THEN
            errors.append(FORMAT("field_mappings[{i}].source_path is required", i = i))
        END IF
        IF mapping.target_column IS EMPTY THEN
            errors.append(FORMAT("field_mappings[{i}].target_column is required", i = i))
        END IF
        IF mapping.type IS EMPTY THEN
            errors.append(FORMAT("field_mappings[{i}].type is required", i = i))
        END IF
    END FOR

    // Validate deduplication key columns exist
    IF config.deduplication.enabled THEN
        all_columns <- get_all_target_columns(config)
        FOR EACH key IN config.deduplication.key_columns DO
            IF key NOT IN all_columns THEN
                errors.append(FORMAT("deduplication key '{key}' not in target columns", key = key))
            END IF
        END FOR
    END IF

    IF LENGTH(errors) > 0 THEN
        RETURN Error(JOIN(errors, "; "))
    ELSE
        RETURN Ok(())
    END IF
END
```

### 4.4 Circular Dependencies in Computed Fields

```
ALGORITHM: validate_computed_dependencies
INPUT: mappings (Array<SilverFieldMapping>)
OUTPUT: ordered_mappings (Array<SilverFieldMapping>)

BEGIN
    // Build dependency graph
    graph <- new DirectedGraph()

    FOR EACH mapping IN mappings DO
        graph.add_node(mapping.target_column)

        IF mapping.transform IS Computed THEN
            FOR EACH dep IN mapping.transform.depends_on DO
                graph.add_edge(dep, mapping.target_column)
            END FOR
        END IF
    END FOR

    // Check for cycles
    IF graph.has_cycle() THEN
        cycle <- graph.find_cycle()
        RAISE Error(FORMAT("Circular dependency detected: {cycle}",
            cycle = JOIN(cycle, " -> ")))
    END IF

    // Topological sort for correct ordering
    RETURN graph.topological_sort()
        .MAP(name => FIND(mappings, m => m.target_column = name))
END
```

---

## 5. Example Generated SQL

### 5.1 Air Quality Stream Example

**Input Config** (abbreviated):
```yaml
silver_etl:
  target_table: silver.air_quality_observations
  timestamp:
    source_field: timestamp
    transform: microseconds_to_timestamp
  field_mappings:
    - source_path: raw_payload.pm02
      target_column: pm25
      type: double_precision
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag
    - source_path: raw_payload.rhum
      target_column: humidity_pct
      type: double_precision
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 100.0
          action: clamp
  deduplication:
    enabled: true
    key_columns: [observation_time, ndp_id]
    strategy: upsert
  incremental:
    enabled: true
    lag_interval: 5 minutes
```

**Generated SQL**:
```sql
INSERT INTO silver.air_quality_observations (
  ingestion_time,
  observation_time,
  ndp_id,
  pm25,
  humidity_pct,
  dq_flags
)
SELECT
  current_timestamp AS ingestion_time,
  to_timestamp(timestamp / 1000000.0) AS observation_time,
  ndp_id,
  CASE
    WHEN (json_extract(raw_payload, '$.pm02'))::DOUBLE < 0.0
      OR (json_extract(raw_payload, '$.pm02'))::DOUBLE > 1000.0
    THEN (json_extract(raw_payload, '$.pm02'))::DOUBLE
    ELSE (json_extract(raw_payload, '$.pm02'))::DOUBLE
  END AS pm25,
  CASE
    WHEN (json_extract(raw_payload, '$.rhum'))::DOUBLE < 0.0
      OR (json_extract(raw_payload, '$.rhum'))::DOUBLE > 100.0
    THEN LEAST(GREATEST((json_extract(raw_payload, '$.rhum'))::DOUBLE, 0.0), 100.0)
    ELSE (json_extract(raw_payload, '$.rhum'))::DOUBLE
  END AS humidity_pct,
  array_filter(ARRAY[
    CASE WHEN (json_extract(raw_payload, '$.pm02'))::DOUBLE < 0.0
           OR (json_extract(raw_payload, '$.pm02'))::DOUBLE > 1000.0
         THEN 'range_check:pm25:out_of_bounds' END,
    CASE WHEN (json_extract(raw_payload, '$.rhum'))::DOUBLE < 0.0
           OR (json_extract(raw_payload, '$.rhum'))::DOUBLE > 100.0
         THEN 'range_check:humidity_pct:clamped' END
  ], x -> x IS NOT NULL) AS dq_flags
FROM read_parquet('/data/raw/air-quality/**/*.parquet')
WHERE to_timestamp(timestamp / 1000000.0) > (
    SELECT COALESCE(MAX(observation_time), '1970-01-01'::TIMESTAMPTZ)
    FROM silver.air_quality_observations
  ) - INTERVAL '5 minutes'
  AND to_timestamp(timestamp / 1000000.0) <= current_timestamp - INTERVAL '5 minutes'
ON CONFLICT (observation_time, ndp_id) DO UPDATE SET
  ingestion_time = EXCLUDED.ingestion_time,
  pm25 = EXCLUDED.pm25,
  humidity_pct = EXCLUDED.humidity_pct,
  dq_flags = EXCLUDED.dq_flags
```

### 5.2 Weather Stream with Unit Conversion

**Input Config** (abbreviated):
```yaml
silver_etl:
  target_table: silver.weather_observations
  field_mappings:
    - source_path: raw_payload.main.temp
      target_column: temperature_c
      type: double_precision
      transform:
        type: unit_conversion
        from: kelvin
        to: celsius
        formula:
          type: linear
          scale: 1.0
          offset: -273.15
      dq_rules:
        - rule: range_check
          min: -60.0
          max: 60.0
          action: flag
    - source_path: raw_payload.wind.speed
      target_column: wind_speed_kmh
      type: double_precision
      transform:
        type: unit_conversion
        from: m_s
        to: km_h
        formula:
          type: linear
          scale: 3.6
          offset: 0.0
```

**Generated SQL** (excerpt):
```sql
SELECT
  -- temperature_c with unit conversion and range check
  CASE
    WHEN ((json_extract(raw_payload, '$.main.temp'))::DOUBLE + -273.15) < -60.0
      OR ((json_extract(raw_payload, '$.main.temp'))::DOUBLE + -273.15) > 60.0
    THEN ((json_extract(raw_payload, '$.main.temp'))::DOUBLE + -273.15)
    ELSE ((json_extract(raw_payload, '$.main.temp'))::DOUBLE + -273.15)
  END AS temperature_c,

  -- wind_speed_kmh with unit conversion (m/s -> km/h)
  ((json_extract(raw_payload, '$.wind.speed'))::DOUBLE * 3.6) AS wind_speed_kmh
```

---

## 6. Complexity Analysis Summary

| Function | Time Complexity | Space Complexity | Notes |
|----------|-----------------|------------------|-------|
| `generate_etl_sql` | O(n * r) | O(n * r) | n=mappings, r=avg rules |
| `generate_select_clause` | O(n * r) | O(n) | Builds all expressions |
| `generate_transform_expr` | O(k) | O(k) | k=lookup table size |
| `generate_dq_case` | O(r) | O(r) | r=rules for field |
| `generate_dq_flags_array` | O(n * r) | O(n * r) | All flag expressions |
| `generate_where_clause` | O(n * r) | O(d) | d=DROP rules |
| `generate_conflict_clause` | O(c) | O(c) | c=columns |
| `validate_config` | O(n) | O(1) | Linear validation |
| `validate_computed_deps` | O(n + e) | O(n) | Graph traversal |

**Overall**: O(n * r) where n = field mappings, r = average rules per mapping

---

## 7. Implementation Notes

### 7.1 DuckDB-Specific Considerations

1. **JSON Functions**: Use `json_extract()` for JSON path extraction
2. **Type Casting**: Use `::TYPE` syntax for casts
3. **Array Functions**: Use `array_filter()` with lambda for NULL removal
4. **Timestamp Functions**: Use `to_timestamp()` for Unix epoch conversion
5. **Postgres Extension**: Requires `ATTACH` with postgres type for writes

### 7.2 SQL Injection Prevention

```
ALGORITHM: escape_sql_string
INPUT: value (String)
OUTPUT: escaped (String)

BEGIN
    // Double single quotes for SQL escaping
    escaped <- REPLACE(value, "'", "''")
    // Escape backslashes
    escaped <- REPLACE(escaped, "\\", "\\\\")
    RETURN escaped
END
```

### 7.3 Configuration Validation Order

1. Validate required fields exist
2. Validate type compatibility
3. Validate DQ rule parameters
4. Validate computed field dependencies (topological sort)
5. Validate deduplication key columns
6. Generate SQL only after validation passes

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP Pseudocode Specialist | Initial specification |

---

## References

1. ADR-006-001: ETL Engine Selection (duckdb-rs)
2. ADR-006-004: DQ Rule Actions
3. CONFIG_DRIVEN_SILVER_ETL_DESIGN.md: Config schema design
4. DQ-FRAMEWORK-DESIGN.md: Complete DQ framework
5. SPECIFICATION.md: Functional requirements FR-003, FR-005, FR-006
