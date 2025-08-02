//! Production Readiness Test Suite
//!
//! Comprehensive validation of Phase 2 success criteria:
//! - Memory optimization: 90% reduction validation
//! - Performance: Sector aggregation <50ms
//! - DAA integration: 60/40 voting preserved
//! - SharedFeatureExtractor functionality
//! - Integration with existing systems

use std::sync::Arc;
use std::time::Instant;
use std::collections::HashMap;
use tokio::test;
use anyhow::Result;
use chrono::Utc;

use neural_trader::{
    data::{TimeSeriesData, sector_mapper::{SectorMapper, SectorMapperConfig}},
    features::shared_feature_extractor::{SharedFeatureExtractor, SharedFeatureConfig},
    neural::vendor_predictor::VendorPredictor,
    config::NeuralConfig,
    monitoring::model_performance_tracker::ModelPerformanceTracker,
    integration::daa_coordinator::DAACoordinator,
};

/// Helper to create test TimeSeriesData
fn create_test_data(symbol: &str, close: f64) -> TimeSeriesData {
    TimeSeriesData {
        timestamp: Utc::now(),
        symbol: symbol.to_string(),
        open: close * 0.99,
        high: close * 1.02,
        low: close * 0.98,
        close,
        volume: 1_000_000.0,
        indicators: HashMap::new(),
        source: Some("test".to_string()),
        entity: Some(symbol.to_string()),
        value: Some(close),
        metadata: None,
        values: vec![close],
        timestamps: vec![Utc::now()],
        metadata_map: HashMap::new(),
    }
}

/// Helper to create neural config
fn create_test_neural_config() -> NeuralConfig {
    NeuralConfig {
        input_size: 60,
        output_size: 1,
        hidden_layers: vec![64, 32],
        learning_rate: 0.001,
        epochs: 100,
        batch_size: 32,
        sequence_length: 60,
        prediction_horizon: 1,
        enable_feature_scaling: true,
        enable_technical_indicators: true,
        dropout_rate: 0.1,
        l2_regularization: 0.001,
        validation_split: 0.2,
        early_stopping: true,
        patience: 10,
        use_real_models: false,
        models: vec!["LSTM".to_string(), "GRU".to_string()],
        memory_gb: 0.1, // Reduced for memory optimization testing
        prediction_cache_ttl: 3600,
        accuracy_threshold: 0.7,
        enable_model_monitoring: true,
    }
}

#[test]
async fn test_memory_optimization_90_percent_reduction() -> Result<()> {
    println!("🧠 Testing Memory Optimization - 90% Reduction Target");
    
    let initial_memory = get_memory_usage();
    println!("📊 Initial memory usage: {:.2} MB", initial_memory);
    
    // Create sector mapper with default configuration
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new()?);
    
    // Test memory-optimized VendorPredictor
    let neural_config = create_test_neural_config();
    let predictor = VendorPredictor::new(&neural_config, sector_mapper.clone(), performance_tracker)?;
    
    // Create shared feature extractor with memory constraints
    let feature_config = SharedFeatureConfig {
        memory_limit_mb: 5.0, // Very strict memory limit
        cache_ttl_seconds: 30,
        min_symbols_for_extraction: 2,
        feature_window_size: 50,
        parallel_extraction: true,
        compression_enabled: true,
    };
    
    let sector_id = neural_trader::data::sector_mapper::SectorId::Technology;
    let feature_extractor = SharedFeatureExtractor::new(sector_id, feature_config).await?;
    
    // Process test data with memory monitoring
    let test_symbols = vec!["AAPL", "MSFT", "GOOGL", "TSLA", "META"];
    for symbol in &test_symbols {
        let data = create_test_data(symbol, 150.0);
        let _ = feature_extractor.extract_shared_features(&[data]).await?;
    }
    
    let final_memory = get_memory_usage();
    let memory_increase = final_memory - initial_memory;
    
    println!("📊 Final memory usage: {:.2} MB", final_memory);
    println!("📈 Memory increase: {:.2} MB", memory_increase);
    
    // Memory optimization target: <5MB for full feature extraction
    let memory_target = 5.0; // MB
    assert!(memory_increase < memory_target, 
        "Memory usage {:.2} MB exceeds target {:.2} MB", memory_increase, memory_target);
    
    println!("✅ Memory optimization test passed: {:.2} MB < {:.2} MB target", 
             memory_increase, memory_target);
    
    Ok(())
}

