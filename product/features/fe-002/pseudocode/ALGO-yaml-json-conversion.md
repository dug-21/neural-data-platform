# ALGO-001: YAML to JSON Domain Configuration Conversion

## Overview

Algorithm for converting `domain.yaml` to `domain.json` while preserving semantic equivalence and ensuring no behavioral change in downstream tooling.

**Feature:** FE-002 Domain Configuration Standardization
**Phase:** Pseudocode (SPARC P)
**Risk Level:** Low (format change only, no logic change)

---

## Algorithm Specification

### ALGORITHM: ConvertDomainYamlToJson

```
ALGORITHM: ConvertDomainYamlToJson
INPUT:
    yaml_path: Path to source domain.yaml file
    schema_path: Path to domain.schema.json for validation
OUTPUT:
    json_path: Path to generated domain.json file
    success: Boolean indicating conversion success

PRECONDITIONS:
    - yaml_path exists and is readable
    - yaml_path contains valid YAML syntax
    - schema_path exists and is valid JSON Schema (draft-07)

POSTCONDITIONS:
    - json_path contains semantically equivalent JSON
    - json_path validates against schema_path
    - Original YAML comments preserved as "description" fields where applicable

COMPLEXITY:
    Time: O(n) where n = number of YAML nodes
    Space: O(n) for in-memory representation
```

---

## Main Conversion Flow

```
BEGIN ConvertDomainYamlToJson(yaml_path, schema_path):

    // ========================================
    // PHASE 1: Parse YAML Source
    // ========================================

    TRY:
        yaml_content <- ReadFile(yaml_path)
    CATCH FileNotFound:
        RETURN Error("Source YAML not found: " + yaml_path)
    CATCH PermissionDenied:
        RETURN Error("Cannot read source YAML: " + yaml_path)
    END TRY

    TRY:
        yaml_tree <- ParseYaml(yaml_content)
    CATCH YamlSyntaxError(line, column, message):
        RETURN Error("YAML parse error at " + line + ":" + column + ": " + message)
    END TRY

    // ========================================
    // PHASE 2: Transform Structure
    // ========================================

    // Current domain.yaml uses FLAT format (no `domain:` wrapper)
    // JSON Schema requires WRAPPED format (with `domain:` root key)

    IF yaml_tree HAS KEY "domain" THEN:
        // Already wrapped - use as-is
        json_tree <- yaml_tree
    ELSE IF yaml_tree HAS KEY "id" AND yaml_tree HAS KEY "streams" THEN:
        // Flat format detected - wrap it
        json_tree <- { "domain": yaml_tree }
    ELSE:
        RETURN Error("Invalid domain config: missing 'id' or 'streams' fields")
    END IF

    // ========================================
    // PHASE 3: Field Mapping
    // ========================================

    json_tree <- MapFields(json_tree)

    // ========================================
    // PHASE 4: Comment Preservation
    // ========================================

    // YAML comments become JSON description fields where semantically appropriate
    json_tree <- PreserveCommentsAsDescriptions(yaml_content, json_tree)

    // ========================================
    // PHASE 5: JSON Serialization
    // ========================================

    TRY:
        json_content <- SerializeJson(json_tree, indent=2)
    CATCH SerializationError(message):
        RETURN Error("JSON serialization failed: " + message)
    END TRY

    // ========================================
    // PHASE 6: Schema Validation (Pre-Write)
    // ========================================

    schema <- LoadJsonSchema(schema_path)
    validation_errors <- ValidateAgainstSchema(json_tree, schema)

    IF validation_errors IS NOT EMPTY THEN:
        RETURN Error("Generated JSON fails schema validation: " +
                     FormatErrors(validation_errors))
    END IF

    // ========================================
    // PHASE 7: Write Output
    // ========================================

    json_path <- ReplacePath(yaml_path, ".yaml", ".json")

    TRY:
        WriteFile(json_path, json_content)
    CATCH WriteError(message):
        RETURN Error("Cannot write JSON output: " + message)
    END TRY

    RETURN Success(json_path)

END ConvertDomainYamlToJson
```

---

## Subroutine: Field Mapping

