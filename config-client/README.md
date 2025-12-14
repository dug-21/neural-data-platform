# config-client

A thin, type-safe wrapper around etcd for configuration management.

## Features

- Type-safe configuration with serde
- Environment variable overrides
- Configuration watching with callbacks
- Key prefix support
- Simple API

## Usage

```rust
use config_client::ConfigClient;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct MqttConfig {
    broker_url: String,
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to etcd
    let client = ConfigClient::with_prefix(
        &["http://localhost:2379"],
        "/air-quality"
    ).await?;

    // Set configuration
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
    };
    client.set("/mqtt", &config).await?;

    // Get configuration
    let mqtt: MqttConfig = client.get("/mqtt").await?;
    println!("MQTT broker: {}:{}", mqtt.broker_url, mqtt.port);

    // Watch for changes
    let handle = client.watch("/", |key, value| {
        println!("Config changed: {} = {:?}", key, value);
    }).await?;

    Ok(())
}
```

## Environment Variable Overrides

```rust
// Checks AIR_QUALITY_MQTT_BROKER_URL before etcd
let broker: String = client.get_with_env("/mqtt/broker_url", "AIR_QUALITY").await?;
```

## Running the Example

```bash
# Start etcd
docker run -d -p 2379:2379 -p 2380:2380 \
  quay.io/coreos/etcd:latest \
  /usr/local/bin/etcd \
  --listen-client-urls http://0.0.0.0:2379 \
  --advertise-client-urls http://localhost:2379

# Run the example
cargo run --example basic
```

## API

### ConfigClient

- `new(endpoints)` - Connect to etcd
- `with_prefix(endpoints, prefix)` - Connect with key prefix
- `get<T>(key)` - Get typed configuration
- `set<T>(key, value)` - Set configuration
- `delete(key)` - Delete configuration
- `list(prefix)` - List all keys under prefix
- `watch(prefix, callback)` - Watch for changes
- `get_with_env<T>(key, env_prefix)` - Get with env override

### Error Types

- `ConfigError::NotFound` - Key not found
- `ConfigError::ConnectionFailed` - etcd connection failed
- `ConfigError::SerializationError` - JSON parsing failed
- `ConfigError::WatchError` - Watch operation failed
- `ConfigError::EnvError` - Environment variable parsing failed
