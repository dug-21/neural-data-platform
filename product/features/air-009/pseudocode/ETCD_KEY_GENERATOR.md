# ETCD_KEY_GENERATOR.md

## Purpose

Generate etcd key paths for storing and retrieving nested `ndp_id` and `context` configuration, ensuring consistent key structure across the NDP stack.

## Algorithm Overview

The key generator transforms nested configuration structures into flat etcd key paths, and vice versa. It maintains a consistent naming scheme that supports both full-path lookups and prefix-based queries.

---

## Data Structures

```
TYPE EtcdKeyPath = String              # e.g., "/streams/air-quality/sources/0/ndp_id"

TYPE KeyGeneratorConfig = {
    root_prefix: String,               # Default: ""
    separator: String,                 # Default: "/"
    array_notation: ArrayNotation      # How to represent array indices
}

ENUM ArrayNotation =
    | Indexed                          # sources/0, sources/1
    | Bracketed                        # sources[0], sources[1]

TYPE KeyValuePair = {
    key: EtcdKeyPath,
    value: String                      # etcd stores all values as strings
}

TYPE StreamKeySet = {
    stream_id: String,
    source_index: Integer,
    keys: List<KeyValuePair>
}
```

---

## Key Path Constants

```
CONSTANTS:
    # Root paths
    STREAMS_ROOT = "/streams"

    # Path templates
    STREAM_PATH = "{STREAMS_ROOT}/{stream_id}"
    SOURCES_PATH = "{STREAM_PATH}/sources"
    SOURCE_PATH = "{SOURCES_PATH}/{source_index}"

    # Source fields
    NDP_ID_PATH = "{SOURCE_PATH}/ndp_id"
    CONTEXT_PATH = "{SOURCE_PATH}/context"

    # Context subpaths (examples)
    LOCATION_PATH = "{CONTEXT_PATH}/location"
    COORDINATES_PATH = "{LOCATION_PATH}/coordinates"
    LOCATION_TYPE_PATH = "{LOCATION_PATH}/type"
    LOCATION_PATH_PATH = "{LOCATION_PATH}/path"
```

---

## Key Generation from Config

```
ALGORITHM: GenerateEtcdKeys
INPUT:
    stream_id: String
    source_index: Integer
    source_config: SourceConfig
OUTPUT:
    List<KeyValuePair>

FUNCTION generate_etcd_keys(stream_id, source_index, source_config) -> List<KeyValuePair>:
    """
    Generates all etcd key-value pairs for a source configuration.

    Complexity: O(n) where n = total number of config fields
    """
    keys = []
    base_path = build_source_path(stream_id, source_index)

    # Step 1: Generate key for source type (required)
    keys.append(KeyValuePair{
        key: base_path + "/type",
        value: source_config.type
    })

    # Step 2: Generate key for ndp_id (if present)
    IF source_config.ndp_id IS NOT None THEN
        keys.append(KeyValuePair{
            key: base_path + "/ndp_id",
            value: source_config.ndp_id
        })
    END IF

    # Step 3: Generate keys for context (if present)
    IF source_config.context IS NOT None THEN
        context_keys = generate_context_keys(
            base_path + "/context",
            source_config.context
        )
        keys.extend(context_keys)
    END IF

    # Step 4: Generate keys for source-specific params
    param_keys = generate_param_keys(base_path, source_config.params)
    keys.extend(param_keys)

    RETURN keys
END FUNCTION


FUNCTION build_source_path(stream_id: String, source_index: Integer) -> String:
    """
    Constructs the base path for a source.

    Example: build_source_path("air-quality", 0)
             -> "/streams/air-quality/sources/0"
    """
    RETURN "/streams/{stream_id}/sources/{source_index}"
END FUNCTION
```

---

## Context Key Generation

