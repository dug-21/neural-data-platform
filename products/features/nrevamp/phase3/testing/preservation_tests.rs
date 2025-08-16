//! Phase 3 Preservation Tests
//! 
//! CRITICAL: These tests validate that ALL existing DAA autonomous trading capabilities
//! are preserved while adding Phase 3 extensions. NO regression is acceptable.

use std::time::{Duration, Instant};
use tokio::time::sleep;
use serde_json::json;
use crate::daa::autonomous_training::AutonomousTrainingEngine;
use crate::daa::coordinator::DAACoordinator;
use crate::daa::types::{PerformanceSnapshot, TradingDecision, VotingWeights};
use crate::neural::vendor_predictor::VendorPredictor;
use crate::features::shared_feature_extractor::SharedFeatureExtractor;

#[cfg(test)]
mod preservation_tests {
    use super::*;

    /// Test 1: Validate 60/40 Neural/Strategy Voting Split is Preserved
    #[tokio::test]
    async fn test_60_40_voting_split_preservation() {
        let mut coordinator = DAACoordinator::new();
        
        // Enable all Phase 3 extensions
        coordinator.enable_real_time_training(true);
        coordinator.enable_data_type_discovery(true);
        coordinator.enable_multi_modal_fusion(true);
        
        // Test with various market contexts
        let test_contexts = vec![
            create_simple_market_context(),
            create_volatile_market_context(),
            create_trending_market_context(),
            create_sideways_market_context(),
        ];
        
        for context in test_contexts {
            let decision = coordinator.make_autonomous_decision(&context).await
                .expect("DAA coordinator should make decisions with Phase 3 extensions");
            
            // CRITICAL PRESERVATION: 60/40 voting split must be exact
            assert_eq!(decision.voting_weights.neural_weight, 0.6, 
                "Neural voting weight must remain exactly 60% with Phase 3 extensions");
            assert_eq!(decision.voting_weights.strategy_weight, 0.4,
                "Strategy voting weight must remain exactly 40% with Phase 3 extensions");
            
            // Verify total voting weights sum to 1.0
            let total_weight = decision.voting_weights.neural_weight + decision.voting_weights.strategy_weight;
            assert!((total_weight - 1.0).abs() < f64::EPSILON, 
                "Total voting weights must sum to exactly 1.0");
        }
    }

    /// Test 2: Validate 70% Byzantine Consensus Threshold is Preserved
    #[tokio::test]
    async fn test_70_percent_consensus_threshold_preservation() {
        let mut coordinator = DAACoordinator::new();
        
        // Enable Phase 3 extensions
        coordinator.enable_advanced_analytics(true);
        coordinator.enable_model_value_assessment(true);
        
        // Test consensus behavior with various agreement levels
        let consensus_scenarios = vec![
            (0.65, false), // Below threshold - should reject
            (0.69, false), // Just below threshold - should reject
            (0.70, true),  // Exactly at threshold - should accept
            (0.71, true),  // Above threshold - should accept
            (0.85, true),  // Well above threshold - should accept
        ];
        
        for (consensus_level, should_accept) in consensus_scenarios {
            let context = create_market_context_with_consensus(consensus_level);
            let decision = coordinator.make_autonomous_decision(&context).await
                .expect("DAA coordinator should handle consensus scenarios");
            
            // CRITICAL PRESERVATION: 70% consensus threshold must be enforced
            assert_eq!(decision.consensus_threshold, 0.7,
                "Byzantine consensus threshold must remain exactly 70%");
            assert_eq!(decision.consensus_reached, should_accept,
                "Consensus behavior with {}% agreement should be {}", 
                consensus_level * 100.0, should_accept);
        }
    }

