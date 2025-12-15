# AIR-004: Generic Multi-Stream Data Platform - SPARC Pseudocode (Revised)

## Overview

This document provides comprehensive algorithm designs for the Generic Multi-Stream Data Platform, **aligned with existing codebase patterns**. The platform extends the current AIR-002 implementation to support dynamic stream registration, heterogeneous source ingestion, dual-layer storage (Bronze/Silver), and cross-stream analytics.

**Key Principle**: AIR-004 extends existing patterns rather than replacing them. All new components wrap or extend current implementations.

---

## Current Implementation Patterns

This section documents the **actual patterns used in the codebase** that AIR-004 must follow.

### Pattern 1: MQTT Handler with mpsc Channels

**Location**: `/workspaces/neural-data-platform/apps/air-quality-app/src/ingestion/mqtt_handler.rs`

```rust
CURRENT ALGORITHM: MqttHandler Pattern (AIR-002)
INPUT: MqttConfig, mpsc::Sender<TimeSeriesPoint>
OUTPUT: Continuous stream of TimeSeriesPoints

DATA STRUCTURES:
    MqttHandler:
        source: MqttSource           // From neural_core
        sender: mpsc::Sender<Point>  // tokio channel

BEGIN
    // 1. Create MQTT source from config
    source ← MqttSource::new(config)
    source.start().await

    // 2. Run fetch loop
    LOOP
        points ← source.fetch().await  // Batch fetch

        FOR EACH point IN points DO
            sender.send(point).await   // Forward to channel
        END FOR

        sleep(100ms)  // Avoid busy-waiting
    END LOOP
END

TIME COMPLEXITY: O(1) per point
SPACE COMPLEXITY: O(b) where b = buffer capacity (default 1000)
```

**Key Characteristics**:
- Uses `neural_core::MqttSource` trait implementation
- Fetch-based pattern (not callback-based)
- mpsc channel for internal communication
- Health check via `source.health_check()`
- No direct reconnection logic (handled by MqttSource)

### Pattern 2: Storage Writer with Batching

**Location**: `/workspaces/neural-data-platform/apps/air-quality-app/src/pipeline/storage_writer.rs`

```rust
CURRENT ALGORITHM: StorageWriter Pattern (AIR-002)
INPUT: Arc<ParquetStore>, mpsc::Receiver<TimeSeriesPoint>, batch_size, batch_timeout
OUTPUT: Persisted data in Parquet files

DATA STRUCTURES:
    StorageWriter:
        store: Arc<ParquetStore>
        receiver: mpsc::Receiver<Point>
        batch_size: usize           // Default: 100
        batch_timeout: Duration     // Default: 5s

BEGIN
    buffer ← Vec::with_capacity(batch_size)
    flush_interval ← interval(batch_timeout)

    LOOP
        SELECT
            CASE point ← receiver.recv():
                buffer.push(point)

                // Flush on batch size
                IF buffer.len() >= batch_size THEN
                    flush(buffer)
                END IF

            CASE _ ← flush_interval.tick():
                // Flush on timeout
                IF buffer.not_empty() THEN
                    flush(buffer)
                END IF
        END SELECT
    END LOOP
END

SUBROUTINE: flush(buffer)
    store.write_batch(buffer.clone()).await
    buffer.clear()
END

TIME COMPLEXITY: O(1) per point, O(n) per flush
SPACE COMPLEXITY: O(batch_size)
```

**Key Characteristics**:
- `tokio::select!` for concurrent timeout and receive
- Batching with dual triggers (size OR timeout)
- Arc-wrapped store for shared access
- Graceful shutdown on channel close
- Clear buffer after successful flush

### Pattern 3: Config Loading with Hierarchy

**Location**: `/workspaces/neural-data-platform/apps/air-quality-app/src/config_etcd.rs`

```rust
CURRENT ALGORITHM: Config Loading Pattern (AIR-002)
INPUT: none (reads from environment)
OUTPUT: EtcdAppConfig with hierarchy applied

PRIORITY HIERARCHY:
    etcd > env vars > yaml > defaults

BEGIN
    etcd_endpoint ← env("ETCD_ENDPOINT") OR "http://localhost:2379"
    client ← ConfigClient::with_prefix([etcd_endpoint], "/air-quality")

    // Load each field with env override
    server.host ← client.get_with_env("/server/host", "AIR_QUALITY")
                  OR env("AIR_QUALITY_SERVER_HOST")
                  OR "0.0.0.0"

    // Special case: storage base_path has 3 env vars
    storage.base_path ←
        client.get("/storage/base_path")
        OR env("DATA_DIR")
        OR env("STORAGE_PATH")
        OR "./data/parquet"

    RETURN config
END

TIME COMPLEXITY: O(k) where k = number of config keys
SPACE COMPLEXITY: O(1)
```

