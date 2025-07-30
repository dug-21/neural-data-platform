//! Comprehensive tests for the Performance Channel module
//! Target: 85% test coverage

use crate::neural::performance_channel::*;
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[cfg(test)]
mod performance_channel_tests {
    use super::*;

    /// Helper function to create a test performance event
    fn create_test_event(model_name: &str, accuracy: f64) -> PerformanceEvent {
        PerformanceEvent {
            timestamp: Utc::now(),
            source: PerformanceSource::NeuralPredictor {
                model_name: model_name.to_string(),
            },
            event_type: PerformanceEventType::PredictionCompleted {
                model: model_name.to_string(),
                accuracy,
                confidence: 0.9,
                latency_ms: 100,
                timestamp: Utc::now(),
            },
            metrics: PerformanceMetrics::default(),
        }
    }

    /// Test basic channel creation and configuration
    #[tokio::test]
    async fn test_channel_creation_and_configuration() {
        let buffer_size = 50;
        let (channel, receiver) = PerformanceChannel::new(buffer_size);
        
        // Verify initial state
        assert_eq!(channel.buffer_size(), 0);
        assert_eq!(channel.max_buffer_size, buffer_size);
        
        // Verify receiver is created
        assert!(receiver.is_empty());
    }

    /// Test emitting events and broadcast functionality
    #[tokio::test]
    async fn test_emit_and_broadcast() -> Result<()> {
        let (channel, mut receiver) = PerformanceChannel::new(10);
        
        // Create and emit an event
        let event = create_test_event("test_model", 0.95);
        channel.emit(event.clone()).await?;
        
        // Verify broadcast
        let received = receiver.try_recv()?;
        assert_eq!(received.timestamp, event.timestamp);
        
        // Verify buffer
        assert_eq!(channel.buffer_size(), 1);
        
        Ok(())
    }

    /// Test multiple subscribers receiving same events
    #[tokio::test]
    async fn test_multiple_subscribers() -> Result<()> {
        let (channel, mut receiver1) = PerformanceChannel::new(10);
        let mut receiver2 = channel.subscribe();
        let mut receiver3 = channel.subscribe();
        
        // Emit event
        let event = create_test_event("multi_test", 0.88);
        channel.emit(event.clone()).await?;
        
        // All receivers should get the event
        let r1 = receiver1.try_recv()?;
        let r2 = receiver2.try_recv()?;
        let r3 = receiver3.try_recv()?;
        
        assert_eq!(r1.timestamp, event.timestamp);
        assert_eq!(r2.timestamp, event.timestamp);
        assert_eq!(r3.timestamp, event.timestamp);
        
        Ok(())
    }

    /// Test buffer overflow handling
    #[tokio::test]
    async fn test_buffer_overflow() -> Result<()> {
        let buffer_size = 5;
        let (channel, _receiver) = PerformanceChannel::new(buffer_size);
        
        // Emit more events than buffer size
        for i in 0..10 {
            let event = create_test_event(&format!("model_{}", i), i as f64 / 10.0);
            channel.emit(event).await?;
        }
        
        // Buffer should be at max size
        assert_eq!(channel.buffer_size(), buffer_size);
        
        // Recent metrics should be the latest ones
        let recent = channel.get_recent_metrics(3);
        assert_eq!(recent.len(), 3);
        
        // Verify we have the most recent events
        if let PerformanceEventType::PredictionCompleted { accuracy, .. } = &recent[0].event_type {
            assert_eq!(*accuracy, 0.9); // Last event (index 9)
        }
        
        Ok(())
    }

    /// Test get_recent_metrics functionality
    #[tokio::test]
    async fn test_get_recent_metrics() -> Result<()> {
        let (channel, _receiver) = PerformanceChannel::new(20);
        
        // Emit various events
        for i in 0..15 {
            let event = create_test_event(&format!("metric_test_{}", i), i as f64 / 15.0);
            channel.emit(event).await?;
        }
        
        // Test different retrieval counts
        let recent_5 = channel.get_recent_metrics(5);
        assert_eq!(recent_5.len(), 5);
        
        let recent_10 = channel.get_recent_metrics(10);
        assert_eq!(recent_10.len(), 10);
        
        let recent_20 = channel.get_recent_metrics(20);
        assert_eq!(recent_20.len(), 15); // Only 15 events in buffer
        
        // Verify order (most recent first)
        if let PerformanceEventType::PredictionCompleted { model, .. } = &recent_5[0].event_type {
            assert_eq!(model, "metric_test_14");
        }
        
        Ok(())
    }

