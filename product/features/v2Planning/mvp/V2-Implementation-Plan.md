# V2 MVP Implementation Plan - Foundation First

## Executive Summary

This implementation plan prioritizes quality, testability, and documentation over speed. V1 failed due to complexity and abandoned testing. V2 will succeed by building a solid foundation where every component is independently testable and every pattern is documented for future development.

## Core Philosophy

**"Every component independently testable, every integration documented, every pattern reusable"**

## Phase 1: Configuration Foundation (Weeks 1-2)

### 1.1 Configuration Store - Building Block #1
**Focus: Create the perfect reference implementation**

```rust
// config-store/src/lib.rs - THE reference pattern
pub trait ConfigStore: Send + Sync {
    async fn get(&self, path: &str) -> Result<ConfigValue>;
    async fn set(&self, path: &str, value: ConfigValue) -> Result<()>;
    async fn get_tree(&self, prefix: &str) -> Result<ConfigTree>;
}

// Implementations can be swapped for testing
pub struct RedisConfigStore { ... }
pub struct InMemoryConfigStore { ... }  // For testing
pub struct FileConfigStore { ... }      // For local development
```

**Deliverables**:
1. **Core Library** with trait-based design
2. **Unit Tests** - 100% coverage of core logic
3. **Integration Tests** - Against real Redis
4. **Load Tests** - Performance benchmarks
5. **Documentation** - Integration patterns, examples
6. **Docker Test Environment** - Isolated testing

### 1.2 Integration Pattern Library
**Focus: Document HOW services should integrate**

```rust
// patterns/src/config_integration.rs
/// Reference implementation for service configuration
pub struct ServiceConfig<T> {
    store: Arc<dyn ConfigStore>,
    path: String,
    cache: Option<T>,
    validator: Box<dyn Validator<T>>,
}

impl<T: DeserializeOwned> ServiceConfig<T> {
    /// Standard pattern for loading config with validation
    pub async fn load(&mut self) -> Result<T> {
        let raw = self.store.get(&self.path).await?;
        let config = serde_json::from_value(raw)?;
        self.validator.validate(&config)?;
        self.cache = Some(config.clone());
        Ok(config)
    }
    
    /// Standard pattern for config refresh
    pub async fn refresh(&mut self) -> Result<bool> {
        // Implementation with proper error handling
    }
}
```

**Testing Patterns Documentation**:
```rust
// Every service MUST implement these test patterns
#[cfg(test)]
mod tests {
    #[test]
    fn test_config_with_mock_store() {
        let store = InMemoryConfigStore::new();
        // Test configuration logic without Redis
    }
    
    #[test]
    fn test_config_validation() {
        // Test invalid configurations are rejected
    }
    
    #[test]
    fn test_config_refresh() {
        // Test configuration can be refreshed
    }
}
```

## Phase 2: Event Bus Foundation (Weeks 3-4)

### 2.1 Event Bus Abstraction
**Focus: Testable event streaming**

```rust
// eventbus/src/lib.rs
pub trait EventBus: Send + Sync {
    async fn publish(&self, stream: &str, event: Event) -> Result<EventId>;
    async fn subscribe(&self, stream: &str, group: &str) -> Result<Subscriber>;
    async fn ack(&self, stream: &str, group: &str, id: EventId) -> Result<()>;
}

// Multiple implementations for different contexts
pub struct RedisEventBus { ... }      // Production
pub struct InMemoryEventBus { ... }   // Unit tests
pub struct RecordingEventBus { ... }  // Integration tests (records all events)
```

### 2.2 Event Patterns Documentation
```rust
// patterns/src/event_patterns.rs

/// Standard pattern for event producers
pub struct EventProducer<T> {
    bus: Arc<dyn EventBus>,
    stream: String,
    serializer: Box<dyn Serializer<T>>,
}

/// Standard pattern for event consumers  
pub struct EventConsumer<T> {
    bus: Arc<dyn EventBus>,
    stream: String,
    group: String,
    handler: Box<dyn EventHandler<T>>,
}

/// Test helper for event-driven testing
pub struct EventTestHarness {
    pub fn given_events(&mut self, events: Vec<Event>);
    pub fn when_event(&mut self, event: Event);
    pub fn then_expect(&self, matcher: EventMatcher);
}
```

## Phase 3: Service Interface Foundation (Weeks 5-6)

### 3.1 gRPC Service Framework
**Focus: Standardized service patterns**

```rust
// services/src/framework.rs

/// Every service MUST implement this trait
pub trait MicroService {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn health_check(&self) -> HealthStatus;
    async fn start(&mut self) -> Result<()>;
    async fn shutdown(&mut self) -> Result<()>;
}

/// Standard gRPC service wrapper
pub struct GrpcService<T: MicroService> {
    inner: T,
    config: ServiceConfig,
    metrics: MetricsCollector,
    tracer: Tracer,
}
```

