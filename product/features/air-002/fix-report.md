# AIR-002: Configuration Adherence Bug Fix Report

## Problem Summary

The air-quality-app was NOT reading configuration from etcd properly. The app was using hardcoded defaults instead of loading values from etcd.

**Observed behavior:**
- App log: `Initializing ParquetStore at: ./data/parquet` (using hardcoded default)
- etcd configuration: `/air-quality/storage/base_path` = `/var/data/air-quality/parquet`
- The app completely ignored etcd configuration

## Root Cause Analysis

### Primary Issue
In `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs` (lines 24-34 before fix), the application used:
1. `AppConfig::from_yaml("config.yaml")` - loads from YAML file
2. `AppConfig::default_config()` - uses hardcoded defaults with env var overrides

**Neither method attempted to load configuration from etcd**, even though the etcd loading code existed in `config_etcd.rs`.

### Configuration Loading Flow (BEFORE FIX)
```
1. Try config.yaml
2. If fails → Use default_config() with env overrides
3. ParquetStore initialized with config.storage.base_path
   └─> Default: "./data/parquet"
```

### What Should Happen (Architecture Requirement)
**Priority:** etcd config > DATA_DIR env var > hardcoded default

## Solution Implemented

### Changes Made

#### 1. Updated `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`

**Changed:** Configuration loading logic to try etcd first
**Lines:** 24-65

**New flow:**
```rust
// Load configuration with priority: etcd > env vars > config.yaml > defaults
let config = match air_quality_app::load_from_etcd().await {
    Ok(etcd_config) => {
        tracing::info!("Loaded configuration from etcd");
        // Convert EtcdAppConfig to AppConfig
        AppConfig { ... }
    }
    Err(e) => {
        tracing::warn!("Failed to load config from etcd: {}. Trying config.yaml...", e);
        match AppConfig::from_yaml("config.yaml") {
            Ok(cfg) => {
                tracing::info!("Loaded configuration from config.yaml");
                cfg
            }
            Err(e) => {
                tracing::warn!("Failed to load config.yaml: {}, using defaults with env overrides", e);
                AppConfig::default_config()
            }
        }
    }
};
```

#### 2. Updated `/workspaces/neural-data-platform/apps/air-quality-app/src/config_etcd.rs`

**Changed:** Storage base_path loading to implement proper fallback chain
**Lines:** 57-85

**New logic:**
```rust
base_path: {
    // Priority: etcd > DATA_DIR env var > STORAGE_PATH env var > default
    match client.get::<String>("/storage/base_path").await {
        Ok(path) => {
            info!("Using storage base_path from etcd: {}", path);
            path
        }
        Err(_) => {
            if let Ok(data_dir) = std::env::var("DATA_DIR") {
                info!("Using storage base_path from DATA_DIR env var: {}", data_dir);
                data_dir
            } else if let Ok(storage_path) = std::env::var("STORAGE_PATH") {
                info!("Using storage base_path from STORAGE_PATH env var: {}", storage_path);
                storage_path
            } else {
                warn!("No storage base_path in etcd or env vars, using default: ./data/parquet");
                "./data/parquet".to_string()
            }
        }
    }
}
```

## Configuration Priority Chain (AFTER FIX)

### Complete Fallback Sequence
1. **etcd** `/air-quality/storage/base_path`
2. **Environment Variable** `DATA_DIR`
3. **Environment Variable** `STORAGE_PATH` (legacy support)
4. **config.yaml** `storage.base_path` (if etcd unavailable)
5. **Hardcoded Default** `./data/parquet`

### Expected Behavior by Scenario

| Scenario | etcd | DATA_DIR | STORAGE_PATH | config.yaml | Result |
|----------|------|----------|--------------|-------------|--------|
| Production (normal) | `/var/data/air-quality/parquet` | - | - | - | `/var/data/air-quality/parquet` |
| etcd down + env override | unavailable | `/custom/path` | - | - | `/custom/path` |
| etcd down + yaml | unavailable | - | - | `/yaml/path` | `/yaml/path` |
| All unavailable | unavailable | - | - | - | `./data/parquet` |

## Testing Validation Required

### 1. etcd Configuration Test
```bash
# Verify etcd has the config
etcdctl get /air-quality/storage/base_path

# Start app and check logs
# Expected: "Using storage base_path from etcd: /var/data/air-quality/parquet"
# Expected: "Initializing ParquetStore at: /var/data/air-quality/parquet"
```

### 2. DATA_DIR Fallback Test
```bash
# Temporarily disable etcd or remove the key
etcdctl del /air-quality/storage/base_path

# Set DATA_DIR
export DATA_DIR=/test/data/path

# Start app and check logs
# Expected: "Using storage base_path from DATA_DIR env var: /test/data/path"
# Expected: "Initializing ParquetStore at: /test/data/path"
```

### 3. Default Fallback Test
```bash
# Disable etcd and clear env vars
unset DATA_DIR
unset STORAGE_PATH
# Remove or rename config.yaml

# Start app and check logs
# Expected: "No storage base_path in etcd or env vars, using default: ./data/parquet"
# Expected: "Initializing ParquetStore at: ./data/parquet"
```

## Files Modified

1. `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`
   - Added etcd configuration loading as primary method
   - Implemented fallback chain: etcd → config.yaml → defaults
   - Added conversion from EtcdAppConfig to AppConfig

2. `/workspaces/neural-data-platform/apps/air-quality-app/src/config_etcd.rs`
   - Enhanced storage.base_path loading with explicit priority chain
   - Added support for DATA_DIR environment variable
   - Maintained backward compatibility with STORAGE_PATH
   - Added informative logging for each configuration source

## Compilation Status

✅ **PASSED** - `cargo check -p air-quality-app` completed successfully with no errors

Only warnings present are unrelated dead code warnings in other modules.

## Next Steps for Tester

1. **DO NOT COMMIT** - Wait for test validation
2. Test all three scenarios described above
3. Verify logging output shows correct config source
4. Verify ParquetStore is initialized with etcd path
5. Test app functionality with etcd configuration
6. Validate fallback behavior when etcd is unavailable

## Architecture Compliance

✅ **Config Pattern Implemented:** etcd config > DATA_DIR env var > hardcoded default
✅ **Code exists and is now being called:** `load_from_etcd()` function
✅ **Proper error handling:** Graceful fallback on etcd failure
✅ **Logging:** Clear visibility into which config source is used

## Impact Assessment

**Risk Level:** LOW
- Changes are additive (new config loading path)
- Existing fallback mechanisms preserved
- Backward compatible with STORAGE_PATH env var
- No changes to storage logic or data processing

**Benefits:**
- App now respects etcd configuration as designed
- Centralized configuration management works correctly
- Clear priority chain matches architecture requirements
- Better observability with config source logging
