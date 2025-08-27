# Redis Streams to Kafka Migration Guide

## Migration Overview

This guide outlines the step-by-step process for migrating from Redis Streams (1K msgs/sec) to Kafka (1-10M msgs/sec) while maintaining system availability and data integrity throughout the transition.

## Current State Analysis

### Redis Streams Implementation
```rust
// Current Redis Streams implementation (simplified)
use redis::streams::{StreamReadOptions, StreamReadReply};

pub struct RedisEventBus {
    client: redis::Client,
    streams: HashMap<String, String>,
}

impl RedisEventBus {
    pub async fn publish_event(&self, stream: &str, event: &Event) -> Result<(), RedisError> {
        let conn = self.client.get_connection()?;
        let _: String = conn.xadd(stream, "*", &[
            ("event_type", &event.event_type),
            ("data", &serde_json::to_string(&event.data)?),
            ("timestamp", &event.timestamp.to_string()),
        ])?;
        Ok(())
    }
    
    pub async fn consume_events(&self, stream: &str, group: &str) -> Result<Vec<Event>, RedisError> {
        let conn = self.client.get_connection()?;
        let opts = StreamReadOptions::default().group(group, "consumer");
        let reply: StreamReadReply = conn.xread_options(&[stream], &[">"], opts)?;
        
        // Parse and return events
        Ok(self.parse_stream_reply(reply)?)
    }
}
```

### Current Limitations
- **Throughput**: Limited to ~1K messages/second
- **Scaling**: Single Redis instance bottleneck
- **Partitioning**: Manual implementation required
- **Ordering**: Basic stream ordering only
- **Durability**: Limited persistence options
- **Monitoring**: Basic Redis monitoring only

## Migration Strategy: Four-Phase Approach

### Phase 1: Infrastructure Setup (Month 1)

**Objective**: Deploy Kafka infrastructure alongside existing Redis setup

#### 1.1 Kafka Cluster Deployment

```yaml
# kafka-cluster.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: kafka-config
data:
  server.properties: |
    # Basic broker configuration
    broker.id=1
    listeners=PLAINTEXT://0.0.0.0:9092
    log.dirs=/var/lib/kafka/logs
    num.network.threads=8
    num.io.threads=16
    socket.send.buffer.bytes=102400
    socket.receive.buffer.bytes=102400
    socket.request.max.bytes=104857600
    
    # Partitioning and replication
    num.partitions=10
    default.replication.factor=3
    min.insync.replicas=2
    
    # Performance tuning
    compression.type=lz4
    log.compression.type=lz4
    log.segment.bytes=1073741824
    log.retention.hours=168
    
    # Kafka Connect and Schema Registry
    confluent.support.metrics.enable=false
---
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: kafka
spec:
  replicas: 3
  serviceName: kafka-headless
  template:
    spec:
      containers:
      - name: kafka
        image: confluentinc/cp-kafka:7.4.0
        ports:
        - containerPort: 9092
        env:
        - name: KAFKA_ZOOKEEPER_CONNECT
          value: "zookeeper:2181"
        - name: KAFKA_ADVERTISED_LISTENERS
          value: "PLAINTEXT://kafka:9092"
        volumeMounts:
        - name: kafka-storage
          mountPath: /var/lib/kafka/logs
  volumeClaimTemplates:
  - metadata:
      name: kafka-storage
    spec:
      accessModes: ["ReadWriteOnce"]
      resources:
        requests:
          storage: 1Ti
```

#### 1.2 Schema Registry Setup

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: schema-registry
spec:
  replicas: 2
  template:
    spec:
      containers:
      - name: schema-registry
        image: confluentinc/cp-schema-registry:7.4.0
        ports:
        - containerPort: 8081
        env:
        - name: SCHEMA_REGISTRY_HOST_NAME
          value: "schema-registry"
        - name: SCHEMA_REGISTRY_KAFKASTORE_BOOTSTRAP_SERVERS
          value: "kafka:9092"
        - name: SCHEMA_REGISTRY_LISTENERS
          value: "http://0.0.0.0:8081"
```

#### 1.3 Topic Creation

```bash
#!/bin/bash
# create-topics.sh

