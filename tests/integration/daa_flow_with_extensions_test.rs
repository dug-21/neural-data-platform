//! DAA Flow Integration Tests with Phase 3 Extensions
//!
//! Critical validation tests ensuring Phase 3 extensions preserve core DAA functionality:
//! - 60/40 voting ratio mathematically preserved in all decision flows
//! - Real-time training preserves accuracy thresholds and consensus mechanisms
//! - Model rollback with Byzantine consensus maintains 70% agreement requirements
//! - Memory budget compliance (<525MB) enforced throughout DAA operations
//! - Cross-component coordination works as integrated extensions, not replacements

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::test;

// Import core DAA components (must be preserved)
use autonomous_platform::integration::daa_coordinator::{
    DaaCoordinator, DaaConfig, AutonomousDecision, TradingAction, RiskAssessment
};
use autonomous_platform::daa::autonomous_training::{
    AutonomousTrainingEngine, TrainingTriggerConfig, PerformanceSnapshot
};
use autonomous_platform::neural::predictor::NeuralPredictor;
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::{TimeSeriesData, sector_mapper::SectorMapper};
use autonomous_platform::utils::market_hours::MarketHours;
use autonomous_platform::strategies::MarketContext;

// Import Phase 3 extensions (must integrate, not replace)
use autonomous_platform::daa::realtime_training_integration::{
    DAATrainingScheduler, CoordinationConfig
};
use autonomous_platform::neural::realtime_training::{
    RealtimeTrainingExtension, RealtimeTrainingConfig, ModelFeedback, FeedbackType
};
use autonomous_platform::neural::memory_optimized_predictor::MemoryOptimizedPredictor;

/// Test environment for DAA flow with Phase 3 extensions
pub struct DAAFlowTestEnvironment {
    pub base_coordinator: Arc<RwLock<DaaCoordinator>>,
    pub training_engine: Arc<RwLock<AutonomousTrainingEngine>>,
    pub training_scheduler: Arc<DAATrainingScheduler>,
    pub memory_optimized_predictor: Arc<MemoryOptimizedPredictor>,
    pub sector_mapper: Arc<SectorMapper>,
    pub market_hours: Arc<MarketHours>,
}

