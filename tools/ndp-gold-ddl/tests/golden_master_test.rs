//! Golden Master Tests for FE-002 Domain Configuration Standardization
//!
//! These tests ensure that the YAML to JSON migration preserves EXACT DDL output.
//! Any change in DDL output indicates a behavioral change that MUST be investigated.
//!
//! # Purpose
//!
//! Golden Master testing captures output BEFORE changes and compares it AFTER changes.
//! For FE-002, this guarantees that converting `domain.yaml` to `domain.json`
//! produces byte-for-byte identical DDL output.
//!
//! # Test Categories
//!
//! - Domain-level outputs (sync and recreate modes)
//! - Stream-level outputs (continuous aggregates)
//! - Transition outputs (state change tracking)
//!
//! # Usage
//!
//! ```bash
//! # Run all golden master tests
//! cargo test -p ndp-gold-ddl --test golden_master_test
//!
//! # Run with verbose output
//! cargo test -p ndp-gold-ddl --test golden_master_test -- --nocapture
//! ```

use std::fs;
use std::path::PathBuf;
use std::process::Command;

// =============================================================================
// Helper Functions
// =============================================================================

/// Get the repository root directory
///
/// Walks up from the current file's location to find the repo root.
fn get_repo_root() -> PathBuf {
    // Start from the crate root (tools/ndp-gold-ddl)
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Go up two levels to get to repo root
    crate_root
        .parent()
        .expect("Expected tools directory")
        .parent()
        .expect("Expected repo root")
        .to_path_buf()
}

/// Get the path to the fixtures directory
fn get_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("golden-master")
}

/// Get the path to the config directory
fn get_config_dir() -> PathBuf {
    get_repo_root().join("config")
}

/// Execute ndp-gold-ddl and return stdout
///
/// Runs the ndp-gold-ddl tool from the repository root with the correct config path.
/// Filters out any --config-dir arguments passed in args since we use absolute paths.
///
/// # Arguments
///
/// * `args` - Command line arguments to pass to ndp-gold-ddl
///
/// # Returns
///
/// The stdout output as a String
///
/// # Panics
///
/// Panics if the command fails to execute or returns non-zero exit code
fn execute_gold_ddl(args: &[&str]) -> String {
    let repo_root = get_repo_root();
    let config_dir = get_config_dir();

    // Filter out any --config-dir arguments from the input since we use absolute path
    let filtered_args: Vec<&str> = {
        let mut result = Vec::new();
        let mut skip_next = false;
        for arg in args {
            if skip_next {
                skip_next = false;
                continue;
            }
            if *arg == "--config-dir" {
                skip_next = true;
                continue;
            }
            result.push(*arg);
        }
        result
    };

    let output = Command::new("cargo")
        .current_dir(&repo_root)
        .arg("run")
        .arg("-p")
        .arg("ndp-gold-ddl")
        .arg("--quiet")
        .arg("--")
        .arg("--config-dir")
        .arg(&config_dir)
        .args(&filtered_args)
        .output()
        .expect("Failed to execute ndp-gold-ddl");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "ndp-gold-ddl failed with exit code {:?}:\n{}",
            output.status.code(),
            stderr
        );
    }

    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Load a baseline fixture file
///
/// # Arguments
///
/// * `filename` - Name of the fixture file
///
/// # Returns
///
/// The file contents as a String
///
/// # Panics
///
/// Panics if the fixture file doesn't exist (indicates capture script wasn't run)
fn load_baseline(filename: &str) -> String {
    let fixtures_dir = get_fixtures_dir();
    let path = fixtures_dir.join(filename);
    fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "Baseline fixture '{}' not found: {}\n\
             Run the capture script first: ./scripts/capture-golden-master.sh",
            path.display(),
            e
        )
    })
}

