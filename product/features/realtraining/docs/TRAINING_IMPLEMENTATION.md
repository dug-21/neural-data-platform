# Real Model Training Implementation Plan

## Executive Summary

This document outlines the comprehensive implementation plan to replace all mock training functions with real neural network training capabilities across the Neural Trader codebase. The current implementation simulates training without actually updating model weights or learning from data, which severely limits the system's predictive capabilities.

## Current State Analysis

### Mock Training Locations

1. **autonomous_training.rs** (Lines 746-796)
   - `execute_emergency_training()` - Simulates with sleep
   - `execute_full_retraining()` - Simulates with sleep
   - `execute_incremental_training()` - Simulates with sleep
   - `execute_fine_tuning()` - Simulates with sleep

2. **fann_predictor.rs** (Lines 721-764)
   - `train_model()` - Creates network but doesn't train
   - Networks initialized but weights never updated
   - Training data prepared but not used

3. **enhanced_predictor.rs**
   - No training implementation
   - Only tracks when retraining is needed
   - Delegates to FannPredictor which doesn't train

## Implementation Strategy

### Phase 1: FANN Model Training Implementation

#### 1.1 Modify fann_predictor.rs

```rust
// Replace current train_model method (line 721)
async fn train_model(
    &self,
    model_name: &str,
    data: &[TimeSeriesData],
) -> Result<()> {
    self.ensure_model(&model_name).await?;
    
    let config = self.model_configs.get(model_name)
        .ok_or_else(|| anyhow::anyhow!("Unknown model: {}", model_name))?;
    
    // Prepare training data
    let training_data = match model_name {
        "LSTM" | "GRU" => self.prepare_recurrent_training_data(model_name, data, config).await?,
        "Transformer" => self.prepare_attention_training_data(model_name, data, config).await?,
        _ => self.prepare_training_data(data, config)?
    };
    
    // Get the network for training
    let mut networks = self.networks.write().await;
    let network = networks.get_mut(model_name)
        .ok_or_else(|| anyhow::anyhow!("Model not initialized: {}", model_name))?;
    
    info!("🎯 Starting real FANN training for '{}' with {} samples", 
          model_name, training_data.inputs.len());
    
    // REAL TRAINING IMPLEMENTATION
    let training_algorithm = match model_name {
        "DeepAR" | "LSTM" | "GRU" => TrainingAlgorithm::Rprop,
        "Transformer" | "NHITS" => TrainingAlgorithm::QuickProp,
        _ => TrainingAlgorithm::IncreamentalBatch,
    };
    
    // Set training parameters
    network.set_learning_rate(config.learning_rate);
    network.set_learning_momentum(config.momentum);
    network.set_training_algorithm(training_algorithm);
    
    // Create FANN training data structure
    let mut fann_train_data = TrainingData::new(
        training_data.inputs.len(),
        config.input_size,
        config.output_size
    );
    
    for (i, (input, output)) in training_data.inputs.iter()
        .zip(training_data.outputs.iter())
        .enumerate() 
    {
        fann_train_data.set_input(i, input);
        fann_train_data.set_output(i, output);
    }
    
    // Perform actual training
    let start_error = network.test_data(&fann_train_data);
    info!("Initial MSE: {:.6} for model '{}'", start_error, model_name);
    
    // Train with early stopping
    let mut best_error = f32::MAX;
    let mut epochs_without_improvement = 0;
    let patience = 50;
    
    for epoch in 0..config.max_epochs {
        // Train for one epoch
        network.train_on_data(&fann_train_data, 1, 0, 0.0);
        
        // Calculate current error
        let current_error = network.get_MSE();
        
        if current_error < best_error {
            best_error = current_error;
            epochs_without_improvement = 0;
        } else {
            epochs_without_improvement += 1;
        }
        
        // Log progress every 100 epochs
        if epoch % 100 == 0 {
            debug!("Epoch {}: MSE = {:.6}, Best = {:.6}", 
                   epoch, current_error, best_error);
        }
        
        // Early stopping
        if current_error <= config.target_error || 
           epochs_without_improvement >= patience {
            info!("✅ Training completed for '{}' at epoch {} with MSE: {:.6}", 
                  model_name, epoch, current_error);
            break;
        }
    }
    
    // Cache the training data for online learning
    self.training_cache.write().await.insert(
        model_name.to_string(), 
        training_data
    );
    
    info!("🏁 Real training completed for FANN model '{}'", model_name);
    Ok(())
}
```

