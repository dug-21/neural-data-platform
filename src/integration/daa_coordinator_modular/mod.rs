//! DAA Coordinator Module
//!
//! Modular structure for the Decentralized Autonomous Agent coordinator
//! Split into logical components for better maintainability.

pub mod config;
pub mod core;
pub mod decisions;
pub mod strategies;
pub mod agents;

// Re-export main types
pub use config::DaaConfig;
pub use core::DaaCoordinator;
pub use decisions::{AutonomousDecision, TradingAction, RiskAssessment};