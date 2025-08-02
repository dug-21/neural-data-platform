//! Common test utilities and shared fixtures
//!
//! This module provides reusable test data generators, mock objects,
//! and utility functions for integration testing.
//!
//! SPARC Architecture:
//! - Specification: Shared test utilities for unit and integration tests
//! - Pseudocode: Helper functions and mock data generators
//! - Architecture: Modular test fixtures with builders
//! - Refinement: Extensible for new test scenarios
//! - Completion: Ready-to-use test helpers

use autonomous_platform::adapters::{MarketData, OrderBook, OrderBookEntry};
use autonomous_platform::config::{
    AlertsConfig, BackupConfig, CircuitBreakerConfig, DatabaseConfig, DevelopmentConfig,
    GracefulShutdownConfig, LoggingConfig, MonitoringConfig, NeuralConfig, ObservabilityConfig,
    PerformanceConfig, PlatformConfig, PlatformInfo, RedisConfig, SecurityConfig,
};
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::strategies::{MarketContext, Position, PositionSide};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Create a test configuration for integration tests
pub fn create_test_config() -> PlatformConfig {
    PlatformConfig {
        platform: PlatformInfo {
            name: "integration-test-platform".to_string(),
            version: "0.1.0".to_string(),
            environment: "development".to_string(),
            log_level: "info".to_string(),
        },
        database: DatabaseConfig {
            url: "postgres://test@localhost/neural_trader_test".to_string(),
            max_connections: 20,
            min_connections: 5,
            connection_timeout: 30,
            idle_timeout: 600,
            max_query_time: 30,
        },
        redis: RedisConfig {
            url: "redis://localhost:6379".to_string(),
            max_connections: 10,
            default_ttl_seconds: 300,
            connection_timeout_ms: 5000,
            cluster_mode: false,
            pool_max_idle: 10,
            pool_timeout_seconds: 30,
        },
        neural: NeuralConfig {
            memory_gb: 4.0,
            models: vec![
                "NHITS".to_string(),
                "DeepAR".to_string(),
                "TCN".to_string(),
                "MLP".to_string(),
            ],
            prediction_cache_ttl: 600,
            model_load_timeout: 300,
            max_concurrent_predictions: 50,
            enable_model_monitoring: true,
            accuracy_threshold: 0.85,
        },
        monitoring: MonitoringConfig {
            metrics_interval_secs: 30,
            quality_threshold: 0.95,
            prometheus_port: Some(8080),
            prometheus_path: "/metrics".to_string(),
            enable_performance_metrics: true,
            enable_memory_monitoring: true,
            enable_error_monitoring: true,
            cpu_usage_threshold: 80.0,
            memory_usage_threshold: 85.0,
            error_rate_threshold: 0.05,
        },
        observability: ObservabilityConfig::default(),
        security: SecurityConfig::default(),
        performance: PerformanceConfig::default(),
        logging: LoggingConfig::default(),
        alerts: AlertsConfig::default(),
        backup: BackupConfig::default(),
        circuit_breaker: CircuitBreakerConfig::default(),
        graceful_shutdown: GracefulShutdownConfig::default(),
        development: DevelopmentConfig {
            enable_hot_reload: false,
            enable_debug_endpoints: true,
            mock_external_services: true,
            enable_test_data_generation: true,
            enable_profiling: false,
            seed_test_data: false,
            reset_database_on_startup: false,
        },
    }
}

/// Create realistic market data for testing
pub fn create_realistic_market_data(
    symbol: &str,
    base_price: f64,
    volatility: f64,
) -> TimeSeriesData {
    let timestamp = Utc::now();
    let price_variation = base_price * volatility * (rand::random::<f64>() - 0.5);
    let current_price = base_price + price_variation;

    TimeSeriesData {
        symbol: symbol.to_string(),
        timestamp,
        open: current_price - (base_price * 0.01),
        high: current_price + (base_price * 0.02),
        low: current_price - (base_price * 0.015),
        close: current_price,
        volume: 1000000.0 + (rand::random::<f64>() * 500000.0),
        indicators: create_realistic_indicators(current_price, base_price),
    }
}

