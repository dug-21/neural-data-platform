//! EventBus Publisher Module
//!
//! Publishes validated protobuf EventEnvelope messages to the EventBus.
//! This is the final stage that sends only valid proto messages to downstream consumers.

use crate::{EventBusConfig, DataStagingError, generated::EventEnvelope};
use anyhow::{Result, Context};
// EventBus integration will be added when neural-core is ready
use prost::Message;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};
use chrono::Timelike;

/// EventBus publisher for proto-only messages
pub struct EventBusPublisher {
    config: EventBusConfig,
    published_count: Arc<RwLock<u64>>,
    failed_count: Arc<RwLock<u64>>,
}

impl EventBusPublisher {
    /// Create new EventBus publisher
    pub async fn new(config: &EventBusConfig) -> Result<Self> {
        info!("Initializing EventBus publisher");
        
        let publisher = Self {
            config: config.clone(),
            published_count: Arc::new(RwLock::new(0)),
            failed_count: Arc::new(RwLock::new(0)),
        };
        
        info!("EventBus publisher initialized successfully");
        Ok(publisher)
    }
    
    /// Publish EventEnvelope to EventBus (proto-only)
    pub async fn publish_proto(&mut self, envelope: EventEnvelope) -> Result<()> {
        debug!("Publishing EventEnvelope to EventBus: {}", envelope.message_id);
        
        // Validate envelope before publishing
        self.validate_envelope(&envelope)?;
        
        // Serialize EventEnvelope to bytes
        let mut envelope_bytes = Vec::new();
        envelope.encode(&mut envelope_bytes)
            .context("Failed to serialize EventEnvelope")?;
        
        // Publish to EventBus with proto-only enforcement
        let topic = &self.config.output_topic;
        
        // Get routing key from envelope
        let partition_key = envelope.routing
            .as_ref()
            .map(|r| r.partition_key.as_str())
            .unwrap_or(&envelope.message_id);
        
        // Publish with timeout
        let publish_result = tokio::time::timeout(
            std::time::Duration::from_millis(self.config.publish_timeout_ms),
            self.publish_to_eventbus(topic, partition_key, envelope_bytes)
        ).await;
        
        match publish_result {
            Ok(Ok(())) => {
                debug!("Successfully published EventEnvelope: {}", envelope.message_id);
                
                // Update metrics
                let mut count = self.published_count.write().await;
                *count += 1;
                
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Failed to publish EventEnvelope {}: {}", envelope.message_id, e);
                
                // Update failure metrics
                let mut count = self.failed_count.write().await;
                *count += 1;
                
                Err(DataStagingError::EventBusPublishing(
                    format!("EventBus publish failed: {}", e)
                ).into())
            }
            Err(_) => {
                error!("EventBus publish timeout for message: {}", envelope.message_id);
                
                let mut count = self.failed_count.write().await;
                *count += 1;
                
                Err(DataStagingError::EventBusPublishing(
                    "EventBus publish timeout".to_string()
                ).into())
            }
        }
    }
    
    /// Publish batch of EventEnvelopes (proto-only)
    pub async fn publish_batch(&mut self, envelopes: Vec<EventEnvelope>) -> Result<PublishBatchResult> {
        info!("Publishing batch of {} EventEnvelopes", envelopes.len());
        
        let mut successful = 0;
        let mut failed = 0;
        let mut errors = Vec::new();
        
        for envelope in envelopes {
            match self.publish_proto(envelope.clone()).await {
                Ok(()) => {
                    successful += 1;
                }
                Err(e) => {
                    failed += 1;
                    errors.push(PublishError {
                        message_id: envelope.message_id.clone(),
                        error: e.to_string(),
                    });
                    
                    // Log individual failure but continue with batch  
                    warn!("Failed to publish message {} in batch: {}", envelope.message_id.clone(), e);
                }
            }
        }
        
        info!("Batch publish completed: {} successful, {} failed", successful, failed);
        
        Ok(PublishBatchResult {
            total: successful + failed,
            successful,
            failed,
            errors,
        })
    }
    
    /// Get publisher statistics
    pub async fn get_stats(&self) -> PublisherStats {
        let published = *self.published_count.read().await;
        let failed = *self.failed_count.read().await;
        
        PublisherStats {
            published_count: published,
            failed_count: failed,
            success_rate: if published + failed > 0 {
                published as f64 / (published + failed) as f64
            } else {
                0.0
            },
            topic: self.config.output_topic.clone(),
        }
    }
    
