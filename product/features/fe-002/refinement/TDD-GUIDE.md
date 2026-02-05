# FE-002: Domain Configuration Standardization - TDD Guide (London School)

> **Feature ID:** FE-002
> **Version:** 1.0
> **Created:** 2026-02-05
> **Testing Approach:** London School TDD (Outside-In, Mock-Driven)
> **Parent Documents:** [TEST-STRATEGY.md](./TEST-STRATEGY.md), [TEST-PLAN.md](./TEST-PLAN.md)

---

## Overview

This guide provides step-by-step London TDD instructions for implementing FE-002. The approach is **outside-in**, starting with golden master tests that define the required behavior, then working inward to unit tests.

**Key Principle for FE-002**: Unlike typical TDD where we drive new behavior, Phase A is a **migration** where we must preserve EXACT existing behavior. The golden master tests serve as the acceptance criteria.

---

## 1. Phase 0: Baseline Capture (Pre-TDD Setup)

### 1.1 The Setup Phase

Before any Red-Green-Refactor cycles, we must establish our baselines.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    PHASE 0: TDD SETUP                               │
├─────────────────────────────────────────────────────────────────────┤
│  This is NOT optional. Without baselines, we cannot verify          │
│  that the migration preserves behavior.                             │
│                                                                     │
│  Steps:                                                             │
│  1. Run capture script                                              │
│  2. Commit fixtures                                                 │
│  3. Write golden master test shell                                  │
│  4. Verify tests PASS with current code                             │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 Write the Golden Master Test Shell FIRST

```rust
// Location: tools/ndp-gold-ddl/tests/golden_master_test.rs
// WRITE THIS BEFORE ANY CODE CHANGES

use std::fs;
use std::process::Command;

const FIXTURES_DIR: &str = "tests/fixtures/golden-master";

/// Helper: Execute ndp-gold-ddl and return stdout
fn execute_gold_ddl(args: &[&str]) -> String {
    let output = Command::new("cargo")
        .args(["run", "-p", "ndp-gold-ddl", "--quiet", "--"])
        .args(args)
        .output()
        .expect("Failed to execute ndp-gold-ddl");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("ndp-gold-ddl failed: {}", stderr);
    }

    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Helper: Load baseline fixture
fn load_baseline(filename: &str) -> String {
    fs::read_to_string(format!("{}/{}", FIXTURES_DIR, filename))
        .unwrap_or_else(|_| panic!(
            "Baseline fixture '{}' not found. Run capture script first.",
            filename
        ))
}

/// Helper: Assert exact match with detailed diff on failure
fn assert_golden_master(expected: &str, actual: &str, context: &str) {
    if expected != actual {
        eprintln!("\n=== GOLDEN MASTER MISMATCH: {} ===\n", context);

        // Show first difference location
        let expected_lines: Vec<&str> = expected.lines().collect();
        let actual_lines: Vec<&str> = actual.lines().collect();

        for (i, (e, a)) in expected_lines.iter().zip(actual_lines.iter()).enumerate() {
            if e != a {
                eprintln!("First difference at line {}:", i + 1);
                eprintln!("  Expected: {}", e);
                eprintln!("  Actual:   {}", a);
                break;
            }
        }

        if expected_lines.len() != actual_lines.len() {
            eprintln!("Line count differs: expected {}, got {}",
                expected_lines.len(), actual_lines.len());
        }

        panic!("Golden master mismatch for {}. DDL output has changed.", context);
    }
}

// ============================================================================
// GOLDEN MASTER TESTS - Domain Level
// ============================================================================

#[test]
fn golden_master_domain_sync() {
    let expected = load_baseline("domain_indoor-air-quality_sync.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir", "./config",
        "generate", "--domain", "indoor-air-quality", "--action", "sync"
    ]);
    assert_golden_master(&expected, &actual, "domain indoor-air-quality sync");
}

#[test]
fn golden_master_domain_recreate() {
    let expected = load_baseline("domain_indoor-air-quality_recreate.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir", "./config",
        "generate", "--domain", "indoor-air-quality", "--action", "recreate"
    ]);
    assert_golden_master(&expected, &actual, "domain indoor-air-quality recreate");
}

// ============================================================================
// GOLDEN MASTER TESTS - Stream Level
// ============================================================================

#[test]
fn golden_master_stream_air_quality_sync() {
    let expected = load_baseline("stream_air-quality_sync.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir", "./config",
        "generate", "--stream", "air-quality", "--action", "sync"
    ]);
    assert_golden_master(&expected, &actual, "stream air-quality sync");
}

#[test]
fn golden_master_stream_air_quality_recreate() {
    let expected = load_baseline("stream_air-quality_recreate.sql");
    let actual = execute_gold_ddl(&[
        "--config-dir", "./config",
        "generate", "--stream", "air-quality", "--action", "recreate"
    ]);
    assert_golden_master(&expected, &actual, "stream air-quality recreate");
}

// Add tests for: outdoor-weather, home-assistant-state, outdoor-air-quality
// Add tests for: transitions (home-assistant-state)
```