KAFKA_BROKERS="kafka-broker-1:9092,kafka-broker-2:9092,kafka-broker-3:9092"

# Trading topics
kafka-topics --create --bootstrap-server $KAFKA_BROKERS \
  --topic trading.market_data.v1 --partitions 100 --replication-factor 3 \
  --config min.insync.replicas=2 --config compression.type=lz4

kafka-topics --create --bootstrap-server $KAFKA_BROKERS \
  --topic trading.orders.v1 --partitions 50 --replication-factor 3 \
  --config min.insync.replicas=2 --config cleanup.policy=compact,delete

kafka-topics --create --bootstrap-server $KAFKA_BROKERS \
  --topic trading.positions.v1 --partitions 30 --replication-factor 3 \
  --config min.insync.replicas=2 --config cleanup.policy=compact

# ML topics  
kafka-topics --create --bootstrap-server $KAFKA_BROKERS \
  --topic ml.predictions.v1 --partitions 20 --replication-factor 3 \
  --config min.insync.replicas=2 --config compression.type=gzip

# Risk topics
kafka-topics --create --bootstrap-server $KAFKA_BROKERS \
  --topic risk.alerts.v1 --partitions 10 --replication-factor 3 \
  --config min.insync.replicas=2

# Analytics topics
kafka-topics --create --bootstrap-server $KAFKA_BROKERS \
  --topic analytics.raw.v1 --partitions 50 --replication-factor 3 \
  --config retention.ms=2592000000  # 30 days
```

### Phase 2: Dual-Write Implementation (Month 2)

**Objective**: Write events to both Redis and Kafka simultaneously

#### 2.1 Dual-Write Event Bus

```rust
use async_trait::async_trait;
use rdkafka::producer::{FutureProducer, FutureRecord};
use redis::streams::StreamCommands;

#[async_trait]
pub trait EventBus {
    async fn publish_event(&self, topic: &str, event: &Event) -> Result<(), EventBusError>;
    async fn consume_events(&self, topic: &str, group: &str) -> Result<Vec<Event>, EventBusError>;
}

pub struct DualWriteEventBus {
    redis_client: redis::Client,
    kafka_producer: FutureProducer,
    partitioner: EventPartitioner,
    metrics: EventBusMetrics,
}

impl DualWriteEventBus {
    pub fn new(
        redis_client: redis::Client,
        kafka_producer: FutureProducer,
    ) -> Self {
        Self {
            redis_client,
            kafka_producer,
            partitioner: EventPartitioner::new(),
            metrics: EventBusMetrics::new(),
        }
    }
}

#[async_trait]
impl EventBus for DualWriteEventBus {
    async fn publish_event(&self, topic: &str, event: &Event) -> Result<(), EventBusError> {
        let start_time = Instant::now();
        
        // Write to Redis (existing functionality)
        let redis_result = self.write_to_redis(topic, event).await;
        
        // Write to Kafka (new functionality)  
        let kafka_result = self.write_to_kafka(topic, event).await;
        
        // Record metrics
        self.metrics.record_dual_write(
            start_time.elapsed(),
            redis_result.is_ok(),
            kafka_result.is_ok(),
        );
        
        // For now, Redis failure is critical, Kafka failure is logged
        match (redis_result, kafka_result) {
            (Ok(_), Ok(_)) => Ok(()),
            (Ok(_), Err(kafka_err)) => {
                warn!("Kafka write failed, but Redis succeeded: {}", kafka_err);
                Ok(())
            },
            (Err(redis_err), Ok(_)) => {
                error!("Redis write failed: {}", redis_err);
                Err(EventBusError::Redis(redis_err))
            },
            (Err(redis_err), Err(kafka_err)) => {
                error!("Both Redis and Kafka writes failed: Redis={}, Kafka={}", redis_err, kafka_err);
                Err(EventBusError::DualWriteFailure { redis_err, kafka_err })
            }
        }
    }
    
    async fn consume_events(&self, topic: &str, group: &str) -> Result<Vec<Event>, EventBusError> {
        // Continue consuming from Redis during dual-write phase
        self.consume_from_redis(topic, group).await
    }
}

