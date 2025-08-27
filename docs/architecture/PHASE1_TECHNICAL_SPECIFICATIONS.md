# Phase 1: Core Shared Infrastructure - Technical Specifications
## Redis Streams Event Bus & Shared Storage Foundation

### Executive Summary

Phase 1 establishes the **mission-critical shared infrastructure** that all subsequent components depend on. This includes a production-ready Redis Streams event bus, TimescaleDB shared storage, monitoring infrastructure, and configuration management. All components are designed for **100K+ messages/second throughput** with **<10ms latency** requirements.

---

## 1. Redis Streams Event Bus Implementation

### 1.1 Stream Architecture Design

#### Stream Definitions
```rust
// src/streaming/stream_definitions.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub name: String,
    pub max_length: Option<u64>,
    pub retention_policy: RetentionPolicy,
    pub consumer_groups: Vec<ConsumerGroupConfig>,
    pub partitioning_strategy: PartitioningStrategy,
}

#[derive(Debug, Clone)]
pub enum RetentionPolicy {
    MaxLength(u64),        // Keep last N messages
    TimeBased(Duration),    // Keep messages for duration
    SizeBased(u64),        // Keep until size limit
}

#[derive(Debug, Clone)]
pub struct ConsumerGroupConfig {
    pub name: String,
    pub start_position: StartPosition,
    pub consumer_count: u32,
    pub block_time_ms: u64,
    pub max_pending: u32,
}

// Core stream configuration
pub fn get_stream_configs() -> HashMap<String, StreamConfig> {
    let mut configs = HashMap::new();
    
    // Market data stream - high throughput
    configs.insert("trading:market-data".to_string(), StreamConfig {
        name: "trading:market-data".to_string(),
        max_length: Some(1_000_000), // 1M messages
        retention_policy: RetentionPolicy::TimeBased(Duration::from_secs(7 * 24 * 3600)), // 7 days
        consumer_groups: vec![
            ConsumerGroupConfig {
                name: "data-processors".to_string(),
                start_position: StartPosition::Latest,
                consumer_count: 4,
                block_time_ms: 100,
                max_pending: 1000,
            },
            ConsumerGroupConfig {
                name: "storage-writers".to_string(),
                start_position: StartPosition::Latest,
                consumer_count: 2,
                block_time_ms: 1000,
                max_pending: 5000,
            },
        ],
        partitioning_strategy: PartitioningStrategy::BySymbol,
    });
    
    // Neural predictions stream
    configs.insert("trading:predictions".to_string(), StreamConfig {
        name: "trading:predictions".to_string(),
        max_length: Some(100_000),
        retention_policy: RetentionPolicy::TimeBased(Duration::from_secs(30 * 24 * 3600)), // 30 days
        consumer_groups: vec![
            ConsumerGroupConfig {
                name: "action-executors".to_string(),
                start_position: StartPosition::Latest,
                consumer_count: 2,
                block_time_ms: 50, // Low latency for trading
                max_pending: 100,
            },
        ],
        partitioning_strategy: PartitioningStrategy::BySymbol,
    });
    
    configs
}
```

#### Message Format Standardization
```rust
// src/streaming/message_format.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessage {
    pub id: String,                    // Redis stream ID
    pub message_id: Uuid,              // Unique message identifier
    pub timestamp: DateTime<Utc>,      // UTC timestamp
    pub domain: String,                // "trading", "system", etc.
    pub message_type: String,          // "market-data", "prediction", etc.
    pub symbol: Option<String>,        // Asset symbol (if applicable)
    pub data: serde_json::Value,       // Message payload
    pub metadata: MessageMetadata,     // Additional context
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub producer_id: String,           // Source service identifier
    pub schema_version: String,        // Message schema version
    pub correlation_id: Option<Uuid>,  // Request correlation
    pub source: String,                // Data source (alpaca, internal, etc.)
    pub priority: MessagePriority,     // Processing priority
    pub retry_count: u32,              // Retry attempts
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePriority {
    Critical,   // Trading decisions, risk alerts
    High,       // Market data, predictions
    Normal,     // Analytics, reporting
    Low,        // Batch processing, cleanup
}

// Message factory for consistent creation
impl StreamMessage {
    pub fn new(
        domain: &str,
        message_type: &str,
        data: serde_json::Value,
        producer_id: &str,
    ) -> Self {
        Self {
            id: "0-0".to_string(), // Will be set by Redis
            message_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            domain: domain.to_string(),
            message_type: message_type.to_string(),
            symbol: None,
            data,
            metadata: MessageMetadata {
                producer_id: producer_id.to_string(),
                schema_version: "1.0".to_string(),
                correlation_id: None,
                source: "internal".to_string(),
                priority: MessagePriority::Normal,
                retry_count: 0,
            },
        }
    }
    
    pub fn with_symbol(mut self, symbol: &str) -> Self {
        self.symbol = Some(symbol.to_string());
        self
    }
    
    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.metadata.priority = priority;
        self
    }
}
```

### 1.2 Producer Implementation

