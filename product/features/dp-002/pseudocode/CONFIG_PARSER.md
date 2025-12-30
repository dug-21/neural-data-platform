# CONFIG_PARSER.md - Entity Schema Configuration Parsing

## Overview

This document defines the pseudocode for parsing and validating `entity_schemas` sections in stream configuration YAML files. The parser ensures configuration correctness before sync to the data dictionary, providing clear error messages with line numbers when validation fails.

---

## Configuration Schema

### Expected YAML Structure

```yaml
# config.yaml
stream_id: "homeassistant"
description: "HomeAssistant event stream"
version: "1.0.0"
enabled: true

# Existing fields section (for ingestion - unchanged)
fields:
  - name: entity_id
    type: string
    nullable: false
  - name: state
    type: string
  - name: attributes
    type: json

# NEW: Entity schemas section (for data dictionary)
entity_schemas:
  - schema_name: "sensor.airgradient_*"
    description: "AirGradient indoor air quality sensors"
    device_class: "air_quality"
    pattern: "sensor.airgradient_*"
    attributes:
      - name: "pm25"
        type: "float"
        unit: "ug/m3"
        description: "Particulate Matter 2.5 micrometers"
        nullable: false
      - name: "co2"
        type: "int"
        unit: "ppm"
        description: "Carbon Dioxide concentration"
      - name: "temperature"
        type: "float"
        unit: "celsius"
```

### Type Definitions

```
TYPE: EntitySchema
FIELDS:
  - schema_name: string (REQUIRED)
  - description: string (REQUIRED)
  - device_class: string (OPTIONAL)
  - pattern: string (OPTIONAL, defaults to schema_name)
  - attributes: List<Attribute> (REQUIRED, min 1)

TYPE: Attribute
FIELDS:
  - name: string (REQUIRED)
  - type: TypeEnum (REQUIRED)
  - unit: string (OPTIONAL)
  - description: string (OPTIONAL)
  - nullable: boolean (OPTIONAL, default true)
  - range: {min, max} (OPTIONAL, for numeric types)
  - enum_values: List<string> (OPTIONAL, for string type)

ENUM: TypeEnum
VALUES: ["string", "int", "float", "boolean", "json", "datetime", "array"]
```

---

## Data Structures

### Parse Result

```
STRUCTURE: ParseResult
FIELDS:
  - success: boolean
  - config: StreamConfig (if success)
  - entity_schemas: List<EntitySchema> (if success)
  - errors: List<ParseError>
  - warnings: List<ParseWarning>
```

### Parse Error

```
STRUCTURE: ParseError
FIELDS:
  - code: ErrorCode enum
  - message: string
  - line: integer (or NULL)
  - column: integer (or NULL)
  - path: string (JSON path to error location)
  - severity: "error" | "warning"
  - context: string (surrounding YAML)
  - suggestion: string (how to fix)
```

### Error Codes

```
ENUM: ErrorCode
VALUES:
  // Structure errors
  - YAML_SYNTAX_ERROR
  - MISSING_REQUIRED_FIELD
  - INVALID_TYPE
  - EMPTY_VALUE

  // Schema validation errors
  - INVALID_SCHEMA_NAME
  - DUPLICATE_SCHEMA_NAME
  - EMPTY_ATTRIBUTES_LIST
  - DUPLICATE_ATTRIBUTE_NAME

  // Attribute validation errors
  - INVALID_ATTRIBUTE_TYPE
  - INVALID_UNIT_FORMAT
  - INVALID_RANGE_VALUES
  - CONFLICTING_NULLABLE

  // Pattern validation errors
  - INVALID_PATTERN_SYNTAX
  - PATTERN_SCHEMA_MISMATCH

  // Compatibility warnings
  - FIELD_SCHEMA_OVERLAP
  - DEPRECATED_TYPE_NAME
```

---

## Algorithm 1: Parse Configuration YAML

