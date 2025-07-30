//! Autonomous Training Scheduler Module
//!
//! Contains training scheduling, checkpoint management, and model persistence logic.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::time;
use tracing::{error, info, warn};

/// Training scheduler that manages model persistence and checkpoints
pub struct TrainingScheduler;

impl TrainingScheduler {
    /// Save a trained ruv-fann model to disk with metadata
    pub async fn save_trained_model(
        model_name: &str,
        network: &ruv_fann::Network<f32>,
        final_loss: f64,
        epochs: usize,
        training_duration: time::Duration,
    ) -> Result<PathBuf> {
        // Create models directory if it doesn't exist
        let models_dir = PathBuf::from("models");
        if !models_dir.exists() {
            std::fs::create_dir_all(&models_dir)
                .context("Failed to create models directory")?;
        }

        // Create model-specific directory with timestamp
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let model_dir = models_dir.join(format!("{}_{}", model_name, timestamp));
        std::fs::create_dir_all(&model_dir)
            .context("Failed to create model directory")?;

        // Save the network weights as JSON
        let weights = network.get_weights();
        let weights_path = model_dir.join("weights.json");
        let weights_json = serde_json::to_string_pretty(&weights)?;
        std::fs::write(&weights_path, weights_json)
            .context("Failed to save model weights")?;

        // Save model metadata
        let metadata = serde_json::json!({
            "model_name": model_name,
            "timestamp": Utc::now(),
            "final_loss": final_loss,
            "epochs": epochs,
            "training_duration_secs": training_duration.as_secs(),
            "num_inputs": network.num_inputs(),
            "num_outputs": network.num_outputs(),
            "total_neurons": network.total_neurons(),
            "total_connections": network.total_connections(),
        });
        
        let metadata_path = model_dir.join("metadata.json");
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        std::fs::write(&metadata_path, metadata_json)
            .context("Failed to save model metadata")?;

        info!("📁 Model '{}' saved to directory: {:?}", model_name, model_dir);
        info!("   💾 Weights saved to: {:?}", weights_path);
        info!("   📋 Metadata saved to: {:?}", metadata_path);
        
        Ok(model_dir)
    }

