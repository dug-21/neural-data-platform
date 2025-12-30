# Message Processor Algorithm - DP-003

## Overview

The message processor handles the complete flow of receiving MQTT messages, routing them to the correct stream, parsing the payload, and sending the data for storage.

## Data Structures

```
STRUCTURE MqttMessage:
    topic: String           // MQTT topic (e.g., "airgradient/readings/ABC123")
    payload: ByteArray      // Raw message payload
    qos: Integer            // Quality of Service level
    retain: Boolean         // Retain flag
    timestamp: DateTime     // Receive timestamp

STRUCTURE ProcessedMessage:
    stream_id: String       // Target stream
    topic: String           // Original topic
    points: Array<TimeSeriesPoint>  // Parsed data points
    metadata: Map<String, String>   // Additional metadata

STRUCTURE DeadLetterItem:
    topic: String           // Original topic
    payload: ByteArray      // Raw payload
    error: String           // Error description
    timestamp: DateTime     // When error occurred
    retry_count: Integer    // Number of retries

STRUCTURE TimeSeriesPoint:
    timestamp: DateTime     // Point timestamp
    location_id: String     // Location/device identifier
    value: Float64          // Metric value
    tags: Map<String, String>  // Additional tags (metric, source, stream_id)
```

---

## Algorithm 1: Message Processing Pipeline

Main algorithm for processing incoming MQTT messages.

### Input/Output

```
INPUT:
    message: MqttMessage
    router: TopicRouter

OUTPUT:
    ProcessedMessage OR DeadLetterItem
```

### Algorithm

```
ALGORITHM: process_message
INPUT: message (MqttMessage), router (TopicRouter)
OUTPUT: ProcessedMessage OR DeadLetterItem

BEGIN
    // Step 1: Record receive time
    receive_time <- current_utc_time()

    // Step 2: Route message to stream
    route <- router.route(message.topic)

    IF route is None THEN
        // No matching subscription - dead letter
        log_warn("No route found for topic: {message.topic}")
        RETURN DeadLetterItem {
            topic: message.topic,
            payload: message.payload,
            error: "No matching subscription pattern",
            timestamp: receive_time,
            retry_count: 0
        }
    END IF

    log_debug("Routed topic '{message.topic}' to stream '{route.stream_id}'")

    // Step 3: Parse payload
    parse_result <- parse_payload(message.payload, route.parser, receive_time)

    IF parse_result is Error THEN
        log_error("Failed to parse payload for topic {message.topic}: {parse_result.message}")
        RETURN DeadLetterItem {
            topic: message.topic,
            payload: message.payload,
            error: "Parse error: {parse_result.message}",
            timestamp: receive_time,
            retry_count: 0
        }
    END IF

    points <- parse_result

    // Step 4: Enrich points with stream metadata
    enriched_points <- enrich_points(points, route.stream_id, message.topic)

    // Step 5: Return processed message
    RETURN ProcessedMessage {
        stream_id: route.stream_id,
        topic: message.topic,
        points: enriched_points,
        metadata: {
            "source": "mqtt",
            "qos": to_string(message.qos),
            "retain": to_string(message.retain)
        }
    }
END
```

---

## Algorithm 2: Payload Parsing

Parses MQTT payload using the appropriate parser.

### Input/Output

```
INPUT:
    payload: ByteArray
    parser: Parser
    timestamp: DateTime

OUTPUT:
    Array<TimeSeriesPoint> OR Error
```

### Algorithm

```
ALGORITHM: parse_payload
INPUT: payload (ByteArray), parser (Parser), timestamp (DateTime)
OUTPUT: Array<TimeSeriesPoint> OR Error

BEGIN
    // Step 1: Attempt JSON deserialization
    TRY
        json_value <- json_parse(payload)
    CATCH json_error
        RETURN Error("Invalid JSON: {json_error}")
    END TRY

    // Step 2: Validate JSON structure
    IF json_value is not Object AND json_value is not Array THEN
        RETURN Error("Expected JSON object or array, got: {type_of(json_value)}")
    END IF

    // Step 3: Call parser
    TRY
        points <- parser.parse(json_value, timestamp)
    CATCH parse_error
        RETURN Error("Parser error: {parse_error}")
    END TRY

    // Step 4: Validate points
    IF points is empty THEN
        log_warn("Parser returned no points for payload")
        // Return empty array, not error - some payloads may be legitimately empty
    END IF

    FOR EACH point IN points DO
        IF point.location_id is empty THEN
            log_warn("Point missing location_id, using 'unknown'")
            point.location_id <- "unknown"
        END IF
    END FOR

    RETURN points
END
```

