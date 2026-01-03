# ADR-002: BronzeStorage Trait Abstraction

## Status

Accepted

## Date

2026-01-03

## Context

The dp-005 Bronze MCP Server needs to read Parquet files from the Bronze layer. The current deployment stores files locally on the Raspberry Pi, but future deployments may use cloud object storage (S3, GCS).

### Current Bronze Layout

```
/data/raw/{stream_id}/
└── year=YYYY/
    └── month=MM/
        └── day=DD/
            └── data.parquet
```

Example: `/data/raw/outdoor-weather/year=2026/month=01/day=03/data.parquet`

### Requirements

| Requirement | Priority | Notes |
|-------------|----------|-------|
| Read Parquet schema | Must | `describe_schema` tool |
| List available streams | Must | `list_streams` tool |
| Sample data rows | Must | `sample_data` tool |
| Partition discovery | Must | Find latest data |
| Cloud storage support | Should | S3/GCS in future |
| Caching | Nice | Reduce I/O for repeated queries |

### Domain Adapter Pattern

Following the NDP Domain Adapter Pattern (hexagonal architecture):
- **Port**: `BronzeStorage` trait defines the interface
- **Adapter**: `LocalParquetStorage` implements for local filesystem
- **Future Adapter**: `S3ParquetStorage` for cloud deployment

## Decision

**Define a `BronzeStorage` trait with four core methods that abstract away storage location details.**

### Trait Definition

```rust
use async_trait::async_trait;
use crate::error::McpResult;
use arrow::datatypes::Schema;
use serde_json::Value;

/// Metadata about a Bronze stream's storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStorageInfo {
    pub stream_id: String,
    pub latest_partition: Option<String>,  // e.g., "year=2026/month=01/day=03"
    pub file_size_bytes: Option<u64>,
    pub file_modified: Option<DateTime<Utc>>,
    pub row_count: Option<u64>,
}

/// Bronze layer storage abstraction (Port)
#[async_trait]
pub trait BronzeStorage: Send + Sync {
    /// List all streams that have data in Bronze storage
    /// Returns stream IDs and basic storage metadata
    async fn list(&self) -> McpResult<Vec<StreamStorageInfo>>;

    /// Get the Parquet schema for a specific stream
    /// Returns Arrow schema with column names and types
    async fn schema(&self, stream_id: &str) -> McpResult<Schema>;

    /// Sample N rows from the most recent partition of a stream
    /// Returns rows as JSON array
    async fn sample(&self, stream_id: &str, n: usize) -> McpResult<Vec<Value>>;

    /// Validate that Bronze storage is accessible
    /// Used for health checks and startup validation
    async fn validate(&self) -> McpResult<()>;
}
```

### Local Implementation

```rust
pub struct LocalParquetStorage {
    base_path: PathBuf,  // e.g., /data/raw
}

impl LocalParquetStorage {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// Find the most recent partition for a stream
    fn find_latest_partition(&self, stream_id: &str) -> Option<PathBuf> {
        // Walk directory tree to find newest year/month/day
        // Returns path to data.parquet
    }
}

#[async_trait]
impl BronzeStorage for LocalParquetStorage {
    async fn list(&self) -> McpResult<Vec<StreamStorageInfo>> {
        // 1. List subdirectories in base_path
        // 2. For each, check if it has partition directories
        // 3. Return stream IDs with latest partition info
    }

    async fn schema(&self, stream_id: &str) -> McpResult<Schema> {
        // 1. Find latest partition
        // 2. Open Parquet file
        // 3. Read and return Arrow schema
    }

    async fn sample(&self, stream_id: &str, n: usize) -> McpResult<Vec<Value>> {
        // 1. Find latest partition
        // 2. Open Parquet file
        // 3. Read last N rows
        // 4. Convert to JSON
    }

    async fn validate(&self) -> McpResult<()> {
        // 1. Check base_path exists
        // 2. Check at least one stream directory exists
        // 3. Check read permissions
    }
}
```

### Future Cloud Implementation

```rust
/// Future: S3-backed Bronze storage
pub struct S3ParquetStorage {
    bucket: String,
    prefix: String,  // e.g., "bronze/"
    client: aws_sdk_s3::Client,
}

#[async_trait]
impl BronzeStorage for S3ParquetStorage {
    async fn list(&self) -> McpResult<Vec<StreamStorageInfo>> {
        // Use S3 ListObjectsV2 with delimiter for stream discovery
    }

    async fn schema(&self, stream_id: &str) -> McpResult<Schema> {
        // Use object_store crate for range requests
        // Read only Parquet metadata (footer)
    }

    async fn sample(&self, stream_id: &str, n: usize) -> McpResult<Vec<Value>> {
        // Download latest partition to temp file
        // Or use object_store with range requests
    }

    async fn validate(&self) -> McpResult<()> {
        // HEAD bucket or list with max_keys=1
    }
}
```

### Dependency Injection

The MCP server receives storage implementation at startup:

