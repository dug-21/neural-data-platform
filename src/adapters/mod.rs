//! Data source adapters module
//!
//! This module provides adapters for various data sources including
//! TimescaleDB for historical data and Redis for real-time streaming.

pub mod daa_service;
pub mod enhanced_neural_adapter;
pub mod errors;
pub mod fallback_manager;
pub mod ffi_wrapper;
pub mod health_monitor;
pub mod integration_bridge;
pub mod model_rollback;
pub mod model_storage;
pub mod neural;

// neuro_divergent module has been removed
// Use enhanced_neural_adapter with FANN predictor instead

pub mod redis;
pub mod timescale;
pub mod vendor_bridge;

use async_trait::async_trait;

// Re-export enhanced error handling
pub use errors::{
    AdapterError, CircuitBreaker, CircuitBreakerConfig, DefaultErrorHandler, ErrorContext,
    ErrorHandler, ErrorMonitoringEvent, ErrorSeverity, FallbackConfig, HealthCheckResult,
    HealthMetrics, RecoveryStrategy,
};

// Re-export health monitoring
pub use health_monitor::{
    BasicHealthChecker, HealthChecker, HealthMonitor, HealthMonitorConfig, HealthStatus,
    SystemHealthSummary, SystemStatus,
};

// Re-export fallback management
pub use fallback_manager::{
    FallbackManager, FallbackMetrics, FallbackResult, FallbackStrategy, ModelUsageStats,
    UltimateFallbackStrategy,
};

// Enhanced neural adapter is internal - access must go through neural::NeuralPredictor
// DO NOT EXPORT: EnhancedNeuralAdapter to prevent bypassing central routing

// Re-export model storage
pub use model_storage::{
    CheckpointMetrics, DataInfo, ModelMetadata, ModelStorage, ModelStorageConfig,
    PerformanceMetrics, PersistableModel, SemanticVersion, StorageMetrics, TrainingParams,
    VersionIncrement,
};

/// Metadata for adapter configuration and runtime information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdapterMetadata {
    pub name: String,
    pub version: String,
    pub adapter_type: String,
    pub capabilities: Vec<String>,
    pub connection_status: ConnectionStatus,
    pub last_connected: Option<i64>,
    pub error_count: u64,
    pub success_count: u64,
}

/// Connection status for adapters
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Connecting,
    Error(String),
}

/// Common trait for all data adapters
#[async_trait]
pub trait DataAdapter: Send + Sync {
    /// Connect to the data source
    async fn connect(&mut self) -> Result<(), AdapterError>;

    /// Disconnect from the data source
    async fn disconnect(&mut self) -> Result<(), AdapterError>;

    /// Check if the adapter is connected
    fn is_connected(&self) -> bool;

    /// Get adapter name
    fn name(&self) -> &str;

    /// Get adapter metadata
    fn metadata(&self) -> AdapterMetadata {
        AdapterMetadata {
            name: self.name().to_string(),
            version: "1.0.0".to_string(),
            adapter_type: "generic".to_string(),
            capabilities: vec![],
            connection_status: if self.is_connected() {
                ConnectionStatus::Connected
            } else {
                ConnectionStatus::Disconnected
            },
            last_connected: None,
            error_count: 0,
            success_count: 0,
        }
    }
}

/// Market data structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MarketData {
    pub symbol: String,
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

/// Order book entry
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrderBookEntry {
    pub price: f64,
    pub quantity: f64,
    pub timestamp: i64,
}

/// Order book snapshot
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    pub timestamp: i64,
    pub bids: Vec<OrderBookEntry>,
    pub asks: Vec<OrderBookEntry>,
}
