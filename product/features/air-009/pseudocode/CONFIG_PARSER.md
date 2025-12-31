# CONFIG_PARSER.md

## Purpose

Parse `ndp_id` and `context` fields from YAML stream configuration or etcd key-value pairs, handling optional fields and validating structure. The context is passed through without transformation to downstream processors.

**Implements:** ADR-002-AMENDMENT-002 (Simple Context Blob Storage)

## Algorithm Overview

The config parser extracts source identity (`ndp_id`) and context attributes from configuration, supporting both YAML file parsing and etcd key reconstruction. It validates required fields while allowing flexible, dynamic context keys.

**Key principle:** The config parser does NOT flatten or transform the context structure. It validates and passes through the complete context object, which is later serialized to a JSON blob by the context processor.

---

## Data Structures

```
TYPE SourceConfig = {
    type: String,                      # Required: "mqtt", "http", etc.
    ndp_id: Option<String>,            # Optional: stable source identifier
    context: Option<Value>,            # Optional: preserved as-is (serde_json::Value)
    params: Map<String, Any>           # Source-specific parameters
}

TYPE LocationContext = {
    coordinates: Option<[f64, f64]>,   # [lat, lon]
    type: Option<String>,              # "indoor" | "outdoor"
    path: Option<String>               # Hierarchical path
}

TYPE ParseError = {
    field: String,
    message: String,
    line: Option<Integer>              # For YAML parsing
}

TYPE ParseResult<T> = Ok(T) | Err(List<ParseError>)
```

---

## Main Algorithm: YAML Parsing

```
ALGORITHM: ParseSourceConfig
INPUT:
    yaml_content: String               # Raw YAML content
    stream_id: String                  # For error context
OUTPUT:
    ParseResult<SourceConfig>

FUNCTION parse_source_config(yaml_content, stream_id) -> ParseResult<SourceConfig>:
    """
    Parses a single source configuration from YAML.

    IMPORTANT: Context is validated but NOT transformed. The complete
    nested structure is preserved for downstream JSON serialization.

    Complexity: O(n) where n = size of YAML content
    """
    errors = []

    # Step 1: Parse YAML structure
    TRY
        raw = yaml_parse(yaml_content)
    CATCH YamlError as e
        errors.append(ParseError{
            field: "yaml",
            message: "Invalid YAML syntax: {e.message}",
            line: e.line
        })
        RETURN Err(errors)
    END TRY

    # Step 2: Validate required 'type' field
    IF "type" NOT IN raw THEN
        errors.append(ParseError{
            field: "type",
            message: "Missing required field 'type'"
        })
    ELSE
        source_type = raw["type"]
        IF source_type NOT IN VALID_SOURCE_TYPES THEN
            errors.append(ParseError{
                field: "type",
                message: "Invalid source type: {source_type}"
            })
        END IF
    END IF

    # Step 3: Parse optional ndp_id
    ndp_id = parse_ndp_id(raw, errors)

    # Step 4: Parse optional context (validate but preserve structure)
    context = parse_context(raw, errors)

    # Step 5: Parse source-specific params
    params = extract_params(raw, source_type)

    IF errors IS NOT empty THEN
        RETURN Err(errors)
    END IF

    RETURN Ok(SourceConfig{
        type: source_type,
        ndp_id: ndp_id,
        context: context,      # Complete nested structure preserved
        params: params
    })
END FUNCTION
```

---

## NDP ID Parsing and Validation

