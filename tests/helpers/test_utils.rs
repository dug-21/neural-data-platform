//! Test Utilities and Infrastructure for Neural Trader Clean Architecture
//!
//! This module provides comprehensive testing utilities including:
//! - Test data generation and fixtures
//! - Mock implementations for testing
//! - Performance measurement utilities
//! - Test configuration and setup
//! - Architecture validation helpers

use std::collections::HashMap;
use std::time::{Duration, Instant};
use chrono::Utc;
use anyhow::Result;
use tokio::sync::mpsc;

use crate::data::TimeSeriesData;
use crate::config::NeuralConfig;
use crate::neural::{PredictionResult, PerformanceEvent};

/// Test configuration builder
pub struct TestConfigBuilder {
    config: NeuralConfig,
}

impl TestConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: NeuralConfig {
                memory_gb: 1.0,
                models: vec!["MLP".to_string(), "LSTM".to_string()],
                prediction_cache_ttl: 300,
                model_load_timeout: 60,
                max_concurrent_predictions: 10,
                enable_model_monitoring: false, // Disabled for tests
                accuracy_threshold: 0.8,
                use_real_models: false, // Always use mock models in tests
                enable_health_checks: false, // Disabled for simple tests
                enable_fallback: false, // Disabled for unit tests
                lookback_window: 24,
                enable_circuit_breakers: false, // Disabled for unit tests
                enable_graceful_degradation: false,
                enable_performance_monitoring: false,
                enable_adaptive_retry: false,
                enable_model_ensembles: false,
                model_timeout_seconds: 5, // Short timeout for tests
                max_retries: 1, // Minimal retries for tests
                error_threshold: 0.1,
            },
        }
    }

    pub fn with_health_monitoring(mut self) -> Self {
        self.config.enable_health_checks = true;
        self
    }

    pub fn with_fallback(mut self) -> Self {
        self.config.enable_fallback = true;
        self
    }

    pub fn with_circuit_breakers(mut self) -> Self {
        self.config.enable_circuit_breakers = true;
        self
    }

    pub fn with_performance_monitoring(mut self) -> Self {
        self.config.enable_performance_monitoring = true;
        self
    }

    pub fn with_models(mut self, models: Vec<String>) -> Self {
        self.config.models = models;
        self
    }

    pub fn build(self) -> NeuralConfig {
        self.config
    }
}

/// Test data generator
pub struct TestDataGenerator;

impl TestDataGenerator {
    /// Generate simple test data for basic functionality tests
    pub fn generate_simple_data(size: usize) -> Vec<TimeSeriesData> {
        let mut data = Vec::with_capacity(size);
        let base_price = 100.0;
        let base_time = Utc::now();

        for i in 0..size {
            let price_change = (i as f64 * 0.1) % 10.0 - 5.0; // Oscillating pattern
            let price = base_price + price_change;
            
            data.push(TimeSeriesData {
                symbol: "TEST".to_string(),
                timestamp: base_time + chrono::Duration::seconds(i as i64 * 60),
                open: price - 0.5,
                high: price + 1.0,
                low: price - 1.0,
                close: price,
                volume: vec![1000.0 + (i as f64 * 10.0)],
                indicators: HashMap::from([
                    ("rsi".to_string(), 50.0 + (i as f64 % 50.0)),
                    ("macd".to_string(), (i as f64 % 20.0) - 10.0),
                    ("bb_upper".to_string(), price + 2.0),
                    ("bb_lower".to_string(), price - 2.0),
                ]),
                source: Some("test_generator".to_string()),
                entity: Some("test".to_string()),
                value: Some(price),
                metadata: None,
            });
        }

        data
    }

    /// Generate trending data for performance tests
    pub fn generate_trending_data(size: usize, trend: f64) -> Vec<TimeSeriesData> {
        let mut data = Vec::with_capacity(size);
        let base_price = 100.0;
        let base_time = Utc::now();

        for i in 0..size {
            let price = base_price + (i as f64 * trend);
            let volatility = 1.0 + (i as f64 * 0.01); // Increasing volatility
            
            data.push(TimeSeriesData {
                symbol: "TREND_TEST".to_string(),
                timestamp: base_time + chrono::Duration::seconds(i as i64 * 60),
                open: price - (volatility * 0.5),
                high: price + volatility,
                low: price - volatility,
                close: price,
                volume: vec![2000.0 + (i as f64 * 50.0)],
                indicators: HashMap::from([
                    ("rsi".to_string(), 30.0 + (i as f64 % 40.0)),
                    ("sma_20".to_string(), price - (i as f64 * 0.1)),
                    ("ema_12".to_string(), price + (i as f64 * 0.05)),
                ]),
                source: Some("trend_generator".to_string()),
                entity: Some("performance_test".to_string()),
                value: Some(price),
                metadata: None,
            });
        }

        data
    }

