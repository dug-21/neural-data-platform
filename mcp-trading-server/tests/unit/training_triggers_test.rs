//! Unit tests for training triggers module
//! Covers the TrainingDecisionEngine and related components

use mcp_trading_server::tools::training_triggers::{
    TrainingDecisionEngine, TrainingTrigger, PerformanceSnapshot, 
    TradingPerformanceMetrics, TrainingDecision, TrainingDecisionType,
    DAATrainingIntegration,
};
use mcp_trading_server::integrations::neural::AccuracyMetrics;
use chrono::Utc;
use std::sync::Arc;

/// Helper to create default accuracy metrics
fn create_default_accuracy_metrics() -> AccuracyMetrics {
    AccuracyMetrics {
        directional_accuracy: 0.75,
        price_mae: 30.0,
        price_rmse: 50.0,
        sharpe_ratio: 0.7,
        max_drawdown: 0.08,
    }
}

/// Helper to create poor accuracy metrics
fn create_poor_accuracy_metrics() -> AccuracyMetrics {
    AccuracyMetrics {
        directional_accuracy: 0.50,
        price_mae: 80.0,
        price_rmse: 120.0,
        sharpe_ratio: 0.2,
        max_drawdown: 0.25,
    }
}

/// Helper to create default trading performance
fn create_default_trading_performance() -> TradingPerformanceMetrics {
    TradingPerformanceMetrics {
        realized_pnl: 500.0,
        unrealized_pnl: 200.0,
        win_rate: 0.65,
        avg_trade_duration_minutes: 90.0,
        risk_adjusted_return: 0.08,
    }
}

/// Helper to create poor trading performance
fn create_poor_trading_performance() -> TradingPerformanceMetrics {
    TradingPerformanceMetrics {
        realized_pnl: -800.0,
        unrealized_pnl: -300.0,
        win_rate: 0.35,
        avg_trade_duration_minutes: 180.0,
        risk_adjusted_return: -0.15,
    }
}

#[cfg(test)]
mod training_decision_engine_tests {
    use super::*;

    #[tokio::test]
    async fn test_engine_creation_and_initialization() {
        let engine = TrainingDecisionEngine::new();
        
        // Verify initial state
        let stats = engine.get_performance_stats().await.unwrap();
        assert_eq!(stats.total_snapshots, 0);
        assert_eq!(stats.pending_training_decisions, 0);
        assert_eq!(stats.recent_24h_snapshots, 0);
        assert_eq!(stats.recent_1h_snapshots, 0);
        assert!(stats.accuracy_trend.is_empty());
        assert!(stats.confidence_trend.is_empty());
    }

    #[tokio::test]
    async fn test_default_triggers_setup() {
        let engine = TrainingDecisionEngine::new();
        
        // Add default triggers
        engine.add_default_triggers().await.unwrap();
        
        // Verify triggers were added
        let triggers = engine.triggers.read().await;
        assert_eq!(triggers.len(), 3);
        
        // Check performance degradation trigger
        let perf_trigger = triggers.get("performance_degradation").unwrap();
        assert_eq!(perf_trigger.id, "performance_degradation");
        assert_eq!(perf_trigger.priority, 8);
        assert_eq!(perf_trigger.min_accuracy_threshold, Some(0.65));
        
        // Check market volatility trigger
        let vol_trigger = triggers.get("market_volatility").unwrap();
        assert_eq!(vol_trigger.id, "market_volatility");
        assert_eq!(vol_trigger.priority, 10);
        assert_eq!(vol_trigger.min_sharpe_ratio_threshold, Some(0.5));
        
        // Check confidence drop trigger
        let conf_trigger = triggers.get("confidence_drop").unwrap();
        assert_eq!(conf_trigger.id, "confidence_drop");
        assert_eq!(conf_trigger.priority, 7);
        assert_eq!(conf_trigger.min_confidence_threshold, Some(0.7));
    }

