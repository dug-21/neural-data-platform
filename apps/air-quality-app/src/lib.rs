pub mod api;
pub mod config;
pub mod config_etcd;
pub mod config_sync;
pub mod coordinator;
pub mod error;
pub mod ingestion;
pub mod pipeline;
pub mod response;
pub mod stream_integration;

#[cfg(feature = "mcp")]
pub mod mcp;

pub use config::AppConfig;
pub use config_etcd::{load_from_etcd, EtcdAppConfig};
pub use error::{ApiError, ApiResult};
pub use response::{ApiResponse, Meta};
