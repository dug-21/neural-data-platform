//! Event Publishing Module
//!
//! Domain-agnostic event publishing system for ML workflow notifications,
//! monitoring, and integration with external systems.

pub mod publisher;

pub use publisher::{EventPublisher};

// Type aliases for backward compatibility  
pub type MLProtoEvent = MLEvent;
pub type MLProtoEventType = MLEventType;

/// Proto channel configuration for ML operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtoChannelConfig {
    pub training: String,
    pub inference: String,
    pub model_lifecycle: String,
    pub feature_engineering: String,
    pub monitoring: String,
}

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// ML workflow event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLEvent {
    pub id: Uuid,
    pub event_type: MLEventType,
    pub job_id: Option<Uuid>,
    pub workflow_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub payload: serde_json::Value,
}

/// Types of ML events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MLEventType {
    // Training Events
    TrainingStarted,
    TrainingProgress,
    TrainingCompleted,
    TrainingFailed,
    TrainingCancelled,
    
    // Model Events
    ModelRegistered,
    ModelUpdated,
    ModelDeployed,
    ModelRetired,
    ModelVersionCreated,
    
    // Feature Events
    FeaturesExtracted,
    FeaturesStored,
    FeatureQualityAlert,
    FeatureDriftDetected,
    
    // System Events
    SystemHealthCheck,
    ResourceAlert,
    StorageAlert,
    SecurityEvent,
    
    // Custom Events
    Custom(String),
}

/// Event severity level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventSeverity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

/// Event configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventConfig {
    pub enabled: bool,
    pub backend: EventBackend,
    pub buffer_size: usize,
    pub batch_size: usize,
    pub flush_interval_ms: u64,
    pub retry_attempts: u32,
    pub enable_filtering: bool,
    pub filters: Vec<EventFilter>,
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: EventBackend::Memory,
            buffer_size: 1000,
            batch_size: 10,
            flush_interval_ms: 1000,
            retry_attempts: 3,
            enable_filtering: false,
            filters: Vec::new(),
        }
    }
}

/// Event backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventBackend {
    Memory,
    Redis { connection_string: String },
    Kafka { brokers: Vec<String>, topic: String },
    Webhook { url: String, headers: HashMap<String, String> },
    File { path: String },
}

/// Event filtering configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFilter {
    pub name: String,
    pub enabled: bool,
    pub event_types: Vec<MLEventType>,
    pub severity_levels: Vec<EventSeverity>,
    pub job_patterns: Vec<String>,
    pub workflow_patterns: Vec<String>,
}

/// Event statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventStats {
    pub total_events_published: u64,
    pub events_by_type: HashMap<String, u64>,
    pub events_by_severity: HashMap<String, u64>,
    pub publish_errors: u64,
    pub average_batch_size: f64,
    pub last_publish: Option<DateTime<Utc>>,
}

/// Event subscription for real-time notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSubscription {
    pub id: String,
    pub subscriber_id: String,
    pub event_types: Vec<MLEventType>,
    pub filters: Option<EventFilter>,
    pub callback_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub active: bool,
}

/// Event publishing trait for different backends
#[async_trait::async_trait]
pub trait EventBackendTrait: Send + Sync {
    /// Publish a single event
    async fn publish_event(&self, event: &MLEvent) -> Result<()>;
    
    /// Publish multiple events in a batch
    async fn publish_batch(&self, events: &[MLEvent]) -> Result<()>;
    
    /// Get backend statistics
    async fn get_stats(&self) -> Result<EventStats>;
    
    /// Health check for the backend
    async fn health_check(&self) -> Result<bool>;
}

/// Event listener trait for consuming events
#[async_trait::async_trait]
pub trait EventListener: Send + Sync {
    /// Handle an incoming event
    async fn handle_event(&self, event: &MLEvent) -> Result<()>;
    
    /// Get listener configuration
    fn get_config(&self) -> EventListenerConfig;
}

/// Event listener configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventListenerConfig {
    pub name: String,
    pub event_types: Vec<MLEventType>,
    pub batch_processing: bool,
    pub max_batch_size: usize,
    pub timeout_ms: u64,
}

