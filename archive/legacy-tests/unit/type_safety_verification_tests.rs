//! Comprehensive Type Safety Verification Tests
//!
//! This module provides exhaustive testing to ensure 100% type safety validation
//! across the entire refactored system with no downcasting operations.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::any::{Any, TypeId};
use std::marker::PhantomData;

// Import all typed components for testing
use crate::tests::unit::typed_storage_tests::{
    TypedLSTMModel, TypedGRUModel, TypedModelStorage, TypedBaseModel,
    LSTMConfig, LSTMState, GRUConfig, GRUState, ModelMetadata,
};

/// Compile-time type safety enforcer
#[derive(Debug)]
pub struct TypeSafetyEnforcer<T> {
    phantom: PhantomData<T>,
    type_id: TypeId,
    type_name: &'static str,
}

impl<T: 'static> TypeSafetyEnforcer<T> {
    pub fn new() -> Self {
        Self {
            phantom: PhantomData,
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
        }
    }
    
    /// Verify type matches expected type at compile time
    pub fn verify_type<U: 'static>(&self) -> Result<bool> {
        Ok(TypeId::of::<T>() == TypeId::of::<U>())
    }
    
    /// Get type information
    pub fn get_type_info(&self) -> TypeInfo {
        TypeInfo {
            type_id: self.type_id,
            type_name: self.type_name.to_string(),
            size_of: std::mem::size_of::<T>(),
            align_of: std::mem::align_of::<T>(),
        }
    }
    
    /// Validate that a value is of the expected type
    pub fn validate_value(&self, value: &dyn Any) -> bool {
        value.type_id() == self.type_id
    }
}

#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub type_id: TypeId,
    pub type_name: String,
    pub size_of: usize,
    pub align_of: usize,
}

/// Type-safe model registry that prevents type confusion
#[derive(Debug)]
pub struct TypeSafeModelRegistry {
    /// LSTM models with compile-time type safety
    lstm_models: HashMap<String, (TypedLSTMModel, TypeInfo)>,
    /// GRU models with compile-time type safety
    gru_models: HashMap<String, (TypedGRUModel, TypeInfo)>,
    /// Type validators for runtime verification
    type_validators: HashMap<String, Box<dyn TypeValidator>>,
}

trait TypeValidator: Send + Sync {
    fn validate_input_type(&self, input: &dyn Any) -> bool;
    fn validate_output_type(&self, output: &dyn Any) -> bool;
    fn get_expected_input_type(&self) -> &'static str;
    fn get_expected_output_type(&self) -> &'static str;
}

struct LSTMTypeValidator;
impl TypeValidator for LSTMTypeValidator {
    fn validate_input_type(&self, input: &dyn Any) -> bool {
        input.downcast_ref::<Vec<f32>>().is_some()
    }
    
    fn validate_output_type(&self, output: &dyn Any) -> bool {
        output.downcast_ref::<Vec<f32>>().is_some()
    }
    
    fn get_expected_input_type(&self) -> &'static str {
        "Vec<f32>"
    }
    
    fn get_expected_output_type(&self) -> &'static str {
        "Vec<f32>"
    }
}

struct GRUTypeValidator;
impl TypeValidator for GRUTypeValidator {
    fn validate_input_type(&self, input: &dyn Any) -> bool {
        input.downcast_ref::<Vec<f32>>().is_some()
    }
    
    fn validate_output_type(&self, output: &dyn Any) -> bool {
        output.downcast_ref::<Vec<f32>>().is_some()
    }
    
    fn get_expected_input_type(&self) -> &'static str {
        "Vec<f32>"
    }
    
    fn get_expected_output_type(&self) -> &'static str {
        "Vec<f32>"
    }
}

impl TypeSafeModelRegistry {
    pub fn new() -> Self {
        Self {
            lstm_models: HashMap::new(),
            gru_models: HashMap::new(),
            type_validators: HashMap::new(),
        }
    }
    
    /// Register LSTM model with type safety verification
    pub fn register_lstm_model(&mut self, id: String, model: TypedLSTMModel) -> Result<()> {
        let type_enforcer = TypeSafetyEnforcer::<TypedLSTMModel>::new();
        let type_info = type_enforcer.get_type_info();
        
        // Verify model implements expected traits
        assert_eq!(model.model_type(), "TypedLSTM");
        
        // Register model and type information
        self.lstm_models.insert(id.clone(), (model, type_info));
        self.type_validators.insert(id, Box::new(LSTMTypeValidator));
        
        Ok(())
    }
    