    /// Generate edge case data for stress testing
    pub fn generate_edge_case_data() -> Vec<TimeSeriesData> {
        vec![
            // Zero values
            TimeSeriesData {
                symbol: "EDGE_ZERO".to_string(),
                timestamp: Utc::now(),
                open: 0.0,
                high: 0.0,
                low: 0.0,
                close: 0.0,
                volume: vec![0.0],
                indicators: HashMap::new(),
                source: Some("edge_case".to_string()),
                entity: Some("zero_test".to_string()),
                value: Some(0.0),
                metadata: None,
            },
            // Extreme values
            TimeSeriesData {
                symbol: "EDGE_EXTREME".to_string(),
                timestamp: Utc::now(),
                open: f64::MAX / 2.0,
                high: f64::MAX / 2.0,
                low: f64::MIN / 2.0,
                close: f64::MAX / 4.0,
                volume: vec![f64::MAX / 10.0],
                indicators: HashMap::from([
                    ("extreme_indicator".to_string(), f64::MAX / 100.0),
                ]),
                source: Some("edge_case".to_string()),
                entity: Some("extreme_test".to_string()),
                value: Some(f64::MAX / 4.0),
                metadata: None,
            },
            // NaN values (should be handled gracefully)
            TimeSeriesData {
                symbol: "EDGE_NAN".to_string(),
                timestamp: Utc::now(),
                open: 100.0,
                high: 101.0,
                low: 99.0,
                close: 100.5,
                volume: vec![1000.0],
                indicators: HashMap::from([
                    ("nan_indicator".to_string(), f64::NAN),
                ]),
                source: Some("edge_case".to_string()),
                entity: Some("nan_test".to_string()),
                value: Some(100.5),
                metadata: None,
            },
        ]
    }
}

/// Performance measurement utilities
pub struct PerformanceMeasurement {
    start_time: Instant,
    name: String,
}

