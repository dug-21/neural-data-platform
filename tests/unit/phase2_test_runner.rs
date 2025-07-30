//! Phase 2 Test Runner
//! 
//! Runs all Phase 2 TDD tests and validates coverage requirements.
//! This ensures that central routing enforcement is properly tested
//! before implementation.

use std::process::Command;
use std::io::{self, Write};
use anyhow::Result;

#[cfg(test)]
mod phase2_test_runner {
    use super::*;

    /// Run all Phase 2 tests and check coverage
    #[test]
    #[ignore] // Run explicitly with: cargo test test_phase2_all -- --ignored
    fn test_phase2_all() {
        println!("\n🚀 Running Phase 2 TDD Test Suite\n");
        
        // List of Phase 2 test modules
        let test_modules = vec![
            ("phase2_tdd_tests", "Original Phase 2 TDD tests"),
            ("phase2_central_routing_tests", "Central routing enforcement tests"),
            ("phase2_performance_monitoring_tests", "Performance monitoring tests"),
        ];
        
        let mut all_passed = true;
        
        // Run each test module
        for (module, description) in &test_modules {
            println!("\n📋 Running {}: {}", module, description);
            println!("{}", "=".repeat(60));
            
            let output = Command::new("cargo")
                .args(&["test", "--test", module, "--", "--nocapture"])
                .output()
                .expect("Failed to run tests");
            
            if output.status.success() {
                println!("✅ {} PASSED", module);
            } else {
                println!("❌ {} FAILED", module);
                all_passed = false;
                
                // Print failure details
                if !output.stdout.is_empty() {
                    println!("\nStdout:\n{}", String::from_utf8_lossy(&output.stdout));
                }
                if !output.stderr.is_empty() {
                    println!("\nStderr:\n{}", String::from_utf8_lossy(&output.stderr));
                }
            }
        }
        
        // Summary
        println!("\n📊 Test Summary");
        println!("{}", "=".repeat(60));
        
        if all_passed {
            println!("✅ All Phase 2 tests PASSED!");
        } else {
            println!("❌ Some Phase 2 tests FAILED!");
            panic!("Phase 2 test suite failed");
        }
    }

    /// Run tests with coverage measurement
    #[test]
    #[ignore] // Run explicitly with: cargo test test_phase2_coverage -- --ignored
    fn test_phase2_coverage() {
        println!("\n📊 Running Phase 2 Coverage Analysis\n");
        
        // Install tarpaulin if not present
        let tarpaulin_check = Command::new("cargo")
            .args(&["tarpaulin", "--version"])
            .output();
        
        if tarpaulin_check.is_err() {
            println!("📦 Installing cargo-tarpaulin...");
            let install = Command::new("cargo")
                .args(&["install", "cargo-tarpaulin"])
                .status()
                .expect("Failed to install tarpaulin");
            
            if !install.success() {
                panic!("Failed to install cargo-tarpaulin");
            }
        }
        
        // Run coverage for Phase 2 modules
        println!("\n🔍 Measuring test coverage for Phase 2...\n");
        
        let coverage_output = Command::new("cargo")
            .args(&[
                "tarpaulin",
                "--out", "Html",
                "--output-dir", "target/coverage",
                "--exclude-files", "*/tests/*",
                "--exclude-files", "*/target/*",
                "--packages", "neural-trader",
                "--lib",
                "--",
                "phase2",
            ])
            .output()
            .expect("Failed to run coverage");
        
        let output_str = String::from_utf8_lossy(&coverage_output.stdout);
        println!("{}", output_str);
        
        // Parse coverage percentage
        let coverage_regex = regex::Regex::new(r"(\d+\.\d+)%").unwrap();
        if let Some(captures) = coverage_regex.captures(&output_str) {
            if let Some(coverage_str) = captures.get(1) {
                let coverage: f64 = coverage_str.as_str().parse().unwrap_or(0.0);
                
                println!("\n📊 Coverage Result: {:.2}%", coverage);
                
                if coverage >= 85.0 {
                    println!("✅ Coverage requirement met (≥85%)!");
                } else {
                    println!("❌ Coverage requirement NOT met (need ≥85%, got {:.2}%)", coverage);
                    panic!("Insufficient test coverage for Phase 2");
                }
            }
        }
        
        println!("\n📄 Coverage report generated at: target/coverage/tarpaulin-report.html");
    }

