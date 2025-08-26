# EventBus Scaling Strategy

## Executive Summary
At scale, the EventBus must handle millions of events per second across global markets within a single data flow architecture: Data-Ingestion → Redis/TimescaleDB → ML-Ops → EventBus → Execution. This document outlines comprehensive scaling strategies from 10K to 10M+ events/second.

## 1. Current vs Target Scale

### Current Baseline
- **Volume**: 348 events/sec (30M events/day)
- **Latency**: P95 < 50ms
- **Channels**: ~100 active channels
- **Consumers**: 10-20 per channel

### Target Scale (3-Year Horizon)
- **Volume**: 10M+ events/sec (864B events/day)
- **Latency**: P95 < 10ms, P99 < 50ms
- **Channels**: 10,000+ active channels
- **Consumers**: 1000+ per channel
- **Geographic**: 5 regions globally

## 2. Scaling Architecture

### 2.1 Horizontal Scaling Strategy

```
┌─────────────────────────────────────────────────────────┐
│                    Load Balancer Layer                   │
│              (HAProxy / Envoy with consistent hashing)   │
└─────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ EventBus     │     │ EventBus     │     │ EventBus     │
│ Node 1       │     │ Node 2       │     │ Node N       │
│ (Partition   │     │ (Partition   │     │ (Partition   │
│  0-99)       │     │  100-199)    │     │  N-N+99)     │
└──────────────┘     └──────────────┘     └──────────────┘
        │                     │                     │
        └─────────────────────┼─────────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────┐
│                    Storage Layer                         │
│         (Redis Cluster / Kafka / ScyllaDB)              │
└─────────────────────────────────────────────────────────┘
```

### 2.2 Backend Scaling Options

#### Option A: Redis Cluster (Current - Good to 100K/sec)
```yaml
redis_cluster:
  mode: cluster
  nodes: 12  # 6 masters, 6 replicas
  shards: 6
  replication_factor: 1
  max_memory_per_node: 64GB
  persistence: AOF with RDB snapshots
  
scaling_limits:
  max_throughput: 100K events/sec
  max_channels: 1000
  max_consumers: 100 per channel
  
pros:
  - Simple deployment
  - Low latency (<5ms)
  - Built-in persistence
  
cons:
  - Memory constraints
  - Limited to 100K/sec
  - Single region optimal
```

#### Option B: Apache Kafka (100K - 1M/sec)
```yaml
kafka_cluster:
  brokers: 15
  partitions_per_topic: 100
  replication_factor: 3
  min_in_sync_replicas: 2
  
scaling_limits:
  max_throughput: 1M events/sec
  max_channels: 10000
  max_consumers: unlimited
  
pros:
  - Proven at scale
  - Excellent durability
  - Multi-region support
  
cons:
  - Higher latency (10-50ms)
  - Operational complexity
  - Storage costs
```

#### Option C: Apache Pulsar (1M - 10M/sec)
```yaml
pulsar_cluster:
  bookies: 20  # Storage nodes
  brokers: 10  # Serving nodes
  zookeeper: 5  # Metadata
  
scaling_limits:
  max_throughput: 10M events/sec
  max_channels: 100000
  max_consumers: unlimited
  
pros:
  - Multi-tenancy
  - Geo-replication native
  - Infinite retention
  - True streaming + queuing
  
cons:
  - Operational complexity
  - Newer technology
  - Larger footprint
```

#### Option D: NATS JetStream (Future - 10M+/sec)
```yaml
nats_cluster:
  servers: 9
  jetstream: enabled
  replicas: 3
  
scaling_limits:
  max_throughput: 10M+ events/sec
  max_channels: unlimited
  max_consumers: unlimited
  
pros:
  - Ultra-low latency (<1ms)
  - Simple operations
  - Small footprint
  
cons:
  - Less mature
  - Limited tooling
```

## 3. Partitioning Strategy