**Key Characteristics**:
- Uses `config-client` crate with `get_with_env<T>()` helper
- Environment variable prefix (e.g., "AIR_QUALITY_")
- Multiple fallback layers for each field
- Type-safe deserialization with serde
- Special handling for path-based config (DATA_DIR precedence)

### Pattern 4: Parquet Storage with WAL and Partitioning

**Location**: `/workspaces/neural-data-platform/core/src/storage/parquet.rs`

```rust
CURRENT ALGORITHM: ParquetStore Pattern
INPUT: base_path (PathBuf)
OUTPUT: Partitioned Parquet files with WAL

DATA STRUCTURES:
    ParquetStore:
        base_path: PathBuf
        wal: Arc<Mutex<WriteAheadLog>>

PARTITION STRUCTURE:
    {base_path}/data/{location_id}/year={YYYY}/month={MM}/day={DD}/readings.parquet

BEGIN
    // Write batch with WAL
    FUNCTION: write_batch(points)
        // 1. Write to WAL first
        wal.lock().await
        FOR EACH point IN points DO
            entry ← serde_json::to_vec(point)
            wal.append(entry)
        END FOR

        // 2. Group by partition
        grouped ← HashMap<PathBuf, Vec<Point>>
        FOR EACH point IN points DO
            path ← partition_path(point.location_id, point.timestamp)
            grouped[path].push(point)
        END FOR

        // 3. Write each partition
        FOR EACH (path, partition_points) IN grouped DO
            append_to_parquet(partition_points, path)
        END FOR

        // 4. Commit WAL
        wal.commit()
    END FUNCTION

    FUNCTION: partition_path(location_id, timestamp)
        RETURN base_path
            .join("data")
            .join(location_id)
            .join(format!("year={}", timestamp.year()))
            .join(format!("month={:02}", timestamp.month()))
            .join(format!("day={:02}", timestamp.day()))
            .join("readings.parquet")
    END FUNCTION
END

TIME COMPLEXITY: O(n log n) for grouping and sorting
SPACE COMPLEXITY: O(n) for grouped HashMap
```

**Key Characteristics**:
- WAL-first pattern (write-ahead, then commit)
- Partition by date (year/month/day) and location_id
- Polars DataFrame for Parquet operations
- SNAPPY compression
- Read-modify-write for appends (loads existing file)
- Health check verifies path writability

---

## 1. Stream Registry Operations (EXTENDS config-client)

### 1.1 Registry Initialization (Wraps ConfigClient)

```
ALGORITHM: InitializeStreamRegistry
INPUT: etcdEndpoints (array<string>), watchCallback (function)
OUTPUT: registryHandle (RegistryHandle)

EXTENDS: config-client crate pattern from config_etcd.rs

DATA STRUCTURES:
    StreamRegistry:
        Type: HashMap<StreamId, StreamConfig>
        Purpose: In-memory cache of stream configurations
        Operations:
            - get(streamId): O(1)
            - insert(streamId, config): O(1)
            - remove(streamId): O(1)

    RegistryHandle:
        client: ConfigClient            // Reuse config-client
        registry: Arc<RwLock<HashMap>>  // Thread-safe cache
        watch_handle: WatchHandle
        prefix: String                  // e.g., "/streams"

BEGIN
    // Initialize config client (same pattern as config_etcd.rs)
    client ← ConfigClient::with_prefix(etcdEndpoints, "/streams").await
    registry ← Arc::new(RwLock::new(HashMap::new()))

    // Load existing streams from etcd using ConfigClient pattern
    stream_ids ← client.get_keys_under("/").await  // Get all stream IDs

    FOR EACH stream_id IN stream_ids DO
        // Load config using get_with_env (allows env overrides)
        config ← client.get_with_env<StreamConfig>(
            "/" + stream_id + "/config",
            "STREAM_" + stream_id.to_uppercase()
        ).await

        schema ← client.get<Schema>("/" + stream_id + "/schema").await
        sources ← client.get<Vec<SourceConfig>>("/" + stream_id + "/sources").await

        IF config.is_ok() AND schema.is_ok() THEN
            full_config ← StreamConfig {
                id: stream_id,
                config: config.unwrap(),
                schema: schema.unwrap(),
                sources: sources.unwrap_or_default()
            }

            validationResult ← validateStreamConfig(full_config)
            IF validationResult.isOk THEN
                registry.write().await.insert(stream_id, full_config)
                Log.info("Loaded stream: {}", stream_id)
            ELSE
                Log.error("Invalid stream config: {} - {}", stream_id, validationResult.error)
            END IF
        ELSE
            Log.warn("Incomplete stream definition: {}", stream_id)
        END IF
    END FOR

    // Setup etcd watch using ConfigClient
    watch_handle ← client.watch("/").await

    // Spawn background task to process watch events
    SPAWN_TASK(ProcessWatchEvents, watch_handle, registry, watchCallback)

    RETURN RegistryHandle {
        client: client,
        registry: registry,
        watch_handle: watch_handle,
        prefix: "/streams"
    }
END

TIME COMPLEXITY: O(n * k) where n = streams, k = keys per stream
SPACE COMPLEXITY: O(n * m) where m = avg stream config size
```

