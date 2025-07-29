//! Test Data Generation Utilities for Real Training System
//!
//! Provides comprehensive utilities for generating realistic market data,
//! feature matrices, and various test scenarios including edge cases.

use chrono::{DateTime, Duration, Utc, Datelike, Timelike};
use rand::{prelude::*, distributions::Uniform};
use std::collections::HashMap;

use crate::{
    common::MarketData,
    realtraining::{TimeSeriesData, Features, ModelType},
};

/// Configuration for test data generation
#[derive(Clone, Debug)]
pub struct DataGenerationConfig {
    pub symbols: Vec<String>,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub interval_seconds: i64,
    pub include_extended_hours: bool,
    pub volatility_factor: f64,
    pub trend_bias: f64,
}

impl Default for DataGenerationConfig {
    fn default() -> Self {
        Self {
            symbols: vec!["AAPL".to_string(), "GOOGL".to_string(), "MSFT".to_string()],
            start_date: Utc::now() - Duration::days(30),
            end_date: Utc::now(),
            interval_seconds: 60,
            include_extended_hours: false,
            volatility_factor: 0.02,
            trend_bias: 0.0001,
        }
    }
}

/// Generate realistic market data with configurable parameters
pub fn generate_market_data(config: DataGenerationConfig) -> Vec<MarketData> {
    let mut rng = thread_rng();
    let mut data = Vec::new();
    
    for symbol in &config.symbols {
        let mut price = 100.0 + rng.gen::<f64>() * 200.0;
        let mut timestamp = config.start_date;
        
        while timestamp <= config.end_date {
            // Skip non-trading hours if configured
            if !config.include_extended_hours && !is_market_hours(timestamp) {
                timestamp = timestamp + Duration::seconds(config.interval_seconds);
                continue;
            }
            
            // Generate realistic price movement
            let volatility = rng.gen::<f64>() * config.volatility_factor;
            let direction = if rng.gen::<f64>() > 0.5 { 1.0 } else { -1.0 };
            let change = direction * volatility + config.trend_bias;
            
            price = (price * (1.0 + change)).max(1.0);
            
            // Generate volume with intraday pattern
            let base_volume = 1_000_000;
            let hour_factor = get_volume_factor(timestamp.hour());
            let random_factor = 0.5 + rng.gen::<f64>();
            let volume = (base_volume as f64 * hour_factor * random_factor) as i64;
            
            // Generate bid/ask spread
            let spread = 0.01 + rng.gen::<f64>() * 0.03;
            let bid = price - spread / 2.0;
            let ask = price + spread / 2.0;
            
            data.push(MarketData {
                timestamp,
                symbol: symbol.clone(),
                price,
                volume,
                bid,
                ask,
                spread,
                market_state: get_market_state(timestamp),
                metadata: generate_metadata(&mut rng),
            });
            
            timestamp = timestamp + Duration::seconds(config.interval_seconds);
        }
    }
    
    data.sort_by_key(|d| d.timestamp);
    data
}

/// Generate data with specific market scenarios
pub mod scenarios {
    use super::*;
    
    /// Generate a bull market scenario
    pub fn bull_market(symbols: Vec<&str>, days: i64) -> Vec<MarketData> {
        let config = DataGenerationConfig {
            symbols: symbols.into_iter().map(String::from).collect(),
            start_date: Utc::now() - Duration::days(days),
            end_date: Utc::now(),
            trend_bias: 0.001, // Positive bias
            ..Default::default()
        };
        generate_market_data(config)
    }
    
    /// Generate a bear market scenario
    pub fn bear_market(symbols: Vec<&str>, days: i64) -> Vec<MarketData> {
        let config = DataGenerationConfig {
            symbols: symbols.into_iter().map(String::from).collect(),
            start_date: Utc::now() - Duration::days(days),
            end_date: Utc::now(),
            trend_bias: -0.001, // Negative bias
            ..Default::default()
        };
        generate_market_data(config)
    }
    
    /// Generate a high volatility scenario
    pub fn high_volatility(symbols: Vec<&str>, days: i64) -> Vec<MarketData> {
        let config = DataGenerationConfig {
            symbols: symbols.into_iter().map(String::from).collect(),
            start_date: Utc::now() - Duration::days(days),
            end_date: Utc::now(),
            volatility_factor: 0.05, // High volatility
            ..Default::default()
        };
        generate_market_data(config)
    }
    
