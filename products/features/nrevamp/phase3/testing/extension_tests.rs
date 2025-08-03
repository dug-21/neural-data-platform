//! Phase 3 Extension Tests
//! 
//! These tests validate that new Phase 3 capabilities work correctly while
//! maintaining integration with existing DAA systems.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use serde_json::json;
use crate::daa::autonomous_training::AutonomousTrainingEngine;
use crate::daa::coordinator::DAACoordinator;
use crate::neural::vendor_predictor::VendorPredictor;
use crate::data::ingestion_adapter::DataIngestionAdapter;
use crate::features::multimodal_fusion::MultiModalFusionEngine;
use crate::monitoring::data_availability::DataAvailabilityTracker;

#[cfg(test)]
mod extension_tests {
    use super::*;

    /// Test 1: Dynamic Data Type Discovery Functionality
    #[tokio::test]
    async fn test_dynamic_data_type_discovery() {
        let mut registry = DataTypeRegistry::new();
        let mut coordinator = DAACoordinator::new();
        
        // Start with basic price data only
        assert_eq!(registry.get_registered_types().len(), 1); // Just price
        
        // Simulate discovery of sentiment data
        let sentiment_data = json!({
            "news_score": 0.8,
            "social_media_score": 0.6,
            "analyst_sentiment": 0.7
        });
        
        let discovered_type = registry.discover_data_type(&sentiment_data).await
            .expect("Should discover sentiment data type");
        
        assert_eq!(discovered_type.name, "sentiment");
        assert_eq!(discovered_type.characteristics.frequency, Duration::from_secs(300)); // 5 min
        assert_eq!(discovered_type.characteristics.scope, DataScope::Symbol);
        
        // Verify discovery integrates with DAA decisions
        let context = create_market_context_with_discovered_data(sentiment_data);
        let decision = coordinator.make_autonomous_decision(&context).await
            .expect("DAA should work with discovered data types");
        
        // Decision should leverage new data but preserve voting structure
        assert_eq!(decision.voting_weights.neural_weight, 0.6);
        assert_eq!(decision.voting_weights.strategy_weight, 0.4);
        assert!(decision.confidence_score > 0.8); // Enhanced by sentiment data
        
        // Verify automatic model activation
        let suitable_models = registry.get_suitable_models(&discovered_type);
        assert!(!suitable_models.is_empty(), "Should find models for sentiment data");
    }

    /// Test 2: Channel-Agnostic Data Ingestion
    #[tokio::test]
    async fn test_channel_agnostic_ingestion() {
        let mut adapter = DataIngestionAdapter::new();
        
        // Test various Redis channel patterns
        let channel_patterns = vec![
            "market:stocks:AAPL",
            "sector:technology:sentiment", 
            "symbol:MSFT:news",
            "geo:US:economic",
            "alternative:social:reddit"
        ];
        
        for pattern in channel_patterns {
            let stream = adapter.subscribe_pattern(pattern).await
                .expect(&format!("Should subscribe to pattern: {}", pattern));
            
            // Simulate data packet for this channel
            let test_packet = create_test_data_packet(pattern);
            let route = adapter.route_by_scope(test_packet.clone()).await;
            
            // Verify correct routing based on pattern
            match pattern {
                p if p.starts_with("symbol:") => {
                    assert_eq!(route.destination, RouteDestination::SymbolProcessor);
                    assert_eq!(route.symbol, Some("MSFT".to_string()));
                }
                p if p.starts_with("sector:") => {
                    assert_eq!(route.destination, RouteDestination::SectorAggregator);
                    assert_eq!(route.sector, Some("technology".to_string()));
                }
                p if p.starts_with("market:") => {
                    assert_eq!(route.destination, RouteDestination::MarketAnalyzer);
                }
                p if p.starts_with("geo:") => {
                    assert_eq!(route.destination, RouteDestination::GeographicHandler);
                    assert_eq!(route.region, Some("US".to_string()));
                }
                _ => panic!("Unexpected pattern: {}", pattern),
            }
        }
        
        // Test consolidation into unified symbol streams
        let aapl_stream = adapter.consolidate_symbol("AAPL").await
            .expect("Should consolidate AAPL data streams");
        
        assert!(aapl_stream.has_price_data());
        assert!(aapl_stream.has_volume_data());
        // May or may not have sentiment/news depending on availability
    }