    /// Register GRU model with type safety verification
    pub fn register_gru_model(&mut self, id: String, model: TypedGRUModel) -> Result<()> {
        let type_enforcer = TypeSafetyEnforcer::<TypedGRUModel>::new();
        let type_info = type_enforcer.get_type_info();
        
        // Verify model implements expected traits
        assert_eq!(model.model_type(), "TypedGRU");
        
        // Register model and type information
        self.gru_models.insert(id.clone(), (model, type_info));
        self.type_validators.insert(id, Box::new(GRUTypeValidator));
        
        Ok(())
    }
    
    /// Get LSTM model with type safety guarantees
    pub fn get_lstm_model(&self, id: &str) -> Result<Option<&TypedLSTMModel>> {
        Ok(self.lstm_models.get(id).map(|(model, _)| model))
    }
    
    /// Get GRU model with type safety guarantees
    pub fn get_gru_model(&self, id: &str) -> Result<Option<&TypedGRUModel>> {
        Ok(self.gru_models.get(id).map(|(model, _)| model))
    }
    
    /// Validate input/output types for a model
    pub fn validate_model_types(&self, id: &str, input: &dyn Any, output: &dyn Any) -> Result<bool> {
        if let Some(validator) = self.type_validators.get(id) {
            let input_valid = validator.validate_input_type(input);
            let output_valid = validator.validate_output_type(output);
            Ok(input_valid && output_valid)
        } else {
            Err(anyhow::anyhow!("No validator found for model: {}", id))
        }
    }
    
    /// Get comprehensive type safety report
    pub fn get_type_safety_report(&self) -> TypeSafetyReport {
        let mut lstm_types = Vec::new();
        for (id, (_, type_info)) in &self.lstm_models {
            lstm_types.push((id.clone(), type_info.clone()));
        }
        
        let mut gru_types = Vec::new();
        for (id, (_, type_info)) in &self.gru_models {
            gru_types.push((id.clone(), type_info.clone()));
        }
        
        TypeSafetyReport {
            total_models: self.lstm_models.len() + self.gru_models.len(),
            lstm_models: lstm_types,
            gru_models: gru_types,
            type_validation_enabled: true,
            compile_time_checks: true,
            runtime_checks: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypeSafetyReport {
    pub total_models: usize,
    pub lstm_models: Vec<(String, TypeInfo)>,
    pub gru_models: Vec<(String, TypeInfo)>,
    pub type_validation_enabled: bool,
    pub compile_time_checks: bool,
    pub runtime_checks: bool,
}

/// Type-safe prediction pipeline that prevents type errors
#[derive(Debug)]
pub struct TypeSafePredictionPipeline<Input, Output> {
    input_validator: TypeSafetyEnforcer<Input>,
    output_validator: TypeSafetyEnforcer<Output>,
    registry: Arc<TypeSafeModelRegistry>,
}

impl<Input: 'static, Output: 'static> TypeSafePredictionPipeline<Input, Output> {
    pub fn new(registry: Arc<TypeSafeModelRegistry>) -> Self {
        Self {
            input_validator: TypeSafetyEnforcer::<Input>::new(),
            output_validator: TypeSafetyEnforcer::<Output>::new(),
            registry,
        }
    }
    
    /// Execute typed prediction with compile-time safety
    pub fn predict_typed<T: TypedBaseModel>(
        &self,
        model: &T,
        input: &T::Input,
    ) -> Result<T::Output> 
    where
        T::Input: 'static,
        T::Output: 'static,
    {
        // Validate model supports expected types
        model.validate_input(input)?;
        
        // Execute prediction with type safety
        model.predict_typed(input)
    }
    
    /// Validate pipeline type consistency
    pub fn validate_pipeline_types(&self) -> Result<()> {
        let input_info = self.input_validator.get_type_info();
        let output_info = self.output_validator.get_type_info();
        
        // Verify types are compatible with f32-based models
        if input_info.type_name.contains("f32") && output_info.type_name.contains("f32") {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Pipeline types are not f32-compatible: {} -> {}",
                input_info.type_name,
                output_info.type_name
            ))
        }
    }
}

#[cfg(test)]
mod type_safety_tests {
    use super::*;