#### 1.2 Add Online Learning Support

```rust
// Update update_with_new_data method in fann_predictor.rs
pub async fn update_with_new_data(
    &self,
    model_name: &str,
    new_data: &[TimeSeriesData],
) -> Result<()> {
    let mut training_cache = self.training_cache.write().await;
    
    if let Some(cached_data) = training_cache.get_mut(model_name) {
        let config = self.model_configs.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Unknown model: {}", model_name))?;
        
        // Prepare new training samples
        let new_training_data = match model_name {
            "LSTM" | "GRU" => self.prepare_recurrent_training_data(model_name, new_data, config).await?,
            _ => self.prepare_training_data(new_data, config)?
        };
        
        // Append to existing training data (with sliding window)
        let max_samples = 50000; // Keep last 50k samples
        cached_data.inputs.extend(new_training_data.inputs);
        cached_data.outputs.extend(new_training_data.outputs);
        
        // Maintain sliding window
        if cached_data.inputs.len() > max_samples {
            let remove_count = cached_data.inputs.len() - max_samples;
            cached_data.inputs.drain(0..remove_count);
            cached_data.outputs.drain(0..remove_count);
        }
        
        // Perform online learning
        let mut networks = self.networks.write().await;
        if let Some(network) = networks.get_mut(model_name) {
            info!("🔄 Performing online learning for '{}' with {} total samples", 
                  model_name, cached_data.inputs.len());
            
            // Create mini-batch from recent data
            let batch_size = 32.min(new_training_data.inputs.len());
            let start_idx = new_training_data.inputs.len().saturating_sub(batch_size);
            
            let mut mini_batch = TrainingData::new(
                batch_size,
                config.input_size,
                config.output_size
            );
            
            for (i, idx) in (start_idx..new_training_data.inputs.len()).enumerate() {
                mini_batch.set_input(i, &new_training_data.inputs[idx]);
                mini_batch.set_output(i, &new_training_data.outputs[idx]);
            }
            
            // Train on mini-batch with reduced learning rate
            let original_lr = network.get_learning_rate();
            network.set_learning_rate(original_lr * 0.1); // Reduce LR for online learning
            
            network.train_on_data(&mini_batch, 10, 0, 0.0); // 10 epochs on mini-batch
            
            network.set_learning_rate(original_lr); // Restore original LR
            
            info!("✅ Online learning completed for '{}', new MSE: {:.6}", 
                  model_name, network.get_MSE());
        }
    } else {
        // No cached data, perform full training
        warn!("No cached training data for '{}', performing full training", model_name);
        self.train_model(model_name, new_data).await?;
    }
    
    Ok(())
}
```

### Phase 2: Autonomous Training Integration

#### 2.1 Replace Mock Training in autonomous_training.rs