```
FUNCTION generate_context_keys(
    base_path: String,
    context: Map<String, Any>
) -> List<KeyValuePair>:
    """
    Recursively generates etcd keys for nested context structure.

    Design principles:
        - Each leaf value becomes a separate key
        - Arrays are serialized as JSON strings
        - Nested maps create path segments

    Complexity: O(n) where n = total context fields
    """
    keys = []

    FOR key, value IN context:
        current_path = base_path + "/" + sanitize_key(key)

        IF value IS Map THEN
            # Recurse for nested objects
            nested_keys = generate_context_keys(current_path, value)
            keys.extend(nested_keys)

        ELSE IF value IS Array THEN
            # Serialize arrays as JSON
            keys.append(KeyValuePair{
                key: current_path,
                value: json_encode(value)
            })

        ELSE
            # Primitive value - convert to string
            keys.append(KeyValuePair{
                key: current_path,
                value: to_etcd_string(value)
            })
        END IF
    END FOR

    RETURN keys
END FUNCTION


FUNCTION sanitize_key(key: String) -> String:
    """
    Sanitizes a key for use in etcd path.

    Rules:
        - Replace spaces with underscores
        - Remove special characters except underscore and hyphen
        - Convert to lowercase

    Complexity: O(k) where k = key length
    """
    sanitized = key.to_lowercase()
    sanitized = replace_chars(sanitized, " ", "_")
    sanitized = remove_special_chars(sanitized, keep: ["-", "_"])

    IF sanitized IS empty THEN
        RAISE InvalidKeyError("Key '{key}' sanitizes to empty string")
    END IF

    RETURN sanitized
END FUNCTION


FUNCTION to_etcd_string(value: Any) -> String:
    """
    Converts a primitive value to etcd-storable string.

    Preserves type information through format:
        - Strings: stored as-is
        - Numbers: stored as decimal string
        - Booleans: "true" or "false"
    """
    MATCH type_of(value):
        CASE String:
            RETURN value
        CASE Integer:
            RETURN integer_to_string(value)
        CASE Float:
            RETURN float_to_string(value, precision: 10)
        CASE Boolean:
            RETURN IF value THEN "true" ELSE "false"
        DEFAULT:
            RETURN json_encode(value)
    END MATCH
END FUNCTION
```

---

## Key Path Patterns

```
ALGORITHM: GenerateKeyPatterns
DESCRIPTION: Common key patterns for querying etcd

FUNCTION get_stream_prefix(stream_id: String) -> String:
    """
    Returns prefix for all keys under a stream.

    Example: get_stream_prefix("air-quality")
             -> "/streams/air-quality/"
    """
    RETURN "/streams/{stream_id}/"
END FUNCTION


FUNCTION get_source_prefix(stream_id: String, source_index: Integer) -> String:
    """
    Returns prefix for all keys under a specific source.

    Example: get_source_prefix("air-quality", 0)
             -> "/streams/air-quality/sources/0/"
    """
    RETURN "/streams/{stream_id}/sources/{source_index}/"
END FUNCTION


FUNCTION get_context_prefix(stream_id: String, source_index: Integer) -> String:
    """
    Returns prefix for all context keys under a source.

    Example: get_context_prefix("air-quality", 0)
             -> "/streams/air-quality/sources/0/context/"
    """
    RETURN "/streams/{stream_id}/sources/{source_index}/context/"
END FUNCTION


FUNCTION list_expected_keys(stream_id: String, source_index: Integer) -> List<String>:
    """
    Returns list of expected key paths for a source.

    Useful for validation and documentation.
    """
    base = "/streams/{stream_id}/sources/{source_index}"

    RETURN [
        base + "/type",
        base + "/ndp_id",
        base + "/context/location/coordinates",
        base + "/context/location/type",
        base + "/context/location/path",
        base + "/context/device_type",
        base + "/context/model",
        base + "/context/tags"
    ]
END FUNCTION
```

---

## Key Reconstruction (etcd -> Config)

```
ALGORITHM: ReconstructFromKeys
INPUT:
    keys: Map<EtcdKeyPath, String>     # All keys under a source prefix
    stream_id: String
    source_index: Integer
OUTPUT:
    SourceConfig

FUNCTION reconstruct_config(keys, stream_id, source_index) -> Result<SourceConfig>:
    """
    Reconstructs a SourceConfig from flat etcd keys.

    This is the inverse of generate_etcd_keys.

    Complexity: O(k * d) where k = keys, d = max depth
    """
    prefix = get_source_prefix(stream_id, source_index)

    # Filter keys to this source
    source_keys = filter_by_prefix(keys, prefix)

    # Extract direct fields
    source_type = source_keys.get(prefix + "type")
    IF source_type IS None THEN
        RETURN Err("Missing required key: type")
    END IF

    ndp_id = source_keys.get(prefix + "ndp_id")  # Optional

    # Reconstruct nested context
    context = reconstruct_nested_structure(
        source_keys,
        prefix + "context/"
    )

    # Reconstruct params
    params = reconstruct_params(source_keys, prefix, source_type)

    RETURN Ok(SourceConfig{
        type: source_type,
        ndp_id: ndp_id,
        context: context,
        params: params
    })
END FUNCTION


FUNCTION reconstruct_nested_structure(
    keys: Map<String, String>,
    prefix: String
) -> Map<String, Any>:
    """
    Rebuilds nested map from flat keys.

    Example:
        Keys:
            /context/location/type = "indoor"
            /context/location/path = "home/office"
            /context/device_type = "airgradient"

        Result:
            {
                location: {type: "indoor", path: "home/office"},
                device_type: "airgradient"
            }
    """
    result = {}

    FOR key, value IN keys:
        IF NOT key.starts_with(prefix) THEN
            CONTINUE
        END IF

        # Get relative path after prefix
        relative = key.substring(length(prefix))
        path_parts = split(relative, "/")

        # Navigate to parent, creating maps as needed
        current = result
        FOR i = 0 TO length(path_parts) - 2:
            part = path_parts[i]
            IF part NOT IN current THEN
                current[part] = {}
            END IF
            current = current[part]
        END FOR

        # Set leaf value
        leaf = path_parts[length(path_parts) - 1]
        current[leaf] = parse_etcd_value(value)
    END FOR

    RETURN result
END FUNCTION
```