```
SUBROUTINE: MapFields(json_tree)
INPUT: json_tree (parsed JSON object with domain wrapper)
OUTPUT: json_tree with all fields correctly mapped

BEGIN:
    domain <- json_tree["domain"]

    // ----------------------------------------
    // 1. Required Fields (must exist)
    // ----------------------------------------

    ASSERT domain HAS KEY "id", "Missing required field: id"
    ASSERT domain HAS KEY "streams", "Missing required field: streams"
    ASSERT domain HAS KEY "alignment", "Missing required field: alignment"

    // ----------------------------------------
    // 2. Stream References Mapping
    // ----------------------------------------

    FOR EACH stream IN domain["streams"]:
        // Required: stream_id and role
        ASSERT stream HAS KEY "stream_id", "Stream missing stream_id"
        ASSERT stream HAS KEY "role", "Stream missing role"

        // Normalize role to lowercase snake_case
        stream["role"] <- ToLowerSnakeCase(stream["role"])

        // Validate role enum
        ASSERT stream["role"] IN ["primary", "context", "actuator", "constraint"],
               "Invalid role: " + stream["role"]

        // Optional: alias defaults to stream_id
        IF stream NOT HAS KEY "alias" THEN:
            // DO NOT set default - let schema handle it
            // Rust serde will default to stream_id if missing
        END IF

        // Optional: null_handling
        IF stream HAS KEY "null_handling" THEN:
            stream["null_handling"] <- ToLowerSnakeCase(stream["null_handling"])
            ASSERT stream["null_handling"] IN ["preserve", "carry_forward", "interpolate"],
                   "Invalid null_handling: " + stream["null_handling"]
        END IF
    END FOR

    // ----------------------------------------
    // 3. Alignment Configuration Mapping
    // ----------------------------------------

    alignment <- domain["alignment"]

    ASSERT alignment HAS KEY "view_name", "Alignment missing view_name"
    ASSERT alignment HAS KEY "granularity", "Alignment missing granularity"

    // Normalize join_strategy if present
    IF alignment HAS KEY "join_strategy" THEN:
        alignment["join_strategy"] <- ToLowerSnakeCase(alignment["join_strategy"])
    END IF

    // Normalize null_handling if present
    IF alignment HAS KEY "null_handling" THEN:
        alignment["null_handling"] <- ToLowerSnakeCase(alignment["null_handling"])
    END IF

    // ----------------------------------------
    // 4. Objectives Mapping (Optional)
    // ----------------------------------------

    IF domain HAS KEY "objectives" THEN:
        FOR EACH objective IN domain["objectives"]:
            ASSERT objective HAS KEY "id", "Objective missing id"
            ASSERT objective HAS KEY "target", "Objective missing target"

            target <- objective["target"]
            ASSERT target HAS KEY "stream", "Target missing stream"
            ASSERT target HAS KEY "metric", "Target missing metric"
            ASSERT target HAS KEY "condition", "Target missing condition"
            ASSERT target HAS KEY "threshold", "Target missing threshold"

            // Normalize priority if present
            IF objective HAS KEY "priority" THEN:
                objective["priority"] <- ToLowerCase(objective["priority"])
            END IF
        END FOR
    END IF

    // ----------------------------------------
    // 5. Constraints Mapping (Optional)
    // ----------------------------------------

    IF domain HAS KEY "constraints" THEN:
        FOR EACH constraint IN domain["constraints"]:
            ASSERT constraint HAS KEY "id", "Constraint missing id"
            ASSERT constraint HAS KEY "condition", "Constraint missing condition"
        END FOR
    END IF

    json_tree["domain"] <- domain
    RETURN json_tree

END MapFields
```

---

## Subroutine: Comment Preservation

