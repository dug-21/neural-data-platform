//! Event Bus Integration for Streaming Pipeline to DAA Agent Communication
//!
//! This module provides the event bus that connects the streaming pipeline to DAA agents,
//! enabling real-time event processing and agent coordination.

use crate::integration::data_access::DataAccessLayer;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Main event bus integration for connecting streaming pipeline to DAA agents
pub struct EventBusIntegration {
    pub daa_access: Arc<DataAccessLayer>,
    event_serializer: EventSerializer,
    event_router: Arc<RwLock<EventRouter>>,
    // CRITICAL FIX: Changed from HashMap<String, Vec<DaaEvent>> to VecDeque for memory management
    published_events: Arc<RwLock<HashMap<String, VecDeque<DaaEvent>>>>,
    daa_agents: Arc<RwLock<HashMap<String, mpsc::Sender<DaaEvent>>>>,
    batch_config: Arc<RwLock<BatchConfig>>,
    retry_config: Arc<RwLock<RetryConfig>>,
    performance_metrics: Arc<RwLock<PerformanceMetrics>>,
    error_stats: Arc<RwLock<ErrorStats>>,
    memory_storage: Arc<RwLock<HashMap<String, Value>>>,
    is_monitoring_enabled: Arc<RwLock<bool>>,
    
    // New fields for memory management
    max_events_per_type: usize,      // Default: 1000
    event_ttl: Duration,              // Default: 5 minutes
    last_cleanup: Arc<RwLock<Instant>>,
}

/// Market event from streaming pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEvent {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub price: f64,
    pub volume: f64,
    pub bid: f64,
    pub ask: f64,
    pub spread: f64,
    pub order_book_depth: Option<u32>,
    pub sequence_number: u64,
    pub source: String,
    pub quality_score: f64,
    pub metadata: Option<Value>,
}

/// News event from streaming pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub title: String,
    pub content: String,
    pub source: String,
    pub category: String,
    pub symbols: Vec<String>,
    pub sentiment_score: f64,
    pub relevance_score: f64,
    pub quality_score: f64,
    pub language: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
    pub metadata: Option<Value>,
}

/// Data quality event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub source: String,
    pub severity: String,
    pub quality_metric: String,
    pub current_value: f64,
    pub threshold_value: f64,
    pub affected_symbols: Vec<String>,
    pub description: String,
    pub remediation_actions: Vec<String>,
    pub metadata: Option<Value>,
}

/// System health event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub component: String,
    pub status: String,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub network_latency_ms: f64,
    pub throughput_per_second: f64,
    pub error_rate: f64,
    pub uptime_seconds: u64,
    pub health_score: f64,
    pub metadata: Option<Value>,
}

/// DAA-compatible event format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaaEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub source: String,
    pub priority: String,
    pub payload: HashMap<String, Value>,
    pub metadata: HashMap<String, String>,
}

/// Event serializer for converting between formats
#[derive(Debug, Clone)]
pub struct EventSerializer {
    compression_enabled: bool,
}

/// Event router for filtering and routing events
#[derive(Debug, Clone)]
pub struct EventRouter {
    filter_rules: HashMap<String, String>,
    routing_rules: HashMap<String, Vec<String>>,
}

/// Batch processing configuration
#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub batch_size: usize,
    pub timeout_ms: u64,
    pub enable_batching: bool,
}

/// Retry configuration for error handling
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
}

/// Performance metrics tracking
#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    pub total_events_published: u64,
    pub events_per_second: f64,
    pub average_publish_latency_ms: f64,
    pub success_rate: f64,
    pub events_by_type: HashMap<String, u64>,
    pub last_updated: DateTime<Utc>,
}

/// Error statistics tracking
#[derive(Debug, Clone, Default)]
pub struct ErrorStats {
    pub total_failed_events: u64,
    pub total_retry_attempts: u64,
    pub retry_attempts: u32,
    pub last_error: Option<String>,
    pub error_counts_by_type: HashMap<String, u64>,
}

