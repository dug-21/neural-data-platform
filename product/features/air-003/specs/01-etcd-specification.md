# AIR-003: etcd Configuration Store - SPARC Specification

## Executive Summary

### Goal
Implement a production-ready configuration management system using etcd as the backend with a thin Rust wrapper client library.

### Key Benefits
- **Battle-tested reliability**: etcd is production-proven by Kubernetes and major cloud platforms
- **Built-in features**: Watch/subscribe, versioning, consistency guarantees, and HA support
- **Reduced development time**: ~5-7 days vs 3-4 weeks for custom solution
- **Lower maintenance burden**: Leverage etcd's extensive testing and community support
- **Type-safe access**: Thin Rust wrapper provides ergonomic, type-safe API

### Timeline
- **Phase 1** (Days 1-2): etcd setup + basic config-client wrapper
- **Phase 2** (Days 3-4): GitOps sync + watch functionality
- **Phase 3** (Days 5-7): Integration with air-quality-app + testing

## Architecture Overview

### Component Stack

```
┌─────────────────────────────────────────────────────────────┐
│                     Applications Layer                       │
│  ┌──────────────────┐         ┌──────────────────┐          │
│  │ air-quality-app  │         │  Future Apps     │          │
│  └────────┬─────────┘         └────────┬─────────┘          │
│           │                            │                     │
└───────────┼────────────────────────────┼─────────────────────┘
            │                            │
            └────────────┬───────────────┘
                         │
            ┌────────────▼────────────┐
            │   config-client crate   │
            │   (~200 lines Rust)     │
            │  - Type-safe API        │
            │  - Watch/Subscribe      │
            │  - Error handling       │
            └────────────┬────────────┘
                         │
            ┌────────────▼────────────┐
            │    etcd v3.5.11         │
            │  (Docker container)     │
            │  - KV Store             │
            │  - Watch API            │
            │  - Versioning           │
            │  - Consensus (Raft)     │
            └────────────┬────────────┘
                         │
            ┌────────────▼────────────┐
            │   Persistent Volume     │
            │     etcd-data:/         │
            └─────────────────────────┘
```

### Components

#### 1. etcd (Docker Container)
- **Image**: `quay.io/coreos/etcd:v3.5.11`
- **Ports**: 2379 (client), 2380 (peer)
- **Storage**: Persistent volume for data durability
- **Health checks**: Built-in endpoint health monitoring

#### 2. config-client (Rust Crate)
- **Size**: ~200 lines of code
- **Purpose**: Type-safe wrapper around etcd client
- **Dependencies**: `etcd-client` crate
- **Location**: `/crates/config-client/`

#### 3. GitOps Sync Script
- **Purpose**: Load YAML config files into etcd on startup
- **Trigger**: Container initialization, manual sync
- **Location**: `/scripts/sync-config-to-etcd.sh`

#### 4. Configuration Files
- **Format**: YAML
- **Location**: `/config/base/`, `/config/overlays/`
- **Examples**: `air-quality.yaml`, `overrides.yaml`

## Data Flow

```
┌──────────────────┐
│  Git Repository  │
│  /config/*.yaml  │
└────────┬─────────┘
         │
         │ (1) GitOps Sync on Startup
         │
         ▼
┌────────────────────────────────────────┐
│              etcd                       │
│  /config/air-quality                   │
│  /config/devices/airgradient-001       │
│  /config/thresholds                    │
└────────┬───────────────────────────────┘
         │
         │ (2) Applications Pull Config
         │
         ▼
┌──────────────────────────────────────┐
│        config-client API              │
│  client.get("/config/air-quality")   │
│  client.watch("/config/*", callback) │
└────────┬─────────────────────────────┘
         │
         │ (3) Type-safe Structs
         │
         ▼
┌──────────────────────────────────────┐
│       air-quality-app                │
│  Uses deserialized config structs    │
│  Receives real-time updates          │
└──────────────────────────────────────┘
```

### Flow Steps

