# Kafka Partitioning Strategies Implementation

## Overview

This document provides specific implementation details for Kafka partitioning strategies that enable the neural trader to handle millions of events per second with proper data distribution and ordering guarantees.

## Core Partitioning Strategy

### 1. Symbol-Based Partitioning for Trading Events

**Use Case**: Market data, trading orders, position updates
**Guarantee**: All events for a specific symbol are processed in order
**Target Topics**: `trading.market_data.v1`, `trading.orders.v1`, `trading.positions.v1`

```rust
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub struct SymbolPartitioner {
    num_partitions: u32,
}

impl SymbolPartitioner {
    pub fn new(num_partitions: u32) -> Self {
        Self { num_partitions }
    }
    
    pub fn calculate_partition(&self, symbol: &str) -> u32 {
        let mut hasher = DefaultHasher::new();
        symbol.hash(&mut hasher);
        (hasher.finish() % self.num_partitions as u64) as u32
    }
}

// Usage example
#[derive(Debug, Serialize, Deserialize)]
pub struct MarketDataEvent {
    pub symbol: String,
    pub timestamp: i64,
    pub price: f64,
    pub volume: f64,
    pub bid: f64,
    pub ask: f64,
}

impl MarketDataEvent {
    pub fn partition_key(&self) -> String {
        self.symbol.clone()
    }
}

// Kafka producer implementation
use rdkafka::producer::{FutureProducer, FutureRecord};

pub struct TradingEventProducer {
    producer: FutureProducer,
    partitioner: SymbolPartitioner,
}

impl TradingEventProducer {
    pub async fn send_market_data(&self, event: MarketDataEvent) -> Result<(), KafkaError> {
        let partition = self.partitioner.calculate_partition(&event.symbol);
        let payload = serde_json::to_string(&event)?;
        
        let record = FutureRecord::to("trading.market_data.v1")
            .partition(partition as i32)
            .key(&event.symbol)
            .payload(&payload);
            
        self.producer.send(record, Duration::from_secs(5)).await?;
        Ok(())
    }
}
```

### 2. Account-Based Partitioning for Position Events

**Use Case**: Position tracking, portfolio management, risk calculations
**Guarantee**: All events for a specific account are processed in order
**Target Topics**: `trading.positions.v1`, `risk.calculations.v1`

```rust
pub struct AccountPartitioner {
    num_partitions: u32,
}

impl AccountPartitioner {
    pub fn calculate_partition(&self, account_id: &str) -> u32 {
        let mut hasher = DefaultHasher::new();
        account_id.hash(&mut hasher);
        (hasher.finish() % self.num_partitions as u64) as u32
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PositionEvent {
    pub account_id: String,
    pub symbol: String,
    pub position_type: PositionType,
    pub quantity: f64,
    pub average_price: f64,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PositionType {
    Open,
    Update,
    Close,
}

impl PositionEvent {
    pub fn partition_key(&self) -> String {
        self.account_id.clone()
    }
}
```

### 3. Domain-Based Partitioning for Cross-Domain Events

**Use Case**: Events that span multiple domains (ML predictions, risk alerts)
**Guarantee**: Events for specific domain-entity combinations are ordered
**Target Topics**: `ml.predictions.v1`, `risk.alerts.v1`, `monitoring.events.v1`

```rust
pub struct DomainPartitioner {
    num_partitions: u32,
}

impl DomainPartitioner {
    pub fn calculate_partition(&self, domain: &str, entity_id: &str) -> u32 {
        let composite_key = format!("{}:{}", domain, entity_id);
        let mut hasher = DefaultHasher::new();
        composite_key.hash(&mut hasher);
        (hasher.finish() % self.num_partitions as u64) as u32
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MLPredictionEvent {
    pub domain: String,           // "trading"
    pub model_id: String,         // "momentum_model_v2"
    pub symbol: String,           // "AAPL"
    pub prediction: f64,
    pub confidence: f64,
    pub features: Vec<f64>,
    pub timestamp: i64,
}

impl MLPredictionEvent {
    pub fn partition_key(&self) -> String {
        format!("{}:{}", self.domain, self.model_id)
    }
}
```