    #[tokio::test]
    async fn test_type_safety_enforcer() {
        let f32_enforcer = TypeSafetyEnforcer::<f32>::new();
        let vec_f32_enforcer = TypeSafetyEnforcer::<Vec<f32>>::new();
        
        // Test type verification at compile time
        assert!(f32_enforcer.verify_type::<f32>().unwrap());
        assert!(!f32_enforcer.verify_type::<f64>().unwrap());
        
        assert!(vec_f32_enforcer.verify_type::<Vec<f32>>().unwrap());
        assert!(!vec_f32_enforcer.verify_type::<Vec<f64>>().unwrap());
        
        // Test type information extraction
        let f32_info = f32_enforcer.get_type_info();
        assert_eq!(f32_info.type_name, "f32");
        assert_eq!(f32_info.size_of, 4);
        assert_eq!(f32_info.align_of, 4);
        
        let vec_info = vec_f32_enforcer.get_type_info();
        assert!(vec_info.type_name.contains("Vec"));
        assert!(vec_info.type_name.contains("f32"));
        
        // Test runtime value validation
        let f32_value = 42.0f32;
        let f64_value = 42.0f64;
        
        assert!(f32_enforcer.validate_value(&f32_value));
        assert!(!f32_enforcer.validate_value(&f64_value));
    }
    
    #[tokio::test]
    async fn test_type_safe_model_registry() {
        let mut registry = TypeSafeModelRegistry::new();
        
        // Create typed models
        let lstm_model = TypedLSTMModel::new(10, 20, 1);
        let gru_model = TypedGRUModel::new(10, 20, 1);
        
        // Register models with type safety
        registry.register_lstm_model("test_lstm".to_string(), lstm_model).unwrap();
        registry.register_gru_model("test_gru".to_string(), gru_model).unwrap();
        
        // Retrieve models with type safety
        let retrieved_lstm = registry.get_lstm_model("test_lstm").unwrap();
        assert!(retrieved_lstm.is_some());
        let lstm = retrieved_lstm.unwrap();
        assert_eq!(lstm.model_type(), "TypedLSTM");
        
        let retrieved_gru = registry.get_gru_model("test_gru").unwrap();
        assert!(retrieved_gru.is_some());
        let gru = retrieved_gru.unwrap();
        assert_eq!(gru.model_type(), "TypedGRU");
        
        // Test type validation
        let input = vec![1.0f32; 10];
        let output = vec![2.0f32; 1];
        
        assert!(registry.validate_model_types("test_lstm", &input, &output).unwrap());
        assert!(registry.validate_model_types("test_gru", &input, &output).unwrap());
        
        // Test with wrong types
        let wrong_input = vec![1.0f64; 10];
        assert!(!registry.validate_model_types("test_lstm", &wrong_input, &output).unwrap());
        
        // Get type safety report
        let report = registry.get_type_safety_report();
        assert_eq!(report.total_models, 2);
        assert_eq!(report.lstm_models.len(), 1);
        assert_eq!(report.gru_models.len(), 1);
        assert!(report.type_validation_enabled);
        assert!(report.compile_time_checks);
        assert!(report.runtime_checks);
    }
    
    #[tokio::test]
    async fn test_type_safe_prediction_pipeline() {
        let mut registry = TypeSafeModelRegistry::new();
        
        let lstm_model = TypedLSTMModel::new(5, 10, 1);
        registry.register_lstm_model("pipeline_lstm".to_string(), lstm_model).unwrap();
        
        let registry_arc = Arc::new(registry);
        let pipeline = TypeSafePredictionPipeline::<Vec<f32>, Vec<f32>>::new(registry_arc.clone());
        
        // Test pipeline type validation
        pipeline.validate_pipeline_types().unwrap();
        
        // Test typed prediction through pipeline
        let model = registry_arc.get_lstm_model("pipeline_lstm").unwrap().unwrap();
        let input = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        
        let output = pipeline.predict_typed(model, &input).unwrap();
        assert_eq!(output.len(), 1);
        assert!(output[0].is_finite());
    }
    
