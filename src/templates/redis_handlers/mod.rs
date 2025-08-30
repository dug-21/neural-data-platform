//! Redis Streams Message Handler Template
//! 
//! This template provides a robust foundation for handling Redis Streams
//! messages with proper isolation, error handling, and observability.
//! 
//! Key Features:
//! - Stream pattern subscription with isolation
//! - Message routing and filtering
//! - Error handling and circuit breaker patterns
//! - Metrics and tracing integration
//! - Backpressure and flow control
//! - Dead letter queue support

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use redis::{Client, Connection, Commands, RedisResult, streams::StreamReadOptions};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, mpsc, Semaphore};
use tokio::time::{interval, sleep};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::{Result, anyhow};
use tracing::{info, warn, error, debug, span, Level};

use crate::templates::module_boilerplate::{Event, MetricsExporter, TraceExporter};

/// Stream naming convention following the architecture
#[derive(Debug, Clone)]
pub struct StreamPattern {
    pub category: String,  // data, features, decisions, executions, metrics
    pub domain: String,    // trading, system-ops, etc.
    pub source: String,    // alpaca, momentum, etc.
    pub stream_type: String, // raw, processed, confirmed, etc.
}

impl StreamPattern {
    pub fn new(category: &str, domain: &str, source: &str, stream_type: &str) -> Self {
        Self {
            category: category.to_string(),
            domain: domain.to_string(),
            source: source.to_string(),
            stream_type: stream_type.to_string(),
        }
    }

    /// Generate Redis stream name following convention: {category}.{domain}.{source}.{type}
    pub fn stream_name(&self) -> String {
        format!("{}.{}.{}.{}", self.category, self.domain, self.source, self.stream_type)
    }

    /// Create a wildcard pattern for subscription
    pub fn pattern(&self) -> String {
        format!("{}.*.*.*", self.category)
    }

    /// Check if this pattern matches a given stream name
    pub fn matches(&self, stream_name: &str) -> bool {
        let parts: Vec<&str> = stream_name.split('.').collect();
        if parts.len() != 4 {
            return false;
        }

        let (cat, dom, src, typ) = (parts[0], parts[1], parts[2], parts[3]);
        
        (self.category == "*" || self.category == cat) &&
        (self.domain == "*" || self.domain == dom) &&
        (self.source == "*" || self.source == src) &&
        (self.stream_type == "*" || self.stream_type == typ)
    }
}

/// Configuration for Redis Stream handler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamHandlerConfig {
    pub redis_url: String,
    pub consumer_group: String,
    pub consumer_name: String,
    pub subscription_patterns: Vec<String>,
    pub publish_patterns: Vec<String>,
    pub max_concurrent_messages: usize,
    pub message_timeout_ms: u64,
    pub retry_attempts: u32,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_timeout_ms: u64,
    pub dead_letter_queue: String,
    pub metrics_prefix: String,
}

impl Default for StreamHandlerConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
            consumer_group: "default-group".to_string(),
            consumer_name: "default-consumer".to_string(),
            subscription_patterns: Vec::new(),
            publish_patterns: Vec::new(),
            max_concurrent_messages: 100,
            message_timeout_ms: 30000,
            retry_attempts: 3,
            circuit_breaker_threshold: 10,
            circuit_breaker_timeout_ms: 60000,
            dead_letter_queue: "dlq.messages".to_string(),
            metrics_prefix: "redis_handler".to_string(),
        }
    }
}

/// Circuit breaker states for fault tolerance
#[derive(Debug, Clone, PartialEq)]
enum CircuitBreakerState {
    Closed,
    Open { opened_at: Instant },
    HalfOpen,
}

/// Circuit breaker for handling Redis connection failures
#[derive(Debug)]
struct CircuitBreaker {
    state: RwLock<CircuitBreakerState>,
    failure_count: RwLock<u32>,
    threshold: u32,
    timeout: Duration,
}