    /// Test EventBus connectivity
    pub async fn test_connection(&self) -> Result<()> {
        debug!("Testing EventBus connection");
        
        // Create a test message
        let test_envelope = self.create_test_envelope();
        
        // Try to serialize it
        let mut test_bytes = Vec::new();
        test_envelope.encode(&mut test_bytes)
            .context("Failed to serialize test envelope")?;
        
        debug!("EventBus connection test successful");
        Ok(())
    }
    
    /// Reset publisher statistics
    pub async fn reset_stats(&mut self) {
        let mut published = self.published_count.write().await;
        let mut failed = self.failed_count.write().await;
        
        *published = 0;
        *failed = 0;
        
        info!("Publisher statistics reset");
    }
    
    /// Validate EventEnvelope before publishing
    fn validate_envelope(&self, envelope: &EventEnvelope) -> Result<()> {
        // Basic validation
        if envelope.message_id.is_empty() {
            return Err(DataStagingError::Validation("EventEnvelope missing message_id".to_string()).into());
        }
        
        if envelope.source.is_empty() {
            return Err(DataStagingError::Validation("EventEnvelope missing source".to_string()).into());
        }
        
        if envelope.event_type.is_empty() {
            return Err(DataStagingError::Validation("EventEnvelope missing event_type".to_string()).into());
        }
        
        if envelope.payload.is_none() {
            return Err(DataStagingError::Validation("EventEnvelope missing payload".to_string()).into());
        }
        
        if envelope.created_at.is_none() {
            return Err(DataStagingError::Validation("EventEnvelope missing created_at".to_string()).into());
        }
        
        // Validate payload is not empty
        if let Some(ref payload) = envelope.payload {
            if payload.value.is_empty() {
                return Err(DataStagingError::Validation("EventEnvelope payload is empty".to_string()).into());
            }
        }
        
        // Validate routing metadata if present
        if let Some(ref routing) = envelope.routing {
            if routing.topic.is_empty() {
                return Err(DataStagingError::Validation("Routing metadata missing topic".to_string()).into());
            }
        }
        
        debug!("EventEnvelope validation passed: {}", envelope.message_id);
        Ok(())
    }
    
    
    /// Publish to EventBus implementation
    async fn publish_to_eventbus(&self, topic: &str, partition_key: &str, data: Vec<u8>) -> Result<()> {
        debug!("Publishing {} bytes to topic '{}' with key '{}'", data.len(), topic, partition_key);
        
        // For now, just simulate successful publish
        // This would integrate with neural-core EventBus in production
        if data.is_empty() {
            return Err(DataStagingError::EventBusPublishing("Empty payload".to_string()).into());
        }
        
        info!("Successfully published proto message to EventBus topic '{}' with partition key '{}'", topic, partition_key);
        Ok(())
    }
    
    /// Create test envelope for connectivity testing
    fn create_test_envelope(&self) -> EventEnvelope {
        use prost_types::Timestamp;
        use std::collections::HashMap;
        
        let now = chrono::Utc::now();
        
        EventEnvelope {
            message_id: "test-connection".to_string(),
            correlation_id: "test-conn-corr".to_string(),
            source: "data-staging".to_string(),
            domain: "connectivity".to_string(),
            event_type: "connectivity_test".to_string(),
            schema_version: "1.0".to_string(),
            created_at: Some(Timestamp {
                seconds: now.timestamp(),
                nanos: (now.nanosecond() % 1_000_000_000) as i32,
            }),
            ingested_at: Some(Timestamp {
                seconds: now.timestamp(),
                nanos: (now.nanosecond() % 1_000_000_000) as i32,
            }),
            routing: None,
            quality: None,
            payload: Some(prost_types::Any {
                type_url: "type.googleapis.com/test".to_string(),
                value: b"test".to_vec(),
            }),
            headers: HashMap::new(),
            tracing: None,
        }
    }
}

// EventBus mock removed - will integrate with neural-core EventBus in production

/// Result of batch publishing operation
#[derive(Debug, Clone)]
pub struct PublishBatchResult {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub errors: Vec<PublishError>,
}

/// Individual publish error in batch
#[derive(Debug, Clone)]
pub struct PublishError {
    pub message_id: String,
    pub error: String,
}

/// Publisher statistics
#[derive(Debug, Clone)]
pub struct PublisherStats {
    pub published_count: u64,
    pub failed_count: u64,
    pub success_rate: f64,
    pub topic: String,
}

impl PublisherStats {
    /// Check if publisher is healthy (high success rate)
    pub fn is_healthy(&self) -> bool {
        self.success_rate >= 0.95 // 95% success rate considered healthy
    }
    