### 3.1 Channel Partitioning
```rust
pub struct PartitionStrategy {
    pub strategy_type: PartitionType,
    pub partition_count: u32,
}

pub enum PartitionType {
    // Hash-based (default)
    HashPartition,     // partition = hash(channel) % partition_count
    
    // Range-based (for ordered data)
    RangePartition,    // partition based on symbol ranges (A-C, D-F, etc.)
    
    // Geo-based (for multi-region)
    GeoPartition,      // partition by geographic region
    
    // Load-based (dynamic)
    LoadBalanced,      // partition based on current load
}

impl EventBus {
    fn get_partition(&self, channel: &str) -> u32 {
        match self.partition_strategy.strategy_type {
            PartitionType::HashPartition => {
                let hash = xxhash::xxh64(channel.as_bytes(), 0);
                (hash % self.partition_strategy.partition_count as u64) as u32
            }
            PartitionType::RangePartition => {
                // Partition by symbol range
                let symbol = extract_symbol(channel);
                match symbol.chars().next() {
                    Some('A'..='F') => 0,
                    Some('G'..='M') => 1,
                    Some('N'..='S') => 2,
                    Some('T'..='Z') => 3,
                    _ => 4,
                }
            }
            PartitionType::GeoPartition => {
                // Route to nearest region
                self.get_nearest_partition()
            }
            PartitionType::LoadBalanced => {
                // Route to least loaded partition
                self.get_least_loaded_partition().await
            }
        }
    }
}
```

### 3.2 Smart Routing
```rust
pub struct SmartRouter {
    partitions: Vec<Partition>,
    health_checker: HealthChecker,
    load_balancer: LoadBalancer,
}

impl SmartRouter {
    pub async fn route(&self, event: Event) -> Result<Partition> {
        // 1. Check partition health
        let healthy_partitions = self.health_checker
            .get_healthy_partitions()
            .await?;
        
        // 2. Apply routing rules
        let partition = match event.priority {
            Priority::Critical => {
                // Route critical events to dedicated partition
                self.get_critical_partition()
            }
            Priority::High => {
                // Route to least loaded healthy partition
                self.load_balancer
                    .select_partition(&healthy_partitions)
                    .await?
            }
            Priority::Normal => {
                // Standard hash-based routing
                self.hash_partition(&event.channel)
            }
        };
        
        // 3. Apply backpressure if needed
        if partition.load > 0.8 {
            self.apply_backpressure().await;
        }
        
        Ok(partition)
    }
}
```

## 4. Multi-Region Deployment

### 4.1 Geographic Distribution
```yaml
regions:
  us-east:
    primary: true
    eventbus_nodes: 10
    storage_nodes: 20
    channels: ["stream:symbol:NYSE:*", "stream:symbol:NASDAQ:*"]
    
  eu-west:
    primary: false
    eventbus_nodes: 8
    storage_nodes: 15
    channels: ["stream:symbol:LSE:*", "stream:symbol:EUREX:*"]
    
  asia-pacific:
    primary: false
    eventbus_nodes: 8
    storage_nodes: 15
    channels: ["stream:symbol:TSE:*", "stream:symbol:HSI:*"]
    
replication:
  mode: active-active
  consistency: eventual
  lag_target: <100ms
  conflict_resolution: last-write-wins
```

### 4.2 Cross-Region Replication
```rust
pub struct CrossRegionReplicator {
    regions: HashMap<String, RegionCluster>,
    replication_lag: HashMap<String, Duration>,
}

impl CrossRegionReplicator {
    pub async fn replicate(&self, event: Event, source_region: &str) {
        // Determine target regions based on event type
        let target_regions = self.get_replication_targets(&event);
        
        // Parallel replication with timeout
        let futures: Vec<_> = target_regions
            .iter()
            .map(|region| {
                let event = event.clone();
                async move {
                    timeout(
                        Duration::from_millis(100),
                        region.replicate(event)
                    ).await
                }
            })
            .collect();
        
        // Wait for majority success
        let results = join_all(futures).await;
        let success_count = results.iter().filter(|r| r.is_ok()).count();
        
        if success_count < target_regions.len() / 2 {
            warn!("Replication degraded: {}/{} regions succeeded", 
                  success_count, target_regions.len());
        }
    }
}
```