impl PerformanceMeasurement {
    pub fn start(name: &str) -> Self {
        Self {
            start_time: Instant::now(),
            name: name.to_string(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn assert_under_threshold(&self, threshold: Duration) {
        let elapsed = self.elapsed();
        assert!(
            elapsed <= threshold,
            "Performance test '{}' failed: {}ms > {}ms threshold",
            self.name,
            elapsed.as_millis(),
            threshold.as_millis()
        );
    }

    pub fn report(&self) -> String {
        format!("{}: {}ms", self.name, self.elapsed().as_millis())
    }
}

/// Memory usage tracker
pub struct MemoryTracker {
    initial_memory: u64,
    name: String,
}

impl MemoryTracker {
    pub fn start(name: &str) -> Self {
        let initial_memory = Self::get_memory_usage();
        Self {
            initial_memory,
            name: name.to_string(),
        }
    }

    fn get_memory_usage() -> u64 {
        // Simplified memory tracking - in real implementation would use proper system calls
        std::process::Command::new("ps")
            .args(["-o", "rss=", "-p"])
            .arg(std::process::id().to_string())
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8(output.stdout)
                    .ok()?
                    .trim()
                    .parse::<u64>()
                    .ok()
            })
            .unwrap_or(0)
    }

    pub fn memory_increase(&self) -> u64 {
        let current = Self::get_memory_usage();
        current.saturating_sub(self.initial_memory)
    }

    pub fn assert_under_threshold(&self, threshold_mb: u64) {
        let increase_kb = self.memory_increase();
        let increase_mb = increase_kb / 1024;
        assert!(
            increase_mb <= threshold_mb,
            "Memory test '{}' failed: {}MB > {}MB threshold",
            self.name,
            increase_mb,
            threshold_mb
        );
    }
}

/// Mock performance event collector
pub struct MockPerformanceCollector {
    pub events: std::sync::Arc<std::sync::Mutex<Vec<PerformanceEvent>>>,
    pub sender: mpsc::UnboundedSender<PerformanceEvent>,
    pub receiver: mpsc::UnboundedReceiver<PerformanceEvent>,
}

impl MockPerformanceCollector {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            events: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            sender,
            receiver,
        }
    }

    pub async fn collect_events(&mut self, timeout: Duration) -> Vec<PerformanceEvent> {
        let mut collected = Vec::new();
        let deadline = Instant::now() + timeout;

        while Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(10), self.receiver.recv()).await {
                Ok(Some(event)) => {
                    collected.push(event);
                }
                Ok(None) => break,
                Err(_) => continue,
            }
        }

        // Store in shared events collection
        if let Ok(mut events) = self.events.lock() {
            events.extend(collected.clone());
        }

        collected
    }

    pub fn get_collected_events(&self) -> Vec<PerformanceEvent> {
        self.events.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

/// Test result validator
pub struct TestResultValidator;

impl TestResultValidator {
    /// Validate prediction results have expected structure and values
    pub fn validate_predictions(
        predictions: &[PredictionResult],
        expected_count: usize,
        min_confidence: f64,
    ) -> Result<()> {
        // Check count
        if predictions.len() != expected_count {
            return Err(anyhow::anyhow!(
                "Expected {} predictions, got {}",
                expected_count,
                predictions.len()
            ));
        }

        // Validate each prediction
        for (i, pred) in predictions.iter().enumerate() {
            // Check confidence range
            if pred.confidence < 0.0 || pred.confidence > 1.0 {
                return Err(anyhow::anyhow!(
                    "Prediction {} has invalid confidence: {}",
                    i,
                    pred.confidence
                ));
            }

            // Check minimum confidence
            if pred.confidence < min_confidence {
                return Err(anyhow::anyhow!(
                    "Prediction {} confidence {} below minimum {}",
                    i,
                    pred.confidence,
                    min_confidence
                ));
            }

            // Check for reasonable values (not NaN/infinite)
            if !pred.value.is_finite() {
                return Err(anyhow::anyhow!(
                    "Prediction {} has invalid value: {}",
                    i,
                    pred.value
                ));
            }

            // Check model name is set
            if pred.model_name.is_empty() {
                return Err(anyhow::anyhow!(
                    "Prediction {} missing model name",
                    i
                ));
            }
        }

        Ok(())
    }

    /// Validate performance metrics are within expected ranges
    pub fn validate_performance_metrics(
        elapsed: Duration,
        max_latency: Duration,
        memory_mb: u64,
        max_memory_mb: u64,
    ) -> Result<()> {
        if elapsed > max_latency {
            return Err(anyhow::anyhow!(
                "Performance test failed: {}ms > {}ms latency threshold",
                elapsed.as_millis(),
                max_latency.as_millis()
            ));
        }

        if memory_mb > max_memory_mb {
            return Err(anyhow::anyhow!(
                "Memory test failed: {}MB > {}MB threshold",
                memory_mb,
                max_memory_mb
            ));
        }

        Ok(())
    }
}

/// File line counter for architecture tests
pub fn count_lines_in_file(file_path: &str) -> Result<usize> {
    use std::fs;
    let content = fs::read_to_string(file_path)?;
    let lines = content.lines().count();
    Ok(lines)
}

/// Module discovery for architecture tests
pub fn discover_rust_modules(base_path: &str) -> Result<Vec<String>> {
    use std::fs;
    use std::path::Path;

    let mut modules = Vec::new();
    
    fn visit_dir(dir: &Path, modules: &mut Vec<String>) -> Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_dir() {
                    visit_dir(&path, modules)?;
                } else if let Some(ext) = path.extension() {
                    if ext == "rs" {
                        if let Some(path_str) = path.to_str() {
                            modules.push(path_str.to_string());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    visit_dir(Path::new(base_path), &mut modules)?;
    Ok(modules)
}

/// Test timeout wrapper
pub async fn with_timeout<F, T>(future: F, timeout: Duration) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    match tokio::time::timeout(timeout, future).await {
        Ok(result) => result,
        Err(_) => Err(anyhow::anyhow!("Test timed out after {:?}", timeout)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = TestConfigBuilder::new()
            .with_health_monitoring()
            .with_models(vec!["TEST_MODEL".to_string()])
            .build();

        assert!(config.enable_health_checks);
        assert_eq!(config.models, vec!["TEST_MODEL"]);
        assert!(!config.use_real_models); // Should always be false for tests
    }

    #[test]
    fn test_data_generation() {
        let data = TestDataGenerator::generate_simple_data(10);
        assert_eq!(data.len(), 10);
        
        for point in &data {
            assert_eq!(point.symbol, "TEST");
            assert!(point.close > 0.0);
            assert!(!point.indicators.is_empty());
        }
    }

    #[test]
    fn test_performance_measurement() {
        let measurement = PerformanceMeasurement::start("test");
        std::thread::sleep(Duration::from_millis(10));
        assert!(measurement.elapsed() >= Duration::from_millis(10));
    }

    #[test]
    fn test_result_validation() {
        let predictions = vec![
            PredictionResult {
                value: 100.0,
                confidence: 0.8,
                model_name: "TEST".to_string(),
                timestamp: Utc::now(),
                metadata: HashMap::new(),
            }
        ];

        let result = TestResultValidator::validate_predictions(&predictions, 1, 0.7);
        assert!(result.is_ok());
    }
}