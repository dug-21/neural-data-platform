# GAP-001: Implementation Details and Code Changes

**Reference**: `product/features/dp-016/analysis/GAP-001-YAML-to-JSON-scope-analysis.md`

---

## Overview

This document provides exact code changes needed to migrate domain configs from YAML to JSON format.

---

## Step 1: Create domain.json File

**File**: `/workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.json`

Convert the existing `/workspaces/neural-data-platform/config/domains/indoor-air-quality/domain.yaml` to JSON format.

**Current YAML Content** (107 lines):
```yaml
# Domain: Indoor Air Quality
# Cross-stream alignment for correlation analysis
#
# This domain combines indoor air quality sensor data with outdoor weather
# and home automation state for pattern detection.
#
# Implements SPEC-C01 (v11-005) and ADR-FE001-004 (NULL handling by stream type)
# - observation streams: NULL preserved
# - state_event streams: NULL filled via LOCF (carry_forward)
#
# NOTE: Flat format (no `domain:` wrapper) per DomainConfig struct requirements

id: indoor-air-quality
description: "Maintain healthy indoor air quality"

streams:
  - stream_id: air-quality
    alias: indoor
    role: primary
    # NULL handling: preserve (observation stream - default)

  - stream_id: outdoor-weather
    alias: outdoor
    role: context
    # NULL handling: preserve (observation stream - default)

  - stream_id: home-assistant-state
    alias: state
    role: actuator
    null_handling: carry_forward  # State persists until changed (state_event stream)

  # Phase D Fast-Follower: Added outdoor-air-quality as 4th stream
  - stream_id: outdoor-air-quality
    alias: outdoor_aqi
    role: constraint
    # NULL handling: preserve (observation stream - default per ADR-FE001-004)

alignment:
  view_name: indoor_air_quality_aligned
  granularity: "1 hour"
  join_strategy: full_outer
  # null_handling: by_stream_type - resolved from stream configs per ADR-FE001-004

objectives:
  - id: healthy_co2
    description: "Keep CO2 below 800 ppm for cognitive performance"
    target:
      stream: air-quality
      metric: co2
      condition: "<"
      threshold: 800
      unit: ppm
    priority: high

  - id: healthy_pm25
    description: "Keep PM2.5 below WHO guideline of 12 ug/m3"
    target:
      stream: air-quality
      metric: pm25
      condition: "<"
      threshold: 12
      unit: ug/m3
    priority: high

  # NOTE: "between" conditions with array thresholds not yet supported by TargetConfig struct
  # These objectives simplified to single threshold for Phase D validation
  # TODO: Extend TargetConfig to support threshold ranges (V1.2)
  - id: comfortable_humidity_min
    description: "Maintain minimum comfortable humidity (40%)"
    target:
      stream: air-quality
      metric: humidity_pct
      condition: ">="
      threshold: 40
      unit: percent
    priority: medium

  - id: comfortable_humidity_max
    description: "Maintain maximum comfortable humidity (60%)"
    target:
      stream: air-quality
      metric: humidity_pct
      condition: "<="
      threshold: 60
      unit: percent
    priority: medium

  - id: comfortable_temperature_min
    description: "Maintain minimum comfortable temperature (20C)"
    target:
      stream: air-quality
      metric: temperature_c
      condition: ">="
      threshold: 20
      unit: celsius
    priority: medium

  - id: comfortable_temperature_max
    description: "Maintain maximum comfortable temperature (24C)"
    target:
      stream: air-quality
      metric: temperature_c
      condition: "<="
      threshold: 24
      unit: celsius
    priority: medium
```