```
ALGORITHM: ParseStreamConfig
PURPOSE: Parse and validate complete stream configuration file

INPUT:
  - yaml_content: string (raw YAML text)
  - file_path: string (for error reporting)

OUTPUT:
  - result: ParseResult

BEGIN
    result ← {
        success: false,
        config: NULL,
        entity_schemas: [],
        errors: [],
        warnings: []
    }

    // Step 1: Parse raw YAML
    TRY
        parsed ← YAMLParser.parse(yaml_content)
    CATCH YAMLSyntaxError AS e
        result.errors.append({
            code: YAML_SYNTAX_ERROR,
            message: e.message,
            line: e.line,
            column: e.column,
            path: "",
            severity: "error",
            context: ExtractContextLines(yaml_content, e.line, 2),
            suggestion: "Check YAML syntax - ensure proper indentation and quoting"
        })
        RETURN result
    END TRY

    // Step 2: Validate top-level structure
    structure_errors ← ValidateTopLevelStructure(parsed, file_path)
    result.errors ← result.errors + structure_errors

    IF structure_errors HAS severity "error" THEN
        RETURN result
    END IF

    // Step 3: Extract and validate entity_schemas
    IF "entity_schemas" IN parsed THEN
        (schemas, schema_errors, schema_warnings) ← ParseEntitySchemas(
            parsed.entity_schemas,
            file_path,
            GetYAMLLineMap(yaml_content)
        )
        result.entity_schemas ← schemas
        result.errors ← result.errors + schema_errors
        result.warnings ← result.warnings + schema_warnings
    ELSE
        result.warnings.append({
            code: MISSING_REQUIRED_FIELD,
            message: "No entity_schemas section found - stream will have no data dictionary entries",
            path: "entity_schemas",
            severity: "warning",
            suggestion: "Add entity_schemas section to enable data dictionary"
        })
    END IF

    // Step 4: Check for field/schema overlap
    IF "fields" IN parsed AND "entity_schemas" IN parsed THEN
        overlap_warnings ← CheckFieldSchemaOverlap(
            parsed.fields,
            parsed.entity_schemas
        )
        result.warnings ← result.warnings + overlap_warnings
    END IF

    // Step 5: Build final config object
    IF LENGTH(result.errors) = 0 OR
       ALL(e.severity = "warning" FOR e IN result.errors) THEN
        result.success ← true
        result.config ← BuildStreamConfig(parsed)
    END IF

    RETURN result
END


SUBROUTINE: ValidateTopLevelStructure
INPUT: parsed (object), file_path (string)
OUTPUT: List<ParseError>

BEGIN
    errors ← []

    // Required fields
    required ← ["stream_id", "description", "version"]

    FOR EACH field IN required DO
        IF field NOT IN parsed THEN
            errors.append({
                code: MISSING_REQUIRED_FIELD,
                message: "Missing required field: " + field,
                path: field,
                severity: "error",
                suggestion: "Add '" + field + ": <value>' to configuration"
            })
        END IF
    END FOR

    // Type validation
    IF "stream_id" IN parsed THEN
        IF NOT IS_STRING(parsed.stream_id) THEN
            errors.append({
                code: INVALID_TYPE,
                message: "stream_id must be a string",
                path: "stream_id",
                severity: "error"
            })
        ELSE IF NOT MATCHES(parsed.stream_id, "^[a-z][a-z0-9-]*$") THEN
            errors.append({
                code: INVALID_SCHEMA_NAME,
                message: "stream_id must be lowercase alphanumeric with hyphens",
                path: "stream_id",
                severity: "error",
                suggestion: "Use format like 'my-stream-name'"
            })
        END IF
    END IF

    IF "enabled" IN parsed AND NOT IS_BOOLEAN(parsed.enabled) THEN
        errors.append({
            code: INVALID_TYPE,
            message: "enabled must be a boolean (true/false)",
            path: "enabled",
            severity: "error"
        })
    END IF

    RETURN errors
END
```

---

## Algorithm 2: Parse Entity Schemas

