//! Test library aggregation
//!
//! This file aggregates all test modules for easier organization

// Adapter tests
pub mod adapters {
    pub mod test_mock_removal;
}

// Configuration tests
pub mod config {
    pub mod test_feature_flags;
}

// Unit tests
pub mod unit {
    pub mod adapters_test;
    pub mod momentum_strategy_test;
    pub mod redis_adapter_test;
    pub mod strategies_test;
    pub mod timescale_adapter_test;
    pub mod phase2_tdd_tests;
    pub mod performance_emitter_trait_tests;
}

// Integration tests
pub mod integration {
    pub mod system_test;
}

// Common test utilities
pub mod common;
