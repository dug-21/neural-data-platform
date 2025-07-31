//! Integration tests for the autonomous training system
//! Tests the full flow from performance monitoring to training decisions

use std::sync::Arc;
use tokio::sync::mpsc;
use chrono::{Utc, Duration};

use neural_trader::daa::autonomous_training::{
    AutonomousTrainingEngine, TrainingTriggerConfig, PerformanceSnapshot,
    TrainingDecisionType, DAATrainingIntegration, TrainingOutcome,
};

use neural_trader::integration::daa_coordinator::{DAACoordinator, DAAConfig};
use neural_trader::integration::autonomous_neural_coordinator::AutonomousNeuralCoordinator;

/// Helper to create a DAA coordinator for testing
async fn create_test_coordinator() -> Arc<DAACoordinator> {
    let config = DAAConfig {
        enable_learning: true,
        enable_coordination: true,
        persistence_mode: neural_trader::integration::daa_coordinator::PersistenceMode::Memory,
        agent_timeout_seconds: 30,
        max_agents: 10,
        enable_neural_integration: true,
        neural_model_path: None,
        performance_threshold: 0.7,
        retraining_interval_hours: 24,
        enable_autonomous_trading: false,
    };
    
    Arc::new(DAACoordinator::new(config).await.unwrap())
}

#[cfg(test)]
mod autonomous_training_integration {
    use super::*;

    #[tokio::test]
    async fn test_full_autonomous_training_flow() {
        // Create the autonomous training engine
        let config = TrainingTriggerConfig::default();
        let (engine, receiver) = AutonomousTrainingEngine::new(config).unwrap();
        let engine_arc = Arc::new(engine);
        
        // Create DAA coordinator
        let coordinator = create_test_coordinator().await;
        
        // Create DAA training integration
        let mut integration = DAATrainingIntegration::new(
            Arc::clone(&engine_arc),
            receiver,
        );
        
        // Spawn integration processing task
        let integration_handle = tokio::spawn(async move {
            integration.start_processing().await
        });
        
        // Simulate performance monitoring
        let poor_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.5,
            confidence: 0.4,
            price_error: 0.2,
            sharpe_ratio: 0.2,
            max_drawdown: 0.25,
            volatility: 0.08,
            model_agreement: 0.6,
            consecutive_failures: 8,
            trading_volume: 500_000.0,
            profit_loss: -0.08,
        };
        
        // Evaluate performance and trigger training
        let decision = engine_arc.evaluate_training_need(poor_performance).await.unwrap();
        
        // Verify training was triggered
        assert!(!matches!(decision.decision_type, TrainingDecisionType::NoTraining { .. }));
        
        // Wait a bit for processing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Check decision was executed
        let history = engine_arc.get_decision_history().await;
        let record = &history[&decision.decision_id];
        assert!(record.execution_started.is_some());
        