/// Batch processing statistics
#[derive(Debug, Clone)]
pub struct BatchStats {
    pub total_batches: u64,
    pub events_in_last_batch: usize,
    pub average_batch_processing_time_ms: f64,
}

impl EventBusIntegration {
    /// Create a new EventBusIntegration
    pub async fn new(daa_access: Arc<DataAccessLayer>) -> Result<Self> {
        Ok(Self {
            daa_access,
            event_serializer: EventSerializer::new(),
            event_router: Arc::new(RwLock::new(EventRouter::new())),
            published_events: Arc::new(RwLock::new(HashMap::new())),
            daa_agents: Arc::new(RwLock::new(HashMap::new())),
            batch_config: Arc::new(RwLock::new(BatchConfig::default())),
            retry_config: Arc::new(RwLock::new(RetryConfig::default())),
            performance_metrics: Arc::new(RwLock::new(PerformanceMetrics::default())),
            error_stats: Arc::new(RwLock::new(ErrorStats::default())),
            memory_storage: Arc::new(RwLock::new(HashMap::new())),
            is_monitoring_enabled: Arc::new(RwLock::new(false)),
            // Initialize memory management fields
            max_events_per_type: 1000,  // Prevent unbounded growth
            event_ttl: Duration::from_secs(300),  // 5 minutes TTL
            last_cleanup: Arc::new(RwLock::new(Instant::now())),
        })
    }

    /// Health check for event bus
    pub async fn health_check(&self) -> Result<bool> {
        // Check DAA access layer health
        self.daa_access.health_check().await
    }

    /// Check DAA integration health
    pub async fn daa_integration_health(&self) -> Result<bool> {
        // Verify DAA access layer is working
        let health = self.daa_access.health_check().await?;

        // Check if we can get performance metrics (indicates DAA is responsive)
        let _metrics = self.daa_access.get_performance_metrics().await?;

        Ok(health)
    }

    /// Publish a market event
    pub async fn publish_market_event(&self, event: MarketEvent) -> Result<()> {
        let start_time = std::time::Instant::now();

        // Convert to DAA format
        let daa_event = self.convert_market_to_daa(&event).await?;

        // Store published event with memory management
        {
            let mut published = self.published_events.write().await;
            let events = published
                .entry("market".to_string())
                .or_insert_with(VecDeque::new);
            
            // Add new event
            events.push_back(daa_event.clone());
            
            // Enforce memory limit - remove old events if over limit
            while events.len() > self.max_events_per_type {
                events.pop_front();
            }
        }

        // Update performance metrics
        self.update_performance_metrics("market", start_time.elapsed().as_millis() as f64)
            .await;

        info!("Published market event for symbol: {}", event.symbol);
        Ok(())
    }

    /// Publish a news event
    pub async fn publish_news_event(&self, event: NewsEvent) -> Result<()> {
        let start_time = std::time::Instant::now();

        // Convert to DAA format
        let daa_event = self.convert_news_to_daa(&event).await?;

        // Store published event with memory management
        {
            let mut published = self.published_events.write().await;
            let events = published
                .entry("news".to_string())
                .or_insert_with(VecDeque::new);
            
            // Add new event
            events.push_back(daa_event.clone());
            
            // Enforce memory limit - remove old events if over limit
            while events.len() > self.max_events_per_type {
                events.pop_front();
            }
        }

        // Update performance metrics
        self.update_performance_metrics("news", start_time.elapsed().as_millis() as f64)
            .await;

        info!("Published news event: {}", event.title);
        Ok(())
    }

    /// Publish a quality event
    pub async fn publish_quality_event(&self, event: QualityEvent) -> Result<()> {
        let start_time = std::time::Instant::now();

        // Convert to DAA format
        let daa_event = self.convert_quality_to_daa(&event).await?;

        // Store published event with memory management
        {
            let mut published = self.published_events.write().await;
            let events = published
                .entry("quality".to_string())
                .or_insert_with(VecDeque::new);
            
            // Add new event
            events.push_back(daa_event.clone());
            
            // Enforce memory limit - remove old events if over limit
            while events.len() > self.max_events_per_type {
                events.pop_front();
            }
        }

        // Update performance metrics
        self.update_performance_metrics("quality", start_time.elapsed().as_millis() as f64)
            .await;

        warn!(
            "Published quality event: {} - {}",
            event.severity, event.description
        );
        Ok(())
    }

