# dp-005: Bronze MCP Server - Implementation Phases

## Overview

This document defines the phased implementation approach for the Bronze MCP Server. Each phase delivers incremental, testable functionality and can be deployed independently.

---

## Phase Summary

| Phase | Scope | Duration | Deliverable |
|-------|-------|----------|-------------|
| 1 | Core Server + list_streams + sample_data | 2-3 days | Basic data exploration |
| 2 | describe_schema (all modes) | 1-2 days | Schema discovery |
| 3 | validate_config | 1 day | Config validation |
| 4 | Performance + Metrics | 1 day | Production-ready |

**Total Estimated Duration**: 5-7 days

---

## Phase 1: Core Server + Basic Tools

### Objective

Establish the MCP server foundation with two essential tools for immediate data exploration.

### Deliverables

1. **HTTP Server (axum)**
   - POST `/mcp` - MCP JSON-RPC handler
   - GET `/health` - Health check endpoint
   - Structured logging with tracing

2. **MCP Protocol Handler**
   - `initialize` - Return server capabilities
   - `tools/list` - Return tool definitions
   - `tools/call` - Route to implementations

3. **Tool: list_streams**
   - Enumerate streams from etcd
   - Include enabled/disabled status
   - Add storage metadata (latest partition, file size)

4. **Tool: sample_data**
   - Read N rows from latest partition
   - Return full Bronze envelope
   - Support configurable row limit (default 10, max 100)

5. **Storage Layer**
   - `BronzeStorage` trait definition
   - `LocalParquetStorage` implementation
   - Hive-style partition discovery

6. **Config Layer**
   - etcd client wrapper
   - Stream config parsing
   - Connection pooling

### Entry Criteria

- Architecture ADRs approved
- Pseudocode reviewed
- Development environment ready (etcd, sample Parquet files)

### Exit Criteria

- [ ] Server starts and accepts connections
- [ ] `tools/list` returns 2 tool definitions
- [ ] `list_streams` shows all configured streams
- [ ] `sample_data` returns valid rows
- [ ] Health endpoint responds
- [ ] Unit tests: 80% coverage
- [ ] Integration tests: list_streams, sample_data

### Files Created

```
/core/ndp-mcp-server/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── server.rs
│   ├── config.rs
│   ├── mcp/
│   │   ├── mod.rs
│   │   ├── protocol.rs
│   │   ├── handler.rs
│   │   └── tools/
│   │       ├── mod.rs
│   │       ├── list_streams.rs
│   │       └── sample_data.rs
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── traits.rs
│   │   └── local.rs
│   └── etcd/
│       ├── mod.rs
│       └── client.rs
└── tests/
    ├── integration/
    │   ├── mod.rs
    │   ├── list_streams_test.rs
    │   └── sample_data_test.rs
    └── fixtures/
        └── test_data.parquet
```

### Key Implementation Notes

**MCP Protocol Types**:
```rust
#[derive(Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(flatten)]
    pub result: McpResult,
}
```

**BronzeStorage Trait**:
```rust
#[async_trait]
pub trait BronzeStorage: Send + Sync {
    async fn list_streams(&self) -> Result<Vec<StreamInfo>>;
    async fn sample(&self, stream_id: &str, n: usize) -> Result<Vec<Row>>;
    async fn schema(&self, stream_id: &str) -> Result<ParquetSchema>;
    async fn latest_partition(&self, stream_id: &str) -> Result<Option<PartitionPath>>;
}
```

---

## Phase 2: Schema Discovery

### Objective

Enable comprehensive schema discovery with multi-mode describe_schema tool.

### Deliverables

1. **Tool: describe_schema (source mode)**
   - Introspect `raw_payload` structure from Parquet
   - Extract nested JSON keys
   - Return parser field mappings from config

2. **Tool: describe_schema (target mode)**
   - Return entity_schemas from etcd
   - Include attribute types and units

3. **Tool: describe_schema (all mode)**
   - Combine source and target views
   - Generate gap_analysis (unmapped fields)

4. **JSON Introspection**
   - Parse `raw_payload` JSON column
   - Build nested key structure
   - Handle varying schemas across rows