1. **Configuration Loading** (GitOps Sync)
   - Read YAML files from `/config/` directory
   - Parse and validate configuration
   - Store in etcd with hierarchical keys
   - Example: `/config/air-quality/mqtt/broker_url`

2. **Application Bootstrap**
   - Application starts and connects to etcd
   - Retrieves configuration using config-client
   - Deserializes to typed Rust structs
   - Initializes services with loaded config

3. **Real-time Updates** (Watch/Subscribe)
   - Application subscribes to config key prefixes
   - etcd notifies on any changes
   - Application receives updates via callback
   - Hot-reload configuration without restart

## API Design (config-client)

### Core API

```rust
use serde::{Deserialize, Serialize};
use etcd_client::{Client, Error as EtcdError, WatchStream};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct ConfigClient {
    client: Client,
}

impl ConfigClient {
    /// Create a new config client
    ///
    /// # Arguments
    /// * `endpoints` - List of etcd endpoints (e.g., ["http://localhost:2379"])
    ///
    /// # Example
    /// ```
    /// let client = ConfigClient::new(&["http://etcd:2379"]).await?;
    /// ```
    pub async fn new(endpoints: &[&str]) -> Result<Self, ConfigError> {
        let client = Client::connect(endpoints, None).await?;
        Ok(Self { client })
    }

    /// Get a configuration value by key
    ///
    /// # Arguments
    /// * `key` - Configuration key path (e.g., "/config/air-quality")
    ///
    /// # Returns
    /// Deserialized configuration struct of type T
    ///
    /// # Example
    /// ```
    /// let config: AirQualityConfig = client.get("/config/air-quality").await?;
    /// ```
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError> {
        let mut client = self.client.clone();
        let resp = client.get(key, None).await?;

        if let Some(kv) = resp.kvs().first() {
            let value = kv.value();
            let config: T = serde_json::from_slice(value)?;
            Ok(config)
        } else {
            Err(ConfigError::NotFound(key.to_string()))
        }
    }

    /// Set a configuration value
    ///
    /// # Arguments
    /// * `key` - Configuration key path
    /// * `value` - Configuration value (will be serialized)
    ///
    /// # Example
    /// ```
    /// let config = AirQualityConfig::default();
    /// client.set("/config/air-quality", &config).await?;
    /// ```
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<(), ConfigError> {
        let mut client = self.client.clone();
        let json = serde_json::to_vec(value)?;
        client.put(key, json, None).await?;
        Ok(())
    }

    /// Watch for configuration changes
    ///
    /// # Arguments
    /// * `prefix` - Key prefix to watch (e.g., "/config/")
    /// * `callback` - Function called on each change
    ///
    /// # Returns
    /// WatchHandle for cancellation
    ///
    /// # Example
    /// ```
    /// let handle = client.watch("/config/air-quality", |key, value| {
    ///     println!("Config changed: {} = {:?}", key, value);
    /// }).await?;
    /// ```
    pub async fn watch<F>(&self, prefix: &str, callback: F) -> Result<WatchHandle, ConfigError>
    where
        F: Fn(&str, &[u8]) + Send + 'static,
    {
        let mut client = self.client.clone();
        let (watcher, stream) = client.watch(prefix, None).await?;

        // Spawn task to process watch events
        let handle = tokio::spawn(async move {
            while let Some(resp) = stream.message().await? {
                for event in resp.events() {
                    if let Some(kv) = event.kv() {
                        let key = String::from_utf8_lossy(kv.key());
                        callback(key.as_ref(), kv.value());
                    }
                }
            }
            Ok::<_, ConfigError>(())
        });

        Ok(WatchHandle { handle })
    }

    /// Get all keys with a given prefix
    ///
    /// # Arguments
    /// * `prefix` - Key prefix (e.g., "/config/devices/")
    ///
    /// # Returns
    /// HashMap of key-value pairs
    pub async fn get_prefix<T: DeserializeOwned>(
        &self,
        prefix: &str,
    ) -> Result<HashMap<String, T>, ConfigError> {
        let mut client = self.client.clone();
        let resp = client.get(prefix, Some(GetOptions::new().with_prefix())).await?;

        let mut results = HashMap::new();
        for kv in resp.kvs() {
            let key = String::from_utf8_lossy(kv.key()).to_string();
            let value: T = serde_json::from_slice(kv.value())?;
            results.insert(key, value);
        }

        Ok(results)
    }
}

