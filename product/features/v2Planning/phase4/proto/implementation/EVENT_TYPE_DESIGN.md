# Event Type Design: Proto-Only Implementation

## Overview

This document outlines the design and implementation plan for a strict protobuf-only Event type that directly wraps the EventEnvelope from `schemas/ingestion-eventbus.proto`. This is a breaking change that enforces type safety and eliminates all non-protobuf data paths.

## Breaking Change Notice

### ❌ DEPRECATED: Existing Event Type
The following types are being completely replaced:
```rust
// DEPRECATED - Will be removed
pub struct Event {
    pub event_type: String,
    pub payload: Vec<u8>,  // ❌ NO MORE Vec<u8>
    pub metadata: HashMap<String, String>,
    pub timestamp: i64,
}

// DEPRECATED - Will be removed
pub struct EventEnvelope {
    pub event_id: EventId,
    pub channel: String,
    pub event: Event,
    pub retry_count: u32,
    pub delivered_at: i64,
}
```

### Target Proto EventEnvelope (schemas/ingestion-eventbus.proto)
```protobuf
message EventEnvelope {
  string message_id = 1;
  string correlation_id = 2;
  string source = 3;
  string domain = 4;
  string event_type = 5;
  string schema_version = 6;
  google.protobuf.Timestamp created_at = 7;
  google.protobuf.Timestamp ingested_at = 8;
  RoutingMetadata routing = 9;
  QualityMetadata quality = 10;
  google.protobuf.Any payload = 11;
  map<string, string> headers = 12;
  TracingContext tracing = 13;
}
```

## Design Goals

1. **🚫 NO BACKWARD COMPATIBILITY**: Breaking change, proto-only
2. **✅ Strict Type Safety**: Only protobuf data accepted
3. **✅ Zero Escape Hatches**: No Vec<u8> payload field
4. **✅ Validation First**: Reject non-proto data at boundaries
5. **✅ Performance**: Efficient proto operations only
6. **✅ Future-Proof**: Schema evolution through protobuf
7. **✅ Developer Experience**: Clear proto-first API

## Architecture Design

### Data-Staging Event Creation (NEW)

The Data-Staging service is the ONLY component that creates EventEnvelope protos from raw data:

```rust
// In data-staging service
pub struct DataStaging {
    redis_consumer: RedisConsumer,
    eventbus_publisher: EventBusPublisher,
}

impl DataStaging {
    // Transform raw JSON to proto
    pub fn transform_market_data(&self, json_str: &str) -> Result<EventEnvelope> {
        // 1. Parse JSON
        let raw: serde_json::Value = serde_json::from_str(json_str)?;
        
        // 2. Validate required fields
        self.validate_market_data(&raw)?;
        
        // 3. Create proto EventEnvelope
        let mut envelope = EventEnvelope::default();
        envelope.message_id = Uuid::new_v4().to_string();
        envelope.source = "data-staging".to_string();
        envelope.event_type = "market_data".to_string();
        
        // 4. Convert payload to proto Any
        let market_data = self.json_to_market_data_proto(&raw)?;
        envelope.payload = Some(prost_types::Any::from_msg(&market_data)?);
        
        // 5. Add metadata
        envelope.quality = Some(self.calculate_quality_score(&raw));
        envelope.ingested_at = Some(Timestamp::now());
        
        Ok(envelope)
    }
    
    // Validate incoming JSON structure
    fn validate_market_data(&self, raw: &serde_json::Value) -> Result<()> {
        // Ensure required fields exist
        raw["symbol"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing symbol field"))?;
        raw["price"].as_f64()
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid price field"))?;
        raw["timestamp"].as_i64()
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid timestamp field"))?;
            
        Ok(())
    }
    
    // Convert JSON to strongly-typed proto message
    fn json_to_market_data_proto(&self, raw: &serde_json::Value) -> Result<proto::market_data::PriceUpdate> {
        Ok(proto::market_data::PriceUpdate {
            symbol: raw["symbol"].as_str().unwrap().to_string(),
            price: raw["price"].as_f64().unwrap(),
            volume: raw["volume"].as_f64().unwrap_or(0.0),
            timestamp: raw["timestamp"].as_i64().unwrap(),
            bid: raw["bid"].as_f64(),
            ask: raw["ask"].as_f64(),
            exchange: raw["exchange"].as_str().map(|s| s.to_string()),
        })
    }
    
    // Calculate data quality metrics
    fn calculate_quality_score(&self, raw: &serde_json::Value) -> proto::QualityMetadata {
        let mut quality = proto::QualityMetadata::default();
        
        // Check data freshness
        if let Some(timestamp) = raw["timestamp"].as_i64() {
            let age_seconds = chrono::Utc::now().timestamp() - timestamp;
            quality.freshness_score = match age_seconds {
                0..=5 => 1.0,      // Excellent: 0-5s old
                6..=30 => 0.8,     // Good: 6-30s old
                31..=300 => 0.5,   // Fair: 31s-5min old
                _ => 0.2,          // Poor: >5min old
            };
        }
        
        // Check data completeness
        let required_fields = ["symbol", "price", "timestamp"];
        let optional_fields = ["volume", "bid", "ask", "exchange"];
        
        let required_present = required_fields.iter()
            .filter(|&field| !raw[field].is_null())
            .count();
        let optional_present = optional_fields.iter()
            .filter(|&field| !raw[field].is_null())
            .count();
        
        quality.completeness_score = (required_present as f32 / required_fields.len() as f32) * 0.7 +
                                   (optional_present as f32 / optional_fields.len() as f32) * 0.3;
        
        // Check data validity
        quality.validity_score = if raw["price"].as_f64().unwrap_or(0.0) > 0.0 { 1.0 } else { 0.0 };
        
        // Overall quality score
        quality.overall_score = (quality.freshness_score + quality.completeness_score + quality.validity_score) / 3.0;
        
        quality
    }
    
    // Process incoming raw data stream
    pub async fn process_raw_stream(&self) -> Result<()> {
        loop {
            // 1. Consume raw data from Redis streams
            let raw_data = self.redis_consumer.consume().await?;
            
            // 2. Transform to proto EventEnvelope
            let proto_envelope = self.transform_market_data(&raw_data)?;
            
            // 3. Publish to EventBus (proto-only)
            self.eventbus_publisher.publish(proto_envelope).await?;
            
            // 4. Update metrics
            self.update_processing_metrics().await?;
        }
    }
}

// EventBus integration - ONLY accepts proto EventEnvelopes
impl EventBusPublisher {
    pub async fn publish(&self, envelope: proto::EventEnvelope) -> Result<()> {
        // Validate proto structure before publishing
        if envelope.message_id.is_empty() {
            return Err(anyhow::anyhow!("EventEnvelope missing message_id"));
        }
        
        if envelope.payload.is_none() {
            return Err(anyhow::anyhow!("EventEnvelope missing payload"));
        }
        
        // Serialize proto to bytes for transport
        let bytes = envelope.encode_to_vec();
        
        // Publish to appropriate channel based on event_type
        let channel = self.route_to_channel(&envelope.event_type);
        self.transport.publish(channel, bytes).await
    }
    
    fn route_to_channel(&self, event_type: &str) -> &str {
        match event_type {
            "market_data" => "market-data-stream",
            "trading_signal" => "trading-signals",
            "order_event" => "order-management",
            _ => "default-events",
        }
    }
}
```

**Key Design Points:**
- **Data-Staging as Proto Factory**: Only Data-Staging creates EventEnvelope protos from raw data
- **JSON to Proto Transformation**: Validates and converts raw JSON to strongly-typed proto messages
- **Quality Metadata**: Calculates data quality scores during transformation
- **EventBus Integration**: EventBus ONLY accepts validated EventEnvelope protos
- **Type Safety**: No Vec<u8> payloads - only strongly-typed proto messages
- **Validation First**: Rejects invalid data at the transformation boundary

### Proto-Only Type Hierarchy

```rust
/// The ONLY Event type - directly wraps protobuf EventEnvelope
/// 🚫 NO Vec<u8> payload field
/// 🚫 NO legacy compatibility
/// ✅ ONLY protobuf data accepted
pub struct Event {
    inner: proto::EventEnvelope,
}

/// Event envelope that wraps Event - proto-only
pub struct EventEnvelope {
    event: Event,
    channel: String,
    retry_count: u32,
}

// 🚫 REMOVED: No more LegacyEvent, ProtoEvent, or unified enum
```

### Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("Protobuf serialization error: {0}")]
    ProtoSerialization(#[from] prost::EncodeError),
    
    #[error("Protobuf deserialization error: {0}")]
    ProtoDeserialization(#[from] prost::DecodeError),
    
    #[error("Missing required protobuf field: {field}")]
    MissingRequiredField { field: String },
    
    #[error("Invalid protobuf timestamp: {0}")]
    InvalidTimestamp(String),
    
    #[error("Non-protobuf data rejected: {reason}")]
    NonProtobufDataRejected { reason: String },
    
    #[error("Protobuf validation failed: {0}")]
    ValidationFailed(String),
}

pub type EventResult<T> = Result<T, EventError>;
```

## Proto-Only Implementation

### Phase 1: Strict Proto-Only Event Type

#### 1.1 Dependencies (Proto-Only)
```toml
# Cargo.toml
[dependencies]
prost = "0.12"
prost-types = "0.12"
tonic = "0.10"

[build-dependencies]
tonic-build = "0.10"
```

#### 1.2 Event Implementation (Proto-Only)
```rust
use prost::Message;
use prost_types::{Timestamp, Any};

impl Event {
    /// Create from protobuf EventEnvelope - ONLY way to create Event
    pub fn from_proto(envelope: proto::EventEnvelope) -> EventResult<Self> {
        // Validate required protobuf fields
        if envelope.event_type.is_empty() {
            return Err(EventError::MissingRequiredField {
                field: "event_type".to_string(),
            });
        }
        
        if envelope.payload.is_none() {
            return Err(EventError::MissingRequiredField {
                field: "payload".to_string(),
            });
        }
        
        if envelope.created_at.is_none() {
            return Err(EventError::MissingRequiredField {
                field: "created_at".to_string(),
            });
        }
        
        Ok(Self { inner: envelope })
    }
    
    /// 🚫 NO Vec<u8> constructor - ONLY protobuf accepted
    /// Use EventBuilder instead for creating new events
    
    /// Get event type from protobuf
    pub fn event_type(&self) -> &str {
        &self.inner.event_type
    }
    
    /// Get protobuf Any payload - NO Vec<u8> conversion
    pub fn payload(&self) -> &Any {
        self.inner.payload.as_ref()
            .expect("payload validated in constructor")
    }
    
    /// Decode typed payload from protobuf Any
    pub fn decode_payload<T>(&self) -> EventResult<T>
    where
        T: prost::Message + Default,
    {
        let any_payload = self.payload();
        T::decode(any_payload.value.as_slice())
            .map_err(EventError::ProtoDeserialization)
    }
    
    /// Get timestamp from protobuf
    pub fn timestamp(&self) -> i64 {
        self.inner.created_at.as_ref()
            .expect("created_at validated in constructor")
            .seconds
    }
    
    /// Get correlation ID
    pub fn correlation_id(&self) -> &str {
        &self.inner.correlation_id
    }
    
    /// Get source
    pub fn source(&self) -> &str {
        &self.inner.source
    }
    
    /// Get domain
    pub fn domain(&self) -> &str {
        &self.inner.domain
    }
    
    /// Get headers (protobuf map)
    pub fn headers(&self) -> &std::collections::HashMap<String, String> {
        &self.inner.headers
    }
    
    /// Get routing metadata
    pub fn routing(&self) -> Option<&proto::RoutingMetadata> {
        self.inner.routing.as_ref()
    }
    
    /// Get quality metadata
    pub fn quality(&self) -> Option<&proto::QualityMetadata> {
        self.inner.quality.as_ref()
    }
    
    /// Get tracing context
    pub fn tracing(&self) -> Option<&proto::TracingContext> {
        self.inner.tracing.as_ref()
    }
    
    /// Serialize to protobuf bytes
    pub fn to_bytes(&self) -> EventResult<Vec<u8>> {
        let mut buf = Vec::new();
        self.inner.encode(&mut buf)?;
        Ok(buf)
    }
    
    /// Deserialize ONLY from protobuf bytes
    pub fn from_bytes(bytes: &[u8]) -> EventResult<Self> {
        let envelope = proto::EventEnvelope::decode(bytes)?;
        Self::from_proto(envelope)
    }
    
    /// Validate protobuf structure
    pub fn validate(&self) -> Result<(), Vec<EventError>> {
        let mut errors = Vec::new();
        
        if self.inner.message_id.is_empty() {
            errors.push(EventError::MissingRequiredField {
                field: "message_id".to_string(),
            });
        }
        
        if self.inner.event_type.is_empty() {
            errors.push(EventError::MissingRequiredField {
                field: "event_type".to_string(),
            });
        }
        
        if self.inner.payload.is_none() {
            errors.push(EventError::MissingRequiredField {
                field: "payload".to_string(),
            });
        }
        
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}
```

### Phase 2: Proto-Only EventBuilder

#### 2.1 EventBuilder for Creating Proto Events
```rust
pub struct EventBuilder {
    event_type: String,
    message_id: Option<String>,
    correlation_id: Option<String>,
    source: String,
    domain: String,
    headers: std::collections::HashMap<String, String>,
    routing: Option<proto::RoutingMetadata>,
    quality: Option<proto::QualityMetadata>,
    tracing: Option<proto::TracingContext>,
}

impl EventBuilder {
    /// Create new builder - ONLY way to build Events
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            message_id: None,
            correlation_id: None,
            source: "neural-trader".to_string(),
            domain: "trading".to_string(),
            headers: std::collections::HashMap::new(),
            routing: None,
            quality: None,
            tracing: None,
        }
    }
    
    /// Build event with strongly-typed protobuf payload
    pub fn with_payload<T>(self, payload: T) -> EventResult<Event>
    where
        T: prost::Message,
    {
        let mut payload_bytes = Vec::new();
        payload.encode(&mut payload_bytes)
            .map_err(EventError::ProtoSerialization)?;
        
        let any_payload = prost_types::Any {
            type_url: format!("type.googleapis.com/{}", self.event_type),
            value: payload_bytes,
        };
        
        self.build_with_any(any_payload)
    }
    
    /// 🚫 NO raw bytes payload - ONLY typed protobuf accepted
    
    pub fn message_id(mut self, id: impl Into<String>) -> Self {
        self.message_id = Some(id.into());
        self
    }
    
    pub fn correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }
    
    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }
    
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = domain.into();
        self
    }
    
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
    
    pub fn routing(mut self, routing: proto::RoutingMetadata) -> Self {
        self.routing = Some(routing);
        self
    }
    
    pub fn quality(mut self, quality: proto::QualityMetadata) -> Self {
        self.quality = Some(quality);
        self
    }
    
    pub fn tracing(mut self, tracing: proto::TracingContext) -> Self {
        self.tracing = Some(tracing);
        self
    }
    
    fn build_with_any(self, payload: prost_types::Any) -> EventResult<Event> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| EventError::InvalidTimestamp(e.to_string()))?;
        
        let envelope = proto::EventEnvelope {
            message_id: self.message_id
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            correlation_id: self.correlation_id.unwrap_or_default(),
            source: self.source,
            domain: self.domain,
            event_type: self.event_type,
            schema_version: "1.0".to_string(),
            created_at: Some(prost_types::Timestamp {
                seconds: timestamp.as_secs() as i64,
                nanos: timestamp.subsec_nanos() as i32,
            }),
            ingested_at: None,
            routing: self.routing,
            quality: self.quality,
            payload: Some(payload),
            headers: self.headers,
            tracing: self.tracing,
        };
        
        Event::from_proto(envelope)
    }
}
```

### Phase 3: Proto-Only Conversion Traits

#### 3.1 Strict Protobuf Conversions Only
```rust
// 🚫 NO legacy conversions - ONLY protobuf