### Entry Criteria

- Phase 1 complete and deployed
- Sample data contains representative `raw_payload` structures

### Exit Criteria

- [ ] describe_schema(mode=source) works correctly
- [ ] describe_schema(mode=target) works correctly
- [ ] describe_schema(mode=all) includes gap_analysis
- [ ] Handles nested JSON (main.temp, wind.speed)
- [ ] Integration tests for all modes

### Files Created/Modified

```
src/mcp/tools/describe_schema.rs  # New
src/schema/                        # New module
├── mod.rs
├── introspection.rs               # Parquet/JSON introspection
└── gap_analysis.rs                # Source vs target comparison
```

### Key Implementation Notes

**JSON Key Extraction**:
```rust
fn extract_json_structure(payload: &Value) -> JsonStructure {
    match payload {
        Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            let nested: HashMap<String, Vec<String>> = map.iter()
                .filter_map(|(k, v)| match v {
                    Value::Object(inner) => Some((
                        k.clone(),
                        inner.keys().cloned().collect()
                    )),
                    _ => None
                })
                .collect();
            JsonStructure { keys, nested }
        }
        _ => JsonStructure::default()
    }
}
```

**Gap Analysis**:
```rust
pub struct GapAnalysis {
    pub unmapped_source_fields: Vec<String>,
    pub target_fields_without_mapping: Vec<String>,
}

fn analyze_gaps(source_keys: &[String], mappings: &[FieldMapping], target_attrs: &[Attribute]) -> GapAnalysis {
    // Find source keys with no mapping
    // Find target attrs with no mapping
}
```

---

## Phase 3: Configuration Validation

### Objective

Enable config-to-data comparison for detecting schema drift and misconfigurations.

### Deliverables

1. **Tool: validate_config**
   - Compare entity_schema attributes vs raw_payload keys
   - Return structured diff (in_config_not_in_payload, in_payload_not_in_config)
   - Include status (match/mismatch)
   - Add contextual notes

2. **Validation Logic**
   - Field name comparison only (MVP)
   - Handle nested paths in config vs flat keys in data
   - Aggregate results across sample rows

### Entry Criteria

- Phase 2 complete
- Schema introspection working correctly

### Exit Criteria

- [ ] validate_config detects all mismatches
- [ ] Returns structured pretty JSON diff
- [ ] Handles missing streams gracefully
- [ ] Integration tests for match/mismatch scenarios

### Files Created/Modified

```
src/mcp/tools/validate_config.rs  # New
src/validation/                    # New module
├── mod.rs
└── field_comparison.rs
```

### Key Implementation Notes

**Validation Result**:
```rust
#[derive(Serialize)]
pub struct ValidationResult {
    pub status: ValidationStatus,
    pub config_fields: Vec<String>,
    pub raw_payload_fields: Vec<String>,
    pub analysis: FieldAnalysis,
    pub notes: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Match,
    Mismatch,
}

#[derive(Serialize)]
pub struct FieldAnalysis {
    pub in_config_not_in_payload: Vec<String>,
    pub in_payload_not_in_config: Vec<String>,
    pub matching: Vec<String>,
}
```

---

## Phase 4: Performance Optimization & Deployment

### Objective

Ensure production-ready performance, observability, and containerized deployment.

### Deliverables

1. **Performance Tuning**
   - Connection pooling for etcd
   - Parquet read optimization (column projection)
   - Response caching (optional, short TTL)

2. **Observability**
   - Structured logging (JSON format)
   - Request tracing (correlation IDs)
   - Metrics preparation (counters, histograms)

3. **Resilience**
   - Graceful shutdown
   - Request timeouts
   - Connection limits

4. **Documentation**
   - API documentation
   - Deployment guide
   - CLAUDE.md integration

5. **Containerized Deployment**
   - Dockerfile creation (multi-stage build)
   - docker-compose.yml service definition
   - deploy.sh integration for health monitoring

### Entry Criteria

- Phases 1-3 complete
- Performance baseline measured

### Exit Criteria

