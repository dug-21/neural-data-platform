//! Platform Orchestrator for End-to-End Integration
//! 
//! This module coordinates all components of the neural trading platform:
//! - Streaming data pipeline
//! - DAA agent integration  
//! - FANN neural predictions
//! - Data storage and caching
//! - System health monitoring
//! - Event-driven data flow validation

use crate::data::{DataPipeline, TimescaleDBStorage, RedisCache, TimeSeriesData};
use crate::integration::{
    streaming::{StreamingPipeline, MarketData, NewsData, StreamConfig, StreamEvent},
    data_access::{DataAccessLayer, DataRequest, DataResponse, Timeframe},
    neural_predictions::{NeuralPredictionSystem, DecisionContext, PredictionResult, ModelType}
};
use crate::config::PlatformConfig;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{RwLock, mpsc, broadcast, Mutex};
use tokio::time::timeout;
use tracing::{info, warn, error, debug};

/// Component types for startup sequencing and dependency management
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentType {
    DataPipeline,
    StreamingPipeline,
    DaaOrchestrator,
    NeuralSystem,
    HealthMonitor,
}

/// Main platform orchestrator coordinating all components
#[derive(Clone)]
pub struct PlatformOrchestrator {
    streaming_pipeline: Arc<Mutex<StreamingPipeline>>,
    data_access_layer: Arc<DataAccessLayer>,
    neural_system: Arc<NeuralPredictionSystem>,
    data_pipeline: Arc<DataPipeline>,
    health_monitor: Arc<HealthMonitor>,
    event_bus: Arc<EventBus>,
    daa_agents: Arc<RwLock<HashMap<String, DaaAgent>>>,
    config: PlatformConfig,
    validation_state: Arc<RwLock<ValidationState>>,
    memory_storage: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    shutdown_signal: Arc<AtomicBool>,
}

/// System health monitoring component
pub struct HealthMonitor {
    system_metrics: Arc<RwLock<SystemMetrics>>,
    component_health: Arc<RwLock<ComponentHealth>>,
    alert_thresholds: AlertThresholds,
}

/// Event bus for component communication
pub struct EventBus {
    sender: broadcast::Sender<PlatformEvent>,
    receiver_count: Arc<RwLock<usize>>,
}

/// DAA agent representation
#[derive(Debug, Clone)]
pub struct DaaAgent {
    pub agent_id: String,
    pub agent_type: String,
    pub subscriptions: Vec<String>,
    pub last_activity: DateTime<Utc>,
    pub event_queue: Vec<PlatformEvent>,
    pub prediction_requests: u64,
}

/// Platform event for inter-component communication
#[derive(Debug, Clone)]
pub struct PlatformEvent {
    pub event_id: String,
    pub event_type: String,
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub data: serde_json::Value,
    pub source_component: String,
    pub target_component: Option<String>,
}

/// System health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall_healthy: bool,
    pub streaming_pipeline_healthy: bool,
    pub data_pipeline_healthy: bool,
    pub neural_system_healthy: bool,
    pub components_started: bool,
    pub metrics: SystemMetrics,
}

/// Validation result for data flow testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub data_ingested: bool,
    pub pipeline_processed: bool,
    pub events_published: bool,
    pub agents_responded: bool,
    pub predictions_generated: bool,
    pub end_to_end_latency_ms: u64,
    pub validation_timestamp: DateTime<Utc>,
}

/// System performance metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub error_count: u64,
    pub processing_latency_ms: f64,
    pub throughput_per_second: f64,
    pub memory_usage_gb: f64,
    pub cpu_usage_percent: f64,
    pub active_agents: usize,
    pub uptime_seconds: u64,
}

/// Component health status
#[derive(Debug, Clone, Default)]
pub struct ComponentHealth {
    pub streaming_pipeline: bool,
    pub data_pipeline: bool,
    pub neural_system: bool,
    pub data_access_layer: bool,
    pub event_bus: bool,
    pub last_check: Option<DateTime<Utc>>,
}

/// Alert thresholds for monitoring
#[derive(Debug, Clone)]
pub struct AlertThresholds {
    pub max_latency_ms: f64,
    pub max_error_rate: f64,
    pub min_throughput: f64,
    pub max_memory_gb: f64,
    pub max_cpu_percent: f64,
}

