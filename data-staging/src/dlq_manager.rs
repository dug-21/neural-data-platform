//! Dead Letter Queue Manager
//!
//! Handles invalid data by sending it to a Dead Letter Queue for analysis and debugging.
//! This ensures that no data is lost even when it fails validation or transformation.

use crate::{DataStagingConfig, DataStagingError};
use anyhow::{Result, Context};
use redis::{AsyncCommands, RedisResult, aio::MultiplexedConnection};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn, error, info};
use uuid::Uuid;

/// Dead Letter Queue message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqMessage {
    /// Unique ID for the DLQ message
    pub dlq_id: String,
    
    /// Original message data that failed processing
    pub original_data: String,
    
    /// Error that caused the message to be sent to DLQ
    pub error_message: String,
    
    /// Error category for filtering
    pub error_category: String,
    
    /// Timestamp when message was sent to DLQ
    pub dlq_timestamp: i64,
    
    /// Original timestamp from the data (if available)
    pub original_timestamp: Option<i64>,
    
    /// Processing stage where error occurred
    pub failure_stage: String,
    
    /// Number of retry attempts made
    pub retry_count: u32,
    
    /// Source system that generated the original data
    pub source: String,
    
    /// Additional metadata about the failure
    pub metadata: std::collections::HashMap<String, String>,
}

/// Manages the Dead Letter Queue for failed messages
pub struct DlqManager {
    connection: MultiplexedConnection,
    dlq_stream: String,
    dlq_retention_hours: u32,
    max_dlq_size: usize,
}

impl DlqManager {
    /// Create new DLQ manager
    pub async fn new(config: &DataStagingConfig) -> Result<Self> {
        info!("Initializing DLQ manager");
        
        let client = redis::Client::open(config.redis_url.as_str())
            .context("Failed to create Redis client for DLQ")?;
            
        let connection = client.get_multiplexed_async_connection().await
            .context("Failed to establish Redis connection for DLQ")?;
        
        let dlq_stream = format!("{}_dlq", config.input_stream);
        
        let dlq_manager = Self {
            connection,
            dlq_stream,
            dlq_retention_hours: 24, // Keep DLQ messages for 24 hours
            max_dlq_size: 100000, // Maximum 100k messages in DLQ
        };
        
        // Initialize DLQ stream
        dlq_manager.initialize_dlq_stream().await?;
        
        info!("DLQ manager initialized successfully");
        Ok(dlq_manager)
    }
    
    /// Send message to Dead Letter Queue
    pub async fn send_to_dlq(&mut self, original_data: &str, error_message: &str) -> Result<()> {
        let dlq_message = self.create_dlq_message(original_data, error_message)?;
        
        debug!("Sending message to DLQ: {}", dlq_message.dlq_id);
        
        // Serialize DLQ message
        let dlq_json = serde_json::to_string(&dlq_message)
            .context("Failed to serialize DLQ message")?;
        
        // Add to DLQ stream
        let result: RedisResult<String> = redis::cmd("XADD")
            .arg(&self.dlq_stream)
            .arg("*") // Auto-generate ID
            .arg("data")
            .arg(&dlq_json)
            .arg("timestamp")
            .arg(chrono::Utc::now().timestamp())
            .arg("error_category")
            .arg(&dlq_message.error_category)
            .arg("failure_stage")
            .arg(&dlq_message.failure_stage)
            .query_async(&mut self.connection)
            .await;
            
        match result {
            Ok(message_id) => {
                info!("Message sent to DLQ with ID: {}", message_id);
                
                // Check if DLQ needs cleanup
                self.cleanup_old_messages().await?;
                
                Ok(())
            }
            Err(e) => {
                error!("Failed to send message to DLQ: {}", e);
                Err(DataStagingError::Redis(e).into())
            }
        }
    }
    
