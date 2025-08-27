//! Configuration Store - A hierarchical configuration management system
//! 
//! This crate provides a trait-based configuration store system that supports
//! hierarchical organization, inheritance, versioning, and multiple storage backends.

pub mod types;
pub mod traits;
pub mod stores;
pub mod security;
pub mod secure_async_store;

// Legacy modules for backward compatibility (but commented out to avoid conflicts)
// pub mod error;
// pub mod in_memory;
// pub mod redis_store;

// Re-export specification-compliant types
pub use types::{
    ConfigValue, ConfigError, ConfigTree, ConfigNode, 
    ConfigMetadata, ConfigVersion, ConfigSnapshot
};

pub use traits::{ConfigStore, ConfigTransaction, path_utils};

pub use stores::{InMemoryConfigStore};

// Re-export commonly used types
pub use serde_json::Value as JsonValue;
pub use std::sync::Arc;