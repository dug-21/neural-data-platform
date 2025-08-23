# Shared Component Interface Contracts
## Clean Boundaries and Integration Specifications

### Executive Summary

This document defines the **interface contracts** between all shared infrastructure components in the V2 MVP architecture. These contracts ensure **loose coupling**, **clear boundaries**, and **independent testability** while enabling **seamless integration** across the entire system.

---

## 1. Event Bus Interface Contract

### 1.1 EventBus Trait Definition

#### Core Interface
```rust
// src/interfaces/event_bus.rs
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish a single message to a stream
    async fn publish(
        &self,
        stream: &str,
        message: StreamMessage,
    ) -> Result<MessageId, EventBusError>;
    
    /// Publish multiple messages in a batch
    async fn publish_batch(
        &self,
        stream: &str,
        messages: Vec<StreamMessage>,
    ) -> Result<Vec<MessageId>, EventBusError>;
    
    /// Subscribe to a stream with a consumer group
    async fn subscribe(
        &self,
        stream: &str,
        group: &str,
        consumer: &str,
    ) -> Result<Box<dyn MessageStream>, EventBusError>;
    
    /// Get stream information
    async fn get_stream_info(&self, stream: &str) -> Result<StreamInfo, EventBusError>;
    
    /// Acknowledge message processing
    async fn acknowledge(&self, stream: &str, group: &str, message_id: &str) -> Result<(), EventBusError>;
    
    /// Health check
    async fn health_check(&self) -> Result<HealthStatus, EventBusError>;
}

/// Message stream for consuming
#[async_trait]
pub trait MessageStream: Send {
    /// Read next batch of messages
    async fn next_batch(&mut self) -> Result<Vec<ReceivedMessage>, EventBusError>;
    
    /// Get consumer information
    async fn get_consumer_info(&self) -> Result<ConsumerInfo, EventBusError>;
    
    /// Close the stream
    async fn close(&mut self) -> Result<(), EventBusError>;
}

/// Standard message format for all event bus communications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessage {
    pub message_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub stream: String,
    pub event_type: String,
    pub source: String,
    pub payload: serde_json::Value,
    pub metadata: MessageMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub correlation_id: Option<Uuid>,
    pub causation_id: Option<Uuid>,
    pub priority: MessagePriority,
    pub schema_version: String,
    pub retry_count: u32,
    pub ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePriority {
    Critical,   // Trading decisions, alerts
    High,       // Market data, predictions
    Normal,     // General events
    Low,        // Batch processing, cleanup
}

#[derive(Debug, Clone)]
pub struct ReceivedMessage {
    pub id: String,
    pub stream: String,
    pub message: StreamMessage,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub name: String,
    pub length: u64,
    pub consumer_groups: Vec<String>,
    pub first_entry_id: String,
    pub last_entry_id: String,
}

#[derive(Debug, Clone)]
pub struct ConsumerInfo {
    pub group: String,
    pub consumer: String,
    pub pending_count: u64,
    pub last_delivered_id: String,
}

pub type MessageId = String;
```

#### Contract Guarantees
```yaml
Delivery Guarantees:
  - At-least-once delivery for acknowledged messages
  - Message ordering within stream partitions
  - Persistence across service restarts
  - Consumer group state management

Performance Guarantees:
  - Publish latency: <10ms for normal priority
  - Batch publish: >1000 messages/second
  - Consumer lag monitoring: <100ms update frequency
  - Stream info queries: <50ms response time

Error Handling:
  - Automatic retry with exponential backoff
  - Dead letter queue for failed messages
  - Circuit breaker for downstream failures
  - Graceful degradation under load

Interface Stability:
  - Backward compatible message format
  - Versioned schema evolution
  - Optional field additions only
  - Deprecation warnings for 2 major versions
```

### 1.2 Message Routing Contracts