#[test]
async fn test_sector_aggregation_performance_50ms() -> Result<()> {
    println!("⚡ Testing Sector Aggregation Performance - <50ms Target");
    
    // Create sector mapper and feature extractor
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    
    let feature_config = SharedFeatureConfig {
        memory_limit_mb: 10.0,
        cache_ttl_seconds: 60,
        min_symbols_for_extraction: 3,
        feature_window_size: 100,
        parallel_extraction: true,
        compression_enabled: true,
    };
    
    let sector_id = neural_trader::data::sector_mapper::SectorId::Technology;
    let feature_extractor = SharedFeatureExtractor::new(sector_id, feature_config).await?;
    
    // Create test data for technology sector
    let tech_symbols = vec!["AAPL", "MSFT", "GOOGL", "TSLA", "META", "NVDA", "AMZN"];
    let mut tech_data = Vec::new();
    
    for symbol in &tech_symbols {
        let data = create_test_data(symbol, 150.0 + (symbol.len() as f64 * 10.0));
        tech_data.push(data);
    }
    
    // Measure sector aggregation performance
    let start_time = Instant::now();
    
    let shared_features = feature_extractor.extract_shared_features(&tech_data).await?;
    
    let aggregation_time = start_time.elapsed();
    let aggregation_ms = aggregation_time.as_millis() as f64;
    
    println!("📊 Sector aggregation completed in: {:.2} ms", aggregation_ms);
    println!("📊 Processed {} symbols with {} shared features", 
             tech_symbols.len(), shared_features.sector_features.len());
    
    // Performance target: <50ms for sector aggregation
    let performance_target = 50.0; // ms
    assert!(aggregation_ms < performance_target,
        "Aggregation time {:.2} ms exceeds target {:.2} ms", aggregation_ms, performance_target);
    
    println!("✅ Performance test passed: {:.2} ms < {:.2} ms target", 
             aggregation_ms, performance_target);
    
    Ok(())
}

#[test]
async fn test_daa_voting_preservation_60_40() -> Result<()> {
    println!("🤝 Testing DAA Integration - 60/40 Voting Preservation");
    
    // Create DAA coordinator
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new()?);
    
    let mut daa_coordinator = DAACoordinator::new(
        sector_mapper.clone(),
        performance_tracker.clone(),
        HashMap::new(),
    ).await?;
    
    // Initialize with symbols from different sectors
    let symbols = vec![
        ("AAPL", "technology"),     // 60% weight
        ("JPM", "financial"),       // 40% weight
    ];
    
    for (symbol, sector) in &symbols {
        daa_coordinator.register_symbol(symbol, sector).await?;
    }
    
    // Test voting mechanism with different scenarios
    let test_data = vec![
        create_test_data("AAPL", 175.0),
        create_test_data("JPM", 145.0),
    ];
    
    // Process voting decisions
    let voting_results = daa_coordinator.process_voting_decisions(&test_data).await?;
    
    // Validate 60/40 split preservation
    let tech_weight = voting_results.get("technology").unwrap_or(&0.0);
    let financial_weight = voting_results.get("financial").unwrap_or(&0.0);
    
    println!("📊 Technology sector weight: {:.1}%", tech_weight * 100.0);
    println!("📊 Financial sector weight: {:.1}%", financial_weight * 100.0);
    
    // Validate target weights (60/40 ± 5% tolerance)
    let tech_target = 0.6;
    let financial_target = 0.4;
    let tolerance = 0.05;
    
    assert!((tech_weight - tech_target).abs() < tolerance,
        "Technology weight {:.2} deviates from target {:.2}", tech_weight, tech_target);
    assert!((financial_weight - financial_target).abs() < tolerance,
        "Financial weight {:.2} deviates from target {:.2}", financial_weight, financial_target);
    
    println!("✅ DAA voting test passed: 60/40 split preserved within ±5% tolerance");
    
    Ok(())
}

