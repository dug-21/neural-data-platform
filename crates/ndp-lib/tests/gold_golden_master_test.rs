//! Golden Master Tests for ndp-lib Gold Module
//!
//! These tests ensure that the gold module migration from ndp-gold-ddl to ndp-lib
//! preserves EXACT DDL output. Any change in DDL output indicates a behavioral
//! change that MUST be investigated.
//!
//! Unlike the ndp-gold-ddl golden master tests (which shell out to the binary),
//! these tests call the ndp-lib API directly, verifying the library functions
//! that the CLI dispatches to.
//!
//! # Test Categories
//!
//! - Domain-level outputs (sync and recreate modes)
//! - Stream-level outputs (continuous aggregates)
//! - Transition outputs (state change tracking)
//! - Events outputs (domain event infrastructure)
//!
//! # Usage
//!
//! ```bash
//! cargo test -p ndp-lib --test gold_golden_master_test
//! cargo test -p ndp-lib --test gold_golden_master_test -- --nocapture
//! ```

use std::fs;
use std::path::PathBuf;

use ndp_lib::gold::{
    config::FileSystemConfigLoader, generate_domain, generate_stream, recreate_stream,
    GenerateOptions,
};

// =============================================================================
// Helper Functions
// =============================================================================

/// Get the repository root directory.
fn get_repo_root() -> PathBuf {
    // crates/ndp-lib
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_root
        .parent()
        .expect("Expected crates directory")
        .parent()
        .expect("Expected repo root")
        .to_path_buf()
}

/// Get the path to the golden master fixtures directory.
fn get_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("golden-master")
}

/// Create a FileSystemConfigLoader pointing at the real config directory.
fn create_loader() -> FileSystemConfigLoader {
    let config_dir = get_repo_root().join("config");
    FileSystemConfigLoader::new(&config_dir)
}

/// Load a baseline fixture file.
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

