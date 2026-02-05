# Implementation Guide: GAP-001 + GAP-003 (Domain Configuration V1.2)

**Audience:** Rust Developer
**Time Budget:** 5 hours total (2h + 3h)
**Complexity:** Medium
**Risk:** Low-Medium

---

## Phase 1: Domain Config Format Migration (GAP-001)

**Budget:** 2 hours
**Objective:** Migrate domain configs from YAML to JSON format

### 1.1: Migrate domain.yaml to domain.json

**File:** `config/domains/indoor-air-quality/domain.yaml`

**Current Content:**
```yaml
id: indoor-air-quality
description: "Maintain healthy indoor air quality"
streams:
  - stream_id: air-quality
    alias: indoor
    role: primary
  - stream_id: outdoor-weather
    alias: outdoor
    role: context
  - stream_id: home-assistant-state
    alias: state
    role: actuator
    null_handling: carry_forward
  - stream_id: outdoor-air-quality
    alias: outdoor_aqi
    role: constraint
alignment:
  view_name: indoor_air_quality_aligned
  granularity: "1 hour"
  join_strategy: full_outer
objectives:
  - id: healthy_co2
    description: "Keep CO2 below 800 ppm"
    target:
      stream: air-quality
      metric: co2
      condition: "<"
      threshold: 800
      unit: ppm
    priority: high
  # ... more objectives ...
```

**Action:**
1. Create new file: `config/domains/indoor-air-quality/domain.json`
2. Convert YAML to JSON format (maintain structure)
3. Delete old file: `config/domains/indoor-air-quality/domain.yaml`

**Result:** `config/domains/indoor-air-quality/domain.json`
```json
{
  "id": "indoor-air-quality",
  "description": "Maintain healthy indoor air quality",
  "streams": [
    {
      "stream_id": "air-quality",
      "alias": "indoor",
      "role": "primary"
    },
    {
      "stream_id": "outdoor-weather",
      "alias": "outdoor",
      "role": "context"
    },
    {
      "stream_id": "home-assistant-state",
      "alias": "state",
      "role": "actuator",
      "null_handling": "carry_forward"
    },
    {
      "stream_id": "outdoor-air-quality",
      "alias": "outdoor_aqi",
      "role": "constraint"
    }
  ],
  "alignment": {
    "view_name": "indoor_air_quality_aligned",
    "granularity": "1 hour",
    "join_strategy": "full_outer"
  },
  "objectives": [
    {
      "id": "healthy_co2",
      "description": "Keep CO2 below 800 ppm",
      "target": {
        "stream": "air-quality",
        "metric": "co2",
        "condition": "<",
        "threshold": 800,
        "unit": "ppm"
      },
      "priority": "high"
    }
  ]
}
```

**Time:** 15 minutes
**Validation:** JSON is valid per schema (use online validator or jq)

---

### 1.2: Update loader.rs - Path Change

**File:** `tools/ndp-gold-ddl/src/config/loader.rs`

**Current Code (Line 42-47):**
```rust
/// Get the path to a domain's config file
fn domain_config_path(&self, domain_id: &str) -> PathBuf {
    self.config_dir
        .join("domains")
        .join(domain_id)
        .join("domain.yaml")  // ← CHANGE THIS
}
```

**Updated Code:**
```rust
/// Get the path to a domain's config file
fn domain_config_path(&self, domain_id: &str) -> PathBuf {
    self.config_dir
        .join("domains")
        .join(domain_id)
        .join("domain.json")  // ✓ Changed to JSON
}
```

**Time:** 1 minute

---

### 1.3: Update loader.rs - Parser Change

**File:** `tools/ndp-gold-ddl/src/config/loader.rs`

**Current Code (Line 69-85):**
```rust
fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig> {
    let path = self.domain_config_path(domain_id);

    if !path.exists() {
        return Err(GoldDdlError::ConfigNotFound {
            path: path.display().to_string(),
        });
    }

    let content = std::fs::read_to_string(&path)?;
    let config: DomainConfig =
        serde_yaml::from_str(&content).map_err(|e| GoldDdlError::ConfigParseError {
            // ↑ CHANGE THIS
            message: format!("Failed to parse {}: {}", path.display(), e),
        })?;

    Ok(config)
}
```