## 5. Caching Strategy

### 5.1 Multi-Layer Cache
```rust
pub struct CacheHierarchy {
    l1_cache: Arc<DashMap<String, Event>>,      // Process-local (1ms)
    l2_cache: Arc<Redis>,                       // Node-local (5ms)
    l3_cache: Arc<CDN>,                         // Edge cache (20ms)
}

impl CacheHierarchy {
    pub async fn get(&self, key: &str) -> Option<Event> {
        // L1: Process cache
        if let Some(event) = self.l1_cache.get(key) {
            metrics::increment_counter!("cache.l1.hit");
            return Some(event.clone());
        }
        
        // L2: Redis cache
        if let Ok(Some(event)) = self.l2_cache.get(key).await {
            self.l1_cache.insert(key.to_string(), event.clone());
            metrics::increment_counter!("cache.l2.hit");
            return Some(event);
        }
        
        // L3: CDN cache
        if let Ok(Some(event)) = self.l3_cache.get(key).await {
            self.l2_cache.set(key, &event, 60).await.ok();
            self.l1_cache.insert(key.to_string(), event.clone());
            metrics::increment_counter!("cache.l3.hit");
            return Some(event);
        }
        
        metrics::increment_counter!("cache.miss");
        None
    }
}
```

### 5.2 Cache Warming
```rust
pub async fn warm_cache(eventbus: &EventBus, patterns: Vec<String>) {
    // Pre-load hot channels
    for pattern in patterns {
        let channels = eventbus.list_channels(&pattern).await?;
        
        for channel in channels {
            // Load recent events
            let events = eventbus
                .get_recent_events(&channel, 100)
                .await?;
            
            // Populate cache
            for event in events {
                cache.set(&event.id, event, ttl).await?;
            }
        }
    }
}
```

## 6. Load Balancing

### 6.1 Client-Side Load Balancing
```rust
pub struct SmartClient {
    nodes: Vec<EventBusNode>,
    strategy: LoadBalanceStrategy,
    circuit_breakers: HashMap<String, CircuitBreaker>,
}

pub enum LoadBalanceStrategy {
    RoundRobin,
    LeastConnections,
    WeightedRandom,
    ConsistentHash,
    PowerOfTwoChoices,
}

impl SmartClient {
    pub async fn publish(&self, event: Event) -> Result<EventId> {
        let node = self.select_node(&event.channel).await?;
        
        // Circuit breaker check
        if let Some(breaker) = self.circuit_breakers.get(&node.id) {
            if breaker.is_open() {
                // Failover to backup node
                let backup = self.select_backup_node(&event.channel).await?;
                return backup.publish(event).await;
            }
        }
        
        // Publish with retry
        match timeout(Duration::from_millis(50), node.publish(event.clone())).await {
            Ok(Ok(id)) => Ok(id),
            _ => {
                // Mark node as degraded
                self.mark_degraded(&node.id);
                
                // Retry on different node
                let retry_node = self.select_node(&event.channel).await?;
                retry_node.publish(event).await
            }
        }
    }
}
```

### 6.2 Server-Side Load Distribution
```yaml
ingress:
  type: envoy
  config:
    load_balancing_policy: LEAST_REQUEST
    outlier_detection:
      consecutive_5xx: 5
      interval: 30s
      base_ejection_time: 30s
    circuit_breakers:
      max_connections: 10000
      max_pending_requests: 10000
      max_requests: 10000
      max_retries: 3
    retry_policy:
      retry_on: 5xx,reset,connect-failure
      num_retries: 2
      per_try_timeout: 50ms
```

