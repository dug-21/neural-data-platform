//! Unit tests for model storage adapter with simulated ruv-fann models
//! 
//! This file tests the ModelStorage component in isolation, ensuring all methods
//! work correctly with binary model data and proper persistence to disk.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;

// Import MD5 for checksum validation
use md5;

// Add the products module to the path
#[path = "../../products/features/realtraining/src/model_storage.rs"]
mod model_storage;

use model_storage::{
    ModelMetadata, ModelStatus, ModelStorage, ModelStorageConfig, PerformanceMetrics,
    TrainingInfo, ModelFilePaths,
};
use serde_json::json;
use std::path::PathBuf;

/// Create a test storage instance with temporary directory
async fn create_test_storage() -> (ModelStorage, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config = ModelStorageConfig {
        base_path: temp_dir.path().to_path_buf(),
        max_checkpoints_per_model: 3,
        archive_retention_days: 7,
        enable_compression: true,
        storage_quota_mb: 100,
    };
    
    let storage = ModelStorage::new(config).expect("Failed to create storage");
    (storage, temp_dir)
}

/// Create a realistic FANN-like model binary representation with proper structure
fn create_realistic_model_data(input_size: usize, hidden_sizes: &[usize], output_size: usize) -> Vec<u8> {
    let mut buffer = Vec::new();
    
    // FANN magic header
    buffer.extend_from_slice(b"FANN");
    
    // Version (2.1.0)
    buffer.extend_from_slice(&[2u8, 1, 0, 0]);
    
    // Network topology
    let mut layer_sizes = vec![input_size];
    layer_sizes.extend_from_slice(hidden_sizes);
    layer_sizes.push(output_size);
    
    buffer.extend_from_slice(&(layer_sizes.len() as u32).to_le_bytes());
    for &size in &layer_sizes {
        buffer.extend_from_slice(&(size as u32).to_le_bytes());
    }
    
    // Activation functions (one per layer)
    for i in 0..layer_sizes.len() {
        let activation = if i == 0 || i == layer_sizes.len() - 1 {
            0u32 // Linear for input/output
        } else {
            1u32 // Sigmoid for hidden
        };
        buffer.extend_from_slice(&activation.to_le_bytes());
    }
    
    // Calculate total weights needed
    let mut total_weights = 0;
    for i in 1..layer_sizes.len() {
        total_weights += layer_sizes[i-1] * layer_sizes[i];
    }
    
    // Write weights
    buffer.extend_from_slice(&(total_weights as u32).to_le_bytes());
    for i in 0..total_weights {
        let weight = (i as f32 * 0.001).sin(); // Deterministic but varied weights
        buffer.extend_from_slice(&weight.to_le_bytes());
    }
    
    // Calculate total biases needed
    let total_biases: usize = layer_sizes[1..].iter().sum();
    
    // Write biases
    buffer.extend_from_slice(&(total_biases as u32).to_le_bytes());
    for i in 0..total_biases {
        let bias = (i as f32 * 0.0001).cos(); // Deterministic but varied biases
        buffer.extend_from_slice(&bias.to_le_bytes());
    }
    
    // Training parameters
    buffer.extend_from_slice(&0.01f32.to_le_bytes()); // Learning rate
    buffer.extend_from_slice(&0.9f32.to_le_bytes());  // Momentum
    
    buffer
}

/// Verify model data integrity by parsing structure
fn verify_model_structure(data: &[u8], expected_input: usize, expected_hidden: &[usize], expected_output: usize) -> bool {
    if data.len() < 16 {
        return false;
    }
    
    let mut cursor = 0;
    
    // Check magic header
    if &data[cursor..cursor + 4] != b"FANN" {
        return false;
    }
    cursor += 4;
    
    // Skip version
    cursor += 4;
    
    // Read layer count
    let layer_count = u32::from_le_bytes([
        data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]
    ]) as usize;
    cursor += 4;
    
    if layer_count != 2 + expected_hidden.len() {
        return false;
    }
    
    // Read and verify layer sizes
    let input_size = u32::from_le_bytes([
        data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]
    ]) as usize;
    cursor += 4;
    
    if input_size != expected_input {
        return false;
    }
    
    for &expected_size in expected_hidden {
        let size = u32::from_le_bytes([
            data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]
        ]) as usize;
        cursor += 4;
        
        if size != expected_size {
            return false;
        }
    }
    
    let output_size = u32::from_le_bytes([
        data[cursor], data[cursor + 1], data[cursor + 2], data[cursor + 3]
    ]) as usize;
    
    output_size == expected_output
}

