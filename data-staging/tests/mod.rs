//! Data-Staging Service Test Suite
//! 
//! Comprehensive test modules for Phase 4 proto-only implementation.
//! 
//! This test suite ensures >90% code coverage and validates:
//! - Strict proto-only messaging enforcement
//! - Complete rejection of Vec<u8> non-protobuf data
//! - Performance requirements (>10k msgs/sec, <1ms latency)
//! - End-to-end pipeline integrity

// Test module declarations
pub mod unit_tests;
pub mod integration_tests;
pub mod performance_tests;
pub mod proto_only_enforcement_tests;
pub mod e2e_pipeline_tests;
pub mod test_coverage_validation;

// Common test utilities
pub mod common;

// Test configuration
use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize test environment (call once per test run)
pub fn init_test_environment() {
    INIT.call_once(|| {
        // Set up test logging
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .is_test(true)
            .try_init();
        
        // Set test-specific environment variables
        std::env::set_var("RUST_LOG", "debug");
        std::env::set_var("DATA_STAGING_TEST_MODE", "true");
        
        println!("🧪 Data-Staging test environment initialized");
        println!("📋 Test suite includes:");
        println!("  • Unit Tests - Individual component testing");
        println!("  • Integration Tests - Service integration testing");
        println!("  • Performance Tests - Throughput and latency validation");
        println!("  • Proto-Only Enforcement - Vec<u8> rejection testing");
        println!("  • End-to-End Pipeline - Complete flow testing");
        println!("  • Coverage Validation - >90% coverage verification");
    });
}

/// Test suite metadata
pub const TEST_SUITE_VERSION: &str = "1.0.0";
pub const COVERAGE_REQUIREMENT: f64 = 90.0;
pub const PERFORMANCE_REQUIREMENT_THROUGHPUT: u32 = 10_000; // msgs/sec
pub const PERFORMANCE_REQUIREMENT_LATENCY_MS: u32 = 1;      // milliseconds

#[cfg(test)]
mod test_suite_validation {
    use super::*;
    
    #[test]
    fn test_suite_initialization() {
        init_test_environment();
        
        // Verify test environment is properly configured
        assert_eq!(std::env::var("DATA_STAGING_TEST_MODE").unwrap(), "true");
        
        println!("✅ Test suite initialization validated");
        println!("📊 Coverage requirement: {}%", COVERAGE_REQUIREMENT);
        println!("🚀 Performance requirement: {} msgs/sec", PERFORMANCE_REQUIREMENT_THROUGHPUT);
        println!("⏱️ Latency requirement: <{}ms", PERFORMANCE_REQUIREMENT_LATENCY_MS);
    }
    
    #[test]
    fn test_suite_metadata() {
        assert_eq!(TEST_SUITE_VERSION, "1.0.0");
        assert_eq!(COVERAGE_REQUIREMENT, 90.0);
        assert_eq!(PERFORMANCE_REQUIREMENT_THROUGHPUT, 10_000);
        assert_eq!(PERFORMANCE_REQUIREMENT_LATENCY_MS, 1);
        
        println!("✅ Test suite metadata validated");
    }
}

/// Common test utilities and fixtures
pub mod test_utils {
    use data_staging::*;
    use serde_json::json;
    use std::collections::HashMap;
    
