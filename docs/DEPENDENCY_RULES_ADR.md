# Architectural Decision Record: Dependency Rules and Constraints

## Status
Accepted

## Context
The Universal Discovery Platform requires strict dependency rules to maintain modularity, testability, and independent scalability. Without clear constraints, the system risks developing tight coupling that would prevent independent evolution of components.

## Decision

We establish the following dependency rules for the Universal Discovery Platform:

### 1. Layer Dependency Hierarchy

```mermaid
graph TB
    subgraph "Execution Domains"
        ED[Execution Domain Layer]
    end
    
    subgraph "Discovery Engine"  
        DE[Discovery Engine Layer]
    end
    
    subgraph "Data Platform"
        DP[Data Platform Layer]
    end
    
    subgraph "Infrastructure"
        IL[Infrastructure Layer]
    end
    
    subgraph "External"
        EX[External Systems]
    end
    
    %% Allowed Dependencies (solid lines)
    ED --> DE
    ED --> DP
    DE --> DP
    DP --> IL
    IL --> EX
    
    %% Data Stream Dependencies (dashed lines)
    DP -.-> ED
    
    %% Forbidden Dependencies (marked with X)
    DE -.x ED
    DP -.x DE
    IL -.x DP
```

### 2. Strict Dependency Rules

#### Rule 1: Unidirectional Layer Dependencies
- **ALLOWED**: Higher layers may depend on lower layers
- **FORBIDDEN**: Lower layers must never depend on higher layers
- **EXCEPTION**: Data streams flow upward through pub/sub (not direct dependencies)

#### Rule 2: No Horizontal Dependencies  
- **FORBIDDEN**: Components within the same layer cannot directly depend on each other
- **ALTERNATIVE**: Use shared abstractions or communicate through lower layers

#### Rule 3: Interface-Only Dependencies
- **REQUIRED**: All cross-layer dependencies must be through defined traits/interfaces
- **FORBIDDEN**: Direct dependencies on concrete implementations across layers

#### Rule 4: Execution Domain Isolation
- **FORBIDDEN**: Execution domains cannot communicate directly with each other
- **REQUIRED**: All inter-domain communication through Data Platform streams

### 3. Dependency Injection Rules

#### Rule 5: Constructor Injection
```rust
// CORRECT: Dependency injection through constructor
impl StreamProcessor {
    pub fn new(
        ingester: Arc<dyn DataIngester>,
        storage: Arc<dyn FeatureStore>,
        router: Arc<dyn StreamRouter>,
    ) -> Self {
        Self { ingester, storage, router }
    }
}

// FORBIDDEN: Direct instantiation of dependencies
impl StreamProcessor {
    pub fn new() -> Self {
        let ingester = ConcreteIngester::new(); // ❌ FORBIDDEN
        Self { ingester }
    }
}
```

#### Rule 6: Abstraction Boundaries
```rust
// CORRECT: Depend on traits, not concrete types
struct DiscoveryEngine {
    feature_store: Arc<dyn FeatureStore>,
    stream_router: Arc<dyn StreamRouter>,
}

// FORBIDDEN: Depending on concrete implementations
struct DiscoveryEngine {
    feature_store: Arc<PostgresFeatureStore>, // ❌ FORBIDDEN
}
```

### 4. Communication Pattern Rules

#### Rule 7: Synchronous vs Asynchronous Communication
- **Synchronous**: Same-layer or downward dependencies only
- **Asynchronous**: Upward data flow through event streams
- **FORBIDDEN**: Synchronous upward calls

```rust
// CORRECT: Async upward data flow
async fn publish_pattern(&self, pattern: Pattern) -> Result<(), PublishError> {
    self.stream_router
        .publish("patterns", &serialize(pattern)?)
        .await
}

// FORBIDDEN: Sync upward call  
async fn notify_execution_domain(&self, pattern: Pattern) -> Result<(), Error> {
    self.execution_domain.handle_pattern(pattern).await // ❌ FORBIDDEN
}
```