**Updated Code:**
```rust
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
            // ✓ Changed to JSON parser
            message: format!("Failed to parse {}: {}", path.display(), e),
        })?;

    Ok(config)
}
```

**Time:** 2 minutes

---

### 1.4: Update domain.rs Tests - Convert YAML to JSON

**File:** `tools/ndp-gold-ddl/src/config/domain.rs`

**Current Test (Line 310-338):**
```rust
#[test]
fn test_domain_config_deserialize() {
    let yaml = r#"
id: indoor-air-quality
description: Indoor air quality monitoring domain
streams:
  - stream_id: air-quality
    alias: indoor
    role: primary
  - stream_id: outdoor-weather
    alias: outdoor
    role: context
  - stream_id: home-assistant-state
    alias: state
    role: actuator
alignment:
  view_name: indoor_air_quality_aligned
  granularity: "1 hour"
  join_strategy: full_outer
  null_handling: preserve
"#;

    let config: DomainConfig = serde_yaml::from_str(yaml).unwrap();
    // ... assertions ...
}
```

**Updated Test:**
```rust
#[test]
fn test_domain_config_deserialize() {
    let json = r#"{
  "id": "indoor-air-quality",
  "description": "Indoor air quality monitoring domain",
  "streams": [
    {
      "stream_id": "air-quality",
      "alias": "indoor",
      "role": "primary"
    },
    {
      "stream_id": "outdoor-weather",
      "alias": "outdoor",
      "role": "context"
    },
    {
      "stream_id": "home-assistant-state",
      "alias": "state",
      "role": "actuator"
    }
  ],
  "alignment": {
    "view_name": "indoor_air_quality_aligned",
    "granularity": "1 hour",
    "join_strategy": "full_outer",
    "null_handling": "preserve"
  }
}"#;

    let config: DomainConfig = serde_json::from_str(json).unwrap();
    // ... assertions ... (no changes needed)
}
```

**Current Test (Line 340-351):**
```rust
#[test]
fn test_stream_ref_with_null_handling_override() {
    let yaml = r#"
stream_id: home-assistant-state
alias: state
role: actuator
null_handling: carry_forward
"#;

    let stream_ref: StreamRef = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(stream_ref.null_handling, Some(NullHandling::CarryForward));
}
```

**Updated Test:**
```rust
#[test]
fn test_stream_ref_with_null_handling_override() {
    let json = r#"{
  "stream_id": "home-assistant-state",
  "alias": "state",
  "role": "actuator",
  "null_handling": "carry_forward"
}"#;

    let stream_ref: StreamRef = serde_json::from_str(json).unwrap();
    assert_eq!(stream_ref.null_handling, Some(NullHandling::CarryForward));
}
```

**Time:** 10 minutes

---

### 1.5: Test & Verify GAP-001

**Command 1: Run unit tests**
```bash
cd /workspaces/neural-data-platform/tools/ndp-gold-ddl
cargo test test_domain_config_deserialize
cargo test test_stream_ref_with_null_handling_override
cargo test test_load_domain_config_success
cargo test test_load_domain_config_not_found
cargo test test_load_domain_config_parse_error

# Expected: All pass (4 domain config tests)
```

**Command 2: Verify loader integration**
```bash
cargo test --lib config::loader

# Expected: All tests pass
```

**Command 3: Full test suite**
```bash
cargo test

# Expected: No new failures; if failures occur, investigate before Phase 2
```

**Command 4: Manual validation**
```bash
cargo run -p ndp-gold-ddl --quiet -- validate --domain indoor-air-quality

# Expected output: "Domain 'indoor-air-quality' configuration is valid"
```

**Time:** 25 minutes

---

## Checkpoint: GAP-001 Complete

