//! Cross-Component Extension Tests for Phase 3
//!
//! Validates that Phase 3 extensions work as integrated additions, not replacements:
//! - SharedFeatures + RealtimeTraining coordination preserves memory optimizations
//! - VendorPredictor + DAA coordination maintains prediction accuracy and consensus
//! - Memory optimization preservation across all component interactions
//! - Performance metrics preserved across extension integration points

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::test;

// Import core components
use autonomous_platform::neural::{
    VendorPredictor, PredictionResult, predictor::NeuralPredictor
};
use autonomous_platform::features::{
    shared_feature_extractor::SharedFeatureExtractor,
    training_features::TrainingFeatureExtractor,
};
use autonomous_platform::daa::{
    autonomous_training::AutonomousTrainingEngine,
    PerformanceSnapshot,
};
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::{
    TimeSeriesData, sector_mapper::{SectorMapper, SectorId}
};
use autonomous_platform::monitoring::{
    health::metrics::PerformanceMetrics,
    model_performance_tracker::ModelPerformanceTracker,
};

// Import Phase 3 extensions
use autonomous_platform::neural::{
    realtime_training::{
        RealtimeTrainingExtension, RealtimeTrainingConfig, ModelFeedback, FeedbackType
    },
    memory_optimized_predictor::MemoryOptimizedPredictor,
};
use autonomous_platform::daa::realtime_training_integration::DAATrainingScheduler;

/// Cross-component test environment for Phase 3 extensions
pub struct CrossComponentTestEnvironment {
    pub shared_feature_extractor: Arc<SharedFeatureExtractor>,
    pub training_feature_extractor: Arc<TrainingFeatureExtractor>,
    pub vendor_predictor: Arc<RwLock<VendorPredictor>>,
    pub memory_optimized_predictor: Arc<MemoryOptimizedPredictor>,
    pub autonomous_engine: Arc<RwLock<AutonomousTrainingEngine>>,
    pub realtime_extension: Arc<RealtimeTrainingExtension>,
    pub sector_mapper: Arc<SectorMapper>,
    pub performance_tracker: Arc<ModelPerformanceTracker>,
}

impl CrossComponentTestEnvironment {
    pub async fn new() -> Result<Self> {
        // Core neural configuration with memory constraints
        let neural_config = NeuralConfig {
            memory_gb: 0.5, // Enforce 525MB budget
            models: vec!["MLP".to_string(), "LSTM".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
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
            error_threshold: 0.05,
            lookback_window: 24,
        };

        // Create sector mapper for shared features
        let sector_mapper = Arc::new(SectorMapper::new(Default::default())?);
        
        // Create shared feature extractor (must preserve memory efficiency)
        let shared_feature_extractor = Arc::new(SharedFeatureExtractor::new(
            sector_mapper.clone(),
            neural_config.clone(),
        )?);

        // Create training feature extractor (extension, not replacement)
        let training_feature_extractor = Arc::new(TrainingFeatureExtractor::new(
            shared_feature_extractor.clone(),
            neural_config.clone(),
        )?);

        // Create performance tracker
        let performance_tracker = Arc::new(ModelPerformanceTracker::new());

        // Create base neural predictor
        let base_predictor = Arc::new(NeuralPredictor::new(neural_config.clone()).await?);
        
        // Create vendor predictor with shared components
        let vendor_predictor = Arc::new(RwLock::new(VendorPredictor::new(
            &neural_config,
            sector_mapper.clone(),
            performance_tracker.clone(),
        )?));

        // Create memory-optimized predictor (extension preserving functionality)
        let memory_optimized_predictor = Arc::new(MemoryOptimizedPredictor::new(
            base_predictor,
            neural_config.clone(),
        ).await?);

        // Create autonomous training engine
        let training_config = autonomous_platform::daa::autonomous_training::TrainingTriggerConfig {
            accuracy_threshold: 0.8,
            error_rate_threshold: 0.05,
            performance_degradation_threshold: 0.1,
            min_training_interval_hours: 1,
            max_training_interval_hours: 24,
            emergency_training_threshold: 0.5,
            voting_consensus_threshold: 0.7,
        };

        let autonomous_engine = Arc::new(RwLock::new(
            AutonomousTrainingEngine::new(training_config.clone()).await?
        ));

        // Create real-time training extension
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
            autonomous_engine.clone(),
            realtime_config,
            training_config,
        ));