#### High-Performance Producer
```rust
// src/streaming/producer.rs
use deadpool_redis::{Config, Pool, Runtime};
use redis::{AsyncCommands, RedisResult};
use tokio::sync::RwLock;
use std::sync::Arc;
use crate::streaming::{StreamMessage, MessagePriority};

pub struct StreamProducer {
    pool: Pool,
    metrics: Arc<ProducerMetrics>,
    config: ProducerConfig,
}

#[derive(Debug, Clone)]
pub struct ProducerConfig {
    pub batch_size: usize,           // Messages per batch
    pub batch_timeout_ms: u64,       // Max time to wait for batch
    pub max_retries: u32,            // Retry attempts
    pub retry_delay_ms: u64,         // Delay between retries
    pub enable_compression: bool,     // Compress large messages
}

impl Default for ProducerConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            batch_timeout_ms: 10,
            max_retries: 3,
            retry_delay_ms: 100,
            enable_compression: false,
        }
    }
}

impl StreamProducer {
    pub async fn new(redis_url: &str, config: ProducerConfig) -> Result<Self, StreamError> {
        let cfg = Config::from_url(redis_url);
        let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
        
        Ok(Self {
            pool,
            metrics: Arc::new(ProducerMetrics::new()),
            config,
        })
    }
    
    /// Publish single message
    pub async fn publish(
        &self,
        stream: &str,
        message: StreamMessage,
    ) -> Result<String, StreamError> {
        let start = std::time::Instant::now();
        
        let mut conn = self.pool.get().await?;
        let fields = self.serialize_message(&message)?;
        
        let id: String = conn.xadd(stream, "*", &fields).await?;
        
        // Update metrics
        self.metrics.record_publish(
            stream,
            message.metadata.priority.clone(),
            start.elapsed(),
        ).await;
        
        Ok(id)
    }
    
    /// Publish with batching for high throughput
    pub async fn publish_batch(
        &self,
        stream: &str,
        messages: Vec<StreamMessage>,
    ) -> Result<Vec<String>, StreamError> {
        if messages.is_empty() {
            return Ok(vec![]);
        }
        
        let start = std::time::Instant::now();
        let mut conn = self.pool.get().await?;
        let mut ids = Vec::with_capacity(messages.len());
        
        // Use Redis pipeline for batch publishing
        let mut pipe = redis::pipe();
        
        for message in &messages {
            let fields = self.serialize_message(message)?;
            pipe.xadd(stream, "*", &fields);
        }
        
        let results: Vec<String> = pipe.query_async(&mut conn).await?;
        ids.extend(results);
        
        // Update metrics
        self.metrics.record_batch_publish(
            stream,
            messages.len(),
            start.elapsed(),
        ).await;
        
        Ok(ids)
    }
    
    /// Publish with guaranteed delivery
    pub async fn publish_reliable(
        &self,
        stream: &str,
        message: StreamMessage,
    ) -> Result<String, StreamError> {
        let mut retries = 0;
        let mut last_error = None;
        
        while retries <= self.config.max_retries {
            match self.publish(stream, message.clone()).await {
                Ok(id) => return Ok(id),
                Err(e) => {
                    last_error = Some(e);
                    retries += 1;
                    
                    if retries <= self.config.max_retries {
                        tokio::time::sleep(
                            Duration::from_millis(self.config.retry_delay_ms * retries as u64)
                        ).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| 
            StreamError::PublishFailed("Max retries exceeded".to_string())
        ))
    }
    
    fn serialize_message(&self, message: &StreamMessage) -> Result<Vec<(String, String)>, StreamError> {
        let mut fields = vec![
            ("message_id".to_string(), message.message_id.to_string()),
            ("timestamp".to_string(), message.timestamp.to_rfc3339()),
            ("domain".to_string(), message.domain.clone()),
            ("type".to_string(), message.message_type.clone()),
            ("producer_id".to_string(), message.metadata.producer_id.clone()),
            ("schema_version".to_string(), message.metadata.schema_version.clone()),
            ("source".to_string(), message.metadata.source.clone()),
            ("priority".to_string(), format!("{:?}", message.metadata.priority)),
        ];
        
        if let Some(symbol) = &message.symbol {
            fields.push(("symbol".to_string(), symbol.clone()));
        }
        
        if let Some(correlation_id) = &message.metadata.correlation_id {
            fields.push(("correlation_id".to_string(), correlation_id.to_string()));
        }
        
        // Serialize data payload
        let data_json = serde_json::to_string(&message.data)?;
        fields.push(("data".to_string(), data_json));
        
        Ok(fields)
    }
}

#[derive(Debug)]
pub struct ProducerMetrics {
    messages_published: Arc<RwLock<HashMap<String, u64>>>,
    publish_latency: Arc<RwLock<HashMap<String, Vec<Duration>>>>,
    error_count: Arc<RwLock<u64>>,
}

impl ProducerMetrics {
    pub fn new() -> Self {
        Self {
            messages_published: Arc::new(RwLock::new(HashMap::new())),
            publish_latency: Arc::new(RwLock::new(HashMap::new())),
            error_count: Arc::new(RwLock::new(0)),
        }
    }
    
    pub async fn record_publish(&self, stream: &str, priority: MessagePriority, latency: Duration) {
        let mut published = self.messages_published.write().await;
        *published.entry(stream.to_string()).or_insert(0) += 1;
        
        let mut latencies = self.publish_latency.write().await;
        latencies.entry(stream.to_string()).or_insert_with(Vec::new).push(latency);
    }
    
    pub async fn get_stats(&self) -> ProducerStats {
        let published = self.messages_published.read().await;
        let latencies = self.publish_latency.read().await;
        let errors = *self.error_count.read().await;
        
        ProducerStats {
            total_published: published.values().sum(),
            messages_by_stream: published.clone(),
            avg_latency_ms: latencies.values()
                .flatten()
                .map(|d| d.as_millis() as f64)
                .sum::<f64>() / latencies.values().flatten().count() as f64,
            error_count: errors,
        }
    }
}
```

### 1.3 Consumer Implementation

