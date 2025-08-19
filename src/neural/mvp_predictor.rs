//! MVP Neural Predictor
//!
//! Simplified neural network predictor focusing on proving core ruv-FANN integration
//! Single MLP model with essential features for market prediction

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::data::TimeSeriesData;
use crate::neural::fann_model_adapter::{FannModelAdapter, FannModelConfig};
use crate::adapters::model_storage::ModelStorageConfig;
use crate::features::{FeaturePipeline, FeatureConfig, FeatureVector};

/// MVP Prediction Result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MVPPredictionResult {
    /// Predicted next-day return (-1.0 to 1.0)
    pub predicted_return: f32,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
    /// Trading decision based on prediction
    pub decision: TradingDecision,
    /// Timestamp of prediction
    pub timestamp: DateTime<Utc>,
    /// Additional metadata
    pub metadata: HashMap<String, f32>,
}

/// Simple trading decision enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TradingDecision {
    Buy,
    Sell,
    Hold,
}

impl std::fmt::Display for TradingDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradingDecision::Buy => write!(f, "BUY"),
            TradingDecision::Sell => write!(f, "SELL"), 
            TradingDecision::Hold => write!(f, "HOLD"),
        }
    }
}

/// Simple decision logic configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleDecisionLogic {
    /// Minimum predicted return to trigger buy (e.g., 0.02 for 2%)
    pub buy_threshold: f32,
    /// Maximum predicted return to trigger sell (e.g., -0.02 for -2%)
    pub sell_threshold: f32,
    /// Minimum confidence required for any trade (e.g., 0.6 for 60%)
    pub min_confidence: f32,
}

impl Default for SimpleDecisionLogic {
    fn default() -> Self {
        Self {
            buy_threshold: 0.02,    // 2% expected return
            sell_threshold: -0.02,  // -2% expected return
            min_confidence: 0.6,    // 60% minimum confidence
        }
    }
}

impl SimpleDecisionLogic {
    /// Evaluate prediction to make trading decision
    pub fn evaluate(&self, prediction: f32, confidence: f32) -> TradingDecision {
        if confidence < self.min_confidence {
            return TradingDecision::Hold;
        }
        
        match prediction {
            p if p > self.buy_threshold => TradingDecision::Buy,
            p if p < self.sell_threshold => TradingDecision::Sell,
            _ => TradingDecision::Hold,
        }
    }
}

/// MVP Neural Predictor - Single model focus
pub struct MVPPredictor {
    /// The core neural network model
    model: FannModelAdapter,
    /// Feature extraction pipeline
    feature_pipeline: FeaturePipeline,
    /// Decision making logic
    decision_logic: SimpleDecisionLogic,
    /// Training statistics for confidence calculation
    training_stats: TrainingStatistics,
}

/// Training statistics for confidence calculation
#[derive(Debug, Clone, Default)]
struct TrainingStatistics {
    training_mse: f64,
    validation_r_squared: f64,
    mean_absolute_error: f64,
    prediction_count: u64,
}

impl MVPPredictor {
    /// Create new MVP predictor
    pub async fn new(
        model_name: String,
        storage_config: ModelStorageConfig,
        decision_logic: Option<SimpleDecisionLogic>,
    ) -> Result<Self> {
        info!("🚀 Creating MVP Neural Predictor: {}", model_name);
        
        // Configure simplified feature pipeline (20 features total)
        let feature_config = FeatureConfig {
            window_size: 20,
            statistical_features: false,    // Skip for MVP
            fourier_features: false,        // Skip for MVP  
            wavelet_features: false,        // Skip for MVP
            technical_features: true,       // Essential technical indicators only
            normalize: true,
            standardize: true,
        };
        
        let feature_pipeline = FeaturePipeline::new(feature_config);
        
        // Configure single MLP model (20→64→32→1)
        let model_config = FannModelConfig {
            model_name: model_name.clone(),
            input_size: 20,              // Match feature count
            hidden_layers: vec![64, 32], // Two hidden layers
            output_size: 1,              // Single prediction
            hidden_activation: "sigmoid".to_string(),
            output_activation: "linear".to_string(),
            learning_rate: 0.001,
            momentum: 0.9,
            max_epochs: 1000,
            target_error: 0.001,
            adaptive_learning_rate: false, // Keep simple for MVP
            early_stopping_patience: 50,
            ..Default::default()
        };
        
        let model = FannModelAdapter::new(model_config, storage_config).await?;
        
        info!("✅ MVP Predictor initialized - Model: {}, Features: 20", model_name);
        
        Ok(Self {
            model,
            feature_pipeline,
            decision_logic: decision_logic.unwrap_or_default(),
            training_stats: TrainingStatistics::default(),
        })
    }
    
