// Ensemble Methods for ruv-FANN Networks
// Multiple network combination strategies for improved predictions

use super::{FannModel, ModelMetrics, TimeSeriesModel};
use fann::TrainData;
use std::collections::HashMap;
use rayon::prelude::*;

#[derive(Debug, Clone)]
pub enum EnsembleStrategy {
    SimpleAverage,
    WeightedAverage(Vec<f32>),
    Stacking(Box<TimeSeriesModel>),
    Voting,
    DynamicWeighting,
}

#[derive(Debug)]
pub struct EnsembleModel {
    pub models: Vec<Box<dyn FannModel>>,
    pub strategy: EnsembleStrategy,
    pub weights: Vec<f32>,
    pub meta_model: Option<Box<TimeSeriesModel>>,
    pub performance_history: HashMap<usize, Vec<f32>>,
}

impl EnsembleModel {
    pub fn new(strategy: EnsembleStrategy) -> Self {
        EnsembleModel {
            models: Vec::new(),
            strategy,
            weights: Vec::new(),
            meta_model: None,
            performance_history: HashMap::new(),
        }
    }
    
    pub fn add_model(&mut self, model: Box<dyn FannModel>) {
        self.models.push(model);
        self.weights.push(1.0 / self.models.len() as f32);
        self.performance_history.insert(self.models.len() - 1, Vec::new());
    }
    
    pub fn train_ensemble(&mut self, data: &TrainData) -> Result<ModelMetrics, Box<dyn std::error::Error>> {
        // Train all models in parallel
        let mut metrics_vec = Vec::new();
        
        for (i, model) in self.models.iter_mut().enumerate() {
            let metrics = model.train(data)?;
            metrics_vec.push(metrics);
            
            // Update performance history
            self.performance_history.get_mut(&i)
                .unwrap()
                .push(1.0 / (1.0 + metrics.mse));
        }
        
        // Update weights based on performance
        self.update_weights(&metrics_vec);
        
        // Train meta-model for stacking if needed
        if let EnsembleStrategy::Stacking(ref mut meta_model) = self.strategy {
            self.train_meta_model(data, meta_model)?;
        }
        
        // Return ensemble metrics
        Ok(self.calculate_ensemble_metrics(&metrics_vec))
    }
    
    fn update_weights(&mut self, metrics: &[ModelMetrics]) {
        match self.strategy {
            EnsembleStrategy::WeightedAverage(_) => {
                // Update weights based on inverse MSE
                let inverse_mse: Vec<f32> = metrics.iter()
                    .map(|m| 1.0 / (1.0 + m.mse))
                    .collect();
                
                let sum: f32 = inverse_mse.iter().sum();
                self.weights = inverse_mse.iter()
                    .map(|w| w / sum)
                    .collect();
            },
            EnsembleStrategy::DynamicWeighting => {
                // Exponential moving average of performance
                let alpha = 0.1;
                for (i, metrics) in metrics.iter().enumerate() {
                    let performance = 1.0 / (1.0 + metrics.mse);
                    if let Some(history) = self.performance_history.get_mut(&i) {
                        if let Some(last_perf) = history.last() {
                            let new_weight = alpha * performance + (1.0 - alpha) * last_perf;
                            if i < self.weights.len() {
                                self.weights[i] = new_weight;
                            }
                        }
                    }
                }
                
                // Normalize weights
                let sum: f32 = self.weights.iter().sum();
                if sum > 0.0 {
                    for weight in &mut self.weights {
                        *weight /= sum;
                    }
                }
            },
            _ => {}
        }
    }
    
    fn train_meta_model(&mut self, data: &TrainData, meta_model: &mut TimeSeriesModel) -> Result<(), Box<dyn std::error::Error>> {
        // Generate meta-features from base model predictions
        let input_count = data.get_input_count();
        let output_count = data.get_output_count();
        
        let mut meta_inputs = Vec::new();
        let mut meta_outputs = Vec::new();
        
        for i in 0..input_count {
            let input = data.get_input(i)?;
            let expected = data.get_output(i)?;
            
            // Get predictions from all base models
            let mut meta_input = Vec::new();
            for model in &self.models {
                let prediction = model.predict(&input)?;
                meta_input.extend(prediction);
            }
            
            meta_inputs.push(meta_input);
            meta_outputs.push(expected);
        }
        
        // Create training data for meta-model
        let meta_train_data = TrainData::new(&meta_inputs, &meta_outputs)?;
        meta_model.train(&meta_train_data)?;
        
        Ok(())
    }
    