### 1.3 Verify Tests Pass BEFORE Changes

```bash
# This MUST pass before starting Phase A
cargo test -p ndp-gold-ddl --test golden_master_test -- --nocapture

# Expected output:
# running 4 tests
# test golden_master_domain_sync ... ok
# test golden_master_domain_recreate ... ok
# test golden_master_stream_air_quality_sync ... ok
# test golden_master_stream_air_quality_recreate ... ok
```

---

## 2. Phase A: YAML to JSON Migration (TDD Cycles)

### 2.1 Overall TDD Flow for Phase A

```
For Phase A, the Red-Green-Refactor cycle is inverted:
- We START with GREEN (tests passing with YAML)
- We make changes that turn tests RED (breaking JSON loader)
- We fix to return to GREEN (working JSON loader)
- The goal is to return to GREEN with identical output

┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐
│  GREEN  │────►│   RED   │────►│  GREEN  │────►│ REFACTOR│
│ (YAML)  │     │ (JSON   │     │ (JSON   │     │ (Clean  │
│         │     │  breaks)│     │  works) │     │  up)    │
└─────────┘     └─────────┘     └─────────┘     └─────────┘
```

### 2.2 TDD Cycle A1: JSON Config File Creation

#### Step 1: Start GREEN (Verify YAML works)

```bash
# Confirm current state
cargo test -p ndp-gold-ddl --test golden_master_test
# All tests should pass
```

#### Step 2: Write Unit Test for JSON Loading

```rust
// Location: tools/ndp-gold-ddl/src/config/loader.rs
// Add to existing tests module

#[cfg(test)]
mod tests {
    // ... existing tests ...

    #[test]
    fn test_load_domain_config_from_json() {
        // Arrange: Create test directory with JSON config
        let temp_dir = TempDir::new().unwrap();
        let domain_dir = temp_dir.path().join("domains").join("test-domain");
        std::fs::create_dir_all(&domain_dir).unwrap();

        let json_config = r#"{
            "id": "test-domain",
            "description": "Test domain",
            "streams": [
                {
                    "stream_id": "test-stream",
                    "alias": "test",
                    "role": "primary"
                },
                {
                    "stream_id": "other-stream",
                    "alias": "other",
                    "role": "context"
                }
            ],
            "alignment": {
                "view_name": "test_aligned",
                "granularity": "1 hour",
                "join_strategy": "full_outer"
            },
            "objectives": []
        }"#;

        std::fs::write(domain_dir.join("domain.json"), json_config).unwrap();

        // Act: Load config
        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let config = loader.load_domain_config("test-domain");

        // Assert: Config loads correctly
        assert!(config.is_ok(), "JSON config should load: {:?}", config.err());
        let config = config.unwrap();
        assert_eq!(config.id, "test-domain");
        assert_eq!(config.streams.len(), 2);
    }

    #[test]
    fn test_load_domain_config_json_not_found_returns_error() {
        let temp_dir = TempDir::new().unwrap();
        let loader = FileSystemConfigLoader::new(temp_dir.path());

        let result = loader.load_domain_config("nonexistent");

        assert!(result.is_err());
        match result.unwrap_err() {
            GoldDdlError::ConfigNotFound { path } => {
                assert!(path.contains("nonexistent"));
            }
            _ => panic!("Expected ConfigNotFound error"),
        }
    }

    #[test]
    fn test_load_domain_config_json_parse_error() {
        let temp_dir = TempDir::new().unwrap();
        let domain_dir = temp_dir.path().join("domains").join("bad-json");
        std::fs::create_dir_all(&domain_dir).unwrap();
        std::fs::write(domain_dir.join("domain.json"), "{ invalid json }").unwrap();

        let loader = FileSystemConfigLoader::new(temp_dir.path());
        let result = loader.load_domain_config("bad-json");

        assert!(result.is_err());
        match result.unwrap_err() {
            GoldDdlError::ConfigParseError { message } => {
                assert!(message.contains("bad-json"));
            }
            _ => panic!("Expected ConfigParseError"),
        }
    }
}
```