**Decision Point:**
- ✅ All tests pass? → Proceed to Phase 2 (GAP-003)
- ❌ Tests failed? → Debug before proceeding
  - Check: Path to `domain.json` exists
  - Check: JSON is valid (use `jq` or online validator)
  - Check: serde_json version is compatible
  - Check: No breaking changes in DomainConfig struct

**Expected Outcome:**
```
✓ config/domains/indoor-air-quality/domain.json exists (JSON format)
✓ config/domains/indoor-air-quality/domain.yaml deleted
✓ tools/ndp-gold-ddl/src/config/loader.rs line 46 updated to "domain.json"
✓ tools/ndp-gold-ddl/src/config/loader.rs line 80 updated to serde_json
✓ tools/ndp-gold-ddl/src/config/domain.rs tests updated to JSON
✓ cargo test passes (no new failures)
✓ ndp-gold-ddl validate --domain indoor-air-quality succeeds
```

---

## Phase 2: JSON Schema Validation (GAP-003)

**Budget:** 3 hours
**Objective:** Add Layer 1 JSON Schema validation before deserialization

### 2.1: Create validator.rs Module

**File:** `tools/ndp-gold-ddl/src/config/validator.rs` (NEW)

**Implementation:**
```rust
//! JSON Schema validation for configurations
//!
//! Provides Layer 1 validation using JSON Schema before
//! Rust struct deserialization (Layer 2).

use crate::error::{GoldDdlError, Result};
use serde_json::json;

/// Validate domain configuration against JSON Schema
///
/// Performs validation before deserialization to provide
/// clear schema violation errors early in the pipeline.
///
/// # Arguments
/// * `content` - Raw JSON string to validate
///
/// # Returns
/// * `Ok(())` if valid
/// * `Err(GoldDdlError::ConfigParseError)` if schema validation fails
///
/// # Example
/// ```rust
/// use ndp_gold_ddl::config::validator::validate_domain_json;
///
/// let valid_json = r#"{
///   "id": "test-domain",
///   "streams": [{"stream_id": "test", "alias": "t", "role": "primary"}],
///   "alignment": {"view_name": "test_aligned", "granularity": "1 hour"}
/// }"#;
///
/// assert!(validate_domain_json(valid_json).is_ok());
/// ```
pub fn validate_domain_json(content: &str) -> Result<()> {
    // Parse as JSON first (fail fast if not valid JSON)
    let json: serde_json::Value =
        serde_json::from_str(content).map_err(|e| GoldDdlError::ConfigParseError {
            message: format!("Invalid JSON: {}", e),
        })?;

    // Load schema from embedded constant
    let schema = get_domain_schema();

    // Compile schema
    let schema_validator =
        jsonschema::JSONSchema::compile(&schema).map_err(|e| GoldDdlError::ConfigParseError {
            message: format!("Schema compilation failed: {}", e),
        })?;

    // Validate JSON against schema
    schema_validator.validate(&json).map_err(|e| {
        GoldDdlError::ConfigParseError {
            message: format!("Schema validation failed: {}", e),
        }
    })?;

    Ok(())
}

