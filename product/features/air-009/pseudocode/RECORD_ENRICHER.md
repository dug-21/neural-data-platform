# RECORD_ENRICHER.md

## Purpose

Attach `ndp_id` and serialized `context` JSON blob to each parsed record before storage. Simple enrichment with two fields - no promoted columns.

**Implements:** ADR-002-AMENDMENT-002 (Simple Context Blob Storage)

## Algorithm Overview

The record enricher operates at source initialization and per-record processing. It serializes the context once when the source starts, then efficiently attaches `ndp_id` and the context blob to each incoming record.

---

## Data Structures

```
TYPE RawRecord = Map<String, Any>      # Parsed record from source (MQTT, HTTP, etc.)

TYPE EnrichedRecord = {
    timestamp: DateTime,               # Original or injected timestamp
    ndp_id: Option<String>,            # Source identity (separate column)
    context: Option<String>,           # JSON blob (serialized once at init)
    fields: Map<String, Any>,          # Measurement fields from record
    raw_payload: Option<Bytes>         # Original payload for debugging
}

TYPE SourceEnricher = {
    ndp_id: Option<String>,            # Cached from config
    context_json: Option<String>       # Pre-serialized JSON blob
}
```

---

## Source Initialization

```
ALGORITHM: InitializeSourceEnricher
INPUT:
    source_config: SourceConfig        # Parsed source configuration
OUTPUT:
    SourceEnricher

FUNCTION initialize_enricher(source_config) -> Result<SourceEnricher>:
    """
    Called once when source starts. Serializes context to JSON.

    Complexity: O(c) where c = context size
    """

    # Step 1: Extract ndp_id (may be None)
    ndp_id = source_config.ndp_id

    # Step 2: Serialize context to JSON blob (once, at init)
    context_json = None

    IF source_config.context IS NOT None THEN
        context_json = process_context(source_config.context)
        IF context_json == "{}" THEN
            context_json = None   # Treat empty object as absent
        END IF
    END IF

    # Step 3: Log enricher configuration
    LOG_INFO("Enricher initialized: ndp_id={ndp_id}, context_size={length(context_json)}")

    RETURN Ok(SourceEnricher{
        ndp_id: ndp_id,
        context_json: context_json
    })
END FUNCTION
```

---

## Per-Record Enrichment

```
ALGORITHM: EnrichRecord
INPUT:
    enricher: SourceEnricher           # Pre-initialized enricher
    raw_record: RawRecord              # Parsed record from source
    receive_time: DateTime             # When record was received
OUTPUT:
    Result<EnrichedRecord>

FUNCTION enrich_record(enricher, raw_record, receive_time) -> Result<EnrichedRecord>:
    """
    Enriches a single record with:
      - ndp_id (separate column)
      - context (JSON blob)
      - Original record fields

    Called for every incoming record - must be efficient.

    Complexity: O(r) where r = record fields
    """

    # Step 1: Start with record fields (measurements)
    fields = shallow_copy(raw_record)

    # Step 2: Ensure timestamp exists
    timestamp = extract_or_inject_timestamp(fields, receive_time)

    RETURN Ok(EnrichedRecord{
        timestamp: timestamp,
        ndp_id: enricher.ndp_id,
        context: enricher.context_json,   # Pre-serialized JSON blob
        fields: fields,
        raw_payload: None  # Set by caller if needed
    })
END FUNCTION
```

---

## Field Mapping for Storage

