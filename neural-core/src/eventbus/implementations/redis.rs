use async_trait::async_trait;
use redis::aio::MultiplexedConnection;
use redis::{RedisResult, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::eventbus::{
    traits::{EventBus, ProtoEventSubscriber, DynamicProtoEventSubscriber},
    types::{ProtoMessage, ProtoEvent, ProtoEventEnvelope, EventId, SubscriptionConfig},
    error::EventBusError,
};
use crate::eventbus::traits::ProtoChannelInfo;

/// Redis Streams implementation of EventBus for production use
pub struct RedisEventBus {
    client: redis::Client,
    connection: Arc<RwLock<MultiplexedConnection>>,
    channel_configs: Arc<RwLock<HashMap<String, ChannelConfig>>>,
}

#[derive(Debug, Clone)]
struct ChannelConfig {
    max_length: Option<usize>,
    ttl_seconds: Option<u64>,
}

impl RedisEventBus {
    pub async fn new(redis_url: &str) -> Result<Self, EventBusError> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| EventBusError::Backend(format!("Failed to create Redis client: {}", e)))?;
        
        let connection = client.get_multiplexed_async_connection().await
            .map_err(|e| EventBusError::Backend(format!("Failed to connect to Redis: {}", e)))?;
        
        Ok(Self {
            client,
            connection: Arc::new(RwLock::new(connection)),
            channel_configs: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    fn convert_channel_name(&self, channel: &str) -> String {
        // Convert from new format to Redis key format
        // stream:symbol:AAPL becomes redis:stream:symbol:AAPL
        if channel.starts_with("stream:") {
            format!("redis:{}", channel)
        } else {
            // Handle legacy format migration
            if channel.starts_with("market:") {
                let symbol = channel.strip_prefix("market:").unwrap_or("");
                format!("redis:stream:symbol:{}", symbol)
            } else {
                format!("redis:stream:unknown:{}", channel)
            }
        }
    }

    async fn xadd_with_maxlen(
        &self,
        key: &str,
        items: &[(&str, Vec<u8>)],
    ) -> Result<String, EventBusError> {
        let mut conn = self.connection.write().await;
        
        // Use XADD with automatic ID generation (*)
        let result: RedisResult<String> = redis::cmd("XADD")
            .arg(key)
            .arg("MAXLEN")
            .arg("~")  // Approximate maxlen for performance
            .arg(100000)  // Keep last 100k messages
            .arg("*")  // Auto-generate ID
            .arg(items[0].0)
            .arg(&items[0].1)
            .query_async(&mut *conn)
            .await;
        
        result.map_err(|e| EventBusError::Backend(format!("Redis XADD failed: {}", e)))
    }
}

#[async_trait]
impl EventBus for RedisEventBus {
    async fn publish<T: ProtoMessage + Default>(
        &self,
        channel: &str,
        event: ProtoEvent<T>,
    ) -> Result<EventId, EventBusError> {
        if !crate::eventbus::implementations::inmemory::validate_channel_name(channel) {
            return Err(EventBusError::InvalidChannel(format!(
                "Invalid channel name format: {}", channel
            )));
        }

        // Validate the proto event
        event.validate()?;

        let redis_channel = self.convert_channel_name(channel);
        
        // Serialize proto message to bytes
        let proto_bytes = event.to_proto_bytes()?;
        let metadata_json = serde_json::to_vec(&event.metadata)
            .map_err(|e| EventBusError::Serialization(format!("Failed to serialize metadata: {}", e)))?;
        
        // Add to Redis Stream with proto metadata
        let message_id = self.xadd_with_maxlen(
            &redis_channel,
            &[("proto_data", proto_bytes), 
              ("proto_type", T::proto_type_name().as_bytes().to_vec()),
              ("metadata", metadata_json),
              ("quality_score", event.quality_score.to_string().into_bytes()),
              ("timestamp", event.timestamp.to_string().into_bytes())],
        ).await?;
        
        Ok(EventId::from(message_id))
    }

    async fn publish_batch<T: ProtoMessage + Default>(
        &self,
        channel: &str,
        events: Vec<ProtoEvent<T>>,
    ) -> Result<Vec<EventId>, EventBusError> {
        let mut event_ids = Vec::new();
        
        // Use pipeline for batch publish
        let redis_channel = self.convert_channel_name(channel);
        let mut conn = self.connection.write().await;
        let mut pipe = redis::pipe();
        
        for event in &events {
            // Validate each proto event
            event.validate()?;
            
            let proto_bytes = event.to_proto_bytes()?;
            let metadata_json = serde_json::to_vec(&event.metadata)
                .map_err(|e| EventBusError::Serialization(format!("Failed to serialize metadata: {}", e)))?;
            
            pipe.cmd("XADD")
                .arg(&redis_channel)
                .arg("*")
                .arg("proto_data")
                .arg(&proto_bytes)
                .arg("proto_type")
                .arg(T::proto_type_name())
                .arg("metadata")
                .arg(&metadata_json)
                .arg("quality_score")
                .arg(event.quality_score.to_string())
                .arg("timestamp")
                .arg(event.timestamp);
        }
        
        let results: Vec<String> = pipe.query_async(&mut *conn).await
            .map_err(|e| EventBusError::Backend(format!("Batch publish failed: {}", e)))?;
        
        for id in results {
            event_ids.push(EventId::from(id));
        }
        
        Ok(event_ids)
    }

    async fn subscribe<T: ProtoMessage + Default>(
        &self,
        channels: &[String],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn ProtoEventSubscriber<T>>, EventBusError> {
        for channel in channels {
            if !crate::eventbus::implementations::inmemory::validate_channel_name(channel) {
                return Err(EventBusError::InvalidChannel(format!(
                    "Invalid channel name format: {}", channel
                )));
            }
        }

        // Create consumer groups for each channel
        let mut conn = self.connection.write().await;
        for channel in channels {
            let redis_channel = self.convert_channel_name(channel);
            
            // Try to create consumer group, ignore if already exists
            let _: RedisResult<()> = redis::cmd("XGROUP")
                .arg("CREATE")
                .arg(&redis_channel)
                .arg(&config.group_name)
                .arg("$")  // Start from new messages
                .arg("MKSTREAM")  // Create stream if doesn't exist
                .query_async(&mut *conn)
                .await;
        }

        // Convert channels to Redis format
        let redis_channels: Vec<String> = channels.iter()
            .map(|c| self.convert_channel_name(c))
            .collect();

        Ok(Box::new(RedisSubscriber::<T> {
            client: self.client.clone(),
            channels: redis_channels,
            group: config.group_name,
            consumer: config.consumer_name,
            block_timeout_ms: config.block_timeout_ms,
            batch_size: config.batch_size,
            _phantom: std::marker::PhantomData,
        }))
    }

    async fn subscribe_dynamic(
        &self,
        channels: &[String],
        proto_types: &[&'static str],
        config: SubscriptionConfig,
    ) -> Result<Box<dyn DynamicProtoEventSubscriber>, EventBusError> {
        // For Redis, we can implement dynamic subscription by storing proto types
        for channel in channels {
            if !crate::eventbus::implementations::inmemory::validate_channel_name(channel) {
                return Err(EventBusError::InvalidChannel(format!(
                    "Invalid channel name format: {}", channel
                )));
            }
        }

        // Create consumer groups for each channel
        let mut conn = self.connection.write().await;
        for channel in channels {
            let redis_channel = self.convert_channel_name(channel);
            
            // Try to create consumer group, ignore if already exists
            let _: RedisResult<()> = redis::cmd("XGROUP")
                .arg("CREATE")
                .arg(&redis_channel)
                .arg(&config.group_name)
                .arg("$")  // Start from new messages
                .arg("MKSTREAM")  // Create stream if doesn't exist
                .query_async(&mut *conn)
                .await;
        }

        // Convert channels to Redis format
        let redis_channels: Vec<String> = channels.iter()
            .map(|c| self.convert_channel_name(c))
            .collect();

        Ok(Box::new(RedisDynamicSubscriber {
            client: self.client.clone(),
            channels: redis_channels,
            proto_types: proto_types.iter().map(|s| s.to_string()).collect(),
            group: config.group_name,
            consumer: config.consumer_name,
            block_timeout_ms: config.block_timeout_ms,
            batch_size: config.batch_size,
        }))
    }

    async fn ack(
        &self,
        channel: &str,
        group: &str,
        event_id: &EventId,
    ) -> Result<(), EventBusError> {
        let redis_channel = self.convert_channel_name(channel);
        let mut conn = self.connection.write().await;
        
        let _: i64 = redis::cmd("XACK")
            .arg(&redis_channel)
            .arg(group)
            .arg(event_id.to_string())
            .query_async(&mut *conn)
            .await
            .map_err(|e| EventBusError::Backend(format!("Failed to ACK message: {}", e)))?;
        
        Ok(())
    }

    async fn nack(
        &self,
        channel: &str,
        group: &str,
        event_id: &EventId,
    ) -> Result<(), EventBusError> {
        // Redis doesn't have explicit NACK, but we can use XCLAIM with idle time 0
        // to make the message available for redelivery
        let redis_channel = self.convert_channel_name(channel);
        let mut conn = self.connection.write().await;
        
        let _: Value = redis::cmd("XCLAIM")
            .arg(&redis_channel)
            .arg(group)
            .arg("pending-consumer")  // Temporary consumer to release message
            .arg(0)  // Min idle time
            .arg(event_id.to_string())
            .query_async(&mut *conn)
            .await
            .map_err(|e| EventBusError::Backend(format!("Failed to NACK message: {}", e)))?;
        
        Ok(())
    }

    async fn create_consumer_group(
        &self,
        channel: &str,
        group: &str,
    ) -> Result<(), EventBusError> {
        let redis_channel = self.convert_channel_name(channel);
        let mut conn = self.connection.write().await;
        
        let result: RedisResult<()> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(&redis_channel)
            .arg(group)
            .arg("0")  // Start from beginning
            .arg("MKSTREAM")
            .query_async(&mut *conn)
            .await;
        
        match result {
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("BUSYGROUP") => {
                // Group already exists
                Ok(())
            }
            Err(e) => Err(EventBusError::Backend(format!("Failed to create consumer group: {}", e))),
        }
    }

    async fn get_channel_info(&self, channel: &str) -> Result<ProtoChannelInfo, EventBusError> {
        let redis_channel = self.convert_channel_name(channel);
        let mut conn = self.connection.write().await;
        
        // Get stream info
        let info: HashMap<String, Value> = redis::cmd("XINFO")
            .arg("STREAM")
            .arg(&redis_channel)
            .query_async(&mut *conn)
            .await
            .map_err(|e| EventBusError::Backend(format!("Failed to get channel info: {}", e)))?;
        
        // Parse the info
        let length = info.get("length")
            .and_then(|v| if let Value::Int(i) = v { Some(*i as u64) } else { None })
            .unwrap_or(0);
        
        let last_id = info.get("last-generated-id")
            .and_then(|v| if let Value::Data(d) = v { 
                String::from_utf8(d.clone()).ok() 
            } else { None })
            .map(EventId::from);
        
        // Get consumer groups
        let groups_result: Result<Vec<HashMap<String, Value>>, _> = redis::cmd("XINFO")
            .arg("GROUPS")
            .arg(&redis_channel)
            .query_async(&mut *conn)
            .await;
            
        let consumer_groups: Vec<String> = match groups_result {
            Ok(groups) => {
                groups.iter()
                    .filter_map(|g| {
                        g.get("name").and_then(|v| {
                            if let Value::Data(d) = v {
                                String::from_utf8(d.clone()).ok()
                            } else { None }
                        })
                    })
                    .collect()
            }
            Err(_) => Vec::new(), // No groups or stream doesn't exist
        };
        
        Ok(ProtoChannelInfo {
            channel_name: channel.to_string(),
            message_count: length,
            proto_type_counts: std::collections::HashMap::new(), // Would need to track proto types
            consumer_groups,
            last_event_id: last_id,
            avg_quality_score: 1.0, // Default quality score
            created_at: chrono::Utc::now().timestamp(),
            subscriber_count: 0,
            total_events: length,
            active: length > 0,
        })
    }

    async fn list_proto_types_on_channel(&self, channel: &str) -> Result<Vec<String>, EventBusError> {
        // For Redis, we'd need to scan through messages to find proto types
        // This is a placeholder implementation
        let _redis_channel = self.convert_channel_name(channel);
        Ok(vec!["unknown.ProtoType".to_string()])
    }

    async fn list_channels(&self) -> Result<Vec<String>, EventBusError> {
        let mut conn = self.connection.write().await;
        
        // Get all keys matching the Redis stream pattern
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg("redis:stream:*")
            .query_async(&mut *conn)
            .await
            .map_err(|e| EventBusError::Backend(format!("Failed to list channels: {}", e)))?;
        
        // Convert Redis keys back to channel names
        let channels = keys.iter()
            .filter_map(|key| {
                if let Some(stripped) = key.strip_prefix("redis:") {
                    Some(stripped.to_string())
                } else {
                    None
                }
            })
            .collect();
        
        Ok(channels)
    }

    async fn channel_subscriber_count(&self, channel: &str) -> Result<usize, EventBusError> {
        let redis_channel = self.convert_channel_name(channel);
        let mut conn = self.connection.write().await;
        
        // Get consumer group info to count active consumers
        let groups_result: Result<Vec<HashMap<String, Value>>, _> = redis::cmd("XINFO")
            .arg("GROUPS")
            .arg(&redis_channel)
            .query_async(&mut *conn)
            .await;
        
        match groups_result {
            Ok(groups) => {
                let mut total_consumers = 0;
                for group in groups {
                    if let Some(consumers_info) = group.get("consumers") {
                        if let Value::Int(count) = consumers_info {
                            total_consumers += *count as usize;
                        }
                    }
                }
                Ok(total_consumers)
            }
            Err(_) => Ok(0), // No groups or stream doesn't exist
        }
    }
}

pub struct RedisSubscriber<T: ProtoMessage> {
    client: redis::Client,
    channels: Vec<String>,
    group: String,
    consumer: String,
    block_timeout_ms: u64,
    batch_size: usize,
    _phantom: std::marker::PhantomData<T>,
}

#[async_trait]
impl<T: ProtoMessage + Default> ProtoEventSubscriber<T> for RedisSubscriber<T> {
    async fn next_proto(&mut self) -> Result<Option<ProtoEvent<T>>, EventBusError> {
        if let Some(envelope) = self.next_proto_envelope().await? {
            let proto_event = envelope.deserialize_proto::<T>()?;
            Ok(Some(proto_event))
        } else {
            Ok(None)
        }
    }

    async fn next_proto_envelope(&mut self) -> Result<Option<ProtoEventEnvelope>, EventBusError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| EventBusError::Backend(format!("Failed to get connection: {}", e)))?;
        
        // Build XREADGROUP command
        let mut cmd = redis::cmd("XREADGROUP");
        cmd.arg("GROUP")
            .arg(&self.group)
            .arg(&self.consumer)
            .arg("COUNT")
            .arg(1)  // Get one message at a time
            .arg("BLOCK")
            .arg(self.block_timeout_ms)
            .arg("STREAMS");
        
        // Add all channels
        for channel in &self.channels {
            cmd.arg(channel);
        }
        
        // Add ">" for each channel to read new messages
        for _ in &self.channels {
            cmd.arg(">");
        }
        
        let result: RedisResult<Vec<(String, Vec<(String, HashMap<String, Vec<u8>>)>)>> = 
            cmd.query_async(&mut conn).await;
        
        match result {
            Ok(streams) => {
                for (channel, messages) in streams {
                    for (id, fields) in messages {
                        if let (Some(proto_type), Some(proto_data)) = 
                            (fields.get("proto_type"), fields.get("proto_data")) {
                            
                            let proto_type_str = String::from_utf8_lossy(proto_type);
                            let metadata_bytes = fields.get("metadata").cloned().unwrap_or_default();
                            let metadata = if !metadata_bytes.is_empty() {
                                serde_json::from_slice(&metadata_bytes).unwrap_or_default()
                            } else {
                                std::collections::HashMap::new()
                            };
                            
                            let quality_score = fields.get("quality_score")
                                .and_then(|bytes| String::from_utf8_lossy(bytes).parse().ok())
                                .unwrap_or(1.0);
                            
                            let created_at = fields.get("timestamp")
                                .and_then(|bytes| String::from_utf8_lossy(bytes).parse().ok())
                                .unwrap_or_else(|| chrono::Utc::now().timestamp());
                            
                            return Ok(Some(ProtoEventEnvelope {
                                event_id: EventId::from(id),
                                channel: channel.replace("redis:", ""),
                                proto_type: proto_type_str.to_string(),
                                proto_bytes: proto_data.clone(),
                                metadata,
                                quality_score,
                                retry_count: 0,
                                created_at,
                                delivered_at: chrono::Utc::now().timestamp(),
                            }));
                        }
                    }
                }
                Ok(None) // No messages available
            }
            Err(e) => Err(EventBusError::Backend(format!("Failed to read messages: {}", e))),
        }
    }

    async fn close(&mut self) -> Result<(), EventBusError> {
        // Redis connections are handled by the connection pool
        Ok(())
    }

    fn id(&self) -> &str {
        &self.consumer
    }
}

pub struct RedisDynamicSubscriber {
    client: redis::Client,
    channels: Vec<String>,
    proto_types: Vec<String>,
    group: String,
    consumer: String,
    block_timeout_ms: u64,
    batch_size: usize,
}

#[async_trait]
impl DynamicProtoEventSubscriber for RedisDynamicSubscriber {
    async fn next_dynamic_proto(&mut self) -> Result<Option<ProtoEventEnvelope>, EventBusError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| EventBusError::Backend(format!("Failed to get connection: {}", e)))?;
        
        // Build XREADGROUP command
        let mut cmd = redis::cmd("XREADGROUP");
        cmd.arg("GROUP")
            .arg(&self.group)
            .arg(&self.consumer)
            .arg("COUNT")
            .arg(1)
            .arg("BLOCK")
            .arg(self.block_timeout_ms)
            .arg("STREAMS");
        
        // Add all channels
        for channel in &self.channels {
            cmd.arg(channel);
        }
        
        // Add ">" for each channel to read new messages
        for _ in &self.channels {
            cmd.arg(">");
        }
        
        let result: RedisResult<Vec<(String, Vec<(String, HashMap<String, Vec<u8>>)>)>> = 
            cmd.query_async(&mut conn).await;
        
        match result {
            Ok(streams) => {
                for (channel, messages) in streams {
                    for (id, fields) in messages {
                        if let (Some(proto_type), Some(proto_data)) = 
                            (fields.get("proto_type"), fields.get("proto_data")) {
                            
                            let proto_type_str = String::from_utf8_lossy(proto_type);
                            
                            // Filter by supported proto types
                            if !self.proto_types.is_empty() && !self.proto_types.contains(&proto_type_str.to_string()) {
                                continue;
                            }
                            
                            let metadata_bytes = fields.get("metadata").cloned().unwrap_or_default();
                            let metadata = if !metadata_bytes.is_empty() {
                                serde_json::from_slice(&metadata_bytes).unwrap_or_default()
                            } else {
                                std::collections::HashMap::new()
                            };
                            
                            let quality_score = fields.get("quality_score")
                                .and_then(|bytes| String::from_utf8_lossy(bytes).parse().ok())
                                .unwrap_or(1.0);
                            
                            let created_at = fields.get("timestamp")
                                .and_then(|bytes| String::from_utf8_lossy(bytes).parse().ok())
                                .unwrap_or_else(|| chrono::Utc::now().timestamp());
                            
                            return Ok(Some(ProtoEventEnvelope {
                                event_id: EventId::from(id),
                                channel: channel.replace("redis:", ""),
                                proto_type: proto_type_str.to_string(),
                                proto_bytes: proto_data.clone(),
                                metadata,
                                quality_score,
                                retry_count: 0,
                                created_at,
                                delivered_at: chrono::Utc::now().timestamp(),
                            }));
                        }
                    }
                }
                Ok(None)
            }
            Err(e) => Err(EventBusError::Backend(format!("Failed to read messages: {}", e))),
        }
    }

    async fn filter_proto_types(&mut self, types: &[&str]) -> Result<(), EventBusError> {
        self.proto_types = types.iter().map(|s| s.to_string()).collect();
        Ok(())
    }

    fn supported_proto_types(&self) -> &[String] {
        &self.proto_types
    }

    async fn close(&mut self) -> Result<(), EventBusError> {
        Ok(())
    }

    fn id(&self) -> &str {
        &self.consumer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_name_conversion() {
        let event_bus = RedisEventBus {
            client: redis::Client::open("redis://127.0.0.1/").unwrap(),
            connection: Arc::new(RwLock::new(unsafe { std::mem::zeroed() })),
            channel_configs: Arc::new(RwLock::new(HashMap::new())),
        };
        
        assert_eq!(
            event_bus.convert_channel_name("stream:symbol:AAPL"),
            "redis:stream:symbol:AAPL"
        );
        assert_eq!(
            event_bus.convert_channel_name("market:AAPL"),
            "redis:stream:symbol:AAPL"
        );
        assert_eq!(
            event_bus.convert_channel_name("unknown"),
            "redis:stream:unknown:unknown"
        );
    }

    // Integration tests would require a Redis instance
    // Use testcontainers or similar in real implementation
}