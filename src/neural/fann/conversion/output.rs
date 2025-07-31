//! Output conversion implementation for FANN predictor
//!
//! This module handles the conversion of neural network outputs into
//! structured prediction results with proper confidence scoring and formatting.

use anyhow::Result;
use chrono::{DateTime, Utc, Duration};
use std::collections::HashMap;
use tracing::{debug, warn};

use super::{ConversionConfig, ConversionError, DataConverter, utils};
use crate::data::TimeSeriesData;
use crate::neural::PredictionResult;

/// Output data converter for neural networks
pub struct OutputConverter {
    /// Conversion configuration
    config: ConversionConfig,
    /// Output interpretation configuration
    output_config: OutputInterpretationConfig,
    /// Historical prediction accuracy for confidence adjustment
    accuracy_history: Vec<f64>,
    /// Maximum accuracy history size
    max_history_size: usize,
}

/// Configuration for output interpretation
#[derive(Debug, Clone)]
pub struct OutputInterpretationConfig {
    /// Output format type
    pub output_format: OutputFormat,
    /// Base confidence for predictions
    pub base_confidence: f64,
    /// Confidence decay factor for future predictions
    pub confidence_decay: f64,
    /// Prediction horizon in time steps
    pub prediction_horizon: usize,
    /// Enable prediction intervals
    pub enable_intervals: bool,
    /// Interval confidence level (e.g., 0.95 for 95% confidence)
    pub interval_confidence: f64,
    /// Volatility-based interval adjustment
    pub volatility_adjustment: bool,
    /// Minimum confidence threshold
    pub min_confidence: f64,
    /// Maximum confidence threshold
    pub max_confidence: f64,
}

/// Output format types
#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    /// Raw neural network outputs (normalized)
    Raw,
    /// Price predictions (absolute values)
    Price,
    /// Return predictions (percentage changes)
    Returns,
    /// Log return predictions
    LogReturns,
    /// Probabilistic outputs (mean, variance)
    Probabilistic,
    /// Classification outputs (direction/trend)
    Classification,
}

/// Prediction interval information
#[derive(Debug, Clone)]
pub struct PredictionInterval {
    /// Lower bound of the interval
    pub lower: f64,
    /// Upper bound of the interval
    pub upper: f64,
    /// Confidence level of the interval
    pub confidence_level: f64,
    /// Interval width
    pub width: f64,
}

impl Default for OutputInterpretationConfig {
    fn default() -> Self {
        Self {
            output_format: OutputFormat::Price,
            base_confidence: 0.7,
            confidence_decay: 0.95,
            prediction_horizon: 5,
            enable_intervals: true,
            interval_confidence: 0.95,
            volatility_adjustment: true,
            min_confidence: 0.1,
            max_confidence: 0.95,
        }
    }
}

impl OutputConverter {
    /// Create a new output converter
    pub fn new(config: ConversionConfig) -> Self {
        Self {
            config,
            output_config: OutputInterpretationConfig::default(),
            accuracy_history: Vec::new(),
            max_history_size: 100,
        }
    }

    /// Create with custom output configuration
    pub fn with_output_config(
        config: ConversionConfig, 
        output_config: OutputInterpretationConfig
    ) -> Self {
        Self {
            config,
            output_config,
            accuracy_history: Vec::new(),
            max_history_size: 100,
        }
    }

    /// Update accuracy history for confidence adjustment
    pub fn update_accuracy(&mut self, accuracy: f64) {
        if self.accuracy_history.len() >= self.max_history_size {
            self.accuracy_history.remove(0);
        }
        self.accuracy_history.push(accuracy.clamp(0.0, 1.0));
    }