```
ALGORITHM: MapToStorageFormat
DESCRIPTION: Maps EnrichedRecord to storage layer columns

FUNCTION map_to_parquet_columns(record: EnrichedRecord) -> ParquetRow:
    """
    Maps enriched record to Bronze layer Parquet columns.

    Output schema (from ADR-002-AMENDMENT-002):
        timestamp: TIMESTAMP
        ndp_id: STRING
        context: STRING (JSON blob)
        <measurement fields>
    """
    row = new ParquetRow()

    # Core fields
    row["timestamp"] = record.timestamp
    row["ndp_id"] = record.ndp_id           # May be null
    row["context"] = record.context          # JSON string, may be null

    # Measurement fields (dynamic)
    FOR key, value IN record.fields:
        IF key NOT IN RESERVED_FIELD_NAMES THEN
            row[key] = value
        END IF
    END FOR

    RETURN row
END FUNCTION


FUNCTION map_to_timescale_columns(record: EnrichedRecord) -> TimescaleRow:
    """
    Maps enriched record to Silver layer TimescaleDB columns.

    Output schema:
        time: TIMESTAMPTZ
        ndp_id: TEXT
        context: JSONB
        <measurement columns>
    """
    row = new TimescaleRow()

    # Core fields
    row["time"] = record.timestamp
    row["ndp_id"] = record.ndp_id           # May be null

    # Parse JSON string to JSONB
    IF record.context IS NOT None THEN
        row["context"] = json_to_jsonb(record.context)
    ELSE
        row["context"] = None
    END IF

    # Measurement fields
    FOR key, value IN record.fields:
        IF key NOT IN RESERVED_FIELD_NAMES THEN
            row[key] = value
        END IF
    END FOR

    RETURN row
END FUNCTION

CONSTANT RESERVED_FIELD_NAMES = {
    "timestamp", "time", "ndp_id", "context"
}
```

---

## Timestamp Handling

```
FUNCTION extract_or_inject_timestamp(
    fields: Map<String, Any>,
    receive_time: DateTime
) -> DateTime:
    """
    Extracts timestamp from record or injects receive time.

    Timestamp field names checked (in order):
        1. "timestamp"
        2. "time"
        3. "ts"
        4. "datetime"
        5. "@timestamp"

    Complexity: O(1) - fixed number of field checks
    """
    TIMESTAMP_FIELDS = ["timestamp", "time", "ts", "datetime", "@timestamp"]

    FOR field_name IN TIMESTAMP_FIELDS:
        IF field_name IN fields THEN
            raw_ts = fields[field_name]

            TRY
                timestamp = parse_timestamp(raw_ts)
                RETURN timestamp
            CATCH ParseError:
                LOG_WARN("Invalid timestamp in '{field_name}': {raw_ts}")
                CONTINUE
            END TRY
        END IF
    END FOR

    # No valid timestamp found - inject receive time
    LOG_DEBUG("No timestamp in record, using receive time")
    fields["timestamp"] = receive_time.to_iso8601()
    RETURN receive_time
END FUNCTION


FUNCTION parse_timestamp(value: Any) -> DateTime:
    """
    Parses timestamp from various formats.

    Supported formats:
        - ISO 8601: "2024-01-15T10:30:00Z"
        - Unix epoch (seconds): 1705315800
        - Unix epoch (milliseconds): 1705315800000
        - RFC 2822: "Mon, 15 Jan 2024 10:30:00 +0000"
    """
    IF value IS Number THEN
        IF value > 1e12 THEN  # Milliseconds (> year 2001)
            RETURN DateTime.from_millis(value)
        ELSE
            RETURN DateTime.from_secs(value)
        END IF
    END IF

    IF value IS String THEN
        TRY
            RETURN DateTime.parse_iso8601(value)
        CATCH
            TRY
                RETURN DateTime.parse_rfc2822(value)
            CATCH
                RAISE ParseError("Unrecognized timestamp format: {value}")
            END TRY
        END TRY
    END IF

    RAISE ParseError("Timestamp must be string or number, got: {type_of(value)}")
END FUNCTION
```

---

## Batch Enrichment

```
ALGORITHM: EnrichBatch
INPUT:
    enricher: SourceEnricher
    records: List<RawRecord>
    receive_time: DateTime
OUTPUT:
    Result<List<EnrichedRecord>>

FUNCTION enrich_batch(enricher, records, receive_time) -> Result<List<EnrichedRecord>>:
    """
    Efficiently enriches a batch of records.

    Optimization: Pre-allocates result list. Context is pre-serialized
    so per-record overhead is minimal.

    Complexity: O(n * r) where n = records, r = fields per record
    Note: Context is O(1) per record since it's pre-serialized and shared
    """
    enriched = List.with_capacity(length(records))

    FOR idx, record IN enumerate(records):
        result = enrich_record(enricher, record, receive_time)

        IF result IS Err THEN
            LOG_ERROR("Failed to enrich record {idx}: {result.error}")
            CONTINUE  # Skip failed record
        END IF

        enriched.append(result.value)
    END FOR

    RETURN Ok(enriched)
END FUNCTION
```

---

## Pipeline Integration