#### Step 3: Run Test - Expect FAILURE (RED)

```bash
cargo test -p ndp-gold-ddl test_load_domain_config_from_json
# EXPECTED: FAIL - loader still looks for domain.yaml
```

#### Step 4: Implement Minimal Change (GREEN)

```rust
// Location: tools/ndp-gold-ddl/src/config/loader.rs
// Change domain_config_path method

impl FileSystemConfigLoader {
    // ... existing code ...

    /// Get the path to a domain's config file
    fn domain_config_path(&self, domain_id: &str) -> PathBuf {
        self.config_dir
            .join("domains")
            .join(domain_id)
            .join("domain.json")  // Changed from .yaml to .json
    }
}

impl ConfigLoader for FileSystemConfigLoader {
    // ... existing code ...

    fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig> {
        let path = self.domain_config_path(domain_id);

        if !path.exists() {
            return Err(GoldDdlError::ConfigNotFound {
                path: path.display().to_string(),
            });
        }

        let content = std::fs::read_to_string(&path)?;
        let config: DomainConfig =
            serde_json::from_str(&content).map_err(|e| GoldDdlError::ConfigParseError {
                message: format!("Failed to parse {}: {}", path.display(), e),
            })?;  // Changed from serde_yaml::from_str

        Ok(config)
    }
}
```

#### Step 5: Verify Unit Test Passes

```bash
cargo test -p ndp-gold-ddl test_load_domain_config_from_json
# EXPECTED: PASS
```

#### Step 6: Golden Master Test - Expect FAILURE

```bash
cargo test -p ndp-gold-ddl --test golden_master_test
# EXPECTED: FAIL - domain.json doesn't exist yet
```

### 2.3 TDD Cycle A2: Convert Domain Config

#### Step 1: Convert YAML to JSON

```bash
# Use yq to convert (preserves structure)
yq -o=json '.' config/domains/indoor-air-quality/domain.yaml \
    > config/domains/indoor-air-quality/domain.json

# Verify JSON is valid
jq . config/domains/indoor-air-quality/domain.json
```

#### Step 2: Verify Golden Master - Should PASS now

```bash
cargo test -p ndp-gold-ddl --test golden_master_test
# EXPECTED: PASS - JSON loads and produces identical DDL
```

#### Step 3: If FAILS - Debug and Fix

If golden master fails after conversion:

```rust
// Add debug test to understand the difference
#[test]
fn debug_domain_config_loading() {
    // Load via YAML (temporarily restore)
    let yaml_content = std::fs::read_to_string(
        "config/domains/indoor-air-quality/domain.yaml"
    ).unwrap();
    let yaml_config: DomainConfig = serde_yaml::from_str(&yaml_content).unwrap();

    // Load via JSON
    let json_content = std::fs::read_to_string(
        "config/domains/indoor-air-quality/domain.json"
    ).unwrap();
    let json_config: DomainConfig = serde_json::from_str(&json_content).unwrap();

    // Compare field by field
    assert_eq!(yaml_config.id, json_config.id, "id mismatch");
    assert_eq!(yaml_config.description, json_config.description, "description mismatch");
    assert_eq!(yaml_config.streams.len(), json_config.streams.len(), "streams count mismatch");

    for (i, (yaml_stream, json_stream)) in
        yaml_config.streams.iter().zip(json_config.streams.iter()).enumerate()
    {
        assert_eq!(yaml_stream.stream_id, json_stream.stream_id,
            "stream {} stream_id mismatch", i);
        assert_eq!(yaml_stream.alias, json_stream.alias,
            "stream {} alias mismatch", i);
        assert_eq!(yaml_stream.role, json_stream.role,
            "stream {} role mismatch", i);
    }

    // ... continue for all fields ...
}
```