```rust
// Replace execute_emergency_training (line 746)
async fn execute_emergency_training(&self, decision: &TrainingDecision) -> Result<TrainingOutcome> {
    info!("🚨 Executing emergency training with high priority");
    
    if let Some(neural_client) = &self.neural_client {
        let start_time = Utc::now();
        
        // Get recent market data for training
        let training_data = self.fetch_recent_market_data(1000).await?; // Last 1000 samples
        
        // Train affected models with aggressive parameters
        for model_name in &decision.affected_models {
            info!("Emergency training model: {}", model_name);
            
            // Use enhanced predictor for coordinated training
            if let Ok(fann_predictor) = neural_client.get_fann_predictor() {
                // Backup current model state
                self.backup_model_state(model_name).await?;
                
                // Aggressive training for emergency
                match fann_predictor.train_model(model_name, &training_data).await {
                    Ok(_) => {
                        info!("✅ Emergency training successful for {}", model_name);
                    }
                    Err(e) => {
                        error!("❌ Emergency training failed for {}: {}", model_name, e);
                        // Restore from backup
                        self.restore_model_state(model_name).await?;
                        return Ok(TrainingOutcome::Failure {
                            error_message: format!("Training failed: {}", e),
                            retry_recommended: true,
                        });
                    }
                }
            }
        }
        
        // Validate improved performance
        let new_performance = self.validate_model_performance(&training_data).await?;
        let improvement = ((new_performance - decision.performance_snapshot.accuracy) / 
                          decision.performance_snapshot.accuracy) * 100.0;
        
        let training_duration = Utc::now() - start_time;
        info!("⏱️ Emergency training completed in {} seconds", training_duration.num_seconds());
        
        Ok(TrainingOutcome::Success {
            improvement_percentage: improvement,
            new_accuracy: new_performance,
        })
    } else {
        Err(anyhow::anyhow!("Neural client not initialized"))
    }
}

// Replace execute_full_retraining (line 760)
async fn execute_full_retraining(&self, decision: &TrainingDecision) -> Result<TrainingOutcome> {
    info!("🔄 Executing full model retraining");
    
    if let Some(neural_client) = &self.neural_client {
        let start_time = Utc::now();
        
        // Get comprehensive training data
        let training_data = self.fetch_historical_market_data(50000).await?; // 50k samples
        
        // Reset and retrain all models
        for model_name in &decision.affected_models {
            info!("Full retraining for model: {}", model_name);
            
            if let Ok(fann_predictor) = neural_client.get_fann_predictor() {
                // Reset model to initial state
                self.reset_model_weights(model_name).await?;
                
                // Full training from scratch
                match fann_predictor.train_model(model_name, &training_data).await {
                    Ok(_) => {
                        // Validate on holdout set
                        let validation_data = self.fetch_validation_data(5000).await?;
                        let validation_score = self.validate_model(&fann_predictor, 
                                                                  model_name, 
                                                                  &validation_data).await?;
                        info!("✅ Full retraining complete for {}, validation score: {:.3}", 
                              model_name, validation_score);
                    }
                    Err(e) => {
                        error!("❌ Full retraining failed for {}: {}", model_name, e);
                        return Ok(TrainingOutcome::Failure {
                            error_message: format!("Retraining failed: {}", e),
                            retry_recommended: true,
                        });
                    }
                }
            }
        }
        
        // Calculate overall improvement
        let new_performance = self.validate_model_performance(&training_data).await?;
        let improvement = ((new_performance - decision.performance_snapshot.accuracy) / 
                          decision.performance_snapshot.accuracy) * 100.0;
        
        let training_duration = Utc::now() - start_time;
        info!("⏱️ Full retraining completed in {} minutes", training_duration.num_minutes());
        
        Ok(TrainingOutcome::Success {
            improvement_percentage: improvement,
            new_accuracy: new_performance,
        })
    } else {
        Err(anyhow::anyhow!("Neural client not initialized"))
    }
}

// Replace execute_incremental_training (line 773)
async fn execute_incremental_training(&self, decision: &TrainingDecision) -> Result<TrainingOutcome> {
    info!("📈 Executing incremental training");
    
    if let Some(neural_client) = &self.neural_client {
        let start_time = Utc::now();
        
        // Get recent data for incremental update
        let new_data = self.fetch_recent_market_data(5000).await?; // Last 5k samples
        
        for model_name in &decision.affected_models {
            info!("Incremental training for model: {}", model_name);
            
            if let Ok(fann_predictor) = neural_client.get_fann_predictor() {
                // Use online learning for incremental updates
                match fann_predictor.update_with_new_data(model_name, &new_data).await {
                    Ok(_) => {
                        info!("✅ Incremental training successful for {}", model_name);
                    }
                    Err(e) => {
                        error!("❌ Incremental training failed for {}: {}", model_name, e);
                        // Continue with other models
                    }
                }
            }
        }
        
        // Validate performance improvement
        let new_performance = self.validate_model_performance(&new_data).await?;
        let improvement = ((new_performance - decision.performance_snapshot.accuracy) / 
                          decision.performance_snapshot.accuracy) * 100.0;
        
        let training_duration = Utc::now() - start_time;
        info!("⏱️ Incremental training completed in {} seconds", training_duration.num_seconds());
        
        Ok(TrainingOutcome::Success {
            improvement_percentage: improvement,
            new_accuracy: new_performance,
        })
    } else {
        Err(anyhow::anyhow!("Neural client not initialized"))
    }
}

// Replace execute_fine_tuning (line 786)
async fn execute_fine_tuning(&self, decision: &TrainingDecision) -> Result<TrainingOutcome> {
    info!("🎯 Executing model fine-tuning for regime: {}", 
          decision.decision_type.target_regime());
    
    if let Some(neural_client) = &self.neural_client {
        let start_time = Utc::now();
        
        // Get regime-specific training data
        let regime_data = self.fetch_regime_specific_data(
            &decision.decision_type.target_regime(), 
            3000
        ).await?;
        
        for model_name in &decision.affected_models {
            info!("Fine-tuning model: {} for {} regime", 
                  model_name, decision.decision_type.target_regime());
            
            if let Ok(fann_predictor) = neural_client.get_fann_predictor() {
                // Reduce learning rate for fine-tuning
                let original_config = fann_predictor.get_model_configs()
                    .get(model_name)
                    .cloned()
                    .unwrap_or_default();
                
                // Create fine-tuning config with reduced learning rate
                let mut fine_tune_config = original_config.clone();
                fine_tune_config.learning_rate *= 0.1;
                fine_tune_config.max_epochs = 500; // Fewer epochs for fine-tuning
                
                // Temporarily update config
                fann_predictor.model_configs.insert(
                    model_name.clone(), 
                    fine_tune_config
                );
                
                // Perform fine-tuning
                match fann_predictor.train_model(model_name, &regime_data).await {
                    Ok(_) => {
                        info!("✅ Fine-tuning successful for {}", model_name);
                    }
                    Err(e) => {
                        error!("❌ Fine-tuning failed for {}: {}", model_name, e);
                    }
                }
                
                // Restore original config
                fann_predictor.model_configs.insert(
                    model_name.clone(), 
                    original_config
                );
            }
        }
        
        // Validate on regime-specific data
        let new_performance = self.validate_model_performance(&regime_data).await?;
        let improvement = ((new_performance - decision.performance_snapshot.accuracy) / 
                          decision.performance_snapshot.accuracy) * 100.0;
        
        let training_duration = Utc::now() - start_time;
        info!("⏱️ Fine-tuning completed in {} seconds", training_duration.num_seconds());
        
        Ok(TrainingOutcome::Success {
            improvement_percentage: improvement,
            new_accuracy: new_performance,
        })
    } else {
        Err(anyhow::anyhow!("Neural client not initialized"))
    }
}
```

