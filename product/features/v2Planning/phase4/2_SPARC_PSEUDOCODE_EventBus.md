# SPARC Pseudocode: EventBus Component
## Algorithm Design and Logic Flow

*Document Version*: 1.0  
*Created*: 2025-01-26  
*Status*: Active Pseudocode  
*Component*: EventBus Abstraction Layer

## 1. Core EventBus Trait Pseudocode

```pseudocode
TRAIT EventBus:
    // Publishing operations
    ASYNC FUNCTION publish(channel: String, event: Event) -> Result<EventId>:
        VALIDATE channel_name_format(channel)
        IF batching_enabled(channel):
            ADD event TO batch_queue
            IF batch_ready():
                FLUSH batch TO backend
            RETURN pending_event_id
        ELSE:
            serialized = serialize_protobuf(event)
            event_id = backend.publish(channel, serialized)
            UPDATE metrics(channel, event_size, latency)
            RETURN event_id

    // Subscription operations  
    ASYNC FUNCTION subscribe(channels: Array<String>, config: SubscriptionConfig) -> Result<Subscriber>:
        FOR EACH channel IN channels:
            VALIDATE channel_exists(channel)
            CREATE_OR_JOIN consumer_group(channel, config.group_name)
        
        subscriber = CREATE Subscriber(channels, config)
        REGISTER subscriber WITH backend
        RETURN subscriber

    // Acknowledgment operations
    ASYNC FUNCTION ack(channel: String, group: String, event_id: EventId) -> Result<()>:
        IF NOT valid_event_id(event_id):
            RETURN Error("Invalid event ID")
        
        backend.acknowledge(channel, group, event_id)
        REMOVE event_id FROM pending_messages
        UPDATE consumer_lag_metrics(channel, group)
        RETURN Ok
```

## 2. RedisEventBus Implementation Pseudocode

```pseudocode
CLASS RedisEventBus IMPLEMENTS EventBus:
    FIELDS:
        redis_client: AsyncRedisConnection
        channel_configs: Map<String, ChannelConfig>
        message_batcher: MessageBatcher
        backpressure_controller: BackpressureController
        metrics_collector: MetricsCollector

    ASYNC FUNCTION publish(channel: String, event: Event) -> Result<EventId>:
        // Check backpressure before publishing
        pressure_status = backpressure_controller.check_pressure(channel)
        IF pressure_status == CRITICAL:
            APPLY rate_limiting(0.25)  // Reduce to 25% rate
            WAIT_OR_REJECT based_on_config()
        
        // Convert to Redis Streams format
        redis_channel = convert_channel_name(channel)  // market:* -> stream:symbol:*
        
        // Serialize event
        protobuf_data = serialize_to_protobuf(event)
        
        // Add to Redis Stream
        TRY:
            event_id = redis_client.XADD(
                redis_channel,
                "*",  // Auto-generate ID
                {"data": protobuf_data, "timestamp": current_time()}
            )
            
            // Track metrics
            metrics_collector.record_publish(channel, SIZE_OF(protobuf_data))
            
            RETURN Ok(event_id)
        CATCH redis_error:
            LOG_ERROR("Redis publish failed", redis_error)
            RETURN Error(redis_error)

    ASYNC FUNCTION subscribe(channels: Array<String>, config: SubscriptionConfig) -> Result<Subscriber>:
        // Create consumer groups if needed
        FOR EACH channel IN channels:
            redis_channel = convert_channel_name(channel)
            TRY:
                redis_client.XGROUP_CREATE(
                    redis_channel,
                    config.group_name,
                    config.start_position  // "0" for beginning, "$" for new only
                )
            CATCH AlreadyExists:
                CONTINUE  // Group already exists, that's fine
        
        // Create Redis subscriber
        subscriber = RedisSubscriber(
            redis_client,
            channels,
            config
        )
        
        RETURN Ok(subscriber)

    FUNCTION convert_channel_name(channel: String) -> String:
        // Migration from old to new format
        IF channel.starts_with("market:"):
            symbol = channel.remove_prefix("market:")
            RETURN "stream:symbol:" + symbol
        ELSE IF channel.starts_with("stream:"):
            RETURN channel  // Already correct format
        ELSE:
            RETURN "stream:" + channel  // Add prefix
```

## 3. InMemoryEventBus Implementation Pseudocode