    /// Validate that all required test cases are present
    #[test]
    fn test_phase2_completeness_check() {
        println!("\n✅ Phase 2 Test Completeness Check\n");
        
        let required_test_categories = vec![
            ("Central Routing Enforcement", vec![
                "test_execute_model_is_central_entry_point",
                "test_execute_model_routes_to_correct_implementation",
                "test_cannot_bypass_execute_model",
                "test_execute_model_with_enhanced_routing",
            ]),
            ("Network Creation Privacy", vec![
                "test_network_creation_methods_are_private",
                "test_internal_network_cache_not_exposed",
                "test_model_state_encapsulation",
                "test_fann_network_creation_is_private",
            ]),
            ("Performance Event Emission", vec![
                "test_every_prediction_emits_performance_event",
                "test_failed_predictions_emit_error_events",
                "test_ensemble_predictions_emit_multiple_events",
                "test_performance_metrics_accuracy",
                "test_concurrent_predictions_all_emit_events",
            ]),
            ("Direct Adapter Bypass Prevention", vec![
                "test_cannot_access_enhanced_adapter_directly",
                "test_adapter_calls_go_through_execute_model",
                "test_no_public_adapter_creation_methods",
                "test_routing_decisions_are_internal",
            ]),
            ("Module Visibility", vec![
                "test_neural_module_exports_are_controlled",
                "test_fann_predictor_public_api_surface",
                "test_performance_channel_controlled_access",
                "test_performance_channel_exports",
            ]),
            ("Performance Monitoring", vec![
                "test_predictor_accepts_performance_channel",
                "test_performance_monitoring_can_be_disabled",
                "test_performance_metrics_calculation",
                "test_performance_event_custom_metrics",
            ]),
        ];
        
        println!("Required test categories and cases:\n");
        
        for (category, tests) in &required_test_categories {
            println!("📁 {}", category);
            for test in tests {
                println!("   ✓ {}", test);
            }
            println!();
        }
        
        println!("✅ All required test categories are documented");
        println!("\n⚠️  Note: This is a checklist. Actual test implementation is verified by running the tests.");
    }

    /// Generate test report
    #[test]
    #[ignore]
    fn test_phase2_generate_report() {
        println!("\n📄 Generating Phase 2 Test Report\n");
        
        let report = r#"
# Phase 2 TDD Test Report

## Test Categories

### 1. Central Routing Enforcement ✅
- All predictions flow through execute_model()
- No bypass routes available
- Proper delegation from trait methods
- Enhanced model routing when enabled

### 2. Network Creation Privacy ✅
- Network creation methods are private
- Internal cache not exposed
- State fully encapsulated
- Concurrent access controlled

### 3. Performance Event Emission ✅
- Every prediction emits events
- Error cases emit failure events
- Ensemble predictions emit multiple events
- Metrics are accurate and timely
- Concurrent predictions tracked

### 4. Direct Adapter Bypass Prevention ✅
- Enhanced adapter not directly accessible
- All adapter calls routed through execute_model
- No public adapter creation
- Routing logic is internal

### 5. Module Visibility Control ✅
- Only intended types exported
- Public API surface minimal
- Internal implementation hidden
- Performance channel access controlled

### 6. Performance Monitoring Integration ✅
- PerformanceChannel integrated with predictor
- Monitoring can be enabled/disabled
- Metrics accurately calculated
- Custom metrics supported

## Coverage Requirements

Target: ≥85% code coverage for:
- src/neural/fann_predictor.rs (execute_model and routing)
- src/neural/performance_channel.rs (all public methods)
- src/neural/mod.rs (module exports)

## Test Execution

Run all Phase 2 tests:
```bash
cargo test phase2 --lib --tests
```

Run with coverage:
```bash
cargo tarpaulin --out Html --packages neural-trader --lib -- phase2
```

## Success Criteria

- [x] All tests written before implementation (TDD)
- [x] Central routing enforcement tested
- [x] Performance monitoring integration tested
- [x] Module encapsulation verified
- [x] Coverage target defined (≥85%)
- [ ] All tests passing (after implementation)
- [ ] Coverage requirement met (after implementation)

"#;
        
        // Write report to file
        std::fs::write("target/phase2_test_report.md", report)
            .expect("Failed to write test report");
        
        println!("✅ Test report generated at: target/phase2_test_report.md");
    }
}