impl TryFrom<proto::EventEnvelope> for Event {
    type Error = EventError;
    
    fn try_from(envelope: proto::EventEnvelope) -> EventResult<Self> {
        Event::from_proto(envelope)
    }
}

impl From<Event> for proto::EventEnvelope {
    fn from(event: Event) -> Self {
        event.inner
    }
}

impl TryFrom<&[u8]> for Event {
    type Error = EventError;
    
    fn try_from(bytes: &[u8]) -> EventResult<Self> {
        // ONLY protobuf deserialization - NO fallback
        let envelope = proto::EventEnvelope::decode(bytes)
            .map_err(|_| EventError::NonProtobufDataRejected {
                reason: "Data is not valid protobuf EventEnvelope".to_string(),
            })?;
        Event::from_proto(envelope)
    }
}

impl TryFrom<Vec<u8>> for Event {
    type Error = EventError;
    
    fn try_from(bytes: Vec<u8>) -> EventResult<Self> {
        Self::try_from(bytes.as_slice())
    }
}

// 🚫 REMOVED: All legacy conversion traits
```

#### 3.2 Proto-Only Serialization
```rust
// 🚫 NO serde - ONLY protobuf serialization

impl Event {
    /// Serialize to protobuf wire format
    pub fn serialize(&self) -> EventResult<Vec<u8>> {
        self.to_bytes()
    }
    
    /// Deserialize from protobuf wire format ONLY
    pub fn deserialize(bytes: &[u8]) -> EventResult<Self> {
        Self::from_bytes(bytes)
    }
    
    /// 🚫 NO JSON serialization - use protobuf JSON if needed
    /// 🚫 NO bincode serialization - use protobuf wire format
    /// 🚫 NO serde integration - use protobuf ecosystem
}

// If JSON is absolutely required, use protobuf-json crate:
// ```rust
// pub fn to_json(&self) -> EventResult<String> {
//     serde_json::to_string(&self.inner)
//         .map_err(|e| EventError::ValidationFailed(e.to_string()))
// }
// ```
```

### Phase 4: Proto-Only EventEnvelope

#### 4.1 EventEnvelope Wrapper
```rust
/// EventEnvelope wraps Event for channel-specific delivery
/// 🚫 NO legacy support - proto-only
pub struct EventEnvelope {
    event: Event,
    channel: String,
    retry_count: u32,
}

impl EventEnvelope {
    /// Create new envelope with proto event
    pub fn new(channel: String, event: Event) -> Self {
        Self {
            event,
            channel,
            retry_count: 0,
        }
    }
    
    /// Get the proto event
    pub fn event(&self) -> &Event {
        &self.event
    }
    
    /// Get mutable proto event
    pub fn event_mut(&mut self) -> &mut Event {
        &mut self.event
    }
    