```
ALGORITHM: SourcePipelineWithEnrichment
DESCRIPTION: Shows how enricher integrates with ingestion pipeline

FUNCTION run_source_pipeline(source_config: SourceConfig):
    """
    Main pipeline loop with simple context blob enrichment.
    """

    # Initialize source and enricher
    source = create_source(source_config)
    enricher = initialize_enricher(source_config)

    # Pipeline loop
    WHILE source.is_active():
        # Receive raw data from source
        raw_message = source.receive()
        receive_time = DateTime.now()

        # Parse raw message to record(s)
        parse_result = source.parser.parse(raw_message)

        IF parse_result IS Err THEN
            LOG_ERROR("Parse failed: {parse_result.error}")
            CONTINUE
        END IF

        records = parse_result.value

        # Enrich each record (adds ndp_id, context blob)
        FOR record IN records:
            enriched = enrich_record(enricher, record, receive_time)

            IF enriched IS Ok THEN
                # Send to channel for storage
                channel.send(enriched.value)
            END IF
        END FOR
    END WHILE
END FUNCTION
```

---

## Edge Cases

```
EDGE CASE HANDLING:

1. No ndp_id or context configured:
   CONFIG:  {type: "mqtt", ...}
   RECORD:  {pm25: 12.5, temperature: 22.3}
   OUTPUT:  EnrichedRecord{
       ndp_id: None,
       context: None,
       fields: {pm25: 12.5, temperature: 22.3},
       timestamp: <receive_time>
   }

2. Full context:
   CONFIG:  {
       ndp_id: "airgradient-office-001",
       context: {
           location: {coordinates: [29.958, -81.308], type: "indoor", path: "home/office"},
           device_type: "airgradient",
           model: "ONE-V9"
       }
   }
   RECORD:  {pm25: 12.5}
   OUTPUT:  EnrichedRecord{
       ndp_id: "airgradient-office-001",
       context: '{"location":{"coordinates":[29.958,-81.308],"type":"indoor","path":"home/office"},"device_type":"airgradient","model":"ONE-V9"}',
       fields: {pm25: 12.5},
       timestamp: <receive_time>
   }

3. Empty context object:
   CONFIG:  {ndp_id: "sensor-001", context: {}}
   RECORD:  {pm25: 12.5}
   OUTPUT:  EnrichedRecord{
       ndp_id: "sensor-001",
       context: None,   # Empty object treated as absent
       fields: {pm25: 12.5}
   }

4. Record with existing timestamp:
   CONFIG:  {ndp_id: "sensor-001"}
   RECORD:  {timestamp: "2024-01-15T10:30:00Z", pm25: 12.5}
   OUTPUT:  timestamp = "2024-01-15T10:30:00Z"  # Preserved, not overwritten

5. Complex nested context:
   CONFIG:  {context: {
       location: {type: "indoor"},
       calibration: {
           sensors: [
               {id: "pm25", offset: 0.5, date: "2024-01-01"},
               {id: "co2", offset: 10, date: "2024-01-01"}
           ]
       }
   }}
   OUTPUT:  EnrichedRecord{
       context: <complete JSON including full calibration array>
   }
   NOTE: All nesting preserved exactly in JSON blob
```

---

## Complexity Analysis

```
TIME COMPLEXITY:
    Initialization (once per source):
        - Context serialization: O(c) where c = context size
        - Total: O(c)

    Per-record enrichment:
        - Shallow copy: O(r) where r = record fields
        - ndp_id/context assignment: O(1) - just reference copy
        - Timestamp extraction: O(1)
        - Total: O(r)

    Batch enrichment:
        - Total: O(n * r) where n = batch size

    IMPROVEMENT over hybrid approach:
        - Old: O(r + p) per record where p = promoted field lookups
        - New: O(r) per record - no promoted field overhead

SPACE COMPLEXITY:
    Enricher state:
        - ndp_id: O(1)
        - context_json: O(c) where c = context size

    Per-record:
        - Enriched fields: O(r) where r = record fields
        - context reference: O(1) - shared string
        - Total: O(r)
```

---

## Rust Implementation Notes