```
CONSTANTS:
    NDP_ID_PATTERN = "^[a-z0-9][a-z0-9-]*[a-z0-9]$"  # kebab-case
    NDP_ID_MIN_LENGTH = 3
    NDP_ID_MAX_LENGTH = 64

FUNCTION parse_ndp_id(raw: Map, errors: List<ParseError>) -> Option<String>:
    """
    Extracts and validates the ndp_id field.

    Validation rules:
        - Must be kebab-case (lowercase, hyphens, no underscores)
        - Length: 3-64 characters
        - Must start and end with alphanumeric
        - No consecutive hyphens

    Complexity: O(m) where m = length of ndp_id
    """
    IF "ndp_id" NOT IN raw THEN
        RETURN None   # Optional field - absence is valid
    END IF

    ndp_id = raw["ndp_id"]

    # Type check
    IF ndp_id IS NOT String THEN
        errors.append(ParseError{
            field: "ndp_id",
            message: "ndp_id must be a string, got: {type_of(ndp_id)}"
        })
        RETURN None
    END IF

    # Length validation
    IF length(ndp_id) < NDP_ID_MIN_LENGTH THEN
        errors.append(ParseError{
            field: "ndp_id",
            message: "ndp_id too short (min {NDP_ID_MIN_LENGTH}): '{ndp_id}'"
        })
        RETURN None
    END IF

    IF length(ndp_id) > NDP_ID_MAX_LENGTH THEN
        errors.append(ParseError{
            field: "ndp_id",
            message: "ndp_id too long (max {NDP_ID_MAX_LENGTH}): '{ndp_id}'"
        })
        RETURN None
    END IF

    # Pattern validation
    IF NOT regex_match(NDP_ID_PATTERN, ndp_id) THEN
        errors.append(ParseError{
            field: "ndp_id",
            message: "ndp_id must be kebab-case: '{ndp_id}'"
        })
        RETURN None
    END IF

    # Consecutive hyphen check
    IF contains(ndp_id, "--") THEN
        errors.append(ParseError{
            field: "ndp_id",
            message: "ndp_id cannot contain consecutive hyphens: '{ndp_id}'"
        })
        RETURN None
    END IF

    RETURN Some(ndp_id)
END FUNCTION
```

---

## Context Parsing (Pass-Through with Validation)

```
FUNCTION parse_context(raw: Map, errors: List<ParseError>) -> Option<Value>:
    """
    Extracts the context, validates known structures, and PRESERVES AS-IS.

    The context will be serialized to a JSON blob by the downstream
    context processor. No transformation happens here.

    Complexity: O(k) where k = number of context keys
    """
    IF "context" NOT IN raw THEN
        RETURN None   # Optional field
    END IF

    context = raw["context"]

    # Type check
    IF context IS NOT Map THEN
        errors.append(ParseError{
            field: "context",
            message: "context must be an object, got: {type_of(context)}"
        })
        RETURN None
    END IF

    # Validate location if present
    IF "location" IN context THEN
        validate_location(context["location"], errors)
    END IF

    # Validate tags if present
    IF "tags" IN context THEN
        validate_tags(context["tags"], errors)
    END IF

    # Dynamic keys: no validation, pass through as-is
    # This allows domain-specific fields like device_type, model, calibration, etc.
    # These will be preserved in the context JSON blob

    # Return the COMPLETE context structure (no transformation)
    RETURN Some(context)
END FUNCTION


FUNCTION validate_location(location: Any, errors: List<ParseError>) -> void:
    """
    Validates the location substructure.

    All location fields will be stored in the context JSON blob.
    Validation ensures data quality before storage.
    """
    IF location IS NOT Map THEN
        errors.append(ParseError{
            field: "context.location",
            message: "location must be an object"
        })
        RETURN
    END IF

    # Validate coordinates if present
    IF "coordinates" IN location THEN
        coords = location["coordinates"]

        IF coords IS NOT Array THEN
            errors.append(ParseError{
                field: "context.location.coordinates",
                message: "coordinates must be an array"
            })
        ELSE IF length(coords) != 2 THEN
            errors.append(ParseError{
                field: "context.location.coordinates",
                message: "coordinates must have exactly 2 elements [lat, lon]"
            })
        ELSE
            lat = coords[0]
            lon = coords[1]

            IF lat IS NOT Number OR lon IS NOT Number THEN
                errors.append(ParseError{
                    field: "context.location.coordinates",
                    message: "coordinates must be numbers"
                })
            ELSE IF lat < -90 OR lat > 90 THEN
                errors.append(ParseError{
                    field: "context.location.coordinates[0]",
                    message: "latitude must be between -90 and 90"
                })
            ELSE IF lon < -180 OR lon > 180 THEN
                errors.append(ParseError{
                    field: "context.location.coordinates[1]",
                    message: "longitude must be between -180 and 180"
                })
            END IF
        END IF
    END IF

    # Validate type if present
    IF "type" IN location THEN
        loc_type = location["type"]
        IF loc_type NOT IN ["indoor", "outdoor"] THEN
            errors.append(ParseError{
                field: "context.location.type",
                message: "location.type must be 'indoor' or 'outdoor'"
            })
        END IF
    END IF

    # path: String - no validation beyond type
    IF "path" IN location AND location["path"] IS NOT String THEN
        errors.append(ParseError{
            field: "context.location.path",
            message: "location.path must be a string"
        })
    END IF
END FUNCTION


FUNCTION validate_tags(tags: Any, errors: List<ParseError>) -> void:
    """
    Validates the tags array.

    Tags will be stored in the context JSON blob.
    """
    IF tags IS NOT Array THEN
        errors.append(ParseError{
            field: "context.tags",
            message: "tags must be an array"
        })
        RETURN
    END IF

    FOR idx, tag IN enumerate(tags):
        IF tag IS NOT String THEN
            errors.append(ParseError{
                field: "context.tags[{idx}]",
                message: "all tags must be strings"
            })
        END IF
    END FOR
END FUNCTION
```