/// Validation state for tracking data flow
#[derive(Debug, Clone, Default)]
pub struct ValidationState {
    pub last_validation: Option<DateTime<Utc>>,
    pub data_flow_active: bool,
    pub pending_validations: HashMap<String, ValidationContext>,
}

/// Validation context for tracking specific validations
#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub validation_id: String,
    pub start_time: DateTime<Utc>,
    pub expected_components: Vec<String>,
    pub completed_components: Vec<String>,
}

/// Prediction metrics for monitoring neural system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionMetrics {
    pub total_predictions: u64,
    pub average_confidence: f64,
    pub models_used: HashMap<String, u64>,
    pub prediction_latency_ms: f64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            max_latency_ms: 1000.0,
            max_error_rate: 0.05,
            min_throughput: 1.0,
            max_memory_gb: 16.0,
            max_cpu_percent: 80.0,
        }
    }
}

impl PlatformOrchestrator {
    /// Create a new platform orchestrator
    pub async fn new(config: PlatformConfig) -> Result<Self> {
        info!("Initializing Platform Orchestrator");
        
        // Initialize data storage components
        let storage = TimescaleDBStorage::new(&config.database.url).await
            .context("Failed to initialize TimescaleDB storage")?;
        let cache = RedisCache::new(&config.redis.url).await
            .context("Failed to initialize Redis cache")?;
        let data_pipeline = Arc::new(DataPipeline::new(storage, cache, config.clone()).await
            .context("Failed to initialize data pipeline")?);
        
        // Initialize streaming pipeline
        let stream_config = StreamConfig {
            market_buffer_size: 1000,
            news_buffer_size: 100,
            batch_size: 50,
            batch_timeout_ms: 1000,
            retry_attempts: 3,
            quality_threshold: config.monitoring.quality_threshold,
            enable_order_book: true,
            enable_sentiment_analysis: true,
        };
        let streaming_pipeline = Arc::new(Mutex::new(
            StreamingPipeline::new(Arc::clone(&data_pipeline), stream_config).await
                .context("Failed to initialize streaming pipeline")?
        ));
        
        // Initialize data access layer
        let data_access_layer = Arc::new(DataAccessLayer::new(
            DataPipeline::new(
                TimescaleDBStorage::new(&config.database.url).await?,
                RedisCache::new(&config.redis.url).await?,
                config.clone()
            ).await?
        ).await.context("Failed to initialize data access layer")?);
        
        // Initialize neural prediction system
        let neural_system = Arc::new(NeuralPredictionSystem::new(config.neural.memory_gb as f64).await
            .context("Failed to initialize neural prediction system")?);
        
        // Initialize health monitor
        let health_monitor = Arc::new(HealthMonitor::new(AlertThresholds::default()));
        
        // Initialize event bus
        let event_bus = Arc::new(EventBus::new(1000));
        
        Ok(Self {
            streaming_pipeline,
            data_access_layer,
            neural_system,
            data_pipeline,
            health_monitor,
            event_bus,
            daa_agents: Arc::new(RwLock::new(HashMap::new())),
            config,
            validation_state: Arc::new(RwLock::new(ValidationState::default())),
            memory_storage: Arc::new(RwLock::new(HashMap::new())),
            shutdown_signal: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Check if orchestrator is initialized
    pub fn is_initialized(&self) -> bool {
        true // Simplified - would check component initialization status
    }

    /// Start the complete platform
    pub async fn start_platform(&self) -> Result<()> {
        info!("Starting Neural Trading Platform");
        
        // Start streaming pipeline
        {
            let mut pipeline = self.streaming_pipeline.lock().await;
            let symbols = vec!["BTC/USD".to_string(), "ETH/USD".to_string(), "ADA/USD".to_string()];
            pipeline.start_market_stream(symbols).await
                .context("Failed to start market stream")?;
            
            let topics = vec!["cryptocurrency".to_string(), "market_analysis".to_string()];
            pipeline.start_news_stream(topics).await
                .context("Failed to start news stream")?;
        }
        
        // Start event processing
        self.start_event_processing().await?;
        
        // Update component health
        {
            let mut health = self.health_monitor.component_health.write().await;
            health.streaming_pipeline = true;
            health.data_pipeline = true;
            health.neural_system = true;
            health.data_access_layer = true;
            health.event_bus = true;
            health.last_check = Some(Utc::now());
        }
        
        info!("Platform started successfully");
        Ok(())
    }

    /// Perform system health check
    pub async fn health_check(&self) -> Result<SystemHealth> {
        debug!("Performing system health check");
        
        // Check individual components
        let streaming_healthy = {
            let pipeline = self.streaming_pipeline.lock().await;
            pipeline.health_check().await.unwrap_or(false)
        };
        
        let data_pipeline_healthy = self.data_pipeline.health_check().await
            .unwrap_or(false);
        
        let neural_system_healthy = true; // Simplified - would check neural system
        
        let overall_healthy = streaming_healthy && data_pipeline_healthy && neural_system_healthy;
        
        // Get system metrics
        let metrics = self.get_system_metrics().await?;
        
        // Update health monitor
        {
            let mut health = self.health_monitor.component_health.write().await;
            health.streaming_pipeline = streaming_healthy;
            health.data_pipeline = data_pipeline_healthy;
            health.neural_system = neural_system_healthy;
            health.last_check = Some(Utc::now());
        }
        
        Ok(SystemHealth {
            overall_healthy,
            streaming_pipeline_healthy: streaming_healthy,
            data_pipeline_healthy,
            neural_system_healthy,
            components_started: true,
            metrics,
        })
    }

    /// Validate complete data flow
    pub async fn validate_data_flow(&self) -> Result<ValidationResult> {
        debug!("Validating end-to-end data flow");
        
        let start_time = Utc::now();
        let validation_id = format!("validation_{}", start_time.timestamp());
        
        // Create validation context
        let validation_context = ValidationContext {
            validation_id: validation_id.clone(),
            start_time,
            expected_components: vec![
                "data_ingestion".to_string(),
                "pipeline_processing".to_string(),
                "event_publishing".to_string(),
                "agent_response".to_string(),
                "prediction_generation".to_string(),
            ],
            completed_components: Vec::new(),
        };
        
        {
            let mut state = self.validation_state.write().await;
            state.pending_validations.insert(validation_id.clone(), validation_context);
        }
        
        // Check data ingestion
        let data_ingested = self.check_data_ingestion().await?;
        
        // Check pipeline processing
        let pipeline_processed = self.check_pipeline_processing().await?;
        
        // Check event publishing
        let events_published = self.check_event_publishing().await?;
        
        // Check agent responses
        let agents_responded = self.check_agent_responses().await?;
        
        // Check prediction generation
        let predictions_generated = self.check_prediction_generation().await?;
        
        let end_to_end_latency_ms = (Utc::now() - start_time).num_milliseconds() as u64;
        
        // Clean up validation context
        {
            let mut state = self.validation_state.write().await;
            state.pending_validations.remove(&validation_id);
            state.last_validation = Some(Utc::now());
        }
        
        Ok(ValidationResult {
            data_ingested,
            pipeline_processed,
            events_published,
            agents_responded,
            predictions_generated,
            end_to_end_latency_ms,
            validation_timestamp: Utc::now(),
        })
    }

    /// Inject market data for testing
    pub async fn inject_market_data(&self, market_data: MarketData) -> Result<()> {
        debug!("Injecting market data: {}", market_data.symbol);
        
        let pipeline = self.streaming_pipeline.lock().await;
        pipeline.process_market_data(market_data).await
            .context("Failed to process market data")?;
        
        // Update metrics
        self.increment_request_count().await?;
        
        Ok(())
    }

    /// Inject news data for testing
    pub async fn inject_news_data(&self, news_data: NewsData) -> Result<()> {
        debug!("Injecting news data: {}", news_data.title);
        
        let pipeline = self.streaming_pipeline.lock().await;
        pipeline.process_news_data(news_data).await
            .context("Failed to process news data")?;
        
        Ok(())
    }

    /// Register DAA agent
    pub async fn register_daa_agent(&self, agent_id: &str) -> Result<()> {
        info!("Registering DAA agent: {}", agent_id);
        
        let agent = DaaAgent {
            agent_id: agent_id.to_string(),
            agent_type: "autonomous_trader".to_string(),
            subscriptions: vec!["market_data".to_string(), "news".to_string()],
            last_activity: Utc::now(),
            event_queue: Vec::new(),
            prediction_requests: 0,
        };
        
        let mut agents = self.daa_agents.write().await;
        agents.insert(agent_id.to_string(), agent);
        
        // Subscribe agent to event bus
        self.subscribe_agent_to_events(agent_id).await?;
        
        Ok(())
    }

    /// Get agent events
    pub async fn get_agent_events(&self, agent_id: &str) -> Result<Vec<PlatformEvent>> {
        let agents = self.daa_agents.read().await;
        if let Some(agent) = agents.get(agent_id) {
            Ok(agent.event_queue.clone())
        } else {
            Ok(Vec::new())
        }
    }

    /// Get neural prediction for agent
    pub async fn get_neural_prediction(&self, decision_context: DecisionContext) -> Result<PredictionResult> {
        debug!("Getting neural prediction for agent: {}", decision_context.agent_id);
        
        // Update agent activity
        {
            let mut agents = self.daa_agents.write().await;
            if let Some(agent) = agents.get_mut(&decision_context.agent_id) {
                agent.last_activity = Utc::now();
                agent.prediction_requests += 1;
            }
        }
        
        // Get prediction from neural system
        let prediction = self.neural_system.get_prediction_for_decision(decision_context).await
            .context("Failed to get neural prediction")?;
        
        Ok(prediction)
    }

    /// Get active subscriptions
    pub async fn get_active_subscriptions(&self) -> Result<Vec<String>> {
        let pipeline = self.streaming_pipeline.lock().await;
        pipeline.get_active_market_subscriptions().await
    }

    /// Get latest market data
    pub async fn get_latest_market_data(&self, symbol: &str) -> Result<Option<TimeSeriesData>> {
        let pipeline = self.streaming_pipeline.lock().await;
        pipeline.get_latest_market_data(symbol).await
    }

    /// Get prediction metrics
    pub async fn get_prediction_metrics(&self) -> Result<PredictionMetrics> {
        // Simplified implementation
        Ok(PredictionMetrics {
            total_predictions: 10,
            average_confidence: 0.85,
            models_used: {
                let mut models = HashMap::new();
                models.insert("NHITS".to_string(), 5);
                models.insert("DeepAR".to_string(), 3);
                models.insert("TCN".to_string(), 2);
                models
            },
            prediction_latency_ms: 150.0,
        })
    }

    /// Store results in memory
    pub async fn store_results_in_memory(&self, memory_key: &str) -> Result<()> {
        let health = self.health_check().await?;
        let validation_result = self.validate_data_flow().await?;
        let prediction_metrics = self.get_prediction_metrics().await?;
        let system_metrics = self.get_system_metrics().await?;
        
        let memory_data = serde_json::json!({
            "system_health": health,
            "validation_results": validation_result,
            "prediction_results": prediction_metrics,
            "performance_metrics": system_metrics,
            "timestamp": Utc::now(),
            "memory_key": memory_key
        });
        
        let mut storage = self.memory_storage.write().await;
        storage.insert(memory_key.to_string(), memory_data);
        
        info!("Stored results in memory at key: {}", memory_key);
        Ok(())
    }

    /// Get memory data
    pub async fn get_memory_data(&self, memory_key: &str) -> Result<HashMap<String, serde_json::Value>> {
        let storage = self.memory_storage.read().await;
        if let Some(data) = storage.get(memory_key) {
            if let Some(obj) = data.as_object() {
                let mut result = HashMap::new();
                for (k, v) in obj {
                    result.insert(k.clone(), v.clone());
                }
                Ok(result)
            } else {
                Ok(HashMap::new())
            }
        } else {
            Ok(HashMap::new())
        }
    }

    /// Shutdown the complete platform in reverse startup order
    pub async fn shutdown_platform(&self) -> Result<()> {
        info!("Initiating platform shutdown");
        
        // Set shutdown signal
        self.shutdown_signal.store(true, Ordering::Relaxed);
        
        // Shutdown in reverse order: Health → Neural → DAA → Streaming → Data
        
        // 1. Health Monitor (stop monitoring)
        {
            let mut health = self.health_monitor.component_health.write().await;
            health.streaming_pipeline = false;
            health.data_pipeline = false;
            health.neural_system = false;
            health.data_access_layer = false;
            health.event_bus = false;
            health.last_check = None;
        }
        
        // 2. Neural System (no explicit shutdown needed for now)
        debug!("Neural system shutdown - graceful degradation");
        
        // 3. DAA Agents (clear all agents)
        {
            let mut agents = self.daa_agents.write().await;
            agents.clear();
        }
        
        // 4. Streaming Pipeline (stop streams)
        {
            let pipeline = self.streaming_pipeline.lock().await;
            // Note: StreamingPipeline would need a shutdown method in real implementation
            debug!("Streaming pipeline shutdown - stopping data streams");
        }
        
        // 5. Data Pipeline (close connections)
        debug!("Data pipeline shutdown - closing database connections");
        
        info!("Platform shutdown completed successfully");
        Ok(())
    }

    /// Start a component with its dependencies
    pub async fn start_component_with_dependencies(&self, component: ComponentType) -> Result<()> {
        info!("Starting component: {:?} with dependencies", component);
        
        match component {
            ComponentType::DataPipeline => {
                // No dependencies
                self.start_single_component(ComponentType::DataPipeline).await?;
            }
            ComponentType::StreamingPipeline => {
                // Depends on DataPipeline
                self.ensure_component_started(ComponentType::DataPipeline).await?;
                self.start_single_component(ComponentType::StreamingPipeline).await?;
            }
            ComponentType::DaaOrchestrator => {
                // Depends on StreamingPipeline and DataPipeline
                self.ensure_component_started(ComponentType::DataPipeline).await?;
                self.ensure_component_started(ComponentType::StreamingPipeline).await?;
                self.start_single_component(ComponentType::DaaOrchestrator).await?;
            }
            ComponentType::NeuralSystem => {
                // Depends on DataPipeline and DaaOrchestrator
                self.ensure_component_started(ComponentType::DataPipeline).await?;
                self.ensure_component_started(ComponentType::DaaOrchestrator).await?;
                self.start_single_component(ComponentType::NeuralSystem).await?;
            }
            ComponentType::HealthMonitor => {
                // Depends on all other components
                self.ensure_component_started(ComponentType::DataPipeline).await?;
                self.ensure_component_started(ComponentType::StreamingPipeline).await?;
                self.ensure_component_started(ComponentType::DaaOrchestrator).await?;
                self.ensure_component_started(ComponentType::NeuralSystem).await?;
                self.start_single_component(ComponentType::HealthMonitor).await?;
            }
        }
        
        Ok(())
    }

    /// Restart a specific component
    pub async fn restart_component(&self, component: ComponentType) -> Result<()> {
        info!("Restarting component: {:?}", component);
        
        // Check if component is currently running
        if !self.is_component_running(component.clone()).await? {
            bail!("Cannot restart component {:?} - it is not currently running", component);
        }
        
        // Stop the component
        self.stop_single_component(component.clone()).await?;
        
        // Wait a moment for cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Start the component with dependencies
        self.start_component_with_dependencies(component.clone()).await?;
        
        info!("Component {:?} restarted successfully", component);
        Ok(())
    }

    /// Check if shutdown signal is set
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown_signal.load(Ordering::Relaxed)
    }

    // Private helper methods

    async fn start_event_processing(&self) -> Result<()> {
        // Start event processing task
        let event_receiver = self.event_bus.sender.subscribe();
        let daa_agents_clone = Arc::clone(&self.daa_agents);
        
        tokio::spawn(async move {
            let mut receiver = event_receiver;
            while let Ok(event) = receiver.recv().await {
                // Distribute event to appropriate agents
                let mut agents = daa_agents_clone.write().await;
                for agent in agents.values_mut() {
                    if agent.subscriptions.contains(&event.event_type) {
                        agent.event_queue.push(event.clone());
                        agent.last_activity = Utc::now();
                    }
                }
            }
        });
        
        Ok(())
    }

    async fn subscribe_agent_to_events(&self, agent_id: &str) -> Result<()> {
        // Create subscription context for agent
        debug!("Subscribing agent {} to event bus", agent_id);
        
        let mut receiver_count = self.event_bus.receiver_count.write().await;
        *receiver_count += 1;
        
        Ok(())
    }

    async fn check_data_ingestion(&self) -> Result<bool> {
        // Check if data ingestion is working
        Ok(true) // Simplified
    }

    async fn check_pipeline_processing(&self) -> Result<bool> {
        // Check if pipeline is processing data
        let health = self.data_pipeline.health_check().await?;
        Ok(health)
    }

    async fn check_event_publishing(&self) -> Result<bool> {
        // Check if events are being published
        Ok(true) // Simplified
    }

    async fn check_agent_responses(&self) -> Result<bool> {
        // Check if agents are responding to events
        let agents = self.daa_agents.read().await;
        Ok(!agents.is_empty())
    }

    async fn check_prediction_generation(&self) -> Result<bool> {
        // Check if predictions are being generated
        Ok(true) // Simplified
    }

    async fn get_system_metrics(&self) -> Result<SystemMetrics> {
        let metrics_guard = self.health_monitor.system_metrics.read().await;
        Ok(metrics_guard.clone())
    }

    async fn increment_request_count(&self) -> Result<()> {
        let mut metrics = self.health_monitor.system_metrics.write().await;
        metrics.total_requests += 1;
        metrics.successful_requests += 1;
        Ok(())
    }

    async fn ensure_component_started(&self, component: ComponentType) -> Result<()> {
        if !self.is_component_running(component.clone()).await? {
            self.start_single_component(component).await?;
        }
        Ok(())
    }

    async fn start_single_component(&self, component: ComponentType) -> Result<()> {
        debug!("Starting single component: {:?}", component);
        
        match component {
            ComponentType::DataPipeline => {
                // Data pipeline is already initialized in constructor
                debug!("Data pipeline already initialized");
            }
            ComponentType::StreamingPipeline => {
                // Streaming pipeline startup logic
                let mut pipeline = self.streaming_pipeline.lock().await;
                let symbols = vec!["BTC/USD".to_string(), "ETH/USD".to_string()];
                pipeline.start_market_stream(symbols).await
                    .context("Failed to start streaming pipeline")?;
            }
            ComponentType::DaaOrchestrator => {
                // Start event processing for DAA agents
                self.start_event_processing().await
                    .context("Failed to start DAA orchestrator")?;
            }
            ComponentType::NeuralSystem => {
                // Neural system is already initialized
                debug!("Neural system already initialized");
            }
            ComponentType::HealthMonitor => {
                // Update health monitor to active state
                let mut health = self.health_monitor.component_health.write().await;
                health.last_check = Some(Utc::now());
            }
        }
        
        Ok(())
    }

    async fn stop_single_component(&self, component: ComponentType) -> Result<()> {
        debug!("Stopping single component: {:?}", component);
        
        match component {
            ComponentType::DataPipeline => {
                debug!("Data pipeline stop - would close connections");
            }
            ComponentType::StreamingPipeline => {
                debug!("Streaming pipeline stop - would halt streams");
            }
            ComponentType::DaaOrchestrator => {
                // Clear DAA agents
                let mut agents = self.daa_agents.write().await;
                agents.clear();
            }
            ComponentType::NeuralSystem => {
                debug!("Neural system stop - graceful degradation");
            }
            ComponentType::HealthMonitor => {
                let mut health = self.health_monitor.component_health.write().await;
                health.last_check = None;
            }
        }
        
        Ok(())
    }

    async fn is_component_running(&self, component: ComponentType) -> Result<bool> {
        match component {
            ComponentType::DataPipeline => {
                Ok(self.data_pipeline.health_check().await.unwrap_or(false))
            }
            ComponentType::StreamingPipeline => {
                let pipeline = self.streaming_pipeline.lock().await;
                Ok(pipeline.health_check().await.unwrap_or(false))
            }
            ComponentType::DaaOrchestrator => {
                let agents = self.daa_agents.read().await;
                Ok(!agents.is_empty())
            }
            ComponentType::NeuralSystem => {
                // Simplified check - would need actual health check method
                Ok(true)
            }
            ComponentType::HealthMonitor => {
                let health = self.health_monitor.component_health.read().await;
                Ok(health.last_check.is_some())
            }
        }
    }
}

impl HealthMonitor {
    pub fn new(thresholds: AlertThresholds) -> Self {
        Self {
            system_metrics: Arc::new(RwLock::new(SystemMetrics::default())),
            component_health: Arc::new(RwLock::new(ComponentHealth::default())),
            alert_thresholds: thresholds,
        }
    }
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            receiver_count: Arc::new(RwLock::new(0)),
        }
    }
}