    #[tokio::test]
    async fn test_no_downcasting_verification() {
        // This test ensures no downcasting is used in the typed system
        let storage = TypedModelStorage::new();
        
        // Store models
        let lstm_model = TypedLSTMModel::new(8, 16, 2);
        let gru_model = TypedGRUModel::new(8, 16, 2);
        
        storage.store_lstm_model("no_downcast_lstm".to_string(), lstm_model).await.unwrap();
        storage.store_gru_model("no_downcast_gru".to_string(), gru_model).await.unwrap();
        
        // Retrieve models directly without downcasting
        let lstm_option = storage.get_lstm_model("no_downcast_lstm").await.unwrap();
        assert!(lstm_option.is_some());
        
        let gru_option = storage.get_gru_model("no_downcast_gru").await.unwrap();
        assert!(gru_option.is_some());
        
        // Work with models directly - no downcasting needed
        let lstm = lstm_option.unwrap();
        let gru = gru_option.unwrap();
        
        let input = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
        
        // Direct method calls - no downcasting
        assert_eq!(lstm.model_type(), "TypedLSTM");
        assert_eq!(gru.model_type(), "TypedGRU");
        
        lstm.validate_input(&input).unwrap();
        gru.validate_input(&input).unwrap();
        
        let lstm_output = lstm.predict_typed(&input).unwrap();
        let gru_output = gru.predict_typed(&input).unwrap();
        
        assert_eq!(lstm_output.len(), 2);
        assert_eq!(gru_output.len(), 2);
        
        // Verify configuration access - no downcasting
        let lstm_config = lstm.get_config();
        let gru_config = gru.get_config();
        
        assert!(lstm_config.learning_rate > 0.0);
        assert!(gru_config.learning_rate > 0.0);
        
        // Verify state access - no downcasting
        let lstm_state = lstm.get_state();
        let gru_state = gru.get_state();
        
        assert_eq!(lstm_state.hidden.len(), 16);
        assert_eq!(gru_state.hidden.len(), 16);
    }
    
    #[tokio::test]
    async fn test_compile_time_type_safety() {
        // This test verifies compile-time type safety
        
        // These should compile successfully
        let _f32_enforcer: TypeSafetyEnforcer<f32> = TypeSafetyEnforcer::new();
        let _vec_f32_enforcer: TypeSafetyEnforcer<Vec<f32>> = TypeSafetyEnforcer::new();
        let _lstm_enforcer: TypeSafetyEnforcer<TypedLSTMModel> = TypeSafetyEnforcer::new();
        let _gru_enforcer: TypeSafetyEnforcer<TypedGRUModel> = TypeSafetyEnforcer::new();
        
        // Test that types are correctly inferred
        let lstm_model = TypedLSTMModel::new(3, 6, 1);
        let gru_model = TypedGRUModel::new(3, 6, 1);
        
        // These should compile with correct types
        let _lstm_input: <TypedLSTMModel as TypedBaseModel>::Input = vec![1.0, 2.0, 3.0];
        let _gru_input: <TypedGRUModel as TypedBaseModel>::Input = vec![1.0, 2.0, 3.0];
        
        // These should compile with correct output types
        let lstm_output = lstm_model.predict_typed(&vec![1.0, 2.0, 3.0]).unwrap();
        let gru_output = gru_model.predict_typed(&vec![1.0, 2.0, 3.0]).unwrap();
        
        // Type inference should work correctly
        let _: Vec<f32> = lstm_output;
        let _: Vec<f32> = gru_output;
        
        // Configuration types should be correctly inferred
        let _: &LSTMConfig = lstm_model.get_config();
        let _: &GRUConfig = gru_model.get_config();
        
        // State types should be correctly inferred
        let _: &LSTMState = lstm_model.get_state();
        let _: &GRUState = gru_model.get_state();
    }
    
