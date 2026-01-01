use std::collections::HashMap;
use neural_trader::{
    daa::{PerformanceSnapshot, DaaError},
    neural::PredictionMetrics,
};

#[tokio::test]
async fn test_existing_fields_preserved() {
    // CRITICAL: Ensure existing decision logic preserved
    let mut snapshot = PerformanceSnapshot::new();
    
    // Verify all existing fields are preserved exactly
    assert_eq!(snapshot.accuracy_threshold, 0.8);
    assert_eq!(snapshot.error_threshold, 0.1);
    assert_eq!(snapshot.consecutive_failure_threshold, 5);
    assert_eq!(snapshot.voting_ratio, (60, 40));
    assert_eq!(snapshot.consensus_threshold, 0.7);
    
    // Test existing metrics collection
    snapshot.record_prediction_accuracy(0.85);
    snapshot.record_error_rate(0.05);
    snapshot.record_latency(150);
    
    // Verify existing decision logic unchanged
    assert!(snapshot.should_continue_trading());
    assert!(!snapshot.should_trigger_failsafe());
    
    // Test existing performance thresholds
    assert!(snapshot.meets_accuracy_requirements());
    assert!(snapshot.within_error_tolerance());
}

#[tokio::test]
async fn test_new_fields_properly_integrated() {
    let mut snapshot = PerformanceSnapshot::new();
    
    // Test new extended fields integrate without breaking existing logic
    snapshot.set_neural_complexity_score(0.75);
    snapshot.set_model_stability_index(0.82);
    snapshot.add_training_checkpoint("checkpoint_1".to_string());
    
    // Critical: Verify new fields don't affect existing decision logic
    assert_eq!(snapshot.accuracy_threshold, 0.8); // Unchanged
    assert_eq!(snapshot.error_threshold, 0.1); // Unchanged
    assert_eq!(snapshot.consecutive_failure_threshold, 5); // Unchanged
    
    // Test new fields are accessible
    assert_eq!(snapshot.get_neural_complexity_score(), Some(0.75));
    assert_eq!(snapshot.get_model_stability_index(), Some(0.82));
    assert!(snapshot.get_training_checkpoints().contains(&"checkpoint_1".to_string()));
    
    // Verify existing autonomous trading decisions unaffected
    snapshot.record_prediction_accuracy(0.85);
    assert!(snapshot.should_continue_trading());
}

#[tokio::test]
async fn test_backward_compatibility() {
    // Test that old PerformanceSnapshot instances work with new extensions
    let legacy_snapshot = PerformanceSnapshot::from_legacy_data(
        0.85, // accuracy
        0.05, // error_rate
        100,  // latency_ms
    );
    
    // Verify legacy data preserved exactly
    assert_eq!(legacy_snapshot.get_accuracy(), 0.85);
    assert_eq!(legacy_snapshot.get_error_rate(), 0.05);
    assert_eq!(legacy_snapshot.get_latency_ms(), 100);
    
    // Critical: Verify decision thresholds maintained
    assert_eq!(legacy_snapshot.accuracy_threshold, 0.8);
    assert_eq!(legacy_snapshot.error_threshold, 0.1);
    assert_eq!(legacy_snapshot.consecutive_failure_threshold, 5);
    
    // Test legacy decision logic preserved
    assert!(legacy_snapshot.should_continue_trading());
    assert!(!legacy_snapshot.should_trigger_failsafe());
    
    // Test extended functionality available on legacy instances
    let mut extended_legacy = legacy_snapshot;
    extended_legacy.set_neural_complexity_score(0.65);
    
    // Verify extension doesn't break existing behavior
    assert!(extended_legacy.should_continue_trading());
    assert_eq!(extended_legacy.get_accuracy(), 0.85); // Original data preserved
}