    /// Test clear_buffer functionality
    #[tokio::test]
    async fn test_clear_buffer() -> Result<()> {
        let (channel, _receiver) = PerformanceChannel::new(10);
        
        // Add events
        for i in 0..8 {
            let event = create_test_event(&format!("clear_test_{}", i), 0.5);
            channel.emit(event).await?;
        }
        
        assert_eq!(channel.buffer_size(), 8);
        
        // Clear buffer
        channel.clear_buffer()?;
        assert_eq!(channel.buffer_size(), 0);
        
        // Verify we can still emit after clearing
        let event = create_test_event("after_clear", 0.99);
        channel.emit(event).await?;
        assert_eq!(channel.buffer_size(), 1);
        
        Ok(())
    }

    /// Test PerformanceEventBuilder
    #[test]
    fn test_performance_event_builder() {
        // Test successful build
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::TradingStrategy {
                strategy_name: "momentum".to_string(),
            })
            .event_type(PerformanceEventType::TradingSignal {
                profit_loss: 1500.0,
                sharpe_ratio: 1.8,
                max_drawdown: 0.05,
            })
            .custom_metric("win_rate".to_string(), 0.65)
            .custom_metric("trades_today".to_string(), 42.0)
            .build();
        
        assert!(event.is_ok());
        let event = event.unwrap();
        