/// Handle for canceling a watch subscription
pub struct WatchHandle {
    handle: JoinHandle<Result<(), ConfigError>>,
}

impl WatchHandle {
    pub async fn cancel(self) -> Result<(), ConfigError> {
        self.handle.abort();
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("etcd error: {0}")]
    Etcd(#[from] EtcdError),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("configuration not found: {0}")]
    NotFound(String),

    #[error("invalid configuration: {0}")]
    Invalid(String),
}
```

### Usage Example

```rust
use config_client::{ConfigClient, ConfigError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct AirQualityConfig {
    mqtt: MqttConfig,
    devices: Vec<DeviceConfig>,
    thresholds: ThresholdConfig,
}

#[tokio::main]
async fn main() -> Result<(), ConfigError> {
    // Initialize client
    let client = ConfigClient::new(&["http://etcd:2379"]).await?;

    // Load configuration
    let config: AirQualityConfig = client.get("/config/air-quality").await?;
    println!("Loaded config: {:?}", config);

    // Watch for changes
    let handle = client.watch("/config/air-quality", |key, value| {
        println!("Configuration updated: {}", key);
        // Trigger hot-reload logic here
    }).await?;

    // Application logic...

    Ok(())
}
```

## Configuration Schema

### Key Hierarchy

```
/config/
├── air-quality/
│   ├── mqtt/
│   │   ├── broker_url
│   │   ├── port
│   │   └── client_id
│   ├── http/
│   │   ├── port
│   │   └── bind_address
│   └── metrics/
│       └── port
├── devices/
│   ├── airgradient-001/
│   │   ├── location
│   │   ├── calibration
│   │   └── enabled
│   └── airgradient-002/
│       └── ...
└── thresholds/
    ├── pm25_warning
    ├── pm25_critical
    ├── co2_warning
    └── co2_critical
```

### Example Configuration (JSON in etcd)

```json
{
  "/config/air-quality": {
    "mqtt": {
      "broker_url": "mqtt://mosquitto:1883",
      "port": 1883,
      "client_id": "air-quality-app"
    },
    "http": {
      "port": 8080,
      "bind_address": "0.0.0.0"
    },
    "metrics": {
      "port": 9090
    }
  },
  "/config/devices/airgradient-001": {
    "location": "office",
    "calibration": {
      "pm25_offset": 0.0,
      "co2_offset": 0.0
    },
    "enabled": true
  },
  "/config/thresholds": {
    "pm25_warning": 35.0,
    "pm25_critical": 55.0,
    "co2_warning": 1000.0,
    "co2_critical": 2000.0
  }
}
```

## Implementation Plan

### Phase 1: Foundation (Days 1-2)

**Day 1: etcd Setup**
- Add etcd service to docker-compose.yml ✅
- Configure health checks and networking
- Test etcd connectivity with etcdctl
- Verify data persistence across restarts

**Day 2: config-client Wrapper**
- Create `/crates/config-client/` crate
- Implement basic get/set operations
- Add error handling and types
- Write unit tests with mock etcd

### Phase 2: Integration (Days 3-4)

**Day 3: GitOps Sync**
- Create sync script to load YAML → etcd
- Parse configuration files
- Validate schema before loading
- Add to container startup process

**Day 4: Watch Functionality**
- Implement watch API in config-client
- Add callback mechanism for updates
- Test real-time configuration changes
- Handle watch failures and reconnection

### Phase 3: Application Integration (Days 5-7)

**Day 5: air-quality-app Integration**
- Replace file-based config with etcd
- Update environment variables
- Test configuration loading
- Verify fallback behavior

**Day 6: Hot Reload**
- Implement configuration hot-reload
- Add watch subscriptions for critical paths
- Test dynamic reconfiguration
- Add logging for config changes

**Day 7: Testing & Documentation**
- End-to-end testing
- Performance benchmarks
- Write integration tests
- Update documentation

## Success Criteria

### Functional Requirements
- ✅ etcd container running in Docker Compose
- ✅ etcd health check passing
- ✅ config-client loads typed configuration structs
- ✅ Watch API detects configuration changes within 1 second
- ✅ air-quality-app uses etcd for all configuration
- ✅ GitOps sync successfully loads YAML files into etcd
- ✅ Configuration persists across container restarts

### Non-Functional Requirements
- **Performance**: Config retrieval < 10ms (p95)
- **Reliability**: 99.9% uptime for etcd
- **Maintainability**: < 300 lines of custom code
- **Testability**: > 80% test coverage for config-client
- **Documentation**: Complete API docs and usage examples

## Testing Strategy

### Unit Tests
- config-client API methods
- Serialization/deserialization
- Error handling
- Mock etcd responses

### Integration Tests
- etcd connectivity
- Configuration loading
- Watch/subscribe functionality
- GitOps sync process

### End-to-End Tests
- Full application startup with etcd config
- Configuration hot-reload
- Multi-service coordination
- Failure recovery scenarios

## Monitoring & Observability

### Metrics to Track
- Config retrieval latency (p50, p95, p99)
- Watch event processing time
- etcd connection failures
- Configuration validation errors
- Hot-reload success rate

### Logging
- Configuration load events
- Watch subscription lifecycle
- Configuration change notifications
- Error conditions and retries

## Security Considerations

### Current Scope (Development)
- No authentication (local development only)
- No TLS encryption
- No access control

### Future Production Requirements
- Enable etcd authentication
- TLS certificates for client-server communication
- RBAC for configuration namespaces
- Audit logging for configuration changes
- Secret management integration (not in etcd)

## Migration Path

### From File-based Config
1. Keep existing YAML files as source of truth
2. GitOps sync loads YAML → etcd on startup
3. Applications gradually migrate to etcd client
4. File-based fallback for backward compatibility
5. Eventually remove file-based loading

### Rollback Strategy
- Environment variable to disable etcd
- Fallback to file-based configuration
- No breaking changes to existing deployments

## Dependencies

### External Crates
- `etcd-client = "0.13"` - Official etcd Rust client
- `serde = "1.0"` - Serialization framework
- `serde_json = "1.0"` - JSON support
- `tokio = "1.0"` - Async runtime
- `thiserror = "1.0"` - Error handling

### Infrastructure
- Docker Compose for local development
- etcd v3.5.11 container image
- Persistent volumes for data storage

## References

- [etcd Documentation](https://etcd.io/docs/v3.5/)
- [etcd-client Rust Crate](https://docs.rs/etcd-client/)
- [etcd API Reference](https://etcd.io/docs/v3.5/learning/api/)
- [Kubernetes ConfigMap Design](https://kubernetes.io/docs/concepts/configuration/configmap/)

## Appendix

### Alternative Approaches Considered

1. **Custom Rust Configuration Server**
   - Time: 3-4 weeks
   - Risk: Higher (new codebase, testing, edge cases)
   - Benefit: Full control, custom features
   - Verdict: Overkill for current needs

2. **Redis as Config Store**
   - Pros: Familiar, fast, simple
   - Cons: No built-in watch, weaker consistency, not designed for config
   - Verdict: Not designed for distributed configuration

3. **Consul**
   - Pros: Service discovery + config
   - Cons: More complex, heavier weight
   - Verdict: Too much overhead for current scope

### Decision Rationale

etcd chosen because:
- Production-proven by Kubernetes
- Purpose-built for distributed configuration
- Watch/subscribe built-in
- Strong consistency guarantees
- Minimal custom code required
- Fastest path to production