### Phase 3: Training Metrics and Monitoring

#### 3.1 Create training_metrics.rs

```rust
// New file: src/monitoring/training_metrics.rs

use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TrainingMetrics {
    pub model_name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub initial_error: f32,
    pub final_error: Option<f32>,
    pub epochs_completed: usize,
    pub samples_processed: usize,
    pub learning_rate: f32,
    pub training_type: TrainingType,
    pub convergence_history: Vec<(usize, f32)>, // (epoch, error)
    pub validation_scores: HashMap<String, f64>,
    pub resource_usage: ResourceMetrics,
}

#[derive(Debug, Clone)]
pub enum TrainingType {
    Full,
    Incremental,
    FineTuning,
    Emergency,
}

#[derive(Debug, Clone)]
pub struct ResourceMetrics {
    pub cpu_usage_percent: f32,
    pub memory_mb: f32,
    pub training_time_seconds: i64,
}

pub struct TrainingMonitor {
    active_trainings: Arc<RwLock<HashMap<String, TrainingMetrics>>>,
    completed_trainings: Arc<RwLock<Vec<TrainingMetrics>>>,
    max_history_size: usize,
}

impl TrainingMonitor {
    pub fn new() -> Self {
        Self {
            active_trainings: Arc::new(RwLock::new(HashMap::new())),
            completed_trainings: Arc::new(RwLock::new(Vec::new())),
            max_history_size: 1000,
        }
    }
    
    pub async fn start_training(
        &self,
        model_name: &str,
        training_type: TrainingType,
        initial_error: f32,
        learning_rate: f32,
        samples: usize,
    ) -> String {
        let training_id = format!("{}_{}", model_name, Utc::now().timestamp());
        
        let metrics = TrainingMetrics {
            model_name: model_name.to_string(),
            start_time: Utc::now(),
            end_time: None,
            initial_error,
            final_error: None,
            epochs_completed: 0,
            samples_processed: samples,
            learning_rate,
            training_type,
            convergence_history: vec![(0, initial_error)],
            validation_scores: HashMap::new(),
            resource_usage: ResourceMetrics {
                cpu_usage_percent: 0.0,
                memory_mb: 0.0,
                training_time_seconds: 0,
            },
        };
        
        self.active_trainings.write().await.insert(training_id.clone(), metrics);
        training_id
    }
    
    pub async fn update_progress(
        &self,
        training_id: &str,
        epoch: usize,
        current_error: f32,
    ) {
        if let Some(metrics) = self.active_trainings.write().await.get_mut(training_id) {
            metrics.epochs_completed = epoch;
            metrics.convergence_history.push((epoch, current_error));
        }
    }
    
    pub async fn complete_training(
        &self,
        training_id: &str,
        final_error: f32,
        validation_scores: HashMap<String, f64>,
    ) {
        if let Some(mut metrics) = self.active_trainings.write().await.remove(training_id) {
            metrics.end_time = Some(Utc::now());
            metrics.final_error = Some(final_error);
            metrics.validation_scores = validation_scores;
            
            // Calculate resource usage
            if let Some(end_time) = metrics.end_time {
                metrics.resource_usage.training_time_seconds = 
                    (end_time - metrics.start_time).num_seconds();
            }
            
            // Add to completed history
            let mut completed = self.completed_trainings.write().await;
            completed.push(metrics);
            
            // Maintain history size
            if completed.len() > self.max_history_size {
                completed.drain(0..completed.len() - self.max_history_size);
            }
        }
    }
    
    pub async fn get_training_summary(&self) -> HashMap<String, serde_json::Value> {
        let active = self.active_trainings.read().await;
        let completed = self.completed_trainings.read().await;
        
        let mut summary = HashMap::new();
        summary.insert("active_trainings".to_string(), 
                      serde_json::json!(active.len()));
        summary.insert("completed_trainings".to_string(), 
                      serde_json::json!(completed.len()));
        
        // Calculate average training metrics
        if !completed.is_empty() {
            let avg_improvement = completed.iter()
                .filter_map(|m| {
                    m.final_error.map(|f| {
                        ((m.initial_error - f) / m.initial_error) * 100.0
                    })
                })
                .sum::<f32>() / completed.len() as f32;
            
            let avg_training_time = completed.iter()
                .map(|m| m.resource_usage.training_time_seconds)
                .sum::<i64>() / completed.len() as i64;
            
            summary.insert("avg_improvement_percent".to_string(), 
                          serde_json::json!(avg_improvement));
            summary.insert("avg_training_seconds".to_string(), 
                          serde_json::json!(avg_training_time));
        }
        
        summary
    }
}
```

