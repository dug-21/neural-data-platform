# PSEUDOCODE.md - Bronze Layer Raw Storage Bug Fix

## Bug Summary

**Problem**: The Bronze layer is storing PARSED data instead of RAW API responses.

The current implementation violates ADR-001's core principle: "raw_payload is sacred - exactly what the source sent."

### Current WRONG Flow

```
HTTP Response (raw JSON)
         |
         v
    Parser.parse()  <-- BUG: parsing happens before storage
         |
         v
Vec<TimeSeriesPoint>  <-- Data already transformed (type coercion, field extraction)
         |
         v
   serialize each point
         |
         v
RawDataPoint {
    raw_payload: { "value": point.value, ... }  <-- WRONG: reconstructed, not original
}
```

### Correct Flow

```
HTTP Response (raw JSON)
         |
         v
RawDataPoint {
    raw_payload: <exact HTTP response>  <-- CORRECT: original bytes
}
         |
         v
   Bronze Storage (Parquet)
         |
         v
   [Later: Silver ETL parses]
```

---

## 1. Algorithm Overview

### High-Level Corrected Data Flow

```
FUNCTION ingest_from_source(source_type, endpoint):
    # Step 1: Fetch raw data (NO PARSING)
    raw_response = fetch_raw_response(endpoint)

    # Step 2: Validate JSON (but do not transform)
    IF not valid_json(raw_response):
        log_error("Invalid JSON from source")
        RETURN error

    # Step 3: Construct RawDataPoint with original payload
    raw_point = RawDataPoint {
        timestamp:    now(),
        source_id:    generate_source_id(stream_id, source_type),
        ndp_id:       source_config.ndp_id,       # from YAML config
        context:      source_config.context,      # from YAML config
        raw_payload:  raw_response                # EXACT original JSON
    }

    # Step 4: Send to Bronze storage via channel
    send_to_ingestion_channel(raw_point)

    # NOTE: No parser invoked. Parsing deferred to Silver ETL.
```

### Key Principle

The source layer MUST NOT parse the response body into domain types.
Parsing is the responsibility of the Silver layer ETL process.

---

## 2. HTTP Polling Source Pseudocode

### GenericHttpPollingSource (External APIs: NWS, OpenWeatherMap, etc.)

**File**: `apps/air-quality-app/src/coordinator/source_manager.rs`
**Function**: `run_generic_http_polling_source()`

```
FUNCTION run_generic_http_polling_source(
    stream_id: String,
    source_id: String,
    config: GenericHttpPollingConfig,
    ingestion_sender: Sender<RawDataPoint>,
    cancel_token: CancellationToken,
    ndp_id: Option<String>,
    context: Option<JSON>
):
    # Initialize HTTP client (timeout, retries)
    client = create_http_client(config.timeout)

    # Polling loop
    LOOP until cancel_token.cancelled():
        FOR each endpoint IN config.endpoints:
            IF not endpoint.enabled:
                CONTINUE

            # Step 1: Build authenticated request
            request = build_request(endpoint)

            # Step 2: Fetch response (with retries)
            response = TRY:
                send_with_retries(request, config.retry_config)
            CATCH error:
                log_warn("Failed to fetch from {endpoint_id}: {error}")
                CONTINUE

            # Step 3: Extract response body as text
            response_body = response.text()

            # Step 4: Validate JSON (do NOT parse into domain types)
            raw_json = TRY:
                parse_json(response_body)  # Validate only
            CATCH error:
                log_warn("Invalid JSON from {endpoint_id}: {error}")
                CONTINUE

            # Step 5: Construct RawDataPoint
            raw_point = RawDataPoint::new(source_id, raw_json)
                .with_timestamp(now())
                .with_ndp_id_opt(ndp_id)
                .with_context_opt(context)

            # Step 6: Send to Bronze layer
            send(ingestion_sender, raw_point)

        # Wait for next poll interval
        sleep(config.poll_interval)
```

### HttpPollingSource (AirGradient Sensors)

**File**: `apps/air-quality-app/src/coordinator/source_manager.rs`
**Function**: `run_http_polling_source()`

