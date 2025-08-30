// Training and Optimization Framework for ruv-FANN
// Advanced training strategies, hyperparameter optimization, and performance monitoring

pub mod strategies;
pub mod hyperopt;
pub mod validation;
pub mod incremental;
pub mod parallel;

use crate::models::{FannModel, ModelMetrics, TimeSeriesModel};
use crate::features::{FeaturePipeline, FeatureVector};
use fann::{TrainData, TrainAlgorithm, ActivationFunc};
use std::collections::HashMap;
use rayon::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    pub max_epochs: u32,
    pub desired_error: f32,
    pub epochs_between_reports: u32,
    pub learning_rate: f32,
    pub momentum: f32,
    pub validation_split: f32,
    pub early_stopping: bool,
    pub early_stopping_patience: u32,
    pub batch_size: Option<usize>,
    pub shuffle_data: bool,
    pub cross_validation_folds: Option<u32>,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        TrainingConfig {
            max_epochs: 10000,
            desired_error: 0.001,
            epochs_between_reports: 100,
            learning_rate: 0.7,
            momentum: 0.1,
            validation_split: 0.2,
            early_stopping: true,
            early_stopping_patience: 100,
            batch_size: None,
            shuffle_data: true,
            cross_validation_folds: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrainingResult {
    pub metrics: ModelMetrics,
    pub validation_metrics: ModelMetrics,
    pub training_history: Vec<f32>,
    pub validation_history: Vec<f32>,
    pub convergence_epoch: Option<u32>,
    pub training_time_seconds: f64,
    pub best_epoch: u32,
}

pub trait TrainingStrategy {
    fn train(
        &self,
        model: &mut dyn FannModel,
        train_data: &TrainData,
        validation_data: Option<&TrainData>,
        config: &TrainingConfig,
    ) -> Result<TrainingResult, Box<dyn std::error::Error>>;
}

// Standard FANN training strategy
#[derive(Debug)]
pub struct StandardTraining;

impl TrainingStrategy for StandardTraining {
    fn train(
        &self,
        model: &mut dyn FannModel,
        train_data: &TrainData,
        validation_data: Option<&TrainData>,
        config: &TrainingConfig,
    ) -> Result<TrainingResult, Box<dyn std::error::Error>> {
        let start_time = std::time::Instant::now();
        let mut training_history = Vec::new();
        let mut validation_history = Vec::new();
        let mut best_validation_error = f32::MAX;
        let mut best_epoch = 0;
        let mut patience_counter = 0;
        
        // Train with monitoring
        for epoch in 0..config.max_epochs {
            // Training step
            let train_metrics = model.train(train_data)?;
            training_history.push(train_metrics.mse);
            
            // Validation step
            let mut validation_error = f32::MAX;
            if let Some(val_data) = validation_data {
                validation_error = self.evaluate_model(model, val_data)?;
                validation_history.push(validation_error);
                
                // Early stopping check
                if config.early_stopping {
                    if validation_error < best_validation_error {
                        best_validation_error = validation_error;
                        best_epoch = epoch;
                        patience_counter = 0;
                    } else {
                        patience_counter += 1;
                        if patience_counter >= config.early_stopping_patience {
                            break;
                        }
                    }
                }
            }
            
            // Convergence check
            if train_metrics.mse <= config.desired_error {
                break;
            }
            
            // Reporting
            if epoch % config.epochs_between_reports == 0 {
                println!("Epoch {}: Train MSE = {:.6}, Val MSE = {:.6}", 
                        epoch, train_metrics.mse, validation_error);
            }
        }
        
        let training_time = start_time.elapsed().as_secs_f64();
        
        // Final evaluation
        let final_train_metrics = self.evaluate_model_detailed(model, train_data)?;
        let final_val_metrics = if let Some(val_data) = validation_data {
            self.evaluate_model_detailed(model, val_data)?
        } else {
            final_train_metrics.clone()
        };
        
        Ok(TrainingResult {
            metrics: final_train_metrics,
            validation_metrics: final_val_metrics,
            training_history,
            validation_history,
            convergence_epoch: if training_history.last().unwrap_or(&f32::MAX) <= &config.desired_error {
                Some(training_history.len() as u32)
            } else {
                None
            },
            training_time_seconds: training_time,
            best_epoch,
        })
    }
}

impl StandardTraining {
    fn evaluate_model(&self, model: &dyn FannModel, data: &TrainData) -> Result<f32, Box<dyn std::error::Error>> {
        let mut total_error = 0.0;
        let sample_count = data.get_input_count();
        
        for i in 0..sample_count {
            let input = data.get_input(i)?;
            let expected = data.get_output(i)?;
            let prediction = model.predict(&input)?;
            
            let error: f32 = prediction.iter()
                .zip(&expected)
                .map(|(p, e)| (p - e).powi(2))
                .sum();
            
            total_error += error;
        }
        
        Ok(total_error / sample_count as f32)
    }
    
    fn evaluate_model_detailed(&self, model: &dyn FannModel, data: &TrainData) -> Result<ModelMetrics, Box<dyn std::error::Error>> {
        let mut total_mse = 0.0;
        let mut total_mae = 0.0;
        let mut correct_directions = 0;
        let sample_count = data.get_input_count();
        
        let mut predictions = Vec::new();
        let mut actuals = Vec::new();
        
        for i in 0..sample_count {
            let input = data.get_input(i)?;
            let expected = data.get_output(i)?;
            let prediction = model.predict(&input)?;
            
            // MSE calculation
            let mse: f32 = prediction.iter()
                .zip(&expected)
                .map(|(p, e)| (p - e).powi(2))
                .sum::<f32>() / prediction.len() as f32;
            total_mse += mse;
            
            // MAE calculation
            let mae: f32 = prediction.iter()
                .zip(&expected)
                .map(|(p, e)| (p - e).abs())
                .sum::<f32>() / prediction.len() as f32;
            total_mae += mae;
            
            // Directional accuracy (for time series)
            if prediction.len() > 0 && expected.len() > 0 {
                let pred_direction = prediction[0] > 0.0;
                let actual_direction = expected[0] > 0.0;
                if pred_direction == actual_direction {
                    correct_directions += 1;
                }
            }
            
            predictions.push(prediction[0]);
            actuals.push(expected[0]);
        }
        
        let avg_mse = total_mse / sample_count as f32;
        let avg_mae = total_mae / sample_count as f32;
        let directional_accuracy = correct_directions as f32 / sample_count as f32;
        
        // Calculate financial metrics
        let sharpe_ratio = self.calculate_sharpe_ratio(&predictions, &actuals);
        let max_drawdown = self.calculate_max_drawdown(&predictions);
        
        Ok(ModelMetrics {
            mse: avg_mse,
            mae: avg_mae,
            directional_accuracy,
            sharpe_ratio,
            max_drawdown,
            training_epochs: 0,
            inference_time_ms: 0.0,
        })
    }
    
    fn calculate_sharpe_ratio(&self, predictions: &[f32], actuals: &[f32]) -> f32 {
        if predictions.len() != actuals.len() || predictions.is_empty() {
            return 0.0;
        }
        
        // Calculate returns based on predictions vs actuals
        let returns: Vec<f32> = predictions.iter()
            .zip(actuals)
            .map(|(p, a)| if p * a > 0.0 { a.abs() } else { -a.abs() })
            .collect();
        
        let mean_return = returns.iter().sum::<f32>() / returns.len() as f32;
        let return_variance = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f32>() / returns.len() as f32;
        
        let std_dev = return_variance.sqrt();
        
        if std_dev > 0.0 {
            mean_return / std_dev
        } else {
            0.0
        }
    }
    
    fn calculate_max_drawdown(&self, returns: &[f32]) -> f32 {
        if returns.is_empty() {
            return 0.0;
        }
        
        let mut cumulative = 1.0;
        let mut peak = 1.0;
        let mut max_dd = 0.0;
        
        for &ret in returns {
            cumulative *= 1.0 + ret;
            if cumulative > peak {
                peak = cumulative;
            }
            let drawdown = (peak - cumulative) / peak;
            if drawdown > max_dd {
                max_dd = drawdown;
            }
        }
        
        max_dd
    }
}

// Cascade correlation training
#[derive(Debug)]
pub struct CascadeTraining {
    pub max_neurons: u32,
    pub candidate_change_fraction: f32,
    pub weight_multiplier: f32,
}

impl CascadeTraining {
    pub fn new() -> Self {
        CascadeTraining {
            max_neurons: 100,
            candidate_change_fraction: 0.01,
            weight_multiplier: 0.4,
        }
    }
}

impl TrainingStrategy for CascadeTraining {
    fn train(
        &self,
        model: &mut dyn FannModel,
        train_data: &TrainData,
        validation_data: Option<&TrainData>,
        config: &TrainingConfig,
    ) -> Result<TrainingResult, Box<dyn std::error::Error>> {
        let start_time = std::time::Instant::now();
        
        // Cascade training is handled internally by FANN
        // We just monitor the progress
        let train_metrics = model.train(train_data)?;
        
        let training_time = start_time.elapsed().as_secs_f64();
        
        let validation_metrics = if let Some(val_data) = validation_data {
            let standard = StandardTraining;
            standard.evaluate_model_detailed(model, val_data)?
        } else {
            train_metrics.clone()
        };
        
        Ok(TrainingResult {
            metrics: train_metrics.clone(),
            validation_metrics,
            training_history: vec![train_metrics.mse],
            validation_history: vec![validation_metrics.mse],
            convergence_epoch: Some(1),
            training_time_seconds: training_time,
            best_epoch: 0,
        })
    }
}

// Cross-validation trainer
#[derive(Debug)]
pub struct CrossValidationTrainer {
    base_strategy: Box<dyn TrainingStrategy>,
    folds: u32,
}

impl CrossValidationTrainer {
    pub fn new(base_strategy: Box<dyn TrainingStrategy>, folds: u32) -> Self {
        CrossValidationTrainer {
            base_strategy,
            folds,
        }
    }
    
    pub fn train_with_cv(
        &self,
        model_factory: &dyn Fn() -> Result<Box<dyn FannModel>, Box<dyn std::error::Error>>,
        data: &TrainData,
        config: &TrainingConfig,
    ) -> Result<Vec<TrainingResult>, Box<dyn std::error::Error>> {
        let total_samples = data.get_input_count();
        let fold_size = total_samples / self.folds as usize;
        
        let mut cv_results = Vec::new();
        
        for fold in 0..self.folds {
            let start_idx = (fold as usize) * fold_size;
            let end_idx = if fold == self.folds - 1 {
                total_samples
            } else {
                start_idx + fold_size
            };
            
            // Split data into train and validation
            let (train_fold, val_fold) = self.create_cv_split(data, start_idx, end_idx)?;
            
            // Create new model instance
            let mut model = model_factory()?;
            
            // Train on this fold
            let result = self.base_strategy.train(
                model.as_mut(),
                &train_fold,
                Some(&val_fold),
                config,
            )?;
            
            cv_results.push(result);
        }
        
        Ok(cv_results)
    }
    
    fn create_cv_split(
        &self,
        data: &TrainData,
        val_start: usize,
        val_end: usize,
    ) -> Result<(TrainData, TrainData), Box<dyn std::error::Error>> {
        let total_samples = data.get_input_count();
        
        let mut train_inputs = Vec::new();
        let mut train_outputs = Vec::new();
        let mut val_inputs = Vec::new();
        let mut val_outputs = Vec::new();
        
        for i in 0..total_samples {
            let input = data.get_input(i)?;
            let output = data.get_output(i)?;
            
            if i >= val_start && i < val_end {
                val_inputs.push(input);
                val_outputs.push(output);
            } else {
                train_inputs.push(input);
                train_outputs.push(output);
            }
        }
        
        let train_data = TrainData::new(&train_inputs, &train_outputs)?;
        let val_data = TrainData::new(&val_inputs, &val_outputs)?;
        
        Ok((train_data, val_data))
    }
}

// Training manager for coordinating different strategies
#[derive(Debug)]
pub struct TrainingManager {
    strategies: HashMap<String, Box<dyn TrainingStrategy>>,
}

impl TrainingManager {
    pub fn new() -> Self {
        let mut manager = TrainingManager {
            strategies: HashMap::new(),
        };
        
        // Register default strategies
        manager.register_strategy("standard".to_string(), Box::new(StandardTraining));
        manager.register_strategy("cascade".to_string(), Box::new(CascadeTraining::new()));
        
        manager
    }
    
    pub fn register_strategy(&mut self, name: String, strategy: Box<dyn TrainingStrategy>) {
        self.strategies.insert(name, strategy);
    }
    
    pub fn train_model(
        &self,
        strategy_name: &str,
        model: &mut dyn FannModel,
        train_data: &TrainData,
        validation_data: Option<&TrainData>,
        config: &TrainingConfig,
    ) -> Result<TrainingResult, Box<dyn std::error::Error>> {
        let strategy = self.strategies.get(strategy_name)
            .ok_or(format!("Strategy '{}' not found", strategy_name))?;
        
        strategy.train(model, train_data, validation_data, config)
    }
    
    pub fn list_strategies(&self) -> Vec<String> {
        self.strategies.keys().cloned().collect()
    }
}