/// Normalize SQL for comparison by sorting SELECT column lines within sections
///
/// The DDL generator may produce fields in different orders due to HashMap iteration.
/// This function normalizes the SQL to make it comparable while preserving structure.
/// It handles:
/// - Simple stream DDLs (SELECT with AVG/MIN/MAX lines)
/// - Multi-stream domain DDLs with section comments like `-- indoor (Observation)`
fn normalize_sql_for_comparison(sql: &str) -> String {
    let mut result = Vec::new();
    let mut in_select = false;
    let mut current_section: Option<String> = None;
    let mut section_lines: Vec<String> = Vec::new();
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    let mut unsectioned_select_lines: Vec<String> = Vec::new();

    for line in sql.lines() {
        let trimmed = line.trim();

        // Detect start of SELECT block
        if trimmed.contains("SELECT") && !trimmed.starts_with("--") {
            in_select = true;
            result.push(line.to_string());
            continue;
        }

        // Detect end of SELECT block (FROM, JOIN, WHERE, etc.)
        if in_select
            && (trimmed.starts_with("FROM")
                || trimmed.starts_with("FULL")
                || trimmed.starts_with("LEFT")
                || trimmed.starts_with("INNER")
                || trimmed.starts_with("WHERE")
                || trimmed.starts_with("GROUP")
                || trimmed.starts_with("ORDER"))
        {
            // Flush last section if we have sectioned content
            if let Some(section_name) = current_section.take() {
                sections.push((section_name, section_lines.drain(..).collect()));
            }

            // Output content: either sections (domain DDL) or unsectioned (stream DDL)
            if !sections.is_empty() {
                // Domain DDL with sections - sort sections by name
                sections.sort_by(|a, b| a.0.cmp(&b.0));
                for (section_header, mut lines) in sections.drain(..) {
                    result.push(section_header);
                    lines.sort();
                    result.extend(lines);
                }
            } else if !unsectioned_select_lines.is_empty() {
                // Stream DDL without sections - sort all SELECT lines
                unsectioned_select_lines.sort();
                result.extend(unsectioned_select_lines.drain(..));
            }

            in_select = false;
            result.push(line.to_string());
            continue;
        }

        if in_select {
            // Check for section header comment (e.g., "    -- indoor (Observation)")
            if trimmed.starts_with("--") && trimmed.contains("(") {
                // Flush unsectioned lines to first section
                if !unsectioned_select_lines.is_empty() {
                    // These are lines like "COALESCE(...) AS bucket,"
                    // Keep them at the top, not sorted with sections
                    for l in unsectioned_select_lines.drain(..) {
                        result.push(l);
                    }
                }
                // Flush previous section
                if let Some(section_name) = current_section.take() {
                    sections.push((section_name, section_lines.drain(..).collect()));
                }
                current_section = Some(line.to_string());
            } else if current_section.is_some() {
                // We're inside a section
                section_lines.push(line.to_string());
            } else {
                // Lines before any section (stream DDL or bucket/ndp_id lines)
                unsectioned_select_lines.push(line.to_string());
            }
        } else {
            result.push(line.to_string());
        }
    }

    // Flush any remaining content
    if let Some(section_name) = current_section.take() {
        sections.push((section_name, section_lines.drain(..).collect()));
    }
    if !sections.is_empty() {
        sections.sort_by(|a, b| a.0.cmp(&b.0));
        for (section_header, mut lines) in sections.drain(..) {
            result.push(section_header);
            lines.sort();
            result.extend(lines);
        }
    }
    if !unsectioned_select_lines.is_empty() {
        unsectioned_select_lines.sort();
        result.extend(unsectioned_select_lines);
    }

    result.join("\n")
}

/// Assert that DDL output matches baseline (normalized for field ordering)
///
/// This comparison normalizes SELECT column ordering since the DDL generator
/// may produce fields in non-deterministic order due to HashMap iteration.
///
/// # Arguments
///
/// * `expected` - The baseline DDL output
/// * `actual` - The current DDL output
/// * `context` - Description of what's being compared (for error messages)
///
/// # Panics
///
/// Panics with detailed diff if normalized expected != normalized actual
fn assert_golden_master(expected: &str, actual: &str, context: &str) {
    let normalized_expected = normalize_sql_for_comparison(expected);
    let normalized_actual = normalize_sql_for_comparison(actual);

    if normalized_expected != normalized_actual {
        eprintln!("\n{}", "=".repeat(70));
        eprintln!("GOLDEN MASTER MISMATCH: {}", context);
        eprintln!("{}", "=".repeat(70));
        eprintln!();

        // Find first difference
        let expected_lines: Vec<&str> = normalized_expected.lines().collect();
        let actual_lines: Vec<&str> = normalized_actual.lines().collect();

        let mut first_diff_shown = false;
        for (i, (e, a)) in expected_lines.iter().zip(actual_lines.iter()).enumerate() {
            if e != a && !first_diff_shown {
                eprintln!("First difference at line {} (normalized):", i + 1);
                eprintln!("  Expected: {}", e);
                eprintln!("  Actual:   {}", a);
                eprintln!();
                first_diff_shown = true;
            }
        }

        // Check for length differences
        if expected_lines.len() != actual_lines.len() {
            eprintln!(
                "Line count differs: expected {} lines, got {} lines",
                expected_lines.len(),
                actual_lines.len()
            );

            if expected_lines.len() > actual_lines.len() {
                eprintln!("Missing lines starting at line {}", actual_lines.len() + 1);
            } else {
                eprintln!("Extra lines starting at line {}", expected_lines.len() + 1);
            }
        }

        eprintln!();
        eprintln!("{}", "=".repeat(70));
        eprintln!("INVESTIGATION REQUIRED");
        eprintln!("{}", "=".repeat(70));
        eprintln!("DDL output has changed. This may indicate:");
        eprintln!("  1. JSON parsing differs from YAML parsing");
        eprintln!("  2. Missing or extra fields");
        eprintln!("  3. Changed SQL structure");
        eprintln!("  4. Optional field defaults changed");
        eprintln!();
        eprintln!("If this is INTENTIONAL, update the baseline:");
        eprintln!("  ./scripts/capture-golden-master.sh");
        eprintln!();

        panic!(
            "Golden master mismatch for '{}'. DDL output has changed.",
            context
        );
    }
}