    /// Convert single prediction output
    pub fn convert_single_output(
        &self,
        output: f32,
        base_data: &TimeSeriesData,
        step_ahead: usize,
        model_name: &str,
    ) -> Result<PredictionResult, ConversionError> {
        let prediction_time = base_data.timestamp + Duration::minutes(step_ahead as i64);
        
        // Convert output based on format
        let predicted_value = match self.output_config.output_format {
            OutputFormat::Raw => output as f64,
            OutputFormat::Price => self.convert_to_price(output as f64, base_data)?,
            OutputFormat::Returns => self.convert_from_returns(output as f64, base_data)?,
            OutputFormat::LogReturns => self.convert_from_log_returns(output as f64, base_data)?,
            OutputFormat::Probabilistic => self.convert_probabilistic_mean(output as f64, base_data)?,
            OutputFormat::Classification => self.convert_classification_to_price(output as f64, base_data)?,
        };

        // Calculate confidence
        let confidence = self.calculate_confidence(step_ahead, model_name, output as f64)?;

        // Calculate prediction intervals
        let (interval_low, interval_high) = if self.output_config.enable_intervals {
            self.calculate_prediction_intervals(predicted_value, base_data, step_ahead)?
        } else {
            (predicted_value * 0.95, predicted_value * 1.05) // Simple 5% intervals
        };

        // Create metadata
        let mut metadata = HashMap::new();
        metadata.insert("model".to_string(), serde_json::Value::String(model_name.to_string()));
        metadata.insert("step_ahead".to_string(), serde_json::Value::Number(serde_json::Number::from(step_ahead)));
        metadata.insert("output_format".to_string(), serde_json::Value::String(format!("{:?}", self.output_config.output_format)));
        metadata.insert("raw_output".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(output as f64).unwrap_or(serde_json::Number::from(0))));

