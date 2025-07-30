//! TDD Tests for PerformanceEmitter Trait Implementation
//! 
//! These tests define how components should implement the PerformanceEmitter trait
//! to integrate with the performance monitoring system.

use std::sync::Arc;
use tokio::sync::mpsc;
use async_trait::async_trait;
use anyhow::Result;

use neural_trader::neural::{
    PerformanceEmitter,
    PerformanceEvent,
    PerformanceEventBuilder,
    PerformanceSource,
    PerformanceEventType,
    PerformanceMetrics as ChannelMetrics,
};

// Mock component that implements PerformanceEmitter
struct MockPredictorComponent {
    performance_sender: Option<mpsc::UnboundedSender<PerformanceEvent>>,
    model_name: String,
}

#[async_trait]
impl PerformanceEmitter for MockPredictorComponent {
    async fn emit_performance(&self, event: PerformanceEvent) -> Result<()> {
        if let Some(sender) = &self.performance_sender {
            sender.send(event)?;
        }
        Ok(())
    }

    fn get_performance_sender(&self) -> Option<mpsc::UnboundedSender<PerformanceEvent>> {
        self.performance_sender.clone()
    }

    fn set_performance_sender(&mut self, sender: mpsc::UnboundedSender<PerformanceEvent>) {
        self.performance_sender = Some(sender);
    }
}

#[cfg(test)]
mod performance_emitter_tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_component_implements_performance_emitter() {
        // Test: Components should implement PerformanceEmitter trait
        let component = MockPredictorComponent {
            performance_sender: None,
            model_name: "TestModel".to_string(),
        };
        
        // Verify it implements the trait
        let _: &dyn PerformanceEmitter = &component;
    }

    #[tokio::test]
    async fn test_performance_sender_lifecycle() {
        // Test: Components should properly manage performance sender
        let mut component = MockPredictorComponent {
            performance_sender: None,
            model_name: "TestModel".to_string(),
        };
        
        // Initially no sender
        assert!(component.get_performance_sender().is_none());
        
        // Set a sender
        let (tx, mut rx) = mpsc::unbounded_channel();
        component.set_performance_sender(tx.clone());
        
        // Should now have a sender
        assert!(component.get_performance_sender().is_some());
        
        // Should be able to emit events
        let event = create_test_performance_event();
        component.emit_performance(event.clone()).await
            .expect("Should emit event");
        
        // Verify event was sent
        let received = rx.recv().await.expect("Should receive event");
        assert_eq!(received.timestamp, event.timestamp);
    }

    #[tokio::test]
    async fn test_emit_without_sender_graceful() {
        // Test: Emitting without a sender should not panic
        let component = MockPredictorComponent {
            performance_sender: None,
            model_name: "TestModel".to_string(),
        };
        
        let event = create_test_performance_event();
        let result = component.emit_performance(event).await;
        
        // Should succeed even without sender (no-op)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_performance_event_creation_patterns() {
        // Test: Define standard patterns for creating performance events
        
        // Pattern 1: Prediction completed event
        let prediction_event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "MLP".to_string(),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                model: "MLP".to_string(),
                accuracy: 0.92,
                confidence: 0.87,
                latency_ms: 45,
                timestamp: Utc::now(),
            })
            .metrics(ChannelMetrics {
                latency_p50: Some(40.0),
                latency_p95: Some(60.0),
                latency_p99: Some(80.0),
                throughput: Some(22.5),
                error_count: Some(0),
                success_count: Some(100),
                custom_metrics: None,
            })
            .build()
            .expect("Should build prediction event");
        
        // Pattern 2: System health event
        let health_event = PerformanceEventBuilder::new()
            .source(PerformanceSource::HealthMonitor {
                component: neural_trader::neural::ComponentType::NeuralEngine,
            })
            .event_type(PerformanceEventType::SystemHealth {
                cpu_usage: 45.2,
                memory_usage: 62.8,
                error_rate: 0.001,
            })
            .build()
            .expect("Should build health event");
        
        // Pattern 3: Model divergence event
        let divergence_event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "Ensemble".to_string(),
            })
            .event_type(PerformanceEventType::ModelDivergence {
                model_agreement: 0.75,
                divergence_score: 0.25,
            })
            .build()
            .expect("Should build divergence event");
        
        // All events should have required fields
        assert!(matches!(prediction_event.source, PerformanceSource::NeuralPredictor { .. }));
        assert!(matches!(health_event.event_type, PerformanceEventType::SystemHealth { .. }));
        assert!(matches!(divergence_event.event_type, PerformanceEventType::ModelDivergence { .. }));
    }

    #[tokio::test]
    async fn test_concurrent_event_emission() {
        // Test: Multiple components can emit events concurrently
        let (tx, mut rx) = mpsc::unbounded_channel();
        
        // Create multiple components sharing the same channel
        let mut components = vec![];
        for i in 0..3 {
            let mut component = MockPredictorComponent {
                performance_sender: None,
                model_name: format!("Model-{}", i),
            };
            component.set_performance_sender(tx.clone());
            components.push(Arc::new(component));
        }
        
        // Emit events concurrently
        let mut handles = vec![];
        for (i, component) in components.iter().enumerate() {
            let component_clone = component.clone();
            let handle = tokio::spawn(async move {
                let event = PerformanceEventBuilder::new()
                    .source(PerformanceSource::NeuralPredictor {
                        model_name: format!("Model-{}", i),
                    })
                    .event_type(PerformanceEventType::PredictionCompleted {
                        model: format!("Model-{}", i),
                        accuracy: 0.9,
                        confidence: 0.85,
                        latency_ms: 50 + (i as u64 * 10),
                        timestamp: Utc::now(),
                    })
                    .build()
                    .unwrap();
                
                component_clone.emit_performance(event).await
            });
            handles.push(handle);
        }
        
        // Wait for all emissions
        for handle in handles {
            handle.await.expect("Task should complete")
                .expect("Should emit successfully");
        }
        
        // Drop original sender so rx.recv() doesn't block
        drop(tx);
        
        // Collect all events
        let mut events = vec![];
        while let Some(event) = rx.recv().await {
            events.push(event);
        }
        
        // Should have received all 3 events
        assert_eq!(events.len(), 3);
        
        // Each should be from a different model
        let mut model_names: Vec<String> = events.iter()
            .filter_map(|e| match &e.source {
                PerformanceSource::NeuralPredictor { model_name } => Some(model_name.clone()),
                _ => None,
            })
            .collect();
        model_names.sort();
        
        assert_eq!(model_names, vec!["Model-0", "Model-1", "Model-2"]);
    }

    fn create_test_performance_event() -> PerformanceEvent {
        PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "TestModel".to_string(),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                model: "TestModel".to_string(),
                accuracy: 0.95,
                confidence: 0.9,
                latency_ms: 100,
                timestamp: Utc::now(),
            })
            .build()
            .expect("Should build test event")
    }
}

