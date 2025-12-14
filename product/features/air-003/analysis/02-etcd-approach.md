# AIR-003: etcd-Based Approach

## Why etcd Instead of Building Custom?

| Feature | Build Custom | Use etcd |
|---------|--------------|----------|
| KV Store | ~2 weeks | ✅ Included |
| Watch/Subscribe | ~1 week | ✅ Included |
| Versioning | ~3 days | ✅ Included |
| Clustering/HA | ~2 weeks | ✅ Included |
| gRPC API | ~1 week | ✅ Included |
| Battle-tested | No | ✅ 10+ years |

**Time saved: ~6-8 weeks of development**

## What We Still Build (Thin Layer)

### 1. Docker Compose Addition
```yaml
services:
  etcd:
    image: quay.io/coreos/etcd:v3.5.11
    ports:
      - "2379:2379"
    environment:
      - ETCD_ADVERTISE_CLIENT_URLS=http://etcd:2379
      - ETCD_LISTEN_CLIENT_URLS=http://0.0.0.0:2379
    volumes:
      - etcd-data:/etcd-data
```

### 2. Thin Rust Client (~200 lines)
```rust
// config-client/src/lib.rs
use etcd_client::{Client, GetOptions, WatchOptions};

pub struct ConfigClient {
    client: Client,
    prefix: String,
}

impl ConfigClient {
    pub async fn new(endpoints: &[&str], prefix: &str) -> Result<Self> {
        let client = Client::connect(endpoints, None).await?;
        Ok(Self { client, prefix: prefix.to_string() })
    }

    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        let full_key = format!("{}/{}", self.prefix, key);
        let resp = self.client.get(full_key, None).await?;
        let value = resp.kvs().first().ok_or(ConfigError::NotFound)?;
        serde_json::from_slice(value.value())
    }

    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let full_key = format!("{}/{}", self.prefix, key);
        let json = serde_json::to_vec(value)?;
        self.client.put(full_key, json, None).await?;
        Ok(())
    }

    pub async fn watch<F>(&self, key: &str, callback: F) -> Result<WatchHandle>
    where F: Fn(ConfigValue) + Send + 'static
    {
        let (watcher, mut stream) = self.client.watch(key, None).await?;
        tokio::spawn(async move {
            while let Some(resp) = stream.message().await? {
                for event in resp.events() {
                    if let Some(kv) = event.kv() {
                        callback(serde_json::from_slice(kv.value())?);
                    }
                }
            }
        });
        Ok(WatchHandle { watcher })
    }
}
```

### 3. Derive Macro (Optional, ~100 lines)
```rust
#[derive(Config)]
#[config(prefix = "/air-quality/app")]
struct AppConfig {
    #[config(key = "mqtt/broker_url", env = "MQTT_BROKER_URL")]
    broker_url: String,

    #[config(key = "mqtt/port", default = 1883)]
    port: u16,
}
```

### 4. GitOps Loader (Populate etcd from git)
```rust
// On startup or git webhook
async fn sync_git_to_etcd(git_path: &Path, etcd: &ConfigClient) {
    let configs = GitOpsLoader::load_all(git_path).await?;
    for (key, value) in configs {
        etcd.set(&key, &value).await?;
    }
}
```

## Architecture with etcd

```
┌─────────────────────────────────────────────────────────────┐
│                    Applications                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │air-quality-app│  │ neural-core │  │ other-service│      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                 │                 │               │
│         └────────────┬────┴────────────────┘               │
│                      ▼                                      │
│         ┌────────────────────────┐                         │
│         │   config-client crate  │  ← Thin wrapper (~200 LOC)
│         │  - Type-safe access    │                         │
│         │  - Env var override    │                         │
│         │  - Derive macro        │                         │
│         └────────────┬───────────┘                         │
└──────────────────────┼──────────────────────────────────────┘
                       │ gRPC
                       ▼
              ┌─────────────────┐
              │      etcd       │  ← Does the heavy lifting
              │  - KV store     │
              │  - Watch/notify │
              │  - Versioning   │
              │  - Clustering   │
              └─────────────────┘
                       ▲
                       │ Sync on startup/webhook
              ┌────────┴────────┐
              │   GitOps Sync   │
              │  (config repo)  │
              └─────────────────┘
```

## Migration Path

### Phase 1: Add etcd (1 day)
- Add to docker-compose.yml
- Basic connectivity test

### Phase 2: Create config-client (2-3 days)
- Wrap etcd-client
- Add env override layer
- Basic tests

### Phase 3: Migrate air-quality-app (1-2 days)
- Replace AppConfig with etcd-backed config
- Add watch for hot-reload

### Phase 4: GitOps sync (1 day)
- Script to populate etcd from config files
- Optional: webhook for auto-sync

**Total: ~5-7 days vs 3-4 weeks**

## Rust Dependencies

```toml
[dependencies]
etcd-client = "0.12"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
```

## etcd CLI Examples

```bash
# Set a config value
etcdctl put /air-quality/mqtt/broker_url "localhost"

# Get a value
etcdctl get /air-quality/mqtt/broker_url

# Watch for changes
etcdctl watch /air-quality --prefix

# Get all configs for a service
etcdctl get /air-quality --prefix

# View history
etcdctl get /air-quality/mqtt/port --rev=5
```

## When NOT to Use etcd

- If you need complex schema validation (add separate validation layer)
- If you need fine-grained ACLs (etcd has basic auth, not RBAC)
- If you're already using Consul/Vault (use those instead)

## Conclusion

**Use etcd.** Build a thin ~200-line wrapper for type safety and env overrides.

Existing config-store code can be repurposed as:
1. GitOps loader (already exists)
2. Schema validation (already exists)
3. Fallback for offline mode