## 7. Performance Optimization

### 7.1 Batching and Compression
```rust
pub struct BatchingPublisher {
    batch_size: usize,
    batch_timeout: Duration,
    compression: CompressionType,
}

impl BatchingPublisher {
    pub async fn publish_batch(&self, events: Vec<Event>) -> Result<Vec<EventId>> {
        // Batch events by channel
        let mut batches: HashMap<String, Vec<Event>> = HashMap::new();
        
        for event in events {
            batches.entry(event.channel.clone())
                .or_insert_with(Vec::new)
                .push(event);
        }
        
        // Compress and publish each batch
        let mut results = Vec::new();
        
        for (channel, batch) in batches {
            // Compress batch
            let compressed = match self.compression {
                CompressionType::Snappy => snappy::compress(&batch)?,
                CompressionType::LZ4 => lz4::compress(&batch)?,
                CompressionType::Zstd => zstd::compress(&batch, 3)?,
                CompressionType::None => serialize(&batch)?,
            };
            
            // Publish compressed batch
            let ids = self.eventbus
                .publish_compressed(&channel, compressed)
                .await?;
            
            results.extend(ids);
        }
        
        Ok(results)
    }
}
```

### 7.2 Zero-Copy Optimization
```rust
use bytes::Bytes;

pub struct ZeroCopyEvent {
    metadata: Arc<EventMetadata>,
    payload: Bytes,  // Zero-copy bytes
}

impl ZeroCopyEvent {
    pub fn slice(&self, start: usize, end: usize) -> Bytes {
        // No copying, just reference adjustment
        self.payload.slice(start..end)
    }
}
```

## 8. Capacity Planning

### 8.1 Growth Projections
```
Year 1: 10K events/sec (864M events/day)
  - 3 Redis nodes
  - 10TB storage
  - $5K/month

Year 2: 100K events/sec (8.6B events/day)
  - Migrate to Kafka
  - 15 broker nodes
  - 100TB storage
  - $25K/month

Year 3: 1M events/sec (86B events/day)
  - Kafka + edge caching
  - 30 broker nodes
  - 1PB storage
  - $100K/month

Year 5: 10M events/sec (864B events/day)
  - Pulsar/NATS
  - 100+ nodes globally
  - 10PB storage
  - $500K/month
```

### 8.2 Auto-Scaling Rules
```yaml
autoscaling:
  eventbus_nodes:
    min: 3
    max: 100
    metrics:
      - type: cpu
        target: 60%
      - type: memory
        target: 70%
      - type: throughput
        target: 80%
      - type: latency
        target: p95 < 50ms
    
  storage_nodes:
    min: 6
    max: 200
    metrics:
      - type: disk_usage
        target: 70%
      - type: iops
        target: 80%
```

## 9. Monitoring & Observability

### 9.1 Key Metrics
```yaml
golden_signals:
  latency:
    - eventbus.publish.p50
    - eventbus.publish.p95
    - eventbus.publish.p99
    
  traffic:
    - eventbus.events.per_second
    - eventbus.bytes.per_second
    - eventbus.channels.active
    
  errors:
    - eventbus.errors.rate
    - eventbus.errors.by_type
    - eventbus.circuit_breaker.trips
    
  saturation:
    - eventbus.queue.depth
    - eventbus.cpu.usage
    - eventbus.memory.usage
    - eventbus.connections.active
```

### 9.2 Scaling Alerts
```yaml
alerts:
  high_latency:
    condition: p95_latency > 100ms
    action: auto_scale_up
    
  high_throughput:
    condition: events_per_sec > capacity * 0.8
    action: add_partition
    
  storage_full:
    condition: disk_usage > 80%
    action: add_storage_node
    
  regional_lag:
    condition: replication_lag > 1s
    action: investigate_network
```

## 10. Migration Path