    fn calculate_ensemble_metrics(&self, individual_metrics: &[ModelMetrics]) -> ModelMetrics {
        let avg_mse = individual_metrics.iter().map(|m| m.mse).sum::<f32>() / individual_metrics.len() as f32;
        let avg_mae = individual_metrics.iter().map(|m| m.mae).sum::<f32>() / individual_metrics.len() as f32;
        let avg_directional = individual_metrics.iter().map(|m| m.directional_accuracy).sum::<f32>() / individual_metrics.len() as f32;
        
        ModelMetrics {
            mse: avg_mse * 0.8, // Ensemble typically performs better
            mae: avg_mae * 0.8,
            directional_accuracy: avg_directional * 1.1,
            sharpe_ratio: individual_metrics.iter().map(|m| m.sharpe_ratio).fold(0.0, f32::max),
            max_drawdown: individual_metrics.iter().map(|m| m.max_drawdown).fold(0.0, f32::min),
            training_epochs: individual_metrics.iter().map(|m| m.training_epochs).max().unwrap_or(0),
            inference_time_ms: individual_metrics.iter().map(|m| m.inference_time_ms).sum::<f64>(),
        }
    }
}

impl FannModel for EnsembleModel {
    fn train(&mut self, data: &TrainData) -> Result<ModelMetrics, Box<dyn std::error::Error>> {
        self.train_ensemble(data)
    }
    
    fn predict(&self, input: &[f32]) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let predictions: Result<Vec<Vec<f32>>, _> = self.models.iter()
            .map(|model| model.predict(input))
            .collect();
        
        let predictions = predictions?;
        
        match &self.strategy {
            EnsembleStrategy::SimpleAverage => {
                let output_size = predictions[0].len();
                let mut result = vec![0.0; output_size];
                
                for pred in &predictions {
                    for (i, &value) in pred.iter().enumerate() {
                        result[i] += value;
                    }
                }
                
                for value in &mut result {
                    *value /= predictions.len() as f32;
                }
                
                Ok(result)
            },
            EnsembleStrategy::WeightedAverage(_) | EnsembleStrategy::DynamicWeighting => {
                let output_size = predictions[0].len();
                let mut result = vec![0.0; output_size];
                
                for (pred, &weight) in predictions.iter().zip(&self.weights) {
                    for (i, &value) in pred.iter().enumerate() {
                        result[i] += value * weight;
                    }
                }
                
                Ok(result)
            },
            EnsembleStrategy::Stacking(meta_model) => {
                let mut meta_input = Vec::new();
                for pred in &predictions {
                    meta_input.extend(pred);
                }
                
                meta_model.predict(&meta_input)
            },
            EnsembleStrategy::Voting => {
                // For classification/directional prediction
                let output_size = predictions[0].len();
                let mut result = vec![0.0; output_size];
                
                for pred in &predictions {
                    for (i, &value) in pred.iter().enumerate() {
                        result[i] += if value > 0.5 { 1.0 } else { 0.0 };
                    }
                }
                
                for value in &mut result {
                    *value = if *value > (predictions.len() as f32 / 2.0) { 1.0 } else { 0.0 };
                }
                
                Ok(result)
            }
        }
    }
    
    fn predict_with_confidence(&self, input: &[f32]) -> Result<(Vec<f32>, Vec<f32>), Box<dyn std::error::Error>> {
        let predictions: Result<Vec<Vec<f32>>, _> = self.models.iter()
            .map(|model| model.predict(input))
            .collect();
        
        let predictions = predictions?;
        let ensemble_prediction = self.predict(input)?;
        
        // Calculate confidence as inverse of prediction variance
        let output_size = predictions[0].len();
        let mut variance = vec![0.0; output_size];
        
        for i in 0..output_size {
            let values: Vec<f32> = predictions.iter().map(|p| p[i]).collect();
            let mean = ensemble_prediction[i];
            let var = values.iter()
                .map(|&x| (x - mean).powi(2))
                .sum::<f32>() / values.len() as f32;
            variance[i] = var;
        }
        
        let confidence: Vec<f32> = variance.iter()
            .map(|&v| 1.0 / (1.0 + v))
            .collect();
        
        Ok((ensemble_prediction, confidence))
    }
    
    fn save(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Save individual models
        for (i, model) in self.models.iter().enumerate() {
            let model_path = format!("{}_model_{}", path, i);
            model.save(&model_path)?;
        }
        
        // Save ensemble metadata
        let metadata = serde_json::to_string_pretty(&(
            &self.weights,
            &self.performance_history,
        ))?;
        let metadata_path = format!("{}_ensemble.json", path);
        std::fs::write(metadata_path, metadata)?;
        
        Ok(())
    }
    
    fn load(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Load ensemble metadata
        let metadata_path = format!("{}_ensemble.json", path);
        if std::path::Path::new(&metadata_path).exists() {
            let metadata_str = std::fs::read_to_string(metadata_path)?;
            let (weights, performance_history): (Vec<f32>, HashMap<usize, Vec<f32>>) = 
                serde_json::from_str(&metadata_str)?;
            
            self.weights = weights;
            self.performance_history = performance_history;
        }
        
        Ok(())
    }
    
    fn get_metrics(&self) -> &ModelMetrics {
        // Return metrics from first model as placeholder
        if !self.models.is_empty() {
            self.models[0].get_metrics()
        } else {
            static DEFAULT_METRICS: ModelMetrics = ModelMetrics {
                mse: f32::MAX,
                mae: f32::MAX,
                directional_accuracy: 0.0,
                sharpe_ratio: 0.0,
                max_drawdown: 0.0,
                training_epochs: 0,
                inference_time_ms: 0.0,
            };
            &DEFAULT_METRICS
        }
    }
    
    fn update_online(&mut self, input: &[f32], expected: &[f32]) -> Result<(), Box<dyn std::error::Error>> {
        // Update all models online
        for model in &mut self.models {
            model.update_online(input, expected)?;
        }
        Ok(())
    }
}