    /// Make prediction from market data
    pub async fn predict(&self, market_data: &[TimeSeriesData]) -> Result<MVPPredictionResult> {
        debug!("🔮 Making prediction with {} data points", market_data.len());
        
        // Validate input data
        if market_data.len() < 20 {
            return Err(anyhow!("Insufficient data: need at least 20 days, got {}", market_data.len()));
        }
        
        // Extract prices for feature calculation
        let prices: Vec<f32> = market_data.iter()
            .map(|d| d.close)
            .collect();
        
        // Extract features using pipeline (should yield 20 features)
        let features = self.feature_pipeline.extract_features(&prices);
        
        if features.len() != 20 {
            warn!("⚠️ Feature count mismatch: expected 20, got {}", features.len());
            return Err(anyhow!("Feature extraction failed: expected 20 features, got {}", features.len()));
        }
        
        debug!("✨ Extracted {} features", features.len());
        
        // Run neural network prediction
        let prediction_value = self.run_model_prediction(&features.features).await?;
        
        // Calculate confidence based on training statistics
        let confidence = self.calculate_confidence(prediction_value);
        
        // Make trading decision
        let decision = self.decision_logic.evaluate(prediction_value, confidence);
        
        // Create metadata
        let mut metadata = HashMap::new();
        metadata.insert("feature_count".to_string(), features.len() as f32);
        metadata.insert("latest_price".to_string(), market_data.last().unwrap().close);
        metadata.insert("training_mse".to_string(), self.training_stats.training_mse as f32);
        metadata.insert("validation_r2".to_string(), self.training_stats.validation_r_squared as f32);
        
        let result = MVPPredictionResult {
            predicted_return: prediction_value,
            confidence,
            decision,
            timestamp: Utc::now(),
            metadata,
        };
        
        info!("📊 Prediction: {:.4} | Confidence: {:.2}% | Decision: {} | Latest Price: ${:.2}", 
              prediction_value, confidence * 100.0, decision, market_data.last().unwrap().close);
        
        Ok(result)
    }
    
    /// Run the actual model prediction
    async fn run_model_prediction(&self, features: &[f32]) -> Result<f32> {
        // Convert to vendor format for model
        use crate::adapters::vendor_bridge::VendorTimeSeriesData;
        
        let vendor_data = VendorTimeSeriesData::new(
            "FEATURES".to_string(),
            vec![Utc::now()],
            features.to_vec(),
        );
        
        let result = self.model.predict(&vendor_data)
            .map_err(|e| anyhow!("Model prediction failed: {}", e))?;
        
        if result.forecasts.is_empty() {
            return Err(anyhow!("Model returned no predictions"));
        }
        
        Ok(result.forecasts[0])
    }
    
    /// Calculate prediction confidence based on training statistics
    fn calculate_confidence(&self, prediction: f32) -> f32 {
        // Simple confidence calculation based on training performance
        let base_confidence = if self.training_stats.validation_r_squared > 0.0 {
            self.training_stats.validation_r_squared.min(1.0) as f32
        } else {
            0.5 // Default moderate confidence
        };
        
        // Adjust confidence based on prediction magnitude
        // Lower confidence for extreme predictions
        let magnitude_factor = (-prediction.abs() * 10.0).exp().min(1.0);
        
        // Consider training error - lower MSE = higher confidence
        let error_factor = if self.training_stats.training_mse > 0.0 {
            (1.0 / (1.0 + self.training_stats.training_mse as f32)).min(1.0)
        } else {
            0.5
        };
        
        // Combine factors
        let final_confidence = (base_confidence * 0.6 + magnitude_factor * 0.2 + error_factor * 0.2).clamp(0.0, 1.0);
        
        debug!("📈 Confidence calculation: base={:.3}, magnitude={:.3}, error={:.3}, final={:.3}", 
               base_confidence, magnitude_factor, error_factor, final_confidence);
        
        final_confidence
    }
    
    /// Update training statistics (called after model training)
    pub fn update_training_stats(&mut self, training_mse: f64, validation_r_squared: f64, mae: f64) {
        self.training_stats.training_mse = training_mse;
        self.training_stats.validation_r_squared = validation_r_squared;
        self.training_stats.mean_absolute_error = mae;
        
        info!("📈 Training stats updated - MSE: {:.6}, R²: {:.4}, MAE: {:.6}", 
              training_mse, validation_r_squared, mae);
    }
    
    /// Check if model is trained and ready
    pub fn is_ready(&self) -> bool {
        self.model.is_trained()
    }
    
    /// Get model metadata
    pub fn get_model_info(&self) -> HashMap<String, String> {
        let mut info = HashMap::new();
        info.insert("model_name".to_string(), self.model.name().to_string());
        info.insert("input_features".to_string(), "20".to_string());
        info.insert("model_type".to_string(), "MLP".to_string());
        info.insert("architecture".to_string(), "20→64→32→1".to_string());
        info.insert("is_trained".to_string(), self.is_ready().to_string());
        
        if self.training_stats.validation_r_squared > 0.0 {
            info.insert("validation_r2".to_string(), format!("{:.4}", self.training_stats.validation_r_squared));
            info.insert("training_mse".to_string(), format!("{:.6}", self.training_stats.training_mse));
        }
        
        info
    }
    
    /// Get feature pipeline information
    pub fn get_feature_info(&self) -> Vec<String> {
        self.feature_pipeline.get_feature_names()
    }
    