    /// Generate a flash crash scenario
    pub fn flash_crash(symbol: &str, crash_time: DateTime<Utc>) -> Vec<MarketData> {
        let mut data = generate_market_data(DataGenerationConfig {
            symbols: vec![symbol.to_string()],
            start_date: crash_time - Duration::hours(2),
            end_date: crash_time + Duration::hours(2),
            interval_seconds: 10, // High frequency
            ..Default::default()
        });
        
        // Inject flash crash
        let crash_start = crash_time;
        let crash_end = crash_time + Duration::minutes(5);
        
        for point in &mut data {
            if point.timestamp >= crash_start && point.timestamp <= crash_end {
                let crash_progress = (point.timestamp - crash_start).num_seconds() as f64
                    / (crash_end - crash_start).num_seconds() as f64;
                
                // Sharp drop then recovery
                if crash_progress < 0.3 {
                    point.price *= 0.95; // 5% drop
                } else if crash_progress < 0.6 {
                    point.price *= 0.90; // Additional 5% drop
                } else {
                    point.price *= 1.08; // Partial recovery
                }
                
                // Spike in volume
                point.volume *= 10;
            }
        }
        
        data
    }
    
    /// Generate data with gaps (missing data)
    pub fn data_with_gaps(symbols: Vec<&str>, days: i64, gap_probability: f64) -> Vec<MarketData> {
        let mut data = generate_market_data(DataGenerationConfig {
            symbols: symbols.into_iter().map(String::from).collect(),
            start_date: Utc::now() - Duration::days(days),
            end_date: Utc::now(),
            ..Default::default()
        });
        
        // Remove random data points to create gaps
        let mut rng = thread_rng();
        data.retain(|_| rng.gen::<f64>() > gap_probability);
        
        data
    }
    
    /// Generate corrupted data for testing validation
    pub fn corrupted_data(base_size: usize) -> Vec<MarketData> {
        let mut data = generate_market_data(DataGenerationConfig {
            symbols: vec!["TEST".to_string()],
            start_date: Utc::now() - Duration::hours(24),
            end_date: Utc::now(),
            ..Default::default()
        });
        
        let mut rng = thread_rng();
        
        // Inject various types of corruption
        for (i, point) in data.iter_mut().enumerate().take(base_size) {
            match i % 10 {
                0 => point.price = f64::NAN,
                1 => point.price = f64::INFINITY,
                2 => point.price = -100.0,
                3 => point.volume = -1000,
                4 => point.bid = point.ask + 1.0, // Inverted spread
                5 => {
                    point.price = 0.0;
                    point.volume = 0;
                }
                6 => point.timestamp = point.timestamp + Duration::days(365), // Future date
                7 => point.spread = -0.01, // Negative spread
                _ => {
                    // Extreme outlier
                    if rng.gen::<f64>() < 0.1 {
                        point.price *= 100.0;
                    }
                }
            }
        }
        
        data
    }
}

/// Generate feature matrices for testing
pub fn generate_feature_matrix(samples: usize, features: usize) -> Features {
    let mut rng = thread_rng();
    let dist = Uniform::new(-1.0, 1.0);
    
    let mut matrix = ndarray::Array2::zeros((samples, features));
    
    for i in 0..samples {
        for j in 0..features {
            matrix[[i, j]] = rng.sample(dist);
        }
    }
    
    // Add some correlation between features
    for j in 1..features {
        if rng.gen::<f64>() < 0.3 {
            let correlation = rng.gen::<f64>() * 0.7;
            for i in 0..samples {
                matrix[[i, j]] += matrix[[i, j-1]] * correlation;
            }
        }
    }
    
    Features::from(matrix)
}

/// Generate labels for supervised learning
pub fn generate_labels(samples: usize, noise_level: f64) -> ndarray::Array1<f64> {
    let mut rng = thread_rng();
    let mut labels = ndarray::Array1::zeros(samples);
    
    for i in 0..samples {
        // Generate base signal
        let signal = ((i as f64 / 100.0).sin() + 1.0) / 2.0;
        
        // Add noise
        let noise = (rng.gen::<f64>() - 0.5) * noise_level;
        
        labels[i] = (signal + noise).clamp(0.0, 1.0);
    }
    
    labels
}

/// Generate synthetic training dataset
pub fn generate_training_dataset(
    samples: usize,
    features: usize,
    model_type: ModelType,
) -> (Features, ndarray::Array1<f64>) {
    let feature_matrix = generate_feature_matrix(samples, features);
    
    let labels = match model_type {
        ModelType::MLP => generate_labels(samples, 0.1),
        ModelType::LSTM => generate_sequential_labels(samples),
        ModelType::Ensemble => generate_multi_target_labels(samples),
        _ => generate_labels(samples, 0.2),
    };
    
    (feature_matrix, labels)
}

