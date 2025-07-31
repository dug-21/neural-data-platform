//! Mock tests for event flow verification
//! 
//! Tests the interaction between PerformanceChannel and MarketHours modules
//! focusing on event subscription, async handling, error scenarios, and decision triggers.

use neural_trader::neural::monitoring::performance_channel::{
    PerformanceChannel, PerformanceEvent, PerformanceEventBuilder, PerformanceEventType,
    PerformanceSource, EventPriority, AlertType, AlertSeverity, PerformanceMetrics,
    ChannelConfig, ComponentType,
};
use neural_trader::utils::market_hours::{
    MarketHours, Exchange, TrainingWindow, MarketSession, MarketIntensity,
    EmergencyPriority, CircuitBreakerLevel,
};
use chrono::{DateTime, Duration, TimeZone, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};
use tokio::time::{timeout, sleep};
use std::time::Duration as StdDuration;
use anyhow::Result;

/// Mock performance monitor that subscribes to events
struct MockPerformanceMonitor {
    received_events: Arc<Mutex<Vec<PerformanceEvent>>>,
    subscriber: broadcast::Receiver<PerformanceEvent>,
}

impl MockPerformanceMonitor {
    fn new(channel: &PerformanceChannel) -> Self {
        Self {
            received_events: Arc::new(Mutex::new(Vec::new())),
            subscriber: channel.subscribe(),
        }
    }

    async fn start_monitoring(&mut self) {
        let events = self.received_events.clone();
        
        while let Ok(event) = self.subscriber.recv().await {
            if let Ok(mut events_vec) = events.lock() {
                events_vec.push(event);
            }
        }
    }

    fn get_event_count(&self) -> usize {
        self.received_events.lock().unwrap().len()
    }

    fn get_events(&self) -> Vec<PerformanceEvent> {
        self.received_events.lock().unwrap().clone()
    }
}

/// Mock training coordinator that makes decisions based on events
struct MockTrainingCoordinator {
    channel: PerformanceChannel,
    market_hours: Arc<MarketHours>,
    training_decisions: Arc<Mutex<Vec<TrainingDecision>>>,
}

#[derive(Debug, Clone)]
struct TrainingDecision {
    timestamp: DateTime<Utc>,
    trigger_event: PerformanceEvent,
    market_window: TrainingWindow,
    decision: String,
    resource_allocation: f64,
}

