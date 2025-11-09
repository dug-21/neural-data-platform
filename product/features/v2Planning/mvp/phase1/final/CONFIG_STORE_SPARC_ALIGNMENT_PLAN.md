# Config-Store SPARC Alignment Plan with GitOps/CICD Integration

## Executive Summary

This SPARC plan aligns config-store with the existing GitOps/CICD infrastructure, ensuring proper integration with the neural-trader platform's configuration management strategy. The plan emphasizes quality, testing, and proper architectural alignment - doing it RIGHT, not just done.

## Current State Analysis

### Existing Infrastructure
- **GitOps Structure**: `/configs` directory with base/overlays pattern
- **CICD Scripts**: `config-seeder.sh` and related automation in `/scripts/v2`
- **Configuration Layout**: Hierarchical YAML configs with environment overlays
- **Schema Validation**: JSON schemas defined in `/configs/schemas`
- **Docker Integration**: docker-compose.v2.yml with config-store service

### Config-Store Status
- ✅ Core trait definitions complete (ConfigStore trait)
- ✅ In-memory store implementation with security features
- ✅ gRPC server compiles with proto definitions
- ✅ Unit tests passing (37 tests with London TDD)
- ❌ GitOpsLoader not implemented
- ❌ Redis backend incomplete
- ❌ Schema validation not integrated
- ❌ CICD integration missing

## SPARC Phase 1: Specification

### Functional Requirements

#### 1. GitOps Integration
- Load configurations from `/configs` directory structure
- Support base/overlay merging pattern
- Handle YAML configuration files
- Environment-specific loading (dev/test/prod)

#### 2. Storage Architecture
- **In-Memory**: Primary runtime cache (fast access)
- **Redis**: Distributed cache for multi-instance deployments
- **Git**: Source of truth (no FileConfigStore needed)

#### 3. Schema Validation
- JSON Schema validation for all configurations
- Service-specific schemas in `/configs/schemas`
- Validation on load and on API requests
- Clear error reporting

#### 4. Service Integration
- gRPC API for configuration retrieval
- Health/readiness endpoints
- Configuration watching/subscription
- Service registration and discovery

### Non-Functional Requirements

#### Performance
- Sub-millisecond in-memory access
- < 10ms Redis access
- < 1 second startup seeding
- Support 1000+ configs

#### Reliability
- Graceful fallback on Redis failure
- Validation prevents bad configs
- Atomic configuration updates
- Version tracking

#### Security
- No secrets in Git
- Input validation and sanitization
- Rate limiting
- Audit logging

## SPARC Phase 2: Pseudocode

### GitOpsLoader Implementation

```rust
class GitOpsLoader {
    base_path: PathBuf
    environment: String
    validator: SchemaValidator
    
    fn load_configs() -> Result<ConfigTree> {
        // 1. Load base configurations
        base_configs = load_directory(base_path / "base")
        
        // 2. Load environment overlays
        overlay_configs = load_directory(base_path / "overlays" / environment)
        
        // 3. Merge configurations
        merged = merge_configs(base_configs, overlay_configs)
        
        // 4. Validate against schemas
        for (service, config) in merged {
            schema = load_schema(service)
            validator.validate(config, schema)?
        }
        
        // 5. Build configuration tree
        tree = build_config_tree(merged)
        
        return Ok(tree)
    }
    
    fn merge_configs(base, overlay) -> ConfigMap {
        // Deep merge with overlay precedence
        for (key, value) in overlay {
            if base.contains(key) {
                base[key] = deep_merge(base[key], value)
            } else {
                base[key] = value
            }
        }
        return base
    }
}
```

### Redis Backend Implementation

```rust
class RedisConfigStore implements ConfigStore {
    client: RedisClient
    cache: InMemoryStore
    ttl: Duration
    
    async fn get(path: &str) -> Result<ConfigValue> {
        // 1. Check in-memory cache first
        if let Some(value) = cache.get(path) {
            return Ok(value)
        }
        
        // 2. Fetch from Redis
        key = format!("config:{}:{}", environment, path)
        if let Some(data) = redis.get(key).await? {
            value = deserialize(data)?
            cache.set(path, value.clone())
            return Ok(value)
        }
        
        // 3. Not found
        return Err(NotFound)
    }
    
    async fn set(path: &str, value: ConfigValue) -> Result<()> {
        // 1. Validate
        validator.validate_value(&value)?
        
        // 2. Store in Redis with TTL
        key = format!("config:{}:{}", environment, path)
        data = serialize(value)?
        redis.setex(key, ttl, data).await?
        
        // 3. Update cache
        cache.set(path, value)
        
        // 4. Notify watchers
        notify_change(path, value)
        
        return Ok(())
    }
}
```