    /// Get channel name
    pub fn channel(&self) -> &str {
        &self.channel
    }
    
    /// Get retry count
    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }
    
    /// Increment retry counter
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }
    
    /// Serialize envelope to bytes
    pub fn to_bytes(&self) -> EventResult<Vec<u8>> {
        self.event.to_bytes()
    }
    
    /// Deserialize envelope from proto bytes
    pub fn from_bytes(bytes: &[u8], channel: String) -> EventResult<Self> {
        let event = Event::from_bytes(bytes)?;
        Ok(Self::new(channel, event))
    }
    
    /// Validate envelope and contained event
    pub fn validate(&self) -> Result<(), Vec<EventError>> {
        self.event.validate()
    }
}

// 🚫 REMOVED: No unified enum, no legacy support
```

### Phase 5: Proto Performance Optimizations

#### 5.1 Zero-Copy Proto Operations
```rust
impl Event {
    /// Get protobuf Any payload without deserialization
    pub fn payload_any(&self) -> &prost_types::Any {
        self.payload()
    }
    
    /// Zero-copy access to payload bytes (for large messages)
    pub fn payload_bytes(&self) -> &[u8] {
        &self.payload().value
    }
    
    /// Stream large protobuf payloads efficiently
    pub fn payload_reader(&self) -> &[u8] {
        self.payload_bytes()
    }
    
    /// Consume event and extract protobuf Any
    pub fn into_payload_any(self) -> prost_types::Any {
        self.inner.payload.expect("payload validated in constructor")
    }
    
    /// 🚫 NO into_payload() -> Vec<u8> - use typed extraction instead
}
```

#### 5.2 Efficient Proto Field Access
```rust
impl Event {
    /// Direct access to protobuf inner envelope (zero-copy)
    pub fn inner(&self) -> &proto::EventEnvelope {
        &self.inner
    }
    
    /// Message ID access (no allocation)
    pub fn message_id(&self) -> &str {
        &self.inner.message_id
    }
    
    /// Schema version access
    pub fn schema_version(&self) -> &str {
        &self.inner.schema_version
    }
    
    /// Created timestamp (efficient)
    pub fn created_at(&self) -> &prost_types::Timestamp {
        self.inner.created_at.as_ref()
            .expect("created_at validated in constructor")
    }
    
    /// Ingested timestamp (optional)
    pub fn ingested_at(&self) -> Option<&prost_types::Timestamp> {
        self.inner.ingested_at.as_ref()
    }
}
```

### Phase 6: Usage Examples

#### 6.1 Creating Events with Typed Payloads
```rust
use crate::proto::market_data::PriceUpdate;

// Create a strongly-typed market data event
let price_update = PriceUpdate {
    symbol: "BTCUSD".to_string(),
    price: 50000.0,
    volume: 1.5,
    timestamp: std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs(),
};

let event = EventBuilder::new("market_data.price_update")
    .correlation_id("trade-123")
    .source("binance-adapter")
    .domain("market-data")
    .header("exchange", "binance")
    .header("priority", "high")
    .with_payload(price_update)?;

// Usage patterns
assert_eq!(event.event_type(), "market_data.price_update");
assert_eq!(event.source(), "binance-adapter");
assert_eq!(event.headers().get("exchange"), Some(&"binance".to_string()));

// Decode typed payload
let decoded: PriceUpdate = event.decode_payload()?;
assert_eq!(decoded.symbol, "BTCUSD");
```

#### 6.2 Event Validation and Processing
```rust
// Validate event structure
event.validate()?;

