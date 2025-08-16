# Universal Discovery Platform - Interface Specifications

## Overview

This document defines the precise interface contracts between layers in the Universal Discovery Platform. Each interface is designed for maximum modularity, testability, and independent evolution.

## Core Interface Definitions

### 1. Infrastructure Layer Interfaces

#### DataIngester Interface
```rust
use async_trait::async_trait;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

#[async_trait]
pub trait DataIngester: Send + Sync {
    /// Ingest data from a configured source
    async fn ingest(&self, source: DataSource) -> Result<StreamHandle, IngestionError>;
    
    /// Register a new data source with the ingester
    async fn register_source(&self, config: SourceConfig) -> Result<SourceId, IngestionError>;
    
    /// Remove a data source
    async fn unregister_source(&self, source_id: SourceId) -> Result<(), IngestionError>;
    
    /// Get health status of all sources
    async fn get_source_health(&self) -> Result<Vec<SourceHealth>, IngestionError>;
}

#[derive(Debug, Clone)]
pub struct DataSource {
    pub source_id: String,
    pub source_type: SourceType,
    pub connection_config: HashMap<String, String>,
    pub data_format: DataFormat,
}

#[derive(Debug, Clone)]
pub enum SourceType {
    WebSocket { url: String, auth: Option<AuthConfig> },
    RestApi { base_url: String, endpoints: Vec<String> },
    MessageQueue { broker: String, topics: Vec<String> },
    Database { connection_string: String, query: String },
    File { path: String, watch_mode: bool },
}

#[derive(Debug, Clone)]
pub struct SourceConfig {
    pub name: String,
    pub source: DataSource,
    pub ingestion_rate: IngestionRate,
    pub retry_policy: RetryPolicy,
    pub quality_rules: Vec<QualityRule>,
}

pub type SourceId = String;
pub type StreamHandle = String;
```

#### ServiceCoordinator Interface
```rust
#[async_trait]
pub trait ServiceCoordinator: Send + Sync {
    /// Register a service with the coordinator
    async fn register_service(&self, service: ServiceInfo) -> Result<ServiceId, CoordinationError>;
    
    /// Discover services matching the filter
    async fn discover_services(&self, filter: ServiceFilter) -> Result<Vec<ServiceInfo>, CoordinationError>;
    
    /// Update service health status
    async fn update_health(&self, service_id: ServiceId, health: HealthStatus) -> Result<(), CoordinationError>;
    
    /// Subscribe to service changes
    async fn subscribe_changes(&self) -> Result<ServiceChangeStream, CoordinationError>;
}

#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub service_id: ServiceId,
    pub service_name: String,
    pub layer: ServiceLayer,
    pub endpoints: Vec<ServiceEndpoint>,
    pub capabilities: Vec<String>,
    pub resource_requirements: ResourceRequirements,
    pub health_check: HealthCheckConfig,
}

#[derive(Debug, Clone)]
pub enum ServiceLayer {
    Infrastructure,
    DataPlatform,
    DiscoveryEngine,
    ExecutionDomain { domain: String },
}

pub type ServiceId = String;
```

### 2. Data Platform Layer Interfaces

#### TimeSeriesProcessor Interface
```rust
#[async_trait]
pub trait TimeSeriesProcessor: Send + Sync {
    /// Process a stream of time series data
    async fn process_stream(&self, input: DataStream) -> Result<ProcessedStream, ProcessingError>;
    
    /// Register a custom transformer
    fn register_transformer(&mut self, name: String, transformer: Box<dyn StreamTransformer>);
    
    /// Get processing statistics
    async fn get_stats(&self) -> Result<ProcessingStats, ProcessingError>;
    
    /// Configure processing pipeline
    async fn configure_pipeline(&self, config: PipelineConfig) -> Result<(), ProcessingError>;
}

#[async_trait]
pub trait StreamTransformer: Send + Sync {
    /// Transform a batch of time series points
    async fn transform(&self, points: Vec<TimeSeriesPoint>) -> Result<Vec<TimeSeriesPoint>, TransformError>;
    
    /// Get transformer metadata
    fn metadata(&self) -> TransformerMetadata;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub entity_id: String,
    pub metric_name: String,
    pub value: f64,
    pub metadata: HashMap<String, serde_json::Value>,
    pub quality_score: f64,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct DataStream {
    pub stream_id: String,
    pub entity_type: String,
    pub schema_version: String,
    pub points: Vec<TimeSeriesPoint>,
    pub stream_metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct ProcessedStream {
    pub original_stream_id: String,
    pub processed_stream_id: String,
    pub processing_timestamp: DateTime<Utc>,
    pub points: Vec<TimeSeriesPoint>,
    pub processing_metadata: ProcessingMetadata,
}
```

