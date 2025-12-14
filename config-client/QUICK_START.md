# Quick Start Guide

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
config-client = { path = "../config-client" }
```

## Basic Usage

### 1. Connect to etcd

```rust
use config_client::ConfigClient;

let client = ConfigClient::new(&["http://localhost:2379"]).await?;
```

### 2. Store and Retrieve Configuration

```rust
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct DatabaseConfig {
    host: String,
    port: u16,
    database: String,
}

// Store
let config = DatabaseConfig {
    host: "localhost".to_string(),
    port: 5432,
    database: "myapp".to_string(),
};
client.set("/database", &config).await?;

// Retrieve
let db_config: DatabaseConfig = client.get("/database").await?;
println!("Connecting to {}:{}", db_config.host, db_config.port);
```

### 3. Use Key Prefixes

```rust
// All operations will be prefixed with "/myapp"
let client = ConfigClient::with_prefix(
    &["http://localhost:2379"],
    "/myapp"
).await?;

// This actually stores at "/myapp/database"
client.set("/database", &config).await?;
```

### 4. Environment Variable Overrides

```rust
// Set env var: MYAPP_DATABASE_HOST=prod-db.example.com
std::env::set_var("MYAPP_DATABASE_HOST", "prod-db.example.com");

// This will use the env var instead of etcd
let host: String = client.get_with_env("/database/host", "MYAPP").await?;
// Returns "prod-db.example.com"
```

### 5. Watch for Configuration Changes

```rust
let handle = client.watch("/database", |key, value| {
    println!("Config changed: {} = {:?}", key, value);
    // Reload your application configuration here
}).await?;

// Keep watching until shutdown
tokio::signal::ctrl_c().await?;
handle.cancel().await;
```

### 6. List Configuration Keys

```rust
let keys = client.list("/database/").await?;
for key in keys {
    println!("Found config key: {}", key);
}
```

### 7. Delete Configuration

```rust
client.delete("/database/temp-setting").await?;
```

## Common Patterns

### Application Bootstrap

```rust
use config_client::ConfigClient;
use serde::Deserialize;

#[derive(Deserialize)]
struct AppConfig {
    database: DatabaseConfig,
    mqtt: MqttConfig,
    api: ApiConfig,
}

async fn load_config() -> Result<AppConfig, Box<dyn std::error::Error>> {
    let client = ConfigClient::with_prefix(
        &["http://localhost:2379"],
        "/myapp"
    ).await?;

    Ok(AppConfig {
        database: client.get("/database").await?,
        mqtt: client.get("/mqtt").await?,
        api: client.get("/api").await?,
    })
}
```

### Hot Reload Configuration

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

async fn watch_config(config: Arc<RwLock<AppConfig>>) -> Result<(), Box<dyn std::error::Error>> {
    let client = ConfigClient::with_prefix(
        &["http://localhost:2379"],
        "/myapp"
    ).await?;

    let config_clone = config.clone();
    client.watch("/", move |key, value| {
        if key.ends_with("/database") {
            if let Some(val) = value {
                if let Ok(db_config) = serde_json::from_value(val) {
                    let mut config = config_clone.blocking_write();
                    config.database = db_config;
                }
            }
        }
    }).await?;

    Ok(())
}
```

### Fallback to Defaults

```rust
fn get_or_default<T: DeserializeOwned + Default>(
    client: &ConfigClient,
    key: &str
) -> Result<T, ConfigError> {
    match client.get(key).await {
        Ok(value) => Ok(value),
        Err(ConfigError::NotFound(_)) => Ok(T::default()),
        Err(e) => Err(e),
    }
}
```

## Testing

### With etcd

Start etcd in Docker:

```bash
docker run -d -p 2379:2379 -p 2380:2380 \
  quay.io/coreos/etcd:latest \
  /usr/local/bin/etcd \
  --listen-client-urls http://0.0.0.0:2379 \
  --advertise-client-urls http://localhost:2379
```

Run tests:

```bash
cargo test -- --ignored
```

### Without etcd (unit tests only)

```bash
cargo test
```

## Error Handling

```rust
use config_client::ConfigError;

match client.get::<DatabaseConfig>("/database").await {
    Ok(config) => println!("Got config: {:?}", config),
    Err(ConfigError::NotFound(key)) => {
        eprintln!("Config not found: {}", key);
        // Use defaults or prompt user
    }
    Err(ConfigError::ConnectionFailed(msg)) => {
        eprintln!("Cannot connect to etcd: {}", msg);
        // Retry or fail startup
    }
    Err(ConfigError::SerializationError(msg)) => {
        eprintln!("Invalid config format: {}", msg);
        // Log and use defaults
    }
    Err(e) => {
        eprintln!("Unexpected error: {}", e);
    }
}
```

## Best Practices

1. **Use prefixes** to namespace your application's configuration
2. **Type everything** - leverage serde for type safety
3. **Watch critical configs** - react to changes without restarts
4. **Handle errors gracefully** - have sensible defaults
5. **Use env overrides** for deployment-specific settings
6. **Document your config schema** - make it clear what's required

## Next Steps

- See `examples/basic.rs` for a complete example
- Read the API documentation: `cargo doc --open`
- Check the README.md for architecture details