/// Create sample training info
fn create_training_info() -> TrainingInfo {
    TrainingInfo {
        epochs: 100,
        duration_secs: 3600,
        num_samples: 10000,
        final_loss: 0.05,
        validation_loss: Some(0.06),
        config: json!({
            "learning_rate": 0.001,
            "batch_size": 32,
            "optimizer": "adam",
            "network_topology": [10, 20, 15, 5],
            "activations": ["sigmoid", "tanh", "linear"]
        }),
    }
}

/// Create sample performance metrics
fn create_performance_metrics() -> PerformanceMetrics {
    PerformanceMetrics {
        accuracy: 0.92,
        sharpe_ratio: 1.5,
        win_rate: 0.62,
        avg_prediction_time_ms: 10.5,
        memory_usage_mb: 512.0,
    }
}

#[tokio::test]
async fn test_save_and_load_model_with_real_structure() {
    let (storage, _temp_dir) = create_test_storage().await;
    
    // Create a realistic model structure
    let model_data = create_realistic_model_data(10, &[20, 15], 5);
    let config_data = json!({
        "model_type": "MLP",
        "architecture": [10, 20, 15, 5],
        "activations": ["sigmoid", "tanh", "linear"],
        "learning_rate": 0.01,
        "momentum": 0.9,
        "checksum": format!("{:x}", md5::compute(&model_data))
    }).to_string().into_bytes();
    
    // Save the model
    let model_id = storage.save_model(
        "MLP",
        model_data.clone(),
        config_data.clone(),
        create_training_info(),
        create_performance_metrics(),
    ).await.expect("Failed to save model");
    
    // Load the model back
    let (loaded_data, metadata) = storage.load_model(&model_id)
        .await
        .expect("Failed to load model");
    
    // Verify data integrity
    assert_eq!(loaded_data, model_data);
    assert_eq!(metadata.model_type, "MLP");
    assert_eq!(metadata.model_id, model_id);
    assert_eq!(metadata.status, ModelStatus::Active);
    
    // Verify model structure is preserved
    assert!(verify_model_structure(&loaded_data, 10, &[20, 15], 5),
        "Model structure should be preserved");
    
    // Verify checksum matches
    let loaded_checksum = format!("{:x}", md5::compute(&loaded_data));
    let original_checksum = format!("{:x}", md5::compute(&model_data));
    assert_eq!(loaded_checksum, original_checksum, "Checksums should match");
}

#[tokio::test]
async fn test_model_versioning_with_incremental_changes() {
    let (storage, _temp_dir) = create_test_storage().await;
    
    let mut versions = Vec::new();
    let mut model_data_versions = Vec::new();
    
    // Create and save 5 different versions with incremental changes
    for i in 0..5 {
        // Create model with slightly different architecture for each version
        let hidden_size = 20 + i * 2;
        let model_data = create_realistic_model_data(10, &[hidden_size], 5);
        let config_data = json!({
            "version": i,
            "hidden_size": hidden_size,
            "learning_rate": 0.1 + i as f64 * 0.01,
            "checksum": format!("{:x}", md5::compute(&model_data))
        }).to_string().into_bytes();
        
        // Add a small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        let model_id = storage.save_model(
            "LSTM",
            model_data.clone(),
            config_data,
            create_training_info(),
            create_performance_metrics(),
        ).await.expect("Failed to save model");
        
        let (_, metadata) = storage.load_model(&model_id)
            .await
            .expect("Failed to load model");
        
        versions.push(metadata.version.clone());
        model_data_versions.push((model_id, model_data, hidden_size));
    }
    
    // Verify versions are chronologically ordered
    for i in 1..versions.len() {
        assert!(versions[i-1] < versions[i], 
            "Versions should be chronologically ordered: {} >= {}", 
            versions[i-1], versions[i]);
    }
    
    // Verify each version maintained its unique characteristics
    for (i, (model_id, original_data, expected_hidden_size)) in model_data_versions.iter().enumerate() {
        let (loaded_data, _) = storage.load_model(model_id)
            .await
            .expect("Failed to load model");
        
        // Verify data integrity
        assert_eq!(loaded_data, *original_data, "Version {} data should match", i);
        
        // Verify structure is preserved
        assert!(verify_model_structure(&loaded_data, 10, &[*expected_hidden_size], 5),
            "Version {} structure should be preserved", i);
    }
}

