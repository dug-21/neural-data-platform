//! Autonomous Neural Training Recognition System
//!
//! This module extends the DAA coordinator with autonomous capabilities to recognize
//! appropriate times for neural training and initiate training processes automatically.
//!
//! The module is organized into the following components:
//! - `config`: Configuration structures and training decision types
//! - `metrics`: Performance tracking and trend analysis
//! - `triggers`: Training trigger evaluation logic
//! - `scheduler`: Training scheduling and model persistence
//! - `engine`: Main training execution engine and DAA integration

pub mod config;
pub mod engine;
pub mod metrics;
pub mod scheduler;
pub mod triggers;

// Re-export commonly used types for convenience
pub use config::{
    ResourceRequirements, TrainingDecisionType, TrainingOutcome, TrainingPriority,
    TrainingTriggerConfig,
};
pub use engine::{AutonomousTrainingEngine, DAATrainingIntegration};
pub use metrics::{ModelInfo, PerformanceSnapshot, TrainingDecision, TrainingDecisionRecord};
pub use scheduler::TrainingScheduler;
pub use triggers::TrainingTriggerEvaluator;

#[cfg(test)]
mod integration_tests {
    use super::*;
    use chrono::Utc;
    use std::sync::Arc;
    use tokio;

    #[tokio::test]
    async fn test_full_autonomous_training_workflow() {
        // Create training configuration
        let config = TrainingTriggerConfig::default();
        
        // Create autonomous training engine
        let (engine, receiver) = AutonomousTrainingEngine::new(config).unwrap();
        let engine_arc = Arc::new(engine);
        
        // Create DAA integration
        let integration = DAATrainingIntegration::new(engine_arc.clone(), receiver);
        
        // Test performance evaluation
        let test_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.6, // Below threshold to trigger training
            confidence: 0.7,
            price_error: 0.12,
            sharpe_ratio: 0.4, // Below threshold
            max_drawdown: 0.18, // Above threshold
            volatility: 0.04,
            model_agreement: 0.8,
            consecutive_failures: 3,
            trading_volume: 1500000.0,
            profit_loss: -0.03,
        };
        
        // Evaluate training need
        let decision = engine_arc.evaluate_training_need(test_performance).await;
        assert!(decision.is_ok());
        
        let decision = decision.unwrap();
        
        // Should trigger some form of training due to poor performance
        match decision.decision_type {
            TrainingDecisionType::NoTraining { .. } => {
                panic!("Expected training to be triggered for poor performance");
            }
            _ => {
                // Training was appropriately triggered
                assert!(!decision.reasoning.is_empty());
                assert!(decision.confidence > 0.0);
            }
        }
        
        // Test decision memory
        let history = engine_arc.get_decision_history().await;
        assert!(history.contains_key(&decision.decision_id));
        
        // Test completion tracking
        let completion_result = engine_arc.mark_training_completed(
            &decision.decision_id,
            TrainingOutcome::Success {
                improvement_percentage: 15.0,
                new_accuracy: 0.82,
            },
        ).await;
        assert!(completion_result.is_ok());
        
        // Verify completion was recorded
        let updated_history = engine_arc.get_decision_history().await;
        let record = updated_history.get(&decision.decision_id).unwrap();
        assert!(record.execution_completed.is_some());
        assert!(matches!(record.outcome, Some(TrainingOutcome::Success { .. })));
        assert_eq!(record.performance_improvement, Some(15.0));
    }

    #[tokio::test]
    async fn test_emergency_training_detection() {
        let config = TrainingTriggerConfig::default();
        let (engine, _receiver) = AutonomousTrainingEngine::new(config).unwrap();
        
        // Create performance snapshot that should trigger emergency training
        let critical_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.25, // Severely below threshold (0.7 * 0.5 = 0.35)
            confidence: 0.15,
            price_error: 0.30,
            sharpe_ratio: -0.8,
            max_drawdown: 0.45, // Very high (0.15 * 1.5 = 0.225)
            volatility: 0.12,
            model_agreement: 0.2,
            consecutive_failures: 15, // Well above threshold
            trading_volume: 2000000.0,
            profit_loss: -0.25,
        };
        
        let decision = engine.evaluate_training_need(critical_performance).await.unwrap();
        
        match decision.decision_type {
            TrainingDecisionType::Emergency { urgency_score, .. } => {
                assert_eq!(urgency_score, 1.0);
                assert_eq!(decision.priority, TrainingPriority::Emergency);
                assert!(decision.reasoning.iter().any(|r| r.contains("Emergency")));
            }
            _ => panic!("Expected emergency training for critical performance degradation"),
        }
    }

    #[tokio::test]
    async fn test_scheduler_integration() {
        // Test that scheduler methods work correctly
        let load_result = TrainingScheduler::load_best_models_on_startup().await;
        assert!(load_result.is_ok());
        
        let requirements = ResourceRequirements::incremental();
        let availability_check = TrainingScheduler::check_resource_availability(&requirements).await;
        assert!(availability_check.is_ok());
        
        let scheduled_time = TrainingScheduler::schedule_training_task(
            TrainingPriority::Medium,
            chrono::Duration::hours(1),
            &requirements,
        ).await;
        assert!(scheduled_time.is_ok());
        assert!(scheduled_time.unwrap() >= Utc::now());
    }

    #[tokio::test]
    async fn test_metrics_analysis() {
        // Test metric trend analysis
        let improving_values = vec![0.60, 0.65, 0.70, 0.75, 0.80];
        let trend = super::metrics::MetricsAnalyzer::analyze_metric_trend(&improving_values);
        matches!(trend, super::metrics::PerformanceTrend::Improving);
        
        let degrading_values = vec![0.80, 0.75, 0.70, 0.65, 0.60];
        let trend = super::metrics::MetricsAnalyzer::analyze_metric_trend(&degrading_values);
        matches!(trend, super::metrics::PerformanceTrend::Degrading);
        
        // Test market regime detection
        let performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.75,
            confidence: 0.8,
            price_error: 0.08,
            sharpe_ratio: 0.6,
            max_drawdown: 0.12,
            volatility: 0.06, // High volatility
            model_agreement: 0.85,
            consecutive_failures: 2,
            trading_volume: 1200000.0,
            profit_loss: 0.02,
        };
        
        let regime = super::metrics::MetricsAnalyzer::detect_market_regime(&performance);
        assert_eq!(regime, "high_volatility");
    }
}