#### FeatureStore Interface
```rust
#[async_trait]
pub trait FeatureStore: Send + Sync {
    /// Store features for an entity
    async fn store_features(&self, entity_id: &str, features: FeatureVector) -> Result<(), StorageError>;
    
    /// Retrieve features for an entity within a time window
    async fn get_features(&self, entity_id: &str, window: TimeWindow) -> Result<FeatureMatrix, StorageError>;
    
    /// Store a batch of features
    async fn store_feature_batch(&self, batch: Vec<(String, FeatureVector)>) -> Result<(), StorageError>;
    
    /// Get feature schema for an entity type
    async fn get_schema(&self, entity_type: &str) -> Result<FeatureSchema, StorageError>;
    
    /// Register a new feature schema
    async fn register_schema(&self, schema: FeatureSchema) -> Result<(), StorageError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVector {
    pub entity_id: String,
    pub timestamp: DateTime<Utc>,
    pub features: HashMap<String, f64>,
    pub feature_metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct FeatureMatrix {
    pub entity_id: String,
    pub time_window: TimeWindow,
    pub feature_names: Vec<String>,
    pub values: Vec<Vec<f64>>, // [time][feature]
    pub timestamps: Vec<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct TimeWindow {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub granularity: Duration,
}
```

#### StreamRouter Interface
```rust
#[async_trait]
pub trait StreamRouter: Send + Sync {
    /// Publish data to a topic
    async fn publish(&self, topic: &str, data: &[u8]) -> Result<(), RoutingError>;
    
    /// Publish with routing key
    async fn publish_with_key(&self, topic: &str, key: &str, data: &[u8]) -> Result<(), RoutingError>;
    
    /// Subscribe to a topic pattern
    async fn subscribe(&self, pattern: &str) -> Result<StreamSubscription, RoutingError>;
    
    /// Create a consumer group
    async fn create_consumer_group(&self, group_id: &str, topics: Vec<String>) -> Result<ConsumerGroup, RoutingError>;
    
    /// Get stream statistics
    async fn get_stream_stats(&self, topic: &str) -> Result<StreamStats, RoutingError>;
}

#[derive(Debug)]
pub struct StreamSubscription {
    pub subscription_id: String,
    pub pattern: String,
    pub message_stream: Box<dyn Stream<Item = Result<Message, RoutingError>> + Send + Unpin>,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub topic: String,
    pub key: Option<String>,
    pub payload: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub headers: HashMap<String, String>,
}
```

### 3. Discovery Engine Layer Interfaces

#### PatternDiscovery Interface
```rust
#[async_trait]
pub trait PatternDiscovery: Send + Sync {
    /// Analyze a stream for patterns
    async fn analyze_stream(&self, stream: TimeSeriesStream) -> Result<Vec<Pattern>, AnalysisError>;
    
    /// Register a pattern detector
    fn register_detector(&mut self, name: String, detector: Box<dyn PatternDetector>);
    
    /// Get historical patterns for an entity
    async fn get_patterns(&self, entity_id: &str, window: TimeWindow) -> Result<Vec<Pattern>, AnalysisError>;
    
    /// Configure detection parameters
    async fn configure_detection(&self, config: DetectionConfig) -> Result<(), AnalysisError>;
}

#[async_trait]
pub trait PatternDetector: Send + Sync {
    /// Detect patterns in time series data
    async fn detect(&self, data: &TimeSeriesStream) -> Result<Vec<Pattern>, DetectionError>;
    
    /// Get detector configuration
    fn get_config(&self) -> DetectorConfig;
    
    /// Update detector parameters
    fn update_config(&mut self, config: DetectorConfig) -> Result<(), DetectionError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    pub pattern_id: String,
    pub pattern_type: PatternType,
    pub confidence: f64,
    pub time_window: TimeWindow,
    pub affected_entities: Vec<String>,
    pub pattern_data: HashMap<String, serde_json::Value>,
    pub detection_metadata: DetectionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternType {
    Anomaly {
        severity: f64,
        anomaly_type: AnomalyType,
    },
    Trend {
        direction: TrendDirection,
        strength: f64,
        duration: Duration,
    },
    Cycle {
        period: Duration,
        amplitude: f64,
        phase: f64,
    },
    Correlation {
        entities: Vec<String>,
        correlation_coefficient: f64,
        lag: Duration,
    },
    Breakpoint {
        change_magnitude: f64,
        change_type: BreakpointType,
    },
}
```

