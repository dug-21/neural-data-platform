# dp-005: Bronze MCP Server - Test Strategy

**Status**: COMPLETE
**Phase**: SPARC Refinement (R)
**Created**: 2026-01-03

---

## Testing Philosophy

### TDD Approach

The Bronze MCP Server follows London School TDD (Outside-In) principles:

1. **Outside-In Development**: Tests start from MCP protocol level, driving down to storage implementation
2. **Mock-Driven Design**: Define contracts between components before implementation
3. **Behavior Verification**: Focus on HOW components collaborate, not just outcomes
4. **Contract Testing**: Verify MCP protocol compliance at integration level

### Test Pyramid

```
                    +---------------------+
                    |   E2E Tests (5%)    |  Pi deployment, Claude Code
                    +---------------------+
                    |                     |
                    | Integration (20%)   |  MCP protocol + etcd + Parquet
                    |                     |
                    +---------------------+
                    |                     |
                    |                     |
                    |  Unit Tests (75%)   |  Tool logic, storage, config
                    |                     |
                    |                     |
                    +---------------------+
```

### Quality Gates

All code must pass these gates before merge:

| Gate | Tool | Threshold |
|------|------|-----------|
| Lint | `cargo clippy` | Zero warnings |
| Format | `cargo fmt --check` | Pass |
| Unit Tests | `cargo test` | 100% pass |
| Coverage | `cargo-tarpaulin` | 85% minimum |
| Performance | Benchmarks | Within targets |

---

## Test Levels

### Level 1: Unit Tests

**Scope**: Individual functions and methods in isolation

**Location**: `#[cfg(test)] mod tests` blocks within source files

**Characteristics**:
- Fast execution (< 100ms per test)
- No external dependencies (etcd, filesystem)
- All dependencies mocked via `mockall`
- Deterministic results

**Test Structure**:

```
/core/ndp-mcp-server/src/
+-- mcp/tools/
|   +-- list_streams.rs        # Unit tests for stream listing
|   +-- describe_schema.rs     # Unit tests for schema modes
|   +-- validate_config.rs     # Unit tests for field comparison
|   +-- sample_data.rs         # Unit tests for row retrieval
+-- storage/
|   +-- local.rs               # LocalParquetStorage unit tests
|   +-- traits.rs              # BronzeStorage trait (mocked)
+-- config/
    +-- etcd.rs                # ConfigStore unit tests
```

**Mock Strategy**:

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

### Level 2: Integration Tests

**Scope**: Component interactions with real (test) infrastructure

**Location**: `/core/ndp-mcp-server/tests/integration/`

**Characteristics**:
- Use testcontainers for etcd
- Use real Parquet fixture files
- Test full MCP JSON-RPC protocol
- Verify response format compliance

**Test Structure**:

```
/core/ndp-mcp-server/tests/
+-- integration/
|   +-- mcp_protocol_test.rs    # MCP handshake, tools/list
|   +-- list_streams_test.rs    # Full list_streams flow
|   +-- describe_schema_test.rs # Full describe_schema flow
|   +-- validate_config_test.rs # Full validate_config flow
|   +-- sample_data_test.rs     # Full sample_data flow
+-- fixtures/
    +-- air-quality/year=2026/month=01/day=03/data.parquet
    +-- outdoor-weather/year=2026/month=01/day=03/data.parquet
    +-- sparse-stream/year=2026/month=01/day=03/data.parquet
```

**Infrastructure Requirements**:

| Component | Approach | Skip Condition |
|-----------|----------|----------------|
| etcd | testcontainers-modules | `#[ignore]` if unavailable |
| Parquet files | Pre-generated fixtures | Always available |
| Filesystem | tempfile crate | Always available |

### Level 3: End-to-End Tests

**Scope**: Full system validation on target environment

**Location**: `/scripts/e2e-tests/`

**Characteristics**:
- Run on actual Raspberry Pi hardware
- Verify memory constraints (< 50 MB)
- Validate Claude Code connectivity
- Manual trigger (release pipeline)

**Test Categories**:

| Category | Tests | Environment |
|----------|-------|-------------|
| Deployment | Container builds, starts, responds | Pi + Docker |
| Performance | Memory, latency under load | Pi + monitoring |
| Integration | Claude Code tool invocation | Pi + Claude |
| Reliability | Graceful shutdown, error recovery | Pi + chaos |

---

## Test Categories

### Functional Tests

**Purpose**: Verify each tool produces correct output

| Tool | Test Cases | Coverage |
|------|------------|----------|
| `list_streams` | TC-LS-001 to TC-LS-004 | Stream enumeration, metadata, empty state |
| `describe_schema` | TC-DS-010 to TC-DS-015 | All 3 modes, gap analysis, edge cases |
| `validate_config` | TC-VC-020 to TC-VC-024 | Field comparison, nested structures |
| `sample_data` | TC-SD-030 to TC-SD-035 | Row retrieval, limits, envelope format |

**Error Handling**:

| Scenario | Expected Behavior | Test |
|----------|-------------------|------|
| Stream not found | STREAM_NOT_FOUND error | TC-DS-015, TC-VC-024 |
| etcd unavailable | CONFIG_UNAVAILABLE, < 1s | TC-LS-003 |
| Parquet missing | STORAGE_UNAVAILABLE | TC-DS-013 |
| Invalid mode | Descriptive error message | Unit test |
| n > 100 | Capped at 100 with note | TC-SD-035 |

### Performance Tests

**Purpose**: Ensure response times and memory within targets

**Targets** (from success-criteria.md):

| Metric | Target | Measurement |
|--------|--------|-------------|
| `list_streams` P95 | < 100 ms | 100 requests, cold cache |
| `describe_schema` P95 | < 150 ms | Combined modes |
| `validate_config` P95 | < 200 ms | Nested config |
| `sample_data(10)` P95 | < 500 ms | 10 MB Parquet |
| `sample_data(100)` P95 | < 1000 ms | 10 MB Parquet |
| Memory (idle) | < 30 MB | RSS after startup |
| Memory (active) | < 50 MB | Under concurrent load |
| Startup | < 5 seconds | Process ready |

**Benchmark Suite**:

```rust
// benches/tool_benchmarks.rs
use criterion::{criterion_group, criterion_main, Criterion};

fn benchmark_list_streams(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(setup_test_server());

    c.bench_function("list_streams", |b| {
        b.iter(|| {
            rt.block_on(server.call_tool("list_streams", json!({})))
        });
    });
}
```

### Reliability Tests

**Purpose**: Verify error recovery and stability

| Scenario | Test Method | Pass Criteria |
|----------|-------------|---------------|
| etcd unavailable | Kill etcd container | Error in < 1s, no hang |
| Parquet corrupted | Malformed fixture | Structured error returned |
| Concurrent load | 100 parallel requests | No panics, memory stable |
| Long-running | 24-hour soak test | RSS growth < 5 MB |
| Graceful shutdown | SIGTERM during requests | In-flight requests complete |

### Portability Tests

**Purpose**: Verify environment configuration works correctly

| Test | Verification |
|------|--------------|
| Custom NDP_RAW_PATH | Server reads from `/tmp/test-raw` |
| Custom etcd endpoint | Server connects to alternative host |
| Custom listen port | Server binds to specified port |
| Storage abstraction | LocalParquetStorage implements trait |

---

## Deployment Tests

### Container Build Tests

| Test | Description | Pass Criteria |
|------|-------------|---------------|
| DT-001 | Dockerfile builds | `docker build` succeeds |
| DT-002 | ARM64 cross-compile | Image runs on Pi |
| DT-003 | Image size | < 50MB compressed |
| DT-004 | Container starts | Exits 0 on startup |

### Docker Compose Integration Tests

| Test | Description | Pass Criteria |
|------|-------------|---------------|
| DT-010 | Service starts with deps | ndp-mcp-server waits for etcd |
| DT-011 | Volume mount works | /data/raw accessible |
| DT-012 | Port mapping | localhost:9100 responds |
| DT-013 | Health check passes | docker ps shows healthy |
| DT-014 | Resource limits | Memory < 64MB limit |

### deploy.sh Integration Tests

