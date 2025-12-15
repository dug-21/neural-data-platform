# AIR-004: Integration Architecture

## System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     AIR QUALITY APPLICATION                      │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                         STARTUP FLOW                             │
└─────────────────────────────────────────────────────────────────┘

    main.rs
       │
       ├─► 1. Load AppConfig from etcd (/air-quality/*)
       │      ├─ /air-quality/server/host
       │      ├─ /air-quality/mqtt/broker_url
       │      └─ /air-quality/storage/base_path
       │
       ├─► 2. Initialize StreamRegistry (NEW - Phase 1)
       │      └─ Try load /streams/air-quality/config
       │         ├─ Found: Log schema details, validate
       │         └─ Not found: Log "expected for existing deployments"
       │
       ├─► 3. Initialize ParquetStore
       │      └─ Replay WAL if enabled
       │
       ├─► 4. Create MQTT Handler
       │      └─ Connect to broker, subscribe to topics
       │
       ├─► 5. Create Storage Writer
       │      └─ Channel-based batching pipeline
       │
       └─► 6. Start HTTP Server
              └─ Serve API endpoints
```

## etcd Configuration Paths

### Current (Legacy) Paths - UNCHANGED
```
etcd://
├── air-quality/
│   ├── server/
│   │   ├── host = "0.0.0.0"
│   │   └── port = 8080
│   ├── mqtt/
│   │   ├── broker_url = "10.0.0.100"
│   │   ├── port = 1883
│   │   ├── client_id = "air-quality-app"
│   │   ├── topic_pattern = "airgradient/readings/+"
│   │   ├── qos = 1
│   │   └── buffer_capacity = 1000
│   └── storage/
│       ├── base_path = "/app/data"
│       ├── wal_enabled = true
│       ├── batch_size = 100
│       └── batch_timeout_secs = 5
```

### New (Multi-Stream) Paths - ADDITIVE
```
etcd://
├── streams/
│   ├── air-quality/
│   │   └── config = {StreamConfig JSON}
│   │       ├── stream_id: "air-quality"
│   │       ├── fields: [pm25, pm10, co2, temp, ...]
│   │       ├── sources: [{type: mqtt, ...}]
│   │       └── storage: {batch_size, timeout, ...}
│   │
│   ├── home-energy/         (Future)
│   │   └── config = {...}
│   │
│   └── weather/             (Future)
│       └── config = {...}
```

## Data Flow Architecture

### Current (Single Stream)
```
AirGradient Sensor
       │
       │ MQTT
       ▼
  MQTT Broker (10.0.0.100:1883)
       │
       │ Subscribe: "airgradient/readings/+"
       ▼
  MqttHandler
       │
       │ mpsc::channel(1000)
       ▼
  StorageWriter
       │
       │ Batching (100 records or 5s)
       ▼
  ParquetStore
       │
       └─► /app/data/air_quality_YYYYMMDD.parquet
```

### Future (Multi-Stream)
```
Multiple Sources                     StreamRegistry
       │                                   │
       ├─ AirGradient MQTT                │ Load Configs
       ├─ Energy Monitor HTTP              │ from etcd
       └─ Weather API Poll                 │
              │                            ▼
              │                     ┌──────────────┐
              │                     │ StreamConfig │
              │                     │  Definitions │
              │                     └──────────────┘
              │                            │
              ▼                            ▼
       Generic Handler Factory ─────► Create Handlers
              │                            │
              │                            ▼
              ├────► MQTT Handler (air-quality)
              ├────► HTTP Poller (home-energy)
              └────► Webhook Listener (weather)
                            │
                            │ mpsc channels
                            ▼
                     Stream Router
                            │
                            ▼
                  ┌─────────┴──────────┐
                  │                    │
            StorageWriter        StorageWriter
                  │                    │
                  ▼                    ▼
            ParquetStore         ParquetStore
             (air-quality)       (home-energy)
```

## Component Interactions

### Phase 1 (Minimal Integration - This Task)

```
┌─────────────┐
│   main.rs   │
└──────┬──────┘
       │
       ├─── ConfigClient ──► etcd /air-quality/*
       │                      (UNCHANGED)
       │
       └─── StreamRegistry ──► etcd /streams/air-quality/config
                                (NEW - Optional load only)

Result: Log StreamConfig if found, continue normally
```

### Phase 2 (Generic Handlers - Future)

```
┌─────────────┐
│   main.rs   │
└──────┬──────┘
       │
       └─── StreamRegistry.load_all_streams()
                   │
                   ▼
            ┌──────────────┐
            │ Stream Configs│
            │  air-quality  │
            │  home-energy  │
            │    weather    │
            └───────┬───────┘
                    │
                    ▼
         ┌──────────────────────┐
         │ Handler Factory       │
         │  .create_from_config()│
         └──────────┬────────────┘
                    │
        ┌───────────┼───────────┐
        ▼           ▼           ▼
   MQTT Handler  HTTP Poller  Webhook
   (air-quality) (energy)     (weather)
```

## StreamConfig Schema

```json
{
  "stream_id": "air-quality",
  "description": "Human-readable description",
  "version": "1.0.0",
  "enabled": true,

  "fields": [
    {
      "name": "pm25",
      "type": "float",
      "unit": "µg/m³",
      "range": [0.0, 500.0],
      "nullable": false
    }
  ],

  "sources": [
    {
      "type": "mqtt",
      "enabled": true,
      "broker_url": "10.0.0.100",
      "topic_pattern": "airgradient/readings/+"
    }
  ],

  "storage": {
    "batch_size": 100,
    "batch_timeout_secs": 5
  }
}
```

## Code Integration Points

### main.rs Integration (Phase 1)

```rust
// EXISTING CODE (lines 25-65)
let config = match air_quality_app::load_from_etcd().await {
    Ok(etcd_config) => { /* convert to AppConfig */ }
    Err(e) => { /* fallback to YAML or defaults */ }
};

