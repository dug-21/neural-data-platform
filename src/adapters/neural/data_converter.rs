//! Data format converter for neural models
//! 
//! This module handles conversion between different data formats used by
//! neural-trader and neuro-divergent models. Note that data normalization
//! is handled upstream in the event pipeline, so this converter focuses
//! only on format transformations.

use ndarray::{Array1, Array2, Array3};
use polars::prelude::*;
use std::collections::HashMap;
use anyhow::{Result, Context};

use crate::data::TimeSeriesData;
use super::neuro_divergent_adapter::{NeuralAdapterError, NeuralModelConfig};

/// Supported conversion formats
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConversionFormat {
    /// Polars DataFrame format
    DataFrame,
    /// NumPy-style ndarray format
    NdArray,
    /// Tensor format (3D array)
    Tensor,
    /// Dictionary of arrays
    DictArray,
}

/// Model input data in various formats
pub enum ModelInput {
    DataFrame(DataFrame),
    Array2D(Array2<f64>),
    Array3D(Array3<f64>),
    DictArray(HashMap<String, Vec<f64>>),
}

/// Data converter for neural model formats
pub struct DataConverter {
    /// Feature columns to include in conversion
    feature_columns: Vec<String>,
}

impl DataConverter {
    /// Create a new data converter
    pub fn new() -> Self {
        Self {
            feature_columns: vec![
                "open".to_string(),
                "high".to_string(),
                "low".to_string(),
                "close".to_string(),
                "volume".to_string(),
            ],
        }
    }
    
    /// Create with custom feature columns
    pub fn with_features(feature_columns: Vec<String>) -> Self {
        Self { feature_columns }
    }
    
    /// Convert TimeSeriesData to the specified model format
    pub fn to_model_format(
        &self,
        data: &[TimeSeriesData],
        config: &NeuralModelConfig,
    ) -> Result<ModelInput, NeuralAdapterError> {
        // Determine format based on model type
        let format = self.get_format_for_model(&config.model_type);
        
        match format {
            ConversionFormat::DataFrame => self.to_dataframe(data),
            ConversionFormat::NdArray => self.to_ndarray(data, config),
            ConversionFormat::Tensor => self.to_tensor(data, config),
            ConversionFormat::DictArray => self.to_dict_array(data),
        }
    }
    
    /// Determine the appropriate format for a model type
    fn get_format_for_model(&self, model_type: &str) -> ConversionFormat {
        match model_type {
            "TimeMixer" | "TimesFM" => ConversionFormat::Tensor,
            "NeuralForecast" => ConversionFormat::DataFrame,
            "Prophet" | "StatsForecast" => ConversionFormat::DictArray,
            _ => ConversionFormat::NdArray,
        }
    }
    
    /// Convert to Polars DataFrame format
    fn to_dataframe(&self, data: &[TimeSeriesData]) -> Result<ModelInput, NeuralAdapterError> {
        if data.is_empty() {
            return Err(NeuralAdapterError::Conversion("Empty data provided".to_string()));
        }
        
        // Extract base columns
        let symbols: Vec<&str> = data.iter().map(|d| d.symbol.as_str()).collect();
        let timestamps: Vec<i64> = data.iter().map(|d| d.timestamp.timestamp()).collect();
        
        let mut columns = vec![
            Series::new("unique_id", symbols),
            Series::new("ds", timestamps),
        ];
        
        // Add feature columns
        for feature in &self.feature_columns {
            let values: Vec<f64> = match feature.as_str() {
                "open" => data.iter().map(|d| d.open).collect(),
                "high" => data.iter().map(|d| d.high).collect(),
                "low" => data.iter().map(|d| d.low).collect(),
                "close" => data.iter().map(|d| d.close).collect(),
                "volume" => data.iter().map(|d| d.volume).collect(),
                _ => continue,
            };
            columns.push(Series::new(feature, &values));
        }
        
        // Add target column (typically close price)
        let targets: Vec<f64> = data.iter().map(|d| d.close).collect();
        columns.push(Series::new("y", &targets));
        
        // Add indicator columns if present
        if let Some(first_point) = data.first() {
            for (indicator_name, _) in &first_point.indicators {
                let values: Vec<f64> = data.iter()
                    .map(|d| d.indicators.get(indicator_name).copied().unwrap_or(0.0))
                    .collect();
                columns.push(Series::new(indicator_name, &values));
            }
        }
        
        let df = DataFrame::new(columns)
            .map_err(|e| NeuralAdapterError::Conversion(format!("DataFrame creation failed: {}", e)))?;
        
        Ok(ModelInput::DataFrame(df))
    }
    
