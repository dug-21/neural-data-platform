# Redis Streams Configuration for MVP EventBus

## Executive Summary

This document defines the Redis Streams architecture for the Neural Trader MVP EventBus, designed to handle 100K messages/second with clear migration path to Kafka for future scaling to 1M+ messages/second.

## Architecture Overview

### Redis Streams as EventBus Platform
- **Primary Technology**: Redis Streams with consumer groups
- **Target Throughput**: 100,000 messages/second
- **Latency Target**: <10ms for trading, <50ms for analytics
- **Deployment**: Single Redis instance (clustered for production)
- **Future Migration**: Clear path to Kafka when scaling requirements exceed Redis

## Stream Organization

### 1. Domain-Based Stream Keys

```
trading:market-data     # Real-time market data from Alpaca
trading:signals         # ML model predictions and signals
trading:actions         # Order execution commands and results
trading:events          # System events and status updates
analytics:metrics       # Performance and monitoring data
system:health          # Health checks and system status
```

### 2. Message Structure

```json
{
  "id": "1234567890123-0",
  "domain": "trading",
  "type": "market-data",
  "symbol": "AAPL",
  "timestamp": 1640995200000,
  "data": {
    "price": 150.25,
    "volume": 1000,
    "bid": 150.20,
    "ask": 150.30
  },
  "metadata": {
    "source": "alpaca",
    "schema_version": "1.0",
    "producer_id": "trading-data-ingestion"
  }
}
```

## Consumer Group Strategy

### 1. Service-Based Consumer Groups

```
ingestion-group     # Trading Data Ingestion services
model-exec-group    # Model Execution services  
action-group        # Action Execution services
analytics-group     # Analytics and monitoring
storage-group       # TimescaleDB persistence
```

### 2. Consumer Group Configuration

```redis
# Create consumer groups for each stream
XGROUP CREATE trading:market-data ingestion-group 0 MKSTREAM
XGROUP CREATE trading:market-data model-exec-group 0 MKSTREAM
XGROUP CREATE trading:market-data storage-group 0 MKSTREAM

XGROUP CREATE trading:signals action-group 0 MKSTREAM
XGROUP CREATE trading:signals analytics-group 0 MKSTREAM

XGROUP CREATE trading:actions analytics-group 0 MKSTREAM
XGROUP CREATE trading:actions storage-group 0 MKSTREAM
```

## Implementation Patterns

### 1. Producer Pattern (XADD)

```rust
// Rust implementation using redis-rs
use redis::{Client, Commands};

pub struct RedisProducer {
    client: Client,
}

impl RedisProducer {
    pub async fn publish_market_data(&self, symbol: &str, data: MarketData) -> Result<String, Error> {
        let stream_key = "trading:market-data";
        let message = vec![
            ("domain", "trading"),
            ("type", "market-data"),
            ("symbol", symbol),
            ("timestamp", &data.timestamp.to_string()),
            ("data", &serde_json::to_string(&data)?),
        ];
        
        let mut conn = self.client.get_connection()?;
        let id: String = conn.xadd(stream_key, "*", &message)?;
        Ok(id)
    }
}
```

### 2. Consumer Pattern (XREADGROUP)

```rust
pub struct RedisConsumer {
    client: Client,
    group_name: String,
    consumer_name: String,
}

impl RedisConsumer {
    pub async fn consume_market_data(&self) -> Result<Vec<MarketDataMessage>, Error> {
        let mut conn = self.client.get_connection()?;
        
        let result: StreamReadReply = conn.xread_group(
            &self.group_name,
            &self.consumer_name,
            &["trading:market-data"],
            &[">"], // Read only new messages
        )?;
        
        let messages = self.parse_stream_messages(result)?;
        
        // Acknowledge processed messages
        for msg in &messages {
            conn.xack("trading:market-data", &self.group_name, &[&msg.id])?;
        }
        
        Ok(messages)
    }
}
```

### 3. Error Handling and Retry Logic

