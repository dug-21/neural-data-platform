//! Integration test for Model Storage with ruv-fann networks
//!
//! This test demonstrates that the model storage system works correctly
//! with real ruv-fann Network<f32> instances.

use anyhow::Result;
use chrono::Utc;
use ruv_fann::Network;
use std::time::Duration;
use tempfile::TempDir;
use tokio;

use autonomous_platform::adapters::model_storage::{
    CheckpointMetrics, DataInfo, ModelMetadata, ModelStorage, ModelStorageConfig,
    PerformanceMetrics, SemanticVersion, TrainingParams, VersionIncrement,
};

#[tokio::test]
async fn test_ruv_fann_model_storage_integration() -> Result<()> {
    // Setup temporary directory
    let temp_dir = TempDir::new()?;
    let config = ModelStorageConfig {
        base_path: temp_dir.path().to_path_buf(),
        max_versions_per_model: 3,
        enable_compression: false,
        enable_encryption: false,
        checkpoint_frequency: 50,
    };

    // Initialize storage
    let storage = ModelStorage::new(config).await?;

    // Create a ruv-fann network
    let network = Network::new(&[3, 5, 2]);

    // Verify network properties
    assert_eq!(network.num_layers(), 3);
    assert_eq!(network.num_inputs(), 3);
    assert_eq!(network.num_outputs(), 2);

    // Create metadata
    let metadata = ModelMetadata {
        model_type: "integration_test".to_string(),
        version: SemanticVersion::new(1, 0, 0),
        timestamp: Utc::now(),
        accuracy: 0.87,
        loss: 0.13,
        training_params: TrainingParams {
            learning_rate: 0.001,
            batch_size: 32,
            epochs: 500,
            optimizer: "backprop".to_string(),
            loss_function: "mse".to_string(),
            early_stopping_patience: Some(25),
            validation_split: 0.2,
        },
        performance_metrics: PerformanceMetrics {
            mae: 0.09,
            mse: 0.016,
            rmse: 0.127,
            mape: 6.8,
            r_squared: 0.89,
            validation_loss: 0.15,
            training_loss: 0.13,
        },
        checksum: String::new(),
        training_duration_secs: 900,
        data_info: DataInfo {
            num_samples: 5000,
            num_features: 3,
            symbol: "TEST-DATA".to_string(),
            time_range: (
                Utc::now() - chrono::Duration::hours(12),
                Utc::now(),
            ),
        },
    };

    // Test 1: Save model
    let version1 = storage
        .save_model(&network, "integration_test", metadata.clone(), VersionIncrement::Patch)
        .await?;

    assert_eq!(version1.model_type, "integration_test");
    assert_eq!(version1.version, SemanticVersion::new(1, 0, 0));
    assert!(version1.path.exists());
    assert!(version1.size_bytes > 0);

    // Test 2: Load model
    let (loaded_network, loaded_metadata) = storage
        .load_model("integration_test", None)
        .await?;

    assert_eq!(loaded_network.num_layers(), network.num_layers());
    assert_eq!(loaded_network.num_inputs(), network.num_inputs());
    assert_eq!(loaded_network.num_outputs(), network.num_outputs());
    assert_eq!(loaded_metadata.model_type, "integration_test");
    assert_eq!(loaded_metadata.accuracy, 0.87);

    // Test 3: Save checkpoint
    let checkpoint_metrics = CheckpointMetrics {
        epoch: 100,
        training_loss: 0.12,
        validation_loss: 0.14,
        learning_rate: 0.001,
        timestamp: Utc::now(),
    };

    storage
        .save_checkpoint(&network, "integration_test", 100, checkpoint_metrics.clone())
        .await?;

    // Test 4: Load checkpoint
    let (checkpoint_network, checkpoint_metrics_loaded) = storage
        .load_checkpoint("integration_test", 100)
        .await?;

    assert_eq!(checkpoint_network.num_layers(), network.num_layers());
    assert_eq!(checkpoint_metrics_loaded.epoch, 100);
    assert_eq!(checkpoint_metrics_loaded.training_loss, 0.12);

    // Test 5: Multiple versions
    let improved_metadata = ModelMetadata {
        accuracy: 0.92,
        loss: 0.08,
        ..metadata
    };

    let version2 = storage
        .save_model(&network, "integration_test", improved_metadata, VersionIncrement::Patch)
        .await?;

    assert_eq!(version2.version, SemanticVersion::new(1, 0, 1));

    // Test 6: List versions
    let versions = storage.list_versions("integration_test").await;
    assert_eq!(versions.len(), 2);
    
    // Verify versions are sorted
    assert_eq!(versions[0].0, SemanticVersion::new(1, 0, 0));
    assert_eq!(versions[1].0, SemanticVersion::new(1, 0, 1));

    // Test 7: Load specific version
    let (old_network, old_metadata) = storage
        .load_model("integration_test", Some(SemanticVersion::new(1, 0, 0)))
        .await?;

    assert_eq!(old_network.num_layers(), network.num_layers());
    assert_eq!(old_metadata.accuracy, 0.87); // Original accuracy

    // Test 8: Rollback functionality
    let (rollback_network, rollback_metadata) = storage
        .rollback("integration_test", 1)
        .await?;

    assert_eq!(rollback_network.num_layers(), network.num_layers());
    assert_eq!(rollback_metadata.version, SemanticVersion::new(1, 0, 0));
    assert_eq!(rollback_metadata.accuracy, 0.87);

    // Test 9: Storage metrics
    let metrics = storage.get_storage_metrics().await;
    assert_eq!(metrics.total_models, 2);
    assert!(metrics.total_size_bytes > 0);
    assert_eq!(metrics.models_by_type.get("integration_test"), Some(&2));

    // Test 10: Network functionality after loading
    let mut test_network = loaded_network;
    let test_input = vec![0.5, -0.2, 0.8];
    let prediction = test_network.run(&test_input);
    
    assert_eq!(prediction.len(), 2); // Should have 2 outputs
    assert!(prediction.iter().all(|&x| x.is_finite())); // All outputs should be finite

    println!("✅ All integration tests passed!");
    Ok(())
}

