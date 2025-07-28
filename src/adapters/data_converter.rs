//! Comprehensive data format converter for neural-trader and vendor models
//! 
//! This module provides efficient, type-safe conversion between our internal
//! TimeSeriesData format and various vendor data formats. Note that data
//! normalization is handled upstream in the event pipeline.

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use anyhow::{Result, Context, anyhow};
use num_traits::Float;

// Internal data structures
use crate::data::TimeSeriesData;
use crate::neural::PredictionResult;

// Vendor data structures
use neuro_divergent_data::{
    TimeSeriesData as VendorTimeSeriesData,
    DataPoint as VendorDataPoint,
    TimeSeriesDataset as VendorDataset,
    TimeSeriesDatasetBuilder,
};

/// Trait for converting to vendor format
pub trait ToVendorFormat<T> {
    type Output;
    fn to_vendor_format(&self) -> Result<Self::Output>;
}

/// Trait for converting from vendor format
pub trait FromVendorFormat<T> {
    fn from_vendor_format(vendor_data: T) -> Result<Self>
    where
        Self: Sized;
}

// ============================================================================
// TimeSeriesData Conversions
// ============================================================================

impl ToVendorFormat<f64> for TimeSeriesData {
    type Output = VendorTimeSeriesData<f64>;

    fn to_vendor_format(&self) -> Result<Self::Output> {
        // Extract timestamps and values
        let timestamp = self.timestamp;
        let value = self.close; // Use close price as primary value
        
        // Create vendor data point with optional exogenous variables
        let exogenous = if !self.indicators.is_empty() {
            let mut exog_values = vec![
                self.open,
                self.high,
                self.low,
                self.volume,
            ];
            
            // Add indicators in a deterministic order
            let mut indicators: Vec<_> = self.indicators.iter().collect();
            indicators.sort_by_key(|(k, _)| k.as_str());
            
            for (_, &indicator_value) in indicators {
                exog_values.push(indicator_value);
            }
            
            Some(exog_values)
        } else {
            None
        };
        
        let data_point = if let Some(exog) = exogenous {
            VendorDataPoint::with_exogenous(timestamp, value, exog)
        } else {
            VendorDataPoint::new(timestamp, value)
        };
        
        // Build vendor time series
        let mut vendor_series = VendorTimeSeriesData::new(
            self.symbol.clone(),
            "1min".to_string(), // Default frequency, can be adjusted
        );
        
        vendor_series.add_point(data_point);
        
        // Add metadata
        if let Some(source) = &self.source {
            vendor_series.metadata.insert("source".to_string(), source.clone());
        }
        
        if let Some(metadata) = &self.metadata {
            if let Ok(metadata_str) = serde_json::to_string(metadata) {
                vendor_series.metadata.insert("original_metadata".to_string(), metadata_str);
            }
        }
        
        Ok(vendor_series)
    }
}

impl FromVendorFormat<VendorTimeSeriesData<f64>> for TimeSeriesData {
    fn from_vendor_format(vendor_data: VendorTimeSeriesData<f64>) -> Result<Self> {
        if vendor_data.data_points.is_empty() {
            return Err(anyhow!("Cannot convert empty vendor time series"));
        }
        
        let data_point = &vendor_data.data_points[0];
        let mut indicators = HashMap::new();
        
        // Extract OHLCV from exogenous variables if available
        let (open, high, low, volume) = if let Some(ref exog) = data_point.exogenous {
            let open = exog.get(0).copied().unwrap_or(data_point.value);
            let high = exog.get(1).copied().unwrap_or(data_point.value);
            let low = exog.get(2).copied().unwrap_or(data_point.value);
            let volume = exog.get(3).copied().unwrap_or(0.0);
            
            // Extract indicators from remaining exogenous variables
            if exog.len() > 4 {
                for (i, &value) in exog.iter().skip(4).enumerate() {
                    indicators.insert(format!("indicator_{}", i), value);
                }
            }
            
            (open, high, low, volume)
        } else {
            // Use the primary value for all price fields
            (data_point.value, data_point.value, data_point.value, 0.0)
        };
        
        // Extract source from metadata
        let source = vendor_data.metadata.get("source").cloned();
        
        // Extract original metadata if available
        let metadata = vendor_data.metadata.get("original_metadata")
            .and_then(|s| serde_json::from_str(s).ok());
        
        Ok(TimeSeriesData {
            symbol: vendor_data.series_id.clone(),
            timestamp: data_point.timestamp,
            open,
            high,
            low,
            close: data_point.value,
            volume,
            indicators,
            source,
            entity: Some(vendor_data.series_id),
            value: Some(data_point.value),
            metadata,
        })
    }
}

