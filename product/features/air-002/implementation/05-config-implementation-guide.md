# AIR-002 T1: Configuration Implementation Guide

**Task:** AIR-002-T1 Configuration Management
**Estimated Time:** 1-2 hours
**Difficulty:** Easy
**Prerequisites:** None

---

## Quick Start Checklist

- [ ] Step 1: Update `config.rs` (30 minutes)
- [ ] Step 2: Create `config.yaml` (10 minutes)
- [ ] Step 3: Add tests (20 minutes)
- [ ] Step 4: Manual verification (10 minutes)
- [ ] Step 5: Documentation (10 minutes)

**Total:** ~80 minutes

---

## Step 1: Update config.rs (30 minutes)

### File Location
`/workspaces/neural-data-platform/apps/air-quality-app/src/config.rs`

### Current State (Reference)
The file currently has:
- `AppConfig` with server, mqtt, storage fields
- `from_yaml()` method
- `default_config()` method

### Changes Required

Replace the existing structs with these updated versions:

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use rumqttc::QoS;

/// Application configuration loaded from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub mqtt: MqttConfigYaml,
    pub storage: StorageConfigYaml,
}

/// HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

/// MQTT configuration (YAML-serializable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttConfigYaml {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub topic_pattern: String,
    pub qos: u8,  // 0, 1, or 2
    pub reconnect_delay_secs: u64,
    pub max_reconnect_delay_secs: u64,
    pub buffer_capacity: usize,
}

/// Storage configuration (YAML-serializable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfigYaml {
    pub base_path: String,
    #[serde(default = "default_wal_enabled")]
    pub wal_enabled: bool,
}

fn default_wal_enabled() -> bool {
    true
}

impl AppConfig {
    /// Load configuration from YAML file with environment variable overrides
    pub fn from_yaml<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let mut config: AppConfig = serde_yaml::from_str(&content)?;

        // Apply environment variable overrides
        config.apply_env_overrides();

        Ok(config)
    }

    /// Apply environment variable overrides
    fn apply_env_overrides(&mut self) {
        // MQTT overrides
        if let Ok(url) = std::env::var("MQTT_BROKER_URL") {
            self.mqtt.broker_url = url;
        }
        if let Ok(port) = std::env::var("MQTT_PORT") {
            if let Ok(port_num) = port.parse::<u16>() {
                self.mqtt.port = port_num;
            }
        }
        if let Ok(client_id) = std::env::var("MQTT_CLIENT_ID") {
            self.mqtt.client_id = client_id;
        }

        // Storage overrides
        if let Ok(path) = std::env::var("STORAGE_PATH") {
            self.storage.base_path = path;
        }

        // Server overrides
        if let Ok(host) = std::env::var("SERVER_HOST") {
            self.server.host = host;
        }
        if let Ok(port) = std::env::var("SERVER_PORT") {
            if let Ok(port_num) = port.parse::<u16>() {
                self.server.port = port_num;
            }
        }
    }

    /// Default configuration for development
    pub fn default_config() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
            },
            mqtt: MqttConfigYaml {
                broker_url: "localhost".to_string(),
                port: 1883,
                client_id: "air-quality-app".to_string(),
                topic_pattern: "airgradient/readings/+".to_string(),
                qos: 1,
                reconnect_delay_secs: 1,
                max_reconnect_delay_secs: 30,
                buffer_capacity: 1000,
            },
            storage: StorageConfigYaml {
                base_path: "/data/parquet".to_string(),
                wal_enabled: true,
            },
        }
    }
}