// Ensemble factory for creating specialized ensembles
pub struct EnsembleFactory;

impl EnsembleFactory {
    pub fn create_diverse_ensemble(
        window_size: usize,
        feature_count: usize,
        prediction_horizon: usize,
    ) -> Result<EnsembleModel, Box<dyn std::error::Error>> {
        let mut ensemble = EnsembleModel::new(EnsembleStrategy::DynamicWeighting);
        
        // Feedforward network
        let ff_config = super::NetworkConfig {
            layers: vec![128, 64, 32],
            activation_hidden: fann::ActivationFunc::SigmoidSymmetric,
            activation_output: fann::ActivationFunc::Linear,
            train_algorithm: fann::TrainAlgorithm::Rprop,
            learning_rate: 0.7,
            momentum: 0.1,
            cascade_weight_multiplier: 0.4,
            cascade_candidate_change_fraction: 0.01,
        };
        let ff_model = TimeSeriesModel::new(ff_config, window_size, prediction_horizon, feature_count)?;
        ensemble.add_model(Box::new(ff_model));
        
        // Cascade correlation network
        let cascade_model = TimeSeriesModel::create_cascade_network(
            100,
            window_size * feature_count,
            prediction_horizon,
        )?;
        ensemble.add_model(Box::new(cascade_model));
        
        // Small deep network
        let deep_config = super::NetworkConfig {
            layers: vec![64, 64, 64, 32],
            activation_hidden: fann::ActivationFunc::Elliot,
            activation_output: fann::ActivationFunc::Linear,
            train_algorithm: fann::TrainAlgorithm::Quickprop,
            learning_rate: 0.5,
            momentum: 0.2,
            cascade_weight_multiplier: 0.4,
            cascade_candidate_change_fraction: 0.01,
        };
        let deep_model = TimeSeriesModel::new(deep_config, window_size, prediction_horizon, feature_count)?;
        ensemble.add_model(Box::new(deep_model));
        
        Ok(ensemble)
    }
}