### Phase 4: Enhanced Neural Adapter Training

#### 4.1 Add Training Support to Enhanced Adapter

```rust
// Add to src/adapters/neural/neuro_divergent_adapter.rs

impl EnhancedNeuralAdapter {
    pub async fn train_model(
        &mut self,
        model_type: &str,
        training_data: &[TimeSeriesData],
        config: &TrainingConfig,
    ) -> Result<TrainingResult> {
        if !self.is_connected() {
            self.connect().await?;
        }
        
        info!("Training enhanced model: {}", model_type);
        
        match model_type {
            "TimeMixer" => self.train_timemixer(training_data, config).await,
            "NeuralForecast" => self.train_neural_forecast(training_data, config).await,
            "TimesFM" => self.train_timesfm(training_data, config).await,
            "DeepAR" => self.train_deepar(training_data, config).await,
            "NHITS" => self.train_nhits(training_data, config).await,
            "TCN" => self.train_tcn(training_data, config).await,
            _ => Err(anyhow::anyhow!("Unsupported model type: {}", model_type)),
        }
    }
    
    async fn train_timemixer(
        &mut self,
        data: &[TimeSeriesData],
        config: &TrainingConfig,
    ) -> Result<TrainingResult> {
        // Prepare data for TimeMixer
        let df = self.prepare_dataframe(data)?;
        
        // Configure TimeMixer training
        let train_config = json!({
            "batch_size": config.batch_size,
            "learning_rate": config.learning_rate,
            "epochs": config.epochs,
            "early_stopping": true,
            "patience": 10,
            "lookback_window": self.config.lookback_window,
            "forecast_horizon": self.config.forecast_horizon,
        });
        
        // Execute training through Python bridge
        let result = self.python_runtime
            .call_method(
                "train_timemixer",
                &df,
                &train_config,
            )
            .await?;
        
        Ok(TrainingResult {
            model_type: "TimeMixer".to_string(),
            final_loss: result["final_loss"].as_f64().unwrap_or(0.0),
            training_time: result["training_time"].as_i64().unwrap_or(0),
            epochs_completed: result["epochs"].as_u64().unwrap_or(0) as usize,
            validation_score: result["validation_score"].as_f64().unwrap_or(0.0),
        })
    }
}

#[derive(Debug, Clone)]
pub struct TrainingConfig {
    pub batch_size: usize,
    pub learning_rate: f64,
    pub epochs: usize,
    pub validation_split: f64,
    pub early_stopping: bool,
    pub optimizer: String,
}

#[derive(Debug)]
pub struct TrainingResult {
    pub model_type: String,
    pub final_loss: f64,
    pub training_time: i64,
    pub epochs_completed: usize,
    pub validation_score: f64,
}
```

