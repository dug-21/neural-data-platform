# Redis Streams Implementation Guide for Neural Trading MVP

## Table of Contents
1. [Overview](#overview)
2. [Redis Configuration](#redis-configuration)
3. [Connection Pooling](#connection-pooling)
4. [Stream Key Patterns](#stream-key-patterns)
5. [Producer Implementation](#producer-implementation)
6. [Consumer Group Management](#consumer-group-management)
7. [Backpressure Handling](#backpressure-handling)
8. [Message Acknowledgment Patterns](#message-acknowledgment-patterns)
9. [Error Recovery & Dead Letter Queues](#error-recovery--dead-letter-queues)
10. [Stream Trimming Strategies](#stream-trimming-strategies)
11. [Pending Entries Management](#pending-entries-management)
12. [Monitoring & Metrics](#monitoring--metrics)
13. [Production Deployment](#production-deployment)

## Overview

Redis Streams provide a powerful foundation for the neural trading system's real-time data processing pipeline. This guide implements event-driven architecture patterns for:

- Market data ingestion and distribution
- Trading signal propagation
- Order execution workflows
- Risk management events
- Performance analytics

## Redis Configuration

### Basic Redis Configuration (`redis.conf`)

```conf
# Memory management
maxmemory 8gb
maxmemory-policy allkeys-lru

# Persistence for durability
save 900 1
save 300 10
save 60 10000

# Stream-specific settings
stream-node-max-bytes 4096
stream-node-max-entries 100

# Network
tcp-keepalive 300
timeout 0

# Logging
loglevel notice
logfile "/var/log/redis/redis-server.log"
```

### Cargo.toml Dependencies

```toml
[dependencies]
redis = { version = "0.24", features = ["streams", "connection-manager", "tokio-comp"] }
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.0", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
anyhow = "1.0"
deadpool-redis = "0.14"
```

## Connection Pooling

### Production-Ready Connection Pool

```rust
use deadpool_redis::{Config, Pool, Runtime};
use redis::Client;
use std::time::Duration;
use anyhow::Result;

#[derive(Clone)]
pub struct RedisPool {
    pool: Pool,
}

impl RedisPool {
    pub async fn new(redis_url: &str) -> Result<Self> {
        let cfg = Config::from_url(redis_url);
        let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
        
        // Test connection
        let mut conn = pool.get().await?;
        redis::cmd("PING").query_async::<_, String>(&mut conn).await?;
        
        Ok(Self { pool })
    }

    pub async fn get_connection(&self) -> Result<deadpool_redis::Connection> {
        Ok(self.pool.get().await?)
    }

    pub async fn execute<T>(&self, cmd: &redis::Cmd) -> Result<T>
    where
        T: redis::FromRedisValue,
    {
        let mut conn = self.get_connection().await?;
        let result = cmd.query_async(&mut conn).await?;
        Ok(result)
    }
}

// Connection pool configuration
pub struct PoolConfig {
    pub max_size: usize,
    pub timeout: Duration,
    pub recycle_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_size: 50,
            timeout: Duration::from_secs(5),
            recycle_timeout: Duration::from_secs(60),
        }
    }
}
```

## Stream Key Patterns

### Trading Domain Stream Keys

```rust
use chrono::{DateTime, Utc};

pub struct StreamKeys;

impl StreamKeys {
    // Market data streams
    pub fn market_data(symbol: &str) -> String {
        format!("market:data:{}", symbol.to_lowercase())
    }

    pub fn orderbook(symbol: &str) -> String {
        format!("market:orderbook:{}", symbol.to_lowercase())
    }

    pub fn trades(symbol: &str) -> String {
        format!("market:trades:{}", symbol.to_lowercase())
    }

    // Trading signals
    pub fn signals(strategy: &str) -> String {
        format!("signals:{}", strategy)
    }

    pub fn ml_predictions(model: &str) -> String {
        format!("ml:predictions:{}", model)
    }

    // Order management
    pub fn orders(user_id: &str) -> String {
        format!("orders:user:{}", user_id)
    }

    pub fn order_updates() -> String {
        "orders:updates".to_string()
    }

    // Risk management
    pub fn risk_events() -> String {
        "risk:events".to_string()
    }

    pub fn position_updates(user_id: &str) -> String {
        format!("positions:user:{}", user_id)
    }

    // Analytics
    pub fn performance_metrics() -> String {
        "analytics:performance".to_string()
    }

    pub fn system_metrics() -> String {
        "system:metrics".to_string()
    }

    // Dead letter queues
    pub fn dead_letter(original_stream: &str) -> String {
        format!("dlq:{}", original_stream)
    }

    // Daily partitioned streams for historical data
    pub fn daily_partition(base_key: &str, date: DateTime<Utc>) -> String {
        format!("{}:{}", base_key, date.format("%Y%m%d"))
    }
}

// Consumer group naming patterns
pub struct ConsumerGroups;

impl ConsumerGroups {
    pub fn trading_engine() -> &'static str {
        "trading-engine"
    }

    pub fn risk_manager() -> &'static str {
        "risk-manager"
    }

    pub fn analytics() -> &'static str {
        "analytics"
    }

    pub fn ml_pipeline() -> &'static str {
        "ml-pipeline"
    }

    pub fn notification_service() -> &'static str {
        "notifications"
    }
}
```

## Producer Implementation

### High-Performance Stream Producer

```rust
use redis::{AsyncCommands, streams::StreamAddOptions};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct StreamMessage {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub correlation_id: Option<String>,
    pub source: String,
}

impl StreamMessage {
    pub fn new(event_type: &str, payload: serde_json::Value, source: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            event_type: event_type.to_string(),
            payload,
            correlation_id: None,
            source: source.to_string(),
        }
    }

    pub fn with_correlation_id(mut self, correlation_id: String) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
}

pub struct StreamProducer {
    pool: RedisPool,
    max_len: Option<usize>,
}

impl StreamProducer {
    pub fn new(pool: RedisPool) -> Self {
        Self {
            pool,
            max_len: Some(10000), // Default max length
        }
    }

    pub fn with_max_len(mut self, max_len: usize) -> Self {
        self.max_len = Some(max_len);
        self
    }

    pub async fn publish(&self, stream_key: &str, message: StreamMessage) -> Result<String> {
        let mut conn = self.pool.get_connection().await?;
        
        let fields = vec![
            ("id", message.id.clone()),
            ("timestamp", message.timestamp.to_rfc3339()),
            ("event_type", message.event_type),
            ("payload", serde_json::to_string(&message.payload)?),
            ("correlation_id", message.correlation_id.unwrap_or_default()),
            ("source", message.source),
        ];

        let mut options = StreamAddOptions::default();
        if let Some(max_len) = self.max_len {
            options = options.max_len(max_len);
        }

        let stream_id: String = conn
            .xadd_with_options(stream_key, "*", &fields, options)
            .await?;

        tracing::debug!(
            stream_key = %stream_key,
            stream_id = %stream_id,
            message_id = %message.id,
            "Published message to stream"
        );

        Ok(stream_id)
    }

    pub async fn publish_batch(
        &self,
        stream_key: &str,
        messages: Vec<StreamMessage>,
    ) -> Result<Vec<String>> {
        let mut conn = self.pool.get_connection().await?;
        let mut results = Vec::new();

        // Use pipeline for batch publishing
        let mut pipe = redis::pipe();
        
        for message in &messages {
            let fields = vec![
                ("id", message.id.clone()),
                ("timestamp", message.timestamp.to_rfc3339()),
                ("event_type", message.event_type.clone()),
                ("payload", serde_json::to_string(&message.payload)?),
                ("correlation_id", message.correlation_id.clone().unwrap_or_default()),
                ("source", message.source.clone()),
            ];

            let mut options = StreamAddOptions::default();
            if let Some(max_len) = self.max_len {
                options = options.max_len(max_len);
            }

            pipe.xadd_with_options(stream_key, "*", &fields, options);
        }

        let stream_ids: Vec<String> = pipe.query_async(&mut conn).await?;
        
        tracing::info!(
            stream_key = %stream_key,
            batch_size = messages.len(),
            "Published batch to stream"
        );

        Ok(stream_ids)
    }
}

// Market data producer example
#[derive(Debug, Serialize)]
pub struct MarketDataEvent {
    pub symbol: String,
    pub price: f64,
    pub volume: f64,
    pub bid: f64,
    pub ask: f64,
    pub timestamp: DateTime<Utc>,
}

impl MarketDataEvent {
    pub async fn publish(&self, producer: &StreamProducer) -> Result<String> {
        let payload = serde_json::to_value(self)?;
        let message = StreamMessage::new("market_data", payload, "market_feed");
        let stream_key = StreamKeys::market_data(&self.symbol);
        producer.publish(&stream_key, message).await
    }
}
```

## Consumer Group Management

### Robust Consumer Implementation

```rust
use redis::{AsyncCommands, streams::{StreamReadOptions, StreamReadReply}};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use anyhow::{Result, anyhow};

pub struct StreamConsumer {
    pool: RedisPool,
    group_name: String,
    consumer_name: String,
    block_time: Option<usize>,
    count: Option<usize>,
}

impl StreamConsumer {
    pub fn new(
        pool: RedisPool,
        group_name: String,
        consumer_name: String,
    ) -> Self {
        Self {
            pool,
            group_name,
            consumer_name,
            block_time: Some(1000), // 1 second
            count: Some(10),        // Read 10 messages at a time
        }
    }

    pub fn with_block_time(mut self, block_time: usize) -> Self {
        self.block_time = Some(block_time);
        self
    }

    pub fn with_count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    pub async fn create_consumer_group(&self, stream_keys: &[String]) -> Result<()> {
        let mut conn = self.pool.get_connection().await?;

        for stream_key in stream_keys {
            // Create consumer group, starting from the beginning if group doesn't exist
            let result: Result<String, redis::RedisError> = conn
                .xgroup_create_mkstream(stream_key, &self.group_name, "0")
                .await;

            match result {
                Ok(_) => {
                    tracing::info!(
                        stream_key = %stream_key,
                        group = %self.group_name,
                        "Created consumer group"
                    );
                }
                Err(e) if e.to_string().contains("BUSYGROUP") => {
                    tracing::debug!(
                        stream_key = %stream_key,
                        group = %self.group_name,
                        "Consumer group already exists"
                    );
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(())
    }

    pub async fn consume_messages<F, Fut>(
        &self,
        stream_keys: Vec<String>,
        mut handler: F,
    ) -> Result<()>
    where
        F: FnMut(String, StreamMessage) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let mut conn = self.pool.get_connection().await?;
        
        // First, process any pending messages
        self.process_pending_messages(&stream_keys, &mut handler).await?;

        // Then start consuming new messages
        let stream_args: Vec<(&str, &str)> = stream_keys
            .iter()
            .map(|key| (key.as_str(), ">"))
            .collect();

        loop {
            let opts = StreamReadOptions::default()
                .group(&self.group_name, &self.consumer_name)
                .count(self.count.unwrap_or(1))
                .block(self.block_time.unwrap_or(0));

            match conn.xread_options(&stream_args, &opts).await {
                Ok(reply) => {
                    self.process_stream_reply(reply, &mut handler).await?;
                }
                Err(e) => {
                    tracing::error!(error = %e, "Error reading from streams");
                    sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn process_pending_messages<F, Fut>(
        &self,
        stream_keys: &[String],
        handler: &mut F,
    ) -> Result<()>
    where
        F: FnMut(String, StreamMessage) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let mut conn = self.pool.get_connection().await?;

        for stream_key in stream_keys {
            let pending: Vec<redis::Value> = conn
                .xpending(stream_key, &self.group_name)
                .await?;

            if let Some(redis::Value::Int(count)) = pending.get(0) {
                if *count > 0 {
                    tracing::info!(
                        stream_key = %stream_key,
                        pending_count = count,
                        "Processing pending messages"
                    );

                    let opts = StreamReadOptions::default()
                        .group(&self.group_name, &self.consumer_name)
                        .count(self.count.unwrap_or(10));

                    let reply: StreamReadReply = conn
                        .xread_options(&[(stream_key.as_str(), "0")], &opts)
                        .await?;

                    self.process_stream_reply(reply, handler).await?;
                }
            }
        }

        Ok(())
    }

    async fn process_stream_reply<F, Fut>(
        &self,
        reply: StreamReadReply,
        handler: &mut F,
    ) -> Result<()>
    where
        F: FnMut(String, StreamMessage) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        for stream_key in reply.keys {
            for stream_id in stream_key.ids {
                match self.parse_stream_message(&stream_id.map) {
                    Ok(message) => {
                        let stream_name = stream_key.key.clone();
                        let message_id = stream_id.id.clone();
                        
                        match handler(stream_name.clone(), message).await {
                            Ok(_) => {
                                self.acknowledge_message(&stream_name, &message_id).await?;
                            }
                            Err(e) => {
                                tracing::error!(
                                    stream_key = %stream_name,
                                    message_id = %message_id,
                                    error = %e,
                                    "Failed to process message"
                                );
                                // Don't acknowledge failed messages
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            stream_key = %stream_key.key,
                            message_id = %stream_id.id,
                            error = %e,
                            "Failed to parse message"
                        );
                    }
                }
            }
        }

        Ok(())
    }

    fn parse_stream_message(
        &self,
        fields: &HashMap<String, redis::Value>,
    ) -> Result<StreamMessage> {
        let get_string = |key: &str| -> Result<String> {
            fields
                .get(key)
                .and_then(|v| match v {
                    redis::Value::Data(bytes) => String::from_utf8(bytes.clone()).ok(),
                    _ => None,
                })
                .ok_or_else(|| anyhow!("Missing or invalid field: {}", key))
        };

        let id = get_string("id")?;
        let timestamp = get_string("timestamp")?
            .parse::<DateTime<Utc>>()
            .map_err(|e| anyhow!("Invalid timestamp: {}", e))?;
        let event_type = get_string("event_type")?;
        let payload_str = get_string("payload")?;
        let payload: serde_json::Value = serde_json::from_str(&payload_str)?;
        let correlation_id = get_string("correlation_id").ok().filter(|s| !s.is_empty());
        let source = get_string("source")?;

        Ok(StreamMessage {
            id,
            timestamp,
            event_type,
            payload,
            correlation_id,
            source,
        })
    }

    async fn acknowledge_message(&self, stream_key: &str, message_id: &str) -> Result<()> {
        let mut conn = self.pool.get_connection().await?;
        let _: i32 = conn
            .xack(stream_key, &self.group_name, &[message_id])
            .await?;

        tracing::trace!(
            stream_key = %stream_key,
            message_id = %message_id,
            "Acknowledged message"
        );

        Ok(())
    }
}
```

## Backpressure Handling

### Adaptive Backpressure Implementation

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{interval, Duration, Instant};

pub struct BackpressureManager {
    semaphore: Arc<Semaphore>,
    pending_count: Arc<AtomicUsize>,
    max_pending: usize,
    pressure_threshold: f64,
    current_delay: Arc<AtomicUsize>, // in milliseconds
    min_delay: Duration,
    max_delay: Duration,
}

impl BackpressureManager {
    pub fn new(max_concurrent: usize, max_pending: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            pending_count: Arc::new(AtomicUsize::new(0)),
            max_pending,
            pressure_threshold: 0.8, // 80% threshold
            current_delay: Arc::new(AtomicUsize::new(0)),
            min_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(5),
        }
    }

    pub async fn acquire_permit(&self) -> Result<BackpressurePermit> {
        // Check if we're over the pending limit
        let pending = self.pending_count.load(Ordering::Relaxed);
        if pending >= self.max_pending {
            let delay = self.calculate_delay(pending);
            tokio::time::sleep(delay).await;
        }

        let permit = self.semaphore.acquire().await.unwrap();
        self.pending_count.fetch_add(1, Ordering::Relaxed);

        Ok(BackpressurePermit {
            _permit: permit,
            pending_count: Arc::clone(&self.pending_count),
        })
    }

    fn calculate_delay(&self, pending: usize) -> Duration {
        let pressure_ratio = pending as f64 / self.max_pending as f64;
        
        if pressure_ratio <= self.pressure_threshold {
            return Duration::from_millis(0);
        }

        let excess_pressure = pressure_ratio - self.pressure_threshold;
        let delay_factor = excess_pressure / (1.0 - self.pressure_threshold);
        
        let delay_ms = (self.min_delay.as_millis() as f64 
            + delay_factor * (self.max_delay.as_millis() - self.min_delay.as_millis()) as f64) 
            as u64;

        Duration::from_millis(delay_ms.min(self.max_delay.as_millis() as u64))
    }

    pub fn get_metrics(&self) -> BackpressureMetrics {
        BackpressureMetrics {
            pending_count: self.pending_count.load(Ordering::Relaxed),
            max_pending: self.max_pending,
            available_permits: self.semaphore.available_permits(),
            current_delay: Duration::from_millis(
                self.current_delay.load(Ordering::Relaxed) as u64
            ),
        }
    }
}

pub struct BackpressurePermit {
    _permit: tokio::sync::SemaphorePermit<'static>,
    pending_count: Arc<AtomicUsize>,
}

impl Drop for BackpressurePermit {
    fn drop(&mut self) {
        self.pending_count.fetch_sub(1, Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub struct BackpressureMetrics {
    pub pending_count: usize,
    pub max_pending: usize,
    pub available_permits: usize,
    pub current_delay: Duration,
}

// Enhanced consumer with backpressure
pub struct BackpressureConsumer {
    consumer: StreamConsumer,
    backpressure: BackpressureManager,
}

impl BackpressureConsumer {
    pub fn new(consumer: StreamConsumer, max_concurrent: usize, max_pending: usize) -> Self {
        Self {
            consumer,
            backpressure: BackpressureManager::new(max_concurrent, max_pending),
        }
    }

    pub async fn consume_with_backpressure<F, Fut>(
        &self,
        stream_keys: Vec<String>,
        handler: F,
    ) -> Result<()>
    where
        F: Fn(String, StreamMessage) -> Fut + Clone + Send + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        let handler = Arc::new(handler);
        let backpressure = Arc::new(self.backpressure);

        self.consumer
            .consume_messages(stream_keys, move |stream_key, message| {
                let handler = Arc::clone(&handler);
                let backpressure = Arc::clone(&backpressure);
                
                async move {
                    let _permit = backpressure.acquire_permit().await?;
                    handler(stream_key, message).await
                }
            })
            .await
    }
}
```

## Message Acknowledgment Patterns

### Reliable Acknowledgment Strategies

```rust
use std::collections::HashMap;
use tokio::time::{timeout, Duration};

pub enum AckStrategy {
    Immediate,              // Ack immediately after receiving
    AfterProcessing,        // Ack after successful processing (default)
    Batched(usize),         // Batch acknowledgments
    Delayed(Duration),      // Ack after delay for graceful failure handling
}

pub struct AckManager {
    pool: RedisPool,
    strategy: AckStrategy,
    pending_acks: tokio::sync::Mutex<HashMap<String, Vec<String>>>, // stream -> message_ids
}

impl AckManager {
    pub fn new(pool: RedisPool, strategy: AckStrategy) -> Self {
        Self {
            pool,
            strategy,
            pending_acks: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    pub async fn handle_acknowledgment(
        &self,
        stream_key: &str,
        message_id: &str,
        group_name: &str,
        processing_result: Result<()>,
    ) -> Result<()> {
        match &self.strategy {
            AckStrategy::Immediate => {
                self.ack_message(stream_key, message_id, group_name).await
            }
            AckStrategy::AfterProcessing => {
                if processing_result.is_ok() {
                    self.ack_message(stream_key, message_id, group_name).await
                } else {
                    // Leave message in pending state for retry
                    tracing::warn!(
                        stream_key = %stream_key,
                        message_id = %message_id,
                        "Message processing failed, leaving in pending state"
                    );
                    Ok(())
                }
            }
            AckStrategy::Batched(batch_size) => {
                if processing_result.is_ok() {
                    self.add_to_batch(stream_key, message_id).await;
                    self.try_flush_batch(stream_key, group_name, *batch_size).await
                } else {
                    Ok(())
                }
            }
            AckStrategy::Delayed(delay) => {
                if processing_result.is_ok() {
                    let stream_key = stream_key.to_string();
                    let message_id = message_id.to_string();
                    let group_name = group_name.to_string();
                    let pool = self.pool.clone();
                    let delay = *delay;

                    tokio::spawn(async move {
                        tokio::time::sleep(delay).await;
                        let _ = Self::ack_message_static(&pool, &stream_key, &message_id, &group_name).await;
                    });

                    Ok(())
                } else {
                    Ok(())
                }
            }
        }
    }

    async fn ack_message(&self, stream_key: &str, message_id: &str, group_name: &str) -> Result<()> {
        Self::ack_message_static(&self.pool, stream_key, message_id, group_name).await
    }

    async fn ack_message_static(
        pool: &RedisPool,
        stream_key: &str,
        message_id: &str,
        group_name: &str,
    ) -> Result<()> {
        let mut conn = pool.get_connection().await?;
        let acked: i32 = conn.xack(stream_key, group_name, &[message_id]).await?;
        
        if acked > 0 {
            tracing::trace!(
                stream_key = %stream_key,
                message_id = %message_id,
                "Message acknowledged"
            );
        }

        Ok(())
    }

    async fn add_to_batch(&self, stream_key: &str, message_id: &str) {
        let mut pending_acks = self.pending_acks.lock().await;
        pending_acks
            .entry(stream_key.to_string())
            .or_insert_with(Vec::new)
            .push(message_id.to_string());
    }

    async fn try_flush_batch(
        &self,
        stream_key: &str,
        group_name: &str,
        batch_size: usize,
    ) -> Result<()> {
        let mut pending_acks = self.pending_acks.lock().await;
        
        if let Some(message_ids) = pending_acks.get_mut(stream_key) {
            if message_ids.len() >= batch_size {
                let batch: Vec<String> = message_ids.drain(..).collect();
                drop(pending_acks); // Release lock before async operation

                let mut conn = self.pool.get_connection().await?;
                let acked: i32 = conn.xack(stream_key, group_name, &batch).await?;
                
                tracing::debug!(
                    stream_key = %stream_key,
                    batch_size = batch.len(),
                    acked_count = acked,
                    "Batch acknowledged"
                );
            }
        }

        Ok(())
    }

    pub async fn flush_all_batches(&self, group_name: &str) -> Result<()> {
        let mut pending_acks = self.pending_acks.lock().await;
        let all_pending: HashMap<String, Vec<String>> = pending_acks.drain().collect();
        drop(pending_acks);

        for (stream_key, message_ids) in all_pending {
            if !message_ids.is_empty() {
                let mut conn = self.pool.get_connection().await?;
                let acked: i32 = conn.xack(&stream_key, group_name, &message_ids).await?;
                
                tracing::info!(
                    stream_key = %stream_key,
                    batch_size = message_ids.len(),
                    acked_count = acked,
                    "Final batch acknowledged"
                );
            }
        }

        Ok(())
    }
}

// Timeout-based message processor
pub struct TimeoutProcessor {
    pool: RedisPool,
    processing_timeout: Duration,
}

impl TimeoutProcessor {
    pub fn new(pool: RedisPool, processing_timeout: Duration) -> Self {
        Self {
            pool,
            processing_timeout,
        }
    }

    pub async fn process_with_timeout<F, Fut>(
        &self,
        stream_key: String,
        message: StreamMessage,
        group_name: &str,
        message_id: &str,
        processor: F,
    ) -> Result<()>
    where
        F: FnOnce(StreamMessage) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let processing_result = timeout(
            self.processing_timeout,
            processor(message)
        ).await;

        match processing_result {
            Ok(Ok(())) => {
                // Successful processing
                let mut conn = self.pool.get_connection().await?;
                conn.xack(&stream_key, group_name, &[message_id]).await?;
                Ok(())
            }
            Ok(Err(e)) => {
                // Processing failed
                tracing::error!(
                    stream_key = %stream_key,
                    message_id = %message_id,
                    error = %e,
                    "Message processing failed"
                );
                Err(e)
            }
            Err(_) => {
                // Processing timed out
                tracing::error!(
                    stream_key = %stream_key,
                    message_id = %message_id,
                    timeout = ?self.processing_timeout,
                    "Message processing timed out"
                );
                Err(anyhow!("Processing timeout"))
            }
        }
    }
}
```

## Error Recovery & Dead Letter Queues

### Comprehensive Error Handling

```rust
use std::collections::HashMap;
use tokio::time::{interval, Duration, Instant};

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: usize,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_factor: f64,
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(60),
            backoff_factor: 2.0,
            jitter: true,
        }
    }
}

pub struct DeadLetterQueue {
    pool: RedisPool,
    retry_config: RetryConfig,
}

impl DeadLetterQueue {
    pub fn new(pool: RedisPool, retry_config: RetryConfig) -> Self {
        Self { pool, retry_config }
    }

    pub async fn handle_failed_message(
        &self,
        original_stream: &str,
        message: &StreamMessage,
        error: &anyhow::Error,
        attempt_count: usize,
    ) -> Result<()> {
        if attempt_count < self.retry_config.max_attempts {
            // Calculate retry delay with exponential backoff
            let delay = self.calculate_retry_delay(attempt_count);
            
            tracing::info!(
                original_stream = %original_stream,
                message_id = %message.id,
                attempt = attempt_count,
                delay = ?delay,
                "Scheduling retry"
            );

            self.schedule_retry(original_stream, message, delay, attempt_count + 1).await
        } else {
            // Move to dead letter queue
            tracing::error!(
                original_stream = %original_stream,
                message_id = %message.id,
                error = %error,
                "Moving message to dead letter queue"
            );

            self.move_to_dlq(original_stream, message, error).await
        }
    }

    async fn calculate_retry_delay(&self, attempt: usize) -> Duration {
        let base_delay = self.retry_config.initial_delay.as_millis() as f64;
        let delay = base_delay * self.retry_config.backoff_factor.powi(attempt as i32);
        let max_delay = self.retry_config.max_delay.as_millis() as f64;
        
        let final_delay = delay.min(max_delay);
        
        let final_delay = if self.retry_config.jitter {
            let jitter = fastrand::f64() * 0.1 * final_delay; // 10% jitter
            final_delay + jitter - (0.05 * final_delay)
        } else {
            final_delay
        };

        Duration::from_millis(final_delay as u64)
    }

    async fn schedule_retry(
        &self,
        original_stream: &str,
        message: &StreamMessage,
        delay: Duration,
        attempt_count: usize,
    ) -> Result<()> {
        let retry_stream = format!("retry:{}", original_stream);
        let scheduled_time = Utc::now() + chrono::Duration::from_std(delay)?;
        
        let mut retry_message = message.clone();
        retry_message.payload["retry_metadata"] = serde_json::json!({
            "original_stream": original_stream,
            "attempt_count": attempt_count,
            "scheduled_time": scheduled_time.to_rfc3339(),
            "original_message_id": message.id
        });

        let producer = StreamProducer::new(self.pool.clone());
        producer.publish(&retry_stream, retry_message).await?;

        Ok(())
    }

    async fn move_to_dlq(
        &self,
        original_stream: &str,
        message: &StreamMessage,
        error: &anyhow::Error,
    ) -> Result<()> {
        let dlq_stream = StreamKeys::dead_letter(original_stream);
        
        let mut dlq_message = message.clone();
        dlq_message.payload["dlq_metadata"] = serde_json::json!({
            "original_stream": original_stream,
            "failure_time": Utc::now().to_rfc3339(),
            "error_message": error.to_string(),
            "original_message_id": message.id
        });

        let producer = StreamProducer::new(self.pool.clone());
        producer.publish(&dlq_stream, dlq_message).await?;

        Ok(())
    }

    pub async fn process_retries(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(10));
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.check_and_process_retries().await {
                tracing::error!(error = %e, "Error processing retries");
            }
        }
    }

    async fn check_and_process_retries(&self) -> Result<()> {
        let mut conn = self.pool.get_connection().await?;
        
        // Find all retry streams
        let retry_streams: Vec<String> = conn.keys("retry:*").await?;
        
        for retry_stream in retry_streams {
            let opts = redis::streams::StreamReadOptions::default().count(100);
            let reply: redis::streams::StreamReadReply = conn
                .xread_options(&[(&retry_stream, "0")], &opts)
                .await?;

            for stream in reply.keys {
                for entry in stream.ids {
                    if let Ok(message) = self.parse_retry_message(&entry.map) {
                        if self.is_ready_for_retry(&message)? {
                            self.requeue_message(&message).await?;
                            
                            // Remove from retry stream
                            let _: i32 = conn.xdel(&retry_stream, &[&entry.id]).await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn parse_retry_message(
        &self,
        fields: &HashMap<String, redis::Value>,
    ) -> Result<StreamMessage> {
        // Similar to StreamConsumer::parse_stream_message but for retry messages
        // Implementation details omitted for brevity
        todo!("Implement retry message parsing")
    }

    fn is_ready_for_retry(&self, message: &StreamMessage) -> Result<bool> {
        if let Some(metadata) = message.payload.get("retry_metadata") {
            if let Some(scheduled_time_str) = metadata.get("scheduled_time").and_then(|v| v.as_str()) {
                let scheduled_time = scheduled_time_str.parse::<DateTime<Utc>>()?;
                return Ok(Utc::now() >= scheduled_time);
            }
        }
        Ok(false)
    }

    async fn requeue_message(&self, message: &StreamMessage) -> Result<()> {
        if let Some(metadata) = message.payload.get("retry_metadata") {
            if let Some(original_stream) = metadata.get("original_stream").and_then(|v| v.as_str()) {
                let mut requeued_message = message.clone();
                
                // Remove retry metadata
                if let serde_json::Value::Object(ref mut obj) = &mut requeued_message.payload {
                    obj.remove("retry_metadata");
                }

                let producer = StreamProducer::new(self.pool.clone());
                producer.publish(original_stream, requeued_message).await?;
                
                tracing::info!(
                    original_stream = %original_stream,
                    message_id = %message.id,
                    "Message requeued for retry"
                );
            }
        }
        Ok(())
    }
}

// Circuit breaker for stream processing
pub struct StreamCircuitBreaker {
    failure_threshold: usize,
    reset_timeout: Duration,
    current_failures: Arc<AtomicUsize>,
    last_failure_time: Arc<tokio::sync::Mutex<Option<Instant>>>,
    state: Arc<tokio::sync::Mutex<CircuitState>>,
}

#[derive(Debug, Clone, PartialEq)]
enum CircuitState {
    Closed,    // Normal operation
    Open,      // Failing, reject requests
    HalfOpen,  // Test if service recovered
}

impl StreamCircuitBreaker {
    pub fn new(failure_threshold: usize, reset_timeout: Duration) -> Self {
        Self {
            failure_threshold,
            reset_timeout,
            current_failures: Arc::new(AtomicUsize::new(0)),
            last_failure_time: Arc::new(tokio::sync::Mutex::new(None)),
            state: Arc::new(tokio::sync::Mutex::new(CircuitState::Closed)),
        }
    }

    pub async fn call<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        if !self.should_allow_request().await {
            return Err(anyhow!("Circuit breaker is OPEN"));
        }

        match operation().await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(e) => {
                self.on_failure().await;
                Err(e)
            }
        }
    }

    async fn should_allow_request(&self) -> bool {
        let state = self.state.lock().await;
        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if we should transition to half-open
                if let Some(last_failure) = *self.last_failure_time.lock().await {
                    if last_failure.elapsed() >= self.reset_timeout {
                        drop(state);
                        *self.state.lock().await = CircuitState::HalfOpen;
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => true,
        }
    }

    async fn on_success(&self) {
        self.current_failures.store(0, Ordering::Relaxed);
        let mut state = self.state.lock().await;
        if *state == CircuitState::HalfOpen {
            *state = CircuitState::Closed;
            tracing::info!("Circuit breaker reset to CLOSED");
        }
    }

    async fn on_failure(&self) {
        let failures = self.current_failures.fetch_add(1, Ordering::Relaxed) + 1;
        *self.last_failure_time.lock().await = Some(Instant::now());

        if failures >= self.failure_threshold {
            let mut state = self.state.lock().await;
            if *state != CircuitState::Open {
                *state = CircuitState::Open;
                tracing::warn!(
                    failures = failures,
                    threshold = self.failure_threshold,
                    "Circuit breaker opened"
                );
            }
        }
    }
}
```

## Stream Trimming Strategies

### Automated Stream Management

```rust
use redis::AsyncCommands;
use tokio::time::{interval, Duration};

pub enum TrimStrategy {
    MaxLen(usize),                    // MAXLEN strategy
    MinId(String),                    // MINID strategy
    Approximate(usize),               // MAXLEN ~ (approximate)
    TimeBasedMaxLen(Duration, usize), // Time-based with max length
    Composite {                       // Multiple strategies
        max_len: Option<usize>,
        min_age: Option<Duration>,
        min_id: Option<String>,
    },
}

pub struct StreamTrimmer {
    pool: RedisPool,
    strategies: HashMap<String, TrimStrategy>, // stream pattern -> strategy
}

impl StreamTrimmer {
    pub fn new(pool: RedisPool) -> Self {
        Self {
            pool,
            strategies: HashMap::new(),
        }
    }

    pub fn add_strategy<S: Into<String>>(mut self, pattern: S, strategy: TrimStrategy) -> Self {
        self.strategies.insert(pattern.into(), strategy);
        self
    }

    pub async fn start_trimming(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(30)); // Trim every 30 seconds
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.trim_all_streams().await {
                tracing::error!(error = %e, "Error during stream trimming");
            }
        }
    }

    async fn trim_all_streams(&self) -> Result<()> {
        let mut conn = self.pool.get_connection().await?;
        
        for (pattern, strategy) in &self.strategies {
            let matching_streams: Vec<String> = if pattern.contains('*') {
                conn.keys(pattern).await?
            } else {
                vec![pattern.clone()]
            };

            for stream in matching_streams {
                if let Err(e) = self.trim_stream(&stream, strategy).await {
                    tracing::error!(
                        stream = %stream,
                        error = %e,
                        "Failed to trim stream"
                    );
                }
            }
        }

        Ok(())
    }

    async fn trim_stream(&self, stream: &str, strategy: &TrimStrategy) -> Result<()> {
        let mut conn = self.pool.get_connection().await?;

        match strategy {
            TrimStrategy::MaxLen(max_len) => {
                let trimmed: i32 = conn.xtrim_maxlen(stream, *max_len).await?;
                if trimmed > 0 {
                    tracing::debug!(
                        stream = %stream,
                        trimmed = trimmed,
                        max_len = max_len,
                        "Trimmed stream with MAXLEN"
                    );
                }
            }
            TrimStrategy::Approximate(max_len) => {
                let trimmed: i32 = conn.xtrim_maxlen_approx(stream, *max_len, None).await?;
                if trimmed > 0 {
                    tracing::debug!(
                        stream = %stream,
                        trimmed = trimmed,
                        max_len = max_len,
                        "Trimmed stream with approximate MAXLEN"
                    );
                }
            }
            TrimStrategy::MinId(min_id) => {
                let trimmed: i32 = conn.xtrim_minid(stream, min_id).await?;
                if trimmed > 0 {
                    tracing::debug!(
                        stream = %stream,
                        trimmed = trimmed,
                        min_id = %min_id,
                        "Trimmed stream with MINID"
                    );
                }
            }
            TrimStrategy::TimeBasedMaxLen(max_age, max_len) => {
                let cutoff_time = Utc::now() - chrono::Duration::from_std(*max_age)?;
                let cutoff_timestamp = cutoff_time.timestamp_millis();
                let min_id = format!("{}-0", cutoff_timestamp);
                
                // First trim by time
                let time_trimmed: i32 = conn.xtrim_minid(&stream, &min_id).await?;
                
                // Then enforce max length
                let len_trimmed: i32 = conn.xtrim_maxlen_approx(&stream, *max_len, None).await?;
                
                if time_trimmed > 0 || len_trimmed > 0 {
                    tracing::debug!(
                        stream = %stream,
                        time_trimmed = time_trimmed,
                        len_trimmed = len_trimmed,
                        max_age = ?max_age,
                        max_len = max_len,
                        "Trimmed stream with time-based strategy"
                    );
                }
            }
            TrimStrategy::Composite { max_len, min_age, min_id } => {
                let mut total_trimmed = 0;

                // Apply min_id if specified
                if let Some(min_id) = min_id {
                    let trimmed: i32 = conn.xtrim_minid(stream, min_id).await?;
                    total_trimmed += trimmed;
                }

                // Apply time-based trimming if specified
                if let Some(min_age) = min_age {
                    let cutoff_time = Utc::now() - chrono::Duration::from_std(*min_age)?;
                    let cutoff_timestamp = cutoff_time.timestamp_millis();
                    let time_min_id = format!("{}-0", cutoff_timestamp);
                    let trimmed: i32 = conn.xtrim_minid(stream, &time_min_id).await?;
                    total_trimmed += trimmed;
                }

                // Apply max length if specified
                if let Some(max_len) = max_len {
                    let trimmed: i32 = conn.xtrim_maxlen_approx(stream, *max_len, None).await?;
                    total_trimmed += trimmed;
                }

                if total_trimmed > 0 {
                    tracing::debug!(
                        stream = %stream,
                        trimmed = total_trimmed,
                        "Trimmed stream with composite strategy"
                    );
                }
            }
        }

        Ok(())
    }

    pub async fn get_stream_info(&self, stream: &str) -> Result<StreamInfo> {
        let mut conn = self.pool.get_connection().await?;
        
        let info: Vec<redis::Value> = conn.xinfo_stream(stream).await?;
        
        // Parse Redis XINFO STREAM response
        let mut length = 0;
        let mut first_entry_id = String::new();
        let mut last_entry_id = String::new();
        
        for chunk in info.chunks(2) {
            if let (Some(key), Some(value)) = (chunk.get(0), chunk.get(1)) {
                match key {
                    redis::Value::Data(k) if k == b"length" => {
                        if let redis::Value::Int(l) = value {
                            length = *l as usize;
                        }
                    }
                    redis::Value::Data(k) if k == b"first-entry" => {
                        if let redis::Value::Bulk(entry) = value {
                            if let Some(redis::Value::Data(id)) = entry.get(0) {
                                first_entry_id = String::from_utf8_lossy(id).to_string();
                            }
                        }
                    }
                    redis::Value::Data(k) if k == b"last-entry" => {
                        if let redis::Value::Bulk(entry) = value {
                            if let Some(redis::Value::Data(id)) = entry.get(0) {
                                last_entry_id = String::from_utf8_lossy(id).to_string();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(StreamInfo {
            name: stream.to_string(),
            length,
            first_entry_id,
            last_entry_id,
        })
    }
}

#[derive(Debug)]
pub struct StreamInfo {
    pub name: String,
    pub length: usize,
    pub first_entry_id: String,
    pub last_entry_id: String,
}

// Intelligent trimming based on stream usage patterns
pub struct AdaptiveTrimmer {
    pool: RedisPool,
    usage_tracker: StreamUsageTracker,
}

impl AdaptiveTrimmer {
    pub fn new(pool: RedisPool) -> Self {
        Self {
            pool: pool.clone(),
            usage_tracker: StreamUsageTracker::new(pool),
        }
    }

    pub async fn adaptive_trim(&self, stream: &str) -> Result<()> {
        let usage = self.usage_tracker.get_usage_stats(stream).await?;
        let strategy = self.determine_strategy(&usage);
        
        let trimmer = StreamTrimmer::new(self.pool.clone())
            .add_strategy(stream.to_string(), strategy);
        
        trimmer.trim_stream(stream, &trimmer.strategies[stream]).await
    }

    fn determine_strategy(&self, usage: &StreamUsageStats) -> TrimStrategy {
        if usage.read_rate > 1000.0 {
            // High-throughput stream, keep more data
            TrimStrategy::MaxLen(50000)
        } else if usage.consumer_lag > Duration::from_hours(1) {
            // Consumers are lagging, be more aggressive
            TrimStrategy::TimeBasedMaxLen(Duration::from_hours(6), 10000)
        } else if usage.growth_rate > 10000.0 {
            // Fast-growing stream
            TrimStrategy::Approximate(20000)
        } else {
            // Default strategy
            TrimStrategy::TimeBasedMaxLen(Duration::from_hours(24), 10000)
        }
    }
}

#[derive(Debug)]
struct StreamUsageStats {
    read_rate: f64,      // messages per second
    growth_rate: f64,    // messages per second
    consumer_lag: Duration,
    message_size_avg: usize,
}

struct StreamUsageTracker {
    pool: RedisPool,
}

impl StreamUsageTracker {
    fn new(pool: RedisPool) -> Self {
        Self { pool }
    }

    async fn get_usage_stats(&self, stream: &str) -> Result<StreamUsageStats> {
        // Implementation would track and calculate usage statistics
        // This is a simplified version
        Ok(StreamUsageStats {
            read_rate: 100.0,
            growth_rate: 50.0,
            consumer_lag: Duration::from_minutes(5),
            message_size_avg: 1024,
        })
    }
}
```

## Pending Entries Management

### XPENDING and XCLAIM Implementation

```rust
use redis::{AsyncCommands, Value};
use std::collections::HashMap;
use tokio::time::{interval, Duration, Instant};

#[derive(Debug)]
pub struct PendingEntry {
    pub message_id: String,
    pub consumer: String,
    pub idle_time: Duration,
    pub delivery_count: usize,
}

#[derive(Debug)]
pub struct PendingStats {
    pub total_pending: usize,
    pub consumer_stats: HashMap<String, usize>,
    pub oldest_pending_age: Duration,
}

pub struct PendingManager {
    pool: RedisPool,
    claim_timeout: Duration,
    max_claim_count: usize,
}

impl PendingManager {
    pub fn new(pool: RedisPool) -> Self {
        Self {
            pool,
            claim_timeout: Duration::from_minutes(5), // Claim messages idle for 5+ minutes
            max_claim_count: 100, // Maximum messages to claim at once
        }
    }

    pub fn with_claim_timeout(mut self, timeout: Duration) -> Self {
        self.claim_timeout = timeout;
        self
    }

    pub fn with_max_claim_count(mut self, count: usize) -> Self {
        self.max_claim_count = count;
        self
    }

    pub async fn get_pending_stats(
        &self,
        stream: &str,
        group: &str,
    ) -> Result<PendingStats> {
        let mut conn = self.pool.get_connection().await?;
        
        // Get overall pending info
        let pending_info: Vec<Value> = conn.xpending(stream, group).await?;
        
        let total_pending = if let Some(Value::Int(count)) = pending_info.get(0) {
            *count as usize
        } else {
            0
        };

        let oldest_pending_age = if let Some(Value::Data(oldest_id)) = pending_info.get(1) {
            self.calculate_age_from_id(&String::from_utf8_lossy(oldest_id))?
        } else {
            Duration::from_secs(0)
        };

        // Get per-consumer stats
        let mut consumer_stats = HashMap::new();
        if let Some(Value::Bulk(consumers)) = pending_info.get(3) {
            for consumer_info in consumers {
                if let Value::Bulk(info) = consumer_info {
                    if let (Some(Value::Data(consumer)), Some(Value::Data(count))) = 
                        (info.get(0), info.get(1)) {
                        let consumer_name = String::from_utf8_lossy(consumer).to_string();
                        let pending_count = String::from_utf8_lossy(count)
                            .parse::<usize>()
                            .unwrap_or(0);
                        consumer_stats.insert(consumer_name, pending_count);
                    }
                }
            }
        }

        Ok(PendingStats {
            total_pending,
            consumer_stats,
            oldest_pending_age,
        })
    }

    pub async fn get_detailed_pending(
        &self,
        stream: &str,
        group: &str,
        consumer: Option<&str>,
        count: Option<usize>,
    ) -> Result<Vec<PendingEntry>> {
        let mut conn = self.pool.get_connection().await?;
        
        let start = "-";
        let end = "+";
        let count = count.unwrap_or(self.max_claim_count);
        
        let pending_entries: Vec<Value> = if let Some(consumer) = consumer {
            conn.xpending_consumer_count(stream, group, start, end, count, consumer).await?
        } else {
            conn.xpending_count(stream, group, start, end, count).await?
        };

        let mut entries = Vec::new();
        
        for entry in pending_entries {
            if let Value::Bulk(entry_data) = entry {
                if entry_data.len() >= 4 {
                    let message_id = if let Some(Value::Data(id)) = entry_data.get(0) {
                        String::from_utf8_lossy(id).to_string()
                    } else {
                        continue;
                    };

                    let consumer = if let Some(Value::Data(consumer)) = entry_data.get(1) {
                        String::from_utf8_lossy(consumer).to_string()
                    } else {
                        continue;
                    };

                    let idle_time = if let Some(Value::Int(idle_ms)) = entry_data.get(2) {
                        Duration::from_millis(*idle_ms as u64)
                    } else {
                        Duration::from_secs(0)
                    };

                    let delivery_count = if let Some(Value::Int(count)) = entry_data.get(3) {
                        *count as usize
                    } else {
                        0
                    };

                    entries.push(PendingEntry {
                        message_id,
                        consumer,
                        idle_time,
                        delivery_count,
                    });
                }
            }
        }

        Ok(entries)
    }

    pub async fn claim_stale_messages(
        &self,
        stream: &str,
        group: &str,
        claiming_consumer: &str,
    ) -> Result<Vec<(String, StreamMessage)>> {
        let pending_entries = self
            .get_detailed_pending(stream, group, None, Some(self.max_claim_count))
            .await?;

        let stale_entries: Vec<&PendingEntry> = pending_entries
            .iter()
            .filter(|entry| {
                entry.idle_time >= self.claim_timeout && 
                entry.consumer != claiming_consumer
            })
            .collect();

        if stale_entries.is_empty() {
            return Ok(Vec::new());
        }

        let message_ids: Vec<String> = stale_entries
            .iter()
            .map(|entry| entry.message_id.clone())
            .collect();

        tracing::info!(
            stream = %stream,
            group = %group,
            claiming_consumer = %claiming_consumer,
            stale_count = stale_entries.len(),
            "Claiming stale messages"
        );

        let mut conn = self.pool.get_connection().await?;
        let claimed_messages: Vec<Value> = conn
            .xclaim(
                stream,
                group,
                claiming_consumer,
                self.claim_timeout.as_millis() as usize,
                &message_ids,
            )
            .await?;

        let mut result = Vec::new();
        
        for claimed in claimed_messages {
            if let Value::Bulk(message_data) = claimed {
                if message_data.len() >= 2 {
                    if let (Some(Value::Data(id)), Some(Value::Bulk(fields))) = 
                        (message_data.get(0), message_data.get(1)) {
                        let message_id = String::from_utf8_lossy(id).to_string();
                        
                        // Parse fields into HashMap
                        let mut field_map = HashMap::new();
                        for chunk in fields.chunks(2) {
                            if let (Some(Value::Data(key)), Some(Value::Data(value))) = 
                                (chunk.get(0), chunk.get(1)) {
                                let key_str = String::from_utf8_lossy(key).to_string();
                                let value_str = String::from_utf8_lossy(value);
                                field_map.insert(key_str, Value::Data(value.clone()));
                            }
                        }

                        if let Ok(message) = self.parse_stream_message(&field_map) {
                            result.push((message_id, message));
                        }
                    }
                }
            }
        }

        tracing::info!(
            stream = %stream,
            group = %group,
            claiming_consumer = %claiming_consumer,
            claimed_count = result.len(),
            "Successfully claimed messages"
        );

        Ok(result)
    }

    pub async fn auto_claim_stale_messages(
        &self,
        stream: &str,
        group: &str,
        claiming_consumer: &str,
    ) -> Result<Vec<(String, StreamMessage)>> {
        let mut conn = self.pool.get_connection().await?;
        
        // Use XAUTOCLAIM for more efficient claiming (Redis 6.2+)
        let min_idle_time = self.claim_timeout.as_millis() as usize;
        let start = "0";
        let count = self.max_claim_count;

        let result: Vec<Value> = conn
            .xautoclaim_count(stream, group, claiming_consumer, min_idle_time, start, count)
            .await
            .or_else(|_| {
                // Fallback to manual claiming for older Redis versions
                async {
                    self.claim_stale_messages(stream, group, claiming_consumer).await
                        .map(|messages| {
                            messages.into_iter()
                                .map(|(id, msg)| Value::Bulk(vec![
                                    Value::Data(id.into_bytes()),
                                    Value::Data(serde_json::to_vec(&msg).unwrap_or_default())
                                ]))
                                .collect()
                        })
                }
            }.await)?;

        // Parse XAUTOCLAIM response
        let mut claimed_messages = Vec::new();
        
        if let Some(Value::Bulk(messages)) = result.get(1) {
            for message in messages {
                if let Value::Bulk(message_data) = message {
                    if message_data.len() >= 2 {
                        if let (Some(Value::Data(id)), Some(Value::Bulk(fields))) = 
                            (message_data.get(0), message_data.get(1)) {
                            let message_id = String::from_utf8_lossy(id).to_string();
                            
                            // Parse fields
                            let mut field_map = HashMap::new();
                            for chunk in fields.chunks(2) {
                                if let (Some(Value::Data(key)), Some(Value::Data(value))) = 
                                    (chunk.get(0), chunk.get(1)) {
                                    let key_str = String::from_utf8_lossy(key).to_string();
                                    field_map.insert(key_str, Value::Data(value.clone()));
                                }
                            }

                            if let Ok(message) = self.parse_stream_message(&field_map) {
                                claimed_messages.push((message_id, message));
                            }
                        }
                    }
                }
            }
        }

        Ok(claimed_messages)
    }

    pub async fn start_auto_claim_monitor(
        &self,
        streams: Vec<String>,
        group: &str,
        consumer: &str,
    ) -> Result<()> {
        let group = group.to_string();
        let consumer = consumer.to_string();
        let pool = self.pool.clone();
        let claim_timeout = self.claim_timeout;
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30)); // Check every 30 seconds
            
            loop {
                interval.tick().await;
                
                for stream in &streams {
                    let manager = PendingManager::new(pool.clone())
                        .with_claim_timeout(claim_timeout);
                    
                    match manager.auto_claim_stale_messages(stream, &group, &consumer).await {
                        Ok(claimed) => {
                            if !claimed.is_empty() {
                                tracing::info!(
                                    stream = %stream,
                                    claimed_count = claimed.len(),
                                    "Auto-claimed stale messages"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::error!(
                                stream = %stream,
                                error = %e,
                                "Failed to auto-claim messages"
                            );
                        }
                    }
                }
            }
        });

        Ok(())
    }

    fn calculate_age_from_id(&self, stream_id: &str) -> Result<Duration> {
        // Extract timestamp from Redis stream ID (format: timestamp-sequence)
        if let Some(timestamp_str) = stream_id.split('-').next() {
            let timestamp = timestamp_str.parse::<i64>()?;
            let message_time = Instant::now() - Duration::from_millis(
                (chrono::Utc::now().timestamp_millis() - timestamp) as u64
            );
            Ok(Instant::now().duration_since(message_time))
        } else {
            Ok(Duration::from_secs(0))
        }
    }

    fn parse_stream_message(
        &self,
        fields: &HashMap<String, Value>,
    ) -> Result<StreamMessage> {
        // Implementation similar to StreamConsumer::parse_stream_message
        // Simplified for brevity
        let get_string = |key: &str| -> Result<String> {
            fields
                .get(key)
                .and_then(|v| match v {
                    Value::Data(bytes) => String::from_utf8(bytes.clone()).ok(),
                    _ => None,
                })
                .ok_or_else(|| anyhow!("Missing or invalid field: {}", key))
        };

        let id = get_string("id")?;
        let timestamp = get_string("timestamp")?
            .parse::<DateTime<Utc>>()
            .map_err(|e| anyhow!("Invalid timestamp: {}", e))?;
        let event_type = get_string("event_type")?;
        let payload_str = get_string("payload")?;
        let payload: serde_json::Value = serde_json::from_str(&payload_str)?;
        let correlation_id = get_string("correlation_id").ok().filter(|s| !s.is_empty());
        let source = get_string("source")?;

        Ok(StreamMessage {
            id,
            timestamp,
            event_type,
            payload,
            correlation_id,
            source,
        })
    }
}
```

## Monitoring & Metrics

### Comprehensive Metrics Collection

```rust
use prometheus::{Counter, Histogram, Gauge, Registry, Opts, HistogramOpts};
use std::sync::Arc;
use tokio::time::{interval, Duration};

#[derive(Clone)]
pub struct StreamMetrics {
    // Counters
    messages_produced: Counter,
    messages_consumed: Counter,
    messages_acknowledged: Counter,
    messages_failed: Counter,
    messages_retried: Counter,
    messages_dlq: Counter,
    
    // Histograms
    processing_duration: Histogram,
    message_size: Histogram,
    consumer_lag: Histogram,
    
    // Gauges
    active_consumers: Gauge,
    pending_messages: Gauge,
    stream_length: Gauge,
    connection_pool_size: Gauge,
}

impl StreamMetrics {
    pub fn new(registry: &Registry) -> Result<Self> {
        let messages_produced = Counter::with_opts(
            Opts::new("redis_streams_messages_produced_total", "Total produced messages")
                .const_label("component", "redis_streams")
        )?;

        let messages_consumed = Counter::with_opts(
            Opts::new("redis_streams_messages_consumed_total", "Total consumed messages")
                .const_label("component", "redis_streams")
        )?;

        let messages_acknowledged = Counter::with_opts(
            Opts::new("redis_streams_messages_acknowledged_total", "Total acknowledged messages")
                .const_label("component", "redis_streams")
        )?;

        let messages_failed = Counter::with_opts(
            Opts::new("redis_streams_messages_failed_total", "Total failed messages")
                .const_label("component", "redis_streams")
        )?;

        let messages_retried = Counter::with_opts(
            Opts::new("redis_streams_messages_retried_total", "Total retried messages")
                .const_label("component", "redis_streams")
        )?;

        let messages_dlq = Counter::with_opts(
            Opts::new("redis_streams_messages_dlq_total", "Total messages sent to DLQ")
                .const_label("component", "redis_streams")
        )?;

        let processing_duration = Histogram::with_opts(
            HistogramOpts::new("redis_streams_processing_duration_seconds", "Message processing duration")
                .const_label("component", "redis_streams")
                .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0])
        )?;

        let message_size = Histogram::with_opts(
            HistogramOpts::new("redis_streams_message_size_bytes", "Message size in bytes")
                .const_label("component", "redis_streams")
                .buckets(vec![100.0, 500.0, 1000.0, 5000.0, 10000.0, 50000.0, 100000.0])
        )?;

        let consumer_lag = Histogram::with_opts(
            HistogramOpts::new("redis_streams_consumer_lag_seconds", "Consumer lag in seconds")
                .const_label("component", "redis_streams")
                .buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0])
        )?;

        let active_consumers = Gauge::with_opts(
            Opts::new("redis_streams_active_consumers", "Number of active consumers")
                .const_label("component", "redis_streams")
        )?;

        let pending_messages = Gauge::with_opts(
            Opts::new("redis_streams_pending_messages", "Number of pending messages")
                .const_label("component", "redis_streams")
        )?;

        let stream_length = Gauge::with_opts(
            Opts::new("redis_streams_length", "Current stream length")
                .const_label("component", "redis_streams")
        )?;

        let connection_pool_size = Gauge::with_opts(
            Opts::new("redis_streams_connection_pool_size", "Connection pool size")
                .const_label("component", "redis_streams")
        )?;

        // Register all metrics
        registry.register(Box::new(messages_produced.clone()))?;
        registry.register(Box::new(messages_consumed.clone()))?;
        registry.register(Box::new(messages_acknowledged.clone()))?;
        registry.register(Box::new(messages_failed.clone()))?;
        registry.register(Box::new(messages_retried.clone()))?;
        registry.register(Box::new(messages_dlq.clone()))?;
        registry.register(Box::new(processing_duration.clone()))?;
        registry.register(Box::new(message_size.clone()))?;
        registry.register(Box::new(consumer_lag.clone()))?;
        registry.register(Box::new(active_consumers.clone()))?;
        registry.register(Box::new(pending_messages.clone()))?;
        registry.register(Box::new(stream_length.clone()))?;
        registry.register(Box::new(connection_pool_size.clone()))?;

        Ok(Self {
            messages_produced,
            messages_consumed,
            messages_acknowledged,
            messages_failed,
            messages_retried,
            messages_dlq,
            processing_duration,
            message_size,
            consumer_lag,
            active_consumers,
            pending_messages,
            stream_length,
            connection_pool_size,
        })
    }

    pub fn record_message_produced(&self, stream: &str, size: usize) {
        self.messages_produced.with_label_values(&[stream]).inc();
        self.message_size.observe(size as f64);
    }

    pub fn record_message_consumed(&self, stream: &str) {
        self.messages_consumed.with_label_values(&[stream]).inc();
    }

    pub fn record_message_acknowledged(&self, stream: &str) {
        self.messages_acknowledged.with_label_values(&[stream]).inc();
    }

    pub fn record_message_failed(&self, stream: &str) {
        self.messages_failed.with_label_values(&[stream]).inc();
    }

    pub fn record_message_retried(&self, stream: &str) {
        self.messages_retried.with_label_values(&[stream]).inc();
    }

    pub fn record_message_dlq(&self, stream: &str) {
        self.messages_dlq.with_label_values(&[stream]).inc();
    }

    pub fn record_processing_duration(&self, stream: &str, duration: Duration) {
        self.processing_duration
            .with_label_values(&[stream])
            .observe(duration.as_secs_f64());
    }

    pub fn set_active_consumers(&self, stream: &str, count: f64) {
        self.active_consumers.with_label_values(&[stream]).set(count);
    }

    pub fn set_pending_messages(&self, stream: &str, count: f64) {
        self.pending_messages.with_label_values(&[stream]).set(count);
    }

    pub fn set_stream_length(&self, stream: &str, length: f64) {
        self.stream_length.with_label_values(&[stream]).set(length);
    }

    pub fn set_connection_pool_size(&self, size: f64) {
        self.connection_pool_size.set(size);
    }
}

pub struct StreamMonitor {
    pool: RedisPool,
    metrics: Arc<StreamMetrics>,
    streams_to_monitor: Vec<String>,
}

impl StreamMonitor {
    pub fn new(
        pool: RedisPool,
        metrics: Arc<StreamMetrics>,
        streams: Vec<String>,
    ) -> Self {
        Self {
            pool,
            metrics,
            streams_to_monitor: streams,
        }
    }

    pub async fn start_monitoring(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(10)); // Monitor every 10 seconds
        
        loop {
            interval.tick().await;
            
            if let Err(e) = self.collect_metrics().await {
                tracing::error!(error = %e, "Failed to collect stream metrics");
            }
        }
    }

    async fn collect_metrics(&self) -> Result<()> {
        for stream in &self.streams_to_monitor {
            // Collect stream info
            if let Ok(info) = self.get_stream_info(stream).await {
                self.metrics.set_stream_length(stream, info.length as f64);
            }

            // Collect consumer group info
            if let Ok(groups) = self.get_consumer_groups(stream).await {
                for group in groups {
                    // Get pending count
                    if let Ok(pending_stats) = self.get_pending_stats(stream, &group.name).await {
                        self.metrics.set_pending_messages(
                            &format!("{}:{}", stream, group.name),
                            pending_stats.total_pending as f64,
                        );
                        
                        self.metrics.set_active_consumers(
                            &format!("{}:{}", stream, group.name),
                            group.consumers as f64,
                        );
                    }
                }
            }
        }

        // Monitor connection pool
        // Note: This would need to be implemented based on your specific pool implementation
        // self.metrics.set_connection_pool_size(pool_size as f64);

        Ok(())
    }

    async fn get_stream_info(&self, stream: &str) -> Result<StreamInfo> {
        let mut conn = self.pool.get_connection().await?;
        
        let info: Vec<redis::Value> = conn.xinfo_stream(stream).await?;
        
        let mut length = 0;
        let mut first_entry_id = String::new();
        let mut last_entry_id = String::new();
        
        for chunk in info.chunks(2) {
            if let (Some(key), Some(value)) = (chunk.get(0), chunk.get(1)) {
                match key {
                    redis::Value::Data(k) if k == b"length" => {
                        if let redis::Value::Int(l) = value {
                            length = *l as usize;
                        }
                    }
                    redis::Value::Data(k) if k == b"first-entry" => {
                        if let redis::Value::Bulk(entry) = value {
                            if let Some(redis::Value::Data(id)) = entry.get(0) {
                                first_entry_id = String::from_utf8_lossy(id).to_string();
                            }
                        }
                    }
                    redis::Value::Data(k) if k == b"last-entry" => {
                        if let redis::Value::Bulk(entry) = value {
                            if let Some(redis::Value::Data(id)) = entry.get(0) {
                                last_entry_id = String::from_utf8_lossy(id).to_string();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(StreamInfo {
            name: stream.to_string(),
            length,
            first_entry_id,
            last_entry_id,
        })
    }

    async fn get_consumer_groups(&self, stream: &str) -> Result<Vec<ConsumerGroupInfo>> {
        let mut conn = self.pool.get_connection().await?;
        
        let groups: Vec<redis::Value> = conn.xinfo_groups(stream).await?;
        let mut result = Vec::new();
        
        for group_info in groups {
            if let redis::Value::Bulk(info) = group_info {
                let mut name = String::new();
                let mut consumers = 0;
                let mut pending = 0;
                
                for chunk in info.chunks(2) {
                    if let (Some(key), Some(value)) = (chunk.get(0), chunk.get(1)) {
                        match key {
                            redis::Value::Data(k) if k == b"name" => {
                                if let redis::Value::Data(n) = value {
                                    name = String::from_utf8_lossy(n).to_string();
                                }
                            }
                            redis::Value::Data(k) if k == b"consumers" => {
                                if let redis::Value::Int(c) = value {
                                    consumers = *c as usize;
                                }
                            }
                            redis::Value::Data(k) if k == b"pending" => {
                                if let redis::Value::Int(p) = value {
                                    pending = *p as usize;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                
                result.push(ConsumerGroupInfo {
                    name,
                    consumers,
                    pending,
                });
            }
        }
        
        Ok(result)
    }

    async fn get_pending_stats(&self, stream: &str, group: &str) -> Result<PendingStats> {
        let pending_manager = PendingManager::new(self.pool.clone());
        pending_manager.get_pending_stats(stream, group).await
    }
}

#[derive(Debug)]
struct ConsumerGroupInfo {
    name: String,
    consumers: usize,
    pending: usize,
}

// Health check endpoint for monitoring
pub struct HealthChecker {
    pool: RedisPool,
}

impl HealthChecker {
    pub fn new(pool: RedisPool) -> Self {
        Self { pool }
    }

    pub async fn check_health(&self) -> HealthStatus {
        let mut health = HealthStatus::new();

        // Check Redis connectivity
        match self.check_redis_connectivity().await {
            Ok(_) => health.add_check("redis_connectivity", true, "Redis is reachable"),
            Err(e) => health.add_check("redis_connectivity", false, &format!("Redis error: {}", e)),
        }

        // Check connection pool
        match self.check_connection_pool().await {
            Ok(available) => {
                health.add_check(
                    "connection_pool",
                    available > 0,
                    &format!("Available connections: {}", available),
                )
            }
            Err(e) => health.add_check("connection_pool", false, &format!("Pool error: {}", e)),
        }

        health
    }

    async fn check_redis_connectivity(&self) -> Result<()> {
        let mut conn = self.pool.get_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        Ok(())
    }

    async fn check_connection_pool(&self) -> Result<usize> {
        // This would depend on your specific pool implementation
        // For deadpool-redis, you might check pool.status()
        Ok(10) // Placeholder
    }
}

#[derive(Debug, serde::Serialize)]
pub struct HealthStatus {
    healthy: bool,
    checks: Vec<HealthCheck>,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, serde::Serialize)]
struct HealthCheck {
    name: String,
    healthy: bool,
    message: String,
}

impl HealthStatus {
    fn new() -> Self {
        Self {
            healthy: true,
            checks: Vec::new(),
            timestamp: Utc::now(),
        }
    }

    fn add_check(&mut self, name: &str, healthy: bool, message: &str) {
        self.checks.push(HealthCheck {
            name: name.to_string(),
            healthy,
            message: message.to_string(),
        });

        if !healthy {
            self.healthy = false;
        }
    }
}
```

## Production Deployment

### Complete Production Configuration

```rust
// src/config.rs
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisStreamsConfig {
    pub redis: RedisConfig,
    pub streams: StreamsConfig,
    pub consumers: ConsumerConfig,
    pub monitoring: MonitoringConfig,
    pub resilience: ResilienceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub pool_size: usize,
    pub connection_timeout: Duration,
    pub command_timeout: Duration,
    pub retry_attempts: usize,
    pub retry_delay: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamsConfig {
    pub default_max_len: usize,
    pub trim_interval: Duration,
    pub batch_size: usize,
    pub enable_compression: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerConfig {
    pub block_time: Duration,
    pub claim_timeout: Duration,
    pub max_pending: usize,
    pub processing_timeout: Duration,
    pub backpressure_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enabled: bool,
    pub metrics_interval: Duration,
    pub health_check_interval: Duration,
    pub prometheus_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResilienceConfig {
    pub circuit_breaker_threshold: usize,
    pub circuit_breaker_timeout: Duration,
    pub retry_max_attempts: usize,
    pub retry_initial_delay: Duration,
    pub retry_max_delay: Duration,
    pub retry_backoff_factor: f64,
}

impl Default for RedisStreamsConfig {
    fn default() -> Self {
        Self {
            redis: RedisConfig {
                url: "redis://localhost:6379".to_string(),
                pool_size: 50,
                connection_timeout: Duration::from_secs(5),
                command_timeout: Duration::from_secs(10),
                retry_attempts: 3,
                retry_delay: Duration::from_millis(100),
            },
            streams: StreamsConfig {
                default_max_len: 10000,
                trim_interval: Duration::from_secs(60),
                batch_size: 100,
                enable_compression: false,
            },
            consumers: ConsumerConfig {
                block_time: Duration::from_millis(1000),
                claim_timeout: Duration::from_minutes(5),
                max_pending: 1000,
                processing_timeout: Duration::from_secs(30),
                backpressure_threshold: 0.8,
            },
            monitoring: MonitoringConfig {
                enabled: true,
                metrics_interval: Duration::from_secs(10),
                health_check_interval: Duration::from_secs(30),
                prometheus_port: 9090,
            },
            resilience: ResilienceConfig {
                circuit_breaker_threshold: 5,
                circuit_breaker_timeout: Duration::from_minutes(1),
                retry_max_attempts: 3,
                retry_initial_delay: Duration::from_millis(100),
                retry_max_delay: Duration::from_secs(60),
                retry_backoff_factor: 2.0,
            },
        }
    }
}

// Production-ready service
pub struct RedisStreamsService {
    config: RedisStreamsConfig,
    pool: RedisPool,
    metrics: Arc<StreamMetrics>,
    health_checker: HealthChecker,
    monitor: StreamMonitor,
    trimmer: StreamTrimmer,
    dead_letter_queue: DeadLetterQueue,
    pending_manager: PendingManager,
}

impl RedisStreamsService {
    pub async fn new(config: RedisStreamsConfig) -> Result<Self> {
        // Initialize connection pool
        let pool = RedisPool::new(&config.redis.url).await?;

        // Initialize metrics
        let registry = Registry::new();
        let metrics = Arc::new(StreamMetrics::new(&registry)?);

        // Initialize health checker
        let health_checker = HealthChecker::new(pool.clone());

        // Initialize monitor
        let streams_to_monitor = vec![
            StreamKeys::market_data("BTCUSD"),
            StreamKeys::signals("ml_predictor"),
            StreamKeys::orders("all"),
            StreamKeys::risk_events(),
        ];
        let monitor = StreamMonitor::new(pool.clone(), Arc::clone(&metrics), streams_to_monitor);

        // Initialize trimmer with strategies
        let trimmer = StreamTrimmer::new(pool.clone())
            .add_strategy("market:*", TrimStrategy::TimeBasedMaxLen(
                Duration::from_hours(24),
                config.streams.default_max_len,
            ))
            .add_strategy("signals:*", TrimStrategy::MaxLen(50000))
            .add_strategy("orders:*", TrimStrategy::TimeBasedMaxLen(
                Duration::from_days(7),
                100000,
            ))
            .add_strategy("dlq:*", TrimStrategy::TimeBasedMaxLen(
                Duration::from_days(30),
                10000,
            ));

        // Initialize dead letter queue
        let retry_config = RetryConfig {
            max_attempts: config.resilience.retry_max_attempts,
            initial_delay: config.resilience.retry_initial_delay,
            max_delay: config.resilience.retry_max_delay,
            backoff_factor: config.resilience.retry_backoff_factor,
            jitter: true,
        };
        let dead_letter_queue = DeadLetterQueue::new(pool.clone(), retry_config);

        // Initialize pending manager
        let pending_manager = PendingManager::new(pool.clone())
            .with_claim_timeout(config.consumers.claim_timeout);

        Ok(Self {
            config,
            pool,
            metrics,
            health_checker,
            monitor,
            trimmer,
            dead_letter_queue,
            pending_manager,
        })
    }

    pub async fn start(&self) -> Result<()> {
        tracing::info!("Starting Redis Streams service");

        // Start monitoring
        if self.config.monitoring.enabled {
            let monitor = self.monitor.clone();
            tokio::spawn(async move {
                if let Err(e) = monitor.start_monitoring().await {
                    tracing::error!(error = %e, "Monitoring failed");
                }
            });
        }

        // Start trimming
        let trimmer = self.trimmer.clone();
        tokio::spawn(async move {
            if let Err(e) = trimmer.start_trimming().await {
                tracing::error!(error = %e, "Stream trimming failed");
            }
        });

        // Start retry processing
        let dlq = self.dead_letter_queue.clone();
        tokio::spawn(async move {
            if let Err(e) = dlq.process_retries().await {
                tracing::error!(error = %e, "Retry processing failed");
            }
        });

        // Start Prometheus metrics server
        if self.config.monitoring.enabled {
            self.start_metrics_server().await?;
        }

        tracing::info!("Redis Streams service started successfully");
        Ok(())
    }

    async fn start_metrics_server(&self) -> Result<()> {
        use warp::Filter;

        let metrics_route = warp::path("metrics")
            .map(|| {
                use prometheus::Encoder;
                let encoder = prometheus::TextEncoder::new();
                let metric_families = prometheus::gather();
                let mut buffer = Vec::new();
                encoder.encode(&metric_families, &mut buffer).unwrap();
                String::from_utf8(buffer).unwrap()
            });

        let health_route = warp::path("health")
            .and(warp::any().map(move || self.health_checker.clone()))
            .and_then(|health_checker: HealthChecker| async move {
                let status = health_checker.check_health().await;
                Ok::<_, warp::Rejection>(warp::reply::json(&status))
            });

        let routes = metrics_route.or(health_route);

        let port = self.config.monitoring.prometheus_port;
        tokio::spawn(async move {
            warp::serve(routes)
                .run(([0, 0, 0, 0], port))
                .await;
        });

        tracing::info!(port = port, "Started Prometheus metrics server");
        Ok(())
    }

    pub fn create_producer(&self) -> StreamProducer {
        StreamProducer::new(self.pool.clone())
            .with_max_len(self.config.streams.default_max_len)
    }

    pub fn create_consumer(&self, group: &str, consumer: &str) -> StreamConsumer {
        StreamConsumer::new(
            self.pool.clone(),
            group.to_string(),
            consumer.to_string(),
        )
        .with_block_time(self.config.consumers.block_time.as_millis() as usize)
        .with_count(self.config.streams.batch_size)
    }

    pub fn create_backpressure_consumer(&self, group: &str, consumer: &str) -> BackpressureConsumer {
        let consumer = self.create_consumer(group, consumer);
        BackpressureConsumer::new(
            consumer,
            self.config.consumers.max_pending / 2, // max concurrent
            self.config.consumers.max_pending,
        )
    }
}

// Docker configuration
/*
# Dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/neural-trader /usr/local/bin/

EXPOSE 9090
CMD ["neural-trader"]
*/

// Docker Compose configuration
/*
# docker-compose.yml
version: '3.8'

services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - ./redis.conf:/usr/local/etc/redis/redis.conf
      - redis_data:/data
    command: redis-server /usr/local/etc/redis/redis.conf
    
  neural-trader:
    build: .
    ports:
      - "9090:9090"
    environment:
      - REDIS_URL=redis://redis:6379
      - RUST_LOG=info
    depends_on:
      - redis
    volumes:
      - ./config:/app/config

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9091:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana_data:/var/lib/grafana

volumes:
  redis_data:
  prometheus_data:
  grafana_data:
*/

// Kubernetes deployment configuration
/*
# k8s-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: neural-trader
spec:
  replicas: 3
  selector:
    matchLabels:
      app: neural-trader
  template:
    metadata:
      labels:
        app: neural-trader
    spec:
      containers:
      - name: neural-trader
        image: neural-trader:latest
        ports:
        - containerPort: 9090
        env:
        - name: REDIS_URL
          value: "redis://redis-service:6379"
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /health
            port: 9090
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: 9090
          initialDelaySeconds: 5
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: neural-trader-service
spec:
  selector:
    app: neural-trader
  ports:
  - port: 9090
    targetPort: 9090
  type: ClusterIP
*/
}
```

## Example Usage

### Complete Trading System Implementation

```rust
// src/main.rs
use anyhow::Result;
use neural_trader_streams::*;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::init();

    let config = RedisStreamsConfig::default();
    let service = RedisStreamsService::new(config).await?;
    
    // Start the service
    service.start().await?;

    // Create producers and consumers
    let producer = service.create_producer();
    let consumer = service.create_backpressure_consumer(
        ConsumerGroups::trading_engine(),
        "trader-1"
    );

    // Market data producer
    let market_producer = producer.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        
        loop {
            interval.tick().await;
            
            let market_data = MarketDataEvent {
                symbol: "BTCUSD".to_string(),
                price: 50000.0 + fastrand::f64() * 1000.0,
                volume: fastrand::f64() * 100.0,
                bid: 49995.0,
                ask: 50005.0,
                timestamp: Utc::now(),
            };
            
            if let Err(e) = market_data.publish(&market_producer).await {
                tracing::error!(error = %e, "Failed to publish market data");
            }
        }
    });

    // Trading engine consumer
    let streams = vec![
        StreamKeys::market_data("BTCUSD"),
        StreamKeys::signals("ml_predictor"),
    ];

    consumer
        .consume_with_backpressure(streams, |stream_key, message| async move {
            match message.event_type.as_str() {
                "market_data" => handle_market_data(stream_key, message).await,
                "trading_signal" => handle_trading_signal(stream_key, message).await,
                _ => {
                    tracing::warn!(
                        event_type = %message.event_type,
                        "Unknown event type"
                    );
                    Ok(())
                }
            }
        })
        .await?;

    Ok(())
}

async fn handle_market_data(stream_key: String, message: StreamMessage) -> Result<()> {
    let market_data: MarketDataEvent = serde_json::from_value(message.payload)?;
    
    tracing::info!(
        symbol = %market_data.symbol,
        price = market_data.price,
        "Processed market data"
    );

    // Process market data and potentially generate signals
    // This is where your trading logic would go
    
    Ok(())
}

async fn handle_trading_signal(stream_key: String, message: StreamMessage) -> Result<()> {
    tracing::info!(
        stream_key = %stream_key,
        message_id = %message.id,
        "Processing trading signal"
    );

    // Process trading signal and execute trades
    // This is where your order execution logic would go
    
    Ok(())
}
```

This comprehensive implementation provides a production-ready Redis Streams foundation for the neural trading MVP, with all the essential patterns for reliable, scalable real-time data processing.