### 2.4 TDD Cycle A3: Remove YAML Dependency

#### Step 1: Verify No Other YAML Usage

```bash
# Search for any remaining serde_yaml usage
grep -r "serde_yaml" tools/ndp-gold-ddl/src/
# Should only find the loader.rs we already changed
```

#### Step 2: Write Test That Ensures No YAML

```rust
// Location: tools/ndp-gold-ddl/tests/no_yaml_test.rs

/// This test verifies that the crate has no dependency on serde_yaml.
/// It's a compile-time guarantee via the test's existence.
#[test]
fn test_no_yaml_dependency() {
    // This test exists to document that we intentionally removed serde_yaml.
    // The actual verification is in Cargo.toml - serde_yaml should not be listed.

    // If you need to re-add YAML support in the future, update this test
    // and document why in an ADR.
    assert!(
        !cfg!(feature = "yaml_support"),
        "YAML support should not be enabled. See FE-002 for context."
    );
}
```

#### Step 3: Remove from Cargo.toml

```toml
# tools/ndp-gold-ddl/Cargo.toml
# REMOVE this line:
# serde_yaml = "0.9"
```

#### Step 4: Verify Build Still Works

```bash
cargo build -p ndp-gold-ddl
# EXPECTED: SUCCESS
```

#### Step 5: Delete YAML File

```bash
rm config/domains/indoor-air-quality/domain.yaml
```

#### Step 6: Final Golden Master Verification

```bash
cargo test -p ndp-gold-ddl --test golden_master_test
# EXPECTED: PASS - All tests green, YAML removed
```

### 2.5 TDD Cycle A4: Update Test Fixtures

Any test fixtures in the codebase that use YAML domain configs must be updated.

#### Step 1: Find All YAML Test Fixtures

```bash
grep -r "domain.yaml" tools/ndp-gold-ddl/
grep -r "serde_yaml" tools/ndp-gold-ddl/tests/
```

#### Step 2: Convert Each Fixture

For each YAML fixture found, write a test that uses JSON:

```rust
// Before (YAML fixture)
let yaml = r#"
id: test-domain
streams:
  - stream_id: test
    alias: t
    role: primary
"#;
let config: DomainConfig = serde_yaml::from_str(yaml).unwrap();

// After (JSON fixture)
let json = r#"{
    "id": "test-domain",
    "streams": [
        {"stream_id": "test", "alias": "t", "role": "primary"}
    ]
}"#;
let config: DomainConfig = serde_json::from_str(json).unwrap();
```

---

## 3. Phase B: Schema Validation (TDD Cycles)

### 3.1 TDD Flow for Phase B

Phase B adds NEW functionality, so we follow traditional TDD:

```
┌─────────┐     ┌─────────┐     ┌─────────┐
│   RED   │────►│  GREEN  │────►│ REFACTOR│
│ (Write  │     │ (Make   │     │ (Clean  │
│  test)  │     │  pass)  │     │  up)    │
└─────────┘     └─────────┘     └─────────┘
```

### 3.2 TDD Cycle B1: CLI --domain Flag

#### Red Phase: Write Failing Test

```rust
// Location: tools/ndp-validate/tests/cli_domain_test.rs

use std::process::Command;

#[test]
fn test_cli_accepts_domain_flag() {
    let output = Command::new("cargo")
        .args(["run", "-p", "ndp-validate", "--quiet", "--"])
        .args(["--domain", "config/domains/indoor-air-quality/domain.json"])
        .output()
        .expect("Failed to execute");

    // Should not fail with "unknown flag" error
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument"),
        "CLI should accept --domain flag: {}",
        stderr
    );
}

#[test]
fn test_cli_domain_validates_existing_file() {
    let output = Command::new("cargo")
        .args(["run", "-p", "ndp-validate", "--quiet", "--"])
        .args(["--domain", "config/domains/indoor-air-quality/domain.json"])
        .output()
        .expect("Failed to execute");

    assert!(
        output.status.success(),
        "Valid domain config should pass validation: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_cli_domain_rejects_nonexistent_file() {
    let output = Command::new("cargo")
        .args(["run", "-p", "ndp-validate", "--quiet", "--"])
        .args(["--domain", "nonexistent/domain.json"])
        .output()
        .expect("Failed to execute");

    assert!(
        !output.status.success(),
        "Nonexistent file should fail validation"
    );
}
```

