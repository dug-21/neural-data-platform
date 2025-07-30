# Module Interface Specifications

## Overview

This document provides detailed interface specifications for each module in the refactored architecture. Each interface is designed with clear boundaries, minimal coupling, and maximum cohesion.

## Core Interfaces

### 1. Neural Adapter Interface

```rust
use async_trait::async_trait;
use tokio::sync::broadcast;

/// Primary neural adapter trait for all prediction operations
#[async_trait]
pub trait NeuralAdapter: Send + Sync {
    /// Perform prediction with automatic health checks and performance tracking
    async fn predict(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
    ) -> Result<PredictionResult, AdapterError>;
    
    /// Get current health status of the adapter
    async fn get_health_status(&self) -> HealthStatus;
    
    /// Subscribe to performance events
    fn subscribe_to_performance(&self) -> broadcast::Receiver<PerformanceEvent>;
    
    /// Get adapter metadata (capabilities, version, etc.)
    fn metadata(&self) -> &AdapterMetadata;
}

/// Health status for monitoring
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { error: String },
}

/// Adapter metadata
#[derive(Debug, Clone)]
pub struct AdapterMetadata {
    pub name: String,
    pub version: String,
    pub supported_models: Vec<String>,
    pub capabilities: Vec<Capability>,
}
```

### 2. Predictor Interface

```rust
/// Core predictor trait for neural network operations
#[async_trait]
pub trait Predictor: Send + Sync {
    /// Execute prediction using specified model
    async fn predict(
        &self,
        input: &PredictionInput,
    ) -> Result<PredictionOutput, PredictionError>;
    
    /// Update model weights (for online learning)
    async fn update_model(
        &self,
        model_type: &str,
        update: ModelUpdate,
    ) -> Result<(), ModelError>;
    
    /// Get information about a specific model
    fn get_model_info(&self, model_type: &str) -> Option<ModelInfo>;
    
    /// List all available models
    fn list_models(&self) -> Vec<String>;
}

/// Input for predictions
#[derive(Debug, Clone)]
pub struct PredictionInput {
    pub data: Vec<f32>,
    pub horizon: usize,
    pub model_type: String,
    pub context: Option<PredictionContext>,
}

/// Prediction context for advanced features
#[derive(Debug, Clone)]
pub struct PredictionContext {
    pub confidence_required: f64,
    pub max_latency_ms: Option<u64>,
    pub ensemble_size: Option<usize>,
}
```

### 3. Performance Channel Interface

```rust
/// Performance event emitter trait
pub trait PerformanceEmitter: Send + Sync {
    /// Emit a performance event
    fn emit(&self, event: PerformanceEvent);
    
    /// Emit with timestamp override (for testing)
    fn emit_with_timestamp(&self, event: PerformanceEvent, timestamp: DateTime<Utc>);
}

/// Performance event subscriber trait
pub trait PerformanceSubscriber: Send + Sync {
    /// Subscribe to performance events
    fn subscribe(&self) -> broadcast::Receiver<PerformanceEvent>;
    
    /// Subscribe with filter
    fn subscribe_filtered(
        &self,
        filter: PerformanceEventFilter,
    ) -> broadcast::Receiver<PerformanceEvent>;
}

/// Event filter for selective subscription
#[derive(Debug, Clone)]
pub struct PerformanceEventFilter {
    pub sources: Option<Vec<PerformanceSource>>,
    pub event_types: Option<Vec<PerformanceEventType>>,
    pub min_severity: Option<EventSeverity>,
}
```

### 4. Model Registry Interface

```rust
/// Registry for managing neural network models
#[async_trait]
pub trait ModelRegistry: Send + Sync {
    /// Register a new model
    async fn register_model(
        &self,
        model_type: &str,
        config: ModelConfig,
    ) -> Result<(), RegistryError>;
    
    /// Get model by type
    async fn get_model(
        &self,
        model_type: &str,
    ) -> Result<Arc<dyn NeuralModel>, RegistryError>;
    
    /// List all registered models
    fn list_models(&self) -> Vec<ModelInfo>;
    
    /// Unregister a model
    async fn unregister_model(&self, model_type: &str) -> Result<(), RegistryError>;
}

/// Individual neural model trait
#[async_trait]
pub trait NeuralModel: Send + Sync {
    /// Forward pass through the network
    async fn forward(&self, input: &[f32]) -> Result<Vec<f32>, ModelError>;
    
    /// Get model architecture info
    fn architecture(&self) -> &ModelArchitecture;
    
    /// Check if model is trainable
    fn is_trainable(&self) -> bool;
}
```

### 5. Health Monitor Interface

```rust
/// Health monitoring trait
#[async_trait]
pub trait HealthMonitor: Send + Sync {
    /// Register a component for health monitoring
    async fn register_component(
        &self,
        name: String,
        checker: Arc<dyn HealthChecker>,
    ) -> Result<(), MonitorError>;
    
    /// Get health status of a component
    async fn get_component_health(&self, name: &str) -> Option<ComponentHealth>;
    
    /// Get overall system health
    async fn get_system_health(&self) -> SystemHealth;
    
    /// Start monitoring (non-blocking)
    fn start_monitoring(&self, interval: Duration);
    
    /// Stop monitoring
    fn stop_monitoring(&self);
}

/// Health checker for individual components
#[async_trait]
pub trait HealthChecker: Send + Sync {
    /// Perform health check
    async fn check_health(&self) -> Result<ComponentHealth, CheckError>;
    
    /// Get component name
    fn component_name(&self) -> &str;
}
```