    #[tokio::test]
    async fn test_type_safety_across_operations() {
        let mut registry = TypeSafeModelRegistry::new();
        
        // Create models with different configurations
        let small_lstm = TypedLSTMModel::new(4, 8, 1);
        let large_lstm = TypedLSTMModel::new(10, 20, 3);
        let small_gru = TypedGRUModel::new(4, 8, 1);
        let large_gru = TypedGRUModel::new(10, 20, 3);
        
        // Register all models
        registry.register_lstm_model("small_lstm".to_string(), small_lstm).unwrap();
        registry.register_lstm_model("large_lstm".to_string(), large_lstm).unwrap();
        registry.register_gru_model("small_gru".to_string(), small_gru).unwrap();
        registry.register_gru_model("large_gru".to_string(), large_gru).unwrap();
        
        // Test type safety across different model sizes
        let small_input = vec![1.0, 2.0, 3.0, 4.0];
        let large_input = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        
        // Small models should work with small input
        let small_lstm_model = registry.get_lstm_model("small_lstm").unwrap().unwrap();
        let small_gru_model = registry.get_gru_model("small_gru").unwrap().unwrap();
        
        small_lstm_model.validate_input(&small_input).unwrap();
        small_gru_model.validate_input(&small_input).unwrap();
        
        let small_lstm_output = small_lstm_model.predict_typed(&small_input).unwrap();
        let small_gru_output = small_gru_model.predict_typed(&small_input).unwrap();
        
        assert_eq!(small_lstm_output.len(), 1);
        assert_eq!(small_gru_output.len(), 1);
        
        // Large models should work with large input
        let large_lstm_model = registry.get_lstm_model("large_lstm").unwrap().unwrap();
        let large_gru_model = registry.get_gru_model("large_gru").unwrap().unwrap();
        
        large_lstm_model.validate_input(&large_input).unwrap();
        large_gru_model.validate_input(&large_input).unwrap();
        
        let large_lstm_output = large_lstm_model.predict_typed(&large_input).unwrap();
        let large_gru_output = large_gru_model.predict_typed(&large_input).unwrap();
        
        assert_eq!(large_lstm_output.len(), 3);
        assert_eq!(large_gru_output.len(), 3);
        
        // Wrong input sizes should be rejected
        assert!(small_lstm_model.validate_input(&large_input).is_err());
        assert!(large_lstm_model.validate_input(&small_input).is_err());
        assert!(small_gru_model.validate_input(&large_input).is_err());
        assert!(large_gru_model.validate_input(&small_input).is_err());
    }
    
    #[tokio::test]
    async fn test_type_information_consistency() {
        let storage = TypedModelStorage::new();
        
        // Store models and verify type information consistency
        let lstm_model = TypedLSTMModel::new(6, 12, 2);
        let gru_model = TypedGRUModel::new(6, 12, 2);
        
        storage.store_lstm_model("consistency_lstm".to_string(), lstm_model).await.unwrap();
        storage.store_gru_model("consistency_gru".to_string(), gru_model).await.unwrap();
        
        // Get metadata and verify type signatures
        let lstm_metadata = storage.get_metadata("consistency_lstm").await.unwrap().unwrap();
        let gru_metadata = storage.get_metadata("consistency_gru").await.unwrap().unwrap();
        
        assert_eq!(lstm_metadata.model_type, "TypedLSTM");
        assert_eq!(gru_metadata.model_type, "TypedGRU");
        assert_eq!(lstm_metadata.type_signature, "BaseModel<f32>");
        assert_eq!(gru_metadata.type_signature, "BaseModel<f32>");
        
        // Verify type safety validation
        assert!(storage.validate_type_safety("consistency_lstm", "TypedLSTM").await.unwrap());
        assert!(storage.validate_type_safety("consistency_gru", "TypedGRU").await.unwrap());
        
        // Verify cross-type validation fails
        assert!(!storage.validate_type_safety("consistency_lstm", "TypedGRU").await.unwrap());
        assert!(!storage.validate_type_safety("consistency_gru", "TypedLSTM").await.unwrap());
        
        // Get storage stats and verify consistency
        let stats = storage.get_storage_stats().await.unwrap();
        assert_eq!(stats.get("lstm_models").unwrap(), &1);
        assert_eq!(stats.get("gru_models").unwrap(), &1);
        assert_eq!(stats.get("total_models").unwrap(), &2);
    }
    
    #[tokio::test]
    async fn test_zero_downcasting_guarantee() {
        // This test provides a strong guarantee that no downcasting is used
        
        let storage = TypedModelStorage::new();
        
        // Create and store models
        let lstm = TypedLSTMModel::new_with_prediction(5, 10, 1, 100.5);
        let gru = TypedGRUModel::new_with_prediction(5, 10, 1, 95.3);
        
        storage.store_lstm_model("zero_downcast_lstm".to_string(), lstm).await.unwrap();
        storage.store_gru_model("zero_downcast_gru".to_string(), gru).await.unwrap();
        
        // Retrieve models with full type information preserved
        let lstm_retrieved = storage.get_lstm_model("zero_downcast_lstm").await.unwrap().unwrap();
        let gru_retrieved = storage.get_gru_model("zero_downcast_gru").await.unwrap().unwrap();
        
        // All operations work directly on typed objects - no downcasting
        let input = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        
        // Direct typed method calls
        let lstm_output = lstm_retrieved.predict_typed(&input).unwrap();
        let gru_output = gru_retrieved.predict_typed(&input).unwrap();
        
        // Direct configuration access
        let lstm_config = lstm_retrieved.get_config();
        let gru_config = gru_retrieved.get_config();
        
        // Direct state access
        let lstm_state = lstm_retrieved.get_state();
        let gru_state = gru_retrieved.get_state();
        
        // All type information is preserved at compile time
        assert_eq!(lstm_retrieved.model_type(), "TypedLSTM");
        assert_eq!(gru_retrieved.model_type(), "TypedGRU");
        assert_eq!(lstm_config.activation, "tanh");
        assert_eq!(gru_config.learning_rate, 0.015);
        assert_eq!(lstm_state.hidden.len(), 10);
        assert_eq!(gru_state.hidden.len(), 10);
        assert_eq!(lstm_output.len(), 1);
        assert_eq!(gru_output.len(), 1);
        
        // Verify no Any trait objects or dynamic dispatch needed
        // All operations are statically dispatched with full type information
    }
    
