# AIR-003: Universal Configuration with etcd

## Overview

AIR-003 implements a universal configuration management system using **etcd** as the backend, with a thin Rust wrapper (`config-client`) for type-safe access.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│              Applications                            │
│  ┌─────────────┐  ┌─────────────┐                  │
│  │air-quality  │  │  other-app  │                  │
│  │    -app     │  │             │                  │
│  └──────┬──────┘  └──────┬──────┘                  │
│         │                │                          │
│         └───────┬────────┘                          │
│                 ▼                                   │
│     ┌───────────────────────┐                      │
│     │    config-client      │  ~200 lines          │
│     │  (thin etcd wrapper)  │                      │
│     └───────────┬───────────┘                      │
└─────────────────┼───────────────────────────────────┘
                  │ gRPC
                  ▼
         ┌─────────────────┐
         │      etcd       │  Production-tested
         │   (container)   │  Watch/subscribe
         └────────┬────────┘  Versioning
                  ▲
                  │ Sync
         ┌────────┴────────┐
         │   GitOps Sync   │
         │  (config repo)  │
         └─────────────────┘
```

## Quick Start

### 1. Start etcd
```bash
docker compose up -d etcd
```

### 2. Sync config from files to etcd
```bash
./scripts/sync-config-to-etcd.sh development
```

### 3. Verify config in etcd
```bash
docker exec etcd etcdctl get --prefix /air-quality
```

### 4. Run air-quality-app with etcd config
```bash
ETCD_ENDPOINT=http://localhost:2379 cargo run -p air-quality-app
```

## Configuration Structure

```
config/
├── base/                    # Default configs (all environments)
│   └── air-quality/
│       └── config.yaml
├── overlays/
│   ├── development/         # Dev overrides
│   │   └── air-quality/
│   │       └── config.yaml
│   └── production/          # Prod overrides
│       └── air-quality/
│           └── config.yaml
└── schemas/                 # JSON schemas (optional)
```

## Environment Variable Overrides

Environment variables take highest precedence:
```bash
# Format: AIR_QUALITY_{PATH_WITH_UNDERSCORES}
export AIR_QUALITY_MQTT_BROKER_URL="custom-broker"
export AIR_QUALITY_SERVER_PORT="9090"
```

## API Usage

```rust
use config_client::ConfigClient;
use serde::Deserialize;

#[derive(Deserialize)]
struct MqttConfig {
    broker_url: String,
    port: u16,
}

// Connect to etcd
let client = ConfigClient::with_prefix(
    &["http://localhost:2379"],
    "/air-quality"
).await?;

// Get typed config
let mqtt: MqttConfig = client.get("/mqtt").await?;

// Watch for changes
client.watch("/mqtt", |key, value| {
    println!("Config changed: {} = {:?}", key, value);
}).await?;
```

## Testing

```bash
# Start etcd
docker compose up -d etcd

# Run integration tests
cargo test -p air-quality-app --test etcd_config_test -- --ignored

# Run config-client tests
cargo test -p config-client
```

## Files

| Component | Location |
|-----------|----------|
| config-client crate | `/config-client/` |
| etcd Docker setup | `/docker-compose.yml` |
| GitOps sync script | `/scripts/sync-config-to-etcd.sh` |
| Base configs | `/config/base/` |
| Environment overlays | `/config/overlays/` |
| SPARC spec | `/product/features/air-003/specs/` |
