//! Proto-only Event implementation for Neural Trader EventBus Phase 4
//!
//! CRITICAL: This module enforces proto-only messaging. ALL Vec<u8> payloads are REJECTED.
//! Only protobuf messages wrapped in EventEnvelope are supported.

use prost::Message;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};

// Import the generated proto types
use crate::eventbus::proto_messages;

/// Proto-only Event wrapper around EventEnvelope
/// 
/// This is the ONLY Event type supported in Phase 4. All events MUST:
/// 1. Use protobuf messages for payload
/// 2. Be wrapped in EventEnvelope from ingestion-eventbus.proto
/// 3. Reject any Vec<u8> payloads with ContractViolation errors
#[derive(Debug, Clone)]
pub struct Event {
    /// Inner EventEnvelope containing all event data
    inner: proto_messages::EventEnvelope,
}

impl Event {
    /// Create a new Event from a protobuf message
    /// 
    /// This is the ONLY constructor allowed - proto messages only!
    pub fn new<T: Message + Clone>(
        event_type: &str,
        payload: T,
        source: &str,
        domain: &str,
    ) -> Result<Self, crate::errors::CoreError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        
        let timestamp = prost_types::Timestamp {
            seconds: now.as_secs() as i64,
            nanos: now.subsec_nanos() as i32,
        };
        
        // Serialize the payload to proto bytes
        let payload_bytes = payload.encode_to_vec();
        let payload_any = prost_types::Any {
            type_url: format!("type.googleapis.com/{}", event_type),
            value: payload_bytes,
        };
        
        let envelope = proto_messages::EventEnvelope {
            message_id: Uuid::new_v4().to_string(),
            correlation_id: String::new(),
            source: source.to_string(),
            domain: domain.to_string(),
            event_type: event_type.to_string(),
            schema_version: "v1".to_string(),
            created_at: Some(timestamp.clone()),
            ingested_at: Some(timestamp),
            routing: Some(proto_messages::RoutingMetadata {
                topic: format!("{}.{}", domain, event_type),
                partition_key: String::new(),
                priority: 5,
                ttl_seconds: 0,
                tags: vec![],
                retry_policy: Some(proto_messages::RetryPolicy {
                    max_attempts: 3,
                    initial_delay_ms: 1000,
                    backoff_multiplier: 2.0,
                    max_delay_ms: 30000,
                    retryable_errors: vec![],
                }),
            }),
            quality: Some(proto_messages::QualityMetadata {
                completeness: 100.0,
                latency_ms: 0,
                validation_status: proto_messages::ValidationStatus::ValidationStatusPassed as i32,
                quality_score: 100.0,
                anomalies: vec![],
            }),
            payload: Some(payload_any),
            headers: HashMap::new(),
            tracing: Some(proto_messages::TracingContext {
                trace_id: Uuid::new_v4().to_string(),
                span_id: Uuid::new_v4().to_string(),
                parent_span_id: String::new(),
                baggage: HashMap::new(),
            }),
        };
        
