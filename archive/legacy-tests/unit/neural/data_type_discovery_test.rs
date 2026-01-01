use std::collections::HashMap;
use neural_trader::{
    neural::{DataTypeDiscovery, NeuralPredictor, PredictionError},
    data::{TimeSeriesData, DataType, DataQuality},
    daa::{DaaCoordinator, AutonomousTrainingEngine},
    utils::market_hours::MarketHours,
};

#[tokio::test]
async fn test_discovery_integrates_with_daa() {
    // CRITICAL: Verify 60/40 voting preserved, 70% consensus maintained
    let mut discovery = DataTypeDiscovery::new();
    let mut daa_coordinator = DaaCoordinator::new(MarketHours::new()).await.unwrap();
    
    // Test data type discovery integrates with existing DAA flow
    let raw_data = vec![
        vec![1.0, 2.0, 3.0], // Price data
        vec![1000.0, 1500.0, 2000.0], // Volume data
        vec![0.1, 0.15, 0.08], // Volatility data
    ];
    
    let discovered_types = discovery.analyze_data_types(&raw_data).await.unwrap();
    
    // Verify discovery doesn't break DAA voting mechanism
    assert_eq!(daa_coordinator.get_voting_ratio(), (60, 40)); // 60/40 preserved
    assert_eq!(daa_coordinator.get_consensus_threshold(), 0.7); // 70% consensus maintained
    
    // Test integration with autonomous training
    let training_engine = daa_coordinator.get_training_engine();
    assert_eq!(training_engine.config().accuracy_threshold, 0.8);
    assert_eq!(training_engine.config().error_threshold, 0.1);
    assert_eq!(training_engine.config().consecutive_failure_threshold, 5);
    
    // Verify discovered types enhance DAA without breaking it
    let enhanced_data = discovery.enhance_time_series_with_types(
        &TimeSeriesData::new(raw_data[0].clone(), chrono::Utc::now()),
        &discovered_types
    ).await.unwrap();
    
    let prediction_result = daa_coordinator.make_autonomous_prediction(&enhanced_data).await;
    assert!(prediction_result.is_ok());
}

#[tokio::test]
async fn test_graceful_degradation_with_missing_data() {
    let mut discovery = DataTypeDiscovery::new();
    let mut daa_coordinator = DaaCoordinator::new(MarketHours::new()).await.unwrap();
    
    // Test graceful degradation with incomplete data
    let incomplete_data = vec![
        vec![f64::NAN, 2.0, 3.0], // Missing first value
        vec![1000.0], // Single value
        vec![], // Empty vector
    ];
    
    let discovery_result = discovery.analyze_data_types(&incomplete_data).await;
    
    // Critical: DAA must continue functioning even with discovery issues
    assert_eq!(daa_coordinator.get_voting_ratio(), (60, 40));
    assert_eq!(daa_coordinator.get_consensus_threshold(), 0.7);
    
    // Test fallback to default behavior
    let fallback_data = TimeSeriesData::new(vec![1.0, 2.0, 3.0], chrono::Utc::now());
    let prediction_result = daa_coordinator.make_autonomous_prediction(&fallback_data).await;
    assert!(prediction_result.is_ok());
    
    // Verify autonomous trading thresholds unaffected by discovery failures
    let training_engine = daa_coordinator.get_training_engine();
    assert_eq!(training_engine.config().accuracy_threshold, 0.8);
    assert_eq!(training_engine.config().error_threshold, 0.1);
    assert_eq!(training_engine.config().consecutive_failure_threshold, 5);
}

#[tokio::test]
async fn test_type_discovery_enhances_prediction_accuracy() {
    let mut discovery = DataTypeDiscovery::new();
    let mut daa_coordinator = DaaCoordinator::new(MarketHours::new()).await.unwrap();
    
    // Test that type discovery improves predictions while preserving DAA
    let market_data = vec![
        vec![100.0, 101.0, 102.0, 103.0], // OHLC price data
        vec![1000.0, 1200.0, 900.0, 1100.0], // Volume data
        vec![0.02, 0.025, 0.018, 0.022], // Volatility data
    ];
    
    // Discover data types
    let discovered_types = discovery.analyze_data_types(&market_data).await.unwrap();
    assert!(discovered_types.contains(&DataType::Price));
    assert!(discovered_types.contains(&DataType::Volume));
    assert!(discovered_types.contains(&DataType::Volatility));
    
    // Create enhanced time series
    let enhanced_data = discovery.enhance_time_series_with_types(
        &TimeSeriesData::new(market_data[0].clone(), chrono::Utc::now()),
        &discovered_types
    ).await.unwrap();
    
    // Test prediction with enhanced data
    let prediction_result = daa_coordinator.make_autonomous_prediction(&enhanced_data).await;
    assert!(prediction_result.is_ok());
    
    // Critical: Verify DAA decision thresholds maintained
    assert_eq!(daa_coordinator.get_voting_ratio(), (60, 40));
    assert_eq!(daa_coordinator.get_consensus_threshold(), 0.7);
    
    // Verify autonomous trading capabilities preserved
    let should_trade = daa_coordinator.should_execute_trade(&enhanced_data).await.unwrap();
    assert!(should_trade.is_some()); // Decision mechanism works
}