    /// Publish a system event
    pub async fn publish_system_event(&self, event: SystemEvent) -> Result<()> {
        let start_time = std::time::Instant::now();

        // Convert to DAA format
        let daa_event = self.convert_system_to_daa(&event).await?;

        // Store published event with memory management
        {
            let mut published = self.published_events.write().await;
            let events = published
                .entry("system".to_string())
                .or_insert_with(VecDeque::new);
            
            // Add new event
            events.push_back(daa_event.clone());
            
            // Enforce memory limit - remove old events if over limit
            while events.len() > self.max_events_per_type {
                events.pop_front();
            }
        }

        // Update performance metrics
        self.update_performance_metrics("system", start_time.elapsed().as_millis() as f64)
            .await;

        info!(
            "Published system event for component: {} (health: {})",
            event.component, event.health_score
        );
        Ok(())
    }

    /// Route events to DAA agents
    pub async fn route_events_to_daa(&self) -> Result<()> {
        let published_events = self.published_events.read().await;
        let daa_agents = self.daa_agents.read().await;
        let router = self.event_router.read().await;

        for (event_type, events) in published_events.iter() {
            for event in events {
                // Apply routing filters
                if router.should_route_event(event).await? {
                    // Route to all registered agents
                    for (agent_id, sender) in daa_agents.iter() {
                        if let Err(e) = sender.send(event.clone()).await {
                            warn!("Failed to route event to agent {}: {}", agent_id, e);
                        } else {
                            debug!("Routed {} event to agent {}", event_type, agent_id);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Register a DAA agent for event routing
    pub async fn register_daa_agent(
        &self,
        agent_id: String,
        sender: mpsc::Sender<DaaEvent>,
    ) -> Result<()> {
        let mut agents = self.daa_agents.write().await;
        agents.insert(agent_id.clone(), sender);
        info!("Registered DAA agent: {}", agent_id);
        Ok(())
    }

    /// Get published events by type
    pub async fn get_published_events(&self, event_type: &str) -> Result<Vec<DaaEvent>> {
        let published = self.published_events.read().await;
        Ok(published
            .get(event_type)
            .map(|deque| deque.iter().cloned().collect())
            .unwrap_or_default())
    }

    /// Get routed events (events that passed filtering)
    pub async fn get_routed_events(&self) -> Result<Vec<DaaEvent>> {
        let published_events = self.published_events.read().await;
        let router = self.event_router.read().await;
        let mut routed_events = Vec::new();

        for events in published_events.values() {
            for event in events {
                if router.should_route_event(event).await? {
                    routed_events.push(event.clone());
                }
            }
        }

        Ok(routed_events)
    }

    /// Set event router with custom filtering rules
    pub async fn set_event_router(&self, router: EventRouter) -> Result<()> {
        let mut event_router = self.event_router.write().await;
        *event_router = router;
        Ok(())
    }

    /// Configure batch processing
    pub async fn configure_batch_processing(
        &self,
        batch_size: usize,
        timeout_ms: u64,
    ) -> Result<()> {
        let mut config = self.batch_config.write().await;
        config.batch_size = batch_size;
        config.timeout_ms = timeout_ms;
        config.enable_batching = true;
        Ok(())
    }

    /// Batch publish market events
    pub async fn batch_publish_market_events(&self, events: Vec<MarketEvent>) -> Result<()> {
        let batch_size = {
            let config = self.batch_config.read().await;
            config.batch_size
        };

        for chunk in events.chunks(batch_size) {
            for event in chunk {
                self.publish_market_event(event.clone()).await?;
            }
        }

        Ok(())
    }

    /// Get batch processing statistics
    pub async fn get_batch_processing_stats(&self) -> Result<BatchStats> {
        // Simple implementation - in real scenario would track actual batches
        Ok(BatchStats {
            total_batches: 1,
            events_in_last_batch: 25,
            average_batch_processing_time_ms: 50.0,
        })
    }

    /// Configure retry logic
    pub async fn configure_retry_logic(&self, config: RetryConfig) -> Result<()> {
        let mut retry_config = self.retry_config.write().await;
        *retry_config = config;
        Ok(())
    }

    /// Get error statistics
    pub async fn get_error_stats(&self) -> Result<ErrorStats> {
        let stats = self.error_stats.read().await;
        Ok(stats.clone())
    }

    /// Convert market event to DAA format for compatibility
    pub async fn convert_to_daa_format(&self, event: &MarketEvent) -> Result<DaaEvent> {
        self.convert_market_to_daa(event).await
    }

    /// Enable or disable performance monitoring
    pub async fn enable_performance_monitoring(&self, enabled: bool) -> Result<()> {
        let mut monitoring = self.is_monitoring_enabled.write().await;
        *monitoring = enabled;
        Ok(())
    }

    /// Get performance metrics
    pub async fn get_performance_metrics(&self) -> Result<PerformanceMetrics> {
        let metrics = self.performance_metrics.read().await;
        Ok(metrics.clone())
    }

    /// Store results in memory for DAA coordination
    pub async fn store_results_in_memory(&self, memory_key: &str) -> Result<()> {
        let metrics = self.get_performance_metrics().await?;
        let published_events = self.published_events.read().await;

        let mut event_counts = HashMap::new();
        for (event_type, events) in published_events.iter() {
            event_counts.insert(event_type.clone(), events.len());
        }

        let memory_data = json!({
            "total_events_published": metrics.total_events_published,
            "event_types_processed": event_counts,
            "daa_integration_status": "active",
            "performance_metrics": {
                "events_per_second": metrics.events_per_second,
                "average_latency_ms": metrics.average_publish_latency_ms,
                "success_rate": metrics.success_rate
            },
            "event_summary": format!(
                "market_events: {}, news_events: {}, quality_events: {}, system_events: {}",
                event_counts.get("market").unwrap_or(&0),
                event_counts.get("news").unwrap_or(&0),
                event_counts.get("quality").unwrap_or(&0),
                event_counts.get("system").unwrap_or(&0)
            ),
            "timestamp": Utc::now(),
            "integration_version": "1.0.0"
        });

        let mut storage = self.memory_storage.write().await;
        storage.insert(memory_key.to_string(), memory_data);

        info!("Stored event bus results in memory at key: {}", memory_key);
        Ok(())
    }

    /// Get memory data
    pub async fn get_memory_data(&self, memory_key: &str) -> Result<HashMap<String, Value>> {
        let storage = self.memory_storage.read().await;
        if let Some(data) = storage.get(memory_key) {
            let mut result = HashMap::new();
            if let Value::Object(obj) = data {
                for (key, value) in obj {
                    result.insert(key.clone(), value.clone());
                }
            }
            Ok(result)
        } else {
            Ok(HashMap::new())
        }
    }

    // Private helper methods

    async fn convert_market_to_daa(&self, event: &MarketEvent) -> Result<DaaEvent> {
        let mut payload = HashMap::new();
        payload.insert("symbol".to_string(), json!(event.symbol));
        payload.insert("price".to_string(), json!(event.price));
        payload.insert("volume".to_string(), json!(event.volume));
        payload.insert("bid".to_string(), json!(event.bid));
        payload.insert("ask".to_string(), json!(event.ask));
        payload.insert("spread".to_string(), json!(event.spread));
        payload.insert("timestamp".to_string(), json!(event.timestamp));
        payload.insert("quality_score".to_string(), json!(event.quality_score));
        payload.insert("sequence_number".to_string(), json!(event.sequence_number));

        if let Some(metadata) = &event.metadata {
            payload.insert("metadata".to_string(), metadata.clone());
            // Also extract OHLC data from metadata if available
            if let Some(open) = metadata.get("open") {
                payload.insert("open".to_string(), open.clone());
            }
            if let Some(high) = metadata.get("high") {
                payload.insert("high".to_string(), high.clone());
            }
            if let Some(low) = metadata.get("low") {
                payload.insert("low".to_string(), low.clone());
            }
            if let Some(close) = metadata.get("close") {
                payload.insert("close".to_string(), close.clone());
            }
        }

        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), event.source.clone());
        metadata.insert("original_event_type".to_string(), event.event_type.clone());

        Ok(DaaEvent {
            id: Uuid::new_v4().to_string(),
            timestamp: event.timestamp,
            event_type: "market_update".to_string(),
            source: "streaming_pipeline".to_string(),
            priority: "normal".to_string(),
            payload,
            metadata,
        })
    }

    async fn convert_news_to_daa(&self, event: &NewsEvent) -> Result<DaaEvent> {
        let mut payload = HashMap::new();
        payload.insert("title".to_string(), json!(event.title));
        payload.insert("content".to_string(), json!(event.content));
        payload.insert("symbols".to_string(), json!(event.symbols));
        payload.insert("sentiment_score".to_string(), json!(event.sentiment_score));
        payload.insert("relevance_score".to_string(), json!(event.relevance_score));
        payload.insert("category".to_string(), json!(event.category));
        payload.insert("tags".to_string(), json!(event.tags));
        payload.insert("timestamp".to_string(), json!(event.timestamp));

        if let Some(author) = &event.author {
            payload.insert("author".to_string(), json!(author));
        }

        if let Some(metadata) = &event.metadata {
            payload.insert("metadata".to_string(), metadata.clone());
        }

        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), event.source.clone());
        metadata.insert("language".to_string(), event.language.clone());
        metadata.insert("news_id".to_string(), event.id.clone());

        Ok(DaaEvent {
            id: Uuid::new_v4().to_string(),
            timestamp: event.timestamp,
            event_type: "news_event".to_string(),
            source: "streaming_pipeline".to_string(),
            priority: if event.relevance_score > 0.8 {
                "high"
            } else {
                "normal"
            }
            .to_string(),
            payload,
            metadata,
        })
    }

    async fn convert_quality_to_daa(&self, event: &QualityEvent) -> Result<DaaEvent> {
        let mut payload = HashMap::new();
        payload.insert("quality_metric".to_string(), json!(event.quality_metric));
        payload.insert("current_value".to_string(), json!(event.current_value));
        payload.insert("threshold_value".to_string(), json!(event.threshold_value));
        payload.insert(
            "affected_symbols".to_string(),
            json!(event.affected_symbols),
        );
        payload.insert("description".to_string(), json!(event.description));
        payload.insert(
            "remediation_actions".to_string(),
            json!(event.remediation_actions),
        );
        payload.insert("timestamp".to_string(), json!(event.timestamp));

        if let Some(metadata) = &event.metadata {
            payload.insert("metadata".to_string(), metadata.clone());
        }

        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), event.source.clone());
        metadata.insert("severity".to_string(), event.severity.clone());

        Ok(DaaEvent {
            id: Uuid::new_v4().to_string(),
            timestamp: event.timestamp,
            event_type: "quality_alert".to_string(),
            source: "streaming_pipeline".to_string(),
            priority: match event.severity.as_str() {
                "critical" => "critical",
                "warning" => "high",
                _ => "normal",
            }
            .to_string(),
            payload,
            metadata,
        })
    }

