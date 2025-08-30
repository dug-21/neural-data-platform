// ruv-FANN Neural Network Models Module
// Comprehensive ML architecture using FANN for time series prediction

pub mod fann_architectures;
pub mod ensemble;
pub mod cascade;
pub mod recurrent;
pub mod online_learning;

use fann::{Fann, TrainData, ActivationFunc, TrainAlgorithm};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub layers: Vec<u32>,
    pub activation_hidden: ActivationFunc,
    pub activation_output: ActivationFunc,
    pub train_algorithm: TrainAlgorithm,
    pub learning_rate: f32,
    pub momentum: f32,
    pub cascade_weight_multiplier: f32,
    pub cascade_candidate_change_fraction: f32,
}

#[derive(Debug, Clone)]
pub struct ModelMetrics {
    pub mse: f32,
    pub mae: f32,
    pub directional_accuracy: f32,
    pub sharpe_ratio: f32,
    pub max_drawdown: f32,
    pub training_epochs: u32,
    pub inference_time_ms: f64,
}

pub trait FannModel {
    fn train(&mut self, data: &TrainData) -> Result<ModelMetrics, Box<dyn std::error::Error>>;
    fn predict(&self, input: &[f32]) -> Result<Vec<f32>, Box<dyn std::error::Error>>;
    fn predict_with_confidence(&self, input: &[f32]) -> Result<(Vec<f32>, Vec<f32>), Box<dyn std::error::Error>>;
    fn save(&self, path: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn load(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn get_metrics(&self) -> &ModelMetrics;
    fn update_online(&mut self, input: &[f32], expected: &[f32]) -> Result<(), Box<dyn std::error::Error>>;
}

#[derive(Debug, Clone)]
pub struct TimeSeriesModel {
    pub network: Fann,
    pub config: NetworkConfig,
    pub metrics: ModelMetrics,
    pub window_size: usize,
    pub prediction_horizon: usize,
    pub feature_count: usize,
    pub model_id: String,
    pub version: u32,
}

impl TimeSeriesModel {
    pub fn new(config: NetworkConfig, window_size: usize, prediction_horizon: usize, feature_count: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let mut layers = vec![window_size as u32 * feature_count as u32];
        layers.extend(config.layers.clone());
        layers.push(prediction_horizon as u32);
        
        let network = Fann::new(&layers)?;
        
        Ok(TimeSeriesModel {
            network,
            config,
            metrics: ModelMetrics::default(),
            window_size,
            prediction_horizon,
            feature_count,
            model_id: uuid::Uuid::new_v4().to_string(),
            version: 1,
        })
    }
    
    pub fn create_cascade_network(max_neurons: u32, feature_count: usize, output_count: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let network = Fann::new_cascade(feature_count as u32, output_count as u32)?;
        
        let config = NetworkConfig {
            layers: vec![],
            activation_hidden: ActivationFunc::SigmoidSymmetric,
            activation_output: ActivationFunc::Linear,
            train_algorithm: TrainAlgorithm::Rprop,
            learning_rate: 0.7,
            momentum: 0.1,
            cascade_weight_multiplier: 0.4,
            cascade_candidate_change_fraction: 0.01,
        };
        
        Ok(TimeSeriesModel {
            network,
            config,
            metrics: ModelMetrics::default(),
            window_size: 1,
            prediction_horizon: output_count,
            feature_count,
            model_id: uuid::Uuid::new_v4().to_string(),
            version: 1,
        })
    }
}

impl Default for ModelMetrics {
    fn default() -> Self {
        ModelMetrics {
            mse: f32::MAX,
            mae: f32::MAX,
            directional_accuracy: 0.0,
            sharpe_ratio: 0.0,
            max_drawdown: 0.0,
            training_epochs: 0,
            inference_time_ms: 0.0,
        }
    }
}

impl FannModel for TimeSeriesModel {
    fn train(&mut self, data: &TrainData) -> Result<ModelMetrics, Box<dyn std::error::Error>> {
        // Configure network
        self.network.set_activation_function_hidden(self.config.activation_hidden);
        self.network.set_activation_function_output(self.config.activation_output);
        self.network.set_train_algorithm(self.config.train_algorithm);
        self.network.set_learning_rate(self.config.learning_rate);
        self.network.set_learning_momentum(self.config.momentum);
        
        // Train network
        let max_epochs = 10000;
        let desired_error = 0.001;
        let epochs_between_reports = 100;
        
        self.network.train_on_data(data, max_epochs, epochs_between_reports, desired_error);
        
        // Calculate metrics
        let mse = self.network.get_mse();
        self.metrics.mse = mse;
        self.metrics.training_epochs = self.network.get_training_algorithm() as u32;
        
        Ok(self.metrics.clone())
    }
    
    fn predict(&self, input: &[f32]) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let output = self.network.run(input)?;
        Ok(output)
    }
    
    fn predict_with_confidence(&self, input: &[f32]) -> Result<(Vec<f32>, Vec<f32>), Box<dyn std::error::Error>> {
        let prediction = self.predict(input)?;
        
        // Simple confidence estimation based on training error
        let confidence = vec![1.0 - self.metrics.mse.sqrt(); prediction.len()];
        
        Ok((prediction, confidence))
    }
    
    fn save(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.network.save(path)?;
        
        // Save metadata
        let metadata_path = format!("{}.meta", path);
        let metadata = serde_json::to_string_pretty(&(
            &self.config,
            &self.metrics,
            self.window_size,
            self.prediction_horizon,
            self.feature_count,
            &self.model_id,
            self.version,
        ))?;
        std::fs::write(metadata_path, metadata)?;
        
        Ok(())
    }
    
    fn load(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.network = Fann::new_from_file(path)?;
        
        // Load metadata
        let metadata_path = format!("{}.meta", path);
        if std::path::Path::new(&metadata_path).exists() {
            let metadata_str = std::fs::read_to_string(metadata_path)?;
            let (config, metrics, window_size, prediction_horizon, feature_count, model_id, version): 
                (NetworkConfig, ModelMetrics, usize, usize, usize, String, u32) = 
                serde_json::from_str(&metadata_str)?;
            
            self.config = config;
            self.metrics = metrics;
            self.window_size = window_size;
            self.prediction_horizon = prediction_horizon;
            self.feature_count = feature_count;
            self.model_id = model_id;
            self.version = version;
        }
        
        Ok(())
    }
    
    fn get_metrics(&self) -> &ModelMetrics {
        &self.metrics
    }
    
    fn update_online(&mut self, input: &[f32], expected: &[f32]) -> Result<(), Box<dyn std::error::Error>> {
        self.network.train(input, expected);
        Ok(())
    }
}

// Model registry for managing multiple models
#[derive(Debug)]
pub struct ModelRegistry {
    models: HashMap<String, Box<dyn FannModel>>,
    active_model: Option<String>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        ModelRegistry {
            models: HashMap::new(),
            active_model: None,
        }
    }
    
    pub fn register_model(&mut self, id: String, model: Box<dyn FannModel>) {
        self.models.insert(id, model);
    }
    
    pub fn set_active_model(&mut self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        if self.models.contains_key(id) {
            self.active_model = Some(id.to_string());
            Ok(())
        } else {
            Err(format!("Model {} not found", id).into())
        }
    }
    
    pub fn get_active_model(&mut self) -> Option<&mut Box<dyn FannModel>> {
        if let Some(id) = &self.active_model {
            self.models.get_mut(id)
        } else {
            None
        }
    }
    
    pub fn list_models(&self) -> Vec<String> {
        self.models.keys().cloned().collect()
    }
}