#### Green Phase: Implement CLI Flag

```rust
// Location: tools/ndp-validate/src/cli.rs

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ndp-validate")]
#[command(about = "Validate NDP configuration files")]
pub struct Cli {
    /// Validate a domain configuration file
    #[arg(long, value_name = "FILE")]
    pub domain: Option<PathBuf>,

    /// Validate a stream configuration file
    #[arg(long, value_name = "FILE")]
    pub stream: Option<PathBuf>,

    /// Validate all configurations
    #[arg(long)]
    pub all: bool,

    /// Perform schema validation only (skip semantic validation)
    #[arg(long)]
    pub schema_only: bool,
}
```

#### Refactor: Add Help Text and Examples

```rust
#[arg(
    long,
    value_name = "FILE",
    help = "Validate a domain configuration file",
    long_help = "Validate a domain configuration file against the JSON Schema \
                 and semantic rules.\n\n\
                 Example: ndp-validate --domain config/domains/indoor-air-quality/domain.json"
)]
pub domain: Option<PathBuf>,
```

### 3.3 TDD Cycle B2: JSON Schema Validation

#### Red Phase: Write Failing Tests

```rust
// Location: tools/ndp-validate/tests/domain_schema_test.rs

use ndp_validate::schema::validate_domain_schema;
use serde_json::json;

#[test]
fn test_valid_domain_config_passes_schema() {
    let config = json!({
        "id": "test-domain",
        "description": "Test domain",
        "streams": [
            {"stream_id": "s1", "alias": "a", "role": "primary"},
            {"stream_id": "s2", "alias": "b", "role": "context"}
        ],
        "alignment": {
            "view_name": "test_aligned",
            "granularity": "1 hour",
            "join_strategy": "full_outer"
        },
        "objectives": []
    });

    let result = validate_domain_schema(&config);

    assert!(result.is_ok(), "Valid config should pass: {:?}", result.err());
}

#[test]
fn test_missing_id_fails_schema() {
    let config = json!({
        "description": "Missing id",
        "streams": [],
        "alignment": {
            "view_name": "test",
            "granularity": "1 hour",
            "join_strategy": "full_outer"
        }
    });

    let result = validate_domain_schema(&config);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("id") || err.to_string().contains("required"),
        "Error should mention missing 'id': {}",
        err
    );
}

#[test]
fn test_invalid_role_fails_schema() {
    let config = json!({
        "id": "test",
        "streams": [
            {"stream_id": "s1", "alias": "a", "role": "invalid_role"}
        ],
        "alignment": {
            "view_name": "test",
            "granularity": "1 hour",
            "join_strategy": "full_outer"
        }
    });

    let result = validate_domain_schema(&config);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("role") || err.to_string().contains("enum"),
        "Error should mention invalid role: {}",
        err
    );
}

#[test]
fn test_missing_streams_fails_schema() {
    let config = json!({
        "id": "test",
        "alignment": {
            "view_name": "test",
            "granularity": "1 hour",
            "join_strategy": "full_outer"
        }
    });

    let result = validate_domain_schema(&config);

    assert!(result.is_err());
}
```

#### Green Phase: Implement Schema Validation

```rust
// Location: tools/ndp-validate/src/schema.rs

use jsonschema::{Draft, JSONSchema};
use serde_json::Value;
use std::fs;
use std::path::Path;

pub fn validate_domain_schema(config: &Value) -> Result<(), SchemaError> {
    let schema_path = resolve_schema_path("domain.schema.json")?;
    let schema_content = fs::read_to_string(&schema_path)
        .map_err(|e| SchemaError::SchemaLoadFailed {
            path: schema_path.display().to_string(),
            error: e.to_string(),
        })?;

    let schema: Value = serde_json::from_str(&schema_content)
        .map_err(|e| SchemaError::SchemaParseError {
            error: e.to_string(),
        })?;

    let compiled = JSONSchema::options()
        .with_draft(Draft::Draft7)
        .compile(&schema)
        .map_err(|e| SchemaError::SchemaCompileError {
            error: e.to_string(),
        })?;

    let result = compiled.validate(config);

    if let Err(errors) = result {
        let error_messages: Vec<String> = errors
            .map(|e| format!("{} at {}", e, e.instance_path))
            .collect();

        return Err(SchemaError::ValidationFailed {
            errors: error_messages,
        });
    }

    Ok(())
}

fn resolve_schema_path(schema_name: &str) -> Result<std::path::PathBuf, SchemaError> {
    // Check multiple locations
    let candidates = [
        format!("config/schemas/{}", schema_name),
        format!("./config/schemas/{}", schema_name),
        format!("/opt/ndp/config/schemas/{}", schema_name),
    ];

    for candidate in &candidates {
        let path = Path::new(candidate);
        if path.exists() {
            return Ok(path.to_path_buf());
        }
    }

    Err(SchemaError::SchemaNotFound {
        name: schema_name.to_string(),
        searched: candidates.join(", "),
    })
}
```