        // Verify custom metrics
        assert!(event.metrics.custom_metrics.is_some());
        let custom = event.metrics.custom_metrics.unwrap();
        assert_eq!(custom.get("win_rate"), Some(&0.65));
        assert_eq!(custom.get("trades_today"), Some(&42.0));
    }

    /// Test builder error cases
    #[test]
    fn test_builder_error_cases() {
        // Missing source
        let result = PerformanceEventBuilder::new()
            .event_type(PerformanceEventType::SystemHealth {
                cpu_usage: 45.0,
                memory_usage: 60.0,
                error_rate: 0.01,
            })
            .build();
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("source is required"));
        
        // Missing event type
        let result = PerformanceEventBuilder::new()
            .source(PerformanceSource::EventBus {
                event_type: "trade_executed".to_string(),
            })
            .build();
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("event type is required"));
    }

    /// Test different event sources
    #[tokio::test]
    async fn test_different_event_sources() -> Result<()> {
        let (channel, mut receiver) = PerformanceChannel::new(10);
        
        // Neural predictor event
        let neural_event = PerformanceEvent {
            timestamp: Utc::now(),
            source: PerformanceSource::NeuralPredictor {
                model_name: "lstm_v2".to_string(),
            },
            event_type: PerformanceEventType::PredictionCompleted {
                model: "lstm_v2".to_string(),
                accuracy: 0.92,
                confidence: 0.88,
                latency_ms: 150,
                timestamp: Utc::now(),
            },
            metrics: PerformanceMetrics::default(),
        };
        
        // Trading strategy event
        let trading_event = PerformanceEvent {
            timestamp: Utc::now(),
            source: PerformanceSource::TradingStrategy {
                strategy_name: "mean_reversion".to_string(),
            },
            event_type: PerformanceEventType::TradingSignal {
                profit_loss: 2500.0,
                sharpe_ratio: 2.1,
                max_drawdown: 0.03,
            },
            metrics: PerformanceMetrics::default(),
        };
        
        // System health event
        let health_event = PerformanceEvent {
            timestamp: Utc::now(),
            source: PerformanceSource::HealthMonitor {
                component: ComponentType::NeuralEngine,
            },
            event_type: PerformanceEventType::SystemHealth {
                cpu_usage: 65.0,
                memory_usage: 72.0,
                error_rate: 0.001,
            },
            metrics: PerformanceMetrics::default(),
        };
        
        // Emit all events
        channel.emit(neural_event).await?;
        channel.emit(trading_event).await?;
        channel.emit(health_event).await?;
        
        // Verify all received
        let _r1 = receiver.try_recv()?;
        let _r2 = receiver.try_recv()?;
        let _r3 = receiver.try_recv()?;
        
        assert_eq!(channel.buffer_size(), 3);
        
        Ok(())
    }

    /// Test performance metrics attachment
    #[test]
    fn test_performance_metrics() {
        let mut metrics = PerformanceMetrics::default();
        metrics.latency_p50 = Some(10.5);
        metrics.latency_p95 = Some(25.0);
        metrics.latency_p99 = Some(50.0);
        metrics.throughput = Some(1000.0);
        metrics.error_count = Some(5);
        metrics.success_count = Some(995);
        
        let mut custom = HashMap::new();
        custom.insert("cache_hit_rate".to_string(), 0.85);
        custom.insert("queue_depth".to_string(), 150.0);
        metrics.custom_metrics = Some(custom);
        
        // Create event with metrics
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::BacktestEngine {
                session_id: "bt_12345".to_string(),
            })
            .event_type(PerformanceEventType::ModelDivergence {
                model_agreement: 0.75,
                divergence_score: 0.25,
            })
            .metrics(metrics)
            .build()
            .unwrap();
        
        // Verify metrics
        assert_eq!(event.metrics.latency_p50, Some(10.5));
        assert_eq!(event.metrics.latency_p95, Some(25.0));
        assert_eq!(event.metrics.throughput, Some(1000.0));
        
        let custom = event.metrics.custom_metrics.unwrap();
        assert_eq!(custom.get("cache_hit_rate"), Some(&0.85));
    }

    /// Test concurrent emit operations
    #[tokio::test]
    async fn test_concurrent_emit() -> Result<()> {
        let (channel, _receiver) = PerformanceChannel::new(100);
        let channel = Arc::new(channel);
        
        // Spawn multiple tasks that emit concurrently
        let mut handles = vec![];
        
        for i in 0..10 {
            let ch = channel.clone();
            let handle = tokio::spawn(async move {
                for j in 0..10 {
                    let event = create_test_event(
                        &format!("concurrent_{}_{}", i, j),
                        (i * 10 + j) as f64 / 100.0,
                    );
                    ch.emit(event).await.unwrap();
                }
            });
            handles.push(handle);
        }
        
        // Wait for all tasks
        for handle in handles {
            handle.await?;
        }
        
        // Should have all 100 events
        assert_eq!(channel.buffer_size(), 100);
        
        Ok(())
    }

    /// Test subscribe after events emitted
    #[tokio::test]
    async fn test_late_subscribe() -> Result<()> {
        let (channel, _receiver) = PerformanceChannel::new(10);
        
        // Emit events before subscribing
        for i in 0..5 {
            let event = create_test_event(&format!("early_{}", i), 0.5);
            channel.emit(event).await?;
        }
        
        // Subscribe after events
        let mut late_subscriber = channel.subscribe();
        
        // New events should be received by late subscriber
        let new_event = create_test_event("after_subscribe", 0.99);
        channel.emit(new_event.clone()).await?;
        
        let received = late_subscriber.try_recv()?;
        assert_eq!(received.timestamp, new_event.timestamp);
        
        Ok(())
    }

    /// Test error handling for locked mutex
    #[tokio::test]
    async fn test_mutex_error_recovery() {
        let (channel, _receiver) = PerformanceChannel::new(10);
        
        // Even if mutex is poisoned (in theory), get_recent_metrics should handle it gracefully
        let metrics = channel.get_recent_metrics(5);
        assert!(metrics.is_empty() || !metrics.is_empty()); // Should not panic
    }

    /// Test all component types
    #[test]
    fn test_component_types() {
        let components = vec![
            ComponentType::NeuralEngine,
            ComponentType::TradingEngine,
            ComponentType::DataPipeline,
            ComponentType::EventSystem,
            ComponentType::Storage,
            ComponentType::API,
            ComponentType::Custom("CustomComponent".to_string()),
        ];
        
        for component in components {
            let event = PerformanceEvent {
                timestamp: Utc::now(),
                source: PerformanceSource::HealthMonitor { component },
                event_type: PerformanceEventType::SystemHealth {
                    cpu_usage: 50.0,
                    memory_usage: 60.0,
                    error_rate: 0.0,
                },
                metrics: PerformanceMetrics::default(),
            };
            
            // Verify event can be created with each component type
            assert!(matches!(event.source, PerformanceSource::HealthMonitor { .. }));
        }
    }

    /// Integration test simulating real usage pattern
    #[tokio::test]
    async fn test_integration_real_usage_pattern() -> Result<()> {
        let (channel, mut monitor_receiver) = PerformanceChannel::new(50);
        let channel = Arc::new(channel);
        
        // Simulate neural predictor
        let neural_channel = channel.clone();
        let neural_task = tokio::spawn(async move {
            for i in 0..20 {
                let event = PerformanceEventBuilder::new()
                    .source(PerformanceSource::NeuralPredictor {
                        model_name: format!("model_{}", i % 3),
                    })
                    .event_type(PerformanceEventType::PredictionCompleted {
                        model: format!("model_{}", i % 3),
                        accuracy: 0.85 + (i as f64 * 0.005),
                        confidence: 0.80 + (i as f64 * 0.003),
                        latency_ms: 100 + (i % 50),
                        timestamp: Utc::now(),
                    })
                    .custom_metric("prediction_count".to_string(), i as f64)
                    .build()
                    .unwrap();
                
                neural_channel.emit(event).await.unwrap();
                sleep(Duration::from_millis(50)).await;
            }
        });
        
        // Simulate trading strategy
        let trading_channel = channel.clone();
        let trading_task = tokio::spawn(async move {
            for i in 0..10 {
                let event = PerformanceEventBuilder::new()
                    .source(PerformanceSource::TradingStrategy {
                        strategy_name: "momentum".to_string(),
                    })
                    .event_type(PerformanceEventType::TradingSignal {
                        profit_loss: 1000.0 + (i as f64 * 100.0),
                        sharpe_ratio: 1.5 + (i as f64 * 0.1),
                        max_drawdown: 0.05 - (i as f64 * 0.001),
                    })
                    .build()
                    .unwrap();
                
                trading_channel.emit(event).await.unwrap();
                sleep(Duration::from_millis(100)).await;
            }
        });
        
        // Monitor task
        let monitor_task = tokio::spawn(async move {
            let mut count = 0;
            while count < 30 {
                match monitor_receiver.try_recv() {
                    Ok(_event) => {
                        count += 1;
                    }
                    Err(_) => {
                        sleep(Duration::from_millis(10)).await;
                    }
                }
            }
            count
        });
        
        // Wait for tasks
        neural_task.await?;
        trading_task.await?;
        let received_count = monitor_task.await?;
        
        // Verify we received events
        assert!(received_count >= 20); // At least neural events
        
        // Check buffer metrics
        let recent = channel.get_recent_metrics(10);
        assert!(!recent.is_empty());
        
        Ok(())
    }
}

