//! Prediction and model-related events
//! Module size: <200 lines as per requirements

use crate::events::traits::LegacyEvent as Event;
use crate::types::prediction::Prediction;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Base prediction event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionEvent {
    pub id: Uuid,
    pub model_name: String,
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub correlation_id: Option<Uuid>,
}

impl PredictionEvent {
    pub fn new(model_name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            model_name,
            timestamp: Utc::now(),
            source: "neural_predictor".to_string(),
            correlation_id: None,
        }
    }
}

/// New prediction generated event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPredictionEvent {
    pub base: PredictionEvent,
    pub symbol: String,
    pub prediction: Prediction,
    pub input_features: Vec<String>,
    pub inference_time_ms: u64,
}

impl ModelPredictionEvent {
    pub fn new(model_name: String, symbol: String, prediction: Prediction) -> Self {
        Self {
            base: PredictionEvent::new(model_name),
            symbol,
            prediction,
            input_features: Vec::new(),
            inference_time_ms: 0,
        }
    }
    
    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.input_features = features;
        self
    }
    
    pub fn with_inference_time(mut self, time_ms: u64) -> Self {
        self.inference_time_ms = time_ms;
        self
    }
}

impl Event for ModelPredictionEvent {
    fn event_type(&self) -> String {
        "model_prediction".to_string()
    }
    
    fn timestamp(&self) -> DateTime<Utc> {
        self.base.timestamp
    }
    
    fn event_id(&self) -> Uuid {
        self.base.id
    }
    
    fn source(&self) -> String {
        self.base.source.clone()
    }
    
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
    
    fn priority(&self) -> u8 {
        // Higher confidence predictions get higher priority
        if self.prediction.confidence() > 0.8 {
            8
        } else if self.prediction.confidence() > 0.6 {
            6
        } else {
            4
        }
    }
    
    fn is_persistent(&self) -> bool {
        true // Predictions should be stored for backtesting
    }
}

/// Model update/retrain event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUpdateEvent {
    pub base: PredictionEvent,
    pub update_type: ModelUpdateType,
    pub version: String,
    pub performance_metrics: HashMap<String, f64>,
    pub training_samples: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelUpdateType {
    Retrain,
    Update,
    Deploy,
    Rollback,
}

impl ModelUpdateEvent {
    pub fn new(model_name: String, update_type: ModelUpdateType, version: String) -> Self {
        Self {
            base: PredictionEvent::new(model_name),
            update_type,
            version,
            performance_metrics: HashMap::new(),
            training_samples: None,
        }
    }
    
    pub fn with_metrics(mut self, metrics: HashMap<String, f64>) -> Self {
        self.performance_metrics = metrics;
        self
    }
    
    pub fn with_training_samples(mut self, samples: u32) -> Self {
        self.training_samples = Some(samples);
        self
    }
}

impl Event for ModelUpdateEvent {
    fn event_type(&self) -> String {
        "model_update".to_string()
    }
    
    fn timestamp(&self) -> DateTime<Utc> {
        self.base.timestamp
    }
    
    fn event_id(&self) -> Uuid {
        self.base.id
    }
    
    fn source(&self) -> String {
        self.base.source.clone()
    }
    
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
    
    fn priority(&self) -> u8 {
        match self.update_type {
            ModelUpdateType::Deploy => 9,    // Highest - new model deployment
            ModelUpdateType::Rollback => 9,  // Highest - emergency rollback
            ModelUpdateType::Retrain => 7,   // High - model retraining
            ModelUpdateType::Update => 5,    // Medium - model update
        }
    }
    
    fn is_persistent(&self) -> bool {
        true // Model updates should be tracked
    }
}

/// Model performance evaluation event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPerformanceEvent {
    pub base: PredictionEvent,
    pub evaluation_period: String,
    pub accuracy: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub mse: Option<f64>,
    pub mae: Option<f64>,
    pub sharpe_ratio: Option<f64>,
    pub total_predictions: u32,
}

impl ModelPerformanceEvent {
    pub fn new(model_name: String, evaluation_period: String) -> Self {
        Self {
            base: PredictionEvent::new(model_name),
            evaluation_period,
            accuracy: 0.0,
            precision: 0.0,
            recall: 0.0,
            f1_score: 0.0,
            mse: None,
            mae: None,
            sharpe_ratio: None,
            total_predictions: 0,
        }
    }
    
    pub fn with_classification_metrics(mut self, accuracy: f64, precision: f64, recall: f64, f1: f64) -> Self {
        self.accuracy = accuracy;
        self.precision = precision;
        self.recall = recall;
        self.f1_score = f1;
        self
    }
    
    pub fn with_regression_metrics(mut self, mse: f64, mae: f64) -> Self {
        self.mse = Some(mse);
        self.mae = Some(mae);
        self
    }
    
    pub fn with_trading_metrics(mut self, sharpe_ratio: f64) -> Self {
        self.sharpe_ratio = Some(sharpe_ratio);
        self
    }
}

impl Event for ModelPerformanceEvent {
    fn event_type(&self) -> String {
        "model_performance".to_string()
    }
    
    fn timestamp(&self) -> DateTime<Utc> {
        self.base.timestamp
    }
    
    fn event_id(&self) -> Uuid {
        self.base.id
    }
    
    fn source(&self) -> String {
        self.base.source.clone()
    }
    
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
    
    fn priority(&self) -> u8 {
        // Lower performance gets higher priority for alerting
        if self.accuracy < 0.5 || self.f1_score < 0.4 {
            9 // Critical - poor performance
        } else if self.accuracy < 0.7 || self.f1_score < 0.6 {
            7 // High - degraded performance
        } else {
            5 // Normal performance update
        }
    }
    
    fn is_persistent(&self) -> bool {
        true // Performance metrics should be stored
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_model_prediction_event() {
        let prediction = Prediction::new(155.0, 0.85);
        let event = ModelPredictionEvent::new(
            "lstm_v1".to_string(),
            "AAPL".to_string(),
            prediction
        ).with_features(vec!["price".to_string(), "volume".to_string()])
         .with_inference_time(50);
        
        assert_eq!(event.base.model_name, "lstm_v1");
        assert_eq!(event.symbol, "AAPL");
        assert_eq!(event.inference_time_ms, 50);
        assert_eq!(event.input_features.len(), 2);
        assert_eq!(event.priority(), 8); // High confidence
        assert!(event.is_persistent());
    }
    
    #[test]
    fn test_model_update_event_priority() {
        let deploy_event = ModelUpdateEvent::new(
            "lstm_v1".to_string(),
            ModelUpdateType::Deploy,
            "2.0.0".to_string()
        );
        assert_eq!(deploy_event.priority(), 9);
        
        let update_event = ModelUpdateEvent::new(
            "lstm_v1".to_string(),
            ModelUpdateType::Update,
            "1.1.0".to_string()
        );
        assert_eq!(update_event.priority(), 5);
    }
    
    #[test]
    fn test_model_performance_event_priority() {
        let poor_performance = ModelPerformanceEvent::new(
            "lstm_v1".to_string(),
            "daily".to_string()
        ).with_classification_metrics(0.4, 0.3, 0.2, 0.25);
        assert_eq!(poor_performance.priority(), 9); // Critical
        
        let good_performance = ModelPerformanceEvent::new(
            "lstm_v1".to_string(),
            "daily".to_string()
        ).with_classification_metrics(0.85, 0.8, 0.75, 0.77);
        assert_eq!(good_performance.priority(), 5); // Normal
    }
}