#### Reliable Consumer with Error Handling
```rust
// src/streaming/consumer.rs
use futures::StreamExt;
use redis::{AsyncCommands, streams::{StreamReadOptions, StreamReadReply}};
use tokio::sync::mpsc;
use std::sync::Arc;

pub struct StreamConsumer {
    pool: Pool,
    group_name: String,
    consumer_name: String,
    config: ConsumerConfig,
    metrics: Arc<ConsumerMetrics>,
    shutdown_tx: mpsc::Sender<()>,
}

#[derive(Debug, Clone)]
pub struct ConsumerConfig {
    pub batch_size: usize,           // Messages per read
    pub block_time_ms: u64,          // Block time for XREADGROUP
    pub idle_timeout_ms: u64,        // Claim messages idle this long
    pub max_pending: u32,            // Maximum pending messages
    pub auto_ack: bool,              // Automatically acknowledge
    pub retry_failed: bool,          // Retry failed messages
}

impl StreamConsumer {
    pub async fn new(
        redis_url: &str,
        group_name: String,
        consumer_name: String,
        config: ConsumerConfig,
    ) -> Result<Self, StreamError> {
        let cfg = Config::from_url(redis_url);
        let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
        let (shutdown_tx, _) = mpsc::channel(1);
        
        Ok(Self {
            pool,
            group_name,
            consumer_name,
            config,
            metrics: Arc::new(ConsumerMetrics::new()),
            shutdown_tx,
        })
    }
    
    /// Start consuming messages from streams
    pub async fn consume<F, Fut>(
        &self,
        streams: Vec<String>,
        mut handler: F,
    ) -> Result<(), StreamError>
    where
        F: FnMut(StreamMessage) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<(), StreamError>> + Send,
    {
        let mut shutdown_rx = self.shutdown_tx.subscribe();
        
        // Ensure consumer groups exist
        for stream in &streams {
            self.ensure_consumer_group(stream).await?;
        }
        
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    tracing::info!("Consumer shutdown requested");
                    break;
                }
                
                result = self.read_messages(&streams) => {
                    match result {
                        Ok(messages) => {
                            for message in messages {
                                let stream = message.stream.clone();
                                let id = message.id.clone();
                                
                                let start = std::time::Instant::now();
                                match handler(message.message).await {
                                    Ok(()) => {
                                        if self.config.auto_ack {
                                            self.acknowledge(&stream, &id).await?;
                                        }
                                        self.metrics.record_success(&stream, start.elapsed()).await;
                                    }
                                    Err(e) => {
                                        tracing::error!("Message processing failed: {}", e);
                                        self.metrics.record_error(&stream).await;
                                        
                                        if self.config.retry_failed {
                                            // Don't acknowledge - will be retried
                                        } else {
                                            self.acknowledge(&stream, &id).await?;
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to read messages: {}", e);
                            tokio::time::sleep(Duration::from_millis(1000)).await;
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn read_messages(&self, streams: &[String]) -> Result<Vec<ConsumedMessage>, StreamError> {
        let mut conn = self.pool.get().await?;
        
        let stream_keys: Vec<&str> = streams.iter().map(|s| s.as_str()).collect();
        let stream_ids: Vec<&str> = vec![">"; streams.len()]; // Read new messages
        
        let opts = StreamReadOptions::default()
            .group(&self.group_name, &self.consumer_name)
            .count(self.config.batch_size)
            .block(self.config.block_time_ms);
        
        let reply: StreamReadReply = conn
            .xread_options(&stream_keys, &stream_ids, &opts)
            .await?;
        
        let mut messages = Vec::new();
        
        for stream_messages in reply.keys {
            let stream_name = stream_messages.key;
            
            for stream_id in stream_messages.ids {
                let message = self.deserialize_message(&stream_id.map)?;
                messages.push(ConsumedMessage {
                    stream: stream_name.clone(),
                    id: stream_id.id,
                    message,
                });
            }
        }
        
        Ok(messages)
    }
    
    async fn ensure_consumer_group(&self, stream: &str) -> Result<(), StreamError> {
        let mut conn = self.pool.get().await?;
        
        // Try to create consumer group, ignore if exists
        let result: Result<String, redis::RedisError> = conn
            .xgroup_create_mkstream(stream, &self.group_name, "0")
            .await;
        
        match result {
            Ok(_) => tracing::info!("Created consumer group {} for stream {}", self.group_name, stream),
            Err(e) if e.to_string().contains("BUSYGROUP") => {
                // Group already exists, this is fine
            }
            Err(e) => return Err(StreamError::Redis(e)),
        }
        
        Ok(())
    }
    
    pub async fn acknowledge(&self, stream: &str, id: &str) -> Result<(), StreamError> {
        let mut conn = self.pool.get().await?;
        let _: u64 = conn.xack(stream, &self.group_name, &[id]).await?;
        Ok(())
    }
    
    /// Claim pending messages that have been idle too long
    pub async fn claim_pending_messages(&self, stream: &str) -> Result<Vec<ConsumedMessage>, StreamError> {
        let mut conn = self.pool.get().await?;
        
        // Get pending messages
        let pending: Vec<redis::streams::StreamPendingData> = conn
            .xpending_count(stream, &self.group_name, "-", "+", 100)
            .await?;
        
        let mut claimed_messages = Vec::new();
        
        if !pending.is_empty() {
            let idle_ids: Vec<String> = pending
                .into_iter()
                .filter(|p| p.last_delivered_ms > self.config.idle_timeout_ms)
                .map(|p| p.id)
                .collect();
            
            if !idle_ids.is_empty() {
                let claimed: StreamReadReply = conn
                    .xclaim(
                        stream,
                        &self.group_name,
                        &self.consumer_name,
                        self.config.idle_timeout_ms,
                        &idle_ids,
                    )
                    .await?;
                
                for stream_messages in claimed.keys {
                    for stream_id in stream_messages.ids {
                        let message = self.deserialize_message(&stream_id.map)?;
                        claimed_messages.push(ConsumedMessage {
                            stream: stream.to_string(),
                            id: stream_id.id,
                            message,
                        });
                    }
                }
            }
        }
        
        Ok(claimed_messages)
    }
    
    fn deserialize_message(&self, fields: &HashMap<String, String>) -> Result<StreamMessage, StreamError> {
        let message_id = fields.get("message_id")
            .ok_or_else(|| StreamError::DeserializationFailed("Missing message_id".to_string()))?
            .parse::<Uuid>()?;
        
        let timestamp = fields.get("timestamp")
            .ok_or_else(|| StreamError::DeserializationFailed("Missing timestamp".to_string()))?
            .parse::<DateTime<Utc>>()?;
        
        let data: serde_json::Value = serde_json::from_str(
            fields.get("data")
                .ok_or_else(|| StreamError::DeserializationFailed("Missing data".to_string()))?
        )?;
        
        Ok(StreamMessage {
            id: "".to_string(), // Set by consumer
            message_id,
            timestamp,
            domain: fields.get("domain").unwrap_or(&"unknown".to_string()).clone(),
            message_type: fields.get("type").unwrap_or(&"unknown".to_string()).clone(),
            symbol: fields.get("symbol").cloned(),
            data,
            metadata: MessageMetadata {
                producer_id: fields.get("producer_id").unwrap_or(&"unknown".to_string()).clone(),
                schema_version: fields.get("schema_version").unwrap_or(&"1.0".to_string()).clone(),
                correlation_id: fields.get("correlation_id")
                    .and_then(|s| s.parse::<Uuid>().ok()),
                source: fields.get("source").unwrap_or(&"unknown".to_string()).clone(),
                priority: match fields.get("priority").map(|s| s.as_str()) {
                    Some("Critical") => MessagePriority::Critical,
                    Some("High") => MessagePriority::High,
                    Some("Low") => MessagePriority::Low,
                    _ => MessagePriority::Normal,
                },
                retry_count: 0,
            },
        })
    }
}

#[derive(Debug)]
struct ConsumedMessage {
    stream: String,
    id: String,
    message: StreamMessage,
}
```

