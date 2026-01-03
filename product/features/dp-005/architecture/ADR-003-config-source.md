# ADR-003: etcd as Configuration Source

## Status

Accepted

## Date

2026-01-03

## Context

The dp-005 Bronze MCP Server needs access to stream configuration for the `validate_config` and `describe_schema` tools. Stream configuration exists in two places:

### Configuration Sources

1. **Source YAML files**: `config/base/streams/{stream_id}/config.yaml`
   - Version controlled in Git
   - Contains full stream configuration
   - Requires file access and YAML parsing

2. **etcd registry**: `/streams/{stream_id}/*` keys
   - Synced from YAML via ConfigSyncService
   - Already denormalized and queryable
   - What the running application uses

### NDP Configuration Flow

```
config/base/streams/air-quality/config.yaml
        │
        │ ConfigSyncService (on app startup)
        │ or scripts/sync-config-to-etcd.sh
        ▼
    etcd cluster
        │
        │ /streams/air-quality/stream_id
        │ /streams/air-quality/enabled
        │ /streams/air-quality/fields/pm25/type
        │ /streams/air-quality/entity_schemas/0/...
        ▼
    Running applications (air-quality-app, MCP server)
```

### Requirements

| Requirement | Priority | Notes |
|-------------|----------|-------|
| Access stream metadata | Must | stream_id, description, enabled |
| Access entity_schemas | Must | Target schema for validation |
| Access field mappings | Should | Parser configuration |
| Validate sync pipeline | Must | Ensure YAML -> etcd works |
| Fast startup | Should | No YAML parsing overhead |
| Consistent with app | Must | Same config source as ingestion |

## Decision

**Read configuration from etcd, not source YAML files. Fail fast if etcd is unavailable.**

### Rationale

1. **Validates the full pipeline**: If MCP reads from etcd, it validates that:
   - ConfigSyncService correctly parses YAML
   - ConfigSyncService correctly writes to etcd
   - etcd is running and accessible
   - Key structure is correct

2. **Single source of truth**: The running ingestion app uses etcd. MCP should see the same configuration.

3. **Already denormalized**: etcd has flattened, queryable key structure.

4. **Consistency**: If YAML is updated but not synced, MCP sees the same stale config as the app.

### ConfigStore Trait

Following the Domain Adapter pattern:

```rust
use async_trait::async_trait;
use crate::error::McpResult;
use serde_json::Value;

/// Stream configuration metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub stream_id: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub enabled: bool,
    pub sources: Vec<SourceInfo>,
    pub entity_schemas: Vec<EntitySchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    pub source_type: String,  // "mqtt", "http_poll", etc.
    pub enabled: bool,
    pub parser: Option<ParserConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserConfig {
    pub parser_type: String,
    pub field_mappings: Vec<FieldMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    pub source_path: String,
    pub target_field: String,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySchema {
    pub schema_name: String,
    pub description: Option<String>,
    pub attributes: Vec<Attribute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribute {
    pub name: String,
    pub attr_type: String,
    pub unit: Option<String>,
    pub nullable: Option<bool>,
}

/// Configuration store abstraction (Port)
#[async_trait]
pub trait ConfigStore: Send + Sync {
    /// List all configured stream IDs
    async fn list_streams(&self) -> McpResult<Vec<String>>;

    /// Get full configuration for a stream
    async fn get_stream(&self, stream_id: &str) -> McpResult<StreamConfig>;

    /// Check if config store is accessible
    async fn health_check(&self) -> McpResult<()>;
}
```

### etcd Implementation

```rust
use etcd_client::Client;

pub struct EtcdConfigStore {
    client: Client,
    prefix: String,  // "/streams"
}

impl EtcdConfigStore {
    pub async fn new(endpoints: &[&str]) -> McpResult<Self> {
        let client = Client::connect(endpoints, None).await
            .map_err(|e| McpError::ConfigUnavailable(e.to_string()))?;
        Ok(Self { client, prefix: "/streams".to_string() })
    }
}

#[async_trait]
impl ConfigStore for EtcdConfigStore {
    async fn list_streams(&self) -> McpResult<Vec<String>> {
        // GET /streams/ with prefix, extract unique stream_ids
        let resp = self.client.get(
            self.prefix.as_bytes(),
            Some(GetOptions::new().with_prefix())
        ).await?;

        let stream_ids: HashSet<String> = resp.kvs().iter()
            .filter_map(|kv| {
                let key = String::from_utf8_lossy(kv.key());
                // /streams/air-quality/enabled -> "air-quality"
                key.strip_prefix("/streams/")
                    .and_then(|s| s.split('/').next())
                    .map(String::from)
            })
            .collect();

        Ok(stream_ids.into_iter().collect())
    }

    async fn get_stream(&self, stream_id: &str) -> McpResult<StreamConfig> {
        // GET /streams/{stream_id}/ with prefix
        // Reconstruct StreamConfig from flattened keys
    }

    async fn health_check(&self) -> McpResult<()> {
        self.client.status().await
            .map_err(|e| McpError::ConfigUnavailable(e.to_string()))?;
        Ok(())
    }
}
```

### Startup Behavior

