# Neural-Trader Multi-Channel Architecture - Phase 2

## System Architecture Overview

The Phase 2 architecture transforms the neural-trader from a single-channel sequential processor into a multi-channel concurrent system with fair resource allocation.

## Core Components

### 1. Multi-Channel Subscription Manager

```rust
pub struct MultiChannelSubscriptionManager {
    // Core subscription state
    subscriptions: Arc<RwLock<HashMap<String, ChannelSubscription>>>,
    redis_adapter: Arc<RedisAdapter>,
    
    // Worker management
    worker_pool: Arc<WorkerPool>,
    channel_router: Arc<ChannelRouter>,
    
    // Fair processing
    fair_scheduler: Arc<RwLock<FairProcessingScheduler>>,
    processing_metrics: Arc<RwLock<ProcessingMetrics>>,
    
    // Event forwarding
    event_bus: Arc<EventBusIntegration>,
    
    // Configuration
    config: MultiChannelConfig,
    shutdown_signal: Arc<AtomicBool>,
}
```

**Responsibilities:**
- Manage concurrent Redis subscriptions
- Coordinate worker pool allocation
- Route messages to appropriate processors
- Monitor fair processing compliance
- Handle subscription lifecycle

### 2. Worker Pool Architecture

```rust
pub struct WorkerPool {
    workers: Vec<Arc<SymbolWorker>>,
    work_distribution: Arc<RwLock<WorkDistributor>>,
    worker_stats: Arc<RwLock<HashMap<usize, WorkerStats>>>,
    channel_assignments: Arc<RwLock<HashMap<String, usize>>>,
}

pub struct SymbolWorker {
    id: usize,
    message_rx: mpsc::Receiver<WorkItem>,
    event_bus: Arc<EventBusIntegration>,
    processing_stats: Arc<RwLock<SymbolProcessingStats>>,
    shutdown: Arc<AtomicBool>,
}

pub struct WorkItem {
    symbol: String,
    channel: String,
    market_data: MarketData,
    received_at: Instant,
    priority: ProcessingPriority,
}
```

**Worker Pool Design:**
- **Worker Count**: CPU cores × 2 (configurable)
- **Assignment Strategy**: Consistent hashing by symbol
- **Load Balancing**: Dynamic rebalancing based on queue depth
- **Isolation**: Each worker operates independently

### 3. Fair Processing Scheduler

```rust
pub struct FairProcessingScheduler {
    // Processing time tracking
    symbol_processing_times: HashMap<String, TimeWindow>,
    total_processing_time: TimeWindow,
    
    // Queue management
    symbol_queues: HashMap<String, VecDeque<WorkItem>>,
    priority_weights: HashMap<String, f64>,
    
    // Throttling state
    throttled_symbols: HashSet<String>,
    throttle_recovery: HashMap<String, Instant>,
    
    // Fairness parameters
    fairness_window: Duration,
    max_symbol_percentage: f64, // Default: 0.20 (20%)
    rebalance_interval: Duration,
}
```

**Fair Scheduling Algorithm:**
1. **Time Window Tracking**: Rolling 1-minute windows
2. **Percentage Enforcement**: Hard limit at 20% per symbol
3. **Throttling**: Exponential backoff for over-limit symbols
4. **Priority Adjustment**: Dynamic weight adjustment
5. **Recovery**: Automatic throttle removal

### 4. Channel Router

```rust
pub struct ChannelRouter {
    // Routing tables
    channel_to_worker: Arc<RwLock<HashMap<String, usize>>>,
    worker_to_channels: Arc<RwLock<HashMap<usize, Vec<String>>>>,
    
    // Load balancing
    worker_load: Arc<RwLock<Vec<f64>>>,
    rebalance_threshold: f64,
    
    // Message forwarding
    worker_senders: Vec<mpsc::Sender<WorkItem>>,
}
```

**Routing Strategy:**
- **Consistent Hashing**: Same symbol always goes to same worker
- **Load Monitoring**: Track queue depth per worker  
- **Dynamic Rebalancing**: Migrate channels when load imbalanced
- **Failover**: Automatic worker reassignment on failure

## Data Flow Architecture

### Subscription Flow
```
Redis Channels → MultiChannelManager → ChannelRouter → WorkerPool → EventBus → DAA Coordinator
     ↓
market:AAPL ────┐
market:NVDA ────┤
market:MSFT ────┤ → Fair Scheduler → Worker Assignment → Parallel Processing
market:GOOGL ───┤
market:TSLA ────┘
```

### Processing Flow
1. **Redis Subscription**: Parallel subscriptions to `market:{symbol}` channels
2. **Message Reception**: Each channel streams to dedicated receiver
3. **Fair Scheduling**: Messages queued with fairness enforcement
4. **Worker Assignment**: Router assigns to appropriate worker
5. **Parallel Processing**: Workers process independently
6. **Event Publishing**: Results forwarded to EventBus
7. **DAA Coordination**: DAA agents receive processed events

## Concurrency Architecture

