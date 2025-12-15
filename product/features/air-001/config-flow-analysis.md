# Air Quality App Configuration Flow Analysis

**Analysis Date**: 2025-12-14
**Branch**: feature/air-001-implementation
**Analyst**: DevOps Agent

---

## Executive Summary

Analysis of the air-quality-app configuration system reveals **critical inconsistencies** between the intended config flow and actual implementation. The app supports etcd-based configuration but does NOT properly read storage paths from etcd. Environment variable fallback has gaps.

### Critical Issues Identified

1. **STORAGE PATH MISMATCH**: Base config uses `/data/parquet` but docker-compose mounts `/app/data`
2. **PRODUCTION OVERRIDE INCORRECT**: Production overlay uses `/var/data/air-quality/parquet` (wrong path)
3. **ENV VAR INCONSISTENCY**: App reads `STORAGE_PATH` env var but docker-compose sets `DATA_DIR`
4. **ETCD NOT ENFORCED**: App falls back to file config instead of using etcd from docker-compose

---

## Configuration Flow (Current State)

### 1. Configuration Priority (Intended)
```
etcd config > Environment variables > YAML config > Defaults
```

### 2. Configuration Priority (Actual)
```
YAML config + Environment variables (partial) > Defaults
```

**Issue**: The main.rs does NOT call `config_etcd::load_from_etcd()`, instead only uses file-based config.

---

## Detailed Configuration Analysis

### A. Docker Compose Configuration
**File**: `/workspaces/neural-data-platform/deploy/pi/docker-compose.yml`

```yaml
air-quality-app:
  volumes:
    - air-quality-data:/app/data  # ✅ CORRECT: Volume mounted at /app/data
  environment:
    - DATA_DIR=/app/data          # ❌ WRONG: App doesn't read DATA_DIR
    - ETCD_ENDPOINT=http://etcd:2379  # ✅ Set but not used in main.rs
```

**Status**: Volume mount is correct, but environment variable name is wrong.

---

### B. Base Configuration (YAML)
**File**: `/workspaces/neural-data-platform/config/base/air-quality/config.yaml`

```yaml
storage:
  base_path: "/data/parquet"  # ❌ WRONG: Should be /app/data for Docker
  wal_enabled: true
  batch_size: 100
  batch_timeout_secs: 5
```

**Issues**:
- Path `/data/parquet` doesn't match docker volume mount `/app/data`
- This config gets synced to etcd via `sync-config-to-etcd.sh`

---

### C. Production Override (YAML)
**File**: `/workspaces/neural-data-platform/config/overlays/production/air-quality/config.yaml`

```yaml
storage:
  base_path: "/var/data/air-quality/parquet"  # ❌ WRONG: Doesn't match /app/data
  batch_size: 500
  batch_timeout_secs: 10
```

**Issues**:
- Production path `/var/data/air-quality/parquet` is completely wrong
- Should be `/app/data` to match docker-compose volume mount

---

### D. Application Config Loader
**File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/config.rs`

```rust
fn apply_env_overrides(&mut self) {
    if let Ok(storage_path) = std::env::var("STORAGE_PATH") {  // ❌ Reads STORAGE_PATH
        self.storage.base_path = storage_path;
    }
}
```

**Issues**:
- Reads `STORAGE_PATH` but docker-compose sets `DATA_DIR`
- Inconsistent naming convention

---

### E. Etcd Config Loader
**File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/config_etcd.rs`

```rust
let storage = StorageConfig {
    base_path: client.get_with_env("/storage/base_path", "AIR_QUALITY").await
        .unwrap_or_else(|_| "./data/parquet".to_string()),  // ❌ Wrong default
    // ...
};
```

**Issues**:
- Default path `./data/parquet` is incorrect (should be `/app/data`)
- This code is implemented but NOT USED in main.rs

---