    /// Convert to 2D ndarray format (samples x features)
    fn to_ndarray(
        &self,
        data: &[TimeSeriesData],
        config: &NeuralModelConfig,
    ) -> Result<ModelInput, NeuralAdapterError> {
        if data.len() < config.lookback_window {
            return Err(NeuralAdapterError::Conversion(format!(
                "Insufficient data for lookback window: need {}, have {}",
                config.lookback_window,
                data.len()
            )));
        }
        
        let n_samples = data.len() - config.lookback_window + 1;
        let n_features = self.feature_columns.len() + data.first()
            .map(|d| d.indicators.len())
            .unwrap_or(0);
        
        let mut array = Array2::<f64>::zeros((n_samples, config.lookback_window * n_features));
        
        for i in 0..n_samples {
            for j in 0..config.lookback_window {
                let idx = i + j;
                let point = &data[idx];
                
                let base_idx = j * n_features;
                
                // Add base features
                for (k, feature) in self.feature_columns.iter().enumerate() {
                    let value = match feature.as_str() {
                        "open" => point.open,
                        "high" => point.high,
                        "low" => point.low,
                        "close" => point.close,
                        "volume" => point.volume,
                        _ => 0.0,
                    };
                    array[[i, base_idx + k]] = value;
                }
                
                // Add indicators
                let mut indicator_idx = self.feature_columns.len();
                for (_, value) in &point.indicators {
                    array[[i, base_idx + indicator_idx]] = *value;
                    indicator_idx += 1;
                }
            }
        }
        
        Ok(ModelInput::Array2D(array))
    }
    
    /// Convert to 3D tensor format (samples x timesteps x features)
    fn to_tensor(
        &self,
        data: &[TimeSeriesData],
        config: &NeuralModelConfig,
    ) -> Result<ModelInput, NeuralAdapterError> {
        if data.len() < config.lookback_window {
            return Err(NeuralAdapterError::Conversion(format!(
                "Insufficient data for lookback window: need {}, have {}",
                config.lookback_window,
                data.len()
            )));
        }
        
        let n_samples = data.len() - config.lookback_window + 1;
        let n_features = self.feature_columns.len() + data.first()
            .map(|d| d.indicators.len())
            .unwrap_or(0);
        
        let mut tensor = Array3::<f64>::zeros((n_samples, config.lookback_window, n_features));
        
        for i in 0..n_samples {
            for j in 0..config.lookback_window {
                let idx = i + j;
                let point = &data[idx];
                
                // Add base features
                for (k, feature) in self.feature_columns.iter().enumerate() {
                    let value = match feature.as_str() {
                        "open" => point.open,
                        "high" => point.high,
                        "low" => point.low,
                        "close" => point.close,
                        "volume" => point.volume,
                        _ => 0.0,
                    };
                    tensor[[i, j, k]] = value;
                }
                
                // Add indicators
                let mut indicator_idx = self.feature_columns.len();
                for (_, value) in &point.indicators {
                    tensor[[i, j, indicator_idx]] = *value;
                    indicator_idx += 1;
                }
            }
        }
        
        Ok(ModelInput::Array3D(tensor))
    }
    
    /// Convert to dictionary of arrays format
    fn to_dict_array(&self, data: &[TimeSeriesData]) -> Result<ModelInput, NeuralAdapterError> {
        if data.is_empty() {
            return Err(NeuralAdapterError::Conversion("Empty data provided".to_string()));
        }
        
        let mut dict = HashMap::new();
        
        // Add timestamp array
        dict.insert(
            "ds".to_string(),
            data.iter().map(|d| d.timestamp.timestamp() as f64).collect()
        );
        
        // Add feature arrays
        for feature in &self.feature_columns {
            let values: Vec<f64> = match feature.as_str() {
                "open" => data.iter().map(|d| d.open).collect(),
                "high" => data.iter().map(|d| d.high).collect(),
                "low" => data.iter().map(|d| d.low).collect(),
                "close" => data.iter().map(|d| d.close).collect(),
                "volume" => data.iter().map(|d| d.volume).collect(),
                _ => continue,
            };
            dict.insert(feature.clone(), values);
        }
        
        // Add target array
        dict.insert(
            "y".to_string(),
            data.iter().map(|d| d.close).collect()
        );
        
        // Add indicator arrays
        if let Some(first_point) = data.first() {
            for (indicator_name, _) in &first_point.indicators {
                let values: Vec<f64> = data.iter()
                    .map(|d| d.indicators.get(indicator_name).copied().unwrap_or(0.0))
                    .collect();
                dict.insert(indicator_name.clone(), values);
            }
        }
        
        Ok(ModelInput::DictArray(dict))
    }
    
