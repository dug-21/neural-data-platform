//! Test runner utilities for emergency tests

use anyhow::Result;

// Re-export test functions for use in test_all.rs
pub use crate::test_health::{test_system_health, test_api_endpoints, test_startup_sequence};
pub use crate::test_data::{test_data_pipeline_integrity, test_timescale_aggregates};
pub use crate::test_neural::{test_neural_predictions, test_model_persistence, test_sector_model_structure};
pub use crate::test_trading::{test_trading_decision_flow, test_risk_limits_enforced};
pub use crate::test_vendor_predictor::{
    test_two_layer_architecture,
    test_autonomous_training_triggers,
    test_training_data_window,
    test_sector_model_assignment,
    test_cluster_model_pool,
    test_market_hours_priority,
    test_validation_gates,
    test_model_persistence_integrity,
};

// Helper function to run a test and return result
pub async fn run_test<F, Fut>(name: &str, test_fn: F) -> TestResult
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    println!("\n🧪 Running {}...", name);
    match test_fn().await {
        Ok(_) => {
            println!("  ✅ {} passed", name);
            TestResult::Passed
        }
        Err(e) => {
            let error_str = e.to_string();
            if error_str.contains("Connection refused") || error_str.contains("not available") {
                println!("  ⚠️  {} skipped: {}", name, error_str);
                TestResult::Skipped(error_str)
            } else {
                println!("  ❌ {} failed: {}", name, error_str);
                TestResult::Failed(error_str)
            }
        }
    }
}

#[derive(Debug)]
pub enum TestResult {
    Passed,
    Failed(String),
    Skipped(String),
}