/// Get domain schema as JSON value
///
/// Returns the embedded domain.schema.json as a serde_json::Value.
/// In production, this could be lazy_loaded from disk or embedded as bytes.
fn get_domain_schema() -> serde_json::Value {
    // TODO: Load from config/schemas/domain.schema.json
    // For now, return minimal schema that enforces structure
    json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "required": ["id", "streams", "alignment"],
        "properties": {
            "id": {
                "type": "string",
                "pattern": "^[a-z][a-z0-9-]*$"
            },
            "description": { "type": "string" },
            "streams": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": ["stream_id", "role"],
                    "properties": {
                        "stream_id": { "type": "string", "pattern": "^[a-z][a-z0-9-]*$" },
                        "alias": { "type": "string", "pattern": "^[a-z][a-z0-9_]*$" },
                        "role": { "type": "string", "enum": ["primary", "context", "actuator", "constraint"] },
                        "null_handling": { "type": "string", "enum": ["preserve", "carry_forward", "interpolate"] }
                    }
                }
            },
            "alignment": {
                "type": "object",
                "required": ["view_name", "granularity"],
                "properties": {
                    "view_name": { "type": "string", "pattern": "^[a-z][a-z0-9_]*$" },
                    "granularity": { "type": "string", "pattern": "^\\d+\\s+(minute|hour|day)s?$" },
                    "join_strategy": { "type": "string", "enum": ["full_outer", "left", "inner"] },
                    "null_handling": { "type": "string", "enum": ["preserve", "carry_forward", "interpolate"] }
                }
            },
            "objectives": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "target"],
                    "properties": {
                        "id": { "type": "string", "pattern": "^[a-z][a-z0-9_]*$" },
                        "description": { "type": "string" },
                        "target": { "type": "object" },
                        "priority": { "type": "string", "enum": ["critical", "high", "medium", "low"] }
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_domain_json() {
        let valid_json = r#"{
  "id": "test-domain",
  "description": "Test domain",
  "streams": [
    {
      "stream_id": "test-stream",
      "alias": "test",
      "role": "primary"
    }
  ],
  "alignment": {
    "view_name": "test_aligned",
    "granularity": "1 hour"
  }
}"#;

        assert!(validate_domain_json(valid_json).is_ok());
    }

    #[test]
    fn test_validate_invalid_json() {
        let invalid_json = "{ this is not valid json }";
        assert!(validate_domain_json(invalid_json).is_err());
    }

    #[test]
    fn test_validate_missing_id() {
        let missing_id = r#"{
  "description": "Test",
  "streams": [{"stream_id": "test", "alias": "t", "role": "primary"}],
  "alignment": {"view_name": "test", "granularity": "1 hour"}
}"#;

        assert!(validate_domain_json(missing_id).is_err());
    }

    #[test]
    fn test_validate_bad_granularity_pattern() {
        let bad_granularity = r#"{
  "id": "test",
  "streams": [{"stream_id": "test", "alias": "t", "role": "primary"}],
  "alignment": {"view_name": "test", "granularity": "invalid"}
}"#;

        assert!(validate_domain_json(bad_granularity).is_err());
    }

    #[test]
    fn test_validate_invalid_stream_role() {
        let invalid_role = r#"{
  "id": "test",
  "streams": [{"stream_id": "test", "alias": "t", "role": "invalid_role"}],
  "alignment": {"view_name": "test", "granularity": "1 hour"}
}"#;

        assert!(validate_domain_json(invalid_role).is_err());
    }
}
```

**Time:** 45 minutes

---

### 2.2: Update Cargo.toml - Add jsonschema Dependency

**File:** `tools/ndp-gold-ddl/Cargo.toml`

**Current Dependencies:**
```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
tokio = { version = "1", features = ["full"] }
# ... other deps ...
```

**Update - Add jsonschema:**
```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
jsonschema = "0.17"  # ← NEW
tokio = { version = "1", features = ["full"] }
# ... other deps ...
```

**Time:** 2 minutes

---

### 2.3: Update loader.rs - Integrate Validator

**File:** `tools/ndp-gold-ddl/src/config/loader.rs`

**Add Module Declaration (Line 1-8):**
```rust
//! Configuration loading for Gold DDL generation
//!
//! Loads stream configurations from the file system.