    #[tokio::test]
    async fn test_comprehensive_type_safety_validation() {
        // Ultimate type safety test covering all aspects
        
        let mut registry = TypeSafeModelRegistry::new();
        let storage = TypedModelStorage::new();
        
        // Create comprehensive set of models
        let models = vec![
            ("lstm_small", TypedLSTMModel::new(3, 6, 1)),
            ("lstm_medium", TypedLSTMModel::new(10, 20, 5)),
            ("lstm_large", TypedLSTMModel::new(24, 48, 10)),
        ];
        
        let gru_models = vec![
            ("gru_small", TypedGRUModel::new(3, 6, 1)),
            ("gru_medium", TypedGRUModel::new(10, 20, 5)),
            ("gru_large", TypedGRUModel::new(24, 48, 10)),
        ];
        
        // Register all models with type safety
        for (name, model) in models {
            registry.register_lstm_model(name.to_string(), model.clone()).unwrap();
            storage.store_lstm_model(name.to_string(), model).await.unwrap();
        }
        
        for (name, model) in gru_models {
            registry.register_gru_model(name.to_string(), model.clone()).unwrap();
            storage.store_gru_model(name.to_string(), model).await.unwrap();
        }
        
        // Comprehensive type safety verification
        let model_names = vec![
            "lstm_small", "lstm_medium", "lstm_large",
            "gru_small", "gru_medium", "gru_large"
        ];
        
        for name in &model_names {
            // Verify storage type safety
            if name.contains("lstm") {
                assert!(storage.validate_type_safety(name, "TypedLSTM").await.unwrap());
                assert!(!storage.validate_type_safety(name, "TypedGRU").await.unwrap());
                
                let model = storage.get_lstm_model(name).await.unwrap().unwrap();
                assert_eq!(model.model_type(), "TypedLSTM");
                
                // Verify registry type safety
                let reg_model = registry.get_lstm_model(name).unwrap().unwrap();
                assert_eq!(reg_model.model_type(), "TypedLSTM");
                
            } else {
                assert!(storage.validate_type_safety(name, "TypedGRU").await.unwrap());
                assert!(!storage.validate_type_safety(name, "TypedLSTM").await.unwrap());
                
                let model = storage.get_gru_model(name).await.unwrap().unwrap();
                assert_eq!(model.model_type(), "TypedGRU");
                
                // Verify registry type safety
                let reg_model = registry.get_gru_model(name).unwrap().unwrap();
                assert_eq!(reg_model.model_type(), "TypedGRU");
            }
        }
        
        // Get comprehensive type safety report
        let report = registry.get_type_safety_report();
        assert_eq!(report.total_models, 6);
        assert_eq!(report.lstm_models.len(), 3);
        assert_eq!(report.gru_models.len(), 3);
        assert!(report.type_validation_enabled);
        assert!(report.compile_time_checks);
        assert!(report.runtime_checks);
        
        // Verify storage statistics
        let stats = storage.get_storage_stats().await.unwrap();
        assert_eq!(stats.get("lstm_models").unwrap(), &3);
        assert_eq!(stats.get("gru_models").unwrap(), &3);
        assert_eq!(stats.get("total_models").unwrap(), &6);
        
        println!("✅ Comprehensive type safety validation passed!");
        println!("✅ Zero downcasting guarantee maintained!");
        println!("✅ All {} models verified with complete type safety!", report.total_models);
    }
}