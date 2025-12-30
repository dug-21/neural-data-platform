# Connection Manager Algorithm - DP-003

## Overview

The Connection Manager handles MQTT broker connections, including subscription management for multiple topic patterns, reconnection with exponential backoff, and health monitoring.

## Data Structures

```
STRUCTURE ConnectionState:
    status: ConnectionStatus    // DISCONNECTED, CONNECTING, CONNECTED, RECONNECTING
    last_connected: DateTime    // Last successful connection time
    last_error: Optional<String>  // Last error message
    reconnect_attempts: Integer // Current reconnect attempt count

ENUMERATION ConnectionStatus:
    DISCONNECTED = 0
    CONNECTING = 1
    CONNECTED = 2
    RECONNECTING = 3

STRUCTURE MqttConnection:
    config: MqttConfig
    client: AsyncMqttClient
    event_loop: EventLoop
    state: ConnectionState
    subscriptions: Array<String>  // Active topic patterns

STRUCTURE ReconnectPolicy:
    initial_delay_ms: Integer     // Starting delay (default: 1000)
    max_delay_ms: Integer         // Maximum delay (default: 30000)
    multiplier: Float             // Backoff multiplier (default: 2.0)
    jitter_factor: Float          // Jitter percentage (default: 0.25)
    max_attempts: Integer         // Max attempts before giving up (0 = infinite)
```

---

## Algorithm 1: Connection Initialization

Creates and initializes an MQTT connection.

### Input/Output

```
INPUT:
    config: MqttConfig

OUTPUT:
    MqttConnection OR Error
```

### Algorithm

```
ALGORITHM: initialize_connection
INPUT: config (MqttConfig)
OUTPUT: MqttConnection OR Error

BEGIN
    // Step 1: Validate configuration
    validation <- validate_connection_config(config)
    IF validation is Error THEN
        RETURN validation
    END IF

    // Step 2: Create MQTT options
    mqtt_options <- MqttOptions {
        client_id: config.client_id,
        broker: config.broker_url,
        port: config.port,
        keep_alive: 30_seconds,
        clean_session: true
    }

    // Step 3: Create client and event loop
    TRY
        (client, event_loop) <- create_async_client(mqtt_options, config.buffer_capacity)
    CATCH creation_error
        RETURN Error("Failed to create MQTT client: {creation_error}")
    END TRY

    // Step 4: Initialize state
    state <- ConnectionState {
        status: DISCONNECTED,
        last_connected: None,
        last_error: None,
        reconnect_attempts: 0
    }

    // Step 5: Get subscription patterns from router
    subscriptions <- get_subscription_patterns(config)

    log_info("Initialized MQTT connection: broker={config.broker_url}:{config.port}, client_id={config.client_id}")
    log_info("Configured {length(subscriptions)} subscription patterns")

    RETURN MqttConnection {
        config: config,
        client: client,
        event_loop: event_loop,
        state: state,
        subscriptions: subscriptions
    }
END

FUNCTION: get_subscription_patterns
INPUT: config (MqttConfig)
OUTPUT: Array<String>

BEGIN
    patterns <- empty array
    subscriptions <- config.get_subscriptions()

    FOR EACH sub IN subscriptions DO
        IF sub.enabled THEN
            patterns.append(sub.topic_pattern)
        END IF
    END FOR

    RETURN patterns
END
```

---

## Algorithm 2: Connect and Subscribe

Establishes connection and subscribes to all configured topics.

### Input/Output

```
INPUT:
    connection: MqttConnection

OUTPUT:
    Success OR Error
```

### Algorithm