```
FUNCTION run_http_polling_source(
    stream_id: String,
    source_id: String,
    config: HttpPollingConfig,
    ingestion_sender: Sender<RawDataPoint>,
    cancel_token: CancellationToken,
    ndp_id: Option<String>,
    context: Option<JSON>
):
    # Initialize HTTP client
    client = create_http_client(config.timeout)

    # Polling loop
    LOOP until cancel_token.cancelled():
        FOR each sensor IN config.sensors:
            # Step 1: Fetch from sensor endpoint
            response = TRY:
                client.get(sensor.url).send()
            CATCH error:
                log_warn("Failed to poll {sensor.serial_number}: {error}")
                CONTINUE

            IF not response.status.is_success():
                log_warn("HTTP {status} from {sensor.serial_number}")
                CONTINUE

            # Step 2: Get raw response body
            response_body = response.text()

            # Step 3: Validate JSON structure
            raw_json = TRY:
                parse_json(response_body)
            CATCH error:
                log_warn("Invalid JSON from {sensor.serial_number}: {error}")
                CONTINUE

            # Step 4: Construct RawDataPoint (NO PARSING)
            raw_point = RawDataPoint::new(source_id, raw_json)
                .with_timestamp(now())
                .with_ndp_id_opt(ndp_id)
                .with_context_opt(context)

            # Step 5: Send to ingestion channel
            send(ingestion_sender, raw_point)

        sleep(config.poll_interval)
```

---

## 3. MQTT Source Pseudocode

**File**: `apps/air-quality-app/src/coordinator/source_manager.rs`
**Function**: `run_mqtt_source()`

```
FUNCTION run_mqtt_source(
    stream_id: String,
    source_id: String,
    config: MqttConfig,
    ingestion_sender: Sender<RawDataPoint>,
    cancel_token: CancellationToken,
    ndp_id: Option<String>,
    context: Option<JSON>
):
    # Connect to MQTT broker
    mqtt_client = connect_mqtt(config.broker_url, config.port, config.client_id)
    subscribe(mqtt_client, config.topic_pattern)

    # Message processing loop
    LOOP until cancel_token.cancelled():
        message = TRY:
            receive_message(mqtt_client)  # blocking with timeout
        CATCH timeout:
            CONTINUE
        CATCH error:
            log_warn("MQTT receive error: {error}")
            attempt_reconnect(mqtt_client, config)
            CONTINUE

        # Step 1: Extract raw payload bytes
        raw_bytes = message.payload()

        # Step 2: Validate as JSON
        raw_json = TRY:
            parse_json(raw_bytes)
        CATCH error:
            log_warn("Invalid JSON from MQTT topic {topic}: {error}")
            CONTINUE

        # Step 3: Construct RawDataPoint (NO PARSING)
        raw_point = RawDataPoint::new(source_id, raw_json)
            .with_timestamp(now())
            .with_ndp_id_opt(ndp_id)
            .with_context_opt(context)

        # Step 4: Send to ingestion channel
        send(ingestion_sender, raw_point)
```

---

## 4. RawDataPoint Construction

### Correct Field Population

```
STRUCT RawDataPoint:
    timestamp:    DateTime<Utc>    # When NDP received the message
    source_id:    String           # "{stream_id}-{source_type}" e.g., "air-quality-Http"
    ndp_id:       Option<String>   # From config: source_config.ndp_id
    context:      Option<JSON>     # From config: source_config.context
    raw_payload:  JSON             # EXACT response from source API/device

FUNCTION construct_raw_point(
    source_id: String,
    raw_response: JSON,      # This is the ORIGINAL HTTP/MQTT payload
    ndp_id: Option<String>,
    context: Option<JSON>
) -> RawDataPoint:

    RETURN RawDataPoint {
        timestamp:   Utc::now(),
        source_id:   source_id,
        ndp_id:      ndp_id,
        context:     context,
        raw_payload: raw_response   # <-- KEY: not reconstructed from parsed fields
    }
```

### What raw_payload Should Contain

**AirGradient Sensor Example**:
```json
{
    "wifi": -50,
    "serialno": "abc123",
    "rco2": 520,
    "pm01": 3,
    "pm02": 4,
    "pm10": 5,
    "atmp": 23.5,
    "rhum": 55,
    "firmware": "3.1.1",
    "model": "I-9PSL"
}
```

**OpenWeatherMap Example**:
```json
{
    "coord": {"lon": -122.09, "lat": 37.39},
    "weather": [{"id": 800, "main": "Clear", "description": "clear sky"}],
    "main": {"temp": 290.88, "feels_like": 290.16, "humidity": 57},
    "wind": {"speed": 3.6, "deg": 350}
}
```

**WRONG raw_payload (what current bug produces)**:
```json
{
    "value": 4.0,
    "location_id": "abc123",
    "tags": {"serialno": "abc123"}
}
```

