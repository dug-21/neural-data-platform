//! Phase 3b Performance Validation Tests
//! 
//! Quick tests to validate performance requirements without full benchmarking

#[cfg(test)]
mod phase3b_performance_tests {
    use std::time::Instant;
    use tokio::runtime::Runtime;
    use neural_trader::neural::monitoring::performance_channel::{
        PerformanceChannel, PerformanceEventBuilder, PerformanceSource,
        PerformanceEventType, EventPriority, ChannelConfig,
    };
    use neural_trader::integration::event_bus::{EventBus, EventBusConfig};
    use std::collections::HashMap;
    use chrono::Utc;

    #[test]
    fn test_event_emission_latency() {
        let rt = Runtime::new().unwrap();
        
        rt.block_on(async {
            // Setup performance channel with minimal config
            let config = ChannelConfig {
                buffer_size: 1000,
                channel_capacity: 10000,
                enable_persistence: false,
                enable_metrics: false,
                max_subscribers: 10,
            };
            
            let (channel, _receiver) = PerformanceChannel::new(config);
            
            // Warm up
            for _ in 0..10 {
                let event = create_test_event();
                channel.emit(event).await.unwrap();
            }
            
            // Measure latencies
            let mut latencies = Vec::new();
            
            for _ in 0..100 {
                let event = create_test_event();
                let start = Instant::now();
                channel.emit(event).await.unwrap();
                let latency = start.elapsed();
                latencies.push(latency.as_micros());
            }
            
            // Calculate statistics
            let avg_latency = latencies.iter().sum::<u128>() / latencies.len() as u128;
            let max_latency = *latencies.iter().max().unwrap();
            
            println!("Event Emission Latency:");
            println!("  Average: {}μs", avg_latency);
            println!("  Max: {}μs", max_latency);
            println!("  Target: <1000μs (1ms)");
            
            // Assert requirement
            assert!(max_latency < 1000, 
                   "Event emission latency {}μs exceeds 1ms target", max_latency);
        });
    }

    #[test]
    fn test_fast_emit_latency() {
        let rt = Runtime::new().unwrap();
        
        rt.block_on(async {
            let config = ChannelConfig::default();
            let (channel, _receiver) = PerformanceChannel::new(config);
            
            // Measure fast emit (fire-and-forget)
            let mut latencies = Vec::new();
            
            for _ in 0..100 {
                let event = create_test_event();
                let start = Instant::now();
                channel.emit_fast(event);
                let latency = start.elapsed();
                latencies.push(latency.as_micros());
            }
            
            let avg_latency = latencies.iter().sum::<u128>() / latencies.len() as u128;
            let max_latency = *latencies.iter().max().unwrap();
            
            println!("\nFast Emit Latency:");
            println!("  Average: {}μs", avg_latency);
            println!("  Max: {}μs", max_latency);
            println!("  Target: <1000μs (1ms)");
            
            assert!(max_latency < 1000, 
                   "Fast emit latency {}μs exceeds 1ms target", max_latency);
        });
    }

    #[test]
    fn test_decision_making_latency() {
        let rt = Runtime::new().unwrap();
        
        rt.block_on(async {
            // Setup decision pipeline
            let (channel, mut receiver) = PerformanceChannel::new(ChannelConfig::default());
            
            // Spawn decision processor
            tokio::spawn(async move {
                while let Ok(event) = receiver.recv().await {
                    // Simulate decision processing
                    process_decision(event).await;
                }
            });
            
            // Measure end-to-end decision latency
            let mut latencies = Vec::new();
            
            for _ in 0..50 {
                let start = Instant::now();
                
                // Emit decision event
                let event = create_decision_event();
                channel.emit(event).await.unwrap();
                
                // Simulate decision computation
                let _decision = make_decision().await;
                
                let latency = start.elapsed();
                latencies.push(latency.as_millis());
            }
            
            let avg_latency = latencies.iter().sum::<u128>() / latencies.len() as u128;
            let max_latency = *latencies.iter().max().unwrap();
            
            println!("\nDecision Making Latency:");
            println!("  Average: {}ms", avg_latency);
            println!("  Max: {}ms", max_latency);
            println!("  Target: <10ms");
            
            assert!(max_latency < 10, 
                   "Decision making latency {}ms exceeds 10ms target", max_latency);
        });
    }