---

## 2. TimescaleDB Shared Storage Implementation

### 2.1 Schema Design

#### Time-Series Optimized Tables
```sql
-- src/storage/schema/001_initial_schema.sql

-- Market data with time-series partitioning
CREATE TABLE market_data (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    price DECIMAL(12,4) NOT NULL,
    volume BIGINT NOT NULL,
    bid DECIMAL(12,4),
    ask DECIMAL(12,4),
    bid_size INTEGER,
    ask_size INTEGER,
    source VARCHAR(20) NOT NULL DEFAULT 'alpaca',
    metadata JSONB,
    
    PRIMARY KEY (time, symbol)
);

-- Convert to hypertable for time-series optimization
SELECT create_hypertable('market_data', 'time', 'symbol', 4);

-- Create indexes for common queries
CREATE INDEX idx_market_data_symbol_time ON market_data (symbol, time DESC);
CREATE INDEX idx_market_data_source_time ON market_data (source, time DESC);
CREATE INDEX idx_market_data_volume ON market_data (volume) WHERE volume > 0;

-- Neural predictions table
CREATE TABLE neural_predictions (
    time TIMESTAMPTZ NOT NULL,
    symbol VARCHAR(20) NOT NULL,
    model_id VARCHAR(50) NOT NULL,
    model_version VARCHAR(20) NOT NULL,
    prediction DECIMAL(8,6) NOT NULL,
    confidence DECIMAL(4,3),
    features JSONB,
    metadata JSONB,
    
    PRIMARY KEY (time, symbol, model_id)
);

SELECT create_hypertable('neural_predictions', 'time', 'symbol', 4);
CREATE INDEX idx_predictions_model_time ON neural_predictions (model_id, time DESC);
CREATE INDEX idx_predictions_confidence ON neural_predictions (confidence) WHERE confidence IS NOT NULL;

-- Trading decisions and executions
CREATE TABLE trading_decisions (
    time TIMESTAMPTZ NOT NULL,
    decision_id UUID NOT NULL DEFAULT gen_random_uuid(),
    symbol VARCHAR(20) NOT NULL,
    action VARCHAR(10) NOT NULL, -- BUY, SELL, HOLD
    quantity INTEGER,
    price DECIMAL(12,4),
    confidence DECIMAL(4,3),
    risk_score DECIMAL(4,3),
    model_prediction DECIMAL(8,6),
    execution_status VARCHAR(20) DEFAULT 'PENDING',
    executed_at TIMESTAMPTZ,
    execution_price DECIMAL(12,4),
    metadata JSONB,
    
    PRIMARY KEY (time, decision_id)
);

SELECT create_hypertable('trading_decisions', 'time');
CREATE INDEX idx_decisions_symbol_time ON trading_decisions (symbol, time DESC);
CREATE INDEX idx_decisions_status ON trading_decisions (execution_status);
CREATE INDEX idx_decisions_action ON trading_decisions (action, time DESC);

-- System events and monitoring
CREATE TABLE system_events (
    time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    event_id UUID NOT NULL DEFAULT gen_random_uuid(),
    service VARCHAR(50) NOT NULL,
    event_type VARCHAR(50) NOT NULL,
    severity VARCHAR(20) NOT NULL, -- DEBUG, INFO, WARN, ERROR, CRITICAL
    message TEXT NOT NULL,
    details JSONB,
    
    PRIMARY KEY (time, event_id)
);

SELECT create_hypertable('system_events', 'time');
CREATE INDEX idx_events_service_time ON system_events (service, time DESC);
CREATE INDEX idx_events_severity_time ON system_events (severity, time DESC) WHERE severity IN ('ERROR', 'CRITICAL');

-- Data retention policies
SELECT add_retention_policy('market_data', INTERVAL '90 days');
SELECT add_retention_policy('neural_predictions', INTERVAL '180 days');
SELECT add_retention_policy('trading_decisions', INTERVAL '7 years'); -- Regulatory requirement
SELECT add_retention_policy('system_events', INTERVAL '1 year');

-- Compression policies for older data
SELECT add_compression_policy('market_data', INTERVAL '7 days');
SELECT add_compression_policy('neural_predictions', INTERVAL '30 days');
SELECT add_compression_policy('system_events', INTERVAL '30 days');
```

### 2.2 Database Connection Management