impl DAAFlowTestEnvironment {
    pub async fn new() -> Result<Self> {
        // Create core neural configuration (must maintain compatibility)
        let neural_config = NeuralConfig {
            memory_gb: 0.5, // <525MB budget enforcement
            models: vec!["MLP".to_string(), "LSTM".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8, // Must preserve DAA thresholds
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 60,
            max_retries: 3,
            error_threshold: 0.05, // Critical for Byzantine consensus
            lookback_window: 24,
        };

        // Create base neural predictor with memory optimization
        let base_predictor = Arc::new(NeuralPredictor::new(neural_config.clone()).await?);
        let memory_optimized_predictor = Arc::new(
            MemoryOptimizedPredictor::new(base_predictor, neural_config.clone()).await?
        );

        let sector_mapper = Arc::new(SectorMapper::new(Default::default())?);
        let market_hours = Arc::new(MarketHours::default());

        // Create base DAA coordinator (must preserve all functionality)
        let daa_config = DaaConfig {
            min_confidence: 0.6, // Critical for 60/40 voting
            max_risk_per_trade: 0.02,
            enabled: true,
            voting_threshold: 0.7, // 70% consensus for Byzantine fault tolerance
            ..Default::default()
        };

        let (decision_tx, _decision_rx) = tokio::sync::mpsc::channel(100);
        let base_coordinator = Arc::new(RwLock::new(DaaCoordinator::new(
            daa_config,
            memory_optimized_predictor.get_base_predictor().clone(),
            decision_tx,
            market_hours.clone(),
        )?));

        // Create autonomous training engine (core DAA component)
        let training_config = TrainingTriggerConfig {
            accuracy_threshold: 0.8, // Must match neural config
            error_rate_threshold: 0.05,
            performance_degradation_threshold: 0.1,
            min_training_interval_hours: 1,
            max_training_interval_hours: 24,
            emergency_training_threshold: 0.5,
            voting_consensus_threshold: 0.7, // 70% for Byzantine consensus
        };

        let training_engine = Arc::new(RwLock::new(
            AutonomousTrainingEngine::new(training_config.clone()).await?
        ));

        // Create Phase 3 real-time training extension (must preserve core functionality)
        let realtime_config = RealtimeTrainingConfig {
            min_learning_rate: 0.0001,
            max_learning_rate: 0.01,
            emergency_accuracy_threshold: 0.6, // Above min_confidence
            batch_size: 32,
            max_update_frequency_per_hour: 100,
            enable_safety_checks: true, // Critical for DAA preservation
        };

        let realtime_extension = Arc::new(RealtimeTrainingExtension::new(
            memory_optimized_predictor.get_vendor_predictor(),
            training_engine.clone(),
            realtime_config,
            training_config,
        ));

        // Create training scheduler for coordination (extension, not replacement)
        let coordination_config = CoordinationConfig {
            allow_concurrent_updates: false, // Conservative for DAA preservation
            batch_training_cooldown_minutes: 30,
            max_realtime_updates_before_batch: 50,
            emergency_batch_threshold: 0.6,
        };

        let training_scheduler = Arc::new(DAATrainingScheduler::new(
            training_engine.clone(),
            realtime_extension,
            coordination_config,
        ));

        // Set autonomous training in coordinator
        base_coordinator.write().await.set_autonomous_training(training_engine.clone());

        Ok(Self {
            base_coordinator,
            training_engine,
            training_scheduler,
            memory_optimized_predictor,
            sector_mapper,
            market_hours,
        })
    }

    /// Generate realistic market data for testing
    pub fn generate_test_data(&self, symbol: &str, size: usize) -> Vec<TimeSeriesData> {
        let mut data = Vec::with_capacity(size);
        let mut price = 100.0;

        for i in 0..size {
            let trend = (i as f64 * 0.01).sin() * 0.001;
            let noise = (i as f64 * 0.5).sin() * 0.005;
            price *= 1.0 + trend + noise;

            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 30.0 + (i as f64 % 40.0));
            indicators.insert("sma_20".to_string(), price * 0.98);
            indicators.insert("volume_ma".to_string(), 1000000.0);

            data.push(TimeSeriesData {
                symbol: symbol.to_string(),
                timestamp: Utc::now() + chrono::Duration::minutes(i as i64),
                open: price * 0.999,
                high: price * 1.002,
                low: price * 0.998,
                close: price,
                volume: vec![1000000.0 * (1.0 + (i as f64 * 0.1).sin() * 0.3)],
                volume_value: 1000000.0 * (1.0 + (i as f64 * 0.1).sin() * 0.3),
                indicators,
                source: Some("test".to_string()),
                entity: Some(symbol.to_string()),
                value: Some(price),
                metadata: None,
                values: vec![price],
                intervals: vec![i as i64],
                timestamps: vec![Utc::now() + chrono::Duration::minutes(i as i64)],
                metadata_map: HashMap::new(),
            });
        }

        data
    }

    /// Validate 60/40 voting ratio preservation
    pub async fn validate_voting_ratio(&self, decisions: &[(String, AutonomousDecision)]) -> Result<(f64, f64)> {
        let total_decisions = decisions.len() as f64;
        if total_decisions == 0.0 {
            return Err(anyhow::anyhow!("Cannot validate empty decision set"));
        }

        // Calculate confidence-weighted and equal-weighted contributions
        let mut confidence_weight_sum = 0.0;
        let mut equal_weight_sum = 0.0;

        for (_, decision) in decisions {
            confidence_weight_sum += decision.confidence * 0.6; // 60% confidence weighting
            equal_weight_sum += (1.0 / total_decisions) * 0.4; // 40% equal weighting
        }

        let total_weight = confidence_weight_sum + equal_weight_sum;
        let confidence_ratio = confidence_weight_sum / total_weight;
        let equal_ratio = equal_weight_sum / total_weight;

        // Validate 60/40 ratio within tolerance
        const TOLERANCE: f64 = 0.05; // 5% tolerance
        if (confidence_ratio - 0.6).abs() > TOLERANCE || (equal_ratio - 0.4).abs() > TOLERANCE {
            return Err(anyhow::anyhow!(
                "Voting ratio deviation: confidence={:.3}, equal={:.3} (expected 0.6/0.4 ±{:.3})",
                confidence_ratio, equal_ratio, TOLERANCE
            ));
        }

        Ok((confidence_ratio, equal_ratio))
    }