        Ok(Self {
            shared_feature_extractor,
            training_feature_extractor,
            vendor_predictor,
            memory_optimized_predictor,
            autonomous_engine,
            realtime_extension,
            sector_mapper,
            performance_tracker,
        })
    }

    /// Generate test time series data for cross-component testing
    pub fn generate_sector_data(&self, sector: SectorId, symbols: &[&str], size: usize) -> HashMap<String, Vec<TimeSeriesData>> {
        let mut sector_data = HashMap::new();

        for symbol in symbols {
            let mut data = Vec::with_capacity(size);
            let mut price = 100.0 + (sector as u8 as f64) * 10.0; // Different base prices per sector

            for i in 0..size {
                let trend = (i as f64 * 0.01).sin() * 0.001;
                let sector_noise = (i as f64 * 0.3 + sector as u8 as f64).sin() * 0.005;
                price *= 1.0 + trend + sector_noise;

                let mut indicators = HashMap::new();
                indicators.insert("rsi".to_string(), 30.0 + (i as f64 % 40.0));
                indicators.insert("sma_20".to_string(), price * 0.98);
                indicators.insert("volume_ma".to_string(), 1000000.0);
                indicators.insert("sector_momentum".to_string(), sector_noise * 100.0);

                data.push(TimeSeriesData {
                    symbol: symbol.to_string(),
                    timestamp: Utc::now() + chrono::Duration::minutes(i as i64),
                    open: price * 0.999,
                    high: price * 1.002,
                    low: price * 0.998,
                    close: price,
                    volume: vec![1000000.0 * (1.0 + sector_noise * 5.0)],
                    volume_value: 1000000.0 * (1.0 + sector_noise * 5.0),
                    indicators,
                    source: Some("cross_component_test".to_string()),
                    entity: Some(symbol.to_string()),
                    value: Some(price),
                    metadata: Some(serde_json::json!({
                        "sector": sector.as_str(),
                        "test_type": "cross_component"
                    })),
                    values: vec![price],
                    intervals: vec![i as i64],
                    timestamps: vec![Utc::now() + chrono::Duration::minutes(i as i64)],
                    metadata_map: HashMap::new(),
                });
            }

            sector_data.insert(symbol.to_string(), data);
        }

        sector_data
    }

    /// Measure memory usage across all components
    pub async fn get_total_memory_usage(&self) -> usize {
        let shared_memory = self.shared_feature_extractor.get_memory_usage().await;
        let training_memory = self.training_feature_extractor.get_memory_usage().await;
        let vendor_memory = self.vendor_predictor.read().await.get_memory_usage().await;
        let optimized_memory = self.memory_optimized_predictor.get_memory_usage().await;
        
        shared_memory + training_memory + vendor_memory + optimized_memory
    }

    /// Validate performance preservation across components
    pub async fn validate_performance_preservation(&self, baseline: &PerformanceMetrics) -> Result<bool> {
        let current = self.performance_tracker.get_current_metrics().await;
        
        // Accuracy should not degrade by more than 5%
        let accuracy_preserved = current.accuracy >= baseline.accuracy * 0.95;
        
        // Latency should not increase by more than 20%
        let latency_preserved = current.latency_ms <= baseline.latency_ms * 1.2;
        
        // Memory should be within budget
        let memory_preserved = current.memory_usage_mb <= 525.0;

        Ok(accuracy_preserved && latency_preserved && memory_preserved)
    }
}