```
ALGORITHM: connect_and_subscribe
INPUT: connection (MqttConnection)
OUTPUT: Success OR Error

BEGIN
    // Step 1: Update state
    connection.state.status <- CONNECTING
    log_info("Connecting to MQTT broker...")

    // Step 2: Start event loop polling (background task)
    // The actual TCP connection happens during first poll

    // Step 3: Wait for ConnAck
    timeout <- 10_seconds
    start_time <- current_time()

    WHILE (current_time() - start_time) < timeout DO
        event <- connection.event_loop.poll_timeout(1_second)

        IF event is ConnAck THEN
            // Connection successful
            connection.state.status <- CONNECTED
            connection.state.last_connected <- current_utc_time()
            connection.state.reconnect_attempts <- 0
            connection.state.last_error <- None

            log_info("Connected to MQTT broker")
            BREAK

        ELSE IF event is Error THEN
            connection.state.last_error <- event.message
            RETURN Error("Connection failed: {event.message}")
        END IF
    END WHILE

    IF connection.state.status != CONNECTED THEN
        RETURN Error("Connection timeout after {timeout}")
    END IF

    // Step 4: Subscribe to all topics
    result <- subscribe_all(connection)
    IF result is Error THEN
        RETURN result
    END IF

    RETURN Success
END
```

---

## Algorithm 3: Subscribe to All Topics

Subscribes to all configured topic patterns.

### Input/Output

```
INPUT:
    connection: MqttConnection

OUTPUT:
    Success OR Error
```

### Algorithm

```
ALGORITHM: subscribe_all
INPUT: connection (MqttConnection)
OUTPUT: Success OR Error

BEGIN
    qos <- connection.config.qos
    subscribed_count <- 0

    FOR EACH pattern IN connection.subscriptions DO
        TRY
            // Subscribe to pattern
            connection.client.subscribe(pattern, qos)

            log_info("Subscribed to topic pattern: {pattern}")
            subscribed_count <- subscribed_count + 1

        CATCH subscribe_error
            log_error("Failed to subscribe to {pattern}: {subscribe_error}")

            // Decide whether to continue or fail
            IF is_critical_subscription(pattern) THEN
                RETURN Error("Critical subscription failed: {pattern}")
            END IF
            // For non-critical, log and continue
        END TRY
    END FOR

    IF subscribed_count == 0 THEN
        RETURN Error("No subscriptions were successful")
    END IF

    log_info("Subscribed to {subscribed_count}/{length(connection.subscriptions)} topic patterns")

    RETURN Success
END

FUNCTION: is_critical_subscription
INPUT: pattern (String)
OUTPUT: Boolean

BEGIN
    // All subscriptions are critical by default
    // Could be extended to mark some as optional
    RETURN true
END
```

---

## Algorithm 4: Event Loop Processing

Main event loop for handling MQTT events.

### Input/Output

```
INPUT:
    connection: MqttConnection
    message_handler: Function(topic, payload) -> Result
    is_running: AtomicBoolean

OUTPUT:
    None (runs until stopped)
```

### Algorithm

```
ALGORITHM: run_event_loop
INPUT: connection (MqttConnection), message_handler (Function), is_running (AtomicBoolean)
OUTPUT: None

BEGIN
    reconnect_policy <- ReconnectPolicy {
        initial_delay_ms: connection.config.reconnect_delay_secs * 1000,
        max_delay_ms: connection.config.max_reconnect_delay_secs * 1000,
        multiplier: 2.0,
        jitter_factor: 0.25,
        max_attempts: 0  // Infinite retries
    }

    WHILE is_running.load() DO
        TRY
            event <- connection.event_loop.poll()

            MATCH event:
                CASE Incoming(ConnAck):
                    handle_connack(connection)

                CASE Incoming(Publish(message)):
                    handle_publish(message, message_handler)

                CASE Incoming(SubAck(ack)):
                    handle_suback(ack)

                CASE Incoming(Disconnect):
                    handle_disconnect(connection, reconnect_policy, is_running)

                CASE Incoming(PingResp):
                    // Keep-alive acknowledged, connection healthy
                    log_trace("Ping response received")

                CASE Outgoing(_):
                    // Outgoing packets handled automatically
                    PASS

                DEFAULT:
                    log_trace("Unhandled event: {event}")
            END MATCH

        CATCH poll_error
            log_error("Event loop error: {poll_error}")
            handle_connection_error(connection, poll_error, reconnect_policy, is_running)
        END TRY
    END WHILE

    log_info("Event loop stopped")
END
```

---

## Algorithm 5: Reconnection with Backoff

Handles reconnection with exponential backoff.

### Input/Output