/// Create realistic technical indicators
pub fn create_realistic_indicators(current_price: f64, base_price: f64) -> HashMap<String, f64> {
    let mut indicators = HashMap::new();

    // RSI (0-100, realistic range 20-80)
    indicators.insert("RSI".to_string(), 30.0 + (rand::random::<f64>() * 40.0));

    // MACD
    indicators.insert("MACD".to_string(), (current_price - base_price) * 0.1);

    // Bollinger Bands
    indicators.insert("BB_UPPER".to_string(), current_price * 1.02);
    indicators.insert("BB_LOWER".to_string(), current_price * 0.98);
    indicators.insert("BB_MIDDLE".to_string(), current_price);

    // Moving Averages
    indicators.insert("SMA_20".to_string(), base_price);
    indicators.insert("EMA_12".to_string(), current_price * 0.99);
    indicators.insert("EMA_26".to_string(), current_price * 1.01);

    // Volume indicators
    indicators.insert("VOLUME_SMA".to_string(), 800000.0);
    indicators.insert("OBV".to_string(), 50000000.0);

    indicators
}

/// Create high volatility market conditions for stress testing
pub fn create_high_volatility_market_data(symbol: &str, base_price: f64) -> TimeSeriesData {
    create_realistic_market_data(symbol, base_price, 0.15) // 15% volatility
}

/// Create low volatility market conditions
pub fn create_low_volatility_market_data(symbol: &str, base_price: f64) -> TimeSeriesData {
    create_realistic_market_data(symbol, base_price, 0.02) // 2% volatility
}

/// Create context metadata for agent decisions
pub fn create_decision_metadata(
    strategy: &str,
    risk_level: f64,
) -> HashMap<String, serde_json::Value> {
    let mut metadata = HashMap::new();
    metadata.insert("strategy".to_string(), json!(strategy));
    metadata.insert("risk_level".to_string(), json!(risk_level));
    metadata.insert("position_size".to_string(), json!(0.1));
    metadata.insert("max_drawdown".to_string(), json!(0.05));
    metadata.insert("session".to_string(), json!("US_MARKET_HOURS"));
    metadata.insert("test_mode".to_string(), json!(true));
    metadata.insert("timestamp".to_string(), json!(Utc::now().timestamp()));
    metadata
}

/// Create a batch of market data for multiple symbols
pub fn create_market_data_batch(symbols: &[&str], base_prices: &[f64]) -> Vec<TimeSeriesData> {
    symbols
        .iter()
        .zip(base_prices.iter())
        .map(|(symbol, price)| create_realistic_market_data(symbol, *price, 0.05))
        .collect()
}

/// Create streaming market data with sequence numbers
pub fn create_streaming_market_data(
    symbol: &str,
    base_price: f64,
    sequence: u64,
) -> TimeSeriesData {
    let mut data = create_realistic_market_data(symbol, base_price, 0.03);
    // Add streaming-specific metadata
    data.indicators
        .insert("SEQUENCE".to_string(), sequence as f64);
    data.indicators
        .insert("LATENCY_MS".to_string(), rand::random::<f64>() * 10.0);
    data
}

/// Market scenarios for testing different conditions
pub enum MarketScenario {
    Normal,
    HighVolatility,
    FlashCrashRecovery,
    TrendingUp,
    TrendingDown,
    Sideways,
}

impl MarketScenario {
    /// Generate market data for specific scenario
    pub fn generate_data(&self, symbol: &str, base_price: f64) -> TimeSeriesData {
        match self {
            MarketScenario::Normal => create_realistic_market_data(symbol, base_price, 0.05),
            MarketScenario::HighVolatility => {
                create_high_volatility_market_data(symbol, base_price)
            }
            MarketScenario::FlashCrashRecovery => {
                let mut data = create_realistic_market_data(symbol, base_price, 0.20);
                data.low = base_price * 0.85; // 15% crash
                data.close = base_price * 0.95; // 5% recovery
                data
            }
            MarketScenario::TrendingUp => {
                let mut data = create_realistic_market_data(symbol, base_price, 0.03);
                data.close = base_price * 1.02; // 2% up
                data.high = data.close * 1.01;
                data
            }
            MarketScenario::TrendingDown => {
                let mut data = create_realistic_market_data(symbol, base_price, 0.03);
                data.close = base_price * 0.98; // 2% down
                data.low = data.close * 0.99;
                data
            }
            MarketScenario::Sideways => {
                let mut data = create_realistic_market_data(symbol, base_price, 0.01);
                data.close = base_price; // No change
                data
            }
        }
    }
}

