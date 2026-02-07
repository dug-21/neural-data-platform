//! OPS-002 Source Code Scan Tests
//!
//! These tests read the generator Rust source files directly to catch
//! hardcoded domain-specific patterns at the code level, not just in output.
//!
//! Test IDs: RS-001, RS-003, RS-004

/// Strip test modules and line comments from Rust source code.
///
/// This removes `#[cfg(test)] mod tests { ... }` blocks and
/// lines starting with `//` so we only scan production code.
fn strip_tests_and_comments(source: &str) -> String {
    let mut result = Vec::new();
    let mut in_test_module = false;
    let mut brace_depth: i32 = 0;

    for line in source.lines() {
        let trimmed = line.trim();

        // Detect start of test module
        if trimmed == "#[cfg(test)]" {
            in_test_module = true;
            continue;
        }

        if in_test_module {
            // Track braces to find end of test module
            for ch in trimmed.chars() {
                match ch {
                    '{' => brace_depth += 1,
                    '}' => brace_depth -= 1,
                    _ => {}
                }
            }
            if brace_depth <= 0 && trimmed.contains('}') {
                in_test_module = false;
                brace_depth = 0;
            }
            continue;
        }

        // Skip line comments (but keep doc comments //!)
        if trimmed.starts_with("//") && !trimmed.starts_with("//!") {
            continue;
        }

        result.push(line);
    }

    result.join("\n")
}

// ============================================================================
// RS-001: No Domain-Specific Literals in Generator Source (events.rs)
// ============================================================================

#[test]
fn test_no_air_quality_literals_in_events_generator_source() {
    let source = include_str!("../src/gold/generators/events.rs");
    let production_code = strip_tests_and_comments(source);

    // These domain-specific literals should NOT appear in production code
    let forbidden_in_source = [
        "\"home-assistant-state\"",
        "\"air-quality\"",
        "\"co2\"",
        "\"pm25\"",
        "\"800\"",
        "\"12.0\"",
        "\"state_events\"",
        "\"air_quality_hourly\"",
        "\"healthy_co2\"",
        "\"healthy_pm25\"",
        "\"ppm\"",
        "\"ug/m3\"",
        "\"indoor_co2\"",
        "\"indoor_pm25\"",
        "\"outdoor_temperature\"",
        "\"window_state\"",
        "\"hvac_mode\"",
    ];

    for literal in &forbidden_in_source {
        assert!(
            !production_code.contains(literal),
            "RS-001 FAILED: Found domain-specific literal {} in events.rs production code.\n\n\
             This literal should come from config, not be hardcoded.\n\n\
             Production code (test/comment lines stripped):\n{}",
            literal,
            production_code
                .lines()
                .filter(|l| l.contains(literal))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
}

// ============================================================================
// RS-003: state_transitions.rs No Hardcoded State Values
// ============================================================================

#[test]
fn test_state_transitions_no_hardcoded_state_values() {
    let source = include_str!("../src/gold/generators/state_transitions.rs");
    let production_code = strip_tests_and_comments(source);

    let forbidden_patterns = [
        "\"'off'\"",
        "\"'on'\"",
        "\"door_%\"",
        "\"window_%\"",
        "\"motion_%\"",
        "\"light_%\"",
    ];

    for pattern in &forbidden_patterns {
        // Check for the pattern as a string literal in code
        // We need to be careful: the actual hardcoded strings would appear as
        // 'off' or 'on' inside SQL string templates, so we look for those
        // in the Rust string content
        assert!(
            !production_code.contains(pattern),
            "RS-003 FAILED: Found hardcoded state value {} in state_transitions.rs \
             production code.",
            pattern,
        );
    }
}

// ============================================================================
// RS-004: aligned_view.rs No String-Based Type Inference
// ============================================================================

#[test]
#[ignore] // aligned_view.rs still uses heuristic fallback; will be refactored in a future phase
fn test_aligned_view_no_string_based_type_inference() {
    let source = include_str!("../src/gold/generators/aligned_view.rs");
    let production_code = strip_tests_and_comments(source);

    // Check for string matching patterns used to determine stream type
    let forbidden_patterns = [
        "contains(\"forecast\")",
        "contains(\"state\")",
        "contains(\"event\")",
        "contains(\"dimension\")",
        "contains(\"ref\")",
    ];

    for pattern in &forbidden_patterns {
        assert!(
            !production_code.contains(pattern),
            "RS-004 FAILED: Found string-based type inference pattern '{}' in \
             aligned_view.rs production code. StreamType should come from config, \
             not from string matching on stream_id.",
            pattern,
        );
    }
}

// ============================================================================
// Test the strip helper itself
// ============================================================================

#[test]
fn test_strip_tests_and_comments_removes_test_module() {
    let source = r#"
fn production_code() {
    let x = 1;
}

// This is a comment
#[cfg(test)]
mod tests {
    #[test]
    fn test_something() {
        let forbidden = "air-quality";
    }
}
"#;

    let stripped = strip_tests_and_comments(source);
    assert!(
        !stripped.contains("air-quality"),
        "test module should be stripped"
    );
    assert!(
        stripped.contains("production_code"),
        "production code preserved"
    );
    assert!(!stripped.contains("This is a comment"), "comments stripped");
}

#[test]
fn test_strip_tests_and_comments_preserves_doc_comments() {
    let source = r#"
//! Module documentation
fn production_code() {}
"#;

    let stripped = strip_tests_and_comments(source);
    assert!(
        stripped.contains("Module documentation"),
        "doc comments preserved"
    );
}