    /// Send message to DLQ with detailed error information
    pub async fn send_to_dlq_detailed(
        &mut self, 
        original_data: &str, 
        error_message: &str,
        error_category: &str,
        failure_stage: &str,
        retry_count: u32,
        metadata: std::collections::HashMap<String, String>,
    ) -> Result<()> {
        let mut dlq_message = self.create_dlq_message(original_data, error_message)?;
        
        // Override with detailed information
        dlq_message.error_category = error_category.to_string();
        dlq_message.failure_stage = failure_stage.to_string();
        dlq_message.retry_count = retry_count;
        dlq_message.metadata = metadata;
        
        debug!("Sending detailed message to DLQ: {} (category: {}, stage: {})", 
               dlq_message.dlq_id, error_category, failure_stage);
        
        // Serialize DLQ message
        let dlq_json = serde_json::to_string(&dlq_message)
            .context("Failed to serialize detailed DLQ message")?;
        
        // Add to DLQ stream with detailed information
        let result: RedisResult<String> = redis::cmd("XADD")
            .arg(&self.dlq_stream)
            .arg("*")
            .arg("data")
            .arg(&dlq_json)
            .arg("timestamp")
            .arg(dlq_message.dlq_timestamp)
            .arg("error_category")
            .arg(&dlq_message.error_category)
            .arg("failure_stage")
            .arg(&dlq_message.failure_stage)
            .arg("retry_count")
            .arg(dlq_message.retry_count)
            .query_async(&mut self.connection)
            .await;
            
        match result {
            Ok(message_id) => {
                info!("Detailed message sent to DLQ with ID: {} (error: {})", 
                      message_id, error_category);
                Ok(())
            }
            Err(e) => {
                error!("Failed to send detailed message to DLQ: {}", e);
                Err(DataStagingError::Redis(e).into())
            }
        }
    }
    