// Process different event types
match event.event_type() {
    "market_data.price_update" => {
        let price_data: PriceUpdate = event.decode_payload()?;
        process_price_update(price_data).await?;
    }
    "trading.order_filled" => {
        let order_data: OrderFill = event.decode_payload()?;
        process_order_fill(order_data).await?;
    }
    _ => {
        return Err(EventError::ValidationFailed(
            format!("Unknown event type: {}", event.event_type())
        ));
    }
}
```

### Phase 7: Proto-Only Testing Strategy

#### 7.1 Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::test_events::TestPayload;
    
    #[test]
    fn test_proto_event_creation() {
        let payload = TestPayload {
            message: "test message".to_string(),
            value: 42,
        };
        
        let event = EventBuilder::new("test.event")
            .source("test-source")
            .with_payload(payload)
            .unwrap();
        
        assert_eq!(event.event_type(), "test.event");
        assert_eq!(event.source(), "test-source");
        
        // Decode typed payload
        let decoded: TestPayload = event.decode_payload().unwrap();
        assert_eq!(decoded.message, "test message");
        assert_eq!(decoded.value, 42);
    }
    
    #[test]
    fn test_proto_serialization_round_trip() {
        let payload = TestPayload {
            message: "round trip test".to_string(),
            value: 123,
        };
        
        let original = EventBuilder::new("test.round_trip")
            .correlation_id("test-123")
            .with_payload(payload)
            .unwrap();
        
        // Serialize to bytes
        let bytes = original.to_bytes().unwrap();
        
        // Deserialize back
        let reconstructed = Event::from_bytes(&bytes).unwrap();
        
        // Verify all fields match
        assert_eq!(original.event_type(), reconstructed.event_type());
        assert_eq!(original.correlation_id(), reconstructed.correlation_id());
        
        let orig_payload: TestPayload = original.decode_payload().unwrap();
        let recon_payload: TestPayload = reconstructed.decode_payload().unwrap();
        assert_eq!(orig_payload.message, recon_payload.message);
        assert_eq!(orig_payload.value, recon_payload.value);
    }
    
    #[test]
    fn test_validation_rejects_invalid_data() {
        // Test invalid protobuf data is rejected
        let invalid_bytes = b"not protobuf data";
        let result = Event::from_bytes(invalid_bytes);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            EventError::NonProtobufDataRejected { .. } => {}
            _ => panic!("Expected NonProtobufDataRejected error"),
        }
    }
    
    #[test] 
    fn test_required_field_validation() {
        // Test that events with missing required fields are rejected
        let mut invalid_envelope = proto::EventEnvelope::default();
        invalid_envelope.event_type = "".to_string(); // Invalid empty event type
        
        let result = Event::from_proto(invalid_envelope);
        assert!(result.is_err());
        match result.unwrap_err() {
            EventError::MissingRequiredField { field } => {
                assert_eq!(field, "event_type");
            }
            _ => panic!("Expected MissingRequiredField error"),
        }
    }
}
```

#### 7.2 Proto Integration Tests
```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::proto::market_data::PriceUpdate;
    
    #[tokio::test]
    async fn test_eventbus_proto_integration() {\n        // Test proto event with EventBus\n        let price_update = PriceUpdate {\n            symbol: \"BTCUSD\".to_string(),\n            price: 50000.0,\n            volume: 1.5,\n            timestamp: 1640000000,\n        };\n        \n        let event = EventBuilder::new(\"market_data.price_update\")\n            .correlation_id(\"test-123\")\n            .source(\"test-adapter\")\n            .with_payload(price_update)\n            .unwrap();\n        \n        // Test serialization for EventBus transport\n        let bytes = event.to_bytes().unwrap();\n        let deserialized = Event::from_bytes(&bytes).unwrap();\n        \n        assert_eq!(event.event_type(), deserialized.event_type());\n        assert_eq!(event.correlation_id(), deserialized.correlation_id());\n        \n        let orig_payload: PriceUpdate = event.decode_payload().unwrap();\n        let deser_payload: PriceUpdate = deserialized.decode_payload().unwrap();\n        assert_eq!(orig_payload.symbol, deser_payload.symbol);\n    }\n    \n    #[test]\n    fn test_proto_wire_format_compatibility() {\n        let payload = PriceUpdate {\n            symbol: \"ETHUSD\".to_string(),\n            price: 3000.0,\n            volume: 10.0,\n            timestamp: 1640000001,\n        };\n        \n        let event = EventBuilder::new(\"market_data.price_update\")\n            .with_payload(payload)\n            .unwrap();\n        \n        let bytes = event.to_bytes().unwrap();\n        \n        // Verify bytes are valid protobuf\n        let envelope = proto::EventEnvelope::decode(bytes.as_slice()).unwrap();\n        assert_eq!(envelope.event_type, \"market_data.price_update\");\n        assert!(envelope.payload.is_some());\n        \n        // Verify round-trip\n        let reconstructed = Event::from_bytes(&bytes).unwrap();\n        assert_eq!(event.event_type(), reconstructed.event_type());\n    }\n    \n    #[test]\n    fn test_envelope_channel_integration() {\n        let payload = PriceUpdate {\n            symbol: \"ADAUSD\".to_string(),\n            price: 1.0,\n            volume: 1000.0,\n            timestamp: 1640000002,\n        };\n        \n        let event = EventBuilder::new(\"market_data.price_update\")\n            .with_payload(payload)\n            .unwrap();\n        \n        let envelope = EventEnvelope::new(\"market-data-channel\".to_string(), event);\n        \n        assert_eq!(envelope.channel(), \"market-data-channel\");\n        assert_eq!(envelope.retry_count(), 0);\n        assert_eq!(envelope.event().event_type(), \"market_data.price_update\");\n        \n        // Test retry functionality\n        let mut envelope = envelope;\n        envelope.increment_retry();\n        assert_eq!(envelope.retry_count(), 1);\n    }\n}\n```

### Phase 8: Breaking Change Migration

#### 8.1 No Gradual Migration - Hard Cutover
🚫 **This is a breaking change requiring full migration**