```pseudocode
CLASS InMemoryEventBus IMPLEMENTS EventBus:
    FIELDS:
        channels: Map<String, Channel>
        consumer_groups: Map<String, ConsumerGroup>
        event_counter: AtomicInteger
        mutex: RwLock

    STRUCTURE Channel:
        events: Queue<EventEnvelope>
        subscribers: List<SubscriberId>
        
    STRUCTURE ConsumerGroup:
        name: String
        members: List<ConsumerId>
        pending_messages: Map<EventId, EventEnvelope>
        last_delivered: EventId

    ASYNC FUNCTION publish(channel: String, event: Event) -> Result<EventId>:
        ACQUIRE write_lock(mutex)
        
        // Create channel if doesn't exist
        IF NOT channels.contains(channel):
            channels[channel] = CREATE Channel()
        
        // Generate event ID
        event_id = "inmem-" + event_counter.increment()
        
        // Create envelope
        envelope = EventEnvelope(
            event_id,
            channel,
            event,
            retry_count: 0
        )
        
        // Add to channel queue
        channels[channel].events.push(envelope)
        
        // Notify subscribers
        FOR EACH subscriber_id IN channels[channel].subscribers:
            NOTIFY subscriber(subscriber_id)
        
        RELEASE write_lock
        RETURN Ok(event_id)

    ASYNC FUNCTION subscribe(channels: Array<String>, config: SubscriptionConfig) -> Result<Subscriber>:
        ACQUIRE write_lock(mutex)
        
        subscriber_id = generate_subscriber_id()
        
        FOR EACH channel IN channels:
            // Create channel if doesn't exist
            IF NOT channels.contains(channel):
                channels[channel] = CREATE Channel()
            
            // Add subscriber to channel
            channels[channel].subscribers.add(subscriber_id)
            
            // Create/join consumer group
            group_key = channel + ":" + config.group_name
            IF NOT consumer_groups.contains(group_key):
                consumer_groups[group_key] = CREATE ConsumerGroup(config.group_name)
            
            consumer_groups[group_key].members.add(subscriber_id)
        
        // Create subscriber instance
        subscriber = InMemorySubscriber(
            subscriber_id,
            channels,
            config,
            self.channels  // Reference to channels map
        )
        
        RELEASE write_lock
        RETURN Ok(subscriber)
```

## 4. RecordingEventBus Implementation Pseudocode

```pseudocode
CLASS RecordingEventBus IMPLEMENTS EventBus:
    FIELDS:
        inner: EventBus  // Wrapped implementation
        recorded_publishes: List<RecordedEvent>
        recorded_subscriptions: List<RecordedSubscription>
        recorded_acks: List<RecordedAck>
        recording_enabled: AtomicBool
        mutex: RwLock

    STRUCTURE RecordedEvent:
        timestamp: Time
        channel: String
        event: Event
        event_id: EventId
        
    STRUCTURE RecordedSubscription:
        timestamp: Time
        channels: Array<String>
        config: SubscriptionConfig
        subscriber_id: String

    ASYNC FUNCTION publish(channel: String, event: Event) -> Result<EventId>:
        // Record the publish attempt
        IF recording_enabled:
            ACQUIRE write_lock(mutex)
            recorded_publishes.add(RecordedEvent(
                timestamp: current_time(),
                channel: channel,
                event: clone(event),
                event_id: pending
            ))
            RELEASE write_lock
        
        // Delegate to inner implementation
        result = inner.publish(channel, event)
        
        // Update recording with actual event_id
        IF recording_enabled AND result.is_ok():
            ACQUIRE write_lock(mutex)
            LAST_ELEMENT(recorded_publishes).event_id = result.value
            RELEASE write_lock
        
        RETURN result

    FUNCTION get_recorded_events() -> List<RecordedEvent>:
        ACQUIRE read_lock(mutex)
        events = clone(recorded_publishes)
        RELEASE read_lock
        RETURN events

    FUNCTION assert_event_published(channel: String, event_type: String) -> Bool:
        recorded = get_recorded_events()
        FOR EACH record IN recorded:
            IF record.channel == channel AND record.event.event_type == event_type:
                RETURN true
        RETURN false

    FUNCTION clear_recordings():
        ACQUIRE write_lock(mutex)
        recorded_publishes.clear()
        recorded_subscriptions.clear()
        recorded_acks.clear()
        RELEASE write_lock
```

## 5. Backpressure Controller Pseudocode

