# Unified Event Bus Architecture

**Insight**: Don't have separate "storage path" and "processing path". Everything is a subscriber to a single event bus.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              SOURCES                                         │
│                                                                              │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                     │
│   │ MQTT Source │    │ HTTP Source │    │ Future Src  │                     │
│   └──────┬──────┘    └──────┬──────┘    └──────┬──────┘                     │
│          │                  │                  │                             │
│          └──────────────────┼──────────────────┘                             │
│                             │                                                │
│                             ▼                                                │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                         EVENT BUS                                    │   │
│   │                  (broadcast::channel<RawDataPoint>)                  │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                             │                                                │
└─────────────────────────────┼────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┬─────────────────────┐
        │                     │                     │                     │
        ▼                     ▼                     ▼                     ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│    BRONZE     │    │    SILVER     │    │  PROCESSORS   │    │   FUTURE...   │
│   Subscriber  │    │   Subscriber  │    │  Subscribers  │    │               │
│               │    │               │    │               │    │  - Gold Layer │
│ - Batching    │    │ - Transform   │    │ - Threshold   │    │  - S3 Archive │
│ - WAL         │    │ - DQ Rules    │    │ - ML Inference│    │  - Replication│
│ - Parquet     │    │ - Upsert      │    │ - Aggregation │    │               │
│               │    │ - TimescaleDB │    │ - Webhooks    │    │               │
└───────────────┘    └───────────────┘    └───────────────┘    └───────────────┘
```

---

## Key Principles

1. **Single Event Bus** - All data flows through one broadcast channel
2. **Everything is a Subscriber** - Bronze, Silver, ML, Alerts are all equal consumers
3. **Config-Driven Subscribers** - Enable/disable via YAML, not code
4. **Independent Lifecycles** - Subscribers can fail/restart independently
5. **Backpressure Per-Subscriber** - Slow subscriber doesn't block others

---

## Configuration

### Subscriber Registry

```yaml
# config/base/platform.yaml
event_bus:
  capacity: 10000  # Broadcast channel size

subscribers:
  # Bronze Layer - raw archival
  - id: bronze
    type: storage
    enabled: true
    config:
      format: parquet
      path: /data/raw/{stream_id}
      partitioning: daily
      batch_size: 50
      batch_timeout_secs: 2
      wal_enabled: true

  # Silver Layer - analytics ready
  - id: silver
    type: timescale
    enabled: true
    config:
      connection_string: ${TIMESCALE_URL}
      batch_size: 100
      batch_timeout_secs: 5
      # Uses stream's silver_etl config for transforms

  # Threshold Alerts
  - id: threshold-alerts
    type: processor
    enabled: true
    processor: threshold
    # Processor config loaded from /processors/threshold-alerts

  # ML Predictions
  - id: ml-predictions
    type: processor
    enabled: true
    processor: ml-inference
    # Processor config loaded from /processors/ml-predictions

  # Rolling Aggregations
  - id: rolling-stats
    type: processor
    enabled: true
    processor: aggregation

  # Future: S3 Archive
  - id: s3-archive
    type: storage
    enabled: false
    config:
      format: parquet
      path: s3://bucket/archive/{stream_id}
      batch_size: 1000
      batch_timeout_secs: 60
```

### Stream-Level Overrides

```yaml
# config/base/streams/air-quality/config.yaml
stream_id: air-quality

# Override which subscribers process this stream
subscribers:
  bronze: true      # Default
  silver: true      # Default
  threshold-alerts: true
  ml-predictions: true
  rolling-stats: true
  s3-archive: false  # Not for this stream
```

---

## Implementation

### Event Bus

```rust
// core/src/event_bus.rs

pub struct EventBus {
    sender: broadcast::Sender<Arc<RawDataPoint>>,
    capacity: usize,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender, capacity }
    }

    pub fn publish(&self, point: RawDataPoint) -> Result<(), EventBusError> {
        // Wrap in Arc for zero-copy broadcast to all subscribers
        self.sender.send(Arc::new(point))?;
        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<RawDataPoint>> {
        self.sender.subscribe()
    }
}
```

### Subscriber Trait

```rust
// core/src/subscribers/traits.rs