```
INPUT:
    connection: MqttConnection
    policy: ReconnectPolicy
    is_running: AtomicBoolean

OUTPUT:
    Success OR Error (if max attempts exceeded)
```

### Algorithm

```
ALGORITHM: reconnect_with_backoff
INPUT: connection (MqttConnection), policy (ReconnectPolicy), is_running (AtomicBoolean)
OUTPUT: Success OR Error

BEGIN
    connection.state.status <- RECONNECTING
    attempt <- connection.state.reconnect_attempts

    WHILE is_running.load() DO
        // Step 1: Check max attempts
        IF policy.max_attempts > 0 AND attempt >= policy.max_attempts THEN
            log_error("Max reconnection attempts ({policy.max_attempts}) exceeded")
            connection.state.status <- DISCONNECTED
            RETURN Error("Max reconnection attempts exceeded")
        END IF

        // Step 2: Calculate delay with backoff
        delay <- calculate_backoff_delay(attempt, policy)

        log_warn("Reconnecting to MQTT broker in {delay}ms (attempt {attempt + 1})")

        // Step 3: Wait before reconnecting
        sleep(delay)

        // Step 4: Check if still running
        IF NOT is_running.load() THEN
            RETURN Error("Reconnection cancelled - shutdown requested")
        END IF

        // Step 5: Create new connection
        TRY
            (new_client, new_event_loop) <- create_async_client(
                connection.config.mqtt_options,
                connection.config.buffer_capacity
            )

            // Replace old client/event_loop
            connection.client <- new_client
            connection.event_loop <- new_event_loop

            // Step 6: Reconnect and resubscribe
            result <- connect_and_subscribe(connection)

            IF result is Success THEN
                log_info("Reconnected successfully after {attempt + 1} attempts")
                RETURN Success
            END IF

        CATCH reconnect_error
            connection.state.last_error <- reconnect_error.message
            log_error("Reconnection attempt {attempt + 1} failed: {reconnect_error}")
        END TRY

        // Step 7: Increment attempt counter
        attempt <- attempt + 1
        connection.state.reconnect_attempts <- attempt
    END WHILE

    RETURN Error("Reconnection cancelled")
END

FUNCTION: calculate_backoff_delay
INPUT: attempt (Integer), policy (ReconnectPolicy)
OUTPUT: Integer (milliseconds)

BEGIN
    // Exponential backoff
    base_delay <- policy.initial_delay_ms * (policy.multiplier ^ attempt)

    // Cap at maximum
    capped_delay <- min(base_delay, policy.max_delay_ms)

    // Add jitter
    jitter_range <- capped_delay * policy.jitter_factor
    jitter <- random(-jitter_range / 2, jitter_range / 2)

    final_delay <- max(0, capped_delay + jitter)

    RETURN round(final_delay)
END
```

### Backoff Examples

| Attempt | Base Delay | With Jitter (25%) |
|---------|------------|-------------------|
| 0 | 1000ms | 750-1250ms |
| 1 | 2000ms | 1500-2500ms |
| 2 | 4000ms | 3000-5000ms |
| 3 | 8000ms | 6000-10000ms |
| 4 | 16000ms | 12000-20000ms |
| 5+ | 30000ms (max) | 22500-37500ms |

---

## Algorithm 6: Handle Disconnect

Handles broker disconnection events.

### Input/Output

```
INPUT:
    connection: MqttConnection
    policy: ReconnectPolicy
    is_running: AtomicBoolean

OUTPUT:
    None (triggers reconnection)
```

### Algorithm

```
ALGORITHM: handle_disconnect
INPUT: connection (MqttConnection), policy (ReconnectPolicy), is_running (AtomicBoolean)
OUTPUT: None

BEGIN
    log_warn("Disconnected from MQTT broker")

    // Step 1: Update state
    connection.state.status <- DISCONNECTED

    // Step 2: Check if intentional shutdown
    IF NOT is_running.load() THEN
        log_info("Disconnect during shutdown - not reconnecting")
        RETURN
    END IF

    // Step 3: Trigger reconnection
    spawn_task(async {
        result <- reconnect_with_backoff(connection, policy, is_running)

        IF result is Error THEN
            log_error("Failed to reconnect: {result.message}")
            // Could trigger alert or graceful shutdown here
        END IF
    })
END
```