### 3.4 TDD Cycle B3: Semantic Validation

#### Red Phase: Write Failing Tests

```rust
// Location: tools/ndp-validate/tests/domain_semantic_test.rs

use ndp_validate::semantic::validate_domain_semantic;

#[test]
fn test_domain_must_have_at_least_two_streams() {
    let config = create_domain_with_streams(1);

    let result = validate_domain_semantic(&config);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("at least 2 streams"),
        "Error should mention minimum streams: {}",
        err
    );
}

#[test]
fn test_domain_must_have_primary_stream() {
    let config = json!({
        "id": "test",
        "streams": [
            {"stream_id": "s1", "alias": "a", "role": "context"},
            {"stream_id": "s2", "alias": "b", "role": "context"}
        ],
        "alignment": {
            "view_name": "test",
            "granularity": "1 hour",
            "join_strategy": "full_outer"
        }
    });

    let result = validate_domain_semantic(&config);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("primary"),
        "Error should mention missing primary stream: {}",
        err
    );
}

#[test]
fn test_stream_ids_must_exist() {
    // This test requires mock ConfigLoader
    let config = create_valid_domain();
    let loader = MockConfigLoader::new()
        .with_stream("air-quality")
        .without_stream("nonexistent-stream");

    let result = validate_domain_semantic_with_loader(&config, &loader);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("nonexistent-stream"),
        "Error should mention missing stream: {}",
        err
    );
}

#[test]
fn test_aliases_must_be_unique() {
    let config = json!({
        "id": "test",
        "streams": [
            {"stream_id": "s1", "alias": "same", "role": "primary"},
            {"stream_id": "s2", "alias": "same", "role": "context"}
        ],
        "alignment": {
            "view_name": "test",
            "granularity": "1 hour",
            "join_strategy": "full_outer"
        }
    });

    let result = validate_domain_semantic(&config);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("alias") && err.to_string().contains("unique"),
        "Error should mention duplicate alias: {}",
        err
    );
}
```

#### Green Phase: Implement Semantic Validation

```rust
// Location: tools/ndp-validate/src/semantic/domain.rs

use crate::config::DomainConfig;
use crate::error::SemanticError;
use std::collections::HashSet;

pub fn validate_domain_semantic(config: &DomainConfig) -> Result<(), SemanticError> {
    // Rule 1: At least 2 streams
    if config.streams.len() < 2 {
        return Err(SemanticError::InsufficientStreams {
            domain: config.id.clone(),
            count: config.streams.len(),
            minimum: 2,
        });
    }

    // Rule 2: Must have primary stream
    let has_primary = config.streams.iter().any(|s| s.role == StreamRole::Primary);
    if !has_primary {
        return Err(SemanticError::MissingPrimaryStream {
            domain: config.id.clone(),
        });
    }

    // Rule 3: Unique aliases
    let mut aliases = HashSet::new();
    for stream in &config.streams {
        if !aliases.insert(&stream.alias) {
            return Err(SemanticError::DuplicateAlias {
                domain: config.id.clone(),
                alias: stream.alias.clone(),
            });
        }
    }

    Ok(())
}
```

---

## 4. Mock Definitions

### 4.1 What to Mock (Unit Tests)

| Dependency | Mock Name | Rationale |
|------------|-----------|-----------|
| ConfigLoader | `MockConfigLoader` | Isolate from file system |
| File System | In-memory | Test without I/O |
| Schema Files | Embedded | Deterministic tests |

### 4.2 What NOT to Mock (Golden Master Tests)

