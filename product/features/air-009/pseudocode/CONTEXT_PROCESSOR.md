# CONTEXT_PROCESSOR.md

## Purpose

Serialize context to a JSON string for storage. No flattening. No promoted fields. Just simple JSON serialization.

**Implements:** ADR-002-AMENDMENT-002 (Simple Context Blob Storage)

## Algorithm Overview

The context processor performs a single operation: serialize the context `Value` to a JSON string. The complete nested structure is preserved exactly as configured.

---

## Data Structures

```
TYPE ContextValue = String | Number | Boolean | Array | Map<String, ContextValue>

TYPE ProcessedContext = String   # JSON-serialized context blob
```

---

## Main Algorithm

```
ALGORITHM: ProcessContext
INPUT:
    context: Map<String, ContextValue>   # Nested context from config (serde_json::Value)
OUTPUT:
    String                               # JSON-serialized context

FUNCTION process_context(context) -> String:
    """
    Entry point: serializes context to JSON string.

    That's it. No flattening. No promoted fields. No transformation.

    Complexity: O(n) where n = total context size
    """
    IF context IS null OR context IS empty THEN
        RETURN "{}"
    END IF

    RETURN json_encode(context)
END FUNCTION
```

---

## Edge Cases

```
EDGE CASE HANDLING:

1. Empty context:
   INPUT:  {}
   OUTPUT: "{}"

2. Null context:
   INPUT:  null
   OUTPUT: "{}"

3. Simple flat context:
   INPUT:  {
       "device_type": "airgradient",
       "model": "ONE-V9"
   }
   OUTPUT: '{"device_type":"airgradient","model":"ONE-V9"}'

4. Nested location context:
   INPUT:  {
       "location": {
           "coordinates": [29.958, -81.308],
           "type": "indoor",
           "path": "home/upstairs/office"
       }
   }
   OUTPUT: '{"location":{"coordinates":[29.958,-81.308],"type":"indoor","path":"home/upstairs/office"}}'

5. Full context with all fields:
   INPUT:  {
       "location": {
           "coordinates": [29.958, -81.308],
           "type": "indoor",
           "path": "home/upstairs/office"
       },
       "device_type": "airgradient",
       "model": "ONE-V9",
       "calibration": {
           "sensor_a": {"offset": 0.5, "last_date": "2024-01-15"}
       },
       "tags": ["primary", "calibrated"]
   }
   OUTPUT: '{"location":{"coordinates":[29.958,-81.308],"type":"indoor","path":"home/upstairs/office"},"device_type":"airgradient","model":"ONE-V9","calibration":{"sensor_a":{"offset":0.5,"last_date":"2024-01-15"}},"tags":["primary","calibrated"]}'
   NOTE: Complete structure preserved exactly

6. Deep nesting (no depth issues):
   INPUT:  {
       "location": {"type": "indoor"},
       "metadata": {
           "a": {"b": {"c": {"d": {"e": "deep value"}}}}
       }
   }
   OUTPUT: '{"location":{"type":"indoor"},"metadata":{"a":{"b":{"c":{"d":{"e":"deep value"}}}}}}'
   NOTE: No flattening ambiguity - structure preserved exactly
```

---

## Complexity Analysis

```
TIME COMPLEXITY:
    - JSON serialization: O(n) where n = total context size
    - Total: O(n)

SPACE COMPLEXITY:
    - Output string: O(n) for n = context size
    - Total: O(n)

COMPARISON TO OLD APPROACHES:
    Old (recursive flatten): O(n * d) time, O(n * d) space
    Hybrid (promote + raw):  O(n) time, O(n) space + promoted map overhead
    New (simple blob):       O(n) time, O(n) space

    Maximum simplicity - single serialization operation.
```

---

## Rust Implementation

```rust
use serde_json::Value;

/// Process context for storage - simple JSON serialization
///
/// No flattening. No promoted fields. Just serialize.
pub fn process_context(context: &Value) -> String {
    serde_json::to_string(context).unwrap_or_else(|_| "{}".to_string())
}

// That's it. That's the entire implementation.
```

---

## Test Cases

```
TEST: process_empty_context
    INPUT:  process_context({})
    EXPECT: "{}"

TEST: process_null_context
    INPUT:  process_context(null)
    EXPECT: "{}"

TEST: process_flat_context
    INPUT:  process_context({"device_type": "airgradient"})
    EXPECT: '{"device_type":"airgradient"}'

TEST: process_nested_context
    INPUT:  process_context({
        "location": {
            "coordinates": [29.958, -81.308],
            "type": "indoor"
        }
    })
    EXPECT: JSON string with exact structure preserved
    VERIFY: Can be deserialized back to identical structure

TEST: process_complex_context
    INPUT:  process_context({
        "location": {"type": "indoor"},
        "calibration": {
            "sensors": [
                {"id": "pm25", "offset": 0.5},
                {"id": "co2", "offset": 10}
            ]
        }
    })
    EXPECT: JSON string with arrays and nested objects preserved exactly
```

---

## Migration Notes

**From hybrid approach (ADR-002-AMENDMENT-001):**

1. Remove `ProcessedContext` struct with `promoted` and `raw` fields
2. Remove `PROMOTED_FIELDS` constant
3. Remove `extract_path` function
4. Remove all promoted field logic
5. Return simple String instead of ProcessedContext

**What was removed:**
- `ctx_location_type` promoted field
- `ctx_location_path` promoted field
- `ctx_location_coordinates` promoted field
- `context_raw` field (now just `context`)
- Path extraction algorithm
- Promoted field validation

**Schema changes:**
- Old: `ctx_location_*` columns + `context_raw` blob
- New: Just `context` column (STRING in Bronze, JSONB in Silver)

See: ADR-002-AMENDMENT-002 for full rationale