#[tokio::test]
async fn test_shared_features_with_real_time_training() -> Result<()> {
    let env = CrossComponentTestEnvironment::new().await?;

    // Generate test data for technology sector
    let tech_symbols = ["AAPL", "MSFT", "GOOGL"];
    let sector_data = env.generate_sector_data(SectorId::Technology, &tech_symbols, 100);

    // Test 1: Shared feature extraction preserves memory efficiency
    let memory_before_features = env.get_total_memory_usage().await;
    
    let mut extracted_features = HashMap::new();
    for (symbol, data) in &sector_data {
        let features = env.shared_feature_extractor
            .extract_features(data, &SectorId::Technology).await?;
        
        extracted_features.insert(symbol.clone(), features);
        
        // Verify feature consistency across symbols in same sector
        assert!(!extracted_features[symbol].is_empty(), "Features should be extracted for {}", symbol);
    }

    let memory_after_features = env.get_total_memory_usage().await;
    
    // Critical assertion: Shared features should scale with sectors, not symbols
    assert!(memory_after_features < 525 * 1024 * 1024, 
        "Memory usage exceeded budget after feature extraction: {} bytes", memory_after_features);
    
    let memory_per_symbol = (memory_after_features - memory_before_features) / tech_symbols.len();
    assert!(memory_per_symbol < 10 * 1024 * 1024, 
        "Memory per symbol too high: {} MB > 10MB", memory_per_symbol / 1024 / 1024);

    // Test 2: Real-time training integration with shared features
    env.realtime_extension.start_processing().await?;

    // Generate feedback for each symbol
    for (i, symbol) in tech_symbols.iter().enumerate() {
        let feedback = ModelFeedback {
            symbol: symbol.to_string(),
            model_id: "shared_model".to_string(),
            accuracy: 0.75 + (i as f64 * 0.05), // Varying accuracy
            prediction_error: 0.05 - (i as f64 * 0.01),
            confidence: 0.8,
            timestamp: Utc::now(),
            feedback_type: if i == 0 { FeedbackType::Performance } else { FeedbackType::Routine },
            actual_value: Some(150.0 + i as f64 * 5.0),
            predicted_value: 149.0 + i as f64 * 5.0,
        };

        env.realtime_extension.send_feedback(feedback).await?;
    }

    // Allow processing time
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    // Test 3: Verify shared features are preserved after real-time updates
    let updated_features = env.shared_feature_extractor
        .extract_features(&sector_data["AAPL"], &SectorId::Technology).await?;
    
    assert!(!updated_features.is_empty(), "Shared features should persist after real-time training");
    
    let final_memory = env.get_total_memory_usage().await;
    assert!(final_memory < 525 * 1024 * 1024, 
        "Memory budget violated after real-time training: {} bytes", final_memory);

    // Test 4: Training features integration maintains efficiency
    let training_features = env.training_feature_extractor
        .extract_training_features(&sector_data["AAPL"], "AAPL").await?;
    
    assert!(!training_features.is_empty(), "Training features should be extractable");
    
    let post_training_memory = env.get_total_memory_usage().await;
    assert!(post_training_memory < 525 * 1024 * 1024, 
        "Memory budget violated after training feature extraction: {} bytes", post_training_memory);

    println!("Shared features + real-time training test results:");
    println!("  Memory before features: {} MB", memory_before_features / 1024 / 1024);
    println!("  Memory after features: {} MB", memory_after_features / 1024 / 1024);
    println!("  Memory after real-time training: {} MB", final_memory / 1024 / 1024);
    println!("  Memory after training features: {} MB", post_training_memory / 1024 / 1024);
    println!("  Memory per symbol: {} MB", memory_per_symbol / 1024 / 1024);

    Ok(())
}

