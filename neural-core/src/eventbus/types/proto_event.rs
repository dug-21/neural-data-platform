//! Proto-only Event types for EventBus Phase 4
//!
//! CRITICAL: This module enforces proto-only messaging. ALL Vec<u8> payloads are REJECTED.

use prost::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::eventbus::{
    error::EventBusError,
    types::EventId,
};

/// Proto-only trait that ALL EventBus messages MUST implement
pub trait ProtoMessage: Message + Clone + Send + Sync + 'static {
    /// Get the protobuf message type name for routing
    fn proto_type_name() -> &'static str;
    
    /// Validate the proto message against business rules
    fn validate(&self) -> Result<(), EventBusError> {
        // Default implementation allows all valid proto messages
        Ok(())
    }
}

/// Proto-only Event container - ZERO Vec<u8> support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtoEvent<T: ProtoMessage> {
    /// Event type identifier (derived from proto type)
    pub event_type: String,
    
    /// Protocol buffer message (strongly typed)
    pub message: T,
    
    /// Event metadata container
    pub metadata: ProtoEventMetadata,
    
    /// Unix timestamp when the event was created
    pub timestamp: i64,
    
    /// Quality score from Data-Staging service
    pub quality_score: f64,
}

/// Metadata container for proto events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtoEventMetadata {
    /// Additional metadata as key-value pairs
    pub headers: HashMap<String, String>,
    
    /// Event creation timestamp
    pub created_at: i64,
    
    /// Event ingestion timestamp
    pub ingested_at: Option<i64>,
    
    /// Source system identifier
    pub source: String,
    
    /// Correlation ID for tracing
    pub correlation_id: Option<String>,
}

impl<T: ProtoMessage> ProtoEvent<T> {
    /// Create a new proto event
    pub fn new(message: T) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            event_type: T::proto_type_name().to_string(),
            message,
            metadata: ProtoEventMetadata {
                headers: HashMap::new(),
                created_at: now,
                ingested_at: None,
                source: "neural-core".to_string(),
                correlation_id: None,
            },
            timestamp: now,
            quality_score: 1.0,
        }
    }
    
    /// Encode the proto message to bytes
    pub fn encode_to_vec(&self) -> Result<Vec<u8>, EventBusError> {
        use prost::Message;
        let mut buf = Vec::new();
        self.message.encode(&mut buf)
            .map_err(|e| EventBusError::Serialization(format!("Failed to encode proto message: {}", e)))?;
        Ok(buf)
    }
    
    /// Set source
    pub fn with_source(mut self, source: String) -> Self {
        self.metadata.source = source;
        self
    }
    
    /// Set correlation ID
    pub fn with_correlation_id(mut self, correlation_id: String) -> Self {
        self.metadata.correlation_id = Some(correlation_id);
        self
    }
    
    /// Create a proto event with metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.headers.insert(key, value);
        self
    }
    
    /// Set custom timestamp
    pub fn with_timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = timestamp;
        self
    }
    
    /// Set quality score from Data-Staging
    pub fn with_quality_score(mut self, score: f64) -> Self {
        self.quality_score = score;
        self
    }
    
    /// Validate the event (message validation + metadata checks)
    pub fn validate(&self) -> Result<(), EventBusError> {
        // Validate the proto message first
        self.message.validate()?;
        
        // Validate quality score
        if self.quality_score < 0.0 || self.quality_score > 1.0 {
            return Err(EventBusError::schema_validation(
                "Quality score must be between 0.0 and 1.0"
            ));
        }
        
        // Validate timestamp is reasonable (not too far in future/past)
        let now = chrono::Utc::now().timestamp();
        let day_in_seconds = 86400;
        
        if self.timestamp < now - (30 * day_in_seconds) || 
           self.timestamp > now + day_in_seconds {
            return Err(EventBusError::schema_validation(
                "Event timestamp is outside reasonable range"
            ));
        }
        
        Ok(())
    }
    
    /// Serialize to bytes (proto format ONLY)
    pub fn to_proto_bytes(&self) -> Result<Vec<u8>, EventBusError> {
        Ok(self.message.encode_to_vec())
    }
    
    /// Get the proto type name
    pub fn proto_type_name(&self) -> &str {
        T::proto_type_name()
    }
}