### 1.2 Watch Event Processing (Hot-Reload)

```
ALGORITHM: ProcessWatchEvents
INPUT: watchHandle (WatchHandle), registry (Arc<RwLock<HashMap>>), callback (function)
OUTPUT: none (continuous loop)

EXTENDS: ConfigClient watch pattern

BEGIN
    LOOP
        event ← watchHandle.recv().await

        IF event IS NULL THEN
            Log.error("Watch connection lost, reconnecting...")
            BREAK
        END IF

        // Parse event key: "/streams/{stream_id}/{component}"
        parsed ← parse_stream_key(event.key)
        stream_id ← parsed.stream_id
        component ← parsed.component  // "config", "schema", "sources"

        CASE event.type OF
            PUT:
                // Configuration update - reload entire stream
                new_config ← reload_stream_config(stream_id, client).await

                IF new_config.is_ok() THEN
                    validation ← validateStreamConfig(new_config)

                    IF validation.isOk THEN
                        old_config ← registry.write().await.insert(stream_id, new_config)

                        // Notify coordinator of changes
                        callback({
                            type: "STREAM_UPDATED",
                            stream_id: stream_id,
                            old_config: old_config,
                            new_config: new_config
                        })

                        Log.info("Hot-reloaded stream: {}", stream_id)
                    ELSE
                        Log.error("Invalid updated config: {} - {}", stream_id, validation.error)
                    END IF
                ELSE
                    Log.error("Failed to reload stream: {}", stream_id)
                END IF

            DELETE:
                // Stream removed
                IF component = "config" THEN
                    old_config ← registry.write().await.remove(stream_id)

                    IF old_config.is_some() THEN
                        callback({
                            type: "STREAM_DELETED",
                            stream_id: stream_id,
                            old_config: old_config
                        })

                        Log.info("Removed stream: {}", stream_id)
                    END IF
                END IF
        END CASE
    END LOOP
END

TIME COMPLEXITY: O(1) per event
SPACE COMPLEXITY: O(1) per event
```

### 1.3 Stream CRUD Operations (Using ConfigClient)

```
ALGORITHM: CreateStream
INPUT: streamId (string), config (StreamConfig), schema (Schema), sources (Vec<SourceConfig>)
OUTPUT: Result<(), Error>

USES: ConfigClient::put() for atomic writes

BEGIN
    // Validate stream ID format
    IF NOT is_valid_stream_id(streamId) THEN
        RETURN Error("Invalid stream ID format")
    END IF

    // Check if stream exists using ConfigClient
    existing ← client.get::<StreamConfig>("/" + streamId + "/config").await
    IF existing.is_ok() THEN
        RETURN Error("Stream already exists: " + streamId)
    END IF

    // Validate schema
    schema_validation ← validateSchema(schema)
    IF schema_validation.is_error() THEN
        RETURN Error("Invalid schema: " + schema_validation.error)
    END IF

    // Validate all sources
    FOR EACH source IN sources DO
        source_validation ← validateSourceConfig(source)
        IF source_validation.is_error() THEN
            RETURN Error("Invalid source: " + source_validation.error)
        END IF
    END FOR

    // Generate TimescaleDB DDL from schema
    ddl ← generateTableDDL(streamId, schema)

    // Execute DDL (atomic transaction)
    db_result ← db_client.execute(ddl).await
    IF db_result.is_error() THEN
        RETURN Error("Failed to create table: " + db_result.error)
    END IF

    // Write to etcd (ConfigClient handles serialization)
    TRY
        client.put("/" + streamId + "/config", config).await
        client.put("/" + streamId + "/schema", schema).await
        client.put("/" + streamId + "/sources", sources).await
        client.put("/" + streamId + "/ddl", ddl).await

        Log.info("Created stream: {}", streamId)
        RETURN Ok(())
    CATCH error
        // Rollback: drop table
        db_client.execute("DROP TABLE IF EXISTS " + streamId).await
        RETURN Error("Failed to write to etcd: " + error)
    END TRY
END

TIME COMPLEXITY: O(s) where s = number of sources
SPACE COMPLEXITY: O(1)
```

---

## 2. Ingestion Coordinator (EXTENDS MqttHandler Pattern)

### 2.1 Coordinator Initialization (Wraps Existing Handlers)