```
SUBROUTINE: PreserveCommentsAsDescriptions(yaml_content, json_tree)
INPUT:
    yaml_content: Original YAML text with comments
    json_tree: Parsed JSON tree
OUTPUT:
    json_tree with description fields populated from relevant comments

BEGIN:
    // ----------------------------------------
    // Extract header comment block
    // ----------------------------------------

    header_comments <- ExtractHeaderComments(yaml_content)

    // Header comment becomes domain-level description if not set
    IF json_tree["domain"] NOT HAS KEY "description" THEN:
        IF header_comments IS NOT EMPTY THEN:
            json_tree["domain"]["description"] <- CleanComment(header_comments[0])
        END IF
    END IF

    // ----------------------------------------
    // Extract inline comments for streams
    // ----------------------------------------

    // YAML inline comments after stream entries:
    //   - stream_id: air-quality
    //     alias: indoor
    //     role: primary
    //     # NULL handling: preserve (observation stream - default)

    // These comments are INFORMATIONAL and should be preserved in
    // documentation but do NOT map to JSON fields since the behavior
    // is determined by stream_type, not the comment.

    // NOTE: For FE-002, we do NOT auto-generate description fields
    // from inline comments. Comments are documentation only.

    RETURN json_tree

END PreserveCommentsAsDescriptions


SUBROUTINE: ExtractHeaderComments(yaml_content)
INPUT: yaml_content string
OUTPUT: Array of header comment lines

BEGIN:
    lines <- SplitLines(yaml_content)
    header_comments <- []

    FOR EACH line IN lines:
        trimmed <- Trim(line)
        IF StartsWith(trimmed, "#") THEN:
            // Remove leading # and whitespace
            comment <- Trim(RemovePrefix(trimmed, "#"))
            IF comment IS NOT EMPTY THEN:
                header_comments.APPEND(comment)
            END IF
        ELSE IF trimmed IS NOT EMPTY THEN:
            // First non-comment, non-empty line ends header
            BREAK
        END IF
    END FOR

    RETURN header_comments

END ExtractHeaderComments


SUBROUTINE: CleanComment(comment_text)
INPUT: Raw comment text
OUTPUT: Cleaned description string

BEGIN:
    // Remove leading/trailing whitespace
    result <- Trim(comment_text)

    // Remove common markdown artifacts
    result <- RemovePrefix(result, "Domain:")
    result <- Trim(result)

    // Ensure first letter is capitalized
    IF Length(result) > 0 THEN:
        result <- ToUpperCase(result[0]) + result[1:]
    END IF

    RETURN result

END CleanComment
```

---

## Subroutine: Schema Validation

```
SUBROUTINE: ValidateAgainstSchema(json_tree, schema)
INPUT:
    json_tree: Parsed JSON object
    schema: Loaded JSON Schema object
OUTPUT:
    Array of validation errors (empty if valid)

BEGIN:
    errors <- []

    // Use jsonschema library for validation
    validator <- CreateValidator(schema, draft="draft-07")

    FOR EACH error IN validator.iter_errors(json_tree):
        errors.APPEND({
            "path": FormatJsonPath(error.absolute_path),
            "message": error.message,
            "validator": error.validator
        })
    END FOR

    RETURN errors

END ValidateAgainstSchema
```

---

## Concrete Example: indoor-air-quality Conversion

### Input: domain.yaml (Flat Format)

```yaml
# Domain: Indoor Air Quality
# Cross-stream alignment for correlation analysis

id: indoor-air-quality
description: "Maintain healthy indoor air quality"

streams:
  - stream_id: air-quality
    alias: indoor
    role: primary
    # NULL handling: preserve (observation stream - default)

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

objectives:
  - id: healthy_co2
    description: "Keep CO2 below 800 ppm for cognitive performance"
    target:
      stream: air-quality
      metric: co2
      condition: "<"
      threshold: 800
      unit: ppm
    priority: high
```

### Output: domain.json (Wrapped Format)

```json
{
  "domain": {
    "id": "indoor-air-quality",
    "description": "Maintain healthy indoor air quality",
    "streams": [
      {
        "stream_id": "air-quality",
        "alias": "indoor",
        "role": "primary"
      },
      {
        "stream_id": "outdoor-weather",
        "alias": "outdoor",
        "role": "context"
      },
      {
        "stream_id": "home-assistant-state",
        "alias": "state",
        "role": "actuator",
        "null_handling": "carry_forward"
      },
      {
        "stream_id": "outdoor-air-quality",
        "alias": "outdoor_aqi",
        "role": "constraint"
      }
    ],
    "alignment": {
      "view_name": "indoor_air_quality_aligned",
      "granularity": "1 hour",
      "join_strategy": "full_outer"
    },
    "objectives": [
      {
        "id": "healthy_co2",
        "description": "Keep CO2 below 800 ppm for cognitive performance",
        "target": {
          "stream": "air-quality",
          "metric": "co2",
          "condition": "<",
          "threshold": 800,
          "unit": "ppm"
        },
        "priority": "high"
      }
    ]
  }
}
```

