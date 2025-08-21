# Phase 1: Configuration Foundation - Detailed Specification

## 1. Overview

### 1.1 Purpose
Create a centralized, hierarchical configuration management system that eliminates configuration duplication across services and provides a single source of truth for all system configuration.

### 1.2 Scope
- Configuration Store core library with trait-based design
- Multiple implementation backends (Redis, InMemory, File)
- Integration pattern library for service adoption
- Comprehensive testing framework
- Docker test environment

### 1.3 Success Criteria
- Eliminate duplicate trading hours configuration between data ingestion and execution layers
- 100% test coverage for core business logic
- Sub-10ms read latency for configuration retrieval
- Zero configuration-related production incidents

## 2. Functional Requirements

### 2.1 Configuration Storage

#### FR-2.1.1: Hierarchical Path Storage
- **Requirement**: Support hierarchical path-based configuration storage
- **Format**: `/domain/service/component/key`
- **Example**: `/system/global/trading_hours/us_equity`
- **Validation**: Paths must follow naming convention, max depth of 6 levels

#### FR-2.1.2: CRUD Operations
- **Create**: Add new configuration at specified path
- **Read**: Retrieve configuration by exact path or prefix
- **Update**: Modify existing configuration with version tracking
- **Delete**: Remove configuration with audit trail

#### FR-2.1.3: Bulk Operations
- **Get Tree**: Retrieve all configurations under a prefix
- **Bulk Update**: Update multiple configurations atomically
- **Export/Import**: Serialize/deserialize configuration trees

### 2.2 Configuration Features

#### FR-2.2.1: Inheritance
- **Requirement**: Support configuration inheritance from parent paths
- **Merge Strategy**: Child overrides parent values
- **Example**: Service inherits from domain defaults

#### FR-2.2.2: Validation
- **Schema Validation**: Optional JSON Schema validation
- **Type Safety**: Strongly typed configuration values
- **Range Checks**: Validate numeric values within bounds

#### FR-2.2.3: Versioning
- **Version Tracking**: Each configuration change creates new version
- **History**: Maintain last 10 versions per path
- **Rollback**: Ability to revert to previous version

### 2.3 Integration Requirements

#### FR-2.3.1: Service Integration Pattern
- **Standard Client**: Provide client library for services
- **Caching**: Local caching with TTL
- **Refresh**: Pull-based refresh on interval or trigger

#### FR-2.3.2: Migration Support
- **ENV Import**: Import existing .env variables
- **Mapping**: Map flat env vars to hierarchical paths
- **Validation**: Verify migrated values

## 3. Non-Functional Requirements

### 3.1 Performance
- **Read Latency**: < 10ms for single key retrieval
- **Write Latency**: < 50ms for single key update
- **Throughput**: 10,000 reads/second, 1,000 writes/second
- **Cache Hit Rate**: > 90% for frequently accessed configs

### 3.2 Reliability
- **Availability**: 99.9% uptime
- **Durability**: No data loss on restart
- **Consistency**: Strong consistency for writes
- **Backup**: Daily backups with 30-day retention

### 3.3 Security
- **Encryption**: Sensitive values encrypted at rest
- **Access Control**: Path-based read/write permissions
- **Audit Trail**: All changes logged with user/timestamp
- **Secrets**: Special handling for API keys and passwords

### 3.4 Testability
- **Unit Tests**: 100% coverage of business logic
- **Integration Tests**: Test with real Redis
- **Contract Tests**: Verify interface compliance
- **Performance Tests**: Benchmark against SLAs

## 4. Technical Constraints

### 4.1 Technology Stack
- **Language**: Rust for performance and safety
- **Primary Backend**: Redis (existing infrastructure)
- **Serialization**: JSON for human readability
- **API**: REST for initial implementation

### 4.2 Deployment
- **Container**: Docker with < 100MB image
- **Memory**: < 512MB RAM usage
- **CPU**: < 0.5 CPU cores
- **Network**: Internal network only

## 5. Configuration Schema

### 5.1 Core Data Model
```yaml
ConfigNode:
  path: string              # Hierarchical path
  value: any               # Configuration value
  schema: JSONSchema?      # Optional validation
  version: integer         # Version number
  metadata:
    description: string
    owner: string
    sensitive: boolean
    runtime_modifiable: boolean
    created_at: timestamp
    updated_at: timestamp
    updated_by: string
  inheritance:
    - parent_path: string
  overrides:
    key: value
```