```pseudocode
CLASS BackpressureController:
    FIELDS:
        channel_limits: Map<String, ChannelLimits>
        current_metrics: Map<String, ChannelMetrics>
        throttle_states: Map<String, ThrottleState>

    STRUCTURE ChannelLimits:
        max_pending_messages: Integer
        max_memory_mb: Integer
        max_lag_ms: Integer
        warning_threshold: Float  // 0.0 to 1.0
        critical_threshold: Float

    ASYNC FUNCTION check_pressure(channel: String) -> PressureStatus:
        limits = channel_limits.get(channel, default_limits())
        metrics = measure_channel_metrics(channel)
        
        // Calculate pressure ratios
        message_pressure = metrics.pending_messages / limits.max_pending_messages
        memory_pressure = metrics.memory_mb / limits.max_memory_mb
        lag_pressure = metrics.lag_ms / limits.max_lag_ms
        
        overall_pressure = MAX(message_pressure, memory_pressure, lag_pressure)
        
        IF overall_pressure >= limits.critical_threshold:
            apply_critical_throttling(channel)
            RETURN PressureStatus.CRITICAL
        ELSE IF overall_pressure >= limits.warning_threshold:
            apply_warning_throttling(channel)
            RETURN PressureStatus.WARNING
        ELSE:
            clear_throttling(channel)
            RETURN PressureStatus.NORMAL

    FUNCTION apply_critical_throttling(channel: String):
        state = ThrottleState(
            rate_limit: 0.25,  // 25% of normal
            batch_size: LARGE,
            consumer_scaling: 2.0  // Double consumers
        )
        throttle_states[channel] = state
        
        // Notify producers to slow down
        broadcast_backpressure_signal(channel, CRITICAL)
        
        // Scale up consumers
        scale_consumers(channel, SCALE_UP)

    FUNCTION apply_warning_throttling(channel: String):
        state = ThrottleState(
            rate_limit: 0.75,  // 75% of normal
            batch_size: MEDIUM,
            consumer_scaling: 1.0  // No change
        )
        throttle_states[channel] = state
```

## 6. Message Batcher Pseudocode

```pseudocode
CLASS MessageBatcher:
    FIELDS:
        batch_configs: Map<String, BatchConfig>
        pending_batches: Map<String, PendingBatch>
        flush_timers: Map<String, Timer>

    STRUCTURE BatchConfig:
        max_batch_size: Integer
        max_wait_ms: Integer
        compression: Bool

    STRUCTURE PendingBatch:
        events: List<Event>
        created_at: Time
        size_bytes: Integer

    ASYNC FUNCTION add_to_batch(channel: String, event: Event) -> Option<Batch>:
        config = batch_configs.get(channel, default_config())
        
        // Get or create pending batch
        IF NOT pending_batches.contains(channel):
            pending_batches[channel] = CREATE PendingBatch()
            flush_timers[channel] = START_TIMER(config.max_wait_ms, flush_callback)
        
        batch = pending_batches[channel]
        batch.events.add(event)
        batch.size_bytes += SIZE_OF(event)
        
        // Check if should flush
        should_flush = (
            batch.events.length >= config.max_batch_size OR
            batch.size_bytes >= MAX_BATCH_BYTES OR
            TIME_SINCE(batch.created_at) >= config.max_wait_ms
        )
        
        IF should_flush:
            CANCEL_TIMER(flush_timers[channel])
            REMOVE flush_timers[channel]
            REMOVE pending_batches[channel]
            RETURN Some(batch.events)
        ELSE:
            RETURN None

    FUNCTION flush_callback(channel: String):
        IF pending_batches.contains(channel):
            batch = pending_batches[channel]
            REMOVE pending_batches[channel]
            REMOVE flush_timers[channel]
            publish_batch(channel, batch.events)
```

## 7. Dead Letter Queue Handler Pseudocode