---

## Critical Consideration: Flat vs Wrapped Format

### Current State Analysis

The existing `domain.yaml` uses **flat format** (no `domain:` wrapper), as noted in the file:
```yaml
# NOTE: Flat format (no `domain:` wrapper) per DomainConfig struct requirements
```

However, `domain.schema.json` requires **wrapped format**:
```json
{
  "required": ["domain"],
  "properties": {
    "domain": { "$ref": "#/definitions/domain_content" }
  }
}
```

### Resolution Strategy

The Rust `DomainConfig` struct (in `domain.rs`) deserializes the **content** directly:

```rust
pub struct DomainConfig {
    pub id: String,
    pub description: String,
    pub streams: Vec<StreamRef>,
    pub alignment: AlignmentConfig,
    pub objectives: Vec<ObjectiveConfig>,
}
```

**Decision**: Update `loader.rs` to expect wrapped JSON format that matches the schema:

1. JSON file contains: `{ "domain": { ... } }`
2. Schema validates: `{ "domain": { ... } }`
3. Loader extracts: The `domain` object and deserializes to `DomainConfig`

This requires a small update to `loader.rs`:

```
// Current (YAML, flat):
let config: DomainConfig = serde_yaml::from_str(&content)?;

// New (JSON, wrapped):
let wrapper: serde_json::Value = serde_json::from_str(&content)?;
let domain_value = wrapper.get("domain")
    .ok_or(GoldDdlError::ConfigParseError { message: "Missing 'domain' key" })?;
let config: DomainConfig = serde_json::from_value(domain_value.clone())?;
```

---

## Error Handling Matrix

| Error Condition | Detection Point | Error Code | Message Template |
|-----------------|-----------------|------------|------------------|
| YAML file not found | Phase 1 | FILE_NOT_FOUND | "Source YAML not found: {path}" |
| YAML syntax error | Phase 1 | YAML_SYNTAX | "YAML parse error at {line}:{col}: {msg}" |
| Missing required field | Phase 3 | MISSING_FIELD | "Missing required field: {field}" |
| Invalid enum value | Phase 3 | INVALID_ENUM | "Invalid {field}: {value}. Expected: {valid}" |
| Schema validation fails | Phase 6 | SCHEMA_INVALID | "Generated JSON fails schema: {errors}" |
| Cannot write output | Phase 7 | WRITE_ERROR | "Cannot write JSON output: {msg}" |

---

## Tooling Reference

### Recommended Tools for Conversion

**Python (for one-time conversion script):**
```python
import yaml
import json

with open('domain.yaml', 'r') as f:
    data = yaml.safe_load(f)

# Wrap in domain key for schema compliance
wrapped = {'domain': data}

with open('domain.json', 'w') as f:
    json.dump(wrapped, f, indent=2)
```

**jq (for verification):**
```bash
# Verify JSON is valid
jq . domain.json

# Extract domain content
jq '.domain' domain.json
```

**Python jsonschema (for validation):**
```bash
pip install jsonschema
python -c "
import json
from jsonschema import validate
with open('domain.json') as f: data = json.load(f)
with open('domain.schema.json') as f: schema = json.load(f)
validate(data, schema)
print('Valid!')
"
```

---

## Complexity Analysis

| Phase | Time Complexity | Space Complexity | Notes |
|-------|-----------------|------------------|-------|
| Parse YAML | O(n) | O(n) | n = file size |
| Transform Structure | O(1) | O(1) | Constant wrapper |
| Field Mapping | O(m) | O(1) | m = number of fields |
| Comment Preservation | O(n) | O(k) | k = comment count |
| JSON Serialization | O(n) | O(n) | Output buffer |
| Schema Validation | O(n * s) | O(s) | s = schema size |
| Write Output | O(n) | O(1) | Streaming write |

**Total: O(n * s)** dominated by schema validation
**Space: O(n)** for in-memory representation

---

## References

- **FE-002 SCOPE.md**: Feature specification
- **ADR-016-001**: Configuration source of truth (JSON)
- **domain.schema.json**: JSON Schema for domain configs
- **domain.rs**: Rust struct definitions
- **loader.rs**: Current YAML loading implementation