---

## Algorithm 3: Point Enrichment

Enriches parsed points with stream and routing metadata.

### Input/Output

```
INPUT:
    points: Array<TimeSeriesPoint>
    stream_id: String
    topic: String

OUTPUT:
    Array<TimeSeriesPoint>  // Enriched points
```

### Algorithm

```
ALGORITHM: enrich_points
INPUT: points (Array<TimeSeriesPoint>), stream_id (String), topic (String)
OUTPUT: Array<TimeSeriesPoint>

BEGIN
    enriched <- empty array

    FOR EACH point IN points DO
        // Step 1: Clone point to avoid mutation
        enriched_point <- clone(point)

        // Step 2: Add stream_id tag
        enriched_point.tags["stream_id"] <- stream_id

        // Step 3: Add topic tag (useful for debugging)
        enriched_point.tags["topic"] <- topic

        // Step 4: Ensure source tag exists
        IF "source" not in enriched_point.tags THEN
            enriched_point.tags["source"] <- "mqtt"
        END IF

        // Step 5: Add to result
        enriched.append(enriched_point)
    END FOR

    RETURN enriched
END
```

---

## Algorithm 4: Batch Processing

Processes multiple messages in a batch for efficiency.

### Input/Output

```
INPUT:
    messages: Array<MqttMessage>
    router: TopicRouter
    batch_size: Integer (default: 100)

OUTPUT:
    BatchResult:
        processed: Array<ProcessedMessage>
        dead_letters: Array<DeadLetterItem>
        stats: BatchStats
```

### Algorithm

```
ALGORITHM: process_batch
INPUT: messages (Array<MqttMessage>), router (TopicRouter), batch_size (Integer)
OUTPUT: BatchResult

BEGIN
    processed <- empty array
    dead_letters <- empty array

    // Stats tracking
    start_time <- current_time()
    messages_by_stream <- empty map (stream_id -> count)

    // Process in batches to manage memory
    batches <- chunk(messages, batch_size)

    FOR EACH batch IN batches DO
        FOR EACH message IN batch DO
            result <- process_message(message, router)

            IF result is ProcessedMessage THEN
                processed.append(result)
                messages_by_stream[result.stream_id] <-
                    messages_by_stream[result.stream_id] + length(result.points)
            ELSE  // DeadLetterItem
                dead_letters.append(result)
            END IF
        END FOR

        // Yield to allow other tasks (if async)
        yield_if_needed()
    END FOR

    // Calculate stats
    end_time <- current_time()

    RETURN BatchResult {
        processed: processed,
        dead_letters: dead_letters,
        stats: BatchStats {
            total_messages: length(messages),
            successful: length(processed),
            failed: length(dead_letters),
            total_points: sum(length(m.points) for m in processed),
            messages_by_stream: messages_by_stream,
            processing_time_ms: (end_time - start_time).milliseconds
        }
    }
END
```

---

## Algorithm 5: Dead Letter Handling

Handles messages that failed processing.

### Input/Output

```
INPUT:
    dead_letter: DeadLetterItem
    dlq_channel: Channel
    max_retries: Integer (default: 3)

OUTPUT:
    RetryDecision: (retry: Boolean, delay: Duration)
```

### Algorithm