```
ALGORITHM: InitializeIngestionCoordinator
INPUT: registryHandle (RegistryHandle)
OUTPUT: coordinator (IngestionCoordinator)

EXTENDS: MqttHandler and StorageWriter patterns from AIR-002

DATA STRUCTURES:
    SourceHandle:
        source_id: String
        stream_id: String
        source_type: SourceType
        handler_task: JoinHandle         // tokio task handle
        channel_tx: mpsc::Sender<Point>
        channel_rx: mpsc::Receiver<Point>
        last_health_check: Instant

    CoordinatorState:
        registry: RegistryHandle
        active_sources: HashMap<SourceId, SourceHandle>
        storage_writers: HashMap<StreamId, JoinHandle>

BEGIN
    state ← CoordinatorState::new()
    state.registry ← registryHandle
    state.active_sources ← HashMap::new()
    state.storage_writers ← HashMap::new()

    // Spawn sources for all registered streams (using MqttHandler pattern)
    streams ← registryHandle.registry.read().await
    FOR EACH (stream_id, stream_config) IN streams DO
        // 1. Create storage writer for this stream
        writer_handle ← spawn_storage_writer(stream_id, stream_config)
        state.storage_writers.insert(stream_id, writer_handle)

        // 2. Spawn all sources for this stream
        FOR EACH source_config IN stream_config.sources DO
            result ← spawn_source(stream_id, source_config, state)
            IF result.is_error() THEN
                Log.error("Failed to spawn source: {}", result.error)
            END IF
        END FOR
    END FOR

    // Register watch callback for dynamic source management
    registryHandle.on_update(handle_registry_update, state)

    // Start background tasks
    SPAWN_TASK(monitor_source_health, state)

    RETURN state
END

ALGORITHM: SpawnSource
INPUT: streamId (string), sourceConfig (SourceConfig), state (CoordinatorState)
OUTPUT: Result<SourceHandle, Error>

WRAPS: MqttHandler pattern with mpsc channel

BEGIN
    source_id ← generate_source_id(streamId, sourceConfig)

    // Check if source already exists
    IF state.active_sources.contains_key(source_id) THEN
        RETURN Error("Source already active: " + source_id)
    END IF

    // Create channel for this source (default buffer: 1000)
    (tx, rx) ← mpsc::channel::<TimeSeriesPoint>(sourceConfig.buffer_size OR 1000)

    // Get storage writer channel for this stream
    writer_tx ← state.get_writer_channel(streamId)

    // Create source handler based on type
    handler_task ← CASE sourceConfig.type OF
        "mqtt":
            // Use MqttHandler pattern from AIR-002
            mqtt_config ← convert_to_mqtt_config(sourceConfig)
            handler ← MqttHandler::new(mqtt_config, tx).await
            SPAWN_TASK(run_mqtt_handler, handler, source_id)

        "http_poll":
            // Similar pattern but with HTTP polling
            http_config ← convert_to_http_config(sourceConfig)
            poller ← HttpPoller::new(http_config, tx).await
            SPAWN_TASK(run_http_poller, poller, source_id)

        "webhook":
            // Webhook server with channel output
            webhook_config ← convert_to_webhook_config(sourceConfig)
            server ← WebhookServer::new(webhook_config, tx).await
            SPAWN_TASK(run_webhook_server, server, source_id)

        DEFAULT:
            RETURN Error("Unsupported source type: " + sourceConfig.type)
    END CASE

    // Spawn router task to forward points from source to storage
    SPAWN_TASK(route_points, rx, writer_tx, stream_id, source_id)

    // Create source handle
    handle ← SourceHandle {
        source_id: source_id,
        stream_id: stream_id,
        source_type: sourceConfig.type,
        handler_task: handler_task,
        channel_tx: tx,
        channel_rx: rx,
        last_health_check: Instant::now()
    }

    state.active_sources.insert(source_id, handle)
    Log.info("Spawned source: {} for stream: {}", source_id, stream_id)

    RETURN Ok(handle)
END

TIME COMPLEXITY: O(1) per source
SPACE COMPLEXITY: O(b) where b = channel buffer size
```

### 2.2 Point Routing (Similar to MqttHandler Forward Pattern)

```
ALGORITHM: RoutePoints
INPUT: source_rx (mpsc::Receiver), writer_tx (mpsc::Sender), stream_id, source_id
OUTPUT: none (continuous loop)

PATTERN: Channel-to-channel forwarding with validation

BEGIN
    schema ← registry.get_schema(stream_id)

    LOOP
        // Receive point from source (like MqttHandler receives from MqttSource)
        point ← source_rx.recv().await

        IF point IS NULL THEN
            Log.warn("Source channel closed: {}", source_id)
            BREAK
        END IF

        // Enrich with metadata (similar to AIR-002 metadata)
        enriched ← TimeSeriesPoint {
            timestamp: point.timestamp,
            location_id: source_id,  // Use source_id as location_id
            value: point.value,
            tags: point.tags
        }

        // Add stream-specific tags
        enriched.tags.insert("stream_id", stream_id)
        enriched.tags.insert("source_type", get_source_type(source_id))

        // Validate against schema
        validation ← validate_point(enriched, schema)
        IF validation.is_error() THEN
            Log.error("Validation failed: {}", validation.error)
            Metrics.increment("validation_errors", {stream: stream_id})
            CONTINUE
        END IF

        // Forward to storage writer (non-blocking with timeout)
        send_result ← writer_tx.send_timeout(enriched, Duration::from_secs(5)).await
        IF send_result.is_error() THEN
            Log.warn("Storage writer channel full: {}", stream_id)
            Metrics.increment("routing_errors", {stream: stream_id})
        ELSE
            Metrics.increment("points_routed", {stream: stream_id})
        END IF
    END LOOP

    Log.info("Router terminated for source: {}", source_id)
END

TIME COMPLEXITY: O(1) per point
SPACE COMPLEXITY: O(1)
```