#### NeuralAnalyzer Interface
```rust
#[async_trait]
pub trait NeuralAnalyzer: Send + Sync {
    /// Generate predictions from features
    async fn predict(&self, features: FeatureVector) -> Result<Prediction, PredictionError>;
    
    /// Batch prediction for multiple entities
    async fn predict_batch(&self, features: Vec<FeatureVector>) -> Result<Vec<Prediction>, PredictionError>;
    
    /// Train model with new data
    async fn train(&mut self, data: TrainingData) -> Result<ModelMetrics, TrainingError>;
    
    /// Get model metadata
    async fn get_model_info(&self) -> Result<ModelInfo, PredictionError>;
    
    /// Load a pre-trained model
    async fn load_model(&mut self, model_path: &str) -> Result<(), PredictionError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    pub entity_id: String,
    pub prediction_timestamp: DateTime<Utc>,
    pub horizon: Duration,
    pub predicted_values: Vec<f64>,
    pub confidence_intervals: Vec<ConfidenceInterval>,
    pub model_name: String,
    pub model_version: String,
}

#[derive(Debug, Clone)]
pub struct TrainingData {
    pub entity_id: String,
    pub features: Vec<FeatureVector>,
    pub target_values: Vec<f64>,
    pub training_metadata: HashMap<String, serde_json::Value>,
}
```

#### ClaudeAnalyzer Interface
```rust
#[async_trait]
pub trait ClaudeAnalyzer: Send + Sync {
    /// Explain detected patterns with context
    async fn explain_pattern(&self, pattern: Pattern, context: AnalysisContext) -> Result<PatternExplanation, AnalysisError>;
    
    /// Suggest potential actions based on patterns
    async fn suggest_actions(&self, patterns: Vec<Pattern>, domain: &str) -> Result<Vec<ActionSuggestion>, AnalysisError>;
    
    /// Analyze pattern relationships
    async fn analyze_relationships(&self, patterns: Vec<Pattern>) -> Result<RelationshipAnalysis, AnalysisError>;
    
    /// Generate natural language summary
    async fn summarize_insights(&self, patterns: Vec<Pattern>, timeframe: TimeWindow) -> Result<InsightSummary, AnalysisError>;
}

#[derive(Debug, Clone)]
pub struct AnalysisContext {
    pub entity_metadata: HashMap<String, serde_json::Value>,
    pub historical_patterns: Vec<Pattern>,
    pub market_conditions: Option<MarketConditions>,
    pub domain_context: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternExplanation {
    pub pattern_id: String,
    pub explanation: String,
    pub contributing_factors: Vec<String>,
    pub potential_causes: Vec<String>,
    pub confidence_level: ExplanationConfidence,
    pub supporting_evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSuggestion {
    pub suggestion_id: String,
    pub action_type: String,
    pub description: String,
    pub priority: Priority,
    pub estimated_impact: ImpactEstimate,
    pub required_parameters: HashMap<String, ParameterSpec>,
}
```

### 4. Execution Domain Layer Interfaces

#### ExecutionDomain Interface
```rust
#[async_trait]
pub trait ExecutionDomain: Send + Sync {
    /// Get domain identifier
    fn domain_name(&self) -> &str;
    
    /// Execute a domain-specific action
    async fn execute_action(&self, action: DomainAction) -> Result<ExecutionResult, ExecutionError>;
    
    /// Validate an action before execution
    async fn validate_action(&self, action: &DomainAction) -> Result<ValidationResult, ValidationError>;
    
    /// Get current domain status
    async fn get_status(&self) -> Result<DomainStatus, StatusError>;
    
    /// Get supported action types
    fn get_action_types(&self) -> Vec<ActionTypeInfo>;
    
    /// Subscribe to execution events
    async fn subscribe_events(&self) -> Result<ExecutionEventStream, ExecutionError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainAction {
    pub action_id: String,
    pub domain: String,
    pub action_type: String,
    pub entity_id: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub constraints: ActionConstraints,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionConstraints {
    pub max_execution_time: Duration,
    pub resource_limits: ResourceLimits,
    pub risk_limits: RiskLimits,
    pub prerequisite_conditions: Vec<PrerequisiteCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub action_id: String,
    pub status: ExecutionStatus,
    pub result_data: HashMap<String, serde_json::Value>,
    pub execution_time: DateTime<Utc>,
    pub duration: Duration,
    pub side_effects: Vec<SideEffect>,
    pub resource_usage: ResourceUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Success,
    Failure { error_code: String, error_message: String },
    PartialSuccess { warnings: Vec<String> },
    Timeout,
    Cancelled,
}
```

## Cross-Layer Communication Protocols