## SPARC Phase 3: Architecture

### Component Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Config-Store Service               │
├─────────────────────────────────────────────────────┤
│                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────┐  │
│  │ gRPC Server  │  │ Health/Ready │  │  Metrics  │  │
│  └──────┬───────┘  └──────┬───────┘  └─────┬────┘  │
│         │                  │                 │       │
│  ┌──────┴──────────────────┴─────────────────┴───┐  │
│  │            Service Layer (API)                 │  │
│  └────────────────────┬───────────────────────────┘  │
│                       │                              │
│  ┌────────────────────┴───────────────────────────┐  │
│  │          Configuration Manager                  │  │
│  │  ┌─────────────┐  ┌──────────┐  ┌──────────┐  │  │
│  │  │   Loader    │  │Validator │  │  Watcher │  │  │
│  │  └─────────────┘  └──────────┘  └──────────┘  │  │
│  └────────────────────┬───────────────────────────┘  │
│                       │                              │
│  ┌────────────────────┴───────────────────────────┐  │
│  │              Storage Layer                      │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────────┐ │  │
│  │  │ InMemory │  │  Redis   │  │  GitOpsLoader│ │  │
│  │  └──────────┘  └──────────┘  └──────────────┘ │  │
│  └─────────────────────────────────────────────────┘  │
│                                                      │
└─────────────────────────────────────────────────────┘
```

### Data Flow

```
1. Startup Sequence:
   Git Repository → GitOpsLoader → Validator → InMemory + Redis

2. Runtime Access:
   Service Request → gRPC → InMemory (cache hit) → Response
                          ↓ (cache miss)
                          Redis → InMemory → Response

3. Configuration Update:
   Git Push → Webhook (future) → GitOpsLoader → Validator → Storage → Notify
