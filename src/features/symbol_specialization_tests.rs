//! Comprehensive tests for SymbolSpecializationLayer
//! 
//! Tests the integration with SharedFeatureExtractor and validates
//! the memory efficiency and performance requirements.

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::data::{TimeSeriesData, SectorId};
    use crate::features::{
        SharedFeatureExtractor, SharedFeatureConfig, 
        SymbolSpecializationLayer, SymbolSpecializationConfig
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use chrono::Utc;

    /// Create test time series data
    fn create_test_data(symbol: &str, length: usize) -> TimeSeriesData {
        let now = Utc::now();
        let values: Vec<f64> = (0..length).map(|i| 100.0 + (i as f64 * 0.1)).collect();
        let volume: Vec<f64> = (0..length).map(|i| 1000.0 + (i as f64 * 10.0)).collect();
        let timestamps = (0..length).map(|i| now + chrono::Duration::seconds(i as i64)).collect();

        TimeSeriesData {
            symbol: symbol.to_string(),
            values,
            volume,
            timestamp: now,
            timestamps,
            intervals: vec![60; length], // 1-minute intervals
        }
    }

    /// Create test sector data
    fn create_sector_data() -> HashMap<String, TimeSeriesData> {
        let mut sector_data = HashMap::new();
        
        // Technology sector symbols
        sector_data.insert("AAPL".to_string(), create_test_data("AAPL", 100));
        sector_data.insert("MSFT".to_string(), create_test_data("MSFT", 100));
        sector_data.insert("GOOGL".to_string(), create_test_data("GOOGL", 100));
        sector_data.insert("NVDA".to_string(), create_test_data("NVDA", 100));
        
        sector_data
    }

    #[tokio::test]
    async fn test_symbol_specialization_layer_creation() {
        let shared_config = SharedFeatureConfig::default();
        let shared_extractor = Arc::new(
            SharedFeatureExtractor::new(SectorId::Technology, shared_config)
                .await
                .expect("Failed to create SharedFeatureExtractor")
        );
        
        let specialization_config = SymbolSpecializationConfig::default();
        let layer = SymbolSpecializationLayer::new(
            SectorId::Technology,
            shared_extractor,
            specialization_config,
        ).await;
        
        assert!(layer.is_ok(), "Failed to create SymbolSpecializationLayer");
        
        let layer = layer.unwrap();
        let (used, capacity, count) = layer.get_memory_stats().await.unwrap();
        
        assert_eq!(used, 0, "Initial memory usage should be zero");
        assert_eq!(count, 0, "Initial symbol count should be zero");
        assert!(capacity > 0, "Memory capacity should be positive");
    }

    #[tokio::test]
    async fn test_specialized_feature_extraction() {
        let shared_config = SharedFeatureConfig::default();
        let shared_extractor = Arc::new(
            SharedFeatureExtractor::new(SectorId::Technology, shared_config)
                .await
                .expect("Failed to create SharedFeatureExtractor")
        );
        
        let specialization_config = SymbolSpecializationConfig::default();
        let layer = SymbolSpecializationLayer::new(
            SectorId::Technology,
            shared_extractor,
            specialization_config,
        ).await.expect("Failed to create SymbolSpecializationLayer");
        
        let sector_data = create_sector_data();
        let symbol_data = create_test_data("AAPL", 100);
        
        let features = layer.extract_specialized_features(
            "AAPL",
            &symbol_data,
            &sector_data,
        ).await;
        
        assert!(features.is_ok(), "Failed to extract specialized features");
        
        let features = features.unwrap();
        assert!(!features.is_empty(), "Features should not be empty");
        
        // Check for expected feature categories
        let has_shared_features = features.keys().any(|k| k.starts_with("shared_feature_"));
        let has_symbol_features = features.keys().any(|k| k.starts_with("symbol_feature_"));
        let has_tech_features = features.keys().any(|k| k.starts_with("tech_"));
        
        assert!(has_shared_features, "Should have shared features");
        assert!(has_symbol_features, "Should have symbol features");
        assert!(has_tech_features, "Should have technical features");
        
        println!("Extracted {} features for AAPL", features.len());
        for (name, value) in features.iter().take(10) {
            println!("  {}: {:.6}", name, value);
        }
    }

    #[tokio::test]
    async fn test_memory_efficiency() {
        let shared_config = SharedFeatureConfig {
            memory_limit_mb: 10.0, // Small limit for testing
            ..Default::default()
        };
        let shared_extractor = Arc::new(
            SharedFeatureExtractor::new(SectorId::Technology, shared_config)
                .await
                .expect("Failed to create SharedFeatureExtractor")
        );
        
        let specialization_config = SymbolSpecializationConfig {
            max_memory_per_symbol: 50 * 1024, // 50KB per symbol
            ..Default::default()
        };
        let layer = SymbolSpecializationLayer::new(
            SectorId::Technology,
            shared_extractor,
            specialization_config,
        ).await.expect("Failed to create SymbolSpecializationLayer");
        
        let sector_data = create_sector_data();
        
        // Extract features for multiple symbols
        let symbols = ["AAPL", "MSFT", "GOOGL"];
        let mut total_features = 0;
        
        for symbol in &symbols {
            let symbol_data = create_test_data(symbol, 50);
            let features = layer.extract_specialized_features(
                symbol,
                &symbol_data,
                &sector_data,
            ).await;
            
            assert!(features.is_ok(), "Failed to extract features for {}", symbol);
            total_features += features.unwrap().len();
        }
        
        let (used, capacity, count) = layer.get_memory_stats().await.unwrap();
        
        println!("Memory usage after processing {} symbols:", symbols.len());
        println!("  Used: {:.2}KB", used as f64 / 1024.0);
        println!("  Capacity: {:.2}KB", capacity as f64 / 1024.0);
        println!("  Symbols: {}", count);
        println!("  Total features: {}", total_features);
        
        assert!(count == symbols.len(), "Should track all processed symbols");
        assert!(used < capacity, "Should not exceed memory capacity");
        
        // Verify memory per symbol is under limit
        let avg_memory_per_symbol = used / count.max(1);
        assert!(avg_memory_per_symbol <= specialization_config.max_memory_per_symbol,
                "Memory per symbol ({}) exceeds limit ({})", 
                avg_memory_per_symbol, specialization_config.max_memory_per_symbol);
    }

    #[tokio::test]
    async fn test_graceful_fallback() {
        let shared_config = SharedFeatureConfig::default();
        let shared_extractor = Arc::new(
            SharedFeatureExtractor::new(SectorId::Technology, shared_config)
                .await
                .expect("Failed to create SharedFeatureExtractor")
        );
        
        // Create config that will force fallback due to memory limits
        let specialization_config = SymbolSpecializationConfig {
            max_memory_per_symbol: 100, // Very small limit to trigger fallback
            min_improvement_threshold: 0.5, // High threshold to trigger fallback
            ..Default::default()
        };
        
        let layer = SymbolSpecializationLayer::new(
            SectorId::Technology,
            shared_extractor,
            specialization_config,
        ).await.expect("Failed to create SymbolSpecializationLayer");
        
        let sector_data = create_sector_data();
        let symbol_data = create_test_data("AAPL", 100);
        
        // This should fallback to sector features due to constraints
        let features = layer.get_features_with_fallback(
            "AAPL",
            &symbol_data,
            &sector_data,
        ).await;
        
        assert!(features.is_ok(), "Fallback should always succeed");
        
        let features = features.unwrap();
        assert!(!features.is_empty(), "Fallback features should not be empty");
        
        // Check for basic sector features
        let expected_features = ["market_regime", "volatility", "sector_momentum", 
                               "correlation", "relative_strength", "beta_to_sector"];
        
        for feature in &expected_features {
            assert!(features.contains_key(*feature), 
                   "Fallback should include feature: {}", feature);
        }
        
        println!("Fallback features for AAPL:");
        for (name, value) in &features {
            println!("  {}: {:.6}", name, value);
        }
    }

    #[tokio::test]
    async fn test_fine_tuning_functionality() {
        let shared_config = SharedFeatureConfig::default();
        let shared_extractor = Arc::new(
            SharedFeatureExtractor::new(SectorId::Technology, shared_config)
                .await
                .expect("Failed to create SharedFeatureExtractor")
        );
        
        let specialization_config = SymbolSpecializationConfig {
            fine_tuning_enabled: true,
            min_training_samples: 10,
            max_training_iterations: 50,
            ..Default::default()
        };
        
        let layer = SymbolSpecializationLayer::new(
            SectorId::Technology,
            shared_extractor,
            specialization_config,
        ).await.expect("Failed to create SymbolSpecializationLayer");
        
        // Create training data
        let training_data: Vec<TimeSeriesData> = (0..20)
            .map(|i| create_test_data(&format!("AAPL_sample_{}", i), 50))
            .collect();
        
        let target_values: Vec<f64> = (0..20).map(|i| i as f64 * 0.1).collect();
        
        // Perform fine-tuning
        let result = layer.fine_tune_specialization(
            "AAPL",
            &training_data,
            &target_values,
            Some(0.001),
        ).await;
        
        assert!(result.is_ok(), "Fine-tuning should succeed");
        
        // Check that weights were created/updated
        let metrics = layer.get_performance_metrics("AAPL").await.unwrap();
        assert!(metrics.is_some(), "Performance metrics should be available after fine-tuning");
        
        let metrics = metrics.unwrap();
        println!("Fine-tuning metrics for AAPL:");
        println!("  Improvement over baseline: {:.4}", metrics.improvement_over_baseline);
        println!("  Validation accuracy: {:.4}", metrics.validation_accuracy);
        println!("  Overfitting score: {:.4}", metrics.overfitting_score);
        println!("  Memory usage: {} bytes", metrics.memory_usage);
    }

    #[tokio::test]
    async fn test_technical_signal_computation() {
        let shared_config = SharedFeatureConfig::default();
        let shared_extractor = Arc::new(
            SharedFeatureExtractor::new(SectorId::Technology, shared_config)
                .await
                .expect("Failed to create SharedFeatureExtractor")
        );
        
        let specialization_config = SymbolSpecializationConfig {
            enable_technical_signals: true,
            enable_price_patterns: true,
            enable_volume_analysis: true,
            enable_order_flow: true,
            ..Default::default()
        };
        
        let layer = SymbolSpecializationLayer::new(
            SectorId::Technology,
            shared_extractor,
            specialization_config,
        ).await.expect("Failed to create SymbolSpecializationLayer");
        
        let sector_data = create_sector_data();
        
        // Create symbol with enough data for technical indicators
        let symbol_data = create_test_data("AAPL", 100);
        
        let features = layer.extract_specialized_features(
            "AAPL",
            &symbol_data,
            &sector_data,
        ).await.expect("Failed to extract features");
        
        // Check for technical signals
        let tech_features: Vec<&String> = features.keys()
            .filter(|k| k.starts_with("tech_"))
            .collect();
        
        assert!(!tech_features.is_empty(), "Should have technical features");
        
        // Check for volume features
        let volume_features: Vec<&String> = features.keys()
            .filter(|k| k.contains("volume"))
            .collect();
        
        assert!(!volume_features.is_empty(), "Should have volume features");
        
        // Check for order flow features
        let order_flow_features: Vec<&String> = features.keys()
            .filter(|k| k.contains("delta") || k.contains("order_flow"))
            .collect();
        
        assert!(!order_flow_features.is_empty(), "Should have order flow features");
        
        println!("Technical signal categories found:");
        println!("  Technical indicators: {}", tech_features.len());
        println!("  Volume features: {}", volume_features.len());
        println!("  Order flow features: {}", order_flow_features.len());
    }

    #[tokio::test]
    async fn test_performance_metrics_tracking() {
        let shared_config = SharedFeatureConfig::default();
        let shared_extractor = Arc::new(
            SharedFeatureExtractor::new(SectorId::Technology, shared_config)
                .await
                .expect("Failed to create SharedFeatureExtractor")
        );
        
        let layer = SymbolSpecializationLayer::new(
            SectorId::Technology,
            shared_extractor,
            SymbolSpecializationConfig::default(),
        ).await.expect("Failed to create SymbolSpecializationLayer");
        
        let sector_data = create_sector_data();
        let symbol_data = create_test_data("AAPL", 100);
        
        // Extract features to initialize weights
        let _features = layer.extract_specialized_features(
            "AAPL",
            &symbol_data,
            &sector_data,
        ).await.expect("Failed to extract features");
        
        // Check initial metrics
        let metrics = layer.get_performance_metrics("AAPL").await.unwrap();
        assert!(metrics.is_some(), "Should have performance metrics");
        
        let metrics = metrics.unwrap();
        assert_eq!(metrics.improvement_over_baseline, 0.0, "Initial improvement should be zero");
        assert!(metrics.loss_history.is_empty(), "Initial loss history should be empty");
        assert_eq!(metrics.validation_accuracy, 0.0, "Initial validation accuracy should be zero");
        
        // Test memory stats
        let (used, capacity, count) = layer.get_memory_stats().await.unwrap();
        assert!(count > 0, "Should have processed at least one symbol");
        assert!(used > 0, "Should have used some memory");
        assert!(used < capacity, "Should not exceed capacity");
    }

    #[test]
    fn test_rsi_calculation() {
        // Create a simple test layer for unit testing
        let layer = create_simple_test_layer();
        
        // Test RSI calculation with trending up data
        let up_trend = vec![10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0, 21.0, 22.0, 23.0, 24.0];
        let rsi_up = layer.calculate_rsi(&up_trend, 14).unwrap();
        assert!(rsi_up > 50.0, "RSI should be above 50 for uptrend: {}", rsi_up);
        assert!(rsi_up < 100.0, "RSI should be below 100: {}", rsi_up);
        
        // Test RSI calculation with trending down data
        let down_trend = vec![24.0, 23.0, 22.0, 21.0, 20.0, 19.0, 18.0, 17.0, 16.0, 15.0, 14.0, 13.0, 12.0, 11.0, 10.0];
        let rsi_down = layer.calculate_rsi(&down_trend, 14).unwrap();
        assert!(rsi_down < 50.0, "RSI should be below 50 for downtrend: {}", rsi_down);
        assert!(rsi_down > 0.0, "RSI should be above 0: {}", rsi_down);
        
        // Test with insufficient data
        let short_data = vec![1.0, 2.0, 3.0];
        let rsi_short = layer.calculate_rsi(&short_data, 14).unwrap();
        assert_eq!(rsi_short, 50.0, "RSI should default to 50 for insufficient data");
        
        println!("RSI tests passed:");
        println!("  Uptrend RSI: {:.2}", rsi_up);
        println!("  Downtrend RSI: {:.2}", rsi_down);
        println!("  Short data RSI: {:.2}", rsi_short);
    }

    #[test]
    fn test_macd_calculation() {
        let layer = create_simple_test_layer();
        
        // Test MACD with sufficient data
        let values: Vec<f64> = (1..=50).map(|i| i as f64 + (i as f64 * 0.1)).collect();
        let macd = layer.calculate_macd(&values).unwrap();
        assert!(macd.is_finite(), "MACD should be finite: {}", macd);
        
        // Test with insufficient data
        let short_data = vec![1.0, 2.0, 3.0];
        let macd_short = layer.calculate_macd(&short_data).unwrap();
        assert_eq!(macd_short, 0.0, "MACD should be 0 for insufficient data");
        
        println!("MACD tests passed:");
        println!("  Long series MACD: {:.6}", macd);
        println!("  Short series MACD: {:.6}", macd_short);
    }

    // Helper function to create a minimal test layer for unit tests
    fn create_simple_test_layer() -> SymbolSpecializationLayer {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use dashmap::DashMap;
        use tokio::sync::{RwLock, Semaphore};
        
        // This is a minimal setup for unit testing non-async methods
        // In practice, we'd use proper test fixtures
        SymbolSpecializationLayer {
            sector_id: SectorId::Technology,
            shared_extractor: Arc::new(unsafe { std::mem::zeroed() }), // Null for unit tests
            symbol_weights: Arc::new(DashMap::new()),
            signal_cache: Arc::new(RwLock::new(HashMap::new())),
            config: SymbolSpecializationConfig::default(),
            memory_tracker: Arc::new(RwLock::new(HashMap::new())),
            memory_semaphore: Arc::new(Semaphore::new(100)),
        }
    }
}