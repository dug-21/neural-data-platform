//! Phase 3 End-to-End Workflow Integration Tests
//!
//! Validates complete trading cycles with Phase 3 extensions:
//! - Complete trading cycle from data ingestion to execution with extensions
//! - Data pipeline to decision flow with memory optimizations
//! - Multi-modal data integration preserving DAA autonomous capabilities
//! - Performance thresholds maintained throughout extended workflow
//! - Memory budget compliance during complex workflow operations

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::test;
use std::time::{Duration, Instant};

// Import core workflow components
use autonomous_platform::integration::{
    daa_coordinator::{DaaCoordinator, DaaConfig, AutonomousDecision, TradingAction},
    data_access::DataAccessLayer,
};
use autonomous_platform::strategies::{MarketContext, neural_enhanced::NeuralEnhancedStrategy};
use autonomous_platform::data::{
    TimeSeriesData, 
    sector_mapper::{SectorMapper, SectorId},
    data_converter::DataConverter,
};
use autonomous_platform::neural::{
    predictor::NeuralPredictor,
    vendor_predictor::VendorPredictor,
    memory_optimized_predictor::MemoryOptimizedPredictor,
    enhanced_predictor::EnhancedPredictor,
};
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::utils::market_hours::MarketHours;
use autonomous_platform::mcp::trading_tools::{MarketData, TradingDecision};

// Import Phase 3 extensions
use autonomous_platform::daa::{
    autonomous_training::AutonomousTrainingEngine,
    realtime_training_integration::DAATrainingScheduler,
};
use autonomous_platform::neural::realtime_training::{
    RealtimeTrainingExtension, RealtimeTrainingConfig, ModelFeedback
};
use autonomous_platform::features::{
    shared_feature_extractor::SharedFeatureExtractor,
    cross_asset::CrossAssetFeatureExtractor,
    market_microstructure::MarketMicrostructureAnalyzer,
};
use autonomous_platform::monitoring::health::{
    metrics::PerformanceMetrics,
    dashboard::HealthDashboard,
};

/// Complete Phase 3 workflow test environment
pub struct Phase3WorkflowEnvironment {
    pub daa_coordinator: Arc<RwLock<DaaCoordinator>>,
    pub neural_strategy: Arc<NeuralEnhancedStrategy>,
    pub data_access: Arc<DataAccessLayer>,
    pub data_converter: Arc<DataConverter>,
    pub memory_optimized_predictor: Arc<MemoryOptimizedPredictor>,
    pub enhanced_predictor: Arc<EnhancedPredictor>,
    pub training_scheduler: Arc<DAATrainingScheduler>,
    pub shared_features: Arc<SharedFeatureExtractor>,
    pub cross_asset_features: Arc<CrossAssetFeatureExtractor>,
    pub microstructure_analyzer: Arc<MarketMicrostructureAnalyzer>,
    pub health_dashboard: Arc<HealthDashboard>,
    pub sector_mapper: Arc<SectorMapper>,
    pub market_hours: Arc<MarketHours>,
}