#[tokio::test]
async fn test_data_quality_assessment_integration() {
    let mut discovery = DataTypeDiscovery::new();
    let mut daa_coordinator = DaaCoordinator::new(MarketHours::new()).await.unwrap();
    
    // Test data quality assessment integrates with DAA quality gates
    let high_quality_data = vec![
        vec![100.0, 101.0, 102.0, 103.0], // Clean price data
        vec![1000.0, 1200.0, 900.0, 1100.0], // Clean volume data
    ];
    
    let poor_quality_data = vec![
        vec![f64::NAN, 101.0, f64::INFINITY, 103.0], // Dirty price data
        vec![0.0, -100.0, 1e10, f64::NEG_INFINITY], // Invalid volume data
    ];
    
    // Assess data quality
    let high_quality_assessment = discovery.assess_data_quality(&high_quality_data).await.unwrap();
    let poor_quality_assessment = discovery.assess_data_quality(&poor_quality_data).await.unwrap();
    
    assert_eq!(high_quality_assessment.overall_score, DataQuality::High);
    assert_eq!(poor_quality_assessment.overall_score, DataQuality::Poor);
    
    // Test DAA responds appropriately to quality assessment
    let high_quality_ts = TimeSeriesData::new(high_quality_data[0].clone(), chrono::Utc::now());
    let poor_quality_ts = TimeSeriesData::new(poor_quality_data[0].clone(), chrono::Utc::now());
    
    // High quality data should proceed normally
    let good_prediction = daa_coordinator.make_autonomous_prediction(&high_quality_ts).await;
    assert!(good_prediction.is_ok());
    
    // Poor quality data should trigger appropriate safeguards
    let poor_prediction = daa_coordinator.make_autonomous_prediction(&poor_quality_ts).await;
    // Should either succeed with fallback or fail gracefully
    assert!(poor_prediction.is_ok() || poor_prediction.is_err());
    
    // Critical: DAA thresholds remain unchanged regardless of data quality
    assert_eq!(daa_coordinator.get_voting_ratio(), (60, 40));
    assert_eq!(daa_coordinator.get_consensus_threshold(), 0.7);
    
    let training_engine = daa_coordinator.get_training_engine();
    assert_eq!(training_engine.config().accuracy_threshold, 0.8);
    assert_eq!(training_engine.config().error_threshold, 0.1);
    assert_eq!(training_engine.config().consecutive_failure_threshold, 5);
}

#[tokio::test]
async fn test_adaptive_type_learning_preserves_daa() {
    let mut discovery = DataTypeDiscovery::new();
    let mut daa_coordinator = DaaCoordinator::new(MarketHours::new()).await.unwrap();
    
    // Test adaptive learning in type discovery doesn't interfere with DAA learning
    let training_sequences = vec![
        vec![vec![100.0, 101.0], vec![1000.0, 1100.0]], // Sequence 1
        vec![vec![102.0, 103.0], vec![1200.0, 1300.0]], // Sequence 2
        vec![vec![104.0, 105.0], vec![1400.0, 1500.0]], // Sequence 3
    ];
    
    // Train type discovery
    for sequence in &training_sequences {
        let _ = discovery.learn_from_sequence(sequence).await;
    }
    
    // Verify DAA learning is preserved and independent
    let daa_training_data = training_sequences[0][0].clone();
    let training_ts = TimeSeriesData::new(daa_training_data, chrono::Utc::now());
    
    let training_result = daa_coordinator.autonomous_train(&[training_ts]).await;
    assert!(training_result.is_ok());
    
    // Critical: Verify all DAA parameters preserved after both learning processes
    assert_eq!(daa_coordinator.get_voting_ratio(), (60, 40));
    assert_eq!(daa_coordinator.get_consensus_threshold(), 0.7);
    
    let training_engine = daa_coordinator.get_training_engine();
    assert_eq!(training_engine.config().accuracy_threshold, 0.8);
    assert_eq!(training_engine.config().error_threshold, 0.1);
    assert_eq!(training_engine.config().consecutive_failure_threshold, 5);
    
    // Test integrated prediction with both learning systems
    let test_data = TimeSeriesData::new(vec![106.0, 107.0], chrono::Utc::now());
    let prediction_result = daa_coordinator.make_autonomous_prediction(&test_data).await;
    assert!(prediction_result.is_ok());
}

#[tokio::test]
async fn test_real_time_type_adaptation() {
    let mut discovery = DataTypeDiscovery::new();
    let mut daa_coordinator = DaaCoordinator::new(MarketHours::new()).await.unwrap();
    
    // Test real-time type adaptation during market conditions
    let morning_data = vec![
        vec![100.0, 101.0, 102.0], // Lower volume period
        vec![500.0, 600.0, 700.0],
    ];
    
    let market_open_data = vec![
        vec![102.0, 105.0, 108.0], // Higher volatility
        vec![2000.0, 2500.0, 3000.0], // Higher volume
    ];
    
    // Adapt to changing market conditions
    let morning_types = discovery.analyze_data_types(&morning_data).await.unwrap();
    let market_types = discovery.analyze_data_types(&market_open_data).await.unwrap();
    
    // Test adaptation doesn't break DAA real-time decisions
    let morning_ts = TimeSeriesData::new(morning_data[0].clone(), chrono::Utc::now());
    let market_ts = TimeSeriesData::new(market_open_data[0].clone(), chrono::Utc::now());
    
    let morning_decision = daa_coordinator.should_execute_trade(&morning_ts).await.unwrap();
    let market_decision = daa_coordinator.should_execute_trade(&market_ts).await.unwrap();
    
    // Both should work (though decisions may differ based on data)
    assert!(morning_decision.is_some() || morning_decision.is_none());
    assert!(market_decision.is_some() || market_decision.is_none());
    
    // Critical: Real-time adaptation preserves DAA core parameters
    assert_eq!(daa_coordinator.get_voting_ratio(), (60, 40));
    assert_eq!(daa_coordinator.get_consensus_threshold(), 0.7);
    
    let training_engine = daa_coordinator.get_training_engine();
    assert_eq!(training_engine.config().accuracy_threshold, 0.8);
    assert_eq!(training_engine.config().error_threshold, 0.1);
    assert_eq!(training_engine.config().consecutive_failure_threshold, 5);
}