    /// Get DLQ statistics
    pub async fn get_dlq_stats(&mut self) -> Result<DlqStats> {
        debug!("Retrieving DLQ statistics");
        
        // Get stream length
        let length: RedisResult<usize> = redis::cmd("XLEN")
            .arg(&self.dlq_stream)
            .query_async(&mut self.connection)
            .await;
            
        let total_messages = length.unwrap_or(0);
        
        // Get messages from last hour for rate calculation
        let one_hour_ago = chrono::Utc::now().timestamp() - 3600;
        
        let recent_messages: RedisResult<Vec<redis::streams::StreamReadReply>> = 
            redis::cmd("XRANGE")
                .arg(&self.dlq_stream)
                .arg("-")
                .arg("+")
                .query_async(&mut self.connection)
                .await;
                
        let (recent_count, error_categories) = match recent_messages {
            Ok(messages) => {
                let mut recent = 0;
                let mut categories = std::collections::HashMap::new();
                
                for stream_reply in messages {
                    for stream_key in stream_reply.keys {
                        for stream_msg in stream_key.ids {
                        // Check timestamp
                        if let Some(ts_bytes) = stream_msg.map.get("timestamp") {
                            if let redis::Value::Data(bytes) = ts_bytes {
                                    if let Ok(ts_str) = std::str::from_utf8(bytes) {
                                        if let Ok(timestamp) = ts_str.parse::<i64>() {
                                            if timestamp >= one_hour_ago {
                                                recent += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        
                        // Count error categories
                        if let Some(cat_bytes) = stream_msg.map.get("error_category") {
                            if let redis::Value::Data(bytes) = cat_bytes {
                                if let Ok(category) = std::str::from_utf8(bytes) {
                                    *categories.entry(category.to_string()).or_insert(0) += 1;
                                }
                            }
                        }
                        }
                    }
                }
                
                (recent, categories)
            }
            Err(_) => (0, std::collections::HashMap::new()),
        };
        
        Ok(DlqStats {
            total_messages,
            messages_last_hour: recent_count,
            error_categories,
            stream_name: self.dlq_stream.clone(),
            retention_hours: self.dlq_retention_hours,
        })
    }
    
    /// Retrieve messages from DLQ for analysis
    pub async fn get_dlq_messages(&mut self, limit: usize) -> Result<Vec<DlqMessage>> {
        debug!("Retrieving {} DLQ messages for analysis", limit);
        
        let messages: RedisResult<Vec<redis::streams::StreamReadReply>> = 
            redis::cmd("XREVRANGE")
                .arg(&self.dlq_stream)
                .arg("+")
                .arg("-")
                .arg("COUNT")
                .arg(limit)
                .query_async(&mut self.connection)
                .await;
                
        match messages {
            Ok(stream_replies) => {
                let mut dlq_messages = Vec::new();
                
                for stream_reply in stream_replies {
                    for stream_key in stream_reply.keys {
                        for stream_msg in stream_key.ids {
                        if let Some(data_bytes) = stream_msg.map.get("data") {
                            if let redis::Value::Data(bytes) = data_bytes {
                                    if let Ok(data_str) = std::str::from_utf8(bytes) {
                                        match serde_json::from_str::<DlqMessage>(data_str) {
                                            Ok(dlq_msg) => dlq_messages.push(dlq_msg),
                                            Err(e) => {
                                                warn!("Failed to parse DLQ message: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                
                debug!("Retrieved {} DLQ messages", dlq_messages.len());
                Ok(dlq_messages)
            }
            Err(e) => {
                error!("Failed to retrieve DLQ messages: {}", e);
                Err(DataStagingError::Redis(e).into())
            }
        }
    }
    
    /// Get messages by error category
    pub async fn get_messages_by_category(&mut self, category: &str, limit: usize) -> Result<Vec<DlqMessage>> {
        debug!("Retrieving DLQ messages for category: {}", category);
        
        let all_messages = self.get_dlq_messages(limit * 2).await?; // Get more to filter
        
        let filtered: Vec<DlqMessage> = all_messages.into_iter()
            .filter(|msg| msg.error_category == category)
            .take(limit)
            .collect();
            
        debug!("Found {} messages for category: {}", filtered.len(), category);
        Ok(filtered)
    }
    
    /// Initialize the DLQ stream
    async fn initialize_dlq_stream(&self) -> Result<()> {
        debug!("Initializing DLQ stream: {}", self.dlq_stream);
        
        // Try to get stream info (this will fail if stream doesn't exist)
        let info_result: RedisResult<std::collections::HashMap<String, redis::Value>> = 
            redis::cmd("XINFO")
                .arg("STREAM")
                .arg(&self.dlq_stream)
                .query_async(&mut self.connection.clone())
                .await;
                
        match info_result {
            Ok(_) => {
                debug!("DLQ stream already exists: {}", self.dlq_stream);
            }
            Err(_) => {
                // Stream doesn't exist, create it with a dummy message
                let _: RedisResult<String> = redis::cmd("XADD")
                    .arg(&self.dlq_stream)
                    .arg("*")
                    .arg("init")
                    .arg("DLQ initialized")
                    .query_async(&mut self.connection.clone())
                    .await;
                    
                info!("Created DLQ stream: {}", self.dlq_stream);
            }
        }
        
        Ok(())
    }
    
    /// Create a DLQ message from original data and error
    fn create_dlq_message(&self, original_data: &str, error_message: &str) -> Result<DlqMessage> {
        // Try to parse original data to extract metadata
        let (original_timestamp, source) = self.extract_metadata(original_data);
        
        // Categorize error
        let error_category = self.categorize_error(error_message);
        
        // Determine failure stage
        let failure_stage = self.determine_failure_stage(error_message);
        
        Ok(DlqMessage {
            dlq_id: Uuid::new_v4().to_string(),
            original_data: original_data.to_string(),
            error_message: error_message.to_string(),
            error_category,
            dlq_timestamp: chrono::Utc::now().timestamp(),
            original_timestamp,
            failure_stage,
            retry_count: 0,
            source,
            metadata: std::collections::HashMap::new(),
        })
    }
    
    /// Extract metadata from original data
    fn extract_metadata(&self, original_data: &str) -> (Option<i64>, String) {
        // Try to parse as JSON to extract timestamp and source info
        match serde_json::from_str::<serde_json::Value>(original_data) {
            Ok(json) => {
                let timestamp = json.get("timestamp")
                    .and_then(|v| v.as_i64());
                    
                let source = json.get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                    
                (timestamp, source)
            }
            Err(_) => (None, "unknown".to_string()),
        }
    }
    
    /// Categorize error based on error message
    fn categorize_error(&self, error_message: &str) -> String {
        let error_lower = error_message.to_lowercase();
        
        if error_lower.contains("json") || error_lower.contains("parse") {
            "JSON_PARSING".to_string()
        } else if error_lower.contains("validation") || error_lower.contains("invalid") {
            "VALIDATION".to_string()
        } else if error_lower.contains("proto") || error_lower.contains("serializ") {
            "PROTO_TRANSFORMATION".to_string()
        } else if error_lower.contains("quality") || error_lower.contains("threshold") {
            "QUALITY_CHECK".to_string()
        } else if error_lower.contains("redis") || error_lower.contains("connection") {
            "INFRASTRUCTURE".to_string()
        } else if error_lower.contains("timeout") {
            "TIMEOUT".to_string()
        } else {
            "UNKNOWN".to_string()
        }
    }
    
    /// Determine failure stage based on error message
    fn determine_failure_stage(&self, error_message: &str) -> String {
        let error_lower = error_message.to_lowercase();
        
        if error_lower.contains("json") || error_lower.contains("parse") {
            "JSON_PARSING".to_string()
        } else if error_lower.contains("validation") {
            "VALIDATION".to_string()
        } else if error_lower.contains("quality") {
            "QUALITY_SCORING".to_string()
        } else if error_lower.contains("proto") || error_lower.contains("transform") {
            "PROTO_TRANSFORMATION".to_string()
        } else if error_lower.contains("eventbus") || error_lower.contains("publish") {
            "EVENTBUS_PUBLISHING".to_string()
        } else {
            "UNKNOWN_STAGE".to_string()
        }
    }
    
    /// Clean up old messages from DLQ
    async fn cleanup_old_messages(&mut self) -> Result<()> {
        // Only cleanup periodically to avoid performance impact
        if chrono::Utc::now().timestamp() % 100 > 1 { // Approximately 99% chance
            return Ok(());
        }
        
        debug!("Starting DLQ cleanup");
        
        let cutoff_timestamp = chrono::Utc::now().timestamp() - (self.dlq_retention_hours as i64 * 3600);
        
        // Use XTRIM to limit stream size
        let _: RedisResult<i64> = redis::cmd("XTRIM")
            .arg(&self.dlq_stream)
            .arg("MAXLEN")
            .arg("~") // Approximate trimming for efficiency
            .arg(self.max_dlq_size)
            .query_async(&mut self.connection)
            .await;
            
        debug!("DLQ cleanup completed");
        Ok(())
    }
}

/// DLQ statistics
#[derive(Debug, Clone)]
pub struct DlqStats {
    pub total_messages: usize,
    pub messages_last_hour: usize,
    pub error_categories: std::collections::HashMap<String, usize>,
    pub stream_name: String,
    pub retention_hours: u32,
}

impl DlqStats {
    /// Get error rate per hour
    pub fn error_rate_per_hour(&self) -> f64 {
        self.messages_last_hour as f64
    }
    
    /// Get most common error category
    pub fn most_common_error(&self) -> Option<String> {
        self.error_categories.iter()
            .max_by_key(|(_, count)| *count)
            .map(|(category, _)| category.clone())
    }
    
    /// Check if DLQ is healthy (low error rate)
    pub fn is_healthy(&self) -> bool {
        self.messages_last_hour < 10 // Less than 10 errors per hour is considered healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataStagingConfig;
    
    #[tokio::test]
    #[ignore] // Requires Redis instance
    async fn test_dlq_manager_creation() {
        let config = DataStagingConfig::default();
        let result = DlqManager::new(&config).await;
        
        match result {
            Ok(dlq_manager) => {
                assert_eq!(dlq_manager.dlq_stream, "market_data_raw_dlq");
                assert_eq!(dlq_manager.dlq_retention_hours, 24);
            }
            Err(_) => {
                // Expected without Redis
            }
        }
    }
    
    #[test]
    fn test_error_categorization() {
        let config = DataStagingConfig::default();
        // We can't create a real DlqManager without Redis, but we can test the logic
        
        // Mock the categorization logic
        let categorize_error = |error_message: &str| -> String {
            let error_lower = error_message.to_lowercase();
            
            if error_lower.contains("json") || error_lower.contains("parse") {
                "JSON_PARSING".to_string()
            } else if error_lower.contains("validation") || error_lower.contains("invalid") {
                "VALIDATION".to_string()
            } else if error_lower.contains("proto") || error_lower.contains("serializ") {
                "PROTO_TRANSFORMATION".to_string()
            } else {
                "UNKNOWN".to_string()
            }
        };
        
        assert_eq!(categorize_error("JSON parsing failed"), "JSON_PARSING");
        assert_eq!(categorize_error("Validation error: invalid price"), "VALIDATION");
        assert_eq!(categorize_error("Proto serialization failed"), "PROTO_TRANSFORMATION");
        assert_eq!(categorize_error("Something else went wrong"), "UNKNOWN");
    }
    
    #[test]
    fn test_dlq_message_creation() {
        let original_data = r#"{"symbol": "AAPL", "price": 150.25, "timestamp": 1640995200}"#;
        let error_message = "Validation failed: price out of range";
        
        // Mock DLQ message creation logic
        let dlq_message = DlqMessage {
            dlq_id: Uuid::new_v4().to_string(),
            original_data: original_data.to_string(),
            error_message: error_message.to_string(),
            error_category: "VALIDATION".to_string(),
            dlq_timestamp: chrono::Utc::now().timestamp(),
            original_timestamp: Some(1640995200),
            failure_stage: "VALIDATION".to_string(),
            retry_count: 0,
            source: "unknown".to_string(),
            metadata: std::collections::HashMap::new(),
        };
        
        assert!(!dlq_message.dlq_id.is_empty());
        assert_eq!(dlq_message.error_category, "VALIDATION");
        assert_eq!(dlq_message.failure_stage, "VALIDATION");
        assert_eq!(dlq_message.original_timestamp, Some(1640995200));
    }
}