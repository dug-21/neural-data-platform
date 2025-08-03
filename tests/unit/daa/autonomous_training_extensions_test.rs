use std::sync::Arc;
use tokio::sync::RwLock;
use neural_trader::{
    daa::{AutonomousTrainingEngine, DaaError, AdaptiveLearningIntegration},
    neural::{NeuralPredictor, PredictionError},
    config::StrategyConfig,
    data::TimeSeriesData,
};

#[tokio::test]
async fn test_extended_training_methods_preserve_thresholds() {
    // CRITICAL: Verify accuracy_threshold=0.8, error_threshold=0.1, consecutive_failure_threshold=5
    let mut engine = AutonomousTrainingEngine::new();
    
    // Verify core thresholds are preserved exactly
    assert_eq!(engine.config().accuracy_threshold, 0.8);
    assert_eq!(engine.config().error_threshold, 0.1);
    assert_eq!(engine.config().consecutive_failure_threshold, 5);
    
    // Test extended training methods
    let training_data = vec![
        TimeSeriesData::new(vec![1.0, 2.0, 3.0], chrono::Utc::now()),
        TimeSeriesData::new(vec![2.0, 3.0, 4.0], chrono::Utc::now()),
        TimeSeriesData::new(vec![3.0, 4.0, 5.0], chrono::Utc::now()),
    ];
    
    // Mock predictor that respects existing thresholds
    let predictor = Arc::new(RwLock::new(MockNeuralPredictor::new()));
    
    // Test extended training preserves existing autonomous behavior
    let result = engine.train_with_adaptive_learning(&training_data, predictor.clone()).await;
    assert!(result.is_ok());
    
    // Verify thresholds unchanged after extension
    assert_eq!(engine.config().accuracy_threshold, 0.8);
    assert_eq!(engine.config().error_threshold, 0.1);
    assert_eq!(engine.config().consecutive_failure_threshold, 5);
    
    // Test autonomous trading capabilities still function
    let trade_signal = engine.generate_autonomous_signal(&training_data[0]).await;
    assert!(trade_signal.is_ok());
}

#[tokio::test]
async fn test_adaptive_learning_integration() {
    let mut engine = AutonomousTrainingEngine::new();
    let predictor = Arc::new(RwLock::new(MockNeuralPredictor::new()));
    
    // Test adaptive learning integrates without breaking existing flow
    let adaptive_config = AdaptiveLearningIntegration {
        learning_rate_adjustment: 0.01,
        momentum_decay: 0.95,
        batch_size_optimization: true,
    };
    
    engine.configure_adaptive_learning(adaptive_config);
    
    // Verify existing autonomous decision-making preserved
    let training_data = vec![TimeSeriesData::new(vec![1.0, 2.0], chrono::Utc::now())];
    let result = engine.train_with_adaptive_learning(&training_data, predictor).await;
    
    assert!(result.is_ok());
    
    // Critical: Verify autonomous trading thresholds still enforced
    assert_eq!(engine.config().accuracy_threshold, 0.8);
    assert_eq!(engine.config().error_threshold, 0.1);
    assert_eq!(engine.consecutive_failures(), 0); // Should reset after successful training
}

#[tokio::test]
async fn test_checkpoint_rollback_functionality() {
    let mut engine = AutonomousTrainingEngine::new();
    let predictor = Arc::new(RwLock::new(MockNeuralPredictor::new()));
    
    // Create checkpoint before training
    let checkpoint = engine.create_checkpoint().await.unwrap();
    
    // Verify checkpoint preserves critical DAA parameters
    assert_eq!(checkpoint.accuracy_threshold, 0.8);
    assert_eq!(checkpoint.error_threshold, 0.1);
    assert_eq!(checkpoint.consecutive_failure_threshold, 5);
    
    // Simulate training that would modify state
    let bad_training_data = vec![TimeSeriesData::new(vec![f64::NAN, f64::INFINITY], chrono::Utc::now())];
    let _ = engine.train_with_adaptive_learning(&bad_training_data, predictor.clone()).await;
    
    // Test rollback functionality
    let rollback_result = engine.rollback_to_checkpoint(checkpoint).await;
    assert!(rollback_result.is_ok());
    
    // Critical: Verify all thresholds restored exactly
    assert_eq!(engine.config().accuracy_threshold, 0.8);
    assert_eq!(engine.config().error_threshold, 0.1);
    assert_eq!(engine.config().consecutive_failure_threshold, 5);
    
    // Verify autonomous capabilities fully restored
    let good_data = vec![TimeSeriesData::new(vec![1.0, 2.0], chrono::Utc::now())];
    let trade_signal = engine.generate_autonomous_signal(&good_data[0]).await;
    assert!(trade_signal.is_ok());
}

#[tokio::test]
async fn test_extended_methods_maintain_voting_consensus() {
    let mut engine = AutonomousTrainingEngine::new();
    
    // Test that extended training methods preserve 60/40 voting and 70% consensus
    let training_data = vec![
        TimeSeriesData::new(vec![1.0, 2.0, 3.0], chrono::Utc::now()),
        TimeSeriesData::new(vec![2.0, 3.0, 4.0], chrono::Utc::now()),
    ];
    
    let predictor = Arc::new(RwLock::new(MockNeuralPredictor::new()));
    let result = engine.train_with_adaptive_learning(&training_data, predictor).await;
    
    assert!(result.is_ok());
    
    // Verify voting consensus mechanism preserved
    assert_eq!(engine.get_voting_ratio(), (60, 40)); // 60/40 voting preserved
    assert_eq!(engine.get_consensus_threshold(), 0.7); // 70% consensus maintained
}

// Mock implementation for testing
struct MockNeuralPredictor {
    accuracy: f64,
}

impl MockNeuralPredictor {
    fn new() -> Self {
        Self { accuracy: 0.85 } // Above 0.8 threshold
    }
}

#[async_trait::async_trait]
impl NeuralPredictor for MockNeuralPredictor {
    async fn predict(&self, _data: &TimeSeriesData) -> Result<Vec<f64>, PredictionError> {
        Ok(vec![0.5, 0.3, 0.2])
    }
    
    async fn train(&mut self, _data: &[TimeSeriesData]) -> Result<(), PredictionError> {
        Ok(())
    }
    
    fn get_accuracy(&self) -> f64 {
        self.accuracy
    }
    
    async fn save_model(&self, _path: &str) -> Result<(), PredictionError> {
        Ok(())
    }
    
    async fn load_model(&mut self, _path: &str) -> Result<(), PredictionError> {
        Ok(())
    }
}