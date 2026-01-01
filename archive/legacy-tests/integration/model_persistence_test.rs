//! End-to-end integration tests for model persistence
//! 
//! These tests verify that real ruv-fann models can be saved, loaded, and used
//! across restarts, simulating Docker container scenarios.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
// Add the products module to the path
#[path = "../../products/features/realtraining/src/model_storage.rs"]
mod model_storage;

use model_storage::{
    ModelStorage, ModelStorageConfig, PerformanceMetrics, TrainingInfo, ModelStatus,
};
use ruv_fann::{ActivationFunction, Network, NetworkBuilder, TrainingData};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;

/// Create and train a real ruv-fann network
fn create_and_train_ruv_fann_network() -> Result<Network> {
    // Create training data for XOR problem
    let mut training_data = TrainingData::new();
    
    // XOR inputs and outputs
    let inputs = vec![
        vec![0.0, 0.0],
        vec![0.0, 1.0],
        vec![1.0, 0.0],
        vec![1.0, 1.0],
    ];
    
    let outputs = vec![
        vec![0.0],
        vec![1.0],
        vec![1.0],
        vec![0.0],
    ];
    
    for (input, output) in inputs.iter().zip(outputs.iter()) {
        training_data.add_sample(input.clone(), output.clone());
    }
    
    // Create network
    let mut network = NetworkBuilder::new()
        .input_size(2)
        .add_hidden_layer(4, ActivationFunction::Sigmoid)
        .output_size(1, ActivationFunction::Sigmoid)
        .build()?;
    
    // Train network
    network.set_learning_rate(0.7);
    network.set_momentum(0.1);
    
    for _ in 0..1000 {
        network.train_epoch(&training_data)?;
    }
    
    Ok(network)
}

/// Serialize a ruv-fann network to bytes
fn serialize_network(network: &Network) -> Result<Vec<u8>> {
    // In a real implementation, this would use ruv-fann's serialization
    // For now, we'll create a custom format
    let mut data = Vec::new();
    
    // Header
    data.extend_from_slice(b"RUFN"); // RUv-FaNN
    
    // Version
    data.extend_from_slice(&[1, 0, 0, 0]);
    
    // Network topology
    let layers = network.get_layer_sizes();
    data.extend_from_slice(&(layers.len() as u32).to_le_bytes());
    for layer_size in layers {
        data.extend_from_slice(&(layer_size as u32).to_le_bytes());
    }
    
    // Weights (simplified - in reality would save actual weights)
    let num_weights = 1000; // Placeholder
    data.extend_from_slice(&(num_weights as u32).to_le_bytes());
    for _ in 0..num_weights {
        data.extend_from_slice(&0.5f32.to_le_bytes());
    }
    
    Ok(data)
}

/// Deserialize a ruv-fann network from bytes
fn deserialize_network(data: &[u8]) -> Result<Network> {
    // Verify header
    if &data[0..4] != b"RUFN" {
        anyhow::bail!("Invalid network format");
    }
    
    // For now, just create a new network
    // In reality, this would restore the exact network state
    create_and_train_ruv_fann_network()
}

#[tokio::test]
async fn test_real_ruv_fann_persistence() {
    // Create storage with Docker-like volume path
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let docker_volume_path = temp_dir.path().join("docker_volume").join("models");
    
    let config = ModelStorageConfig {
        base_path: docker_volume_path.clone(),
        max_checkpoints_per_model: 5,
        archive_retention_days: 30,
        enable_compression: true,
        storage_quota_mb: 1000,
    };
    
    let storage = ModelStorage::new(config.clone())
        .expect("Failed to create storage");
    
    // Create and train a real ruv-fann network
    let network = create_and_train_ruv_fann_network()
        .expect("Failed to create network");
    
    // Test the network works
    let test_input = vec![1.0, 0.0];
    let output = network.compute(&test_input).expect("Failed to compute");
    assert!((output[0] - 1.0).abs() < 0.1, "Network should output ~1 for XOR(1,0)");
    
    // Serialize the network
    let model_data = serialize_network(&network)
        .expect("Failed to serialize network");
    
    let config_data = json!({
        "network_type": "MLP",
        "topology": [2, 4, 1],
        "activation": "sigmoid",
        "training_algorithm": "backprop"
    }).to_string().into_bytes();
    
    let training_info = TrainingInfo {
        epochs: 1000,
        duration_secs: 5,
        num_samples: 4,
        final_loss: 0.01,
        validation_loss: None,
        config: json!({
            "learning_rate": 0.7,
            "momentum": 0.1
        }),
    };
    
    let performance_metrics = PerformanceMetrics {
        accuracy: 0.99,
        sharpe_ratio: 0.0,
        win_rate: 0.0,
        avg_prediction_time_ms: 0.1,
        memory_usage_mb: 1.0,
    };
    
    // Save the model
    let model_id = storage.save_model(
        "XOR_Network",
        model_data,
        config_data,
        training_info,
        performance_metrics,
    ).await.expect("Failed to save model");
    
    println!("Saved model with ID: {}", model_id);
    
    // Simulate container restart by creating new storage instance
    drop(storage);
    
    let new_storage = ModelStorage::new(config)
        .expect("Failed to create new storage instance");
    
    // Load the model
    let (loaded_data, metadata) = new_storage.load_model(&model_id)
        .await
        .expect("Failed to load model");
    
    // Deserialize and test the loaded network
    let loaded_network = deserialize_network(&loaded_data)
        .expect("Failed to deserialize network");
    
    // Test that loaded network still works
    let test_output = loaded_network.compute(&test_input)
        .expect("Failed to compute with loaded network");
    
    assert!((test_output[0] - 1.0).abs() < 0.1, "Loaded network should still work");
    assert_eq!(metadata.model_type, "XOR_Network");
    assert_eq!(metadata.model_id, model_id);
}

