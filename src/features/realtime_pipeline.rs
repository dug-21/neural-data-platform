//! Real-time Feature Processing Pipeline
//!
//! This module provides real-time feature processing capabilities for streaming
//! market data, enabling low-latency feature computation for live trading.

use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::data::TimeSeriesData;

/// Real-time feature processing pipeline
#[derive(Debug)]
pub struct RealtimePipeline {
    feature_sender: Option<mpsc::UnboundedSender<FeatureUpdate>>,
}

/// Feature update message for real-time processing
#[derive(Debug, Clone)]
pub struct FeatureUpdate {
    pub symbol: String,
    pub features: HashMap<String, f64>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl RealtimePipeline {
    /// Create a new real-time pipeline
    pub fn new() -> Self {
        Self {
            feature_sender: None,
        }
    }

    /// Start the real-time processing pipeline
    pub async fn start(&mut self) -> Result<mpsc::UnboundedReceiver<FeatureUpdate>> {
        let (sender, receiver) = mpsc::unbounded_channel();
        self.feature_sender = Some(sender);
        Ok(receiver)
    }

    /// Process incoming market data in real-time
    pub async fn process_market_data(&self, data: &TimeSeriesData) -> Result<()> {
        if let Some(sender) = &self.feature_sender {
            // Compute basic features in real-time
            let mut features = HashMap::new();
            
            if data.values.len() >= 2 {
                let return_1d = (data.close / data.values[data.values.len() - 2]) - 1.0;
                features.insert("return_1d".to_string(), return_1d);
            }
            
            features.insert("price".to_string(), data.close);
            features.insert("volume".to_string(), data.volume_value);
            
            let update = FeatureUpdate {
                symbol: data.symbol.clone(),
                features,
                timestamp: data.timestamp,
            };
            
            let _ = sender.send(update);
        }
        
        Ok(())
    }

    /// Stop the real-time pipeline
    pub fn stop(&mut self) {
        self.feature_sender = None;
    }
}

impl Default for RealtimePipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_realtime_pipeline() {
        let mut pipeline = RealtimePipeline::new();
        let _receiver = pipeline.start().await.unwrap();
        
        let data = TimeSeriesData {
            symbol: "AAPL".to_string(),
            timestamp: Utc::now(),
            values: vec![100.0, 101.0],
            volume: vec![1000.0, 1100.0],
            open: 100.0,
            high: 101.5,
            low: 99.5,
            close: 101.0,
            volume_value: 1100.0,
        };
        
        let result = pipeline.process_market_data(&data).await;
        assert!(result.is_ok());
    }
}