    /// Test 3: Validate Performance Thresholds Are Preserved
    #[tokio::test]
    async fn test_performance_thresholds_preservation() {
        let mut training_engine = AutonomousTrainingEngine::new();
        
        // Enable Phase 3 real-time training extensions
        training_engine.enable_real_time_parameter_updates(true);
        training_engine.enable_adaptive_learning_rate(true);
        training_engine.enable_model_checkpointing(true);
        
        // CRITICAL PRESERVATION: All thresholds must remain unchanged
        assert_eq!(training_engine.get_accuracy_threshold(), 0.8,
            "Accuracy threshold must remain exactly 0.8 (80%)");
        assert_eq!(training_engine.get_error_threshold(), 0.1,
            "Error threshold must remain exactly 0.1 (10%)");
        assert_eq!(training_engine.get_consecutive_failure_threshold(), 5,
            "Consecutive failure threshold must remain exactly 5");
        
        // Test that thresholds are enforced during real-time training
        let performance_scenarios = vec![
            // (accuracy, error_rate, failures, should_trigger_retraining)
            (0.85, 0.05, 2, false), // Good performance - no retraining
            (0.79, 0.05, 2, true),  // Low accuracy - should retrain
            (0.85, 0.11, 2, true),  // High error - should retrain  
            (0.85, 0.05, 6, true),  // Too many failures - should retrain
        ];
        
        for (accuracy, error_rate, failures, should_retrain) in performance_scenarios {
            let snapshot = create_performance_snapshot(accuracy, error_rate, failures);
            let should_trigger = training_engine.should_trigger_retraining(&snapshot);
            
            assert_eq!(should_trigger, should_retrain,
                "Retraining trigger with accuracy={}, error={}, failures={} should be {}",
                accuracy, error_rate, failures, should_retrain);
        }
    }

    /// Test 4: Validate Memory Budget Preservation (<525MB)
    #[tokio::test]
    async fn test_memory_budget_preservation() {
        // Baseline memory usage measurement
        let baseline_memory = get_current_memory_usage();
        
        // Initialize all Phase 3 extensions
        let mut feature_extractor = SharedFeatureExtractor::new();
        let mut coordinator = DAACoordinator::new();
        let mut training_engine = AutonomousTrainingEngine::new();
        
        // Enable all Phase 3 capabilities
        feature_extractor.enable_multi_modal_fusion(true);
        feature_extractor.enable_dynamic_data_types(true);
        coordinator.enable_real_time_training(true);
        coordinator.enable_advanced_analytics(true);
        training_engine.enable_real_time_parameter_updates(true);
        
        // Load typical trading session data
        let symbols = vec!["AAPL", "MSFT", "GOOGL", "TSLA", "NVDA"];
        for symbol in &symbols {
            let features = feature_extractor.extract_features(symbol).await
                .expect("Feature extraction should work with Phase 3 extensions");
            
            let context = create_market_context_for_symbol(symbol, &features);
            let _ = coordinator.make_autonomous_decision(&context).await
                .expect("DAA decisions should work with enhanced features");
        }
        
        // Measure memory after Phase 3 system is fully loaded
        let loaded_memory = get_current_memory_usage();
        let memory_increase = loaded_memory - baseline_memory;
        
        // CRITICAL PRESERVATION: Total memory must stay under 525MB
        assert!(loaded_memory < 525_000_000, 
            "Total system memory {} bytes exceeds 525MB limit", loaded_memory);
        
        // Phase 3 overhead should be minimal (<5% of Phase 2's 500MB)
        assert!(memory_increase < 25_000_000,
            "Phase 3 memory overhead {} bytes exceeds 25MB (5%) allowance", memory_increase);
        
        // Verify Phase 2's 90% memory reduction is preserved
        // Original system used ~5GB, Phase 2 reduced to 500MB, Phase 3 adds <25MB
        let phase2_memory_target = 500_000_000; // 500MB from Phase 2
        assert!(loaded_memory <= phase2_memory_target + 25_000_000,
            "Memory usage {} exceeds Phase 2 target {} + 25MB overhead", 
            loaded_memory, phase2_memory_target);
    }