### F. Main Application Entry
**File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`

```rust
// Load configuration
let config = match AppConfig::from_yaml("config.yaml") {
    Ok(cfg) => {
        tracing::info!("Loaded configuration from config.yaml");
        cfg
    }
    Err(e) => {
        tracing::warn!("Failed to load config.yaml: {}, using defaults", e);
        AppConfig::default_config()
    }
};
```

**Critical Issue**:
- Does NOT attempt to load from etcd despite `config_etcd.rs` being available
- Only uses file-based config with env var overrides
- Ignores `ETCD_ENDPOINT` environment variable set in docker-compose

---

### G. Config Sync Script
**File**: `/workspaces/neural-data-platform/scripts/sync-config-to-etcd.sh`

```bash
sync_yaml_to_etcd() {
    local file=$1
    local service=$(basename $(dirname $file))
    local prefix="/$service"  # Creates /air-quality prefix

    # Flattens YAML and stores in etcd as:
    # /air-quality/storage/base_path = "/data/parquet"
}
```

**Status**: Script works correctly, but syncs incorrect paths to etcd.

---

## Configuration Values Comparison

| Config Source | Storage Base Path | Volume Mount | Environment Var | Match? |
|--------------|-------------------|--------------|-----------------|--------|
| docker-compose.yml | - | `/app/data` | `DATA_DIR=/app/data` | N/A |
| base/config.yaml | `/data/parquet` | - | - | ❌ NO |
| overlays/production/config.yaml | `/var/data/air-quality/parquet` | - | - | ❌ NO |
| config.rs (env override) | - | - | `STORAGE_PATH` | ❌ WRONG VAR |
| config_etcd.rs (default) | `./data/parquet` | - | - | ❌ NO |
| config.rs (default) | `./data/parquet` | - | - | ❌ NO |

**NONE OF THE CONFIGS MATCH THE DOCKER VOLUME MOUNT**

---

## Correct Configuration Flow (Should Be)

```
┌─────────────────┐
│  Docker Compose │
│  Starts App     │
└────────┬────────┘
         │
         ├─ Volume: /app/data mounted
         ├─ Env: ETCD_ENDPOINT=http://etcd:2379
         └─ Env: STORAGE_PATH=/app/data  (should be this, not DATA_DIR)
         │
         ▼
┌─────────────────┐
│   App Startup   │
│   (main.rs)     │
└────────┬────────┘
         │
         ├─ Try etcd first (config_etcd::load_from_etcd())
         │   └─ Priority: etcd value > env var > default
         │
         ├─ Fallback to YAML if etcd unavailable
         │   └─ Priority: yaml value + env var overrides
         │
         └─ Final fallback to defaults
             └─ Priority: env var > hardcoded default
         │
         ▼
┌─────────────────┐
│  Config Loaded  │
│  storage.base_  │
│  path=/app/data │
└─────────────────┘
```

---

## Fixes Required

### 1. Fix docker-compose.yml Environment Variable
**File**: `/workspaces/neural-data-platform/deploy/pi/docker-compose.yml`

```yaml
# CHANGE FROM:
- DATA_DIR=/app/data

# CHANGE TO:
- STORAGE_PATH=/app/data  # Matches config.rs env var name
```

### 2. Fix Base Config YAML
**File**: `/workspaces/neural-data-platform/config/base/air-quality/config.yaml`

```yaml
# CHANGE FROM:
storage:
  base_path: "/data/parquet"

# CHANGE TO:
storage:
  base_path: "/app/data"  # Match docker volume mount
```

### 3. Fix Production Overlay Config
**File**: `/workspaces/neural-data-platform/config/overlays/production/air-quality/config.yaml`

```yaml
# CHANGE FROM:
storage:
  base_path: "/var/data/air-quality/parquet"

# CHANGE TO:
storage:
  base_path: "/app/data"  # Match docker volume mount
```

### 4. Update main.rs to Use Etcd Config (Developer Task)
**File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`

**Note**: Developer is handling this. The code should:
1. First try to load from etcd using `config_etcd::load_from_etcd()`
2. Fall back to file-based config if etcd unavailable
3. Apply environment variable overrides in both cases

---

## Environment Variable Strategy

### Current vs. Correct

| Purpose | Current (Wrong) | Correct |
|---------|----------------|---------|
| Storage path override | `DATA_DIR` | `STORAGE_PATH` |
| MQTT broker | `MQTT_BROKER_URL` | ✅ Correct |
| MQTT port | `MQTT_PORT` | ✅ Correct |
| Etcd endpoint | `ETCD_ENDPOINT` | ✅ Correct (but not used) |

---