// Helper functions

fn is_market_hours(timestamp: DateTime<Utc>) -> bool {
    let eastern = timestamp.with_timezone(&chrono_tz::US::Eastern);
    let weekday = eastern.weekday();
    let hour = eastern.hour();
    let minute = eastern.minute();
    
    // NYSE regular trading hours: 9:30 AM - 4:00 PM ET, Monday-Friday
    weekday != chrono::Weekday::Sat
        && weekday != chrono::Weekday::Sun
        && ((hour == 9 && minute >= 30) || (hour > 9 && hour < 16))
}

fn get_volume_factor(hour: u32) -> f64 {
    match hour {
        9..=10 => 1.5,   // Opening surge
        11..=12 => 1.0,  // Mid-morning
        13..=14 => 0.8,  // Lunch lull
        15..=16 => 1.3,  // Closing activity
        _ => 0.1,        // After hours
    }
}

fn get_market_state(timestamp: DateTime<Utc>) -> String {
    let eastern = timestamp.with_timezone(&chrono_tz::US::Eastern);
    let hour = eastern.hour();
    let minute = eastern.minute();
    
    if hour < 9 || (hour == 9 && minute < 30) {
        "pre-market".to_string()
    } else if (hour == 9 && minute >= 30) || (hour > 9 && hour < 16) {
        "regular".to_string()
    } else if hour >= 16 && hour < 20 {
        "after-hours".to_string()
    } else {
        "closed".to_string()
    }
}

fn generate_metadata(rng: &mut ThreadRng) -> HashMap<String, serde_json::Value> {
    let mut metadata = HashMap::new();
    
    // Add random metadata fields
    if rng.gen::<f64>() < 0.3 {
        metadata.insert(
            "order_imbalance".to_string(),
            serde_json::json!(rng.gen::<f64>() * 2.0 - 1.0),
        );
    }
    
    if rng.gen::<f64>() < 0.2 {
        metadata.insert(
            "news_sentiment".to_string(),
            serde_json::json!(rng.gen::<f64>() * 2.0 - 1.0),
        );
    }
    
    metadata
}

fn generate_sequential_labels(samples: usize) -> ndarray::Array1<f64> {
    let mut rng = thread_rng();
    let mut labels = ndarray::Array1::zeros(samples);
    
    // Generate autoregressive labels for LSTM
    labels[0] = rng.gen();
    for i in 1..samples {
        labels[i] = labels[i-1] * 0.9 + rng.gen::<f64>() * 0.1;
        labels[i] = labels[i].clamp(0.0, 1.0);
    }
    
    labels
}

fn generate_multi_target_labels(samples: usize) -> ndarray::Array1<f64> {
    let mut rng = thread_rng();
    
    // Generate labels that combine multiple patterns
    let trend = generate_labels(samples, 0.05);
    let cyclic = generate_sequential_labels(samples);
    let noise = ndarray::Array1::from_shape_fn(samples, |_| rng.gen::<f64>());
    
    // Weighted combination
    &trend * 0.4 + &cyclic * 0.4 + &noise * 0.2
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_market_data_generation() {
        let config = DataGenerationConfig::default();
        let data = generate_market_data(config);
        
        assert!(!data.is_empty());
        for point in &data {
            assert!(point.price > 0.0);
            assert!(point.volume >= 0);
            assert!(point.bid < point.ask);
            assert!(point.spread > 0.0);
        }
    }
    
    #[test]
    fn test_scenario_generation() {
        let bull_data = scenarios::bull_market(vec!["AAPL"], 5);
        let bear_data = scenarios::bear_market(vec!["AAPL"], 5);
        
        // Calculate average returns
        let bull_return = calculate_return(&bull_data);
        let bear_return = calculate_return(&bear_data);
        
        assert!(bull_return > 0.0, "Bull market should have positive returns");
        assert!(bear_return < 0.0, "Bear market should have negative returns");
    }
    
    fn calculate_return(data: &[MarketData]) -> f64 {
        if data.len() < 2 {
            return 0.0;
        }
        
        let first_price = data.first().unwrap().price;
        let last_price = data.last().unwrap().price;
        
        (last_price - first_price) / first_price
    }
}