// ============================================================================
// Batch Conversions
// ============================================================================

/// Convert a vector of TimeSeriesData to vendor dataset
pub fn to_vendor_dataset(data: &[TimeSeriesData]) -> Result<VendorDataset<f64>> {
    if data.is_empty() {
        return Ok(VendorDataset::new());
    }
    
    // Group by symbol to create separate series
    let mut series_map: HashMap<String, Vec<&TimeSeriesData>> = HashMap::new();
    
    for item in data {
        series_map.entry(item.symbol.clone())
            .or_insert_with(Vec::new)
            .push(item);
    }
    
    let mut dataset = VendorDataset::new();
    
    for (symbol, series_data) in series_map {
        // Sort by timestamp
        let mut sorted_data = series_data.clone();
        sorted_data.sort_by_key(|d| d.timestamp);
        
        // Extract values and timestamps
        let timestamps: Vec<DateTime<Utc>> = sorted_data.iter()
            .map(|d| d.timestamp)
            .collect();
            
        let values: Vec<f64> = sorted_data.iter()
            .map(|d| d.close)
            .collect();
        
        // Determine frequency
        let frequency = if timestamps.len() >= 2 {
            let diff = timestamps[1].signed_duration_since(timestamps[0]);
            match diff.num_seconds() {
                60 => "1min",
                300 => "5min",
                900 => "15min",
                3600 => "1H",
                86400 => "D",
                _ => "custom",
            }
        } else {
            "1min"
        }.to_string();
        
        // Build exogenous variables matrix
        let exogenous: Vec<Vec<f64>> = sorted_data.iter()
            .map(|d| {
                let mut exog = vec![d.open, d.high, d.low, d.volume];
                
                // Add indicators in sorted order
                let mut indicators: Vec<_> = d.indicators.iter().collect();
                indicators.sort_by_key(|(k, _)| k.as_str());
                
                for (_, &value) in indicators {
                    exog.push(value);
                }
                
                exog
            })
            .collect();
        
        let vendor_series = TimeSeriesDatasetBuilder::new(symbol)
            .with_frequency(frequency)
            .with_values(values)
            .with_timestamps(timestamps)
            .with_exogenous(exogenous)
            .build()
            .context("Failed to build vendor time series")?;
        
        dataset.add_series(vendor_series);
    }
    
    Ok(dataset)
}

/// Convert vendor dataset back to vector of TimeSeriesData
pub fn from_vendor_dataset(dataset: &VendorDataset<f64>) -> Result<Vec<TimeSeriesData>> {
    let mut result = Vec::new();
    
    for series in dataset.iter() {
        for (i, data_point) in series.data_points.iter().enumerate() {
            let mut indicators = HashMap::new();
            
            // Extract OHLCV and indicators from exogenous
            let (open, high, low, volume) = if let Some(ref exog) = data_point.exogenous {
                let open = exog.get(0).copied().unwrap_or(data_point.value);
                let high = exog.get(1).copied().unwrap_or(data_point.value);
                let low = exog.get(2).copied().unwrap_or(data_point.value);
                let volume = exog.get(3).copied().unwrap_or(0.0);
                
                // Extract indicators
                if exog.len() > 4 {
                    for (idx, &value) in exog.iter().skip(4).enumerate() {
                        indicators.insert(format!("indicator_{}", idx), value);
                    }
                }
                
                (open, high, low, volume)
            } else {
                (data_point.value, data_point.value, data_point.value, 0.0)
            };
            
            result.push(TimeSeriesData {
                symbol: series.series_id.clone(),
                timestamp: data_point.timestamp,
                open,
                high,
                low,
                close: data_point.value,
                volume,
                indicators,
                source: series.metadata.get("source").cloned(),
                entity: Some(series.series_id.clone()),
                value: Some(data_point.value),
                metadata: series.metadata.get("original_metadata")
                    .and_then(|s| serde_json::from_str(s).ok()),
            });
        }
    }
    
    result.sort_by_key(|d| (d.symbol.clone(), d.timestamp));
    Ok(result)
}

// ============================================================================
// Specialized Converters
// ============================================================================

