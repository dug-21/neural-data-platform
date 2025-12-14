pub mod api;
pub mod config;
pub mod error;
pub mod ingestion;
pub mod pipeline;
pub mod response;

#[cfg(feature = "mcp")]
pub mod mcp;

pub use config::AppConfig;
pub use error::{ApiError, ApiResult};
pub use response::{ApiResponse, Meta};