```
ALGORITHM: ParseEntitySchemas
PURPOSE: Parse and validate the entity_schemas section

INPUT:
  - schemas_yaml: List of schema objects from YAML
  - file_path: string
  - line_map: Map<path, line_number>

OUTPUT:
  - schemas: List<EntitySchema>
  - errors: List<ParseError>
  - warnings: List<ParseWarning>

BEGIN
    schemas ← []
    errors ← []
    warnings ← []
    seen_names ← SET()

    // Validate schemas is a list
    IF NOT IS_LIST(schemas_yaml) THEN
        errors.append({
            code: INVALID_TYPE,
            message: "entity_schemas must be a list",
            path: "entity_schemas",
            severity: "error",
            suggestion: "Use YAML list format:\nentity_schemas:\n  - schema_name: ..."
        })
        RETURN ([], errors, warnings)
    END IF

    // Validate empty list
    IF LENGTH(schemas_yaml) = 0 THEN
        warnings.append({
            code: EMPTY_VALUE,
            message: "entity_schemas list is empty",
            path: "entity_schemas",
            severity: "warning",
            suggestion: "Add at least one schema definition"
        })
        RETURN ([], errors, warnings)
    END IF

    // Process each schema
    FOR index FROM 0 TO LENGTH(schemas_yaml) - 1 DO
        schema_yaml ← schemas_yaml[index]
        path_prefix ← "entity_schemas[" + index + "]"
        line ← GetLineNumber(line_map, path_prefix)

        (schema, schema_errors, schema_warnings) ← ParseSingleSchema(
            schema_yaml,
            path_prefix,
            line
        )

        // Check for duplicate schema names
        IF schema IS NOT NULL THEN
            IF schema.schema_name IN seen_names THEN
                errors.append({
                    code: DUPLICATE_SCHEMA_NAME,
                    message: "Duplicate schema_name: " + schema.schema_name,
                    path: path_prefix + ".schema_name",
                    line: line,
                    severity: "error",
                    suggestion: "Each schema_name must be unique within a stream"
                })
            ELSE
                seen_names.add(schema.schema_name)
                schemas.append(schema)
            END IF
        END IF

        errors ← errors + schema_errors
        warnings ← warnings + schema_warnings
    END FOR

    RETURN (schemas, errors, warnings)
END


SUBROUTINE: ParseSingleSchema
INPUT: schema_yaml (object), path_prefix (string), line (integer)
OUTPUT: (EntitySchema or NULL, List<ParseError>, List<ParseWarning>)

BEGIN
    errors ← []
    warnings ← []
    schema ← NULL

    // Validate schema_name (required)
    IF "schema_name" NOT IN schema_yaml THEN
        errors.append({
            code: MISSING_REQUIRED_FIELD,
            message: "Missing required field: schema_name",
            path: path_prefix + ".schema_name",
            line: line,
            severity: "error"
        })
    ELSE IF NOT IS_STRING(schema_yaml.schema_name) THEN
        errors.append({
            code: INVALID_TYPE,
            message: "schema_name must be a string",
            path: path_prefix + ".schema_name",
            line: line,
            severity: "error"
        })
    ELSE IF TRIM(schema_yaml.schema_name) = "" THEN
        errors.append({
            code: EMPTY_VALUE,
            message: "schema_name cannot be empty",
            path: path_prefix + ".schema_name",
            line: line,
            severity: "error"
        })
    END IF

    // Validate description (required)
    IF "description" NOT IN schema_yaml THEN
        errors.append({
            code: MISSING_REQUIRED_FIELD,
            message: "Missing required field: description",
            path: path_prefix + ".description",
            line: line,
            severity: "error"
        })
    END IF

    // Validate pattern (optional, defaults to schema_name)
    pattern ← schema_yaml.pattern OR schema_yaml.schema_name
    IF pattern IS NOT NULL THEN
        pattern_result ← ValidatePattern(pattern)
        IF NOT pattern_result.valid THEN
            errors ← errors + MapErrors(pattern_result.errors, path_prefix + ".pattern", line)
        END IF
        warnings ← warnings + MapWarnings(pattern_result.warnings, path_prefix + ".pattern")
    END IF

    // Validate attributes (required, must be non-empty list)
    IF "attributes" NOT IN schema_yaml THEN
        errors.append({
            code: MISSING_REQUIRED_FIELD,
            message: "Missing required field: attributes",
            path: path_prefix + ".attributes",
            line: line,
            severity: "error",
            suggestion: "Add attributes list with at least one attribute"
        })
    ELSE
        (attrs, attr_errors, attr_warnings) ← ParseAttributes(
            schema_yaml.attributes,
            path_prefix + ".attributes",
            line
        )
        errors ← errors + attr_errors
        warnings ← warnings + attr_warnings

        IF LENGTH(errors) = 0 THEN
            schema ← {
                schema_name: schema_yaml.schema_name,
                description: schema_yaml.description OR "",
                device_class: schema_yaml.device_class OR NULL,
                pattern: pattern,
                attributes: attrs
            }
        END IF
    END IF

    // Validate optional device_class
    IF "device_class" IN schema_yaml THEN
        IF NOT IS_STRING(schema_yaml.device_class) THEN
            errors.append({
                code: INVALID_TYPE,
                message: "device_class must be a string",
                path: path_prefix + ".device_class",
                severity: "error"
            })
        ELSE
            // Warn if unknown device class
            known_classes ← [
                "air_quality", "battery", "carbon_dioxide", "carbon_monoxide",
                "door", "energy", "gas", "humidity", "illuminance", "moisture",
                "motion", "occupancy", "power", "pressure", "temperature",
                "timestamp", "voltage", "window"
            ]
            IF schema_yaml.device_class NOT IN known_classes THEN
                warnings.append({
                    code: DEPRECATED_TYPE_NAME,
                    message: "Unknown device_class: " + schema_yaml.device_class,
                    path: path_prefix + ".device_class",
                    severity: "warning",
                    suggestion: "Known classes: " + JOIN(known_classes, ", ")
                })
            END IF
        END IF
    END IF

    RETURN (schema, errors, warnings)
END
```