/// Event notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub enabled: bool,
    pub channels: Vec<NotificationChannel>,
    pub templates: HashMap<String, MessageTemplate>,
    pub rate_limiting: RateLimitConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationChannel {
    Email { 
        smtp_server: String,
        from: String,
        to: Vec<String>,
    },
    Slack { 
        webhook_url: String,
        channel: String,
    },
    Discord {
        webhook_url: String,
    },
    Teams {
        webhook_url: String,
    },
    Custom {
        name: String,
        config: HashMap<String, String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageTemplate {
    pub subject: Option<String>,
    pub body: String,
    pub format: MessageFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageFormat {
    PlainText,
    Markdown,
    HTML,
    JSON,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub max_events_per_minute: u32,
    pub max_events_per_hour: u32,
    pub burst_limit: u32,
}

/// Event aggregation for analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventAggregation {
    pub time_window: AggregationWindow,
    pub event_type: MLEventType,
    pub count: u64,
    pub first_occurrence: DateTime<Utc>,
    pub last_occurrence: DateTime<Utc>,
    pub related_jobs: Vec<Uuid>,
    pub related_workflows: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AggregationWindow {
    Minute,
    FiveMinutes,
    FifteenMinutes,
    Hour,
    Day,
    Week,
}

/// Event query for searching and filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventQuery {
    pub event_types: Option<Vec<MLEventType>>,
    pub job_ids: Option<Vec<Uuid>>,
    pub workflow_ids: Option<Vec<String>>,
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub severity_levels: Option<Vec<EventSeverity>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub order_by: Option<EventOrderBy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventOrderBy {
    Timestamp,
    EventType,
    Severity,
    JobId,
}

impl Default for EventQuery {
    fn default() -> Self {
        Self {
            event_types: None,
            job_ids: None,
            workflow_ids: None,
            time_range: None,
            severity_levels: None,
            limit: Some(100),
            offset: None,
            order_by: Some(EventOrderBy::Timestamp),
        }
    }
}

/// Utility functions for event handling
pub mod utils {
    use super::*;
    
    /// Create a training started event
    pub fn create_training_started_event(job_id: Uuid, workflow_id: String) -> MLEvent {
        MLEvent {
            id: Uuid::new_v4(),
            event_type: MLEventType::TrainingStarted,
            job_id: Some(job_id),
            workflow_id: Some(workflow_id.clone()),
            timestamp: Utc::now(),
            payload: serde_json::json!({
                "message": "Training job started",
                "job_id": job_id,
                "workflow_id": workflow_id
            }),
        }
    }
    
    /// Create a training completed event with metrics
    pub fn create_training_completed_event(
        job_id: Uuid, 
        workflow_id: String,
        metrics: HashMap<String, f64>
    ) -> MLEvent {
        MLEvent {
            id: Uuid::new_v4(),
            event_type: MLEventType::TrainingCompleted,
            job_id: Some(job_id),
            workflow_id: Some(workflow_id.clone()),
            timestamp: Utc::now(),
            payload: serde_json::json!({
                "message": "Training job completed successfully",
                "job_id": job_id,
                "workflow_id": workflow_id,
                "metrics": metrics
            }),
        }
    }
    
    /// Create a model registered event
    pub fn create_model_registered_event(model_id: String, model_name: String) -> MLEvent {
        MLEvent {
            id: Uuid::new_v4(),
            event_type: MLEventType::ModelRegistered,
            job_id: None,
            workflow_id: None,
            timestamp: Utc::now(),
            payload: serde_json::json!({
                "message": "New model registered",
                "model_id": model_id,
                "model_name": model_name
            }),
        }
    }
    
    /// Create a system health check event
    pub fn create_health_check_event(component: String, healthy: bool, details: Option<String>) -> MLEvent {
        MLEvent {
            id: Uuid::new_v4(),
            event_type: MLEventType::SystemHealthCheck,
            job_id: None,
            workflow_id: None,
            timestamp: Utc::now(),
            payload: serde_json::json!({
                "message": format!("Health check for {}", component),
                "component": component,
                "healthy": healthy,
                "details": details
            }),
        }
    }
    
    /// Create a resource alert event
    pub fn create_resource_alert_event(resource: String, usage: f64, threshold: f64) -> MLEvent {
        MLEvent {
            id: Uuid::new_v4(),
            event_type: MLEventType::ResourceAlert,
            job_id: None,
            workflow_id: None,
            timestamp: Utc::now(),
            payload: serde_json::json!({
                "message": format!("Resource usage alert for {}", resource),
                "resource": resource,
                "usage_percent": usage,
                "threshold_percent": threshold
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_event_creation() {
        let event = utils::create_training_started_event(
            Uuid::new_v4(),
            "test-workflow".to_string()
        );
        
        assert!(matches!(event.event_type, MLEventType::TrainingStarted));
        assert!(event.job_id.is_some());
        assert!(event.workflow_id.is_some());
    }
    
    #[test]
    fn test_event_serialization() {
        let event = MLEvent {
            id: Uuid::new_v4(),
            event_type: MLEventType::ModelRegistered,
            job_id: None,
            workflow_id: Some("test".to_string()),
            timestamp: Utc::now(),
            payload: serde_json::json!({"test": "data"}),
        };
        
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: MLEvent = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.id, event.id);
        assert!(matches!(deserialized.event_type, MLEventType::ModelRegistered));
    }
    
    #[test]
    fn test_event_config_default() {
        let config = EventConfig::default();
        assert!(config.enabled);
        assert!(matches!(config.backend, EventBackend::Memory));
        assert_eq!(config.buffer_size, 1000);
    }
}