| Test | Description | Pass Criteria |
|------|-------------|---------------|
| DT-020 | Status includes MCP | deploy.sh status shows MCP health |
| DT-021 | Logs accessible | deploy.sh logs shows MCP output |
| DT-022 | Restart recovery | Service restarts on failure |
| DT-023 | Stop/start cycle | Clean stop and restart |

---

## Test Environment

### Local Development

**Requirements**:
- Rust toolchain (stable)
- Docker (for testcontainers)
- Pre-generated Parquet fixtures

**Setup**:

```bash
# Install dependencies
cargo install cargo-tarpaulin

# Run unit tests
cargo test --package ndp-mcp-server --lib

# Run with integration tests (requires Docker)
cargo test --package ndp-mcp-server --test '*' -- --ignored
```

**Mock Configuration**:

```rust
// tests/helpers/mock_config.rs
pub fn test_config_store() -> MockConfigStore {
    MockConfigStore::with_test_data()
}

pub fn test_storage() -> MockBronzeStorage {
    MockBronzeStorage::with_fixtures()
}
```

### CI/CD (GitHub Actions)

**Pipeline Stages**:

```yaml
test:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - run: cargo test --package ndp-mcp-server --lib

integration:
  runs-on: ubuntu-latest
  services:
    etcd:
      image: bitnami/etcd:3.5
      ports:
        - 2379:2379
  steps:
    - uses: actions/checkout@v4
    - run: cargo test --package ndp-mcp-server --test '*' -- --ignored

coverage:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - run: cargo install cargo-tarpaulin
    - run: cargo tarpaulin --package ndp-mcp-server --out Xml
    - uses: codecov/codecov-action@v3
```

### Pi Integration

**Manual Verification**:

```bash
# Deploy to Pi
./deploy/pi/deploy.sh deploy ndp-mcp-server

# Verify MCP server health check
./deploy/pi/deploy.sh status
# Expected: ndp-mcp-server shows "healthy" status

# Verify MCP tools/list endpoint
curl -X POST http://pi:9100/rpc \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
# Expected: JSON response with list_streams, describe_schema, validate_config, sample_data

# Run system tests
./scripts/system-tests/run-mcp-tests.sh

# Performance validation
./scripts/perf-tests/mcp-load-test.sh --duration 60s --concurrency 10

# Memory monitoring
ssh pi "while true; do ps aux | grep ndp-mcp; sleep 5; done"
```

**MCP Server Verification Checklist**:

| Step | Command | Expected Result |
|------|---------|-----------------|
| Health check | `deploy.sh status` | MCP server shows "healthy" |
| Tools available | `tools/list` RPC call | 4 tools returned |
| etcd connectivity | `list_streams` call | Returns stream list or empty array |
| Parquet access | `sample_data` call | Returns data or STREAM_NOT_FOUND |

---

## Test Data Management

### Fixture Files

**Location**: `/core/ndp-mcp-server/tests/fixtures/`

**Structure** (from test-fixtures.md):

| Fixture | Rows | Purpose |
|---------|------|---------|
| air-quality.parquet | 10 | Standard Bronze data, flat JSON |
| outdoor-weather.parquet | 10 | Nested JSON payload |
| sparse-stream.parquet | 3 | Test n > available rows |
| empty-stream/ | 0 | Test missing data handling |

**Generation**:

```rust
// tests/helpers/parquet.rs
pub fn create_test_parquet(path: &Path, points: Vec<RawDataPoint>) -> Result<()>;
pub fn air_quality_point(offset_minutes: i64, pm25: f64, co2: f64) -> RawDataPoint;
pub fn outdoor_weather_point(offset_minutes: i64, temp: f64, humidity: f64) -> RawDataPoint;
```

### Mock Data

**etcd Test Configs**:

```rust
// tests/helpers/mock_config.rs
impl MockConfigStore {
    pub fn with_test_data() -> Self {
        let mut configs = HashMap::new();
        configs.insert("air-quality".to_string(), air_quality_config());
        configs.insert("outdoor-weather".to_string(), outdoor_weather_config());
        configs.insert("nws-forecast-hourly".to_string(), disabled_config());
        Self { configs }
    }
}
```

### Cleanup Procedures

**Unit Tests**: Automatic (mocks destroyed on test completion)