use crate::config::domain::DomainConfig;
use crate::config::types::StreamConfig;
use crate::config::validator;  // ← ADD THIS
use crate::error::{GoldDdlError, Result};
use std::path::{Path, PathBuf};
```

**Update load_domain_config() (Line 69-85):**
```rust
fn load_domain_config(&self, domain_id: &str) -> Result<DomainConfig> {
    let path = self.domain_config_path(domain_id);

    if !path.exists() {
        return Err(GoldDdlError::ConfigNotFound {
            path: path.display().to_string(),
        });
    }

    let content = std::fs::read_to_string(&path)?;

    // ← ADD: Layer 1 validation (JSON Schema)
    validator::validate_domain_json(&content)?;

    // Layer 2 validation (Rust deserialization)
    let config: DomainConfig =
        serde_json::from_str(&content).map_err(|e| GoldDdlError::ConfigParseError {
            message: format!("Failed to parse {}: {}", path.display(), e),
        })?;

    Ok(config)
}
```

**Time:** 10 minutes

---

### 2.4: Create Test Fixtures

**File:** `tools/ndp-gold-ddl/tests/fixtures/domains/valid_complete.json`
```json
{
  "id": "valid-test-domain",
  "description": "A valid complete domain configuration",
  "streams": [
    {
      "stream_id": "primary-stream",
      "alias": "primary",
      "role": "primary"
    },
    {
      "stream_id": "context-stream",
      "alias": "context",
      "role": "context",
      "null_handling": "preserve"
    },
    {
      "stream_id": "actuator-stream",
      "alias": "actuator",
      "role": "actuator",
      "null_handling": "carry_forward"
    }
  ],
  "alignment": {
    "view_name": "valid_test_aligned",
    "granularity": "1 hour",
    "join_strategy": "full_outer",
    "null_handling": "preserve"
  },
  "objectives": [
    {
      "id": "test_objective",
      "description": "Test objective",
      "target": {
        "stream": "primary-stream",
        "metric": "test_metric",
        "condition": "<",
        "threshold": 100,
        "unit": "units"
      },
      "priority": "high"
    }
  ]
}
```

**File:** `tools/ndp-gold-ddl/tests/fixtures/domains/invalid_missing_id.json`
```json
{
  "description": "Missing required 'id' field",
  "streams": [
    {
      "stream_id": "test",
      "alias": "test",
      "role": "primary"
    }
  ],
  "alignment": {
    "view_name": "test_aligned",
    "granularity": "1 hour"
  }
}
```

**File:** `tools/ndp-gold-ddl/tests/fixtures/domains/invalid_missing_streams.json`
```json
{
  "id": "invalid-test",
  "description": "Missing required 'streams' field",
  "alignment": {
    "view_name": "test_aligned",
    "granularity": "1 hour"
  }
}
```

**File:** `tools/ndp-gold-ddl/tests/fixtures/domains/invalid_bad_granularity.json`
```json
{
  "id": "invalid-granularity",
  "description": "Invalid granularity pattern",
  "streams": [
    {
      "stream_id": "test",
      "alias": "test",
      "role": "primary"
    }
  ],
  "alignment": {
    "view_name": "test_aligned",
    "granularity": "invalid_granularity"
  }
}
```

**File:** `tools/ndp-gold-ddl/tests/fixtures/domains/invalid_bad_role.json`
```json
{
  "id": "invalid-role",
  "description": "Invalid stream role",
  "streams": [
    {
      "stream_id": "test",
      "alias": "test",
      "role": "invalid_role"
    }
  ],
  "alignment": {
    "view_name": "test_aligned",
    "granularity": "1 hour"
  }
}
```

**File:** `tools/ndp-gold-ddl/tests/fixtures/domains/invalid_bad_id_pattern.json`
```json
{
  "id": "Invalid-ID-With-Capitals",
  "description": "ID doesn't match kebab-case pattern",
  "streams": [
    {
      "stream_id": "test",
      "alias": "test",
      "role": "primary"
    }
  ],
  "alignment": {
    "view_name": "test_aligned",
    "granularity": "1 hour"
  }
}
```

**Time:** 30 minutes

---

### 2.5: Add Integration Tests

**File:** `tools/ndp-gold-ddl/tests/domain_validation_tests.rs` (NEW)

```rust
//! Integration tests for domain configuration validation
//!
//! Tests Layer 1 (JSON Schema) + Layer 2 (Rust deserialization)

use ndp_gold_ddl::config::loader::{FileSystemConfigLoader, ConfigLoader};
use ndp_gold_ddl::error::GoldDdlError;
use std::path::PathBuf;
use tempfile::TempDir;
use std::io::Write;