### 4. Time-Based Partitioning for Analytics

**Use Case**: Historical data analysis, batch processing, reporting
**Guarantee**: Events are distributed across time windows for parallel processing
**Target Topics**: `analytics.raw.v1`, `analytics.aggregated.v1`

```rust
use chrono::{DateTime, Utc, Timelike};

pub struct TimePartitioner {
    num_partitions: u32,
    time_window_hours: u32,
}

impl TimePartitioner {
    pub fn new(num_partitions: u32, time_window_hours: u32) -> Self {
        Self { num_partitions, time_window_hours }
    }
    
    pub fn calculate_partition(&self, timestamp: DateTime<Utc>) -> u32 {
        let window = timestamp.hour() / self.time_window_hours;
        window % self.num_partitions
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyticsEvent {
    pub event_type: String,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

impl AnalyticsEvent {
    pub fn partition_key(&self) -> String {
        self.timestamp.format("%Y%m%d%H").to_string()
    }
}
```

## Production Kafka Configuration

### Topic Configuration for High Throughput

```yaml
# trading.market_data.v1 - High frequency market data
name: trading.market_data.v1
partitions: 100
replication_factor: 3
config:
  min.insync.replicas: 2
  compression.type: lz4
  retention.ms: 604800000  # 7 days
  segment.ms: 3600000      # 1 hour segments
  max.message.bytes: 1048576
  
# trading.orders.v1 - Trade orders
name: trading.orders.v1  
partitions: 50
replication_factor: 3
config:
  min.insync.replicas: 2
  compression.type: snappy
  retention.ms: 2592000000  # 30 days
  cleanup.policy: compact,delete
  
# trading.positions.v1 - Position tracking
name: trading.positions.v1
partitions: 30
replication_factor: 3
config:
  min.insync.replicas: 2
  compression.type: snappy
  retention.ms: 7776000000  # 90 days
  cleanup.policy: compact
  
# ml.predictions.v1 - ML model predictions
name: ml.predictions.v1
partitions: 20
replication_factor: 3
config:
  min.insync.replicas: 2
  compression.type: gzip
  retention.ms: 1209600000  # 14 days
  
# risk.alerts.v1 - Risk management alerts
name: risk.alerts.v1
partitions: 10
replication_factor: 3
config:
  min.insync.replicas: 2
  compression.type: snappy
  retention.ms: 7776000000  # 90 days
  cleanup.policy: delete
```

### Producer Configuration

```rust
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::config::ClientConfig;

pub fn create_high_throughput_producer() -> Result<FutureProducer, KafkaError> {
    ClientConfig::new()
        // Exactly-once semantics
        .set("enable.idempotence", "true")
        .set("transactional.id", "neural-trader-producer")
        .set("acks", "all")
        .set("retries", "2147483647")
        .set("max.in.flight.requests.per.connection", "1")
        
        // Performance optimization
        .set("compression.type", "lz4")
        .set("batch.size", "1048576")  // 1MB batches
        .set("linger.ms", "5")         // 5ms batching window
        .set("buffer.memory", "67108864") // 64MB buffer
        
        // Network optimization
        .set("socket.send.buffer.bytes", "1048576")
        .set("socket.receive.buffer.bytes", "1048576")
        
        // Bootstrap servers
        .set("bootstrap.servers", "kafka-broker-1:9092,kafka-broker-2:9092,kafka-broker-3:9092")
        
        .create()
}

pub fn create_analytics_producer() -> Result<FutureProducer, KafkaError> {
    ClientConfig::new()
        // At-least-once semantics (analytics can handle duplicates)
        .set("acks", "1")
        .set("retries", "10")
        .set("enable.idempotence", "false")
        
        // Higher batching for throughput
        .set("compression.type", "gzip")
        .set("batch.size", "2097152")  // 2MB batches
        .set("linger.ms", "100")       // 100ms batching window
        .set("buffer.memory", "134217728") // 128MB buffer
        
        .set("bootstrap.servers", "kafka-broker-1:9092,kafka-broker-2:9092,kafka-broker-3:9092")
        .create()
}
```

