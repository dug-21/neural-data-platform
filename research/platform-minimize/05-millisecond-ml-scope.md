# Scope Analysis: Millisecond ML Processing

**Question**: What changes are needed to enable ML processing within milliseconds of data receipt?

**Assumption**: Ignoring source polling/receipt latency - focusing only on what happens AFTER data arrives.

---

## Current Data Flow (Post-Receipt)

```
MQTT Message Arrives
        │
        ▼ (microseconds)
┌───────────────────────────────────────┐
│  process_events() in mqtt/mod.rs:290  │
│  - JSON parsing (~100-500µs)          │
│  - cached_raw_points.lock().await     │
│  - push to cache                      │
└───────────────────┬───────────────────┘
                    │
        (accumulated in cache)
                    │
                    ▼ (polled periodically)
┌───────────────────────────────────────┐
│  IngestionCoordinator.fetch_raw()     │
│  - drains cache                       │
│  - sends to mpsc channel              │
└───────────────────┬───────────────────┘
                    │
                    ▼ (channel hop)
┌───────────────────────────────────────┐
│  RawStorageWriter.run()               │
│  - accumulates in buffer              │
│  - waits for batch_size OR timeout    │  ◄── BOTTLENECK: 5 second timeout
└───────────────────┬───────────────────┘
                    │
                    ▼ (on flush)
┌───────────────────────────────────────┐
│  ParquetStore.write_raw_batch()       │
│  - WAL append + fsync (10-50ms)       │  ◄── BOTTLENECK: disk I/O
│  - spawn_blocking for Parquet         │
└───────────────────────────────────────┘
```

**Current latency after receipt**: 5 seconds (batch timeout) + WAL/Parquet write

---

## Architecture for Millisecond ML

The key insight: **ML processing must happen BEFORE storage batching**, not after.

### Option A: Inline Processing Hook (Recommended)

Add a processing hook directly in `process_events()` before caching:

```
MQTT Message Arrives
        │
        ▼ (microseconds)
┌───────────────────────────────────────┐
│  process_events() in mqtt/mod.rs      │
│  - JSON parsing (~100-500µs)          │
│                                       │
│  ┌─────────────────────────────────┐  │
│  │  NEW: ML Processing Hook       │  │  ◄── INSERT HERE
│  │  - Call registered processors   │  │
│  │  - Async, non-blocking         │  │
│  │  - ~1-10ms for inference       │  │
│  └─────────────────────────────────┘  │
│                                       │
│  - push to cache (unchanged)          │
└───────────────────────────────────────┘
```

**Scope of Changes**:

| Component | Change | Effort |
|-----------|--------|--------|
| `core/src/sources/mqtt/mod.rs` | Add `processors: Vec<Arc<dyn Processor>>` field to `MqttSource` | Small |
| `core/src/sources/mqtt/mod.rs` | Call processors in `process_events()` after JSON parse | Small |
| `core/src/traits.rs` | Add new `Processor` trait | Small |
| ML integration | Implement `Processor` for your ML model | Medium |
| Configuration | Add processor config to stream YAML | Small |

**New Trait (Example)**:
```rust
// core/src/traits.rs
#[async_trait]
pub trait Processor: Send + Sync {
    /// Process a data point inline. Must complete quickly (<50ms).
    async fn process(&self, point: &RawDataPoint) -> ProcessorResult;

    /// Name for logging/metrics
    fn name(&self) -> &str;
}

pub struct ProcessorResult {
    pub predictions: Option<serde_json::Value>,
    pub alerts: Vec<Alert>,
    pub should_store: bool,  // Can skip storage for filtered data
}
```

**Code Change in mqtt/mod.rs (~20 lines)**:
```rust
// In process_events(), after JSON parsing, before cache push:
for processor in &processors {
    match processor.process(&raw_point).await {
        Ok(result) => {
            if let Some(preds) = result.predictions {
                // Emit to predictions channel or alert system
            }
        }
        Err(e) => warn!("Processor {} failed: {}", processor.name(), e),
    }
}
```

### Option B: Fan-Out Pattern

Fork the data to both storage AND ML processing in parallel:

```
MQTT Message Arrives
        │
        ▼
    [parse JSON]
        │
        ├──────────────────────┐
        ▼                      ▼
┌───────────────┐    ┌───────────────────┐
│  Storage Path │    │  ML Processing    │
│  (unchanged)  │    │  (new channel)    │
│  ~5s latency  │    │  ~1-10ms latency  │
└───────────────┘    └───────────────────┘
```