```rust
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Source enricher with pre-serialized context
pub struct SourceEnricher {
    pub ndp_id: Option<Arc<str>>,
    pub context_json: Option<Arc<str>>,  // Shared across all records
}

/// Enriched record ready for storage
pub struct EnrichedRecord {
    pub timestamp: DateTime<Utc>,
    pub ndp_id: Option<Arc<str>>,
    pub context: Option<Arc<str>>,       // JSON blob
    pub fields: HashMap<String, Value>,
}

impl SourceEnricher {
    /// Initialize enricher from source config
    pub fn new(config: &SourceConfig) -> Self {
        let context_json = config.context.as_ref()
            .map(|c| serde_json::to_string(c).unwrap_or_default())
            .filter(|s| s != "{}")
            .map(|s| Arc::from(s.as_str()));

        Self {
            ndp_id: config.ndp_id.as_ref().map(|s| Arc::from(s.as_str())),
            context_json,
        }
    }

    /// Enrich a single record
    pub fn enrich(&self, record: HashMap<String, Value>, receive_time: DateTime<Utc>) -> EnrichedRecord {
        let timestamp = extract_timestamp(&record).unwrap_or(receive_time);

        EnrichedRecord {
            timestamp,
            ndp_id: self.ndp_id.clone(),
            context: self.context_json.clone(),
            fields: record,
        }
    }
}
```

---

## Test Cases

```
TEST: enrich_basic_record
    SETUP:
        enricher = initialize_enricher(SourceConfig{
            ndp_id: "sensor-001",
            context: None
        })
    INPUT:
        record = {pm25: 12.5, temperature: 22.3}
    EXPECT:
        EnrichedRecord{
            ndp_id: "sensor-001",
            context: None,
            fields: {pm25: 12.5, temperature: 22.3}
        }

TEST: enrich_with_full_context
    SETUP:
        enricher = initialize_enricher(SourceConfig{
            ndp_id: "airgradient-office-001",
            context: {
                location: {coordinates: [29.958, -81.308], type: "indoor", path: "home/office"},
                device_type: "airgradient",
                model: "ONE-V9"
            }
        })
    INPUT:
        record = {pm25: 12.5}
    EXPECT:
        EnrichedRecord{
            ndp_id: "airgradient-office-001",
            context: <JSON string containing all context fields>,
            fields: {pm25: 12.5}
        }
    VERIFY: context JSON is valid and contains location, device_type, model

TEST: enrich_preserves_record_timestamp
    SETUP:
        enricher = initialize_enricher(SourceConfig{ndp_id: "sensor-001"})
    INPUT:
        record = {timestamp: "2024-01-15T10:30:00Z", pm25: 12.5}
    EXPECT:
        timestamp = "2024-01-15T10:30:00Z"  # Preserved, not overwritten

TEST: enrich_batch_shares_context
    SETUP:
        enricher = initialize_enricher(SourceConfig{
            ndp_id: "sensor-001",
            context: {location: {type: "indoor"}}
        })
    INPUT:
        records = [
            {pm25: 12.5},
            {pm25: 13.0},
            {pm25: 12.8}
        ]
    EXPECT:
        All records have:
            - ndp_id: "sensor-001"
            - Same context reference (Arc<str>)

TEST: map_to_parquet_columns
    INPUT:
        EnrichedRecord{
            timestamp: "2024-01-15T10:30:00Z",
            ndp_id: "sensor-001",
            context: '{"location":{"type":"indoor"}}',
            fields: {pm25: 12.5, temperature: 22.3}
        }
    EXPECT:
        ParquetRow{
            timestamp: "2024-01-15T10:30:00Z",
            ndp_id: "sensor-001",
            context: '{"location":{"type":"indoor"}}',
            pm25: 12.5,
            temperature: 22.3
        }

TEST: map_to_timescale_columns
    INPUT:
        EnrichedRecord{
            timestamp: "2024-01-15T10:30:00Z",
            ndp_id: "sensor-001",
            context: '{"location":{"type":"indoor"}}',
            fields: {pm25: 12.5}
        }
    EXPECT:
        TimescaleRow{
            time: "2024-01-15T10:30:00Z",
            ndp_id: "sensor-001",
            context: JSONB({"location":{"type":"indoor"}}),
            pm25: 12.5
        }
```

---

## Related Documentation

- ADR-002-AMENDMENT-002: Simple Context Blob Storage (defines the approach)
- CONTEXT_PROCESSOR.md: Simple JSON serialization (process_context)
- CONFIG_PARSER.md: Parses context from YAML/etcd