### Consumer Configuration

```rust
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::config::ClientConfig;

pub fn create_trading_consumer(group_id: &str) -> Result<StreamConsumer, KafkaError> {
    ClientConfig::new()
        .set("group.id", group_id)
        .set("bootstrap.servers", "kafka-broker-1:9092,kafka-broker-2:9092,kafka-broker-3:9092")
        
        // Exactly-once consumption
        .set("enable.auto.commit", "false")
        .set("isolation.level", "read_committed")
        
        // Performance settings
        .set("fetch.min.bytes", "1048576")    // 1MB minimum fetch
        .set("fetch.max.wait.ms", "10")       // 10ms max wait
        .set("max.partition.fetch.bytes", "10485760") // 10MB per partition
        
        // Consumer group settings
        .set("session.timeout.ms", "30000")
        .set("heartbeat.interval.ms", "3000")
        .set("auto.offset.reset", "earliest")
        
        .create()
}

pub fn create_analytics_consumer(group_id: &str) -> Result<StreamConsumer, KafkaError> {
    ClientConfig::new()
        .set("group.id", group_id)
        .set("bootstrap.servers", "kafka-broker-1:9092,kafka-broker-2:9092,kafka-broker-3:9092")
        
        // At-least-once consumption with auto-commit
        .set("enable.auto.commit", "true")
        .set("auto.commit.interval.ms", "5000")
        
        // Higher throughput settings
        .set("fetch.min.bytes", "10485760")   // 10MB minimum fetch
        .set("fetch.max.wait.ms", "500")      // 500ms max wait
        .set("max.partition.fetch.bytes", "52428800") // 50MB per partition
        
        .set("auto.offset.reset", "earliest")
        .create()
}
```

## Implementation Example: Trading Event Pipeline

```rust
use tokio_stream::StreamExt;
use futures::TryStreamExt;

pub struct TradingEventPipeline {
    producer: FutureProducer,
    consumer: StreamConsumer,
    partitioner: SymbolPartitioner,
}

impl TradingEventPipeline {
    pub async fn process_market_data_stream(&self) -> Result<(), ProcessingError> {
        let mut stream = self.consumer.stream();
        
        while let Some(message) = stream.next().await {
            match message {
                Ok(m) => {
                    // Deserialize market data
                    let payload = m.payload().ok_or(ProcessingError::EmptyPayload)?;
                    let market_data: MarketDataEvent = serde_json::from_slice(payload)?;
                    
                    // Process the event
                    let processed = self.process_market_data(market_data).await?;
                    
                    // Send to next stage with proper partitioning
                    self.send_processed_event(processed).await?;
                    
                    // Commit offset (exactly-once)
                    self.consumer.commit_message(&m, CommitMode::Async)?;
                }
                Err(e) => {
                    error!("Kafka error: {}", e);
                    // Implement retry logic or dead letter queue
                }
            }
        }
        Ok(())
    }
    
    async fn process_market_data(&self, data: MarketDataEvent) -> Result<ProcessedEvent, ProcessingError> {
        // Apply trading logic, risk checks, etc.
        Ok(ProcessedEvent {
            symbol: data.symbol,
            signal: calculate_trading_signal(&data)?,
            timestamp: Utc::now(),
            confidence: 0.85,
        })
    }
    
    async fn send_processed_event(&self, event: ProcessedEvent) -> Result<(), KafkaError> {
        let partition = self.partitioner.calculate_partition(&event.symbol);
        let payload = serde_json::to_string(&event)?;
        
        let record = FutureRecord::to("trading.signals.v1")
            .partition(partition as i32)
            .key(&event.symbol)
            .payload(&payload);
            
        self.producer.send(record, Duration::from_secs(5)).await?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessedEvent {
    pub symbol: String,
    pub signal: TradingSignal,
    pub timestamp: DateTime<Utc>,
    pub confidence: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TradingSignal {
    Buy { price: f64, quantity: f64 },
    Sell { price: f64, quantity: f64 },
    Hold,
}
```

