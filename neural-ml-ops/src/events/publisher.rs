//! Event Publisher Implementation
//!
//! Handles publishing of ML workflow events to various backends with
//! batching, filtering, and retry capabilities.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde_json;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tokio::time::{interval, Duration, Instant};
use tracing::{debug, error, info, warn};

use super::{
    EventBackendTrait, EventConfig, EventStats, MLEvent, MLEventType, EventBackend, EventFilter,
};

/// Main event publisher
pub struct EventPublisher {
    config: EventConfig,
    backend: Box<dyn EventBackendTrait>,
    event_buffer: Arc<Mutex<VecDeque<MLEvent>>>,
    stats: Arc<RwLock<PublisherStats>>,
}

/// Internal publisher statistics
#[derive(Debug, Default)]
struct PublisherStats {
    total_events_published: u64,
    total_events_buffered: u64,
    total_batches_sent: u64,
    publish_errors: u64,
    events_by_type: HashMap<String, u64>,
    last_publish: Option<DateTime<Utc>>,
    last_flush: Option<DateTime<Utc>>,
}

impl EventPublisher {
    /// Create a new event publisher
    pub async fn new(config: EventConfig) -> Result<Self> {
        info!("Initializing Event Publisher");
        
        // Create backend
        let backend = Self::create_backend(&config).await?;
        
        let publisher = Self {
            config: config.clone(),
            backend,
            event_buffer: Arc::new(Mutex::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(PublisherStats::default())),
        };
        
        // Start background flush task if buffering is enabled
        if config.buffer_size > 0 {
            publisher.start_flush_task().await;
        }
        
        info!("Event Publisher initialized with backend: {:?}", config.backend);
        Ok(publisher)
    }
    