#[tokio::test]
async fn test_model_rollback_with_version_tracking() {
    let (storage, _temp_dir) = create_test_storage().await;
    
    let model_type = "GRU";
    let mut version_history = Vec::new();
    
    // Save 4 versions of the model with different characteristics
    for i in 0..4 {
        let input_size = 8 + i;
        let hidden_sizes = vec![16 + i * 2, 12 + i];
        let output_size = 3;
        
        let model_data = create_realistic_model_data(input_size, &hidden_sizes, output_size);
        let config_data = json!({
            "version": i,
            "input_size": input_size,
            "hidden_sizes": hidden_sizes,
            "output_size": output_size,
            "rollback_test": true,
            "checksum": format!("{:x}", md5::compute(&model_data))
        }).to_string().into_bytes();
        
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
        
        let model_id = storage.save_model(
            model_type,
            model_data.clone(),
            config_data,
            create_training_info(),
            create_performance_metrics(),
        ).await.expect("Failed to save model");
        
        version_history.push((model_id, model_data, input_size, hidden_sizes, output_size));
    }
    
    // Test rollback to version 1 (second model)
    let (rollback_id, expected_data, expected_input, expected_hidden, expected_output) = &version_history[1];
    let (rollback_data, rollback_metadata) = storage.load_model(rollback_id)
        .await
        .expect("Failed to load rollback model");
    
    // Verify rollback data integrity
    assert_eq!(rollback_data, *expected_data, "Rollback data should match version 1");
    assert_eq!(rollback_metadata.model_type, model_type, "Model type should match");
    
    // Verify rollback structure is correct
    assert!(verify_model_structure(&rollback_data, *expected_input, expected_hidden, *expected_output),
        "Rollback model structure should match version 1");
    
    // Verify rollback is earlier than later versions  
    let (latest_id, _, _, _, _) = &version_history[3];
    let (_, latest_metadata) = storage.load_model(latest_id)
        .await
        .expect("Failed to load latest model");
    
    assert!(rollback_metadata.saved_at < latest_metadata.saved_at,
        "Rollback version should be earlier than latest");
}

#[tokio::test] 
async fn test_concurrent_model_persistence_with_integrity() {
    let (storage, _temp_dir) = create_test_storage().await;
    let storage = Arc::new(storage);
    
    // Spawn 8 concurrent tasks to save different models
    let mut handles = Vec::new();
    
    for i in 0..8 {
        let storage_clone = Arc::clone(&storage);
        
        let handle = tokio::spawn(async move {
            let input_size = 5 + i;
            let hidden_size = 10 + i * 2;
            let output_size = 2;
            
            let model_data = create_realistic_model_data(input_size, &[hidden_size], output_size);
            let config_data = json!({
                "task_id": i,
                "input_size": input_size,
                "hidden_size": hidden_size,
                "output_size": output_size,
                "checksum": format!("{:x}", md5::compute(&model_data))
            }).to_string().into_bytes();
            
            let model_id = storage_clone.save_model(
                &format!("ConcurrentModel_{}", i),
                model_data.clone(),
                config_data,
                create_training_info(),
                create_performance_metrics(),
            ).await?;
            
            // Immediately try to load it back
            let (loaded_data, metadata) = storage_clone.load_model(&model_id).await?;
            
            // Verify data integrity
            if loaded_data != model_data {
                return Err(anyhow::anyhow!("Data integrity check failed for task {}", i));
            }
            
            // Verify structure integrity
            if !verify_model_structure(&loaded_data, input_size, &[hidden_size], output_size) {
                return Err(anyhow::anyhow!("Structure integrity check failed for task {}", i));
            }
            
            Ok((model_id, metadata, model_data))
        });
        
        handles.push(handle);
    }
    
    // Wait for all concurrent operations to complete
    let results: Vec<_> = futures::future::join_all(handles).await;
    
    // Verify all operations succeeded
    let mut successful_models = Vec::new();
    for (i, result) in results.into_iter().enumerate() {
        assert!(result.is_ok(), "Task {} panicked", i);
        let inner_result = result.unwrap();
        assert!(inner_result.is_ok(), "Task {} failed: {:?}", i, inner_result.err());
        successful_models.push(inner_result.unwrap());
    }
    
    assert_eq!(successful_models.len(), 8, "All 8 concurrent operations should succeed");
    
    // Verify all models have unique IDs and correct characteristics
    let mut model_ids = std::collections::HashSet::new();
    for (i, (model_id, metadata, original_data)) in successful_models.into_iter().enumerate() {
        assert!(model_ids.insert(model_id.clone()), "Model ID {} should be unique", model_id);
        
        // Load and verify each model independently
        let (loaded_data, _) = storage.load_model(&model_id)
            .await
            .expect("Failed to reload concurrent model");
        
        // Verify data integrity
        assert_eq!(loaded_data, original_data, "Model {} data should match", i);
        
        // Verify structure integrity
        let expected_input = 5 + i;
        let expected_hidden = 10 + i * 2;
        assert!(verify_model_structure(&loaded_data, expected_input, &[expected_hidden], 2),
            "Model {} structure should be correct", i);
    }
}