#[tokio::test]
async fn test_vendor_predictor_with_daa_coordination() -> Result<()> {
    let env = CrossComponentTestEnvironment::new().await?;

    // Create test data across multiple sectors for DAA coordination
    let tech_data = env.generate_sector_data(SectorId::Technology, &["AAPL", "MSFT"], 80);
    let finance_data = env.generate_sector_data(SectorId::Financial, &["JPM", "BAC"], 80);

    // Test 1: Baseline vendor predictor performance
    let baseline_start = std::time::Instant::now();
    
    let tech_prediction = env.vendor_predictor.read().await
        .predict(&tech_data["AAPL"], 1, None).await?;
    
    let baseline_latency = baseline_start.elapsed();
    
    assert!(!tech_prediction.is_empty(), "Vendor predictor should return predictions");
    assert!(tech_prediction[0].confidence > 0.0, "Predictions should have valid confidence");
    assert!(baseline_latency.as_millis() < 200, "Baseline prediction should be fast: {:?}", baseline_latency);

    // Test 2: Memory-optimized predictor coordination
    let optimized_prediction = env.memory_optimized_predictor
        .predict(&tech_data["AAPL"], 1, None).await?;
    
    assert!(!optimized_prediction.is_empty(), "Memory-optimized predictor should return predictions");
    
    // Verify prediction quality preservation
    let quality_diff = (optimized_prediction[0].confidence - tech_prediction[0].confidence).abs();
    assert!(quality_diff < 0.1, "Memory optimization should preserve prediction quality");

    // Test 3: DAA autonomous training coordination
    env.autonomous_engine.write().await.start_training().await?;
    
    // Create performance snapshot for DAA
    let performance_snapshot = PerformanceSnapshot {
        timestamp: Utc::now(),
        accuracy: 0.82, // Above threshold
        latency_ms: baseline_latency.as_millis() as f64,
        error_rate: 0.03, // Below threshold
        recent_predictions: 100,
        confidence: tech_prediction[0].confidence,
        price_error: 0.02,
        sharpe_ratio: 1.2,
        max_drawdown: 0.05,
        volatility: 0.02,
        model_agreement: 0.85,
        consecutive_failures: 0,
        trading_volume: vec![5000000.0],
        profit_loss: 125.0,
    };

    // Submit performance to autonomous engine
    env.autonomous_engine.write().await
        .update_performance(performance_snapshot.clone()).await?;

    // Test 4: Cross-sector prediction consistency
    let finance_prediction = env.vendor_predictor.read().await
        .predict(&finance_data["JPM"], 1, None).await?;
    
    assert!(!finance_prediction.is_empty(), "Finance predictions should be available");
    
    // Test 5: Real-time updates with DAA coordination
    env.realtime_extension.start_processing().await?;

    let coordination_feedback = ModelFeedback {
        symbol: "AAPL".to_string(),
        model_id: "daa_coordinated_model".to_string(),
        accuracy: 0.78, // Slight degradation to test coordination
        prediction_error: 0.06,
        confidence: 0.75,
        timestamp: Utc::now(),
        feedback_type: FeedbackType::Performance,
        actual_value: Some(152.0),
        predicted_value: 150.0,
    };

    env.realtime_extension.send_feedback(coordination_feedback).await?;
    
    // Allow coordination processing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Test 6: Post-coordination prediction quality
    let post_coordination_prediction = env.vendor_predictor.read().await
        .predict(&tech_data["AAPL"], 1, None).await?;
    
    assert!(!post_coordination_prediction.is_empty(), "Predictions should continue after coordination");
    assert!(post_coordination_prediction[0].confidence > 0.5, "Prediction confidence should remain reasonable");

    // Test 7: Memory and performance preservation
    let final_memory = env.get_total_memory_usage().await;
    assert!(final_memory < 525 * 1024 * 1024, 
        "Memory budget must be preserved through DAA coordination: {} bytes", final_memory);

    // Verify DAA thresholds preserved
    let training_status = env.autonomous_engine.read().await.get_training_status().await?;
    assert!(training_status.is_training_active || training_status.total_training_sessions > 0, 
        "DAA training functionality should be preserved");

    println!("Vendor predictor + DAA coordination test results:");
    println!("  Baseline prediction confidence: {:.3}", tech_prediction[0].confidence);
    println!("  Optimized prediction confidence: {:.3}", optimized_prediction[0].confidence);
    println!("  Post-coordination confidence: {:.3}", post_coordination_prediction[0].confidence);
    println!("  Memory usage: {} MB", final_memory / 1024 / 1024);
    println!("  Performance snapshot accuracy: {:.3}", performance_snapshot.accuracy);

    Ok(())
}