/// Generate time series for backtesting
pub fn generate_time_series(
    symbol: &str,
    base_price: f64,
    duration_hours: u32,
    scenario: MarketScenario,
) -> Vec<TimeSeriesData> {
    let mut series = Vec::new();
    let mut current_price = base_price;

    for i in 0..duration_hours {
        let timestamp = Utc::now() - chrono::Duration::hours((duration_hours - i) as i64);
        let mut data = scenario.generate_data(symbol, current_price);
        data.timestamp = timestamp;

        current_price = data.close; // Price continuity
        series.push(data);
    }

    series
}

/// Test data builder for MarketData
pub struct MarketDataBuilder {
    symbol: String,
    timestamp: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
}

impl MarketDataBuilder {
    pub fn new(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            timestamp: 1704067200, // 2024-01-01 00:00:00 UTC
            open: 100.0,
            high: 105.0,
            low: 95.0,
            close: 102.0,
            volume: vec![1000.0],
        }
    }

    pub fn with_timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = timestamp;
        self
    }

    pub fn with_prices(mut self, open: f64, high: f64, low: f64, close: f64) -> Self {
        self.open = open;
        self.high = high;
        self.low = low;
        self.close = close;
        self
    }

    pub fn with_volume(mut self, volume: f64) -> Self {
        self.volume = volume;
        self
    }

    pub fn build(self) -> MarketData {
        MarketData {
            symbol: self.symbol,
            timestamp: self.timestamp,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.volume,
        }
    }
}

/// Test data builder for MarketContext
pub struct MarketContextBuilder {
    symbol: String,
    current_price: f64,
    bid: f64,
    ask: f64,
    volume_24h: f64,
    volatility: f64,
    timestamp: i64,
}

impl MarketContextBuilder {
    pub fn new(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            current_price: 100.0,
            bid: 99.9,
            ask: 100.1,
            volume_24h: 1_000_000.0,
            volatility: 0.02,
            timestamp: 1704067200,
        }
    }

    pub fn with_prices(mut self, current: f64, bid: f64, ask: f64) -> Self {
        self.current_price = current;
        self.bid = bid;
        self.ask = ask;
        self
    }

    pub fn with_volume(mut self, volume: f64) -> Self {
        self.volume_24h = volume;
        self
    }

    pub fn with_volatility(mut self, volatility: f64) -> Self {
        self.volatility = volatility;
        self
    }

    pub fn build(self) -> MarketContext {
        MarketContext {
            symbol: self.symbol,
            current_price: self.current_price,
            bid: self.bid,
            ask: self.ask,
            volume_24h: self.volume_24h,
            volatility: self.volatility,
            timestamp: self.timestamp,
        }
    }
}

/// Generate a series of market data for testing
pub fn generate_price_series(symbol: &str, start_price: f64, count: usize) -> Vec<MarketData> {
    let mut data = Vec::with_capacity(count);
    let mut price = start_price;
    let mut timestamp = 1704067200;

    for _ in 0..count {
        // Generate random price movement
        let change = (rand::random::<f64>() - 0.5) * 2.0; // -1% to +1%
        price *= 1.0 + change / 100.0;

        let high = price * 1.01;
        let low = price * 0.99;
        let open = price * (1.0 + (rand::random::<f64>() - 0.5) * 0.01);
        let close = price;
        let volume = 1000.0 + rand::random::<f64>() * 9000.0;

        data.push(MarketData {
            symbol: symbol.to_string(),
            timestamp,
            open,
            high,
            low,
            close,
            volume,
        });

        timestamp += 60; // 1 minute intervals
    }

    data
}

/// Setup test logging
pub fn setup_test_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("autonomous_platform=debug")
        .with_test_writer()
        .try_init();
}

/// Assertion helpers for test validation
pub mod assertions {
    use super::*;

    /// Assert that prediction confidence is within expected range
    pub fn assert_confidence_range(confidence: f64, min: f64, max: f64) {
        assert!(
            confidence >= min && confidence <= max,
            "Confidence {} is not within range [{}, {}]",
            confidence,
            min,
            max
        );
    }