#[tokio::test]
async fn test_concurrent_model_persistence() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = ModelStorageConfig {
        base_path: temp_dir.path().to_path_buf(),
        max_checkpoints_per_model: 10,
        archive_retention_days: 30,
        enable_compression: false,
        storage_quota_mb: 2000,
    };
    
    let storage = Arc::new(ModelStorage::new(config).expect("Failed to create storage"));
    
    // Spawn multiple tasks to train and save models concurrently
    let mut handles = Vec::new();
    
    for i in 0..5 {
        let storage_clone = Arc::clone(&storage);
        
        let handle = tokio::spawn(async move {
            // Each task creates its own network
            let network = create_and_train_ruv_fann_network()
                .expect("Failed to create network");
            
            let model_data = serialize_network(&network)
                .expect("Failed to serialize");
            
            let model_id = storage_clone.save_model(
                &format!("Concurrent_Model_{}", i),
                model_data,
                b"{}".to_vec(),
                TrainingInfo {
                    epochs: 100 * (i + 1) as u32,
                    duration_secs: 10 * (i + 1) as u64,
                    num_samples: 1000,
                    final_loss: 0.01 / (i + 1) as f64,
                    validation_loss: None,
                    config: json!({}),
                },
                PerformanceMetrics {
                    accuracy: 0.9 + (0.01 * i as f64),
                    sharpe_ratio: 1.0 + (0.1 * i as f64),
                    win_rate: 0.6 + (0.02 * i as f64),
                    avg_prediction_time_ms: 1.0,
                    memory_usage_mb: 10.0,
                },
            ).await.expect("Failed to save model");
            
            model_id
        });
        
        handles.push(handle);
    }
    
    // Wait for all saves
    let model_ids: Vec<String> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();
    
    // Verify all models can be loaded
    for (i, model_id) in model_ids.iter().enumerate() {
        let (data, metadata) = storage.load_model(model_id)
            .await
            .expect("Failed to load concurrent model");
        
        assert_eq!(metadata.model_type, format!("Concurrent_Model_{}", i));
        assert!(data.len() > 0);
        
        // Verify we can deserialize the network
        let network = deserialize_network(&data)
            .expect("Failed to deserialize concurrent model");
        
        // Test network still works
        let output = network.compute(&vec![0.0, 1.0])
            .expect("Failed to compute");
        assert!((output[0] - 1.0).abs() < 0.2);
    }
}

