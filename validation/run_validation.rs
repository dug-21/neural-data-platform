//! Validation Test Runner
//! 
//! Simple script to run the comprehensive ruv-FANN validation suite.
//! This can be used in CI/CD pipelines or for manual validation.

use anyhow::Result;
use chrono::Utc;
use std::env;

use crate::validation::{
    ValidationCoordinator, ValidationCriteria,
    run_quick_validation, export_validation_results,
    RuvFannValidationTestSuite, PerformanceBenchmarker, BenchmarkConfig,
};

/// Main validation runner
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("🚀 ruv-FANN Integration Validation Suite");
    println!("==========================================");
    println!("Date: {}", Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));
    println!();

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    let validation_type = args.get(1).map(|s| s.as_str()).unwrap_or("quick");

    match validation_type {
        "quick" => run_quick_validation_suite().await,
        "comprehensive" => run_comprehensive_validation_suite().await,
        "performance" => run_performance_only().await,
        "integration" => run_integration_only().await,
        _ => {
            println!("Usage: {} [quick|comprehensive|performance|integration]", args[0]);
            println!();
            println!("Options:");
            println!("  quick         - Run essential validation tests (default)");
            println!("  comprehensive - Run full validation suite with detailed reporting");
            println!("  performance   - Run performance benchmarks only");
            println!("  integration   - Run integration tests only");
            Ok(())
        }
    }
}

/// Run quick validation (default)
async fn run_quick_validation_suite() -> Result<()> {
    println!("📋 Running Quick Validation Suite");
    println!("----------------------------------");
    
    let summary = run_quick_validation().await?;
    
    // Export results
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("validation_results_quick_{}.json", timestamp);
    export_validation_results(&summary, &filename).await?;
    
    // Exit with appropriate code
    if summary.ready_for_production {
        println!("\n🎉 SUCCESS: System is ready for production!");
        std::process::exit(0);
    } else {
        println!("\n❌ FAILED: System is not ready for production. Check the results above.");
        std::process::exit(1);
    }
}

/// Run comprehensive validation suite
async fn run_comprehensive_validation_suite() -> Result<()> {
    println!("🔬 Running Comprehensive Validation Suite");
    println!("------------------------------------------");
    
    let criteria = ValidationCriteria {
        max_prediction_latency_p95_ms: 1000.0,
        max_memory_usage_mb: 300.0,
        min_accuracy_threshold: 0.75,
        max_error_rate: 0.03,
        min_throughput_per_second: 8.0,
        min_stability_score: 0.85,
        require_all_models_pass: true,
        require_enhanced_adapter: false, // Set to true if enhanced adapter is required
        perform_stability_tests: true,
    };
    
    let coordinator = ValidationCoordinator::new(criteria);
    let report = coordinator.run_comprehensive_validation().await?;
    
    // Print summary
    print_detailed_summary(&report);
    
    // Export detailed report
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("validation_report_comprehensive_{}.json", timestamp);
    coordinator.export_detailed_report(&report, &filename).await?;
    
    // Exit with appropriate code
    if report.summary.ready_for_production {
        println!("\n🎉 SUCCESS: Comprehensive validation passed!");
        std::process::exit(0);
    } else {
        println!("\n❌ FAILED: Comprehensive validation failed. Check the detailed report.");
        std::process::exit(1);
    }
}

/// Run performance benchmarks only
async fn run_performance_only() -> Result<()> {
    println!("📊 Running Performance Benchmarks Only");
    println!("---------------------------------------");
    
    let config = BenchmarkConfig {
        test_iterations: 100,
        warmup_iterations: 20,
        concurrent_users: 15,
        data_sizes: vec![50, 100, 200, 500, 1000],
        prediction_horizons: vec![1, 5, 10, 24, 48],
        enable_long_running_tests: true,
        test_duration_seconds: 600, // 10 minutes
        ..Default::default()
    };
    
    let benchmarker = PerformanceBenchmarker::new(config);
    let results = benchmarker.run_comprehensive_benchmarks().await?;
    
    // Export results
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("performance_benchmark_{}.json", timestamp);
    benchmarker.export_results(&filename).await?;
    
    // Exit with appropriate code based on performance grade
    if results.performance_grade.score >= 70.0 {
        println!("\n🎉 SUCCESS: Performance benchmarks passed!");
        std::process::exit(0);
    } else {
        println!("\n❌ FAILED: Performance benchmarks below threshold.");
        std::process::exit(1);
    }
}