#### High-Performance Connection Pool
```rust
// src/storage/connection_pool.rs
use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::{NoTls, Row};
use std::sync::Arc;
use serde_json::Value;

#[derive(Clone)]
pub struct DatabasePool {
    pool: Pool,
    config: DatabaseConfig,
    metrics: Arc<PoolMetrics>,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub max_connections: usize,
    pub connection_timeout_secs: u64,
    pub idle_timeout_secs: u64,
    pub max_lifetime_secs: u64,
    pub statement_cache_size: usize,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            max_connections: 20,
            connection_timeout_secs: 30,
            idle_timeout_secs: 600,
            max_lifetime_secs: 3600,
            statement_cache_size: 100,
        }
    }
}

impl DatabasePool {
    pub async fn new(database_url: &str, config: DatabaseConfig) -> Result<Self, DatabaseError> {
        let mut cfg = Config::from_url(database_url)?;
        
        cfg.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        });
        
        cfg.max_size = config.max_connections;
        cfg.timeouts.wait = Some(Duration::from_secs(config.connection_timeout_secs));
        
        let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;
        
        // Test connection
        let conn = pool.get().await?;
        let _: Row = conn.query_one("SELECT 1", &[]).await?;
        
        Ok(Self {
            pool,
            config,
            metrics: Arc::new(PoolMetrics::new()),
        })
    }
    
    pub async fn execute(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64, DatabaseError> {
        let start = std::time::Instant::now();
        let conn = self.pool.get().await?;
        
        let result = conn.execute(query, params).await?;
        
        self.metrics.record_query("execute", start.elapsed()).await;
        Ok(result)
    }
    
    pub async fn query(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>, DatabaseError> {
        let start = std::time::Instant::now();
        let conn = self.pool.get().await?;
        
        let result = conn.query(query, params).await?;
        
        self.metrics.record_query("query", start.elapsed()).await;
        Ok(result)
    }
    
    pub async fn query_one(&self, query: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Row, DatabaseError> {
        let start = std::time::Instant::now();
        let conn = self.pool.get().await?;
        
        let result = conn.query_one(query, params).await?;
        
        self.metrics.record_query("query_one", start.elapsed()).await;
        Ok(result)
    }
    
    /// Optimized bulk insert for high-throughput scenarios
    pub async fn bulk_insert_market_data(
        &self,
        data: Vec<MarketDataPoint>,
    ) -> Result<u64, DatabaseError> {
        if data.is_empty() {
            return Ok(0);
        }
        
        let start = std::time::Instant::now();
        let conn = self.pool.get().await?;
        
        // Use COPY for maximum performance
        let stmt = conn.prepare("
            COPY market_data (time, symbol, price, volume, bid, ask, bid_size, ask_size, source, metadata)
            FROM STDIN BINARY
        ").await?;
        
        let sink = conn.copy_in(&stmt).await?;
        let writer = BinaryCopyInWriter::new(sink, &[
            Type::TIMESTAMPTZ, Type::VARCHAR, Type::NUMERIC, Type::INT8,
            Type::NUMERIC, Type::NUMERIC, Type::INT4, Type::INT4,
            Type::VARCHAR, Type::JSONB,
        ]);
        
        tokio::pin!(writer);
        
        for point in &data {
            let row = (
                point.timestamp,
                &point.symbol,
                point.price,
                point.volume as i64,
                point.bid,
                point.ask,
                point.bid_size.map(|v| v as i32),
                point.ask_size.map(|v| v as i32),
                &point.source,
                &point.metadata,
            );
            
            writer.as_mut().write(&row).await?;
        }
        
        let rows_copied = writer.finish().await?;
        
        self.metrics.record_bulk_insert("market_data", data.len(), start.elapsed()).await;
        Ok(rows_copied)
    }
    
    pub async fn get_pool_status(&self) -> PoolStatus {
        PoolStatus {
            size: self.pool.status().size,
            available: self.pool.status().available,
            max_size: self.pool.status().max_size,
            metrics: self.metrics.get_stats().await,
        }
    }
}

#[derive(Debug)]
pub struct MarketDataPoint {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub price: Decimal,
    pub volume: u64,
    pub bid: Option<Decimal>,
    pub ask: Option<Decimal>,
    pub bid_size: Option<u32>,
    pub ask_size: Option<u32>,
    pub source: String,
    pub metadata: Option<Value>,
}
```

---

## 3. Monitoring Infrastructure Implementation

### 3.1 Metrics Collection System