/// Convert data for feature engineering pipeline
pub fn prepare_for_feature_engineering(
    data: &[TimeSeriesData],
    lookback_window: usize,
) -> Result<Vec<VendorTimeSeriesData<f64>>> {
    // Group by symbol
    let mut series_map: HashMap<String, Vec<&TimeSeriesData>> = HashMap::new();
    
    for item in data {
        series_map.entry(item.symbol.clone())
            .or_insert_with(Vec::new)
            .push(item);
    }
    
    let mut result = Vec::new();
    
    for (symbol, series_data) in series_map {
        if series_data.len() < lookback_window {
            continue; // Skip series with insufficient data
        }
        
        // Sort by timestamp
        let mut sorted_data = series_data.clone();
        sorted_data.sort_by_key(|d| d.timestamp);
        
        let mut vendor_series = VendorTimeSeriesData::new(
            symbol,
            determine_frequency(&sorted_data),
        );
        
        for data_point in sorted_data {
            let exogenous = vec![
                data_point.open,
                data_point.high,
                data_point.low,
                data_point.volume,
            ];
            
            vendor_series.add_point(
                VendorDataPoint::with_exogenous(
                    data_point.timestamp,
                    data_point.close,
                    exogenous,
                )
            );
        }
        
        result.push(vendor_series);
    }
    
    Ok(result)
}