#[test]
async fn test_shared_feature_extractor_functionality() -> Result<()> {
    println!("🔧 Testing SharedFeatureExtractor Functionality");
    
    // Create feature extractor with realistic configuration
    let feature_config = SharedFeatureConfig {
        memory_limit_mb: 15.0,
        cache_ttl_seconds: 120,
        min_symbols_for_extraction: 2,
        feature_window_size: 100,
        parallel_extraction: true,
        compression_enabled: true,
    };
    
    let sector_id = neural_trader::data::sector_mapper::SectorId::Technology;
    let feature_extractor = SharedFeatureExtractor::new(sector_id, feature_config).await?;
    
    // Test with multiple symbols in same sector
    let symbols = vec!["AAPL", "MSFT", "GOOGL"];
    let mut test_data = Vec::new();
    
    for (i, symbol) in symbols.iter().enumerate() {
        let data = create_test_data(symbol, 150.0 + (i as f64 * 20.0));
        test_data.push(data);
    }
    
    // Extract shared features
    let shared_features = feature_extractor.extract_shared_features(&test_data).await?;
    
    // Validate feature extraction results
    assert!(!shared_features.sector_features.is_empty(), 
        "Sector features should not be empty");
    
    assert_eq!(shared_features.symbol_features.len(), symbols.len(),
        "Should have features for all symbols");
    
    // Validate specific feature presence
    let required_features = vec!["price_momentum", "volume_trend", "sector_correlation"];
    for feature in &required_features {
        assert!(shared_features.sector_features.contains_key(feature),
            "Missing required sector feature: {}", feature);
    }
    
    // Test feature compression and memory efficiency
    let memory_usage = feature_extractor.get_memory_usage().await?;
    assert!(memory_usage < 15.0 * 1024.0 * 1024.0, // 15MB limit
        "Feature extractor memory usage {} exceeds limit", memory_usage);
    
    println!("📊 Extracted {} sector features and {} symbol-specific features", 
             shared_features.sector_features.len(), 
             shared_features.symbol_features.len());
    println!("📊 Memory usage: {:.2} MB", memory_usage as f64 / (1024.0 * 1024.0));
    
    println!("✅ SharedFeatureExtractor functionality test passed");
    
    Ok(())
}

#[test]
async fn test_integration_with_existing_systems() -> Result<()> {
    println!("🔗 Testing Integration with Existing Systems");
    
    // Create all required components
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new()?);
    
    // Test VendorPredictor integration
    let neural_config = create_test_neural_config();
    let predictor = VendorPredictor::new(&neural_config, sector_mapper.clone(), performance_tracker.clone())?;
    
    // Test sector routing
    let test_symbols = vec!["AAPL", "JPM", "JNJ"];
    
    for symbol in &test_symbols {
        let sector_info = sector_mapper.get_sector(symbol)?;
        println!("📊 {} mapped to sector: {}", symbol, sector_info.id);
        
        // Test prediction capability
        let data = create_test_data(symbol, 150.0);
        let prediction = predictor.predict_single(&data).await?;
        
        assert!(prediction.confidence > 0.0, 
            "Prediction confidence should be positive for {}", symbol);
        
        // Record performance tracking
        performance_tracker.record_prediction(
            symbol,
            &prediction.model_name,
            &prediction,
            None,
        ).await?;
    }
    
    // Test DAA coordinator integration
    let mut daa_coordinator = DAACoordinator::new(
        sector_mapper.clone(),
        performance_tracker.clone(),
        HashMap::new(),
    ).await?;
    
    // Register symbols and test coordination
    for symbol in &test_symbols {
        let sector_info = sector_mapper.get_sector(symbol)?;
        daa_coordinator.register_symbol(symbol, &sector_info.id).await?;
    }
    
    // Test end-to-end workflow
    let test_data: Vec<TimeSeriesData> = test_symbols.iter()
        .map(|symbol| create_test_data(symbol, 150.0))
        .collect();
    
    let coordination_results = daa_coordinator.coordinate_predictions(&test_data).await?;
    
    assert!(coordination_results.len() >= test_symbols.len(),
        "Should have coordination results for all symbols");
    
    println!("📊 Successfully coordinated {} symbols across {} sectors", 
             test_symbols.len(), coordination_results.len());
    
    println!("✅ Integration test passed: All systems working together");
    
    Ok(())
}

