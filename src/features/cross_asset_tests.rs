//! Comprehensive tests for cross-asset correlation analysis module
//! 
//! Tests cover enhanced correlations, dynamic analysis, and regime detection

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::TimeSeriesData;
    use crate::features::cross_asset::{CrossAssetCorrelationEngine, CorrelationRegime, RotationSignal};
    use chrono::{DateTime, Utc, TimeZone};
    use std::collections::HashMap;
    use approx::assert_relative_eq;

    /// Helper function to create test time series data
    fn create_test_data(symbol: &str, prices: Vec<f64>, volumes: Vec<f64>) -> Vec<TimeSeriesData> {
        prices.iter().zip(volumes.iter()).enumerate().map(|(i, (&price, &volume))| {
            TimeSeriesData {
                timestamp: Utc.timestamp_opt(1640000000 + (i as i64 * 3600), 0).unwrap(),
                symbol: symbol.to_string(),
                open: price - 0.5,
                high: price + 0.5,
                low: price - 1.0,
                close: price,
                volume,
            }
        }).collect()
    }

    /// Create correlated market data
    fn create_correlated_market_data() -> HashMap<String, Vec<TimeSeriesData>> {
        let mut market_data = HashMap::new();
        
        // Base series (e.g., main asset)
        let base_prices: Vec<f64> = (0..100).map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0).collect();
        let base_volumes: Vec<f64> = vec![1000.0; 100];
        
        // Highly correlated with SPY (correlation ~0.9)
        let spy_prices: Vec<f64> = base_prices.iter()
            .map(|&p| p * 1.05 + (rand::random::<f64>() - 0.5) * 2.0)
            .collect();
        market_data.insert("SPY".to_string(), create_test_data("SPY", spy_prices, base_volumes.clone()));
        
        // Moderately correlated with QQQ (correlation ~0.6)
        let qqq_prices: Vec<f64> = base_prices.iter()
            .map(|&p| p * 1.1 + (rand::random::<f64>() - 0.5) * 5.0)
            .collect();
        market_data.insert("QQQ".to_string(), create_test_data("QQQ", qqq_prices, base_volumes.clone()));
        
        // Negatively correlated with VIX (correlation ~-0.7)
        let vix_prices: Vec<f64> = base_prices.iter()
            .map(|&p| 150.0 - p * 0.5 + (rand::random::<f64>() - 0.5) * 3.0)
            .collect();
        market_data.insert("VIX".to_string(), create_test_data("VIX", vix_prices, base_volumes.clone()));
        
        // Uncorrelated with DXY (correlation ~0.0)
        let dxy_prices: Vec<f64> = (0..100)
            .map(|i| 90.0 + (i as f64 * 0.2).cos() * 5.0)
            .collect();
        market_data.insert("DXY".to_string(), create_test_data("DXY", dxy_prices, base_volumes.clone()));
        
        // Sector ETFs with varying correlations
        let xlf_prices: Vec<f64> = base_prices.iter()
            .map(|&p| p * 0.95 + (rand::random::<f64>() - 0.5) * 3.0)
            .collect();
        market_data.insert("XLF".to_string(), create_test_data("XLF", xlf_prices, base_volumes.clone()));
        
        let xlk_prices: Vec<f64> = base_prices.iter()
            .map(|&p| p * 1.15 + (rand::random::<f64>() - 0.5) * 4.0)
            .collect();
        market_data.insert("XLK".to_string(), create_test_data("XLK", xlk_prices, base_volumes.clone()));
        
        // Commodities
        let gld_prices: Vec<f64> = (0..100)
            .map(|i| 150.0 + (i as f64 * 0.15).sin() * 8.0)
            .collect();
        market_data.insert("GLD".to_string(), create_test_data("GLD", gld_prices, base_volumes.clone()));
        
        // Interest rates
        let tlt_prices: Vec<f64> = base_prices.iter()
            .map(|&p| 130.0 - p * 0.3 + (rand::random::<f64>() - 0.5) * 2.0)
            .collect();
        market_data.insert("TLT".to_string(), create_test_data("TLT", tlt_prices, base_volumes.clone()));
        
        // Target asset
        market_data.insert("TEST".to_string(), create_test_data("TEST", base_prices, base_volumes));
        
        market_data
    }

    /// Create regime change market data
    fn create_regime_change_data() -> HashMap<String, Vec<TimeSeriesData>> {
        let mut market_data = HashMap::new();
        
        // Create data with regime change
        let mut spy_prices = vec![];
        let mut test_prices = vec![];
        
        // Period 1: High correlation
        for i in 0..50 {
            let base = 100.0 + (i as f64 * 0.5);
            spy_prices.push(base);
            test_prices.push(base * 0.95 + (rand::random::<f64>() - 0.5) * 1.0);
        }
        
        // Period 2: Decorrelation
        for i in 50..100 {
            let spy = 125.0 + ((i - 50) as f64 * 0.3);
            let test = 120.0 - ((i - 50) as f64 * 0.2);
            spy_prices.push(spy);
            test_prices.push(test);
        }
        
        // Period 3: Negative correlation
        for i in 100..150 {
            let spy = 140.0 + ((i - 100) as f64 * 0.4);
            let test = 110.0 - ((i - 100) as f64 * 0.3);
            spy_prices.push(spy);
            test_prices.push(test);
        }
        
        let volumes = vec![1000.0; 150];
        market_data.insert("SPY".to_string(), create_test_data("SPY", spy_prices, volumes.clone()));
        market_data.insert("TEST".to_string(), create_test_data("TEST", test_prices, volumes));
        
        market_data
    }

    #[tokio::test]
    async fn test_index_correlations() {
        let engine = CrossAssetCorrelationEngine::new();
        let market_data = create_correlated_market_data();
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        
        // Check SPY correlations at different periods
        assert!(features.contains_key("corr_spy_20"));
        assert!(features.contains_key("corr_spy_60"));
        assert!(features.contains_key("corr_spy_120"));
        assert!(features.contains_key("corr_spy_252"));
        
        // Check correlation values are in valid range
        for period in &[20, 60, 120, 252] {
            if let Some(&corr) = features.get(&format!("corr_spy_{}", period)) {
                assert!(corr >= -1.0 && corr <= 1.0, "Correlation should be between -1 and 1");
            }
        }
        
        // Check rolling correlations
        assert!(features.contains_key("rolling_corr_spy_10"));
        assert!(features.contains_key("rolling_corr_spy_20"));
        assert!(features.contains_key("rolling_corr_spy_40"));
        
        // Check VIX negative correlation
        if let Some(&vix_corr) = features.get("corr_vix_60") {
            assert!(vix_corr < 0.0, "VIX should be negatively correlated");
        }
    }

    #[tokio::test]
    async fn test_sector_correlations() {
        let engine = CrossAssetCorrelationEngine::new();
        let market_data = create_correlated_market_data();
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        
        // Check sector correlations
        let sectors = vec!["financials", "technology"];
        for sector in sectors {
            assert!(features.contains_key(&format!("sector_corr_{}", sector)));
        }
        
        // Check dominant sector detection
        assert!(features.contains_key("dominant_sector_corr"));
        let dominant_corr = features.get("dominant_sector_corr").unwrap();
        assert!(dominant_corr.abs() <= 1.0);
        
        // Check if a dominant sector is identified
        let sector_keys: Vec<String> = features.keys()
            .filter(|k| k.starts_with("is_dominant_sector_"))
            .cloned()
            .collect();
        assert!(!sector_keys.is_empty(), "Should identify a dominant sector");
    }

    #[tokio::test]
    async fn test_currency_correlations() {
        let engine = CrossAssetCorrelationEngine::new();
        let market_data = create_correlated_market_data();
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        
        // Check DXY correlation
        assert!(features.contains_key("currency_corr_dollar_index"));
        
        // Check currency sensitivity
        let dxy_corr = features.get("currency_corr_dollar_index").unwrap();
        if dxy_corr.abs() > 0.5 {
            assert!(features.contains_key("currency_sensitive_dollar_index"));
        }
    }

    #[tokio::test]
    async fn test_commodity_correlations() {
        let engine = CrossAssetCorrelationEngine::new();
        let market_data = create_correlated_market_data();
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        
        // Check gold correlation
        assert!(features.contains_key("commodity_corr_gold"));
        
        // Check lead-lag analysis
        assert!(features.contains_key("lead_lag_gold"));
        let lead_lag = features.get("lead_lag_gold").unwrap();
        assert!(lead_lag.abs() <= 1.0, "Lead-lag correlation should be valid");
    }

    #[tokio::test]
    async fn test_interest_rate_correlations() {
        let engine = CrossAssetCorrelationEngine::new();
        let market_data = create_correlated_market_data();
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        
        // Check TLT correlation
        assert!(features.contains_key("rate_corr_long_term_rates"));
        
        // Check rate sensitivity classification
        let has_rate_sensitivity = features.contains_key("rate_sensitive_negative") ||
                                  features.contains_key("rate_sensitive_positive");
        assert!(has_rate_sensitivity, "Should classify rate sensitivity");
    }

    #[tokio::test]
    async fn test_dynamic_correlations() {
        let engine = CrossAssetCorrelationEngine::new();
        let market_data = create_correlated_market_data();
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        
        // Check DCC features
        assert!(features.contains_key("dcc_spy"));
        assert!(features.contains_key("dcc_tlt"));
        assert!(features.contains_key("dcc_gld"));
        assert!(features.contains_key("dcc_dxy"));
        
        // Check correlation trends
        assert!(features.contains_key("dcc_trend_spy"));
        
        // Verify DCC values are in valid range
        for asset in &["spy", "tlt", "gld", "dxy"] {
            if let Some(&dcc) = features.get(&format!("dcc_{}", asset)) {
                assert!(dcc >= -1.0 && dcc <= 1.0, "DCC should be between -1 and 1");
            }
        }
    }

    #[tokio::test]
    async fn test_correlation_regimes() {
        let engine = CrossAssetCorrelationEngine::new();
        let market_data = create_regime_change_data();
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        
        // Check regime detection features
        let regime_features = vec![
            "high_correlation_regime",
            "negative_correlation_regime",
            "stable_correlation_regime",
            "volatile_correlation_regime"
        ];
        
        let mut regime_found = false;
        for regime in regime_features {
            if features.contains_key(regime) {
                regime_found = true;
                let value = features.get(regime).unwrap();
                assert_eq!(*value, 1.0, "Regime indicator should be 1.0");
            }
        }
        assert!(regime_found, "Should detect at least one correlation regime");
    }

    #[tokio::test]
    async fn test_cross_asset_momentum() {
        let engine = CrossAssetCorrelationEngine::new();
        let market_data = create_correlated_market_data();
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        
        // Check momentum indicators exist
        let momentum_keys: Vec<String> = features.keys()
            .filter(|k| k.contains("momentum"))
            .cloned()
            .collect();
        assert!(!momentum_keys.is_empty(), "Should compute momentum features");
    }

    #[tokio::test]
    async fn test_sector_rotation_signals() {
        let engine = CrossAssetCorrelationEngine::new();
        let market_data = create_correlated_market_data();
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        
        // Check rotation signal features
        let rotation_keys: Vec<String> = features.keys()
            .filter(|k| k.contains("rotation"))
            .cloned()
            .collect();
        assert!(!rotation_keys.is_empty(), "Should compute rotation signals");
    }

    #[tokio::test]
    async fn test_market_betas() {
        let engine = CrossAssetCorrelationEngine::new();
        let market_data = create_correlated_market_data();
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        
        // Check beta calculations
        let benchmarks = vec!["market", "tech"];
        let periods = vec![20, 60, 252];
        
        for benchmark in benchmarks {
            for period in &periods {
                let key = format!("beta_{}_{}", benchmark, period);
                if features.contains_key(&key) {
                    let beta = features.get(&key).unwrap();
                    assert!(*beta >= -5.0 && *beta <= 5.0, "Beta should be reasonable");
                }
            }
        }
        
        // Check beta classification
        let beta_class_keys: Vec<String> = features.keys()
            .filter(|k| k.starts_with("high_beta_") || k.starts_with("low_beta_"))
            .cloned()
            .collect();
        assert!(!beta_class_keys.is_empty(), "Should classify beta");
        
        // Check rolling beta
        assert!(features.contains_key("rolling_beta_market"));
    }

    #[tokio::test]
    async fn test_correlation_stability() {
        let engine = CrossAssetCorrelationEngine::new();
        let market_data = create_correlated_market_data();
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        
        // Check stability metrics
        assert!(features.contains_key("correlation_stability"));
        let stability = features.get("correlation_stability").unwrap();
        assert!(*stability >= 0.0, "Stability should be non-negative");
    }

    #[tokio::test]
    async fn test_carry_trade_indicators() {
        let engine = CrossAssetCorrelationEngine::new();
        let market_data = create_correlated_market_data();
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        
        // Check carry trade features
        let carry_keys: Vec<String> = features.keys()
            .filter(|k| k.contains("carry"))
            .cloned()
            .collect();
        // Carry trade indicators might not always be present
        println!("Carry trade features found: {:?}", carry_keys);
    }

    #[tokio::test]
    async fn test_correlation_calculation_accuracy() {
        let engine = CrossAssetCorrelationEngine::new();
        
        // Create perfectly correlated data
        let prices1: Vec<f64> = (0..100).map(|i| 100.0 + i as f64).collect();
        let prices2 = prices1.clone();
        let volumes = vec![1000.0; 100];
        
        let mut market_data = HashMap::new();
        market_data.insert("TEST".to_string(), create_test_data("TEST", prices1, volumes.clone()));
        market_data.insert("SPY".to_string(), create_test_data("SPY", prices2, volumes));
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        
        // Perfect correlation should be very close to 1.0
        if let Some(&corr) = features.get("corr_spy_60") {
            assert_relative_eq!(corr, 1.0, epsilon = 0.01);
        }
    }

    #[tokio::test]
    async fn test_edge_cases() {
        let engine = CrossAssetCorrelationEngine::new();
        
        // Test with minimal market data
        let mut market_data = HashMap::new();
        let prices = vec![100.0, 101.0, 102.0];
        let volumes = vec![1000.0; 3];
        market_data.insert("TEST".to_string(), create_test_data("TEST", prices.clone(), volumes.clone()));
        market_data.insert("SPY".to_string(), create_test_data("SPY", prices, volumes));
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        assert!(!features.is_empty(), "Should return some features with minimal data");
        
        // Test with missing target symbol
        let empty_market = HashMap::new();
        let result = engine.compute_correlations("MISSING", &empty_market).await;
        assert!(result.is_err(), "Should error on missing target symbol");
    }

    #[tokio::test]
    async fn test_performance_benchmark() {
        use std::time::Instant;
        
        let engine = CrossAssetCorrelationEngine::new();
        
        // Create large market dataset
        let mut market_data = HashMap::new();
        let assets = vec!["TEST", "SPY", "QQQ", "IWM", "DIA", "VIX", "TLT", "GLD", "DXY",
                         "XLF", "XLK", "XLE", "XLV", "XLI", "XLY", "XLP", "XLU", "XLB"];
        
        for asset in assets {
            let prices: Vec<f64> = (0..500)
                .map(|i| 100.0 + (i as f64 * 0.1).sin() * 10.0 + rand::random::<f64>() * 2.0)
                .collect();
            let volumes = vec![1000.0; 500];
            market_data.insert(asset.to_string(), create_test_data(asset, prices, volumes));
        }
        
        let start = Instant::now();
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        let duration = start.elapsed();
        
        println!("Computed {} cross-asset features in {:?}", features.len(), duration);
        assert!(features.len() > 50, "Should compute many cross-asset features");
        assert!(duration.as_millis() < 1000, "Should complete within 1 second");
    }

    #[tokio::test]
    async fn test_multiple_correlation_windows() {
        let engine = CrossAssetCorrelationEngine::new();
        let market_data = create_correlated_market_data();
        
        let features = engine.compute_correlations("TEST", &market_data).await.unwrap();
        
        // Check that different window sizes produce different correlations
        let windows = vec![10, 20, 40, 60];
        let mut spy_corrs = vec![];
        
        for window in windows {
            let key = format!("rolling_corr_spy_{}", window);
            if let Some(&corr) = features.get(&key) {
                spy_corrs.push(corr);
            }
        }
        
        // Different windows should generally produce different correlations
        if spy_corrs.len() > 1 {
            let all_same = spy_corrs.windows(2).all(|w| (w[0] - w[1]).abs() < 0.001);
            assert!(!all_same, "Different windows should produce different correlations");
        }
    }
}