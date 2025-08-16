//! Compatibility Tests for Enhanced Performance Snapshot
//!
//! These tests verify that the DAA extension maintains full backward compatibility
//! and that performance thresholds still trigger retraining as expected.

#[cfg(test)]
mod compatibility_tests {
    use super::super::{
        autonomous_training::{AutonomousTrainingEngine, PerformanceSnapshot, TrainingTriggerConfig, TrainingDecisionType},
        compatibility_adapter::EnhancedTrainingEngineAdapter,
        enhanced_performance_snapshot::{EnhancedPerformanceSnapshot, DataTypeMetrics, DataQualityIssue, QualityIssueType},
    };
    use chrono::Utc;
    use tokio;

    fn create_test_base_snapshot(accuracy: f64, error_rate: f64) -> PerformanceSnapshot {
        PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy,
            latency_ms: 100,
            error_rate,
            recent_predictions: 50,
            confidence: 0.8,
            price_error: 0.05,
            sharpe_ratio: 1.2,
            max_drawdown: 0.05,
            volatility: 0.1,
            model_agreement: 0.9,
            consecutive_failures: 0,
            trading_volume: 1000000.0,
            profit_loss: 50.0,
            data_type_metrics: None,
            event_count: 100,
            window_duration: chrono::Duration::minutes(60),
            symbol: "TEST".to_string(),
            trading_performance: None,
            accuracy_metrics: None,
            cpu_usage: 25.0,
            memory_usage: 128.0,
            active_connections: 5,
            requests_per_second: 10.0,
            average_response_time: 50.0,
            cache_hit_rate: 0.85,
        }
    }

    #[tokio::test]
    async fn test_original_training_thresholds_unchanged() {
        // Create original engine with default thresholds
        let config = TrainingTriggerConfig::default();
        let engine = AutonomousTrainingEngine::new(config).unwrap();
        
        // Test that low accuracy still triggers retraining (original behavior)
        let poor_performance = create_test_base_snapshot(0.6, 0.2); // Below 0.8 threshold
        let decision = engine.evaluate_training_need(poor_performance).await.unwrap();
        
        match decision.decision_type {
            TrainingDecisionType::FullRetraining { .. } => {
                // This is expected - original behavior preserved
                assert!(decision.reasons.iter().any(|r| r.contains("Accuracy below threshold")));
            }
            _ => panic!("Expected FullRetraining for low accuracy, got {:?}", decision.decision_type),
        }
    }

    #[tokio::test]
    async fn test_enhanced_adapter_preserves_base_decisions() {
        // Create adapter from original engine
        let config = TrainingTriggerConfig::default();
        let base_engine = AutonomousTrainingEngine::new(config).unwrap();
        let adapter = EnhancedTrainingEngineAdapter::new(base_engine);
        
        // Test same performance snapshot through both paths
        let test_snapshot = create_test_base_snapshot(0.6, 0.2);
        let enhanced_snapshot = EnhancedPerformanceSnapshot::from_base_snapshot(test_snapshot.clone());
        
        // Get decisions from both approaches
        let base_decision = adapter.evaluate_training_need(test_snapshot).await.unwrap();
        let enhanced_decision = adapter.evaluate_training_need_enhanced(&enhanced_snapshot).await.unwrap();
        
        // Core decision should be the same
        assert_eq!(
            std::mem::discriminant(&base_decision.decision_type),
            std::mem::discriminant(&enhanced_decision.decision_type)
        );
        
        // Enhanced version may have additional reasoning, but base reasoning should be present
        assert!(enhanced_decision.reasoning.iter().any(|r| 
            base_decision.reasoning.iter().any(|br| r.contains(br))
        ));
    }

    #[tokio::test]
    async fn test_performance_thresholds_still_active() {
        let config = TrainingTriggerConfig {
            accuracy_threshold: 0.75,
            error_rate_threshold: 0.15,
            min_predictions_for_evaluation: 10,
        };
        let base_engine = AutonomousTrainingEngine::new(config).unwrap();
        let adapter = EnhancedTrainingEngineAdapter::new(base_engine);

        // Test accuracy threshold
        let low_accuracy = create_test_base_snapshot(0.7, 0.1); // Below 0.75 threshold
        let enhanced_snapshot = EnhancedPerformanceSnapshot::from_base_snapshot(low_accuracy);
        let decision = adapter.evaluate_training_need_enhanced(&enhanced_snapshot).await.unwrap();
        
        match decision.decision_type {
            TrainingDecisionType::FullRetraining { .. } => {
                assert!(decision.reasons.iter().any(|r| r.contains("threshold")));
            }
            _ => panic!("Expected training trigger for low accuracy"),
        }

        // Test error rate threshold  
        let high_error = create_test_base_snapshot(0.9, 0.2); // Above 0.15 threshold
        let enhanced_snapshot = EnhancedPerformanceSnapshot::from_base_snapshot(high_error);
        let decision = adapter.evaluate_training_need_enhanced(&enhanced_snapshot).await.unwrap();
        
        match decision.decision_type {
            TrainingDecisionType::IncrementalTraining => {
                // This is expected for high error rate
            }
            _ => panic!("Expected incremental training for high error rate"),
        }
    }

    #[tokio::test]
    async fn test_enhanced_features_add_value_without_breaking() {
        let config = TrainingTriggerConfig::default();
        let base_engine = AutonomousTrainingEngine::new(config).unwrap();
        let adapter = EnhancedTrainingEngineAdapter::new(base_engine);
        
        // Create enhanced snapshot with quality issues
        let base_snapshot = create_test_base_snapshot(0.85, 0.05); // Good performance
        let mut enhanced_snapshot = EnhancedPerformanceSnapshot::from_base_snapshot(base_snapshot);
        
        // Add critical quality issue
        enhanced_snapshot.add_quality_issue(DataQualityIssue {
            issue_type: QualityIssueType::MissingData,
            affected_field: "price_feed".to_string(),
            severity: 0.9,
            description: "Critical data missing".to_string(),
            remediation: Some("Check connectivity".to_string()),
            detected_at: Utc::now(),
        });
        
        let decision = adapter.evaluate_training_need_enhanced(&enhanced_snapshot).await.unwrap();
        
        // Should still make base decision (no training for good performance)
        match decision.decision_type {
            TrainingDecisionType::NoTraining { .. } => {
                // But should include enhanced reasoning about data quality
                assert!(decision.reasoning.iter().any(|r| r.contains("quality")));
            }
            _ => {
                // Or enhanced features might suggest training due to quality issues
                assert!(decision.reasoning.iter().any(|r| r.contains("quality")));
            }
        }
    }

    #[tokio::test]
    async fn test_serialization_backward_compatibility() {
        // Create enhanced snapshot
        let base = create_test_base_snapshot(0.85, 0.1);
        let enhanced = EnhancedPerformanceSnapshot::from_base_snapshot(base.clone());
        
        // Serialize enhanced snapshot
        let json = serde_json::to_string(&enhanced).unwrap();
        
        // Should be able to deserialize back
        let deserialized: EnhancedPerformanceSnapshot = serde_json::from_str(&json).unwrap();
        
        // Base data should be preserved
        assert_eq!(deserialized.base_snapshot.accuracy, base.accuracy);
        assert_eq!(deserialized.base_snapshot.error_rate, base.error_rate);
        
        // Should be able to convert back to base
        let recovered_base: PerformanceSnapshot = deserialized.into();
        assert_eq!(recovered_base.accuracy, base.accuracy);
    }

    #[tokio::test]
    async fn test_mixed_snapshot_processing() {
        let config = TrainingTriggerConfig::default();
        let base_engine = AutonomousTrainingEngine::new(config).unwrap();
        let adapter = EnhancedTrainingEngineAdapter::new(base_engine);
        
        let base1 = create_test_base_snapshot(0.6, 0.3); // Poor performance
        let base2 = create_test_base_snapshot(0.9, 0.05); // Good performance
        let enhanced = EnhancedPerformanceSnapshot::from_base_snapshot(base1.clone());
        
        let snapshots = vec![
            crate::daa::compatibility_adapter::SnapshotType::Base(base1),
            crate::daa::compatibility_adapter::SnapshotType::Enhanced(enhanced),
            crate::daa::compatibility_adapter::SnapshotType::Base(base2),
        ];
        
        let decisions = adapter.process_mixed_snapshots(snapshots).await.unwrap();
        assert_eq!(decisions.len(), 3);
        
        // First two should suggest training (poor performance)
        match &decisions[0].decision_type {
            TrainingDecisionType::FullRetraining { .. } => {}
            _ => panic!("Expected training for poor performance"),
        }
        
        // Last should not need training (good performance)
        match &decisions[2].decision_type {
            TrainingDecisionType::NoTraining { .. } => {}
            _ => panic!("Expected no training for good performance"),
        }
    }

    #[tokio::test]
    async fn test_legacy_mode_exact_compatibility() {
        let config = TrainingTriggerConfig::default();
        let base_engine = AutonomousTrainingEngine::new(config.clone()).unwrap();
        let legacy_adapter = EnhancedTrainingEngineAdapter::new_legacy_mode(base_engine.clone());
        
        let test_snapshot = create_test_base_snapshot(0.7, 0.2);
        
        // Get decision from original engine
        let original_decision = base_engine.evaluate_training_need(test_snapshot.clone()).await.unwrap();
        
        // Get decision from legacy mode adapter
        let adapter_decision = legacy_adapter.evaluate_training_need(test_snapshot).await.unwrap();
        
        // Should be identical
        assert_eq!(
            std::mem::discriminant(&original_decision.decision_type),
            std::mem::discriminant(&adapter_decision.decision_type)
        );
        assert_eq!(original_decision.confidence, adapter_decision.confidence);
        assert_eq!(original_decision.reasoning, adapter_decision.reasoning);
    }
}