        // Clean up
        drop(engine_arc); // This will close the channel
        let _ = tokio::time::timeout(
            tokio::time::Duration::from_secs(1),
            integration_handle
        ).await;
    }

    #[tokio::test]
    async fn test_autonomous_neural_coordinator_integration() {
        // Create autonomous neural coordinator
        let coordinator = create_test_coordinator().await;
        let neural_coordinator = AutonomousNeuralCoordinator::new(coordinator).await.unwrap();
        
        // Start the coordinator
        let handle = tokio::spawn(async move {
            neural_coordinator.start().await
        });
        
        // Let it run briefly
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Cancel the task
        handle.abort();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn test_multiple_agent_coordination() {
        let config = TrainingTriggerConfig::default();
        let (engine, mut receiver) = AutonomousTrainingEngine::new(config).unwrap();
        let engine_arc = Arc::new(engine);
        
        // Simulate multiple agents reporting performance
        let agents = vec!["agent1", "agent2", "agent3"];
        let mut handles = Vec::new();
        
        for (i, agent) in agents.iter().enumerate() {
            let engine_clone = Arc::clone(&engine_arc);
            let agent = agent.to_string();
            
            let handle = tokio::spawn(async move {
                let mut performance = PerformanceSnapshot {
                    timestamp: Utc::now(),
                    accuracy: 0.6 - (i as f64 * 0.1),
                    confidence: 0.7 - (i as f64 * 0.1),
                    price_error: 0.1 + (i as f64 * 0.05),
                    sharpe_ratio: 0.5 - (i as f64 * 0.1),
                    max_drawdown: 0.15 + (i as f64 * 0.05),
                    volatility: 0.03 + (i as f64 * 0.01),
                    model_agreement: 0.8 - (i as f64 * 0.1),
                    consecutive_failures: i * 2,
                    trading_volume: 1_000_000.0,
                    profit_loss: -0.01 * (i as f64),
                };
                
                engine_clone.evaluate_training_need(performance).await
            });
            
            handles.push(handle);
        }
        
        // Wait for all evaluations
        for handle in handles {
            handle.await.unwrap().unwrap();
        }
        
        // Count decisions sent to DAA
        let mut decision_count = 0;
        while receiver.try_recv().is_ok() {
            decision_count += 1;
        }
        
        // At least some agents should trigger training
        assert!(decision_count > 0);
        
        // Check decision history
        let history = engine_arc.get_decision_history().await;
        assert_eq!(history.len(), 3);
    }

    #[tokio::test]
    async fn test_performance_improvement_tracking() {
        let config = TrainingTriggerConfig::default();
        let (engine, _receiver) = AutonomousTrainingEngine::new(config).unwrap();
        
        // Initial poor performance
        let initial_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.5,
            confidence: 0.6,
            price_error: 0.15,
            sharpe_ratio: 0.3,
            max_drawdown: 0.2,
            volatility: 0.05,
            model_agreement: 0.7,
            consecutive_failures: 6,
            trading_volume: 1_000_000.0,
            profit_loss: -0.05,
        };
        
        let decision = engine.evaluate_training_need(initial_performance).await.unwrap();
        
        // Mark training as started
        engine.mark_decision_executed(&decision.decision_id).await.unwrap();
        
        // Simulate successful training
        engine.mark_training_completed(
            &decision.decision_id,
            TrainingOutcome::Success {
                improvement_percentage: 20.0,
                new_accuracy: 0.7,
            }
        ).await.unwrap();
        
        // Evaluate improved performance
        let improved_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.7,
            confidence: 0.8,
            price_error: 0.08,
            sharpe_ratio: 0.6,
            max_drawdown: 0.12,
            volatility: 0.03,
            model_agreement: 0.85,
            consecutive_failures: 0,
            trading_volume: 1_000_000.0,
            profit_loss: 0.03,
        };
        
        let new_decision = engine.evaluate_training_need(improved_performance).await.unwrap();
        
        // Should not trigger immediate retraining
        assert!(matches!(new_decision.decision_type, TrainingDecisionType::NoTraining { .. }));
    }

    #[tokio::test]
    async fn test_emergency_response_time() {
        let config = TrainingTriggerConfig::default();
        let (engine, mut receiver) = AutonomousTrainingEngine::new(config).unwrap();
        
        // Critical performance requiring emergency response
        let critical_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.2, // Very low
            confidence: 0.3,
            price_error: 0.4,
            sharpe_ratio: -0.5,
            max_drawdown: 0.5, // 50% drawdown!
            volatility: 0.15,
            model_agreement: 0.3,
            consecutive_failures: 20,
            trading_volume: 1_000_000.0,
            profit_loss: -0.25,
        };
        
        let start = std::time::Instant::now();
        let decision = engine.evaluate_training_need(critical_performance).await.unwrap();
        let evaluation_time = start.elapsed();
        
        // Should respond quickly
        assert!(evaluation_time.as_millis() < 50);
        
        // Should trigger emergency training
        assert!(matches!(decision.decision_type, TrainingDecisionType::Emergency { .. }));
        assert_eq!(decision.priority, neural_trader::daa::autonomous_training::TrainingPriority::Emergency);
        
        // Decision should be sent immediately
        let sent_decision = receiver.try_recv().unwrap();
        assert_eq!(sent_decision.decision_id, decision.decision_id);
    }

    #[tokio::test]
    async fn test_training_cooldown_period() {
        let mut config = TrainingTriggerConfig::default();
        config.min_training_interval_hours = 2;
        
        let (engine, _receiver) = AutonomousTrainingEngine::new(config).unwrap();
        
        // First training
        let poor_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.5,
            confidence: 0.5,
            price_error: 0.2,
            sharpe_ratio: 0.3,
            max_drawdown: 0.2,
            volatility: 0.06,
            model_agreement: 0.6,
            consecutive_failures: 7,
            trading_volume: 1_000_000.0,
            profit_loss: -0.06,
        };
        
        let decision1 = engine.evaluate_training_need(poor_performance.clone()).await.unwrap();
        assert!(!matches!(decision1.decision_type, TrainingDecisionType::NoTraining { .. }));
        
        // Mark as completed
        engine.mark_decision_executed(&decision1.decision_id).await.unwrap();
        engine.mark_training_completed(
            &decision1.decision_id,
            TrainingOutcome::Success {
                improvement_percentage: 10.0,
                new_accuracy: 0.7,
            }
        ).await.unwrap();
        
        // Try again immediately (should be blocked)
        let decision2 = engine.evaluate_training_need(poor_performance).await.unwrap();
        assert!(matches!(decision2.decision_type, TrainingDecisionType::NoTraining { .. }));
        assert!(decision2.reasoning.iter().any(|r| r.contains("Too soon")));
    }

    #[tokio::test]
    async fn test_market_regime_adaptation() {
        let config = TrainingTriggerConfig::default();
        let (engine, _receiver) = AutonomousTrainingEngine::new(config).unwrap();
        
        // Test different market regimes
        let regimes = vec![
            ("high_volatility", 0.1, 0.0),
            ("low_volatility", 0.005, 0.0),
            ("bullish", 0.03, 0.08),
            ("bearish", 0.03, -0.08),
            ("sideways", 0.02, 0.001),
        ];
        
        for (expected_regime, volatility, profit_loss) in regimes {
            let mut performance = PerformanceSnapshot {
                timestamp: Utc::now(),
                accuracy: 0.68, // Slightly below threshold to trigger fine-tuning
                confidence: 0.75,
                price_error: 0.08,
                sharpe_ratio: 0.55,
                max_drawdown: 0.12,
                volatility,
                model_agreement: 0.8,
                consecutive_failures: 2,
                trading_volume: 1_000_000.0,
                profit_loss,
            };
            
            let decision = engine.evaluate_training_need(performance).await.unwrap();
            
            if let TrainingDecisionType::FineTuning { target_regime, .. } = &decision.decision_type {
                assert_eq!(target_regime, expected_regime);
            }
        }
    }

    #[tokio::test]
    async fn test_resource_allocation() {
        let config = TrainingTriggerConfig::default();
        let (engine, _receiver) = AutonomousTrainingEngine::new(config).unwrap();
        
        // Create decisions with different priorities
        let test_cases = vec![
            (0.3, 0.1, 0.4, 15), // Emergency
            (0.5, 0.3, 0.2, 8),  // Full retraining
            (0.65, 0.45, 0.12, 3), // Incremental
            (0.68, 0.55, 0.10, 1), // Fine-tuning
        ];
        
        for (accuracy, sharpe, drawdown, failures) in test_cases {
            let performance = PerformanceSnapshot {
                timestamp: Utc::now(),
                accuracy,
                confidence: accuracy + 0.1,
                price_error: 0.3 - accuracy,
                sharpe_ratio: sharpe,
                max_drawdown: drawdown,
                volatility: 0.03,
                model_agreement: accuracy,
                consecutive_failures: failures,
                trading_volume: 1_000_000.0,
                profit_loss: sharpe - 0.5,
            };
            
            let decision = engine.evaluate_training_need(performance).await.unwrap();
            
            // Higher priority should get more resources
            match decision.priority {
                neural_trader::daa::autonomous_training::TrainingPriority::Emergency => {
                    assert!(decision.resource_requirements.cpu_cores >= 12);
                    assert!(decision.resource_requirements.memory_gb >= 32.0);
                }
                neural_trader::daa::autonomous_training::TrainingPriority::High => {
                    assert!(decision.resource_requirements.cpu_cores >= 8);
                    assert!(decision.resource_requirements.memory_gb >= 16.0);
                }
                neural_trader::daa::autonomous_training::TrainingPriority::Medium => {
                    assert!(decision.resource_requirements.cpu_cores >= 4);
                    assert!(decision.resource_requirements.memory_gb >= 8.0);
                }
                neural_trader::daa::autonomous_training::TrainingPriority::Low => {
                    assert!(decision.resource_requirements.cpu_cores >= 2);
                    assert!(decision.resource_requirements.memory_gb >= 4.0);
                }
                _ => {}
            }
        }
    }

    #[tokio::test]
    async fn test_continuous_monitoring() {
        let config = TrainingTriggerConfig::default();
        let (engine, mut receiver) = AutonomousTrainingEngine::new(config).unwrap();
        let engine_arc = Arc::new(engine);
        
        // Simulate continuous monitoring for 1 second
        let monitoring_duration = tokio::time::Duration::from_secs(1);
        let start = tokio::time::Instant::now();
        
        let monitor_handle = tokio::spawn({
            let engine = Arc::clone(&engine_arc);
            async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));
                let mut performance_count = 0;
                
                while tokio::time::Instant::now() - start < monitoring_duration {
                    interval.tick().await;
                    
                    // Simulate varying performance
                    let performance = PerformanceSnapshot {
                        timestamp: Utc::now(),
                        accuracy: 0.6 + (performance_count as f64 * 0.01),
                        confidence: 0.7,
                        price_error: 0.1,
                        sharpe_ratio: 0.5,
                        max_drawdown: 0.15,
                        volatility: 0.03 + (performance_count as f64 * 0.005),
                        model_agreement: 0.8,
                        consecutive_failures: performance_count % 3,
                        trading_volume: 1_000_000.0,
                        profit_loss: 0.01,
                    };
                    
                    engine.evaluate_training_need(performance).await.unwrap();
                    performance_count += 1;
                }
                
                performance_count
            }
        });
        
        let performance_count = monitor_handle.await.unwrap();
        assert!(performance_count >= 9); // Should have at least 9 evaluations in 1 second
        
        // Check if any decisions were made
        let mut decision_count = 0;
        while receiver.try_recv().is_ok() {
            decision_count += 1;
        }
        
        println!("Continuous monitoring: {} evaluations, {} decisions", performance_count, decision_count);
    }

    #[tokio::test]
    async fn test_training_failure_recovery() {
        let config = TrainingTriggerConfig::default();
        let (engine, _receiver) = AutonomousTrainingEngine::new(config).unwrap();
        
        // Trigger training
        let poor_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.5,
            confidence: 0.5,
            price_error: 0.2,
            sharpe_ratio: 0.3,
            max_drawdown: 0.2,
            volatility: 0.05,
            model_agreement: 0.6,
            consecutive_failures: 7,
            trading_volume: 1_000_000.0,
            profit_loss: -0.05,
        };
        
        let decision = engine.evaluate_training_need(poor_performance).await.unwrap();
        
        // Mark as failed
        engine.mark_decision_executed(&decision.decision_id).await.unwrap();
        engine.mark_training_completed(
            &decision.decision_id,
            TrainingOutcome::Failure {
                error_message: "Insufficient GPU memory".to_string(),
                retry_recommended: true,
            }
        ).await.unwrap();
        
        // Verify failure was recorded
        let history = engine.get_decision_history().await;
        let record = &history[&decision.decision_id];
        
        match &record.outcome {
            Some(TrainingOutcome::Failure { retry_recommended, .. }) => {
                assert!(*retry_recommended);
            }
            _ => panic!("Expected failure outcome"),
        }
    }
}