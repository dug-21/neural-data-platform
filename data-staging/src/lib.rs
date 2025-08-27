//! Data-Staging Service - JSON to Proto Transformation Layer
//!
//! This service acts as the ONLY bridge between raw JSON data (from data-ingestion)
//! and the proto-only EventBus system. It enforces strict data quality validation
//! and transformation to ensure only valid protobuf messages reach the EventBus.
//!
//! ## Architecture
//! ```
//! Data-Ingestion (JSON) → Redis Streams → Data-Staging → EventBus (Proto Only)
//!                                            ↓
//!                                       DLQ (Invalid Data)
//! ```

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use thiserror::Error;

// Generated proto code from tonic_build or placeholder types
pub mod generated {
    // Include the generated proto code (real or placeholder)
    tonic::include_proto!("neural_trader.interfaces.ingestion");
    
    // Proto types are available in this module
}

pub mod redis_consumer;
pub mod json_validator;
pub mod proto_transformer;
pub mod quality_scorer;
pub mod dlq_manager;
pub mod eventbus_publisher;
pub mod metrics;

// Remove these imports since config and error modules don't exist separately

pub use generated::*;

#[derive(Error, Debug)]
pub enum DataStagingError {
    #[error("Redis connection error: {0}")]
    Redis(#[from] redis::RedisError),
    
    #[error("JSON parsing error: {0}")]
    JsonParsing(#[from] serde_json::Error),
    
    #[error("Proto serialization error: {0}")]
    ProtoSerialization(#[from] prost::EncodeError),
    
    #[error("Proto deserialization error: {0}")]
    ProtoDeserialization(#[from] prost::DecodeError),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Data quality error: {0}")]
    DataQuality(String),
    
    #[error("EventBus publishing error: {0}")]
    EventBusPublishing(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Contract violation: {0}")]
    ContractViolation(String),
}

/// Configuration for the Data-Staging service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataStagingConfig {
    /// Redis connection URL
    pub redis_url: String,
    
    /// Redis stream to consume raw JSON from
    pub input_stream: String,
    
    /// Consumer group name
    pub consumer_group: String,
    
    /// Consumer name
    pub consumer_name: String,
    
    /// EventBus connection configuration
    pub eventbus_config: EventBusConfig,
    
    /// Quality scoring thresholds
    pub quality_thresholds: QualityThresholds,
    
    /// Processing limits
    pub processing_limits: ProcessingLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBusConfig {
    /// EventBus topic for validated proto messages
    pub output_topic: String,
    
    /// Connection timeout in milliseconds
    pub connection_timeout_ms: u64,
    
    /// Publish timeout in milliseconds
    pub publish_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityThresholds {
    /// Minimum quality score to accept (0.0 - 1.0)
    pub minimum_quality_score: f32,
    
    /// Freshness threshold in seconds
    pub max_age_seconds: i64,
    
    /// Required fields for market data
    pub required_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingLimits {
    /// Maximum batch size for processing
    pub max_batch_size: usize,
    
    /// Processing timeout per message in milliseconds
    pub message_timeout_ms: u64,
    
    /// Maximum retries for failed transformations
    pub max_retries: u32,
}

impl Default for DataStagingConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://localhost:6379".to_string(),
            input_stream: "market_data_raw".to_string(),
            consumer_group: "data-staging".to_string(),
            consumer_name: "data-staging-1".to_string(),
            eventbus_config: EventBusConfig {
                output_topic: "market_data_proto".to_string(),
                connection_timeout_ms: 5000,
                publish_timeout_ms: 1000,
            },
            quality_thresholds: QualityThresholds {
                minimum_quality_score: 0.7,
                max_age_seconds: 300, // 5 minutes
                required_fields: vec![
                    "symbol".to_string(),
                    "price".to_string(),
                    "timestamp".to_string(),
                ],
            },
            processing_limits: ProcessingLimits {
                max_batch_size: 100,
                message_timeout_ms: 1000,
                max_retries: 3,
            },
        }
    }
}

/// Raw JSON market data structure (input)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawMarketData {
    pub symbol: Option<String>,
    pub price: Option<f64>,
    pub volume: Option<f64>,
    pub timestamp: Option<i64>,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub exchange: Option<String>,
    pub sequence: Option<u64>,
    
    // Additional optional fields
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub open: Option<f64>,
    pub close: Option<f64>,
    pub vwap: Option<f64>,
    
    // Metadata
    #[serde(flatten)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Data quality metrics for a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQualityMetrics {
    /// Overall quality score (0.0 - 1.0)
    pub overall_score: f32,
    
    /// Freshness score based on timestamp age
    pub freshness_score: f32,
    
    /// Completeness score based on required vs optional fields
    pub completeness_score: f32,
    
    /// Data validity score based on field validation
    pub validity_score: f32,
    
    /// Number of missing required fields
    pub missing_required_fields: u32,
    
    /// Number of present optional fields
    pub present_optional_fields: u32,
    
    /// Age of the data in seconds
    pub data_age_seconds: i64,
    
    /// Validation errors encountered
    pub validation_errors: Vec<String>,
}

/// Main Data-Staging service
pub struct DataStagingService {
    config: DataStagingConfig,
    redis_consumer: redis_consumer::RedisConsumer,
    json_validator: json_validator::JsonValidator,
    proto_transformer: proto_transformer::ProtoTransformer,
    quality_scorer: quality_scorer::QualityScorer,
    dlq_manager: dlq_manager::DlqManager,
    eventbus_publisher: eventbus_publisher::EventBusPublisher,
    metrics: metrics::MetricsCollector,
}

impl DataStagingService {
    /// Create a new Data-Staging service instance
    pub async fn new(config: DataStagingConfig) -> Result<Self> {
        let redis_consumer = redis_consumer::RedisConsumer::new(&config).await
            .context("Failed to create Redis consumer")?;
            
        let json_validator = json_validator::JsonValidator::new(&config.quality_thresholds);
        
        let proto_transformer = proto_transformer::ProtoTransformer::new();
        
        let quality_scorer = quality_scorer::QualityScorer::new(&config.quality_thresholds);
        
        let dlq_manager = dlq_manager::DlqManager::new(&config).await
            .context("Failed to create DLQ manager")?;
            
        let eventbus_publisher = eventbus_publisher::EventBusPublisher::new(&config.eventbus_config).await
            .context("Failed to create EventBus publisher")?;
            
        let metrics = metrics::MetricsCollector::new()
            .context("Failed to create metrics collector")?;

        Ok(Self {
            config,
            redis_consumer,
            json_validator,
            proto_transformer,
            quality_scorer,
            dlq_manager,
            eventbus_publisher,
            metrics,
        })
    }
    
    /// Start the main processing loop
    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("Starting Data-Staging service processing loop");
        
        loop {
            match self.process_batch().await {
                Ok(processed_count) => {
                    self.metrics.record_batch_processed(processed_count).await;
                    
                    if processed_count == 0 {
                        // No messages available, brief pause
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
                Err(e) => {
                    tracing::error!("Batch processing failed: {}", e);
                    self.metrics.record_processing_error().await;
                    
                    // Brief pause before retrying
                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                }
            }
        }
    }
    
    /// Process a batch of messages
    async fn process_batch(&mut self) -> Result<usize> {
        let messages = self.redis_consumer.consume_batch().await?;
        
        if messages.is_empty() {
            return Ok(0);
        }
        
        tracing::debug!("Processing batch of {} messages", messages.len());
        
        let mut processed_count = 0;
        
        for message in messages {
            match self.process_message(&message.data).await {
                Ok(_) => {
                    processed_count += 1;
                    self.redis_consumer.acknowledge_message(&message.id).await?;
                }
                Err(e) => {
                    tracing::warn!("Failed to process message {}: {}", message.id, e);
                    self.dlq_manager.send_to_dlq(&message.data, &e.to_string()).await?;
                    self.redis_consumer.acknowledge_message(&message.id).await?;
                }
            }
        }
        
        Ok(processed_count)
    }
    
    /// Process a single message: JSON → Validation → Proto → EventBus
    async fn process_message(&mut self, json_data: &str) -> Result<()> {
        // Step 1: Parse JSON
        let raw_data: RawMarketData = serde_json::from_str(json_data)
            .context("Failed to parse JSON message")?;
            
        // Step 2: Validate JSON structure and data quality
        self.json_validator.validate(&raw_data)?;
        
        // Step 3: Calculate quality score
        let quality_metrics = self.quality_scorer.calculate_quality(&raw_data);
        
        // Step 4: Check if quality meets threshold
        if quality_metrics.overall_score < self.config.quality_thresholds.minimum_quality_score {
            return Err(DataStagingError::DataQuality(
                format!("Quality score {} below threshold {}", 
                    quality_metrics.overall_score, 
                    self.config.quality_thresholds.minimum_quality_score)
            ).into());
        }
        
        // Step 5: Transform to EventEnvelope proto
        let event_envelope = self.proto_transformer.transform_to_event_envelope(
            &raw_data,
            &quality_metrics
        )?;
        
        // Step 6: Publish to EventBus (proto-only)
        self.eventbus_publisher.publish_proto(event_envelope).await?;
        
        // Step 7: Record success metrics
        self.metrics.record_message_processed(&quality_metrics).await;
        
        Ok(())
    }
}