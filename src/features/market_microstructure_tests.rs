//! Comprehensive tests for market microstructure analysis module
//! 
//! Tests cover order flow toxicity, bid-ask dynamics, and liquidity patterns

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::TimeSeriesData;
    use crate::features::market_microstructure::{
        MicrostructureAnalyzer, MicrostructureConfig, OrderBookSnapshot, TradeEvent, TradeSide
    };
    use chrono::{DateTime, Utc, TimeZone, Duration};
    use std::collections::HashMap;
    use approx::assert_relative_eq;

    /// Helper function to create test time series data
    fn create_test_data(prices: Vec<(f64, f64, f64, f64, f64)>) -> Vec<TimeSeriesData> {
        prices.iter().enumerate().map(|(i, &(open, high, low, close, volume))| {
            TimeSeriesData {
                timestamp: Utc.timestamp_opt(1640000000 + (i as i64 * 60), 0).unwrap(),
                symbol: "TEST".to_string(),
                open,
                high,
                low,
                close,
                volume,
            }
        }).collect()
    }

    /// Create toxic order flow data pattern
    fn create_toxic_flow_data() -> Vec<TimeSeriesData> {
        let mut prices = vec![];
        
        // Simulate informed trading: large volume with directional price moves
        for i in 0..10 {
            let price = 100.0 + (i as f64 * 2.0);
            let volume = 5000.0 + (i as f64 * 1000.0); // Increasing volume
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, volume));
        }
        
        // Adverse selection: prices move against market maker
        for i in 0..10 {
            let price = 120.0 - (i as f64 * 1.5);
            let volume = 8000.0 - (i as f64 * 500.0); // High volume on reversal
            prices.push((price + 0.5, price + 1.0, price - 0.5, price, volume));
        }
        
        // Quote stuffing pattern: high volume, minimal price movement
        for i in 0..20 {
            let price = 105.0 + ((i as f64 * 0.1).sin() * 0.1);
            let volume = 10000.0 + ((i as f64).sin() * 2000.0);
            prices.push((price - 0.05, price + 0.05, price - 0.1, price, volume));
        }
        
        // Spoofing pattern: volume spikes with immediate reversals
        for i in 0..15 {
            let price = if i % 3 == 1 { 
                105.0 + 2.0  // Spike
            } else { 
                105.0 
            };
            let volume = if i % 3 == 1 { 
                15000.0  // Volume spike
            } else { 
                2000.0 
            };
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, volume));
        }
        
        create_test_data(prices)
    }

    /// Create healthy order flow data
    fn create_healthy_flow_data() -> Vec<TimeSeriesData> {
        let mut prices = vec![];
        
        // Normal trading: gradual price moves with consistent volume
        for i in 0..50 {
            let price = 100.0 + ((i as f64 * 0.2).sin() * 5.0);
            let volume = 3000.0 + ((i as f64 * 0.3).cos() * 500.0);
            prices.push((price - 0.3, price + 0.3, price - 0.5, price, volume));
        }
        
        create_test_data(prices)
    }

    #[tokio::test]
    async fn test_order_flow_toxicity_metrics() {
        let analyzer = MicrostructureAnalyzer::new();
        let data = create_toxic_flow_data();
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        
        // Check all toxicity metrics are present
        assert!(features.contains_key("adverse_selection_component"));
        assert!(features.contains_key("realized_spread_toxicity"));
        assert!(features.contains_key("toxicity_volatility"));
        assert!(features.contains_key("flow_toxicity_index"));
        assert!(features.contains_key("predatory_trading_indicator"));
        assert!(features.contains_key("quote_stuffing_indicator"));
        assert!(features.contains_key("spoofing_score"));
        
        // Verify toxicity is detected
        let toxicity_index = features.get("flow_toxicity_index").unwrap();
        assert!(*toxicity_index > 0.3, "Should detect high toxicity in toxic flow data");
        
        let adverse_selection = features.get("adverse_selection_component").unwrap();
        assert!(*adverse_selection > 0.1, "Should detect adverse selection");
    }

    #[tokio::test]
    async fn test_adverse_selection_component() {
        let analyzer = MicrostructureAnalyzer::new();
        let data = create_toxic_flow_data();
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        
        let adverse_selection = features.get("adverse_selection_component").unwrap();
        assert!(*adverse_selection >= 0.0 && *adverse_selection <= 1.0);
        
        // Test with healthy flow data
        let healthy_data = create_healthy_flow_data();
        let healthy_current = healthy_data.last().unwrap();
        let healthy_historical = &healthy_data[..healthy_data.len() - 1];
        
        let healthy_features = analyzer.analyze(healthy_current, healthy_historical).await.unwrap();
        let healthy_adverse = healthy_features.get("adverse_selection_component").unwrap();
        
        assert!(*healthy_adverse < *adverse_selection, 
                "Healthy flow should have lower adverse selection");
    }

    #[tokio::test]
    async fn test_quote_stuffing_detection() {
        let analyzer = MicrostructureAnalyzer::new();
        
        // Create specific quote stuffing pattern
        let mut prices = vec![];
        for i in 0..30 {
            let price = 100.0 + ((i as f64 * 0.01).sin() * 0.01); // Minimal price movement
            let volume = if i % 2 == 0 { 20000.0 } else { 1000.0 }; // High volume variation
            prices.push((price - 0.01, price + 0.01, price - 0.02, price, volume));
        }
        
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        
        let quote_stuffing = features.get("quote_stuffing_indicator").unwrap();
        assert!(*quote_stuffing > 0.1, "Should detect quote stuffing pattern");
    }

    #[tokio::test]
    async fn test_spoofing_detection() {
        let analyzer = MicrostructureAnalyzer::new();
        
        // Create specific spoofing pattern
        let mut prices = vec![];
        for i in 0..30 {
            let (price, volume) = if i % 3 == 0 {
                (100.0, 2000.0)  // Normal
            } else if i % 3 == 1 {
                (102.0, 15000.0) // Spike with high volume
            } else {
                (99.5, 3000.0)   // Immediate reversal
            };
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, volume));
        }
        
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        
        let spoofing = features.get("spoofing_score").unwrap();
        assert!(*spoofing > 0.0, "Should detect spoofing pattern");
    }

    #[tokio::test]
    async fn test_spread_analysis() {
        let analyzer = MicrostructureAnalyzer::new();
        let prices = vec![
            (100.0, 102.0, 99.0, 101.0, 1000.0),
            (101.0, 103.0, 100.0, 102.0, 1100.0),
            (102.0, 105.0, 101.0, 104.0, 1200.0), // Wide spread
            (104.0, 104.5, 103.5, 104.0, 1300.0), // Tight spread
        ];
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        
        assert!(features.contains_key("spread"));
        assert!(features.contains_key("spread_percentage"));
        assert!(features.contains_key("relative_spread"));
        assert!(features.contains_key("spread_volatility"));
        
        let spread = features.get("spread").unwrap();
        assert_relative_eq!(*spread, 1.0, epsilon = 0.01); // 104.5 - 103.5
    }

    #[tokio::test]
    async fn test_order_flow_imbalance() {
        let analyzer = MicrostructureAnalyzer::new();
        
        // Create imbalanced flow
        let mut prices = vec![];
        // Strong buying pressure
        for i in 0..10 {
            let price = 100.0 + (i as f64);
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, 5000.0));
        }
        // Weak selling
        for i in 0..5 {
            let price = 110.0 - (i as f64 * 0.5);
            prices.push((price + 0.5, price + 1.0, price - 0.5, price, 1000.0));
        }
        
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        
        assert!(features.contains_key("order_flow_imbalance"));
        assert!(features.contains_key("buy_volume_ratio"));
        assert!(features.contains_key("sell_volume_ratio"));
        assert!(features.contains_key("vpin"));
        assert!(features.contains_key("kyle_lambda"));
        
        let imbalance = features.get("order_flow_imbalance").unwrap();
        assert!(*imbalance > 0.0, "Should show positive imbalance for buying pressure");
    }

    #[tokio::test]
    async fn test_tick_patterns() {
        let analyzer = MicrostructureAnalyzer::new();
        
        // Create specific tick pattern
        let prices = vec![
            (100.0, 100.5, 99.5, 100.0, 1000.0),
            (100.0, 100.5, 99.5, 100.5, 1000.0), // Uptick
            (100.5, 101.0, 100.0, 101.0, 1000.0), // Uptick
            (101.0, 101.0, 100.5, 100.5, 1000.0), // Downtick
            (100.5, 100.5, 100.0, 100.5, 1000.0), // Zero tick
        ];
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        
        assert!(features.contains_key("uptick_ratio"));
        assert!(features.contains_key("downtick_ratio"));
        assert!(features.contains_key("zero_tick_ratio"));
        assert!(features.contains_key("tick_rule"));
        assert!(features.contains_key("positive_runs"));
        assert!(features.contains_key("negative_runs"));
        assert!(features.contains_key("run_ratio"));
    }

    #[tokio::test]
    async fn test_liquidity_metrics() {
        let analyzer = MicrostructureAnalyzer::new();
        let data = create_healthy_flow_data();
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        
        assert!(features.contains_key("amihud_illiquidity"));
        assert!(features.contains_key("roll_spread"));
        assert!(features.contains_key("avg_trade_size"));
        assert!(features.contains_key("liquidity_ratio"));
        assert!(features.contains_key("market_depth_proxy"));
        
        // Verify liquidity metrics are reasonable
        let amihud = features.get("amihud_illiquidity").unwrap();
        assert!(*amihud >= 0.0, "Amihud illiquidity should be non-negative");
        
        let roll_spread = features.get("roll_spread").unwrap();
        assert!(*roll_spread >= 0.0, "Roll spread should be non-negative");
    }

    #[tokio::test]
    async fn test_price_impact() {
        let analyzer = MicrostructureAnalyzer::new();
        
        // Create data with clear price impact
        let mut prices = vec![];
        for i in 0..15 {
            let price = 100.0 + (i as f64 * 0.5);
            let volume = 1000.0 + (i as f64 * 200.0);
            prices.push((price - 0.3, price + 0.3, price - 0.5, price, volume));
        }
        
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        
        assert!(features.contains_key("temporary_price_impact"));
        assert!(features.contains_key("permanent_price_impact"));
        assert!(features.contains_key("normalized_price_impact"));
        
        let temp_impact = features.get("temporary_price_impact").unwrap();
        assert!(*temp_impact != 0.0, "Should detect price impact");
    }

    #[tokio::test]
    async fn test_trade_intensity() {
        let analyzer = MicrostructureAnalyzer::new();
        
        // Create data with varying trade intensity
        let mut prices = vec![];
        // Low intensity period
        for i in 0..10 {
            let price = 100.0 + (i as f64 * 0.1);
            prices.push((price - 0.1, price + 0.1, price - 0.2, price, 500.0));
        }
        // High intensity period
        for i in 0..10 {
            let price = 101.0 + (i as f64 * 0.2);
            prices.push((price - 0.2, price + 0.2, price - 0.3, price, 5000.0));
        }
        
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        
        assert!(features.contains_key("trade_intensity_ratio"));
        assert!(features.contains_key("volume_concentration"));
        assert!(features.contains_key("volume_variance"));
        
        let intensity_ratio = features.get("trade_intensity_ratio").unwrap();
        assert!(*intensity_ratio > 1.0, "Current period should show high intensity");
    }

    #[tokio::test]
    async fn test_microstructure_noise() {
        let analyzer = MicrostructureAnalyzer::new();
        
        // Create noisy data
        let mut prices = vec![];
        for i in 0..30 {
            let noise = ((i as f64 * 0.5).sin() * 0.5);
            let price = 100.0 + (i as f64 * 0.1) + noise;
            prices.push((price - 0.2, price + 0.2, price - 0.3, price, 1000.0));
        }
        
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        
        assert!(features.contains_key("variance_ratio"));
        assert!(features.contains_key("microstructure_noise"));
        assert!(features.contains_key("return_autocorrelation"));
        
        let noise = features.get("microstructure_noise").unwrap();
        assert!(*noise >= 0.0, "Noise indicator should be non-negative");
    }

    #[tokio::test]
    async fn test_vpin_calculation() {
        let analyzer = MicrostructureAnalyzer::new();
        let data = create_toxic_flow_data();
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        
        let vpin = features.get("vpin").unwrap();
        assert!(*vpin >= 0.0 && *vpin <= 1.0, "VPIN should be between 0 and 1");
    }

    #[tokio::test]
    async fn test_kyle_lambda() {
        let analyzer = MicrostructureAnalyzer::new();
        
        // Create data with clear price impact
        let mut prices = vec![];
        for i in 0..20 {
            let price = if i % 2 == 0 {
                100.0 + (i as f64 * 0.1)
            } else {
                100.0 + (i as f64 * 0.1) - 0.05
            };
            let volume = 1000.0 + (i as f64 * 100.0);
            prices.push((price - 0.1, price + 0.1, price - 0.2, price, volume));
        }
        
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        
        let kyle_lambda = features.get("kyle_lambda").unwrap();
        assert!(*kyle_lambda >= 0.0, "Kyle's lambda should be non-negative");
    }

    #[tokio::test]
    async fn test_custom_config() {
        let config = MicrostructureConfig {
            max_book_depth: 5,
            flow_window_seconds: 600,
            enable_tick_analysis: false,
            enable_flow_imbalance: true,
            enable_liquidity_analysis: false,
        };
        
        let analyzer = MicrostructureAnalyzer::with_config(config);
        let data = create_healthy_flow_data();
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        
        // Tick analysis should be disabled
        assert!(!features.contains_key("uptick_ratio"));
        assert!(!features.contains_key("tick_rule"));
        
        // Flow imbalance should be enabled
        assert!(features.contains_key("order_flow_imbalance"));
        
        // Liquidity analysis should be disabled
        assert!(!features.contains_key("amihud_illiquidity"));
        assert!(!features.contains_key("roll_spread"));
    }

    #[tokio::test]
    async fn test_edge_cases() {
        let analyzer = MicrostructureAnalyzer::new();
        
        // Test with minimal data
        let prices = vec![
            (100.0, 101.0, 99.0, 100.0, 1000.0),
            (100.0, 101.0, 99.0, 100.0, 1000.0),
        ];
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let features = analyzer.analyze(current, historical).await.unwrap();
        assert!(!features.is_empty(), "Should return some features with minimal data");
        
        // Test with zero spread
        let zero_spread_prices = vec![
            (100.0, 100.0, 100.0, 100.0, 1000.0),
            (100.0, 100.0, 100.0, 100.0, 1000.0),
        ];
        let zero_data = create_test_data(zero_spread_prices);
        let zero_current = zero_data.last().unwrap();
        let zero_historical = &zero_data[..zero_data.len() - 1];
        
        let zero_features = analyzer.analyze(zero_current, zero_historical).await.unwrap();
        let spread = zero_features.get("spread").unwrap();
        assert_relative_eq!(*spread, 0.0, epsilon = 0.0001);
    }

    #[tokio::test]
    async fn test_performance_benchmark() {
        use std::time::Instant;
        
        let analyzer = MicrostructureAnalyzer::new();
        
        // Create large dataset
        let mut prices = vec![];
        for i in 0..1000 {
            let price = 100.0 + (i as f64 * 0.01).sin() * 5.0;
            let volume = 1000.0 + (i as f64 * 0.02).cos() * 200.0;
            prices.push((price - 0.5, price + 0.5, price - 1.0, price, volume));
        }
        
        let data = create_test_data(prices);
        let current = data.last().unwrap();
        let historical = &data[..data.len() - 1];
        
        let start = Instant::now();
        let features = analyzer.analyze(current, historical).await.unwrap();
        let duration = start.elapsed();
        
        println!("Computed {} microstructure features in {:?}", features.len(), duration);
        assert!(features.len() > 20, "Should compute many features");
        assert!(duration.as_millis() < 500, "Should complete within 500ms");
    }
}