### 5.2 Standard Configuration Hierarchy
```yaml
/system
  /global
    /trading_hours      # Market hours configuration
    /holidays          # Market holidays
    /feature_flags     # System-wide toggles
    
/domain
  /trading
    /symbols          # Traded symbols
    /risk_limits      # Risk parameters
    /broker_config    # Broker connections
    
/infrastructure
  /storage
    /timescale       # Database config
    /redis           # Cache config
    
/services
  /data_ingestion
    /providers       # Data source configs
    /schedules       # Polling schedules
  /model_execution
    /models          # Model parameters
    /features        # Feature configs
```

## 6. Interface Specifications

### 6.1 ConfigStore Trait
```rust
#[async_trait]
pub trait ConfigStore: Send + Sync {
    // Core CRUD operations
    async fn get(&self, path: &str) -> Result<ConfigValue, ConfigError>;
    async fn set(&self, path: &str, value: ConfigValue) -> Result<(), ConfigError>;
    async fn delete(&self, path: &str) -> Result<(), ConfigError>;
    
    // Bulk operations
    async fn get_tree(&self, prefix: &str) -> Result<ConfigTree, ConfigError>;
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, ConfigError>;
    
    // Versioning
    async fn get_version(&self, path: &str, version: u32) -> Result<ConfigValue, ConfigError>;
    async fn get_history(&self, path: &str) -> Result<Vec<ConfigVersion>, ConfigError>;
    
    // Transactions
    async fn transaction<F>(&self, f: F) -> Result<(), ConfigError>
    where
        F: FnOnce(&mut ConfigTransaction) -> Result<(), ConfigError>;
}
```

### 6.2 ServiceConfig Pattern
```rust
pub struct ServiceConfig<T: DeserializeOwned + Validate> {
    store: Arc<dyn ConfigStore>,
    path: String,
    cache: RwLock<Option<(T, Instant)>>,
    cache_ttl: Duration,
    validator: Box<dyn Validator<T>>,
}

impl<T> ServiceConfig<T> {
    pub async fn load(&self) -> Result<T, ConfigError>;
    pub async fn reload(&self) -> Result<T, ConfigError>;
    pub async fn watch<F>(&self, callback: F) where F: Fn(&T);
}
```

## 7. Testing Requirements

### 7.1 Unit Test Coverage
- Core CRUD operations: 100%
- Inheritance logic: 100%
- Validation logic: 100%
- Caching logic: 100%

### 7.2 Integration Test Scenarios
1. Redis backend connection and operations
2. Concurrent read/write operations
3. Transaction rollback on failure
4. Cache invalidation on update
5. Migration from .env file

### 7.3 Performance Test Targets
- 10,000 sequential reads: < 100ms total
- 1,000 concurrent reads: < 500ms total
- 100 writes with validation: < 1000ms total

### 7.4 Test Data Requirements
- Standard test configuration tree
- Invalid configuration examples
- Performance test dataset (1000+ keys)

## 8. Migration Plan

### 8.1 Phase 1: Core Implementation
1. Implement ConfigStore trait
2. Create RedisConfigStore implementation
3. Create InMemoryConfigStore for testing
4. Implement ServiceConfig pattern

### 8.2 Phase 2: Service Integration
1. Migrate trading hours configuration
2. Update data_ingestion service
3. Update execution service
4. Verify duplicate elimination

### 8.3 Phase 3: Full Migration
1. Export all .env variables
2. Map to hierarchical paths
3. Import to ConfigStore
4. Update all services

## 9. Acceptance Criteria

### 9.1 Functional Acceptance
- [ ] Trading hours configuration shared between services
- [ ] All CRUD operations working with Redis backend
- [ ] Inheritance and override logic functioning
- [ ] Validation preventing invalid configurations

### 9.2 Quality Acceptance
- [ ] 100% unit test coverage achieved
- [ ] All integration tests passing
- [ ] Performance SLAs met
- [ ] Zero memory leaks detected

### 9.3 Documentation Acceptance
- [ ] API documentation complete
- [ ] Integration guide written
- [ ] Migration guide provided
- [ ] Example code available

## 10. Risk Assessment

### 10.1 Technical Risks
- **Risk**: Redis connection failures
- **Mitigation**: Implement circuit breaker and local cache fallback

### 10.2 Migration Risks
- **Risk**: Configuration mapping errors
- **Mitigation**: Dry-run mode with validation before commit

### 10.3 Performance Risks
- **Risk**: Cache invalidation storms
- **Mitigation**: Implement cache warm-up and gradual invalidation

---

*Specification Version*: 1.0
*Created*: 2025-01-20
*Status*: Ready for Pseudocode Development