impl DualWriteEventBus {
    async fn write_to_redis(&self, topic: &str, event: &Event) -> Result<(), redis::RedisError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        let _: String = conn.xadd(topic, "*", &[
            ("event_type", &event.event_type),
            ("data", &serde_json::to_string(&event.data)?),
            ("timestamp", &event.timestamp.to_string()),
            ("id", &event.id.to_string()),
        ]).await?;
        Ok(())
    }
    
    async fn write_to_kafka(&self, topic: &str, event: &Event) -> Result<(), KafkaError> {
        let partition_key = self.partitioner.get_partition_key(event);
        let payload = serde_json::to_string(event)?;
        
        let record = FutureRecord::to(topic)
            .key(&partition_key)
            .payload(&payload)
            .headers(self.build_kafka_headers(event));
            
        self.kafka_producer
            .send(record, Duration::from_secs(5))
            .await
            .map_err(|(e, _)| e)?;
            
        Ok(())
    }
    
    fn build_kafka_headers(&self, event: &Event) -> rdkafka::message::OwnedHeaders {
        rdkafka::message::OwnedHeaders::new()
            .insert(rdkafka::message::Header {
                key: "event_id",
                value: Some(event.id.to_string()),
            })
            .insert(rdkafka::message::Header {
                key: "event_type", 
                value: Some(event.event_type.clone()),
            })
            .insert(rdkafka::message::Header {
                key: "timestamp",
                value: Some(event.timestamp.timestamp().to_string()),
            })
    }
}
```

#### 2.2 Migration Configuration

```rust
#[derive(Debug, Clone)]
pub struct MigrationConfig {
    pub phase: MigrationPhase,
    pub kafka_write_enabled: bool,
    pub kafka_read_enabled: bool,
    pub redis_write_enabled: bool,
    pub redis_read_enabled: bool,
    pub dual_write_timeout: Duration,
    pub consistency_check_enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MigrationPhase {
    Phase1Setup,
    Phase2DualWrite,
    Phase3ReadMigration,
    Phase4Cleanup,
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            phase: MigrationPhase::Phase2DualWrite,
            kafka_write_enabled: true,
            kafka_read_enabled: false,
            redis_write_enabled: true, 
            redis_read_enabled: true,
            dual_write_timeout: Duration::from_secs(10),
            consistency_check_enabled: true,
        }
    }
}
```

#### 2.3 Consistency Monitoring

```rust
pub struct ConsistencyChecker {
    redis_client: redis::Client,
    kafka_consumer: StreamConsumer,
    metrics: ConsistencyMetrics,
}

impl ConsistencyChecker {
    pub async fn verify_event_consistency(&self, time_window: Duration) -> Result<ConsistencyReport, CheckError> {
        let end_time = Utc::now();
        let start_time = end_time - time_window;
        
        // Get events from Redis
        let redis_events = self.get_redis_events(start_time, end_time).await?;
        
        // Get events from Kafka
        let kafka_events = self.get_kafka_events(start_time, end_time).await?;
        
        // Compare event sets
        let report = self.compare_event_sets(redis_events, kafka_events);
        
        // Record metrics
        self.metrics.record_consistency_check(&report);
        
        Ok(report)
    }
    