// NEW CODE (insert after line 65)
use config_client::StreamRegistry;

let etcd_endpoint = std::env::var("ETCD_ENDPOINT")
    .unwrap_or_else(|_| "http://localhost:2379".to_string());

match StreamRegistry::new(&[&etcd_endpoint]).await {
    Ok(registry) => {
        match registry.load_stream("air-quality").await {
            Ok(stream_config) => {
                tracing::info!(
                    "✓ StreamConfig loaded: {} fields, {} sources",
                    stream_config.fields.len(),
                    stream_config.sources.len()
                );
                // Future: Use stream_config to create handlers
            }
            Err(e) => {
                tracing::info!(
                    "No StreamConfig (expected for existing deployments): {}",
                    e
                );
            }
        }
    }
    Err(e) => {
        tracing::warn!("StreamRegistry init failed: {}", e);
    }
}

// EXISTING CODE CONTINUES (lines 67+)
// Initialize ParquetStore, MQTT Handler, etc.
```

## Migration Path

### Step 1: Current State (Working)
- Single MQTT stream
- Config at /air-quality/*
- No StreamConfig

### Step 2: Add Registry (This Task)
- Optionally load StreamConfig
- Log if found, continue if not
- Zero breaking changes

### Step 3: Create StreamConfig (AIR-006)
- Migration tool converts AppConfig → StreamConfig
- Save to /streams/air-quality/config
- Both paths coexist

### Step 4: Use StreamConfig (AIR-007)
- Handler factory reads StreamConfig
- Creates handlers dynamically
- Prefer StreamConfig, fallback to legacy

### Step 5: Deprecate Legacy (Future)
- Remove /air-quality/* paths
- StreamConfig-only configuration
- All streams managed uniformly

## Testing Strategy

### Unit Tests
- StreamConfig validation
- Field type constraints
- Range validation
- ID format validation

### Integration Tests
1. **No StreamConfig:** App starts with legacy config
2. **Valid StreamConfig:** App starts, logs schema
3. **Invalid StreamConfig:** App logs error, uses legacy
4. **etcd Down:** App falls back to YAML config

### Deployment Tests
1. Deploy to Pi with no StreamConfig
2. Verify MQTT ingestion works
3. Add StreamConfig to etcd
4. Restart app, verify log messages
5. Verify data still flows correctly

## Rollback Plan

If issues arise:

1. **Remove Registry Code:**
   ```bash
   git revert <commit-hash>
   docker compose restart
   ```

2. **Config Intact:**
   - Legacy /air-quality/* paths unchanged
   - Data flow unchanged
   - Zero data loss

3. **Time to Rollback:** <5 minutes

## Success Criteria

✅ App starts successfully with no StreamConfig (existing deployments)
✅ App starts successfully with valid StreamConfig (new deployments)
✅ App logs StreamConfig schema when found
✅ App logs validation errors when StreamConfig invalid
✅ MQTT handler continues to work in all scenarios
✅ Data writes to ParquetStore in all scenarios
✅ No performance degradation
✅ No breaking changes to existing deployments

## Next Steps

1. **Implement Integration** (AIR-004)
   - Add StreamRegistry initialization to main.rs
   - Test both scenarios (with/without StreamConfig)
   - Deploy to development environment

2. **Create Migration Tool** (AIR-006)
   - CLI to convert AppConfig → StreamConfig
   - Validation and dry-run modes
   - Batch migration for multiple streams

3. **Build Handler Factory** (AIR-005)
   - Generic MQTT handler from StreamConfig
   - Generic HTTP poller from StreamConfig
   - Stream routing and multiplexing

4. **Auto-Generated APIs** (AIR-007)
   - /api/streams/{id}/data
   - /api/streams/{id}/schema
   - /api/streams/{id}/health

---

**Architecture Status:** Ready for Implementation
**Risk Assessment:** LOW
**Deployment Impact:** ZERO (additive only)