impl MockTrainingCoordinator {
    fn new(channel: PerformanceChannel, market_hours: Arc<MarketHours>) -> Self {
        Self {
            channel,
            market_hours,
            training_decisions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn process_event(&self, event: PerformanceEvent) -> Result<()> {
        // Check if event requires training decision
        let should_train = match &event.event_type {
            PerformanceEventType::ModelError { recoverable, .. } => !recoverable,
            PerformanceEventType::PerformanceDegradation { degradation_percent, .. } => {
                *degradation_percent > 10.0
            }
            PerformanceEventType::Alert { alert_type, .. } => {
                matches!(alert_type, AlertType::TrainingRequired | AlertType::LowAccuracy)
            }
            _ => false,
        };

        if should_train {
            // Check market conditions
            let market_window = self.market_hours.get_training_window(Utc::now()).await;
            let resource_limit = self.market_hours.get_resource_limit(Utc::now()).await;

            let decision = TrainingDecision {
                timestamp: Utc::now(),
                trigger_event: event.clone(),
                market_window: market_window.clone(),
                decision: self.make_decision(&market_window, resource_limit),
                resource_allocation: resource_limit,
            };

            // Store decision
            if let Ok(mut decisions) = self.training_decisions.lock() {
                decisions.push(decision.clone());
            }

            // Emit decision event
            let decision_event = PerformanceEventBuilder::new()
                .source(PerformanceSource::TrainingSystem {
                    trainer_id: "mock_trainer".to_string(),
                    model_type: "mock".to_string(),
                })
                .event_type(PerformanceEventType::TrainingStarted {
                    model: "mock_model".to_string(),
                    training_type: "adaptive".to_string(),
                    estimated_duration_mins: 30,
                })
                .priority(EventPriority::High)
                .tag("decision".to_string(), decision.decision)
                .tag("market_window".to_string(), format!("{:?}", market_window))
                .build()?;

            self.channel.emit(decision_event).await?;
        }

        Ok(())
    }

    fn make_decision(&self, window: &TrainingWindow, resource_limit: f64) -> String {
        match window {
            TrainingWindow::Optimal => "immediate_full_training".to_string(),
            TrainingWindow::Good => "scheduled_training".to_string(),
            TrainingWindow::Acceptable => {
                if resource_limit > 0.5 {
                    "limited_training".to_string()
                } else {
                    "defer_training".to_string()
                }
            }
            TrainingWindow::Poor | TrainingWindow::Restricted => "defer_to_next_window".to_string(),
        }
    }

    fn get_decisions(&self) -> Vec<TrainingDecision> {
        self.training_decisions.lock().unwrap().clone()
    }
}

#[tokio::test]
async fn test_event_subscription_basic() {
    let config = ChannelConfig::default();
    let (channel, _receiver) = PerformanceChannel::new(config);
    
    // Create multiple monitors
    let mut monitor1 = MockPerformanceMonitor::new(&channel);
    let mut monitor2 = MockPerformanceMonitor::new(&channel);
    
    // Start monitoring in background
    let monitor1_events = monitor1.received_events.clone();
    let monitor2_events = monitor2.received_events.clone();
    
    tokio::spawn(async move {
        monitor1.start_monitoring().await;
    });
    
    tokio::spawn(async move {
        monitor2.start_monitoring().await;
    });
    
    // Give monitors time to start
    sleep(StdDuration::from_millis(100)).await;
    
    // Emit test event
    let event = PerformanceEventBuilder::new()
        .source(PerformanceSource::NeuralPredictor {
            model_name: "test_model".to_string(),
            predictor_id: "pred_1".to_string(),
        })
        .event_type(PerformanceEventType::PredictionCompleted {
            model: "test_model".to_string(),
            accuracy: 0.95,
            confidence: 0.9,
            latency_ms: 50,
            input_features: 10,
            output_dimension: 1,
            timestamp: Utc::now(),
        })
        .priority(EventPriority::Medium)
        .build()
        .unwrap();
    
    channel.emit(event.clone()).await.unwrap();
    
    // Wait for event propagation
    sleep(StdDuration::from_millis(200)).await;
    
    // Both monitors should receive the event
    assert_eq!(monitor1_events.lock().unwrap().len(), 1);
    assert_eq!(monitor2_events.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn test_event_priority_ordering() {
    let (channel, mut receiver) = PerformanceChannel::new_with_buffer(100);
    
    // Emit events with different priorities
    let critical_event = PerformanceEventBuilder::new()
        .event_type(PerformanceEventType::Alert {
            alert_type: AlertType::ModelFailure,
            message: "Critical model failure".to_string(),
            severity: AlertSeverity::Critical,
            resolution_required: true,
        })
        .priority(EventPriority::Critical)
        .build()
        .unwrap();
    
    let high_event = PerformanceEventBuilder::new()
        .event_type(PerformanceEventType::PerformanceDegradation {
            metric_name: "accuracy".to_string(),
            current_value: 0.7,
            baseline_value: 0.9,
            degradation_percent: 22.2,
            impact_severity: "high".to_string(),
        })
        .priority(EventPriority::High)
        .build()
        .unwrap();
    
    let low_event = PerformanceEventBuilder::new()
        .event_type(PerformanceEventType::MetricsUpdate {
            component: "test".to_string(),
            metrics: HashMap::new(),
            timestamp: Utc::now(),
        })
        .priority(EventPriority::Low)
        .build()
        .unwrap();
    
    // Emit all events
    channel.emit(low_event.clone()).await.unwrap();
    channel.emit(critical_event.clone()).await.unwrap();
    channel.emit(high_event.clone()).await.unwrap();
    
    // Collect events
    let mut received = Vec::new();
    while let Ok(event) = timeout(StdDuration::from_millis(100), receiver.recv()).await {
        if let Ok(event) = event {
            received.push(event);
        }
    }
    
    assert_eq!(received.len(), 3);
    // Events should be in order of emission, not priority
    // (broadcast channel doesn't reorder)
    assert_eq!(received[0].priority, EventPriority::Low);
    assert_eq!(received[1].priority, EventPriority::Critical);
    assert_eq!(received[2].priority, EventPriority::High);
}

#[tokio::test]
async fn test_async_event_handling() {
    let (channel, _) = PerformanceChannel::new_with_buffer(1000);
    let market_hours = Arc::new(MarketHours::new());
    
    let coordinator = MockTrainingCoordinator::new(channel.clone(), market_hours);
    let coordinator_ref = Arc::new(coordinator);
    
    // Create subscriber for coordinator
    let mut subscriber = channel.subscribe();
    let coord = coordinator_ref.clone();
    
    // Process events asynchronously
    tokio::spawn(async move {
        while let Ok(event) = subscriber.recv().await {
            let _ = coord.process_event(event).await;
        }
    });
    
    // Emit performance degradation event
    let degradation_event = PerformanceEventBuilder::new()
        .source(PerformanceSource::NeuralPredictor {
            model_name: "prod_model".to_string(),
            predictor_id: "pred_prod".to_string(),
        })
        .event_type(PerformanceEventType::PerformanceDegradation {
            metric_name: "prediction_accuracy".to_string(),
            current_value: 0.65,
            baseline_value: 0.85,
            degradation_percent: 23.5,
            impact_severity: "high".to_string(),
        })
        .priority(EventPriority::High)
        .build()
        .unwrap();
    
    channel.emit(degradation_event).await.unwrap();
    
    // Wait for processing
    sleep(StdDuration::from_millis(500)).await;
    
    // Check training decision was made
    let decisions = coordinator_ref.get_decisions();
    assert_eq!(decisions.len(), 1);
    
    let decision = &decisions[0];
    assert!(decision.decision.contains("training"));
    assert!(decision.resource_allocation >= 0.0 && decision.resource_allocation <= 1.0);
}

#[tokio::test]
async fn test_error_event_scenarios() {
    let (channel, mut receiver) = PerformanceChannel::new_with_buffer(100);
    
    // Test various error scenarios
    let recoverable_error = PerformanceEventBuilder::new()
        .source(PerformanceSource::DataAdapter {
            adapter_name: "market_data".to_string(),
            adapter_type: "websocket".to_string(),
        })
        .event_type(PerformanceEventType::ModelError {
            model: "data_adapter".to_string(),
            error_type: "connection_timeout".to_string(),
            error_message: "WebSocket connection timed out".to_string(),
            recoverable: true,
        })
        .priority(EventPriority::Medium)
        .build()
        .unwrap();
    
    let critical_error = PerformanceEventBuilder::new()
        .source(PerformanceSource::System {
            service_name: "trading_engine".to_string(),
        })
        .event_type(PerformanceEventType::ModelError {
            model: "trading_engine".to_string(),
            error_type: "memory_corruption".to_string(),
            error_message: "Critical memory corruption detected".to_string(),
            recoverable: false,
        })
        .priority(EventPriority::Critical)
        .build()
        .unwrap();
    
    // Emit errors
    channel.emit(recoverable_error.clone()).await.unwrap();
    channel.emit(critical_error.clone()).await.unwrap();
    
    // Verify reception
    let event1 = receiver.recv().await.unwrap();
    let event2 = receiver.recv().await.unwrap();
    
    // Check error handling logic
    match &event1.event_type {
        PerformanceEventType::ModelError { recoverable, .. } => {
            assert!(*recoverable);
        }
        _ => panic!("Expected ModelError"),
    }
    
    match &event2.event_type {
        PerformanceEventType::ModelError { recoverable, .. } => {
            assert!(!*recoverable);
            assert_eq!(event2.priority, EventPriority::Critical);
        }
        _ => panic!("Expected ModelError"),
    }
}

#[tokio::test]
async fn test_market_hours_integration() {
    let (channel, _) = PerformanceChannel::new_with_buffer(100);
    let market_hours = Arc::new(MarketHours::new());
    
    // Test during market hours
    let nyse_open = Utc.with_ymd_and_hms(2024, 1, 8, 15, 0, 0).unwrap(); // Monday 3PM UTC (10AM EST)
    let is_open = market_hours.is_exchange_open(Exchange::NYSE, nyse_open).await;
    
    // Emit event based on market status
    let event_type = if is_open {
        PerformanceEventType::TradingSignal {
            signal_type: "buy".to_string(),
            profit_loss: 150.0,
            sharpe_ratio: 1.8,
            max_drawdown: 0.05,
            position_size: 1000.0,
            risk_score: 0.3,
        }
    } else {
        PerformanceEventType::SystemHealth {
            component: "market_monitor".to_string(),
            cpu_usage_percent: 5.0,
            memory_usage_mb: 100.0,
            error_rate: 0.0,
            availability_percent: 100.0,
        }
    };
    
    let event = PerformanceEventBuilder::new()
        .source(PerformanceSource::TradingStrategy {
            strategy_name: "momentum".to_string(),
            strategy_id: "mom_1".to_string(),
        })
        .event_type(event_type)
        .tag("market_status".to_string(), if is_open { "open" } else { "closed" })
        .build()
        .unwrap();
    
    channel.emit(event).await.unwrap();
    
    // Check training window
    let window = market_hours.get_training_window(nyse_open).await;
    assert!(matches!(window, TrainingWindow::Poor | TrainingWindow::Restricted));
}

#[tokio::test]
async fn test_decision_flow_triggers() {
    let (channel, _) = PerformanceChannel::new_with_buffer(100);
    let market_hours = Arc::new(MarketHours::new());
    let coordinator = Arc::new(MockTrainingCoordinator::new(channel.clone(), market_hours.clone()));
    
    // Create event processor
    let mut subscriber = channel.subscribe();
    let coord = coordinator.clone();
    
    tokio::spawn(async move {
        while let Ok(event) = subscriber.recv().await {
            let _ = coord.process_event(event).await;
        }
    });
    
    // Simulate weekend (optimal training window)
    let weekend = Utc.with_ymd_and_hms(2024, 1, 6, 12, 0, 0).unwrap(); // Saturday
    
    // Create low accuracy alert during weekend
    let low_accuracy_event = PerformanceEventBuilder::new()
        .source(PerformanceSource::NeuralPredictor {
            model_name: "main_predictor".to_string(),
            predictor_id: "pred_main".to_string(),
        })
        .event_type(PerformanceEventType::Alert {
            alert_type: AlertType::LowAccuracy,
            message: "Model accuracy dropped below threshold".to_string(),
            severity: AlertSeverity::Warning,
            resolution_required: true,
        })
        .priority(EventPriority::High)
        .build()
        .unwrap();
    
    channel.emit(low_accuracy_event).await.unwrap();
    
    // Wait for decision
    sleep(StdDuration::from_millis(300)).await;
    
    let decisions = coordinator.get_decisions();
    assert!(!decisions.is_empty());
    
    // Weekend should trigger immediate training
    let decision = &decisions[0];
    assert_eq!(decision.decision, "immediate_full_training");
    assert!(decision.resource_allocation >= 0.9); // High resources on weekend
}

#[tokio::test]
async fn test_channel_buffer_overflow() {
    let mut config = ChannelConfig::default();
    config.buffer_size = 10; // Small buffer
    config.channel_capacity = 10;
    
    let (channel, mut receiver) = PerformanceChannel::new(config);
    
    // Emit more events than buffer can hold
    for i in 0..20 {
        let event = PerformanceEventBuilder::new()
            .event_type(PerformanceEventType::MetricsUpdate {
                component: format!("test_{}", i),
                metrics: HashMap::new(),
                timestamp: Utc::now(),
            })
            .build()
            .unwrap();
        
        channel.emit_fast(event); // Use fast emit to avoid await
    }
    
    // Buffer should only have last 10 events
    assert_eq!(channel.buffer_size(), 10);
    
    // Recent metrics should be from the last events
    let recent = channel.get_recent_metrics(5);
    assert_eq!(recent.len(), 5);
}

#[tokio::test]
async fn test_model_divergence_detection() {
    let (channel, mut receiver) = PerformanceChannel::new_with_buffer(100);
    
    // Create divergence event
    let divergence_event = PerformanceEventBuilder::new()
        .source(PerformanceSource::IntegrationHub {
            component_name: "ensemble_manager".to_string(),
        })
        .event_type(PerformanceEventType::ModelDivergence {
            model_agreement: 0.45,
            divergence_score: 0.55,
            model_count: 5,
            disagreement_threshold: 0.3,
        })
        .priority(EventPriority::High)
        .tag("ensemble_id".to_string(), "ensemble_prod".to_string())
        .custom_metric("max_divergence".to_string(), 0.75)
        .build()
        .unwrap();
    
    channel.emit(divergence_event.clone()).await.unwrap();
    
    let received = receiver.recv().await.unwrap();
    
    // Verify divergence detection
    match &received.event_type {
        PerformanceEventType::ModelDivergence { model_agreement, divergence_score, .. } => {
            assert!(*model_agreement < 0.5);
            assert!(*divergence_score > 0.5);
        }
        _ => panic!("Expected ModelDivergence event"),
    }
    
    // Check custom metrics
    if let Some(custom) = &received.metrics.custom_metrics {
        assert_eq!(custom.get("max_divergence"), Some(&0.75));
    }
}

#[tokio::test]
async fn test_emergency_override_flow() {
    let market_hours = Arc::new(MarketHours::new());
    let (channel, _) = PerformanceChannel::new_with_buffer(100);
    
    // Request emergency override
    let override_result = market_hours.request_emergency_override(
        "Critical model failure requiring immediate retraining".to_string(),
        EmergencyPriority::Critical,
        1.0, // Request full resources
        Duration::hours(2),
        "system_admin".to_string(),
        vec![Exchange::NYSE, Exchange::NASDAQ],
    ).await;
    
    // Should succeed
    assert!(override_result.is_ok());
    let override_id = override_result.unwrap();
    
    // Emit emergency training event
    let emergency_event = PerformanceEventBuilder::new()
        .source(PerformanceSource::System {
            service_name: "emergency_coordinator".to_string(),
        })
        .event_type(PerformanceEventType::TrainingStarted {
            model: "critical_model".to_string(),
            training_type: "emergency_recovery".to_string(),
            estimated_duration_mins: 120,
        })
        .priority(EventPriority::Critical)
        .tag("override_id".to_string(), override_id.clone())
        .build()
        .unwrap();
    
    channel.emit(emergency_event).await.unwrap();
    
    // Cancel override when done
    let cancel_result = market_hours.cancel_emergency_override(&override_id).await;
    assert!(cancel_result.is_ok());
}

#[tokio::test]
async fn test_health_monitoring_cascade() {
    let (channel, mut receiver) = PerformanceChannel::new_with_buffer(100);
    
    // Create cascading health events
    let component_health = PerformanceEventBuilder::new()
        .source(PerformanceSource::HealthMonitor {
            component: ComponentType::NeuralEngine,
            monitor_id: "health_mon_1".to_string(),
        })
        .event_type(PerformanceEventType::SystemHealth {
            component: "neural_engine".to_string(),
            cpu_usage_percent: 95.0,
            memory_usage_mb: 8000.0,
            error_rate: 0.02,
            availability_percent: 99.8,
        })
        .priority(EventPriority::Medium)
        .build()
        .unwrap();
    
    // High CPU should trigger resource alert
    let resource_alert = PerformanceEventBuilder::new()
        .source(PerformanceSource::System {
            service_name: "resource_monitor".to_string(),
        })
        .event_type(PerformanceEventType::Alert {
            alert_type: AlertType::ResourceExhaustion,
            message: "CPU usage critical - consider scaling".to_string(),
            severity: AlertSeverity::Warning,
            resolution_required: false,
        })
        .priority(EventPriority::High)
        .correlation_id(component_health.id.clone())
        .build()
        .unwrap();
    
    // Emit both events
    channel.emit(component_health).await.unwrap();
    channel.emit(resource_alert).await.unwrap();
    
    // Verify cascade
    let event1 = receiver.recv().await.unwrap();
    let event2 = receiver.recv().await.unwrap();
    
    // Second event should reference first via correlation_id
    assert_eq!(event2.correlation_id, Some(event1.id.clone()));
}

#[tokio::test]
async fn test_channel_statistics() {
    let (channel, _) = PerformanceChannel::new_with_buffer(100);
    
    // Emit several events
    for i in 0..5 {
        let event = PerformanceEventBuilder::new()
            .event_type(PerformanceEventType::MetricsUpdate {
                component: format!("component_{}", i),
                metrics: HashMap::new(),
                timestamp: Utc::now(),
            })
            .build()
            .unwrap();
        
        channel.emit(event).await.unwrap();
        sleep(StdDuration::from_millis(10)).await;
    }
    
    // Check statistics
    let stats = channel.get_statistics().unwrap();
    assert_eq!(stats.total_events_emitted, 5);
    assert!(stats.average_latency_ms > 0.0);
    assert!(stats.buffer_utilization_percent > 0.0);
    assert!(stats.last_event_timestamp.is_some());
}

/// Test helper for creating mock events
fn create_mock_prediction_event(model: &str, accuracy: f64) -> PerformanceEvent {
    PerformanceEventBuilder::new()
        .source(PerformanceSource::NeuralPredictor {
            model_name: model.to_string(),
            predictor_id: format!("{}_id", model),
        })
        .event_type(PerformanceEventType::PredictionCompleted {
            model: model.to_string(),
            accuracy,
            confidence: accuracy * 0.95,
            latency_ms: 50,
            input_features: 20,
            output_dimension: 1,
            timestamp: Utc::now(),
        })
        .build()
        .unwrap()
}

#[tokio::test]
async fn test_concurrent_event_emission() {
    let (channel, mut receiver) = PerformanceChannel::new_with_buffer(1000);
    let channel = Arc::new(channel);
    
    // Spawn multiple tasks emitting events concurrently
    let mut handles = vec![];
    
    for i in 0..10 {
        let ch = channel.clone();
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let event = create_mock_prediction_event(&format!("model_{}_{}", i, j), 0.85);
                ch.emit(event).await.unwrap();
            }
        });
        handles.push(handle);
    }
    
    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Should have received 100 events
    let mut count = 0;
    while let Ok(Ok(_)) = timeout(StdDuration::from_millis(100), receiver.recv()).await {
        count += 1;
    }
    
    assert_eq!(count, 100);
}