impl Phase3WorkflowEnvironment {
    pub async fn new() -> Result<Self> {
        // Core configuration maintaining compatibility
        let neural_config = NeuralConfig {
            memory_gb: 0.5, // Enforce 525MB budget
            models: vec!["MLP".to_string(), "LSTM".to_string(), "TCN".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 15,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: true,
            model_timeout_seconds: 60,
            max_retries: 3,
            error_threshold: 0.05,
            lookback_window: 24,
        };

        // Create foundational components
        let sector_mapper = Arc::new(SectorMapper::new(Default::default())?);
        let market_hours = Arc::new(MarketHours::default());
        let data_converter = Arc::new(DataConverter::new(sector_mapper.clone())?);

        // Create core neural predictors
        let base_predictor = Arc::new(NeuralPredictor::new(neural_config.clone()).await?);
        let vendor_predictor = Arc::new(RwLock::new(VendorPredictor::new(
            &neural_config,
            sector_mapper.clone(),
            Arc::new(autonomous_platform::monitoring::model_performance_tracker::ModelPerformanceTracker::new()),
        )?));

        // Create Phase 3 enhanced predictors
        let memory_optimized_predictor = Arc::new(MemoryOptimizedPredictor::new(
            base_predictor.clone(),
            neural_config.clone(),
        ).await?);

        let enhanced_predictor = Arc::new(EnhancedPredictor::new(
            base_predictor,
            neural_config.clone(),
        ).await?);

        // Create feature extractors with memory optimization
        let shared_features = Arc::new(SharedFeatureExtractor::new(
            sector_mapper.clone(),
            neural_config.clone(),
        )?);

        let cross_asset_features = Arc::new(CrossAssetFeatureExtractor::new(
            shared_features.clone(),
            sector_mapper.clone(),
        )?);

        let microstructure_analyzer = Arc::new(MarketMicrostructureAnalyzer::new(
            neural_config.clone()
        )?);

        // Create DAA components with Phase 3 extensions
        let daa_config = DaaConfig {
            min_confidence: 0.6,
            max_risk_per_trade: 0.02,
            enabled: true,
            voting_threshold: 0.7,
            ..Default::default()
        };

        let (decision_tx, _decision_rx) = tokio::sync::mpsc::channel(100);
        let daa_coordinator = Arc::new(RwLock::new(DaaCoordinator::new(
            daa_config,
            enhanced_predictor.get_base_predictor().clone(),
            decision_tx,
            market_hours.clone(),
        )?));

        // Create autonomous training with real-time extensions
        let training_config = autonomous_platform::daa::autonomous_training::TrainingTriggerConfig {
            accuracy_threshold: 0.8,
            error_rate_threshold: 0.05,
            performance_degradation_threshold: 0.1,
            min_training_interval_hours: 1,
            max_training_interval_hours: 24,
            emergency_training_threshold: 0.5,
            voting_consensus_threshold: 0.7,
        };

        let training_engine = Arc::new(RwLock::new(
            AutonomousTrainingEngine::new(training_config.clone()).await?
        ));

        let realtime_config = RealtimeTrainingConfig {
            min_learning_rate: 0.0001,
            max_learning_rate: 0.01,
            emergency_accuracy_threshold: 0.6,
            batch_size: 32,
            max_update_frequency_per_hour: 100,
            enable_safety_checks: true,
        };

        let realtime_extension = Arc::new(RealtimeTrainingExtension::new(
            vendor_predictor.clone(),
            training_engine.clone(),
            realtime_config,
            training_config,
        ));

        let coordination_config = autonomous_platform::daa::realtime_training_integration::CoordinationConfig {
            allow_concurrent_updates: false,
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
        daa_coordinator.write().await.set_autonomous_training(training_engine);

        // Create strategy with all enhancements
        let neural_strategy = Arc::new(NeuralEnhancedStrategy::new(
            enhanced_predictor.clone(),
            daa_coordinator.clone(),
            shared_features.clone(),
        ).await?);

        // Create data access layer
        let data_access = Arc::new(DataAccessLayer::new(
            data_converter.clone(),
            sector_mapper.clone(),
        ).await?);

        // Create health monitoring
        let health_dashboard = Arc::new(HealthDashboard::new(
            neural_config.clone(),
        ).await?);

        Ok(Self {
            daa_coordinator,
            neural_strategy,
            data_access,
            data_converter,
            memory_optimized_predictor,
            enhanced_predictor,
            training_scheduler,
            shared_features,
            cross_asset_features,
            microstructure_analyzer,
            health_dashboard,
            sector_mapper,
            market_hours,
        })
    }

    /// Generate comprehensive multi-modal test data
    pub fn generate_multi_modal_data(&self, symbols: &[&str], size: usize) -> HashMap<String, MultiModalData> {
        let mut multi_modal_data = HashMap::new();

        for (i, symbol) in symbols.iter().enumerate() {
            let sector = match i % 3 {
                0 => SectorId::Technology,
                1 => SectorId::Financial,
                _ => SectorId::Healthcare,
            };

            let time_series = self.generate_time_series_data(symbol, size, sector);
            let market_data = self.generate_market_data(symbol, size);
            let microstructure_data = self.generate_microstructure_data(symbol, size);
            let cross_asset_data = self.generate_cross_asset_data(symbol, symbols, size);

            multi_modal_data.insert(symbol.to_string(), MultiModalData {
                time_series,
                market_data,
                microstructure_data,
                cross_asset_data,
                sector,
            });
        }

        multi_modal_data
    }

    fn generate_time_series_data(&self, symbol: &str, size: usize, sector: SectorId) -> Vec<TimeSeriesData> {
        let mut data = Vec::with_capacity(size);
        let mut price = 100.0 + (sector as u8 as f64) * 20.0;

        for i in 0..size {
            let trend = (i as f64 * 0.02).sin() * 0.002;
            let sector_correlation = (i as f64 * 0.1 + sector as u8 as f64).sin() * 0.003;
            let noise = (i as f64 * 0.7).sin() * 0.001;
            
            price *= 1.0 + trend + sector_correlation + noise;

            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 30.0 + (i as f64 % 40.0));
            indicators.insert("sma_20".to_string(), price * 0.98);
            indicators.insert("sma_50".to_string(), price * 0.96);
            indicators.insert("volume_ma".to_string(), 2000000.0);
            indicators.insert("bollinger_upper".to_string(), price * 1.02);
            indicators.insert("bollinger_lower".to_string(), price * 0.98);
            indicators.insert("sector_momentum".to_string(), sector_correlation * 100.0);

            data.push(TimeSeriesData {
                symbol: symbol.to_string(),
                timestamp: Utc::now() + chrono::Duration::minutes(i as i64),
                open: price * 0.9995,
                high: price * 1.0025,
                low: price * 0.9975,
                close: price,
                volume: vec![2000000.0 * (1.0 + trend * 10.0)],
                volume_value: 2000000.0 * (1.0 + trend * 10.0),
                indicators,
                source: Some("phase3_workflow_test".to_string()),
                entity: Some(symbol.to_string()),
                value: Some(price),
                metadata: Some(serde_json::json!({
                    "sector": sector.as_str(),
                    "test_type": "multi_modal",
                    "workflow_phase": "data_generation"
                })),
                values: vec![price],
                intervals: vec![i as i64],
                timestamps: vec![Utc::now() + chrono::Duration::minutes(i as i64)],
                metadata_map: HashMap::new(),
            });
        }

        data
    }

    fn generate_market_data(&self, symbol: &str, size: usize) -> Vec<MarketData> {
        let mut market_data = Vec::with_capacity(size);
        let mut price = 100.0;

        for i in 0..size {
            price *= 1.0 + (i as f64 * 0.1).sin() * 0.001;
            
            market_data.push(MarketData {
                timestamp: Utc::now() + chrono::Duration::minutes(i as i64),
                open: price * 0.999,
                high: price * 1.001,
                low: price * 0.999,
                close: price,
                volume: vec![1500000.0 + (i as f64 * 1000.0)],
            });
        }

        market_data
    }

    fn generate_microstructure_data(&self, symbol: &str, size: usize) -> Vec<MicrostructureData> {
        let mut microstructure_data = Vec::with_capacity(size);

        for i in 0..size {
            let bid_ask_spread = 0.01 + (i as f64 * 0.001).sin().abs() * 0.005;
            let order_flow_imbalance = (i as f64 * 0.3).sin() * 0.2;
            let trade_intensity = 50.0 + (i as f64 * 0.2).cos() * 20.0;

            microstructure_data.push(MicrostructureData {
                timestamp: Utc::now() + chrono::Duration::minutes(i as i64),
                bid_ask_spread,
                order_flow_imbalance,
                trade_intensity,
                market_depth: 1000.0 + (i as f64 * 100.0),
                price_impact: bid_ask_spread * 2.0,
            });
        }

        microstructure_data
    }

    fn generate_cross_asset_data(&self, symbol: &str, all_symbols: &[&str], size: usize) -> Vec<CrossAssetData> {
        let mut cross_asset_data = Vec::with_capacity(size);

        for i in 0..size {
            let mut correlations = HashMap::new();
            
            for other_symbol in all_symbols {
                if *other_symbol != symbol {
                    let correlation = 0.3 + (i as f64 * 0.05).sin() * 0.4;
                    correlations.insert(other_symbol.to_string(), correlation);
                }
            }

            cross_asset_data.push(CrossAssetData {
                timestamp: Utc::now() + chrono::Duration::minutes(i as i64),
                correlations,
                sector_beta: 1.0 + (i as f64 * 0.02).cos() * 0.3,
                market_beta: 1.0 + (i as f64 * 0.01).sin() * 0.2,
            });
        }

        cross_asset_data
    }

    /// Execute complete trading workflow with all Phase 3 extensions
    pub async fn execute_complete_workflow(&self, symbol: &str, multi_modal_data: &MultiModalData) -> Result<WorkflowResult> {
        let workflow_start = Instant::now();
        let mut stage_timings = HashMap::new();

        // Stage 1: Data Ingestion and Conversion
        let stage_start = Instant::now();
        let converted_data = self.data_converter.convert_to_time_series(&multi_modal_data.market_data).await?;
        stage_timings.insert("data_ingestion".to_string(), stage_start.elapsed().as_millis() as f64);

        // Stage 2: Feature Extraction (Shared + Cross-Asset + Microstructure)
        let stage_start = Instant::now();
        
        let shared_features = self.shared_features
            .extract_features(&multi_modal_data.time_series, &multi_modal_data.sector).await?;
        
        let cross_asset_features = self.cross_asset_features
            .extract_cross_asset_features(&multi_modal_data.cross_asset_data).await?;
        
        let microstructure_features = self.microstructure_analyzer
            .extract_features(&multi_modal_data.microstructure_data).await?;
        
        stage_timings.insert("feature_extraction".to_string(), stage_start.elapsed().as_millis() as f64);

        // Stage 3: Memory-Optimized Neural Prediction
        let stage_start = Instant::now();
        let memory_prediction = self.memory_optimized_predictor
            .predict(&multi_modal_data.time_series, 1, None).await?;
        stage_timings.insert("memory_prediction".to_string(), stage_start.elapsed().as_millis() as f64);

        // Stage 4: Enhanced Neural Prediction
        let stage_start = Instant::now();
        let enhanced_prediction = self.enhanced_predictor
            .predict(&multi_modal_data.time_series, 1, None).await?;
        stage_timings.insert("enhanced_prediction".to_string(), stage_start.elapsed().as_millis() as f64);

        // Stage 5: Market Context Creation
        let stage_start = Instant::now();
        let market_context = MarketContext {
            symbol: symbol.to_string(),
            current_price: multi_modal_data.time_series.last().unwrap().close,
            bid: multi_modal_data.time_series.last().unwrap().close * 0.9995,
            ask: multi_modal_data.time_series.last().unwrap().close * 1.0005,
            volume_24h: multi_modal_data.time_series.iter().map(|d| d.volume_value).sum(),
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };
        stage_timings.insert("market_context".to_string(), stage_start.elapsed().as_millis() as f64);

        // Stage 6: DAA Decision Making (preserving autonomous capabilities)
        let stage_start = Instant::now();
        let daa_decision = self.daa_coordinator.write().await
            .make_decision(&market_context, None, &multi_modal_data.time_series).await?;
        stage_timings.insert("daa_decision".to_string(), stage_start.elapsed().as_millis() as f64);

        // Stage 7: Neural Enhanced Strategy Execution
        let stage_start = Instant::now();
        let strategy_result = self.neural_strategy
            .execute_strategy(&market_context, &multi_modal_data.time_series).await?;
        stage_timings.insert("strategy_execution".to_string(), stage_start.elapsed().as_millis() as f64);

        // Stage 8: Health Monitoring and Validation
        let stage_start = Instant::now();
        let health_status = self.health_dashboard.get_system_health().await?;
        let memory_usage = self.memory_optimized_predictor.get_memory_usage().await;
        stage_timings.insert("health_monitoring".to_string(), stage_start.elapsed().as_millis() as f64);

        let total_execution_time = workflow_start.elapsed().as_millis() as f64;

        // Validate workflow success criteria
        let success = self.validate_workflow_success(
            &memory_prediction,
            &enhanced_prediction,
            &daa_decision,
            memory_usage,
        ).await?;

        Ok(WorkflowResult {
            symbol: symbol.to_string(),
            success,
            total_execution_time,
            stage_timings,
            memory_prediction: memory_prediction.into_iter().next(),
            enhanced_prediction: enhanced_prediction.into_iter().next(),
            daa_decision,
            strategy_result,
            health_status,
            memory_usage,
            feature_counts: FeatureCounts {
                shared: shared_features.len(),
                cross_asset: cross_asset_features.len(),
                microstructure: microstructure_features.len(),
            },
        })
    }

    async fn validate_workflow_success(
        &self,
        memory_prediction: &[autonomous_platform::neural::PredictionResult],
        enhanced_prediction: &[autonomous_platform::neural::PredictionResult],
        daa_decision: &AutonomousDecision,
        memory_usage: usize,
    ) -> Result<bool> {
        // Validation criteria for Phase 3 workflow success
        
        // 1. Predictions must be available and valid
        let predictions_valid = !memory_prediction.is_empty() 
            && !enhanced_prediction.is_empty()
            && memory_prediction[0].confidence > 0.0
            && enhanced_prediction[0].confidence > 0.0;

        // 2. DAA decision must meet quality thresholds
        let daa_valid = daa_decision.confidence >= 0.6
            && !daa_decision.reasoning.is_empty();

        // 3. Memory budget must be preserved
        let memory_valid = memory_usage < 525 * 1024 * 1024;

        // 4. Enhanced prediction should preserve or improve quality
        let quality_preserved = enhanced_prediction[0].confidence >= memory_prediction[0].confidence * 0.95;

        Ok(predictions_valid && daa_valid && memory_valid && quality_preserved)
    }
}

// Supporting data structures
#[derive(Debug, Clone)]
pub struct MultiModalData {
    pub time_series: Vec<TimeSeriesData>,
    pub market_data: Vec<MarketData>,
    pub microstructure_data: Vec<MicrostructureData>,
    pub cross_asset_data: Vec<CrossAssetData>,
    pub sector: SectorId,
}

#[derive(Debug, Clone)]
pub struct MicrostructureData {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub bid_ask_spread: f64,
    pub order_flow_imbalance: f64,
    pub trade_intensity: f64,
    pub market_depth: f64,
    pub price_impact: f64,
}

#[derive(Debug, Clone)]
pub struct CrossAssetData {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub correlations: HashMap<String, f64>,
    pub sector_beta: f64,
    pub market_beta: f64,
}

#[derive(Debug, Clone)]
pub struct WorkflowResult {
    pub symbol: String,
    pub success: bool,
    pub total_execution_time: f64,
    pub stage_timings: HashMap<String, f64>,
    pub memory_prediction: Option<autonomous_platform::neural::PredictionResult>,
    pub enhanced_prediction: Option<autonomous_platform::neural::PredictionResult>,
    pub daa_decision: AutonomousDecision,
    pub strategy_result: autonomous_platform::strategies::StrategyResult,
    pub health_status: autonomous_platform::monitoring::health::types::SystemHealth,
    pub memory_usage: usize,
    pub feature_counts: FeatureCounts,
}

#[derive(Debug, Clone)]
pub struct FeatureCounts {
    pub shared: usize,
    pub cross_asset: usize,
    pub microstructure: usize,
}

#[tokio::test]
async fn test_complete_trading_cycle_with_extensions() -> Result<()> {
    let env = Phase3WorkflowEnvironment::new().await?;

    // Start all Phase 3 extensions
    env.training_scheduler.start_coordination().await?;

    // Generate comprehensive multi-modal test data
    let symbols = ["AAPL", "MSFT", "JPM"];
    let multi_modal_data = env.generate_multi_modal_data(&symbols, 100);

    let mut workflow_results = Vec::new();

    // Execute complete workflow for each symbol
    for symbol in &symbols {
        let symbol_data = &multi_modal_data[*symbol];
        
        println!("Executing complete workflow for {}", symbol);
        let workflow_result = env.execute_complete_workflow(symbol, symbol_data).await?;
        
        // Validate workflow success
        assert!(workflow_result.success, "Workflow should succeed for {}", symbol);
        assert!(workflow_result.total_execution_time < 1000.0, 
            "Workflow should complete in <1s for {}: {:.1}ms", symbol, workflow_result.total_execution_time);

        // Validate all stages executed
        let expected_stages = vec![
            "data_ingestion", "feature_extraction", "memory_prediction", 
            "enhanced_prediction", "market_context", "daa_decision", 
            "strategy_execution", "health_monitoring"
        ];

        for stage in &expected_stages {
            assert!(workflow_result.stage_timings.contains_key(stage),
                "Missing stage timing for {}: {}", symbol, stage);
            assert!(workflow_result.stage_timings[stage] > 0.0,
                "Stage {} should have positive execution time for {}", stage, symbol);
        }

        // Validate memory budget compliance
        assert!(workflow_result.memory_usage < 525 * 1024 * 1024,
            "Memory budget exceeded for {}: {} MB > 525MB", 
            symbol, workflow_result.memory_usage / 1024 / 1024);

        // Validate prediction quality
        if let Some(memory_pred) = &workflow_result.memory_prediction {
            assert!(memory_pred.confidence > 0.0, "Memory prediction should have valid confidence");
        }

        if let Some(enhanced_pred) = &workflow_result.enhanced_prediction {
            assert!(enhanced_pred.confidence > 0.0, "Enhanced prediction should have valid confidence");
        }

        // Validate DAA decision quality
        assert!(workflow_result.daa_decision.confidence >= 0.6,
            "DAA decision should meet confidence threshold for {}: {:.3}", 
            symbol, workflow_result.daa_decision.confidence);

        // Validate feature extraction
        assert!(workflow_result.feature_counts.shared > 0, "Should extract shared features");
        assert!(workflow_result.feature_counts.cross_asset > 0, "Should extract cross-asset features");
        assert!(workflow_result.feature_counts.microstructure > 0, "Should extract microstructure features");

        workflow_results.push(workflow_result);
    }

    // Cross-workflow analysis
    let total_execution_time: f64 = workflow_results.iter()
        .map(|r| r.total_execution_time)
        .sum();
    let avg_execution_time = total_execution_time / workflow_results.len() as f64;

    let max_memory_usage = workflow_results.iter()
        .map(|r| r.memory_usage)
        .max()
        .unwrap();

    println!("Complete trading cycle test results:");
    println!("  Symbols processed: {}", symbols.len());
    println!("  Average execution time: {:.1} ms", avg_execution_time);
    println!("  Max memory usage: {} MB", max_memory_usage / 1024 / 1024);
    println!("  All workflows successful: {}", workflow_results.iter().all(|r| r.success));

    // Final validations
    assert!(avg_execution_time < 800.0, "Average execution time should be reasonable");
    assert!(max_memory_usage < 525 * 1024 * 1024, "Peak memory should respect budget");
    assert!(workflow_results.iter().all(|r| r.success), "All workflows should succeed");

    Ok(())
}

#[tokio::test]
async fn test_data_pipeline_to_decision_flow() -> Result<()> {
    let env = Phase3WorkflowEnvironment::new().await?;

    // Test data pipeline efficiency with Phase 3 optimizations
    let symbol = "AAPL";
    let multi_modal_data = env.generate_multi_modal_data(&[symbol], 150);
    let symbol_data = &multi_modal_data[symbol];

    // Phase 1: Data Pipeline Ingestion
    let pipeline_start = Instant::now();
    
    let converted_data = env.data_converter
        .convert_to_time_series(&symbol_data.market_data).await?;
    
    assert!(!converted_data.is_empty(), "Data conversion should produce results");
    assert_eq!(converted_data.len(), symbol_data.market_data.len(), 
        "Converted data should match input size");

    let conversion_time = pipeline_start.elapsed();

    // Phase 2: Feature Pipeline with Memory Optimization
    let feature_start = Instant::now();

    let shared_features = env.shared_features
        .extract_features(&symbol_data.time_series, &symbol_data.sector).await?;
    
    let cross_asset_features = env.cross_asset_features
        .extract_cross_asset_features(&symbol_data.cross_asset_data).await?;

    let microstructure_features = env.microstructure_analyzer
        .extract_features(&symbol_data.microstructure_data).await?;

    let feature_time = feature_start.elapsed();

    // Validate feature extraction efficiency
    assert!(!shared_features.is_empty(), "Shared features should be extracted");
    assert!(!cross_asset_features.is_empty(), "Cross-asset features should be extracted");
    assert!(!microstructure_features.is_empty(), "Microstructure features should be extracted");

    // Phase 3: Prediction Pipeline
    let prediction_start = Instant::now();

    let memory_prediction = env.memory_optimized_predictor
        .predict(&symbol_data.time_series, 3, None).await?;
    
    let enhanced_prediction = env.enhanced_predictor
        .predict(&symbol_data.time_series, 3, None).await?;

    let prediction_time = prediction_start.elapsed();

    // Validate prediction pipeline
    assert_eq!(memory_prediction.len(), 3, "Should return requested number of predictions");
    assert_eq!(enhanced_prediction.len(), 3, "Enhanced predictor should return same count");

    // Phase 4: Decision Pipeline with DAA
    let decision_start = Instant::now();

    let market_context = MarketContext {
        symbol: symbol.to_string(),
        current_price: symbol_data.time_series.last().unwrap().close,
        bid: symbol_data.time_series.last().unwrap().close * 0.9995,
        ask: symbol_data.time_series.last().unwrap().close * 1.0005,
        volume_24h: symbol_data.time_series.iter().map(|d| d.volume_value).sum(),
        volatility: 0.025,
        timestamp: Utc::now().timestamp(),
    };

    let daa_decision = env.daa_coordinator.write().await
        .make_decision(&market_context, None, &symbol_data.time_series).await?;

    let decision_time = decision_start.elapsed();

    // Phase 5: Strategy Execution Pipeline
    let strategy_start = Instant::now();

    let strategy_result = env.neural_strategy
        .execute_strategy(&market_context, &symbol_data.time_series).await?;

    let strategy_time = strategy_start.elapsed();

    // Pipeline Performance Analysis
    let total_pipeline_time = conversion_time + feature_time + prediction_time + decision_time + strategy_time;

    println!("Data pipeline to decision flow analysis:");
    println!("  Data conversion: {:?}", conversion_time);
    println!("  Feature extraction: {:?}", feature_time);
    println!("  Prediction generation: {:?}", prediction_time);
    println!("  DAA decision making: {:?}", decision_time);
    println!("  Strategy execution: {:?}", strategy_time);
    println!("  Total pipeline time: {:?}", total_pipeline_time);

    // Performance validations
    assert!(conversion_time.as_millis() < 100, "Data conversion should be fast");
    assert!(feature_time.as_millis() < 200, "Feature extraction should be efficient");
    assert!(prediction_time.as_millis() < 300, "Prediction should be fast with optimizations");
    assert!(decision_time.as_millis() < 150, "DAA decisions should be fast");
    assert!(strategy_time.as_millis() < 100, "Strategy execution should be efficient");
    assert!(total_pipeline_time.as_millis() < 800, "Total pipeline should complete quickly");

    // Quality validations
    assert!(daa_decision.confidence >= 0.6, "DAA decision should meet quality threshold");
    
    match &daa_decision.action {
        TradingAction::Buy { size, .. } | TradingAction::Sell { size, .. } => {
            assert!(*size > 0.0 && *size <= 0.02, "Position size should be reasonable");
        }
        TradingAction::Hold { .. } | TradingAction::AdjustPosition { .. } => {
            // Valid actions for uncertain conditions
        }
    }

    // Memory efficiency validation
    let pipeline_memory = env.memory_optimized_predictor.get_memory_usage().await;
    assert!(pipeline_memory < 525 * 1024 * 1024, 
        "Pipeline memory usage should respect budget: {} MB", pipeline_memory / 1024 / 1024);

    Ok(())
}

#[tokio::test]
async fn test_multi_modal_data_integration() -> Result<()> {
    let env = Phase3WorkflowEnvironment::new().await?;

    // Test integration of multiple data modalities
    let symbols = ["AAPL", "MSFT", "JPM", "JNJ"];
    let multi_modal_data = env.generate_multi_modal_data(&symbols, 80);

    // Start real-time training for enhanced integration
    env.training_scheduler.start_coordination().await?;

    let mut integration_results = Vec::new();

    for symbol in &symbols {
        let symbol_data = &multi_modal_data[*symbol];
        
        // Test 1: Time Series + Market Data Integration
        let time_series_prediction = env.memory_optimized_predictor
            .predict(&symbol_data.time_series, 1, None).await?;

        let market_data_converted = env.data_converter
            .convert_to_time_series(&symbol_data.market_data).await?;

        // Validate data consistency
        assert!(time_series_prediction[0].confidence > 0.0, 
            "Time series prediction should be valid for {}", symbol);

        // Test 2: Microstructure Data Integration
        let microstructure_features = env.microstructure_analyzer
            .extract_features(&symbol_data.microstructure_data).await?;

        assert!(!microstructure_features.is_empty(), 
            "Microstructure features should be extracted for {}", symbol);

        // Test 3: Cross-Asset Data Integration
        let cross_asset_features = env.cross_asset_features
            .extract_cross_asset_features(&symbol_data.cross_asset_data).await?;

        assert!(!cross_asset_features.is_empty(),
            "Cross-asset features should be extracted for {}", symbol);

        // Test 4: Integrated Prediction with All Modalities
        let enhanced_prediction = env.enhanced_predictor
            .predict(&symbol_data.time_series, 1, None).await?;

        // Enhanced prediction should leverage all data modalities
        let quality_improvement = enhanced_prediction[0].confidence - time_series_prediction[0].confidence;
        
        println!("Multi-modal integration for {}:", symbol);
        println!("  Time series confidence: {:.3}", time_series_prediction[0].confidence);
        println!("  Enhanced confidence: {:.3}", enhanced_prediction[0].confidence);
        println!("  Quality improvement: {:.3}", quality_improvement);
        println!("  Microstructure features: {}", microstructure_features.len());
        println!("  Cross-asset features: {}", cross_asset_features.len());

        // Test 5: DAA Decision with Multi-Modal Input
        let market_context = MarketContext {
            symbol: symbol.to_string(),
            current_price: symbol_data.time_series.last().unwrap().close,
            bid: symbol_data.time_series.last().unwrap().close * 0.9995,
            ask: symbol_data.time_series.last().unwrap().close * 1.0005,
            volume_24h: symbol_data.time_series.iter().map(|d| d.volume_value).sum(),
            volatility: symbol_data.microstructure_data.last().unwrap().bid_ask_spread * 10.0,
            timestamp: Utc::now().timestamp(),
        };

        let multi_modal_decision = env.daa_coordinator.write().await
            .make_decision(&market_context, None, &symbol_data.time_series).await?;

        // Validate multi-modal decision quality
        assert!(multi_modal_decision.confidence >= 0.6,
            "Multi-modal DAA decision should meet threshold for {}: {:.3}", 
            symbol, multi_modal_decision.confidence);

        // Test 6: Real-time Feedback Integration
        let feedback = autonomous_platform::neural::realtime_training::ModelFeedback {
            symbol: symbol.to_string(),
            model_id: "multi_modal_model".to_string(),
            accuracy: enhanced_prediction[0].confidence,
            prediction_error: 0.03,
            confidence: enhanced_prediction[0].confidence,
            timestamp: Utc::now(),
            feedback_type: autonomous_platform::neural::realtime_training::FeedbackType::Performance,
            actual_value: Some(symbol_data.time_series.last().unwrap().close * 1.01),
            predicted_value: enhanced_prediction[0].value,
        };

        env.training_scheduler.get_realtime_extension().send_feedback(feedback).await?;

        integration_results.push(IntegrationResult {
            symbol: symbol.to_string(),
            time_series_confidence: time_series_prediction[0].confidence,
            enhanced_confidence: enhanced_prediction[0].confidence,
            quality_improvement,
            daa_confidence: multi_modal_decision.confidence,
            microstructure_feature_count: microstructure_features.len(),
            cross_asset_feature_count: cross_asset_features.len(),
        });
    }

    // Allow real-time processing
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Cross-symbol analysis
    let avg_quality_improvement: f64 = integration_results.iter()
        .map(|r| r.quality_improvement)
        .sum::<f64>() / integration_results.len() as f64;

    let avg_daa_confidence: f64 = integration_results.iter()
        .map(|r| r.daa_confidence)
        .sum::<f64>() / integration_results.len() as f64;

    println!("Multi-modal integration summary:");
    println!("  Symbols processed: {}", symbols.len());
    println!("  Average quality improvement: {:.3}", avg_quality_improvement);
    println!("  Average DAA confidence: {:.3}", avg_daa_confidence);

    // Final validations
    assert!(avg_quality_improvement >= -0.05, 
        "Multi-modal integration should not significantly degrade quality");
    assert!(avg_daa_confidence >= 0.6, 
        "Average DAA confidence should meet threshold");

    // Memory budget validation
    let final_memory = env.memory_optimized_predictor.get_memory_usage().await;
    assert!(final_memory < 525 * 1024 * 1024,
        "Memory budget preserved during multi-modal integration: {} MB", final_memory / 1024 / 1024);

    // Validate autonomous training preservation
    let training_status = env.training_scheduler.get_training_status().await;
    assert!(training_status.contains_key("batch_training_active"),
        "Autonomous training capabilities should be preserved");

    Ok(())
}

#[derive(Debug)]
struct IntegrationResult {
    symbol: String,
    time_series_confidence: f64,
    enhanced_confidence: f64,
    quality_improvement: f64,
    daa_confidence: f64,
    microstructure_feature_count: usize,
    cross_asset_feature_count: usize,
}