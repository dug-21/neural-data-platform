//! Proto types for ML Operations EventBus integration
//!
//! ALL ML events MUST be protobuf messages. JSON and Vec<u8> are BANNED.

use anyhow::{Result, Context};
use chrono::{DateTime, Utc};
use prost::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use neural_core::eventbus::{
    types::{ProtoEvent, ProtoMessage},
    error::EventBusError,
};

/// Proto-only ML Event wrapper
#[derive(Debug, Clone)]
pub struct MLProtoEvent {
    pub event_id: String,
    pub event_type: MLProtoEventType,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
    pub proto_payload: Box<dyn ProtoMessage>,
}

/// ML Proto Event Types (strongly typed)
#[derive(Debug, Clone, PartialEq)]
pub enum MLProtoEventType {
    TrainingStarted,
    TrainingCompleted,
    TrainingFailed,
    InferenceRequested,
    InferenceCompleted,
    ModelRegistered,
    ModelDeployed,
    ModelRetired,
    FeatureExtracted,
    FeatureStored,
    MetricsCollected,
    AlertGenerated,
}

impl MLProtoEvent {
    /// Create ML proto event from typed proto event
    pub fn from_proto_event<T: ProtoMessage>(event: ProtoEvent<T>) -> Result<Self> {
        let event_type = match T::proto_type_name() {
            "neural_ml.TrainingStartedEvent" => MLProtoEventType::TrainingStarted,
            "neural_ml.TrainingCompletedEvent" => MLProtoEventType::TrainingCompleted,
            "neural_ml.TrainingFailedEvent" => MLProtoEventType::TrainingFailed,
            "neural_ml.InferenceRequestedEvent" => MLProtoEventType::InferenceRequested,
            "neural_ml.InferenceCompletedEvent" => MLProtoEventType::InferenceCompleted,
            "neural_ml.ModelRegisteredEvent" => MLProtoEventType::ModelRegistered,
            "neural_ml.ModelDeployedEvent" => MLProtoEventType::ModelDeployed,
            "neural_ml.ModelRetiredEvent" => MLProtoEventType::ModelRetired,
            "neural_ml.FeatureExtractedEvent" => MLProtoEventType::FeatureExtracted,
            "neural_ml.FeatureStoredEvent" => MLProtoEventType::FeatureStored,
            "neural_ml.MetricsCollectedEvent" => MLProtoEventType::MetricsCollected,
            "neural_ml.AlertGeneratedEvent" => MLProtoEventType::AlertGenerated,
            _ => return Err(anyhow::anyhow!("Unsupported proto type: {}", T::proto_type_name())),
        };
        
        Ok(Self {
            event_id: Uuid::new_v4().to_string(),
            event_type,
            timestamp: DateTime::from_timestamp(event.timestamp, 0)
                .unwrap_or_else(|| Utc::now()),
            metadata: event.metadata,
            proto_payload: Box::new(event.message),
        })
    }
    
    /// Get the appropriate channel for this event type
    pub fn get_channel(&self, channels: &ProtoChannelConfig) -> &str {
        match self.event_type {
            MLProtoEventType::TrainingStarted | 
            MLProtoEventType::TrainingCompleted | 
            MLProtoEventType::TrainingFailed => &channels.training,
            
            MLProtoEventType::InferenceRequested | 
            MLProtoEventType::InferenceCompleted => &channels.inference,
            
            MLProtoEventType::ModelRegistered | 
            MLProtoEventType::ModelDeployed | 
            MLProtoEventType::ModelRetired => &channels.model_lifecycle,
            
            MLProtoEventType::FeatureExtracted | 
            MLProtoEventType::FeatureStored => &channels.feature_engineering,
            
            MLProtoEventType::MetricsCollected | 
            MLProtoEventType::AlertGenerated => &channels.monitoring,
        }
    }
}

/// Proto channel configuration for ML operations
#[derive(Debug, Clone)]
pub struct ProtoChannelConfig {
    pub training: String,
    pub inference: String,
    pub model_lifecycle: String,
    pub feature_engineering: String,
    pub monitoring: String,
}