---

## etcd Key Reconstruction

```
ALGORITHM: ReconstructFromEtcd
INPUT:
    keys: Map<String, String>          # etcd key-value pairs
    stream_id: String
    source_index: Integer
OUTPUT:
    ParseResult<SourceConfig>

FUNCTION reconstruct_from_etcd(keys, stream_id, source_index) -> ParseResult<SourceConfig>:
    """
    Reconstructs SourceConfig from flat etcd key-value pairs.

    Expected key patterns:
        /streams/{stream_id}/sources/{index}/type
        /streams/{stream_id}/sources/{index}/ndp_id
        /streams/{stream_id}/sources/{index}/context/location/coordinates
        /streams/{stream_id}/sources/{index}/context/{dynamic_key}

    The reconstructed context is a complete nested structure.

    Complexity: O(k) where k = number of keys
    """
    prefix = "/streams/{stream_id}/sources/{source_index}"
    errors = []

    # Extract type (required)
    type_key = prefix + "/type"
    IF type_key NOT IN keys THEN
        errors.append(ParseError{
            field: "type",
            message: "Missing required key: {type_key}"
        })
        RETURN Err(errors)
    END IF
    source_type = keys[type_key]

    # Extract ndp_id (optional)
    ndp_id_key = prefix + "/ndp_id"
    ndp_id = keys.get(ndp_id_key, None)

    # Reconstruct context from nested keys (preserves full structure)
    context = reconstruct_context(keys, prefix + "/context")

    # Extract params (source-specific)
    params = reconstruct_params(keys, prefix, source_type)

    RETURN Ok(SourceConfig{
        type: source_type,
        ndp_id: ndp_id,
        context: context,   # Complete nested structure
        params: params
    })
END FUNCTION


FUNCTION reconstruct_context(keys: Map, context_prefix: String) -> Option<Value>:
    """
    Rebuilds nested context from flat etcd keys.

    Example:
        /context/location/type = "indoor"
        /context/location/coordinates = "[29.958, -81.308]"
        /context/device_type = "airgradient"

    Becomes:
        {
            "location": {"type": "indoor", "coordinates": [29.958, -81.308]},
            "device_type": "airgradient"
        }

    This nested structure is then serialized to JSON by process_context().
    """
    context = {}

    FOR key, value IN keys:
        IF NOT key.starts_with(context_prefix + "/") THEN
            CONTINUE
        END IF

        # Extract the path after context prefix
        relative_path = key.substring(length(context_prefix) + 1)
        path_parts = split(relative_path, "/")

        # Navigate/create nested structure
        current = context
        FOR i = 0 TO length(path_parts) - 2:
            part = path_parts[i]
            IF part NOT IN current THEN
                current[part] = {}
            END IF
            current = current[part]
        END FOR

        # Set the leaf value (parse JSON if needed)
        leaf_key = path_parts[length(path_parts) - 1]
        current[leaf_key] = parse_etcd_value(value)
    END FOR

    IF context IS empty THEN
        RETURN None
    END IF

    RETURN Some(context)
END FUNCTION


FUNCTION parse_etcd_value(value: String) -> Any:
    """
    Parses etcd string value to appropriate type.

    etcd stores everything as strings, so we need to:
        - Parse JSON arrays: "[29.958, -81.308]" -> [29.958, -81.308]
        - Parse numbers: "42" -> 42, "3.14" -> 3.14
        - Parse booleans: "true" -> true
        - Keep strings as-is: "indoor" -> "indoor"
    """
    # Try JSON parse first (for arrays and complex values)
    TRY
        parsed = json_parse(value)
        RETURN parsed
    CATCH
        # Not valid JSON, continue with primitives
    END TRY

    # Try boolean
    IF value == "true" THEN RETURN true
    IF value == "false" THEN RETURN false

    # Try number
    TRY
        IF contains(value, ".") THEN
            RETURN parse_float(value)
        ELSE
            RETURN parse_int(value)
        END IF
    CATCH
        # Not a number
    END TRY

    # Return as string
    RETURN value
END FUNCTION
```