### Phase 1: Redis Cluster (Current - 10K/sec)
```bash
# Current implementation
redis-cli --cluster create node1:6379 node2:6379 node3:6379
```

### Phase 2: Redis + Kafka Hybrid (10K - 100K/sec)
```bash
# Add Kafka for high-volume channels
kafka-topics.sh --create --topic stream.symbol.* --partitions 100
```

### Phase 3: Full Kafka Migration (100K - 1M/sec)
```bash
# Migrate all channels to Kafka
./migrate-to-kafka.sh --batch-size 1000 --parallel 10
```

### Phase 4: Next-Gen (Pulsar/NATS) (1M+/sec)
```bash
# Future migration to Pulsar or NATS
pulsar-admin namespaces create neural-trader/eventbus
```

## 11. Cost Optimization

### 11.1 Storage Tiering
```yaml
storage_tiers:
  eventbus_hot:  # EventBus processed events - Last 24 hours
    storage: NVMe SSD
    replication: 3
    cost: $0.50/GB/month
    
  redis_fast:  # Redis real-time data - <1s TTL
    storage: Memory
    replication: 2
    cost: $2.00/GB/month
    
  timescale_historical:  # TimescaleDB historical - >1s retention
    storage: SSD
    replication: 2
    compression: native
    cost: $0.20/GB/month
    
  hot:  # EventBus warm data - 1-24 hours
    storage: NVMe SSD
    replication: 3
    cost: $0.50/GB/month
    
  warm:  # 1-7 days
    storage: SSD
    replication: 2
    cost: $0.10/GB/month
    
  cold:  # 7-30 days
    storage: HDD
    replication: 2
    compression: zstd
    cost: $0.03/GB/month
    
  archive:  # >30 days
    storage: S3/Glacier
    replication: 1
    compression: zstd-max
    cost: $0.004/GB/month
```

### 11.2 Reserved Capacity
```yaml
capacity_reservation:
  baseline:
    nodes: 10
    commitment: 3 years
    discount: 50%
    
  burst:
    nodes: 0-90
    pricing: on-demand
    auto_shutdown: true
```

## 12. Disaster Recovery

### 12.1 Backup Strategy
```yaml
backup:
  frequency:
    incremental: every 15 minutes
    full: daily
    
  retention:
    incremental: 7 days
    full: 30 days
    archive: 7 years
    
  locations:
    primary: same-region S3
    secondary: cross-region S3
    tertiary: glacier
```

### 12.2 Recovery Procedures
```bash
# Recovery time objective (RTO): 15 minutes
# Recovery point objective (RPO): 1 minute

# Automated recovery
./disaster-recovery.sh --scenario region-failure --target us-west

# Manual recovery
kubectl apply -f eventbus-dr-cluster.yaml
./restore-from-backup.sh --timestamp 2024-01-01T12:00:00Z
```

## Summary

**Scaling Strategy by Growth Stage:**

| Stage | Volume | Technology | Nodes | Cost/Month |
|-------|--------|------------|-------|------------|
| Current | 10K/sec | Redis | 3 | $5K |
| Growth | 100K/sec | Kafka | 15 | $25K |
| Scale | 1M/sec | Kafka+Cache | 30 | $100K |
| Hyper | 10M+/sec | Pulsar/NATS | 100+ | $500K |

**Key Recommendations:**
1. Start with Redis Cluster (simple, sufficient for Year 1)
2. Plan Kafka migration at 50K events/sec threshold
3. Implement caching layer before 500K events/sec
4. Consider Pulsar/NATS for 1M+ events/sec
5. Use geographic partitioning for global markets
6. Implement auto-scaling from day 1
7. Monitor golden signals continuously
8. Plan capacity 6 months ahead

The EventBus can scale from current 348 events/sec to 10M+ events/sec through progressive technology upgrades, horizontal scaling, and intelligent partitioning strategies.