```rust
pub struct AppState {
    pub storage: Arc<dyn BronzeStorage>,
    pub config: Arc<dyn ConfigStore>,
}

// In main.rs
let storage: Arc<dyn BronzeStorage> = match env::var("NDP_STORAGE_TYPE") {
    Ok(t) if t == "s3" => Arc::new(S3ParquetStorage::new(...)),
    _ => Arc::new(LocalParquetStorage::new(raw_path)),
};

let state = AppState { storage, config };
let app = Router::new()
    .route("/mcp", post(mcp_handler))
    .with_state(state);
```

## Consequences

### Positive

1. **Storage agnostic**: Same MCP tools work on Pi or cloud
2. **Testable**: Mock `BronzeStorage` for unit tests
3. **Consistent with NDP patterns**: Follows Domain Adapter pattern
4. **Clear interface**: Four methods cover all MVP needs
5. **Future-proof**: S3/GCS implementation straightforward to add

### Negative

1. **Abstraction overhead**: Trait dispatch vs direct filesystem calls
   - Mitigation: Negligible compared to I/O costs

2. **Different performance characteristics**: S3 vs local disk
   - Mitigation: Document expected latencies per implementation

3. **Cloud implementation complexity**: S3 pagination, range requests
   - Mitigation: Use `object_store` crate for unified interface

### Method Rationale

| Method | Purpose | MVP Tool |
|--------|---------|----------|
| `list()` | Stream discovery | `list_streams` |
| `schema()` | Parquet introspection | `describe_schema` |
| `sample()` | Data preview | `sample_data` |
| `validate()` | Health checks | Startup, `/health` |

### Not Included (Out of Scope)

These methods were considered but excluded from MVP:

| Method | Why Excluded |
|--------|--------------|
| `query(sql)` | Complex, Silver layer concern |
| `write(...)` | MCP server is read-only |
| `delete(...)` | MCP server is read-only |
| `list_partitions()` | Too low-level for MCP tools |

## Alternatives Considered

### Alternative 1: Direct Parquet Crate Usage

**How it works**: Call `parquet::file::reader` directly in tool handlers.

```rust
async fn list_streams() -> McpResult<Response> {
    let paths = std::fs::read_dir("/data/raw")?;
    // ... direct filesystem operations
}
```

**Rejected because**:
- No abstraction for cloud storage
- Harder to test (requires real filesystem)
- Duplicated path logic across tools
- Violates Domain Adapter pattern

### Alternative 2: object_store as Primary Abstraction

**How it works**: Use `object_store` crate directly as the interface.

```rust
use object_store::{ObjectStore, local::LocalFileSystem};

pub struct BronzeAccess {
    store: Arc<dyn ObjectStore>,
}
```

**Rejected because**:
- `ObjectStore` is too generic (get/put bytes)
- Still need Parquet-specific methods (schema, sample)
- Would duplicate partition logic
- Our trait is higher-level and domain-specific

### Alternative 3: Repository Pattern with Query Objects

**How it works**: Generic repository with query builders.

```rust
pub trait BronzeRepository {
    fn find(&self, query: BronzeQuery) -> BronzeResult;
}
```

**Rejected because**:
- Over-engineered for four simple operations
- Query object complexity not justified
- MCP tools have fixed, predictable access patterns

## Implementation Notes

### Dependencies

```toml
[dependencies]
parquet = "53"
arrow = "53"
async-trait = "0.1"

# Future (optional)
object_store = { version = "0.11", features = ["aws"], optional = true }
```

### Error Handling

Storage errors map to MCP responses:

| Error | MCP Response |
|-------|--------------|
| Stream not found | `{"success": false, "error": "Stream 'foo' not found"}` |
| Permission denied | `{"success": false, "error": "Storage access denied"}` |
| Corrupted Parquet | `{"success": false, "error": "Invalid Parquet file: ..."}` |
| I/O error | `{"success": false, "error": "Storage I/O error: ..."}` |

### Caching Strategy (Future)

For frequently accessed metadata:

```rust
pub struct CachedBronzeStorage<S: BronzeStorage> {
    inner: S,
    schema_cache: RwLock<HashMap<String, (Schema, Instant)>>,
    ttl: Duration,
}
```

## Related Decisions

- [ADR-001: MCP Transport](./ADR-001-mcp-transport.md) - How server is accessed
- [ADR-004: Schema Discovery](./ADR-004-schema-discovery.md) - Parquet introspection strategy
- [DP-004 ADR-001: Bronze Schema](../dp-004/architecture/ADR-001-bronze-raw-json-schema.md) - Bronze schema design

## References

- [Domain Adapter Pattern](../../../docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md) - NDP architecture
- [Apache Arrow Rust](https://docs.rs/arrow/latest/arrow/) - Schema types
- [object_store crate](https://docs.rs/object_store/latest/object_store/) - Cloud storage abstraction
- [NDP traits.rs](../../../core/src/traits.rs) - Existing trait patterns