**Integration Tests**:

```rust
#[tokio::test]
#[ignore]
async fn test_with_cleanup() {
    let etcd = setup_test_etcd().await;
    let temp_dir = tempfile::tempdir().unwrap();

    // Test logic...

    // Cleanup happens automatically via Drop
}
```

**CI/CD**: Fresh containers per run, no persistent state

---

## Coverage Requirements

### Targets by Component

| Component | Target | Rationale |
|-----------|--------|-----------|
| Tool implementations | 90% | Core functionality, high risk |
| MCP protocol handler | 80% | Standard routing logic |
| Storage layer | 85% | Data integrity critical |
| Config parsing | 90% | Complex transformation logic |
| Error handling | 80% | All error paths exercised |
| **Overall** | **85%** | High-quality MVP |

### Measurement

**Tool**: cargo-tarpaulin or llvm-cov

**Command**:

```bash
# Generate coverage report
cargo tarpaulin --package ndp-mcp-server --out Html --output-dir target/coverage

# CI threshold check
cargo tarpaulin --package ndp-mcp-server --fail-under 85
```

### Exclusions

Coverage excludes:
- Debug/trace logging statements
- `#[cfg(test)]` blocks
- Generated code (serde derives)
- Main entrypoint (tested via integration)

---

## Quality Gates

### Pre-Commit

```bash
# Quick validation (< 30 seconds)
cargo fmt --check
cargo clippy --package ndp-mcp-server -- -D warnings
cargo test --package ndp-mcp-server --lib
```

### Pull Request

```bash
# Full validation (< 5 minutes)
cargo fmt --check
cargo clippy --package ndp-mcp-server -- -D warnings
cargo test --package ndp-mcp-server --lib
cargo test --package ndp-mcp-server --test '*' -- --ignored
cargo tarpaulin --package ndp-mcp-server --fail-under 85
```

### Release

```bash
# Complete validation (< 30 minutes)
# All PR gates plus:
./scripts/perf-tests/mcp-benchmarks.sh
./scripts/e2e-tests/claude-code-test.sh
./scripts/system-tests/pi-deployment-test.sh
```

### Gate Thresholds

| Gate | Threshold | Blocking |
|------|-----------|----------|
| Clippy warnings | 0 | Yes |
| Format check | Pass | Yes |
| Unit tests | 100% pass | Yes |
| Integration tests | 100% pass | Yes |
| Coverage | >= 85% | Yes |
| Response times | Within targets | Yes |
| Memory usage | < 50 MB | Yes |

---

## Test Execution Plan

### Development Cycle

| Phase | Tests | Trigger | Duration |
|-------|-------|---------|----------|
| Pre-commit | Unit | Manual/hook | < 30s |
| CI | Unit + Integration | Push | < 5m |
| PR Merge | Full suite | PR merge | < 10m |
| Release | E2E + Pi + Deployment (DT-001 to DT-023) | Manual | < 30m |

### Deployment Test Execution

**Container Build Tests** (DT-001 to DT-004):

```bash
# DT-001: Dockerfile builds
docker build -t ndp-mcp-server:test -f deploy/docker/Dockerfile.mcp-server .

# DT-002: ARM64 cross-compile
cross build --release --target aarch64-unknown-linux-gnu --package ndp-mcp-server
docker buildx build --platform linux/arm64 -t ndp-mcp-server:arm64-test .

# DT-003: Image size check
docker images ndp-mcp-server:test --format "{{.Size}}"
# Must be < 50MB compressed

# DT-004: Container starts
docker run --rm ndp-mcp-server:test --version
# Must exit 0
```

**Docker Compose Tests** (DT-010 to DT-014):

```bash
# Start services
docker-compose -f deploy/docker/docker-compose.yml up -d

# DT-010: Service dependency check
docker-compose logs ndp-mcp-server | grep "Connected to etcd"

# DT-011: Volume mount
docker exec ndp-mcp-server ls /data/raw

# DT-012: Port mapping
curl -s http://localhost:9100/health

# DT-013: Health check
docker ps --filter "name=ndp-mcp-server" --format "{{.Status}}"
# Must show "healthy"

# DT-014: Resource limits
docker stats ndp-mcp-server --no-stream --format "{{.MemUsage}}"
# Must be < 64MB

docker-compose down
```

