# Core Platform Infrastructure Analysis

**Analysis Date:** December 14, 2025
**Scope:** `neural-core/`, `core/`, `config-store/`, `data-staging/`

---

## 1. Generic Traits Architecture

### TimeSeriesPoint Trait

**File:** `/workspaces/neural-data-platform/core/src/traits.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub location_id: String,
    pub value: f64,
    pub tags: HashMap<String, String>,
}
```

**Assessment:** Core abstraction for all time series data. Used by AirQualityAdapter to convert domain-specific readings to generic points.

**E2E Ready:** YES

### Source Trait

**File:** `/workspaces/neural-data-platform/core/src/traits.rs`

```rust
#[async_trait]
pub trait Source: Send + Sync {
    async fn fetch(&self) -> CoreResult<Vec<TimeSeriesPoint>>;
    async fn health_check(&self) -> CoreResult<HealthStatus>;
}
```

**Implementations Available:**
- MQTT Source (`core/src/sources/mqtt.rs`)
- HTTP Polling Source (`core/src/sources/http_poll.rs`)
- Merge Source (`core/src/sources/merge.rs`)

**E2E Ready:** YES (trait exists, implementations need integration)

### Predictor Trait

**File:** `/workspaces/neural-data-platform/neural-core/src/traits/predictor.rs`

```rust
#[async_trait]
pub trait Predictor: Send + Sync {
    async fn predict(&self, market_data: &[MarketData]) -> Result<PredictionResult>;
    async fn train(&mut self, training_data: &[MarketData], config: &TrainingConfig) -> Result<ModelMetrics>;
    async fn evaluate(&self, test_data: &[MarketData]) -> Result<ModelMetrics>;
    async fn save_model(&self, path: &str) -> Result<()>;
    async fn load_model(&mut self, path: &str) -> Result<()>;
}
```

**Assessment:** Comprehensive ML pipeline. Currently uses `MarketData` type - needs adaptation for air quality.

**E2E Ready:** PARTIAL (needs air quality adaptation)

---

## 2. Storage Implementations

### Parquet Storage with WAL

**File:** `/workspaces/neural-data-platform/core/src/storage/parquet.rs` (638 lines)

**Architecture:**
```
TimeSeriesPoint → WAL (Write-Ahead Log) → Parquet Files
     ↓
Partitioned by location_id/year/month/day
     ↓
Columnar Format (Snappy compression)
```

**Features:**
- Partition Strategy: `data/{location_id}/year={YYYY}/month={MM}/day={DD}/readings.parquet`
- Write-Ahead Log for crash recovery
- Query with time range filtering
- Aggregations: Mean, Median, Min, Max, Sum, Count, Percentile
- Automatic temporal binning

**WAL Implementation:** `/workspaces/neural-data-platform/core/src/storage/wal.rs` (243 lines)
- Ensures durability before Parquet commit
- `replay()` recovers uncommitted data on startup
- `commit()` clears WAL after successful persistence

**E2E Ready:** YES

### Storage Trait Abstraction

**File:** `/workspaces/neural-data-platform/neural-core/src/traits/storage.rs`

**Supported Backends:**
- Memory
- Redis
- PostgreSQL
- TimescaleDB
- InfluxDB
- S3

**E2E Ready:** YES

---

## 3. Data Quality Scoring

**File:** `/workspaces/neural-data-platform/data-staging/src/quality_scorer.rs` (429 lines)

### Quality Metrics Structure

```rust
pub struct DataQualityMetrics {
    pub overall_score: f32,           // 0.0-1.0
    pub freshness_score: f32,         // Age-based
    pub completeness_score: f32,      // Field presence
    pub validity_score: f32,          // Range/logic checks
    pub missing_required_fields: u32,
    pub validation_errors: Vec<String>,
}
```

### Scoring Weights
- **Freshness:** 30% (0-5 sec = 1.0, 6-30s = 0.9, ... >1800s = 0.0)
- **Completeness:** 40% (required fields 70%, overall 30%)
- **Validity:** 30% (range checks, logical consistency)

**Assessment:** Currently optimized for market data. Needs adaptation for air quality thresholds (PM2.5, CO2, AQI).

**E2E Ready:** PARTIAL (needs air quality scoring rules)

---

## 4. Configuration Management

### ConfigStore Trait

**File:** `/workspaces/neural-data-platform/config-store/src/traits.rs` (241 lines)