fn create_domain_config(dir: &std::path::Path, domain_id: &str, content: &str) {
    let domain_dir = dir.join("domains").join(domain_id);
    std::fs::create_dir_all(&domain_dir).unwrap();
    let config_path = domain_dir.join("domain.json");
    let mut file = std::fs::File::create(config_path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
}

#[test]
fn test_load_valid_domain() {
    let temp_dir = TempDir::new().unwrap();
    let valid_json = r#"{
  "id": "test-domain",
  "streams": [
    {"stream_id": "test-stream", "alias": "test", "role": "primary"}
  ],
  "alignment": {
    "view_name": "test_aligned",
    "granularity": "1 hour"
  }
}"#;

    create_domain_config(temp_dir.path(), "test-domain", valid_json);

    let loader = FileSystemConfigLoader::new(temp_dir.path());
    let config = loader.load_domain_config("test-domain");

    assert!(config.is_ok());
    assert_eq!(config.unwrap().id, "test-domain");
}

#[test]
fn test_load_invalid_json() {
    let temp_dir = TempDir::new().unwrap();
    let invalid_json = "{ this is not valid json }";

    create_domain_config(temp_dir.path(), "invalid", invalid_json);

    let loader = FileSystemConfigLoader::new(temp_dir.path());
    let config = loader.load_domain_config("invalid");

    assert!(config.is_err());
    match config.unwrap_err() {
        GoldDdlError::ConfigParseError { message } => {
            assert!(message.contains("Invalid JSON") || message.contains("JSON"));
        }
        _ => panic!("Expected ConfigParseError"),
    }
}

#[test]
fn test_load_domain_missing_id() {
    let temp_dir = TempDir::new().unwrap();
    let missing_id = r#"{
  "streams": [{"stream_id": "test", "alias": "test", "role": "primary"}],
  "alignment": {"view_name": "test", "granularity": "1 hour"}
}"#;

    create_domain_config(temp_dir.path(), "no-id", missing_id);

    let loader = FileSystemConfigLoader::new(temp_dir.path());
    let config = loader.load_domain_config("no-id");

    assert!(config.is_err());
    match config.unwrap_err() {
        GoldDdlError::ConfigParseError { message } => {
            assert!(message.contains("Schema validation") || message.contains("required"));
        }
        _ => panic!("Expected ConfigParseError"),
    }
}

#[test]
fn test_load_domain_bad_granularity() {
    let temp_dir = TempDir::new().unwrap();
    let bad_granularity = r#"{
  "id": "test",
  "streams": [{"stream_id": "test", "alias": "test", "role": "primary"}],
  "alignment": {"view_name": "test", "granularity": "invalid_granularity"}
}"#;

    create_domain_config(temp_dir.path(), "bad-gran", bad_granularity);

    let loader = FileSystemConfigLoader::new(temp_dir.path());
    let config = loader.load_domain_config("bad-gran");

    assert!(config.is_err());
    match config.unwrap_err() {
        GoldDdlError::ConfigParseError { message } => {
            assert!(message.contains("Schema validation") || message.contains("granularity"));
        }
        _ => panic!("Expected ConfigParseError"),
    }
}

#[test]
fn test_load_domain_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let loader = FileSystemConfigLoader::new(temp_dir.path());

    let config = loader.load_domain_config("nonexistent");

    assert!(config.is_err());
    match config.unwrap_err() {
        GoldDdlError::ConfigNotFound { path } => {
            assert!(path.contains("nonexistent"));
        }
        _ => panic!("Expected ConfigNotFound"),
    }
}
```

**Time:** 40 minutes

---

### 2.6: Test & Verify GAP-003

**Command 1: Build with new dependency**
```bash
cd /workspaces/neural-data-platform/tools/ndp-gold-ddl
cargo build

# Expected: Builds successfully (jsonschema compiles)
```

**Command 2: Run validator unit tests**
```bash
cargo test config::validator