#### Rule 8: Event-Driven Architecture
- **REQUIRED**: All upward communication must be event-driven
- **REQUIRED**: Events must be domain-agnostic at platform boundaries
- **FORBIDDEN**: Request-response patterns across layer boundaries upward

### 5. Testing Dependency Rules

#### Rule 9: Test Isolation
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    // CORRECT: Test with mocked dependencies
    #[tokio::test]
    async fn test_stream_processor_isolation() {
        let mock_ingester = Arc::new(MockDataIngester::new());
        let mock_storage = Arc::new(MockFeatureStore::new());
        let mock_router = Arc::new(MockStreamRouter::new());
        
        let processor = StreamProcessor::new(mock_ingester, mock_storage, mock_router);
        // Test processor in complete isolation
    }
    
    // FORBIDDEN: Test requiring real dependencies
    #[tokio::test]
    async fn test_with_real_database() {
        let processor = StreamProcessor::new_with_real_deps(); // ❌ FORBIDDEN
    }
}
```

#### Rule 10: Contract Testing
```rust
// REQUIRED: Test contract compliance for all implementations
#[cfg(test)]
mod contract_tests {
    use crate::testing::contracts::*;
    
    #[tokio::test]
    async fn test_postgres_feature_store_contract() {
        let store = PostgresFeatureStore::new_test();
        assert_feature_store_contract(store).await;
    }
    
    #[tokio::test]
    async fn test_redis_feature_store_contract() {
        let store = RedisFeatureStore::new_test();
        assert_feature_store_contract(store).await;
    }
}
```

### 6. Configuration Dependency Rules

#### Rule 11: Configuration Injection
```rust
// CORRECT: Configuration passed at startup
#[derive(Debug, Clone)]
pub struct DataPlatformConfig {
    pub feature_store_config: FeatureStoreConfig,
    pub stream_router_config: StreamRouterConfig,
    pub processing_config: ProcessingConfig,
}

impl DataPlatform {
    pub fn new(config: DataPlatformConfig) -> Result<Self, ConfigError> {
        let feature_store = create_feature_store(&config.feature_store_config)?;
        let stream_router = create_stream_router(&config.stream_router_config)?;
        Ok(Self { feature_store, stream_router })
    }
}

// FORBIDDEN: Runtime configuration lookup
impl DataPlatform {
    pub fn new() -> Result<Self, ConfigError> {
        let config = ConfigManager::global().get_config(); // ❌ FORBIDDEN
        // ...
    }
}
```

#### Rule 12: Environment Isolation
- **REQUIRED**: All external dependencies configurable through environment
- **FORBIDDEN**: Hardcoded external system addresses or credentials
- **REQUIRED**: Different implementations for different environments

### 7. Package/Module Structure Rules

#### Rule 13: Module Organization
```
src/
├── infrastructure/           # Infrastructure layer
│   ├── ingestion/
│   ├── coordination/
│   └── storage/
├── data_platform/           # Data platform layer  
│   ├── processing/
│   ├── features/
│   └── routing/
├── discovery_engine/        # Discovery engine layer
│   ├── pattern_detection/
│   ├── neural_analysis/
│   └── claude_integration/
├── execution_domains/       # Execution domains
│   ├── trading/
│   ├── monitoring/
│   └── betting/
└── shared/                  # Shared abstractions only
    ├── types/
    ├── traits/
    └── errors/
```

#### Rule 14: Import Restrictions
```rust
// CORRECT: Import from same or lower layers
use crate::data_platform::FeatureStore;
use crate::infrastructure::DataIngester;

// FORBIDDEN: Import from higher layers
use crate::execution_domains::TradingDomain; // ❌ FORBIDDEN in discovery_engine