```pseudocode
CLASS DeadLetterQueue:
    FIELDS:
        dlq_config: DLQConfig
        retry_policies: Map<String, RetryPolicy>
        retry_counts: Map<EventId, Integer>
        dlq_channels: Map<String, String>  // channel -> dlq_channel

    STRUCTURE DLQConfig:
        max_retries: Integer
        base_delay_ms: Integer
        backoff_multiplier: Float
        retention_hours: Integer

    ASYNC FUNCTION handle_failed_message(
        channel: String,
        event_id: EventId,
        event: Event,
        error: Error
    ) -> MessageDisposition:
        
        retry_count = retry_counts.get(event_id, 0)
        policy = retry_policies.get(channel, default_policy())
        
        IF retry_count < policy.max_retries AND is_retriable(error):
            // Calculate exponential backoff
            delay_ms = policy.base_delay_ms * POW(policy.backoff_multiplier, retry_count)
            
            // Schedule retry
            schedule_retry(channel, event_id, event, delay_ms)
            
            // Update retry count
            retry_counts[event_id] = retry_count + 1
            
            RETURN MessageDisposition.RETRY(retry_count + 1, delay_ms)
        ELSE:
            // Send to dead letter queue
            dlq_channel = get_dlq_channel(channel)
            
            dlq_event = Event(
                event_type: "DeadLetter",
                payload: serialize({
                    original_event: event,
                    original_channel: channel,
                    error_message: error.to_string(),
                    retry_count: retry_count,
                    failed_at: current_time()
                }),
                metadata: {
                    "original_channel": channel,
                    "final_error": error.type,
                    "retries": retry_count.to_string()
                }
            )
            
            publish(dlq_channel, dlq_event)
            
            // Clean up retry tracking
            REMOVE retry_counts[event_id]
            
            RETURN MessageDisposition.DEAD_LETTER(error, retry_count)

    FUNCTION is_retriable(error: Error) -> Bool:
        RETURN error.type IN [
            TEMPORARY_FAILURE,
            TIMEOUT,
            RATE_LIMIT,
            CONNECTION_ERROR
        ]
```

## 8. Channel Naming Migration Pseudocode

```pseudocode
FUNCTION migrate_channel_name(old_name: String) -> String:
    // Handle legacy naming conventions
    IF old_name.starts_with("market:"):
        // Old format: market:AAPL
        symbol = old_name.split(":")[1]
        RETURN "stream:symbol:" + symbol
    
    ELSE IF old_name.starts_with("sector_"):
        // Old format: sector_technology
        sector = old_name.split("_")[1]
        RETURN "stream:sector:" + sector
    
    ELSE IF old_name.starts_with("ml_"):
        // Old format: ml_training
        operation = old_name.split("_")[1]
        RETURN "stream:ml:" + operation
    
    ELSE IF old_name.starts_with("stream:"):
        // Already migrated
        RETURN old_name
    
    ELSE:
        // Unknown format, add stream prefix
        LOG_WARNING("Unknown channel format", old_name)
        RETURN "stream:unknown:" + old_name

FUNCTION validate_channel_name(name: String) -> Bool:
    // Must follow pattern: stream:domain:identifier
    parts = name.split(":")
    
    IF parts.length != 3:
        RETURN false
    
    IF parts[0] != "stream":
        RETURN false
    
    valid_domains = ["symbol", "sector", "portfolio", "cross_sector", "ml", "action", "dlq"]
    IF parts[1] NOT IN valid_domains:
        RETURN false
    
    IF parts[2].is_empty():
        RETURN false
    
    RETURN true
```

## 9. Testing Harness Pseudocode

```pseudocode
CLASS EventBusTestHarness:
    FIELDS:
        event_bus: EventBus
        test_events: List<Event>
        test_channels: List<String>
        assertions: List<Assertion>

    FUNCTION given_event_bus(implementation: EventBusType) -> Self:
        event_bus = MATCH implementation:
            CASE IN_MEMORY:
                CREATE InMemoryEventBus()
            CASE RECORDING:
                inner = CREATE InMemoryEventBus()
                CREATE RecordingEventBus(inner)
            CASE REDIS:
                CREATE RedisEventBus(test_config())
        
        RETURN EventBusTestHarness(event_bus)

    FUNCTION when_publish_event(channel: String, event: Event) -> Self:
        result = event_bus.publish(channel, event)
        ASSERT result.is_ok()
        test_events.add(event)
        test_channels.add(channel)
        RETURN self

    FUNCTION then_subscriber_receives(expected_events: List<Event>) -> Self:
        subscriber = event_bus.subscribe(
            test_channels,
            SubscriptionConfig(group: "test-group")
        )
        
        received_events = List<Event>()
        
        FOR i IN 0..expected_events.length:
            envelope = subscriber.next()
            ASSERT envelope.is_some()
            received_events.add(envelope.event)
            event_bus.ack(envelope.channel, "test-group", envelope.event_id)
        
        ASSERT received_events == expected_events
        RETURN self

    FUNCTION then_dead_letter_queue_contains(expected_count: Integer) -> Self:
        dlq_channel = "stream:dlq:test"
        dlq_subscriber = event_bus.subscribe(
            [dlq_channel],
            SubscriptionConfig(group: "dlq-test")
        )
        
        dlq_count = 0
        WHILE envelope = dlq_subscriber.next():
            dlq_count += 1
        
        ASSERT dlq_count == expected_count
        RETURN self
```

---

*This pseudocode defines the complete algorithmic design for the EventBus component, providing clear logic flow for all implementations and supporting systems.*