#### Prometheus Integration
```rust
// src/monitoring/prometheus_metrics.rs
use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec,
    Registry, Opts, HistogramOpts, register_counter_vec,
    register_gauge_vec, register_histogram_vec,
};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct MetricsCollector {
    registry: Registry,
    
    // Redis Streams metrics
    stream_messages_total: CounterVec,
    stream_consumer_lag: GaugeVec,
    stream_publish_duration: HistogramVec,
    stream_consume_duration: HistogramVec,
    
    // Database metrics
    db_connections_active: Gauge,
    db_query_duration: HistogramVec,
    db_operations_total: CounterVec,
    
    // System metrics
    memory_usage_bytes: Gauge,
    cpu_usage_percent: Gauge,
    disk_usage_bytes: GaugeVec,
    
    // Application metrics
    neural_predictions_total: CounterVec,
    trading_decisions_total: CounterVec,
    error_count_total: CounterVec,
}

impl MetricsCollector {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();
        
        // Stream metrics
        let stream_messages_total = register_counter_vec!(
            Opts::new("stream_messages_total", "Total messages published to streams"),
            &["stream", "producer", "priority"]
        )?;
        
        let stream_consumer_lag = register_gauge_vec!(
            Opts::new("stream_consumer_lag", "Consumer lag in messages"),
            &["stream", "group", "consumer"]
        )?;
        
        let stream_publish_duration = register_histogram_vec!(
            HistogramOpts::new("stream_publish_duration_seconds", "Stream publish latency")
                .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
            &["stream", "priority"]
        )?;
        
        // Database metrics
        let db_connections_active = Gauge::new(
            "db_connections_active", "Active database connections"
        )?;
        
        let db_query_duration = register_histogram_vec!(
            HistogramOpts::new("db_query_duration_seconds", "Database query latency")
                .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5]),
            &["operation", "table"]
        )?;
        
        // System metrics
        let memory_usage_bytes = Gauge::new(
            "memory_usage_bytes", "Current memory usage in bytes"
        )?;
        
        let cpu_usage_percent = Gauge::new(
            "cpu_usage_percent", "Current CPU usage percentage"
        )?;
        
        // Application metrics
        let neural_predictions_total = register_counter_vec!(
            Opts::new("neural_predictions_total", "Total neural predictions generated"),
            &["model_id", "symbol", "outcome"]
        )?;
        
        let trading_decisions_total = register_counter_vec!(
            Opts::new("trading_decisions_total", "Total trading decisions made"),
            &["action", "symbol", "outcome"]
        )?;
        
        // Register all metrics
        registry.register(Box::new(stream_messages_total.clone()))?;
        registry.register(Box::new(stream_consumer_lag.clone()))?;
        registry.register(Box::new(stream_publish_duration.clone()))?;
        registry.register(Box::new(db_connections_active.clone()))?;
        registry.register(Box::new(db_query_duration.clone()))?;
        registry.register(Box::new(memory_usage_bytes.clone()))?;
        registry.register(Box::new(cpu_usage_percent.clone()))?;
        registry.register(Box::new(neural_predictions_total.clone()))?;
        registry.register(Box::new(trading_decisions_total.clone()))?;
        
        Ok(Self {
            registry,
            stream_messages_total,
            stream_consumer_lag,
            stream_publish_duration,
            stream_consume_duration: register_histogram_vec!(
                HistogramOpts::new("stream_consume_duration_seconds", "Stream consume latency")
                    .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]),
                &["stream", "group"]
            )?,
            db_connections_active,
            db_query_duration,
            db_operations_total: register_counter_vec!(
                Opts::new("db_operations_total", "Total database operations"),
                &["operation", "status"]
            )?,
            memory_usage_bytes,
            cpu_usage_percent,
            disk_usage_bytes: register_gauge_vec!(
                Opts::new("disk_usage_bytes", "Disk usage in bytes"),
                &["mount_point", "type"]
            )?,
            neural_predictions_total,
            trading_decisions_total,
            error_count_total: register_counter_vec!(
                Opts::new("error_count_total", "Total errors by component"),
                &["component", "error_type"]
            )?,
        })
    }
    
    // Stream metrics methods
    pub fn record_stream_publish(&self, stream: &str, producer: &str, priority: &str, duration: Duration) {
        self.stream_messages_total
            .with_label_values(&[stream, producer, priority])
            .inc();
            
        self.stream_publish_duration
            .with_label_values(&[stream, priority])
            .observe(duration.as_secs_f64());
    }
    
    pub fn set_consumer_lag(&self, stream: &str, group: &str, consumer: &str, lag: u64) {
        self.stream_consumer_lag
            .with_label_values(&[stream, group, consumer])
            .set(lag as f64);
    }
    
    // Database metrics methods
    pub fn record_db_query(&self, operation: &str, table: &str, duration: Duration) {
        self.db_query_duration
            .with_label_values(&[operation, table])
            .observe(duration.as_secs_f64());
            
        self.db_operations_total
            .with_label_values(&[operation, "success"])
            .inc();
    }
    
    pub fn set_active_connections(&self, count: usize) {
        self.db_connections_active.set(count as f64);
    }
    
    // System metrics methods
    pub fn set_memory_usage(&self, bytes: u64) {
        self.memory_usage_bytes.set(bytes as f64);
    }
    
    pub fn set_cpu_usage(&self, percent: f64) {
        self.cpu_usage_percent.set(percent);
    }
    
    // Application metrics methods
    pub fn record_neural_prediction(&self, model_id: &str, symbol: &str, outcome: &str) {
        self.neural_predictions_total
            .with_label_values(&[model_id, symbol, outcome])
            .inc();
    }
    
    pub fn record_trading_decision(&self, action: &str, symbol: &str, outcome: &str) {
        self.trading_decisions_total
            .with_label_values(&[action, symbol, outcome])
            .inc();
    }
    
    pub fn record_error(&self, component: &str, error_type: &str) {
        self.error_count_total
            .with_label_values(&[component, error_type])
            .inc();
    }
    
    /// Generate Prometheus metrics output
    pub fn render_metrics(&self) -> String {
        use prometheus::Encoder;
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder.encode_to_string(&metric_families).unwrap_or_default()
    }
}

/// System resource monitoring
pub struct SystemMonitor {
    metrics: Arc<MetricsCollector>,
    collection_interval: Duration,
}

impl SystemMonitor {
    pub fn new(metrics: Arc<MetricsCollector>, collection_interval: Duration) -> Self {
        Self {
            metrics,
            collection_interval,
        }
    }
    
    pub async fn start_monitoring(&self) {
        let mut interval = tokio::time::interval(self.collection_interval);
        
        loop {
            interval.tick().await;
            self.collect_system_metrics().await;
        }
    }
    
    async fn collect_system_metrics(&self) {
        // Memory usage
        if let Ok(memory) = self.get_memory_usage().await {
            self.metrics.set_memory_usage(memory);
        }
        
        // CPU usage
        if let Ok(cpu) = self.get_cpu_usage().await {
            self.metrics.set_cpu_usage(cpu);
        }
        
        // Add more system metrics as needed
    }
    
    async fn get_memory_usage(&self) -> Result<u64, std::io::Error> {
        // Implementation depends on platform
        // For Linux: parse /proc/meminfo
        // For macOS: use system calls
        // For simplicity, returning placeholder
        Ok(0)
    }
    
    async fn get_cpu_usage(&self) -> Result<f64, std::io::Error> {
        // Implementation depends on platform
        // Return CPU usage percentage
        Ok(0.0)
    }
}
```

### 3.2 Health Check System

