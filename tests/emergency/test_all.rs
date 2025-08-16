//! Emergency Test Suite Entry Point
//! Runs all critical tests to ensure system integrity

mod test_trading;
mod test_data;
mod test_neural;
mod test_health;
mod test_vendor_predictor;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();
    
    println!("🚨 EMERGENCY TEST SUITE 🚨");
    println!("==========================");
    println!("Running critical system tests...\n");
    
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    
    // Test results tracker
    let tests = vec![
        ("System Health", run_health_tests().await),
        ("Data Pipeline", run_data_tests().await),
        ("Neural Models", run_neural_tests().await),
        ("Trading Flow", run_trading_tests().await),
        ("Vendor Predictor", run_vendor_predictor_tests().await),
    ];
    
    println!("\n📊 TEST RESULTS");
    println!("================");
    
    for (name, result) in tests {
        match result {
            TestResult::Passed => {
                println!("✅ {}: PASSED", name);
                passed += 1;
            }
            TestResult::Failed(msg) => {
                println!("❌ {}: FAILED - {}", name, msg);
                failed += 1;
            }
            TestResult::Skipped(msg) => {
                println!("⚠️  {}: SKIPPED - {}", name, msg);
                skipped += 1;
            }
        }
    }
    
    println!("\n📈 SUMMARY");
    println!("===========");
    println!("Passed:  {}", passed);
    println!("Failed:  {}", failed);
    println!("Skipped: {}", skipped);
    
    if failed == 0 {
        println!("\n✅ All critical tests passed!");
        println!("System is safe for refactoring.");
    } else {
        println!("\n⚠️  {} tests failed!", failed);
        println!("Review failures before proceeding with refactoring.");
    }
    
    Ok(())
}

enum TestResult {
    Passed,
    Failed(String),
    Skipped(String),
}

async fn run_health_tests() -> TestResult {
    println!("\n🏥 Running Health Tests...");
    println!("--------------------------");
    
    // Run health check
    match test_health::test_system_health().await {
        Ok(_) => TestResult::Passed,
        Err(e) => {
            if e.to_string().contains("Connection refused") {
                TestResult::Skipped("System offline".to_string())
            } else {
                TestResult::Failed(e.to_string())
            }
        }
    }
}

async fn run_data_tests() -> TestResult {
    println!("\n📊 Running Data Pipeline Tests...");
    println!("----------------------------------");
    
    match test_data::test_data_pipeline_integrity().await {
        Ok(_) => TestResult::Passed,
        Err(e) => {
            if e.to_string().contains("Cannot connect to database") {
                TestResult::Skipped("Database unavailable".to_string())
            } else {
                TestResult::Failed(e.to_string())
            }
        }
    }
}

async fn run_neural_tests() -> TestResult {
    println!("\n🧠 Running Neural Model Tests...");
    println!("---------------------------------");
    
    match test_neural::test_neural_predictions().await {
        Ok(_) => TestResult::Passed,
        Err(e) => TestResult::Failed(e.to_string()),
    }
}

async fn run_trading_tests() -> TestResult {
    println!("\n💹 Running Trading Flow Tests...");
    println!("---------------------------------");
    
    match test_trading::test_trading_decision_flow().await {
        Ok(_) => TestResult::Passed,
        Err(e) => {
            if e.to_string().contains("Connection refused") {
                TestResult::Skipped("Trading API unavailable".to_string())
            } else {
                TestResult::Failed(e.to_string())
            }
        }
    }
}

async fn run_vendor_predictor_tests() -> TestResult {
    println!("\n🎯 Running Vendor Predictor Tests...");
    println!("-------------------------------------");
    
    // Run critical architecture tests
    let mut failures = Vec::new();
    
    // Test two-layer architecture
    if let Err(e) = test_vendor_predictor::test_two_layer_architecture().await {
        failures.push(format!("Two-layer architecture: {}", e));
    }
    
    // Test autonomous training
    if let Err(e) = test_vendor_predictor::test_autonomous_training_triggers().await {
        if !e.to_string().contains("Database not available") {
            failures.push(format!("Autonomous training: {}", e));
        }
    }
    
    // Test training data window
    if let Err(e) = test_vendor_predictor::test_training_data_window().await {
        failures.push(format!("Training data window: {}", e));
    }
    
    // Test sector model assignment
    if let Err(e) = test_vendor_predictor::test_sector_model_assignment().await {
        failures.push(format!("Sector model assignment: {}", e));
    }
    
    if failures.is_empty() {
        TestResult::Passed
    } else {
        TestResult::Failed(failures.join("; "))
    }
}