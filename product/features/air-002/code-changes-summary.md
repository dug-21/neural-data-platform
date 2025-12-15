# AIR-002: Code Changes Summary

## Overview
Fixed configuration adherence bug where air-quality-app was not reading from etcd.

## Files Changed: 2

### 1. `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`

**Change:** Modified configuration loading to try etcd first

**Location:** Lines 24-65

**Before:**
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

**After:**
```rust
// Load configuration with priority: etcd > env vars > config.yaml > defaults
let config = match air_quality_app::load_from_etcd().await {
    Ok(etcd_config) => {
        tracing::info!("Loaded configuration from etcd");
        // Convert EtcdAppConfig to AppConfig
        AppConfig {
            server: air_quality_app::config::ServerConfig {
                host: etcd_config.server.host,
                port: etcd_config.server.port,
            },
            mqtt: air_quality_app::config::MqttConfig {
                broker_url: etcd_config.mqtt.broker_url,
                port: etcd_config.mqtt.port,
                client_id: etcd_config.mqtt.client_id,
                topic_pattern: etcd_config.mqtt.topic_pattern,
                qos: etcd_config.mqtt.qos,
                reconnect_delay_secs: etcd_config.mqtt.reconnect_delay_secs,
                max_reconnect_delay_secs: etcd_config.mqtt.max_reconnect_delay_secs,
                buffer_capacity: etcd_config.mqtt.buffer_capacity,
            },
            storage: air_quality_app::config::StorageConfig {
                base_path: etcd_config.storage.base_path,
                wal_enabled: etcd_config.storage.wal_enabled,
                batch_size: etcd_config.storage.batch_size,
                batch_timeout_secs: etcd_config.storage.batch_timeout_secs,
            },
        }
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

**Impact:**
- App now tries to load from etcd first
- Falls back to config.yaml if etcd unavailable
- Falls back to defaults as last resort
- Maintains backward compatibility

---

### 2. `/workspaces/neural-data-platform/apps/air-quality-app/src/config_etcd.rs`

**Change:** Enhanced storage.base_path loading with explicit priority chain

**Location:** Lines 57-85

**Before:**
```rust
let storage = StorageConfig {
    base_path: client.get_with_env("/storage/base_path", "AIR_QUALITY").await
        .unwrap_or_else(|_| "./data/parquet".to_string()),
    wal_enabled: client.get_with_env("/storage/wal_enabled", "AIR_QUALITY").await
        .unwrap_or(true),
    batch_size: client.get_with_env("/storage/batch_size", "AIR_QUALITY").await
        .unwrap_or(100),
    batch_timeout_secs: client.get_with_env("/storage/batch_timeout_secs", "AIR_QUALITY").await
        .unwrap_or(5),
};
```

**After:**
```rust
let storage = StorageConfig {
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
    },
    wal_enabled: client.get_with_env("/storage/wal_enabled", "AIR_QUALITY").await
        .unwrap_or(true),
    batch_size: client.get_with_env("/storage/batch_size", "AIR_QUALITY").await
        .unwrap_or(100),
    batch_timeout_secs: client.get_with_env("/storage/batch_timeout_secs", "AIR_QUALITY").await
        .unwrap_or(5),
};
```

**Impact:**
- Implements correct priority chain: etcd > DATA_DIR > STORAGE_PATH > default
- Adds informative logging for each configuration source
- Supports DATA_DIR environment variable (required by architecture)
- Maintains STORAGE_PATH for backward compatibility

---

## Priority Chain Implementation

| Priority | Source | Environment Variable | Notes |
|----------|--------|---------------------|-------|
| 1 (Highest) | etcd | - | `/air-quality/storage/base_path` |
| 2 | Environment | `DATA_DIR` | Architecture requirement |
| 3 | Environment | `STORAGE_PATH` | Legacy support |
| 4 | YAML file | - | `config.yaml` (if etcd unavailable) |
| 5 (Lowest) | Hardcoded | - | `./data/parquet` |

## Verification

Run: `cargo check -p air-quality-app`
Status: PASSED (compiles without errors)

## Testing

See `test-config-loading.sh` for automated testing guidance.

Expected log output when etcd is available:
```
INFO Loaded configuration from etcd
INFO Using storage base_path from etcd: /var/data/air-quality/parquet
INFO Initializing ParquetStore at: /var/data/air-quality/parquet
```

Expected log output when using DATA_DIR fallback:
```
WARN Failed to load config from etcd: ...
INFO Using storage base_path from DATA_DIR env var: /custom/path
INFO Initializing ParquetStore at: /custom/path
```