# Expected: All 5 tests pass
#   ✓ test_validate_valid_domain_json
#   ✓ test_validate_invalid_json
#   ✓ test_validate_missing_id
#   ✓ test_validate_bad_granularity_pattern
#   ✓ test_validate_invalid_stream_role
```

**Command 3: Run integration tests**
```bash
cargo test domain_validation_tests

# Expected: All 6 tests pass
#   ✓ test_load_valid_domain
#   ✓ test_load_invalid_json
#   ✓ test_load_domain_missing_id
#   ✓ test_load_domain_bad_granularity
#   ✓ test_load_domain_file_not_found
```

**Command 4: Full test suite**
```bash
cargo test

# Expected: All tests pass (both old and new)
```

**Command 5: Verify with real config**
```bash
cargo run -p ndp-gold-ddl --quiet -- validate --domain indoor-air-quality

# Expected output: "Domain 'indoor-air-quality' configuration is valid"
```

**Command 6: Test error message quality**
```bash
# Create temp bad config for testing
mkdir -p /tmp/ndp-test/domains/bad-domain
echo '{"id": "bad"}' > /tmp/ndp-test/domains/bad-domain/domain.json

cargo run -p ndp-gold-ddl --quiet -- validate --domain bad-domain --config-dir /tmp/ndp-test

# Expected: Clear error message about missing 'streams' or 'alignment'
```

**Time:** 30 minutes

---

## Checkpoint: GAP-003 Complete

**Decision Point:**
- ✅ All tests pass? → Phase 2 complete, ready for documentation
- ❌ Tests failed? → Debug and fix before proceeding
  - Check: jsonschema crate version compatible
  - Check: Schema structure matches test expectations
  - Check: Validator module exports properly

**Expected Outcome:**
```
✓ tools/ndp-gold-ddl/src/config/validator.rs created (120 lines)
✓ tools/ndp-gold-ddl/Cargo.toml updated (jsonschema dependency)
✓ tools/ndp-gold-ddl/src/config/loader.rs updated (validator call)
✓ tools/ndp-gold-ddl/tests/domain_validation_tests.rs created (6 tests)
✓ test fixtures created (5 JSON files)
✓ cargo test passes (all domain validation tests green)
✓ ndp-gold-ddl validate --domain indoor-air-quality succeeds with schema validation
```

---

## Phase 3: Documentation Updates

**Budget:** 30 minutes

### 3.1: Update VALIDATION-PROCEDURE.md

**File:** `docs/procedures/VALIDATION-PROCEDURE.md`

**Add Section:** Domain Configuration Validation

```markdown
## Domain Configuration Validation

Domain configurations are validated in two layers:

### Layer 1: JSON Schema Validation
- Validates against `config/schemas/domain.schema.json`
- Runs automatically when loading domain configs
- Checks: required fields, enum values, pattern compliance

### Layer 2: Rust Struct Deserialization
- Rust type safety and additional semantic validation
- Occurs after Layer 1 passes
- Checks: type compatibility, constraint satisfaction

### Common Validation Errors

#### Missing required field
```
Error: Schema validation failed: domain 'my-domain' missing required field 'id'
Fix: Add "id" field to domain.json
```

#### Invalid granularity pattern
```
Error: Schema validation failed: granularity must match pattern "\\d+ (minute|hour|day)s?"
Fix: Use format like "1 hour", "24 hours", "30 minutes"
```

#### Invalid stream role
```
Error: Schema validation failed: role must be one of: primary, context, actuator, constraint
Fix: Check spelling and capitalization
```

### How to Add a New Domain

1. Create directory: `config/domains/my-domain/`
2. Create JSON file: `config/domains/my-domain/domain.json`
3. Use this template:
```json
{
  "id": "my-domain",
  "description": "Description of domain purpose",
  "streams": [
    {
      "stream_id": "primary-stream-id",
      "alias": "primary",
      "role": "primary"
    }
  ],
  "alignment": {
    "view_name": "my_domain_aligned",
    "granularity": "1 hour",
    "join_strategy": "full_outer"
  }
}
```