```
ALGORITHM: handle_dead_letter
INPUT: dead_letter (DeadLetterItem), dlq_channel (Channel), max_retries (Integer)
OUTPUT: RetryDecision

BEGIN
    // Step 1: Log the dead letter
    log_warn("Dead letter: topic={dead_letter.topic}, error={dead_letter.error}, retry={dead_letter.retry_count}")

    // Step 2: Determine if retryable
    is_retryable <- is_retryable_error(dead_letter.error)

    // Step 3: Check retry limit
    IF is_retryable AND dead_letter.retry_count < max_retries THEN
        // Calculate backoff delay
        delay <- calculate_backoff(dead_letter.retry_count)

        log_info("Will retry dead letter in {delay}ms, attempt {dead_letter.retry_count + 1}/{max_retries}")

        RETURN RetryDecision {
            retry: true,
            delay: delay
        }
    END IF

    // Step 4: Send to dead letter queue for manual inspection
    dlq_entry <- DeadLetterQueueEntry {
        item: dead_letter,
        final_error: dead_letter.error,
        exhausted_retries: dead_letter.retry_count >= max_retries,
        created_at: current_utc_time()
    }

    TRY
        dlq_channel.send(dlq_entry)
        log_info("Dead letter sent to DLQ: topic={dead_letter.topic}")
    CATCH send_error
        log_error("Failed to send to DLQ: {send_error}")
        // DLQ overflow - log payload for debugging
        log_error("Dropped dead letter payload: {truncate(dead_letter.payload, 1000)}")
    END TRY

    RETURN RetryDecision {
        retry: false,
        delay: 0
    }
END

FUNCTION: is_retryable_error
INPUT: error_message (String)
OUTPUT: Boolean

BEGIN
    // Retryable errors
    retryable_patterns <- [
        "connection",
        "timeout",
        "temporary",
        "unavailable",
        "rate limit"
    ]

    // Non-retryable errors
    non_retryable_patterns <- [
        "invalid json",
        "parse error",
        "no matching subscription",
        "invalid payload"
    ]

    error_lower <- lowercase(error_message)

    FOR EACH pattern IN non_retryable_patterns DO
        IF error_lower contains pattern THEN
            RETURN false
        END IF
    END FOR

    FOR EACH pattern IN retryable_patterns DO
        IF error_lower contains pattern THEN
            RETURN true
        END IF
    END FOR

    // Default: not retryable
    RETURN false
END

FUNCTION: calculate_backoff
INPUT: retry_count (Integer)
OUTPUT: Duration (milliseconds)

BEGIN
    base_delay <- 100  // 100ms base
    max_delay <- 10000 // 10s max

    // Exponential backoff with jitter
    delay <- base_delay * (2 ^ retry_count)
    delay <- min(delay, max_delay)

    // Add jitter (0-25% of delay)
    jitter <- random(0, delay * 0.25)
    delay <- delay + jitter

    RETURN delay
END
```

---

## Algorithm 6: Send to Storage

Sends processed points to the storage layer.

### Input/Output

```
INPUT:
    processed: ProcessedMessage
    ingestion_channel: Channel<TimeSeriesPoint>

OUTPUT:
    SendResult: (success: Boolean, sent_count: Integer, errors: Array<Error>)
```

### Algorithm

```
ALGORITHM: send_to_storage
INPUT: processed (ProcessedMessage), ingestion_channel (Channel)
OUTPUT: SendResult

BEGIN
    sent_count <- 0
    errors <- empty array

    FOR EACH point IN processed.points DO
        TRY
            // Non-blocking send with timeout
            result <- ingestion_channel.send_timeout(point, 5_seconds)

            IF result is Timeout THEN
                log_warn("Ingestion channel full, applying backpressure")
                // Wait and retry once
                sleep(100_milliseconds)
                result <- ingestion_channel.send_timeout(point, 10_seconds)

                IF result is Timeout THEN
                    errors.append(Error("Channel timeout for point at {point.timestamp}"))
                    CONTINUE
                END IF
            END IF

            sent_count <- sent_count + 1

        CATCH send_error
            errors.append(send_error)
            log_error("Failed to send point: {send_error}")
        END TRY
    END FOR

    // Log results
    IF length(errors) > 0 THEN
        log_warn("Sent {sent_count}/{length(processed.points)} points, {length(errors)} errors")
    ELSE
        log_debug("Sent {sent_count} points to stream '{processed.stream_id}'")
    END IF

    RETURN SendResult {
        success: length(errors) == 0,
        sent_count: sent_count,
        errors: errors
    }
END
```

---

## Message Processing Flow Diagram

```
                        +------------------+
                        |  MQTT Message    |
                        |  (topic, payload)|
                        +--------+---------+
                                 |
                                 v
                    +------------+------------+
                    |      route_topic()      |
                    |     (TopicRouter)       |
                    +------------+------------+
                                 |
                   +-------------+-------------+
                   |                           |
                   v                           v
            +------+------+            +------+------+
            | Route Found |            |  No Match   |
            +------+------+            +------+------+
                   |                           |
                   v                           v
           +-------+--------+          +------+------+
           | parse_payload()|          | DeadLetter  |
           | (using parser) |          |   Queue     |
           +-------+--------+          +-------------+
                   |
          +--------+--------+
          |                 |
          v                 v
   +------+------+   +------+------+
   | Parse OK    |   | Parse Error |
   +------+------+   +------+------+
          |                 |
          v                 v
   +------+-------+  +------+------+
   |enrich_points()|  | DeadLetter |
   +------+-------+  +-------------+
          |
          v
   +------+-------+
   |send_to_      |
   |storage()     |
   +------+-------+
          |
          v
   +------+-------+
   | Parquet      |
   | Storage      |
   +--------------+
```

