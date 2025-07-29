//! Compilation validation test for Neural Trader
//!
//! This test ensures that all components compile and basic instantiation works

use autonomous_platform::agents::AutonomousAgent;
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::neural::NeuralPredictor;

#[test]
fn test_component_instantiation() {
    // Test neural predictor can be created
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };

    let _predictor =
        NeuralPredictor::new(neural_config).expect("Failed to create neural predictor");

    // Test autonomous agent can be created
    let _agent = AutonomousAgent::default();

    println!("✅ Neural Trader compilation test passed");
    println!("✅ All components instantiate successfully");
}

#[test]
fn test_library_functionality() {
    // Test that the library compiles and basic structs work
    let config = autonomous_platform::config::NeuralConfig::default();
    assert!(!config.models.is_empty());

    println!("✅ Library functionality test passed");
}
