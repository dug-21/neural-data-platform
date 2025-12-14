# Air Quality App - etcd Configuration Integration

## Overview

This document describes the integration of etcd-based configuration into the air-quality-app, enabling centralized configuration management with environment variable overrides.

## Implementation Summary

### Files Modified

1. **`/workspaces/neural-data-platform/apps/air-quality-app/Cargo.toml`**
   - Added `config-client = { path = "../../config-client" }` dependency

2. **`/workspaces/neural-data-platform/apps/air-quality-app/src/config.rs`**
   - Extended `StorageConfig` with `batch_size` and `batch_timeout_secs` fields
   - Updated default values and tests accordingly

3. **`/workspaces/neural-data-platform/apps/air-quality-app/src/lib.rs`**
   - Added `pub mod config_etcd;`
   - Exported `EtcdAppConfig` and `load_from_etcd` function

### Files Created

1. **`/workspaces/neural-data-platform/apps/air-quality-app/src/config_etcd.rs`**
   - New module for etcd-based configuration loading
   - Implements `load_from_etcd()` async function
   - Defines `EtcdAppConfig` with nested config structs
   - Environment variable override support via `get_with_env()`
   - Helper methods for QoS and duration conversion

2. **`/workspaces/neural-data-platform/apps/air-quality-app/tests/etcd_config_test.rs`**
   - Integration tests for etcd configuration
   - Tests for basic config loading
   - Tests for environment variable overrides
   - Tests for watching config changes
   - All tests marked with `#[ignore]` to require etcd running

## Configuration Structure

### etcd Key Layout

All keys are prefixed with `/air-quality/`:

```
/air-quality/server/host              -> "0.0.0.0"
/air-quality/server/port              -> 8080
/air-quality/mqtt/broker_url          -> "localhost"
/air-quality/mqtt/port                -> 1883
/air-quality/mqtt/client_id           -> "air-quality-app"
/air-quality/mqtt/topic_pattern       -> "airgradient/readings/+"
/air-quality/mqtt/qos                 -> 1
/air-quality/mqtt/reconnect_delay_secs -> 1
/air-quality/mqtt/max_reconnect_delay_secs -> 30
/air-quality/mqtt/buffer_capacity     -> 1000
/air-quality/storage/base_path        -> "./data/parquet"
/air-quality/storage/wal_enabled      -> true
/air-quality/storage/batch_size       -> 100
/air-quality/storage/batch_timeout_secs -> 5
```

### Environment Variable Overrides

Environment variables use the format `AIR_QUALITY_<SECTION>_<KEY>`:

- `AIR_QUALITY_SERVER_HOST` overrides `/air-quality/server/host`
- `AIR_QUALITY_SERVER_PORT` overrides `/air-quality/server/port`
- `AIR_QUALITY_MQTT_BROKER_URL` overrides `/air-quality/mqtt/broker_url`
- etc.

## Usage

### Basic Usage

```rust
use air_quality_app::load_from_etcd;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load config from etcd (falls back to file config if unavailable)
    let config = load_from_etcd().await?;

    println!("Server will listen on {}:{}", config.server.host, config.server.port);
    println!("MQTT broker: {}:{}", config.mqtt.broker_url, config.mqtt.port);

    // Use config to initialize services...
    Ok(())
}
```

### With Custom etcd Endpoint

```bash
# Set etcd endpoint via environment variable
export ETCD_ENDPOINT="http://etcd-cluster:2379"
cargo run
```

### Environment Variable Override Example

```bash
# Override server port via environment variable
export AIR_QUALITY_SERVER_PORT=9090
cargo run
```

## Testing

### Run Unit Tests

```bash
# All config tests (note: some may be flaky due to test isolation issues)
cargo test -p air-quality-app --lib config::tests

# Run tests serially to avoid race conditions
cargo test -p air-quality-app --lib config::tests -- --test-threads=1
```

### Run Integration Tests (Requires etcd)

```bash
# Start etcd via Docker Compose
docker compose up -d etcd

# Run integration tests
cargo test -p air-quality-app --test etcd_config_test -- --ignored

# Cleanup
docker compose down
```

### Integration Test Coverage

1. **test_load_config_from_etcd**: Verifies loading configuration from etcd
2. **test_env_override**: Validates environment variable precedence over etcd values
3. **test_watch_config_changes**: Tests dynamic configuration updates via watch

## Build Status

```bash
# Verify compilation
cargo check -p air-quality-app
```

Output: ✅ Compiles successfully with no errors (only upstream warnings)

## Architecture Benefits

1. **Centralized Configuration**: Single source of truth in etcd
2. **Dynamic Updates**: Watch API enables runtime config changes
3. **Environment Flexibility**: Override any setting via environment variables
4. **Fallback Support**: Gracefully falls back to file-based config if etcd unavailable
5. **Type Safety**: Strong typing for all configuration values
6. **Namespace Isolation**: `/air-quality` prefix prevents conflicts

## Next Steps

1. Update `main.rs` to use `load_from_etcd()` instead of file-based config
2. Add configuration documentation for operators
3. Create etcd initialization scripts with default values
4. Add health checks for etcd connectivity
5. Implement configuration validation
6. Add metrics for config load success/failure rates

## Related Files

- `/workspaces/neural-data-platform/config-client/` - Shared etcd client library
- `/workspaces/neural-data-platform/apps/air-quality-app/src/config.rs` - Original file-based config
- `/workspaces/neural-data-platform/docs/architecture/config-store-client-architecture.drawio` - Overall architecture diagram