// =============================================================================
// Golden Master Tests - Domain Level
// =============================================================================

/// GM-001: Domain aligned view generation in SYNC mode
///
/// Tests that `--domain indoor-air-quality --action sync` produces identical DDL
/// before and after the YAML to JSON migration.
#[test]
fn golden_master_domain_sync() {
    let expected = load_baseline("domain_indoor-air-quality_sync.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir",
        "./config",
        "generate",
        "--domain",
        "indoor-air-quality",
        "--action",
        "sync",
    ]);
    assert_golden_master(&expected, &actual, "domain indoor-air-quality sync");
}

/// GM-002: Domain aligned view generation in RECREATE mode
///
/// Tests that `--domain indoor-air-quality --action recreate` produces identical DDL.
/// This includes DROP CASCADE statements.
#[test]
fn golden_master_domain_recreate() {
    let expected = load_baseline("domain_indoor-air-quality_recreate.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir",
        "./config",
        "generate",
        "--domain",
        "indoor-air-quality",
        "--action",
        "recreate",
    ]);
    assert_golden_master(&expected, &actual, "domain indoor-air-quality recreate");
}

// =============================================================================
// Golden Master Tests - Stream Level (air-quality)
// =============================================================================

/// GM-003: air-quality stream continuous aggregate in SYNC mode
#[test]
fn golden_master_stream_air_quality_sync() {
    let expected = load_baseline("stream_air-quality_sync.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir",
        "./config",
        "generate",
        "--stream",
        "air-quality",
        "--action",
        "sync",
    ]);
    assert_golden_master(&expected, &actual, "stream air-quality sync");
}

/// GM-004: air-quality stream continuous aggregate in RECREATE mode
#[test]
fn golden_master_stream_air_quality_recreate() {
    let expected = load_baseline("stream_air-quality_recreate.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir",
        "./config",
        "generate",
        "--stream",
        "air-quality",
        "--action",
        "recreate",
    ]);
    assert_golden_master(&expected, &actual, "stream air-quality recreate");
}

// =============================================================================
// Golden Master Tests - Stream Level (outdoor-weather)
// =============================================================================

/// GM-005: outdoor-weather stream continuous aggregate in SYNC mode
#[test]
fn golden_master_stream_outdoor_weather_sync() {
    let expected = load_baseline("stream_outdoor-weather_sync.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir",
        "./config",
        "generate",
        "--stream",
        "outdoor-weather",
        "--action",
        "sync",
    ]);
    assert_golden_master(&expected, &actual, "stream outdoor-weather sync");
}

/// GM-006: outdoor-weather stream continuous aggregate in RECREATE mode
#[test]
fn golden_master_stream_outdoor_weather_recreate() {
    let expected = load_baseline("stream_outdoor-weather_recreate.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir",
        "./config",
        "generate",
        "--stream",
        "outdoor-weather",
        "--action",
        "recreate",
    ]);
    assert_golden_master(&expected, &actual, "stream outdoor-weather recreate");
}

// =============================================================================
// Golden Master Tests - Stream Level (home-assistant-state)
// =============================================================================

/// GM-007: home-assistant-state stream continuous aggregate in SYNC mode
#[test]
fn golden_master_stream_home_assistant_state_sync() {
    let expected = load_baseline("stream_home-assistant-state_sync.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir",
        "./config",
        "generate",
        "--stream",
        "home-assistant-state",
        "--action",
        "sync",
    ]);
    assert_golden_master(&expected, &actual, "stream home-assistant-state sync");
}

/// GM-008: home-assistant-state stream continuous aggregate in RECREATE mode
#[test]
fn golden_master_stream_home_assistant_state_recreate() {
    let expected = load_baseline("stream_home-assistant-state_recreate.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir",
        "./config",
        "generate",
        "--stream",
        "home-assistant-state",
        "--action",
        "recreate",
    ]);
    assert_golden_master(&expected, &actual, "stream home-assistant-state recreate");
}