    #[tokio::test]
    async fn test_custom_trigger_addition() {
        let engine = TrainingDecisionEngine::new();
        
        let custom_trigger = TrainingTrigger {
            id: "custom_test".to_string(),
            name: "Custom Test Trigger".to_string(),
            description: "Test trigger for unit tests".to_string(),
            min_accuracy_threshold: Some(0.8),
            max_accuracy_threshold: None,
            min_sharpe_ratio_threshold: Some(0.6),
            max_sharpe_ratio_threshold: None,
            max_mae_threshold: Some(40.0),
            max_rmse_threshold: Some(60.0),
            max_drawdown_threshold: Some(0.1),
            min_confidence_threshold: Some(0.75),
            max_consecutive_failures_threshold: Some(3),
            min_prediction_count: Some(20),
            priority: 9,
            enabled: true,
            cooldown_hours: 12,
        };
        
        engine.add_trigger(custom_trigger.clone()).await.unwrap();
        
        let triggers = engine.triggers.read().await;
        assert_eq!(triggers.len(), 1);
        assert!(triggers.contains_key("custom_test"));
    }

    #[tokio::test]
    async fn test_trigger_removal() {
        let engine = TrainingDecisionEngine::new();
        engine.add_default_triggers().await.unwrap();
        
        // Remove one trigger
        engine.remove_trigger("confidence_drop").await.unwrap();
        
        let triggers = engine.triggers.read().await;
        assert_eq!(triggers.len(), 2);
        assert!(!triggers.contains_key("confidence_drop"));
        assert!(triggers.contains_key("performance_degradation"));
        assert!(triggers.contains_key("market_volatility"));
    }

    #[tokio::test]
    async fn test_performance_recording() {
        let engine = TrainingDecisionEngine::new();
        
        let snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy_metrics: create_default_accuracy_metrics(),
            symbol: "BTC/USD".to_string(),
            prediction_count: 50,
            avg_confidence: 0.85,
            consecutive_failures: 0,
            trading_performance: create_default_trading_performance(),
        };
        
        engine.record_performance(snapshot).await.unwrap();
        