    fn compare_event_sets(&self, redis_events: Vec<Event>, kafka_events: Vec<Event>) -> ConsistencyReport {
        let redis_ids: HashSet<_> = redis_events.iter().map(|e| &e.id).collect();
        let kafka_ids: HashSet<_> = kafka_events.iter().map(|e| &e.id).collect();
        
        let missing_in_kafka: Vec<_> = redis_ids.difference(&kafka_ids).collect();
        let missing_in_redis: Vec<_> = kafka_ids.difference(&redis_ids).collect();
        
        ConsistencyReport {
            total_redis_events: redis_events.len(),
            total_kafka_events: kafka_events.len(),
            missing_in_kafka: missing_in_kafka.len(),
            missing_in_redis: missing_in_redis.len(),
            consistency_percentage: if redis_events.len() > 0 {
                ((redis_events.len() - missing_in_kafka.len()) as f64 / redis_events.len() as f64) * 100.0
            } else {
                100.0
            },
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug)]
pub struct ConsistencyReport {
    pub total_redis_events: usize,
    pub total_kafka_events: usize,
    pub missing_in_kafka: usize,
    pub missing_in_redis: usize,
    pub consistency_percentage: f64,
    pub timestamp: DateTime<Utc>,
}
```

### Phase 3: Read Migration (Month 3)

**Objective**: Gradually migrate consumers from Redis to Kafka

#### 3.1 Feature Flag-Based Consumer Migration

```rust
pub struct MigrationAwareEventBus {
    redis_bus: RedisEventBus,
    kafka_bus: KafkaEventBus,
    dual_write_bus: DualWriteEventBus,
    config: MigrationConfig,
    feature_flags: FeatureFlags,
}

#[async_trait]
impl EventBus for MigrationAwareEventBus {
    async fn publish_event(&self, topic: &str, event: &Event) -> Result<(), EventBusError> {
        match self.config.phase {
            MigrationPhase::Phase1Setup => {
                self.redis_bus.publish_event(topic, event).await
            },
            MigrationPhase::Phase2DualWrite | MigrationPhase::Phase3ReadMigration => {
                self.dual_write_bus.publish_event(topic, event).await
            },
            MigrationPhase::Phase4Cleanup => {
                self.kafka_bus.publish_event(topic, event).await
            }
        }
    }
    
    async fn consume_events(&self, topic: &str, group: &str) -> Result<Vec<Event>, EventBusError> {
        let consumer_key = format!("{}:{}", topic, group);
        
        if self.feature_flags.is_enabled(&format!("kafka_consumer_{}", consumer_key)) {
            info!("Using Kafka consumer for {}", consumer_key);
            self.kafka_bus.consume_events(topic, group).await
        } else {
            info!("Using Redis consumer for {}", consumer_key);
            self.redis_bus.consume_events(topic, group).await
        }
    }
}

// Feature flag implementation
pub struct FeatureFlags {
    flags: Arc<RwLock<HashMap<String, bool>>>,
}

impl FeatureFlags {
    pub fn is_enabled(&self, flag: &str) -> bool {
        self.flags.read().unwrap().get(flag).copied().unwrap_or(false)
    }
    
    pub async fn enable_flag(&self, flag: &str) {
        self.flags.write().unwrap().insert(flag.to_string(), true);
        info!("Enabled feature flag: {}", flag);
    }
    
    pub async fn disable_flag(&self, flag: &str) {
        self.flags.write().unwrap().insert(flag.to_string(), false);
        info!("Disabled feature flag: {}", flag);
    }
}
```

#### 3.2 Consumer Migration Script

```bash
#!/bin/bash
# migrate-consumers.sh

CONSUMERS=(
    "trading.market_data.v1:trading_signal_generator"
    "trading.market_data.v1:risk_monitor" 
    "trading.orders.v1:order_processor"
    "trading.positions.v1:portfolio_tracker"
    "ml.predictions.v1:action_executor"
)

MIGRATION_DELAY=300  # 5 minutes between migrations

for consumer in "${CONSUMERS[@]}"; do
    echo "Migrating consumer: $consumer"
    
    # Enable Kafka consumer
    curl -X POST "http://neural-trader-api/feature-flags/enable" \
         -H "Content-Type: application/json" \
         -d "{\"flag\": \"kafka_consumer_$consumer\"}"
    
    # Wait for migration delay
    echo "Waiting $MIGRATION_DELAY seconds before next migration..."
    sleep $MIGRATION_DELAY
    
    # Check consumer health
    curl -s "http://neural-trader-api/health/consumer/$consumer" | jq .
done

echo "All consumers migrated to Kafka"
```

#### 3.3 Rollback Mechanism

```rust
pub struct MigrationRollback {
    feature_flags: FeatureFlags,
    metrics: MigrationMetrics,
}

impl MigrationRollback {
    pub async fn rollback_consumer(&self, topic: &str, group: &str, reason: &str) -> Result<(), RollbackError> {
        let consumer_key = format!("{}:{}", topic, group);
        
        warn!("Rolling back consumer {} to Redis: {}", consumer_key, reason);
        
        // Disable Kafka consumer flag
        self.feature_flags.disable_flag(&format!("kafka_consumer_{}", consumer_key)).await;
        
        // Record rollback metrics
        self.metrics.record_rollback(&consumer_key, reason);
        
        // Verify Redis consumer is working
        self.verify_redis_consumer_health(topic, group).await?;
        
        Ok(())
    }
    