#### Comprehensive Health Monitoring
```rust
// src/monitoring/health_check.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub message: String,
    pub details: HashMap<String, serde_json::Value>,
    pub last_checked: DateTime<Utc>,
    pub check_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall_status: HealthStatus,
    pub checks: HashMap<String, HealthCheck>,
    pub timestamp: DateTime<Utc>,
}

pub struct HealthMonitor {
    checks: Arc<RwLock<HashMap<String, Box<dyn HealthChecker>>>>,
    cache: Arc<RwLock<SystemHealth>>,
    check_interval: Duration,
}

#[async_trait::async_trait]
pub trait HealthChecker: Send + Sync {
    async fn check(&self) -> HealthCheck;
    fn name(&self) -> &str;
}

impl HealthMonitor {
    pub fn new(check_interval: Duration) -> Self {
        Self {
            checks: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(SystemHealth {
                overall_status: HealthStatus::Healthy,
                checks: HashMap::new(),
                timestamp: Utc::now(),
            })),
            check_interval,
        }
    }
    
    pub async fn register_check(&self, checker: Box<dyn HealthChecker>) {
        let mut checks = self.checks.write().await;
        checks.insert(checker.name().to_string(), checker);
    }
    
    pub async fn start_monitoring(&self) {
        let mut interval = tokio::time::interval(self.check_interval);
        
        loop {
            interval.tick().await;
            self.run_all_checks().await;
        }
    }
    
    async fn run_all_checks(&self) {
        let checks = self.checks.read().await;
        let mut results = HashMap::new();
        let mut overall_status = HealthStatus::Healthy;
        
        for (name, checker) in checks.iter() {
            let check_result = checker.check().await;
            
            // Update overall status based on individual checks
            overall_status = match (&overall_status, &check_result.status) {
                (_, HealthStatus::Critical) => HealthStatus::Critical,
                (HealthStatus::Critical, _) => HealthStatus::Critical,
                (_, HealthStatus::Unhealthy) => HealthStatus::Unhealthy,
                (HealthStatus::Unhealthy, _) => HealthStatus::Unhealthy,
                (_, HealthStatus::Degraded) => HealthStatus::Degraded,
                (HealthStatus::Degraded, _) => HealthStatus::Degraded,
                _ => HealthStatus::Healthy,
            };
            
            results.insert(name.clone(), check_result);
        }
        
        // Update cache
        let mut cache = self.cache.write().await;
        *cache = SystemHealth {
            overall_status,
            checks: results,
            timestamp: Utc::now(),
        };
    }
    
    pub async fn get_health(&self) -> SystemHealth {
        self.cache.read().await.clone()
    }
}

// Redis Streams health checker
pub struct RedisStreamsHealthChecker {
    producer: Arc<StreamProducer>,
    streams_to_check: Vec<String>,
}

impl RedisStreamsHealthChecker {
    pub fn new(producer: Arc<StreamProducer>, streams: Vec<String>) -> Self {
        Self {
            producer,
            streams_to_check: streams,
        }
    }
}

#[async_trait::async_trait]
impl HealthChecker for RedisStreamsHealthChecker {
    async fn check(&self) -> HealthCheck {
        let start = std::time::Instant::now();
        let mut details = HashMap::new();
        let mut status = HealthStatus::Healthy;
        let mut message = "Redis Streams operational".to_string();
        
        // Check Redis connection
        match self.producer.ping().await {
            Ok(_) => {
                details.insert("redis_connection".to_string(), json!("connected"));
            }
            Err(e) => {
                status = HealthStatus::Critical;
                message = format!("Redis connection failed: {}", e);
                details.insert("redis_connection".to_string(), json!("failed"));
            }
        }
        
        // Check stream info for each monitored stream
        for stream in &self.streams_to_check {
            match self.producer.get_stream_info(stream).await {
                Ok(info) => {
                    details.insert(
                        format!("stream_{}_length", stream),
                        json!(info.length),
                    );
                    details.insert(
                        format!("stream_{}_groups", stream),
                        json!(info.groups),
                    );
                }
                Err(e) => {
                    if status == HealthStatus::Healthy {
                        status = HealthStatus::Degraded;
                        message = format!("Some streams unhealthy: {}", e);
                    }
                    details.insert(
                        format!("stream_{}_error", stream),
                        json!(e.to_string()),
                    );
                }
            }
        }
        
        HealthCheck {
            name: "redis_streams".to_string(),
            status,
            message,
            details,
            last_checked: Utc::now(),
            check_duration_ms: start.elapsed().as_millis() as u64,
        }
    }
    
    fn name(&self) -> &str {
        "redis_streams"
    }
}

// Database health checker
pub struct DatabaseHealthChecker {
    pool: Arc<DatabasePool>,
}

impl DatabaseHealthChecker {
    pub fn new(pool: Arc<DatabasePool>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl HealthChecker for DatabaseHealthChecker {
    async fn check(&self) -> HealthCheck {
        let start = std::time::Instant::now();
        let mut details = HashMap::new();
        let mut status = HealthStatus::Healthy;
        let mut message = "Database operational".to_string();
        
        // Check basic connectivity
        match self.pool.query_one("SELECT 1", &[]).await {
            Ok(_) => {
                details.insert("connectivity".to_string(), json!("ok"));
            }
            Err(e) => {
                status = HealthStatus::Critical;
                message = format!("Database connectivity failed: {}", e);
                details.insert("connectivity".to_string(), json!("failed"));
            }
        }
        
        // Check pool status
        let pool_status = self.pool.get_pool_status().await;
        details.insert("active_connections".to_string(), json!(pool_status.size));
        details.insert("available_connections".to_string(), json!(pool_status.available));
        details.insert("max_connections".to_string(), json!(pool_status.max_size));
        
        // Check if pool is getting full
        let utilization = pool_status.size as f64 / pool_status.max_size as f64;
        if utilization > 0.9 {
            status = HealthStatus::Degraded;
            message = "Connection pool utilization high".to_string();
        }
        
        HealthCheck {
            name: "database".to_string(),
            status,
            message,
            details,
            last_checked: Utc::now(),
            check_duration_ms: start.elapsed().as_millis() as u64,
        }
    }
    
    fn name(&self) -> &str {
        "database"
    }
}
```

---

## 4. Configuration Management System