        Ok(PredictionResult {
            timestamp: prediction_time,
            value: predicted_value,
            confidence,
            interval_low,
            interval_high,
            model_name: model_name.to_string(),
            metadata: Some(metadata),
        })
    }

    /// Convert multiple prediction outputs
    pub fn convert_multiple_outputs(
        &self,
        outputs: &[f32],
        base_data: &TimeSeriesData,
        model_name: &str,
    ) -> Result<Vec<PredictionResult>, ConversionError> {
        let mut predictions = Vec::new();
        let horizon = outputs.len().min(self.output_config.prediction_horizon);

        for (i, &output) in outputs.iter().take(horizon).enumerate() {
            let prediction = self.convert_single_output(output, base_data, i + 1, model_name)?;
            predictions.push(prediction);
        }

        debug!("Converted {} outputs to predictions for model: {}", 
               predictions.len(), model_name);

        Ok(predictions)
    }

    /// Convert probabilistic outputs (mean and variance)
    pub fn convert_probabilistic_outputs(
        &self,
        outputs: &[f32],
        base_data: &TimeSeriesData,
        model_name: &str,
    ) -> Result<Vec<PredictionResult>, ConversionError> {
        if outputs.len() % 2 != 0 {
            return Err(ConversionError::OutputTransformError(
                "Probabilistic outputs must have even length (mean, variance pairs)".to_string()
            ));
        }

        let mut predictions = Vec::new();
        let step_count = outputs.len() / 2;

        for i in 0..step_count {
            let mean_output = outputs[i * 2];
            let var_output = outputs[i * 2 + 1];
            
            // Convert mean to prediction value
            let predicted_value = self.convert_to_price(mean_output as f64, base_data)?;
            
            // Use variance for confidence and intervals
            let variance = var_output.abs() as f64;
            let std_dev = variance.sqrt();
            
            let prediction_time = base_data.timestamp + Duration::minutes((i + 1) as i64);
            
            // Confidence inversely related to variance
            let base_confidence = self.calculate_confidence(i + 1, model_name, mean_output as f64)?;
            let variance_penalty = (1.0 + variance).ln();
            let confidence = (base_confidence / variance_penalty).clamp(
                self.output_config.min_confidence,
                self.output_config.max_confidence
            );

            // Probabilistic intervals using standard deviation
            let z_score = 1.96; // 95% confidence interval
            let interval_width = z_score * std_dev * predicted_value;
            
            let mut metadata = HashMap::new();
            metadata.insert("model".to_string(), serde_json::Value::String(model_name.to_string()));
            metadata.insert("mean_output".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(mean_output as f64).unwrap_or(serde_json::Number::from(0))));
            metadata.insert("variance_output".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(var_output as f64).unwrap_or(serde_json::Number::from(0))));
            metadata.insert("std_dev".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(std_dev).unwrap_or(serde_json::Number::from(0))));

            predictions.push(PredictionResult {
                timestamp: prediction_time,
                value: predicted_value,
                confidence,
                interval_low: predicted_value - interval_width,
                interval_high: predicted_value + interval_width,
                model_name: model_name.to_string(),
                metadata: Some(metadata),
            });
        }

        Ok(predictions)
    }

    /// Convert classification outputs to trend predictions
    pub fn convert_classification_outputs(
        &self,
        outputs: &[f32],
        base_data: &TimeSeriesData,
        model_name: &str,
        class_labels: &[String],
    ) -> Result<Vec<PredictionResult>, ConversionError> {
        if outputs.len() != class_labels.len() {
            return Err(ConversionError::OutputTransformError(
                "Output count must match class label count for classification".to_string()
            ));
        }

        // Find the class with highest probability
        let (max_idx, max_prob) = outputs
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .unwrap();

        let predicted_class = &class_labels[max_idx];
        let confidence = *max_prob as f64;

        // Convert class to price prediction
        let predicted_value = match predicted_class.to_lowercase().as_str() {
            "up" | "bullish" | "buy" => base_data.close * 1.02, // 2% increase
            "down" | "bearish" | "sell" => base_data.close * 0.98, // 2% decrease
            "flat" | "sideways" | "hold" => base_data.close, // No change
            _ => {
                warn!("Unknown class label: {}, using current price", predicted_class);
                base_data.close
            }
        };

        let prediction_time = base_data.timestamp + Duration::minutes(1);
        
        // Classification intervals based on predicted direction
        let interval_width = base_data.close * 0.01; // 1% interval
        let (interval_low, interval_high) = match predicted_class.to_lowercase().as_str() {
            "up" | "bullish" | "buy" => (predicted_value - interval_width, predicted_value + interval_width * 2.0),
            "down" | "bearish" | "sell" => (predicted_value - interval_width * 2.0, predicted_value + interval_width),
            _ => (predicted_value - interval_width, predicted_value + interval_width),
        };

        let mut metadata = HashMap::new();
        metadata.insert("predicted_class".to_string(), serde_json::Value::String(predicted_class.clone()));
        metadata.insert("class_probability".to_string(), serde_json::Value::Number(serde_json::Number::from_f64(*max_prob as f64).unwrap_or(serde_json::Number::from(0))));
        metadata.insert("all_probabilities".to_string(), 
                       serde_json::Value::String(outputs.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")));

        Ok(vec![PredictionResult {
            timestamp: prediction_time,
            value: predicted_value,
            confidence,
            interval_low,
            interval_high,
            model_name: model_name.to_string(),
            metadata: Some(metadata),
        }])
    }

    /// Convert output to price format
    fn convert_to_price(&self, output: f64, base_data: &TimeSeriesData) -> Result<f64, ConversionError> {
        match self.output_config.output_format {
            OutputFormat::Raw => {
                // Assume raw output is normalized price change
                let price_change = (output - 0.5) * 0.1; // Convert [0,1] to [-0.05, 0.05] change
                Ok(base_data.close * (1.0 + price_change))
            },
            OutputFormat::Price => Ok(output.max(0.0)), // Ensure positive price
            OutputFormat::Returns => Ok(base_data.close * (1.0 + output)),
            OutputFormat::LogReturns => Ok(base_data.close * output.exp()),
            _ => Ok(output), // Other formats handled elsewhere
        }
    }

    /// Convert from return format
    fn convert_from_returns(&self, return_value: f64, base_data: &TimeSeriesData) -> Result<f64, ConversionError> {
        let clamped_return = utils::clamp(return_value, -0.5, 0.5); // Limit extreme returns
        Ok(base_data.close * (1.0 + clamped_return))
    }

    /// Convert from log return format
    fn convert_from_log_returns(&self, log_return: f64, base_data: &TimeSeriesData) -> Result<f64, ConversionError> {
        let clamped_log_return = utils::clamp(log_return, -0.7, 0.7); // Limit extreme log returns
        Ok(base_data.close * clamped_log_return.exp())
    }

    /// Convert probabilistic mean
    fn convert_probabilistic_mean(&self, mean_output: f64, base_data: &TimeSeriesData) -> Result<f64, ConversionError> {
        // Treat as normalized return
        let return_value = (mean_output - 0.5) * 0.2; // Convert to [-0.1, 0.1] return range
        Ok(base_data.close * (1.0 + return_value))
    }

    /// Convert classification to price
    fn convert_classification_to_price(&self, class_output: f64, base_data: &TimeSeriesData) -> Result<f64, ConversionError> {
        // Interpret as directional strength
        let direction_strength = (class_output - 0.5) * 0.04; // Max 2% move
        Ok(base_data.close * (1.0 + direction_strength))
    }

    /// Calculate confidence for prediction
    fn calculate_confidence(&self, step_ahead: usize, model_name: &str, raw_output: f64) -> Result<f64, ConversionError> {
        let mut confidence = self.output_config.base_confidence;

        // Apply step-ahead decay
        confidence *= self.output_config.confidence_decay.powi(step_ahead as i32);

        // Adjust based on historical accuracy
        if !self.accuracy_history.is_empty() {
            let avg_accuracy = self.accuracy_history.iter().sum::<f64>() / self.accuracy_history.len() as f64;
            confidence *= avg_accuracy;
        }

        // Adjust based on output certainty (distance from 0.5 for normalized outputs)
        if self.output_config.output_format == OutputFormat::Raw {
            let certainty = (raw_output - 0.5).abs() * 2.0; // Convert to [0,1]
            confidence *= 0.5 + 0.5 * certainty; // Weight by certainty
        }

        // Model-specific adjustments
        match model_name {
            "DeepAR" | "NHITS" => confidence *= 1.1, // Boost sophisticated models
            "Transformer" => confidence *= 1.05,
            "MLP" => confidence *= 0.95, // Slightly lower for simple models
            _ => {},
        }

        // Clamp to configured range
        Ok(confidence.clamp(self.output_config.min_confidence, self.output_config.max_confidence))
    }

    /// Calculate prediction intervals
    fn calculate_prediction_intervals(
        &self,
        predicted_value: f64,
        base_data: &TimeSeriesData,
        step_ahead: usize,
    ) -> Result<(f64, f64), ConversionError> {
        // Base interval width
        let mut interval_width = predicted_value * 0.02; // 2% base width

        // Adjust for volatility if enabled
        if self.output_config.volatility_adjustment {
            let volatility = self.estimate_volatility(base_data);
            interval_width *= 1.0 + volatility;
        }

        // Increase interval width with prediction horizon
        interval_width *= 1.0 + (step_ahead as f64 * 0.1);

        // Adjust for confidence level
        let z_score = self.get_z_score_for_confidence(self.output_config.interval_confidence);
        interval_width *= z_score;

        Ok((
            predicted_value - interval_width,
            predicted_value + interval_width,
        ))
    }

    /// Estimate volatility from base data (simplified)
    fn estimate_volatility(&self, base_data: &TimeSeriesData) -> f64 {
        // Simple volatility estimate from high-low range
        let hl_volatility = (base_data.high - base_data.low) / base_data.close;
        hl_volatility.clamp(0.01, 0.1) // Reasonable volatility range
    }

    /// Get z-score for confidence level
    fn get_z_score_for_confidence(&self, confidence_level: f64) -> f64 {
        match confidence_level {
            x if x >= 0.99 => 2.576,
            x if x >= 0.95 => 1.96,
            x if x >= 0.90 => 1.645,
            x if x >= 0.80 => 1.282,
            x if x >= 0.68 => 1.0,
            _ => 0.674, // 50% confidence
        }
    }

    /// Get current configuration
    pub fn config(&self) -> &ConversionConfig {
        &self.config
    }

    /// Get output configuration
    pub fn output_config(&self) -> &OutputInterpretationConfig {
        &self.output_config
    }

    /// Get accuracy history
    pub fn accuracy_history(&self) -> &[f64] {
        &self.accuracy_history
    }

    /// Clear accuracy history
    pub fn clear_accuracy_history(&mut self) {
        self.accuracy_history.clear();
    }
}

impl DataConverter for OutputConverter {
    fn convert_input(&self, _data: &[TimeSeriesData]) -> Result<Vec<Vec<f32>>, ConversionError> {
        // Output converter doesn't handle input conversion
        Err(ConversionError::InvalidInput(
            "Output converter cannot convert inputs".to_string()
        ))
    }

    fn convert_output(&self, outputs: &[f32], base_data: &TimeSeriesData) -> Result<Vec<PredictionResult>, ConversionError> {
        self.convert_multiple_outputs(outputs, base_data, "unknown")
    }

    fn validate_input(&self, _data: &[TimeSeriesData]) -> Result<(), ConversionError> {
        // Output converter doesn't validate input
        Ok(())
    }

    fn feature_count(&self) -> usize {
        // Output converter doesn't have input features
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    fn create_test_data() -> TimeSeriesData {
        TimeSeriesData {
            timestamp: Utc::now(),
            open: 100.0,
            high: 105.0,
            low: 95.0,
            close: 102.0,
            volume: 1000.0,
            indicators: HashMap::new(),
        }
    }

    #[test]
    fn test_output_converter_creation() {
        let config = ConversionConfig::default();
        let converter = OutputConverter::new(config);
        
        assert_eq!(converter.output_config.output_format, OutputFormat::Price);
        assert!(converter.accuracy_history.is_empty());
    }

    #[test]
    fn test_single_output_conversion() {
        let config = ConversionConfig::default();
        let converter = OutputConverter::new(config);
        let base_data = create_test_data();
        
        let result = converter.convert_single_output(0.02, &base_data, 1, "test_model").unwrap();
        
        assert!(result.value > 0.0);
        assert!(result.confidence > 0.0);
        assert!(result.confidence <= 1.0);
        assert_eq!(result.model_name, "test_model");
    }

    #[test]
    fn test_multiple_output_conversion() {
        let config = ConversionConfig::default();
        let converter = OutputConverter::new(config);
        let base_data = create_test_data();
        
        let outputs = vec![0.01, 0.02, -0.01];
        let results = converter.convert_multiple_outputs(&outputs, &base_data, "test_model").unwrap();
        
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.value > 0.0));
        assert!(results.iter().all(|r| r.confidence > 0.0));
    }

    #[test]
    fn test_probabilistic_conversion() {
        let config = ConversionConfig::default();
        let converter = OutputConverter::new(config);
        let base_data = create_test_data();
        
        let outputs = vec![0.02, 0.001, 0.01, 0.002]; // mean1, var1, mean2, var2
        let results = converter.convert_probabilistic_outputs(&outputs, &base_data, "test_model").unwrap();
        
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.interval_high > r.interval_low));
    }

    #[test]
    fn test_classification_conversion() {
        let config = ConversionConfig::default();
        let converter = OutputConverter::new(config);
        let base_data = create_test_data();
        
        let outputs = vec![0.1, 0.7, 0.2]; // down, up, flat probabilities
        let labels = vec!["down".to_string(), "up".to_string(), "flat".to_string()];
        
        let results = converter.convert_classification_outputs(&outputs, &base_data, "test_model", &labels).unwrap();
        
        assert_eq!(results.len(), 1);
        assert!(results[0].value > base_data.close); // Should predict "up"
        assert_eq!(results[0].confidence, 0.7);
    }

    #[test]
    fn test_accuracy_update() {
        let config = ConversionConfig::default();
        let mut converter = OutputConverter::new(config);
        
        converter.update_accuracy(0.8);
        converter.update_accuracy(0.9);
        
        assert_eq!(converter.accuracy_history.len(), 2);
        assert_eq!(converter.accuracy_history[0], 0.8);
        assert_eq!(converter.accuracy_history[1], 0.9);
    }

    #[test]
    fn test_confidence_calculation() {
        let config = ConversionConfig::default();
        let mut converter = OutputConverter::new(config);
        
        // Add some accuracy history
        converter.update_accuracy(0.8);
        
        let confidence = converter.calculate_confidence(1, "MLP", 0.7).unwrap();
        
        assert!(confidence > 0.0);
        assert!(confidence <= 1.0);
        assert!(confidence < converter.output_config.base_confidence); // Should be reduced
    }
}