# dp-005: Bronze MCP Server - Test Plan

## Overview

This document defines the comprehensive testing strategy for the Bronze MCP Server (dp-005). The server exposes 4 tools for Bronze layer data exploration and configuration validation, designed to run on Raspberry Pi with <50MB memory overhead.

## Test Strategy

### London School TDD Approach

Following NDP's established testing patterns (see AIR-005-TEST-DESIGN.md), we apply London School TDD principles:

1. **Outside-In Development**: Tests start from MCP protocol level down to storage
2. **Mock-Driven**: Define contracts between components before implementation
3. **Behavior Verification**: Focus on HOW components collaborate
4. **Contract Testing**: Verify MCP protocol compliance and component integration

### Test Pyramid

```
                    ┌─────────────────┐
                    │  System Tests   │  ← Pi deployment, memory constraints
                    │    (Manual)     │
                    ├─────────────────┤
                    │  Integration    │  ← Full MCP flow, etcd+Parquet
                    │    Tests        │
                    ├─────────────────┤
                    │                 │
                    │   Unit Tests    │  ← Each tool in isolation
                    │                 │
                    └─────────────────┘
```

### Test Categories

| Category | Focus | Run Frequency | Infrastructure |
|----------|-------|---------------|----------------|
| **Unit** | Tool functions in isolation | Every commit | None (mocked) |
| **Integration** | MCP protocol + real storage | PR merge | etcd + sample Parquet |
| **System** | Pi deployment validation | Release | Full Pi environment |
| **Performance** | Memory/response time | Weekly/Release | Pi + monitoring |

---

## Unit Tests

### Scope

Test each tool function in isolation with mocked dependencies:
- **BronzeStorage trait** - Mocked for all tools
- **ConfigStore (etcd)** - Mocked for `list_streams`, `describe_schema`, `validate_config`
- **Parquet reader** - Mocked schema/data responses

### Test Organization

```
/core/ndp-mcp-server/src/
├── mcp/tools/
│   ├── list_streams.rs      # Tests in #[cfg(test)] mod tests
│   ├── describe_schema.rs   # Tests in #[cfg(test)] mod tests
│   ├── validate_config.rs   # Tests in #[cfg(test)] mod tests
│   └── sample_data.rs       # Tests in #[cfg(test)] mod tests
├── storage/
│   ├── local.rs             # LocalParquetStorage unit tests
│   └── traits.rs            # BronzeStorage trait definition
└── config/
    └── etcd.rs              # ConfigStore unit tests
```

### Mock Strategy

Using `mockall` crate following NDP patterns:

```rust
use mockall::{automock, predicate::*};

#[automock]
#[async_trait]
pub trait BronzeStorage: Send + Sync {
    async fn list_streams(&self) -> Result<Vec<StreamInfo>, StorageError>;
    async fn get_schema(&self, stream_id: &str) -> Result<ParquetSchema, StorageError>;
    async fn sample(&self, stream_id: &str, n: usize) -> Result<Vec<Row>, StorageError>;
}

#[automock]
#[async_trait]
pub trait ConfigStore: Send + Sync {
    async fn get_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigError>;
    async fn list_stream_ids(&self) -> Result<Vec<String>, ConfigError>;
    async fn health_check(&self) -> Result<(), ConfigError>;
}
```

### Unit Test Coverage Targets

| Component | Target | Priority | Rationale |
|-----------|--------|----------|-----------|
| `list_streams` | 90% | High | Core discovery tool |
| `describe_schema` | 90% | High | Complex mode logic |
| `validate_config` | 85% | High | Field comparison logic |
| `sample_data` | 80% | Medium | Simpler data retrieval |
| `LocalParquetStorage` | 80% | High | Core storage implementation |
| `ConfigStore` | 75% | Medium | etcd client wrapper |
| MCP protocol handler | 70% | Medium | JSON-RPC routing |

### Test Patterns

#### Behavior Verification Pattern

```rust
#[tokio::test]
async fn test_list_streams_queries_both_sources() {
    // ARRANGE
    let mut mock_config = MockConfigStore::new();
    let mut mock_storage = MockBronzeStorage::new();

    mock_config.expect_list_stream_ids()
        .times(1)
        .returning(|| Ok(vec!["air-quality".to_string()]));

    mock_storage.expect_list_streams()
        .times(1)
        .returning(|| Ok(vec![StreamInfo { ... }]));

    let tool = ListStreamsTool::new(
        Arc::new(mock_config),
        Arc::new(mock_storage)
    );

    // ACT
    let result = tool.execute().await;

    // ASSERT
    assert!(result.is_ok());
    // Mock expectations verified automatically on drop
}
```