#### Stream Classification
```rust
// src/interfaces/stream_routing.rs
pub struct StreamRouter {
    routing_rules: HashMap<String, RoutingRule>,
}

#[derive(Debug, Clone)]
pub struct RoutingRule {
    pub stream_pattern: String,
    pub partitioning_strategy: PartitioningStrategy,
    pub retention_policy: RetentionPolicy,
    pub consumer_groups: Vec<ConsumerGroupRule>,
}

#[derive(Debug, Clone)]
pub enum PartitioningStrategy {
    None,                           // Single partition
    BySymbol,                       // Partition by symbol field
    ByHash(String),                 // Hash-based on field
    Custom(fn(&StreamMessage) -> String), // Custom partitioning function
}

#[derive(Debug, Clone)]
pub struct ConsumerGroupRule {
    pub group_name: String,
    pub processing_type: ProcessingType,
    pub max_consumers: u32,
    pub auto_scaling: bool,
}

#[derive(Debug, Clone)]
pub enum ProcessingType {
    RealTime,    // Low latency, immediate processing
    Batch,       // High throughput, batched processing
    Analytics,   // Background processing, can have lag
}

// Standard stream definitions
pub const MARKET_DATA_STREAM: &str = "trading.market-data";
pub const NEURAL_PREDICTIONS_STREAM: &str = "trading.predictions";
pub const TRADING_DECISIONS_STREAM: &str = "trading.decisions";
pub const EXECUTION_RESULTS_STREAM: &str = "trading.executions";
pub const SYSTEM_EVENTS_STREAM: &str = "system.events";
pub const HEALTH_CHECKS_STREAM: &str = "system.health";

impl StreamRouter {
    pub fn get_stream_for_event(event_type: &str, symbol: Option<&str>) -> String {
        match event_type {
            "market_data" => MARKET_DATA_STREAM.to_string(),
            "prediction" => NEURAL_PREDICTIONS_STREAM.to_string(),
            "trading_decision" => TRADING_DECISIONS_STREAM.to_string(),
            "execution_result" => EXECUTION_RESULTS_STREAM.to_string(),
            "health_check" => HEALTH_CHECKS_STREAM.to_string(),
            _ => SYSTEM_EVENTS_STREAM.to_string(),
        }
    }
    
    pub fn get_partition_key(message: &StreamMessage) -> Option<String> {
        // Extract symbol or use message_id for partitioning
        message.payload.get("symbol")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| Some(message.message_id.to_string()))
    }
}
```

---

## 2. Storage Interface Contract

### 2.1 Storage Repository Pattern

#### Generic Repository Interface
```rust
// src/interfaces/storage.rs
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait Repository<T, K>: Send + Sync 
where
    T: Send + Sync,
    K: Send + Sync,
{
    /// Insert a single entity
    async fn insert(&self, entity: &T) -> Result<K, StorageError>;
    
    /// Insert multiple entities in a batch
    async fn insert_batch(&self, entities: &[T]) -> Result<Vec<K>, StorageError>;
    
    /// Find entity by primary key
    async fn find_by_id(&self, id: &K) -> Result<Option<T>, StorageError>;
    
    /// Find entities by criteria
    async fn find_by_criteria(&self, criteria: &SearchCriteria) -> Result<Vec<T>, StorageError>;
    
    /// Update existing entity
    async fn update(&self, id: &K, entity: &T) -> Result<(), StorageError>;
    
    /// Delete entity
    async fn delete(&self, id: &K) -> Result<(), StorageError>;
    
    /// Count entities matching criteria
    async fn count(&self, criteria: &SearchCriteria) -> Result<u64, StorageError>;
}

/// Time-series specific repository for market data
#[async_trait]
pub trait TimeSeriesRepository<T>: Repository<T, TimeSeriesKey> 
where
    T: Send + Sync,
{
    /// Insert time-series data with automatic time partitioning
    async fn insert_time_series(
        &self,
        timestamp: DateTime<Utc>,
        symbol: &str,
        data: &T,
    ) -> Result<TimeSeriesKey, StorageError>;
    
    /// Query time-series data by time range
    async fn query_time_range(
        &self,
        symbol: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        limit: Option<u32>,
    ) -> Result<Vec<TimeSeriesEntry<T>>, StorageError>;
    
    /// Get latest data point for symbol
    async fn get_latest(&self, symbol: &str) -> Result<Option<TimeSeriesEntry<T>>, StorageError>;
    
    /// Get aggregated data (OHLC, averages, etc.)
    async fn get_aggregated(
        &self,
        symbol: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        interval: AggregationInterval,
    ) -> Result<Vec<AggregatedEntry<T>>, StorageError>;
}

#[derive(Debug, Clone)]
pub struct TimeSeriesKey {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct TimeSeriesEntry<T> {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub data: T,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct AggregatedEntry<T> {
    pub interval_start: DateTime<Utc>,
    pub interval_end: DateTime<Utc>,
    pub symbol: String,
    pub aggregated_data: T,
    pub sample_count: u32,
}

#[derive(Debug, Clone)]
pub enum AggregationInterval {
    Minute,
    FiveMinutes,
    FifteenMinutes,
    Hour,
    Day,
}

#[derive(Debug, Clone)]
pub struct SearchCriteria {
    pub filters: HashMap<String, FilterValue>,
    pub sort_by: Option<String>,
    pub sort_order: SortOrder,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone)]
pub enum FilterValue {
    Equals(serde_json::Value),
    GreaterThan(serde_json::Value),
    LessThan(serde_json::Value),
    In(Vec<serde_json::Value>),
    Between(serde_json::Value, serde_json::Value),
    Like(String),
}

#[derive(Debug, Clone)]
pub enum SortOrder {
    Ascending,
    Descending,
}
```