```rust
// main.rs
async fn main() -> Result<()> {
    let etcd_endpoints = env::var("NDP_ETCD_ENDPOINTS")
        .unwrap_or_else(|_| "http://localhost:2379".to_string());

    // Fail fast if etcd unavailable
    let config_store = EtcdConfigStore::new(&[&etcd_endpoints]).await
        .expect("etcd must be available for MCP server to start");

    config_store.health_check().await
        .expect("etcd health check failed");

    info!("Connected to etcd at {}", etcd_endpoints);

    // Continue with server setup...
}
```

### Error Handling

| Scenario | Behavior |
|----------|----------|
| etcd unavailable at startup | Server fails to start with clear error |
| etcd unavailable during request | Tool returns error response |
| Stream not found in etcd | Tool returns "stream not configured" error |
| Malformed etcd data | Tool returns parsing error with context |

## Consequences

### Positive

1. **Validates sync pipeline**: Catches YAML->etcd sync issues
2. **Consistency**: MCP sees same config as running apps
3. **No file parsing**: Faster startup, no YAML dependencies
4. **Already available**: etcd runs as part of NDP stack
5. **Watch capability (future)**: Can detect config changes

### Negative

1. **etcd dependency**: MCP server requires etcd running
   - Mitigation: etcd is core NDP infrastructure

2. **Eventual consistency**: Config changes need sync before visible
   - Mitigation: This matches app behavior (consistency, not a bug)

3. **Flattened key reconstruction**: Must reassemble nested structures
   - Mitigation: One-time parsing logic, well-tested

4. **No offline operation**: Cannot run without etcd
   - Mitigation: Not a real use case for MCP server

### Failure Modes

| Failure | User Experience |
|---------|-----------------|
| etcd down at startup | "MCP server failed to start: etcd unavailable" |
| etcd down during request | Tool error: "Configuration unavailable" |
| Config not synced | Tool shows stale/missing config (matches app) |
| etcd corrupted | Tool error with specific parsing failure |

## Alternatives Considered

### Alternative 1: Read Source YAML Files

**How it works**: Parse `config/base/streams/*/config.yaml` directly.

```rust
impl ConfigStore for YamlConfigStore {
    async fn get_stream(&self, stream_id: &str) -> McpResult<StreamConfig> {
        let path = format!("config/base/streams/{}/config.yaml", stream_id);
        let content = fs::read_to_string(&path)?;
        let config: StreamConfig = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}
```

**Rejected because**:
- Does not validate sync pipeline
- May show different config than running app
- Requires YAML parsing and serde_yaml dependency
- File paths may differ between Pi and dev environments

### Alternative 2: Dual Source with Fallback

**How it works**: Try etcd first, fall back to YAML.

```rust
async fn get_stream(&self, stream_id: &str) -> McpResult<StreamConfig> {
    match self.etcd.get_stream(stream_id).await {
        Ok(config) => Ok(config),
        Err(_) => self.yaml.get_stream(stream_id).await,
    }
}
```

**Rejected because**:
- Masks etcd failures (bad for debugging)
- Inconsistent behavior
- Added complexity
- Defeats the "validate pipeline" goal

### Alternative 3: Embedded Config (Compile-time)

**How it works**: Embed config files in binary.

**Rejected because**:
- Requires rebuild for config changes
- Completely impractical for dynamic configuration

### Alternative 4: HTTP Config Endpoint

**How it works**: Call air-quality-app's config API.

**Rejected because**:
- Creates circular dependency
- Adds network hop
- air-quality-app may not expose full config

## Implementation Notes

### Dependencies

```toml
[dependencies]
etcd-client = "0.14"
```

### etcd Key Structure

Current flattened structure synced by ConfigSyncService:

```
/streams/air-quality/stream_id           = "air-quality"
/streams/air-quality/description         = "Indoor air quality..."
/streams/air-quality/version             = "1.0.0"
/streams/air-quality/enabled             = true
/streams/air-quality/fields/pm25/type    = "float"
/streams/air-quality/fields/pm25/unit    = "ug/m3"
/streams/air-quality/sources/0/type      = "mqtt"
/streams/air-quality/sources/0/enabled   = true
/streams/air-quality/entity_schemas/0/schema_name = "airgradient"
/streams/air-quality/entity_schemas/0/attributes/0/name = "pm25"
```

### Reconstruction Algorithm

```rust
fn reconstruct_config(kvs: Vec<(String, Value)>) -> StreamConfig {
    // Group by path segments
    // Build nested structure from flattened keys
    // Handle arrays (indexed keys like sources/0/type)
}
```

## Related Decisions

- [AIR-003 Configuration](../../air-003/architecture/) - Original etcd design
- [DP-001 ADR-003: GitOps Pattern](../../dp-001/architecture/) - Config sync approach
- [ADR-002: Storage Abstraction](./ADR-002-storage-abstraction.md) - Similar trait pattern

## References

- [etcd-client crate](https://docs.rs/etcd-client/latest/etcd_client/)
- [NDP config-client](../../../config-client/) - Existing etcd wrapper
- [ConfigSyncService](../../../apps/air-quality-app/src/config_sync/) - YAML->etcd sync