impl MqttConfigYaml {
    /// Convert to platform-core MqttConfig
    pub fn to_mqtt_config(&self) -> platform_core::sources::mqtt::MqttConfig {
        platform_core::sources::mqtt::MqttConfig {
            broker_url: self.broker_url.clone(),
            port: self.port,
            client_id: self.client_id.clone(),
            topic_pattern: self.topic_pattern.clone(),
            qos: match self.qos {
                0 => QoS::AtMostOnce,
                1 => QoS::AtLeastOnce,
                2 => QoS::ExactlyOnce,
                _ => QoS::AtLeastOnce,  // Default to 1
            },
            reconnect_delay: Duration::from_secs(self.reconnect_delay_secs),
            max_reconnect_delay: Duration::from_secs(self.max_reconnect_delay_secs),
            buffer_capacity: self.buffer_capacity,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default_config();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.mqtt.client_id, "air-quality-app");
        assert_eq!(config.mqtt.broker_url, "localhost");
        assert_eq!(config.storage.base_path, "/data/parquet");
        assert!(config.storage.wal_enabled);
    }

    #[test]
    fn test_from_yaml() {
        let yaml_content = r#"
server:
  host: "127.0.0.1"
  port: 9000
mqtt:
  broker_url: "mqtt.example.com"
  port: 8883
  client_id: "test-client"
  topic_pattern: "test/topic/+"
  qos: 2
  reconnect_delay_secs: 2
  max_reconnect_delay_secs: 60
  buffer_capacity: 500
storage:
  base_path: "/tmp/parquet"
  wal_enabled: false
"#;

        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_air_quality_config.yaml");
        let mut file = std::fs::File::create(&temp_file).unwrap();
        file.write_all(yaml_content.as_bytes()).unwrap();

        let config = AppConfig::from_yaml(&temp_file).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 9000);
        assert_eq!(config.mqtt.broker_url, "mqtt.example.com");
        assert_eq!(config.mqtt.port, 8883);
        assert_eq!(config.mqtt.qos, 2);
        assert_eq!(config.storage.base_path, "/tmp/parquet");
        assert!(!config.storage.wal_enabled);

        std::fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_env_overrides() {
        std::env::set_var("MQTT_BROKER_URL", "override-broker");
        std::env::set_var("MQTT_PORT", "8883");
        std::env::set_var("STORAGE_PATH", "/override/path");

        let yaml_content = r#"
server:
  host: "0.0.0.0"
  port: 8080
mqtt:
  broker_url: "localhost"
  port: 1883
  client_id: "test"
  topic_pattern: "test/+"
  qos: 1
  reconnect_delay_secs: 1
  max_reconnect_delay_secs: 30
  buffer_capacity: 1000
storage:
  base_path: "/data/parquet"
"#;

        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_env_override.yaml");
        let mut file = std::fs::File::create(&temp_file).unwrap();
        file.write_all(yaml_content.as_bytes()).unwrap();

        let config = AppConfig::from_yaml(&temp_file).unwrap();

        // Verify overrides applied
        assert_eq!(config.mqtt.broker_url, "override-broker");
        assert_eq!(config.mqtt.port, 8883);
        assert_eq!(config.storage.base_path, "/override/path");

        // Cleanup
        std::env::remove_var("MQTT_BROKER_URL");
        std::env::remove_var("MQTT_PORT");
        std::env::remove_var("STORAGE_PATH");
        std::fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_mqtt_config_conversion() {
        let config = AppConfig::default_config();
        let mqtt_config = config.mqtt.to_mqtt_config();

        assert_eq!(mqtt_config.broker_url, "localhost");
        assert_eq!(mqtt_config.port, 1883);
        assert_eq!(mqtt_config.client_id, "air-quality-app");
        assert_eq!(mqtt_config.topic_pattern, "airgradient/readings/+");
        assert_eq!(mqtt_config.buffer_capacity, 1000);
    }

    #[test]
    fn test_qos_conversion() {
        let mut config = AppConfig::default_config();

        // Test QoS 0
        config.mqtt.qos = 0;
        assert!(matches!(config.mqtt.to_mqtt_config().qos, QoS::AtMostOnce));

        // Test QoS 1
        config.mqtt.qos = 1;
        assert!(matches!(config.mqtt.to_mqtt_config().qos, QoS::AtLeastOnce));

        // Test QoS 2
        config.mqtt.qos = 2;
        assert!(matches!(config.mqtt.to_mqtt_config().qos, QoS::ExactlyOnce));

        // Test invalid QoS (defaults to 1)
        config.mqtt.qos = 99;
        assert!(matches!(config.mqtt.to_mqtt_config().qos, QoS::AtLeastOnce));
    }
}
```

### Key Changes Explained

1. **MqttConfigYaml**: YAML-serializable version of MQTT config
   - Uses `u8` for QoS instead of `QoS` enum (YAML-friendly)
   - Uses `u64` for durations instead of `Duration` (YAML-friendly)

2. **to_mqtt_config()**: Converts YAML config to platform-core types
   - Maps `qos: u8` → `QoS` enum
   - Maps `reconnect_delay_secs` → `Duration`

3. **apply_env_overrides()**: Supports environment variables
   - `MQTT_BROKER_URL`, `MQTT_PORT`, `MQTT_CLIENT_ID`
   - `STORAGE_PATH`
   - `SERVER_HOST`, `SERVER_PORT`

4. **Tests**: Comprehensive test coverage
   - Default config
   - YAML loading
   - Environment overrides
   - Type conversion

---

## Step 2: Create config.yaml (10 minutes)

### File Location
`/workspaces/neural-data-platform/apps/air-quality-app/config.yaml`

### File Content

```yaml
# Air Quality Application Configuration
# This file defines MQTT broker, storage, and server settings

server:
  host: "0.0.0.0"
  port: 8080

mqtt:
  # MQTT broker connection
  broker_url: "localhost"
  port: 1883
  client_id: "air-quality-app"