#### Domain-Specific Repository Contracts
```rust
// src/interfaces/domain_repositories.rs

/// Market data repository contract
#[async_trait]
pub trait MarketDataRepository: TimeSeriesRepository<MarketDataPoint> {
    /// Get real-time market data stream
    async fn get_real_time_stream(
        &self,
        symbols: Vec<String>,
    ) -> Result<Box<dyn Stream<Item = MarketDataPoint>>, StorageError>;
    
    /// Get OHLCV bars for time period
    async fn get_ohlcv_bars(
        &self,
        symbol: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        interval: BarInterval,
    ) -> Result<Vec<OHLCVBar>, StorageError>;
}

/// Neural predictions repository contract
#[async_trait] 
pub trait PredictionRepository: TimeSeriesRepository<NeuralPrediction> {
    /// Get latest prediction for symbol
    async fn get_latest_prediction(
        &self,
        symbol: &str,
        model_id: &str,
    ) -> Result<Option<NeuralPrediction>, StorageError>;
    
    /// Get prediction accuracy metrics
    async fn get_accuracy_metrics(
        &self,
        model_id: &str,
        time_range: (DateTime<Utc>, DateTime<Utc>),
    ) -> Result<AccuracyMetrics, StorageError>;
    
    /// Store prediction with outcome for training
    async fn store_prediction_with_outcome(
        &self,
        prediction: &NeuralPrediction,
        actual_outcome: f64,
    ) -> Result<(), StorageError>;
}

/// Trading decisions repository contract
#[async_trait]
pub trait TradingRepository: TimeSeriesRepository<TradingDecision> {
    /// Get open positions
    async fn get_open_positions(&self) -> Result<Vec<Position>, StorageError>;
    
    /// Get trading performance metrics
    async fn get_performance_metrics(
        &self,
        time_range: (DateTime<Utc>, DateTime<Utc>),
    ) -> Result<PerformanceMetrics, StorageError>;
    
    /// Get risk metrics
    async fn get_risk_metrics(&self) -> Result<RiskMetrics, StorageError>;
}

// Data structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataPoint {
    pub symbol: String,
    pub price: Decimal,
    pub volume: u64,
    pub bid: Option<Decimal>,
    pub ask: Option<Decimal>,
    pub timestamp: DateTime<Utc>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralPrediction {
    pub model_id: String,
    pub model_version: String,
    pub symbol: String,
    pub prediction: f64,
    pub confidence: Option<f64>,
    pub features: HashMap<String, f64>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingDecision {
    pub decision_id: Uuid,
    pub symbol: String,
    pub action: TradingAction,
    pub quantity: u32,
    pub price: Option<Decimal>,
    pub reasoning: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradingAction {
    Buy,
    Sell,
    Hold,
}
```

### 2.2 Storage Contract Guarantees

#### Data Consistency Guarantees
```yaml
ACID Properties:
  - Atomicity: All batch operations succeed or fail together
  - Consistency: All constraints and validations enforced
  - Isolation: Concurrent operations don't interfere
  - Durability: Committed data persists across failures

Time-Series Guarantees:
  - Monotonic timestamps within symbol partitions
  - Automatic time-based partitioning
  - Efficient range queries on time and symbol
  - Compressed storage for historical data

Performance Guarantees:
  - Insert latency: <10ms for single records
  - Batch insert: >10,000 records/second
  - Query latency: <100ms for recent data
  - Aggregation queries: <1 second for daily data

Data Retention:
  - Market data: 90 days online, compressed archive beyond
  - Predictions: 180 days for model validation
  - Trading decisions: 7 years for regulatory compliance
  - System events: 1 year for operational analysis
```

---

## 3. Configuration Interface Contract

### 3.1 Configuration Provider Interface