// FORBIDDEN: Cross-layer concrete imports
use crate::infrastructure::concrete::PostgresIngester; // ❌ FORBIDDEN from data_platform
```

### 8. Deployment Dependency Rules

#### Rule 15: Container Independence
```yaml
# CORRECT: Each layer in separate containers
services:
  data-platform:
    image: platform/data-platform:latest
    depends_on: [infrastructure-layer]
    
  discovery-engine:
    image: platform/discovery-engine:latest
    depends_on: [data-platform]
    
  trading-domain:
    image: platform/trading-domain:latest
    depends_on: [discovery-engine, data-platform]

# FORBIDDEN: Shared containers across layers
```

#### Rule 16: Network Boundaries
- **REQUIRED**: Each layer communicates over network APIs
- **FORBIDDEN**: Shared memory or file system communication across layers
- **REQUIRED**: All cross-layer communication through defined protocols

### 9. Data Access Rules

#### Rule 17: Data Flow Direction
```rust
// CORRECT: Data flows downward through APIs
async fn get_features(&self, entity_id: &str) -> Result<FeatureVector, Error> {
    self.feature_store.get_features(entity_id, time_window).await
}

// CORRECT: Data flows upward through events
async fn publish_prediction(&self, prediction: Prediction) -> Result<(), Error> {
    let event = PredictionEvent::from(prediction);
    self.event_bus.publish("predictions", event).await
}

// FORBIDDEN: Upward data queries
async fn get_trading_positions(&self) -> Result<Vec<Position>, Error> {
    self.trading_domain.get_positions().await // ❌ FORBIDDEN
}
```

#### Rule 18: State Isolation
- **REQUIRED**: Each layer maintains its own state
- **FORBIDDEN**: Shared state across layers
- **REQUIRED**: State synchronization through events only

### 10. Error Handling Dependencies

#### Rule 19: Error Propagation
```rust
// CORRECT: Layer-specific error types
#[derive(Debug, thiserror::Error)]
pub enum DataPlatformError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    
    #[error("Processing error: {0}")]
    Processing(#[from] ProcessingError),
}

// FORBIDDEN: Exposing lower-layer errors directly
#[derive(Debug, thiserror::Error)]
pub enum DataPlatformError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error), // ❌ FORBIDDEN
}
```

## Consequences

### Positive Consequences
1. **Independent Evolution**: Each layer can evolve without affecting others
2. **Testability**: Each component can be tested in isolation
3. **Scalability**: Each layer can scale independently based on its needs
4. **Maintainability**: Clear boundaries make the system easier to understand
5. **Technology Flexibility**: Can swap implementations without affecting other layers

### Negative Consequences
1. **Complexity**: More interfaces and abstractions to maintain
2. **Performance**: Network overhead for cross-layer communication
3. **Development Overhead**: More ceremony for simple operations
4. **Debugging**: Harder to trace issues across layer boundaries

### Risk Mitigation
1. **Tooling**: Build tools to validate dependency rules automatically
2. **Testing**: Comprehensive contract testing between layers
3. **Documentation**: Clear interface specifications and examples
4. **Monitoring**: Observability across layer boundaries

## Compliance Validation

### Automated Checks
```bash
# Dependency validation script
#!/bin/bash
cargo machete                    # Check for unused dependencies
cargo depgraph --all-deps       # Visualize dependency graph
cargo-modules graph --layout lr # Check module structure
```

### Code Review Checklist
- [ ] No upward synchronous dependencies
- [ ] All cross-layer communication through defined interfaces
- [ ] No shared concrete implementations across layers
- [ ] Test isolation maintained
- [ ] Configuration properly injected

### Runtime Validation
```rust
#[cfg(debug_assertions)]
fn validate_dependency_rules() {
    // Runtime checks for dependency violations
    assert!(!has_circular_dependencies());
    assert!(!has_forbidden_upward_calls());
}
```

This ADR establishes strict, enforceable rules that maintain the architectural integrity of the Universal Discovery Platform while enabling independent evolution and scaling of each layer.