    /// Test 3: Real-Time Adaptive Training
    #[tokio::test]
    async fn test_real_time_adaptive_training() {
        let mut training_engine = AutonomousTrainingEngine::new();
        let mut predictor = VendorPredictor::new();
        
        // Enable real-time training
        training_engine.enable_real_time_parameter_updates(true);
        predictor.enable_real_time_training(true);
        
        // Establish baseline performance
        let baseline_context = create_stable_market_context();
        let baseline_prediction = predictor.predict("AAPL", &baseline_context.features).await
            .expect("Baseline prediction should work");
        
        // Simulate market regime change
        let regime_change_data = vec![
            create_volatile_market_data(0.3), // High volatility
            create_trending_market_data(0.15), // Strong trend
            create_reversal_market_data(-0.08), // Market reversal
        ];
        
        let mut accuracy_improvements = Vec::new();
        
        for (i, market_data) in regime_change_data.iter().enumerate() {
            // Make prediction with current model
            let prediction = predictor.predict("AAPL", &market_data.features).await
                .expect("Prediction should work during adaptation");
            
            // Simulate actual market outcome
            let actual_outcome = market_data.get_actual_outcome();
            let accuracy = calculate_prediction_accuracy(&prediction, &actual_outcome);
            
            // Trigger real-time parameter update
            let feedback = ModelFeedback {
                prediction_accuracy: accuracy,
                market_regime: market_data.regime_type.clone(),
                volatility_level: market_data.volatility,
                confidence_level: prediction.confidence,
            };
            
            training_engine.update_realtime_parameters(&feedback).await
                .expect("Real-time parameter update should work");
            
            // Test that thresholds are still enforced during adaptation
            assert_eq!(training_engine.get_accuracy_threshold(), 0.8,
                "Accuracy threshold preserved during real-time training");
            
            // Track improvement over adaptation period
            if i > 0 {
                let previous_accuracy = accuracy_improvements.last().copied().unwrap_or(0.5);
                if accuracy > previous_accuracy {
                    accuracy_improvements.push(accuracy);
                }
            }
        }
        
        // Verify adaptation improved performance
        let final_context = create_stable_market_context();
        let final_prediction = predictor.predict("AAPL", &final_context.features).await
            .expect("Final prediction should work");
        
        assert!(final_prediction.confidence >= baseline_prediction.confidence,
            "Real-time training should maintain or improve confidence");
    }

    /// Test 4: Multi-Modal Data Fusion
    #[tokio::test]
    async fn test_multi_modal_data_fusion() {
        let mut fusion_engine = MultiModalFusionEngine::new();
        let mut feature_extractor = SharedFeatureExtractor::new();
        
        // Enable multi-modal fusion
        fusion_engine.enable_cross_modal_attention(true);
        feature_extractor.enable_multi_modal_fusion(true);
        
        // Prepare multiple data modalities
        let price_data = create_price_data("AAPL");
        let sentiment_data = create_sentiment_data("AAPL");
        let news_data = create_news_data("AAPL");
        let alternative_data = create_alternative_data("AAPL");
        
        // Test fusion of different combinations
        let fusion_scenarios = vec![
            ("price_only", vec![price_data.clone()]),
            ("price_sentiment", vec![price_data.clone(), sentiment_data.clone()]),
            ("price_news", vec![price_data.clone(), news_data.clone()]),
            ("all_modalities", vec![price_data.clone(), sentiment_data.clone(), 
                                  news_data.clone(), alternative_data.clone()]),
        ];
        
        for (scenario_name, data_modalities) in fusion_scenarios {
            let fused_features = fusion_engine.fuse_modalities(data_modalities).await
                .expect(&format!("Fusion should work for scenario: {}", scenario_name));
            
            // Verify fusion quality
            assert!(fused_features.quality_score >= 0.7,
                "Fused features quality should be high in scenario: {}", scenario_name);
            
            // Test integration with shared feature extractor
            let enhanced_features = feature_extractor.enhance_with_fusion(
                "AAPL", &fused_features
            ).await.expect("Fusion should integrate with feature extractor");
            
            // Verify memory efficiency preserved
            let memory_footprint = enhanced_features.get_memory_footprint();
            assert!(memory_footprint < 10_000_000, // <10MB per symbol
                "Multi-modal features should be memory efficient: {} bytes", memory_footprint);
            
            // More modalities should improve confidence but not dramatically increase memory
            match scenario_name {
                "price_only" => assert!(fused_features.confidence >= 0.7),
                "price_sentiment" => assert!(fused_features.confidence >= 0.75),
                "all_modalities" => assert!(fused_features.confidence >= 0.85),
                _ => {}
            }
        }
    }