#[tokio::test]
async fn test_memory_optimization_preservation() -> Result<()> {
    let env = CrossComponentTestEnvironment::new().await?;

    // Test memory optimization across multiple component interactions
    let initial_memory = env.get_total_memory_usage().await;
    
    // Test 1: Multiple sector data processing
    let sectors_and_symbols = vec![
        (SectorId::Technology, vec!["AAPL", "MSFT", "GOOGL"]),
        (SectorId::Financial, vec!["JPM", "BAC", "WFC"]),
        (SectorId::Healthcare, vec!["JNJ", "PFE", "UNH"]),
    ];

    let mut all_predictions = Vec::new();
    let mut memory_snapshots = Vec::new();

    for (sector, symbols) in &sectors_and_symbols {
        let sector_data = env.generate_sector_data(*sector, symbols, 60);
        
        for symbol in symbols {
            let memory_before = env.get_total_memory_usage().await;
            
            // Shared feature extraction (should reuse across sector)
            let features = env.shared_feature_extractor
                .extract_features(&sector_data[*symbol], sector).await?;
            
            // Vendor prediction
            let prediction = env.vendor_predictor.read().await
                .predict(&sector_data[*symbol], 1, None).await?;
            
            // Memory-optimized prediction
            let optimized_prediction = env.memory_optimized_predictor
                .predict(&sector_data[*symbol], 1, None).await?;
            
            let memory_after = env.get_total_memory_usage().await;
            
            memory_snapshots.push((symbol.to_string(), memory_before, memory_after));
            all_predictions.push((symbol.to_string(), prediction, optimized_prediction));
            
            // Per-symbol memory usage should be minimal due to sharing
            let memory_delta = memory_after - memory_before;
            assert!(memory_delta < 20 * 1024 * 1024, 
                "Memory delta for {} too high: {} MB", symbol, memory_delta / 1024 / 1024);
        }
    }

    // Test 2: Memory scaling analysis
    let final_memory = env.get_total_memory_usage().await;
    let total_symbols = sectors_and_symbols.iter().map(|(_, symbols)| symbols.len()).sum::<usize>();
    let memory_per_symbol = (final_memory - initial_memory) / total_symbols;
    
    println!("Memory scaling analysis:");
    println!("  Initial memory: {} MB", initial_memory / 1024 / 1024);
    println!("  Final memory: {} MB", final_memory / 1024 / 1024);
    println!("  Total symbols processed: {}", total_symbols);
    println!("  Memory per symbol: {} MB", memory_per_symbol / 1024 / 1024);

    // Critical assertions for memory optimization preservation
    assert!(final_memory < 525 * 1024 * 1024, 
        "Total memory exceeded budget: {} MB > 525MB", final_memory / 1024 / 1024);
    
    assert!(memory_per_symbol < 50 * 1024 * 1024, 
        "Memory per symbol too high: {} MB > 50MB", memory_per_symbol / 1024 / 1024);

    // Test 3: Prediction quality across optimizations
    for (symbol, vendor_pred, optimized_pred) in &all_predictions {
        assert!(!vendor_pred.is_empty() && !optimized_pred.is_empty(), 
            "Both predictions should be available for {}", symbol);
        
        let quality_preservation = (vendor_pred[0].confidence - optimized_pred[0].confidence).abs() < 0.15;
        assert!(quality_preservation, 
            "Prediction quality not preserved for {}: vendor={:.3}, optimized={:.3}",
            symbol, vendor_pred[0].confidence, optimized_pred[0].confidence);
    }

    // Test 4: Real-time training memory preservation
    env.realtime_extension.start_processing().await?;
    
    // Send feedback for multiple symbols
    for (symbol, _, _) in all_predictions.iter().take(5) {
        let feedback = ModelFeedback {
            symbol: symbol.clone(),
            model_id: "memory_test_model".to_string(),
            accuracy: 0.8,
            prediction_error: 0.04,
            confidence: 0.8,
            timestamp: Utc::now(),
            feedback_type: FeedbackType::Routine,
            actual_value: Some(150.0),
            predicted_value: 149.5,
        };

        env.realtime_extension.send_feedback(feedback).await?;
    }

    // Allow processing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let post_training_memory = env.get_total_memory_usage().await;
    assert!(post_training_memory < 525 * 1024 * 1024, 
        "Memory budget violated after real-time training: {} MB", post_training_memory / 1024 / 1024);

    // Test 5: Memory leak detection
    let memory_growth = post_training_memory - final_memory;
    assert!(memory_growth < 100 * 1024 * 1024, 
        "Potential memory leak detected: {} MB growth", memory_growth / 1024 / 1024);

    println!("Memory optimization preservation test completed:");
    println!("  Memory growth during training: {} MB", memory_growth / 1024 / 1024);
    println!("  Total predictions generated: {}", all_predictions.len());
    println!("  Memory efficiency: {:.2} MB per prediction", 
        (final_memory as f64 / all_predictions.len() as f64) / 1024.0 / 1024.0);

    Ok(())
}