---

## Algorithm 7: Graceful Shutdown

Gracefully shuts down the connection.

### Input/Output

```
INPUT:
    connection: MqttConnection
    is_running: AtomicBoolean
    timeout: Duration

OUTPUT:
    Success OR Error (if timeout)
```

### Algorithm

```
ALGORITHM: graceful_shutdown
INPUT: connection (MqttConnection), is_running (AtomicBoolean), timeout (Duration)
OUTPUT: Success OR Error

BEGIN
    log_info("Initiating graceful shutdown...")

    // Step 1: Signal event loop to stop
    is_running.store(false)

    // Step 2: Unsubscribe from all topics
    FOR EACH pattern IN connection.subscriptions DO
        TRY
            connection.client.unsubscribe(pattern)
            log_debug("Unsubscribed from: {pattern}")
        CATCH unsubscribe_error
            log_warn("Failed to unsubscribe from {pattern}: {unsubscribe_error}")
            // Continue with other unsubscriptions
        END TRY
    END FOR

    // Step 3: Disconnect from broker
    TRY
        connection.client.disconnect()
        log_info("Disconnect request sent")
    CATCH disconnect_error
        log_warn("Disconnect error: {disconnect_error}")
    END TRY

    // Step 4: Wait for clean shutdown
    start_time <- current_time()
    WHILE connection.state.status != DISCONNECTED DO
        IF (current_time() - start_time) > timeout THEN
            log_warn("Shutdown timeout - forcing disconnect")
            RETURN Error("Shutdown timeout")
        END IF
        sleep(100_milliseconds)
    END WHILE

    // Step 5: Update final state
    connection.state.status <- DISCONNECTED
    connection.state.last_error <- None

    log_info("Graceful shutdown complete")
    RETURN Success
END
```

---

## Algorithm 8: Health Check

Performs connection health check.

### Input/Output

```
INPUT:
    connection: MqttConnection

OUTPUT:
    HealthStatus
```

### Algorithm

```
ALGORITHM: health_check
INPUT: connection (MqttConnection)
OUTPUT: HealthStatus

BEGIN
    status <- connection.state.status

    MATCH status:
        CASE CONNECTED:
            // Check last activity
            time_since_connect <- current_time() - connection.state.last_connected

            RETURN HealthStatus {
                healthy: true,
                message: "MQTT connection healthy",
                details: {
                    "status": "connected",
                    "broker": "{connection.config.broker_url}:{connection.config.port}",
                    "client_id": connection.config.client_id,
                    "subscriptions": length(connection.subscriptions),
                    "uptime_seconds": time_since_connect.seconds
                }
            }

        CASE CONNECTING:
            RETURN HealthStatus {
                healthy: false,
                message: "MQTT connecting",
                details: {
                    "status": "connecting"
                }
            }

        CASE RECONNECTING:
            RETURN HealthStatus {
                healthy: false,
                message: "MQTT reconnecting",
                details: {
                    "status": "reconnecting",
                    "reconnect_attempts": connection.state.reconnect_attempts,
                    "last_error": connection.state.last_error OR "unknown"
                }
            }

        CASE DISCONNECTED:
            RETURN HealthStatus {
                healthy: false,
                message: "MQTT disconnected",
                details: {
                    "status": "disconnected",
                    "last_error": connection.state.last_error OR "no error"
                }
            }
    END MATCH
END
```

---

## State Machine Diagram

```
                     +-------------+
                     | DISCONNECTED|
                     +------+------+
                            |
                   start()  |
                            v
                     +------+------+
          +--------->| CONNECTING  |<---------+
          |          +------+------+          |
          |                 |                 |
          |        ConnAck  |  Error/Timeout  |
          |                 v                 |
          |          +------+------+          |
          |          |  CONNECTED  |          |
          |          +------+------+          |
          |                 |                 |
          |    Disconnect/  |                 |
          |    Error        |                 |
          |                 v                 |
          |          +------+------+          |
          +----------+ RECONNECTING+----------+
                     +-------------+
                            |
                   max attempts exceeded
                            |
                            v
                     +------+------+
                     | DISCONNECTED|
                     |  (failed)   |
                     +-------------+
```

