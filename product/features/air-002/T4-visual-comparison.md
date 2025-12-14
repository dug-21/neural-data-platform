# Task T4: Visual Comparison - Mock vs Real Implementation

## Before T4 (Mock Implementation)

```
┌─────────────────────────────────────────────────────────────┐
│                          main.rs                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  fn main() {                                                │
│      let services = create_mock_services();   ◄────────┐    │
│      let app = create_router(services);                │    │
│      axum::serve(listener, app).await;                 │    │
│  }                                                     │    │
│                                                        │    │
│  fn create_mock_services() {   ◄───────────────────────┘    │
│      struct MockStore;     // Empty implementation         │
│      struct MockSource;    // Returns empty Vec            │
│      struct MockForecast;  // Returns 0.0 metrics          │
│                                                             │
│      AppServices {                                          │
│          store: Arc::new(MockStore),     ◄── No data       │
│          source: Arc::new(MockSource),   ◄── No ingestion  │
│          forecast: Arc::new(MockForecast),                  │
│      }                                                      │
│  }                                                          │
└─────────────────────────────────────────────────────────────┘

No MQTT connection ✗
No data storage ✗
No background tasks ✗
No graceful shutdown ✗
```

## After T4 (Real Implementation)

```
┌─────────────────────────────────────────────────────────────┐
│                          main.rs                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  fn main() {                                                │
│      // 1. Initialize real ParquetStore                     │
│      let store = Arc::new(                                  │
│          ParquetStore::new(&config.storage.base_path)?      │
│      );                                                     │
│      store.replay_wal().await?;  ◄── Crash recovery        │
│                                                             │
│      // 2. Create pipeline channel                          │
│      let (tx, rx) = mpsc::channel(1000);                    │
│                                                             │
│      // 3. Initialize MQTT handler                          │
│      let mqtt_handler = MqttHandler::new(                   │
│          mqtt_config, tx                                    │
│      ).await?;                                              │
│                                                             │
│      // 4. Create storage writer                            │
│      let storage_writer = StorageWriter::new(               │
│          store.clone(), rx,                                 │
│          batch_size: 100,                                   │
│          timeout: 5s                                        │
│      );                                                     │
│                                                             │
│      // 5. Spawn background tasks                           │
│      tokio::spawn(storage_writer.run());                    │
│      tokio::spawn(mqtt_handler.run());                      │
│                                                             │
│      // 6. Start server with graceful shutdown              │
│      tokio::select! {                                       │
│          _ = axum::serve(listener, app) => {}               │
│          _ = shutdown_signal() => {                         │
│              drop(tx);  // Signal shutdown                  │
│              await all tasks;                               │
│          }                                                  │
│      }                                                      │
│  }                                                          │
└─────────────────────────────────────────────────────────────┘

Real MQTT connection ✓
Real Parquet storage ✓
Background task pipeline ✓
Graceful shutdown ✓
WAL crash recovery ✓
```

## Data Flow Comparison

### Before (Mock):
```
HTTP Request
    ↓
MockStore::query()
    ↓
Returns: []  (always empty)
```

### After (Real):
```
MQTT Broker (AirGradient sensors)
    ↓
MqttHandler::run()  ← Background Task 1
    ↓
mpsc::channel (buffered, 1000 capacity)
    ↓
StorageWriter::run()  ← Background Task 2
    ↓
Batch (100 points or 5s timeout)
    ↓
ParquetStore::write_batch()
    ↓
WAL (write-ahead log) → Parquet Files
    ↓
HTTP Request (API)
    ↓
ParquetStore::query()
    ↓
Returns: Vec<TimeSeriesPoint>  (real data!)
```

## Component Integration

### Before:
```
┌──────────────┐
│     API      │
│   (Axum)     │
└──────┬───────┘
       │
       ↓
┌──────────────┐
│  MockStore   │
│   (empty)    │
└──────────────┘
```

### After:
```
                    ┌─────────────────┐
                    │  MQTT Broker    │
                    └────────┬────────┘
                             │
                             ↓
                    ┌─────────────────┐
                    │  MqttHandler    │
                    │  (background)   │
                    └────────┬────────┘
                             │
                             ↓ mpsc::channel
                    ┌─────────────────┐
                    │ StorageWriter   │
                    │  (background)   │
                    └────────┬────────┘
                             │
                             ↓
       ┌─────────────────────┴─────────────────────┐
       │                                           │
       ↓                                           ↓
┌──────────────┐                          ┌──────────────┐
│     API      │                          │     WAL      │
│   (Axum)     │                          │   (crash     │
│              │                          │  recovery)   │
└──────┬───────┘                          └──────┬───────┘
       │                                         │
       ↓                                         ↓
┌─────────────────────────────────────────────────────────┐
│                   ParquetStore                          │
│  - query()                                              │
│  - write_batch()                                        │
│  - aggregate()                                          │
└─────────────────────────────────────────────────────────┘
```