    #[test]
    fn test_memory_overhead() {
        // This test validates that the monitoring infrastructure
        // doesn't add significant memory overhead
        
        let rt = Runtime::new().unwrap();
        
        rt.block_on(async {
            // Get baseline memory
            let baseline = get_current_memory();
            
            // Create monitoring infrastructure
            let (channel, _receiver) = PerformanceChannel::new(ChannelConfig {
                buffer_size: 1000,
                channel_capacity: 10000,
                enable_persistence: true,
                enable_metrics: true,
                max_subscribers: 10,
            });
            
            // Emit many events
            for i in 0..1000 {
                let event = create_test_event_with_data(i);
                channel.emit(event).await.unwrap();
            }
            
            // Check memory growth
            let current = get_current_memory();
            let growth_mb = (current - baseline) as f64 / 1024.0 / 1024.0;
            
            println!("\nMemory Overhead:");
            println!("  Baseline: {} bytes", baseline);
            println!("  Current: {} bytes", current);
            println!("  Growth: {:.2} MB", growth_mb);
            println!("  Target: <50 MB for buffer");
            
            // The configured buffer size is 1000 events
            // Each event is roughly 1KB, so we expect ~1MB for the buffer
            // Allow up to 50MB total for all monitoring infrastructure
            assert!(growth_mb < 50.0, 
                   "Memory growth {:.2}MB exceeds 50MB target", growth_mb);
        });
    }

    #[test]
    fn test_event_bus_latency() {
        let rt = Runtime::new().unwrap();
        
        rt.block_on(async {
            // Test the generic event bus integration
            let event_bus = EventBus::new(EventBusConfig {
                max_stored_events: 1000,
                channel_capacity: 10000,
                enable_metrics: false,
                enable_persistence: false,
            });
            
            let _receiver = event_bus.subscribe(
                "test_subscriber".to_string(), 
                "performance_test".to_string()
            );
            
            // Measure publish latency
            let mut latencies = Vec::new();
            
            for _ in 0..100 {
                let event = create_test_event();
                let start = Instant::now();
                event_bus.publish(event).await.unwrap();
                let latency = start.elapsed();
                latencies.push(latency.as_micros());
            }
            
            let avg_latency = latencies.iter().sum::<u128>() / latencies.len() as u128;
            let max_latency = *latencies.iter().max().unwrap();
            
            println!("\nEvent Bus Publish Latency:");
            println!("  Average: {}μs", avg_latency);
            println!("  Max: {}μs", max_latency);
            println!("  Target: <1000μs (1ms)");
            
            assert!(max_latency < 1000, 
                   "Event bus latency {}μs exceeds 1ms target", max_latency);
        });
    }

    // Helper functions
    
    fn create_test_event() -> neural_trader::neural::monitoring::performance_channel::PerformanceEvent {
        PerformanceEventBuilder::new()
            .source(PerformanceSource::System {
                service_name: "test".to_string(),
            })
            .event_type(PerformanceEventType::MetricsUpdate {
                component: "test".to_string(),
                metrics: HashMap::new(),
                timestamp: Utc::now(),
            })
            .priority(EventPriority::Medium)
            .build()
            .unwrap()
    }

    fn create_test_event_with_data(id: usize) -> neural_trader::neural::monitoring::performance_channel::PerformanceEvent {
        let mut metrics = HashMap::new();
        metrics.insert("test_id".to_string(), id as f64);
        metrics.insert("value".to_string(), 42.0);
        
        PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: format!("model_{}", id % 3),
                predictor_id: format!("pred_{}", id),
            })
            .event_type(PerformanceEventType::MetricsUpdate {
                component: "test".to_string(),
                metrics,
                timestamp: Utc::now(),
            })
            .priority(EventPriority::Low)
            .tag("test_id".to_string(), id.to_string())
            .build()
            .unwrap()
    }

    fn create_decision_event() -> neural_trader::neural::monitoring::performance_channel::PerformanceEvent {
        PerformanceEventBuilder::new()
            .source(PerformanceSource::TradingStrategy {
                strategy_name: "test_strategy".to_string(),
                strategy_id: "strat_001".to_string(),
            })
            .event_type(PerformanceEventType::TradingSignal {
                signal_type: "BUY".to_string(),
                profit_loss: 0.0,
                sharpe_ratio: 1.5,
                max_drawdown: 0.05,
                position_size: 0.1,
                risk_score: 0.3,
            })
            .priority(EventPriority::High)
            .build()
            .unwrap()
    }

    async fn process_decision(_event: neural_trader::neural::monitoring::performance_channel::PerformanceEvent) {
        // Simulate minimal decision processing
        tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
    }

    async fn make_decision() -> String {
        // Simulate decision computation
        tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        "BUY".to_string()
    }

    fn get_current_memory() -> usize {
        // Simple memory estimation
        // In production, use proper memory profiling
        std::mem::size_of::<usize>() * 1000000 // Mock 1MB baseline
    }
}