```rust
impl RedisConsumer {
    pub async fn handle_pending_messages(&self) -> Result<(), Error> {
        let mut conn = self.client.get_connection()?;
        
        // Check for pending messages (not acknowledged)
        let pending: StreamPendingReply = conn.xpending(
            "trading:market-data",
            &self.group_name,
            "-", "+", 100
        )?;
        
        if pending.count > 0 {
            // Claim and reprocess old pending messages
            let claimed: StreamClaimReply = conn.xclaim(
                "trading:market-data",
                &self.group_name,
                &self.consumer_name,
                60000, // 60 seconds idle time
                &pending.ids
            )?;
            
            self.process_claimed_messages(claimed).await?;
        }
        
        Ok(())
    }
}
```

## Performance Configuration

### 1. Redis Configuration

```redis
# redis.conf optimizations for high throughput
maxmemory 8gb
maxmemory-policy allkeys-lru

# Persistence settings (balance durability vs performance)
save 900 1
save 300 10
save 60 10000

# Network optimizations
tcp-keepalive 300
timeout 0

# Stream-specific settings
stream-node-max-bytes 4kb
stream-node-max-entries 100
```

### 2. Connection Pooling

```rust
use deadpool_redis::{Config, Runtime};

pub struct RedisPool {
    pool: deadpool_redis::Pool,
}

impl RedisPool {
    pub fn new(redis_url: &str) -> Result<Self, Error> {
        let cfg = Config::from_url(redis_url);
        let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
        Ok(Self { pool })
    }
    
    pub async fn get_connection(&self) -> Result<deadpool_redis::Connection, Error> {
        self.pool.get().await.map_err(Into::into)
    }
}
```

## Partitioning Strategy

### 1. Symbol-Based Partitioning

For high-frequency symbols, use multiple streams:

```
trading:market-data:AAPL
trading:market-data:GOOGL
trading:market-data:TSLA
trading:market-data:other    # For low-frequency symbols
```

### 2. Time-Based Partitioning

For analytics and historical data:

```
analytics:metrics:2024-01
analytics:metrics:2024-02
```

## Monitoring and Observability

### 1. Redis Stream Metrics

```rust
pub struct StreamMetrics {
    pub stream_length: u64,
    pub consumer_lag: u64,
    pub pending_messages: u64,
    pub consumers_count: u64,
}

impl StreamMetrics {
    pub async fn collect(conn: &mut Connection, stream: &str) -> Result<Self, Error> {
        let length: u64 = conn.xlen(stream)?;
        
        let info: StreamInfoStreamReply = conn.xinfo_stream(stream)?;
        let groups: StreamInfoGroupsReply = conn.xinfo_groups(stream)?;
        
        Ok(StreamMetrics {
            stream_length: length,
            consumer_lag: info.last_generated_id.parse::<u64>()? - groups[0].last_delivered_id.parse::<u64>()?,
            pending_messages: groups[0].pending,
            consumers_count: groups[0].consumers,
        })
    }
}
```

### 2. Prometheus Metrics Export

```rust
use prometheus::{Gauge, IntGaugeVec, register_int_gauge_vec};

lazy_static! {
    static ref STREAM_LENGTH: IntGaugeVec = register_int_gauge_vec!(
        "redis_stream_length",
        "Current length of Redis streams",
        &["stream_name"]
    ).unwrap();
    
    static ref CONSUMER_LAG: IntGaugeVec = register_int_gauge_vec!(
        "redis_consumer_lag",
        "Consumer lag in Redis streams",
        &["stream_name", "group_name"]
    ).unwrap();
}

pub async fn export_metrics() {
    // Export metrics to Prometheus
    let metrics = StreamMetrics::collect(&mut conn, "trading:market-data").await?;
    STREAM_LENGTH.with_label_values(&["trading:market-data"]).set(metrics.stream_length as i64);
    CONSUMER_LAG.with_label_values(&["trading:market-data", "model-exec-group"]).set(metrics.consumer_lag as i64);
}
```

## Deployment Architecture

### 1. Single Node (MVP)

```yaml
# docker-compose.yml
version: '3.8'
services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
      - ./redis.conf:/usr/local/etc/redis/redis.conf
    command: redis-server /usr/local/etc/redis/redis.conf
    deploy:
      resources:
        limits:
          memory: 8GB
          cpus: '4'
```

### 2. Future Cluster (Production)

```yaml
# Redis Cluster for scaling beyond 100K msgs/sec
services:
  redis-cluster:
    image: redis:7-alpine
    deploy:
      replicas: 6
      resources:
        limits:
          memory: 16GB
          cpus: '8'
```

## Migration Path to Kafka

