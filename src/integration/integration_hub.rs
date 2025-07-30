//! Integration Hub - Central Coordination Point
//!
//! The IntegrationHub serves as the central nervous system for the neural trading platform,
//! coordinating events between all major components through specialized event buses.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};

use super::event_bus::{EventBus, EventBusConfig, EventBusMetrics};
use crate::neural::monitoring::{PerformanceEvent, TrainingNotification};

/// Central integration event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntegrationEvent {
    /// Performance-related events for training decisions
    PerformanceThresholdBreached {
        model_name: String,
        metric_type: String,
        current_value: f64,
        threshold: f64,
        urgency: TrainingUrgency,
        timestamp: DateTime<Utc>,
    },
    
    /// Market timing signals for DAA coordination
    MarketTimingSignal {
        signal_type: MarketSignalType,
        confidence: f64,
        symbol: String,
        current_price: f64,
        recommended_action: Option<String>,
        timestamp: DateTime<Utc>,
    },
    
    /// Training completion notifications
    TrainingCompleted {
        model_name: String,
        training_duration_ms: u64,
        performance_improvement: f64,
        new_accuracy: f64,
        timestamp: DateTime<Utc>,
    },
    
    /// DAA autonomous decisions
    DaaDecision {
        decision_id: String,
        action_type: String,
        confidence: f64,
        risk_score: f64,
        market_context: HashMap<String, f64>,
        timestamp: DateTime<Utc>,
    },
    
    /// System health and monitoring
    SystemHealth {
        component: String,
        status: HealthStatus,
        metrics: HashMap<String, f64>,
        alerts: Vec<String>,
        timestamp: DateTime<Utc>,
    },
}

/// Training urgency levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrainingUrgency {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
}

/// Market signal types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketSignalType {
    BuySignal,
    SellSignal,
    HoldSignal,
    VolatilityAlert,
    TrendReversal,
    MomentumShift,
}

/// Health status for system components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
    Offline,
}

/// Integration hub configuration
#[derive(Debug, Clone)]
pub struct IntegrationConfig {
    /// Event bus configurations for different event types
    pub performance_bus_config: EventBusConfig,
    pub market_bus_config: EventBusConfig,
    pub training_bus_config: EventBusConfig,
    pub daa_bus_config: EventBusConfig,
    pub health_bus_config: EventBusConfig,
    
    /// Integration settings
    pub enable_cross_bus_routing: bool,
    pub enable_event_replay: bool,
    pub max_integration_history: usize,
    pub coordination_timeout_ms: u64,
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        let default_bus_config = EventBusConfig::default();
        
        Self {
            performance_bus_config: default_bus_config.clone(),
            market_bus_config: default_bus_config.clone(),
            training_bus_config: default_bus_config.clone(),
            daa_bus_config: default_bus_config.clone(),
            health_bus_config: default_bus_config,
            enable_cross_bus_routing: true,
            enable_event_replay: false,
            max_integration_history: 1000,
            coordination_timeout_ms: 5000,
        }
    }
}

/// Integration state tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationState {
    pub started_at: DateTime<Utc>,
    pub total_events_processed: u64,
    pub active_coordinators: Vec<String>,
    pub current_integrations: HashMap<String, IntegrationStatus>,
    pub last_health_check: DateTime<Utc>,
    pub performance_summary: IntegrationPerformanceSummary,
}

/// Status of individual integrations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationStatus {
    pub name: String,
    pub status: HealthStatus,
    pub events_processed: u64,
    pub last_activity: DateTime<Utc>,
    pub error_count: u64,
    pub average_latency_ms: f64,
}

/// Performance summary for the integration system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationPerformanceSummary {
    pub events_per_second: f64,
    pub average_coordination_latency_ms: f64,
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub error_rate_percent: f64,
}

impl Default for IntegrationState {
    fn default() -> Self {
        Self {
            started_at: Utc::now(),
            total_events_processed: 0,
            active_coordinators: Vec::new(),
            current_integrations: HashMap::new(),
            last_health_check: Utc::now(),
            performance_summary: IntegrationPerformanceSummary {
                events_per_second: 0.0,
                average_coordination_latency_ms: 0.0,
                memory_usage_mb: 0.0,
                cpu_usage_percent: 0.0,
                error_rate_percent: 0.0,
            },
        }
    }
}