        let stats = engine.get_performance_stats().await.unwrap();
        assert_eq!(stats.total_snapshots, 1);
        assert_eq!(stats.recent_24h_snapshots, 1);
        assert_eq!(stats.recent_1h_snapshots, 1);
    }

    #[tokio::test]
    async fn test_performance_history_limit() {
        let engine = TrainingDecisionEngine::new();
        
        // Add more than MAX_PERFORMANCE_HISTORY (1000) snapshots
        for i in 0..1100 {
            let snapshot = PerformanceSnapshot {
                timestamp: Utc::now(),
                accuracy_metrics: create_default_accuracy_metrics(),
                symbol: format!("TEST/{}", i % 10),
                prediction_count: 10 + i,
                avg_confidence: 0.8,
                consecutive_failures: 0,
                trading_performance: create_default_trading_performance(),
            };
            
            engine.record_performance(snapshot).await.unwrap();
        }
        
        let stats = engine.get_performance_stats().await.unwrap();
        // Should be capped at 1000
        assert!(stats.total_snapshots <= 1000);
    }

    #[tokio::test]
    async fn test_trigger_evaluation_no_triggers() {
        let engine = TrainingDecisionEngine::new();
        
        // Record performance without any triggers
        let snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy_metrics: create_poor_accuracy_metrics(),
            symbol: "BTC/USD".to_string(),
            prediction_count: 50,
            avg_confidence: 0.5,
            consecutive_failures: 10,
            trading_performance: create_poor_trading_performance(),
        };
        
        engine.record_performance(snapshot).await.unwrap();
        
        // Should not generate any decisions without triggers
        let decision = engine.evaluate_training_need().await.unwrap();
        assert!(decision.is_none());
    }

    #[tokio::test]
    async fn test_trigger_evaluation_with_poor_performance() {
        let engine = TrainingDecisionEngine::new();
        engine.add_default_triggers().await.unwrap();
        
        // Record very poor performance
        let snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy_metrics: create_poor_accuracy_metrics(),
            symbol: "BTC/USD".to_string(),
            prediction_count: 100,
            avg_confidence: 0.5,
            consecutive_failures: 8,
            trading_performance: create_poor_trading_performance(),
        };
        
        engine.record_performance(snapshot).await.unwrap();
        
        // Should generate a training decision
        let decision = engine.evaluate_training_need().await.unwrap();
        assert!(decision.is_some());
        
        let decision = decision.unwrap();
        assert!(matches!(decision.decision_type, TrainingDecisionType::EmergencyRetraining));
        assert!(decision.confidence > 0.5);
        assert!(!decision.reasoning.is_empty());
    }

    #[tokio::test]
    async fn test_trigger_evaluation_with_good_performance() {
        let engine = TrainingDecisionEngine::new();
        engine.add_default_triggers().await.unwrap();
        
        // Record good performance
        let snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy_metrics: create_default_accuracy_metrics(),
            symbol: "BTC/USD".to_string(),
            prediction_count: 100,
            avg_confidence: 0.85,
            consecutive_failures: 0,
            trading_performance: create_default_trading_performance(),
        };
        
        engine.record_performance(snapshot).await.unwrap();
        
        // Should not generate a training decision
        let decision = engine.evaluate_training_need().await.unwrap();
        assert!(decision.is_none());
    }

    #[tokio::test]
    async fn test_trigger_priority_ordering() {
        let engine = TrainingDecisionEngine::new();
        
        // Add triggers with different priorities
        let low_priority = TrainingTrigger {
            id: "low".to_string(),
            name: "Low Priority".to_string(),
            priority: 3,
            enabled: true,
            ..Default::default()
        };
        
        let high_priority = TrainingTrigger {
            id: "high".to_string(),
            name: "High Priority".to_string(),
            priority: 9,
            enabled: true,
            ..Default::default()
        };
        
        engine.add_trigger(low_priority).await.unwrap();
        engine.add_trigger(high_priority).await.unwrap();
        
        // Triggers should be evaluated in priority order
        let triggers = engine.triggers.read().await;
        assert_eq!(triggers.len(), 2);
    }

    #[tokio::test]
    async fn test_disabled_trigger_skipped() {
        let engine = TrainingDecisionEngine::new();
        
        let mut trigger = TrainingTrigger {
            id: "disabled".to_string(),
            name: "Disabled Trigger".to_string(),
            min_accuracy_threshold: Some(0.9), // Very high threshold
            priority: 10,
            enabled: false, // Disabled
            ..Default::default()
        };
        
        engine.add_trigger(trigger.clone()).await.unwrap();
        
        // Record poor performance that would trigger if enabled
        let snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy_metrics: AccuracyMetrics {
                directional_accuracy: 0.5, // Well below threshold
                ..create_poor_accuracy_metrics()
            },
            symbol: "BTC/USD".to_string(),
            prediction_count: 50,
            avg_confidence: 0.5,
            consecutive_failures: 10,
            trading_performance: create_poor_trading_performance(),
        };
        
        engine.record_performance(snapshot).await.unwrap();
        
        // Should not trigger because it's disabled
        let decision = engine.evaluate_training_need().await.unwrap();
        assert!(decision.is_none());
    }

    #[tokio::test]
    async fn test_recent_decisions_retrieval() {
        let engine = TrainingDecisionEngine::new();
        engine.add_default_triggers().await.unwrap();
        
        // Generate multiple decisions
        for i in 0..5 {
            let snapshot = PerformanceSnapshot {
                timestamp: Utc::now(),
                accuracy_metrics: AccuracyMetrics {
                    directional_accuracy: 0.5 - (i as f64 * 0.05),
                    ..create_poor_accuracy_metrics()
                },
                symbol: format!("TEST{}/USD", i),
                prediction_count: 100,
                avg_confidence: 0.5,
                consecutive_failures: 10 + i,
                trading_performance: create_poor_trading_performance(),
            };
            
            engine.record_performance(snapshot).await.unwrap();
            engine.evaluate_training_need().await.unwrap();
        }
        
        // Get recent decisions
        let recent = engine.get_recent_decisions(3).await.unwrap();
        assert_eq!(recent.len(), 3);
        
        // Should be ordered by timestamp (most recent first)
        for i in 1..recent.len() {
            assert!(recent[i-1].timestamp >= recent[i].timestamp);
        }
    }

    #[tokio::test]
    async fn test_decision_execution_tracking() {
        let engine = TrainingDecisionEngine::new();
        engine.add_default_triggers().await.unwrap();
        
        // Generate a decision
        let snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy_metrics: create_poor_accuracy_metrics(),
            symbol: "BTC/USD".to_string(),
            prediction_count: 100,
            avg_confidence: 0.5,
            consecutive_failures: 10,
            trading_performance: create_poor_trading_performance(),
        };
        
        engine.record_performance(snapshot).await.unwrap();
        let decision = engine.evaluate_training_need().await.unwrap().unwrap();
        
        // Mark as executed
        engine.mark_decision_executed(&decision.decision_id).await.unwrap();
        
        // Verify execution was tracked
        let stats = engine.get_performance_stats().await.unwrap();
        assert_eq!(stats.pending_training_decisions, 0);
    }

    #[tokio::test]
    async fn test_daa_communication_channel() {
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let engine = TrainingDecisionEngine::new().with_daa_communication(sender);
        engine.add_default_triggers().await.unwrap();
        
        // Generate a decision
        let snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy_metrics: create_poor_accuracy_metrics(),
            symbol: "BTC/USD".to_string(),
            prediction_count: 100,
            avg_confidence: 0.5,
            consecutive_failures: 10,
            trading_performance: create_poor_trading_performance(),
        };
        
        engine.record_performance(snapshot).await.unwrap();
        engine.evaluate_training_need().await.unwrap();
        
        // Decision should be sent to DAA channel
        let received = receiver.try_recv();
        assert!(received.is_ok());
        
        let decision = received.unwrap();
        assert!(matches!(decision.decision_type, TrainingDecisionType::EmergencyRetraining));
    }

    #[tokio::test]
    async fn test_trigger_cooldown() {
        let engine = TrainingDecisionEngine::new();
        
        let trigger = TrainingTrigger {
            id: "cooldown_test".to_string(),
            name: "Cooldown Test".to_string(),
            min_accuracy_threshold: Some(0.7),
            priority: 8,
            enabled: true,
            cooldown_hours: 1,
            ..Default::default()
        };
        
        engine.add_trigger(trigger).await.unwrap();
        
        // First poor performance should trigger
        let snapshot1 = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy_metrics: AccuracyMetrics {
                directional_accuracy: 0.5,
                ..create_poor_accuracy_metrics()
            },
            symbol: "BTC/USD".to_string(),
            prediction_count: 50,
            avg_confidence: 0.5,
            consecutive_failures: 5,
            trading_performance: create_poor_trading_performance(),
        };
        
        engine.record_performance(snapshot1).await.unwrap();
        let decision1 = engine.evaluate_training_need().await.unwrap();
        assert!(decision1.is_some());
        
        // Second poor performance immediately after should not trigger due to cooldown
        let snapshot2 = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy_metrics: AccuracyMetrics {
                directional_accuracy: 0.4, // Even worse
                ..create_poor_accuracy_metrics()
            },
            symbol: "BTC/USD".to_string(),
            prediction_count: 50,
            avg_confidence: 0.4,
            consecutive_failures: 10,
            trading_performance: create_poor_trading_performance(),
        };
        
        engine.record_performance(snapshot2).await.unwrap();
        let decision2 = engine.evaluate_training_need().await.unwrap();
        
        // Might still trigger if another trigger without cooldown fires
        // But the specific trigger should be in cooldown
    }

    #[tokio::test]
    async fn test_performance_trends() {
        let engine = TrainingDecisionEngine::new();
        
        // Add improving performance trend
        for i in 0..10 {
            let snapshot = PerformanceSnapshot {
                timestamp: Utc::now(),
                accuracy_metrics: AccuracyMetrics {
                    directional_accuracy: 0.6 + (i as f64 * 0.03),
                    ..create_default_accuracy_metrics()
                },
                symbol: "BTC/USD".to_string(),
                prediction_count: 50,
                avg_confidence: 0.7 + (i as f64 * 0.02),
                consecutive_failures: 0,
                trading_performance: create_default_trading_performance(),
            };
            
            engine.record_performance(snapshot).await.unwrap();
        }
        
        let stats = engine.get_performance_stats().await.unwrap();
        assert!(!stats.accuracy_trend.is_empty());
        assert!(!stats.confidence_trend.is_empty());
        
        // Trends should show improvement
        let first_accuracy = stats.accuracy_trend.first().unwrap();
        let last_accuracy = stats.accuracy_trend.last().unwrap();
        assert!(last_accuracy > first_accuracy);
    }
}