### Phase 5: Integration Workflow

#### 5.1 Training Coordinator

```rust
// New file: src/neural/training_coordinator.rs

use crate::neural::{FannPredictor, EnhancedNeuralPredictor};
use crate::daa::autonomous_training::{AutonomousTrainingEngine, TrainingDecision};
use crate::monitoring::training_metrics::TrainingMonitor;

pub struct TrainingCoordinator {
    fann_predictor: Arc<FannPredictor>,
    enhanced_predictor: Arc<EnhancedNeuralPredictor>,
    training_engine: Arc<AutonomousTrainingEngine>,
    training_monitor: Arc<TrainingMonitor>,
}

impl TrainingCoordinator {
    pub async fn execute_training_decision(
        &self,
        decision: &TrainingDecision,
    ) -> Result<()> {
        info!("Executing training decision: {:?}", decision.decision_type);
        
        // Start monitoring
        let training_ids: Vec<String> = Vec::new();
        
        for model in &decision.affected_models {
            let training_id = self.training_monitor.start_training(
                model,
                self.map_training_type(&decision.decision_type),
                0.0, // Will be updated
                self.get_learning_rate(model),
                decision.performance_snapshot.trading_volume as usize,
            ).await;
            
            training_ids.push(training_id);
        }
        
        // Execute training based on decision type
        let outcome = match &decision.decision_type {
            TrainingDecisionType::Emergency { .. } => {
                self.execute_emergency_training(decision, &training_ids).await?
            }
            TrainingDecisionType::FullRetraining { .. } => {
                self.execute_full_retraining(decision, &training_ids).await?
            }
            TrainingDecisionType::IncrementalTraining { .. } => {
                self.execute_incremental_training(decision, &training_ids).await?
            }
            TrainingDecisionType::FineTuning { .. } => {
                self.execute_fine_tuning(decision, &training_ids).await?
            }
            TrainingDecisionType::NoTraining { .. } => {
                return Ok(());
            }
        };
        
        // Update training engine with outcome
        self.training_engine
            .mark_training_completed(&decision.decision_id, outcome)
            .await?;
        
        Ok(())
    }
}
```

## Testing Strategy

### Unit Tests

1. **FANN Training Tests**
   ```rust
   #[tokio::test]
   async fn test_real_fann_training() {
       let predictor = FannPredictor::new(config).unwrap();
       let data = generate_test_data(1000);
       
       // Train model
       predictor.train_model("MLP", &data).await.unwrap();
       
       // Verify weights changed
       let predictions_before = predictor.predict(&data[..100], 5).await.unwrap();
       predictor.train_model("MLP", &data).await.unwrap();
       let predictions_after = predictor.predict(&data[..100], 5).await.unwrap();
       
       // Predictions should differ after training
       assert_ne!(predictions_before[0].value, predictions_after[0].value);
   }
   ```