---

## Algorithm 3: Parse Attributes

```
ALGORITHM: ParseAttributes
PURPOSE: Parse and validate the attributes list within a schema

INPUT:
  - attrs_yaml: List of attribute objects
  - path_prefix: string
  - base_line: integer

OUTPUT:
  - attributes: List<Attribute>
  - errors: List<ParseError>
  - warnings: List<ParseWarning>

BEGIN
    attributes ← []
    errors ← []
    warnings ← []
    seen_names ← SET()

    // Validate is list
    IF NOT IS_LIST(attrs_yaml) THEN
        errors.append({
            code: INVALID_TYPE,
            message: "attributes must be a list",
            path: path_prefix,
            severity: "error"
        })
        RETURN ([], errors, warnings)
    END IF

    // Validate non-empty
    IF LENGTH(attrs_yaml) = 0 THEN
        errors.append({
            code: EMPTY_ATTRIBUTES_LIST,
            message: "attributes list cannot be empty",
            path: path_prefix,
            severity: "error",
            suggestion: "Add at least one attribute definition"
        })
        RETURN ([], errors, warnings)
    END IF

    // Parse each attribute
    FOR index FROM 0 TO LENGTH(attrs_yaml) - 1 DO
        attr_yaml ← attrs_yaml[index]
        attr_path ← path_prefix + "[" + index + "]"

        (attr, attr_errors, attr_warnings) ← ParseSingleAttribute(
            attr_yaml,
            attr_path
        )

        // Check for duplicate names
        IF attr IS NOT NULL THEN
            IF attr.name IN seen_names THEN
                errors.append({
                    code: DUPLICATE_ATTRIBUTE_NAME,
                    message: "Duplicate attribute name: " + attr.name,
                    path: attr_path + ".name",
                    severity: "error"
                })
            ELSE
                seen_names.add(attr.name)
                attributes.append(attr)
            END IF
        END IF

        errors ← errors + attr_errors
        warnings ← warnings + attr_warnings
    END FOR

    RETURN (attributes, errors, warnings)
END


SUBROUTINE: ParseSingleAttribute
INPUT: attr_yaml (object), path (string)
OUTPUT: (Attribute or NULL, List<ParseError>, List<ParseWarning>)

BEGIN
    errors ← []
    warnings ← []
    attr ← NULL

    // Validate name (required)
    IF "name" NOT IN attr_yaml THEN
        errors.append({
            code: MISSING_REQUIRED_FIELD,
            message: "Missing required field: name",
            path: path + ".name",
            severity: "error"
        })
        RETURN (NULL, errors, warnings)
    ELSE IF NOT IS_STRING(attr_yaml.name) THEN
        errors.append({
            code: INVALID_TYPE,
            message: "name must be a string",
            path: path + ".name",
            severity: "error"
        })
        RETURN (NULL, errors, warnings)
    ELSE IF NOT MATCHES(attr_yaml.name, "^[a-z_][a-z0-9_]*$") THEN
        errors.append({
            code: INVALID_SCHEMA_NAME,
            message: "Attribute name must be snake_case",
            path: path + ".name",
            severity: "error",
            suggestion: "Use lowercase with underscores: '" +
                        ToSnakeCase(attr_yaml.name) + "'"
        })
    END IF

    // Validate type (required)
    valid_types ← ["string", "int", "float", "boolean", "json", "datetime", "array"]
    type_aliases ← {
        "integer": "int",
        "number": "float",
        "double": "float",
        "bool": "boolean",
        "object": "json",
        "timestamp": "datetime"
    }

    IF "type" NOT IN attr_yaml THEN
        errors.append({
            code: MISSING_REQUIRED_FIELD,
            message: "Missing required field: type",
            path: path + ".type",
            severity: "error",
            suggestion: "Valid types: " + JOIN(valid_types, ", ")
        })
        RETURN (NULL, errors, warnings)
    END IF

    attr_type ← LOWERCASE(attr_yaml.type)

    // Handle type aliases
    IF attr_type IN type_aliases THEN
        original_type ← attr_type
        attr_type ← type_aliases[attr_type]
        warnings.append({
            code: DEPRECATED_TYPE_NAME,
            message: "Type '" + original_type + "' is an alias for '" + attr_type + "'",
            path: path + ".type",
            severity: "warning",
            suggestion: "Consider using '" + attr_type + "' instead"
        })
    END IF

    IF attr_type NOT IN valid_types THEN
        errors.append({
            code: INVALID_ATTRIBUTE_TYPE,
            message: "Invalid type: " + attr_yaml.type,
            path: path + ".type",
            severity: "error",
            suggestion: "Valid types: " + JOIN(valid_types, ", ")
        })
        RETURN (NULL, errors, warnings)
    END IF

    // Validate unit (optional)
    unit ← NULL
    IF "unit" IN attr_yaml THEN
        IF NOT IS_STRING(attr_yaml.unit) THEN
            errors.append({
                code: INVALID_TYPE,
                message: "unit must be a string",
                path: path + ".unit",
                severity: "error"
            })
        ELSE
            unit ← attr_yaml.unit
            // Warn about unit format
            IF NOT MATCHES(unit, "^[A-Za-z0-9/%°µ]+$") THEN
                warnings.append({
                    code: INVALID_UNIT_FORMAT,
                    message: "Unusual unit format: " + unit,
                    path: path + ".unit",
                    severity: "warning"
                })
            END IF
        END IF
    END IF

    // Validate nullable (optional, default true)
    nullable ← true
    IF "nullable" IN attr_yaml THEN
        IF NOT IS_BOOLEAN(attr_yaml.nullable) THEN
            errors.append({
                code: INVALID_TYPE,
                message: "nullable must be a boolean",
                path: path + ".nullable",
                severity: "error"
            })
        ELSE
            nullable ← attr_yaml.nullable
        END IF
    END IF

    // Validate range (optional, only for numeric types)
    range ← NULL
    IF "range" IN attr_yaml THEN
        IF attr_type NOT IN ["int", "float"] THEN
            errors.append({
                code: CONFLICTING_NULLABLE,
                message: "range is only valid for numeric types (int, float)",
                path: path + ".range",
                severity: "error"
            })
        ELSE
            (range, range_errors) ← ValidateRange(attr_yaml.range, path + ".range")
            errors ← errors + range_errors
        END IF
    END IF

    // Build attribute if no errors
    IF LENGTH(errors) = 0 THEN
        attr ← {
            name: attr_yaml.name,
            type: attr_type,
            unit: unit,
            description: attr_yaml.description OR "",
            nullable: nullable,
            range: range
        }
    END IF

    RETURN (attr, errors, warnings)
END


SUBROUTINE: ValidateRange
INPUT: range_yaml, path
OUTPUT: ({min, max} or NULL, List<ParseError>)

BEGIN
    errors ← []

    IF NOT IS_OBJECT(range_yaml) THEN
        errors.append({
            code: INVALID_TYPE,
            message: "range must be an object with min/max",
            path: path,
            severity: "error",
            suggestion: "Use format: range: {min: 0, max: 100}"
        })
        RETURN (NULL, errors)
    END IF

    min_val ← range_yaml.min OR NULL
    max_val ← range_yaml.max OR NULL

    IF min_val IS NOT NULL AND NOT IS_NUMBER(min_val) THEN
        errors.append({
            code: INVALID_TYPE,
            message: "range.min must be a number",
            path: path + ".min",
            severity: "error"
        })
    END IF

    IF max_val IS NOT NULL AND NOT IS_NUMBER(max_val) THEN
        errors.append({
            code: INVALID_TYPE,
            message: "range.max must be a number",
            path: path + ".max",
            severity: "error"
        })
    END IF

    IF min_val IS NOT NULL AND max_val IS NOT NULL AND min_val > max_val THEN
        errors.append({
            code: INVALID_RANGE_VALUES,
            message: "range.min cannot be greater than range.max",
            path: path,
            severity: "error"
        })
    END IF

    IF LENGTH(errors) > 0 THEN
        RETURN (NULL, errors)
    END IF

    RETURN ({min: min_val, max: max_val}, errors)
END
```

