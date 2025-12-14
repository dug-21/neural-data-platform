//! Config Client - Thin etcd wrapper for type-safe configuration
//!
//! # Example
//! ```rust,ignore
//! use config_client::ConfigClient;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct MqttConfig {
//!     broker_url: String,
//!     port: u16,
//! }
//!
//! let client = ConfigClient::new(&["http://localhost:2379"]).await?;
//! let mqtt: MqttConfig = client.get("/air-quality/mqtt").await?;
//! ```

mod client;
mod error;
mod watch;

pub use client::ConfigClient;
pub use error::ConfigError;
pub use watch::WatchHandle;

// Re-export for convenience
pub use serde_json::Value as JsonValue;
