pub mod grpc_traits;
pub mod mocks;

// Re-export the main traits for easy access
pub use grpc_traits::*;
pub use mocks::*;

// Common types used across all interfaces
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use thiserror::Error;

#[cfg(feature = "grpc")]
use tonic::{Code, Status};

// Include generated protobuf types when gRPC is enabled
#[cfg(feature = "grpc")]
pub mod proto {
    // TODO: Enable when proto generation is working
    pub mod common {}
    pub mod market_data {}
    pub mod features {}
    pub mod models {}
    pub mod trading {}
}

// Stub proto module for when gRPC is disabled
#[cfg(not(feature = "grpc"))]
pub mod proto {
    pub mod common {}
    pub mod market_data {}
    pub mod features {}
    pub mod models {}
    pub mod trading {}
}

// Common error types
#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },
    
    #[error("Resource not found: {resource_type} with id {resource_id}")]
    NotFound { resource_type: String, resource_id: String },
    
    #[error("Service unavailable: {service_name} - {reason}")]
    ServiceUnavailable { service_name: String, reason: String },
    
    #[error("Rate limit exceeded: {limit} requests per {window}")]
    RateLimitExceeded { limit: u32, window: String },
    
    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },
    
    #[error("Authorization denied: {required_permission}")]
    AuthorizationDenied { required_permission: String },
    
    #[error("Data validation failed: {field} - {reason}")]
    ValidationFailed { field: String, reason: String },
    
    #[error("Internal server error: {message}")]
    Internal { message: String },
    
    #[error("External dependency error: {dependency} - {error}")]
    ExternalDependency { dependency: String, error: String },
    
    #[error("Timeout: operation took longer than {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
}

impl ServiceError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, 
            ServiceError::ServiceUnavailable { .. } |
            ServiceError::ExternalDependency { .. } |
            ServiceError::Timeout { .. }
        )
    }
    
    pub fn retry_after_seconds(&self) -> Option<u32> {
        match self {
            ServiceError::RateLimitExceeded { .. } => Some(60),
            ServiceError::ServiceUnavailable { .. } => Some(30),
            _ => None,
        }
    }
}

#[cfg(feature = "grpc")]
impl From<ServiceError> for Status {
    fn from(err: ServiceError) -> Self {
        let (code, message) = match err {
            ServiceError::InvalidRequest { message } => (Code::InvalidArgument, message),
            ServiceError::NotFound { .. } => (Code::NotFound, err.to_string()),
            ServiceError::ServiceUnavailable { .. } => (Code::Unavailable, err.to_string()),
            ServiceError::RateLimitExceeded { .. } => (Code::ResourceExhausted, err.to_string()),
            ServiceError::AuthenticationFailed { .. } => (Code::Unauthenticated, err.to_string()),
            ServiceError::AuthorizationDenied { .. } => (Code::PermissionDenied, err.to_string()),
            ServiceError::ValidationFailed { .. } => (Code::InvalidArgument, err.to_string()),
            ServiceError::Internal { message } => (Code::Internal, message),
            ServiceError::ExternalDependency { .. } => (Code::Unavailable, err.to_string()),
            ServiceError::Timeout { .. } => (Code::DeadlineExceeded, err.to_string()),
        };
        
        Status::new(code, message)
    }
}

// Result type alias for convenience
pub type ServiceResult<T> = Result<T, ServiceError>;

// Common data structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: chrono::DateTime<chrono::Utc>,
    pub end: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol(pub String);

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Symbol {
    fn from(s: String) -> Self {
        Symbol(s)
    }
}

impl From<&str> for Symbol {
    fn from(s: &str) -> Self {
        Symbol(s.to_string())
    }
}

// Interface status tracking
#[derive(Debug, Clone)]
pub struct InterfaceHealth {
    pub service_name: String,
    pub is_healthy: bool,
    pub latency_ms: f64,
    pub error_rate: f64,
    pub last_check: chrono::DateTime<chrono::Utc>,
    pub details: HashMap<String, String>,
}

// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay: std::time::Duration,
    pub max_delay: std::time::Duration,
    pub backoff_multiplier: f64,
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: std::time::Duration::from_millis(100),
            max_delay: std::time::Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

// Test utilities
#[cfg(test)]
pub mod test_utils {
    use super::*;
    
    pub fn create_test_symbol() -> Symbol {
        Symbol("AAPL".to_string())
    }
    
    pub fn create_test_time_range() -> TimeRange {
        let now = chrono::Utc::now();
        TimeRange {
            start: now - chrono::Duration::hours(1),
            end: now,
        }
    }
    
    pub fn create_test_service_error() -> ServiceError {
        ServiceError::Internal {
            message: "Test error".to_string(),
        }
    }
}