        Ok(Self { inner: envelope })
    }
    
    /// Create an Event with custom correlation ID
    pub fn with_correlation_id(mut self, correlation_id: &str) -> Self {
        self.inner.correlation_id = correlation_id.to_string();
        self
    }
    
    /// Add routing metadata
    pub fn with_routing(mut self, topic: &str, partition_key: &str, priority: i32) -> Self {
        if let Some(routing) = &mut self.inner.routing {
            routing.topic = topic.to_string();
            routing.partition_key = partition_key.to_string();
            routing.priority = priority;
        }
        self
    }
    
    /// Add header metadata
    pub fn with_header(mut self, key: &str, value: &str) -> Self {
        self.inner.headers.insert(key.to_string(), value.to_string());
        self
    }
    
    /// Set quality metadata
    pub fn with_quality(mut self, completeness: f32, quality_score: f32) -> Self {
        if let Some(quality) = &mut self.inner.quality {
            quality.completeness = completeness;
            quality.quality_score = quality_score;
        }
        self
    }
    
    /// Get the event type
    pub fn event_type(&self) -> &str {
        &self.inner.event_type
    }
    
    /// Get the message ID
    pub fn message_id(&self) -> &str {
        &self.inner.message_id
    }
    
    /// Get the correlation ID
    pub fn correlation_id(&self) -> &str {
        &self.inner.correlation_id
    }
    
    /// Get the source
    pub fn source(&self) -> &str {
        &self.inner.source
    }
    
    /// Get the domain
    pub fn domain(&self) -> &str {
        &self.inner.domain
    }
    
    /// Get the created timestamp
    pub fn created_at(&self) -> Option<DateTime<Utc>> {
        self.inner.created_at.as_ref().map(|ts| {
            DateTime::from_timestamp(ts.seconds, ts.nanos as u32).unwrap_or_else(Utc::now)
        })
    }
    
    /// Get the ingested timestamp
    pub fn ingested_at(&self) -> Option<DateTime<Utc>> {
        self.inner.ingested_at.as_ref().map(|ts| {
            DateTime::from_timestamp(ts.seconds, ts.nanos as u32).unwrap_or_else(Utc::now)
        })
    }
    
    /// Get headers
    pub fn headers(&self) -> &HashMap<String, String> {
        &self.inner.headers
    }
    
    /// Get quality score
    pub fn quality_score(&self) -> f32 {
        self.inner.quality
            .as_ref()
            .map(|q| q.quality_score)
            .unwrap_or(0.0)
    }
    
    /// Get routing topic
    pub fn topic(&self) -> &str {
        self.inner.routing
            .as_ref()
            .map(|r| r.topic.as_str())
            .unwrap_or("")
    }
    
    /// Get routing priority
    pub fn priority(&self) -> i32 {
        self.inner.routing
            .as_ref()
            .map(|r| r.priority)
            .unwrap_or(5)
    }
    
    /// Deserialize the payload to a specific protobuf type
    pub fn payload<T: Message + Default>(&self) -> Result<T, crate::errors::CoreError> {
        let payload = self.inner.payload
            .as_ref()
            .ok_or_else(|| crate::errors::CoreError::EventError("No payload found".to_string()))?;
        
        T::decode(&payload.value[..])
            .map_err(|e| crate::errors::CoreError::EventError(format!("Failed to decode payload: {}", e)))
    }
    
    /// Get the raw EventEnvelope (for advanced use cases)
    pub fn inner(&self) -> &proto_messages::EventEnvelope {
        &self.inner
    }
    
    /// Convert to EventEnvelope (consuming the Event)
    pub fn into_inner(self) -> proto_messages::EventEnvelope {
        self.inner
    }
    
    /// Serialize the entire event to proto bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.encode_to_vec()
    }
    
    /// Deserialize from proto bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, crate::errors::CoreError> {
        let envelope = proto_messages::EventEnvelope::decode(bytes)
            .map_err(|e| crate::errors::CoreError::EventError(format!("Failed to decode event: {}", e)))?;
        
        Ok(Self { inner: envelope })
    }
    
    /// Validate the event structure and payload
    pub fn validate(&self) -> Result<(), crate::errors::CoreError> {
        if self.inner.message_id.is_empty() {
            return Err(crate::errors::CoreError::EventError("message_id cannot be empty".to_string()));
        }
        
        if self.inner.event_type.is_empty() {
            return Err(crate::errors::CoreError::EventError("event_type cannot be empty".to_string()));
        }
        
        if self.inner.source.is_empty() {
            return Err(crate::errors::CoreError::EventError("source cannot be empty".to_string()));
        }
        
        if self.inner.domain.is_empty() {
            return Err(crate::errors::CoreError::EventError("domain cannot be empty".to_string()));
        }
        
        if self.inner.payload.is_none() {
            return Err(crate::errors::CoreError::EventError("payload is required".to_string()));
        }
        
        // Validate quality score if present
        if let Some(quality) = &self.inner.quality {
            if quality.quality_score < 0.0 || quality.quality_score > 100.0 {
                return Err(crate::errors::CoreError::EventError("quality_score must be between 0.0 and 100.0".to_string()));
            }
        }
        
        Ok(())
    }
}

/// Implement From trait for EventEnvelope
impl From<proto_messages::EventEnvelope> for Event {
    fn from(envelope: proto_messages::EventEnvelope) -> Self {
        Self { inner: envelope }
    }
}

