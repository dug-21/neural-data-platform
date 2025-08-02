//! Neural Trader Autonomous Platform Library
//!
//! This library provides the core functionality for an autonomous trading platform that
//! integrates real-time data acquisition, machine learning models, and swarm intelligence
//! for intelligent trading decisions.
//!
//! # Quick Start
//!
//! ```rust
//! use autonomous_platform::{PlatformConfig, load_default_config, Result};
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Load platform configuration
//!     let config = load_default_config()?;
//!     println!("Loaded config for: {}", config.platform.name);
//!     
//!     // Initialize data pipeline
//!     // let pipeline = DataPipeline::new(&config).await?;
//!     
//!     // Start real-time processing
//!     // pipeline.start().await?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! # Features
//!
//! - **Real-time Data Processing**: High-performance streaming data pipeline
//! - **Neural Network Integration**: Support for multiple ML frameworks (PyTorch, TensorFlow, ONNX)
//! - **Swarm Intelligence**: Coordinated multi-agent decision making
//! - **Robust Configuration**: Environment-based configuration with validation
//! - **Scalable Storage**: TimescaleDB for historical data, Redis for caching
//! - **Comprehensive Monitoring**: Built-in metrics and health checks
//!
//! # Architecture
//!
//! The platform is organized into several key modules:
//!
//! - [`config`] - Configuration management and validation
//! - [`data`] - Time series data processing, storage, and caching
//! - [`integration`] - External service integrations (market data, trading platforms)
//! - [`adapters`] - Data source adapters (TimescaleDB, Redis)
//! - [`strategies`] - Trading strategies and signal generation
//! - [`orchestration`] - Platform-wide coordination and lifecycle management
//!
//! # Configuration
//!
//! The platform uses TOML configuration files with environment variable overrides:
//!
//! ```toml
//! [platform]
//! name = "neural-trader-autonomous"
//! version = "0.1.0"
//!
//! [database]
//! url = "postgres://user:pass@localhost/neural_trader_db"
//! max_connections = 20
//!
//! [neural]
//! memory_gb = 2.0
//! models = ["NHITS", "DeepAR", "TCN", "MLP"]
//! ```
//!
//! # Examples
//!
//! See the `examples/` directory for complete working examples:
//!
//! - `basic_usage.rs` - Platform initialization and basic operations
//! - `trading_scenario.rs` - End-to-end trading workflow
//! - `performance_monitoring.rs` - Metrics collection and monitoring

pub mod adapters;
pub mod agents;
pub mod config; // Modular configuration system
pub mod daa;
pub mod data;
pub mod features; // Advanced feature engineering for neural trading
pub mod integration;
pub mod mcp;
pub mod monitoring;
pub mod neural;
pub mod observability;
pub mod performance;
pub mod orchestration;
pub mod security;
pub mod strategies;
pub mod streaming;
pub mod utils;

// Re-export commonly used types
pub use anyhow::Result;
pub use config::{load_default_config, PlatformConfig};
pub use monitoring::{
    Alert, AlertConfig, AlertSeverity, ComponentHealth, ComponentType, HealthMonitor, HealthStatus,
    PerformanceMetrics, SystemHealth,
};
pub use orchestration::PlatformOrchestrator;