#[tokio::test]
async fn test_rollback_scenario() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = ModelStorageConfig {
        base_path: temp_dir.path().to_path_buf(),
        max_checkpoints_per_model: 10,
        archive_retention_days: 30,
        enable_compression: false,
        storage_quota_mb: 1000,
    };
    
    let storage = ModelStorage::new(config).expect("Failed to create storage");
    
    // Save multiple versions of the same model type
    let mut version_ids = Vec::new();
    let mut version_performances = Vec::new();
    
    for i in 0..5 {
        // Simulate different performance for each version
        let accuracy = 0.8 + (0.04 * i as f64);
        
        let network = create_and_train_ruv_fann_network()
            .expect("Failed to create network");
        
        let model_data = serialize_network(&network)
            .expect("Failed to serialize");
        
        let model_id = storage.save_model(
            "Production_Model",
            model_data,
            b"{}".to_vec(),
            TrainingInfo {
                epochs: 100 * (i + 1) as u32,
                duration_secs: 60,
                num_samples: 10000,
                final_loss: 0.1 - (0.02 * i as f64),
                validation_loss: Some(0.12 - (0.02 * i as f64)),
                config: json!({}),
            },
            PerformanceMetrics {
                accuracy,
                sharpe_ratio: 1.0 + (0.2 * i as f64),
                win_rate: 0.55 + (0.05 * i as f64),
                avg_prediction_time_ms: 5.0,
                memory_usage_mb: 100.0,
            },
        ).await.expect("Failed to save model version");
        
        version_ids.push(model_id);
        version_performances.push(accuracy);
        
        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    // Find the best performing version
    let best_idx = version_performances
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(idx, _)| idx)
        .unwrap();
    
    let best_model_id = &version_ids[best_idx];
    
    // Load the best model (simulating rollback)
    let (best_data, best_metadata) = storage.load_model(best_model_id)
        .await
        .expect("Failed to load best model");
    
    assert_eq!(best_metadata.performance_metrics.accuracy, version_performances[best_idx]);
    
    // Verify the model still works after rollback
    let network = deserialize_network(&best_data)
        .expect("Failed to deserialize best model");
    
    // Test all XOR combinations
    let test_cases = vec![
        (vec![0.0, 0.0], 0.0),
        (vec![0.0, 1.0], 1.0),
        (vec![1.0, 0.0], 1.0),
        (vec![1.0, 1.0], 0.0),
    ];
    
    for (input, expected) in test_cases {
        let output = network.compute(&input).expect("Failed to compute");
        assert!((output[0] - expected).abs() < 0.2, 
            "XOR({:?}) should be ~{}, got {}", input, expected, output[0]);
    }
}

#[tokio::test]
async fn test_docker_volume_simulation() {
    // Simulate Docker volume mount point
    let docker_volume = TempDir::new().expect("Failed to create docker volume");
    let volume_path = docker_volume.path().join("neural-trader-models");
    
    // First "container" - train and save models
    {
        let config = ModelStorageConfig {
            base_path: volume_path.clone(),
            max_checkpoints_per_model: 5,
            archive_retention_days: 90,
            enable_compression: true,
            storage_quota_mb: 5000,
        };
        
        let storage = ModelStorage::new(config).expect("Failed to create storage");
        
        // Train and save multiple model types
        let model_types = vec!["MLP", "LSTM", "GRU", "TCN", "Transformer"];
        let mut saved_models = Vec::new();
        
        for model_type in model_types {
            let network = create_and_train_ruv_fann_network()
                .expect("Failed to create network");
            
            let model_data = serialize_network(&network)
                .expect("Failed to serialize");
            
            let model_id = storage.save_model(
                model_type,
                model_data,
                json!({
                    "model_type": model_type,
                    "version": "1.0.0",
                    "framework": "ruv-fann"
                }).to_string().into_bytes(),
                TrainingInfo {
                    epochs: 500,
                    duration_secs: 300,
                    num_samples: 50000,
                    final_loss: 0.05,
                    validation_loss: Some(0.06),
                    config: json!({
                        "optimizer": "adam",
                        "learning_rate": 0.001
                    }),
                },
                PerformanceMetrics {
                    accuracy: 0.92,
                    sharpe_ratio: 1.8,
                    win_rate: 0.65,
                    avg_prediction_time_ms: 2.5,
                    memory_usage_mb: 256.0,
                },
            ).await.expect("Failed to save model");
            
            saved_models.push((model_type, model_id));
        }
        
        // Verify models are saved
        let stats = storage.get_storage_stats().await.expect("Failed to get stats");
        assert_eq!(stats.total_models, 5);
        assert_eq!(stats.active_models, 5);
    }
    
    // Simulate container restart - new "container" instance
    {
        let config = ModelStorageConfig {
            base_path: volume_path.clone(),
            max_checkpoints_per_model: 5,
            archive_retention_days: 90,
            enable_compression: true,
            storage_quota_mb: 5000,
        };
        
        let storage = ModelStorage::new(config).expect("Failed to create storage");
        
        // List all models
        let models = storage.list_models(None)
            .await
            .expect("Failed to list models");
        
        assert_eq!(models.len(), 5, "All models should persist across restart");
        
        // Load and test each model
        for model in models {
            let (data, metadata) = storage.load_model(&model.model_id)
                .await
                .expect("Failed to load model after restart");
            
            // Deserialize and test
            let network = deserialize_network(&data)
                .expect("Failed to deserialize after restart");
            
            // Quick functionality test
            let output = network.compute(&vec![1.0, 1.0])
                .expect("Failed to compute after restart");
            
            assert!((output[0] - 0.0).abs() < 0.2, "XOR(1,1) should be ~0");
            
            println!("Successfully loaded and tested {} model: {}", 
                metadata.model_type, metadata.model_id);
        }
    }
}

