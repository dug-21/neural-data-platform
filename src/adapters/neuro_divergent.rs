//! Adapter for neuro-divergent library integration
//! 
//! This module provides adapters to convert between neural-trader's TimeSeriesData
//! and neuro-divergent's TimeSeriesDataFrame formats, enabling seamless integration
//! with the advanced neural network models.

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use ndarray::{Array1, Array2};
use polars::prelude::*;
use anyhow::{Result, Context};

use crate::data::TimeSeriesData;
use super::AdapterError;

/// Adapter for converting between neural-trader and neuro-divergent data formats
pub struct NeuroDivergentAdapter;

impl NeuroDivergentAdapter {
    /// Convert TimeSeriesData to neuro-divergent format (as DataFrame)
    /// 
    /// This creates a Polars DataFrame suitable for use with neuro-divergent models
    pub fn to_neuro_divergent_df(data: &[TimeSeriesData]) -> Result<DataFrame> {
        if data.is_empty() {
            return Err(AdapterError::Serialization("Empty data provided".to_string()).into());
        }

        // Extract unique symbols
        let symbols: Vec<&str> = data.iter().map(|d| d.symbol.as_str()).collect();
        let timestamps: Vec<i64> = data.iter().map(|d| d.timestamp.timestamp()).collect();
        let opens: Vec<f64> = data.iter().map(|d| d.open).collect();
        let highs: Vec<f64> = data.iter().map(|d| d.high).collect();
        let lows: Vec<f64> = data.iter().map(|d| d.low).collect();
        let closes: Vec<f64> = data.iter().map(|d| d.close).collect();
        let volumes: Vec<f64> = data.iter().map(|d| d.volume).collect();

        // Create base columns
        let mut columns = vec![
            Series::new("unique_id", symbols),
            Series::new("ds", timestamps),
            Series::new("y", &closes), // Use close price as target
            Series::new("open", &opens),
            Series::new("high", &highs),
            Series::new("low", &lows),
            Series::new("volume", &volumes),
        ];

        // Add indicators as historical exogenous features
        if let Some(first_point) = data.first() {
            for (indicator_name, _) in &first_point.indicators {
                let values: Vec<f64> = data.iter()
                    .map(|d| d.indicators.get(indicator_name).copied().unwrap_or(0.0))
                    .collect();
                columns.push(Series::new(indicator_name, &values));
            }
        }

        DataFrame::new(columns)
            .context("Failed to create DataFrame from TimeSeriesData")
    }

    /// Convert neuro-divergent DataFrame back to TimeSeriesData
    pub fn from_neuro_divergent_df(df: &DataFrame, symbol: &str) -> Result<Vec<TimeSeriesData>> {
        let timestamps = df.column("ds")
            .context("Missing 'ds' column")?
            .i64()
            .context("Invalid timestamp type")?;
        
        let closes = df.column("y")
            .context("Missing 'y' column")?
            .f64()
            .context("Invalid close price type")?;

        let opens = df.column("open").ok().and_then(|c| c.f64().ok());
        let highs = df.column("high").ok().and_then(|c| c.f64().ok());
        let lows = df.column("low").ok().and_then(|c| c.f64().ok());
        let volumes = df.column("volume").ok().and_then(|c| c.f64().ok());

        let mut result = Vec::new();
        
        for i in 0..df.height() {
            let timestamp = timestamps.get(i)
                .ok_or_else(|| AdapterError::Serialization("Missing timestamp".to_string()))?;
            
            let close = closes.get(i)
                .ok_or_else(|| AdapterError::Serialization("Missing close price".to_string()))?;

            let mut indicators = HashMap::new();
            
            // Extract indicator columns (those not in standard OHLCV)
            let standard_cols = ["unique_id", "ds", "y", "open", "high", "low", "volume"];
            for col_name in df.get_column_names() {
                if !standard_cols.contains(&col_name) {
                    if let Ok(col) = df.column(col_name) {
                        if let Ok(values) = col.f64() {
                            if let Some(value) = values.get(i) {
                                indicators.insert(col_name.to_string(), value);
                            }
                        }
                    }
                }
            }

            let ts_data = TimeSeriesData {
                symbol: symbol.to_string(),
                timestamp: DateTime::from_timestamp(timestamp, 0)
                    .unwrap_or_else(|| Utc::now()),
                open: opens.and_then(|o| o.get(i)).unwrap_or(close),
                high: highs.and_then(|h| h.get(i)).unwrap_or(close),
                low: lows.and_then(|l| l.get(i)).unwrap_or(close),
                close,
                volume: volumes.and_then(|v| v.get(i)).unwrap_or(0.0),
                indicators,
                source: Some("neuro-divergent".to_string()),
                entity: Some(symbol.to_string()),
                value: Some(close),
                metadata: None,
            };

            result.push(ts_data);
        }

        Ok(result)
    }