    /// Get decision logic configuration
    pub fn get_decision_config(&self) -> &SimpleDecisionLogic {
        &self.decision_logic
    }
    
    /// Update decision logic configuration
    pub fn update_decision_config(&mut self, new_logic: SimpleDecisionLogic) {
        info!("🎯 Updating decision logic - Buy: {:.2}%, Sell: {:.2}%, Min Confidence: {:.1}%", 
              new_logic.buy_threshold * 100.0, new_logic.sell_threshold * 100.0, new_logic.min_confidence * 100.0);
        self.decision_logic = new_logic;
    }
}

/// MVP Predictor Builder for easier construction
pub struct MVPPredictorBuilder {
    model_name: String,
    storage_config: Option<ModelStorageConfig>,
    decision_logic: Option<SimpleDecisionLogic>,
}

impl MVPPredictorBuilder {
    pub fn new(model_name: String) -> Self {
        Self {
            model_name,
            storage_config: None,
            decision_logic: None,
        }
    }
    
    pub fn with_storage_config(mut self, config: ModelStorageConfig) -> Self {
        self.storage_config = Some(config);
        self
    }
    
    pub fn with_decision_logic(mut self, logic: SimpleDecisionLogic) -> Self {
        self.decision_logic = Some(logic);
        self
    }
    
    pub async fn build(self) -> Result<MVPPredictor> {
        let storage_config = self.storage_config.unwrap_or_default();
        MVPPredictor::new(self.model_name, storage_config, self.decision_logic).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::data::TimeSeriesData;
    
    async fn create_test_predictor() -> MVPPredictor {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = ModelStorageConfig {
            base_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        MVPPredictor::new("test_mvp".to_string(), storage_config, None).await.unwrap()
    }
    
    fn create_test_data(days: usize) -> Vec<TimeSeriesData> {
        let mut data = Vec::new();
        let mut price = 100.0;
        
        for i in 0..days {
            price += (i as f32 * 0.1) - 0.5; // Small random walk
            data.push(TimeSeriesData {
                timestamp: chrono::Utc::now(),
                open: price - 0.5,
                high: price + 1.0,
                low: price - 1.0,
                close: price,
                volume: 1000000.0,
            });
        }
        
        data
    }
    
    #[tokio::test]
    async fn test_mvp_predictor_creation() {
        let predictor = create_test_predictor().await;
        
        assert!(!predictor.is_ready()); // Not trained yet
        
        let info = predictor.get_model_info();
        assert_eq!(info.get("model_name").unwrap(), "test_mvp");
        assert_eq!(info.get("input_features").unwrap(), "20");
        assert_eq!(info.get("architecture").unwrap(), "20→64→32→1");
    }
    
    #[tokio::test]
    async fn test_prediction_with_insufficient_data() {
        let predictor = create_test_predictor().await;
        let data = create_test_data(10); // Less than required 20 days
        
        let result = predictor.predict(&data).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Insufficient data"));
    }
    
    #[tokio::test]
    async fn test_decision_logic() {
        let logic = SimpleDecisionLogic {
            buy_threshold: 0.02,
            sell_threshold: -0.02,
            min_confidence: 0.6,
        };
        
        // High confidence, positive prediction
        assert!(matches!(logic.evaluate(0.03, 0.8), TradingDecision::Buy));
        
        // High confidence, negative prediction
        assert!(matches!(logic.evaluate(-0.03, 0.8), TradingDecision::Sell));
        
        // High confidence, neutral prediction
        assert!(matches!(logic.evaluate(0.01, 0.8), TradingDecision::Hold));
        
        // Low confidence, any prediction
        assert!(matches!(logic.evaluate(0.05, 0.4), TradingDecision::Hold));
    }
    
    #[tokio::test]
    async fn test_confidence_calculation() {
        let mut predictor = create_test_predictor().await;
        
        // Update with good training stats
        predictor.update_training_stats(0.001, 0.7, 0.02);
        
        let confidence = predictor.calculate_confidence(0.01);
        assert!(confidence > 0.5); // Should have reasonable confidence
        assert!(confidence <= 1.0);
        
        // Test extreme prediction (should lower confidence)
        let extreme_confidence = predictor.calculate_confidence(0.5);
        assert!(extreme_confidence < confidence);
    }
    
    #[tokio::test]
    async fn test_builder_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let storage_config = ModelStorageConfig {
            base_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let decision_logic = SimpleDecisionLogic {
            buy_threshold: 0.03,
            sell_threshold: -0.01,
            min_confidence: 0.7,
        };
        
        let predictor = MVPPredictorBuilder::new("builder_test".to_string())
            .with_storage_config(storage_config)
            .with_decision_logic(decision_logic)
            .build()
            .await
            .unwrap();
        
        assert_eq!(predictor.get_decision_config().buy_threshold, 0.03);
        assert_eq!(predictor.get_decision_config().sell_threshold, -0.01);
        assert_eq!(predictor.get_decision_config().min_confidence, 0.7);
    }
}