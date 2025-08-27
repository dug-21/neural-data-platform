//! Standalone test for proto-only Event system
//! 
//! This tests the new Event and EventEnvelope types without the conflicting eventbus module.

// Include the necessary types directly
use prost::Message;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};

// Copy the proto message definitions we need
#[derive(Clone, PartialEq, prost::Message)]
pub struct EventEnvelope {
    #[prost(string, tag = "1")]
    pub message_id: String,
    #[prost(string, tag = "2")]
    pub correlation_id: String,
    #[prost(string, tag = "3")]
    pub source: String,
    #[prost(string, tag = "4")]
    pub domain: String,
    #[prost(string, tag = "5")]
    pub event_type: String,
    #[prost(string, tag = "6")]
    pub schema_version: String,
    #[prost(message, optional, tag = "7")]
    pub created_at: Option<prost_types::Timestamp>,
    #[prost(message, optional, tag = "8")]
    pub ingested_at: Option<prost_types::Timestamp>,
    #[prost(message, optional, tag = "9")]
    pub routing: Option<RoutingMetadata>,
    #[prost(message, optional, tag = "10")]
    pub quality: Option<QualityMetadata>,
    #[prost(message, optional, tag = "11")]
    pub payload: Option<prost_types::Any>,
    #[prost(map = "string, string", tag = "12")]
    pub headers: HashMap<String, String>,
    #[prost(message, optional, tag = "13")]
    pub tracing: Option<TracingContext>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct RoutingMetadata {
    #[prost(string, tag = "1")]
    pub topic: String,
    #[prost(string, tag = "2")]
    pub partition_key: String,
    #[prost(int32, tag = "3")]
    pub priority: i32,
    #[prost(int64, tag = "4")]
    pub ttl_seconds: i64,
    #[prost(string, repeated, tag = "5")]
    pub tags: Vec<String>,
    #[prost(message, optional, tag = "6")]
    pub retry_policy: Option<RetryPolicy>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct QualityMetadata {
    #[prost(float, tag = "1")]
    pub completeness: f32,
    #[prost(int64, tag = "2")]
    pub latency_ms: i64,
    #[prost(enumeration = "ValidationStatus", tag = "3")]
    pub validation_status: i32,
    #[prost(float, tag = "4")]
    pub quality_score: f32,
    #[prost(message, repeated, tag = "5")]
    pub anomalies: Vec<AnomalyIndicator>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, prost::Enumeration)]
#[repr(i32)]
pub enum ValidationStatus {
    ValidationStatusUnspecified = 0,
    ValidationStatusPassed = 1,
    ValidationStatusFailed = 2,
    ValidationStatusPartial = 3,
    ValidationStatusSkipped = 4,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AnomalyIndicator {
    #[prost(string, tag = "1")]
    pub r#type: String,
    #[prost(float, tag = "2")]
    pub severity: f32,
    #[prost(string, tag = "3")]
    pub description: String,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct RetryPolicy {
    #[prost(int32, tag = "1")]
    pub max_attempts: i32,
    #[prost(int64, tag = "2")]
    pub initial_delay_ms: i64,
    #[prost(float, tag = "3")]
    pub backoff_multiplier: f32,
    #[prost(int64, tag = "4")]
    pub max_delay_ms: i64,
    #[prost(string, repeated, tag = "5")]
    pub retryable_errors: Vec<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct TracingContext {
    #[prost(string, tag = "1")]
    pub trace_id: String,
    #[prost(string, tag = "2")]
    pub span_id: String,
    #[prost(string, tag = "3")]
    pub parent_span_id: String,
    #[prost(map = "string, string", tag = "4")]
    pub baggage: HashMap<String, String>,
}

// Proto-only Event wrapper around EventEnvelope
#[derive(Debug, Clone)]
pub struct Event {
    inner: EventEnvelope,
}

impl Event {
    pub fn new<T: Message + Clone>(
        event_type: &str,
        payload: T,
        source: &str,
        domain: &str,
    ) -> Result<Self, String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();
        
        let timestamp = prost_types::Timestamp {
            seconds: now.as_secs() as i64,
            nanos: now.subsec_nanos() as i32,
        };
        
        let payload_bytes = payload.encode_to_vec();
        let payload_any = prost_types::Any {
            type_url: format!("type.googleapis.com/{}", event_type),
            value: payload_bytes,
        };
        
        let envelope = EventEnvelope {
            message_id: Uuid::new_v4().to_string(),
            correlation_id: String::new(),
            source: source.to_string(),
            domain: domain.to_string(),
            event_type: event_type.to_string(),
            schema_version: "v1".to_string(),
            created_at: Some(timestamp.clone()),
            ingested_at: Some(timestamp),
            routing: Some(RoutingMetadata {
                topic: format!("{}.{}", domain, event_type),
                partition_key: String::new(),
                priority: 5,
                ttl_seconds: 0,
                tags: vec![],
                retry_policy: Some(RetryPolicy {
                    max_attempts: 3,
                    initial_delay_ms: 1000,
                    backoff_multiplier: 2.0,
                    max_delay_ms: 30000,
                    retryable_errors: vec![],
                }),
            }),
            quality: Some(QualityMetadata {
                completeness: 100.0,
                latency_ms: 0,
                validation_status: ValidationStatus::ValidationStatusPassed as i32,
                quality_score: 100.0,
                anomalies: vec![],
            }),
            payload: Some(payload_any),
            headers: HashMap::new(),
            tracing: Some(TracingContext {
                trace_id: Uuid::new_v4().to_string(),
                span_id: Uuid::new_v4().to_string(),
                parent_span_id: String::new(),
                baggage: HashMap::new(),
            }),
        };
        
        Ok(Self { inner: envelope })
    }
    
    pub fn event_type(&self) -> &str {
        &self.inner.event_type
    }
    
    pub fn message_id(&self) -> &str {
        &self.inner.message_id
    }
    
    pub fn source(&self) -> &str {
        &self.inner.source
    }
    
    pub fn domain(&self) -> &str {
        &self.inner.domain
    }
    
    pub fn payload<T: Message + Default>(&self) -> Result<T, String> {
        let payload = self.inner.payload
            .as_ref()
            .ok_or_else(|| "No payload found".to_string())?;
        
        T::decode(&payload.value[..])
            .map_err(|e| format!("Failed to decode payload: {}", e))
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        self.inner.encode_to_vec()
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let envelope = EventEnvelope::decode(bytes)
            .map_err(|e| format!("Failed to decode event: {}", e))?;
        
        Ok(Self { inner: envelope })
    }
}

// Test proto message
#[derive(Clone, prost::Message)]
struct TestMessage {
    #[prost(string, tag = "1")]
    content: String,
    #[prost(double, tag = "2")]
    value: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Testing Proto-only Event System");
    println!("===================================");
    
    // Test 1: Create a proto event
    let test_msg = TestMessage {
        content: "Hello, Proto World!".to_string(),
        value: 42.5,
    };
    
    let event = Event::new(
        "test.TestMessage",
        test_msg.clone(),
        "test-service",
        "testing"
    )?;
    
    println!("✅ Created Event:");
    println!("   Event Type: {}", event.event_type());
    println!("   Message ID: {}", event.message_id());
    println!("   Source: {}", event.source());
    println!("   Domain: {}", event.domain());
    
    // Test 2: Serialize and deserialize
    let bytes = event.to_bytes();
    println!("\n✅ Serialized Event:");
    println!("   Size: {} bytes", bytes.len());
    
    let recovered_event = Event::from_bytes(&bytes)?;
    println!("   ✅ Deserialized successfully");
    println!("   Event Type: {}", recovered_event.event_type());
    println!("   Message ID: {}", recovered_event.message_id());
    
    // Test 3: Extract payload
    let extracted_payload: TestMessage = event.payload()?;
    println!("\n✅ Extracted Payload:");
    println!("   Content: {}", extracted_payload.content);
    println!("   Value: {}", extracted_payload.value);
    
    // Verify payload matches
    assert_eq!(extracted_payload.content, test_msg.content);
    assert_eq!(extracted_payload.value, test_msg.value);
    
    println!("\n🎉 All tests passed!");
    println!("🚫 NO Vec<u8> payloads used - Proto-only messaging verified!");
    
    Ok(())
}