    /// Validate Byzantine consensus requirements
    pub async fn validate_byzantine_consensus(&self, decisions: &[(String, AutonomousDecision)]) -> Result<bool> {
        if decisions.len() < 3 {
            return Ok(true); // Byzantine consensus requires ≥3 nodes
        }

        // Calculate agreement threshold (70% consensus)
        let min_agreement = (decisions.len() as f64 * 0.7).ceil() as usize;
        
        // Group decisions by action type for consensus calculation
        let mut buy_count = 0;
        let mut sell_count = 0;
        let mut hold_count = 0;

        for (_, decision) in decisions {
            match &decision.action {
                TradingAction::Buy { .. } => buy_count += 1,
                TradingAction::Sell { .. } => sell_count += 1,
                TradingAction::Hold { .. } => hold_count += 1,
                TradingAction::AdjustPosition { .. } => hold_count += 1, // Conservative classification
            }
        }

        let max_agreement = buy_count.max(sell_count).max(hold_count);
        let consensus_achieved = max_agreement >= min_agreement;

        if !consensus_achieved {
            tracing::warn!(
                "Byzantine consensus not achieved: buy={}, sell={}, hold={}, required={}",
                buy_count, sell_count, hold_count, min_agreement
            );
        }

        Ok(consensus_achieved)
    }
}

#[tokio::test]
async fn test_complete_decision_flow_with_new_data() -> Result<()> {
    let env = DAAFlowTestEnvironment::new().await?;
    
    // Generate test market data
    let market_data = env.generate_test_data("AAPL", 100);
    let market_context = MarketContext {
        symbol: "AAPL".to_string(),
        current_price: 150.0,
        bid: 149.95,
        ask: 150.05,
        volume_24h: 50000000.0,
        volatility: 0.02,
        timestamp: Utc::now().timestamp(),
    };

    // Test complete flow: data ingestion → neural prediction → DAA decision
    let start_time = std::time::Instant::now();
    
    // Phase 1: Memory-optimized prediction (must preserve accuracy)
    let prediction = env.memory_optimized_predictor.predict(&market_data, 1, None).await?;
    assert!(!prediction.is_empty(), "Memory-optimized predictor should return predictions");
    assert!(prediction[0].confidence >= 0.0 && prediction[0].confidence <= 1.0);

    // Phase 2: DAA decision making (must preserve 60/40 voting)
    let decision = env.base_coordinator.write().await
        .make_decision(&market_context, None, &market_data).await?;
    
    assert!(decision.confidence >= env.base_coordinator.read().await.config.min_confidence);
    assert!(!decision.reasoning.is_empty(), "DAA decision should include reasoning");

    // Phase 3: Validate decision properties
    match &decision.action {
        TradingAction::Buy { size, .. } | TradingAction::Sell { size, .. } => {
            assert!(*size > 0.0 && *size <= env.base_coordinator.read().await.config.max_risk_per_trade);
        }
        TradingAction::Hold { .. } | TradingAction::AdjustPosition { .. } => {
            // Valid actions for uncertain markets
        }
    }

    let total_latency = start_time.elapsed();
    assert!(total_latency.as_millis() < 500, "Complete flow should execute in <500ms");

    // Validate memory usage
    let memory_usage = env.memory_optimized_predictor.get_memory_usage().await;
    assert!(memory_usage < 525 * 1024 * 1024, "Memory usage must stay under 525MB: {} bytes", memory_usage);

    Ok(())
}