// =============================================================================
// Golden Master Tests - Stream Level (outdoor-air-quality)
// =============================================================================

/// GM-009: outdoor-air-quality stream continuous aggregate in SYNC mode
#[test]
fn golden_master_stream_outdoor_aqi_sync() {
    let expected = load_baseline("stream_outdoor-air-quality_sync.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir",
        "./config",
        "generate",
        "--stream",
        "outdoor-air-quality",
        "--action",
        "sync",
    ]);
    assert_golden_master(&expected, &actual, "stream outdoor-air-quality sync");
}

/// GM-010: outdoor-air-quality stream continuous aggregate in RECREATE mode
#[test]
fn golden_master_stream_outdoor_aqi_recreate() {
    let expected = load_baseline("stream_outdoor-air-quality_recreate.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir",
        "./config",
        "generate",
        "--stream",
        "outdoor-air-quality",
        "--action",
        "recreate",
    ]);
    assert_golden_master(&expected, &actual, "stream outdoor-air-quality recreate");
}

// =============================================================================
// Golden Master Tests - Transitions
// =============================================================================

/// GM-011: State transitions view in SYNC mode
///
/// Tests that state transition DDL (for detecting state changes) is unchanged.
#[test]
fn golden_master_transitions_sync() {
    let expected = load_baseline("stream_home-assistant-state_transitions_sync.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir",
        "./config",
        "generate",
        "--stream",
        "home-assistant-state",
        "--transitions",
        "--action",
        "sync",
    ]);
    assert_golden_master(
        &expected,
        &actual,
        "stream home-assistant-state transitions sync",
    );
}

/// GM-012: State transitions view in RECREATE mode
#[test]
fn golden_master_transitions_recreate() {
    let expected = load_baseline("stream_home-assistant-state_transitions_recreate.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir",
        "./config",
        "generate",
        "--stream",
        "home-assistant-state",
        "--transitions",
        "--action",
        "recreate",
    ]);
    assert_golden_master(
        &expected,
        &actual,
        "stream home-assistant-state transitions recreate",
    );
}

// =============================================================================
// Golden Master Tests - Events
// =============================================================================

/// GM-013: Domain events DDL generation in SYNC mode
///
/// Tests that `--domain indoor-air-quality --events --action sync` produces
/// identical DDL, ensuring events infrastructure generation is stable.
#[test]
fn golden_master_events_sync() {
    let expected = load_baseline("domain_indoor-air-quality_events_sync.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir",
        "./config",
        "generate",
        "--domain",
        "indoor-air-quality",
        "--events",
        "--action",
        "sync",
    ]);
    assert_golden_master(&expected, &actual, "domain indoor-air-quality events sync");
}

/// GM-014: Domain events DDL generation in RECREATE mode
///
/// Tests that `--domain indoor-air-quality --events --action recreate` produces
/// identical DDL. This includes DROP CASCADE statements for all events objects.
#[test]
fn golden_master_events_recreate() {
    let expected = load_baseline("domain_indoor-air-quality_events_recreate.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir",
        "./config",
        "generate",
        "--domain",
        "indoor-air-quality",
        "--events",
        "--action",
        "recreate",
    ]);
    assert_golden_master(
        &expected,
        &actual,
        "domain indoor-air-quality events recreate",
    );
}

// =============================================================================
// Checksum Verification Test
// =============================================================================

/// Verify that the baseline fixtures haven't been accidentally modified
///
/// This test reads CHECKSUMS.sha256 and verifies all fixture files match.
#[test]
fn verify_fixture_checksums() {
    use sha2::Digest;

    let fixtures_dir = get_fixtures_dir();
    let checksums_path = fixtures_dir.join("CHECKSUMS.sha256");
    let checksums_content = fs::read_to_string(&checksums_path).unwrap_or_else(|e| {
        panic!(
            "CHECKSUMS.sha256 not found: {}\n\
             Run the capture script first: ./scripts/capture-golden-master.sh",
            e
        )
    });

    // Parse and verify each checksum
    for line in checksums_content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() != 2 {
            continue;
        }

        let expected_hash = parts[0];
        let filename = parts[1];

        let file_path = fixtures_dir.join(filename);
        let content = match fs::read(&file_path) {
            Ok(c) => c,
            Err(e) => {
                panic!("Failed to read fixture '{}': {}", file_path.display(), e);
            }
        };

        // Calculate SHA-256
        let mut hasher = sha2::Sha256::new();
        hasher.update(&content);
        let actual_hash = format!("{:x}", hasher.finalize());

        assert_eq!(
            expected_hash, actual_hash,
            "Checksum mismatch for '{}'. Fixture may have been modified.",
            filename
        );
    }
}