#### Hierarchical Configuration
```rust
// src/interfaces/configuration.rs
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait ConfigurationProvider: Send + Sync {
    /// Get configuration value by key path
    async fn get<T>(&self, key_path: &str) -> Result<Option<T>, ConfigError>
    where
        T: for<'de> Deserialize<'de> + Send;
    
    /// Get configuration with default value
    async fn get_or_default<T>(&self, key_path: &str, default: T) -> Result<T, ConfigError>
    where
        T: for<'de> Deserialize<'de> + Send;
    
    /// Set configuration value (if mutable)
    async fn set<T>(&self, key_path: &str, value: &T) -> Result<(), ConfigError>
    where
        T: Serialize + Send + Sync;
    
    /// Watch for configuration changes
    async fn watch(&self, key_path: &str) -> Result<Box<dyn ConfigWatcher>, ConfigError>;
    
    /// Validate configuration
    async fn validate(&self) -> Result<ValidationResult, ConfigError>;
    
    /// Get all configuration keys
    async fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>, ConfigError>;
}

#[async_trait]
pub trait ConfigWatcher: Send {
    /// Wait for next configuration change
    async fn next_change(&mut self) -> Result<ConfigChange, ConfigError>;
}

#[derive(Debug, Clone)]
pub struct ConfigChange {
    pub key_path: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
    pub change_type: ConfigChangeType,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum ConfigChangeType {
    Created,
    Updated,
    Deleted,
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ConfigValidationError>,
    pub warnings: Vec<ConfigValidationWarning>,
}

#[derive(Debug, Clone)]
pub struct ConfigValidationError {
    pub key_path: String,
    pub error_message: String,
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConfigValidationWarning {
    pub key_path: String,
    pub warning_message: String,
    pub impact: ImpactLevel,
}

#[derive(Debug, Clone)]
pub enum ImpactLevel {
    Low,
    Medium,
    High,
}
```

#### Feature Flag Interface
```rust
// src/interfaces/feature_flags.rs

#[async_trait]
pub trait FeatureFlagProvider: Send + Sync {
    /// Check if feature is enabled
    async fn is_enabled(&self, feature_name: &str) -> Result<bool, FeatureFlagError>;
    
    /// Check if feature is enabled for specific context
    async fn is_enabled_for_context(
        &self,
        feature_name: &str,
        context: &FeatureContext,
    ) -> Result<bool, FeatureFlagError>;
    
    /// Get feature configuration
    async fn get_feature_config<T>(
        &self,
        feature_name: &str,
    ) -> Result<Option<T>, FeatureFlagError>
    where
        T: for<'de> Deserialize<'de> + Send;
    
    /// List all features
    async fn list_features(&self) -> Result<Vec<FeatureDefinition>, FeatureFlagError>;
    
    /// Watch for feature flag changes
    async fn watch_feature(
        &self,
        feature_name: &str,
    ) -> Result<Box<dyn FeatureFlagWatcher>, FeatureFlagError>;
}

#[derive(Debug, Clone)]
pub struct FeatureContext {
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub environment: String,
    pub timestamp: DateTime<Utc>,
    pub custom_attributes: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct FeatureDefinition {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub rollout_percentage: Option<f64>,
    pub conditions: Vec<FeatureCondition>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct FeatureCondition {
    pub attribute: String,
    pub operator: ConditionOperator,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone)]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    In,
    NotIn,
    Contains,
    StartsWith,
    EndsWith,
}

// Standard feature flags for MVP
pub const FEATURE_NEURAL_PREDICTIONS: &str = "neural_predictions";
pub const FEATURE_PAPER_TRADING: &str = "paper_trading";
pub const FEATURE_LIVE_TRADING: &str = "live_trading";
pub const FEATURE_ADVANCED_METRICS: &str = "advanced_metrics";
pub const FEATURE_DEBUG_LOGGING: &str = "debug_logging";
pub const FEATURE_MODEL_RETRAINING: &str = "model_retraining";
pub const FEATURE_RISK_OVERRIDE: &str = "risk_override";
```

---

## 4. Monitoring Interface Contract

### 4.1 Metrics Collection Interface