**Target JSON Format** (preserving all data):
```json
{
  "id": "indoor-air-quality",
  "description": "Maintain healthy indoor air quality. Implements SPEC-C01 (v11-005) and ADR-FE001-004 (NULL handling by stream type). Combines indoor air quality sensor data with outdoor weather and home automation state for pattern detection.",
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
      "description": "Keep CO2 below 800 ppm for cognitive performance",
      "target": {
        "stream": "air-quality",
        "metric": "co2",
        "condition": "<",
        "threshold": 800,
        "unit": "ppm"
      },
      "priority": "high"
    },
    {
      "id": "healthy_pm25",
      "description": "Keep PM2.5 below WHO guideline of 12 ug/m3",
      "target": {
        "stream": "air-quality",
        "metric": "pm25",
        "condition": "<",
        "threshold": 12,
        "unit": "ug/m3"
      },
      "priority": "high"
    },
    {
      "id": "comfortable_humidity_min",
      "description": "Maintain minimum comfortable humidity (40%)",
      "target": {
        "stream": "air-quality",
        "metric": "humidity_pct",
        "condition": ">=",
        "threshold": 40,
        "unit": "percent"
      },
      "priority": "medium"
    },
    {
      "id": "comfortable_humidity_max",
      "description": "Maintain maximum comfortable humidity (60%)",
      "target": {
        "stream": "air-quality",
        "metric": "humidity_pct",
        "condition": "<=",
        "threshold": 60,
        "unit": "percent"
      },
      "priority": "medium"
    },
    {
      "id": "comfortable_temperature_min",
      "description": "Maintain minimum comfortable temperature (20C)",
      "target": {
        "stream": "air-quality",
        "metric": "temperature_c",
        "condition": ">=",
        "threshold": 20,
        "unit": "celsius"
      },
      "priority": "medium"
    },
    {
      "id": "comfortable_temperature_max",
      "description": "Maintain maximum comfortable temperature (24C)",
      "target": {
        "stream": "air-quality",
        "metric": "temperature_c",
        "condition": "<=",
        "threshold": 24,
        "unit": "celsius"
      },
      "priority": "medium"
    }
  ]
}
```

**Validation**:
```bash
# Test JSON syntax
jq . config/domains/indoor-air-quality/domain.json

# Test schema compliance
jq --slurpfile schema config/schemas/domain.schema.json \
  '. as $data | $schema[0].definitions.domain_content as $def |
   if ($data | keys) == ($def.required) then
     "Valid"
   else
     "Invalid: missing required fields"
   end' \
  config/domains/indoor-air-quality/domain.json
```

---

## Step 2: Update Loader Code

**File**: `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/loader.rs`

### Change 1: Update domain_config_path() method

**Location**: Line 42-47

**Before**:
```rust
/// Get the path to a domain's config file
fn domain_config_path(&self, domain_id: &str) -> PathBuf {
    self.config_dir
        .join("domains")
        .join(domain_id)
        .join("domain.yaml")
}
```

**After**:
```rust
/// Get the path to a domain's config file
fn domain_config_path(&self, domain_id: &str) -> PathBuf {
    self.config_dir
        .join("domains")
        .join(domain_id)
        .join("domain.json")
}
```

### Change 2: Update load_domain_config() method

**Location**: Line 69-85

**Before**:
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
            message: format!("Failed to parse {}: {}", path.display(), e),
        })?;

    Ok(config)
}
```

**After**:
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
            message: format!("Failed to parse {}: {}", path.display(), e),
        })?;

    Ok(config)
}
```

**Summary**: Only 2 characters changed: `serde_yaml` → `serde_json`

---

## Step 3: Update Test Cases

**File**: `/workspaces/neural-data-platform/tools/ndp-gold-ddl/src/config/domain.rs`

### Test 1: test_domain_config_deserialize

**Location**: Line 310-338

**Before**:
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
    assert_eq!(config.id, "indoor-air-quality");
    assert_eq!(config.streams.len(), 3);
    assert_eq!(config.streams[0].alias, "indoor");
    assert_eq!(config.streams[0].role, StreamRole::Primary);
    assert_eq!(config.alignment.view_name, "indoor_air_quality_aligned");
    assert_eq!(config.alignment.join_strategy, JoinStrategy::FullOuter);
}
```

**After**:
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
    assert_eq!(config.id, "indoor-air-quality");
    assert_eq!(config.streams.len(), 3);
    assert_eq!(config.streams[0].alias, "indoor");
    assert_eq!(config.streams[0].role, StreamRole::Primary);
    assert_eq!(config.alignment.view_name, "indoor_air_quality_aligned");
    assert_eq!(config.alignment.join_strategy, JoinStrategy::FullOuter);
}
```

### Test 2: test_stream_ref_with_null_handling_override

**Location**: Line 340-351

**Before**:
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

**After**:
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

### Test 3: test_objective_config_deserialize

**Location**: Line 353-371

**Before**:
```rust
#[test]
fn test_objective_config_deserialize() {
    let yaml = r#"
id: healthy_co2
description: Keep CO2 below healthy threshold
target:
  stream: air-quality
  metric: co2
  condition: "<"
  threshold: 800
  unit: ppm
priority: high
"#;

    let objective: ObjectiveConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(objective.id, "healthy_co2");
    assert_eq!(objective.target.threshold, 800.0);
    assert_eq!(objective.priority, Priority::High);
}
```