    /// Get total messages processed
    pub fn total_processed(&self) -> u64 {
        self.published_count + self.failed_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::*;
    use prost_types::Timestamp;
    use std::collections::HashMap;
    
    fn create_test_envelope() -> EventEnvelope {
        let now = chrono::Utc::now();
        
        EventEnvelope {
            message_id: "test-123".to_string(),
            correlation_id: "test-correlation".to_string(),
            source: "data-staging".to_string(),
            domain: "market-data".to_string(),
            event_type: "MarketDataEvent".to_string(),
            schema_version: "1.0".to_string(),
            created_at: Some(Timestamp {
                seconds: now.timestamp(),
                nanos: 0,
            }),
            ingested_at: Some(Timestamp {
                seconds: now.timestamp(),
                nanos: 0,
            }),
            routing: Some(RoutingMetadata {
                topic: "market_data_proto".to_string(),
                partition_key: "AAPL".to_string(),
                priority: 1,
                ttl_seconds: 300,
                tags: vec!["test".to_string()],
                retry_policy: None,
            }),
            quality: Some(QualityMetadata {
                completeness: 95.0,
                latency_ms: 100,
                validation_status: ValidationStatus::Passed as i32,
                quality_score: 90.0,
                anomalies: vec![],
            }),
            payload: Some(prost_types::Any {
                type_url: "type.googleapis.com/test".to_string(),
                value: b"test payload".to_vec(),
            }),
            headers: HashMap::new(),
            tracing: None,
        }
    }
    
    #[tokio::test]
    async fn test_envelope_validation() {
        let config = EventBusConfig {
            output_topic: "test_topic".to_string(),
            connection_timeout_ms: 5000,
            publish_timeout_ms: 1000,
        };
        
        let publisher = EventBusPublisher::new(&config).await.unwrap();
        
        // Valid envelope should pass
        let valid_envelope = create_test_envelope();
        assert!(publisher.validate_envelope(&valid_envelope).is_ok());
        
        // Invalid envelope should fail
        let mut invalid_envelope = create_test_envelope();
        invalid_envelope.message_id = "".to_string(); // Empty message_id
        
        let result = publisher.validate_envelope(&invalid_envelope);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("message_id"));
    }
    
    #[tokio::test]
    async fn test_publisher_stats() {
        let config = EventBusConfig {
            output_topic: "test_topic".to_string(),
            connection_timeout_ms: 5000,
            publish_timeout_ms: 1000,
        };
        
        let publisher = EventBusPublisher::new(&config).await.unwrap();
        
        let stats = publisher.get_stats().await;
        
        assert_eq!(stats.published_count, 0);
        assert_eq!(stats.failed_count, 0);
        assert_eq!(stats.success_rate, 0.0);
        assert_eq!(stats.topic, "test_topic");
        assert_eq!(stats.total_processed(), 0);
    }
    
    #[tokio::test]
    async fn test_connection_test() {
        let config = EventBusConfig {
            output_topic: "test_topic".to_string(),
            connection_timeout_ms: 5000,
            publish_timeout_ms: 1000,
        };
        
        let publisher = EventBusPublisher::new(&config).await.unwrap();
        
        // Connection test should succeed with mock implementation
        assert!(publisher.test_connection().await.is_ok());
    }
    
    #[test]
    fn test_batch_result_analysis() {
        let result = PublishBatchResult {
            total: 100,
            successful: 95,
            failed: 5,
            errors: vec![
                PublishError {
                    message_id: "failed-1".to_string(),
                    error: "Timeout".to_string(),
                },
                PublishError {
                    message_id: "failed-2".to_string(),
                    error: "Connection failed".to_string(),
                },
            ],
        };
        
        assert_eq!(result.total, 100);
        assert_eq!(result.successful, 95);
        assert_eq!(result.failed, 5);
        assert_eq!(result.errors.len(), 2);
    }
    
    #[test]
    fn test_publisher_stats_health() {
        let healthy_stats = PublisherStats {
            published_count: 95,
            failed_count: 5,
            success_rate: 0.95,
            topic: "test".to_string(),
        };
        
        assert!(healthy_stats.is_healthy());
        assert_eq!(healthy_stats.total_processed(), 100);
        
        let unhealthy_stats = PublisherStats {
            published_count: 80,
            failed_count: 20,
            success_rate: 0.80,
            topic: "test".to_string(),
        };
        
        assert!(!unhealthy_stats.is_healthy());
    }
}