#### Metrics Provider
```rust
// src/interfaces/metrics.rs
use async_trait::async_trait;
use std::collections::HashMap;

#[async_trait]
pub trait MetricsProvider: Send + Sync {
    /// Record a counter metric
    async fn increment_counter(
        &self,
        name: &str,
        labels: HashMap<String, String>,
        value: f64,
    ) -> Result<(), MetricsError>;
    
    /// Record a gauge metric
    async fn set_gauge(
        &self,
        name: &str,
        labels: HashMap<String, String>,
        value: f64,
    ) -> Result<(), MetricsError>;
    
    /// Record a histogram metric
    async fn record_histogram(
        &self,
        name: &str,
        labels: HashMap<String, String>,
        value: f64,
    ) -> Result<(), MetricsError>;
    
    /// Record timing metric
    async fn record_timing(
        &self,
        name: &str,
        labels: HashMap<String, String>,
        duration: Duration,
    ) -> Result<(), MetricsError>;
    
    /// Get current metric value
    async fn get_metric(
        &self,
        name: &str,
        labels: HashMap<String, String>,
    ) -> Result<Option<MetricValue>, MetricsError>;
    
    /// Export all metrics
    async fn export_metrics(&self) -> Result<String, MetricsError>;
}

#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(f64),
    Gauge(f64),
    Histogram(HistogramData),
}

#[derive(Debug, Clone)]
pub struct HistogramData {
    pub count: u64,
    pub sum: f64,
    pub buckets: Vec<HistogramBucket>,
}

#[derive(Debug, Clone)]
pub struct HistogramBucket {
    pub upper_bound: f64,
    pub count: u64,
}

/// Standard metrics for MVP components
pub struct StandardMetrics {
    // Event Bus Metrics
    pub stream_messages_published: &'static str,
    pub stream_consumer_lag: &'static str,
    pub stream_publish_latency: &'static str,
    pub stream_consume_latency: &'static str,
    
    // Storage Metrics
    pub db_connections_active: &'static str,
    pub db_query_latency: &'static str,
    pub db_operations_total: &'static str,
    
    // Application Metrics
    pub neural_predictions_total: &'static str,
    pub trading_decisions_total: &'static str,
    pub model_accuracy: &'static str,
    pub position_count: &'static str,
    pub portfolio_value: &'static str,
    
    // System Metrics
    pub memory_usage_bytes: &'static str,
    pub cpu_usage_percent: &'static str,
    pub error_count_total: &'static str,
}

impl StandardMetrics {
    pub const fn new() -> Self {
        Self {
            stream_messages_published: "stream_messages_published_total",
            stream_consumer_lag: "stream_consumer_lag_messages",
            stream_publish_latency: "stream_publish_latency_seconds",
            stream_consume_latency: "stream_consume_latency_seconds",
            
            db_connections_active: "db_connections_active",
            db_query_latency: "db_query_latency_seconds",
            db_operations_total: "db_operations_total",
            
            neural_predictions_total: "neural_predictions_total",
            trading_decisions_total: "trading_decisions_total",
            model_accuracy: "model_accuracy_ratio",
            position_count: "positions_active_count",
            portfolio_value: "portfolio_value_usd",
            
            memory_usage_bytes: "memory_usage_bytes",
            cpu_usage_percent: "cpu_usage_percent",
            error_count_total: "errors_total",
        }
    }
}
```

#### Health Monitoring Interface
```rust
// src/interfaces/health.rs

#[async_trait]
pub trait HealthProvider: Send + Sync {
    /// Get current health status
    async fn get_health(&self) -> Result<HealthReport, HealthError>;
    
    /// Register a health check
    async fn register_check(
        &self,
        name: &str,
        checker: Box<dyn HealthChecker>,
    ) -> Result<(), HealthError>;
    
    /// Unregister a health check
    async fn unregister_check(&self, name: &str) -> Result<(), HealthError>;
    
    /// Get individual check status
    async fn get_check_status(&self, name: &str) -> Result<Option<HealthCheckResult>, HealthError>;
    
    /// Watch for health changes
    async fn watch_health(&self) -> Result<Box<dyn HealthWatcher>, HealthError>;
}

#[async_trait]
pub trait HealthChecker: Send + Sync {
    /// Perform health check
    async fn check(&self) -> HealthCheckResult;
    
    /// Get check configuration
    fn get_config(&self) -> HealthCheckConfig;
}

#[async_trait]
pub trait HealthWatcher: Send {
    /// Wait for next health change
    async fn next_change(&mut self) -> Result<HealthChange, HealthError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub overall_status: HealthStatus,
    pub checks: HashMap<String, HealthCheckResult>,
    pub timestamp: DateTime<Utc>,
    pub system_info: SystemInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub name: String,
    pub status: HealthStatus,
    pub message: String,
    pub details: HashMap<String, serde_json::Value>,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
    pub error_count: u32,
    pub last_success: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Critical,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    pub timeout: Duration,
    pub interval: Duration,
    pub max_failures: u32,
    pub retry_delay: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub version: String,
    pub uptime: Duration,
    pub memory_usage: MemoryInfo,
    pub cpu_usage: f64,
    pub disk_usage: Vec<DiskInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub mount_point: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
}
```