### 4.1 Hierarchical Configuration
```rust
// src/config/manager.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub redis: RedisConfig,
    pub database: DatabaseConfig,
    pub monitoring: MonitoringConfig,
    pub security: SecurityConfig,
    pub features: FeatureFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub max_connections: usize,
    pub connection_timeout_secs: u64,
    pub streams: StreamsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamsConfig {
    pub max_length: u64,
    pub retention_days: u32,
    pub consumer_batch_size: usize,
    pub producer_batch_size: usize,
    pub block_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub enable_neural_predictions: bool,
    pub enable_paper_trading: bool,
    pub enable_live_trading: bool,
    pub enable_advanced_metrics: bool,
    pub enable_debug_logging: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            enable_neural_predictions: true,
            enable_paper_trading: true,
            enable_live_trading: false, // Safe default
            enable_advanced_metrics: true,
            enable_debug_logging: false,
        }
    }
}

pub struct ConfigManager {
    config: SystemConfig,
    environment: Environment,
}

#[derive(Debug, Clone)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl ConfigManager {
    pub fn load() -> Result<Self, ConfigError> {
        let environment = Self::detect_environment();
        
        // Start with base configuration
        let mut config = Self::load_base_config()?;
        
        // Apply environment-specific overrides
        Self::apply_environment_config(&mut config, &environment)?;
        
        // Apply environment variable overrides
        Self::apply_environment_variables(&mut config)?;
        
        // Validate configuration
        Self::validate_config(&config)?;
        
        Ok(Self {
            config,
            environment,
        })
    }
    
    fn detect_environment() -> Environment {
        match env::var("NEURAL_TRADER_ENV").as_deref() {
            Ok("production") => Environment::Production,
            Ok("staging") => Environment::Staging,
            _ => Environment::Development,
        }
    }
    
    fn load_base_config() -> Result<SystemConfig, ConfigError> {
        let config_str = include_str!("../../config/base.toml");
        toml::from_str(config_str).map_err(ConfigError::ParseError)
    }
    
    fn apply_environment_config(
        config: &mut SystemConfig, 
        env: &Environment
    ) -> Result<(), ConfigError> {
        let env_config_str = match env {
            Environment::Development => include_str!("../../config/development.toml"),
            Environment::Staging => include_str!("../../config/staging.toml"),
            Environment::Production => include_str!("../../config/production.toml"),
        };
        
        let env_config: SystemConfig = toml::from_str(env_config_str)
            .map_err(ConfigError::ParseError)?;
        
        // Merge configurations (environment overrides base)
        Self::merge_configs(config, env_config);
        
        Ok(())
    }
    
    fn apply_environment_variables(config: &mut SystemConfig) -> Result<(), ConfigError> {
        // Redis configuration
        if let Ok(url) = env::var("REDIS_URL") {
            config.redis.url = url;
        }
        
        // Database configuration  
        if let Ok(url) = env::var("DATABASE_URL") {
            config.database.url = url;
        }
        
        // Feature flags
        if let Ok(val) = env::var("ENABLE_LIVE_TRADING") {
            config.features.enable_live_trading = val.parse().unwrap_or(false);
        }
        
        if let Ok(val) = env::var("ENABLE_DEBUG_LOGGING") {
            config.features.enable_debug_logging = val.parse().unwrap_or(false);
        }
        
        Ok(())
    }
    
    fn validate_config(config: &SystemConfig) -> Result<(), ConfigError> {
        // Validate Redis URL
        if config.redis.url.is_empty() {
            return Err(ConfigError::ValidationError("Redis URL cannot be empty".to_string()));
        }
        
        // Validate database URL
        if config.database.url.is_empty() {
            return Err(ConfigError::ValidationError("Database URL cannot be empty".to_string()));
        }
        
        // Safety check: don't allow live trading in development
        if matches!(Self::detect_environment(), Environment::Development) 
            && config.features.enable_live_trading {
            return Err(ConfigError::ValidationError(
                "Live trading not allowed in development environment".to_string()
            ));
        }
        
        Ok(())
    }
    
    fn merge_configs(base: &mut SystemConfig, override_config: SystemConfig) {
        // Simple field-by-field merge
        // In a real implementation, you might want more sophisticated merging
        base.redis = override_config.redis;
        base.database = override_config.database;
        base.monitoring = override_config.monitoring;
        base.security = override_config.security;
        base.features = override_config.features;
    }
    
    pub fn get_config(&self) -> &SystemConfig {
        &self.config
    }
    
    pub fn get_environment(&self) -> &Environment {
        &self.environment
    }
    
    pub fn is_production(&self) -> bool {
        matches!(self.environment, Environment::Production)
    }
    
    pub fn is_feature_enabled(&self, feature: &str) -> bool {
        match feature {
            "neural_predictions" => self.config.features.enable_neural_predictions,
            "paper_trading" => self.config.features.enable_paper_trading,
            "live_trading" => self.config.features.enable_live_trading,
            "advanced_metrics" => self.config.features.enable_advanced_metrics,
            "debug_logging" => self.config.features.enable_debug_logging,
            _ => false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Configuration parse error: {0}")]
    ParseError(#[from] toml::de::Error),
    
    #[error("Configuration validation error: {0}")]
    ValidationError(String),
    
    #[error("Environment variable error: {0}")]
    EnvironmentError(String),
}
```

---

## 5. Implementation Timeline & Dependencies

### Week 1: Redis Streams Foundation
```yaml
Day 1-2: Stream Definitions & Message Format
  - Define stream configurations
  - Implement standardized message format
  - Create stream management utilities

Day 3-4: Producer Implementation
  - High-performance producer with batching
  - Error handling and retry logic
  - Metrics integration

Day 5: Consumer Implementation  
  - Reliable consumer with acknowledgments
  - Pending message recovery
  - Consumer group management
```

### Week 2: Storage & Configuration
```yaml
Day 1-2: TimescaleDB Schema & Pool
  - Create optimized time-series schema
  - Implement connection pooling
  - Bulk insert optimization

Day 3-4: Configuration Management
  - Hierarchical configuration system
  - Environment-specific overrides
  - Feature flag implementation

Day 5: Integration Testing
  - End-to-end messaging tests
  - Database performance validation
  - Configuration validation
```

### Week 3: Monitoring & Validation
```yaml
Day 1-2: Prometheus Metrics
  - Comprehensive metrics collection
  - System resource monitoring
  - Performance tracking

Day 3-4: Health Check System
  - Component health monitoring
  - Alert generation
  - Dashboard integration

Day 5: Performance Testing
  - Load testing Redis Streams
  - Database performance validation
  - End-to-end latency testing
```

---

## 6. Success Criteria & Validation

### Performance Targets
- **Redis Streams Throughput**: >100,000 messages/second
- **Message Latency**: <10ms for high-priority messages
- **Database Inserts**: >10,000 rows/second
- **Connection Pool**: <100ms connection acquisition
- **Memory Usage**: <2GB total system memory

### Reliability Targets
- **Message Delivery**: Zero message loss during normal operations
- **Consumer Lag**: <100 messages under normal load
- **Health Checks**: All components reporting healthy
- **Error Recovery**: Automatic recovery from transient failures

### Integration Validation
- **End-to-End Flow**: Market data → Processing → Storage
- **Configuration**: All settings configurable via environment
- **Monitoring**: Real-time visibility into all components
- **Deployment**: Single-command deployment capability

---

This comprehensive Phase 1 specification establishes the foundational shared infrastructure that all subsequent phases will build upon. The Redis Streams event bus provides the messaging backbone, TimescaleDB offers optimized time-series storage, and the monitoring infrastructure ensures operational visibility from day one.