---

## Algorithm 4: Field/Schema Overlap Check

```
ALGORITHM: CheckFieldSchemaOverlap
PURPOSE: Warn about potential inconsistencies between fields and entity_schemas

INPUT:
  - fields: List of field definitions (existing ingestion schema)
  - entity_schemas: List of EntitySchema

OUTPUT:
  - warnings: List<ParseWarning>

BEGIN
    warnings ← []

    // Build set of field names from ingestion schema
    field_names ← SET(f.name FOR f IN fields)

    // Check each schema's attributes against fields
    FOR EACH schema IN entity_schemas DO
        schema_attr_names ← SET(a.name FOR a IN schema.attributes)

        // Find attributes that exist in both but might have different types
        overlap ← field_names INTERSECTION schema_attr_names

        IF LENGTH(overlap) > 0 THEN
            FOR EACH attr_name IN overlap DO
                field ← FIND(f IN fields WHERE f.name = attr_name)
                attr ← FIND(a IN schema.attributes WHERE a.name = attr_name)

                // Check for type mismatches
                IF NormalizeType(field.type) != NormalizeType(attr.type) THEN
                    warnings.append({
                        code: FIELD_SCHEMA_OVERLAP,
                        message: "Type mismatch for '" + attr_name + "': " +
                                 "fields=" + field.type + ", entity_schema=" + attr.type,
                        path: "entity_schemas." + schema.schema_name + ".attributes." + attr_name,
                        severity: "warning",
                        suggestion: "Consider aligning types between fields and entity_schemas"
                    })
                END IF

                // Check for unit mismatches
                IF field.unit != attr.unit AND field.unit IS NOT NULL AND attr.unit IS NOT NULL THEN
                    warnings.append({
                        code: FIELD_SCHEMA_OVERLAP,
                        message: "Unit mismatch for '" + attr_name + "': " +
                                 "fields=" + field.unit + ", entity_schema=" + attr.unit,
                        path: "entity_schemas." + schema.schema_name + ".attributes." + attr_name,
                        severity: "warning"
                    })
                END IF
            END FOR
        END IF
    END FOR

    RETURN warnings
END


SUBROUTINE: NormalizeType
INPUT: type_str (string)
OUTPUT: normalized_type (string)

BEGIN
    type_map ← {
        "integer": "int",
        "number": "float",
        "double": "float",
        "real": "float",
        "bool": "boolean",
        "object": "json",
        "timestamp": "datetime",
        "time": "datetime"
    }

    lower ← LOWERCASE(type_str)

    IF lower IN type_map THEN
        RETURN type_map[lower]
    ELSE
        RETURN lower
    END IF
END
```