### 2.3 Storage Writer Spawning (Extends StorageWriter Pattern)

```
ALGORITHM: SpawnStorageWriter
INPUT: streamId (string), streamConfig (StreamConfig)
OUTPUT: JoinHandle

WRAPS: StorageWriter pattern from storage_writer.rs

BEGIN
    // Create ParquetStore for this stream (same pattern as AIR-002)
    base_path ← storage_config.base_path + "/streams/" + streamId
    store ← Arc::new(ParquetStore::new(base_path).await?)

    // Replay WAL on startup (same as AIR-002)
    store.replay_wal().await?

    // Create channel for incoming points
    (tx, rx) ← mpsc::channel::<TimeSeriesPoint>(streamConfig.buffer_capacity OR 1000)

    // Create storage writer (same pattern as AIR-002)
    writer ← StorageWriter::new(
        store,
        rx,
        Some(streamConfig.batch_size OR 100),
        Some(Duration::from_secs(streamConfig.batch_timeout_secs OR 5))
    )

    // Spawn writer task (runs indefinitely like AIR-002)
    task_handle ← SPAWN_TASK(async move {
        match writer.run().await {
            Ok(_) => Log.info("Storage writer completed: {}", streamId),
            Err(e) => Log.error("Storage writer error: {} - {}", streamId, e)
        }
    })

    // Store channel sender for routing
    state.writer_channels.insert(streamId, tx)

    Log.info("Spawned storage writer for stream: {}", streamId)
    RETURN task_handle
END

TIME COMPLEXITY: O(1)
SPACE COMPLEXITY: O(b) where b = channel buffer
```

---

## 3. Source Trait Implementations (EXTENDS neural_core::Source)

### 3.1 MQTT Source (Wraps Existing MqttSource)

```
ALGORITHM: MqttSourceWrapper
INPUT: sourceConfig (SourceConfig)
OUTPUT: mpsc::Receiver<TimeSeriesPoint>

WRAPS: neural_core::MqttSource from mqtt_handler.rs

DATA STRUCTURES:
    MqttSourceConfig:
        broker_url: String
        port: u16
        topic: String
        qos: QoS
        client_id: String
        credentials: Option<Credentials>
        buffer_capacity: usize

BEGIN
    // Convert generic SourceConfig to MqttConfig (AIR-002 format)
    mqtt_config ← MqttConfig {
        broker_url: sourceConfig.params.get("broker_url"),
        port: sourceConfig.params.get("port").parse::<u16>(),
        client_id: sourceConfig.params.get("client_id") OR generate_client_id(),
        topic_pattern: sourceConfig.params.get("topic"),
        qos: parse_qos(sourceConfig.params.get("qos") OR "1"),
        reconnect_delay: Duration::from_secs(
            sourceConfig.params.get("reconnect_delay_secs") OR "1"
        ),
        max_reconnect_delay: Duration::from_secs(
            sourceConfig.params.get("max_reconnect_delay_secs") OR "30"
        ),
        buffer_capacity: sourceConfig.params.get("buffer_capacity") OR 1000
    }

    // Create channel for output
    (sender, receiver) ← mpsc::channel<TimeSeriesPoint>(mqtt_config.buffer_capacity)

    // Create MqttHandler using existing pattern
    handler ← MqttHandler::new(mqtt_config, sender).await?

    // Spawn handler task (runs indefinitely like AIR-002)
    SPAWN_TASK(async move {
        match handler.run().await {
            Ok(_) => Log.info("MQTT handler completed"),
            Err(e) => Log.error("MQTT handler error: {}", e)
        }
    })

    RETURN receiver
END

TIME COMPLEXITY: O(1) initialization, O(n) for n points
SPACE COMPLEXITY: O(buffer_capacity)
```

### 3.2 HTTP Poller Source (New Implementation, AIR-002 Pattern)