    /// Prepare data for neuro-divergent model input
    /// Returns (features, target) arrays suitable for model training
    pub fn prepare_model_input(
        data: &[TimeSeriesData],
        lookback: usize,
        forecast_horizon: usize,
    ) -> Result<(Array2<f64>, Array1<f64>)> {
        if data.len() < lookback + forecast_horizon {
            return Err(AdapterError::Serialization(
                format!("Insufficient data: need {} points, have {}", 
                    lookback + forecast_horizon, data.len())
            ).into());
        }

        let n_samples = data.len() - lookback - forecast_horizon + 1;
        let n_features = 5 + data.first().map(|d| d.indicators.len()).unwrap_or(0);
        
        let mut features = Array2::<f64>::zeros((n_samples, lookback * n_features));
        let mut targets = Array1::<f64>::zeros(n_samples);

        for i in 0..n_samples {
            // Extract features for lookback window
            for j in 0..lookback {
                let idx = i + j;
                let point = &data[idx];
                
                let base_idx = j * n_features;
                features[[i, base_idx]] = point.open;
                features[[i, base_idx + 1]] = point.high;
                features[[i, base_idx + 2]] = point.low;
                features[[i, base_idx + 3]] = point.close;
                features[[i, base_idx + 4]] = point.volume;
                
                // Add indicators
                let mut indicator_idx = 5;
                for (_, value) in &point.indicators {
                    features[[i, base_idx + indicator_idx]] = *value;
                    indicator_idx += 1;
                }
            }
            
            // Target is the close price at forecast horizon
            targets[i] = data[i + lookback + forecast_horizon - 1].close;
        }

        Ok((features, targets))
    }

    /// Convert prediction results back to TimeSeriesData format
    pub fn predictions_to_timeseries(
        predictions: &[f64],
        base_timestamp: DateTime<Utc>,
        symbol: &str,
        interval_seconds: i64,
    ) -> Vec<TimeSeriesData> {
        predictions.iter().enumerate().map(|(i, &pred)| {
            let timestamp = base_timestamp + chrono::Duration::seconds(interval_seconds * i as i64);
            TimeSeriesData {
                symbol: symbol.to_string(),
                timestamp,
                open: pred,
                high: pred,
                low: pred,
                close: pred,
                volume: 0.0,
                indicators: HashMap::new(),
                source: Some("prediction".to_string()),
                entity: Some(symbol.to_string()),
                value: Some(pred),
                metadata: Some(serde_json::json!({
                    "type": "forecast",
                    "model": "neuro-divergent"
                })),
            }
        }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_conversions() {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 65.5);
        indicators.insert("macd".to_string(), 0.0012);

        let data = vec![
            TimeSeriesData {
                symbol: "BTC/USD".to_string(),
                timestamp: Utc::now(),
                open: 50000.0,
                high: 51000.0,
                low: 49500.0,
                close: 50500.0,
                volume: 1000.0,
                indicators: indicators.clone(),
                source: None,
                entity: None,
                value: None,
                metadata: None,
            }
        ];

        // Test conversion to DataFrame
        let df = NeuroDivergentAdapter::to_neuro_divergent_df(&data).unwrap();
        assert_eq!(df.height(), 1);
        assert!(df.get_column_names().contains(&"rsi"));
        assert!(df.get_column_names().contains(&"macd"));

        // Test conversion back
        let converted = NeuroDivergentAdapter::from_neuro_divergent_df(&df, "BTC/USD").unwrap();
        assert_eq!(converted.len(), 1);
        assert_eq!(converted[0].close, 50500.0);
    }
}