    /// Test 5: Advanced Model Analytics
    #[tokio::test]
    async fn test_advanced_model_analytics() {
        let mut analytics_engine = ModelAnalyticsEngine::new();
        let mut coordinator = DAACoordinator::new();
        
        // Enable advanced analytics
        analytics_engine.enable_model_value_assessment(true);
        analytics_engine.enable_resource_efficiency_analysis(true);
        coordinator.enable_advanced_analytics(true);
        
        // Create test models with different characteristics
        let test_models = vec![
            create_high_accuracy_model("lstm_v1", 0.85, 100_000), // High accuracy, high memory
            create_medium_accuracy_model("mlp_v2", 0.78, 50_000), // Medium accuracy, medium memory
            create_fast_model("linear_v1", 0.72, 10_000), // Lower accuracy, low memory
        ];
        
        for model in test_models {
            // Analyze model value
            let value_assessment = analytics_engine.assess_model_value(&model).await
                .expect("Model value assessment should work");
            
            // Verify value scoring components
            assert!(value_assessment.accuracy_score >= 0.0 && value_assessment.accuracy_score <= 1.0);
            assert!(value_assessment.efficiency_score >= 0.0 && value_assessment.efficiency_score <= 1.0);
            assert!(value_assessment.reliability_score >= 0.0 && value_assessment.reliability_score <= 1.0);
            
            // Calculate overall value score
            let overall_value = analytics_engine.calculate_overall_value(&value_assessment);
            assert!(overall_value >= 0.0 && overall_value <= 1.0);
            
            // Test resource efficiency analysis
            let efficiency_analysis = analytics_engine.analyze_resource_efficiency(&model).await
                .expect("Resource efficiency analysis should work");
            
            assert!(efficiency_analysis.performance_per_mb > 0.0);
            assert!(efficiency_analysis.predictions_per_cpu_second > 0.0);
            
            // Verify analytics integration with DAA decisions
            let enhanced_context = MarketContext {
                model_analytics: Some(value_assessment.clone()),
                efficiency_metrics: Some(efficiency_analysis.clone()),
                ..create_market_context()
            };
            
            let decision = coordinator.make_autonomous_decision(&enhanced_context).await
                .expect("DAA should work with analytics data");
            
            // Analytics should enhance confidence but preserve voting structure
            assert_eq!(decision.voting_weights.neural_weight, 0.6);
            assert_eq!(decision.voting_weights.strategy_weight, 0.4);
            
            // Higher value models should get higher confidence
            if overall_value > 0.8 {
                assert!(decision.confidence_score > 0.8,
                    "High value models should increase decision confidence");
            }
        }
    }