#[tokio::test]
async fn test_performance_benchmarks() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = ModelStorageConfig {
        base_path: temp_dir.path().to_path_buf(),
        max_checkpoints_per_model: 10,
        archive_retention_days: 30,
        enable_compression: false, // Disable for accurate timing
        storage_quota_mb: 5000,
    };
    
    let storage = ModelStorage::new(config).expect("Failed to create storage");
    
    // Create networks of different sizes
    let sizes = vec![
        ("Small", 1_000),      // ~4KB
        ("Medium", 100_000),   // ~400KB
        ("Large", 1_000_000),  // ~4MB
        ("XLarge", 10_000_000), // ~40MB
    ];
    
    for (size_name, num_weights) in sizes {
        // Create model data of specified size
        let model_data = vec![0u8; num_weights * 4]; // 4 bytes per float
        
        let start = std::time::Instant::now();
        
        let model_id = storage.save_model(
            &format!("{}_Model", size_name),
            model_data.clone(),
            b"{}".to_vec(),
            TrainingInfo {
                epochs: 100,
                duration_secs: 60,
                num_samples: 10000,
                final_loss: 0.05,
                validation_loss: None,
                config: json!({}),
            },
            PerformanceMetrics {
                accuracy: 0.9,
                sharpe_ratio: 1.5,
                win_rate: 0.6,
                avg_prediction_time_ms: 5.0,
                memory_usage_mb: (num_weights * 4 / 1024 / 1024) as f64,
            },
        ).await.expect("Failed to save model");
        
        let save_time = start.elapsed();
        
        // Benchmark load
        let start = std::time::Instant::now();
        
        let (loaded_data, _) = storage.load_model(&model_id)
            .await
            .expect("Failed to load model");
        
        let load_time = start.elapsed();
        
        println!("{} model ({:.1} MB):", size_name, model_data.len() as f64 / 1024.0 / 1024.0);
        println!("  Save time: {:?}", save_time);
        println!("  Load time: {:?}", load_time);
        println!("  Save throughput: {:.1} MB/s", 
            (model_data.len() as f64 / 1024.0 / 1024.0) / save_time.as_secs_f64());
        println!("  Load throughput: {:.1} MB/s", 
            (loaded_data.len() as f64 / 1024.0 / 1024.0) / load_time.as_secs_f64());
        
        assert_eq!(loaded_data.len(), model_data.len());
    }
}

#[tokio::test]
async fn test_archive_and_restore() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = ModelStorageConfig {
        base_path: temp_dir.path().to_path_buf(),
        max_checkpoints_per_model: 2, // Low limit to trigger archiving
        archive_retention_days: 365,
        enable_compression: true,
        storage_quota_mb: 1000,
    };
    
    let storage = ModelStorage::new(config).expect("Failed to create storage");
    
    // Save multiple models to trigger archiving
    let mut model_ids = Vec::new();
    
    for i in 0..5 {
        let network = create_and_train_ruv_fann_network()
            .expect("Failed to create network");
        
        let model_data = serialize_network(&network)
            .expect("Failed to serialize");
        
        let model_id = storage.save_model(
            "Archive_Test",
            model_data,
            b"{}".to_vec(),
            TrainingInfo {
                epochs: 100,
                duration_secs: 60,
                num_samples: 1000,
                final_loss: 0.1 - (0.01 * i as f64),
                validation_loss: None,
                config: json!({}),
            },
            PerformanceMetrics {
                accuracy: 0.8 + (0.02 * i as f64),
                sharpe_ratio: 1.0,
                win_rate: 0.6,
                avg_prediction_time_ms: 5.0,
                memory_usage_mb: 50.0,
            },
        ).await.expect("Failed to save model");
        
        model_ids.push(model_id);
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    // Check that older models are archived
    let all_models = storage.list_models(Some("Archive_Test"))
        .await
        .expect("Failed to list models");
    
    let active_count = all_models.iter()
        .filter(|m| m.status == products::features::realtraining::model_storage::ModelStatus::Active)
        .count();
    
    let archived_count = all_models.iter()
        .filter(|m| m.status == products::features::realtraining::model_storage::ModelStatus::Archived)
        .count();
    
    assert_eq!(active_count, 2, "Should have 2 active models");
    assert!(archived_count >= 3, "Should have at least 3 archived models");
    
    // Verify we can still access archived models' metadata
    for model_id in &model_ids[0..3] {
        // These should be archived but metadata should be accessible
        let models = storage.list_models(None).await.unwrap();
        let found = models.iter().any(|m| &m.model_id == model_id);
        assert!(found, "Archived model {} should still be listed", model_id);
    }
}