---

## Algorithm 5: Error Formatting

```
ALGORITHM: FormatParseErrors
PURPOSE: Format parse errors for human-readable output

INPUT:
  - result: ParseResult
  - yaml_content: string (original YAML)
  - format: "text" | "json" | "markdown"

OUTPUT:
  - formatted: string

BEGIN
    SWITCH format
        CASE "text":
            RETURN FormatAsText(result, yaml_content)

        CASE "json":
            RETURN FormatAsJSON(result)

        CASE "markdown":
            RETURN FormatAsMarkdown(result, yaml_content)
    END SWITCH
END


SUBROUTINE: FormatAsText
INPUT: result, yaml_content
OUTPUT: formatted string

BEGIN
    output ← ""

    IF result.success THEN
        output ← "Configuration parsed successfully.\n"
        output ← output + "  Schemas: " + LENGTH(result.entity_schemas) + "\n"

        total_attrs ← SUM(LENGTH(s.attributes) FOR s IN result.entity_schemas)
        output ← output + "  Attributes: " + total_attrs + "\n"
    ELSE
        output ← "Configuration parsing failed.\n"
    END IF

    // Format errors
    IF LENGTH(result.errors) > 0 THEN
        output ← output + "\nErrors (" + LENGTH(result.errors) + "):\n"

        FOR EACH error IN result.errors DO
            output ← output + FormatSingleError(error, yaml_content) + "\n"
        END FOR
    END IF

    // Format warnings
    IF LENGTH(result.warnings) > 0 THEN
        output ← output + "\nWarnings (" + LENGTH(result.warnings) + "):\n"

        FOR EACH warning IN result.warnings DO
            output ← output + FormatSingleError(warning, yaml_content) + "\n"
        END FOR
    END IF

    RETURN output
END


SUBROUTINE: FormatSingleError
INPUT: error, yaml_content
OUTPUT: formatted string

BEGIN
    output ← ""

    // Error header
    IF error.line IS NOT NULL THEN
        output ← "  Line " + error.line + ": "
    ELSE
        output ← "  "
    END IF

    output ← output + "[" + error.code + "] " + error.message + "\n"

    // Path
    IF error.path IS NOT NULL AND error.path != "" THEN
        output ← output + "    Path: " + error.path + "\n"
    END IF

    // Context (show surrounding lines)
    IF error.line IS NOT NULL THEN
        lines ← SPLIT(yaml_content, "\n")
        start ← MAX(0, error.line - 2)
        end ← MIN(LENGTH(lines), error.line + 2)

        output ← output + "    Context:\n"
        FOR i FROM start TO end - 1 DO
            prefix ← "      "
            IF i + 1 = error.line THEN
                prefix ← "  >>> "
            END IF
            output ← output + prefix + (i + 1) + ": " + lines[i] + "\n"
        END FOR
    END IF

    // Suggestion
    IF error.suggestion IS NOT NULL THEN
        output ← output + "    Suggestion: " + error.suggestion + "\n"
    END IF

    RETURN output
END
```

