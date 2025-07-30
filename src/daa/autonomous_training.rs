//! Autonomous Neural Training Recognition System
//!
//! This module extends the DAA coordinator with autonomous capabilities to recognize
//! appropriate times for neural training and initiate training processes automatically.
//!
//! ## Modular Architecture
//!
//! The autonomous training system is organized into specialized modules:
//!
//! - **config**: Configuration structures, decision types, and resource requirements
//! - **metrics**: Performance tracking, trend analysis, and decision recording
//! - **triggers**: Training trigger evaluation and decision-making logic
//! - **scheduler**: Training scheduling, checkpoint management, and model persistence
//! - **engine**: Main training execution engine and DAA coordinator integration
//!
//! ## Usage
//!
//! ```rust,no_run
//! use crate::daa::autonomous_training::{
//!     AutonomousTrainingEngine, DAATrainingIntegration, 
//!     TrainingTriggerConfig, PerformanceSnapshot
//! };
//!
//! // Create autonomous training engine
//! let config = TrainingTriggerConfig::default();
//! let (engine, receiver) = AutonomousTrainingEngine::new(config)?;
//!
//! // Create DAA integration
//! let integration = DAATrainingIntegration::new(engine.into(), receiver)
//!     .with_fann_predictor(fann_predictor)
//!     .with_training_data_service(training_service);
//!
//! // Evaluate training needs
//! let performance = PerformanceSnapshot { /* ... */ };
//! let decision = engine.evaluate_training_need(performance).await?;
//! ```

// Re-export the modular autonomous training system
pub use autonomous_training::{
    AutonomousTrainingEngine, DAATrainingIntegration, ModelInfo, PerformanceSnapshot,
    ResourceRequirements, TrainingDecision, TrainingDecisionRecord, TrainingDecisionType,
    TrainingOutcome, TrainingPriority, TrainingScheduler, TrainingTriggerConfig,
    TrainingTriggerEvaluator,
};

/// Modular autonomous training system
mod autonomous_training;

// Legacy compatibility - keep existing tests working
#[cfg(test)]
mod legacy_tests {
    use super::*;
    use chrono::Utc;
    use std::sync::atomic::Ordering;
    use tokio;

    #[tokio::test]
    async fn test_autonomous_training_engine_creation() {
        let config = TrainingTriggerConfig::default();
        let (engine, _receiver) = AutonomousTrainingEngine::new(config).unwrap();

        // Test that engine was created successfully
        let history = engine.get_decision_history().await;
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_training_decision_logic() {
        let config = TrainingTriggerConfig::default();
        let (engine, _receiver) = AutonomousTrainingEngine::new(config).unwrap();

        // Test performance that should trigger training
        let poor_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.6, // Below 0.7 threshold
            confidence: 0.5,
            price_error: 0.15,
            sharpe_ratio: 0.3, // Below 0.5 threshold
            max_drawdown: 0.2, // Above 0.15 threshold
            volatility: 0.03,
            model_agreement: 0.6,
            consecutive_failures: 6, // Above 5 threshold
            trading_volume: 1000000.0,
            profit_loss: -0.05,
        };

        let decision = engine
            .evaluate_training_need(poor_performance)
            .await
            .unwrap();

        match decision.decision_type {
            TrainingDecisionType::FullRetraining { .. }
            | TrainingDecisionType::Emergency { .. } => {
                assert!(decision.confidence > 0.8);
                assert!(!decision.reasoning.is_empty());
            }
            _ => panic!("Expected training to be triggered for poor performance"),
        }
    }

    #[tokio::test]
    async fn test_no_training_decision() {
        let config = TrainingTriggerConfig::default();
        let (engine, _receiver) = AutonomousTrainingEngine::new(config).unwrap();

        // Test good performance that should not trigger training
        let good_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.85, // Above threshold
            confidence: 0.9,
            price_error: 0.05,
            sharpe_ratio: 0.8,  // Above threshold
            max_drawdown: 0.08, // Below threshold
            volatility: 0.02,
            model_agreement: 0.9,
            consecutive_failures: 1, // Below threshold
            trading_volume: 1000000.0,
            profit_loss: 0.03,
        };

        let decision = engine
            .evaluate_training_need(good_performance)
            .await
            .unwrap();

        match decision.decision_type {
            TrainingDecisionType::NoTraining { .. } => {
                assert!(decision
                    .reasoning
                    .iter()
                    .any(|r| r.contains("within acceptable ranges")));
            }
            _ => panic!("Expected no training for good performance"),
        }
    }

    #[tokio::test]
    async fn test_emergency_training_conditions() {
        let config = TrainingTriggerConfig::default();
        let evaluator = TrainingTriggerEvaluator::new(config);

        // Set up multiple failures to trigger emergency
        for _ in 0..12 {
            evaluator
                .consecutive_failure_count
                .fetch_add(1, Ordering::Relaxed);
        }

        let critical_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.3, // Far below threshold
            confidence: 0.2,
            price_error: 0.25,
            sharpe_ratio: -0.5,
            max_drawdown: 0.4, // Very high
            volatility: 0.08,
            model_agreement: 0.3,
            consecutive_failures: 12,
            trading_volume: 1000000.0,
            profit_loss: -0.15,
        };

        let decision = evaluator
            .evaluate_training_need(critical_performance)
            .await
            .unwrap();

        match decision.decision_type {
            TrainingDecisionType::Emergency { urgency_score, .. } => {
                assert_eq!(urgency_score, 1.0);
                assert_eq!(decision.priority, TrainingPriority::Emergency);
            }
            _ => panic!("Expected emergency training for critical performance"),
        }
    }
}