```

### Directory Structure

```
config-store/
├── src/
│   ├── lib.rs                 # Library exports
│   ├── traits.rs              # ConfigStore trait
│   ├── types.rs               # Core types
│   ├── stores/
│   │   ├── mod.rs
│   │   ├── in_memory.rs       # In-memory implementation
│   │   ├── redis.rs           # Redis backend
│   │   └── secure_in_memory.rs
│   ├── loaders/
│   │   ├── mod.rs
│   │   └── gitops.rs          # GitOps loader
│   ├── validation/
│   │   ├── mod.rs
│   │   └── schema.rs          # JSON Schema validator
│   ├── security/
│   │   ├── mod.rs
│   │   ├── validator.rs
│   │   ├── sanitizer.rs
│   │   └── blocklist.rs
│   └── bin/
│       └── config-store-server.rs
├── tests/
│   ├── integration_test.rs
│   └── gitops_test.rs
└── Cargo.toml
```

## SPARC Phase 4: Refinement (Implementation Plan)

### Sprint 1: GitOps Foundation (Week 1)
- [ ] Remove FileConfigStore completely
- [ ] Implement GitOpsLoader
  - [ ] YAML parsing
  - [ ] Base/overlay merging
  - [ ] Directory walking
- [ ] Unit tests for GitOpsLoader
- [ ] Integration with existing InMemoryStore

### Sprint 2: Redis Backend (Week 1-2)
- [ ] Complete RedisConfigStore implementation
  - [ ] Async connection management
  - [ ] Serialization/deserialization
  - [ ] TTL management
  - [ ] Error handling
- [ ] Integration with InMemory caching
- [ ] Fallback mechanism
- [ ] Unit tests with mocks
- [ ] Integration tests with testcontainers

### Sprint 3: Schema Validation (Week 2)
- [ ] Implement SchemaValidator
  - [ ] JSON Schema loading
  - [ ] YAML to JSON conversion
  - [ ] Validation engine
  - [ ] Error reporting
- [ ] Service-specific schemas
- [ ] Validation hooks in storage layer
- [ ] Tests for all validation paths

### Sprint 4: CICD Integration (Week 2-3)
- [ ] Docker entrypoint script
- [ ] Integration with config-seeder.sh
- [ ] Health/readiness endpoints
- [ ] Startup validation
- [ ] Environment variable configuration
- [ ] Docker-compose integration

### Sprint 5: Testing & Documentation (Week 3)
- [ ] Comprehensive integration tests
- [ ] Load testing
- [ ] Security testing
- [ ] Documentation updates
- [ ] Deployment guide
- [ ] Troubleshooting guide

## SPARC Phase 5: Completion Criteria

### Definition of Done
- ✅ All unit tests passing (> 90% coverage)
- ✅ Integration tests with real Redis
- ✅ GitOps loading from `/configs` directory
- ✅ Schema validation working
- ✅ Docker container builds and runs
- ✅ CICD pipeline integration complete
- ✅ Load tested with 1000+ configs
- ✅ Security scan passed
- ✅ Documentation complete

### Acceptance Tests

#### Test 1: GitOps Loading
```bash
# Place test configs in /configs
./scripts/v2/config-seeder.sh dev
# Verify configs loaded
grpcurl -plaintext localhost:50051 config.ConfigStore/GetConfig
```

#### Test 2: Redis Persistence
```bash
# Start with Redis
docker-compose up -d redis config-store
# Load configs
./scripts/v2/config-seeder.sh dev
# Restart config-store
docker-compose restart config-store
# Verify configs still available
```

#### Test 3: Schema Validation
```bash
# Create invalid config
echo "invalid: yaml" > /configs/test/bad.yaml
# Attempt to load
./scripts/v2/config-seeder.sh test
# Should fail with validation error
```

#### Test 4: Service Integration
```bash
# Start full stack
docker-compose -f docker-compose.v2.yml up -d
# Verify services get configs
docker logs neural-trading | grep "Config loaded"
```

## Implementation Priority

1. **MUST HAVE** (Week 1)
   - GitOpsLoader
   - Remove FileConfigStore
   - Basic Redis implementation

2. **SHOULD HAVE** (Week 2)
   - Schema validation
   - Complete Redis features
   - CICD integration

3. **NICE TO HAVE** (Week 3)
   - Advanced monitoring
   - Configuration history
   - Hot reload support

## Risk Mitigation

### Technical Risks
- **Risk**: Redis connection failures
  - **Mitigation**: In-memory fallback, connection pooling
  
- **Risk**: Invalid configurations break services
  - **Mitigation**: Schema validation, staging environment testing

- **Risk**: Performance degradation with many configs
  - **Mitigation**: Efficient caching, lazy loading, pagination

### Operational Risks
- **Risk**: Configuration drift between environments
  - **Mitigation**: GitOps single source of truth, automated validation

- **Risk**: Service startup failures
  - **Mitigation**: Health checks, retry logic, graceful degradation

## Success Metrics

### Performance
- Configuration load time < 1ms (in-memory)
- Redis fetch time < 10ms
- Startup seeding < 1 second
- Support 1000+ configurations

### Reliability
- 99.9% uptime
- Zero configuration corruption
- Successful validation rate > 99%
- Automatic recovery from Redis failures

### Quality
- Test coverage > 90%
- Zero critical security issues
- Documentation coverage 100%
- All CICD pipelines green

## Timeline

```
Week 1: Foundation
├── Mon-Tue: Remove FileConfigStore, implement GitOpsLoader
├── Wed-Thu: Redis backend implementation
└── Fri: Testing and integration

Week 2: Integration
├── Mon-Tue: Schema validation
├── Wed-Thu: CICD integration
└── Fri: Integration testing

Week 3: Completion
├── Mon-Tue: Load testing and optimization
├── Wed-Thu: Documentation and deployment
└── Fri: Final validation and signoff
```

## Conclusion

This SPARC plan provides a comprehensive approach to aligning config-store with the GitOps/CICD infrastructure. By focusing on quality, testing, and proper architectural patterns, we ensure the implementation is done RIGHT, providing a robust configuration management system for the neural-trader platform.

The plan emphasizes:
1. **Simplicity**: No unnecessary backends (removing FileConfigStore)
2. **Reliability**: In-memory + Redis with proper fallbacks
3. **Quality**: Comprehensive testing at every level
4. **Integration**: Seamless CICD and GitOps alignment
5. **Security**: Validation, sanitization, and no secrets in Git

Following this plan will result in a production-ready config-store that serves as the configuration backbone for all neural-trader microservices.