---

## Concurrency Model

```
CONCURRENT ARCHITECTURE:

Main Event Loop (async):
    |
    +-- MQTT Event Handler (async task)
    |       |
    |       +-- on_message(topic, payload)
    |               |
    |               +-- process_message() -> ProcessedMessage
    |               |
    |               +-- send_to_storage() -> ingestion_channel
    |
    +-- Dead Letter Handler (async task)
    |       |
    |       +-- retry_dead_letters()
    |       |
    |       +-- persist_to_dlq()
    |
    +-- Metrics Collector (async task)
            |
            +-- collect_processing_stats()
            |
            +-- expose_metrics()

Channel Buffer Sizes:
    - ingestion_channel: 1000 points
    - dlq_channel: 100 items
    - metrics_channel: 50 samples
```

---

## Error Handling Matrix

| Error Type | Action | Retryable | Logging |
|------------|--------|-----------|---------|
| No route match | Dead letter | No | WARN |
| Invalid JSON | Dead letter | No | ERROR |
| Parser error | Dead letter | No | ERROR |
| Channel full | Backpressure, retry | Yes | WARN |
| Send timeout | Retry with backoff | Yes | WARN |
| Storage error | Retry with backoff | Yes | ERROR |

---

## Performance Considerations

```
PERFORMANCE METRICS:

Target Throughput:
    - Single message processing: < 1ms
    - Batch processing (100 messages): < 50ms
    - Points per second: > 10,000

Memory Usage:
    - Per message buffer: ~1KB average
    - Dead letter queue: 100 items * ~2KB = ~200KB
    - Ingestion channel: 1000 points * ~500B = ~500KB

Optimization Strategies:
    1. Pre-compiled regex in router (done once at startup)
    2. Reuse parser instances (not per-message)
    3. Batch channel sends when possible
    4. Async I/O for storage operations
    5. Bounded queues with backpressure
```

---

## Complexity Analysis

| Algorithm | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| `process_message` | O(k + p) | O(p) |
| `parse_payload` | O(n) | O(n) |
| `enrich_points` | O(p) | O(p) |
| `process_batch` | O(m * (k + p)) | O(m * p) |
| `handle_dead_letter` | O(1) | O(1) |
| `send_to_storage` | O(p) | O(1) |

Where:
- k = number of route patterns
- p = number of points in message
- n = payload size in bytes
- m = number of messages in batch

---

## Test Cases

```
TEST: successful_message_processing
    INPUT:
        - topic: "airgradient/readings/ABC123"
        - payload: {"pm02": 12.5, "serialno": "ABC123"}
        - route exists for pattern
    EXPECTED:
        - ProcessedMessage with stream_id="air-quality"
        - Points have stream_id and topic tags
        - No dead letters

TEST: unmatched_topic_to_dead_letter
    INPUT:
        - topic: "unknown/topic/here"
        - no matching route
    EXPECTED:
        - DeadLetterItem returned
        - Error message: "No matching subscription pattern"

TEST: invalid_json_to_dead_letter
    INPUT:
        - topic: "airgradient/readings/ABC123"
        - payload: "not valid json {"
    EXPECTED:
        - DeadLetterItem returned
        - Error message contains "Invalid JSON"

TEST: batch_processing_with_mixed_results
    INPUT:
        - 10 messages, 8 valid, 2 invalid
    EXPECTED:
        - BatchResult.processed has 8 items
        - BatchResult.dead_letters has 2 items
        - Stats reflect correct counts

TEST: backpressure_handling
    INPUT:
        - Full ingestion channel
        - New message arrives
    EXPECTED:
        - send_to_storage applies backpressure
        - Retry after delay
        - Success or timeout error
```

---

## Related Documents

- TOPIC_ROUTER.md: Topic pattern matching
- CONFIG_PARSER.md: Configuration loading
- CONNECTION_MANAGER.md: MQTT connection handling