#[async_trait]
pub trait Subscriber: Send + Sync {
    /// Unique identifier
    fn id(&self) -> &str;

    /// Start consuming from the event bus
    async fn start(&mut self, receiver: broadcast::Receiver<Arc<RawDataPoint>>) -> Result<(), SubscriberError>;

    /// Stop consuming
    async fn stop(&mut self) -> Result<(), SubscriberError>;

    /// Check if this subscriber should process a given stream
    fn accepts_stream(&self, stream_id: &str) -> bool;

    /// Health check
    async fn health_check(&self) -> HealthStatus;

    /// Reconfigure (hot reload)
    async fn reconfigure(&mut self, config: SubscriberConfig) -> Result<(), SubscriberError>;
}
```

### Subscriber Coordinator

```rust
// core/src/subscribers/coordinator.rs

pub struct SubscriberCoordinator {
    event_bus: Arc<EventBus>,
    subscribers: HashMap<String, Box<dyn Subscriber>>,
    handles: HashMap<String, JoinHandle<()>>,
}

impl SubscriberCoordinator {
    pub async fn start_all(&mut self) -> Result<(), CoordinatorError> {
        for (id, subscriber) in &mut self.subscribers {
            let receiver = self.event_bus.subscribe();
            let handle = tokio::spawn(async move {
                subscriber.start(receiver).await
            });
            self.handles.insert(id.clone(), handle);
        }
        Ok(())
    }

    pub async fn add_subscriber(&mut self, subscriber: Box<dyn Subscriber>) {
        let id = subscriber.id().to_string();
        let receiver = self.event_bus.subscribe();
        // Start immediately
        let handle = tokio::spawn(async move {
            subscriber.start(receiver).await
        });
        self.handles.insert(id, handle);
    }

    pub async fn remove_subscriber(&mut self, id: &str) {
        if let Some(handle) = self.handles.remove(id) {
            handle.abort();
        }
        self.subscribers.remove(id);
    }
}
```

### Bronze Subscriber

```rust
// core/src/subscribers/bronze.rs

pub struct BronzeSubscriber {
    id: String,
    store: Arc<ParquetStore>,
    config: BronzeConfig,
    buffer: Vec<Arc<RawDataPoint>>,
    stream_filter: Option<HashSet<String>>,
}

#[async_trait]
impl Subscriber for BronzeSubscriber {
    fn id(&self) -> &str { &self.id }

