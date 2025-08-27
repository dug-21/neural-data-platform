//! Neural ML-Ops Library
//!
//! Domain-agnostic ML Operations platform providing:
//! - Training coordination and scheduling
//! - Feature engineering and storage
//! - Model registry and versioning  
//! - Event publishing for ML workflows

pub mod training;
pub mod features;
pub mod models;
pub mod events;

// Re-export commonly used types (commented out until modules are properly structured)
// pub use training::TrainingCoordinator;
// pub use features::FeatureStoreConfig;
// pub use models::ModelRegistryConfig;
// pub use events::EventPublisher;

#[derive(Debug, Clone)]
pub struct MLOpsConfig {
    pub feature_store_backend: String,
    pub model_registry_backend: String,
    pub training_backend: String,
    pub event_publisher_backend: String,
}

impl Default for MLOpsConfig {
    fn default() -> Self {
        Self {
            feature_store_backend: "memory".to_string(),
            model_registry_backend: "filesystem".to_string(),
            training_backend: "local".to_string(),
            event_publisher_backend: "memory".to_string(),
        }
    }
}