## Etcd Key Structure

When `sync-config-to-etcd.sh` runs, it creates these keys:

```
/air-quality/server/host = "0.0.0.0"
/air-quality/server/port = 8080
/air-quality/mqtt/broker_url = "mosquitto"
/air-quality/mqtt/port = 1883
/air-quality/storage/base_path = "/data/parquet"  # ❌ WRONG (from base config)
/air-quality/storage/wal_enabled = true
/air-quality/storage/batch_size = 100
```

After production overlay:
```
/air-quality/storage/base_path = "/var/data/air-quality/parquet"  # ❌ STILL WRONG
/air-quality/storage/batch_size = 500
```

**Both are wrong!** Should be `/app/data`.

---

## Testing the Config Flow

### 1. Verify etcd has correct values
```bash
docker exec etcd etcdctl get --prefix /air-quality/storage
```

### 2. Verify app reads from etcd
```bash
docker logs air-quality-app | grep -i "etcd\|config"
```

Expected log output:
```
INFO Connecting to etcd at http://etcd:2379
INFO Connected to etcd, loading configuration
INFO Loaded storage path: /app/data
```

### 3. Verify volume mount works
```bash
docker exec air-quality-app ls -la /app/data
```

Should show parquet files if data is being written.

---

## Summary of Inconsistencies

### Critical
1. ❌ Storage path mismatch between all configs and docker volume
2. ❌ Environment variable name mismatch (DATA_DIR vs STORAGE_PATH)
3. ❌ main.rs doesn't use etcd config loader

### Important
4. ❌ Production overlay has completely wrong storage path
5. ❌ Default paths in config loaders don't match docker environment

### Minor
6. ⚠️ No validation that storage path is writable on startup
7. ⚠️ No health check for config consistency

---

## Recommended Actions

**Immediate (DevOps)**:
1. ✅ Fix docker-compose.yml environment variable (DATA_DIR → STORAGE_PATH)
2. ✅ Fix base config.yaml storage path (/data/parquet → /app/data)
3. ✅ Fix production overlay storage path (/var/data/... → /app/data)
4. Re-sync configs to etcd after fixes

**Developer Task**:
5. Update main.rs to use etcd config loader
6. Ensure proper fallback chain works
7. Add config validation on startup

**Testing**:
8. Verify parquet files are written to /app/data
9. Verify data persists across container restarts
10. Verify config can be updated in etcd without redeployment

---

## Configuration Pattern Documentation

### Environment Variable Naming Convention
```
<SERVICE>_<CONFIG_SECTION>_<PROPERTY>

Examples:
- AIR_QUALITY_STORAGE_BASE_PATH (etcd client pattern)
- STORAGE_PATH (legacy env var pattern)
- MQTT_BROKER_URL (global pattern)
```

**Recommendation**: Standardize on one pattern. Current code uses mixed patterns.

### Priority Chain (Correct Implementation)
```rust
// 1. Try etcd with env override
let path = client.get_with_env("/storage/base_path", "AIR_QUALITY").await?;

// get_with_env internally does:
// - Check AIR_QUALITY_STORAGE_BASE_PATH env var first
// - Fall back to etcd value
// - Fall back to default

// 2. Legacy env var override
if let Ok(val) = std::env::var("STORAGE_PATH") {
    path = val;
}
```

---

## Appendix: File Locations

```
/workspaces/neural-data-platform/
├── deploy/pi/
│   └── docker-compose.yml                    # ❌ DATA_DIR → STORAGE_PATH
├── config/
│   ├── base/air-quality/
│   │   └── config.yaml                       # ❌ /data/parquet → /app/data
│   └── overlays/production/air-quality/
│       └── config.yaml                       # ❌ /var/data/... → /app/data
├── apps/air-quality-app/src/
│   ├── config.rs                             # ✅ Correct (reads STORAGE_PATH)
│   ├── config_etcd.rs                        # ⚠️ Implemented but not used
│   └── main.rs                               # ❌ Needs to use config_etcd
└── scripts/
    └── sync-config-to-etcd.sh                # ✅ Works correctly
```

---

## End of Analysis

**Next Steps**: Apply the three critical fixes to deployment configs, then wait for developer to integrate etcd config loading in main.rs.