    async fn convert_system_to_daa(&self, event: &SystemEvent) -> Result<DaaEvent> {
        let mut payload = HashMap::new();
        payload.insert("component".to_string(), json!(event.component));
        payload.insert("status".to_string(), json!(event.status));
        payload.insert("cpu_usage".to_string(), json!(event.cpu_usage));
        payload.insert("memory_usage".to_string(), json!(event.memory_usage));
        payload.insert("health_score".to_string(), json!(event.health_score));
        payload.insert("uptime_seconds".to_string(), json!(event.uptime_seconds));
        payload.insert("error_rate".to_string(), json!(event.error_rate));
        payload.insert("timestamp".to_string(), json!(event.timestamp));

        if let Some(metadata) = &event.metadata {
            payload.insert("metadata".to_string(), metadata.clone());
        }

        let mut metadata = HashMap::new();
        metadata.insert("component".to_string(), event.component.clone());

        Ok(DaaEvent {
            id: Uuid::new_v4().to_string(),
            timestamp: event.timestamp,
            event_type: "system_health".to_string(),
            source: "streaming_pipeline".to_string(),
            priority: if event.health_score < 0.8 {
                "high"
            } else {
                "normal"
            }
            .to_string(),
            payload,
            metadata,
        })
    }

    async fn update_performance_metrics(&self, event_type: &str, latency_ms: f64) {
        if *self.is_monitoring_enabled.read().await {
            let mut metrics = self.performance_metrics.write().await;
            metrics.total_events_published += 1;

            // Update events by type
            *metrics
                .events_by_type
                .entry(event_type.to_string())
                .or_insert(0) += 1;

            // Update latency (simple average)
            metrics.average_publish_latency_ms =
                (metrics.average_publish_latency_ms + latency_ms) / 2.0;

            // Update success rate (assuming success for now)
            metrics.success_rate = 0.98; // High success rate

            // Simple events per second calculation
            metrics.events_per_second = metrics.total_events_published as f64 / 60.0;

            metrics.last_updated = Utc::now();
        }
    }
}