---

## Complexity Analysis

### ParseStreamConfig

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| YAML parse | O(n) | O(n) |
| Structure validation | O(1) | O(1) |
| Schema parsing | O(s * a) | O(s * a) |
| Overlap check | O(f * s * a) | O(f + a) |
| **Total** | **O(n + s*a + f*s*a)** | **O(n + s*a)** |

Where:
- n = YAML content size
- s = number of schemas
- a = average attributes per schema
- f = number of fields

### Error Formatting

| Format | Time Complexity | Space Complexity |
|--------|-----------------|------------------|
| Text | O(e * c) | O(e * c) |
| JSON | O(e) | O(e) |
| Markdown | O(e * c) | O(e * c) |

Where:
- e = number of errors
- c = context lines per error

---

## Worked Example

### Input Configuration

```yaml
stream_id: "homeassistant"
description: "HomeAssistant event stream"
version: "1.0.0"

entity_schemas:
  - schema_name: "sensor.airgradient_*"
    description: "AirGradient sensors"
    device_class: "air_quality"
    attributes:
      - name: "pm25"
        type: "float"
        unit: "ug/m3"
      - name: "PM10"           # Error: should be snake_case
        type: "integer"        # Warning: alias for 'int'
      - name: "co2"
        type: "string"         # Possible mismatch
        nullable: "yes"        # Error: should be boolean

  - schema_name: "sensor.airgradient_*"  # Error: duplicate name
    description: "Duplicate"
    attributes: []               # Error: empty attributes list
```