/// Proto-only Event Envelope for EventBus transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtoEventEnvelope {
    /// Unique identifier for this event
    pub event_id: EventId,
    
    /// Channel this event was delivered from
    pub channel: String,
    
    /// Proto message type name
    pub proto_type: String,
    
    /// Serialized protobuf bytes
    pub proto_bytes: Vec<u8>,
    
    /// Event metadata
    pub metadata: HashMap<String, String>,
    
    /// Quality score from Data-Staging
    pub quality_score: f64,
    
    /// Number of retry attempts
    pub retry_count: u32,
    
    /// Unix timestamp when the event was created
    pub created_at: i64,
    
    /// Unix timestamp when the event was delivered
    pub delivered_at: i64,
}

impl ProtoEventEnvelope {
    /// Create a new proto event envelope
    pub fn new<T: ProtoMessage>(
        event_id: EventId, 
        channel: String, 
        event: ProtoEvent<T>
    ) -> Result<Self, EventBusError> {
        // MANDATORY: Validate the proto event before creating envelope
        event.validate()?;
        
        let proto_bytes = event.to_proto_bytes()?;
        
        Ok(Self {
            event_id,
            channel,
            proto_type: event.event_type,
            proto_bytes,
            metadata: event.metadata.headers,
            quality_score: event.quality_score,
            retry_count: 0,
            created_at: event.timestamp,
            delivered_at: chrono::Utc::now().timestamp(),
        })
    }
    
    /// Deserialize the proto message to a specific type
    pub fn deserialize_proto<T: ProtoMessage + Default>(&self) -> Result<ProtoEvent<T>, EventBusError> {
        // MANDATORY: Type safety check
        if self.proto_type != T::proto_type_name() {
            return Err(EventBusError::contract_violation(format!(
                "Proto type mismatch: expected {}, got {}", 
                T::proto_type_name(), 
                self.proto_type
            )));
        }
        
        let message = T::decode(&self.proto_bytes[..]).map_err(|e| {
            EventBusError::proto_deserialization(format!("Failed to deserialize proto: {}", e))
        })?;
        
        let event = ProtoEvent {
            event_type: self.proto_type.clone(),
            message,
            metadata: ProtoEventMetadata { 
                headers: self.metadata.clone(), 
                created_at: self.created_at, 
                ingested_at: Some(self.delivered_at), 
                source: "eventbus".to_string(), 
                correlation_id: None 
            },
            timestamp: self.created_at,
            quality_score: self.quality_score,
        };
        
        // MANDATORY: Validate deserialized event
        event.validate()?;
        
        Ok(event)
    }
    
    /// Increment retry count
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
        self.delivered_at = chrono::Utc::now().timestamp();
    }
    
    /// Check if proto type matches expected type
    pub fn is_proto_type<T: ProtoMessage>(&self) -> bool {
        self.proto_type == T::proto_type_name()
    }
}

/// Type-erased proto event for dynamic handling
#[derive(Debug, Clone)]
pub struct DynamicProtoEvent {
    /// Event type identifier
    pub event_type: String,
    
    /// Serialized protobuf bytes
    pub proto_bytes: Vec<u8>,
    
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    
    /// Unix timestamp when the event was created
    pub timestamp: i64,
    
    /// Quality score from Data-Staging
    pub quality_score: f64,
}

impl DynamicProtoEvent {
    /// Create from a strongly-typed proto event
    pub fn from_proto_event<T: ProtoMessage>(event: ProtoEvent<T>) -> Result<Self, EventBusError> {
        let proto_bytes = event.to_proto_bytes()?;
        
        Ok(Self {
            event_type: event.event_type,
            proto_bytes,
            metadata: event.metadata.headers,
            timestamp: event.timestamp,
            quality_score: event.quality_score,
        })
    }
    
    /// Convert to strongly-typed proto event
    pub fn to_proto_event<T: ProtoMessage + Default>(&self) -> Result<ProtoEvent<T>, EventBusError> {
        if self.event_type != T::proto_type_name() {
            return Err(EventBusError::contract_violation(format!(
                "Proto type mismatch: expected {}, got {}", 
                T::proto_type_name(), 
                self.event_type
            )));
        }
        
        let message = T::decode(&self.proto_bytes[..]).map_err(|e| {
            EventBusError::proto_deserialization(format!("Failed to deserialize proto: {}", e))
        })?;
        
        let event = ProtoEvent {
            event_type: self.event_type.clone(),
            message,
            metadata: ProtoEventMetadata { headers: self.metadata.clone(), created_at: self.timestamp, ingested_at: None, source: "eventbus".to_string(), correlation_id: None },
            timestamp: self.timestamp,
            quality_score: self.quality_score,
        };
        
        // MANDATORY: Validate
        event.validate()?;
        
        Ok(event)
    }
    
