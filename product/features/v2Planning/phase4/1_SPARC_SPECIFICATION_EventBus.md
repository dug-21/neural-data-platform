# SPARC Specification: EventBus Component
## Neural-Trader V2 Phase 4 Implementation

*Document Version*: 1.0  
*Created*: 2025-01-26  
*Status*: Active Specification  
*Component*: EventBus Abstraction Layer

## 1. Problem Statement

The Neural-Trader V2 platform requires a unified EventBus abstraction layer to decouple services and enable reliable, high-performance message passing between components. Currently, Redis Streams implementation exists but lacks the abstraction needed for testing, multiple backends, and proper service boundaries.

### Current Gaps
- No unified EventBus trait interface
- Missing testing implementations (InMemory, Recording)
- Tight coupling to Redis implementation
- Wrong channel naming conventions (`market:*` instead of `stream:symbol:*`)
- No backpressure handling abstraction
- Missing dead letter queue patterns

## 2. Requirements

### 2.1 Functional Requirements

#### Core Messaging
- **FR-1**: Publish messages to named channels with guaranteed delivery
- **FR-2**: Subscribe to channels with consumer group support
- **FR-3**: Acknowledge message processing (success/failure)
- **FR-4**: Support multiple message types via Protocol Buffers
- **FR-5**: Enable multi-channel subscriptions
- **FR-6**: Provide message batching for efficiency

#### Channel Management
- **FR-7**: Support hierarchical channel naming per Redis Streams spec:
  - `stream:symbol:{SYMBOL}` - Individual symbol data
  - `stream:sector:{SECTOR}` - Sector aggregation
  - `stream:portfolio:*` - Portfolio decisions
  - `stream:cross_sector:*` - Cross-sector analysis
  - `stream:ml:*` - ML operations
  - `stream:action:*` - Action layer

#### Error Handling
- **FR-8**: Dead letter queue for failed messages
- **FR-9**: Configurable retry policies with backoff
- **FR-10**: Poison message detection and isolation

### 2.2 Non-Functional Requirements

#### Performance
- **NFR-1**: Support 10,000 messages/second per symbol channel
- **NFR-2**: P99 latency <50ms for symbol channels
- **NFR-3**: P99 latency <500ms for portfolio channels
- **NFR-4**: Memory usage <2.5MB per 1K messages

#### Scalability
- **NFR-5**: Horizontal scaling via consumer groups
- **NFR-6**: Dynamic consumer scaling (2-8 consumers per channel)
- **NFR-7**: Support for 100+ concurrent channels

#### Reliability
- **NFR-8**: Zero message loss with persistence
- **NFR-9**: At-least-once delivery guarantee
- **NFR-10**: Graceful degradation under load

#### Testability
- **NFR-11**: Multiple backend implementations (Redis, InMemory, Recording)
- **NFR-12**: Deterministic testing with Recording backend
- **NFR-13**: Performance benchmarking support

### 2.3 Integration Requirements

#### Existing Components
- **IR-1**: Integrate with existing Redis Streams infrastructure
- **IR-2**: Support existing Protocol Buffer message schemas
- **IR-3**: Work with DAA Coordinator communication patterns
- **IR-4**: Compatible with neural-ml-ops training notifications
- **IR-5**: Support neural-trading execution events

#### Migration Path
- **IR-6**: Gradual migration from direct Redis usage
- **IR-7**: Feature flag for channel naming migration
- **IR-8**: Backward compatibility during transition

## 3. Constraints

### Technical Constraints
- **TC-1**: Must use Rust async/await patterns
- **TC-2**: Protocol Buffers for message serialization
- **TC-3**: Redis Streams as primary production backend
- **TC-4**: Compatible with existing monitoring (Prometheus)

### Resource Constraints
- **RC-1**: Implementation within 1 week (Week 1 of Phase 4)
- **RC-2**: Single engineer allocation
- **RC-3**: Reuse existing Redis adapter code

### Design Constraints
- **DC-1**: Follow trait-based design from V2 plan
- **DC-2**: Maintain clean separation between interface and implementation
- **DC-3**: Support dependency injection for testing

## 4. Success Criteria

### Acceptance Criteria
- [ ] EventBus trait defined with all required methods
- [ ] RedisEventBus implementation passing all tests
- [ ] InMemoryEventBus implementation for unit testing
- [ ] RecordingEventBus implementation for integration testing
- [ ] 100% test coverage for core functionality
- [ ] Performance benchmarks meeting NFRs
- [ ] Migration guide for existing Redis usage