impl CircuitBreaker {
    fn new(threshold: u32, timeout: Duration) -> Self {
        Self {
            state: RwLock::new(CircuitBreakerState::Closed),
            failure_count: RwLock::new(0),
            threshold,
            timeout,
        }
    }

    async fn can_execute(&self) -> bool {
        let state = self.state.read().await;
        match *state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open { opened_at } => {
                if opened_at.elapsed() > self.timeout {
                    drop(state);
                    let mut state = self.state.write().await;
                    *state = CircuitBreakerState::HalfOpen;
                    true
                } else {
                    false
                }
            }
            CircuitBreakerState::HalfOpen => true,
        }
    }

    async fn record_success(&self) {
        let mut failure_count = self.failure_count.write().await;
        *failure_count = 0;
        
        let mut state = self.state.write().await;
        *state = CircuitBreakerState::Closed;
    }

    async fn record_failure(&self) {
        let mut failure_count = self.failure_count.write().await;
        *failure_count += 1;

        if *failure_count >= self.threshold {
            let mut state = self.state.write().await;
            *state = CircuitBreakerState::Open {
                opened_at: Instant::now(),
            };
        }
    }
}

/// Message handler trait for processing Redis Stream messages
#[async_trait]
pub trait MessageHandler: Send + Sync {
    type PayloadType: Send + Sync + for<'de> Deserialize<'de>;

    /// Process a single message
    async fn handle_message(&self, event: Event<Self::PayloadType>) -> Result<()>;

    /// Get the patterns this handler subscribes to
    fn subscription_patterns(&self) -> Vec<StreamPattern>;

    /// Get the patterns this handler publishes to
    fn publication_patterns(&self) -> Vec<StreamPattern>;

    /// Validate message before processing (optional override)
    async fn validate_message(&self, _event: &Event<Self::PayloadType>) -> Result<()> {
        Ok(())
    }

    /// Handle processing errors (optional override)
    async fn handle_error(&self, event: Event<Self::PayloadType>, error: anyhow::Error) -> Result<()> {
        error!("Message processing failed: {} for event {}", error, event.id);
        Ok(())
    }
}

