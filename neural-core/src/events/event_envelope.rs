//! Proto-only Event Envelope implementation
//!
//! This module provides the EventEnvelope implementation that wraps proto-only Events
//! for transport through the EventBus system. NO Vec<u8> payloads allowed.

use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use prost::Message;

use super::event::Event;

/// Event envelope containing a proto-only Event with delivery metadata
/// 
/// This envelope provides additional transport and delivery metadata around
/// the core Event, while maintaining proto-only compliance.
#[derive(Debug, Clone)]
pub struct EventEnvelope {
    /// Unique identifier for this envelope
    pub envelope_id: Uuid,
    
    /// Channel this event was delivered from
    pub channel: String,
    
    /// The proto-only event
    pub event: Event,
    
    /// Number of retry attempts
    pub retry_count: u32,
    
    /// Timestamp when the envelope was created
    pub delivered_at: DateTime<Utc>,
    
    /// Transport-level metadata
    pub transport_metadata: HashMap<String, String>,
    
    /// Processing status
    pub processing_status: ProcessingStatus,
}

/// Processing status of the event
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessingStatus {
    /// Event is pending processing
    Pending,
    /// Event is currently being processed
    Processing,
    /// Event has been successfully processed
    Completed,
    /// Event processing failed
    Failed(String),
    /// Event was rejected due to validation failure
    Rejected(String),
    /// Event was discarded (e.g., exceeded max retries)
    Discarded,
}

impl EventEnvelope {
    /// Create a new event envelope
    pub fn new(channel: String, event: Event) -> Result<Self, crate::errors::CoreError> {
        // Validate the event before wrapping
        event.validate()?;
        
        Ok(Self {
            envelope_id: Uuid::new_v4(),
            channel,
            event,
            retry_count: 0,
            delivered_at: Utc::now(),
            transport_metadata: HashMap::new(),
            processing_status: ProcessingStatus::Pending,
        })
    }
    
    /// Create envelope with custom envelope ID
    pub fn with_envelope_id(mut self, envelope_id: Uuid) -> Self {
        self.envelope_id = envelope_id;
        self
    }
    
    /// Add transport metadata
    pub fn with_transport_metadata(mut self, key: &str, value: &str) -> Self {
        self.transport_metadata.insert(key.to_string(), value.to_string());
        self
    }
    
    /// Set processing status
    pub fn with_status(mut self, status: ProcessingStatus) -> Self {
        self.processing_status = status;
        self
    }
    
    /// Increment retry count and update delivered timestamp
    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
        self.delivered_at = Utc::now();
        self.processing_status = ProcessingStatus::Pending;
    }
    
    /// Mark as processing
    pub fn mark_processing(&mut self) {
        self.processing_status = ProcessingStatus::Processing;
    }
    
    /// Mark as completed
    pub fn mark_completed(&mut self) {
        self.processing_status = ProcessingStatus::Completed;
    }
    
    /// Mark as failed with error message
    pub fn mark_failed(&mut self, error: &str) {
        self.processing_status = ProcessingStatus::Failed(error.to_string());
    }
    
    /// Mark as rejected with reason
    pub fn mark_rejected(&mut self, reason: &str) {
        self.processing_status = ProcessingStatus::Rejected(reason.to_string());
    }
    
    /// Mark as discarded
    pub fn mark_discarded(&mut self) {
        self.processing_status = ProcessingStatus::Discarded;
    }
    
    /// Check if envelope has exceeded max retry attempts
    pub fn has_exceeded_max_retries(&self, max_retries: u32) -> bool {
        self.retry_count > max_retries
    }
    
    /// Check if envelope is ready for retry
    pub fn is_ready_for_retry(&self) -> bool {
        matches!(self.processing_status, ProcessingStatus::Failed(_))
    }
    
    /// Check if processing is complete (either successfully or permanently failed)
    pub fn is_processing_complete(&self) -> bool {
        matches!(
            self.processing_status,
            ProcessingStatus::Completed | ProcessingStatus::Rejected(_) | ProcessingStatus::Discarded
        )
    }
    
    /// Get the event type (delegation to inner event)
    pub fn event_type(&self) -> &str {
        self.event.event_type()
    }
    
    /// Get the message ID (delegation to inner event)
    pub fn message_id(&self) -> &str {
        self.event.message_id()
    }
    
    /// Get the correlation ID (delegation to inner event)
    pub fn correlation_id(&self) -> &str {
        self.event.correlation_id()
    }
    
    /// Get the event source
    pub fn source(&self) -> &str {
        self.event.source()
    }
    
    /// Get the event domain
    pub fn domain(&self) -> &str {
        self.event.domain()
    }
    
    /// Get the event quality score
    pub fn quality_score(&self) -> f32 {
        self.event.quality_score()
    }
    
    /// Deserialize the event payload
    pub fn payload<T: Message + Default>(&self) -> Result<T, crate::errors::CoreError> {
        self.event.payload()
    }
    
    /// Convert to proto bytes for transport
    pub fn to_proto_bytes(&self) -> Vec<u8> {
        self.event.to_bytes()
    }
    
    /// Validate the entire envelope
    pub fn validate(&self) -> Result<(), crate::errors::CoreError> {
        // Validate the inner event
        self.event.validate()?;
        
        // Validate envelope-specific fields
        if self.channel.is_empty() {
            return Err(crate::errors::CoreError::EventError("channel cannot be empty".to_string()));
        }
        
        Ok(())
    }
    
    /// Create a new envelope from proto bytes
    pub fn from_proto_bytes(
        envelope_id: Uuid,
        channel: String,
        bytes: &[u8]
    ) -> Result<Self, crate::errors::CoreError> {
        let event = Event::from_bytes(bytes)?;
        
        Ok(Self {
            envelope_id,
            channel,
            event,
            retry_count: 0,
            delivered_at: Utc::now(),
            transport_metadata: HashMap::new(),
            processing_status: ProcessingStatus::Pending,
        })
    }
    
    /// Clone the envelope with a new envelope ID (for retry scenarios)
    pub fn clone_for_retry(&self) -> Self {
        let mut cloned = self.clone();
        cloned.envelope_id = Uuid::new_v4();
        cloned.delivered_at = Utc::now();
        cloned.processing_status = ProcessingStatus::Pending;
        cloned
    }
}