    /// Test 5: Validate Prediction Latency Preservation (<100ms)
    #[tokio::test]
    async fn test_prediction_latency_preservation() {
        let mut predictor = VendorPredictor::new();
        
        // Enable all Phase 3 real-time capabilities
        predictor.enable_real_time_training(true);
        predictor.enable_data_type_discovery(true);
        predictor.enable_multi_modal_prediction(true);
        
        // Warm up the system
        for _ in 0..10 {
            let _ = predictor.predict("AAPL", &create_sample_features()).await;
        }
        
        // Test prediction latency under various loads
        let test_scenarios = vec![
            ("simple_prediction", create_simple_features()),
            ("enhanced_prediction", create_enhanced_features_with_sentiment()),
            ("multi_modal_prediction", create_multi_modal_features()),
            ("real_time_training_active", create_features_during_training()),
        ];
        
        for (scenario_name, features) in test_scenarios {
            let start_time = Instant::now();
            let prediction = predictor.predict("AAPL", &features).await
                .expect(&format!("Prediction should work in scenario: {}", scenario_name));
            let latency = start_time.elapsed();
            
            // CRITICAL PRESERVATION: Prediction latency must stay under 100ms
            assert!(latency < Duration::from_millis(100),
                "Prediction latency {}ms exceeds 100ms limit in scenario: {}",
                latency.as_millis(), scenario_name);
            
            // Verify prediction quality is maintained
            assert!(prediction.confidence >= 0.7,
                "Prediction confidence {} below minimum 0.7 in scenario: {}",
                prediction.confidence, scenario_name);
        }
    }

    /// Test 6: Validate DAA Autonomous Capabilities Preservation
    #[tokio::test]
    async fn test_autonomous_capabilities_preservation() {
        let mut system = create_neural_trading_system_with_phase3();
        
        // Test autonomous operation without human intervention
        let trading_session = create_24_hour_trading_session();
        let mut autonomous_decisions = Vec::new();
        
        for hour in 0..24 {
            let market_state = trading_session.get_market_state(hour);
            
            // System must make autonomous decisions
            let decision = system.make_autonomous_trading_decision(&market_state).await
                .expect("System should make autonomous decisions with Phase 3 extensions");
            
            // Verify all autonomous trading criteria preserved
            assert!(decision.is_autonomous, "Decision should be fully autonomous");
            assert_eq!(decision.voting_weights.neural_weight, 0.6, "Neural weight preserved");
            assert_eq!(decision.voting_weights.strategy_weight, 0.4, "Strategy weight preserved");
            assert!(decision.consensus_reached, "Consensus should be reached");
            assert!(decision.consensus_percentage >= 0.7, "70% consensus threshold enforced");
            
            // Verify decision quality metrics
            assert!(decision.confidence_score >= 0.7, "Decision confidence maintained");
            assert!(decision.risk_assessment.is_within_limits(), "Risk limits enforced");
            
            autonomous_decisions.push(decision);
        }
        
        // Verify autonomous learning occurred
        let initial_accuracy = autonomous_decisions.first().unwrap().accuracy_estimate;
        let final_accuracy = autonomous_decisions.last().unwrap().accuracy_estimate;
        
        // System should learn and adapt autonomously
        assert!(final_accuracy >= initial_accuracy,
            "Autonomous learning should maintain or improve accuracy");
    }

    /// Test 7: Validate Sector Aggregation Preservation (Phase 2)
    #[tokio::test]
    async fn test_sector_aggregation_preservation() {
        let mut feature_extractor = SharedFeatureExtractor::new();
        
        // Enable Phase 3 extensions
        feature_extractor.enable_multi_modal_fusion(true);
        feature_extractor.enable_dynamic_data_discovery(true);
        
        // Test that Phase 2's sector aggregation still works
        let tech_symbols = vec!["AAPL", "MSFT", "GOOGL", "NVDA"];
        let finance_symbols = vec!["JPM", "BAC", "WFC", "GS"];
        
        // Extract features for tech sector
        let tech_features = feature_extractor.extract_sector_features("technology", &tech_symbols).await
            .expect("Sector feature extraction should work with Phase 3");
        
        // Extract features for finance sector  
        let finance_features = feature_extractor.extract_sector_features("finance", &finance_symbols).await
            .expect("Sector feature extraction should work with Phase 3");
        
        // CRITICAL PRESERVATION: Verify Phase 2's memory optimization maintained
        let memory_before = get_current_memory_usage();
        
        // Load features for all symbols in both sectors
        for symbol in tech_symbols.iter().chain(finance_symbols.iter()) {
            let _ = feature_extractor.get_cached_features(symbol).await;
        }
        
        let memory_after = get_current_memory_usage();
        let memory_increase = memory_after - memory_before;
        
        // Sector sharing should prevent linear memory growth
        assert!(memory_increase < 50_000_000, // <50MB for 8 symbols
            "Sector aggregation memory efficiency lost - increase: {} bytes", memory_increase);
        
        // Verify feature quality maintained
        assert!(tech_features.quality_score >= 0.8, "Tech sector feature quality preserved");
        assert!(finance_features.quality_score >= 0.8, "Finance sector feature quality preserved");
    }

