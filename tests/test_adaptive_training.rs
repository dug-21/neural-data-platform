//! Test adaptive learning rate and early stopping features in FANN model adapter

use autonomous_platform::neural::fann_model_adapter::{FannModelAdapter, FannModelConfig};
use autonomous_platform::adapters::model_storage::ModelStorageConfig;
use autonomous_platform::adapters::vendor_bridge::TrainingConfig;
use ruv_fann::TrainingData;
use tempfile::TempDir;
use tokio;

#[tokio::test]
async fn test_adaptive_learning_rate() {
    let temp_dir = TempDir::new().unwrap();
    let storage_config = ModelStorageConfig {
        base_path: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let config = FannModelConfig {
        model_name: "test_adaptive".to_string(),
        input_size: 3,
        hidden_layers: vec![5],
        output_size: 1,
        adaptive_learning_rate: true,
        initial_lr_multiplier: 0.1, // Start with 10% of configured LR
        lr_increase_factor: 1.5,
        lr_decrease_factor: 0.8,
        plateau_patience: 5,        // Very short for testing
        early_stopping_patience: 20, // Short for testing
        min_improvement_threshold: 0.01,
        max_epochs: 50,
        target_error: 0.01,
        ..Default::default()
    };

    let mut adapter = FannModelAdapter::new(config, storage_config).await.unwrap();
    
    // Create some test training data with a learnable pattern
    let mut training_data = TrainingData::new();
    for i in 0..20 {
        let x = i as f32 * 0.1;
        let input = vec![x, x * x, x * x * x]; // polynomial features
        let output = vec![2.0 * x + 0.5 * x * x]; // target function
        training_data.add_sample(&input, &output).unwrap();
    }

    let train_config = TrainingConfig {
        max_epochs: 50,
        learning_rate: 0.1, // This will be adjusted by adaptive mechanism
        batch_size: 1,
        validation_size: 0.0,
        early_stopping_patience: 20,
        save_best_model: false,
        verbose: false,
        use_gpu: false,
        gradient_clipping: None,
        weight_decay: None,
        scheduler_config: None,
    };

    // Train the model and check that it completes without panicking
    let record = adapter.train_with_real_backprop(&training_data, &train_config).await;
    
    assert!(record.is_ok(), "Training should complete successfully");
    let record = record.unwrap();
    
    println!("✅ Adaptive learning rate test completed:");
    println!("   Epochs: {}", record.epochs_completed);
    println!("   Final MSE: {:.6}", record.final_mse);
    println!("   Training time: {}s", record.training_time_secs);
    
    // Check that training made progress
    assert!(record.final_mse < 1.0, "Training should reduce error significantly");
    
    // Verify model is marked as trained
    assert!(adapter.is_trained(), "Model should be marked as trained");
}

#[tokio::test]
async fn test_early_stopping() {
    let temp_dir = TempDir::new().unwrap();
    let storage_config = ModelStorageConfig {
        base_path: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let config = FannModelConfig {
        model_name: "test_early_stopping".to_string(),
        input_size: 2,
        hidden_layers: vec![3],
        output_size: 1,
        adaptive_learning_rate: false, // Disable adaptive LR for this test
        early_stopping_patience: 10,   // Very short for testing
        max_epochs: 1000,              // High max to ensure early stopping kicks in
        target_error: 0.001,           // Very low target to test early stopping
        ..Default::default()
    };

    let mut adapter = FannModelAdapter::new(config, storage_config).await.unwrap();
    
    // Create training data that's harder to learn perfectly (to trigger early stopping)
    let mut training_data = TrainingData::new();
    for i in 0..10 {
        let x1 = (i as f32 * 0.3).sin();
        let x2 = (i as f32 * 0.2).cos();
        let input = vec![x1, x2];
        let output = vec![x1 * x2 + 0.1 * (x1 + x2).sin()]; // Complex function
        training_data.add_sample(&input, &output).unwrap();
    }

    let train_config = TrainingConfig {
        max_epochs: 1000,  // High number
        learning_rate: 0.01, // Fixed learning rate
        batch_size: 1,
        validation_size: 0.0,
        early_stopping_patience: 10,
        save_best_model: false,
        verbose: false,
        use_gpu: false,
        gradient_clipping: None,
        weight_decay: None,
        scheduler_config: None,
    };

    let record = adapter.train_with_real_backprop(&training_data, &train_config).await.unwrap();
    
    println!("✅ Early stopping test completed:");
    println!("   Epochs: {} (should be much less than max 1000)", record.epochs_completed);
    println!("   Final MSE: {:.6}", record.final_mse);
    
    // Early stopping should trigger before max epochs
    assert!(record.epochs_completed < 1000, "Early stopping should prevent reaching max epochs");
    
    // Should still be within reasonable bounds
    assert!(record.epochs_completed >= 10, "Should train for at least the patience period");
}

#[tokio::test]
async fn test_disabled_adaptive_features() {
    let temp_dir = TempDir::new().unwrap();
    let storage_config = ModelStorageConfig {
        base_path: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let config = FannModelConfig {
        model_name: "test_disabled_adaptive".to_string(),
        input_size: 2,
        hidden_layers: vec![3],
        output_size: 1,
        adaptive_learning_rate: false, // Disabled
        early_stopping_patience: 0,    // Disabled
        max_epochs: 20,
        target_error: 0.001,
        ..Default::default()
    };

    let mut adapter = FannModelAdapter::new(config, storage_config).await.unwrap();
    
    // Simple training data
    let mut training_data = TrainingData::new();
    for i in 0..5 {
        let x = i as f32 * 0.2;
        let input = vec![x, x * x];
        let output = vec![x + 0.1];
        training_data.add_sample(&input, &output).unwrap();
    }

    let train_config = TrainingConfig {
        max_epochs: 20,
        learning_rate: 0.05,
        batch_size: 1,
        validation_size: 0.0,
        early_stopping_patience: 5,
        save_best_model: false,
        verbose: false,
        use_gpu: false,
        gradient_clipping: None,
        weight_decay: None,
        scheduler_config: None,
    };

    let record = adapter.train_with_real_backprop(&training_data, &train_config).await.unwrap();
    
    println!("✅ Disabled adaptive features test completed:");
    println!("   Epochs: {} (should be close to max 20)", record.epochs_completed);
    println!("   Final MSE: {:.6}", record.final_mse);
    
    // Without early stopping, should run close to max epochs (unless target reached)
    assert!(record.epochs_completed >= 10, "Should train for a reasonable number of epochs");
}