    /// Convert model predictions back to TimeSeriesData
    pub fn from_predictions(
        &self,
        predictions: &[f64],
        base_data: &TimeSeriesData,
        start_offset: i64,
        interval_seconds: i64,
    ) -> Vec<TimeSeriesData> {
        predictions.iter().enumerate().map(|(i, &pred)| {
            let timestamp = base_data.timestamp + 
                chrono::Duration::seconds(start_offset + (interval_seconds * i as i64));
            
            TimeSeriesData {
                symbol: base_data.symbol.clone(),
                timestamp,
                open: pred,
                high: pred,
                low: pred,
                close: pred,
                volume: 0.0,
                indicators: HashMap::new(),
                source: Some("neural_prediction".to_string()),
                entity: base_data.entity.clone(),
                value: Some(pred),
                metadata: Some(serde_json::json!({
                    "type": "forecast",
                    "model": "neuro-divergent",
                    "base_timestamp": base_data.timestamp.to_rfc3339(),
                })),
            }
        }).collect()
    }
}

impl Default for DataConverter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    
    fn create_test_data(points: usize) -> Vec<TimeSeriesData> {
        let mut data = Vec::new();
        for i in 0..points {
            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 50.0 + (i as f64 * 0.5));
            indicators.insert("macd".to_string(), 0.001 * i as f64);
            
            data.push(TimeSeriesData {
                symbol: "TEST/USD".to_string(),
                timestamp: Utc::now() + chrono::Duration::minutes(i as i64),
                open: 100.0 + i as f64,
                high: 101.0 + i as f64,
                low: 99.0 + i as f64,
                close: 100.5 + i as f64,
                volume: 1000.0 * (1.0 + i as f64 * 0.1),
                indicators,
                source: Some("test".to_string()),
                entity: None,
                value: None,
                metadata: None,
            });
        }
        data
    }
    
    #[test]
    fn test_dataframe_conversion() {
        let converter = DataConverter::new();
        let data = create_test_data(10);
        let config = NeuralModelConfig::default();
        
        let result = converter.to_model_format(&data, &config).unwrap();
        
        match result {
            ModelInput::DataFrame(df) => {
                assert_eq!(df.height(), 10);
                assert!(df.get_column_names().contains(&"unique_id"));
                assert!(df.get_column_names().contains(&"ds"));
                assert!(df.get_column_names().contains(&"y"));
                assert!(df.get_column_names().contains(&"rsi"));
                assert!(df.get_column_names().contains(&"macd"));
            }
            _ => panic!("Expected DataFrame output"),
        }
    }
    
    #[test]
    fn test_ndarray_conversion() {
        let converter = DataConverter::new();
        let data = create_test_data(30);
        let mut config = NeuralModelConfig::default();
        config.model_type = "Generic".to_string();
        
        let result = converter.to_model_format(&data, &config).unwrap();
        
        match result {
            ModelInput::Array2D(arr) => {
                // 30 points - 24 lookback + 1 = 7 samples
                assert_eq!(arr.shape()[0], 7);
                // 24 lookback * (5 features + 2 indicators) = 168
                assert_eq!(arr.shape()[1], 168);
            }
            _ => panic!("Expected Array2D output"),
        }
    }
    
    #[test]
    fn test_tensor_conversion() {
        let converter = DataConverter::new();
        let data = create_test_data(30);
        let mut config = NeuralModelConfig::default();
        config.model_type = "TimeMixer".to_string();
        
        let result = converter.to_model_format(&data, &config).unwrap();
        
        match result {
            ModelInput::Array3D(tensor) => {
                assert_eq!(tensor.shape()[0], 7);  // samples
                assert_eq!(tensor.shape()[1], 24); // timesteps
                assert_eq!(tensor.shape()[2], 7);  // features
            }
            _ => panic!("Expected Array3D output"),
        }
    }
    
    #[test]
    fn test_dict_array_conversion() {
        let converter = DataConverter::new();
        let data = create_test_data(10);
        let mut config = NeuralModelConfig::default();
        config.model_type = "Prophet".to_string();
        
        let result = converter.to_model_format(&data, &config).unwrap();
        
        match result {
            ModelInput::DictArray(dict) => {
                assert!(dict.contains_key("ds"));
                assert!(dict.contains_key("y"));
                assert!(dict.contains_key("open"));
                assert!(dict.contains_key("rsi"));
                assert_eq!(dict.get("ds").unwrap().len(), 10);
            }
            _ => panic!("Expected DictArray output"),
        }
    }
    
    #[test]
    fn test_prediction_conversion() {
        let converter = DataConverter::new();
        let base_data = create_test_data(1)[0].clone();
        let predictions = vec![101.0, 102.0, 103.0];
        
        let result = converter.from_predictions(
            &predictions,
            &base_data,
            3600, // 1 hour offset
            3600, // 1 hour intervals
        );
        
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].close, 101.0);
        assert_eq!(result[1].close, 102.0);
        assert_eq!(result[2].close, 103.0);
        
        // Check timestamps are correct
        let base_ts = base_data.timestamp.timestamp();
        assert_eq!(result[0].timestamp.timestamp(), base_ts + 3600);
        assert_eq!(result[1].timestamp.timestamp(), base_ts + 7200);
        assert_eq!(result[2].timestamp.timestamp(), base_ts + 10800);
    }
}