    /// Test 8: Validate Fault Tolerance and Recovery Preservation
    #[tokio::test]
    async fn test_fault_tolerance_preservation() {
        let mut coordinator = DAACoordinator::new();
        
        // Enable Phase 3 extensions
        coordinator.enable_real_time_training(true);
        coordinator.enable_model_checkpointing(true);
        
        // Test Byzantine fault tolerance with model failures
        let fault_scenarios = vec![
            simulate_neural_model_failure(),
            simulate_strategy_agent_failure(),
            simulate_data_source_failure(),
            simulate_network_partition(),
        ];
        
        for fault in fault_scenarios {
            // Inject fault
            coordinator.inject_fault(fault.clone()).await;
            
            // System should detect and recover
            let recovery_start = Instant::now();
            let decision = coordinator.make_autonomous_decision(&create_market_context()).await
                .expect("System should recover from faults");
            let recovery_time = recovery_start.elapsed();
            
            // CRITICAL PRESERVATION: Fault tolerance mechanisms preserved
            assert!(decision.consensus_reached, "Consensus should work despite fault: {:?}", fault);
            assert!(decision.consensus_percentage >= 0.7, "70% threshold enforced during fault");
            assert!(recovery_time < Duration::from_secs(5), "Recovery should be fast");
            
            // System should maintain voting weights even during faults
            assert_eq!(decision.voting_weights.neural_weight, 0.6, "Neural weight during fault");
            assert_eq!(decision.voting_weights.strategy_weight, 0.4, "Strategy weight during fault");
            
            // Clear fault for next test
            coordinator.clear_fault(fault).await;
        }
    }
}

// Helper functions for test setup
fn get_current_memory_usage() -> usize {
    // Implementation to get actual memory usage
    // This would interface with system memory APIs
    std::process::Command::new("ps")
        .args(&["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<usize>()
                .unwrap_or(0) * 1024 // Convert KB to bytes
        })
        .unwrap_or(0)
}

fn create_market_context_with_consensus(consensus_level: f64) -> MarketContext {
    MarketContext {
        symbol: "AAPL".to_string(),
        timestamp: chrono::Utc::now(),
        consensus_level,
        ..Default::default()
    }
}

fn create_performance_snapshot(accuracy: f64, error_rate: f64, failures: u32) -> PerformanceSnapshot {
    PerformanceSnapshot {
        accuracy,
        error_rate,
        consecutive_failures: failures,
        timestamp: chrono::Utc::now(),
        ..Default::default()
    }
}

fn create_sample_features() -> Features {
    Features {
        price_features: vec![100.0, 101.0, 99.5],
        volume_features: vec![1000000.0],
        technical_indicators: vec![0.5, 0.3, 0.8],
        ..Default::default()
    }
}

fn create_enhanced_features_with_sentiment() -> Features {
    let mut features = create_sample_features();
    features.sentiment_features = Some(vec![0.7, 0.6, 0.8]); // Positive sentiment
    features
}

fn create_multi_modal_features() -> Features {
    let mut features = create_enhanced_features_with_sentiment();
    features.alternative_data = Some(vec![0.4, 0.6, 0.5]); // Alternative data sources
    features.news_features = Some(vec![0.8, 0.7, 0.9]); // News analysis
    features
}

#[derive(Debug, Clone)]
enum FaultType {
    NeuralModelFailure,
    StrategyAgentFailure, 
    DataSourceFailure,
    NetworkPartition,
}

fn simulate_neural_model_failure() -> FaultType {
    FaultType::NeuralModelFailure
}

fn simulate_strategy_agent_failure() -> FaultType {
    FaultType::StrategyAgentFailure
}

fn simulate_data_source_failure() -> FaultType {
    FaultType::DataSourceFailure
}

fn simulate_network_partition() -> FaultType {
    FaultType::NetworkPartition
}