/// Core Redis Stream handler with isolation and observability
pub struct RedisStreamHandler<H: MessageHandler> {
    config: StreamHandlerConfig,
    redis_client: Client,
    handler: Arc<H>,
    circuit_breaker: CircuitBreaker,
    metrics_exporter: Arc<dyn MetricsExporter>,
    trace_exporter: Arc<dyn TraceExporter>,
    semaphore: Arc<Semaphore>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl<H: MessageHandler + 'static> RedisStreamHandler<H> {
    /// Create a new Redis Stream handler
    pub async fn new(
        config: StreamHandlerConfig,
        handler: H,
        metrics_exporter: Arc<dyn MetricsExporter>,
        trace_exporter: Arc<dyn TraceExporter>,
    ) -> Result<Self> {
        let redis_client = Client::open(config.redis_url.as_str())?;
        
        // Test Redis connection
        let mut conn = redis_client.get_connection()?;
        let _: String = conn.ping()?;

        let circuit_breaker = CircuitBreaker::new(
            config.circuit_breaker_threshold,
            Duration::from_millis(config.circuit_breaker_timeout_ms),
        );

        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_messages));

        Ok(Self {
            config,
            redis_client,
            handler: Arc::new(handler),
            circuit_breaker,
            metrics_exporter,
            trace_exporter,
            semaphore,
            shutdown_tx: None,
        })
    }

    /// Start the message processing loop
    pub async fn start(&mut self) -> Result<()> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        self.shutdown_tx = Some(shutdown_tx);

        // Create consumer groups for subscription patterns
        self.create_consumer_groups().await?;

        info!("Starting Redis Stream handler with config: {:?}", self.config);

        // Start message processing loop
        let handler = self.handler.clone();
        let config = self.config.clone();
        let redis_client = self.redis_client.clone();
        let circuit_breaker = Arc::new(self.circuit_breaker);
        let metrics_exporter = self.metrics_exporter.clone();
        let trace_exporter = self.trace_exporter.clone();
        let semaphore = self.semaphore.clone();

        tokio::spawn(async move {
            let mut processing_interval = interval(Duration::from_millis(100));
            
            loop {
                tokio::select! {
                    _ = processing_interval.tick() => {
                        if circuit_breaker.can_execute().await {
                            if let Err(e) = Self::process_messages(
                                &config,
                                &redis_client,
                                &handler,
                                &circuit_breaker,
                                &metrics_exporter,
                                &trace_exporter,
                                &semaphore,
                            ).await {
                                error!("Error processing messages: {}", e);
                                circuit_breaker.record_failure().await;
                            } else {
                                circuit_breaker.record_success().await;
                            }
                        } else {
                            debug!("Circuit breaker is open, skipping message processing");
                            sleep(Duration::from_millis(1000)).await;
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Received shutdown signal, stopping message processing");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the message processing loop
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(()).await;
        }
        Ok(())
    }

    /// Publish a message to a Redis Stream
    pub async fn publish_message<T: Serialize>(
        &self,
        stream_pattern: &StreamPattern,
        event: Event<T>,
    ) -> Result<()> {
        let span_id = self.trace_exporter
            .start_span(&format!("publish_{}", stream_pattern.stream_name()), None)
            .await;

        let start_time = Instant::now();
        let stream_name = stream_pattern.stream_name();

        // Serialize event
        let serialized = serde_json::to_string(&event)?;

        // Get Redis connection
        let mut conn = self.redis_client.get_connection()?;

        // Publish to stream
        let _: String = conn.xadd(
            &stream_name,
            "*",
            &[("data", serialized), ("correlation_id", event.correlation_id.to_string())],
        )?;

        // Record metrics
        let mut tags = HashMap::new();
        tags.insert("stream".to_string(), stream_name.clone());
        tags.insert("domain".to_string(), event.domain.clone());

        self.metrics_exporter
            .increment_counter(
                &format!("{}_messages_published_total", self.config.metrics_prefix),
                1.0,
                tags.clone(),
            )
            .await;

        let latency_ms = start_time.elapsed().as_millis() as f64;
        self.metrics_exporter
            .record_histogram(
                &format!("{}_publish_latency_ms", self.config.metrics_prefix),
                latency_ms,
                tags,
            )
            .await;

        self.trace_exporter
            .add_span_attribute(&span_id, "stream_name", &stream_name)
            .await;
        self.trace_exporter.end_span(&span_id).await;

        debug!("Published message {} to stream {}", event.id, stream_name);

        Ok(())
    }

    /// Create consumer groups for subscription patterns
    async fn create_consumer_groups(&self) -> Result<()> {
        let mut conn = self.redis_client.get_connection()?;

        for pattern in &self.config.subscription_patterns {
            // Note: In a real implementation, you'd need to discover actual stream names
            // matching the pattern and create consumer groups for each
            let group_result: RedisResult<String> = conn.xgroup_create_mkstream(
                pattern,
                &self.config.consumer_group,
                "$",
            );

            match group_result {
                Ok(_) => info!("Created consumer group {} for stream {}", self.config.consumer_group, pattern),
                Err(e) => {
                    // Group might already exist
                    debug!("Consumer group creation result for {}: {}", pattern, e);
                }
            }
        }

        Ok(())
    }

    /// Process messages from Redis Streams
    async fn process_messages(
        config: &StreamHandlerConfig,
        redis_client: &Client,
        handler: &Arc<H>,
        circuit_breaker: &Arc<CircuitBreaker>,
        metrics_exporter: &Arc<dyn MetricsExporter>,
        trace_exporter: &Arc<dyn TraceExporter>,
        semaphore: &Arc<Semaphore>,
    ) -> Result<()> {
        let mut conn = redis_client.get_connection()?;

        // Read from streams using consumer group
        let opts = StreamReadOptions::default()
            .group(&config.consumer_group, &config.consumer_name)
            .count(10)
            .block(1000);

        // Note: In a real implementation, you'd need to resolve subscription patterns
        // to actual stream names. This is a simplified version.
        let streams: Vec<String> = config.subscription_patterns.clone();
        
        if streams.is_empty() {
            return Ok(());
        }

        let stream_ids: Vec<(&str, &str)> = streams
            .iter()
            .map(|s| (s.as_str(), ">"))
            .collect();

        let results: RedisResult<Vec<HashMap<String, Vec<HashMap<String, Vec<(String, HashMap<String, String>)>>>>>> = 
            conn.xread_options(&stream_ids, &opts);

        match results {
            Ok(stream_data) => {
                for stream_map in stream_data {
                    for (stream_name, messages) in stream_map {
                        for message in messages {
                            for (message_id, fields) in message {
                                // Acquire semaphore permit for concurrency control
                                let permit = semaphore.clone().acquire_owned().await?;

                                let handler = handler.clone();
                                let message_id = message_id.clone();
                                let fields = fields.clone();
                                let stream_name = stream_name.clone();
                                let metrics_exporter = metrics_exporter.clone();
                                let trace_exporter = trace_exporter.clone();
                                let config = config.clone();

                                tokio::spawn(async move {
                                    let _permit = permit; // Hold permit for the duration

                                    if let Err(e) = Self::handle_single_message(
                                        &config,
                                        &handler,
                                        &message_id,
                                        &fields,
                                        &stream_name,
                                        &metrics_exporter,
                                        &trace_exporter,
                                    ).await {
                                        error!("Failed to handle message {}: {}", message_id, e);
                                    }
                                });
                            }
                        }
                    }
                }
            }
            Err(e) => {
                debug!("No messages available or error reading stream: {}", e);
            }
        }

        Ok(())
    }

    /// Handle a single message
    async fn handle_single_message(
        config: &StreamHandlerConfig,
        handler: &Arc<H>,
        message_id: &str,
        fields: &HashMap<String, String>,
        stream_name: &str,
        metrics_exporter: &Arc<dyn MetricsExporter>,
        trace_exporter: &Arc<dyn TraceExporter>,
    ) -> Result<()> {
        let span_id = trace_exporter
            .start_span(&format!("handle_message_{}", message_id), None)
            .await;

        let start_time = Instant::now();

        // Extract message data
        let data = fields.get("data").ok_or_else(|| anyhow!("Missing data field"))?;
        let correlation_id = fields.get("correlation_id")
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(Uuid::new_v4);

        // Deserialize event
        let event: Event<H::PayloadType> = serde_json::from_str(data)?;

        // Validate message
        handler.validate_message(&event).await?;

        // Process message
        let result = handler.handle_message(event.clone()).await;

        // Record metrics
        let mut tags = HashMap::new();
        tags.insert("stream".to_string(), stream_name.to_string());
        tags.insert("domain".to_string(), event.domain.clone());
        tags.insert("success".to_string(), result.is_ok().to_string());

        metrics_exporter
            .increment_counter(
                &format!("{}_messages_processed_total", config.metrics_prefix),
                1.0,
                tags.clone(),
            )
            .await;

        let latency_ms = start_time.elapsed().as_millis() as f64;
        metrics_exporter
            .record_histogram(
                &format!("{}_processing_latency_ms", config.metrics_prefix),
                latency_ms,
                tags,
            )
            .await;

        // Handle errors
        if let Err(e) = result {
            handler.handle_error(event, e).await?;
        }

        trace_exporter
            .add_span_attribute(&span_id, "message_id", message_id)
            .await;
        trace_exporter
            .add_span_attribute(&span_id, "correlation_id", &correlation_id.to_string())
            .await;
        trace_exporter.end_span(&span_id).await;

        Ok(())
    }
}

/// Example implementation of a message handler
pub struct ExampleMessageHandler {
    name: String,
}

impl ExampleMessageHandler {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExamplePayload {
    pub action: String,
    pub data: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

#[async_trait]
impl MessageHandler for ExampleMessageHandler {
    type PayloadType = ExamplePayload;

    async fn handle_message(&self, event: Event<Self::PayloadType>) -> Result<()> {
        info!(
            "Handler {} processing message {} with action: {}",
            self.name, event.id, event.payload.action
        );

        // TODO: Add your message processing logic here
        // Example: Transform data, make decisions, trigger actions, etc.

        Ok(())
    }

    fn subscription_patterns(&self) -> Vec<StreamPattern> {
        vec![
            StreamPattern::new("data", "trading", "*", "raw"),
            StreamPattern::new("features", "trading", "*", "*"),
        ]
    }

    fn publication_patterns(&self) -> Vec<StreamPattern> {
        vec![
            StreamPattern::new("data", "trading", "*", "processed"),
            StreamPattern::new("decisions", "trading", "example", "*"),
        ]
    }

    async fn validate_message(&self, event: &Event<Self::PayloadType>) -> Result<()> {
        // Example validation
        if event.payload.action.is_empty() {
            return Err(anyhow!("Action cannot be empty"));
        }

        // Validate domain isolation
        if event.domain != "trading" {
            return Err(anyhow!("Handler {} only processes trading domain messages", self.name));
        }

        Ok(())
    }

    async fn handle_error(&self, event: Event<Self::PayloadType>, error: anyhow::Error) -> Result<()> {
        error!(
            "Handler {} failed to process message {}: {}",
            self.name, event.id, error
        );

        // TODO: Implement error handling strategy
        // Example: Send to dead letter queue, retry with backoff, alert monitoring

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_pattern() {
        let pattern = StreamPattern::new("data", "trading", "alpaca", "raw");
        assert_eq!(pattern.stream_name(), "data.trading.alpaca.raw");

        let wildcard_pattern = StreamPattern::new("data", "*", "*", "*");
        assert!(wildcard_pattern.matches("data.trading.alpaca.raw"));
        assert!(wildcard_pattern.matches("data.system-ops.logs.processed"));
        assert!(!wildcard_pattern.matches("features.trading.rsi.15m"));

        let specific_pattern = StreamPattern::new("data", "trading", "alpaca", "raw");
        assert!(specific_pattern.matches("data.trading.alpaca.raw"));
        assert!(!specific_pattern.matches("data.trading.binance.raw"));
    }

    #[test]
    fn test_config_default() {
        let config = StreamHandlerConfig::default();
        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.consumer_group, "default-group");
        assert_eq!(config.max_concurrent_messages, 100);
    }

    #[tokio::test]
    async fn test_example_handler() {
        let handler = ExampleMessageHandler::new("test-handler".to_string());
        
        let event = Event::new(
            "trading".to_string(),
            "test-source".to_string(),
            ExamplePayload {
                action: "BUY".to_string(),
                data: serde_json::json!({"symbol": "AAPL", "quantity": 100}),
                timestamp: Utc::now(),
            },
        );

        // Test validation
        assert!(handler.validate_message(&event).await.is_ok());

        // Test message handling
        assert!(handler.handle_message(event).await.is_ok());

        // Test subscription patterns
        let patterns = handler.subscription_patterns();
        assert_eq!(patterns.len(), 2);
        assert_eq!(patterns[0].stream_name(), "data.trading.*.raw");
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let circuit_breaker = CircuitBreaker::new(2, Duration::from_millis(100));

        // Initially closed
        assert!(circuit_breaker.can_execute().await);

        // Record failures
        circuit_breaker.record_failure().await;
        assert!(circuit_breaker.can_execute().await);

        circuit_breaker.record_failure().await;
        assert!(!circuit_breaker.can_execute().await); // Should be open now

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(circuit_breaker.can_execute().await); // Should be half-open

        // Record success to close
        circuit_breaker.record_success().await;
        assert!(circuit_breaker.can_execute().await);
    }
}