---

## Data Flow Summary

```
CONTEXT DATA FLOW:

1. CONFIG_PARSER (this file):
   - Parses YAML/etcd into SourceConfig
   - Validates known structures (location, tags)
   - Preserves complete context as serde_json::Value
   - NO transformation or flattening

2. CONTEXT_PROCESSOR (process_context):
   - Receives complete context from SourceConfig
   - Serializes to JSON string: serde_json::to_string(context)
   - That's it - simple serialization

3. RECORD_ENRICHER:
   - Calls process_context() once at source initialization
   - Adds ndp_id and context JSON blob to each record
   - Efficient per-record processing

4. STORAGE:
   - Bronze (Parquet): ndp_id: STRING, context: STRING (JSON)
   - Silver (TimescaleDB): ndp_id: TEXT, context: JSONB
```

---

## Edge Cases

```
EDGE CASE HANDLING:

1. Missing ndp_id and context (both optional):
   INPUT:  {type: "mqtt", ...params}
   OUTPUT: SourceConfig{ndp_id: None, context: None}

2. Empty context object:
   INPUT:  {type: "mqtt", context: {}}
   OUTPUT: SourceConfig{context: None}  # Treat empty as absent

3. Partial location (only some fields):
   INPUT:  {context: {location: {type: "indoor"}}}
   OUTPUT: Valid - coordinates and path are optional
   NOTE: Entire location stored in context JSON blob

4. Invalid coordinate range:
   INPUT:  {context: {location: {coordinates: [999, -81.308]}}}
   OUTPUT: ParseError on latitude out of range

5. Unknown context fields (allowed):
   INPUT:  {context: {custom_field: "value", another: 123}}
   OUTPUT: Valid - dynamic fields pass through
   NOTE: These will appear in context JSON blob

6. Type mismatch in known fields:
   INPUT:  {context: {location: "not an object"}}
   OUTPUT: ParseError on location type

7. etcd special characters in paths:
   KEY:    /streams/air-quality/sources/0/context/location/path
   VALUE:  "home/upstairs/office"
   OUTPUT: location.path = "home/upstairs/office" (slashes in value preserved)

8. Malformed ndp_id:
   INPUT:  {ndp_id: "Invalid_ID"}
   OUTPUT: ParseError - underscores not allowed (kebab-case required)

9. Complex nested context (preserved):
   INPUT:  {context: {
       location: {type: "indoor"},
       calibration: {
           sensors: [
               {id: "pm25", offset: 0.5},
               {id: "co2", offset: 10}
           ]
       }
   }}
   OUTPUT: SourceConfig{context: <complete structure>}
   NOTE: calibration preserved exactly for JSON serialization
```

---

## Complexity Analysis

```
TIME COMPLEXITY:
    YAML parsing:
        - YAML parse: O(n) where n = content size
        - Field extraction: O(k) where k = number of fields
        - Validation: O(m) where m = length of validated strings
        - Total: O(n)

    etcd reconstruction:
        - Key iteration: O(k) where k = number of keys
        - Nested structure building: O(k * d) where d = avg path depth
        - Value parsing: O(v) where v = avg value length
        - Total: O(k * d)

SPACE COMPLEXITY:
    - Parsed config: O(k) for k fields
    - Error list: O(e) for e errors
    - Nested context: O(c) for c = context size
    - Total: O(n) where n = config size
```