/// Batch envelope for processing multiple events together
#[derive(Debug, Clone)]
pub struct BatchEventEnvelope {
    /// Unique identifier for this batch
    pub batch_id: Uuid,
    
    /// Channel all events were delivered from
    pub channel: String,
    
    /// Collection of event envelopes
    pub envelopes: Vec<EventEnvelope>,
    
    /// Batch metadata
    pub metadata: HashMap<String, String>,
    
    /// Timestamp when the batch was created
    pub created_at: DateTime<Utc>,
    
    /// Overall batch processing status
    pub status: BatchProcessingStatus,
}

/// Batch processing status
#[derive(Debug, Clone, PartialEq)]
pub enum BatchProcessingStatus {
    /// All events are pending
    Pending,
    /// Some events are being processed
    PartiallyProcessed(usize, usize), // (processed_count, total_count)
    /// All events completed successfully
    AllCompleted,
    /// Some events failed
    PartiallyFailed(usize, usize), // (failed_count, total_count)
    /// All events failed
    AllFailed,
}

impl BatchEventEnvelope {
    /// Create a new batch envelope
    pub fn new(channel: String, envelopes: Vec<EventEnvelope>) -> Result<Self, crate::errors::CoreError> {
        if envelopes.is_empty() {
            return Err(crate::errors::CoreError::EventError("Batch cannot be empty".to_string()));
        }
        
        // Validate all envelopes
        for envelope in &envelopes {
            envelope.validate()?;
        }
        
        Ok(Self {
            batch_id: Uuid::new_v4(),
            channel,
            envelopes,
            metadata: HashMap::new(),
            created_at: Utc::now(),
            status: BatchProcessingStatus::Pending,
        })
    }
    
    /// Add metadata to the batch
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
    
    /// Get the number of envelopes in the batch
    pub fn size(&self) -> usize {
        self.envelopes.len()
    }
    
    /// Update batch status based on individual envelope statuses
    pub fn update_batch_status(&mut self) {
        let total = self.envelopes.len();
        let completed = self.envelopes.iter()
            .filter(|e| e.processing_status == ProcessingStatus::Completed)
            .count();
        let failed = self.envelopes.iter()
            .filter(|e| matches!(e.processing_status, ProcessingStatus::Failed(_) | ProcessingStatus::Rejected(_) | ProcessingStatus::Discarded))
            .count();
        let processing = self.envelopes.iter()
            .filter(|e| e.processing_status == ProcessingStatus::Processing)
            .count();
        
        self.status = match (completed, failed, processing) {
            (c, _, _) if c == total => BatchProcessingStatus::AllCompleted,
            (_, f, _) if f == total => BatchProcessingStatus::AllFailed,
            (_, f, _) if f > 0 => BatchProcessingStatus::PartiallyFailed(f, total),
            (c, _, _) if c > 0 || processing > 0 => BatchProcessingStatus::PartiallyProcessed(c + processing, total),
            _ => BatchProcessingStatus::Pending,
        };
    }
    
    /// Check if the entire batch is complete
    pub fn is_complete(&self) -> bool {
        matches!(
            self.status,
            BatchProcessingStatus::AllCompleted | BatchProcessingStatus::AllFailed
        )
    }
    