    /// Test 6: Model Checkpoint and Rollback System
    #[tokio::test]
    async fn test_model_checkpoint_rollback_system() {
        let mut training_engine = AutonomousTrainingEngine::new();
        let mut checkpoint_manager = ModelCheckpointManager::new();
        
        // Enable checkpointing
        training_engine.enable_model_checkpointing(true);
        checkpoint_manager.enable_automatic_checkpoints(true);
        
        // Create baseline model performance
        let baseline_performance = measure_model_performance(&training_engine).await;
        let checkpoint_id = checkpoint_manager.create_checkpoint("baseline").await
            .expect("Should create baseline checkpoint");
        
        // Simulate model improvement
        let improvement_data = create_high_quality_training_data();
        training_engine.train_with_data(&improvement_data).await
            .expect("Training with good data should work");
        
        let improved_performance = measure_model_performance(&training_engine).await;
        assert!(improved_performance.accuracy > baseline_performance.accuracy,
            "Model should improve with good training data");
        
        // Create checkpoint after improvement
        let improved_checkpoint = checkpoint_manager.create_checkpoint("improved").await
            .expect("Should create improved checkpoint");
        
        // Simulate model degradation
        let corrupted_data = create_corrupted_training_data();
        training_engine.train_with_data(&corrupted_data).await
            .expect("Training should not fail with bad data");
        
        let degraded_performance = measure_model_performance(&training_engine).await;
        
        // Verify degradation detection triggers rollback
        if degraded_performance.consecutive_failures > 5 {
            // Automatic rollback should occur
            let rollback_result = checkpoint_manager.rollback_to_checkpoint(&improved_checkpoint).await
                .expect("Rollback should work");
            
            assert!(rollback_result.success);
            assert_eq!(rollback_result.restored_checkpoint_id, improved_checkpoint);
            
            // Verify performance restored
            let restored_performance = measure_model_performance(&training_engine).await;
            assert!(restored_performance.accuracy >= improved_performance.accuracy * 0.95,
                "Rollback should restore most of the improved performance");
            
            // Verify thresholds still enforced after rollback
            assert_eq!(training_engine.get_accuracy_threshold(), 0.8);
            assert_eq!(training_engine.get_error_threshold(), 0.1);
        }
    }

    /// Test 7: Data Availability Tracking and Graceful Degradation
    #[tokio::test]
    async fn test_data_availability_tracking() {
        let mut availability_tracker = DataAvailabilityTracker::new();
        let mut coordinator = DAACoordinator::new();
        
        // Enable data availability tracking
        availability_tracker.enable_real_time_tracking(true);
        coordinator.enable_data_availability_awareness(true);
        
        // Test various data availability scenarios
        let availability_scenarios = vec![
            ("full_data", 1.0, vec!["price", "volume", "sentiment", "news"]),
            ("partial_data", 0.75, vec!["price", "volume", "sentiment"]),
            ("minimal_data", 0.5, vec!["price", "volume"]),
            ("price_only", 0.25, vec!["price"]),
        ];
        
        for (scenario_name, expected_completeness, available_types) in availability_scenarios {
            // Simulate data availability
            for data_type in &available_types {
                availability_tracker.mark_available(data_type, Instant::now()).await;
            }
            
            // Create market context with available data
            let context = create_market_context_with_data_types(available_types.clone());
            let availability = availability_tracker.assess_availability("AAPL", &context).await
                .expect("Should assess data availability");
            
            assert!((availability.completeness - expected_completeness).abs() < 0.1,
                "Data completeness should match expected for scenario: {}", scenario_name);
            
            // Test DAA decision with varying data availability
            let decision = coordinator.make_autonomous_decision(&context).await
                .expect("DAA should work with any data availability");
            
            // Verify graceful degradation
            match scenario_name {
                "full_data" => {
                    assert!(decision.confidence_score >= 0.85,
                        "Full data should give high confidence");
                }
                "partial_data" => {
                    assert!(decision.confidence_score >= 0.75,
                        "Partial data should give good confidence");
                }
                "minimal_data" => {
                    assert!(decision.confidence_score >= 0.65,
                        "Minimal data should still give reasonable confidence");
                }
                "price_only" => {
                    assert!(decision.confidence_score >= 0.55,
                        "Price-only data should still enable decisions");
                }
                _ => {}
            }
            
            // Voting structure should always be preserved
            assert_eq!(decision.voting_weights.neural_weight, 0.6);
            assert_eq!(decision.voting_weights.strategy_weight, 0.4);
            
            // Clear availability for next test
            availability_tracker.clear_availability().await;
        }
    }