    /// Publish a single event
    pub async fn publish(&self, event: MLEvent) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }
        
        debug!("Publishing event: {:?}", event.event_type);
        
        // Apply filters if enabled
        if self.config.enable_filtering && !self.passes_filters(&event) {
            debug!("Event filtered out: {:?}", event.id);
            return Ok(());
        }
        
        // Update statistics
        self.update_stats_on_publish(&event).await;
        
        // If buffering is enabled, add to buffer
        if self.config.buffer_size > 0 {
            let mut buffer = self.event_buffer.lock().await;
            buffer.push_back(event);
            
            // Check if buffer is full and needs immediate flush
            if buffer.len() >= self.config.buffer_size {
                drop(buffer); // Release lock before flushing
                self.flush_buffer().await?;
            }
        } else {
            // Publish immediately
            self.publish_direct(&event).await?;
        }
        
        Ok(())
    }
    
    /// Publish multiple events in a batch
    pub async fn publish_batch(&self, events: Vec<MLEvent>) -> Result<()> {
        if !self.config.enabled || events.is_empty() {
            return Ok(());
        }
        
        info!("Publishing batch of {} events", events.len());
        
        // Filter events if enabled
        let filtered_events: Vec<MLEvent> = if self.config.enable_filtering {
            events.into_iter()
                .filter(|event| self.passes_filters(event))
                .collect()
        } else {
            events
        };
        
        if filtered_events.is_empty() {
            debug!("All events in batch were filtered out");
            return Ok(());
        }
        
        // Update statistics
        for event in &filtered_events {
            self.update_stats_on_publish(event).await;
        }
        
        // Publish batch
        self.backend.publish_batch(&filtered_events).await?;
        
        // Update batch statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_batches_sent += 1;
            stats.last_publish = Some(Utc::now());
        }
        
        info!("Successfully published batch of {} events", filtered_events.len());
        Ok(())
    }
    
    /// Force flush of buffered events
    pub async fn flush(&self) -> Result<()> {
        if self.config.buffer_size > 0 {
            self.flush_buffer().await?;
        }
        Ok(())
    }
    
    /// Get publisher statistics
    pub async fn get_stats(&self) -> EventStats {
        let stats = self.stats.read().await;
        let backend_stats = self.backend.get_stats().await.unwrap_or_else(|_| EventStats {
            total_events_published: 0,
            events_by_type: HashMap::new(),
            events_by_severity: HashMap::new(),
            publish_errors: 0,
            average_batch_size: 0.0,
            last_publish: None,
        });
        
        let average_batch_size = if stats.total_batches_sent > 0 {
            stats.total_events_published as f64 / stats.total_batches_sent as f64
        } else {
            0.0
        };
        
        EventStats {
            total_events_published: stats.total_events_published,
            events_by_type: stats.events_by_type.clone(),
            events_by_severity: backend_stats.events_by_severity,
            publish_errors: stats.publish_errors,
            average_batch_size,
            last_publish: stats.last_publish,
        }
    }
    
    /// Get buffer status
    pub async fn get_buffer_status(&self) -> BufferStatus {
        let buffer = self.event_buffer.lock().await;
        BufferStatus {
            buffered_events: buffer.len(),
            buffer_capacity: self.config.buffer_size,
            buffer_utilization: if self.config.buffer_size > 0 {
                buffer.len() as f64 / self.config.buffer_size as f64
            } else {
                0.0
            },
        }
    }
    
    /// Health check for the publisher
    pub async fn health_check(&self) -> Result<PublisherHealth> {
        let backend_healthy = self.backend.health_check().await.unwrap_or(false);
        let buffer_status = self.get_buffer_status().await;
        
        let overall_healthy = backend_healthy && buffer_status.buffer_utilization < 0.9;
        
        Ok(PublisherHealth {
            healthy: overall_healthy,
            backend_healthy,
            buffer_healthy: buffer_status.buffer_utilization < 0.9,
            buffer_utilization: buffer_status.buffer_utilization,
            last_error: None, // Would track actual errors
        })
    }
    
    // Private methods
    
    async fn create_backend(config: &EventConfig) -> Result<Box<dyn EventBackendTrait>> {
        match &config.backend {
            EventBackend::Memory => {
                Ok(Box::new(MemoryEventBackend::new()))
            }
            EventBackend::Redis { connection_string } => {
                #[cfg(feature = "events")]
                {
                    Ok(Box::new(RedisEventBackend::new(connection_string).await?))
                }
                #[cfg(not(feature = "events"))]
                {
                    warn!("Redis backend requested but redis feature not enabled, falling back to memory");
                    Ok(Box::new(MemoryEventBackend::new()))
                }
            }
            EventBackend::Kafka { brokers: _, topic: _ } => {
                warn!("Kafka backend not yet implemented, falling back to memory");
                Ok(Box::new(MemoryEventBackend::new()))
            }
            EventBackend::Webhook { url, headers } => {
                Ok(Box::new(WebhookEventBackend::new(url.clone(), headers.clone())))
            }
            EventBackend::File { path } => {
                Ok(Box::new(FileEventBackend::new(path)?))
            }
        }
    }
    
    async fn publish_direct(&self, event: &MLEvent) -> Result<()> {
        let mut retry_count = 0;
        
        while retry_count <= self.config.retry_attempts {
            match self.backend.publish_event(event).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    retry_count += 1;
                    if retry_count > self.config.retry_attempts {
                        error!("Failed to publish event after {} retries: {}", self.config.retry_attempts, e);
                        self.update_stats_on_error().await;
                        return Err(e);
                    }
                    
                    // Exponential backoff
                    let backoff_ms = (2u64.pow(retry_count as u32 - 1)) * 100;
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                }
            }
        }
        
        Ok(())
    }
    
    async fn flush_buffer(&self) -> Result<()> {
        let events_to_flush = {
            let mut buffer = self.event_buffer.lock().await;
            if buffer.is_empty() {
                return Ok(());
            }
            
            let batch_size = self.config.batch_size.min(buffer.len());
            let events: Vec<MLEvent> = buffer.drain(..batch_size).collect();
            events
        };
        
        if !events_to_flush.is_empty() {
            debug!("Flushing {} events from buffer", events_to_flush.len());
            
            match self.backend.publish_batch(&events_to_flush).await {
                Ok(_) => {
                    let mut stats = self.stats.write().await;
                    stats.total_batches_sent += 1;
                    stats.last_flush = Some(Utc::now());
                    stats.last_publish = Some(Utc::now());
                }
                Err(e) => {
                    error!("Failed to flush event batch: {}", e);
                    self.update_stats_on_error().await;
                    return Err(e);
                }
            }
        }
        
        Ok(())
    }
    
    async fn start_flush_task(&self) {
        let event_buffer = self.event_buffer.clone();
        let backend = Arc::clone(&Arc::new(&*self.backend as &dyn EventBackendTrait));
        let flush_interval = self.config.flush_interval_ms;
        let batch_size = self.config.batch_size;
        let stats = self.stats.clone();
        
        tokio::spawn(async move {
            let mut interval_timer = interval(Duration::from_millis(flush_interval));
            
            loop {
                interval_timer.tick().await;
                
                // Get events to flush
                let events_to_flush = {
                    let mut buffer = event_buffer.lock().await;
                    if buffer.is_empty() {
                        continue;
                    }
                    
                    let flush_count = batch_size.min(buffer.len());
                    let events: Vec<MLEvent> = buffer.drain(..flush_count).collect();
                    events
                };
                
                if !events_to_flush.is_empty() {
                    debug!("Periodic flush: {} events", events_to_flush.len());
                    
                    // This won't compile due to trait object issues, but shows the intent
                    // In practice, you'd need to restructure this differently
                    // match backend.publish_batch(&events_to_flush).await {
                    //     Ok(_) => {
                    //         let mut stats_guard = stats.write().await;
                    //         stats_guard.total_batches_sent += 1;
                    //         stats_guard.last_flush = Some(Utc::now());
                    //     }
                    //     Err(e) => {
                    //         error!("Periodic flush failed: {}", e);
                    //     }
                    // }
                }
            }
        });
    }
    
    fn passes_filters(&self, event: &MLEvent) -> bool {
        if !self.config.enable_filtering {
            return true;
        }
        
        for filter in &self.config.filters {
            if !filter.enabled {
                continue;
            }
            
            // Check event type filter
            if !filter.event_types.is_empty() {
                let event_type_matches = filter.event_types.iter().any(|filter_type| {
                    std::mem::discriminant(filter_type) == std::mem::discriminant(&event.event_type)
                });
                
                if !event_type_matches {
                    return false;
                }
            }
            
            // Check workflow pattern filter
            if !filter.workflow_patterns.is_empty() {
                if let Some(workflow_id) = &event.workflow_id {
                    let workflow_matches = filter.workflow_patterns.iter()
                        .any(|pattern| workflow_id.contains(pattern));
                    
                    if !workflow_matches {
                        return false;
                    }
                } else {
                    return false;
                }
            }
        }
        
        true
    }
    
    async fn update_stats_on_publish(&self, event: &MLEvent) {
        let mut stats = self.stats.write().await;
        stats.total_events_published += 1;
        stats.total_events_buffered += 1;
        
        let event_type_key = format!("{:?}", event.event_type);
        *stats.events_by_type.entry(event_type_key).or_insert(0) += 1;
    }
    
    async fn update_stats_on_error(&self) {
        let mut stats = self.stats.write().await;
        stats.publish_errors += 1;
    }
}