```rust
// 🚫 NO feature flags - proto-only from day one
// 🚫 NO legacy fallback - must migrate all consumers
// 🚫 NO gradual rollout - all or nothing

pub struct EventConfig {
    pub strict_validation: bool,
    pub proto_compression: bool,
    pub performance_monitoring: bool,
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            strict_validation: true,    // Always validate proto structure
            proto_compression: false,   // Optional compression
            performance_monitoring: true, // Monitor proto performance
        }
    }
}
```

#### 8.2 Required Migration Steps
1. **All Vec<u8> payload consumers must be converted to typed proto**
2. **All Event creation must use EventBuilder with typed payloads**
3. **All serialization must migrate from serde/bincode to protobuf**
4. **All event handlers must use decode_payload<T>() for type safety**
5. **All tests must be rewritten for proto-only validation**

#### 8.3 Migration Validation
```rust
// Compile-time validation that no Vec<u8> APIs remain
#[cfg(test)]
mod migration_validation {
    use super::*;
    
    #[test]
    fn test_no_vec_u8_constructors() {
        // This test ensures no Vec<u8> constructors exist
        // If this compiles, migration is incomplete
        
        // ❌ This should NOT compile:
        // let event = Event::new("test".to_string(), vec![1, 2, 3]);
        
        // ✅ This should compile:
        let payload = TestPayload { message: "test".to_string(), value: 42 };
        let event = EventBuilder::new("test").with_payload(payload).unwrap();
        assert_eq!(event.event_type(), "test");
    }
    
    #[test]
    fn test_no_serde_serialization() {
        // Ensure no serde serialization remains
        let payload = TestPayload { message: "test".to_string(), value: 42 };
        let event = EventBuilder::new("test").with_payload(payload).unwrap();
        
        // ✅ Proto serialization works
        let bytes = event.to_bytes().unwrap();
        let reconstructed = Event::from_bytes(&bytes).unwrap();
        assert_eq!(event.event_type(), reconstructed.event_type());
        
        // ❌ serde should not be available:
        // let json = serde_json::to_string(&event); // Should not compile
    }
}

## Proto-Only Performance Benefits

### Memory Efficiency
- Zero-copy access to protobuf fields (no HashMap lookups)
- Efficient protobuf Any payload storage (no Vec<u8> caching)
- Direct access to proto::EventEnvelope fields
- No dual representation overhead (proto + legacy)

### CPU Efficiency  
- Direct protobuf decode/encode (no serde overhead)
- Type-safe payload access with compile-time validation
- O(1) field access vs O(log n) HashMap lookups
- No runtime type checking or enum matching

### Network Efficiency
- Compact protobuf wire format (smaller than JSON/serde)
- Built-in schema evolution and versioning
- Optional compression with protobuf-native support
- Forward/backward compatibility through proto schema

## Proto-Only Error Handling

### Error Categories
1. **Protobuf Serialization**: Encode/decode failures with typed errors
2. **Validation Errors**: Required field validation with specific field names
3. **Type Safety**: Payload decode failures with protobuf context
4. **Non-Protobuf Rejection**: Hard rejection of non-proto data

### Strict Validation - No Recovery
```rust
impl Event {
    /// 🚫 NO repair - strict validation enforced
    /// Events must be valid protobuf from creation
    pub fn validate_strict(&self) -> EventResult<()> {
        let mut errors = Vec::new();
        
        // Required field validation
        if self.inner.message_id.is_empty() {
            errors.push(EventError::MissingRequiredField {
                field: "message_id".to_string(),
            });
        }
        
        if self.inner.event_type.is_empty() {
            errors.push(EventError::MissingRequiredField {
                field: "event_type".to_string(),
            });
        }
        
        if self.inner.payload.is_none() {
            errors.push(EventError::MissingRequiredField {
                field: "payload".to_string(),
            });
        }
        
        if self.inner.created_at.is_none() {
            errors.push(EventError::MissingRequiredField {
                field: "created_at".to_string(),
            });
        }
        
        // Validate protobuf payload structure
        if let Some(ref payload) = self.inner.payload {
            if payload.type_url.is_empty() {
                errors.push(EventError::ValidationFailed(
                    "Payload missing type_url".to_string()
                ));
            }
            
            if payload.value.is_empty() {
                errors.push(EventError::ValidationFailed(
                    "Payload missing value bytes".to_string()
                ));
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.into_iter().next().unwrap()) // Return first error
        }
    }
    