    /// Test 8: Performance Enhancement Validation
    #[tokio::test]
    async fn test_performance_enhancement_validation() {
        let mut baseline_system = create_neural_trading_system_without_phase3();
        let mut enhanced_system = create_neural_trading_system_with_phase3();
        
        // Test performance comparison over time
        let test_scenarios = create_performance_test_scenarios();
        
        let mut baseline_results = Vec::new();
        let mut enhanced_results = Vec::new();
        
        for scenario in test_scenarios {
            // Baseline system (Phase 2)
            let baseline_start = Instant::now();
            let baseline_decision = baseline_system.make_trading_decision(&scenario).await
                .expect("Baseline system should work");
            let baseline_latency = baseline_start.elapsed();
            
            // Enhanced system (Phase 3)
            let enhanced_start = Instant::now();
            let enhanced_decision = enhanced_system.make_trading_decision(&scenario).await
                .expect("Enhanced system should work");
            let enhanced_latency = enhanced_start.elapsed();
            
            // Record results
            baseline_results.push(PerformanceResult {
                accuracy: baseline_decision.accuracy_estimate,
                confidence: baseline_decision.confidence_score,
                latency: baseline_latency,
                memory_usage: baseline_system.get_memory_usage(),
            });
            
            enhanced_results.push(PerformanceResult {
                accuracy: enhanced_decision.accuracy_estimate,
                confidence: enhanced_decision.confidence_score,
                latency: enhanced_latency,
                memory_usage: enhanced_system.get_memory_usage(),
            });
        }
        
        // Analyze overall performance improvements
        let baseline_avg_confidence = baseline_results.iter()
            .map(|r| r.confidence).sum::<f64>() / baseline_results.len() as f64;
        let enhanced_avg_confidence = enhanced_results.iter()
            .map(|r| r.confidence).sum::<f64>() / enhanced_results.len() as f64;
        
        // Phase 3 should improve confidence while maintaining other metrics
        assert!(enhanced_avg_confidence >= baseline_avg_confidence + 0.02,
            "Phase 3 should improve average confidence by at least 2%");
        
        // Latency should not significantly degrade
        let baseline_avg_latency = baseline_results.iter()
            .map(|r| r.latency.as_millis()).sum::<u128>() / baseline_results.len() as u128;
        let enhanced_avg_latency = enhanced_results.iter()
            .map(|r| r.latency.as_millis()).sum::<u128>() / enhanced_results.len() as u128;
        
        assert!(enhanced_avg_latency <= baseline_avg_latency * 2,
            "Phase 3 latency should not exceed 2x baseline");
        
        // Memory usage should stay within bounds
        let enhanced_max_memory = enhanced_results.iter()
            .map(|r| r.memory_usage).max().unwrap();
        
        assert!(enhanced_max_memory < 525_000_000,
            "Phase 3 memory usage should stay under 525MB");
    }
}

// Helper types and functions
#[derive(Clone)]
struct PerformanceResult {
    accuracy: f64,
    confidence: f64,
    latency: Duration,
    memory_usage: usize,
}

#[derive(Clone)]
struct ModelFeedback {
    prediction_accuracy: f64,
    market_regime: String,
    volatility_level: f64,
    confidence_level: f64,
}

fn create_test_data_packet(pattern: &str) -> DataPacket {
    DataPacket {
        channel: pattern.to_string(),
        timestamp: chrono::Utc::now(),
        data: json!({"test": "data"}),
        data_type: infer_data_type_from_pattern(pattern),
    }
}

fn infer_data_type_from_pattern(pattern: &str) -> String {
    if pattern.contains("sentiment") {
        "sentiment".to_string()
    } else if pattern.contains("news") {
        "news".to_string()
    } else if pattern.contains("economic") {
        "economic".to_string()
    } else {
        "price".to_string()
    }
}