#[tokio::test]
async fn test_disk_persistence_with_docker_volume_simulation() {
    let (storage, temp_dir) = create_test_storage().await;
    
    // Create and save a model
    let model_data = create_realistic_model_data(15, &[30, 25, 20], 10);
    let config_data = json!({
        "persistence_test": true,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "docker_simulation": true,
        "checksum": format!("{:x}", md5::compute(&model_data))
    }).to_string().into_bytes();
    
    let model_id = storage.save_model(
        "PersistenceTest",
        model_data.clone(),
        config_data.clone(),
        create_training_info(),
        create_performance_metrics(),
    ).await.expect("Failed to save model");
    
    // Verify files exist on disk
    let base_path = temp_dir.path();
    let model_dir = base_path.join("active").join(&model_id);
    
    assert!(model_dir.exists(), "Model directory should exist on disk");
    assert!(model_dir.join("model.bin").exists(), "Model binary should exist");
    assert!(model_dir.join("config.json").exists(), "Config file should exist");
    assert!(model_dir.join("metadata.json").exists(), "Metadata file should exist");
    
    // Verify file contents match saved data
    let saved_model_data = std::fs::read(model_dir.join("model.bin"))
        .expect("Failed to read model file");
    assert_eq!(saved_model_data, model_data, "Saved model data should match");
    
    let saved_config_data = std::fs::read(model_dir.join("config.json"))
        .expect("Failed to read config file");
    assert_eq!(saved_config_data, config_data, "Saved config data should match");
    
    // Simulate "Docker volume persistence" by creating a new storage instance
    // pointing to the same directory (simulates container restart)
    let new_config = ModelStorageConfig {
        base_path: base_path.to_path_buf(),
        max_checkpoints_per_model: 3,
        archive_retention_days: 7,
        enable_compression: true,
        storage_quota_mb: 100,
    };
    
    let new_storage = ModelStorage::new(new_config)
        .expect("Failed to create new storage instance");
    
    // Load the model from the "persistent" storage
    let (loaded_data, loaded_metadata) = new_storage.load_model(&model_id)
        .await
        .expect("Failed to load model from persistent storage");
    
    // Verify data integrity across storage instances
    assert_eq!(loaded_data, model_data, "Data should persist across storage instances");
    assert_eq!(loaded_metadata.model_id, model_id, "Model ID should persist");
    assert_eq!(loaded_metadata.model_type, "PersistenceTest", "Model type should persist");
    
    // Verify the model structure is still correct
    assert!(verify_model_structure(&loaded_data, 15, &[30, 25, 20], 10),
        "Model structure should persist across storage instances");
    
    // Verify checksums match after persistence
    let original_checksum = format!("{:x}", md5::compute(&model_data));
    let loaded_checksum = format!("{:x}", md5::compute(&loaded_data));
    assert_eq!(loaded_checksum, original_checksum, "Checksums should match after persistence");
}

#[cfg(test)]
mod performance_benchmarks {
    use super::*;
    use std::time::Instant;
    