---

## 5. Cross-Component Integration Patterns

### 5.1 Service Discovery Interface

#### Service Registry Contract
```rust
// src/interfaces/service_discovery.rs

#[async_trait]
pub trait ServiceRegistry: Send + Sync {
    /// Register a service instance
    async fn register_service(
        &self,
        service: ServiceDefinition,
    ) -> Result<ServiceInstance, ServiceDiscoveryError>;
    
    /// Unregister a service instance
    async fn unregister_service(&self, instance_id: &str) -> Result<(), ServiceDiscoveryError>;
    
    /// Discover services by name
    async fn discover_services(
        &self,
        service_name: &str,
    ) -> Result<Vec<ServiceInstance>, ServiceDiscoveryError>;
    
    /// Get healthy service instances
    async fn get_healthy_instances(
        &self,
        service_name: &str,
    ) -> Result<Vec<ServiceInstance>, ServiceDiscoveryError>;
    
    /// Watch for service changes
    async fn watch_service(
        &self,
        service_name: &str,
    ) -> Result<Box<dyn ServiceWatcher>, ServiceDiscoveryError>;
    
    /// Update service health
    async fn update_health(
        &self,
        instance_id: &str,
        health: HealthStatus,
    ) -> Result<(), ServiceDiscoveryError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDefinition {
    pub name: String,
    pub version: String,
    pub endpoints: Vec<ServiceEndpoint>,
    pub metadata: HashMap<String, String>,
    pub health_check: Option<HealthCheckDefinition>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInstance {
    pub id: String,
    pub definition: ServiceDefinition,
    pub address: String,
    pub port: u16,
    pub status: ServiceStatus,
    pub registered_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEndpoint {
    pub name: String,
    pub protocol: Protocol,
    pub path: String,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Protocol {
    Http,
    Https,
    Grpc,
    WebSocket,
    Tcp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceStatus {
    Starting,
    Healthy,
    Degraded,
    Unhealthy,
    Stopping,
}

// Standard service definitions
pub const EVENT_BUS_SERVICE: &str = "neural-trader-event-bus";
pub const STORAGE_SERVICE: &str = "neural-trader-storage";
pub const CONFIG_SERVICE: &str = "neural-trader-config";
pub const METRICS_SERVICE: &str = "neural-trader-metrics";
pub const HEALTH_SERVICE: &str = "neural-trader-health";
```

### 5.2 Error Handling Contracts

#### Standardized Error Types
```rust
// src/interfaces/errors.rs
use thiserror::Error;

/// Base error type for all shared components
#[derive(Error, Debug)]
pub enum SharedComponentError {
    #[error("Event bus error: {0}")]
    EventBus(#[from] EventBusError),
    
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    
    #[error("Configuration error: {0}")]
    Configuration(#[from] ConfigError),
    
    #[error("Metrics error: {0}")]
    Metrics(#[from] MetricsError),
    
    #[error("Health check error: {0}")]
    Health(#[from] HealthError),
    
    #[error("Service discovery error: {0}")]
    ServiceDiscovery(#[from] ServiceDiscoveryError),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Timeout error: operation timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    
    #[error("Rate limit exceeded: {message}")]
    RateLimit { message: String },
    
    #[error("Resource exhausted: {resource}")]
    ResourceExhausted { resource: String },
    
    #[error("Invalid configuration: {message}")]
    InvalidConfiguration { message: String },
    
    #[error("Internal error: {message}")]
    Internal { message: String },
}

/// Error handling strategy contract
#[async_trait]
pub trait ErrorHandler: Send + Sync {
    /// Handle recoverable errors
    async fn handle_recoverable_error(
        &self,
        error: &SharedComponentError,
        context: &ErrorContext,
    ) -> Result<ErrorAction, SharedComponentError>;
    
    /// Handle fatal errors
    async fn handle_fatal_error(
        &self,
        error: &SharedComponentError,
        context: &ErrorContext,
    ) -> ErrorAction;
    
    /// Check if error is recoverable
    fn is_recoverable(&self, error: &SharedComponentError) -> bool;
    
    /// Get retry strategy for error
    fn get_retry_strategy(&self, error: &SharedComponentError) -> Option<RetryStrategy>;
}

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub component: String,
    pub operation: String,
    pub attempt_count: u32,
    pub correlation_id: Option<Uuid>,
    pub timestamp: DateTime<Utc>,
    pub additional_context: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub enum ErrorAction {
    Retry(RetryStrategy),
    Fallback(String),
    CircuitBreaker,
    Alert(AlertLevel),
    Shutdown,
    Continue,
}

#[derive(Debug, Clone)]
pub struct RetryStrategy {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter: bool,
}

#[derive(Debug, Clone)]
pub enum AlertLevel {
    Info,
    Warning,
    Error,
    Critical,
}
```