#[tokio::test]
async fn test_performance_metrics_preservation() -> Result<()> {
    let env = CrossComponentTestEnvironment::new().await?;

    // Establish baseline performance metrics
    let test_data = env.generate_sector_data(SectorId::Technology, &["AAPL"], 100);
    
    let baseline_start = std::time::Instant::now();
    let baseline_prediction = env.vendor_predictor.read().await
        .predict(&test_data["AAPL"], 1, None).await?;
    let baseline_latency = baseline_start.elapsed();

    let baseline_metrics = PerformanceMetrics {
        accuracy: baseline_prediction[0].confidence,
        latency_ms: baseline_latency.as_millis() as f64,
        memory_usage_mb: env.get_total_memory_usage().await as f64 / 1024.0 / 1024.0,
        throughput: 1.0 / baseline_latency.as_secs_f64(),
        error_rate: 0.05,
    };

    println!("Baseline metrics established:");
    println!("  Accuracy: {:.3}", baseline_metrics.accuracy);
    println!("  Latency: {:.1} ms", baseline_metrics.latency_ms);
    println!("  Memory: {:.1} MB", baseline_metrics.memory_usage_mb);
    println!("  Throughput: {:.1} predictions/sec", baseline_metrics.throughput);

    // Test 1: Performance preservation through memory optimization
    let optimized_start = std::time::Instant::now();
    let optimized_prediction = env.memory_optimized_predictor
        .predict(&test_data["AAPL"], 1, None).await?;
    let optimized_latency = optimized_start.elapsed();

    let optimized_metrics = PerformanceMetrics {
        accuracy: optimized_prediction[0].confidence,
        latency_ms: optimized_latency.as_millis() as f64,
        memory_usage_mb: env.get_total_memory_usage().await as f64 / 1024.0 / 1024.0,
        throughput: 1.0 / optimized_latency.as_secs_f64(),
        error_rate: 0.05,
    };

    // Verify performance preservation
    assert!(env.validate_performance_preservation(&baseline_metrics).await?, 
        "Performance not preserved through memory optimization");

    // Test 2: Performance preservation through real-time training
    env.realtime_extension.start_processing().await?;

    let feedback = ModelFeedback {
        symbol: "AAPL".to_string(),
        model_id: "performance_test_model".to_string(),
        accuracy: 0.85, // Good performance feedback
        prediction_error: 0.03,
        confidence: 0.85,
        timestamp: Utc::now(),
        feedback_type: FeedbackType::Performance,
        actual_value: Some(151.0),
        predicted_value: 150.5,
    };

    env.realtime_extension.send_feedback(feedback).await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let post_training_start = std::time::Instant::now();
    let post_training_prediction = env.vendor_predictor.read().await
        .predict(&test_data["AAPL"], 1, None).await?;
    let post_training_latency = post_training_start.elapsed();

    let post_training_metrics = PerformanceMetrics {
        accuracy: post_training_prediction[0].confidence,
        latency_ms: post_training_latency.as_millis() as f64,
        memory_usage_mb: env.get_total_memory_usage().await as f64 / 1024.0 / 1024.0,
        throughput: 1.0 / post_training_latency.as_secs_f64(),
        error_rate: 0.04, // Slight improvement expected
    };

    // Verify performance maintained or improved after training
    assert!(post_training_metrics.accuracy >= baseline_metrics.accuracy * 0.95,
        "Accuracy degraded too much: {:.3} vs {:.3}", 
        post_training_metrics.accuracy, baseline_metrics.accuracy);

    assert!(post_training_metrics.latency_ms <= baseline_metrics.latency_ms * 1.5,
        "Latency increased too much: {:.1}ms vs {:.1}ms",
        post_training_metrics.latency_ms, baseline_metrics.latency_ms);

    // Test 3: Stress test with multiple concurrent operations
    let stress_start = std::time::Instant::now();
    let mut stress_tasks = Vec::new();

    for i in 0..10 {
        let env_clone = env.vendor_predictor.clone();
        let data_clone = test_data["AAPL"].clone();
        
        let task = tokio::spawn(async move {
            let start = std::time::Instant::now();
            let result = env_clone.read().await.predict(&data_clone, 1, None).await;
            let duration = start.elapsed();
            (i, result, duration)
        });
        
        stress_tasks.push(task);
    }

    let stress_results = futures::future::join_all(stress_tasks).await;
    let stress_duration = stress_start.elapsed();

    let mut successful_predictions = 0;
    let mut total_latency = 0;
    
    for result in stress_results {
        let (_, prediction_result, latency) = result?;
        if prediction_result.is_ok() {
            successful_predictions += 1;
            total_latency += latency.as_millis() as u64;
        }
    }

    let avg_stress_latency = total_latency as f64 / successful_predictions as f64;
    let stress_throughput = successful_predictions as f64 / stress_duration.as_secs_f64();

    println!("Stress test results:");
    println!("  Successful predictions: {}/10", successful_predictions);
    println!("  Average latency: {:.1} ms", avg_stress_latency);
    println!("  Throughput: {:.1} predictions/sec", stress_throughput);

    // Performance should remain reasonable under stress
    assert!(successful_predictions >= 8, "Most predictions should succeed under stress");
    assert!(avg_stress_latency < baseline_metrics.latency_ms * 2.0,
        "Stress latency too high: {:.1}ms vs baseline {:.1}ms",
        avg_stress_latency, baseline_metrics.latency_ms);

    // Final memory check
    let final_memory = env.get_total_memory_usage().await;
    assert!(final_memory < 525 * 1024 * 1024,
        "Final memory check failed: {} MB > 525MB", final_memory / 1024 / 1024);

    println!("Performance metrics preservation test completed successfully");
    println!("Final metrics:");
    println!("  Optimized accuracy: {:.3}", optimized_metrics.accuracy);
    println!("  Post-training accuracy: {:.3}", post_training_metrics.accuracy);
    println!("  Final memory usage: {:.1} MB", final_memory as f64 / 1024.0 / 1024.0);

    Ok(())
}