```
ALGORITHM: HttpPollerSource
INPUT: sourceConfig (SourceConfig)
OUTPUT: mpsc::Receiver<TimeSeriesPoint>

FOLLOWS: MqttHandler fetch pattern

DATA STRUCTURES:
    HttpPollerConfig:
        url: String
        method: HttpMethod
        headers: HashMap<String, String>
        auth: Option<AuthConfig>
        interval: Duration
        timeout: Duration
        response_format: ResponseFormat

BEGIN
    // Parse config
    config ← parse_http_config(sourceConfig.params)

    // Create channel
    (sender, receiver) ← mpsc::channel<TimeSeriesPoint>(config.buffer_capacity OR 1000)

    // Spawn polling task (similar to MqttHandler::run loop)
    SPAWN_TASK(async move {
        LOOP
            // Fetch data from HTTP endpoint
            TRY
                response ← http_client.request(config.url, config.method)
                    .headers(config.headers)
                    .timeout(config.timeout)
                    .send()
                    .await?

                // Parse response based on format
                points ← CASE config.response_format OF
                    "json": parse_json_to_points(response.body)
                    "csv": parse_csv_to_points(response.body)
                    "xml": parse_xml_to_points(response.body)
                END CASE

                // Send points through channel (like MqttHandler)
                FOR EACH point IN points DO
                    sender.send(point).await?
                END FOR

                Metrics.increment("http_poll_success", {url: config.url})
            CATCH error
                Log.error("HTTP poll failed: {}", error)
                Metrics.increment("http_poll_errors", {url: config.url})
            END TRY

            // Wait for next poll interval (like MqttHandler sleep)
            tokio::time::sleep(config.interval).await
        END LOOP
    })

    RETURN receiver
END

TIME COMPLEXITY: O(n) per poll where n = points per response
SPACE COMPLEXITY: O(n)
```

### 3.3 Health Check Pattern (Extends AIR-002)

```
ALGORITHM: SourceHealthCheck
INPUT: sourceHandle (SourceHandle)
OUTPUT: HealthStatus

USES: MqttHandler::health_check() pattern

BEGIN
    CASE sourceHandle.source_type OF
        "mqtt":
            // Delegate to MqttHandler health check
            health ← sourceHandle.handler.health_check().await
            RETURN health

        "http_poll":
            // Check if HTTP endpoint is reachable
            TRY
                response ← http_client.head(config.url)
                    .timeout(Duration::from_secs(5))
                    .send()
                    .await

                IF response.status().is_success() THEN
                    RETURN HealthStatus {
                        healthy: true,
                        message: "HTTP endpoint reachable",
                        details: {
                            "status_code": response.status().as_u16(),
                            "url": config.url
                        }
                    }
                ELSE
                    RETURN HealthStatus {
                        healthy: false,
                        message: "HTTP endpoint unhealthy",
                        details: {
                            "status_code": response.status().as_u16(),
                            "url": config.url
                        }
                    }
                END IF
            CATCH error
                RETURN HealthStatus {
                    healthy: false,
                    message: "HTTP endpoint unreachable: " + error,
                    details: {"url": config.url}
                }
            END TRY

        "webhook":
            // Check if webhook server is running
            server_running ← sourceHandle.handler.is_running()
            RETURN HealthStatus {
                healthy: server_running,
                message: if server_running then "Webhook server running" else "Webhook server stopped",
                details: {
                    "port": config.port,
                    "path": config.path
                }
            }
    END CASE
END

TIME COMPLEXITY: O(1) for local checks, O(timeout) for HTTP
SPACE COMPLEXITY: O(1)
```

---

## 4. Storage Operations (EXTENDS ParquetStore Pattern)

### 4.1 Multi-Stream Parquet Storage

```
ALGORITHM: MultiStreamParquetStore
INPUT: baseStoragePath (PathBuf)
OUTPUT: Store manager for multiple streams

EXTENDS: ParquetStore from parquet.rs

DATA STRUCTURES:
    MultiStreamStore:
        base_path: PathBuf
        stores: HashMap<StreamId, Arc<ParquetStore>>
        wal_dir: PathBuf

PARTITION STRUCTURE (per stream):
    {base_path}/streams/{stream_id}/data/{location_id}/year={YYYY}/month={MM}/day={DD}/readings.parquet

    Example:
    /data/parquet/streams/home-events/data/front-door/year=2024/month=12/day=15/readings.parquet
    /data/parquet/streams/air-quality/data/living-room/year=2024/month=12/day=15/readings.parquet

BEGIN
    FUNCTION: get_or_create_store(stream_id)
        IF stores.contains_key(stream_id) THEN
            RETURN stores.get(stream_id)
        END IF

        // Create store using existing ParquetStore pattern
        stream_path ← base_path.join("streams").join(stream_id)
        store ← ParquetStore::new(stream_path)?

        // Replay WAL on first access (like AIR-002)
        store.replay_wal().await?

        stores.insert(stream_id, Arc::new(store))
        RETURN stores.get(stream_id)
    END FUNCTION

    FUNCTION: write_batch(stream_id, points)
        store ← get_or_create_store(stream_id)

        // Use existing ParquetStore::write_batch logic
        store.write_batch(points).await
    END FUNCTION

    FUNCTION: query(stream_id, location_id, start, end, filters)
        store ← get_or_create_store(stream_id)

        // Use existing ParquetStore::query logic
        store.query(location_id, start, end, filters).await
    END FUNCTION
END

TIME COMPLEXITY: Same as ParquetStore
SPACE COMPLEXITY: O(s * n) where s = active streams, n = points per stream
```

