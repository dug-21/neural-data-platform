# Immediate Action Plan for Config-Store Recovery

## Current Situation
- **77,419 lines deleted** (46% of codebase) in commit d2d68fe
- **2,034 lines recovered** from deleted src/config/ directory
- **54% functionality restored** but service not operational
- **gRPC server has 72 compilation errors** preventing build

## Critical Issues Blocking Service

### 1. Proto Type References Not Compiling
The gRPC server (config-store-server.rs) cannot resolve proto-generated types:
- `ValueType`, `ChangeType`, `ConfigChangeEvent`, etc. are not found
- Stream types are ambiguous
- Proto generation may not be working correctly

### 2. Missing Core Implementations
- **RedisConfigStore**: Production storage backend (0% implemented)
- **FileConfigStore**: File-based configuration (0% implemented)  
- **Real-time streaming**: Watch/subscribe mechanism (10% implemented)
- **Schema validation**: JSON Schema support (0% implemented)

### 3. Integration Gaps
- Services cannot connect to config-store
- No client library for service integration
- Migration tools missing

## Immediate Next Steps (Priority Order)

### Step 1: Fix Proto Generation (TODAY)
```bash
# 1. Verify proto file exists
ls -la /workspaces/neural-trader/proto/config_store.proto

# 2. Run proto generation manually
cd /workspaces/neural-trader
cargo build --package config-store

# 3. Check generated files
find target -name "*.rs" -path "*/neural_platform/*" 

# 4. Fix import paths in server
```

### Step 2: Simplify gRPC Server (TODAY)
Rather than fixing 72 errors, create minimal working server:
```rust
// Minimal server with just core functionality
- Remove ConfigManagementService for now
- Implement only essential ConfigStoreService methods
- Use simple in-memory store initially
- Add streaming later
```

### Step 3: Implement RedisConfigStore (TOMORROW)
```rust
// Priority implementation
pub struct RedisConfigStore {
    client: redis::Client,
    cache: DashMap<String, (ConfigValue, Instant)>,
}

impl ConfigStore for RedisConfigStore {
    async fn get(&self, path: &str) -> Result<ConfigValue, ConfigError>
    async fn set(&self, path: &str, value: ConfigValue) -> Result<(), ConfigError>
}
```

### Step 4: Create Integration Test (DAY 3)
```rust
#[tokio::test]
async fn test_config_store_integration() {
    // Start server
    // Connect client
    // Test CRUD operations
    // Verify other services can connect
}
```

## Quick Fix Commands

```bash
# Fix proto generation
cd /workspaces/neural-trader
cargo clean
cargo build

# Test config-store specifically
cargo test -p config-store

# Run pipeline
./scripts/v2/run-pipeline.sh config-store

# Start service
cargo run --bin config-store-server
```

## Success Criteria

1. ✅ config-store-server compiles without errors
2. ✅ gRPC service starts on port 50051
3. ✅ Basic get/set operations work
4. ✅ Other services can retrieve configuration
5. ✅ Tests pass in CI/CD pipeline

## Resources Available

### Working Components
- InMemoryConfigStore implementation
- Security features (rate limiting, sanitization)
- Platform configuration types
- Proto definition file
- Integration tests

### Documentation
- MVP Phase 1 Specification
- PSEUDOCODE.md with implementation details
- TDD-TEST-PLAN.md with test requirements
- Security remediation docs

## Risk Mitigation

If proto generation continues to fail:
1. Use REST API temporarily instead of gRPC
2. Implement simple HTTP endpoints
3. Migrate to gRPC once proto issues resolved

## Timeline

- **Day 1 (Today)**: Fix proto generation, simplify server
- **Day 2**: Implement RedisConfigStore
- **Day 3**: Integration testing
- **Day 4**: Real-time streaming
- **Day 5**: Schema validation
- **Week 2**: Full feature parity with specification

## Contact Points

For questions or blockers:
- Check `/product/features/v2Planning/mvp/phase1/` for specifications
- Review git history for deleted implementations
- Use ruv-swarm for complex analysis tasks

## Conclusion

The config-store service is recoverable but requires immediate action on proto generation and server simplification. Once these blockers are resolved, the implementation can proceed rapidly using the recovered configuration types and existing security features.