### Expected Parse Result

```
Configuration parsing failed.

Errors (4):
  Line 12: [INVALID_SCHEMA_NAME] Attribute name must be snake_case
    Path: entity_schemas[0].attributes[1].name
    Context:
      10:       - name: "pm25"
      11:         type: "float"
  >>> 12:       - name: "PM10"
      13:         type: "integer"
    Suggestion: Use lowercase with underscores: 'pm10'

  Line 16: [INVALID_TYPE] nullable must be a boolean
    Path: entity_schemas[0].attributes[2].nullable
    Context:
      14:       - name: "co2"
      15:         type: "string"
  >>> 16:         nullable: "yes"
    Suggestion: Use true or false

  Line 18: [DUPLICATE_SCHEMA_NAME] Duplicate schema_name: sensor.airgradient_*
    Path: entity_schemas[1].schema_name

  Line 20: [EMPTY_ATTRIBUTES_LIST] attributes list cannot be empty
    Path: entity_schemas[1].attributes
    Suggestion: Add at least one attribute definition

Warnings (1):
  Line 13: [DEPRECATED_TYPE_NAME] Type 'integer' is an alias for 'int'
    Path: entity_schemas[0].attributes[1].type
    Suggestion: Consider using 'int' instead
```

---

## Integration with Sync

### Pre-Sync Validation

```
ALGORITHM: ValidateBeforeSync
PURPOSE: Validate all stream configs before syncing to data dictionary

INPUT:
  - config_paths: List of YAML file paths

OUTPUT:
  - validation_report: {
      valid_configs: List<ParseResult>,
      invalid_configs: List<ParseResult>,
      can_proceed: boolean
    }

BEGIN
    valid ← []
    invalid ← []

    FOR EACH path IN config_paths DO
        content ← ReadFile(path)
        result ← ParseStreamConfig(content, path)

        IF result.success THEN
            valid.append({path: path, result: result})
        ELSE
            invalid.append({path: path, result: result})
        END IF
    END FOR

    can_proceed ← LENGTH(invalid) = 0 OR
                  ALL(e.severity = "warning" FOR config IN invalid FOR e IN config.result.errors)

    RETURN {
        valid_configs: valid,
        invalid_configs: invalid,
        can_proceed: can_proceed
    }
END
```