// Proto message definitions (would normally be generated from .proto files)

/// Training started event
#[derive(Clone, Debug, prost::Message)]
pub struct TrainingStartedEvent {
    #[prost(string, tag = "1")]
    pub job_id: String,
    #[prost(string, tag = "2")]
    pub model_type: String,
    #[prost(string, tag = "3")]
    pub dataset_path: String,
    #[prost(int64, tag = "4")]
    pub started_at: i64,
}

impl ProtoMessage for TrainingStartedEvent {
    fn proto_type_name() -> &'static str {
        "neural_ml.TrainingStartedEvent"
    }
}

/// Training completed event
#[derive(Clone, Debug, prost::Message)]
pub struct TrainingCompletedEvent {
    #[prost(string, tag = "1")]
    pub job_id: String,
    #[prost(string, tag = "2")]
    pub model_id: String,
    #[prost(double, tag = "3")]
    pub final_accuracy: f64,
    #[prost(int64, tag = "4")]
    pub training_duration_ms: i64,
    #[prost(int64, tag = "5")]
    pub completed_at: i64,
}

impl ProtoMessage for TrainingCompletedEvent {
    fn proto_type_name() -> &'static str {
        "neural_ml.TrainingCompletedEvent"
    }
}

/// Model registered event
#[derive(Clone, Debug, prost::Message)]
pub struct ModelRegisteredEvent {
    #[prost(string, tag = "1")]
    pub model_id: String,
    #[prost(string, tag = "2")]
    pub model_name: String,
    #[prost(string, tag = "3")]
    pub version: String,
    #[prost(string, tag = "4")]
    pub model_type: String,
    #[prost(int64, tag = "5")]
    pub registered_at: i64,
}

impl ProtoMessage for ModelRegisteredEvent {
    fn proto_type_name() -> &'static str {
        "neural_ml.ModelRegisteredEvent"
    }
}

/// Inference completed event
#[derive(Clone, Debug, prost::Message)]
pub struct InferenceCompletedEvent {
    #[prost(string, tag = "1")]
    pub inference_id: String,
    #[prost(string, tag = "2")]
    pub model_id: String,
    #[prost(bytes, tag = "3")]
    pub result_data: Vec<u8>,
    #[prost(double, tag = "4")]
    pub confidence: f64,
    #[prost(int64, tag = "5")]
    pub inference_time_ms: i64,
    #[prost(int64, tag = "6")]
    pub completed_at: i64,
}

impl ProtoMessage for InferenceCompletedEvent {
    fn proto_type_name() -> &'static str {
        "neural_ml.InferenceCompletedEvent"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neural_core::eventbus::types::ProtoEvent;
    
    #[test]
    fn test_ml_proto_event_creation() {
        let training_event = TrainingStartedEvent {
            job_id: "job-123".to_string(),
            model_type: "neural_network".to_string(),
            dataset_path: "/data/training.csv".to_string(),
            started_at: 1640995200,
        };
        
        let proto_event = ProtoEvent::new(training_event);
        let ml_proto_event = MLProtoEvent::from_proto_event(proto_event).unwrap();
        
        assert_eq!(ml_proto_event.event_type, MLProtoEventType::TrainingStarted);
        assert!(!ml_proto_event.event_id.is_empty());
    }
    
    #[test]
    fn test_channel_routing() {
        let channels = ProtoChannelConfig {
            training: "ml_training_proto".to_string(),
            inference: "ml_inference_proto".to_string(),
            model_lifecycle: "ml_models_proto".to_string(),
            feature_engineering: "ml_features_proto".to_string(),
            monitoring: "ml_monitoring_proto".to_string(),
        };
        
        let training_event = MLProtoEvent {
            event_id: "test-123".to_string(),
            event_type: MLProtoEventType::TrainingStarted,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
            proto_payload: Box::new(TrainingStartedEvent {
                job_id: "job-123".to_string(),
                model_type: "test".to_string(),
                dataset_path: "/test".to_string(),
                started_at: 1640995200,
            }),
        };
        
        assert_eq!(training_event.get_channel(&channels), "ml_training_proto");
    }
}