    async fn start(&mut self, mut receiver: broadcast::Receiver<Arc<RawDataPoint>>) -> Result<(), SubscriberError> {
        let mut flush_interval = tokio::time::interval(self.config.batch_timeout);

        loop {
            tokio::select! {
                result = receiver.recv() => {
                    match result {
                        Ok(point) => {
                            if self.accepts_stream(&point.source_id) {
                                self.buffer.push(point);
                                if self.buffer.len() >= self.config.batch_size {
                                    self.flush().await?;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Bronze subscriber lagged, missed {} events", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = flush_interval.tick() => {
                    if !self.buffer.is_empty() {
                        self.flush().await?;
                    }
                }
            }
        }
        Ok(())
    }

    fn accepts_stream(&self, stream_id: &str) -> bool {
        self.stream_filter.as_ref()
            .map(|f| f.contains(stream_id))
            .unwrap_or(true)
    }
}
```

### Silver Subscriber (Streaming ETL)

```rust
// core/src/subscribers/silver.rs

pub struct SilverSubscriber {
    id: String,
    db_pool: Arc<Pool>,
    stream_configs: HashMap<String, SilverEtlConfig>,
    buffer: HashMap<String, Vec<Arc<RawDataPoint>>>,
    config: SilverConfig,
}

#[async_trait]
impl Subscriber for SilverSubscriber {
    async fn start(&mut self, mut receiver: broadcast::Receiver<Arc<RawDataPoint>>) -> Result<(), SubscriberError> {
        loop {
            tokio::select! {
                result = receiver.recv() => {
                    match result {
                        Ok(point) => {
                            let stream_id = extract_stream_id(&point.source_id);
                            if let Some(etl_config) = self.stream_configs.get(&stream_id) {
                                // Transform inline using existing ETL logic
                                let transformed = transform_point(&point, etl_config)?;

                                // Buffer by stream
                                self.buffer.entry(stream_id)
                                    .or_default()
                                    .push(transformed);

                                // Flush if batch full
                                if self.buffer.get(&stream_id).map(|b| b.len()).unwrap_or(0) >= self.config.batch_size {
                                    self.flush_stream(&stream_id).await?;
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Silver subscriber lagged, missed {} events", n);
                            // Could trigger catch-up from Bronze here
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                _ = flush_interval.tick() => {
                    self.flush_all().await?;
                }
            }
        }
        Ok(())
    }
}
```

---

## Benefits Over Two-Path Design

| Aspect | Two-Path | Unified Event Bus |
|--------|----------|-------------------|
| Complexity | Storage vs Processing distinction | Everything is a subscriber |
| Adding new consumers | Modify code to fork | Just add subscriber config |
| Bronze/Silver relationship | Separate pipelines | Both subscribe to same bus |
| Failure isolation | Path-level | Per-subscriber |
| Config model | Different for each path | Uniform subscriber config |
| Future extensibility | Add more paths? | Add more subscribers |

---

## Latency Profile

With NVMe and unified bus:

| Subscriber | Latency from Event | Notes |
|------------|-------------------|-------|
| Bronze | 1-2s (batching) | Configurable, could be <1s |
| Silver | 1-5s (batching + transform) | Streaming, not batch ETL |
| Threshold Alerts | <10ms | No batching needed |
| ML Inference | 10-100ms | Depends on model |
| Aggregations | <50ms | Windowed, but streaming |

**Key insight**: Silver becomes a **streaming subscriber**, not a batch ETL job. It processes data as it arrives, just with its own batching for DB efficiency.

---

## Migration Path

### Phase 1: Add Event Bus
- Insert broadcast channel between sources and current storage
- Existing RawStorageWriter becomes first subscriber
- Zero behavior change, just architectural prep

### Phase 2: Add Streaming Silver
- New SilverSubscriber processes events directly
- Can run alongside existing batch ETL initially
- Validate data consistency

### Phase 3: Add Processors
- Threshold, ML, etc. as subscribers
- Each with own config

### Phase 4: Deprecate Batch ETL
- Once streaming Silver is validated
- Keep batch ETL for backfill/reprocessing only

---

## Scope Revision

| Component | Lines | Notes |
|-----------|-------|-------|
| `event_bus.rs` | ~100 | Broadcast channel wrapper |
| `subscribers/traits.rs` | ~80 | Subscriber trait |
| `subscribers/coordinator.rs` | ~200 | Manages subscriber lifecycle |
| `subscribers/bronze.rs` | ~150 | Refactor from RawStorageWriter |
| `subscribers/silver.rs` | ~300 | Streaming ETL (new) |
| `subscribers/processor.rs` | ~200 | Generic processor subscriber |
| Processor implementations | ~1,000 | Threshold, ML, etc. |
| Config/integration | ~500 | |
| **Total** | **~2,500** | Simpler than two-path design |

---

## Summary

The unified event bus is cleaner because:

1. **One concept**: Everything is a subscriber
2. **Bronze and Silver are peers**: Not a pipeline, parallel consumers
3. **Processors are just subscribers**: Same pattern, same config model
4. **Easy to extend**: S3 archive? New subscriber. Replication? New subscriber.
5. **Silver becomes streaming**: No more 5-minute batch ETL delay

The existing batch ETL (`silver-etl`) becomes a **backfill/reprocessing tool**, not the primary path to Silver.