    /// Validate proto bytes can be decoded
    pub fn validate_proto_bytes<T: ProtoMessage + Default>(&self) -> Result<(), EventBusError> {
        T::decode(&self.proto_bytes[..]).map_err(|e| {
            EventBusError::schema_validation(format!("Invalid proto bytes for {}: {}", T::proto_type_name(), e))
        })?;
        
        Ok(())
    }
}

/// Contract violation helper - REJECTS Vec<u8> payloads
pub fn reject_raw_payload() -> EventBusError {
    EventBusError::contract_violation(
        "Contract violation: Only protobuf messages are allowed. Vec<u8> payloads are REJECTED. \
         Use Data-Staging service to convert JSON to proto messages."
    )
}

/// Contract violation helper - REJECTS JSON payloads  
pub fn reject_json_payload() -> EventBusError {
    EventBusError::contract_violation(
        "Contract violation: JSON messages are not allowed in EventBus. \
         Use Data-Staging service to convert JSON to proto messages first."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Mock proto message for testing
    #[derive(Clone, prost::Message)]
    pub struct MockProtoMessage {
        #[prost(string, tag = "1")]
        pub content: String,
        #[prost(int64, tag = "2")]
        pub value: i64,
    }
    
    impl ProtoMessage for MockProtoMessage {
        fn proto_type_name() -> &'static str {
            "test.MockProtoMessage"
        }
        
        fn validate(&self) -> Result<(), EventBusError> {
            if self.content.is_empty() {
                return Err(EventBusError::schema_validation("Content cannot be empty"));
            }
            Ok(())
        }
    }
    
    #[test]
    fn test_proto_event_creation() {
        let message = MockProtoMessage {
            content: "test content".to_string(),
            value: 42,
        };
        
        let event = ProtoEvent::new(message.clone())
            .with_metadata("key".to_string(), "value".to_string())
            .with_quality_score(0.95);
        
        assert_eq!(event.event_type, "test.MockProtoMessage");
        assert_eq!(event.message.content, "test content");
        assert_eq!(event.message.value, 42);
        assert_eq!(event.quality_score, 0.95);
        assert!(event.metadata.headers.contains_key("key"));
    }
    
    #[test]
    fn test_proto_event_validation() {
        let valid_message = MockProtoMessage {
            content: "test".to_string(),
            value: 42,
        };
        let valid_event = ProtoEvent::new(valid_message);
        assert!(valid_event.validate().is_ok());
        
        let invalid_message = MockProtoMessage {
            content: "".to_string(),
            value: 42,
        };
        let invalid_event = ProtoEvent::new(invalid_message);
        assert!(invalid_event.validate().is_err());
    }
    
    #[test]
    fn test_proto_event_envelope_serialization() {
        let message = MockProtoMessage {
            content: "test".to_string(),
            value: 42,
        };
        let event = ProtoEvent::new(message);
        let event_id = EventId::new();
        let channel = "test-channel".to_string();
        
        let envelope = ProtoEventEnvelope::new(event_id.clone(), channel.clone(), event);
        assert!(envelope.is_ok());
        
        let envelope = envelope.unwrap();
        assert_eq!(envelope.event_id, event_id);
        assert_eq!(envelope.channel, channel);
        assert_eq!(envelope.proto_type, "test.MockProtoMessage");
    }
    
    #[test]
    fn test_proto_event_envelope_deserialization() {
        let message = MockProtoMessage {
            content: "test".to_string(),
            value: 42,
        };
        let event = ProtoEvent::new(message);
        let envelope = ProtoEventEnvelope::new(
            EventId::new(), 
            "test-channel".to_string(), 
            event
        ).unwrap();
        
        let deserialized = envelope.deserialize_proto::<MockProtoMessage>();
        assert!(deserialized.is_ok());
        
        let deserialized = deserialized.unwrap();
        assert_eq!(deserialized.message.content, "test");
        assert_eq!(deserialized.message.value, 42);
    }
    
    #[test]
    fn test_contract_violation_rejection() {
        let error = reject_raw_payload();
        assert!(matches!(error, EventBusError::ContractViolation(_)));
        assert!(error.to_string().contains("Vec<u8> payloads are REJECTED"));
        
        let error = reject_json_payload();
        assert!(matches!(error, EventBusError::ContractViolation(_)));
        assert!(error.to_string().contains("JSON messages are not allowed"));
    }
}