### 1. Stream Subscription Protocol
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionRequest {
    pub subscriber_id: String,
    pub stream_patterns: Vec<String>,
    pub filter_criteria: FilterCriteria,
    pub delivery_guarantees: DeliveryGuarantees,
    pub consumer_group: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterCriteria {
    pub entity_types: Option<Vec<String>>,
    pub metric_patterns: Option<Vec<String>>,
    pub quality_threshold: Option<f64>,
    pub time_window: Option<TimeWindow>,
    pub custom_filters: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeliveryGuarantees {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}
```

### 2. Health Check Protocol
```rust
#[async_trait]
pub trait HealthCheck: Send + Sync {
    /// Get component health status
    async fn health_check(&self) -> Result<HealthStatus, HealthError>;
    
    /// Get detailed health information
    async fn detailed_health(&self) -> Result<DetailedHealth, HealthError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub component_id: String,
    pub status: ComponentStatus,
    pub last_check: DateTime<Utc>,
    pub uptime: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComponentStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { error: String },
    Unknown,
}
```

### 3. Configuration Protocol
```rust
#[async_trait]
pub trait Configurable: Send + Sync {
    /// Get current configuration
    async fn get_config(&self) -> Result<ComponentConfig, ConfigError>;
    
    /// Update configuration
    async fn update_config(&self, config: ComponentConfig) -> Result<(), ConfigError>;
    
    /// Validate configuration
    async fn validate_config(&self, config: &ComponentConfig) -> Result<ValidationResult, ConfigError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentConfig {
    pub component_id: String,
    pub layer: ServiceLayer,
    pub config_version: String,
    pub settings: HashMap<String, serde_json::Value>,
    pub updated_at: DateTime<Utc>,
}
```

## Error Handling Specifications

### 1. Error Type Hierarchy
```rust
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("Ingestion error: {0}")]
    Ingestion(#[from] IngestionError),
    
    #[error("Processing error: {0}")]
    Processing(#[from] ProcessingError),
    
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    
    #[error("Analysis error: {0}")]
    Analysis(#[from] AnalysisError),
    
    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),
    
    #[error("Configuration error: {0}")]
    Configuration(#[from] ConfigError),
    
    #[error("Network error: {0}")]
    Network(#[from] NetworkError),
}

#[derive(Debug, thiserror::Error)]
pub enum IngestionError {
    #[error("Source connection failed: {source}")]
    ConnectionFailed { source: String },
    
    #[error("Data format invalid: {reason}")]
    InvalidFormat { reason: String },
    
    #[error("Rate limit exceeded for source: {source}")]
    RateLimitExceeded { source: String },
    
    #[error("Authentication failed: {source}")]
    AuthenticationFailed { source: String },
}
```

### 2. Retry Specifications
```rust
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_strategy: BackoffStrategy,
    pub retryable_errors: Vec<ErrorType>,
}

#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    Linear { increment: Duration },
    Exponential { multiplier: f64 },
    Fixed { delay: Duration },
}
```

## Versioning and Compatibility

### 1. Interface Versioning
```rust
#[derive(Debug, Clone)]
pub struct InterfaceVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

pub trait VersionedInterface {
    fn interface_version() -> InterfaceVersion;
    fn compatible_versions() -> Vec<InterfaceVersion>;
}
```

### 2. Backward Compatibility Requirements
- Major version changes may break compatibility
- Minor version changes must be backward compatible
- Patch version changes must be fully compatible
- All interfaces must support at least 2 major versions

### 3. Migration Support
```rust
#[async_trait]
pub trait Migratable {
    async fn migrate_from(&mut self, from_version: InterfaceVersion, data: MigrationData) -> Result<(), MigrationError>;
    fn supported_migrations(&self) -> Vec<(InterfaceVersion, InterfaceVersion)>;
}
```

## Testing Interface Contracts

### 1. Contract Testing Framework
```rust
#[cfg(test)]
pub mod contract_tests {
    use super::*;
    
    /// Test that an implementation satisfies the interface contract
    pub async fn assert_data_ingester_contract<T: DataIngester>(ingester: T) {
        // Test all required methods
        let config = create_test_source_config();
        let source_id = ingester.register_source(config).await.unwrap();
        
        let health = ingester.get_source_health().await.unwrap();
        assert!(!health.is_empty());
        
        ingester.unregister_source(source_id).await.unwrap();
    }
    
    /// Test error handling compliance
    pub async fn assert_error_handling_contract<T: DataIngester>(ingester: T) {
        let invalid_config = create_invalid_source_config();
        let result = ingester.register_source(invalid_config).await;
        assert!(result.is_err());
    }
}
```

### 2. Mock Implementations
```rust
#[cfg(test)]
pub struct MockDataIngester {
    sources: HashMap<SourceId, SourceConfig>,
    health_status: Vec<SourceHealth>,
}

#[async_trait]
impl DataIngester for MockDataIngester {
    async fn ingest(&self, source: DataSource) -> Result<StreamHandle, IngestionError> {
        Ok(format!("mock-stream-{}", source.source_id))
    }
    
    async fn register_source(&self, config: SourceConfig) -> Result<SourceId, IngestionError> {
        Ok(format!("mock-source-{}", config.name))
    }
    
    // ... other methods
}
```

This comprehensive interface specification ensures that all components can be developed, tested, and evolved independently while maintaining clear contracts between layers.