**deploy.sh Tests** (DT-020 to DT-023):

```bash
# DT-020: Status includes MCP
./deploy/pi/deploy.sh status | grep -q "ndp-mcp-server"

# DT-021: Logs accessible
./deploy/pi/deploy.sh logs ndp-mcp-server | head -20

# DT-022: Restart recovery
docker kill ndp-mcp-server
sleep 10
./deploy/pi/deploy.sh status | grep -q "healthy"

# DT-023: Stop/start cycle
./deploy/pi/deploy.sh stop
./deploy/pi/deploy.sh start
./deploy/pi/deploy.sh status
```

### Running Tests

**Unit Tests**:

```bash
# All unit tests
cargo test --package ndp-mcp-server --lib

# Specific tool
cargo test --package ndp-mcp-server list_streams::tests

# With output
cargo test --package ndp-mcp-server -- --nocapture
```

**Integration Tests**:

```bash
# Requires Docker for etcd
docker run -d -p 2379:2379 bitnami/etcd:3.5

# Run integration tests
cargo test --package ndp-mcp-server --test '*' -- --ignored
```

**Performance Benchmarks**:

```bash
# Run criterion benchmarks
cargo bench --package ndp-mcp-server

# View results
open target/criterion/report/index.html
```

**Coverage**:

```bash
# Generate HTML report
cargo tarpaulin --package ndp-mcp-server --out Html

# Open report
open target/tarpaulin/tarpaulin-report.html
```

---

## Test Dependencies

### Cargo.toml

```toml
[dev-dependencies]
tokio = { version = "1", features = ["test-util", "macros", "rt-multi-thread"] }
mockall = "0.12"
tempfile = "3"
assert_json_diff = "2"
criterion = "0.5"
testcontainers = "0.15"
testcontainers-modules = { version = "0.3", features = ["etcd"] }
```

### External Tools

| Tool | Purpose | Installation |
|------|---------|--------------|
| cargo-tarpaulin | Coverage | `cargo install cargo-tarpaulin` |
| criterion | Benchmarks | dev-dependency |
| Docker | testcontainers, container build tests | System install |
| docker-compose | Compose integration tests (DT-010 to DT-014) | System install |
| cross | ARM64 cross-compilation (DT-002) | `cargo install cross` |

---

## Risk-Based Test Priority

### High Risk (Priority 1)

| Risk | Tests | Mitigation |
|------|-------|------------|
| etcd unavailable | TC-LS-003, integration | Fail-fast behavior verified |
| Memory pressure | ST-001, ST-002 | 24-hour soak test |
| MCP protocol errors | TC-INT-001 to TC-INT-005 | Full protocol compliance |
| Data integrity | TC-SD-032 | Bronze envelope verification |

### Medium Risk (Priority 2)

| Risk | Tests | Mitigation |
|------|-------|------------|
| Large Parquet files | TC-SD-031 | Stream processing, limits |
| Malformed config | TC-VC-021, TC-VC-022 | Structured error handling |
| Nested JSON | TC-DS-010, TC-VC-023 | Path traversal logic |

### Low Risk (Priority 3)

| Risk | Tests | Mitigation |
|------|-------|------------|
| Default parameter values | TC-DS-014, TC-SD-034 | Unit tests |
| Disabled streams | TC-LS-001 | Handled in list_streams |

---

## Related Documents

| Document | Purpose | Location |
|----------|---------|----------|
| Test Plan | Testing strategy overview | `specification/test-plan.md` |
| Test Cases | Detailed test specifications | `specification/test-cases.md` |
| Test Fixtures | Test data definitions | `specification/test-fixtures.md` |
| Success Criteria | Performance targets | `refinement/success-criteria.md` |
| Acceptance Checklist | Verification checklist | `refinement/acceptance-checklist.md` |
| NDP Test Design | Project testing patterns | `/docs/testing/AIR-005-TEST-DESIGN.md` |

---

*Test Strategy defined for dp-005 Bronze MCP Server - SPARC Refinement Phase*