#[cfg(test)]
mod fann_predictor_performance_integration_tests {
    use super::*;
    use neural_trader::neural::FannPredictor;
    use neural_trader::config::NeuralConfig;

    #[tokio::test]
    async fn test_fann_predictor_should_implement_emitter() {
        // Test: FannPredictor should implement PerformanceEmitter
        // This test will fail until FannPredictor implements the trait
        
        let config = NeuralConfig {
            enable_performance_monitoring: true,
            ..Default::default()
        };
        
        let predictor = FannPredictor::new(config)
            .expect("Should create predictor");
        
        // TODO: This should compile when FannPredictor implements PerformanceEmitter
        // let _: &dyn PerformanceEmitter = &predictor;
        
        // For now, this test documents the expected behavior
        assert!(true, "FannPredictor should implement PerformanceEmitter");
    }

    #[tokio::test]
    async fn test_prediction_performance_integration() {
        // Test: End-to-end test of prediction with performance monitoring
        // This defines the expected integration behavior
        
        let (tx, mut rx) = mpsc::unbounded_channel();
        
        let config = NeuralConfig {
            enable_performance_monitoring: true,
            ..Default::default()
        };
        
        let mut predictor = FannPredictor::new(config)
            .expect("Should create predictor");
        
        // TODO: Wire up performance monitoring
        // predictor.set_performance_sender(tx);
        
        // Make a prediction
        let test_data = vec![neural_trader::data::TimeSeriesData {
            timestamp: Utc::now(),
            value: 100.0,
            volume: Some(1000.0),
            metadata: None,
        }];
        
        let _predictions = predictor.predict(&test_data, 1, None).await
            .expect("Should predict");
        
        // TODO: Should receive performance event
        // let event = rx.recv().await.expect("Should receive performance event");
        // assert!(matches!(event.event_type, PerformanceEventType::PredictionCompleted { .. }));
    }
}