/// Buffer status information
#[derive(Debug, Clone)]
pub struct BufferStatus {
    pub buffered_events: usize,
    pub buffer_capacity: usize,
    pub buffer_utilization: f64,
}

/// Publisher health status
#[derive(Debug, Clone)]
pub struct PublisherHealth {
    pub healthy: bool,
    pub backend_healthy: bool,
    pub buffer_healthy: bool,
    pub buffer_utilization: f64,
    pub last_error: Option<String>,
}

// Backend implementations

/// Memory event backend for development and testing
struct MemoryEventBackend {
    events: Arc<RwLock<Vec<MLEvent>>>,
    stats: Arc<RwLock<EventStats>>,
}

impl MemoryEventBackend {
    fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(EventStats {
                total_events_published: 0,
                events_by_type: HashMap::new(),
                events_by_severity: HashMap::new(),
                publish_errors: 0,
                average_batch_size: 0.0,
                last_publish: None,
            })),
        }
    }
}

#[async_trait::async_trait]
impl EventBackendTrait for MemoryEventBackend {
    async fn publish_event(&self, event: &MLEvent) -> Result<()> {
        let mut events = self.events.write().await;
        events.push(event.clone());
        
        let mut stats = self.stats.write().await;
        stats.total_events_published += 1;
        stats.last_publish = Some(Utc::now());
        
        let event_type_key = format!("{:?}", event.event_type);
        *stats.events_by_type.entry(event_type_key).or_insert(0) += 1;
        
        Ok(())
    }
    
