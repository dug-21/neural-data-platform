//! Test library aggregation
//! 
//! This file aggregates all test modules for easier organization

// Unit tests
pub mod unit {
    pub mod adapters_test;
    pub mod momentum_strategy_test;
    pub mod redis_adapter_test;
    pub mod strategies_test;
    pub mod timescale_adapter_test;
}

// Integration tests
pub mod integration {
    pub mod system_test;
}

// Common test utilities
pub mod common;