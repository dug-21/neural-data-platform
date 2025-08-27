//! Redis Stream Consumer - Ingests raw JSON data from Redis streams
//! 
//! This module handles consuming raw JSON data from Redis streams with
//! proper consumer group management, acknowledgment, and error handling.

use redis::{Client, Connection, Commands, streams::StreamReadReply};
use redis::aio::MultiplexedConnection;
use async_trait::async_trait;
use anyhow::{Result, Context};
use std::collections::HashMap;

use crate::{DataStagingConfig, DataStagingError};

/// Message from Redis stream
#[derive(Debug, Clone)]
pub struct RedisMessage {
    pub id: String,
    pub data: String,
    pub timestamp: i64,
}

/// Redis stream consumer for raw JSON data
pub struct RedisConsumer {
    client: Client,
    connection: MultiplexedConnection,
    stream_name: String,
    consumer_group: String,
    consumer_name: String,
    batch_size: usize,
}

impl RedisConsumer {
    pub async fn new(config: &DataStagingConfig) -> Result<Self> {
        let client = Client::open(config.redis_url.as_str())
            .context("Failed to create Redis client")?;
        
        let connection = client.get_multiplexed_async_connection().await
            .context("Failed to establish Redis connection")?;
        
        let mut consumer = Self {
            client: client.clone(),
            connection,
            stream_name: config.input_stream.clone(),
            consumer_group: config.consumer_group.clone(),
            consumer_name: config.consumer_name.clone(),
            batch_size: config.processing_limits.max_batch_size,
        };
        
        // Create consumer group if it doesn't exist
        consumer.ensure_consumer_group().await?;
        
        Ok(consumer)
    }
    
    async fn ensure_consumer_group(&mut self) -> Result<()> {
        let result: Result<(), redis::RedisError> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(&self.stream_name)
            .arg(&self.consumer_group)
            .arg("$") // Start from latest messages
            .arg("MKSTREAM") // Create stream if doesn't exist
            .query_async(&mut self.connection)
            .await;
            
        match result {
            Ok(_) => {
                tracing::info!("Created consumer group {} for stream {}", 
                    self.consumer_group, self.stream_name);
                Ok(())
            }
            Err(e) if e.to_string().contains("BUSYGROUP") => {
                // Consumer group already exists
                tracing::debug!("Consumer group {} already exists", self.consumer_group);
                Ok(())
            }
            Err(e) => Err(DataStagingError::Redis(e).into())
        }
    }
    
    pub async fn consume_batch(&mut self) -> Result<Vec<RedisMessage>> {
        let result: Vec<StreamReadReply> = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(&self.consumer_group)
            .arg(&self.consumer_name)
            .arg("COUNT")
            .arg(self.batch_size)
            .arg("BLOCK")
            .arg(5000) // 5 second timeout
            .arg("STREAMS")
            .arg(&self.stream_name)
            .arg(">") // Read new messages
            .query_async(&mut self.connection)
            .await
            .context("Failed to read from Redis stream")?;
        
        let mut messages = Vec::new();
        
        for stream_reply in result {
            for stream_key in stream_reply.keys {
                for stream_msg in stream_key.ids {
                    if let Some(data_bytes) = stream_msg.map.get("data") {
                        if let redis::Value::Data(bytes) = data_bytes {
                            match std::str::from_utf8(bytes) {
                            Ok(json_data) => {
                                messages.push(RedisMessage {
                                    id: stream_msg.id,
                                    data: json_data.to_string(),
                                    timestamp: chrono::Utc::now().timestamp(),
                                });
                            }
                                Err(e) => {
                                    tracing::warn!("Failed to decode message data as UTF-8: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(messages)
    }
    
    pub async fn acknowledge_message(&mut self, message_id: &str) -> Result<()> {
        let _: i64 = redis::cmd("XACK")
            .arg(&self.stream_name)
            .arg(&self.consumer_group)
            .arg(message_id)
            .query_async(&mut self.connection)
            .await
            .context("Failed to acknowledge message")?;
            
        Ok(())
    }
    
    pub async fn get_pending_count(&mut self) -> Result<usize> {
        let result: HashMap<String, Vec<(String, String, i64, u64)>> = redis::cmd("XPENDING")
            .arg(&self.stream_name)
            .arg(&self.consumer_group)
            .query_async(&mut self.connection)
            .await
            .context("Failed to get pending message count")?;
            
        Ok(result.values().map(|v| v.len()).sum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataStagingConfig;
    
    #[tokio::test]
    #[ignore] // Requires Redis instance
    async fn test_redis_consumer_basic() {
        let config = DataStagingConfig::default();
        let mut consumer = RedisConsumer::new(&config).await.unwrap();
        
        // Should be able to consume (even if no messages)
        let messages = consumer.consume_batch().await.unwrap();
        assert!(messages.len() <= config.processing_limits.max_batch_size);
    }
}