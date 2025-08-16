# Universal Discovery Platform - Architectural Decision Records (ADRs)

## ADR Index

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [ADR-001](#adr-001-domain-agnostic-core-architecture) | Domain-Agnostic Core Architecture | Accepted | 2025-08-16 |
| [ADR-002](#adr-002-multi-consumer-stream-architecture) | Multi-Consumer Stream Architecture | Accepted | 2025-08-16 |
| [ADR-003](#adr-003-claude-analysis-only-integration) | Claude Analysis-Only Integration | Accepted | 2025-08-16 |
| [ADR-004](#adr-004-layer-based-independent-scaling) | Layer-Based Independent Scaling | Accepted | 2025-08-16 |
| [ADR-005](#adr-005-container-native-microservice-design) | Container-Native Microservice Design | Accepted | 2025-08-16 |
| [ADR-006](#adr-006-event-driven-upward-communication) | Event-Driven Upward Communication | Accepted | 2025-08-16 |
| [ADR-007](#adr-007-interface-based-dependency-injection) | Interface-Based Dependency Injection | Accepted | 2025-08-16 |
| [ADR-008](#adr-008-rust-primary-language-choice) | Rust as Primary Language Choice | Accepted | 2025-08-16 |
| [ADR-009](#adr-009-timescaledb-for-time-series-storage) | TimescaleDB for Time Series Storage | Accepted | 2025-08-16 |
| [ADR-010](#adr-010-kafka-for-stream-processing) | Kafka for Stream Processing | Accepted | 2025-08-16 |

---

## ADR-001: Domain-Agnostic Core Architecture

### Status
**Accepted** - 2025-08-16

### Context
The Universal Discovery Platform needs to process time series data from diverse domains (financial markets, IoT sensors, system logs, social media streams) while enabling domain-specific execution logic. The challenge is maintaining a unified core that doesn't become coupled to any specific domain.

### Decision
We will implement a completely domain-agnostic core that operates on abstract time series data structures without any domain knowledge.

#### Core Principles:
1. **Universal Time Series Abstraction**: All data normalized to `TimeSeriesPoint` format
2. **Pattern Detection Without Context**: Patterns identified by mathematical/statistical properties only
3. **Pluggable Domain Logic**: All domain-specific knowledge isolated to execution domain plugins
4. **Generic Feature Engineering**: Features extracted based on time series characteristics, not domain semantics

#### Implementation:
```rust
// Domain-agnostic time series representation
#[derive(Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub entity_id: String,
    pub metric_name: String,
    pub value: f64,
    pub metadata: HashMap<String, Value>,
    pub quality_score: f64,
}

// Domain-agnostic pattern detection
pub trait PatternDetector: Send + Sync {
    async fn detect(&self, data: &TimeSeriesStream) -> Result<Vec<Pattern>, DetectionError>;
}

// Domain-specific execution isolated to plugins
pub trait ExecutionDomain: Send + Sync {
    fn domain_name(&self) -> &str;
    async fn execute_action(&self, action: DomainAction) -> Result<ExecutionResult, ExecutionError>;
}
```

### Rationale
1. **Maximum Reusability**: Core platform can be applied to any time series domain
2. **Independent Evolution**: Domain logic can evolve without affecting core platform
3. **Simplified Testing**: Core logic testable without domain-specific complexity
4. **Performance**: No domain-specific branching in hot paths

### Consequences

#### Positive:
- Platform applicable to unlimited domains
- Clear separation of concerns
- Simplified core logic
- Easy to add new domains

#### Negative:
- More abstraction layers
- Potential performance overhead from generic structures
- Domain-specific optimizations require plugin development

#### Mitigation:
- Use zero-cost abstractions where possible
- Provide domain-specific optimization hooks
- Build rich plugin SDK for common patterns

### Compliance Verification:
```rust
#[test]
fn test_core_has_no_domain_dependencies() {
    // Verify core modules don't import domain-specific code
    assert!(!has_imports_matching("src/core/", "trading|finance|iot|monitoring"));
}
```

---

## ADR-002: Multi-Consumer Stream Architecture

### Status
**Accepted** - 2025-08-16

### Context
Multiple execution domains need simultaneous access to the same data streams (e.g., trading and monitoring both need market data). Traditional point-to-point communication would create tight coupling and prevent independent scaling.

### Decision
We will implement a multi-consumer stream architecture where data flows through shared streams that multiple consumers can independently subscribe to.

#### Architecture:
```rust
// Stream router supports multiple consumers per topic
pub trait StreamRouter: Send + Sync {
    async fn publish(&self, topic: &str, data: &[u8]) -> Result<(), RoutingError>;
    async fn subscribe(&self, pattern: &str) -> Result<StreamSubscription, RoutingError>;
    async fn create_consumer_group(&self, group_id: &str, topics: Vec<String>) -> Result<ConsumerGroup, RoutingError>;
}

// Each execution domain subscribes independently
impl TradingDomain {
    pub async fn initialize(&self) -> Result<(), InitError> {
        let subscription = self.stream_router
            .subscribe("market.*")
            .await?;
        self.start_processing_stream(subscription).await
    }
}

impl MonitoringDomain {
    pub async fn initialize(&self) -> Result<(), InitError> {
        let subscription = self.stream_router
            .subscribe("market.*")
            .await?;
        self.start_monitoring_stream(subscription).await
    }
}
```

#### Stream Delivery Guarantees:
- **At-least-once delivery** for critical data (trading, monitoring)
- **At-most-once delivery** for high-volume analytics
- **Exactly-once delivery** for financial transactions

### Rationale
1. **Decoupling**: Execution domains don't know about each other
2. **Independent Scaling**: Each consumer scales based on its needs
3. **Fault Isolation**: One consumer failure doesn't affect others
4. **Flexible Routing**: Can add new consumers without changing producers

### Consequences

#### Positive:
- True microservice independence
- Easy to add new execution domains
- Fault tolerance through isolation
- Flexible deployment topologies

#### Negative:
- Increased infrastructure complexity
- Message ordering challenges across consumers
- Potential message duplication

#### Mitigation:
- Use Kafka's partition-based ordering
- Implement idempotent message processing
- Provide message deduplication utilities

### Performance Requirements:
- **Latency**: < 10ms for real-time streams
- **Throughput**: > 1M messages/second aggregate
- **Durability**: 7-day message retention minimum

---

## ADR-003: Claude Analysis-Only Integration

### Status
**Accepted** - 2025-08-16

### Context
Claude AI integration can provide valuable pattern explanation and context analysis. However, allowing AI to directly execute actions raises concerns about accountability, determinism, and control.

### Decision
Claude integration will be limited to analysis and explanation only, never direct execution of actions.

#### Integration Pattern:
```rust
pub trait ClaudeAnalyzer: Send + Sync {
    // ALLOWED: Analysis and explanation
    async fn explain_pattern(&self, pattern: Pattern, context: AnalysisContext) -> Result<PatternExplanation, AnalysisError>;
    async fn suggest_actions(&self, patterns: Vec<Pattern>, domain: &str) -> Result<Vec<ActionSuggestion>, AnalysisError>;
    async fn analyze_relationships(&self, patterns: Vec<Pattern>) -> Result<RelationshipAnalysis, AnalysisError>;
    
    // FORBIDDEN: Direct execution methods would not be implemented
    // async fn execute_action(&self, action: DomainAction) -> Result<ExecutionResult, ExecutionError>;
}

// Claude suggestions are advisory only
pub struct ActionSuggestion {
    pub suggestion_id: String,
    pub description: String,
    pub rationale: String,
    pub confidence: f64,
    pub required_parameters: HashMap<String, ParameterSpec>,
    // Note: No execute() method
}
```

#### Execution Flow:
1. Pattern detected by mathematical/statistical methods
2. Claude provides explanation and context
3. Execution domain makes final decision based on its rules
4. Human operators maintain ultimate control

### Rationale
1. **Human Accountability**: Humans/deterministic systems make final decisions
2. **Auditability**: All actions traceable to deterministic rules
3. **Reliability**: No dependency on external AI for critical operations
4. **Explainability**: AI provides understanding, not decisions

### Consequences

#### Positive:
- Clear AI/human boundaries
- Auditable decision trail
- System continues functioning if Claude unavailable
- Regulatory compliance friendly

#### Negative:
- Cannot leverage AI for direct optimization
- Requires manual rule implementation
- Potential missed opportunities from AI insights

#### Mitigation:
- Build rich rule engines that can incorporate AI insights
- Provide tools for converting suggestions to rules
- Enable A/B testing of AI-suggested strategies

### Compliance Enforcement:
```rust
// Compiler-enforced: ClaudeAnalyzer trait has no execute methods
// Runtime check: Ensure no execution paths from Claude components
#[cfg(test)]
fn test_claude_cannot_execute_actions() {
    let claude = ClaudeAnalyzer::new();
    // Should not compile:
    // claude.execute_action(action); 
}
```

---

## ADR-004: Layer-Based Independent Scaling

### Status
**Accepted** - 2025-08-16

### Context
Different layers of the platform have different scaling characteristics and bottlenecks. A monolithic scaling approach would be inefficient and expensive.

### Decision
Each layer will scale independently based on its specific metrics and constraints.

#### Scaling Strategies by Layer:

**Infrastructure Layer:**
- Metric: Connection count, message throughput
- Strategy: Horizontal scaling with connection pooling
- Scaling Unit: Individual ingester/coordinator instances

**Data Platform Layer:**
- Metric: Processing latency, queue depth
- Strategy: Dynamic worker pools with auto-scaling
- Scaling Unit: Stream processor workers, storage shards

**Discovery Engine Layer:**
- Metric: Analysis queue depth, model inference time
- Strategy: GPU-aware scaling with model optimization
- Scaling Unit: Pattern detection workers, neural inference instances

**Execution Domains:**
- Metric: Domain-specific performance (trade latency, alert processing time)
- Strategy: Domain-optimized scaling with resource limits
- Scaling Unit: Individual domain service instances

#### Implementation:
```rust
pub trait LayerScaler: Send + Sync {
    async fn analyze_scaling_needs(&self) -> Result<Option<ScalingRequest>, ScalingError>;
    async fn execute_scaling(&self, decision: ScalingDecision) -> Result<(), ScalingError>;
    fn get_scaling_metrics(&self) -> Vec<ScalingMetric>;
}

pub struct PlatformScalingCoordinator {
    layer_scalers: HashMap<LayerId, Box<dyn LayerScaler>>,
    resource_manager: Arc<ResourceManager>,
}
```

### Rationale
1. **Resource Efficiency**: Each layer gets resources based on actual needs
2. **Performance Optimization**: Scaling tuned to layer-specific bottlenecks
3. **Cost Control**: No over-provisioning due to mismatched requirements
4. **Operational Simplicity**: Clear scaling responsibilities per layer

### Consequences

#### Positive:
- Optimal resource utilization
- Layer-specific performance tuning
- Independent scaling decisions
- Clear operational boundaries

#### Negative:
- Complex coordination requirements
- Potential resource conflicts
- More sophisticated monitoring needed

#### Mitigation:
- Implement resource allocation policies
- Build cross-layer scaling coordination
- Provide unified monitoring dashboards

### Scaling Policies:
```yaml
scaling_policies:
  infrastructure:
    min_replicas: 3
    max_replicas: 100
    scale_up_threshold: 0.8
    scale_down_threshold: 0.3
    
  data_platform:
    min_replicas: 5
    max_replicas: 200
    scale_up_threshold: 0.9
    scale_down_threshold: 0.2
    
  discovery_engine:
    min_replicas: 2
    max_replicas: 50
    scale_up_threshold: 0.85
    scale_down_threshold: 0.25
```

---

## ADR-005: Container-Native Microservice Design

### Status
**Accepted** - 2025-08-16

### Context
The platform needs to run efficiently in both local development environments and large-scale cloud deployments. Container-native design enables consistent deployment across environments.

### Decision
All components will be designed as container-native microservices from the ground up, not retrofitted for containers.

#### Design Principles:
1. **12-Factor App Compliance**: Configuration through environment, stateless processes
2. **Health Check Endpoints**: Every service exposes health and readiness checks
3. **Graceful Shutdown**: Services handle SIGTERM properly
4. **Resource Awareness**: Services adjust behavior based on available resources
5. **Observability Built-in**: Metrics, logging, and tracing integrated

#### Implementation:
```rust
// Every service implements the microservice pattern
pub struct ServiceConfig {
    pub service_name: String,
    pub port: u16,
    pub health_check_interval: Duration,
    pub graceful_shutdown_timeout: Duration,
    pub resource_limits: ResourceLimits,
}

#[async_trait]
pub trait MicroService: Send + Sync {
    async fn start(&self, config: ServiceConfig) -> Result<(), ServiceError>;
    async fn health_check(&self) -> Result<HealthStatus, HealthError>;
    async fn graceful_shutdown(&self) -> Result<(), ShutdownError>;
}

// Container-optimized resource management
pub struct ResourceManager {
    pub fn detect_container_limits() -> ResourceLimits;
    pub fn adjust_for_environment(&self, config: &mut ServiceConfig);
}
```

#### Container Standards:
- **Base Images**: Distroless for security, Alpine for development
- **Multi-stage Builds**: Separate build and runtime environments
- **Security**: Non-root user, minimal attack surface
- **Size Optimization**: Layer caching, dependency optimization

### Rationale
1. **Deployment Consistency**: Same container runs everywhere
2. **Resource Efficiency**: Optimized for container orchestration
3. **Operational Simplicity**: Standard container management tools
4. **Development Velocity**: Easy local development with containers

### Consequences

#### Positive:
- Consistent deployment experience
- Easy local development setup
- Standard operational procedures
- Cloud-native ecosystem compatibility

#### Negative:
- Container orchestration complexity
- Network overhead between services
- Distributed system debugging challenges

#### Mitigation:
- Provide docker-compose for local development
- Implement comprehensive service mesh
- Build strong observability and debugging tools

### Container Requirements:
```dockerfile
# Standard Dockerfile pattern for all services
FROM rust:1.70 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM gcr.io/distroless/cc-debian11
COPY --from=builder /app/target/release/service /usr/local/bin/service
USER 1000
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:8080/health || exit 1
CMD ["service"]
```

---

## ADR-006: Event-Driven Upward Communication

### Status
**Accepted** - 2025-08-16

### Context
Lower layers need to communicate with higher layers (e.g., pattern detection results to execution domains) without creating upward dependencies that would violate the layer architecture.

### Decision
All upward communication will be event-driven through asynchronous message streams, never direct method calls.

#### Communication Patterns:
```rust
// CORRECT: Event-driven upward communication
impl PatternDiscovery {
    async fn publish_pattern(&self, pattern: Pattern) -> Result<(), PublishError> {
        let event = PatternDetectedEvent {
            pattern,
            timestamp: Utc::now(),
            detector_id: self.detector_id.clone(),
        };
        
        self.event_bus
            .publish("patterns.detected", &serde_json::to_vec(&event)?)
            .await
    }
}

// FORBIDDEN: Direct upward calls
impl PatternDiscovery {
    async fn notify_execution_domain(&self, pattern: Pattern) -> Result<(), Error> {
        // This violates layer architecture
        self.execution_domain.handle_pattern(pattern).await
    }
}
```

#### Event Flow Architecture:
1. **Producer**: Lower layer publishes events to message bus
2. **Message Bus**: Routes events to interested consumers
3. **Consumer**: Higher layer subscribes to relevant event patterns
4. **Processing**: Consumer processes events asynchronously

### Rationale
1. **Architectural Integrity**: Maintains unidirectional dependencies
2. **Loose Coupling**: Producers don't know about consumers
3. **Scalability**: Asynchronous processing prevents blocking
4. **Flexibility**: Easy to add new consumers without changing producers

### Consequences

#### Positive:
- Clean layer architecture maintained
- Independent scaling of producers/consumers
- Easy to add new event consumers
- Built-in fault tolerance through message queues

#### Negative:
- Eventual consistency instead of immediate consistency
- More complex error handling across events
- Message ordering and delivery guarantees needed

#### Mitigation:
- Use Kafka's partition-based ordering
- Implement event sourcing for audit trails
- Build comprehensive monitoring for event flows

### Event Schema Standards:
```rust
// Standard event envelope
#[derive(Serialize, Deserialize)]
pub struct PlatformEvent<T> {
    pub event_id: String,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub source_layer: LayerId,
    pub source_component: ComponentId,
    pub payload: T,
    pub correlation_id: Option<String>,
}

// All events implement this trait
pub trait DomainEvent: Serialize + DeserializeOwned {
    fn event_type() -> &'static str;
    fn schema_version() -> &'static str;
}
```

---

## ADR-007: Interface-Based Dependency Injection

### Status
**Accepted** - 2025-08-16

### Context
Components need to depend on other components without tight coupling to specific implementations. This enables testing, modularity, and implementation swapping.

### Decision
All cross-component dependencies will be through trait interfaces, injected at construction time.

#### Dependency Injection Pattern:
```rust
// Define behavior through traits
#[async_trait]
pub trait FeatureStore: Send + Sync {
    async fn store_features(&self, entity_id: &str, features: FeatureVector) -> Result<(), StorageError>;
    async fn get_features(&self, entity_id: &str, window: TimeWindow) -> Result<FeatureMatrix, StorageError>;
}

// Multiple implementations possible
pub struct PostgresFeatureStore { /* ... */ }
pub struct RedisFeatureStore { /* ... */ }
pub struct InMemoryFeatureStore { /* ... */ }

#[async_trait]
impl FeatureStore for PostgresFeatureStore { /* ... */ }

// Components depend on traits, not concrete types
pub struct StreamProcessor {
    feature_store: Arc<dyn FeatureStore>,
    stream_router: Arc<dyn StreamRouter>,
}

impl StreamProcessor {
    pub fn new(
        feature_store: Arc<dyn FeatureStore>,
        stream_router: Arc<dyn StreamRouter>,
    ) -> Self {
        Self { feature_store, stream_router }
    }
}
```

#### Dependency Injection Container:
```rust
pub struct DependencyContainer {
    components: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl DependencyContainer {
    pub fn register<T: Send + Sync + 'static>(&mut self, instance: T) {
        self.components.insert(TypeId::of::<T>(), Box::new(instance));
    }
    
    pub fn resolve<T: Send + Sync + 'static>(&self) -> Result<&T, DIError> {
        self.components
            .get(&TypeId::of::<T>())
            .ok_or(DIError::NotRegistered)?
            .downcast_ref()
            .ok_or(DIError::TypeMismatch)
    }
}
```

### Rationale
1. **Testability**: Easy to inject mocks for testing
2. **Modularity**: Components can be developed independently
3. **Flexibility**: Swap implementations without code changes
4. **Separation of Concerns**: Interface defines contract, implementation provides behavior

### Consequences

#### Positive:
- Excellent testability with mock implementations
- Clear contracts between components
- Easy to swap implementations
- Supports different environments (dev/test/prod)

#### Negative:
- Runtime overhead from dynamic dispatch
- More complex initialization code
- Potential for dependency injection container complexity

#### Mitigation:
- Use static dispatch where performance critical
- Keep dependency graphs simple and flat
- Provide convenience constructors for common configurations

### Testing Benefits:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_stream_processor_with_mocks() {
        let mock_feature_store = Arc::new(MockFeatureStore::new());
        let mock_stream_router = Arc::new(MockStreamRouter::new());
        
        let processor = StreamProcessor::new(mock_feature_store, mock_stream_router);
        
        // Test processor in complete isolation
        let result = processor.process_stream(test_stream()).await;
        assert!(result.is_ok());
    }
}
```

---

## ADR-008: Rust as Primary Language Choice

### Status
**Accepted** - 2025-08-16

### Context
The platform requires high performance, memory safety, and concurrent processing capabilities. Language choice significantly impacts development velocity, runtime performance, and operational characteristics.

### Decision
Rust will be the primary language for all platform components, with Python used only for specific neural network training where ecosystem advantages are compelling.

#### Language Distribution:
- **Core Platform (90%)**: Rust
  - Infrastructure layer
  - Data platform layer
  - Discovery engine layer
  - Execution domains
  - System utilities

- **Neural Training (10%)**: Python
  - Model training scripts
  - Experiment frameworks
  - Data science notebooks

#### Rust Advantages for Our Use Case:
```rust
// Memory safety without garbage collection
pub struct StreamProcessor {
    // No memory leaks, no null pointer dereferences
    buffer: Vec<TimeSeriesPoint>,
    worker_pool: Arc<ThreadPool>,
}

// Zero-cost abstractions
pub trait DataProcessor: Send + Sync {
    fn process(&self, data: &[u8]) -> Result<ProcessedData, ProcessError>;
}
// Trait objects compiled to direct function calls when possible

// Fearless concurrency
pub async fn process_streams_concurrently(
    streams: Vec<DataStream>,
    processor: Arc<dyn DataProcessor>,
) -> Result<Vec<ProcessedStream>, ProcessError> {
    let futures = streams.into_iter().map(|stream| {
        let processor = processor.clone();
        async move {
            processor.process_stream(stream).await
        }
    });
    
    futures::future::try_join_all(futures).await
}
```

### Rationale
1. **Performance**: Zero-cost abstractions, no garbage collection pauses
2. **Memory Safety**: Prevents entire classes of bugs at compile time
3. **Concurrency**: Built-in async/await, fearless concurrency
4. **Ecosystem**: Excellent libraries for systems programming, networking, serialization
5. **Operational**: Single binary deployment, minimal runtime dependencies

### Consequences

#### Positive:
- Excellent runtime performance
- Memory safety guarantees
- Strong type system prevents many bugs
- Great tooling (cargo, clippy, rustfmt)
- Minimal operational overhead

#### Negative:
- Steeper learning curve for some developers
- Longer compile times
- Some neural network libraries not as mature as Python equivalents
- Smaller talent pool

#### Mitigation:
- Provide Rust training and mentoring
- Use incremental compilation and build caching
- Bridge to Python for ML training where needed
- Build internal Rust expertise gradually

### Performance Benchmarks:
```rust
// Target performance characteristics achievable with Rust
const PERFORMANCE_TARGETS: &str = r#"
- Stream Processing: 1M+ messages/second/core
- Memory Usage: <100MB base + O(data) 
- Latency: <1ms for data processing
- CPU Efficiency: >90% utilization under load
- Memory Safety: Zero segfaults, zero memory leaks
"#;
```

---

## ADR-009: TimescaleDB for Time Series Storage

### Status
**Accepted** - 2025-08-16

### Context
The platform needs to store massive amounts of time series data efficiently with fast queries for pattern detection and feature extraction. The storage solution must scale horizontally and provide excellent query performance.

### Decision
TimescaleDB (PostgreSQL extension) will be the primary time series storage solution.

#### Storage Architecture:
```sql
-- Hypertable for time series data with automatic partitioning
CREATE TABLE time_series_data (
    timestamp TIMESTAMPTZ NOT NULL,
    entity_id VARCHAR(256) NOT NULL,
    metric_name VARCHAR(128) NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    metadata JSONB,
    quality_score DOUBLE PRECISION DEFAULT 1.0,
    PRIMARY KEY (timestamp, entity_id, metric_name)
);

-- Convert to hypertable with time-based partitioning
SELECT create_hypertable('time_series_data', 'timestamp', 
    chunk_time_interval => INTERVAL '1 hour');

-- Enable compression for older data
ALTER TABLE time_series_data SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'entity_id, metric_name',
    timescaledb.compress_orderby = 'timestamp DESC'
);

-- Automatic compression policy
SELECT add_compression_policy('time_series_data', INTERVAL '7 days');
```

#### Query Optimization:
```sql
-- Continuous aggregates for common query patterns
CREATE MATERIALIZED VIEW hourly_averages
WITH (timescaledb.continuous) AS
SELECT 
    entity_id,
    metric_name,
    time_bucket('1 hour', timestamp) AS hour,
    AVG(value) as avg_value,
    COUNT(*) as data_points
FROM time_series_data
GROUP BY entity_id, metric_name, hour
WITH NO DATA;

-- Automatic refresh policy
SELECT add_continuous_aggregate_policy('hourly_averages',
    start_offset => INTERVAL '2 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');
```

### Rationale
1. **Performance**: Excellent query performance with time-based partitioning
2. **Scalability**: Horizontal scaling with distributed hypertables
3. **Compression**: 90%+ compression ratios for historical data
4. **SQL Compatibility**: Standard PostgreSQL interface and ecosystem
5. **Operational Maturity**: Proven in production at scale

### Consequences

#### Positive:
- Fast analytical queries with standard SQL
- Automatic data lifecycle management
- Excellent compression for storage efficiency
- Rich ecosystem of tools and integrations
- Strong consistency guarantees

#### Negative:
- PostgreSQL operational complexity
- Limited to single-master writes per hypertable
- Storage costs for high-frequency data

#### Mitigation:
- Use managed TimescaleDB service where possible
- Implement read replicas for query scaling
- Aggressive compression and retention policies

### Performance Expectations:
```yaml
performance_targets:
  ingestion_rate: "100K+ inserts/second"
  query_latency: "<10ms for time range queries"
  compression_ratio: "90% for data >7 days old"
  storage_growth: "1TB/day for 1M entities @ 1Hz"
  concurrent_queries: "1000+ simultaneous analytical queries"
```

---

## ADR-010: Kafka for Stream Processing

### Status
**Accepted** - 2025-08-16

### Context
The platform requires a high-throughput, fault-tolerant message streaming system to handle real-time data flow between components. The solution must support multiple consumers, guaranteed delivery, and horizontal scaling.

### Decision
Apache Kafka will be the primary stream processing backbone for the platform.

#### Kafka Configuration:
```yaml
# Production Kafka cluster configuration
kafka:
  brokers: 12  # 3 controllers + 9 brokers
  replication_factor: 3
  min_insync_replicas: 2
  
  # Performance tuning
  num_network_threads: 16
  num_io_threads: 16
  socket_send_buffer_bytes: 1048576
  socket_receive_buffer_bytes: 1048576
  
  # Topic configuration
  default_partitions: 64
  retention_hours: 168  # 7 days
  compression_type: snappy
  
  # JVM optimization
  heap_size: "16g"
  gc_type: "G1GC"
  max_gc_pause_ms: 20
```

#### Topic Strategy:
```rust
// Topic naming convention
const TOPIC_PATTERNS: &[&str] = &[
    "raw.{source}.{entity_type}",     // Raw data from sources
    "processed.{entity_type}",        // Processed time series
    "patterns.{pattern_type}",        // Detected patterns
    "predictions.{model_type}",       // Neural predictions
    "actions.{domain}",               // Domain actions
    "events.{event_type}",            // System events
];

// Partitioning strategy
fn partition_key(entity_id: &str, metric_name: &str) -> String {
    format!("{}:{}", entity_id, metric_name)
}
```

#### Consumer Groups:
```rust
// Each execution domain has its own consumer group
pub struct DomainStreamConsumer {
    consumer: StreamConsumer,
    group_id: String,
    topics: Vec<String>,
}

impl DomainStreamConsumer {
    pub fn new(domain: &str, topics: Vec<String>) -> Self {
        let group_id = format!("domain-{}", domain);
        let consumer = ClientConfig::new()
            .set("group.id", &group_id)
            .set("bootstrap.servers", "kafka:9092")
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest")
            .create()
            .expect("Consumer creation failed");
            
        Self { consumer, group_id, topics }
    }
}
```

### Rationale
1. **High Throughput**: Millions of messages per second capability
2. **Fault Tolerance**: Replication and partition recovery
3. **Scalability**: Horizontal scaling through partitioning
4. **Durability**: Configurable persistence and retention
5. **Ecosystem**: Rich ecosystem of connectors and tools

### Consequences

#### Positive:
- Proven high-throughput performance
- Strong durability guarantees
- Flexible consumer patterns
- Excellent operational tooling
- Large community and ecosystem

#### Negative:
- Operational complexity (ZooKeeper dependency in older versions)
- Memory and storage requirements
- Learning curve for optimal configuration

#### Mitigation:
- Use KRaft mode (ZooKeeper-free) where possible
- Implement comprehensive monitoring and alerting
- Provide operational runbooks and automation

### Message Guarantees:
```rust
// Producer configuration for different reliability levels
pub enum DeliveryGuarantee {
    AtMostOnce,   // acks=0, retries=0
    AtLeastOnce,  // acks=1, retries>0  
    ExactlyOnce,  // acks=all, enable.idempotence=true
}

pub fn create_producer(guarantee: DeliveryGuarantee) -> FutureProducer {
    let mut config = ClientConfig::new();
    config.set("bootstrap.servers", "kafka:9092");
    
    match guarantee {
        DeliveryGuarantee::AtMostOnce => {
            config.set("acks", "0");
            config.set("retries", "0");
        },
        DeliveryGuarantee::AtLeastOnce => {
            config.set("acks", "1");
            config.set("retries", "10");
        },
        DeliveryGuarantee::ExactlyOnce => {
            config.set("acks", "all");
            config.set("enable.idempotence", "true");
            config.set("retries", "10");
        },
    }
    
    config.create().expect("Producer creation failed")
}
```

---

## ADR Summary

These architectural decisions establish the foundation for a Universal Discovery Platform that is:

1. **Domain-Agnostic**: Core platform works with any time series data
2. **Modular**: Clear boundaries enable independent development and scaling
3. **Scalable**: Each layer scales independently based on its characteristics
4. **Reliable**: Event-driven architecture with fault tolerance
5. **Maintainable**: Strong interfaces and dependency injection enable testing and evolution
6. **Performant**: Rust and optimized infrastructure deliver high throughput and low latency

The decisions are designed to work together as a coherent architectural vision while remaining flexible enough to evolve as requirements change.