/// Implement TryFrom trait for Event to EventEnvelope conversion
impl TryFrom<Event> for proto_messages::EventEnvelope {
    type Error = crate::errors::CoreError;
    
    fn try_from(event: Event) -> Result<Self, Self::Error> {
        event.validate()?;
        Ok(event.inner)
    }
}

/// Contract violation helper - REJECTS Vec<u8> payloads
pub fn reject_vec_u8_payload() -> crate::errors::CoreError {
    crate::errors::CoreError::EventError(
        "Contract violation: Vec<u8> payloads are FORBIDDEN in Phase 4. \
         Use proto-only Event::new(proto_message) instead."
            .to_string()
    )
}

/// Contract violation helper - REJECTS serde JSON
pub fn reject_json_payload() -> crate::errors::CoreError {
    crate::errors::CoreError::EventError(
        "Contract violation: JSON payloads are FORBIDDEN in Phase 4. \
         Use Data-Staging service to convert JSON to proto messages first."
            .to_string()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eventbus::proto_messages::*;
    
    // Mock proto message for testing
    #[derive(Clone, prost::Message)]
    struct TestMessage {
        #[prost(string, tag = "1")]
        content: String,
        #[prost(int64, tag = "2")]
        value: i64,
    }
    
    #[test]
    fn test_event_creation() {
        let test_msg = TestMessage {
            content: "test content".to_string(),
            value: 42,
        };
        
        let event = Event::new("test.TestMessage", test_msg, "test-source", "test-domain")
            .expect("Should create event");
        
        assert_eq!(event.event_type(), "test.TestMessage");
        assert_eq!(event.source(), "test-source");
        assert_eq!(event.domain(), "test-domain");
        assert!(!event.message_id().is_empty());
    }
    
    #[test]
    fn test_event_with_metadata() {
        let test_msg = TestMessage {
            content: "test".to_string(),
            value: 123,
        };
        
        let event = Event::new("test.TestMessage", test_msg, "source", "domain")
            .expect("Should create event")
            .with_correlation_id("corr-123")
            .with_header("custom-header", "custom-value")
            .with_routing("test-topic", "partition-1", 8)
            .with_quality(95.0, 98.0);
        
        assert_eq!(event.correlation_id(), "corr-123");
        assert_eq!(event.headers().get("custom-header"), Some(&"custom-value".to_string()));
        assert_eq!(event.topic(), "test-topic");
        assert_eq!(event.priority(), 8);
        assert_eq!(event.quality_score(), 98.0);
    }
    
    #[test]
    fn test_event_payload_deserialization() {
        let test_msg = TestMessage {
            content: "hello world".to_string(),
            value: 999,
        };
        
        let event = Event::new("test.TestMessage", test_msg.clone(), "source", "domain")
            .expect("Should create event");
        
        let deserialized: TestMessage = event.payload().expect("Should deserialize payload");
        assert_eq!(deserialized.content, "hello world");
        assert_eq!(deserialized.value, 999);
    }
    
    #[test]
    fn test_event_validation() {
        let test_msg = TestMessage {
            content: "test".to_string(),
            value: 42,
        };
        
        let event = Event::new("test.TestMessage", test_msg, "source", "domain")
            .expect("Should create event");
        
        assert!(event.validate().is_ok());
    }
    
    #[test]
    fn test_event_serialization_roundtrip() {
        let test_msg = TestMessage {
            content: "roundtrip test".to_string(),
            value: 777,
        };
        
        let original = Event::new("test.TestMessage", test_msg, "source", "domain")
            .expect("Should create event")
            .with_correlation_id("roundtrip-test");
        
        let bytes = original.to_bytes();
        let recovered = Event::from_bytes(&bytes).expect("Should deserialize");
        
        assert_eq!(recovered.event_type(), "test.TestMessage");
        assert_eq!(recovered.correlation_id(), "roundtrip-test");
        assert_eq!(recovered.source(), "source");
        assert_eq!(recovered.domain(), "domain");
    }
    
    #[test]
    fn test_contract_violation_helpers() {
        let error = reject_vec_u8_payload();
        assert!(error.to_string().contains("Vec<u8> payloads are FORBIDDEN"));
        
        let error = reject_json_payload();
        assert!(error.to_string().contains("JSON payloads are FORBIDDEN"));
    }
}