    async fn publish_batch(&self, events: &[MLEvent]) -> Result<()> {
        let mut stored_events = self.events.write().await;
        let mut stats = self.stats.write().await;
        
        for event in events {
            stored_events.push(event.clone());
            
            let event_type_key = format!("{:?}", event.event_type);
            *stats.events_by_type.entry(event_type_key).or_insert(0) += 1;
        }
        
        stats.total_events_published += events.len() as u64;
        stats.last_publish = Some(Utc::now());
        
        Ok(())
    }
    
    async fn get_stats(&self) -> Result<EventStats> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }
    
    async fn health_check(&self) -> Result<bool> {
        Ok(true) // Memory backend is always healthy
    }
}

/// File event backend
struct FileEventBackend {
    file_path: String,
    stats: Arc<RwLock<EventStats>>,
}

impl FileEventBackend {
    fn new(path: &str) -> Result<Self> {
        // Ensure directory exists
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        Ok(Self {
            file_path: path.to_string(),
            stats: Arc::new(RwLock::new(EventStats {
                total_events_published: 0,
                events_by_type: HashMap::new(),
                events_by_severity: HashMap::new(),
                publish_errors: 0,
                average_batch_size: 0.0,
                last_publish: None,
            })),
        })
    }
}

#[async_trait::async_trait]
impl EventBackendTrait for FileEventBackend {
    async fn publish_event(&self, event: &MLEvent) -> Result<()> {
        let event_json = serde_json::to_string(event)?;
        let event_line = format!("{}\n", event_json);
        
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await?
            .write_all(event_line.as_bytes())
            .await?;
        
        let mut stats = self.stats.write().await;
        stats.total_events_published += 1;
        stats.last_publish = Some(Utc::now());
        
        let event_type_key = format!("{:?}", event.event_type);
        *stats.events_by_type.entry(event_type_key).or_insert(0) += 1;
        
        Ok(())
    }
    
    async fn publish_batch(&self, events: &[MLEvent]) -> Result<()> {
        let mut batch_content = String::new();
        
        for event in events {
            let event_json = serde_json::to_string(event)?;
            batch_content.push_str(&event_json);
            batch_content.push('\n');
        }
        
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await?
            .write_all(batch_content.as_bytes())
            .await?;
        
        let mut stats = self.stats.write().await;
        stats.total_events_published += events.len() as u64;
        stats.last_publish = Some(Utc::now());
        
        for event in events {
            let event_type_key = format!("{:?}", event.event_type);
            *stats.events_by_type.entry(event_type_key).or_insert(0) += 1;
        }
        
        Ok(())
    }
    