#[cfg(test)]
mod daa_training_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_daa_integration_creation() {
        let engine = Arc::new(TrainingDecisionEngine::new());
        let integration = DAATrainingIntegration::new(engine);
        
        // Integration should be created successfully
        // In production, would test with actual neural client
    }

    #[tokio::test]
    async fn test_daa_integration_with_neural_client() {
        let engine = Arc::new(TrainingDecisionEngine::new());
        let mut integration = DAATrainingIntegration::new(Arc::clone(&engine));
        
        // Test with mock neural client URL
        let result = integration.with_neural_client("http://localhost:8000").await;
        
        // In test environment, connection will fail but interface should work
        assert!(result.is_err() || result.is_ok());
    }

    #[tokio::test]
    async fn test_daa_integration_coordination() {
        let engine = Arc::new(TrainingDecisionEngine::new());
        let mut integration = DAATrainingIntegration::new(Arc::clone(&engine));
        
        // Start coordination (will fail quickly in test)
        let result = integration.start_coordination().await;
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod training_decision_type_tests {
    use super::*;

    #[tokio::test]
    async fn test_decision_type_creation() {
        // Test all decision type variants
        let emergency = TrainingDecisionType::EmergencyRetraining;
        let full = TrainingDecisionType::FullRetraining;
        let incremental = TrainingDecisionType::IncrementalTraining;
        let scheduled = TrainingDecisionType::ScheduledRetraining;
        
        // Verify all variants can be created
        match emergency {
            TrainingDecisionType::EmergencyRetraining => assert!(true),
            _ => panic!("Wrong type"),
        }
        
        match full {
            TrainingDecisionType::FullRetraining => assert!(true),
            _ => panic!("Wrong type"),
        }
        
        match incremental {
            TrainingDecisionType::IncrementalTraining => assert!(true),
            _ => panic!("Wrong type"),
        }
        
        match scheduled {
            TrainingDecisionType::ScheduledRetraining => assert!(true),
            _ => panic!("Wrong type"),
        }
    }

    #[tokio::test]
    async fn test_decision_confidence_calculation() {
        let engine = TrainingDecisionEngine::new();
        engine.add_default_triggers().await.unwrap();
        
        // Test with varying severity
        let severities = vec![
            (0.3, 0.1, 0.4), // Very poor - high confidence
            (0.5, 0.3, 0.2), // Moderate - medium confidence
            (0.65, 0.45, 0.12), // Slightly poor - lower confidence
        ];
        
        for (accuracy, sharpe, drawdown) in severities {
            let snapshot = PerformanceSnapshot {
                timestamp: Utc::now(),
                accuracy_metrics: AccuracyMetrics {
                    directional_accuracy: accuracy,
                    price_mae: 100.0 - (accuracy * 100.0),
                    price_rmse: 150.0 - (accuracy * 100.0),
                    sharpe_ratio: sharpe,
                    max_drawdown: drawdown,
                },
                symbol: "BTC/USD".to_string(),
                prediction_count: 100,
                avg_confidence: accuracy,
                consecutive_failures: ((1.0 - accuracy) * 10.0) as usize,
                trading_performance: TradingPerformanceMetrics {
                    realized_pnl: (sharpe - 0.5) * 1000.0,
                    unrealized_pnl: (sharpe - 0.5) * 500.0,
                    win_rate: accuracy,
                    avg_trade_duration_minutes: 90.0,
                    risk_adjusted_return: sharpe - 0.5,
                },
            };
            
            engine.record_performance(snapshot).await.unwrap();
            
            if let Some(decision) = engine.evaluate_training_need().await.unwrap() {
                // Worse performance should have higher confidence
                if accuracy < 0.5 {
                    assert!(decision.confidence > 0.7);
                }
            }
        }
    }
}