### 4.2 Cross-Stream Query Pattern

```
ALGORITHM: CrossStreamQuery
INPUT: streamIds (Vec<StreamId>), timeRange (TimeRange), alignment (AlignmentStrategy)
OUTPUT: Vec<AlignedPoint>

USES: Existing ParquetStore::query() per stream

BEGIN
    // Query each stream in parallel
    queries ← streamIds.map(|stream_id| {
        SPAWN_ASYNC(async move {
            store ← multi_store.get_or_create_store(stream_id)
            points ← store.query(
                "*",  // All locations
                timeRange.start,
                timeRange.end,
                None
            ).await
            RETURN (stream_id, points)
        })
    })

    // Await all queries
    results ← join_all(queries).await

    // Align points by timestamp
    aligned ← CASE alignment OF
        "asof":
            align_asof(results, timeRange.tolerance)
        "interpolate":
            align_interpolate(results, timeRange.interval)
        "nearest":
            align_nearest(results, timeRange.tolerance)
    END CASE

    RETURN aligned
END

SUBROUTINE: align_asof(results, tolerance)
    // ASOF join: for each timestamp in primary stream,
    // find most recent point in other streams within tolerance

    primary_stream ← results[0]
    aligned_points ← []

    FOR EACH primary_point IN primary_stream.points DO
        aligned ← AlignedPoint {
            timestamp: primary_point.timestamp,
            values: HashMap::new()
        }

        aligned.values.insert(primary_stream.stream_id, primary_point.value)

        FOR EACH (stream_id, points) IN results[1..] DO
            // Binary search for closest timestamp <= primary_point.timestamp
            closest ← points.binary_search_by(|p| {
                p.timestamp.cmp(&primary_point.timestamp)
            })

            IF closest.is_ok() OR closest.is_err_and_within(tolerance) THEN
                idx ← closest.unwrap_or_else(|x| x.saturating_sub(1))

                IF abs(points[idx].timestamp - primary_point.timestamp) <= tolerance THEN
                    aligned.values.insert(stream_id, points[idx].value)
                END IF
            END IF
        END FOR

        // Only include if all streams have values
        IF aligned.values.len() = results.len() THEN
            aligned_points.push(aligned)
        END IF
    END FOR

    RETURN aligned_points
END

TIME COMPLEXITY: O(s * n log m) where s = streams, n = primary points, m = avg points per stream
SPACE COMPLEXITY: O(s * n)
```

---

## 5. Migration Path from AIR-002 to AIR-004

### 5.1 Wrapping Existing MqttHandler

```
ALGORITHM: MigrateAIR002ToAIR004
INPUT: existing AIR-002 deployment
OUTPUT: AIR-004 with AIR-002 as first stream

STRATEGY: Wrap, don't replace

BEGIN
    // Step 1: Create "air-quality" stream in registry
    air_quality_stream ← StreamConfig {
        id: "air-quality",
        schema: Schema {
            fields: [
                {name: "pm25", type: "float", nullable: false},
                {name: "pm10", type: "float", nullable: true},
                {name: "temperature", type: "float", nullable: true},
                {name: "humidity", type: "float", nullable: true}
            ]
        },
        sources: [
            {
                type: "mqtt",
                id: "airgradient-mqtt",
                params: {
                    broker_url: env("MQTT_BROKER_URL"),
                    port: env("MQTT_PORT"),
                    topic: "airgradient/readings/+",
                    qos: 1,
                    buffer_capacity: 1000
                }
            }
        ],
        storage: {
            batch_size: 100,
            batch_timeout_secs: 5
        }
    }

    // Step 2: Register stream in etcd
    client.put("/streams/air-quality/config", air_quality_stream.config)
    client.put("/streams/air-quality/schema", air_quality_stream.schema)
    client.put("/streams/air-quality/sources", air_quality_stream.sources)

    // Step 3: Coordinator automatically spawns MqttHandler
    // (uses existing MqttHandler::new() and run() logic)

    // Step 4: Data continues to flow to same Parquet structure
    // /data/parquet/streams/air-quality/data/{location_id}/...

    // NO CODE CHANGES to existing MqttHandler or StorageWriter
    Log.info("AIR-002 migrated to AIR-004 as 'air-quality' stream")
END

MIGRATION IMPACT:
    - Zero changes to MqttHandler code
    - Zero changes to StorageWriter code
    - Zero changes to ParquetStore code
    - Only adds: registry, coordinator, multi-stream support
    - Backward compatible: AIR-002 continues working as-is
```

### 5.2 Adding Second Stream (Home Events)

