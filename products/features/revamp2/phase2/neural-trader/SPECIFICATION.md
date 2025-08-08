# Neural-Trader Multi-Channel Redis Subscription - Phase 2 Specification

## Overview
Transform the neural-trader Rust service from single Redis channel subscription to multi-channel symbol-specific subscriptions to eliminate processing monopolization and enable fair resource allocation.

## Current State Analysis

### Current Implementation Problems
1. **Single Channel Monopolization**: All market data flows through one channel `"market:updates"`
2. **Sequential Processing**: Events processed one at a time in main loop (lines 354-393 in main.rs)  
3. **Symbol Bias**: NVDA and high-volume symbols monopolize processing time
4. **Unfair Resource Allocation**: Single tokio spawn handles all symbols sequentially

### Current Architecture
- **Redis Subscription**: Single `subscribe_market_data("market:updates")` call
- **Processing Flow**: RedisAdapter → EventBus → DAA Coordinator
- **Event Loop**: One async task processes all market events sequentially
- **Bottleneck Location**: Lines 420-439 in main.rs where events are processed by symbol groups

## Phase 2 Requirements

### R1: Multi-Channel Subscription Architecture
- **REQ-001**: Subscribe to individual symbol channels: `market:AAPL`, `market:NVDA`, `market:MSFT`, etc.
- **REQ-002**: Maintain backward compatibility with existing `market:updates` channel
- **REQ-003**: Dynamic subscription management for new symbols
- **REQ-004**: Concurrent subscription handling using tokio async/await patterns

### R2: Fair Processing Algorithm  
- **REQ-005**: No single symbol shall consume >20% of processing time over 1-minute windows
- **REQ-006**: Round-robin processing queue with symbol-specific workers
- **REQ-007**: Configurable processing priority weights per symbol
- **REQ-008**: Adaptive throttling for high-volume symbols

### R3: Concurrent Architecture
- **REQ-009**: Worker pool with `Arc<RwLock<>>` shared state management
- **REQ-010**: Symbol-specific processing queues using `tokio::sync::mpsc` channels
- **REQ-011**: Parallel subscription tasks (one per symbol)
- **REQ-012**: Load balancing across available CPU cores

### R4: Performance Requirements
- **REQ-013**: Processing latency <200ms per market event
- **REQ-014**: Support for 100+ concurrent symbol subscriptions
- **REQ-015**: Memory usage <500MB for full symbol set
- **REQ-016**: Message throughput >10,000 events/second across all symbols

### R5: Redis Integration
- **REQ-017**: Extend `RedisAdapter::subscribe_market_data()` for multi-channel support
- **REQ-018**: Connection pooling for multiple subscriptions
- **REQ-019**: Automatic reconnection handling per channel
- **REQ-020**: Redis pub/sub pattern matching support

### R6: Event Bus Integration
- **REQ-021**: Preserve existing `EventBusIntegration` interface
- **REQ-022**: Symbol-tagged event routing
- **REQ-023**: Performance metrics per symbol
- **REQ-024**: Batch processing optimization

## Technical Specifications

### Data Structures
```rust
// Multi-channel subscription manager
pub struct MultiChannelSubscriptionManager {
    subscriptions: Arc<RwLock<HashMap<String, ChannelSubscription>>>,
    worker_pool: WorkerPool,
    fair_scheduler: FairProcessingScheduler,
    metrics: Arc<RwLock<ChannelMetrics>>,
}

// Per-symbol subscription state
pub struct ChannelSubscription {
    symbol: String,
    channel: String,
    stream: BoxStream<'static, Result<MarketData, AdapterError>>,
    worker_tx: mpsc::Sender<MarketData>,
    stats: ChannelStats,
}

// Fair processing scheduler
pub struct FairProcessingScheduler {
    symbol_queues: HashMap<String, VecDeque<MarketData>>,
    processing_times: HashMap<String, Duration>,
    priority_weights: HashMap<String, f64>,
    last_reset: Instant,
}
```

### Key Interfaces
```rust
impl RedisAdapter {
    // Enhanced multi-channel subscription
    pub async fn subscribe_multiple_channels(
        &self, 
        channels: Vec<String>
    ) -> Result<HashMap<String, BoxStream<MarketData>>, AdapterError>;
    
    // Dynamic channel management
    pub async fn add_channel_subscription(
        &self, 
        channel: &str
    ) -> Result<BoxStream<MarketData>, AdapterError>;
    
    pub async fn remove_channel_subscription(&self, channel: &str) -> Result<(), AdapterError>;
}

impl MultiChannelSubscriptionManager {
    pub async fn subscribe_to_symbols(&mut self, symbols: Vec<String>) -> Result<(), Error>;
    pub async fn get_fair_processing_stats(&self) -> Result<FairProcessingStats, Error>;
    pub async fn rebalance_workers(&mut self) -> Result<(), Error>;
}
```

### Message Format Compatibility
- **Channel Format**: `market:{SYMBOL}` (e.g., `market:AAPL`, `market:NVDA`)
- **Payload Format**: Unchanged JSON structure from existing MarketData
- **Metadata**: Add `channel_source` and `processing_priority` fields

## Implementation Phases

### Phase 2.1: Core Multi-Channel Infrastructure
- Implement `MultiChannelSubscriptionManager`
- Create worker pool architecture
- Add fair processing scheduler
- Basic multi-channel Redis subscriptions

### Phase 2.2: Fair Processing Algorithm
- Implement round-robin scheduling
- Add processing time tracking
- Implement throttling mechanisms
- Performance monitoring integration

### Phase 2.3: Integration & Testing
- Update main.rs event loop
- Integration with existing EventBus
- Comprehensive testing suite
- Performance benchmarks

## Success Criteria
- Fair processing: No symbol >20% processing time
- Latency: <200ms per event processing
- Throughput: >10,000 events/second
- Memory: <500MB total usage
- Zero data loss during transition

## Risk Mitigation
- **Connection Limits**: Use Redis connection pooling
- **Memory Growth**: Implement bounded queues with backpressure
- **Processing Starvation**: Guaranteed minimum processing time per symbol
- **Network Failures**: Per-channel reconnection with exponential backoff

## Coordination with Python Sub-Swarm
- **Channel Format**: Agreed `market:{symbol}` pattern
- **Message Structure**: Compatible JSON format
- **Migration Timing**: Coordinated deployment schedule
- **Testing**: Cross-language integration tests

## Dependencies
- `redis = "0.23"` for pub/sub support
- `tokio = "1.0"` for async/await patterns
- `futures = "0.3"` for stream processing
- `dashmap = "5.0"` for concurrent hash maps
- `serde_json = "1.0"` for message serialization

This specification provides the foundation for transforming the neural-trader service into a fair, concurrent, multi-channel market data processing system that eliminates symbol monopolization while maintaining high performance.