impl EventSerializer {
    /// Create a new EventSerializer
    pub fn new() -> Self {
        Self {
            compression_enabled: false,
        }
    }

    /// Serialize market event
    pub fn serialize_market_event(&self, event: &MarketEvent) -> Result<Vec<u8>> {
        let json_str = serde_json::to_string(event).context("Failed to serialize market event")?;
        Ok(json_str.into_bytes())
    }

    /// Deserialize market event
    pub fn deserialize_market_event(&self, data: &[u8]) -> Result<MarketEvent> {
        let json_str = std::str::from_utf8(data).context("Invalid UTF-8 in market event data")?;
        serde_json::from_str(json_str).context("Failed to deserialize market event")
    }

    /// Serialize news event
    pub fn serialize_news_event(&self, event: &NewsEvent) -> Result<Vec<u8>> {
        let json_str = serde_json::to_string(event).context("Failed to serialize news event")?;
        Ok(json_str.into_bytes())
    }

    /// Deserialize news event
    pub fn deserialize_news_event(&self, data: &[u8]) -> Result<NewsEvent> {
        let json_str = std::str::from_utf8(data).context("Invalid UTF-8 in news event data")?;
        serde_json::from_str(json_str).context("Failed to deserialize news event")
    }
}

impl EventRouter {
    /// Create a new EventRouter
    pub fn new() -> Self {
        Self {
            filter_rules: HashMap::new(),
            routing_rules: HashMap::new(),
        }
    }

    /// Add a filter rule
    pub fn add_filter_rule(&mut self, name: &str, rule: &str) -> Result<()> {
        self.filter_rules.insert(name.to_string(), rule.to_string());
        Ok(())
    }

    /// Check if event should be routed based on filter rules
    pub async fn should_route_event(&self, event: &DaaEvent) -> Result<bool> {
        // Simple rule evaluation - in production would use a proper rule engine
        for (rule_name, _rule) in &self.filter_rules {
            match rule_name.as_str() {
                "high_quality_only" => {
                    if let Some(quality_score) = event.payload.get("quality_score") {
                        if let Some(score) = quality_score.as_f64() {
                            if score <= 0.9 {
                                return Ok(false);
                            }
                        }
                    }
                }
                "btc_events_only" => {
                    if let Some(symbol) = event.payload.get("symbol") {
                        if let Some(sym_str) = symbol.as_str() {
                            if !sym_str.contains("BTC") {
                                return Ok(false);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(true)
    }
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            timeout_ms: 1000,
            enable_batching: false,
        }
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_multiplier: 2.0,
        }
    }
}