---

## Thread Safety and Concurrency

```
CONCURRENCY MODEL:

MqttConnection:
    - client: Thread-safe (internal locking)
    - event_loop: Single-threaded (owned by event loop task)
    - state: Protected by Mutex
    - subscriptions: Immutable after init

Access Patterns:
    1. Event loop task: Exclusive access to event_loop
    2. Health check: Read-only access to state (via Mutex)
    3. Shutdown: Write access to is_running (AtomicBoolean)

Synchronization:
    - is_running: AtomicBoolean for lock-free shutdown signaling
    - state: Mutex<ConnectionState> for thread-safe updates
    - No deadlock risk (single lock per operation)
```

---

## Error Handling

| Error Condition | Handling | Recovery |
|-----------------|----------|----------|
| Connection refused | Log, backoff, retry | Exponential backoff |
| DNS resolution failure | Log, backoff, retry | Exponential backoff |
| TLS handshake failure | Log, fail (config issue) | Manual config fix |
| Subscribe rejected | Log, continue if non-critical | Manual verification |
| Keep-alive timeout | Reconnect | Automatic reconnection |
| Broker shutdown | Reconnect | Wait for broker restart |
| Network partition | Reconnect | Exponential backoff |

---

## Metrics

```
METRICS TO EXPOSE:

Counters:
    mqtt_connections_total{status="success|failure"}
    mqtt_reconnections_total
    mqtt_messages_received_total{stream_id}
    mqtt_subscribe_failures_total

Gauges:
    mqtt_connection_status{broker, client_id}  // 0=disconnected, 1=connecting, 2=connected, 3=reconnecting
    mqtt_subscriptions_active{broker}
    mqtt_reconnect_attempts

Histograms:
    mqtt_connection_duration_seconds
    mqtt_reconnect_delay_seconds
```

---

## Complexity Analysis

| Algorithm | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| `initialize_connection` | O(k) | O(k) |
| `connect_and_subscribe` | O(k) | O(1) |
| `subscribe_all` | O(k) | O(1) |
| `run_event_loop` | O(1) per event | O(1) |
| `reconnect_with_backoff` | O(k) per attempt | O(1) |
| `graceful_shutdown` | O(k) | O(1) |
| `health_check` | O(1) | O(1) |

Where k = number of subscription patterns

---

## Test Cases

```
TEST: successful_connection
    SETUP: Mock MQTT broker accepting connections
    INPUT: Valid MqttConfig
    EXPECTED:
        - Connection state becomes CONNECTED
        - All subscriptions successful
        - Health check returns healthy

TEST: connection_failure_triggers_reconnect
    SETUP: Broker unavailable
    INPUT: Valid MqttConfig
    EXPECTED:
        - Connection fails
        - State becomes RECONNECTING
        - Exponential backoff applied
        - Reconnection attempts increment

TEST: successful_reconnection_after_disconnect
    SETUP: Broker disconnects then becomes available
    INPUT: Active connection
    EXPECTED:
        - Disconnect detected
        - Reconnection triggered
        - Subscriptions restored
        - State returns to CONNECTED

TEST: graceful_shutdown
    SETUP: Active connection
    INPUT: Shutdown signal
    EXPECTED:
        - Unsubscribe from all topics
        - Disconnect cleanly
        - State becomes DISCONNECTED
        - Event loop exits

TEST: backoff_calculation
    INPUT: Various attempt numbers
    EXPECTED:
        - Attempt 0: ~1000ms
        - Attempt 1: ~2000ms
        - Attempt 5+: capped at 30000ms
        - All values include jitter

TEST: subscribe_to_multiple_patterns
    SETUP: Connected to broker
    INPUT: Config with 3 subscription patterns
    EXPECTED:
        - All 3 patterns subscribed
        - Logs show successful subscriptions
```

---

## Related Documents

- TOPIC_ROUTER.md: Topic pattern management
- CONFIG_PARSER.md: Configuration loading
- MESSAGE_PROCESSOR.md: Message handling after receipt
- ADR-001-MQTT-SUBSCRIPTIONS.md: Architecture decision