### Performance Metrics
- Throughput: 10K msgs/sec sustained
- Latency: P99 <50ms for high-frequency channels
- Memory: <2.5MB per 1K messages
- Error rate: <0.01% message loss

### Integration Validation
- DAA Coordinator communication verified
- ML Ops training events working
- Trading execution events flowing
- Multi-channel subscriptions functional

## 5. Component Boundaries

### What EventBus IS
- Unified messaging abstraction
- Channel management system
- Consumer group coordinator
- Backpressure controller
- Message batching optimizer

### What EventBus IS NOT
- Message transformation layer (handled by services)
- Business logic processor (services responsibility)
- Storage system (uses backing stores)
- RPC framework (separate gRPC interfaces)

## 6. API Surface

### Core Trait
```rust
#[async_trait]
pub trait EventBus: Send + Sync {
    // Publishing
    async fn publish(&self, channel: &str, event: Event) -> Result<EventId>;
    async fn publish_batch(&self, channel: &str, events: Vec<Event>) -> Result<Vec<EventId>>;
    
    // Subscription
    async fn subscribe(&self, channels: &[String], config: SubscriptionConfig) -> Result<Box<dyn EventSubscriber>>;
    
    // Acknowledgment
    async fn ack(&self, channel: &str, group: &str, event_id: &EventId) -> Result<()>;
    async fn nack(&self, channel: &str, group: &str, event_id: &EventId) -> Result<()>;
    
    // Management
    async fn create_consumer_group(&self, channel: &str, group: &str) -> Result<()>;
    async fn get_channel_info(&self, channel: &str) -> Result<ChannelInfo>;
}
```

### Subscriber Trait
```rust
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    async fn next(&mut self) -> Result<Option<EventEnvelope>>;
    async fn close(&mut self) -> Result<()>;
}
```

### Event Types
```rust
pub struct Event {
    pub event_type: String,
    pub payload: Vec<u8>, // Protocol Buffer encoded
    pub metadata: HashMap<String, String>,
    pub timestamp: i64,
}

pub struct EventEnvelope {
    pub event_id: EventId,
    pub channel: String,
    pub event: Event,
    pub retry_count: u32,
}
```

## 7. Data Flow

### Publishing Flow
1. Service creates Event with Protocol Buffer payload
2. EventBus validates channel name format
3. Batching logic aggregates if configured
4. Backend publishes to channel
5. EventId returned for tracking

### Subscription Flow
1. Service requests subscription with consumer group
2. EventBus creates/joins consumer group
3. Messages pulled in configurable batches
4. Service processes and acknowledges
5. Failed messages sent to DLQ after retries

## 8. Testing Strategy

### Unit Testing
- Trait compliance tests for all implementations
- Channel naming validation
- Message serialization/deserialization
- Error handling scenarios

### Integration Testing
- Multi-service communication patterns
- Consumer group coordination
- Backpressure handling
- Dead letter queue flows

### Performance Testing
- Throughput benchmarks per channel type
- Latency distribution analysis
- Memory usage profiling
- Concurrent channel stress tests

## 9. Migration Plan

### Phase 1: Create Abstraction (Day 1-2)
- Define EventBus trait
- Implement InMemoryEventBus
- Create test harness

### Phase 2: Migrate Redis (Day 3-4)
- Wrap existing Redis adapter
- Fix channel naming conventions
- Add batching and DLQ support

### Phase 3: Service Integration (Day 5)
- Update neural-ml-ops to use EventBus
- Update neural-trading to use EventBus
- Verify DAA Coordinator integration

## 10. Risk Assessment

### Technical Risks
- **Risk**: Performance regression from abstraction
  - **Mitigation**: Benchmark continuously, optimize hot paths
  
- **Risk**: Breaking existing Redis communication
  - **Mitigation**: Feature flag for gradual migration

### Schedule Risks
- **Risk**: 1-week timeline aggressive
  - **Mitigation**: Reuse existing Redis code, focus on abstraction

## Appendix A: Channel Naming Specification

Per Redis Streams specification, all channels MUST follow:
```
stream:domain:identifier
```

Examples:
- `stream:symbol:AAPL`
- `stream:sector:technology`
- `stream:portfolio:decisions`
- `stream:ml:training_requests`

## Appendix B: Message Schema References

All messages use Protocol Buffers defined in:
- `proto/streaming_messages.proto`
- `proto/market_data.proto`
- `proto/ml_ops.proto`
- `proto/trading.proto`

---

*This specification defines the complete requirements for the EventBus component implementation in Week 1 of Phase 4.*