---

## 5. Simplification Opportunities

### Code That Can Be Removed/Simplified

#### A. Parser Creation in Source Managers

**Current code creates parsers that won't be used in Bronze path**:

```rust
// In run_http_polling_source() - REMOVE THIS:
let parser_config = ParserConfig {
    parser_type: ParserType::FlatJson,
    location_id_field: "serialno".to_string(),
    // ... parser configuration
};
let parser = create_parser_from_config(parser_config);
```

**After fix**: Parser creation moves to Silver ETL pipeline, not source layer.

#### B. TimeSeriesPoint Conversion

**Current code converts parsed points back to JSON**:

```rust
// REMOVE THIS ENTIRE BLOCK:
for point in points {
    let raw_point = RawDataPoint::new(
        &source_id,
        serde_json::json!({
            "value": point.value,
            "location_id": point.location_id,
            "tags": point.tags,
        }),
    )
    // ...
}
```

**After fix**: Raw response goes directly to RawDataPoint.

#### C. Source Trait Changes

The `Source` trait method `fetch() -> Vec<TimeSeriesPoint>` is no longer needed in the Bronze ingestion path. Sources should implement `RawSource::fetch_raw()` instead.

**Note**: Keep `Source` trait for backward compatibility with any code that needs parsed data inline (testing, development dashboards).

#### D. Parser Parameter in GenericHttpPollingSource

**Current signature**:
```rust
pub fn run_generic_http_polling_source(
    // ...
    parser_config: ParserConfig,   // <-- Not needed for Bronze
    // ...
)
```

**After fix**: Remove `parser_config` parameter from Bronze ingestion path.

---

## 6. Implementation Checklist

### Phase 1: Minimal Fix

1. [ ] Modify `run_http_polling_source()`:
   - Remove parser creation
   - Fetch raw HTTP response directly
   - Create RawDataPoint with original JSON

2. [ ] Modify `run_generic_http_polling_source()`:
   - Remove parser invocation
   - Store raw response in raw_payload

3. [ ] Modify `run_mqtt_source()`:
   - Remove parser creation
   - Store MQTT payload directly

### Phase 2: Cleanup

4. [ ] Remove unused parser imports in source_manager.rs
5. [ ] Remove `parser_config` parameter from source runner functions
6. [ ] Update tests to verify raw payloads match source responses

### Phase 3: Verification

7. [ ] Add integration test: HTTP response == Bronze raw_payload
8. [ ] Add integration test: MQTT message == Bronze raw_payload
9. [ ] Verify Parquet files contain expected raw JSON

---

## 7. Test Verification Pseudocode

```
TEST raw_payload_matches_http_response():
    # Setup: Mock HTTP server that returns known JSON
    mock_response = {
        "pm02": 12.5,
        "rco2": 450,
        "serialno": "test123",
        "firmware": "3.1.1"
    }
    mock_server = start_mock_http_server(returns: mock_response)

    # Act: Run HTTP polling source
    (tx, rx) = channel()
    run_http_polling_source(
        stream_id: "test-stream",
        config: { endpoint: mock_server.url },
        ingestion_sender: tx
    )

    # Assert: Received RawDataPoint contains EXACT response
    raw_point = rx.receive()
    ASSERT raw_point.raw_payload == mock_response  # Exact match
    ASSERT raw_point.raw_payload["firmware"] == "3.1.1"  # All fields present
    ASSERT raw_point.raw_payload["serialno"] == "test123"

TEST raw_payload_not_transformed():
    # Verify we're NOT seeing the wrong format
    raw_point = receive_from_ingestion_channel()

    # These fields should NOT exist (they're artifacts of parsing)
    ASSERT "value" NOT IN raw_point.raw_payload
    ASSERT "location_id" NOT IN raw_point.raw_payload

    # These fields SHOULD exist (original source fields)
    ASSERT "pm02" IN raw_point.raw_payload
    ASSERT "rco2" IN raw_point.raw_payload
```

---

## References

- [ADR-001: Bronze Layer Raw JSON Schema](../architecture/ADR-001-bronze-raw-json-schema.md)
- [RawDataPoint struct](../../../../core/src/types/raw_data_point.rs)
- [Source Manager](../../../../apps/air-quality-app/src/coordinator/source_manager.rs)
- [GenericHttpPollingSource](../../../../core/src/sources/http_poll.rs)