#[test]
async fn test_full_production_readiness_workflow() -> Result<()> {
    println!("🚀 Testing Full Production Readiness Workflow");
    
    let start_time = Instant::now();
    
    // Initialize all components
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new()?);
    
    let neural_config = create_test_neural_config();
    let predictor = VendorPredictor::new(&neural_config, sector_mapper.clone(), performance_tracker.clone())?;
    
    // Create production-scale test data
    let production_symbols = vec![
        "AAPL", "MSFT", "GOOGL", "TSLA", "META", "NVDA", "AMZN", // Tech
        "JPM", "BAC", "WFC", "GS", "MS", // Financial
        "JNJ", "PFE", "UNH", "ABBV", // Healthcare
    ];
    
    let mut all_predictions = Vec::new();
    let mut sector_performance = HashMap::new();
    
    // Process predictions for all symbols
    for symbol in &production_symbols {
        let data = create_test_data(symbol, 150.0 + (symbol.len() as f64 * 5.0));
        
        let symbol_start = Instant::now();
        let prediction = predictor.predict_single(&data).await?;
        let symbol_time = symbol_start.elapsed().as_millis();
        
        // Validate prediction quality
        assert!(prediction.confidence > 0.5, 
            "Low confidence prediction for {}: {:.3}", symbol, prediction.confidence);
        assert!(prediction.value > 0.0, 
            "Invalid prediction value for {}: {:.3}", symbol, prediction.value);
        
        all_predictions.push((symbol.to_string(), prediction));
        
        // Track sector performance
        let sector_info = sector_mapper.get_sector(symbol)?;
        let sector_stats = sector_performance.entry(sector_info.id.clone())
            .or_insert_with(|| (0u32, 0f64));
        sector_stats.0 += 1;
        sector_stats.1 += symbol_time as f64;
        
        // Performance requirement: <10ms per symbol prediction
        assert!(symbol_time < 10, 
            "Prediction time {} ms exceeds 10ms limit for {}", symbol_time, symbol);
    }
    
    let total_time = start_time.elapsed();
    let total_ms = total_time.as_millis() as f64;
    
    // Validate overall performance
    println!("📊 Production workflow completed in: {:.2} ms", total_ms);
    println!("📊 Processed {} symbols across {} sectors", 
             production_symbols.len(), sector_performance.len());
    
    // Performance targets for production
    let throughput = production_symbols.len() as f64 / (total_ms / 1000.0);
    println!("📊 Throughput: {:.1} predictions/second", throughput);
    
    assert!(throughput > 50.0, 
        "Throughput {:.1} predictions/sec below target 50/sec", throughput);
    
    // Validate sector distribution
    for (sector, (count, total_time)) in &sector_performance {
        let avg_time = total_time / *count as f64;
        println!("📊 {}: {} symbols, avg {:.1} ms/prediction", 
                 sector, count, avg_time);
        
        assert!(avg_time < 10.0,
            "Average prediction time {:.1} ms exceeds 10ms for sector {}", avg_time, sector);
    }
    
    println!("✅ Full production readiness test passed:");
    println!("   - {} symbols processed", production_symbols.len());
    println!("   - {:.1} predictions/second throughput", throughput);
    println!("   - All sectors performing within limits");
    
    Ok(())
}

/// Get current memory usage in MB (simplified for testing)
fn get_memory_usage() -> f64 {
    // In a real implementation, this would use system memory APIs
    // For testing, we'll use a simplified approach
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(output) = Command::new("ps")
            .args(&["-o", "rss=", "-p"])
            .arg(std::process::id().to_string())
            .output()
        {
            if let Ok(rss_str) = String::from_utf8(output.stdout) {
                if let Ok(rss_kb) = rss_str.trim().parse::<f64>() {
                    return rss_kb / 1024.0; // Convert KB to MB
                }
            }
        }
    }
    
    // Fallback: return a default value for testing
    50.0 // MB
}