### 3.2 Testing Framework
```rust
// testing/src/service_testing.rs

/// Standard test harness for any microservice
pub struct ServiceTestHarness<T: MicroService> {
    service: T,
    config_store: InMemoryConfigStore,
    event_bus: RecordingEventBus,
    mock_clients: HashMap<String, MockClient>,
}

impl<T: MicroService> ServiceTestHarness<T> {
    /// Test service initialization
    pub async fn test_startup(&mut self) -> TestResult;
    
    /// Test configuration handling
    pub async fn test_configuration(&mut self) -> TestResult;
    
    /// Test event processing
    pub async fn test_event_handling(&mut self) -> TestResult;
    
    /// Test health checks
    pub async fn test_health_check(&mut self) -> TestResult;
}
```

## Phase 4: Reference Implementation (Weeks 7-8)

### 4.1 Trading Hours Service - The Perfect Example
**Focus: A complete, simple service showing all patterns**

```rust
// services/trading-hours/src/lib.rs

pub struct TradingHoursService {
    config: ServiceConfig<TradingHoursConfig>,
    event_bus: Arc<dyn EventBus>,
    cache: TradingHoursCache,
}

impl MicroService for TradingHoursService {
    // Full implementation following all patterns
}

// Comprehensive test suite showing best practices
#[cfg(test)]
mod tests {
    use super::*;
    use testing::ServiceTestHarness;
    
    #[tokio::test]
    async fn test_trading_hours_service() {
        let harness = ServiceTestHarness::new();
        harness.test_all_standard_patterns().await.unwrap();
    }
    
    #[tokio::test]
    async fn test_market_open_logic() {
        // Domain-specific tests
    }
}
```

### 4.2 Integration Test Suite
```rust
// tests/integration/trading_hours_integration.rs

#[tokio::test]
async fn test_trading_hours_with_real_redis() {
    // Test with real Redis using docker-compose.test.yml
}

#[tokio::test]
async fn test_trading_hours_with_other_services() {
    // Test interaction patterns
}
```

## Documentation Architecture

### 1. Pattern Catalog
```markdown
docs/patterns/
├── configuration-integration.md
├── event-handling.md
├── service-initialization.md
├── testing-strategies.md
├── error-handling.md
└── monitoring-integration.md
```

### 2. Service Template
```markdown
docs/templates/
├── new-service-checklist.md
├── service-structure.md
├── testing-requirements.md
└── deployment-guide.md
```

### 3. Integration Guides
```markdown
docs/integration/
├── config-store-integration.md
├── eventbus-integration.md
├── grpc-service-setup.md
└── docker-integration.md
```

## Testing Strategy

### 1. Unit Test Requirements
- **100% coverage** for business logic
- **Mocked dependencies** using traits
- **Property-based testing** for complex logic
- **Snapshot testing** for configurations

### 2. Integration Test Layers
```yaml
# docker-compose.test.yml
services:
  redis-test:
    image: redis:7-alpine
    
  config-store-test:
    build: ./config-store
    environment:
      - TEST_MODE=true
      
  test-runner:
    build: ./tests
    command: cargo test --all
```

### 3. Contract Testing
```rust
// Every service interface must have contract tests
#[test]
fn test_grpc_contract_compliance() {
    // Verify service implements required gRPC methods
}

#[test]
fn test_event_contract_compliance() {
    // Verify events match schemas
}
```

## Quality Gates

### Phase Gate Criteria
Each phase must meet these criteria before proceeding:

1. **Code Coverage**: >90% for business logic
2. **Documentation**: Complete pattern docs and examples
3. **Integration Tests**: Passing with real dependencies
4. **Performance Tests**: Meeting defined SLAs
5. **Code Review**: Approved by team
6. **Example Usage**: Working example in another service

## Success Metrics

### Technical Quality Metrics
- **Test Coverage**: >90% overall, 100% for critical paths
- **Test Execution Time**: <5 minutes for full suite
- **Documentation Coverage**: Every public API documented
- **Pattern Compliance**: 100% of services follow patterns

### Architectural Quality Metrics
- **Coupling**: Services only depend on interfaces, not implementations
- **Cohesion**: Each service has single, clear responsibility  
- **Testability**: Every service can run with mocked dependencies
- **Observability**: Every service exports standard metrics

## Implementation Priorities

### Week 1-2: Configuration Store
- Build it right, test it thoroughly
- Document integration patterns
- Create InMemory and Redis implementations

### Week 3-4: Event Bus
- Abstract interface with multiple implementations
- Recording implementation for testing
- Event testing patterns

### Week 5-6: Service Framework
- Standard service structure
- Testing harness
- Health check patterns

### Week 7-8: Reference Implementation
- Trading Hours service as the perfect example
- Full test coverage
- Complete documentation

## Key Differences from Original Plan

1. **No Rush to Complete System**: Focus on foundation quality
2. **Test-First Development**: Tests drive the design
3. **Pattern Documentation**: Equal priority with code
4. **Multiple Implementations**: Every interface has test/prod versions
5. **Reference Implementations**: Show the right way
6. **Quality Gates**: Can't proceed without meeting standards

This approach ensures that V2 doesn't repeat V1's mistakes. Every component is testable, every pattern is documented, and future developers have clear examples to follow.

---

*Document Version*: 1.0  
*Created*: 2025-01-20  
*Status*: Foundation-First Implementation Plan