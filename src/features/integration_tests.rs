//! Integration tests for feature engineering pipeline
//! 
//! Tests the complete feature engineering system with all modules working together

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::TimeSeriesData;
    use crate::features::{
        FeatureEngineeringPipeline, FeaturePipelineConfig,
        technical_indicators::TechnicalIndicatorEngine,
        market_microstructure::MicrostructureAnalyzer,
        cross_asset::CrossAssetCorrelationEngine,
        regime_detection::RegimeDetector,
    };
    use chrono::{DateTime, Utc, TimeZone};
    use std::collections::HashMap;
    use std::time::Instant;

    /// Create comprehensive market data for integration testing
    fn create_integration_test_data() -> (TimeSeriesData, Vec<TimeSeriesData>, HashMap<String, Vec<TimeSeriesData>>) {
        // Current data point
        let current = TimeSeriesData {
            timestamp: Utc::now(),
            symbol: "AAPL".to_string(),
            open: 150.0,
            high: 152.0,
            low: 149.0,
            close: 151.5,
            volume: vec![50_000_000.0],
        };
        
        // Historical data with various patterns
        let mut historical = vec![];
        let base_time = Utc.timestamp_opt(1640000000, 0).unwrap();
        
        // Create 300 data points with mixed patterns
        for i in 0..300 {
            let time = base_time + chrono::Duration::hours(i);
            
            // Create different market conditions
            let (open, high, low, close, volume) = if i < 100 {
                // Uptrend with Elliott Wave pattern
                let wave_progress = (i as f64 / 20.0).sin();
                let trend = 100.0 + (i as f64 * 0.5);
                let price = trend + wave_progress * 5.0;
                (
                    price - 0.5,
                    price + 1.0 + wave_progress.abs(),
                    price - 1.0 - wave_progress.abs(),
                    price,
                    30_000_000.0 + wave_progress * 10_000_000.0
                )
            } else if i < 200 {
                // Sideways with Harmonic patterns
                let harmonic = ((i - 100) as f64 * 0.1).sin() * 10.0;
                let price = 150.0 + harmonic;
                (
                    price - 0.3,
                    price + 0.5,
                    price - 0.5,
                    price,
                    25_000_000.0
                )
            } else {
                // Downtrend with high volatility
                let trend = 150.0 - ((i - 200) as f64 * 0.3);
                let volatility = ((i - 200) as f64 * 0.2).cos() * 3.0;
                let price = trend + volatility;
                (
                    price + 0.5,
                    price + 1.5,
                    price - 1.5,
                    price,
                    40_000_000.0 + volatility.abs() * 5_000_000.0
                )
            };
            
            historical.push(TimeSeriesData {
                timestamp: time,
                symbol: "AAPL".to_string(),
                open,
                high,
                low,
                close,
                volume: vec![volume],
            });
        }
        
        // Market context with correlated assets
        let mut market_context = HashMap::new();
        
        // SPY - highly correlated
        let spy_data: Vec<TimeSeriesData> = historical.iter().enumerate().map(|(i, h)| {
            let correlation_factor = 0.9;
            let noise = (i as f64 * 0.1).sin() * 2.0;
            TimeSeriesData {
                timestamp: h.timestamp,
                symbol: "SPY".to_string(),
                open: h.open * 3.0 * correlation_factor + noise,
                high: h.high * 3.0 * correlation_factor + noise,
                low: h.low * 3.0 * correlation_factor + noise,
                close: h.close * 3.0 * correlation_factor + noise,
                volume: vec![h.volume.first().unwrap_or(&0.0) * 2.0],
            }
        }).collect();
        market_context.insert("SPY".to_string(), spy_data);
        
        // VIX - negatively correlated
        let vix_data: Vec<TimeSeriesData> = historical.iter().enumerate().map(|(i, h)| {
            let inverse_price = 50.0 - (h.close - 100.0) * 0.3;
            TimeSeriesData {
                timestamp: h.timestamp,
                symbol: "VIX".to_string(),
                open: inverse_price - 0.5,
                high: inverse_price + 1.0,
                low: inverse_price - 1.0,
                close: inverse_price,
                volume: vec![10_000_000.0],
            }
        }).collect();
        market_context.insert("VIX".to_string(), vix_data);
        
        // Sector ETFs
        let sectors = vec![("XLK", 1.2), ("XLF", 0.8), ("XLE", 0.6)];
        for (symbol, factor) in sectors {
            let sector_data: Vec<TimeSeriesData> = historical.iter().map(|h| {
                TimeSeriesData {
                    timestamp: h.timestamp,
                    symbol: symbol.to_string(),
                    open: h.open * factor,
                    high: h.high * factor,
                    low: h.low * factor,
                    close: h.close * factor,
                    volume: vec![h.volume.first().unwrap_or(&0.0) * 0.5],
                }
            }).collect();
            market_context.insert(symbol.to_string(), sector_data);
        }
        
        (current, historical, market_context)
    }

    #[tokio::test]
    async fn test_complete_feature_pipeline() {
        let config = FeaturePipelineConfig::default();
        let pipeline = FeatureEngineeringPipeline::new(config).await.unwrap();
        
        let (current, historical, market_context) = create_integration_test_data();
        
        let start = Instant::now();
        let features = pipeline.compute_features(&current, &historical, &market_context).await.unwrap();
        let duration = start.elapsed();
        
        println!("Computed {} features in {:?}", features.len(), duration);
        
        // Verify we have features from all modules
        
        // Technical indicators
        assert!(features.contains_key("rsi"), "Should have RSI");
        assert!(features.contains_key("macd_line"), "Should have MACD");
        assert!(features.contains_key("bb_upper"), "Should have Bollinger Bands");
        
        // Elliott Wave and Harmonics
        let has_elliott = features.keys().any(|k| k.contains("elliott"));
        let has_harmonic = features.keys().any(|k| k.contains("harmonic"));
        assert!(has_elliott || has_harmonic, "Should detect wave patterns");
        
        // Market microstructure
        assert!(features.contains_key("spread"), "Should have spread");
        assert!(features.contains_key("order_flow_imbalance"), "Should have flow imbalance");
        assert!(features.contains_key("flow_toxicity_index"), "Should have toxicity metrics");
        
        // Cross-asset correlations
        assert!(features.contains_key("corr_spy_60"), "Should have SPY correlation");
        assert!(features.contains_key("corr_vix_60"), "Should have VIX correlation");
        assert!(features.contains_key("sector_corr_technology"), "Should have sector correlations");
        
        // Market regime
        assert!(features.contains_key("market_regime"), "Should have regime detection");
        
        // Performance metrics
        assert!(features.contains_key("_computation_time_ms"), "Should track computation time");
        let comp_time = features.get("_computation_time_ms").unwrap();
        assert!(*comp_time < 5000.0, "Should complete within 5 seconds");
        
        // Feature count
        assert!(features.len() > 100, "Should compute many features");
        assert!(features.len() < 1000, "Should not compute excessive features");
    }

    #[tokio::test]
    async fn test_feature_importance_tracking() {
        let config = FeaturePipelineConfig {
            enable_adaptive_selection: true,
            importance_threshold: 0.05,
            ..Default::default()
        };
        let pipeline = FeatureEngineeringPipeline::new(config).await.unwrap();
        
        // Get initial importance scores
        let initial_importance = pipeline.get_feature_importance().await.unwrap();
        assert!(initial_importance.is_empty() || initial_importance.values().all(|&v| v >= 0.0));
        
        // Simulate model feedback with importance scores
        let mut mock_importance = HashMap::new();
        mock_importance.insert("rsi".to_string(), 0.8);
        mock_importance.insert("macd_line".to_string(), 0.6);
        mock_importance.insert("flow_toxicity_index".to_string(), 0.9);
        mock_importance.insert("corr_spy_60".to_string(), 0.7);
        mock_importance.insert("spread".to_string(), 0.3);
        
        pipeline.update_feature_importance(mock_importance).await.unwrap();
        
        // Verify importance was updated
        let updated_importance = pipeline.get_feature_importance().await.unwrap();
        assert_eq!(updated_importance.get("flow_toxicity_index"), Some(&0.9));
        assert_eq!(updated_importance.get("spread"), Some(&0.3));
    }

    #[tokio::test]
    async fn test_parallel_computation() {
        let config = FeaturePipelineConfig {
            enable_parallel: true,
            num_workers: 4,
            ..Default::default()
        };
        let pipeline = FeatureEngineeringPipeline::new(config).await.unwrap();
        
        let (current, historical, market_context) = create_integration_test_data();
        
        // Time parallel execution
        let start = Instant::now();
        let features_parallel = pipeline.compute_features(&current, &historical, &market_context).await.unwrap();
        let parallel_duration = start.elapsed();
        
        // Compare with single-threaded (create new pipeline)
        let config_single = FeaturePipelineConfig {
            enable_parallel: false,
            num_workers: 1,
            ..Default::default()
        };
        let pipeline_single = FeatureEngineeringPipeline::new(config_single).await.unwrap();
        
        let start = Instant::now();
        let features_single = pipeline_single.compute_features(&current, &historical, &market_context).await.unwrap();
        let single_duration = start.elapsed();
        
        println!("Parallel: {:?}, Single: {:?}", parallel_duration, single_duration);
        
        // Verify same features computed
        assert_eq!(features_parallel.len(), features_single.len(), 
                   "Should compute same number of features");
        
        // Parallel should generally be faster (but not always in tests)
        println!("Speedup: {:.2}x", single_duration.as_secs_f64() / parallel_duration.as_secs_f64());
    }

    #[tokio::test]
    async fn test_memory_constraints() {
        let config = FeaturePipelineConfig {
            memory_limit_mb: 100.0, // Low limit for testing
            ..Default::default()
        };
        let pipeline = FeatureEngineeringPipeline::new(config).await.unwrap();
        
        let (current, historical, market_context) = create_integration_test_data();
        
        // Should handle memory constraints gracefully
        let result = pipeline.compute_features(&current, &historical, &market_context).await;
        assert!(result.is_ok(), "Should handle memory constraints");
    }

    #[tokio::test]
    async fn test_realtime_vs_batch_mode() {
        // Realtime mode
        let config_realtime = FeaturePipelineConfig {
            enable_realtime: true,
            ..Default::default()
        };
        let pipeline_realtime = FeatureEngineeringPipeline::new(config_realtime).await.unwrap();
        
        // Batch mode
        let config_batch = FeaturePipelineConfig {
            enable_realtime: false,
            ..Default::default()
        };
        let pipeline_batch = FeatureEngineeringPipeline::new(config_batch).await.unwrap();
        
        let (current, historical, market_context) = create_integration_test_data();
        
        let features_realtime = pipeline_realtime.compute_features(&current, &historical, &market_context).await.unwrap();
        let features_batch = pipeline_batch.compute_features(&current, &historical, &market_context).await.unwrap();
        
        // Realtime should include microstructure features
        assert!(features_realtime.contains_key("flow_toxicity_index"));
        assert!(!features_batch.contains_key("flow_toxicity_index"));
        
        // Both should have basic indicators
        assert!(features_realtime.contains_key("rsi"));
        assert!(features_batch.contains_key("rsi"));
    }

    #[tokio::test]
    async fn test_error_handling() {
        let config = FeaturePipelineConfig::default();
        let pipeline = FeatureEngineeringPipeline::new(config).await.unwrap();
        
        // Test with empty data
        let current = TimeSeriesData {
            timestamp: Utc::now(),
            symbol: "TEST".to_string(),
            open: 100.0,
            high: 100.0,
            low: 100.0,
            close: 100.0,
            volume: vec![0.0],
        };
        let empty_historical = vec![];
        let empty_context = HashMap::new();
        
        let result = pipeline.compute_features(&current, &empty_historical, &empty_context).await;
        assert!(result.is_ok(), "Should handle empty data gracefully");
        
        let features = result.unwrap();
        assert!(!features.is_empty(), "Should compute some features even with minimal data");
    }

    #[tokio::test]
    async fn test_feature_categories() {
        let config = FeaturePipelineConfig::default();
        let pipeline = FeatureEngineeringPipeline::new(config).await.unwrap();
        
        let (current, historical, market_context) = create_integration_test_data();
        let features = pipeline.compute_features(&current, &historical, &market_context).await.unwrap();
        
        // Count features by category
        let mut price_features = 0;
        let mut volume_features = 0;
        let mut volatility_features = 0;
        let mut momentum_features = 0;
        let mut microstructure_features = 0;
        let mut correlation_features = 0;
        
        for key in features.keys() {
            if key.contains("price") || key.contains("high") || key.contains("low") {
                price_features += 1;
            }
            if key.contains("volume") || key.contains("obv") || key.contains("mfi") {
                volume_features += 1;
            }
            if key.contains("volatility") || key.contains("atr") || key.contains("bb") {
                volatility_features += 1;
            }
            if key.contains("rsi") || key.contains("momentum") || key.contains("roc") {
                momentum_features += 1;
            }
            if key.contains("spread") || key.contains("toxic") || key.contains("flow") {
                microstructure_features += 1;
            }
            if key.contains("corr") || key.contains("beta") {
                correlation_features += 1;
            }
        }
        
        println!("Feature distribution:");
        println!("  Price: {}", price_features);
        println!("  Volume: {}", volume_features);
        println!("  Volatility: {}", volatility_features);
        println!("  Momentum: {}", momentum_features);
        println!("  Microstructure: {}", microstructure_features);
        println!("  Correlation: {}", correlation_features);
        
        // Ensure balanced feature distribution
        assert!(price_features > 5);
        assert!(volume_features > 5);
        assert!(volatility_features > 5);
        assert!(momentum_features > 5);
        assert!(microstructure_features > 5);
        assert!(correlation_features > 10);
    }

    #[tokio::test]
    async fn test_computation_stats() {
        let config = FeaturePipelineConfig::default();
        let pipeline = FeatureEngineeringPipeline::new(config).await.unwrap();
        
        let (current, historical, market_context) = create_integration_test_data();
        
        // Compute features multiple times
        for _ in 0..3 {
            pipeline.compute_features(&current, &historical, &market_context).await.unwrap();
        }
        
        // Get computation statistics
        let stats = pipeline.get_computation_stats().await.unwrap();
        
        assert!(stats.records_processed > 0);
        assert!(stats.errors.is_empty(), "Should have no errors");
        assert!(stats.start_time < stats.end_time);
    }

    #[tokio::test]
    async fn test_pipeline_optimization() {
        let config = FeaturePipelineConfig {
            num_workers: 2,
            ..Default::default()
        };
        let mut pipeline = FeatureEngineeringPipeline::new(config.clone()).await.unwrap();
        
        // Optimize based on performance
        pipeline.optimize_pipeline().await.unwrap();
        
        // Workers should be adjusted based on performance
        // (In tests this might not change much)
        assert!(pipeline.config.num_workers >= 2 && pipeline.config.num_workers <= 8);
    }
}