//! Autonomous Platform Library
//! 
//! This library provides the core functionality for the autonomous trading platform,
//! integrating data acquisition, machine learning models, and swarm intelligence.

pub mod config;
pub mod data;
pub mod integration;
pub mod adapters;

// Re-export commonly used types
pub use anyhow::Result;
pub use config::{PlatformConfig, load_default_config};