    #[tokio::test]
    async fn benchmark_model_save_load_performance() {
        let (storage, _temp_dir) = create_test_storage().await;
        
        // Create a large model for performance testing
        let large_model_data = create_realistic_model_data(100, &[200, 150, 100], 50);
        let config_data = json!({
            "benchmark": true,
            "model_size": "large",
            "parameters_estimate": "~50000",
            "checksum": format!("{:x}", md5::compute(&large_model_data))
        }).to_string().into_bytes();
        
        println!("Model size: {} bytes", large_model_data.len());
        
        // Benchmark save operation
        let start = Instant::now();
        
        let model_id = storage.save_model(
            "BenchmarkModel",
            large_model_data.clone(),
            config_data.clone(),
            create_training_info(),
            create_performance_metrics(),
        ).await.expect("Failed to save benchmark model");
        
        let save_duration = start.elapsed();
        println!("Save large model: {:?}", save_duration);
        
        // Benchmark load operation
        let start = Instant::now();
        
        let (loaded_data, _metadata) = storage.load_model(&model_id)
            .await
            .expect("Failed to load benchmark model");
        
        let load_duration = start.elapsed();
        println!("Load large model: {:?}", load_duration);
        
        // Benchmark structure verification
        let start = Instant::now();
        
        let structure_valid = verify_model_structure(&loaded_data, 100, &[200, 150, 100], 50);
        
        let verify_duration = start.elapsed();
        println!("Verify large model structure: {:?}", verify_duration);
        
        // Verify performance is reasonable
        assert!(save_duration.as_millis() < 1000, 
            "Save should complete in under 1 second, took {:?}", save_duration);
        assert!(load_duration.as_millis() < 100, 
            "Load should complete in under 100ms, took {:?}", load_duration);
        assert!(verify_duration.as_millis() < 10,
            "Structure verification should complete in under 10ms, took {:?}", verify_duration);
        
        // Verify integrity
        assert_eq!(loaded_data, large_model_data, "Loaded data should match saved data");
        assert!(structure_valid, "Model structure should be preserved");
        
        // Calculate throughput
        let data_size_mb = large_model_data.len() as f64 / (1024.0 * 1024.0);
        let save_throughput = data_size_mb / save_duration.as_secs_f64();
        let load_throughput = data_size_mb / load_duration.as_secs_f64();
        
        println!("Model size: {:.2} MB", data_size_mb);
        println!("Save throughput: {:.2} MB/s", save_throughput);
        println!("Load throughput: {:.2} MB/s", load_throughput);
        
        // Performance expectations
        assert!(save_throughput > 0.1, "Save throughput should be > 0.1 MB/s");
        assert!(load_throughput > 1.0, "Load throughput should be > 1.0 MB/s");
    }
    
    #[tokio::test]
    async fn benchmark_concurrent_operations() {
        let (storage, _temp_dir) = create_test_storage().await;
        let storage = Arc::new(storage);
        
        println!("Starting concurrent operations benchmark...");
        
        // Benchmark 10 concurrent save operations
        let start = Instant::now();
        
        let mut handles = Vec::new();
        for i in 0..10 {
            let storage_clone = Arc::clone(&storage);
            
            let handle = tokio::spawn(async move {
                let model_data = create_realistic_model_data(20, &[40], 10);
                let config_data = json!({
                    "concurrent_id": i,
                    "checksum": format!("{:x}", md5::compute(&model_data))
                }).to_string().into_bytes();
                
                storage_clone.save_model(
                    &format!("ConcurrentBench_{}", i),
                    model_data,
                    config_data,
                    create_training_info(),
                    create_performance_metrics(),
                ).await
            });
            
            handles.push(handle);
        }
        
        let results: Vec<_> = futures::future::join_all(handles).await;
        let concurrent_save_duration = start.elapsed();
        
        // Verify all operations succeeded
        let mut model_ids = Vec::new();
        for result in results {
            assert!(result.is_ok(), "Concurrent task should not panic");
            let model_id = result.unwrap().expect("Concurrent save should succeed");
            model_ids.push(model_id);
        }
        
        println!("10 concurrent saves: {:?}", concurrent_save_duration);
        println!("Average save time: {:?}", concurrent_save_duration / 10);
        
        // Benchmark 10 concurrent load operations
        let start = Instant::now();
        
        let mut load_handles = Vec::new();
        for model_id in &model_ids {
            let storage_clone = Arc::clone(&storage);
            let model_id_clone = model_id.clone();
            
            let handle = tokio::spawn(async move {
                storage_clone.load_model(&model_id_clone).await
            });
            
            load_handles.push(handle);
        }
        
        let load_results: Vec<_> = futures::future::join_all(load_handles).await;
        let concurrent_load_duration = start.elapsed();
        
        // Verify all loads succeeded
        for result in load_results {
            assert!(result.is_ok(), "Concurrent load task should not panic");
            assert!(result.unwrap().is_ok(), "Concurrent load should succeed");
        }
        
        println!("10 concurrent loads: {:?}", concurrent_load_duration);
        println!("Average load time: {:?}", concurrent_load_duration / 10);
        
        // Performance expectations for concurrent operations
        assert!(concurrent_save_duration.as_secs() < 10, 
            "10 concurrent saves should complete in under 10 seconds");
        assert!(concurrent_load_duration.as_millis() < 1000,
            "10 concurrent loads should complete in under 1 second");
        
        // Verify data integrity across all concurrent operations
        let models_list = storage.list_models(None).await
            .expect("Failed to list models after concurrent benchmark");
        
        println!("Total models after benchmark: {}", models_list.len());
        assert_eq!(models_list.len(), 10, "Should have exactly 10 models from concurrent test");
    }
}