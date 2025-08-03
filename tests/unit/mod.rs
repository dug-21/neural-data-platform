//! Unit test modules for autonomous platform
//!
//! This module organizes all unit tests for the platform components

// Core adapter tests
pub mod adapter_core_test;
pub mod adapter_fann_test;
pub mod adapter_integration_test;

// Database adapter tests
pub mod adapter_postgres_test;
pub mod adapter_redis_test;

// Neural network tests
pub mod neural_ensemble_test;
pub mod enhanced_predictor_test;

// Neuro-divergent integration tests
pub mod neuro_divergent_adapter_test;
pub mod neuro_divergent_adapter_comprehensive_test;
pub mod fann_predictor_integration_test;
pub mod neuro_divergent_error_handling_test;

// Data pipeline tests
pub mod data_pipeline_test;
pub mod data_pipeline_routing_test;

// Service layer tests
pub mod service_data_test;
pub mod service_prediction_test;

// API tests
pub mod api_routes_test;

// Configuration tests
pub mod config_test;

// Model storage tests
pub mod model_storage_test;

// Training scheduler tests
pub mod training_scheduler_test;
pub mod market_hours_test;

// Module encapsulation tests
pub mod test_module_encapsulation;

// Phase 3B tests
pub mod phase3b_mock_tests;
pub mod event_subscription_tests;

// Memory optimization tests
pub mod memory_optimization_test;

// Phase 3 DAA Extensions Tests
pub mod daa;