    /// Get envelopes that are ready for retry
    pub fn get_retry_envelopes(&self, max_retries: u32) -> Vec<&EventEnvelope> {
        self.envelopes.iter()
            .filter(|e| e.is_ready_for_retry() && !e.has_exceeded_max_retries(max_retries))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eventbus::{ProtoEvent, ProtoMessage};
    
    // Mock proto message for testing
    #[derive(Clone, prost::Message)]
    struct TestMessage {
        #[prost(string, tag = "1")]
        content: String,
    }
    
    impl ProtoMessage for TestMessage {
        fn proto_type_name() -> &'static str {
            "neural_trader.test.TestMessage"
        }
    }
    
    #[test]
    fn test_event_envelope_creation() {
        let test_msg = TestMessage {
            content: "test".to_string(),
        };
        
        let event = ProtoEvent::new(test_msg);
        
        let envelope = EventEnvelope::new("test-channel".to_string(), event)
            .expect("Should create envelope");
        
        assert_eq!(envelope.channel, "test-channel");
        assert_eq!(envelope.retry_count, 0);
        assert_eq!(envelope.processing_status, ProcessingStatus::Pending);
        assert!(envelope.validate().is_ok());
    }
    
    #[test]
    fn test_envelope_retry_logic() {
        let test_msg = TestMessage {
            content: "retry test".to_string(),
        };
        
        let event = ProtoEvent::new(test_msg);
        
        let mut envelope = EventEnvelope::new("test-channel".to_string(), event)
            .expect("Should create envelope");
        
        // Test initial state
        assert_eq!(envelope.retry_count, 0);
        assert!(!envelope.has_exceeded_max_retries(3));
        
        // Test retry increment
        envelope.increment_retry();
        assert_eq!(envelope.retry_count, 1);
        
        // Test max retries
        envelope.retry_count = 5;
        assert!(envelope.has_exceeded_max_retries(3));
    }
    
    #[test]
    fn test_processing_status_transitions() {
        let test_msg = TestMessage {
            content: "status test".to_string(),
        };
        
        let event = ProtoEvent::new(test_msg);
        
        let mut envelope = EventEnvelope::new("test-channel".to_string(), event)
            .expect("Should create envelope");
        
        // Test status transitions
        envelope.mark_processing();
        assert_eq!(envelope.processing_status, ProcessingStatus::Processing);
        
        envelope.mark_completed();
        assert_eq!(envelope.processing_status, ProcessingStatus::Completed);
        assert!(envelope.is_processing_complete());
        
        envelope.mark_failed("test error");
        assert!(matches!(envelope.processing_status, ProcessingStatus::Failed(_)));
        assert!(envelope.is_ready_for_retry());
        
        envelope.mark_rejected("validation failed");
        assert!(matches!(envelope.processing_status, ProcessingStatus::Rejected(_)));
        assert!(envelope.is_processing_complete());
    }
    
    #[test]
    fn test_batch_envelope() {
        let mut envelopes = Vec::new();
        for i in 0..3 {
            let test_msg = TestMessage {
                content: format!("batch test {}", i),
            };
            let event = Event::new("test.TestMessage", test_msg, "source", "domain")
                .expect("Should create event");
            let envelope = EventEnvelope::new("batch-channel".to_string(), event)
                .expect("Should create envelope");
            envelopes.push(envelope);
        }
        
        let mut batch = BatchEventEnvelope::new("batch-channel".to_string(), envelopes)
            .expect("Should create batch");
        
        assert_eq!(batch.size(), 3);
        assert_eq!(batch.status, BatchProcessingStatus::Pending);
        
        // Mark some as completed
        batch.envelopes[0].mark_completed();
        batch.envelopes[1].mark_processing();
        batch.update_batch_status();
        
        assert!(matches!(batch.status, BatchProcessingStatus::PartiallyProcessed(_, _)));
        
        // Mark all as completed
        for envelope in &mut batch.envelopes {
            envelope.mark_completed();
        }
        batch.update_batch_status();
        
        assert_eq!(batch.status, BatchProcessingStatus::AllCompleted);
        assert!(batch.is_complete());
    }
    
    #[test]
    fn test_envelope_serialization() {
        let test_msg = TestMessage {
            content: "serialization test".to_string(),
        };
        
        let event = ProtoEvent::new(test_msg);
        
        let envelope = EventEnvelope::new("test-channel".to_string(), event)
            .expect("Should create envelope")
            .with_transport_metadata("transport-key", "transport-value");
        
        let bytes = envelope.to_proto_bytes();
        assert!(!bytes.is_empty());
        
        // Test round-trip (limited since we don't serialize transport metadata)
        let recovered_event = Event::from_bytes(&bytes).expect("Should deserialize");
        assert_eq!(recovered_event.event_type(), "test.TestMessage");
    }
}