---

## Batch Operations

```
ALGORITHM: GenerateStreamKeys
INPUT:
    stream_config: StreamConfig        # Full stream with multiple sources
OUTPUT:
    List<KeyValuePair>

FUNCTION generate_stream_keys(stream_config) -> List<KeyValuePair>:
    """
    Generates all etcd keys for a complete stream configuration.

    Complexity: O(s * f) where s = sources, f = fields per source
    """
    all_keys = []
    stream_id = stream_config.id

    # Stream-level metadata
    all_keys.append(KeyValuePair{
        key: "/streams/{stream_id}/name",
        value: stream_config.name
    })

    IF stream_config.description IS NOT None THEN
        all_keys.append(KeyValuePair{
            key: "/streams/{stream_id}/description",
            value: stream_config.description
        })
    END IF

    # Source configurations
    FOR idx, source IN enumerate(stream_config.sources):
        source_keys = generate_etcd_keys(stream_id, idx, source)
        all_keys.extend(source_keys)
    END FOR

    RETURN all_keys
END FUNCTION


FUNCTION sync_to_etcd(client: EtcdClient, keys: List<KeyValuePair>) -> Result<SyncReport>:
    """
    Syncs generated keys to etcd with transaction support.

    Uses etcd transactions for atomic updates.
    """
    txn = client.begin_transaction()

    TRY
        FOR kv IN keys:
            txn.put(kv.key, kv.value)
        END FOR

        txn.commit()

        RETURN Ok(SyncReport{
            keys_written: length(keys),
            success: true
        })
    CATCH EtcdError as e:
        txn.rollback()
        RETURN Err(SyncError{message: e.message})
    END TRY
END FUNCTION
```

---

## Edge Cases

```
EDGE CASE HANDLING:

1. Empty stream_id:
   INPUT:  generate_etcd_keys("", 0, config)
   OUTPUT: ERROR - stream_id cannot be empty

2. Negative source index:
   INPUT:  generate_etcd_keys("air", -1, config)
   OUTPUT: ERROR - source_index must be >= 0

3. Special characters in stream_id:
   INPUT:  stream_id = "air quality"
   OUTPUT: Sanitize to "air-quality" or reject
   RECOMMENDATION: Validate at config parse time, not key generation

4. Deep nesting (>5 levels):
   INPUT:  context = {a: {b: {c: {d: {e: {f: "value"}}}}}}
   OUTPUT: /context/a/b/c/d/e/f = "value"
   NOTE:   May want to limit depth for practical reasons

5. Empty context object:
   INPUT:  context = {}
   OUTPUT: No context keys generated

6. Coordinates with extra precision:
   INPUT:  coordinates = [29.958123456789, -81.308987654321]
   OUTPUT: /context/location/coordinates = "[29.958123456789,-81.308987654321]"
   NOTE:   JSON preserves precision

7. Unicode in values:
   INPUT:  context = {location: {path: "casa/habitacion"}}
   OUTPUT: /context/location/path = "casa/habitacion"
   NOTE:   etcd supports UTF-8

8. Array with mixed types:
   INPUT:  tags = ["primary", 42, true]
   OUTPUT: /context/tags = "[\"primary\",42,true]"
   NOTE:   JSON serialization handles mixed types

9. Key already exists in etcd (update):
   ACTION: PUT overwrites existing value
   NOTE:   Use etcd transactions for atomic multi-key updates
```

---

## Complexity Analysis

```
TIME COMPLEXITY:
    Key generation:
        - Per source: O(f) where f = total fields
        - Per stream: O(s * f) where s = sources

    Key reconstruction:
        - Per source: O(k * d) where k = keys, d = max depth
        - Path splitting: O(p) where p = path length

    etcd sync:
        - Network: O(k) requests (or O(1) with transaction)
        - Local: O(k) key generations

SPACE COMPLEXITY:
    - Key list: O(k) for k keys
    - Key strings: O(k * p) where p = avg path length
    - Reconstructed config: O(f) for f fields

OPTIMIZATION NOTES:
    - Use string builder for path construction
    - Pool key strings for repeated stream/source patterns
    - Batch etcd writes in transactions
```