**Scope of Changes**:

| Component | Change | Effort |
|-----------|--------|--------|
| `MqttSource` | Add second `mpsc::Sender<RawDataPoint>` for ML path | Small |
| `process_events()` | Clone and send to both channels | Small |
| New binary/service | ML processor that consumes from ML channel | Medium |
| Configuration | Enable/disable ML path per stream | Small |

---

## Files That Need Changes

### Minimal Path (Option A - Inline Hook)

1. **`core/src/traits.rs`** - Add `Processor` trait (~30 lines)
2. **`core/src/sources/mqtt/mod.rs`** - Add processor support (~50 lines)
   - Add `processors` field to `MqttSource`
   - Add `with_processors()` builder method
   - Call processors in `process_events()`
3. **`core/src/config/stream_config.rs`** - Add processor config to schema (~20 lines)
4. **`apps/air-quality-app/src/coordinator/source_manager.rs`** - Wire up processors from config (~30 lines)

**Total**: ~130 lines of Rust across 4 files

### Full Path (Option B - Fan-Out)

Same as above, plus:
5. **New binary**: `apps/ml-processor/` - Dedicated ML service
6. **Shared memory/channel**: For fast IPC if ML is separate process

---

## Latency Breakdown: Before vs After

### Current (Storage-First)

| Step | Latency | Cumulative |
|------|---------|------------|
| JSON parse | ~500µs | 500µs |
| Cache lock + push | ~10µs | 510µs |
| Wait for fetch cycle | variable | ~100ms avg |
| Channel send | ~1µs | ~100ms |
| **Batch accumulation** | **up to 5s** | **~5s** |
| WAL + Parquet write | ~50ms | ~5.05s |

### With Inline ML Hook

| Step | Latency | Cumulative |
|------|---------|------------|
| JSON parse | ~500µs | 500µs |
| **ML inference** | **1-10ms** | **1-10ms** |
| Cache lock + push | ~10µs | ~10ms |
| (storage continues async) | - | - |

**ML results available**: 1-10ms after message receipt

---

## What You DON'T Need to Change

1. **Storage pipeline** - Keep batching for efficient Parquet writes
2. **ETL to Silver** - Still needed for analytics, just not for ML
3. **Channel architecture** - Works fine, just add a hook before it
4. **Parquet/WAL** - No changes needed

---

## Implementation Recommendations

### Phase 1: Add Processor Trait (Day 1)

```rust
// core/src/traits.rs
#[async_trait]
pub trait Processor: Send + Sync {
    async fn process(&self, point: &RawDataPoint) -> Result<ProcessorOutput, CoreError>;
    fn name(&self) -> &str;
}
```

### Phase 2: Wire into MqttSource (Day 1-2)

```rust
// core/src/sources/mqtt/mod.rs
pub struct MqttSource {
    // ... existing fields ...
    processors: Vec<Arc<dyn Processor + Send + Sync>>,
}

// In process_events():
for processor in &processors {
    let _ = processor.process(&raw_point).await;  // Fire and forget for speed
}
```

### Phase 3: Implement Your ML Processor (Day 2+)

```rust
// Your ML crate
pub struct AirQualityMLProcessor {
    model: Arc<YourModelType>,
    output_tx: mpsc::Sender<Prediction>,
}

#[async_trait]
impl Processor for AirQualityMLProcessor {
    async fn process(&self, point: &RawDataPoint) -> Result<ProcessorOutput, CoreError> {
        let features = extract_features(&point.raw_payload);
        let prediction = self.model.predict(&features);
        self.output_tx.send(prediction).await?;
        Ok(ProcessorOutput::default())
    }
}
```

---

## Summary

| Aspect | Scope |
|--------|-------|
| **Core library changes** | ~130 lines across 4 files |
| **New trait** | `Processor` with async `process()` method |
| **Primary insertion point** | `mqtt/mod.rs:process_events()` line ~324 |
| **Latency target** | 1-10ms (depends on your ML model) |
| **Breaking changes** | None (additive only) |
| **Config changes** | Optional processor list in stream YAML |

The architecture is already well-structured for this. The key is inserting the ML hook **before** the batching/storage pipeline, not trying to speed up the storage pipeline itself.