```rust
#[async_trait]
pub trait ConfigStore: Send + Sync {
    async fn get(&self, path: &str) -> Result<ConfigValue, ConfigError>;
    async fn set(&self, path: &str, value: ConfigValue) -> Result<(), ConfigError>;
    async fn get_tree(&self, prefix: &str) -> Result<ConfigTree, ConfigError>;
    async fn get_version(&self, path: &str, version: u32) -> Result<ConfigValue, ConfigError>;
    async fn get_history(&self, path: &str) -> Result<Vec<ConfigVersion>, ConfigError>;
}
```

**Features:**
- Hierarchical path-based config: `/system/global/timeout`
- Version history (last 10 versions)
- Transaction support for atomic multi-op commits
- Path normalization utilities

**Implementations:**
- In-Memory Store (`stores/in_memory.rs`)
- Redis Store (`stores/redis.rs`)
- Secure In-Memory Store (`stores/secure_in_memory.rs`)

**E2E Ready:** YES

---

## 5. Dead Letter Queue Management

**File:** `/workspaces/neural-data-platform/data-staging/src/dlq_manager.rs` (240+ lines)

### DLQ Message Structure

```rust
pub struct DlqMessage {
    pub dlq_id: String,
    pub original_data: String,
    pub error_message: String,
    pub error_category: String,
    pub dlq_timestamp: i64,
    pub retry_count: u32,
    pub source: String,
    pub metadata: HashMap<String, String>,
}
```

**Features:**
- Backed by Redis Streams
- Configurable retention: 24 hours (default)
- Max size: 100,000 messages
- Error categorization for filtering

**E2E Ready:** YES

---

## 6. EventBus (Proto-Only)

**File:** `/workspaces/neural-data-platform/neural-core/src/eventbus/traits/event_bus.rs` (118 lines)

```rust
#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish<T: ProtoMessage + Default>(
        &self,
        channel: &str,
        event: ProtoEvent<T>,
    ) -> Result<EventId, EventBusError>;

    async fn subscribe<T: ProtoMessage + Default>(
        &self,
        channels: &[String],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn ProtoEventSubscriber<T>>, EventBusError>;
}
```

**Strict Proto Enforcement:** Legacy JSON methods return `ContractViolation` errors.

**Implementations:**
- In-Memory
- Proto In-Memory
- Redis
- Recording (for testing)

**E2E Ready:** YES

---

## 7. Platform Capability Matrix for Air Quality

| Capability | Status | E2E Ready | Notes |
|------------|--------|-----------|-------|
| Data Ingestion | FULL | YES | MQTT + HTTP Polling sources exist |
| Validation | PARTIAL | YES | Needs air quality thresholds |
| Quality Scoring | PARTIAL | PARTIAL | Market data focused |
| Time Series Storage | FULL | YES | Parquet + WAL |
| DLQ Management | FULL | YES | Redis Streams |
| Configuration | FULL | YES | Hierarchical, versioned |
| Event Publishing | FULL | YES | Proto-only, type-safe |
| Prediction | FULL | PARTIAL | Needs air quality adaptation |
| Domain Adapter | FULL | YES | AirQualityAdapter complete |

---

## 8. Key Files Summary

| Component | File Path | Lines | Purpose |
|-----------|-----------|-------|---------|
| Core Traits | `core/src/traits.rs` | 1,042 | Store, Source, Forecast abstractions |
| Parquet Storage | `core/src/storage/parquet.rs` | 638 | Partitioned columnar storage |
| Write-Ahead Log | `core/src/storage/wal.rs` | 243 | Crash recovery mechanism |
| Quality Scoring | `data-staging/src/quality_scorer.rs` | 429 | Multi-factor quality assessment |
| DLQ Manager | `data-staging/src/dlq_manager.rs` | 240+ | Failed message tracking |
| Config Store Traits | `config-store/src/traits.rs` | 241 | Configuration abstraction |
| EventBus Trait | `neural-core/src/eventbus/traits/event_bus.rs` | 118 | Proto-only pub/sub |

---

## 9. Integration Requirements for E2E

### Already Complete
1. TimeSeriesPoint trait matches air quality adapter output
2. Parquet storage handles partitioned time series
3. WAL ensures data durability
4. Configuration management ready

### Needs Integration Work
1. **Source trait implementation** for AirGradient MQTT
2. **Quality scorer** needs air quality thresholds
3. **Predictor trait** needs air quality data types
4. **EventBus** needs air quality proto definitions