    async fn verify_redis_consumer_health(&self, topic: &str, group: &str) -> Result<(), RollbackError> {
        // Implementation to verify Redis consumer can process events
        Ok(())
    }
}
```

### Phase 4: Cleanup and Optimization (Month 4)

**Objective**: Remove Redis dependencies and optimize Kafka performance

#### 4.1 Redis Cleanup

```rust
pub struct RedisCleanup {
    redis_client: redis::Client,
    retention_period: Duration,
}

impl RedisCleanup {
    pub async fn cleanup_old_streams(&self) -> Result<CleanupReport, CleanupError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        let cutoff_time = Utc::now() - self.retention_period;
        
        let streams = self.discover_streams(&mut conn).await?;
        let mut cleanup_report = CleanupReport::new();
        
        for stream in streams {
            let removed_count = self.cleanup_stream(&mut conn, &stream, cutoff_time).await?;
            cleanup_report.add_stream_cleanup(&stream, removed_count);
        }
        
        Ok(cleanup_report)
    }
    
    async fn discover_streams(&self, conn: &mut redis::aio::Connection) -> Result<Vec<String>, redis::RedisError> {
        let keys: Vec<String> = redis::cmd("KEYS").arg("*").query_async(conn).await?;
        
        let mut streams = Vec::new();
        for key in keys {
            let key_type: String = redis::cmd("TYPE").arg(&key).query_async(conn).await?;
            if key_type == "stream" {
                streams.push(key);
            }
        }
        
        Ok(streams)
    }
    
    async fn cleanup_stream(
        &self, 
        conn: &mut redis::aio::Connection, 
        stream: &str, 
        cutoff_time: DateTime<Utc>
    ) -> Result<usize, redis::RedisError> {
        // Implementation to remove old entries from Redis stream
        // Use XTRIM or XDEL commands based on retention policy
        Ok(0)
    }
}
```

#### 4.2 Kafka Performance Optimization

```rust
pub struct KafkaOptimizer {
    admin_client: AdminClient<DefaultClientContext>,
}

impl KafkaOptimizer {
    pub async fn optimize_topic_config(&self, topic: &str) -> Result<(), OptimizationError> {
        let topic_metadata = self.get_topic_metadata(topic).await?;
        let current_config = self.get_topic_config(topic).await?;
        
        let optimized_config = self.calculate_optimal_config(&topic_metadata, &current_config);
        
        if optimized_config != current_config {
            self.apply_config_changes(topic, optimized_config).await?;
            info!("Applied optimizations to topic: {}", topic);
        }
        
        Ok(())
    }
    
    fn calculate_optimal_config(
        &self, 
        metadata: &TopicMetadata, 
        current: &TopicConfig
    ) -> TopicConfig {
        let mut optimized = current.clone();
        
        // Optimize based on message patterns
        if metadata.avg_message_size > 1024 {
            optimized.compression_type = "lz4".to_string();
        } else {
            optimized.compression_type = "snappy".to_string();
        }
        
        // Optimize retention based on throughput
        if metadata.messages_per_second > 10000 {
            optimized.segment_ms = 1800000; // 30 minutes
        } else {
            optimized.segment_ms = 3600000; // 1 hour
        }
        
        // Optimize based on consumer patterns
        if metadata.consumer_count > 10 {
            optimized.min_cleanable_dirty_ratio = 0.1;
        }
        
        optimized
    }
}
```

## Migration Monitoring and Alerting

### Key Metrics to Track

```rust
pub struct MigrationMetrics {
    dual_write_success_rate: Gauge,
    consistency_percentage: Gauge,
    consumer_migration_progress: Gauge,
    kafka_lag: Gauge,
    redis_lag: Gauge,
    error_rates: Counter,
}