### 6. Training Data Service Interface

```rust
/// Service for managing training data
#[async_trait]
pub trait TrainingDataService: Send + Sync {
    /// Store training data
    async fn store_training_data(
        &self,
        data: TrainingDataBatch,
    ) -> Result<String, StorageError>;
    
    /// Retrieve training data
    async fn get_training_data(
        &self,
        batch_id: &str,
    ) -> Result<TrainingDataBatch, StorageError>;
    
    /// Stream training data for online learning
    async fn stream_training_data(
        &self,
        filter: TrainingDataFilter,
    ) -> Result<TrainingDataStream, StorageError>;
    
    /// Delete old training data
    async fn cleanup_old_data(&self, older_than: DateTime<Utc>) -> Result<usize, StorageError>;
}

/// Training data batch
#[derive(Debug, Clone)]
pub struct TrainingDataBatch {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub model_type: String,
    pub features: Vec<Vec<f32>>,
    pub labels: Vec<f32>,
    pub metadata: Option<HashMap<String, String>>,
}
```

## Module-Specific Interfaces

### FannPredictor Module Interfaces

```rust
// In src/neural/fann_predictor/models/mod.rs

/// Trait for FANN-based models
#[async_trait]
pub trait FannModel: Send + Sync {
    /// Create network with configuration
    fn create_network(&self, config: &ModelConfig) -> Result<Network, ModelError>;
    
    /// Prepare input data for the model
    fn prepare_input(&self, data: &[f32], context: &ModelContext) -> Vec<f32>;
    
    /// Post-process model output
    fn post_process(&self, output: Vec<f32>, context: &ModelContext) -> PredictionOutput;
    
    /// Get model-specific metadata
    fn metadata(&self) -> &ModelMetadata;
}

/// Model context for advanced features
#[derive(Debug, Clone)]
pub struct ModelContext {
    pub timestamp: DateTime<Utc>,
    pub historical_accuracy: Option<f64>,
    pub market_conditions: Option<MarketConditions>,
}
```

### Config Module Interfaces

```rust
// In src/config/mod.rs

/// Configuration provider trait
pub trait ConfigProvider: Send + Sync {
    /// Load configuration from source
    fn load(&self) -> Result<Config, ConfigError>;
    
    /// Validate configuration
    fn validate(&self, config: &Config) -> Result<(), ValidationError>;
    
    /// Watch for configuration changes
    fn watch(&self) -> Option<ConfigWatcher>;
}

/// Configuration sections
pub trait ConfigSection: Serialize + Deserialize {
    /// Validate this configuration section
    fn validate(&self) -> Result<(), ValidationError>;
    
    /// Apply defaults to missing values
    fn with_defaults(self) -> Self;
}
```

### DAA Coordinator Module Interfaces

```rust
// In src/integration/daa_coordinator/mod.rs

/// Agent manager for DAA operations
#[async_trait]
pub trait AgentManager: Send + Sync {
    /// Spawn a new agent
    async fn spawn_agent(&self, config: AgentConfig) -> Result<AgentId, AgentError>;
    
    /// Get agent status
    async fn get_agent_status(&self, id: &AgentId) -> Option<AgentStatus>;
    
    /// Send message to agent
    async fn send_to_agent(
        &self,
        id: &AgentId,
        message: AgentMessage,
    ) -> Result<(), MessageError>;
    
    /// Terminate agent
    async fn terminate_agent(&self, id: &AgentId) -> Result<(), AgentError>;
}

/// Consensus mechanism trait
#[async_trait]
pub trait ConsensusMechanism: Send + Sync {
    /// Propose a value for consensus
    async fn propose(&self, value: ConsensusValue) -> Result<ProposalId, ConsensusError>;
    
    /// Vote on a proposal
    async fn vote(&self, proposal_id: &ProposalId, vote: Vote) -> Result<(), ConsensusError>;
    
    /// Get consensus result
    async fn get_result(&self, proposal_id: &ProposalId) -> Option<ConsensusResult>;
}
```

## Error Types

```rust
/// Base error type for adapters
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("Model not available: {0}")]
    ModelNotAvailable(String),
    
    #[error("Prediction failed: {0}")]
    PredictionFailed(String),
    
    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Model-specific errors
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("Invalid input dimensions: expected {expected}, got {actual}")]
    InvalidDimensions { expected: usize, actual: usize },
    
    #[error("Model initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Training failed: {0}")]
    TrainingFailed(String),
    
    #[error("Model not found: {0}")]
    NotFound(String),
}
```

## Best Practices

### 1. Interface Segregation
- Keep interfaces focused on a single responsibility
- Avoid "fat" interfaces with many methods
- Use composition over inheritance

### 2. Dependency Inversion
- Depend on abstractions (traits), not concrete types
- Use Arc<dyn Trait> for shared ownership
- Inject dependencies through constructors

### 3. Error Handling
- Use specific error types for each module
- Provide context in error messages
- Use Result<T, E> consistently

### 4. Async Design
- Mark trait methods as async where I/O is involved
- Use tokio::sync primitives for coordination
- Avoid blocking operations in async contexts

### 5. Testing
- Create mock implementations for all traits
- Use dependency injection for testability
- Test error paths as thoroughly as success paths