---

## 6. Contract Validation and Testing

### 6.1 Interface Compliance Testing

#### Contract Test Framework
```rust
// src/interfaces/testing.rs
use async_trait::async_trait;

/// Contract test suite for shared components
#[async_trait]
pub trait ContractTest: Send + Sync {
    /// Test basic functionality
    async fn test_basic_functionality(&self) -> Result<(), TestError>;
    
    /// Test error handling
    async fn test_error_handling(&self) -> Result<(), TestError>;
    
    /// Test performance characteristics
    async fn test_performance(&self) -> Result<PerformanceTestResult, TestError>;
    
    /// Test concurrent access
    async fn test_concurrency(&self) -> Result<(), TestError>;
    
    /// Test resource cleanup
    async fn test_cleanup(&self) -> Result<(), TestError>;
}

#[derive(Debug, Clone)]
pub struct PerformanceTestResult {
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub throughput_per_sec: f64,
    pub error_rate: f64,
    pub resource_usage: ResourceUsage,
}

#[derive(Debug, Clone)]
pub struct ResourceUsage {
    pub peak_memory_mb: f64,
    pub avg_cpu_percent: f64,
    pub network_io_mb: f64,
    pub disk_io_mb: f64,
}

/// Event bus contract tests
pub struct EventBusContractTest {
    event_bus: Box<dyn EventBus>,
}

#[async_trait]
impl ContractTest for EventBusContractTest {
    async fn test_basic_functionality(&self) -> Result<(), TestError> {
        // Test message publish and consume
        let test_message = StreamMessage {
            message_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            stream: "test-stream".to_string(),
            event_type: "test-event".to_string(),
            source: "test-source".to_string(),
            payload: json!({"test": "data"}),
            metadata: MessageMetadata::default(),
        };
        
        // Publish message
        let message_id = self.event_bus.publish("test-stream", test_message.clone()).await?;
        assert!(!message_id.is_empty());
        
        // Subscribe and consume
        let mut stream = self.event_bus.subscribe(
            "test-stream",
            "test-group",
            "test-consumer"
        ).await?;
        
        let messages = stream.next_batch().await?;
        assert!(!messages.is_empty());
        
        Ok(())
    }
    
    async fn test_performance(&self) -> Result<PerformanceTestResult, TestError> {
        let start = std::time::Instant::now();
        let message_count = 1000;
        
        // Publish messages
        for i in 0..message_count {
            let message = StreamMessage {
                message_id: Uuid::new_v4(),
                timestamp: Utc::now(),
                stream: "perf-test".to_string(),
                event_type: "perf-event".to_string(),
                source: "perf-source".to_string(),
                payload: json!({"index": i}),
                metadata: MessageMetadata::default(),
            };
            
            self.event_bus.publish("perf-test", message).await?;
        }
        
        let duration = start.elapsed();
        let throughput = message_count as f64 / duration.as_secs_f64();
        
        Ok(PerformanceTestResult {
            latency_p50_ms: duration.as_millis() as f64 / message_count as f64,
            latency_p95_ms: 0.0, // Would need proper measurement
            latency_p99_ms: 0.0,
            throughput_per_sec: throughput,
            error_rate: 0.0,
            resource_usage: ResourceUsage {
                peak_memory_mb: 0.0,
                avg_cpu_percent: 0.0,
                network_io_mb: 0.0,
                disk_io_mb: 0.0,
            },
        })
    }
    
    // Additional test implementations...
}
```

### 6.2 Integration Test Contracts

