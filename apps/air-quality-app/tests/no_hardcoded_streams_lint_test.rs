//! Lint test to prevent hardcoded stream registration
//!
//! This test ensures streams are registered dynamically from StreamRegistry,
//! not from hardcoded arrays in main.rs. Any hardcoded patterns will cause
//! this test to fail, protecting the config-driven architecture.

use std::fs;

/// Detects hardcoded stream arrays in main.rs
///
/// FAILS if hardcoded stream lists are found (drift from config-driven goal)
/// PASSES when registration uses `register_all_streams_from_registry()`
#[test]
fn test_no_hardcoded_stream_arrays_in_main() {
    let main_rs_path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs");
    let source = fs::read_to_string(main_rs_path).expect("Failed to read main.rs");

    // Patterns that indicate hardcoded stream registration
    let forbidden_patterns = vec![
        (r#"for stream_id in &["#, "Hardcoded array iteration"),
        (r#"&["air-quality""#, "Hardcoded air-quality stream"),
        (r#"&["outdoor-weather""#, "Hardcoded outdoor-weather stream"),
        (r#"vec!["air-quality""#, "Hardcoded stream vector"),
    ];

    let mut violations = Vec::new();

    for (line_num, line) in source.lines().enumerate() {
        let trimmed = line.trim();

        // Skip comments
        if trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }

        for (pattern, description) in &forbidden_patterns {
            if line.contains(pattern) {
                violations.push(format!(
                    "Line {}: {} - Found: {}",
                    line_num + 1,
                    description,
                    trimmed
                ));
            }
        }
    }

    if !violations.is_empty() {
        panic!(
            "\n\n❌ HARDCODED STREAM ARRAYS DETECTED!\n\n\
            This violates the config-driven architecture.\n\
            Use `router.register_all_streams_from_registry()` instead.\n\n\
            Violations:\n{}\n\n\
            Fix: Replace hardcoded arrays with StreamRegistry.list_streams()\n",
            violations.join("\n")
        );
    }
}

/// Verifies that main.rs uses the config-driven registration method
#[test]
fn test_uses_config_driven_registration() {
    let main_rs_path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs");
    let source = fs::read_to_string(main_rs_path).expect("Failed to read main.rs");

    // Must use the config-driven method
    let has_config_driven = source.contains("register_all_streams_from_registry");

    assert!(
        has_config_driven,
        "\n\n❌ CONFIG-DRIVEN REGISTRATION NOT FOUND!\n\n\
        main.rs should call `router.register_all_streams_from_registry()`\n\
        to load streams from StreamRegistry (etcd) instead of hardcoding.\n"
    );
}