/// Run integration tests only
async fn run_integration_only() -> Result<()> {
    println!("🧪 Running Integration Tests Only");
    println!("----------------------------------");
    
    // Run all validation tests
    match RuvFannValidationTestSuite::run_all_tests().await {
        Ok(()) => {
            println!("\n🎉 SUCCESS: All integration tests passed!");
            std::process::exit(0);
        }
        Err(e) => {
            println!("\n❌ FAILED: Integration tests failed: {}", e);
            std::process::exit(1);
        }
    }
}

/// Print detailed summary of comprehensive validation
fn print_detailed_summary(report: &crate::validation::ValidationReport) {
    println!("\n" + "=".repeat(80).as_str());
    println!("📋 COMPREHENSIVE VALIDATION REPORT");
    println!("=".repeat(80));
    
    println!("\n📊 Validation ID: {}", report.validation_id);
    println!("⏱️  Total Execution Time: {:?}", report.execution_metadata.total_execution_time);
    println!("🏆 Overall Status: {:?}", report.summary.overall_status);
    println!("🚀 Ready for Production: {}", 
             if report.summary.ready_for_production { "YES ✅" } else { "NO ❌" });
    
    println!("\n🧪 Integration Tests:");
    println!("   Total: {}", report.summary.integration_tests.total_tests);
    println!("   ✅ Passed: {}", report.summary.integration_tests.passed_tests);
    println!("   ❌ Failed: {}", report.summary.integration_tests.failed_tests);
    println!("   ⚠️ Warnings: {}", report.summary.integration_tests.warning_tests);
    
    println!("\n📈 Performance Benchmarks:");
    println!("   Overall Grade: {}", report.summary.performance_benchmarks.overall_grade);
    println!("   Performance Score: {:.1}/100", report.summary.performance_benchmarks.performance_score);
    println!("   Meets Requirements: {}", 
             if report.summary.performance_benchmarks.meets_requirements { "YES ✅" } else { "NO ❌" });
    
    println!("\n🔄 Migration Validation:");
    println!("   Migration Successful: {}", 
             if report.summary.migration_validation.migration_successful { "YES ✅" } else { "NO ❌" });
    println!("   No Fallback Scores: {}", 
             if report.summary.migration_validation.no_fallback_scores_detected { "YES ✅" } else { "NO ❌" });
    println!("   All Models Using ruv-FANN: {}", 
             if report.summary.migration_validation.all_models_using_ruv_fann { "YES ✅" } else { "NO ❌" });
    
    println!("\n🧠 Model Validation Details:");
    for (model_name, details) in &report.detailed_results.model_validation_details {
        println!("   {}: Architecture: {}, Real NN: {}, No Fallbacks: {}", 
                 model_name,
                 if details.architecture_verified { "✅" } else { "❌" },
                 if details.real_neural_network_confirmed { "✅" } else { "❌" },
                 if !details.fallback_usage_detected { "✅" } else { "❌" });
    }
    
    println!("\n⚙️ System Health:");
    let health = &report.detailed_results.system_health_check;
    println!("   Memory Leaks: {}", if !health.memory_leaks_detected { "None ✅" } else { "Detected ❌" });
    println!("   Resource Cleanup: {}", if health.resource_cleanup_verified { "OK ✅" } else { "Issues ❌" });
    println!("   Concurrent Safety: {}", if health.concurrent_safety_verified { "OK ✅" } else { "Issues ❌" });
    
    println!("\n💾 Resource Usage:");
    let resources = &report.execution_metadata.resource_usage;
    println!("   Peak Memory: {:.1} MB", resources.peak_memory_mb);
    println!("   Avg CPU: {:.1}%", resources.avg_cpu_usage_percent);
    println!("   Disk I/O: {:.1} MB", resources.disk_io_mb);
    
    if !report.summary.recommendations.is_empty() {
        println!("\n💡 Recommendations:");
        for (i, rec) in report.summary.recommendations.iter().enumerate() {
            println!("   {}. {}", i + 1, rec);
        }
    }
    
    println!("\n" + "=".repeat(80).as_str());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quick_validation_runner() {
        // This test would run the quick validation in a test environment
        println!("Testing quick validation runner (mock)");
        // In a real test, you'd call run_quick_validation_suite() with proper setup
    }
}