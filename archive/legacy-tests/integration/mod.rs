//! Integration tests for neural-trader components

pub mod data_conversion_integration_test;
pub mod model_persistence_test;
pub mod market_aware_training_test;
pub mod simplified_routing_tests;
pub mod error_handling_tests;
pub mod test_performance_channel_subscription;
pub mod phase3b_integration_tests;
pub mod simple_field_validation_tests;

// Phase 3 Integration Tests
pub mod daa_flow_with_extensions_test;
pub mod cross_component_extensions_test;
pub mod phase3_workflow_test;