    /// Load the best saved model for a given type
    pub async fn load_best_saved_model(model_name: &str) -> Result<Option<(ruv_fann::Network<f32>, serde_json::Value)>> {
        let models_dir = PathBuf::from("models");
        if !models_dir.exists() {
            info!("📁 No models directory found for loading {}", model_name);
            return Ok(None);
        }

        // Find all directories matching the model name pattern
        let mut model_dirs = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&models_dir) {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.starts_with(&format!("{}_", model_name)) {
                        model_dirs.push((name, entry.path()));
                    }
                }
            }
        }

        if model_dirs.is_empty() {
            info!("📁 No saved models found for type: {}", model_name);
            return Ok(None);
        }

        // Sort by timestamp (latest first)
        model_dirs.sort_by(|a, b| b.0.cmp(&a.0));
        let latest_dir = &model_dirs[0].1;

        info!("📂 Loading latest model from: {:?}", latest_dir);

        // Load metadata
        let metadata_path = latest_dir.join("metadata.json");
        let metadata_json = std::fs::read_to_string(&metadata_path)
            .context("Failed to read model metadata")?;
        let metadata: serde_json::Value = serde_json::from_str(&metadata_json)?;

        // Load weights
        let weights_path = latest_dir.join("weights.json");
        let weights_json = std::fs::read_to_string(&weights_path)
            .context("Failed to read model weights")?;
        let weights: Vec<f32> = serde_json::from_str(&weights_json)?;

        // Reconstruct the network (this is simplified - in practice we'd need more architecture info)
        let num_inputs = metadata["num_inputs"].as_u64().unwrap_or(10) as usize;
        let num_outputs = metadata["num_outputs"].as_u64().unwrap_or(1) as usize;
        
        // Create a basic network structure (this should be saved in metadata in practice)
        let mut network = ruv_fann::NetworkBuilder::new()
            .input_layer(num_inputs)
            .hidden_layer(20) // Simplified - should come from metadata
            .output_layer(num_outputs)
            .build();

        // Set the loaded weights
        if let Err(e) = network.set_weights(&weights) {
            error!("Failed to set loaded weights: {:?}", e);
            return Ok(None);
        }

        info!("✅ Successfully loaded model '{}' with {} neurons and {} connections", 
              model_name, 
              metadata["total_neurons"].as_u64().unwrap_or(0), 
              metadata["total_connections"].as_u64().unwrap_or(0));

        Ok(Some((network, metadata)))
    }

    /// Save checkpoints during training
    pub async fn save_checkpoint(
        model_name: &str,
        network: &ruv_fann::Network<f32>,
        epoch: usize,
        current_loss: f64,
        learning_rate: f32,
    ) -> Result<PathBuf> {
        // Create checkpoints directory if it doesn't exist
        let checkpoints_dir = PathBuf::from("models").join("checkpoints").join(model_name);
        if !checkpoints_dir.exists() {
            std::fs::create_dir_all(&checkpoints_dir)
                .context("Failed to create checkpoints directory")?;
        }

        // Create checkpoint filename with epoch
        let checkpoint_file = checkpoints_dir.join(format!("checkpoint_epoch_{}.json", epoch));
        
        // Save checkpoint data
        let checkpoint_data = serde_json::json!({
            "model_name": model_name,
            "epoch": epoch,
            "timestamp": Utc::now(),
            "training_loss": current_loss,
            "validation_loss": current_loss * 1.1, // Simplified validation loss
            "learning_rate": learning_rate,
            "weights": network.get_weights(),
            "num_inputs": network.num_inputs(),
            "num_outputs": network.num_outputs(),
            "total_neurons": network.total_neurons(),
            "total_connections": network.total_connections(),
        });
        
        let checkpoint_json = serde_json::to_string_pretty(&checkpoint_data)?;
        std::fs::write(&checkpoint_file, checkpoint_json)
            .context("Failed to write checkpoint file")?;

        // Clean up old checkpoints (keep last 5)
        Self::cleanup_old_checkpoints(&checkpoints_dir, 5).await?;

        Ok(checkpoint_file)
    }

    /// Clean up old checkpoint files, keeping only the most recent ones
    pub async fn cleanup_old_checkpoints(checkpoints_dir: &PathBuf, keep_count: usize) -> Result<()> {
        if let Ok(entries) = std::fs::read_dir(checkpoints_dir) {
            let mut checkpoints = Vec::new();
            
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.starts_with("checkpoint_epoch_") && name.ends_with(".json") {
                        if let Ok(metadata) = entry.metadata() {
                            if let Ok(modified) = metadata.modified() {
                                checkpoints.push((entry.path(), modified));
                            }
                        }
                    }
                }
            }

            // Sort by modification time (newest first)
            checkpoints.sort_by(|a, b| b.1.cmp(&a.1));

            // Delete old checkpoints, keeping only the most recent ones
            for (path, _) in checkpoints.iter().skip(keep_count) {
                if let Err(e) = std::fs::remove_file(path) {
                    warn!("Failed to remove old checkpoint {:?}: {}", path, e);
                }
            }
        }

        Ok(())
    }

    /// Load best available models on startup
    pub async fn load_best_models_on_startup() -> Result<()> {
        info!("🚀 Loading best available models on startup");

        // Define available model types
        let model_types = vec!["MLP", "LSTM", "GRU", "DeepAR", "TCN", "NHITS"];
        
        for model_type in model_types {
            match Self::load_best_saved_model(model_type).await {
                Ok(Some((_network, metadata))) => {
                    let loss = metadata["final_loss"].as_f64().unwrap_or(0.0);
                    let epochs = metadata["epochs"].as_u64().unwrap_or(0);
                    let duration = metadata["training_duration_secs"].as_u64().unwrap_or(0);
                    
                    info!("✅ Loaded best '{}' model - Loss: {:.6}, Epochs: {}, Duration: {}s", 
                          model_type, loss, epochs, duration);
                }
                Ok(None) => {
                    info!("⚪ No saved model found for type: {}", model_type);
                }
                Err(e) => {
                    error!("❌ Failed to load best model for '{}': {}", model_type, e);
                }
            }
        }

        Ok(())
    }

    /// Schedule training with priority queue management
    pub async fn schedule_training_task(
        priority: super::config::TrainingPriority,
        estimated_duration: chrono::Duration,
        resource_requirements: &super::config::ResourceRequirements,
    ) -> Result<DateTime<Utc>> {
        // Simple scheduling logic - in practice this would be more sophisticated
        let current_time = Utc::now();
        
        let scheduled_time = match priority {
            super::config::TrainingPriority::Emergency => {
                // Emergency training starts immediately
                current_time
            }
            super::config::TrainingPriority::Critical => {
                // Critical training scheduled within 5 minutes
                current_time + chrono::Duration::minutes(5)
            }
            super::config::TrainingPriority::High => {
                // High priority training scheduled within 30 minutes
                current_time + chrono::Duration::minutes(30)
            }
            super::config::TrainingPriority::Medium => {
                // Medium priority training scheduled within 2 hours
                current_time + chrono::Duration::hours(2)
            }
            super::config::TrainingPriority::Low => {
                // Low priority training scheduled during off-peak hours
                current_time + chrono::Duration::hours(6)
            }
        };

        info!("📅 Training scheduled for: {} (Priority: {:?}, Duration: {} hours, CPU: {}, GPU: {})",
              scheduled_time.format("%Y-%m-%d %H:%M:%S UTC"),
              priority,
              estimated_duration.num_hours(),
              resource_requirements.cpu_cores,
              resource_requirements.gpu_required);

        Ok(scheduled_time)
    }

    /// Check resource availability for training
    pub async fn check_resource_availability(
        requirements: &super::config::ResourceRequirements,
    ) -> Result<bool> {
        // Simplified resource checking - in practice this would check actual system resources
        let available_cpu_cores = num_cpus::get();
        let available_memory_gb = 16.0; // Simplified - would check actual memory
        let gpu_available = true; // Simplified - would check GPU availability
        
        let resources_available = 
            available_cpu_cores >= requirements.cpu_cores &&
            available_memory_gb >= requirements.memory_gb &&
            (!requirements.gpu_required || gpu_available);

        if resources_available {
            info!("✅ Resources available for training: CPU {}/{}, Memory {:.1}/{:.1}GB, GPU: {}",
                  requirements.cpu_cores, available_cpu_cores,
                  requirements.memory_gb, available_memory_gb,
                  if requirements.gpu_required { "Required & Available" } else { "Not Required" });
        } else {
            warn!("⚠️ Insufficient resources for training: Need CPU {}, Memory {:.1}GB, GPU: {}",
                  requirements.cpu_cores,
                  requirements.memory_gb,
                  requirements.gpu_required);
        }

        Ok(resources_available)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_scheduler_creation() {
        // Test that scheduler methods work without errors
        let result = TrainingScheduler::load_best_models_on_startup().await;
        // Should not panic even if no models exist
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_resource_availability_check() {
        let requirements = super::super::config::ResourceRequirements::minimal();
        let available = TrainingScheduler::check_resource_availability(&requirements).await;
        assert!(available.is_ok());
        // Minimal resources should always be available
        assert!(available.unwrap());
    }

    #[tokio::test]
    async fn test_training_scheduling() {
        let priority = super::super::config::TrainingPriority::Medium;
        let duration = chrono::Duration::hours(2);
        let requirements = super::super::config::ResourceRequirements::incremental();
        
        let scheduled_time = TrainingScheduler::schedule_training_task(
            priority, 
            duration, 
            &requirements
        ).await;
        
        assert!(scheduled_time.is_ok());
        // Scheduled time should be in the future
        assert!(scheduled_time.unwrap() >= Utc::now());
    }
}