    /// Create a valid RawMarketData for testing
    pub fn create_valid_market_data() -> RawMarketData {
        RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(chrono::Utc::now().timestamp_millis()),
            bid: Some(150.20),
            ask: Some(150.30),
            exchange: Some("NASDAQ".to_string()),
            sequence: Some(12345),
            high: Some(151.0),
            low: Some(149.0),
            open: Some(150.0),
            close: Some(150.25),
            vwap: Some(150.1),
            metadata: HashMap::new(),
        }
    }
    
    /// Create valid JSON string for testing
    pub fn create_valid_json_string() -> String {
        json!({
            "symbol": "AAPL",
            "price": 150.25,
            "volume": 1000.0,
            "timestamp": chrono::Utc::now().timestamp_millis(),
            "bid": 150.20,
            "ask": 150.30,
            "exchange": "NASDAQ"
        }).to_string()
    }
    
    /// Create invalid JSON string (missing required fields)
    pub fn create_invalid_json_string() -> String {
        json!({
            "price": 150.25,
            "volume": 1000.0
            // Missing symbol and timestamp
        }).to_string()
    }
    
    /// Create test quality thresholds
    pub fn create_test_quality_thresholds() -> QualityThresholds {
        QualityThresholds {
            minimum_quality_score: 0.6,
            max_age_seconds: 3600,
            required_fields: vec![
                "symbol".to_string(),
                "price".to_string(),
                "timestamp".to_string(),
            ],
        }
    }
    
    /// Create test data staging configuration
    pub fn create_test_config() -> DataStagingConfig {
        DataStagingConfig {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            input_stream: "test_raw_data".to_string(),
            consumer_group: "test-staging".to_string(),
            consumer_name: "test-worker-1".to_string(),
            eventbus_config: EventBusConfig {
                output_topic: "test_proto_events".to_string(),
                connection_timeout_ms: 5000,
                publish_timeout_ms: 1000,
            },
            quality_thresholds: create_test_quality_thresholds(),
            processing_limits: ProcessingLimits {
                max_batch_size: 10,
                message_timeout_ms: 1000,
                max_retries: 2,
            },
        }
    }
    
    /// Generate Vec<u8> test data that should be rejected
    pub fn generate_non_proto_test_data() -> Vec<Vec<u8>> {
        vec![
            vec![0x01, 0x02, 0x03, 0x04],                              // Raw bytes
            r#"{"symbol": "AAPL", "price": 150.25}"#.as_bytes().to_vec(), // JSON
            b"<xml>data</xml>".to_vec(),                               // XML
            b"symbol,price,volume\nAAPL,150.25,1000".to_vec(),        // CSV
            vec![0xFF; 1000],                                          // Large random data
            vec![],                                                    // Empty data
        ]
    }
}

/// Test result tracking
pub struct TestRunSummary {
    pub total_tests: usize,
    pub passed_tests: usize, 
    pub failed_tests: usize,
    pub coverage_percentage: f64,
    pub performance_tests_passed: bool,
    pub proto_enforcement_tests_passed: bool,
}

impl TestRunSummary {
    pub fn new() -> Self {
        Self {
            total_tests: 0,
            passed_tests: 0,
            failed_tests: 0,
            coverage_percentage: 0.0,
            performance_tests_passed: false,
            proto_enforcement_tests_passed: false,
        }
    }
    
    pub fn meets_all_requirements(&self) -> bool {
        self.failed_tests == 0 &&
        self.coverage_percentage >= COVERAGE_REQUIREMENT &&
        self.performance_tests_passed &&
        self.proto_enforcement_tests_passed
    }
    
    pub fn generate_summary_report(&self) -> String {
        let mut report = String::new();
        
        report.push_str("=== DATA-STAGING TEST RUN SUMMARY ===\n\n");
        
        report.push_str(&format!("Total Tests: {}\n", self.total_tests));
        report.push_str(&format!("Passed: {}\n", self.passed_tests));
        report.push_str(&format!("Failed: {}\n", self.failed_tests));
        report.push_str(&format!("Success Rate: {:.1}%\n", 
                                (self.passed_tests as f64 / self.total_tests as f64) * 100.0));
        
        report.push_str(&format!("\nCoverage: {:.1}% (≥{:.0}% required)\n", 
                                self.coverage_percentage, COVERAGE_REQUIREMENT));
        
        report.push_str(&format!("Performance Tests: {}\n", 
                                if self.performance_tests_passed { "✅ PASSED" } else { "❌ FAILED" }));
        
        report.push_str(&format!("Proto Enforcement: {}\n", 
                                if self.proto_enforcement_tests_passed { "✅ PASSED" } else { "❌ FAILED" }));
        
        report.push_str(&format!("\nOverall Result: {}\n", 
                                if self.meets_all_requirements() { "✅ ALL REQUIREMENTS MET" } else { "❌ REQUIREMENTS NOT MET" }));
        
        report
    }
}