impl MigrationMetrics {
    pub fn record_dual_write_result(&self, redis_success: bool, kafka_success: bool) {
        match (redis_success, kafka_success) {
            (true, true) => self.dual_write_success_rate.set(1.0),
            (true, false) => {
                self.dual_write_success_rate.set(0.5);
                self.error_rates.with_label_values(&["kafka_write_failure"]).inc();
            },
            (false, true) => {
                self.dual_write_success_rate.set(0.5);
                self.error_rates.with_label_values(&["redis_write_failure"]).inc();
            },
            (false, false) => {
                self.dual_write_success_rate.set(0.0);
                self.error_rates.with_label_values(&["dual_write_failure"]).inc();
            }
        }
    }
}
```

### Alert Rules

```yaml
# migration-alerts.yaml
groups:
- name: kafka_migration
  rules:
  - alert: DualWriteFailureHigh
    expr: rate(dual_write_failures_total[5m]) > 0.01
    for: 2m
    annotations:
      summary: "High dual-write failure rate during Kafka migration"
      
  - alert: ConsistencyDegraded  
    expr: consistency_percentage < 99.5
    for: 5m
    annotations:
      summary: "Data consistency between Redis and Kafka below threshold"
      
  - alert: KafkaConsumerLag
    expr: kafka_consumer_lag_seconds > 60
    for: 3m
    annotations:
      summary: "Kafka consumer lag exceeding 60 seconds"
      
  - alert: MigrationStalled
    expr: increase(consumer_migrations_completed_total[1h]) == 0 AND migration_phase == 3
    for: 30m
    annotations:
      summary: "Consumer migration has stalled"
```

## Testing Strategy

### 1. Load Testing

```bash
#!/bin/bash
# load-test-migration.sh

# Test dual-write performance
echo "Testing dual-write performance..."
kafka-producer-perf-test --topic trading.test.v1 \
  --num-records 100000 \
  --record-size 1024 \
  --throughput 1000 \
  --producer-props bootstrap.servers=kafka:9092

# Test consumer migration
echo "Testing consumer migration..."
kafka-consumer-perf-test --topic trading.test.v1 \
  --messages 100000 \
  --consumer.config consumer.properties
```

### 2. Chaos Testing

```rust
#[cfg(test)]
mod chaos_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_kafka_broker_failure_during_migration() {
        let migration_bus = setup_migration_bus().await;
        
        // Start dual-write
        let write_handle = tokio::spawn(async move {
            for i in 0..1000 {
                let event = create_test_event(i);
                migration_bus.publish_event("test.topic", &event).await.unwrap();
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
        
        // Simulate Kafka broker failure after 100 events
        tokio::time::sleep(Duration::from_secs(1)).await;
        simulate_kafka_failure().await;
        
        // Verify Redis continues working
        let result = write_handle.await;
        assert!(result.is_ok(), "Dual-write should continue with Redis when Kafka fails");
    }
    
    #[tokio::test]
    async fn test_consumer_rollback_mechanism() {
        let migration_system = setup_migration_system().await;
        
        // Migrate consumer to Kafka
        migration_system.migrate_consumer("test.topic", "test.group").await.unwrap();
        
        // Simulate Kafka consumer failure
        simulate_kafka_consumer_failure().await;
        
        // Verify automatic rollback to Redis
        tokio::time::sleep(Duration::from_secs(5)).await;
        let consumer_status = migration_system.get_consumer_status("test.topic", "test.group").await.unwrap();
        assert_eq!(consumer_status.backend, ConsumerBackend::Redis);
    }
}
```

## Risk Mitigation

### 1. Data Loss Prevention
- **Dual-write validation**: Every event written to both systems
- **Consistency monitoring**: Continuous verification of data integrity
- **Automatic rollback**: Immediate fallback to Redis on Kafka failures

### 2. Performance Impact Minimization  
- **Gradual migration**: Consumer-by-consumer migration with monitoring
- **Feature flags**: Instant rollback capability
- **Load testing**: Validation at each phase

### 3. Operational Continuity
- **Zero-downtime migration**: System remains operational throughout
- **Monitoring retention**: Historical data preserved across migration
- **Rollback procedures**: Well-tested rollback for each phase

This migration guide provides a comprehensive, low-risk approach to transitioning from Redis Streams to Kafka while maintaining system availability and data integrity throughout the process.