  # Topic pattern: + is wildcard for serial number
  topic_pattern: "airgradient/readings/+"

  # Quality of Service: 0 (at most once), 1 (at least once), 2 (exactly once)
  qos: 1

  # Reconnection settings
  reconnect_delay_secs: 1
  max_reconnect_delay_secs: 30

  # Internal buffer for messages
  buffer_capacity: 1000

storage:
  # Base path for Parquet files
  base_path: "/data/parquet"

  # Enable Write-Ahead Log for durability
  wal_enabled: true
```

### Alternative: Development Config

Create `config.dev.yaml` for local development:

```yaml
server:
  host: "127.0.0.1"
  port: 3000

mqtt:
  broker_url: "localhost"
  port: 1883
  client_id: "air-quality-dev"
  topic_pattern: "airgradient/readings/+"
  qos: 1
  reconnect_delay_secs: 1
  max_reconnect_delay_secs: 30
  buffer_capacity: 100

storage:
  base_path: "./data/parquet"
  wal_enabled: true
```

---

## Step 3: Add Tests (20 minutes)

Tests are already included in the `config.rs` code above. Run them:

```bash
cd /workspaces/neural-data-platform/apps/air-quality-app
cargo test config
```

Expected output:
```
running 6 tests
test config::tests::test_default_config ... ok
test config::tests::test_from_yaml ... ok
test config::tests::test_env_overrides ... ok
test config::tests::test_mqtt_config_conversion ... ok
test config::tests::test_qos_conversion ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## Step 4: Manual Verification (10 minutes)

### Test 1: Default Config

```bash
cd /workspaces/neural-data-platform/apps/air-quality-app

# Temporarily move config.yaml if it exists
mv config.yaml config.yaml.bak 2>/dev/null || true

# Run app (should use defaults)
cargo run --bin air-quality-server 2>&1 | grep -i "config\|mqtt\|storage"

# Expected logs:
# INFO Using default configuration
# INFO MQTT broker: localhost:1883
# INFO Storage path: /data/parquet
```

### Test 2: YAML Config Loading

```bash
# Restore config.yaml
mv config.yaml.bak config.yaml 2>/dev/null || true

# Run app
cargo run --bin air-quality-server 2>&1 | grep -i "config\|mqtt\|storage"

# Expected logs:
# INFO Loaded configuration from config.yaml
# INFO MQTT broker: localhost:1883
# INFO Storage path: /data/parquet
```

### Test 3: Environment Variable Overrides

```bash
# Test MQTT_BROKER_URL override
MQTT_BROKER_URL=test-broker cargo run --bin air-quality-server 2>&1 | grep "MQTT broker"

# Expected log:
# INFO MQTT broker: test-broker:1883

# Test STORAGE_PATH override
STORAGE_PATH=/tmp/test cargo run --bin air-quality-server 2>&1 | grep "Storage path"

# Expected log:
# INFO Storage path: /tmp/test
```

### Test 4: Invalid Config Handling

```bash
# Create invalid config
echo "invalid: yaml: content" > config.yaml

# Run app
cargo run --bin air-quality-server 2>&1 | grep -i "error\|failed"

# Expected error:
# ERROR Failed to load config: ...

# Restore valid config
git checkout config.yaml
```

---

## Step 5: Documentation (10 minutes)

### Update README.md

Add configuration section to `/workspaces/neural-data-platform/apps/air-quality-app/README.md`:

```markdown
## Configuration

The application uses `config.yaml` for configuration. If the file is missing, default values are used.

### Configuration File

Create `config.yaml` in the application root:

```yaml
server:
  host: "0.0.0.0"
  port: 8080

mqtt:
  broker_url: "localhost"
  port: 1883
  client_id: "air-quality-app"
  topic_pattern: "airgradient/readings/+"
  qos: 1
  reconnect_delay_secs: 1
  max_reconnect_delay_secs: 30
  buffer_capacity: 1000

storage:
  base_path: "/data/parquet"
  wal_enabled: true
```

### Environment Variables

Override configuration values using environment variables:

| Variable | Description | Example |
|----------|-------------|---------|
| `MQTT_BROKER_URL` | MQTT broker hostname | `mqtt.example.com` |
| `MQTT_PORT` | MQTT broker port | `8883` |
| `MQTT_CLIENT_ID` | MQTT client identifier | `air-quality-prod` |
| `STORAGE_PATH` | Base path for Parquet files | `/mnt/data/parquet` |
| `SERVER_HOST` | HTTP server bind address | `127.0.0.1` |
| `SERVER_PORT` | HTTP server port | `3000` |

### Usage Examples

```bash
# Use default config
cargo run --bin air-quality-server

# Use custom config file
cargo run --bin air-quality-server -- --config config.prod.yaml

# Override with environment variables
MQTT_BROKER_URL=mqtt.example.com \
STORAGE_PATH=/mnt/data \
cargo run --bin air-quality-server
```
```

---

## Troubleshooting

### Issue: "Failed to parse config file"

**Cause:** Invalid YAML syntax

**Solution:**
```bash
# Validate YAML syntax
python3 -c "import yaml; yaml.safe_load(open('config.yaml'))"

# Or use online validator: https://www.yamllint.com/
```

### Issue: "MQTT config type mismatch"

**Cause:** platform-core exports may be commented out

**Solution:**
Check `/workspaces/neural-data-platform/core/src/lib.rs`:
```rust
// Uncomment if needed:
pub use sources::{MqttConfig, MqttSource};
```

### Issue: "Cannot find config.yaml"

**Cause:** Working directory issue

**Solution:**
```bash
# Check current directory
pwd

# Should be: /workspaces/neural-data-platform/apps/air-quality-app

# If not, specify full path
cargo run --bin air-quality-server -- --config /full/path/to/config.yaml
```

---

## Integration with Other Tasks

### For T2 (MQTT Handler)

In `src/ingestion/mqtt_handler.rs`:

```rust
use crate::config::AppConfig;

impl MqttHandler {
    pub async fn new(config: &AppConfig) -> Result<Self> {
        // Convert to platform-core type
        let mqtt_config = config.mqtt.to_mqtt_config();

        // Initialize MQTT source
        let mut source = MqttSource::new(mqtt_config);
        source.start().await?;

        // ...
    }
}
```

### For T3 (Storage Writer)

In `src/pipeline/storage_writer.rs`:

```rust
use crate::config::AppConfig;

impl StorageWriter {
    pub fn new(config: &AppConfig) -> Result<Self> {
        // Use storage path from config
        let store = ParquetStore::new(&config.storage.base_path)?;

        // WAL enabled by default from config
        if config.storage.wal_enabled {
            // WAL is automatic in ParquetStore
        }

        // ...
    }
}
```

### For T4 (Main Integration)

In `src/main.rs`:

```rust
use air_quality_app::config::AppConfig;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = match AppConfig::from_yaml("config.yaml") {
        Ok(cfg) => cfg,
        Err(_) => {
            tracing::warn!("Config file not found, using defaults");
            AppConfig::default_config()
        }
    };

    tracing::info!("MQTT broker: {}:{}", config.mqtt.broker_url, config.mqtt.port);
    tracing::info!("Storage path: {}", config.storage.base_path);

    // Initialize components
    let storage = Arc::new(ParquetStore::new(&config.storage.base_path)?);
    let mqtt_handler = MqttHandler::new(&config).await?;

    // ...
}
```

---

## Acceptance Criteria Checklist

- [ ] Configuration loads from config.yaml
  - Test: Create config.yaml and run app
  - Expected: Logs show loaded values

- [ ] Environment variables override config values
  - Test: `MQTT_BROKER_URL=test cargo run`
  - Expected: Logs show "test" as broker

- [ ] Default config available if file missing
  - Test: Remove config.yaml and run app
  - Expected: App starts with defaults

- [ ] Converts to platform-core MqttConfig
  - Test: Call `config.mqtt.to_mqtt_config()`
  - Expected: Returns valid MqttConfig struct

- [ ] ParquetStore can use storage.base_path
  - Test: `ParquetStore::new(&config.storage.base_path)`
  - Expected: Store initializes successfully

---

## Out of Scope (Deferred to AIR-003)

Do NOT implement these in T1:

- ❌ config-store integration
- ❌ TOML format support
- ❌ Schema validation beyond basic serde
- ❌ Config versioning
- ❌ Dynamic config reloading
- ❌ Config encryption
- ❌ Multi-environment support

These will be added in AIR-003: Configuration Standardization.

---

## Time Tracking

Estimate vs Actual:

| Step | Estimated | Actual | Notes |
|------|-----------|--------|-------|
| 1. Update config.rs | 30min | ___ | ___ |
| 2. Create config.yaml | 10min | ___ | ___ |
| 3. Add tests | 20min | ___ | ___ |
| 4. Manual verification | 10min | ___ | ___ |
| 5. Documentation | 10min | ___ | ___ |
| **TOTAL** | **80min** | **___** | **___** |

---

## Git Commit Message

When you're done, commit with:

```bash
git add apps/air-quality-app/src/config.rs
git add apps/air-quality-app/config.yaml
git add apps/air-quality-app/README.md

git commit -m "$(cat <<'EOF'
feat(air-002): implement minimal YAML configuration system

Implements T1: Configuration Management for AIR-002 ingestion pipeline.

Changes:
- Add MqttConfigYaml with conversion to platform-core types
- Add StorageConfigYaml for Parquet storage settings
- Implement environment variable overrides
- Add comprehensive test coverage
- Create config.yaml with sensible defaults

Scope:
- YAML-based configuration loading
- Environment variable overrides (MQTT_BROKER_URL, STORAGE_PATH)
- Type conversion to platform-core structs
- Default config fallback

Out of Scope (Deferred to AIR-003):
- config-store integration
- TOML format support
- Advanced validation schemas

Estimated: 1-2 hours
Files: 3 modified/created
Tests: 6 passing

Generated with Claude Code

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>
EOF
)"
```

---

## Next Steps

After completing T1:

1. **Update Roadmap Status:**
   - Mark T1 as COMPLETED in `/workspaces/neural-data-platform/product/features/air-002/implementation/01-roadmap.md`
   - Update actual time spent

2. **Move to T2:**
   - Start implementing MQTT ingestion module
   - Use `config.mqtt.to_mqtt_config()` to get MQTT settings

3. **Integration Note:**
   - Config loading happens in `main.rs`
   - Pass `&AppConfig` to components as needed
   - Components convert to platform-core types internally

---

**End of Implementation Guide**