---

## Rust Implementation Notes

```
RUST CONSIDERATIONS:

1. Key path builder with compile-time safety:
   struct KeyPath<'a> {
       parts: SmallVec<[&'a str; 8]>,
   }

   impl KeyPath<'_> {
       fn push(&mut self, part: &str) -> &mut Self;
       fn build(&self) -> String;
   }

2. Use format! macro sparingly - prefer string concatenation:
   // Faster
   let path = String::with_capacity(64);
   path.push_str("/streams/");
   path.push_str(&stream_id);

   // Slower
   let path = format!("/streams/{}/sources/{}", stream_id, idx);

3. etcd client integration (etcd-client crate):
   use etcd_client::{Client, PutOptions};

   async fn sync_keys(client: &mut Client, keys: &[KeyValue]) -> Result<()> {
       let txn = client.txn();
       // ... build transaction
       txn.commit().await?;
   }

4. Serde for key-value serialization:
   #[derive(Serialize)]
   struct EtcdKeyValue {
       key: String,
       value: String,
   }

5. Use Arc<str> for shared stream_id across keys:
   struct KeyGenerator {
       stream_id: Arc<str>,
   }

6. Iterator-based key generation (lazy evaluation):
   fn generate_keys<'a>(config: &'a SourceConfig) -> impl Iterator<Item = KeyValue> + 'a {
       // ...
   }
```

---

## Test Cases

```
TEST: generate_basic_keys
    INPUT:
        stream_id = "air-quality"
        source_index = 0
        config = SourceConfig{type: "mqtt", ndp_id: "sensor-001"}
    EXPECT:
        [
            ("/streams/air-quality/sources/0/type", "mqtt"),
            ("/streams/air-quality/sources/0/ndp_id", "sensor-001")
        ]

TEST: generate_full_context_keys
    INPUT:
        stream_id = "air-quality"
        source_index = 0
        config = SourceConfig{
            type: "mqtt",
            ndp_id: "airgradient-office-001",
            context: {
                location: {
                    coordinates: [29.958, -81.308],
                    type: "indoor",
                    path: "home/upstairs/office"
                },
                device_type: "airgradient",
                model: "ONE-V9",
                tags: ["primary", "calibrated"]
            }
        }
    EXPECT:
        [
            ("/streams/air-quality/sources/0/type", "mqtt"),
            ("/streams/air-quality/sources/0/ndp_id", "airgradient-office-001"),
            ("/streams/air-quality/sources/0/context/location/coordinates", "[29.958,-81.308]"),
            ("/streams/air-quality/sources/0/context/location/type", "indoor"),
            ("/streams/air-quality/sources/0/context/location/path", "home/upstairs/office"),
            ("/streams/air-quality/sources/0/context/device_type", "airgradient"),
            ("/streams/air-quality/sources/0/context/model", "ONE-V9"),
            ("/streams/air-quality/sources/0/context/tags", "[\"primary\",\"calibrated\"]")
        ]

TEST: reconstruct_from_keys
    INPUT:
        keys = {
            "/streams/air/sources/0/type": "mqtt",
            "/streams/air/sources/0/ndp_id": "sensor-001",
            "/streams/air/sources/0/context/location/type": "indoor"
        }
        stream_id = "air"
        source_index = 0
    EXPECT:
        SourceConfig{
            type: "mqtt",
            ndp_id: "sensor-001",
            context: {location: {type: "indoor"}}
        }

TEST: get_prefix_patterns
    INPUT:
        stream_id = "nws-observations"
        source_index = 0
    EXPECT:
        stream_prefix = "/streams/nws-observations/"
        source_prefix = "/streams/nws-observations/sources/0/"
        context_prefix = "/streams/nws-observations/sources/0/context/"
```

---

## Integration with ConfigSyncService

```
ALGORITHM: ConfigSyncWithContextKeys
DESCRIPTION: How key generator integrates with existing ConfigSyncService

FUNCTION sync_stream_config(stream_config: StreamConfig):
    """
    Full sync workflow from YAML to etcd.
    """

    # Step 1: Parse YAML (using CONFIG_PARSER)
    parsed = parse_stream_config(yaml_content)

    # Step 2: Generate all etcd keys
    keys = generate_stream_keys(parsed)

    # Step 3: Sync to etcd with transaction
    result = sync_to_etcd(etcd_client, keys)

    IF result IS Err THEN
        LOG_ERROR("Config sync failed: {result.error}")
        RETURN
    END IF

    LOG_INFO("Synced {result.keys_written} keys for stream {stream_config.id}")
END FUNCTION
```