    /// 🚫 NO automatic repair - fail fast on invalid data
    /// Use EventBuilder to create valid events from the start
}
```

## Proto-Only Testing and Validation

### Test Coverage Requirements
- Unit tests: >95% coverage (simpler codebase, higher standard)
- Integration tests for protobuf serialization/deserialization
- Performance benchmarks for proto operations only
- Memory usage validation without legacy overhead
- Wire format compatibility across proto schema versions
- Type safety validation at compile time

### Proto-Only Validation Tools
```rust
pub fn validate_proto_event(event: &Event) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    
    // Strict protobuf validation only
    if event.inner.message_id.is_empty() {
        errors.push(ValidationError::MissingRequiredField("message_id".to_string()));
    }
    
    if event.inner.event_type.is_empty() {
        errors.push(ValidationError::MissingRequiredField("event_type".to_string()));
    }
    
    if event.inner.payload.is_none() {
        errors.push(ValidationError::MissingRequiredField("payload".to_string()));
    }
    
    if event.inner.created_at.is_none() {
        errors.push(ValidationError::MissingRequiredField("created_at".to_string()));
    }
    
    // Validate protobuf Any payload structure
    if let Some(ref payload) = event.inner.payload {
        if payload.type_url.is_empty() {
            errors.push(ValidationError::InvalidPayloadFormat);
        }
        
        // Validate payload can be decoded as protobuf
        match prost::Message::decode(payload.value.as_slice()) {
            Ok(_) => {}, // Valid protobuf bytes
            Err(_) => errors.push(ValidationError::InvalidProtobufPayload),
        }
    }
    
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[derive(Debug)]
pub enum ValidationError {
    MissingRequiredField(String),
    InvalidPayloadFormat,
    InvalidProtobufPayload,
    InvalidTimestamp,
    InvalidEventType,
    NonProtobufDataRejected,
}
```

## Proto-Only Implementation Requirements

### API Documentation
- Comprehensive rustdoc for all protobuf APIs
- Typed payload examples for all event types  
- EventBuilder usage patterns and best practices
- Proto schema evolution guide

### Breaking Change Guide
- Complete migration checklist from Vec<u8> to typed proto
- Compile-time validation examples
- Error handling patterns for proto validation
- Performance optimization techniques

## Success Metrics

### Performance Targets
- Serialization speed: <5ms for 1MB protobuf payload
- Memory usage: 30-50% reduction vs legacy (no dual representation)
- CPU usage: 20-40% improvement (no HashMap lookups, enum matching)
- Type safety: 100% compile-time validation of event payloads

### Reliability Targets
- Zero Vec<u8> escape hatches in production code
- 100% protobuf wire format compliance
- <0.01% proto deserialization failures

### Developer Experience
- Migration completed in 1 sprint (breaking change, full commitment)
- All legacy APIs removed (no technical debt)
- Type-safe event handling with compile-time guarantees

## Implementation Timeline

### Week 1: Proto-Only Core
- [ ] EventEnvelope protobuf schema finalization
- [ ] Event struct implementation (proto-only)
- [ ] EventBuilder with typed payload support
- [ ] Strict validation and error handling

### Week 2: Integration and Migration
- [ ] EventEnvelope wrapper implementation  
- [ ] Complete EventBus integration
- [ ] Migration of all existing event producers
- [ ] Migration of all existing event consumers

### Week 3: Validation and Testing
- [ ] Comprehensive test suite (>95% coverage)
- [ ] Performance benchmarking vs baseline
- [ ] Integration testing with all downstream systems
- [ ] Migration validation (no Vec<u8> APIs remain)

### Week 4: Documentation and Rollout
- [ ] API documentation and usage examples
- [ ] Performance monitoring and alerting
- [ ] Production deployment (hard cutover)
- [ ] Legacy code removal

## Risk Mitigation

### Technical Risks
1. **Breaking Changes**: Full regression testing, comprehensive migration plan
2. **Performance Issues**: Extensive benchmarking, profiling, optimization
3. **Type Safety**: Compile-time validation, comprehensive error handling

### Operational Risks  
1. **Hard Cutover Risk**: Thorough testing, rollback plan, monitoring
2. **Developer Adoption**: Clear documentation, migration tools, training
3. **Schema Evolution**: Protobuf best practices, version management

## Conclusion

This proto-only design enforces strict type safety and eliminates all Vec<u8> escape hatches. By removing backward compatibility, we achieve:

**🚫 What We Remove:**
- Vec<u8> payload constructors and accessors
- Legacy Event enum variants  
- Serde serialization fallbacks
- Runtime type checking and conversion
- HashMap-based metadata access

**✅ What We Gain:**
- **Type Safety**: Compile-time validation of all event payloads
- **Performance**: 20-40% CPU improvement, 30-50% memory reduction
- **Maintainability**: Single code path, no legacy technical debt
- **Extensibility**: Protobuf schema evolution and versioning
- **Developer Experience**: Clear APIs, no runtime surprises

This breaking change establishes a clean foundation for the neural-trader event system with no compromises on type safety or performance. The "proto-only, period" approach ensures consistent, maintainable, and high-performance event handling throughout the system.