2. **Online Learning Tests**
   ```rust
   #[tokio::test]
   async fn test_online_learning() {
       let predictor = FannPredictor::new(config).unwrap();
       let initial_data = generate_test_data(500);
       let new_data = generate_test_data(100);
       
       // Initial training
       predictor.train_model("LSTM", &initial_data).await.unwrap();
       
       // Online update
       predictor.update_with_new_data("LSTM", &new_data).await.unwrap();
       
       // Verify model adapted
       let predictions = predictor.predict(&new_data, 5).await.unwrap();
       assert!(!predictions.is_empty());
   }
   ```

### Integration Tests

1. **Autonomous Training Integration**
   ```rust
   #[tokio::test]
   async fn test_autonomous_training_integration() {
       let engine = AutonomousTrainingEngine::new(config).unwrap();
       let predictor = Arc::new(EnhancedNeuralPredictor::new(neural_config).unwrap());
       
       // Simulate poor performance
       let poor_snapshot = PerformanceSnapshot {
           accuracy: 0.5,
           // ... other fields
       };
       
       let decision = engine.evaluate_training_need(poor_snapshot).await.unwrap();
       assert!(matches!(decision.decision_type, TrainingDecisionType::Emergency { .. }));
       
       // Execute training
       let integration = DAATrainingIntegration::new(engine, receiver)
           .with_neural_client(predictor);
       
       integration.process_training_decision(decision).await.unwrap();
   }
   ```

### Performance Tests

1. **Training Speed Benchmarks**
   ```rust
   #[bench]
   fn bench_fann_training(b: &mut Bencher) {
       let runtime = tokio::runtime::Runtime::new().unwrap();
       let predictor = FannPredictor::new(config).unwrap();
       let data = generate_test_data(10000);
       
       b.iter(|| {
           runtime.block_on(async {
               predictor.train_model("MLP", &data).await.unwrap();
           });
       });
   }
   ```

## Migration Plan

### Phase 1: FANN Implementation (Week 1)
- Replace mock training in fann_predictor.rs
- Add real FANN training with all algorithms
- Implement online learning
- Unit test coverage

### Phase 2: Autonomous Integration (Week 2)
- Replace mock functions in autonomous_training.rs
- Add data fetching utilities
- Implement model backup/restore
- Integration testing

### Phase 3: Monitoring & Metrics (Week 3)
- Create training_metrics.rs
- Add training monitoring
- Performance tracking
- Dashboard integration

### Phase 4: Enhanced Models (Week 4)
- Add training to enhanced adapter
- Python bridge for TimeMixer/TimesFM
- Cross-model validation
- Full system testing

## Success Metrics

1. **Training Effectiveness**
   - MSE reduction > 50% after training
   - Validation accuracy > 80%
   - Convergence within 1000 epochs

2. **Performance**
   - Training time < 5 minutes for 10k samples
   - Online learning < 10 seconds
   - Memory usage < 2GB during training

3. **Reliability**
   - Zero training failures in testing
   - Graceful handling of edge cases
   - Automatic recovery from errors

## Risk Mitigation

1. **Model Corruption**
   - Automatic backup before training
   - Validation after training
   - Rollback capability

2. **Performance Degradation**
   - A/B testing of new models
   - Gradual rollout
   - Performance monitoring

3. **Resource Exhaustion**
   - Training limits
   - Memory management
   - Queue management

## Conclusion

This implementation plan provides a comprehensive approach to replacing all mock training functions with real neural network training. The phased approach ensures minimal disruption while maximizing the benefits of actual model learning and adaptation.

The key improvements include:
- Real FANN network training with multiple algorithms
- Online learning capabilities
- Autonomous training with actual model updates
- Enhanced model training through adapters
- Comprehensive monitoring and metrics
- Robust error handling and recovery

Following this plan will transform the Neural Trader from a system that simulates training to one that actually learns and improves from market data.