/// Convert predictions back to our format
pub fn predictions_from_vendor(
    vendor_predictions: &[f64],
    base_data: &TimeSeriesData,
    model_name: &str,
    confidence: f64,
) -> Vec<PredictionResult> {
    vendor_predictions.iter().enumerate().map(|(i, &value)| {
        let timestamp = base_data.timestamp + chrono::Duration::minutes((i + 1) as i64);
        
        PredictionResult {
            timestamp,
            value,
            confidence,
            interval_low: value * 0.95, // 5% confidence interval
            interval_high: value * 1.05,
            model_name: model_name.to_string(),
        }
    }).collect()
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Determine the frequency of time series data
fn determine_frequency(data: &[&TimeSeriesData]) -> String {
    if data.len() < 2 {
        return "1min".to_string();
    }
    
    let diff = data[1].timestamp.signed_duration_since(data[0].timestamp);
    match diff.num_seconds() {
        60 => "1min",
        300 => "5min",
        900 => "15min",
        3600 => "1H",
        86400 => "D",
        604800 => "W",
        _ => "custom",
    }.to_string()
}

/// Create a mapping between our indicator names and vendor feature indices
pub fn create_indicator_mapping(indicators: &HashMap<String, f64>) -> HashMap<String, usize> {
    let mut mapping = HashMap::new();
    
    // Standard features always come first
    mapping.insert("open".to_string(), 0);
    mapping.insert("high".to_string(), 1);
    mapping.insert("low".to_string(), 2);
    mapping.insert("volume".to_string(), 3);
    
    // Add indicators in sorted order
    let mut sorted_indicators: Vec<_> = indicators.keys().collect();
    sorted_indicators.sort();
    
    for (idx, indicator_name) in sorted_indicators.iter().enumerate() {
        mapping.insert((*indicator_name).clone(), 4 + idx);
    }
    
    mapping
}

// ============================================================================
// Implementation of From/Into traits for seamless conversion
// ============================================================================

impl From<TimeSeriesData> for Result<VendorTimeSeriesData<f64>> {
    fn from(data: TimeSeriesData) -> Self {
        data.to_vendor_format()
    }
}

impl TryFrom<VendorTimeSeriesData<f64>> for TimeSeriesData {
    type Error = anyhow::Error;
    
    fn try_from(vendor_data: VendorTimeSeriesData<f64>) -> Result<Self, Self::Error> {
        TimeSeriesData::from_vendor_format(vendor_data)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    
    fn create_test_data() -> TimeSeriesData {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 55.0);
        indicators.insert("macd".to_string(), 0.002);
        
        TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: Utc.ymd_opt(2024, 1, 1).unwrap().and_hms_opt(12, 0, 0).unwrap(),
            open: 100.0,
            high: 105.0,
            low: 98.0,
            close: 102.0,
            volume: 1000.0,
            indicators,
            source: Some("test".to_string()),
            entity: Some("BTC/USD".to_string()),
            value: Some(102.0),
            metadata: Some(serde_json::json!({"test": true})),
        }
    }
    
    #[test]
    fn test_to_vendor_format() {
        let data = create_test_data();
        let vendor_data = data.to_vendor_format().unwrap();
        
        assert_eq!(vendor_data.series_id, "BTC/USD");
        assert_eq!(vendor_data.data_points.len(), 1);
        
        let point = &vendor_data.data_points[0];
        assert_eq!(point.value, 102.0); // close price
        assert_eq!(point.timestamp, data.timestamp);
        
        // Check exogenous variables
        assert!(point.exogenous.is_some());
        let exog = point.exogenous.as_ref().unwrap();
        assert_eq!(exog[0], 100.0); // open
        assert_eq!(exog[1], 105.0); // high
        assert_eq!(exog[2], 98.0);  // low
        assert_eq!(exog[3], 1000.0); // volume
    }
    
    #[test]
    fn test_from_vendor_format() {
        let data = create_test_data();
        let vendor_data = data.to_vendor_format().unwrap();
        let converted_back = TimeSeriesData::from_vendor_format(vendor_data).unwrap();
        
        assert_eq!(converted_back.symbol, data.symbol);
        assert_eq!(converted_back.timestamp, data.timestamp);
        assert_eq!(converted_back.open, data.open);
        assert_eq!(converted_back.high, data.high);
        assert_eq!(converted_back.low, data.low);
        assert_eq!(converted_back.close, data.close);
        assert_eq!(converted_back.volume, data.volume);
    }
    
    #[test]
    fn test_batch_conversion() {
        let data = vec![
            create_test_data(),
            {
                let mut d = create_test_data();
                d.timestamp = d.timestamp + chrono::Duration::minutes(1);
                d.close = 103.0;
                d
            },
            {
                let mut d = create_test_data();
                d.timestamp = d.timestamp + chrono::Duration::minutes(2);
                d.close = 104.0;
                d
            },
        ];
        
        let dataset = to_vendor_dataset(&data).unwrap();
        assert_eq!(dataset.len(), 1); // One series
        assert_eq!(dataset.get(0).unwrap().len(), 3); // Three data points
        
        let converted_back = from_vendor_dataset(&dataset).unwrap();
        assert_eq!(converted_back.len(), 3);
        assert_eq!(converted_back[0].close, 102.0);
        assert_eq!(converted_back[1].close, 103.0);
        assert_eq!(converted_back[2].close, 104.0);
    }
    
    #[test]
    fn test_frequency_determination() {
        let base_time = Utc.ymd_opt(2024, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap();
        
        let data_1min = vec![
            &TimeSeriesData {
                timestamp: base_time,
                ..create_test_data()
            },
            &TimeSeriesData {
                timestamp: base_time + chrono::Duration::minutes(1),
                ..create_test_data()
            },
        ];
        
        assert_eq!(determine_frequency(&data_1min), "1min");
        
        let data_daily = vec![
            &TimeSeriesData {
                timestamp: base_time,
                ..create_test_data()
            },
            &TimeSeriesData {
                timestamp: base_time + chrono::Duration::days(1),
                ..create_test_data()
            },
        ];
        
        assert_eq!(determine_frequency(&data_daily), "D");
    }
    
    #[test]
    fn test_indicator_mapping() {
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0);
        indicators.insert("macd".to_string(), 0.001);
        indicators.insert("bb_upper".to_string(), 110.0);
        
        let mapping = create_indicator_mapping(&indicators);
        
        assert_eq!(mapping.get("open"), Some(&0));
        assert_eq!(mapping.get("high"), Some(&1));
        assert_eq!(mapping.get("low"), Some(&2));
        assert_eq!(mapping.get("volume"), Some(&3));
        
        // Indicators should be in alphabetical order
        assert_eq!(mapping.get("bb_upper"), Some(&4));
        assert_eq!(mapping.get("macd"), Some(&5));
        assert_eq!(mapping.get("rsi"), Some(&6));
    }
    
    #[test]
    fn test_predictions_conversion() {
        let base_data = create_test_data();
        let predictions = vec![103.0, 104.0, 105.0];
        
        let results = predictions_from_vendor(
            &predictions,
            &base_data,
            "test_model",
            0.85,
        );
        
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].value, 103.0);
        assert_eq!(results[0].confidence, 0.85);
        assert_eq!(results[0].model_name, "test_model");
        
        // Check timestamps are incremented
        assert_eq!(
            results[0].timestamp,
            base_data.timestamp + chrono::Duration::minutes(1)
        );
        assert_eq!(
            results[1].timestamp,
            base_data.timestamp + chrono::Duration::minutes(2)
        );
    }
}