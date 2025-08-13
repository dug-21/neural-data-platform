//! Test for model storage deletion bug fix
//!
//! This test verifies that when saving multiple model types, only versions
//! of the same model type are deleted when the version limit is exceeded.

use std::path::PathBuf;
use tempfile::TempDir;
use ruv_fann::Network;

#[path = "../src/adapters/model_storage.rs"]
mod model_storage;

use model_storage::*;

#[tokio::test]
async fn test_model_deletion_by_type() {
    let temp_dir = TempDir::new().unwrap();
    let config = ModelStorageConfig {
        base_path: temp_dir.path().to_path_buf(),
        max_versions_per_model: 2, // Only keep 2 versions per model type
        ..Default::default()
    };

    let storage = ModelStorage::new(config).await.unwrap();

    // Create a simple network
    let network = Network::<f32>::new_simple(3, 4, 1).unwrap();
    let metadata = ModelMetadata {
        description: "Test model".to_string(),
        hyperparameters: std::collections::HashMap::new(),
        training_metrics: None,
        model_size_mb: 0.1,
        created_by: "test".to_string(),
    };

    // Save 3 versions of healthcare_base_model
    for i in 1..=3 {
        storage
            .save_model(
                &network,
                "healthcare_base_model",
                metadata.clone(),
                VersionIncrement::Patch,
            )
            .await
            .unwrap();
    }

    // Save 1 version of real_estate_base_model
    let real_estate_version = storage
        .save_model(
            &network,
            "real_estate_base_model",
            metadata.clone(),
            VersionIncrement::Patch,
        )
        .await
        .unwrap();

    // Verify that healthcare_base_model has only 2 versions (oldest was deleted)
    let healthcare_versions = storage.list_versions("healthcare_base_model").await;
    assert_eq!(healthcare_versions.len(), 2, "Healthcare model should have 2 versions");

    // Verify that real_estate_base_model has 1 version (should not be affected)
    let real_estate_versions = storage.list_versions("real_estate_base_model").await;
    assert_eq!(real_estate_versions.len(), 1, "Real estate model should have 1 version");

    // Verify that the real estate model file still exists
    assert!(real_estate_version.path.exists(), "Real estate model file should still exist");

    println!("✅ Model deletion fix test passed!");
}

#[tokio::test]
async fn test_mixed_model_types_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let config = ModelStorageConfig {
        base_path: temp_dir.path().to_path_buf(),
        max_versions_per_model: 1, // Only keep 1 version per model type
        ..Default::default()
    };

    let storage = ModelStorage::new(config).await.unwrap();
    let network = Network::<f32>::new_simple(3, 4, 1).unwrap();
    let metadata = ModelMetadata {
        description: "Test model".to_string(),
        hyperparameters: std::collections::HashMap::new(),
        training_metrics: None,
        model_size_mb: 0.1,
        created_by: "test".to_string(),
    };

    // Interleave saving different model types
    let healthcare_v1 = storage
        .save_model(&network, "healthcare_base_model", metadata.clone(), VersionIncrement::Patch)
        .await
        .unwrap();

    let real_estate_v1 = storage
        .save_model(&network, "real_estate_base_model", metadata.clone(), VersionIncrement::Patch)
        .await
        .unwrap();

    let tech_v1 = storage
        .save_model(&network, "tech_base_model", metadata.clone(), VersionIncrement::Patch)
        .await
        .unwrap();

    // Add second versions - this should delete the first versions of each type
    let healthcare_v2 = storage
        .save_model(&network, "healthcare_base_model", metadata.clone(), VersionIncrement::Patch)
        .await
        .unwrap();

    let real_estate_v2 = storage
        .save_model(&network, "real_estate_base_model", metadata.clone(), VersionIncrement::Patch)
        .await
        .unwrap();

    // Verify each model type has only 1 version
    assert_eq!(storage.list_versions("healthcare_base_model").await.len(), 1);
    assert_eq!(storage.list_versions("real_estate_base_model").await.len(), 1);
    assert_eq!(storage.list_versions("tech_base_model").await.len(), 1);

    // Verify the latest versions exist and old ones don't
    assert!(!healthcare_v1.path.exists(), "Old healthcare v1 should be deleted");
    assert!(healthcare_v2.path.exists(), "New healthcare v2 should exist");
    
    assert!(!real_estate_v1.path.exists(), "Old real estate v1 should be deleted");
    assert!(real_estate_v2.path.exists(), "New real estate v2 should exist");
    
    assert!(tech_v1.path.exists(), "Tech v1 should still exist (only 1 version)");

    println!("✅ Mixed model types cleanup test passed!");
}