- [ ] Memory usage < 50 MB on Pi
- [ ] All response time targets met
- [ ] 24-hour soak test passes (no memory growth)
- [ ] Graceful shutdown works
- [ ] Deployment documentation complete
- [ ] Dockerfile builds successfully
- [ ] Container runs on Pi ARM64
- [ ] docker-compose service starts with dependencies
- [ ] Health check passes in docker-compose
- [ ] deploy.sh status shows MCP server health

### Files Created/Modified

```
src/observability/                    # New module
├── mod.rs
├── logging.rs
└── tracing.rs

src/server.rs                         # Add middleware, timeouts

docs/dp-005/
├── deployment.md
└── api.md

core/ndp-mcp-server/Dockerfile        # Multi-stage build
deploy/pi/docker-compose.yml          # Add ndp-mcp-server service
```

### Key Implementation Notes

**Graceful Shutdown**:
```rust
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C handler");
    tracing::info!("Shutdown signal received, draining connections...");
}

// In main
axum::serve(listener, app)
    .with_graceful_shutdown(shutdown_signal())
    .await?;
```

**Connection Pooling**:
```rust
// etcd client is already connection-pooled
// For Parquet, we open files on-demand (no persistent connections)
// Consider caching file handles for frequently accessed streams
```

**Dockerfile Strategy**:
- Multi-stage build: builder (rust:1.75) -> runtime (debian:bookworm-slim)
- Copy only binary to runtime stage
- Install minimal runtime deps (ca-certificates, libssl3)
- Target: < 50MB final image size

```dockerfile
# Stage 1: Build
FROM rust:1.75 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin ndp-mcp-server

# Stage 2: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/ndp-mcp-server /usr/local/bin/
EXPOSE 3002
HEALTHCHECK --interval=30s --timeout=5s CMD curl -f http://localhost:3002/health || exit 1
CMD ["ndp-mcp-server"]
```

---

## Dependency Graph

```
Phase 1 (Core + list_streams + sample_data)
    │
    ├──► Phase 2 (describe_schema)
    │       │
    │       └──► Phase 3 (validate_config)
    │
    └──► Phase 4 (Performance) ◄── Can start after Phase 1
```

**Notes**:
- Phase 4 can begin in parallel with Phases 2-3
- Each phase is independently deployable
- Rollback to previous phase possible

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Parquet file size too large | Implement row limit, column projection |
| etcd latency spikes | Add timeouts, circuit breaker pattern |
| Memory pressure on Pi | Profile early, optimize hot paths |
| JSON parsing performance | Use simd-json or streaming parser if needed |

---

## Testing Strategy Per Phase

### Phase 1
- Unit: Tool logic, storage trait
- Integration: Full request/response flow
- Manual: Claude Code connectivity

### Phase 2
- Unit: JSON introspection, gap analysis
- Integration: describe_schema all modes
- Manual: Compare against actual Parquet

### Phase 3
- Unit: Validation logic
- Integration: Match/mismatch detection
- Manual: Create known-bad configs

### Phase 4
- Load: wrk benchmark, P95 latencies
- Soak: 24-hour memory stability
- Chaos: Kill etcd during requests

---

## Rollout Plan

### Stage 1: Development
- Local docker build and test
- Deploy each phase locally first
- Full integration test suite
- Verify Dockerfile builds: `docker build -t ndp-mcp-server:dev .`
- Run local container: `docker run -p 3002:3002 ndp-mcp-server:dev`

### Stage 2: Pi Staging
- Deploy to Pi, validate ARM64 build
- Build on Pi: `docker build --platform linux/arm64 -t ndp-mcp-server:arm64 .`
- Validate memory/performance targets (< 50MB, < 100ms P95)
- Test etcd connectivity from container
- Verify Parquet volume mounts work correctly

### Stage 3: Production
- Enable in docker-compose with full service definition
- Verify with deploy.sh: `./deploy/pi/deploy.sh status`
- Monitor initial usage patterns
- Confirm health check passes: `curl http://localhost:3002/health`

### Stage 4: Integration
- Add to Claude Code `.claude/mcp.json`
- Update CLAUDE.md with MCP server usage guidance
- Validate Claude Code can connect to MCP server
- Document available tools and example usage

---

*Implementation phases defined for dp-005*