#[tokio::test]
async fn test_real_time_training_preserves_thresholds() -> Result<()> {
    let env = DAAFlowTestEnvironment::new().await?;
    
    // Start real-time training coordination
    env.training_scheduler.start_coordination().await?;

    // Generate market data with degraded performance scenario
    let market_data = env.generate_test_data("AAPL", 50);
    
    // Create feedback indicating performance degradation
    let feedback = ModelFeedback {
        symbol: "AAPL".to_string(),
        model_id: "test_model".to_string(),
        accuracy: 0.65, // Below 0.8 threshold but above emergency (0.6)
        prediction_error: 0.08,
        confidence: 0.7,
        timestamp: Utc::now(),
        feedback_type: FeedbackType::Performance,
        actual_value: Some(152.0),
        predicted_value: 150.0,
    };

    // Send feedback through real-time system
    env.training_scheduler.get_realtime_extension().send_feedback(feedback.clone()).await?;
    
    // Allow processing time
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Test that DAA thresholds are preserved after real-time updates
    let market_context = MarketContext {
        symbol: "AAPL".to_string(),
        current_price: 152.0,
        bid: 151.95,
        ask: 152.05,
        volume_24h: 45000000.0,
        volatility: 0.025,
        timestamp: Utc::now().timestamp(),
    };

    let decision_after_update = env.base_coordinator.write().await
        .make_decision(&market_context, None, &market_data).await?;

    // Critical assertion: DAA thresholds must be preserved
    assert!(
        decision_after_update.confidence >= env.base_coordinator.read().await.config.min_confidence,
        "DAA min_confidence threshold must be preserved after real-time updates: {} < {}",
        decision_after_update.confidence,
        env.base_coordinator.read().await.config.min_confidence
    );

    // Validate that autonomous training thresholds are maintained
    let training_config = env.training_engine.read().await.get_config();
    assert_eq!(training_config.accuracy_threshold, 0.8, "Accuracy threshold must remain unchanged");
    assert_eq!(training_config.voting_consensus_threshold, 0.7, "Voting consensus must remain unchanged");

    // Check that memory budget is still enforced
    let memory_usage = env.memory_optimized_predictor.get_memory_usage().await;
    assert!(memory_usage < 525 * 1024 * 1024, "Memory budget violation after real-time training");

    Ok(())
}

#[tokio::test]
async fn test_model_rollback_with_byzantine_consensus() -> Result<()> {
    let env = DAAFlowTestEnvironment::new().await?;

    // Create scenario requiring model rollback
    let market_data = env.generate_test_data("AAPL", 80);
    
    // Create multiple decision scenarios for Byzantine consensus testing
    let mut decisions = Vec::new();
    
    // Scenario 1: Majority legitimate decisions (70%+ agreement required)
    for i in 0..5 {
        let decision = AutonomousDecision {
            timestamp: Utc::now(),
            action: TradingAction::Buy {
                symbol: "AAPL".to_string(),
                size: 0.02,
                stop_loss: None,
                take_profit: None,
            },
            confidence: 0.8 + (i as f64 * 0.02), // High confidence legitimate decisions
            risk_assessment: RiskAssessment {
                market_risk: 0.02,
                position_risk: 0.0,
                portfolio_risk: 0.01,
                volatility_adjusted_size: 0.02,
            },
            reasoning: vec![format!("Legitimate decision {}", i)],
            neural_consensus: HashMap::new(),
            adapted_parameters: None,
        };
        decisions.push((format!("legitimate_{}", i), decision));
    }

    // Scenario 2: Minority Byzantine failures (should not affect consensus)
    for i in 0..2 {
        let decision = AutonomousDecision {
            timestamp: Utc::now(),
            action: TradingAction::Sell {
                symbol: "AAPL".to_string(),
                size: 0.1, // Suspicious large size
                reason: "Byzantine failure".to_string(),
            },
            confidence: 0.2, // Low confidence indicating potential failure
            risk_assessment: RiskAssessment {
                market_risk: 0.1,
                position_risk: 0.05,
                portfolio_risk: 0.08,
                volatility_adjusted_size: 0.1,
            },
            reasoning: vec![format!("Byzantine failure {}", i)],
            neural_consensus: HashMap::new(),
            adapted_parameters: None,
        };
        decisions.push((format!("byzantine_{}", i), decision));
    }

    // Test 1: Validate 60/40 voting ratio preservation with Byzantine failures
    let (confidence_ratio, equal_ratio) = env.validate_voting_ratio(&decisions).await?;
    assert!((confidence_ratio - 0.6).abs() < 0.05, "60/40 voting ratio violated in Byzantine scenario");
    assert!((equal_ratio - 0.4).abs() < 0.05, "40% equal voting ratio violated in Byzantine scenario");

    // Test 2: Validate Byzantine consensus (70% agreement)
    let consensus_achieved = env.validate_byzantine_consensus(&decisions).await?;
    assert!(consensus_achieved, "Byzantine consensus should be achieved with 5 legitimate vs 2 byzantine failures");

    // Test 3: Model rollback trigger based on consensus failure
    let mut high_byzantine_decisions = Vec::new();
    
    // Create scenario with too many Byzantine failures (consensus should fail)
    for i in 0..2 {
        let decision = AutonomousDecision {
            timestamp: Utc::now(),
            action: TradingAction::Buy {
                symbol: "AAPL".to_string(),
                size: 0.02,
                stop_loss: None,
                take_profit: None,
            },
            confidence: 0.8,
            risk_assessment: RiskAssessment {
                market_risk: 0.02,
                position_risk: 0.0,
                portfolio_risk: 0.01,
                volatility_adjusted_size: 0.02,
            },
            reasoning: vec![format!("Legitimate decision {}", i)],
            neural_consensus: HashMap::new(),
            adapted_parameters: None,
        };
        high_byzantine_decisions.push((format!("legitimate_{}", i), decision));
    }

    // Add majority Byzantine failures
    for i in 0..5 {
        let decision = AutonomousDecision {
            timestamp: Utc::now(),
            action: TradingAction::Sell {
                symbol: "AAPL".to_string(),
                size: 0.15,
                reason: "Major Byzantine failure".to_string(),
            },
            confidence: 0.1,
            risk_assessment: RiskAssessment {
                market_risk: 0.15,
                position_risk: 0.1,
                portfolio_risk: 0.12,
                volatility_adjusted_size: 0.15,
            },
            reasoning: vec![format!("Major Byzantine failure {}", i)],
            neural_consensus: HashMap::new(),
            adapted_parameters: None,
        };
        high_byzantine_decisions.push((format!("major_byzantine_{}", i), decision));
    }

    // Test consensus failure triggers appropriate response
    let consensus_failed = !env.validate_byzantine_consensus(&high_byzantine_decisions).await?;
    assert!(consensus_failed, "Consensus should fail with majority Byzantine failures");

    // Test that system can still process individual decisions correctly
    let market_context = MarketContext {
        symbol: "AAPL".to_string(),
        current_price: 150.0,
        bid: 149.95,
        ask: 150.05,
        volume_24h: 50000000.0,
        volatility: 0.02,
        timestamp: Utc::now().timestamp(),
    };

    let individual_decision = env.base_coordinator.write().await
        .make_decision(&market_context, None, &market_data).await?;
    
    // Individual decisions should maintain quality thresholds
    assert!(individual_decision.confidence >= 0.6, "Individual decisions should maintain quality");
    
    Ok(())
}