#[tokio::test]
async fn test_model_storage_version_limit() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config = ModelStorageConfig {
        base_path: temp_dir.path().to_path_buf(),
        max_versions_per_model: 2, // Only keep 2 versions
        ..Default::default()
    };

    let storage = ModelStorage::new(config).await?;
    let network = Network::new(&[2, 3, 1]);

    // Create base metadata
    let base_metadata = ModelMetadata {
        model_type: "version_test".to_string(),
        version: SemanticVersion::new(1, 0, 0),
        timestamp: Utc::now(),
        accuracy: 0.80,
        loss: 0.20,
        training_params: TrainingParams {
            learning_rate: 0.001,
            batch_size: 16,
            epochs: 100,
            optimizer: "backprop".to_string(),
            loss_function: "mse".to_string(),
            early_stopping_patience: None,
            validation_split: 0.1,
        },
        performance_metrics: PerformanceMetrics {
            mae: 0.15,
            mse: 0.04,
            rmse: 0.20,
            mape: 12.0,
            r_squared: 0.80,
            validation_loss: 0.22,
            training_loss: 0.20,
        },
        checksum: String::new(),
        training_duration_secs: 300,
        data_info: DataInfo {
            num_samples: 1000,
            num_features: 2,
            symbol: "VERSION-TEST".to_string(),
            time_range: (
                Utc::now() - chrono::Duration::hours(6),
                Utc::now(),
            ),
        },
    };

    // Save 4 versions (should only keep the last 2)
    for i in 0..4 {
        let mut metadata = base_metadata.clone();
        metadata.accuracy = 0.80 + (i as f64 * 0.02);
        
        storage
            .save_model(&network, "version_test", metadata, VersionIncrement::Patch)
            .await?;

        // Small delay to ensure different timestamps
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Check that only 2 versions are kept (the latest ones)
    let versions = storage.list_versions("version_test").await;
    assert_eq!(versions.len(), 2);
    
    // Should have versions 1.0.2 and 1.0.3 (the last two)
    assert_eq!(versions[0].0, SemanticVersion::new(1, 0, 2));
    assert_eq!(versions[1].0, SemanticVersion::new(1, 0, 3));

    println!("✅ Version limit test passed!");
    Ok(())
}

#[tokio::test]
async fn test_model_storage_checksum_verification() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config = ModelStorageConfig {
        base_path: temp_dir.path().to_path_buf(),
        ..Default::default()
    };

    let storage = ModelStorage::new(config).await?;
    let network = Network::new(&[4, 4, 1]);

    let metadata = ModelMetadata {
        model_type: "checksum_test".to_string(),
        version: SemanticVersion::new(1, 0, 0),
        timestamp: Utc::now(),
        accuracy: 0.95,
        loss: 0.05,
        training_params: TrainingParams {
            learning_rate: 0.0005,
            batch_size: 64,
            epochs: 200,
            optimizer: "backprop".to_string(),
            loss_function: "mse".to_string(),
            early_stopping_patience: Some(15),
            validation_split: 0.25,
        },
        performance_metrics: PerformanceMetrics {
            mae: 0.03,
            mse: 0.0025,
            rmse: 0.05,
            mape: 3.2,
            r_squared: 0.96,
            validation_loss: 0.06,
            training_loss: 0.05,
        },
        checksum: String::new(),
        training_duration_secs: 1200,
        data_info: DataInfo {
            num_samples: 8000,
            num_features: 4,
            symbol: "CHECKSUM-TEST".to_string(),
            time_range: (
                Utc::now() - chrono::Duration::days(7),
                Utc::now(),
            ),
        },
    };

    // Save model
    let version = storage
        .save_model(&network, "checksum_test", metadata, VersionIncrement::Patch)
        .await?;

    // Load the saved metadata and verify checksum was calculated
    let metadata_path = version.metadata_path;
    let metadata_content = tokio::fs::read_to_string(&metadata_path).await?;
    let loaded_metadata: ModelMetadata = serde_json::from_str(&metadata_content)?;
    
    assert!(!loaded_metadata.checksum.is_empty());
    assert_eq!(loaded_metadata.checksum.len(), 64); // SHA256 hex string

    // Load the model and verify the checksum is validated
    let (loaded_network, _) = storage
        .load_model("checksum_test", None)
        .await?;

    assert_eq!(loaded_network.num_layers(), network.num_layers());

    println!("✅ Checksum verification test passed!");
    Ok(())
}