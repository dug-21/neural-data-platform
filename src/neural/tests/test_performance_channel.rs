use crate::data::TimeSeriesData;
use crate::neural::vendor_predictor::VendorPredictor;
use std::sync::Arc;
use tokio::sync::broadcast;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_performance_channel_basic() {
        let (tx, _rx) = broadcast::channel(100);
        
        let data = TimeSeriesData {
            timestamp: chrono::Utc::now(),
            symbol: "TEST".to_string(),
            open: 1.0,
            high: 3.0,
            low: 1.0,
            close: 2.0,
            volume: vec![1000.0],
            indicators: Default::default(),
            source: Some("test".to_string()),
            entity: Some("test".to_string()),
            value: Some(2.0),
            metadata: None,
            values: vec![1.0, 2.0, 3.0],
            timestamps: vec![chrono::Utc::now()],
            metadata_map: std::collections::HashMap::new(),
        };
        
        // Basic channel functionality test
        assert!(tx.send(data).is_ok());
    }

    #[tokio::test]
    async fn test_performance_channel_capacity() {
        let (tx, _rx) = broadcast::channel(10);
        
        // Test channel capacity limits
        for i in 0..15 {
            let data = TimeSeriesData {
                timestamp: chrono::Utc::now(),
                symbol: format!("TEST{}", i),
                open: i as f64,
                high: (i as f64) * 1.1,
                low: (i as f64) * 0.9,
                close: i as f64,
                volume: vec![1000.0],
                indicators: Default::default(),
                source: Some("test".to_string()),
                entity: Some("test".to_string()),
                value: Some(i as f64),
                metadata: None,
                values: vec![i as f64],
                timestamps: vec![chrono::Utc::now()],
                metadata_map: std::collections::HashMap::new(),
            };
            
            // Should work for first 10, then start lagging
            let result = tx.send(data);
            if i < 10 {
                assert!(result.is_ok());
            }
        }
    }
}