/// Central integration hub coordinating all system events
pub struct IntegrationHub {
    /// Specialized event buses for different concerns
    performance_bus: EventBus<PerformanceEvent>,
    market_bus: EventBus<IntegrationEvent>,
    training_bus: EventBus<TrainingNotification>,
    daa_bus: EventBus<IntegrationEvent>,
    health_bus: EventBus<IntegrationEvent>,
    
    /// Integration state
    state: Arc<RwLock<IntegrationState>>,
    
    /// Configuration
    config: IntegrationConfig,
    
    /// Cross-bus coordination channels
    coordination_tx: mpsc::Sender<CoordinationMessage>,
    coordination_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<CoordinationMessage>>>,
}

/// Messages for cross-bus coordination
#[derive(Debug, Clone)]
pub enum CoordinationMessage {
    RouteEvent {
        from_bus: String,
        to_bus: String,
        event: IntegrationEvent,
    },
    HealthCheck {
        component: String,
        status: HealthStatus,
    },
    MetricsUpdate {
        bus_name: String,
        metrics: EventBusMetrics,
    },
}

impl IntegrationHub {
    /// Create new integration hub with configuration
    pub fn new(config: IntegrationConfig) -> Self {
        let (coordination_tx, coordination_rx) = mpsc::channel(1000);
        
        Self {
            performance_bus: EventBus::new(config.performance_bus_config.clone()),
            market_bus: EventBus::new(config.market_bus_config.clone()),
            training_bus: EventBus::new(config.training_bus_config.clone()),
            daa_bus: EventBus::new(config.daa_bus_config.clone()),
            health_bus: EventBus::new(config.health_bus_config.clone()),
            state: Arc::new(RwLock::new(IntegrationState::default())),
            config,
            coordination_tx,
            coordination_rx: Arc::new(tokio::sync::Mutex::new(coordination_rx)),
        }
    }
    
    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(IntegrationConfig::default())
    }
    
    /// Start the integration hub and all coordination tasks
    pub async fn start(&self) -> Result<()> {
        info!("Starting Integration Hub");
        
        // Update state
        if let Ok(mut state) = self.state.write() {
            state.started_at = Utc::now();
            state.active_coordinators = vec![
                "performance_coordinator".to_string(),
                "market_coordinator".to_string(),
                "training_coordinator".to_string(),
                "health_coordinator".to_string(),
            ];
        }
        
        // Start coordination task if cross-bus routing is enabled
        if self.config.enable_cross_bus_routing {
            self.start_coordination_task().await;
        }
        
        // Start health monitoring
        self.start_health_monitoring().await;
        
        info!("Integration Hub started successfully");
        Ok(())
    }
    
    /// Get performance event bus for subscribing
    pub fn get_performance_bus(&self) -> &EventBus<PerformanceEvent> {
        &self.performance_bus
    }
    
    /// Get market event bus for subscribing
    pub fn get_market_bus(&self) -> &EventBus<IntegrationEvent> {
        &self.market_bus
    }
    
    /// Get training event bus for subscribing
    pub fn get_training_bus(&self) -> &EventBus<TrainingNotification> {
        &self.training_bus
    }
    
    /// Get DAA event bus for subscribing
    pub fn get_daa_bus(&self) -> &EventBus<IntegrationEvent> {
        &self.daa_bus
    }
    
    /// Get health event bus for subscribing  
    pub fn get_health_bus(&self) -> &EventBus<IntegrationEvent> {
        &self.health_bus
    }
    
    /// Publish performance event
    pub async fn publish_performance_event(&self, event: PerformanceEvent) -> Result<usize> {
        let count = self.performance_bus.publish(event).await?;
        self.update_event_count().await;
        Ok(count)
    }
    
    /// Publish market event
    pub async fn publish_market_event(&self, event: IntegrationEvent) -> Result<usize> {
        let count = self.market_bus.publish(event).await?;
        self.update_event_count().await;
        Ok(count)
    }
    
    /// Publish training notification
    pub async fn publish_training_event(&self, event: TrainingNotification) -> Result<usize> {
        let count = self.training_bus.publish(event).await?;
        self.update_event_count().await;
        Ok(count)
    }
    
    /// Publish DAA event
    pub async fn publish_daa_event(&self, event: IntegrationEvent) -> Result<usize> {
        let count = self.daa_bus.publish(event).await?;
        self.update_event_count().await;
        Ok(count)
    }
    
    /// Publish health event
    pub async fn publish_health_event(&self, event: IntegrationEvent) -> Result<usize> {
        let count = self.health_bus.publish(event).await?;
        self.update_event_count().await;
        Ok(count)
    }
    
    /// Get current integration state
    pub fn get_state(&self) -> IntegrationState {
        if let Ok(state) = self.state.read() {
            state.clone()
        } else {
            IntegrationState::default()
        }
    }
    
    /// Get comprehensive metrics from all buses
    pub fn get_comprehensive_metrics(&self) -> HashMap<String, EventBusMetrics> {
        let mut metrics = HashMap::new();
        
        metrics.insert("performance".to_string(), self.performance_bus.get_metrics());
        metrics.insert("market".to_string(), self.market_bus.get_metrics());
        metrics.insert("training".to_string(), self.training_bus.get_metrics());
        metrics.insert("daa".to_string(), self.daa_bus.get_metrics());
        metrics.insert("health".to_string(), self.health_bus.get_metrics());
        
        metrics
    }
    
    /// Register a new integration component
    pub fn register_integration(&self, name: String, component_type: String) -> Result<()> {
        if let Ok(mut state) = self.state.write() {
            let integration_status = IntegrationStatus {
                name: name.clone(),
                status: HealthStatus::Healthy,
                events_processed: 0,
                last_activity: Utc::now(),
                error_count: 0,
                average_latency_ms: 0.0,
            };
            
            state.current_integrations.insert(name, integration_status);
        }
        
        info!("Registered integration component: {} ({})", name, component_type);
        Ok(())
    }
    
    /// Start cross-bus coordination task
    async fn start_coordination_task(&self) {
        let coordination_rx = Arc::clone(&self.coordination_rx);
        let state = Arc::clone(&self.state);
        
        tokio::spawn(async move {
            let mut rx = coordination_rx.lock().await;
            
            while let Some(message) = rx.recv().await {
                match message {
                    CoordinationMessage::RouteEvent { from_bus, to_bus, event } => {
                        // Handle cross-bus event routing
                        info!("Routing event from {} to {}: {:?}", from_bus, to_bus, event);
                        // Implementation would route events between buses
                    }
                    
                    CoordinationMessage::HealthCheck { component, status } => {
                        // Update component health in state
                        if let Ok(mut state_guard) = state.write() {
                            if let Some(integration) = state_guard.current_integrations.get_mut(&component) {
                                integration.status = status;
                                integration.last_activity = Utc::now();
                            }
                        }
                    }
                    
                    CoordinationMessage::MetricsUpdate { bus_name, metrics } => {
                        // Process metrics updates for performance tracking
                        info!("Metrics update for {}: {} events/sec", bus_name, metrics.events_per_second);
                    }
                }
            }
        });
    }
    
    /// Start health monitoring task
    async fn start_health_monitoring(&self) {
        let state = Arc::clone(&self.state);
        let health_bus = self.health_bus.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // Perform health check
                if let Ok(mut state_guard) = state.write() {
                    state_guard.last_health_check = Utc::now();
                    
                    // Check each integration's health
                    for (name, integration) in &mut state_guard.current_integrations {
                        let time_since_activity = Utc::now() - integration.last_activity;
                        
                        // Mark as degraded if no activity for 5 minutes
                        if time_since_activity > chrono::Duration::minutes(5) {
                            integration.status = HealthStatus::Degraded;
                            warn!("Integration {} marked as degraded due to inactivity", name);
                        }
                        
                        // Mark as critical if no activity for 30 minutes
                        if time_since_activity > chrono::Duration::minutes(30) {
                            integration.status = HealthStatus::Critical;
                            error!("Integration {} marked as critical due to extended inactivity", name);
                        }
                    }
                }
                
                // Publish system health event
                let health_event = IntegrationEvent::SystemHealth {
                    component: "integration_hub".to_string(),
                    status: HealthStatus::Healthy,
                    metrics: HashMap::new(),
                    alerts: Vec::new(),
                    timestamp: Utc::now(),
                };
                
                if let Err(e) = health_bus.publish(health_event).await {
                    error!("Failed to publish health event: {}", e);
                }
            }
        });
    }
    
    /// Update total event count in state
    async fn update_event_count(&self) {
        if let Ok(mut state) = self.state.write() {
            state.total_events_processed += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};
    
    #[tokio::test]
    async fn test_integration_hub_creation() {
        let hub = IntegrationHub::with_defaults();
        let state = hub.get_state();
        
        assert!(state.started_at <= Utc::now());
        assert_eq!(state.total_events_processed, 0);
        assert!(state.current_integrations.is_empty());
    }
    
    #[tokio::test]
    async fn test_integration_registration() {
        let hub = IntegrationHub::with_defaults();
        
        hub.register_integration("test_component".to_string(), "test_type".to_string())
            .unwrap();
        
        let state = hub.get_state();
        assert_eq!(state.current_integrations.len(), 1);
        assert!(state.current_integrations.contains_key("test_component"));
        
        let integration = &state.current_integrations["test_component"];
        assert_eq!(integration.name, "test_component");
        assert!(matches!(integration.status, HealthStatus::Healthy));
    }
    
    #[tokio::test]
    async fn test_event_publishing() {
        let hub = IntegrationHub::with_defaults();
        
        // Subscribe to market events
        let mut market_rx = hub.get_market_bus().subscribe("test".to_string(), "test".to_string());
        
        // Publish market event
        let market_event = IntegrationEvent::MarketTimingSignal {
            signal_type: MarketSignalType::BuySignal,
            confidence: 0.95,
            symbol: "BTC/USD".to_string(),
            current_price: 50000.0,
            recommended_action: Some("buy".to_string()),
            timestamp: Utc::now(),
        };
        
        let subscriber_count = hub.publish_market_event(market_event.clone()).await.unwrap();
        assert_eq!(subscriber_count, 1);
        
        // Receive event
        let received = timeout(Duration::from_millis(100), market_rx.recv())
            .await
            .expect("Timeout")
            .expect("Failed to receive");
        
        // Verify event content
        if let IntegrationEvent::MarketTimingSignal { signal_type, confidence, symbol, .. } = received {
            assert!(matches!(signal_type, MarketSignalType::BuySignal));
            assert_eq!(confidence, 0.95);
            assert_eq!(symbol, "BTC/USD");
        } else {
            panic!("Wrong event type received");
        }
        
        // Check state update
        let state = hub.get_state();
        assert_eq!(state.total_events_processed, 1);
    }
    
    #[tokio::test]
    async fn test_comprehensive_metrics() {
        let hub = IntegrationHub::with_defaults();
        
        // Publish some events
        let health_event = IntegrationEvent::SystemHealth {
            component: "test".to_string(),
            status: HealthStatus::Healthy,
            metrics: HashMap::new(),
            alerts: Vec::new(),
            timestamp: Utc::now(),
        };
        
        hub.publish_health_event(health_event).await.unwrap();
        
        let metrics = hub.get_comprehensive_metrics();
        assert_eq!(metrics.len(), 5); // 5 different buses
        
        assert!(metrics.contains_key("performance"));
        assert!(metrics.contains_key("market"));
        assert!(metrics.contains_key("training"));
        assert!(metrics.contains_key("daa"));
        assert!(metrics.contains_key("health"));
        
        // Health bus should show 1 event published
        let health_metrics = &metrics["health"];
        assert_eq!(health_metrics.total_events_published, 1);
    }
}