```
ALGORITHM: AddHomeEventsStream
INPUT: none (new stream registration)
OUTPUT: Second stream running alongside air-quality

BEGIN
    // Define home events stream
    home_events_stream ← StreamConfig {
        id: "home-events",
        schema: Schema {
            fields: [
                {name: "event_type", type: "string", nullable: false},
                {name: "room", type: "string", nullable: false},
                {name: "state", type: "string", nullable: true}
            ]
        },
        sources: [
            {
                type: "webhook",
                id: "home-webhook",
                params: {
                    port: 9000,
                    path: "/events",
                    auth_token: env("HOME_WEBHOOK_TOKEN")
                }
            }
        ]
    }

    // Register in etcd
    client.put("/streams/home-events/config", home_events_stream.config)
    client.put("/streams/home-events/schema", home_events_stream.schema)
    client.put("/streams/home-events/sources", home_events_stream.sources)

    // Coordinator automatically:
    // 1. Creates new ParquetStore for home-events
    // 2. Spawns WebhookServer with channel
    // 3. Spawns StorageWriter for home-events
    // 4. Both streams run independently

    Log.info("Added home-events stream, now running 2 streams")
END

RESULT:
    Two independent pipelines:
    - air-quality: MQTT -> Channel -> StorageWriter -> Parquet
    - home-events: Webhook -> Channel -> StorageWriter -> Parquet

    Both use same underlying patterns (mpsc, batching, WAL)
```

---

## 6. Implementation File Mapping

### 6.1 Current AIR-002 Files (Keep As-Is)

```
EXISTING FILES (no changes):
    /apps/air-quality-app/src/ingestion/mqtt_handler.rs
        - Keep: MqttHandler struct and run() loop
        - Reuse: Spawn via coordinator

    /apps/air-quality-app/src/pipeline/storage_writer.rs
        - Keep: StorageWriter with batching logic
        - Reuse: One instance per stream

    /apps/air-quality-app/src/config_etcd.rs
        - Keep: EtcdAppConfig loading
        - Extend: Add stream registry loading

    /core/src/storage/parquet.rs
        - Keep: ParquetStore implementation
        - Extend: MultiStreamStore wrapper
```

### 6.2 New AIR-004 Files (Extensions)

```
NEW FILES (extend existing patterns):
    /apps/air-quality-app/src/registry/
        - stream_registry.rs: Wraps ConfigClient for stream configs
        - schema_validator.rs: Validates stream schemas

    /apps/air-quality-app/src/coordinator/
        - ingestion_coordinator.rs: Spawns MqttHandler instances
        - source_manager.rs: Manages multiple source types
        - router.rs: Channel-to-channel forwarding

    /apps/air-quality-app/src/sources/
        - http_poller.rs: HTTP polling (follows MqttHandler pattern)
        - webhook_server.rs: Webhook handler (follows MqttHandler pattern)
        - source_wrapper.rs: Generic wrapper for all source types

    /apps/air-quality-app/src/storage/
        - multi_stream_store.rs: HashMap of ParquetStore instances
        - cross_stream_query.rs: Join/align logic
```

### 6.3 Integration Points

```
INTEGRATION FLOW:
    main.rs
    ├── Load config (config_etcd.rs - existing)
    ├── Initialize registry (stream_registry.rs - new)
    │   └── Uses ConfigClient (existing pattern)
    ├── Initialize coordinator (ingestion_coordinator.rs - new)
    │   └── Spawns MqttHandler (existing)
    │   └── Spawns StorageWriter (existing)
    │   └── Creates ParquetStore (existing)
    └── Start API server (existing)
```

---

## Complexity Analysis Summary

### Stream Registry
- Initialization: O(n * k) where n = streams, k = keys per stream
- Watch Processing: O(1) per event
- CRUD Operations: O(1) for cache, O(log n) for etcd

### Ingestion Coordinator
- Source Spawning: O(1) per source (reuses MqttHandler)
- Point Routing: O(1) per point (channel forwarding)
- Health Checks: O(s) where s = active sources

### Storage Operations
- Write Batch: O(n log n) for grouping (same as ParquetStore)
- Query: O(p) where p = partition files to scan
- Cross-Stream Align: O(s * n log m) where s = streams, n = points, m = avg per stream

### Migration
- AIR-002 to AIR-004: O(1) - just configuration, no code changes
- Adding Stream: O(1) - registry insert, coordinator spawns handlers

---

## Design Patterns Summary

### Coordinator Pattern
- Spawns multiple MqttHandler instances (one per source)
- Uses tokio::spawn for concurrent tasks
- Arc<RwLock<>> for shared registry state

### Channel Pattern (Consistent with AIR-002)
- mpsc::channel for all internal communication
- Source -> Router -> StorageWriter pipeline
- Non-blocking send with timeout

### Batching Pattern (Same as AIR-002)
- tokio::select! for timeout OR batch size
- Write-ahead logging before flush
- Graceful shutdown on channel close

### Extension Pattern (Key Principle)
- Wrap existing components, don't replace
- ConfigClient for all etcd operations
- ParquetStore remains unchanged
- MqttHandler remains unchanged

---

*Last Updated: 2025-12-15*
*SPARC Phase: Pseudocode (Revised)*
*Status: Aligned with AIR-002 Implementation*
*Next Phase: Architecture (showing integration points)*