### 1. Interface Abstraction

```rust
#[async_trait]
pub trait EventBus {
    async fn publish(&self, topic: &str, message: &[u8]) -> Result<String, Error>;
    async fn subscribe(&self, topic: &str, group: &str) -> Result<MessageStream, Error>;
}

pub struct RedisEventBus {
    client: RedisPool,
}

pub struct KafkaEventBus {
    producer: FutureProducer,
    consumer: StreamConsumer,
}

// Both implement EventBus trait for seamless migration
```

### 2. Migration Triggers

**Migrate to Kafka when:**
- Stream length consistently > 1M messages
- Consumer lag > 1 second
- Memory usage > 80% of Redis capacity
- Need for multiple data center replication
- Advanced stream processing requirements

### 3. Migration Strategy

1. **Phase 1**: Deploy Kafka alongside Redis
2. **Phase 2**: Dual-write to both systems
3. **Phase 3**: Switch consumers to Kafka
4. **Phase 4**: Deprecate Redis streams
5. **Phase 5**: Full Kafka deployment

## Performance Benchmarks

### Expected Performance (Single Redis Instance)
- **Throughput**: 100,000 messages/second
- **Latency P50**: 5ms
- **Latency P95**: 15ms
- **Latency P99**: 50ms
- **Memory Usage**: ~4GB for 1M messages
- **CPU Usage**: ~50% of 4-core system

### Bottleneck Analysis
- **Network I/O**: Primary bottleneck at scale
- **Memory**: Secondary bottleneck for large backlogs  
- **CPU**: Minimal impact for simple message routing
- **Disk**: Only impacts persistence, not real-time performance

## Error Handling Patterns

### 1. Message Delivery Guarantees

```rust
pub enum DeliveryGuarantee {
    AtMostOnce,   // Fire and forget
    AtLeastOnce,  // With acknowledgments (default)
    ExactlyOnce,  // Application-level deduplication
}
```

### 2. Dead Letter Queue Pattern

```rust
impl RedisConsumer {
    async fn handle_poison_message(&self, msg: &Message, error: &Error) -> Result<(), Error> {
        let dlq_stream = format!("{}.dlq", msg.stream);
        
        let dlq_message = vec![
            ("original_stream", &msg.stream),
            ("original_id", &msg.id),
            ("error", &error.to_string()),
            ("timestamp", &Utc::now().timestamp().to_string()),
            ("data", &msg.raw_data),
        ];
        
        let mut conn = self.client.get_connection().await?;
        conn.xadd(&dlq_stream, "*", &dlq_message)?;
        
        // Acknowledge the poison message to prevent reprocessing
        conn.xack(&msg.stream, &self.group_name, &[&msg.id])?;
        
        Ok(())
    }
}
```

## Security Considerations

### 1. Authentication

```redis
# Redis AUTH configuration
requirepass your-strong-password
```

### 2. Network Security

```yaml
# Docker network isolation
networks:
  neural-trader:
    driver: bridge
    internal: true
```

### 3. Message Encryption (Optional)

```rust
pub struct EncryptedMessage {
    pub encrypted_data: Vec<u8>,
    pub nonce: Vec<u8>,
}

impl RedisProducer {
    pub async fn publish_encrypted(&self, topic: &str, data: &[u8], key: &[u8]) -> Result<String, Error> {
        let encrypted = encrypt(data, key)?;
        let message = vec![
            ("encrypted_data", base64::encode(&encrypted.encrypted_data)),
            ("nonce", base64::encode(&encrypted.nonce)),
        ];
        
        let mut conn = self.client.get_connection().await?;
        conn.xadd(topic, "*", &message)
    }
}
```

## Conclusion

This Redis Streams configuration provides a robust, scalable foundation for the Neural Trader MVP that can handle 100K messages/second with low latency. The design maintains clean interfaces and clear migration paths to Kafka when scaling requirements exceed Redis capabilities.

**Key Benefits:**
- **Simple Deployment**: Single Redis instance
- **Production Ready**: Consumer groups, persistence, monitoring
- **Low Latency**: <10ms for trading operations
- **Future Proof**: Clear Kafka migration path
- **Cost Effective**: Minimal infrastructure overhead

The architecture supports all MVP requirements while maintaining the flexibility to scale to full production requirements as the system grows.