/// Normalize SQL for comparison by sorting SELECT column lines within sections.
///
/// The DDL generator may produce fields in different orders due to HashMap iteration.
/// This function normalizes the SQL to make it comparable while preserving structure.
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
                // Flush unsectioned lines to top
                if !unsectioned_select_lines.is_empty() {
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
                section_lines.push(line.to_string());
            } else {
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

/// Assert that DDL output matches baseline (normalized for field ordering).
///
/// Both sides are trimmed before comparison to handle trailing newline differences
/// between file-read baselines (which may have trailing `\n\n`) and API output.
fn assert_golden_master(expected: &str, actual: &str, context: &str) {
    let normalized_expected = normalize_sql_for_comparison(expected.trim());
    let normalized_actual = normalize_sql_for_comparison(actual.trim());

    if normalized_expected != normalized_actual {
        eprintln!("\n{}", "=".repeat(70));
        eprintln!("GOLDEN MASTER MISMATCH: {}", context);
        eprintln!("{}", "=".repeat(70));
        eprintln!();

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

        if expected_lines.len() != actual_lines.len() {
            eprintln!(
                "Line count differs: expected {} lines, got {} lines",
                expected_lines.len(),
                actual_lines.len()
            );
        }

        eprintln!();
        eprintln!("If this is INTENTIONAL, update the baseline fixtures.");
        eprintln!();

        panic!(
            "Golden master mismatch for '{}'. DDL output has changed.",
            context
        );
    }
}

// =============================================================================
// Golden Master Tests - Domain Level (via ndp_lib::gold API)
// =============================================================================

/// GM-LIB-001: Domain aligned view generation in SYNC mode
#[test]
fn golden_master_lib_domain_sync() {
    let loader = create_loader();
    let opts = GenerateOptions {
        transitions: false,
        events: false,
        verbose: false,
    };

    let expected = load_baseline("domain_indoor-air-quality_sync.sql");
    let actual = generate_domain(&loader, "indoor-air-quality", &opts)
        .expect("generate_domain should succeed");

    assert_golden_master(&expected, &actual, "ndp-lib domain indoor-air-quality sync");
}

/// GM-LIB-002: Domain aligned view generation in RECREATE mode
///
/// Recreate for domains goes through AlignedViewGenerator directly with
/// Action::Recreate, which includes DROP CASCADE statements.
#[test]
fn golden_master_lib_domain_recreate() {
    let loader = create_loader();

    let expected = load_baseline("domain_indoor-air-quality_recreate.sql");

    // Domain recreate uses AlignedViewGenerator with Action::Recreate
    let domain_config =
        ndp_lib::gold::config::ConfigLoader::load_domain_config(&loader, "indoor-air-quality")
            .expect("load_domain_config should succeed");

    let generator = ndp_lib::gold::AlignedViewGenerator::new(loader);
    let actual = generator
        .generate(&domain_config, ndp_lib::gold::Action::Recreate)
        .expect("AlignedViewGenerator::generate should succeed");

    assert_golden_master(
        &expected,
        &actual,
        "ndp-lib domain indoor-air-quality recreate",
    );
}

// =============================================================================
// Golden Master Tests - Stream Level (air-quality)
// =============================================================================

/// GM-LIB-003: air-quality stream continuous aggregate in SYNC mode
#[test]
fn golden_master_lib_stream_air_quality_sync() {
    let loader = create_loader();
    let opts = GenerateOptions {
        transitions: false,
        events: false,
        verbose: false,
    };

    let expected = load_baseline("stream_air-quality_sync.sql");
    let actual =
        generate_stream(&loader, "air-quality", &opts).expect("generate_stream should succeed");

    assert_golden_master(&expected, &actual, "ndp-lib stream air-quality sync");
}

/// GM-LIB-004: air-quality stream continuous aggregate in RECREATE mode
#[test]
fn golden_master_lib_stream_air_quality_recreate() {
    let loader = create_loader();
    let opts = GenerateOptions {
        transitions: false,
        events: false,
        verbose: false,
    };

    let expected = load_baseline("stream_air-quality_recreate.sql");
    let actual =
        recreate_stream(&loader, "air-quality", &opts).expect("recreate_stream should succeed");

    assert_golden_master(&expected, &actual, "ndp-lib stream air-quality recreate");
}

// =============================================================================
// Golden Master Tests - Stream Level (outdoor-weather)
// =============================================================================

/// GM-LIB-005: outdoor-weather stream continuous aggregate in SYNC mode
#[test]
fn golden_master_lib_stream_outdoor_weather_sync() {
    let loader = create_loader();
    let opts = GenerateOptions {
        transitions: false,
        events: false,
        verbose: false,
    };

    let expected = load_baseline("stream_outdoor-weather_sync.sql");
    let actual =
        generate_stream(&loader, "outdoor-weather", &opts).expect("generate_stream should succeed");

    assert_golden_master(&expected, &actual, "ndp-lib stream outdoor-weather sync");
}

/// GM-LIB-006: outdoor-weather stream continuous aggregate in RECREATE mode
#[test]
fn golden_master_lib_stream_outdoor_weather_recreate() {
    let loader = create_loader();
    let opts = GenerateOptions {
        transitions: false,
        events: false,
        verbose: false,
    };

    let expected = load_baseline("stream_outdoor-weather_recreate.sql");
    let actual =
        recreate_stream(&loader, "outdoor-weather", &opts).expect("recreate_stream should succeed");

    assert_golden_master(
        &expected,
        &actual,
        "ndp-lib stream outdoor-weather recreate",
    );
}

// =============================================================================
// Golden Master Tests - Stream Level (home-assistant-state)
// =============================================================================

/// GM-LIB-007: home-assistant-state stream continuous aggregate in SYNC mode
#[test]
fn golden_master_lib_stream_home_assistant_state_sync() {
    let loader = create_loader();
    let opts = GenerateOptions {
        transitions: false,
        events: false,
        verbose: false,
    };

    let expected = load_baseline("stream_home-assistant-state_sync.sql");
    let actual = generate_stream(&loader, "home-assistant-state", &opts)
        .expect("generate_stream should succeed");

    assert_golden_master(
        &expected,
        &actual,
        "ndp-lib stream home-assistant-state sync",
    );
}

/// GM-LIB-008: home-assistant-state stream continuous aggregate in RECREATE mode
#[test]
fn golden_master_lib_stream_home_assistant_state_recreate() {
    let loader = create_loader();
    let opts = GenerateOptions {
        transitions: false,
        events: false,
        verbose: false,
    };

    let expected = load_baseline("stream_home-assistant-state_recreate.sql");
    let actual = recreate_stream(&loader, "home-assistant-state", &opts)
        .expect("recreate_stream should succeed");

    assert_golden_master(
        &expected,
        &actual,
        "ndp-lib stream home-assistant-state recreate",
    );
}

// =============================================================================
// Golden Master Tests - Stream Level (outdoor-air-quality)
// =============================================================================

/// GM-LIB-009: outdoor-air-quality stream continuous aggregate in SYNC mode
#[test]
fn golden_master_lib_stream_outdoor_aqi_sync() {
    let loader = create_loader();
    let opts = GenerateOptions {
        transitions: false,
        events: false,
        verbose: false,
    };

    let expected = load_baseline("stream_outdoor-air-quality_sync.sql");
    let actual = generate_stream(&loader, "outdoor-air-quality", &opts)
        .expect("generate_stream should succeed");

    assert_golden_master(
        &expected,
        &actual,
        "ndp-lib stream outdoor-air-quality sync",
    );
}

/// GM-LIB-010: outdoor-air-quality stream continuous aggregate in RECREATE mode
#[test]
fn golden_master_lib_stream_outdoor_aqi_recreate() {
    let loader = create_loader();
    let opts = GenerateOptions {
        transitions: false,
        events: false,
        verbose: false,
    };

    let expected = load_baseline("stream_outdoor-air-quality_recreate.sql");
    let actual = recreate_stream(&loader, "outdoor-air-quality", &opts)
        .expect("recreate_stream should succeed");

    assert_golden_master(
        &expected,
        &actual,
        "ndp-lib stream outdoor-air-quality recreate",
    );
}

// =============================================================================
// Golden Master Tests - Transitions
// =============================================================================

/// GM-LIB-011: State transitions view in SYNC mode
#[test]
fn golden_master_lib_transitions_sync() {
    let loader = create_loader();
    let opts = GenerateOptions {
        transitions: true,
        events: false,
        verbose: false,
    };

    let expected = load_baseline("stream_home-assistant-state_transitions_sync.sql");
    let actual = generate_stream(&loader, "home-assistant-state", &opts)
        .expect("generate_stream with transitions should succeed");

    assert_golden_master(
        &expected,
        &actual,
        "ndp-lib stream home-assistant-state transitions sync",
    );
}

/// GM-LIB-012: State transitions view in RECREATE mode
///
/// Note: recreate_stream does not currently support transitions flag.
/// The transitions flag is only used with generate_stream, which always
/// uses Action::Sync. For recreate with transitions, we use generate_stream
/// with transitions=true since the ndp-gold-ddl baseline was captured that way.
#[test]
fn golden_master_lib_transitions_recreate() {
    let loader = create_loader();

    let expected = load_baseline("stream_home-assistant-state_transitions_recreate.sql");

    // For transitions recreate, we need to go through the generator directly
    // since recreate_stream does not support the transitions flag.
    let stream_config =
        ndp_lib::gold::config::ConfigLoader::load_stream_config(&loader, "home-assistant-state")
            .expect("load_stream_config should succeed");

    let transition_config = ndp_lib::gold::TransitionConfig::from_stream_config(&stream_config)
        .unwrap_or_else(|| ndp_lib::gold::TransitionConfig::new("state", "ndp_id"));
    let generator = ndp_lib::gold::StateTransitionGenerator::from_stream_config(&stream_config)
        .expect("StateTransitionGenerator::from_stream_config should succeed");
    let actual = generator
        .generate(&transition_config, ndp_lib::gold::Action::Recreate)
        .expect("StateTransitionGenerator::generate should succeed");

    assert_golden_master(
        &expected,
        &actual,
        "ndp-lib stream home-assistant-state transitions recreate",
    );
}

// =============================================================================
// Golden Master Tests - Events
// =============================================================================

/// GM-LIB-013: Domain events DDL generation in SYNC mode
#[test]
fn golden_master_lib_events_sync() {
    let loader = create_loader();
    let opts = GenerateOptions {
        transitions: false,
        events: true,
        verbose: false,
    };

    let expected = load_baseline("domain_indoor-air-quality_events_sync.sql");
    let actual = generate_domain(&loader, "indoor-air-quality", &opts)
        .expect("generate_domain with events should succeed");

    assert_golden_master(
        &expected,
        &actual,
        "ndp-lib domain indoor-air-quality events sync",
    );
}

/// GM-LIB-014: Domain events DDL generation in RECREATE mode
///
/// Events recreate goes through EventsGenerator directly with Action::Recreate.
#[test]
fn golden_master_lib_events_recreate() {
    let loader = create_loader();

    let expected = load_baseline("domain_indoor-air-quality_events_recreate.sql");

    let domain_config =
        ndp_lib::gold::config::ConfigLoader::load_domain_config(&loader, "indoor-air-quality")
            .expect("load_domain_config should succeed");

    let generator =
        ndp_lib::gold::EventsGenerator::from_domain_config(&domain_config, Box::new(loader));
    let actual = generator
        .generate(ndp_lib::gold::Action::Recreate)
        .expect("EventsGenerator::generate should succeed");

    assert_golden_master(
        &expected,
        &actual,
        "ndp-lib domain indoor-air-quality events recreate",
    );
}

// =============================================================================
// Checksum Verification Test
// =============================================================================

/// Verify that the baseline fixtures have not been accidentally modified.
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