#[tokio::test]
async fn test_memory_budget_compliance_during_daa_operations() -> Result<()> {
    let env = DAAFlowTestEnvironment::new().await?;

    // Start all systems for comprehensive memory testing
    env.training_scheduler.start_coordination().await?;

    // Test memory usage across multiple concurrent operations
    let symbols = vec!["AAPL", "MSFT", "GOOGL", "AMZN", "TSLA"];
    let mut memory_snapshots = Vec::new();

    for (i, symbol) in symbols.iter().enumerate() {
        let market_data = env.generate_test_data(symbol, 100);
        let market_context = MarketContext {
            symbol: symbol.to_string(),
            current_price: 150.0 + i as f64 * 10.0,
            bid: 149.95 + i as f64 * 10.0,
            ask: 150.05 + i as f64 * 10.0,
            volume_24h: 50000000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };

        // Memory snapshot before operation
        let memory_before = env.memory_optimized_predictor.get_memory_usage().await;

        // Perform full DAA flow
        let _prediction = env.memory_optimized_predictor.predict(&market_data, 1, None).await?;
        let _decision = env.base_coordinator.write().await
            .make_decision(&market_context, None, &market_data).await?;

        // Send real-time feedback
        let feedback = ModelFeedback {
            symbol: symbol.to_string(),
            model_id: "test_model".to_string(),
            accuracy: 0.75,
            prediction_error: 0.05,
            confidence: 0.8,
            timestamp: Utc::now(),
            feedback_type: FeedbackType::Routine,
            actual_value: Some(150.0 + i as f64 * 10.0),
            predicted_value: 149.0 + i as f64 * 10.0,
        };
        
        env.training_scheduler.get_realtime_extension().send_feedback(feedback).await?;

        // Memory snapshot after operation
        let memory_after = env.memory_optimized_predictor.get_memory_usage().await;
        
        memory_snapshots.push((symbol.to_string(), memory_before, memory_after));

        // Critical assertion: Memory budget compliance throughout operations
        assert!(memory_after < 525 * 1024 * 1024, 
            "Memory budget violation during {} operation: {} bytes > 525MB", 
            symbol, memory_after);

        println!("Symbol {}: Memory before={} MB, after={} MB", 
            symbol, 
            memory_before / 1024 / 1024, 
            memory_after / 1024 / 1024);
    }

    // Allow processing time for all async operations
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Final memory check after all operations
    let final_memory = env.memory_optimized_predictor.get_memory_usage().await;
    assert!(final_memory < 525 * 1024 * 1024, 
        "Final memory budget violation: {} bytes > 525MB", final_memory);

    // Test memory growth pattern (should not grow significantly with more symbols)
    let memory_growth = final_memory - memory_snapshots[0].1;
    let growth_per_symbol = memory_growth / symbols.len();
    
    println!("Memory growth analysis:");
    println!("  Total growth: {} MB", memory_growth / 1024 / 1024);
    println!("  Growth per symbol: {} MB", growth_per_symbol / 1024 / 1024);
    println!("  Final memory usage: {} MB", final_memory / 1024 / 1024);

    // Memory should scale sub-linearly with symbols (due to shared optimizations)
    assert!(growth_per_symbol < 50 * 1024 * 1024, 
        "Memory growth per symbol too high: {} MB > 50MB", growth_per_symbol / 1024 / 1024);

    Ok(())
}

