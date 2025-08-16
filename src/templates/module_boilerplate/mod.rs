//! Module Boilerplate Template
//! 
//! This template provides a foundation for creating new modules that follow
//! the strict module isolation principles defined in the architecture.
//! 
//! Key Features:
//! - Enforces module isolation boundaries
//! - Implements required traits for lifecycle management
//! - Provides observability hooks
//! - Handles message passing via Redis Streams
//! - Includes configuration management
//! - Built-in health monitoring

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::{Result, anyhow};

/// Base event schema for all inter-module communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event<T> {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub domain: String,
    pub source: String,
    pub correlation_id: Uuid,
    pub payload: T,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl<T> Event<T> {
    pub fn new(domain: String, source: String, payload: T) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            domain,
            source,
            correlation_id: Uuid::new_v4(),
            payload,
            metadata: HashMap::new(),
        }
    }

    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = correlation_id;
        self
    }

    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Health status for module monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

/// Configuration trait for module-specific configuration
pub trait ModuleConfig: Send + Sync + Clone + std::fmt::Debug {
    /// Validate configuration before module initialization
    fn validate(&self) -> Result<()>;
    
    /// Get module name for namespacing
    fn module_name(&self) -> &str;
    
    /// Get domain this module serves
    fn domain(&self) -> &str;
    
    /// Get Redis stream patterns this module subscribes to
    fn input_streams(&self) -> Vec<String>;
    
    /// Get Redis stream patterns this module publishes to
    fn output_streams(&self) -> Vec<String>;
}

/// Metrics exporter trait for observability
#[async_trait]
pub trait MetricsExporter: Send + Sync {
    async fn export_metrics(&self) -> Result<HashMap<String, f64>>;
    async fn increment_counter(&self, name: &str, value: f64, tags: HashMap<String, String>);
    async fn record_histogram(&self, name: &str, value: f64, tags: HashMap<String, String>);
}

/// Trace exporter trait for distributed tracing
#[async_trait]
pub trait TraceExporter: Send + Sync {
    async fn start_span(&self, name: &str, parent_id: Option<String>) -> String;
    async fn end_span(&self, span_id: &str);
    async fn add_span_attribute(&self, span_id: &str, key: &str, value: &str);
}

/// Core module trait that all modules must implement
#[async_trait]
pub trait Module: Send + Sync {
    type Config: ModuleConfig;
    type PayloadType: Send + Sync + Serialize + for<'de> Deserialize<'de>;

    /// Initialize the module with configuration
    async fn initialize(&self, config: Self::Config) -> Result<()>;
    
    /// Perform health check
    async fn health_check(&self) -> HealthStatus;
    
    /// Graceful shutdown
    async fn shutdown(&self) -> Result<()>;
    
    /// Get metrics exporter
    fn metrics(&self) -> Box<dyn MetricsExporter>;
    
    /// Get trace exporter
    fn traces(&self) -> Box<dyn TraceExporter>;
    
    /// Handle incoming message
    async fn handle_message(&self, msg: Event<Self::PayloadType>) -> Result<()>;
    
    /// Get module name for identification
    fn name(&self) -> &str;
}

/// Base module implementation providing common functionality
pub struct BaseModule<C, P> {
    name: String,
    config: Option<C>,
    state: Arc<RwLock<ModuleState>>,
    metrics_exporter: Box<dyn MetricsExporter>,
    trace_exporter: Box<dyn TraceExporter>,
    _phantom: std::marker::PhantomData<P>,
}

#[derive(Debug, Clone)]
enum ModuleState {
    Uninitialized,
    Initializing,
    Running,
    Degraded { reason: String },
    Stopping,
    Stopped,
}