#### Error Path Pattern

```rust
#[tokio::test]
async fn test_describe_schema_stream_not_found() {
    // ARRANGE
    let mut mock_storage = MockBronzeStorage::new();
    mock_storage.expect_get_schema()
        .with(eq("nonexistent"))
        .returning(|_| Err(StorageError::StreamNotFound("nonexistent".to_string())));

    let tool = DescribeSchemaTool::new(Arc::new(mock_storage), ...);

    // ACT
    let result = tool.execute(DescribeSchemaInput {
        stream_id: "nonexistent".to_string(),
        mode: "all".to_string(),
    }).await;

    // ASSERT
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ToolError::StreamNotFound(_)));
}
```

---

## Integration Tests

### Scope

Test full MCP protocol flow with real (test) infrastructure:
- Real Parquet file reading
- Real etcd interactions (test instance)
- MCP JSON-RPC protocol compliance
- Response format validation

### Test Organization

```
/core/ndp-mcp-server/tests/
├── integration/
│   ├── mcp_protocol_test.rs    # MCP handshake, tools/list, tools/call
│   ├── list_streams_test.rs    # Full list_streams flow
│   ├── describe_schema_test.rs # Full describe_schema flow
│   ├── validate_config_test.rs # Full validate_config flow
│   └── sample_data_test.rs     # Full sample_data flow
└── fixtures/
    ├── sample_air_quality.parquet
    ├── sample_outdoor_weather.parquet
    └── etcd_test_config.yaml
```

### Integration Test Requirements

| Requirement | Implementation |
|-------------|---------------|
| etcd availability | Use `testcontainers` or skip with `#[ignore]` |
| Test Parquet files | Pre-generated fixtures in `tests/fixtures/` |
| Isolation | Each test uses unique stream IDs |
| Cleanup | Automatic teardown after each test |

### Integration Test Pattern

```rust
#[tokio::test]
#[ignore] // Requires etcd
async fn test_full_mcp_tools_list_flow() {
    // ARRANGE - Setup test environment
    let etcd = setup_test_etcd().await;
    let storage_path = setup_test_parquet().await;

    let server = McpServer::new(McpConfig {
        etcd_endpoints: vec![etcd.endpoint()],
        bronze_path: storage_path,
        ..Default::default()
    }).await.unwrap();

    // ACT - Send MCP tools/list request
    let request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list"
    });

    let response = server.handle_request(request).await;

    // ASSERT - Verify MCP protocol compliance
    assert_eq!(response["jsonrpc"], "2.0");
    assert!(response["result"]["tools"].is_array());
    assert_eq!(response["result"]["tools"].as_array().unwrap().len(), 4);
}
```

---

## System Tests

### Scope

Validate server operation on target Raspberry Pi environment:
- Memory footprint <50MB
- Response times acceptable
- Graceful degradation when dependencies unavailable
- Startup/shutdown behavior

### System Test Checklist

| Test | Pass Criteria | Method |
|------|---------------|--------|
| ST-001: Memory baseline | <50MB RSS at startup | `ps aux` / Prometheus |
| ST-002: Memory under load | <60MB with 100 concurrent requests | Load test + monitoring |
| ST-003: Response time | p95 < 500ms for all tools | Load test metrics |
| ST-004: etcd unavailable | Server fails fast, clear error | Kill etcd, observe behavior |
| ST-005: Empty Bronze dir | `list_streams` returns empty array | Fresh deployment |
| ST-006: Large Parquet file | `sample_data` handles 1GB+ files | Test fixture |
| ST-007: Graceful shutdown | Completes in-flight requests | SIGTERM during load |

### Performance Test Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| Startup time | <5s | Time from process start to healthy |
| `list_streams` p95 | <200ms | With 10 streams |
| `describe_schema` p95 | <300ms | 100MB Parquet file |
| `sample_data(n=100)` p95 | <400ms | 100MB Parquet file |
| `validate_config` p95 | <300ms | Complex config |
| Memory (idle) | <30MB | RSS after startup |
| Memory (active) | <50MB | RSS during requests |