#### Cross-Component Integration Tests
```rust
// src/interfaces/integration_tests.rs

/// Integration test suite for component interactions
#[async_trait]
pub trait IntegrationTest: Send + Sync {
    /// Test end-to-end data flow
    async fn test_end_to_end_flow(&self) -> Result<(), TestError>;
    
    /// Test failure scenarios
    async fn test_failure_scenarios(&self) -> Result<(), TestError>;
    
    /// Test load handling
    async fn test_load_handling(&self) -> Result<LoadTestResult, TestError>;
    
    /// Test data consistency
    async fn test_data_consistency(&self) -> Result<(), TestError>;
}

/// Event Bus + Storage integration test
pub struct EventBusStorageIntegrationTest {
    event_bus: Box<dyn EventBus>,
    storage: Box<dyn Repository<MarketDataPoint, TimeSeriesKey>>,
}

#[async_trait]
impl IntegrationTest for EventBusStorageIntegrationTest {
    async fn test_end_to_end_flow(&self) -> Result<(), TestError> {
        // Create test market data
        let market_data = MarketDataPoint {
            symbol: "TEST".to_string(),
            price: Decimal::from_str("100.50").unwrap(),
            volume: 1000,
            bid: Some(Decimal::from_str("100.45").unwrap()),
            ask: Some(Decimal::from_str("100.55").unwrap()),
            timestamp: Utc::now(),
            source: "test".to_string(),
        };
        
        // Publish to event bus
        let message = StreamMessage {
            message_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            stream: MARKET_DATA_STREAM.to_string(),
            event_type: "market_data".to_string(),
            source: "test".to_string(),
            payload: serde_json::to_value(&market_data)?,
            metadata: MessageMetadata::default(),
        };
        
        let message_id = self.event_bus.publish(MARKET_DATA_STREAM, message).await?;
        
        // Wait for processing (in real test, would use proper synchronization)
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Verify data was stored
        let stored_data = self.storage.find_by_criteria(&SearchCriteria {
            filters: [
                ("symbol".to_string(), FilterValue::Equals(json!("TEST"))),
            ].into_iter().collect(),
            sort_by: Some("timestamp".to_string()),
            sort_order: SortOrder::Descending,
            limit: Some(1),
            offset: None,
        }).await?;
        
        assert!(!stored_data.is_empty());
        assert_eq!(stored_data[0].symbol, "TEST");
        
        Ok(())
    }
}
```

---

## 7. Contract Versioning and Evolution

### 7.1 Version Management Strategy

#### Interface Versioning
```rust
// src/interfaces/versioning.rs

/// Version management for interface contracts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre_release: Option<String>,
}

impl InterfaceVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
            pre_release: None,
        }
    }
    
    pub fn is_compatible_with(&self, other: &InterfaceVersion) -> bool {
        // Semantic versioning compatibility rules
        self.major == other.major && self.minor >= other.minor
    }
}

/// Contract evolution strategy
#[derive(Debug, Clone)]
pub enum EvolutionStrategy {
    /// Backward compatible changes only
    BackwardCompatible,
    /// Forward compatible changes only  
    ForwardCompatible,
    /// Breaking changes allowed
    Breaking,
}

/// Interface deprecation notice
#[derive(Debug, Clone)]
pub struct DeprecationNotice {
    pub interface_name: String,
    pub version: InterfaceVersion,
    pub deprecation_date: DateTime<Utc>,
    pub removal_date: DateTime<Utc>,
    pub replacement: Option<String>,
    pub migration_guide: String,
}
```

### 7.2 Migration Support

#### Interface Migration Framework
```yaml
Migration Guidelines:
  Version Support:
    - Current version: Full support
    - Previous major version: Bug fixes only
    - Deprecated versions: 6 months notice before removal
    
  Breaking Changes:
    - Major version increment required
    - Migration guide must be provided
    - Backward compatibility layer when possible
    - Automated migration tools preferred
    
  Non-Breaking Changes:
    - Minor version increment for new features
    - Patch version for bug fixes
    - All changes must maintain existing contracts
    
  Deprecation Process:
    1. Announce deprecation with 6 months notice
    2. Provide migration guide and tools
    3. Mark deprecated methods with warnings
    4. Remove in next major version
```

---

## Summary

These interface contracts establish **clear boundaries** between all shared infrastructure components while ensuring **loose coupling** and **independent testability**. The contracts provide:

### Key Benefits

1. **Clear Separation of Concerns**: Each component has well-defined responsibilities
2. **Technology Independence**: Implementations can be swapped without affecting consumers  
3. **Testing Isolation**: Components can be tested independently with mock implementations
4. **Evolution Support**: Versioned interfaces enable safe system evolution
5. **Error Handling**: Standardized error types and handling strategies
6. **Performance Guarantees**: Clear SLA definitions for each interface

### Implementation Priority

1. **Phase 1A**: Event Bus and Storage interfaces (critical path)
2. **Phase 1B**: Configuration and Metrics interfaces  
3. **Phase 1C**: Health monitoring and Service discovery
4. **Phase 1D**: Integration testing and validation

These contracts serve as the **foundation** for all subsequent development phases, ensuring that shared components can be built independently while maintaining seamless integration across the entire Neural Trader V2 MVP system.