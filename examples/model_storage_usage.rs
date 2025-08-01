//! Example usage of the ModelStorage system for ruv-fann models
//!
//! This demonstrates how to:
//! - Save ruv-fann Network<f32> models with versioning
//! - Load specific versions
//! - Use rollback functionality 
//! - Save and load checkpoints during training

use anyhow::Result;
use chrono::Utc;
use ruv_fann::Network;
use std::path::PathBuf;
use tokio;

use autonomous_platform::adapters::model_storage::{
    CheckpointMetrics, DataInfo, ModelMetadata, ModelStorage, ModelStorageConfig,
    PerformanceMetrics, SemanticVersion, TrainingParams, VersionIncrement,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Configure storage
    let config = ModelStorageConfig {
        base_path: PathBuf::from("models"),
        max_versions_per_model: 5,
        enable_compression: true,
        enable_encryption: false,
        checkpoint_frequency: 100,
    };

    // Create storage instance
    let storage = ModelStorage::new(config).await?;
    println!("✅ Storage initialized");

    // Create a sample ruv-fann network (3-5-1 topology)
    let mut network = Network::new(&[3, 5, 1]);
    println!("✅ Created 3-5-1 neural network");

    // Create metadata for the model
    let metadata = ModelMetadata {
        model_type: "price_predictor".to_string(),
        version: SemanticVersion::new(1, 0, 0),
        timestamp: Utc::now(),
        accuracy: 0.85,
        loss: 0.15,
        training_params: TrainingParams {
            learning_rate: 0.001,
            batch_size: 32,
            epochs: 1000,
            optimizer: "backprop".to_string(),
            loss_function: "mse".to_string(),
            early_stopping_patience: Some(50),
            validation_split: 0.2,
        },
        performance_metrics: PerformanceMetrics {
            mae: 0.12,
            mse: 0.025,
            rmse: 0.15,
            mape: 8.5,
            r_squared: 0.87,
            validation_loss: 0.18,
            training_loss: 0.15,
        },
        checksum: String::new(), // Will be calculated automatically
        training_duration_secs: 1800, // 30 minutes
        data_info: DataInfo {
            num_samples: 10000,
            num_features: 3,
            symbol: "BTC-USD".to_string(),
            time_range: (
                Utc::now() - chrono::Duration::days(30),
                Utc::now(),
            ),
        },
    };

    // Save the model (version 1.0.0)
    let version1 = storage
        .save_model(&network, "price_predictor", metadata.clone(), VersionIncrement::Patch)
        .await?;
    println!("✅ Saved model version {} to {:?}", version1.version, version1.path);

    // Train network further and save improved version
    println!("🔄 Simulating training improvement...");
    
    let improved_metadata = ModelMetadata {
        accuracy: 0.92,
        loss: 0.08,
        performance_metrics: PerformanceMetrics {
            mae: 0.07,
            mse: 0.015,
            rmse: 0.12,
            mape: 5.2,
            r_squared: 0.93,
            validation_loss: 0.09,
            training_loss: 0.08,
        },
        training_duration_secs: 3600, // 1 hour total
        ..metadata
    };

    // Save improved version (version 1.0.1)
    let version2 = storage
        .save_model(&network, "price_predictor", improved_metadata, VersionIncrement::Patch)
        .await?;
    println!("✅ Saved improved model version {}", version2.version);

    // Demonstrate checkpoint saving during training
    println!("💾 Saving training checkpoints...");
    
    for epoch in [100, 200, 300, 400, 500] {
        let checkpoint_metrics = CheckpointMetrics {
            epoch,
            training_loss: 0.2 - (epoch as f64 * 0.0002),
            validation_loss: 0.22 - (epoch as f64 * 0.0001),
            learning_rate: 0.001,
            timestamp: Utc::now(),
        };

        storage
            .save_checkpoint(&network, "price_predictor", epoch, checkpoint_metrics)
            .await?;
        println!("  ✅ Saved checkpoint at epoch {}", epoch);
    }

    // List all versions
    println!("\n📋 Available versions:");
    let versions = storage.list_versions("price_predictor").await;
    for (version, timestamp) in versions {
        println!("  - Version {} (saved: {})", version, timestamp.format("%Y-%m-%d %H:%M:%S"));
    }

    // Load the latest version
    println!("\n📥 Loading latest model...");
    let (loaded_network, loaded_metadata) = storage
        .load_model("price_predictor", None)
        .await?;
    println!("✅ Loaded model version {} with accuracy: {}", 
        loaded_metadata.version, loaded_metadata.accuracy);
    println!("  Network topology: {} layers", loaded_network.num_layers());

    // Load a specific version
    println!("\n⏪ Loading specific version 1.0.0...");
    let (old_network, old_metadata) = storage
        .load_model("price_predictor", Some(SemanticVersion::new(1, 0, 0)))
        .await?;
    println!("✅ Loaded version {} with accuracy: {}", 
        old_metadata.version, old_metadata.accuracy);

    // Demonstrate rollback
    println!("\n⏪ Rolling back 1 version...");
    let (rollback_network, rollback_metadata) = storage
        .rollback("price_predictor", 1)
        .await?;
    println!("✅ Rolled back to version {} with accuracy: {}", 
        rollback_metadata.version, rollback_metadata.accuracy);

    // Load a checkpoint
    println!("\n🔄 Loading checkpoint from epoch 300...");
    let (checkpoint_network, checkpoint_metrics) = storage
        .load_checkpoint("price_predictor", 300)
        .await?;
    println!("✅ Loaded checkpoint from epoch {} with training loss: {}", 
        checkpoint_metrics.epoch, checkpoint_metrics.training_loss);

    // Get storage metrics
    println!("\n📊 Storage metrics:");
    let metrics = storage.get_storage_metrics().await;
    println!("  Total models: {}", metrics.total_models);
    println!("  Total size: {} bytes", metrics.total_size_bytes);
    println!("  Storage path: {:?}", metrics.storage_path);
    for (model_type, count) in metrics.models_by_type {
        println!("  {}: {} versions", model_type, count);
    }

    // Test model prediction capability
    println!("\n🧠 Testing loaded model prediction...");
    let mut test_network = loaded_network;
    let test_input = vec![1.0, 0.5, -0.2];
    let prediction = test_network.run(&test_input);
    println!("✅ Model prediction for input {:?}: {:?}", test_input, prediction);

    println!("\n🎉 Model storage demonstration completed successfully!");
    
    Ok(())
}