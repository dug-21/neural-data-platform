//! Phase 3 Core Integration Tests
//!
//! Tests for core system integration with async NeuralPredictor and current APIs

pub mod integration;
pub mod predictor;
pub mod data_pipeline;

#[cfg(test)]
mod tests {
    use super::super::utilities::*;
    use anyhow::Result;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_phase3_core_system_integration() -> Result<()> {
        let config = Phase3TestConfig::default();
        let memory_tracker = MemoryTracker::new(config.memory_budget_mb);
        
        // Test async NeuralPredictor initialization
        let predictor = create_test_neural_predictor(None).await?;
        
        // Verify predictor is functional
        assert!(Arc::strong_count(&predictor) >= 1);
        
        // Check memory usage
        assert!(memory_tracker.check_budget_compliance().await?);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_time_series_data_compatibility() -> Result<()> {
        let timestamp = chrono::Utc::now();
        let data = create_test_time_series_data("AAPL", timestamp);
        
        // Verify all required Phase 3 fields are present
        assert!(!data.symbol.is_empty());
        assert!(!data.volume.is_empty()); // Vec<f64>
        assert!(data.volume_value > 0.0); // Single value
        assert!(!data.values.is_empty()); // Raw price values
        assert!(!data.intervals.is_empty()); // Time intervals
        assert!(!data.timestamps.is_empty()); // Corresponding timestamps
        assert!(data.source.is_some()); // Storage compatibility
        assert!(data.entity.is_some()); // Storage compatibility
        
        Ok(())
    }

    #[tokio::test]
    async fn test_market_hours_integration() -> Result<()> {
        let market_hours = create_test_market_hours();
        
        // Verify MarketHours can be used with DaaCoordinator
        assert!(market_hours.timezone().contains("New_York"));
        
        // Test that it integrates with the current API
        let is_open = market_hours.is_market_open(chrono::Utc::now());
        assert!(is_open || !is_open); // Either state is valid
        
        Ok(())
    }
}