---

## Rust Implementation Notes

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceConfig {
    #[serde(rename = "type")]
    pub source_type: String,

    pub ndp_id: Option<String>,

    // Context preserved as-is - complete nested structure
    pub context: Option<Value>,

    #[serde(flatten)]
    pub params: HashMap<String, Value>,
}

// Context is NOT a custom struct - it's dynamic Value
// This allows any nested structure to pass through unchanged

// Validation extracts temporarily for checking but doesn't modify
fn validate_location(context: &Value) -> Vec<ParseError> {
    let mut errors = vec![];
    if let Some(location) = context.get("location") {
        // Validate structure without modifying
    }
    errors
}
```

---

## Test Cases

```
TEST: parse_minimal_source
    INPUT:  "type: mqtt\nhost: localhost"
    EXPECT: Ok(SourceConfig{type: "mqtt", ndp_id: None, context: None})

TEST: parse_full_source_preserves_context
    INPUT:  """
        type: mqtt
        ndp_id: airgradient-office-001
        context:
          location:
            coordinates: [29.958, -81.308]
            type: indoor
            path: home/upstairs/office
          device_type: airgradient
          model: ONE-V9
          calibration:
            sensor_a:
              offset: 0.5
        host: localhost
    """
    EXPECT: Ok(SourceConfig{
        type: "mqtt",
        ndp_id: Some("airgradient-office-001"),
        context: Some({
            "location": {
                "coordinates": [29.958, -81.308],
                "type": "indoor",
                "path": "home/upstairs/office"
            },
            "device_type": "airgradient",
            "model": "ONE-V9",
            "calibration": {"sensor_a": {"offset": 0.5}}
        }),  // COMPLETE nested structure
        params: {host: "localhost"}
    })
    VERIFY: context is NOT flattened
    VERIFY: calibration.sensor_a.offset preserved as nested

TEST: parse_invalid_ndp_id
    INPUT:  "type: mqtt\nndp_id: Invalid_ID"
    EXPECT: Err([ParseError{field: "ndp_id", ...}])

TEST: parse_invalid_coordinates
    INPUT:  """
        type: mqtt
        context:
          location:
            coordinates: [999, -81.308]
    """
    EXPECT: Err([ParseError{field: "context.location.coordinates[0]", ...}])

TEST: parse_context_with_dynamic_fields
    INPUT:  """
        type: mqtt
        context:
          device_type: purpleair
          firmware: "2.0.3"
    """
    EXPECT: Ok(SourceConfig{
        context: Some({"device_type": "purpleair", "firmware": "2.0.3"})
    })
    NOTE: No location fields - context still valid

TEST: reconstruct_from_etcd
    INPUT: {
        "/streams/air/sources/0/type": "mqtt",
        "/streams/air/sources/0/ndp_id": "sensor-001",
        "/streams/air/sources/0/context/location/type": "indoor",
        "/streams/air/sources/0/context/device_type": "airgradient"
    }
    EXPECT: Ok(SourceConfig{
        type: "mqtt",
        ndp_id: Some("sensor-001"),
        context: Some({
            "location": {"type": "indoor"},
            "device_type": "airgradient"
        })  // Reconstructed nested structure
    })

TEST: context_preserved_for_serialization
    INPUT:  """
        type: mqtt
        context:
          location:
            type: indoor
          calibration:
            sensors:
              - id: pm25
                offset: 0.5
              - id: co2
                offset: 10
    """
    EXPECT:
        config.context["calibration"]["sensors"][0]["offset"] == 0.5
    NOTE: Array-of-objects preserved exactly
```

---

## Related Documentation

- ADR-002-AMENDMENT-002: Simple Context Blob Storage (defines the approach)
- CONTEXT_PROCESSOR.md: Simple JSON serialization
- RECORD_ENRICHER.md: Attaches ndp_id and context blob to records