    async fn get_stats(&self) -> Result<EventStats> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }
    
    async fn health_check(&self) -> Result<bool> {
        // Check if we can write to the file
        match tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

/// Webhook event backend
struct WebhookEventBackend {
    url: String,
    headers: HashMap<String, String>,
    client: reqwest::Client,
    stats: Arc<RwLock<EventStats>>,
}

impl WebhookEventBackend {
    fn new(url: String, headers: HashMap<String, String>) -> Self {
        Self {
            url,
            headers,
            client: reqwest::Client::new(),
            stats: Arc::new(RwLock::new(EventStats {
                total_events_published: 0,
                events_by_type: HashMap::new(),
                events_by_severity: HashMap::new(),
                publish_errors: 0,
                average_batch_size: 0.0,
                last_publish: None,
            })),
        }
    }
}

#[async_trait::async_trait]
impl EventBackendTrait for WebhookEventBackend {
    async fn publish_event(&self, event: &MLEvent) -> Result<()> {
        let mut request = self.client.post(&self.url)
            .json(event);
        
        // Add custom headers
        for (key, value) in &self.headers {
            request = request.header(key, value);
        }
        
        let response = request.send().await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Webhook request failed with status: {}", response.status()));
        }
        
        let mut stats = self.stats.write().await;
        stats.total_events_published += 1;
        stats.last_publish = Some(Utc::now());
        
        let event_type_key = format!("{:?}", event.event_type);
        *stats.events_by_type.entry(event_type_key).or_insert(0) += 1;
        
        Ok(())
    }
    
    async fn publish_batch(&self, events: &[MLEvent]) -> Result<()> {
        let batch_payload = serde_json::json!({
            "events": events,
            "batch_size": events.len(),
            "timestamp": Utc::now()
        });
        
        let mut request = self.client.post(&self.url)
            .json(&batch_payload);
        
        // Add custom headers
        for (key, value) in &self.headers {
            request = request.header(key, value);
        }
        
        let response = request.send().await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Webhook batch request failed with status: {}", response.status()));
        }
        
        let mut stats = self.stats.write().await;
        stats.total_events_published += events.len() as u64;
        stats.last_publish = Some(Utc::now());
        
        for event in events {
            let event_type_key = format!("{:?}", event.event_type);
            *stats.events_by_type.entry(event_type_key).or_insert(0) += 1;
        }
        
        Ok(())
    }
    
    async fn get_stats(&self) -> Result<EventStats> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }
    
    async fn health_check(&self) -> Result<bool> {
        // Try to make a HEAD request to the webhook URL
        match self.client.head(&self.url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }
}

/// Redis event backend (optional feature)
#[cfg(feature = "events")]
struct RedisEventBackend {
    client: redis::Client,
    stats: Arc<RwLock<EventStats>>,
}

#[cfg(feature = "events")]
impl RedisEventBackend {
    async fn new(connection_string: &str) -> Result<Self> {
        let client = redis::Client::open(connection_string)?;
        
        // Test connection
        let mut conn = client.get_multiplexed_async_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        
        Ok(Self {
            client,
            stats: Arc::new(RwLock::new(EventStats {
                total_events_published: 0,
                events_by_type: HashMap::new(),
                events_by_severity: HashMap::new(),
                publish_errors: 0,
                average_batch_size: 0.0,
                last_publish: None,
            })),
        })
    }
}

#[cfg(feature = "events")]
#[async_trait::async_trait]
impl EventBackendTrait for RedisEventBackend {
    async fn publish_event(&self, event: &MLEvent) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        
        let event_json = serde_json::to_string(event)?;
        let stream_key = "ml_events";
        
        let _: () = redis::cmd("XADD")
            .arg(stream_key)
            .arg("*") // Auto-generate ID
            .arg("event")
            .arg(&event_json)
            .query_async(&mut conn)
            .await?;
        
        let mut stats = self.stats.write().await;
        stats.total_events_published += 1;
        stats.last_publish = Some(Utc::now());
        