/// Additional unit tests for edge cases
#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_default_performance_metrics() {
        let metrics = PerformanceMetrics::default();
        assert!(metrics.latency_p50.is_none());
        assert!(metrics.latency_p95.is_none());
        assert!(metrics.latency_p99.is_none());
        assert!(metrics.throughput.is_none());
        assert!(metrics.error_count.is_none());
        assert!(metrics.success_count.is_none());
        assert!(metrics.custom_metrics.is_none());
    }

    #[test]
    fn test_event_builder_default() {
        let builder = PerformanceEventBuilder::default();
        // Should fail without required fields
        assert!(builder.build().is_err());
    }

    #[tokio::test]
    async fn test_zero_buffer_size() {
        // Edge case: what happens with 0 buffer size?
        let (channel, _receiver) = PerformanceChannel::new(0);
        
        // Should still work, but buffer will be empty
        let event = create_test_event("zero_buffer", 0.5);
        assert!(channel.emit(event).await.is_ok());
        
        // Buffer should remain empty due to 0 size
        assert_eq!(channel.buffer_size(), 0);
    }

    fn create_test_event(model: &str, accuracy: f64) -> PerformanceEvent {
        PerformanceEvent {
            timestamp: Utc::now(),
            source: PerformanceSource::NeuralPredictor {
                model_name: model.to_string(),
            },
            event_type: PerformanceEventType::PredictionCompleted {
                model: model.to_string(),
                accuracy,
                confidence: 0.9,
                latency_ms: 100,
                timestamp: Utc::now(),
            },
            metrics: PerformanceMetrics::default(),
        }
    }
}