#[tokio::test]
async fn test_cross_component_coordination_preservation() -> Result<()> {
    let env = DAAFlowTestEnvironment::new().await?;

    // Test that all components coordinate properly as extensions
    env.training_scheduler.start_coordination().await?;

    let market_data = env.generate_test_data("AAPL", 100);
    let market_context = MarketContext {
        symbol: "AAPL".to_string(),
        current_price: 150.0,
        bid: 149.95,
        ask: 150.05,
        volume_24h: 50000000.0,
        volatility: 0.02,
        timestamp: Utc::now().timestamp(),
    };

    // Test 1: Base DAA functionality preserved
    let base_decision = env.base_coordinator.write().await
        .make_decision(&market_context, None, &market_data).await?;
    
    assert!(base_decision.confidence >= 0.6, "Base DAA min_confidence preserved");
    assert!(!base_decision.reasoning.is_empty(), "Base DAA reasoning preserved");

    // Test 2: Memory optimization integration
    let optimized_prediction = env.memory_optimized_predictor.predict(&market_data, 1, None).await?;
    assert!(!optimized_prediction.is_empty(), "Memory optimization preserves prediction capability");
    assert!(optimized_prediction[0].confidence > 0.0, "Memory optimization preserves confidence");

    // Test 3: Real-time training coordination
    let training_status_before = env.training_scheduler.get_training_status().await;
    assert!(training_status_before.contains_key("batch_training_active"), "Training status accessible");

    let feedback = ModelFeedback {
        symbol: "AAPL".to_string(),
        model_id: "test_model".to_string(),
        accuracy: 0.75,
        prediction_error: 0.05,
        confidence: 0.8,
        timestamp: Utc::now(),
        feedback_type: FeedbackType::Performance,
        actual_value: Some(151.0),
        predicted_value: 150.0,
    };

    env.training_scheduler.get_realtime_extension().send_feedback(feedback).await?;
    
    // Allow coordination processing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Test 4: Post-coordination DAA functionality
    let post_coordination_decision = env.base_coordinator.write().await
        .make_decision(&market_context, None, &market_data).await?;

    // Critical assertions: All DAA properties preserved after cross-component operations
    assert!(post_coordination_decision.confidence >= 0.6, "DAA confidence threshold preserved");
    
    // Test voting ratio preservation across components
    let test_decisions = vec![
        ("base".to_string(), base_decision),
        ("post_coordination".to_string(), post_coordination_decision),
    ];
    
    let (confidence_ratio, equal_ratio) = env.validate_voting_ratio(&test_decisions).await?;
    assert!((confidence_ratio - 0.6).abs() < 0.05, "60/40 voting preserved across components");
    assert!((equal_ratio - 0.4).abs() < 0.05, "Equal voting ratio preserved across components");

    // Test 5: Memory budget maintained across all components
    let final_memory = env.memory_optimized_predictor.get_memory_usage().await;
    assert!(final_memory < 525 * 1024 * 1024, "Memory budget preserved across components");

    println!("Cross-component coordination test passed:");
    println!("  Base decision confidence: {:.3}", test_decisions[0].1.confidence);
    println!("  Post-coordination confidence: {:.3}", test_decisions[1].1.confidence);
    println!("  Voting ratios: {:.3}/{:.3}", confidence_ratio, equal_ratio);
    println!("  Final memory: {} MB", final_memory / 1024 / 1024);

    Ok(())
}