## Monitoring and Metrics

```rust
use prometheus::{Counter, Histogram, Gauge, register_counter, register_histogram, register_gauge};

pub struct KafkaMetrics {
    messages_produced: Counter,
    messages_consumed: Counter,
    processing_latency: Histogram,
    consumer_lag: Gauge,
    partition_distribution: Gauge,
}

impl KafkaMetrics {
    pub fn new() -> Self {
        Self {
            messages_produced: register_counter!(
                "kafka_messages_produced_total",
                "Total number of messages produced to Kafka"
            ).unwrap(),
            
            messages_consumed: register_counter!(
                "kafka_messages_consumed_total", 
                "Total number of messages consumed from Kafka"
            ).unwrap(),
            
            processing_latency: register_histogram!(
                "kafka_message_processing_duration_seconds",
                "Time spent processing each message"
            ).unwrap(),
            
            consumer_lag: register_gauge!(
                "kafka_consumer_lag_messages",
                "Current consumer lag in messages"
            ).unwrap(),
            
            partition_distribution: register_gauge!(
                "kafka_partition_message_count",
                "Number of messages per partition"
            ).unwrap(),
        }
    }
    
    pub fn record_message_produced(&self, topic: &str) {
        self.messages_produced.inc();
    }
    
    pub fn record_processing_time(&self, duration: Duration) {
        self.processing_latency.observe(duration.as_secs_f64());
    }
}
```

## Performance Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Instant;
    
    #[tokio::test]
    async fn test_high_throughput_partitioning() {
        let partitioner = SymbolPartitioner::new(100);
        let symbols = vec!["AAPL", "GOOGL", "MSFT", "TSLA", "AMZN"];
        
        // Test partition distribution
        let mut partition_counts = HashMap::new();
        
        for _ in 0..100_000 {
            for symbol in &symbols {
                let partition = partitioner.calculate_partition(symbol);
                *partition_counts.entry(partition).or_insert(0) += 1;
            }
        }
        
        // Verify reasonably even distribution
        let avg_count = 100_000 * symbols.len() / 100;
        for (partition, count) in partition_counts {
            let deviation = ((count as f64 - avg_count as f64) / avg_count as f64).abs();
            assert!(deviation < 0.1, "Partition {} has poor distribution: {}", partition, deviation);
        }
    }
    
    #[tokio::test]
    async fn test_processing_latency() {
        let pipeline = TradingEventPipeline::new().await.unwrap();
        let start = Instant::now();
        
        // Process 1000 events
        for i in 0..1000 {
            let event = MarketDataEvent {
                symbol: format!("TEST{}", i % 10),
                timestamp: Utc::now().timestamp_micros(),
                price: 100.0 + (i as f64 * 0.1),
                volume: 1000.0,
                bid: 99.9,
                ask: 100.1,
            };
            
            pipeline.send_market_data(event).await.unwrap();
        }
        
        let duration = start.elapsed();
        let throughput = 1000.0 / duration.as_secs_f64();
        
        assert!(throughput > 1000.0, "Throughput too low: {} msgs/sec", throughput);
        println!("Achieved throughput: {:.2} msgs/sec", throughput);
    }
}
```

This implementation provides the foundation for handling millions of events per second with proper partitioning, ordering guarantees, and performance monitoring. The partitioning strategies ensure optimal data distribution while maintaining the ordering requirements critical for trading systems.