**After**:
```rust
#[test]
fn test_objective_config_deserialize() {
    let json = r#"{
  "id": "healthy_co2",
  "description": "Keep CO2 below healthy threshold",
  "target": {
    "stream": "air-quality",
    "metric": "co2",
    "condition": "<",
    "threshold": 800,
    "unit": "ppm"
  },
  "priority": "high"
}"#;

    let objective: ObjectiveConfig = serde_json::from_str(json).unwrap();
    assert_eq!(objective.id, "healthy_co2");
    assert_eq!(objective.target.threshold, 800.0);
    assert_eq!(objective.priority, Priority::High);
}
```

**Changes Summary**:
- Replace `let yaml = r#"...YAML...`"#;` with `let json = r#"...JSON...`"#;`
- Replace `serde_yaml::from_str(yaml)` with `serde_json::from_str(json)`

---

## Step 4: Update Dependencies (Optional Cleanup)

**File**: `/workspaces/neural-data-platform/tools/ndp-gold-ddl/Cargo.toml`

### Remove serde_yaml dependency

**Location**: Line 19

**Before**:
```toml
[dependencies]
# CLI interface
clap = { version = "4", features = ["derive", "env"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"  # <-- REMOVE THIS LINE

# Error handling
thiserror = "1.0"
```

**After**:
```toml
[dependencies]
# CLI interface
clap = { version = "4", features = ["derive", "env"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
thiserror = "1.0"
```

**Verification**:
```bash
# Ensure serde_yaml not used elsewhere in ndp-gold-ddl
grep -r "serde_yaml" tools/ndp-gold-ddl/src
# Should return nothing if no other uses

# Rebuild to verify no breakage
cargo build -p ndp-gold-ddl
```

---

## Testing Checklist

### Unit Tests
```bash
# Run domain config tests
cargo test -p ndp-gold-ddl config::domain::tests::test_domain_config_deserialize
cargo test -p ndp-gold-ddl config::domain::tests::test_stream_ref_with_null_handling_override
cargo test -p ndp-gold-ddl config::domain::tests::test_objective_config_deserialize

# Run all ndp-gold-ddl tests
cargo test -p ndp-gold-ddl
```

### Integration Tests
```bash
# Run Phase C tests (use fixtures, not file loads)
cargo test -p ndp-gold-ddl aligned_view
```

### Manual Validation
```bash
# Validate JSON syntax
jq . config/domains/indoor-air-quality/domain.json

# Test loader can read domain.json
RUST_LOG=debug cargo run -p ndp-gold-ddl -- validate --domain indoor-air-quality --config-dir ./config

# Generate DDL from domain (smoke test)
cargo run -p ndp-gold-ddl -- generate --domain indoor-air-quality --config-dir ./config
```

### Regression Testing
```bash
# Verify stream configs still work (no regression)
cargo test -p ndp-gold-ddl config::
cargo test -p ndp-gold-ddl loader::

# Verify generators work with new format
cargo test -p ndp-gold-ddl generators::
```

---

## Commit Message

```
fix(ndp-gold-ddl): migrate domain config from YAML to JSON (ADR-016-001)

Domain configuration files now use JSON instead of YAML to comply with
ADR-016-001 (Configuration Source of Truth) which mandates JSON as the
platform-wide configuration format.

Changes:
- Convert config/domains/indoor-air-quality/domain.yaml to domain.json
- Update FileSystemConfigLoader to read domain.json (serde_json)
- Update domain config tests to use JSON instead of YAML
- Remove unused serde_yaml dependency from Cargo.toml

This change improves consistency with stream configs (which already use
JSON) and aligns with platform standards for agent reliability and
MCP integration.

Fixes: #11
Related: ADR-016-001, dp-016

No behavioral changes. All tests pass.
```

---

## Rollback Plan

If issues arise:

```bash
# Revert to YAML temporarily
git checkout config/domains/indoor-air-quality/domain.yaml

# Restore loader changes
git checkout tools/ndp-gold-ddl/src/config/loader.rs

# Restore test changes
git checkout tools/ndp-gold-ddl/src/config/domain.rs

# Restore Cargo.toml
git checkout tools/ndp-gold-ddl/Cargo.toml

# Rebuild
cargo build -p ndp-gold-ddl
```

---

## Success Indicators

After implementation, verify:

✓ `jq . config/domains/indoor-air-quality/domain.json` returns valid JSON
✓ `cargo test -p ndp-gold-ddl` all tests pass
✓ `grep serde_yaml tools/ndp-gold-ddl/Cargo.toml` returns nothing
✓ `cargo build -p ndp-gold-ddl` completes with no warnings
✓ `ndp-gold-ddl validate --domain indoor-air-quality` works
✓ `ndp-gold-ddl generate --domain indoor-air-quality` generates DDL
✓ Stream configs still work (no regression)
✓ Phase C aligned view tests pass