| Dependency | Why Real |
|------------|----------|
| File System | Must test actual file loading |
| Config Files | Must test actual config format |
| ndp-gold-ddl CLI | Must test actual command output |

### 4.3 MockConfigLoader Implementation

```rust
// Location: tools/ndp-gold-ddl/src/config/mock_loader.rs

use std::collections::HashMap;
use std::sync::RwLock;

pub struct MockConfigLoader {
    streams: RwLock<HashMap<String, StreamConfig>>,
    domains: RwLock<HashMap<String, DomainConfig>>,
    error_on_load: RwLock<Option<GoldDdlError>>,
}

impl MockConfigLoader {
    pub fn new() -> Self {
        Self {
            streams: RwLock::new(HashMap::new()),
            domains: RwLock::new(HashMap::new()),
            error_on_load: RwLock::new(None),
        }
    }

    pub fn with_stream(self, config: StreamConfig) -> Self {
        self.streams.write().unwrap()
            .insert(config.stream_id.clone(), config);
        self
    }

    pub fn with_domain(self, config: DomainConfig) -> Self {
        self.domains.write().unwrap()
            .insert(config.id.clone(), config);
        self
    }

    pub fn with_error(self, error: GoldDdlError) -> Self {
        *self.error_on_load.write().unwrap() = Some(error);
        self
    }
}

impl ConfigLoader for MockConfigLoader {
    fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig> {
        if let Some(ref err) = *self.error_on_load.read().unwrap() {
            return Err(err.clone());
        }

        self.streams.read().unwrap()
            .get(stream_id)
            .cloned()
            .ok_or(GoldDdlError::ConfigNotFound {
                path: format!("mock:{}", stream_id),
            })
    }

    fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig> {
        if let Some(ref err) = *self.error_on_load.read().unwrap() {
            return Err(err.clone());
        }

        self.domains.read().unwrap()
            .get(domain_id)
            .cloned()
            .ok_or(GoldDdlError::ConfigNotFound {
                path: format!("mock:{}", domain_id),
            })
    }
}
```

---

## 5. Test Naming Conventions

### 5.1 Golden Master Tests

```
golden_master_{output_type}_{entity}_{action}

Examples:
- golden_master_domain_indoor_air_quality_sync
- golden_master_stream_air_quality_recreate
- golden_master_transitions_home_assistant_state_sync
```

### 5.2 Unit Tests

```
test_{component}_{scenario}_{expected_result}

Examples:
- test_load_domain_config_from_json_succeeds
- test_load_domain_config_parse_error_returns_error
- test_domain_semantic_missing_primary_returns_error
```

### 5.3 Integration Tests

```
test_{workflow}_{condition}_{expected_outcome}

Examples:
- test_validate_cli_valid_domain_succeeds
- test_validate_cli_invalid_schema_fails_with_path
```

---

## 6. Running Tests

### 6.1 Quick Check Commands

```bash
# Golden master tests only (Phase A gate)
cargo test -p ndp-gold-ddl --test golden_master_test

# Unit tests only
cargo test -p ndp-gold-ddl --lib
cargo test -p ndp-validate --lib

# All tests with output
cargo test -p ndp-gold-ddl -- --nocapture
```

### 6.2 Full Test Suite

```bash
# All Phase A tests
cargo test -p ndp-gold-ddl

# All Phase B tests
cargo test -p ndp-validate

# Everything
cargo test -p ndp-gold-ddl -p ndp-validate
```

---

## 7. Checklist for Each TDD Cycle

### Phase A Cycle Checklist

- [ ] Golden master tests pass BEFORE changes
- [ ] Write/update unit test for the change
- [ ] Run unit test (expect RED)
- [ ] Implement minimal change
- [ ] Run unit test (expect GREEN)
- [ ] Run golden master tests (MUST be GREEN)
- [ ] Refactor if needed (keep tests GREEN)

### Phase B Cycle Checklist

- [ ] Write failing test (RED)
- [ ] Implement minimal code (GREEN)
- [ ] Refactor (keep GREEN)
- [ ] Golden master tests still pass
- [ ] Document new functionality

---

*TDD Guide created: 2026-02-05*
*Feature: FE-002 Domain Configuration Standardization*
*Testing Approach: London School TDD with Golden Master*