#[tokio::test]
async fn test_enhanced_metrics_integration() {
    let mut snapshot = PerformanceSnapshot::new();
    
    // Test enhanced metrics integrate with existing autonomous logic
    let enhanced_metrics = PredictionMetrics {
        accuracy: 0.88,
        precision: 0.85,
        recall: 0.82,
        f1_score: 0.835,
        confusion_matrix: HashMap::new(),
    };
    
    snapshot.integrate_enhanced_metrics(enhanced_metrics);
    
    // Critical: Verify core autonomous trading thresholds preserved
    assert_eq!(snapshot.accuracy_threshold, 0.8);
    assert_eq!(snapshot.error_threshold, 0.1);
    assert_eq!(snapshot.consecutive_failure_threshold, 5);
    
    // Verify enhanced metrics available
    assert_eq!(snapshot.get_precision(), Some(0.85));
    assert_eq!(snapshot.get_recall(), Some(0.82));
    assert_eq!(snapshot.get_f1_score(), Some(0.835));
    
    // Test autonomous decision logic with enhanced metrics
    assert!(snapshot.should_continue_trading());
    assert!(snapshot.meets_enhanced_performance_criteria());
}

#[tokio::test]
async fn test_checkpoint_metadata_preservation() {
    let mut snapshot = PerformanceSnapshot::new();
    
    // Test checkpoint metadata preserves critical DAA state
    snapshot.add_checkpoint_metadata("checkpoint_1", hashmap!{
        "accuracy_threshold".to_string() => "0.8".to_string(),
        "error_threshold".to_string() => "0.1".to_string(),
        "consecutive_failure_threshold".to_string() => "5".to_string(),
        "voting_ratio".to_string() => "60,40".to_string(),
        "consensus_threshold".to_string() => "0.7".to_string(),
    });
    
    // Verify metadata preserves exact DAA parameters
    let metadata = snapshot.get_checkpoint_metadata("checkpoint_1").unwrap();
    assert_eq!(metadata.get("accuracy_threshold").unwrap(), "0.8");
    assert_eq!(metadata.get("error_threshold").unwrap(), "0.1");
    assert_eq!(metadata.get("consecutive_failure_threshold").unwrap(), "5");
    assert_eq!(metadata.get("voting_ratio").unwrap(), "60,40");
    assert_eq!(metadata.get("consensus_threshold").unwrap(), "0.7");
    
    // Test restoration from checkpoint metadata
    snapshot.restore_from_checkpoint_metadata("checkpoint_1").unwrap();
    
    // Critical: Verify all thresholds restored exactly
    assert_eq!(snapshot.accuracy_threshold, 0.8);
    assert_eq!(snapshot.error_threshold, 0.1);
    assert_eq!(snapshot.consecutive_failure_threshold, 5);
    assert_eq!(snapshot.voting_ratio, (60, 40));
    assert_eq!(snapshot.consensus_threshold, 0.7);
}

#[tokio::test]
async fn test_real_time_adaptation_preserves_core_logic() {
    let mut snapshot = PerformanceSnapshot::new();
    
    // Test real-time adaptation doesn't break autonomous trading
    snapshot.enable_real_time_adaptation(true);
    
    // Simulate market condition changes
    snapshot.adapt_to_market_conditions(MarketCondition::HighVolatility);
    
    // Critical: Core DAA thresholds must remain unchanged
    assert_eq!(snapshot.accuracy_threshold, 0.8);
    assert_eq!(snapshot.error_threshold, 0.1);
    assert_eq!(snapshot.consecutive_failure_threshold, 5);
    
    // Test autonomous decisions still work under adaptation
    snapshot.record_prediction_accuracy(0.85);
    assert!(snapshot.should_continue_trading());
    
    // Verify adaptation metadata doesn't interfere with core logic
    assert!(snapshot.get_adaptation_history().len() > 0);
    assert!(snapshot.should_continue_trading()); // Decision logic preserved
}

// Helper enum for testing
#[derive(Clone, Debug)]
enum MarketCondition {
    HighVolatility,
    LowVolatility,
    Trending,
    Sideways,
}

// Helper macro for creating HashMaps
macro_rules! hashmap {
    ($($key:expr => $value:expr),* $(,)?) => {
        {
            let mut map = HashMap::new();
            $(map.insert($key, $value);)*
            map
        }
    };
}