    /// Assert that latency is within acceptable limits
    pub fn assert_latency_acceptable(latency_ms: f64, max_ms: f64) {
        assert!(
            latency_ms <= max_ms,
            "Latency {}ms exceeds maximum acceptable {}ms",
            latency_ms,
            max_ms
        );
    }

    /// Assert that market data is valid
    pub fn assert_market_data_valid(data: &TimeSeriesData) {
        assert!(!data.symbol.is_empty(), "Symbol cannot be empty");
        assert!(data.volume >= 0.0, "Volume cannot be negative");
        assert!(data.high >= data.low, "High must be >= low");
        assert!(
            data.high >= data.open && data.high >= data.close,
            "High must be >= open and close"
        );
        assert!(
            data.low <= data.open && data.low <= data.close,
            "Low must be <= open and close"
        );
    }

    /// Assert system health metrics
    pub fn assert_system_healthy(
        total_requests: u64,
        error_rate: f64,
        avg_latency: f64,
        max_error_rate: f64,
        max_latency: f64,
    ) {
        assert!(total_requests > 0, "No requests processed");
        assert!(
            error_rate <= max_error_rate,
            "Error rate {} exceeds maximum {}",
            error_rate,
            max_error_rate
        );
        assert!(
            avg_latency <= max_latency,
            "Average latency {}ms exceeds maximum {}ms",
            avg_latency,
            max_latency
        );
    }
}

/// Memory storage helpers for swarm coordination
pub mod memory {
    use super::*;

    /// Standard memory key for integration test results
    pub fn integration_test_results_key() -> String {
        "swarm-auto-centralized-1751484080479/integration-testing/results".to_string()
    }

    /// Store test results in standardized format
    pub fn store_test_results(
        test_name: &str,
        success: bool,
        metrics: HashMap<String, serde_json::Value>,
    ) -> HashMap<String, serde_json::Value> {
        let mut results = HashMap::new();
        results.insert("test_name".to_string(), json!(test_name));
        results.insert("success".to_string(), json!(success));
        results.insert("timestamp".to_string(), json!(Utc::now().timestamp()));
        results.insert("metrics".to_string(), json!(metrics));
        results
    }

    /// Create performance benchmark results
    pub fn create_performance_results(
        throughput: f64,
        latency_p95: f64,
        error_rate: f64,
        memory_usage_mb: f64,
    ) -> HashMap<String, serde_json::Value> {
        let mut metrics = HashMap::new();
        metrics.insert("throughput_per_second".to_string(), json!(throughput));
        metrics.insert("latency_p95_ms".to_string(), json!(latency_p95));
        metrics.insert("error_rate_percent".to_string(), json!(error_rate * 100.0));
        metrics.insert("memory_usage_mb".to_string(), json!(memory_usage_mb));
        metrics.insert("test_category".to_string(), json!("performance"));
        metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_config() {
        let config = create_test_config();
        assert_eq!(config.platform.name, "integration-test-platform");
        assert!(config.neural.memory_gb > 0.0);
        assert!(!config.neural.models.is_empty());
    }

    #[test]
    fn test_realistic_market_data() {
        let data = create_realistic_market_data("BTC/USD", 45000.0, 0.05);
        assertions::assert_market_data_valid(&data);
        assert_eq!(data.symbol, "BTC/USD");
        assert!(data.close > 40000.0 && data.close < 50000.0); // Within reasonable range
    }

    #[test]
    fn test_market_scenarios() {
        let scenarios = vec![
            MarketScenario::Normal,
            MarketScenario::HighVolatility,
            MarketScenario::FlashCrashRecovery,
            MarketScenario::TrendingUp,
            MarketScenario::TrendingDown,
            MarketScenario::Sideways,
        ];

        for scenario in scenarios {
            let data = scenario.generate_data("ETH/USD", 3000.0);
            assertions::assert_market_data_valid(&data);
        }
    }

    #[test]
    fn test_time_series_generation() {
        let series = generate_time_series("ADA/USD", 1.0, 24, MarketScenario::Normal);
        assert_eq!(series.len(), 24);

        // Verify chronological order
        for i in 1..series.len() {
            assert!(series[i].timestamp > series[i - 1].timestamp);
        }
    }
}