impl<C, P> BaseModule<C, P>
where
    C: ModuleConfig,
    P: Send + Sync + Serialize + for<'de> Deserialize<'de>,
{
    pub fn new(
        name: String,
        metrics_exporter: Box<dyn MetricsExporter>,
        trace_exporter: Box<dyn TraceExporter>,
    ) -> Self {
        Self {
            name,
            config: None,
            state: Arc::new(RwLock::new(ModuleState::Uninitialized)),
            metrics_exporter,
            trace_exporter,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Check if module is in a healthy state
    pub async fn is_healthy(&self) -> bool {
        matches!(
            *self.state.read().await,
            ModuleState::Running | ModuleState::Initializing
        )
    }

    /// Update module state
    async fn set_state(&self, new_state: ModuleState) {
        let mut state = self.state.write().await;
        *state = new_state;
    }

    /// Get current state
    pub async fn get_state(&self) -> ModuleState {
        self.state.read().await.clone()
    }
}

#[async_trait]
impl<C, P> Module for BaseModule<C, P>
where
    C: ModuleConfig + 'static,
    P: Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
{
    type Config = C;
    type PayloadType = P;

    async fn initialize(&self, config: Self::Config) -> Result<()> {
        self.set_state(ModuleState::Initializing).await;
        
        // Validate configuration
        config.validate()?;
        
        // Record initialization metric
        let mut tags = HashMap::new();
        tags.insert("module".to_string(), self.name.clone());
        tags.insert("domain".to_string(), config.domain().to_string());
        
        self.metrics_exporter
            .increment_counter("module_initializations_total", 1.0, tags)
            .await;

        // Start tracing span for initialization
        let span_id = self.traces()
            .start_span(&format!("{}_initialization", self.name), None)
            .await;

        // TODO: Add module-specific initialization logic here
        // Example: Connect to Redis, validate stream access, etc.

        self.traces().end_span(&span_id).await;
        self.set_state(ModuleState::Running).await;

        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        let state = self.get_state().await;
        match state {
            ModuleState::Running => HealthStatus::Healthy,
            ModuleState::Degraded { reason } => HealthStatus::Degraded { reason },
            ModuleState::Uninitialized | ModuleState::Stopping | ModuleState::Stopped => {
                HealthStatus::Unhealthy {
                    reason: format!("Module state: {:?}", state),
                }
            }
            ModuleState::Initializing => HealthStatus::Degraded {
                reason: "Module is still initializing".to_string(),
            },
        }
    }

    async fn shutdown(&self) -> Result<()> {
        self.set_state(ModuleState::Stopping).await;

        // Record shutdown metric
        let mut tags = HashMap::new();
        tags.insert("module".to_string(), self.name.clone());
        
        self.metrics_exporter
            .increment_counter("module_shutdowns_total", 1.0, tags)
            .await;

        // TODO: Add module-specific cleanup logic here
        // Example: Close Redis connections, flush buffers, etc.

        self.set_state(ModuleState::Stopped).await;
        Ok(())
    }

    fn metrics(&self) -> Box<dyn MetricsExporter> {
        // Note: In a real implementation, you might want to return a reference or clone
        // This is a simplified version for the template
        Box::new(NoOpMetricsExporter)
    }

    fn traces(&self) -> Box<dyn TraceExporter> {
        // Note: In a real implementation, you might want to return a reference or clone
        // This is a simplified version for the template
        Box::new(NoOpTraceExporter)
    }

    async fn handle_message(&self, msg: Event<Self::PayloadType>) -> Result<()> {
        // Ensure module is in a healthy state
        if !self.is_healthy().await {
            return Err(anyhow!("Module {} is not in a healthy state", self.name));
        }

        // Start tracing span for message handling
        let span_id = self.traces()
            .start_span(&format!("{}_handle_message", self.name), None)
            .await;

        self.traces()
            .add_span_attribute(&span_id, "correlation_id", &msg.correlation_id.to_string())
            .await;

        // Record message handling metric
        let mut tags = HashMap::new();
        tags.insert("module".to_string(), self.name.clone());
        tags.insert("domain".to_string(), msg.domain.clone());
        tags.insert("source".to_string(), msg.source.clone());

        self.metrics_exporter
            .increment_counter("messages_processed_total", 1.0, tags.clone())
            .await;

        let start_time = std::time::Instant::now();

        // TODO: Add module-specific message processing logic here
        let result = self.process_message(msg).await;

        // Record processing latency
        let latency_ms = start_time.elapsed().as_millis() as f64;
        self.metrics_exporter
            .record_histogram("message_processing_latency_ms", latency_ms, tags)
            .await;

        self.traces().end_span(&span_id).await;

        result
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl<C, P> BaseModule<C, P>
where
    C: ModuleConfig,
    P: Send + Sync + Serialize + for<'de> Deserialize<'de>,
{
    /// Module-specific message processing (to be implemented by concrete modules)
    async fn process_message(&self, _msg: Event<P>) -> Result<()> {
        // Default implementation - override in concrete modules
        Ok(())
    }
}

/// No-op implementations for testing and template purposes
pub struct NoOpMetricsExporter;

#[async_trait]
impl MetricsExporter for NoOpMetricsExporter {
    async fn export_metrics(&self) -> Result<HashMap<String, f64>> {
        Ok(HashMap::new())
    }

    async fn increment_counter(&self, _name: &str, _value: f64, _tags: HashMap<String, String>) {
        // No-op
    }

    async fn record_histogram(&self, _name: &str, _value: f64, _tags: HashMap<String, String>) {
        // No-op
    }
}

pub struct NoOpTraceExporter;

#[async_trait]
impl TraceExporter for NoOpTraceExporter {
    async fn start_span(&self, _name: &str, _parent_id: Option<String>) -> String {
        Uuid::new_v4().to_string()
    }

    async fn end_span(&self, _span_id: &str) {
        // No-op
    }

    async fn add_span_attribute(&self, _span_id: &str, _key: &str, _value: &str) {
        // No-op
    }
}

/// Example configuration implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleConfig {
    pub module_name: String,
    pub domain: String,
    pub redis_url: String,
    pub input_streams: Vec<String>,
    pub output_streams: Vec<String>,
    pub worker_threads: usize,
    pub max_message_size: usize,
}

impl ModuleConfig for ExampleConfig {
    fn validate(&self) -> Result<()> {
        if self.module_name.is_empty() {
            return Err(anyhow!("Module name cannot be empty"));
        }
        if self.domain.is_empty() {
            return Err(anyhow!("Domain cannot be empty"));
        }
        if self.worker_threads == 0 {
            return Err(anyhow!("Worker threads must be greater than 0"));
        }
        Ok(())
    }

    fn module_name(&self) -> &str {
        &self.module_name
    }

    fn domain(&self) -> &str {
        &self.domain
    }

    fn input_streams(&self) -> Vec<String> {
        self.input_streams.clone()
    }

    fn output_streams(&self) -> Vec<String> {
        self.output_streams.clone()
    }
}

/// Example payload type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExamplePayload {
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

/// Example concrete module implementation
pub struct ExampleModule {
    base: BaseModule<ExampleConfig, ExamplePayload>,
}

impl ExampleModule {
    pub fn new() -> Self {
        Self {
            base: BaseModule::new(
                "example-module".to_string(),
                Box::new(NoOpMetricsExporter),
                Box::new(NoOpTraceExporter),
            ),
        }
    }
}

#[async_trait]
impl Module for ExampleModule {
    type Config = ExampleConfig;
    type PayloadType = ExamplePayload;

    async fn initialize(&self, config: Self::Config) -> Result<()> {
        // Delegate to base implementation
        self.base.initialize(config).await?;
        
        // Add module-specific initialization logic here
        // Example: Set up Redis connections, validate stream access
        
        Ok(())
    }

    async fn health_check(&self) -> HealthStatus {
        // Delegate to base implementation and add module-specific checks
        let base_health = self.base.health_check().await;
        
        // Add module-specific health checks here
        // Example: Check Redis connectivity, validate stream access
        
        base_health
    }

    async fn shutdown(&self) -> Result<()> {
        // Add module-specific cleanup logic here
        // Example: Close connections, flush buffers
        
        // Delegate to base implementation
        self.base.shutdown().await
    }

    fn metrics(&self) -> Box<dyn MetricsExporter> {
        self.base.metrics()
    }

    fn traces(&self) -> Box<dyn TraceExporter> {
        self.base.traces()
    }

    async fn handle_message(&self, msg: Event<Self::PayloadType>) -> Result<()> {
        // Add message validation and preprocessing
        
        // Delegate to base implementation
        self.base.handle_message(msg).await
    }

    fn name(&self) -> &str {
        self.base.name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_module_lifecycle() {
        let module = ExampleModule::new();
        
        // Test initial state
        assert!(matches!(
            module.base.get_state().await,
            ModuleState::Uninitialized
        ));

        // Test configuration validation
        let config = ExampleConfig {
            module_name: "test-module".to_string(),
            domain: "test".to_string(),
            redis_url: "redis://localhost:6379".to_string(),
            input_streams: vec!["data.test.*.raw".to_string()],
            output_streams: vec!["data.test.*.processed".to_string()],
            worker_threads: 4,
            max_message_size: 1024 * 1024,
        };

        assert!(config.validate().is_ok());

        // Test initialization
        assert!(module.initialize(config).await.is_ok());
        assert!(module.base.is_healthy().await);

        // Test health check
        assert!(matches!(module.health_check().await, HealthStatus::Healthy));

        // Test message handling
        let event = Event::new(
            "test".to_string(),
            "test-source".to_string(),
            ExamplePayload {
                data: serde_json::json!({"test": "data"}),
                timestamp: Utc::now(),
                metadata: HashMap::new(),
            },
        );

        assert!(module.handle_message(event).await.is_ok());

        // Test shutdown
        assert!(module.shutdown().await.is_ok());
        assert!(matches!(
            module.base.get_state().await,
            ModuleState::Stopped
        ));
    }

    #[test]
    fn test_config_validation() {
        let mut config = ExampleConfig {
            module_name: "".to_string(), // Invalid: empty name
            domain: "test".to_string(),
            redis_url: "redis://localhost:6379".to_string(),
            input_streams: vec![],
            output_streams: vec![],
            worker_threads: 4,
            max_message_size: 1024,
        };

        assert!(config.validate().is_err());

        config.module_name = "test-module".to_string();
        config.domain = "".to_string(); // Invalid: empty domain
        assert!(config.validate().is_err());

        config.domain = "test".to_string();
        config.worker_threads = 0; // Invalid: zero threads
        assert!(config.validate().is_err());

        config.worker_threads = 4;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_event_creation() {
        let payload = ExamplePayload {
            data: serde_json::json!({"test": "data"}),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        };

        let event = Event::new("test".to_string(), "test-source".to_string(), payload);

        assert_eq!(event.domain, "test");
        assert_eq!(event.source, "test-source");
        assert!(!event.id.is_nil());
        assert!(!event.correlation_id.is_nil());
    }
}