---

## Test Execution

### Running Unit Tests

```bash
# All unit tests
cd /workspaces/neural-data-platform
cargo test --package ndp-mcp-server --lib

# Specific tool tests
cargo test --package ndp-mcp-server list_streams::tests
cargo test --package ndp-mcp-server describe_schema::tests

# With output
cargo test --package ndp-mcp-server -- --nocapture
```

### Running Integration Tests

```bash
# Start test infrastructure
docker-compose -f deploy/test/docker-compose.yml up -d etcd

# Run integration tests
cargo test --package ndp-mcp-server --test '*' -- --ignored

# Or use testcontainers (auto-starts etcd)
TESTCONTAINERS=1 cargo test --package ndp-mcp-server --test '*'
```

### Running System Tests

```bash
# Deploy to Pi
./deploy/pi/deploy.sh deploy ndp-mcp-server

# Run system test suite
./scripts/system-tests/run-mcp-tests.sh

# Performance test
./scripts/perf-tests/mcp-load-test.sh --duration 60s --concurrency 10
```

---

## Test Dependencies

### Cargo.toml dev-dependencies

```toml
[dev-dependencies]
tokio = { version = "1", features = ["test-util", "macros", "rt-multi-thread"] }
mockall = "0.12"
wiremock = "0.6"
testcontainers = "0.15"
testcontainers-modules = { version = "0.3", features = ["etcd"] }
tempfile = "3"
assert_json_diff = "2"
criterion = "0.5"  # For benchmarks
```

---

## Test Coverage Analysis

### Coverage by Category

| Category | Test Count | Coverage Focus |
|----------|------------|----------------|
| Unit - list_streams | 4 | Stream enumeration, metadata |
| Unit - describe_schema | 6 | Mode logic, schema extraction |
| Unit - validate_config | 5 | Field comparison, gap detection |
| Unit - sample_data | 4 | Row retrieval, envelope structure |
| Unit - Storage | 6 | Parquet reading, file discovery |
| Unit - Config | 4 | etcd client, key parsing |
| Integration - MCP | 8 | Protocol compliance |
| Integration - Tools | 8 | End-to-end flows |
| **Total** | **45** | |

### Risk-Based Test Priority

| Risk Area | Mitigation Tests |
|-----------|------------------|
| etcd unavailable at startup | TC-003, TC-013, ST-004 |
| Large Parquet files | TC-031, ST-006 |
| Malformed config | TC-021, TC-022, TC-023 |
| Memory pressure | ST-001, ST-002 |
| MCP protocol errors | Integration MCP tests |

---

## Test Data Management

### Fixture Strategy

1. **Pre-generated Parquet files** in `tests/fixtures/`
   - Small but representative (10-100 rows each)
   - Cover all Bronze envelope columns
   - Include various `raw_payload` structures

2. **etcd test configs**
   - YAML files synced to test etcd
   - Cover all `entity_schemas` variations
   - Include enabled/disabled streams

3. **Mock response generators**
   - Rust helper functions for consistent test data
   - Configurable for edge cases

### Fixture Files

See `test-fixtures.md` for detailed fixture specifications.

---

## Continuous Integration

### CI Pipeline Stages

```yaml
test:
  stage: test
  script:
    - cargo test --package ndp-mcp-server --lib

integration:
  stage: integration
  services:
    - etcd:v3.5
  script:
    - cargo test --package ndp-mcp-server --test '*' -- --ignored

system:
  stage: system
  when: manual  # Requires Pi hardware
  script:
    - ./scripts/system-tests/run-mcp-tests.sh
```

---

## Future Enhancements

### Phase 2 Testing Additions

- [ ] Type validation tests (when implemented)
- [ ] Value constraint tests (when implemented)
- [ ] SQL query tool tests
- [ ] Authentication/authorization tests
- [ ] SSE transport tests

### Test Tooling

- [ ] Property-based testing with `proptest`
- [ ] Chaos engineering tests (network partitions)
- [ ] Benchmarks with `criterion`

---

## Related Documents

- `test-cases.md` - Detailed test case specifications
- `test-fixtures.md` - Test data and fixture definitions
- `/docs/testing/AIR-005-TEST-DESIGN.md` - NDP testing patterns
- `/product/features/dp-005/SCOPE.md` - Feature specification
