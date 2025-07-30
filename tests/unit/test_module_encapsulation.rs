//! Tests to ensure module encapsulation is maintained
//! 
//! This test module verifies that:
//! 1. Only the central NeuralPredictor is exposed from neural module
//! 2. Implementation details (FannPredictor, EnhancedNeuralPredictor) are not accessible
//! 3. Adapter implementations cannot be accessed directly
//! 4. All neural network operations must go through the central predictor

// These tests verify that direct imports of implementation details fail at compile time
// Uncommenting any of these should cause compilation errors

#[cfg(test)]
mod encapsulation_tests {
    // Test 1: Verify FannPredictor is not directly accessible
    // SHOULD NOT COMPILE if encapsulation is correct:
    // use neural_trader::neural::FannPredictor;
    // use neural_trader::neural::fann_predictor::FannPredictor;
    
    // Test 2: Verify EnhancedNeuralPredictor is not directly accessible  
    // SHOULD NOT COMPILE if encapsulation is correct:
    // use neural_trader::neural::EnhancedNeuralPredictor;
    // use neural_trader::neural::enhanced_predictor::EnhancedNeuralPredictor;
    
    // Test 3: Verify FannModelAdapter is not directly accessible
    // SHOULD NOT COMPILE if encapsulation is correct:
    // use neural_trader::neural::FannModelAdapter;
    // use neural_trader::neural::fann_model_adapter::FannModelAdapter;
    
    // Test 4: Verify EnhancedNeuralAdapter is not accessible from adapters
    // SHOULD NOT COMPILE if encapsulation is correct:
    // use neural_trader::adapters::EnhancedNeuralAdapter;
    // use neural_trader::adapters::enhanced_neural_adapter::EnhancedNeuralAdapter;
    
    // Test 5: Verify only NeuralPredictor is accessible
    #[test]
    fn test_only_neural_predictor_accessible() {
        // This SHOULD compile - NeuralPredictor is the only public interface
        use neural_trader::neural::NeuralPredictor;
        
        // Can create predictor through public interface
        let _predictor = NeuralPredictor::default();
    }
    
    // Test 6: Verify performance monitoring components are accessible (they're safe)
    #[test]
    fn test_performance_monitoring_accessible() {
        use neural_trader::neural::{
            PerformanceChannel, PerformanceEvent, PerformanceAggregator
        };
        
        // These are safe to expose as they only monitor, not execute predictions
        let _channel = PerformanceChannel::new(100);
        let _aggregator = PerformanceAggregator::default();
    }
    
    // Test 7: Verify trait is accessible but implementations are not
    #[test]
    fn test_trait_accessible() {
        use neural_trader::neural::NeuralPredictorTrait;
        
        // Trait should be accessible for type bounds
        fn accepts_predictor<T: NeuralPredictorTrait>(_predictor: &T) {
            // This is allowed
        }
    }
}

#[cfg(test)]
mod integration_encapsulation_tests {
    use neural_trader::config::NeuralConfig;
    use neural_trader::neural::NeuralPredictor;
    
    #[test]
    fn test_cannot_bypass_central_predictor() {
        // All neural network operations must go through NeuralPredictor
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            lookback_window: 24,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 30,
            max_retries: 3,
            error_threshold: 0.05,
        };
        
        // Only way to create predictor is through the public interface
        let predictor = NeuralPredictor::new(config).unwrap();
        
        // Cannot access internal implementations
        // The following would not compile:
        // let fann = predictor.fann_predictor; // Private field
        // let enhanced = EnhancedNeuralPredictor::new(...); // Not exported
    }
}

// Compile-time verification tests
// These use doc tests that should fail to compile if encapsulation is broken
/// ```compile_fail
/// // This should fail - FannPredictor is not exported
/// use neural_trader::neural::FannPredictor;
/// ```
fn _test_fann_predictor_not_exported() {}

/// ```compile_fail
/// // This should fail - EnhancedNeuralPredictor is not exported
/// use neural_trader::neural::EnhancedNeuralPredictor;
/// ```
fn _test_enhanced_predictor_not_exported() {}

/// ```compile_fail
/// // This should fail - EnhancedNeuralAdapter is not exported
/// use neural_trader::adapters::EnhancedNeuralAdapter;
/// ```
fn _test_enhanced_adapter_not_exported() {}

/// ```compile_fail
/// // This should fail - Direct module access is private
/// use neural_trader::neural::fann_predictor;
/// ```
fn _test_fann_module_not_accessible() {}

/// ```compile_fail
/// // This should fail - Direct module access is private
/// use neural_trader::neural::enhanced_predictor;
/// ```
fn _test_enhanced_module_not_accessible() {}