### Thread Model
```rust
// Main subscription threads (one per symbol)
async fn symbol_subscription_task(
    symbol: String,
    redis_adapter: Arc<RedisAdapter>,
    router: Arc<ChannelRouter>,
    scheduler: Arc<RwLock<FairProcessingScheduler>>,
) {
    let mut stream = redis_adapter
        .subscribe_market_data(&format!("market:{}", symbol))
        .await?;
        
    while let Some(result) = stream.next().await {
        match result {
            Ok(market_data) => {
                let work_item = WorkItem {
                    symbol: symbol.clone(),
                    channel: format!("market:{}", symbol),
                    market_data,
                    received_at: Instant::now(),
                    priority: scheduler.read().await.get_priority(&symbol),
                };
                
                // Fair scheduling check
                if !scheduler.write().await.should_process(&symbol) {
                    // Throttle this symbol
                    continue;
                }
                
                // Route to worker
                router.route_work_item(work_item).await?;
            }
            Err(e) => {
                error!("Channel {} error: {}", symbol, e);
                // Implement reconnection logic
            }
        }
    }
}

// Worker processing threads
async fn worker_process_loop(
    worker: Arc<SymbolWorker>,
    event_bus: Arc<EventBusIntegration>,
) {
    while !worker.shutdown.load(Ordering::Relaxed) {
        if let Ok(work_item) = worker.message_rx.recv().await {
            let start_time = Instant::now();
            
            // Process market data
            let market_event = convert_to_market_event(&work_item);
            
            // Publish to event bus
            if let Err(e) = event_bus.publish_market_event(market_event).await {
                error!("Failed to publish event: {}", e);
            }
            
            // Update processing stats
            let processing_time = start_time.elapsed();
            worker.processing_stats.write().await
                .record_processing_time(&work_item.symbol, processing_time);
        }
    }
}
```

### Synchronization Strategy
- **Arc<RwLock<>>**: Shared state with reader preference
- **tokio::sync::mpsc**: Worker communication channels
- **AtomicBool**: Shutdown coordination
- **DashMap**: Lock-free concurrent hash maps for hot paths

## Memory Management

### Memory Architecture
```rust
pub struct MemoryManager {
    // Bounded queues prevent memory explosion
    max_queue_size_per_symbol: usize, // Default: 1000
    total_memory_limit: usize,        // Default: 500MB
    
    // Memory pools for frequent allocations
    market_data_pool: MemoryPool<MarketData>,
    work_item_pool: MemoryPool<WorkItem>,
    
    // Garbage collection
    cleanup_interval: Duration,
    memory_pressure_threshold: f64,
}
```

**Memory Safety Measures:**
- **Bounded Queues**: Prevent unbounded growth
- **Backpressure**: Drop messages when queues full
- **Memory Pools**: Reduce allocation overhead
- **Periodic Cleanup**: Remove stale data
- **Memory Monitoring**: Alert on high usage

## Configuration Management

### Multi-Channel Configuration
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct MultiChannelConfig {
    // Subscription settings
    pub enabled_symbols: Vec<String>,
    pub max_concurrent_subscriptions: usize,
    pub reconnect_interval_ms: u64,
    
    // Worker pool settings  
    pub worker_pool_size: Option<usize>, // Default: num_cpus * 2
    pub worker_queue_size: usize,        // Default: 1000
    
    // Fair processing settings
    pub max_symbol_percentage: f64,      // Default: 0.20
    pub fairness_window_seconds: u64,    // Default: 60
    pub throttle_backoff_ms: u64,        // Default: 1000
    
    // Performance settings
    pub processing_timeout_ms: u64,      // Default: 200
    pub memory_limit_mb: usize,          // Default: 500
    pub enable_metrics: bool,            // Default: true
    
    // Redis settings
    pub redis_connection_pool_size: usize, // Default: 10
    pub redis_command_timeout_ms: u64,     // Default: 5000
}
```

## Error Handling & Resilience

### Error Recovery Strategies
1. **Connection Failures**: Per-channel reconnection with exponential backoff
2. **Processing Errors**: Dead letter queue for failed messages
3. **Worker Failures**: Automatic worker restart and channel reassignment
4. **Memory Pressure**: Graceful degradation with message dropping
5. **Fair Processing Violations**: Automatic throttling and recovery

### Health Monitoring
```rust
pub struct SystemHealth {
    // Subscription health
    pub active_subscriptions: usize,
    pub failed_subscriptions: Vec<String>,
    
    // Processing health
    pub worker_utilization: f64,
    pub average_processing_latency: Duration,
    pub fair_processing_compliance: f64,
    
    // Resource health
    pub memory_usage_mb: usize,
    pub cpu_usage_percentage: f64,
    pub queue_depths: HashMap<String, usize>,
}
```

## Integration Points

### EventBus Integration
- **Preserved Interface**: Existing `EventBusIntegration` methods unchanged
- **Enhanced Metadata**: Add channel source and processing stats
- **Performance Metrics**: Per-symbol processing metrics
- **Batch Optimization**: Symbol-specific batching

### DAA Coordinator Integration  
- **Event Reception**: No changes to DAA event handling
- **Coordination Loop**: Enhanced with fair processing metrics
- **Position Tracking**: Symbol-aware position management
- **Decision Making**: Access to per-symbol processing stats

### Redis Adapter Enhancement
- **Multi-Channel Support**: New `subscribe_multiple_channels()` method
- **Connection Pooling**: Efficient connection management
- **Stream Management**: Concurrent stream handling
- **Reconnection Logic**: Robust error recovery

## Performance Characteristics

### Expected Performance Metrics
- **Throughput**: 10,000+ events/second across all channels
- **Latency**: <200ms per event processing
- **Fair Processing**: No symbol >20% of processing time
- **Memory Usage**: <500MB for 100+ symbols
- **CPU Utilization**: Efficient multi-core usage

### Scalability Factors
- **Horizontal**: Add more worker threads
- **Symbol Count**: Linear scaling up to Redis limits
- **Processing Complexity**: Isolated per worker
- **Memory Growth**: Bounded by configuration

This architecture provides a robust, scalable foundation for fair multi-channel market data processing while maintaining compatibility with existing systems and ensuring high performance under load.