4. Validate: `ndp-gold-ddl validate --domain my-domain`
5. If valid, proceed to deployment

### Debugging Schema Validation Issues

To check if your domain.json is valid:

```bash
# Using jq (if available)
jq . config/domains/my-domain/domain.json

# Using ndp-gold-ddl
ndp-gold-ddl validate --domain my-domain --verbose
```

The validator will return specific errors indicating which schema constraint failed.
```

---

## Final Verification

### Full End-to-End Test

```bash
# Phase 1 + Phase 2 verification
cd /workspaces/neural-data-platform

# 1. Config file format
file config/domains/indoor-air-quality/domain.json
# Expected: JSON data

# 2. Rust tests
cd tools/ndp-gold-ddl
cargo test

# 3. Tool validation
cargo run -p ndp-gold-ddl --quiet -- validate --domain indoor-air-quality

# 4. Create invalid config to test validation
mkdir -p /tmp/test-invalid/domains/bad
echo '{"id": "bad"}' > /tmp/test-invalid/domains/bad/domain.json
cargo run -p ndp-gold-ddl --quiet -- validate --domain bad --config-dir /tmp/test-invalid 2>&1 | grep -i "error\|schema"
# Expected: Error message mentioning schema validation

# All verified!
echo "✓ GAP-001 + GAP-003 Implementation Complete"
```

---

## Timeline Summary

| Phase | Task | Time |
|-------|------|------|
| 1.1 | Migrate YAML→JSON | 15 min |
| 1.2 | Update loader path | 1 min |
| 1.3 | Update parser | 2 min |
| 1.4 | Update tests | 10 min |
| 1.5 | Test & verify | 25 min |
| **Phase 1 Total** | **GAP-001** | **~53 min** |
| 2.1 | Create validator.rs | 45 min |
| 2.2 | Update Cargo.toml | 2 min |
| 2.3 | Integrate validator | 10 min |
| 2.4 | Create fixtures | 30 min |
| 2.5 | Integration tests | 40 min |
| 2.6 | Test & verify | 30 min |
| **Phase 2 Total** | **GAP-003** | **~157 min** (~2.6 hrs) |
| 3.1 | Documentation | 30 min |
| **Total** | **GAP-001 + GAP-003** | **~4.5-5 hours** |

---

## Rollback Plan

If issues occur:

### If Phase 1 Breaks Compilation
```bash
# Revert to YAML format
git checkout config/domains/indoor-air-quality/domain.yaml
git checkout tools/ndp-gold-ddl/src/config/loader.rs
git checkout tools/ndp-gold-ddl/src/config/domain.rs
cargo test  # Should pass again
```

### If Phase 2 Breaks Tests
```bash
# Remove validator integration (keep Phase 1)
git checkout tools/ndp-gold-ddl/src/config/loader.rs  # Removes validator call only
git checkout tools/ndp-gold-ddl/src/config/validator.rs
git checkout tools/ndp-gold-ddl/Cargo.toml
cargo test  # Should pass (Phase 1 still works)
```

### Both Rollback
```bash
git reset --hard HEAD~N  # Where N = number of commits
```

---

## Success Criteria

✅ **Phase 1 Complete when:**
- [ ] domain.yaml deleted, domain.json exists
- [ ] loader.rs path and parser updated
- [ ] Tests updated to JSON format
- [ ] `cargo test` passes
- [ ] `ndp-gold-ddl validate --domain indoor-air-quality` succeeds

✅ **Phase 2 Complete when:**
- [ ] validator.rs created and tested
- [ ] jsonschema crate added
- [ ] Validator integrated into loader
- [ ] All 5 validator tests pass
- [ ] All 6 integration tests pass
- [ ] Invalid configs are properly rejected with clear errors

✅ **Documentation when:**
- [ ] VALIDATION-PROCEDURE.md updated
- [ ] Domain contribution guide added
- [ ] Examples provided

---

**Implementation Ready. Proceed when Phase 1 checkpoint reached.**