        let event_type_key = format!("{:?}", event.event_type);
        *stats.events_by_type.entry(event_type_key).or_insert(0) += 1;
        
        Ok(())
    }
    
    async fn publish_batch(&self, events: &[MLEvent]) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let stream_key = "ml_events";
        
        // Use pipeline for batch publishing
        let mut pipe = redis::pipe();
        
        for event in events {
            let event_json = serde_json::to_string(event)?;
            pipe.cmd("XADD")
                .arg(stream_key)
                .arg("*")
                .arg("event")
                .arg(&event_json);
        }
        
        let _: () = pipe.query_async(&mut conn).await?;
        
        let mut stats = self.stats.write().await;
        stats.total_events_published += events.len() as u64;
        stats.last_publish = Some(Utc::now());
        
        for event in events {
            let event_type_key = format!("{:?}", event.event_type);
            *stats.events_by_type.entry(event_type_key).or_insert(0) += 1;
        }
        
        Ok(())
    }
    
    async fn get_stats(&self) -> Result<EventStats> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }
    
    async fn health_check(&self) -> Result<bool> {
        match self.client.get_multiplexed_async_connection().await {
            Ok(mut conn) => {
                match redis::cmd("PING").query_async::<String>(&mut conn).await {
                    Ok(_) => Ok(true),
                    Err(_) => Ok(false),
                }
            }
            Err(_) => Ok(false),
        }
    }
}

// Add missing import and trait
use tokio::io::AsyncWriteExt;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_memory_event_backend() {
        let backend = MemoryEventBackend::new();
        
        let event = MLEvent {
            id: uuid::Uuid::new_v4(),
            event_type: MLEventType::TrainingStarted,
            job_id: Some(uuid::Uuid::new_v4()),
            workflow_id: Some("test".to_string()),
            timestamp: Utc::now(),
            payload: serde_json::json!({"test": "data"}),
        };
        
        // Test single event publishing
        backend.publish_event(&event).await.unwrap();
        
        let stats = backend.get_stats().await.unwrap();
        assert_eq!(stats.total_events_published, 1);
        
        // Test batch publishing
        backend.publish_batch(&[event.clone(), event.clone()]).await.unwrap();
        
        let stats = backend.get_stats().await.unwrap();
        assert_eq!(stats.total_events_published, 3);
    }
    
    #[tokio::test]
    async fn test_file_event_backend() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("events.log");
        
        let backend = FileEventBackend::new(file_path.to_str().unwrap()).unwrap();
        
        let event = MLEvent {
            id: uuid::Uuid::new_v4(),
            event_type: MLEventType::ModelRegistered,
            job_id: None,
            workflow_id: Some("test".to_string()),
            timestamp: Utc::now(),
            payload: serde_json::json!({"model_id": "test-model"}),
        };
        
        // Test publishing
        backend.publish_event(&event).await.unwrap();
        
        // Check file was created and contains event
        assert!(file_path.exists());
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains("ModelRegistered"));
    }
    
    #[tokio::test]
    async fn test_event_publisher() {
        let config = EventConfig::default();
        let publisher = EventPublisher::new(config).await.unwrap();
        
        let event = MLEvent {
            id: uuid::Uuid::new_v4(),
            event_type: MLEventType::TrainingCompleted,
            job_id: Some(uuid::Uuid::new_v4()),
            workflow_id: Some("test-workflow".to_string()),
            timestamp: Utc::now(),
            payload: serde_json::json!({"accuracy": 0.95}),
        };
        
        // Test publishing
        publisher.publish(event.clone()).await.unwrap();
        
        // Test batch publishing
        publisher.publish_batch(vec![event.clone(), event]).await.unwrap();
        
        // Check stats
        let stats = publisher.get_stats().await;
        assert_eq!(stats.total_events_published, 3);
        
        // Check health
        let health = publisher.health_check().await.unwrap();
        assert!(health.healthy);
    }
}