## Code Size Comparison

### Before:
- `create_mock_services()`: 109 lines (lines 53-162)
- Mock implementations: 3 structs with trait impls
- Total mock code: ~110 lines

### After:
- `create_services_with_real_store()`: 70 lines (simplified)
- Real pipeline setup: 70 lines
- Background task management: 15 lines
- Graceful shutdown: 20 lines
- Total real code: ~175 lines (more functionality!)

## Key Improvements

| Feature | Before (Mock) | After (Real) |
|---------|--------------|--------------|
| Data Persistence | ✗ None | ✓ Parquet + WAL |
| MQTT Ingestion | ✗ None | ✓ Real-time |
| Crash Recovery | ✗ None | ✓ WAL replay |
| Background Tasks | ✗ None | ✓ 2 tasks |
| Graceful Shutdown | ✗ Immediate | ✓ Flushes pending writes |
| Degraded Mode | ✗ N/A | ✓ Continues without MQTT |
| Batching | ✗ N/A | ✓ 100 points or 5s |
| Backpressure | ✗ N/A | ✓ Channel buffering |

## Startup Sequence

### Before:
```
1. Load config
2. Create mock services
3. Start HTTP server
```

### After:
```
1. Load config
2. Initialize ParquetStore
3. Replay WAL (crash recovery)
4. Create pipeline channel
5. Build MqttConfig from AppConfig
6. Initialize MqttHandler (with degradation handling)
7. Create StorageWriter
8. Spawn storage background task
9. Spawn MQTT ingestion background task (if available)
10. Create API services with real store
11. Start HTTP server
12. Setup Ctrl+C handler
13. Run with graceful shutdown
```

## Shutdown Sequence

### Before:
```
1. Ctrl+C
2. Server exits immediately
```

### After:
```
1. Ctrl+C signal received
2. Close mpsc channel (tx)
3. StorageWriter receives None from channel
4. StorageWriter flushes remaining points
5. StorageWriter exits cleanly
6. MqttHandler receives shutdown signal
7. MqttHandler exits cleanly
8. HTTP server stops
9. All tasks joined
10. Clean exit
```

## Error Handling

### Before:
```rust
// No error handling - mocks never fail
fn create_mock_services() -> AppServices {
    // Always succeeds
}
```

### After:
```rust
// Graceful degradation on MQTT failure
let mqtt_handler = match MqttHandler::new(config, tx).await {
    Ok(handler) => {
        tracing::info!("MQTT initialized");
        Some(handler)
    }
    Err(e) => {
        tracing::warn!("MQTT failed: {}. Running in degraded mode", e);
        None  // API still works, just no ingestion
    }
};

// ParquetStore errors propagated
let store = Arc::new(ParquetStore::new(&path)?);

// WAL replay errors logged, not fatal
match store.replay_wal().await {
    Ok(_) => tracing::info!("WAL replay completed"),
    Err(e) => tracing::warn!("WAL replay failed (may be empty): {}", e),
}
```

## Configuration Mapping

### Before:
```rust
// No configuration used
```

### After:
```rust
// AppConfig → MqttConfig transformation
let mqtt_config = MqttConfig {
    broker_url: config.mqtt.broker_url.clone(),
    port: config.mqtt.port,
    client_id: config.mqtt.client_id.clone(),
    topic_pattern: config.mqtt.topic_pattern.clone(),
    qos: config.mqtt.get_qos(),  // u8 → QoS enum
    reconnect_delay: config.mqtt.get_reconnect_delay(),  // u64 → Duration
    max_reconnect_delay: config.mqtt.get_max_reconnect_delay(),
    buffer_capacity: config.mqtt.buffer_capacity,
};
```

## Summary

Task T4 transformed main.rs from a **non-functional mock** into a **production-ready air quality monitoring platform** with:

1. ✅ Real-time MQTT data ingestion
2. ✅ Persistent Parquet storage
3. ✅ Crash recovery via WAL
4. ✅ Batched writes for performance
5. ✅ Graceful shutdown handling
6. ✅ Degraded mode operation
7. ✅ Background task orchestration
8. ✅ Proper error handling

All core pipeline components (T1-T4) are now integrated and operational!
