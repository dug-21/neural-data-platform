//! DAA (Decentralized Autonomous Agents) Test Suite for Phase 3
//!
//! Tests ensuring DAA functionality is preserved and enhanced in Phase 3

pub mod preservation;
pub mod coordination;
pub mod training;

#[cfg(test)]
mod tests {
    use super::super::utilities::*;
    use anyhow::Result;
    use neural_trader::integration::daa_coordinator::DaaCoordinator;
    use neural_trader::utils::market_hours::MarketHours;

    #[tokio::test]
    async fn test_daa_basic_integration() -> Result<()> {
        let config = Phase3TestConfig::default();
        let memory_tracker = MemoryTracker::new(config.memory_budget_mb);
        
        // Test DaaCoordinator initialization with MarketHours parameter
        let market_hours = create_test_market_hours();
        let predictor = create_test_neural_predictor(None).await?;
        
        // Create DaaCoordinator with all required parameters
        let coordinator = DaaCoordinator::new(
            predictor,
            market_hours,
        ).await?;
        
        // Verify coordinator is functional
        assert!(coordinator.is_initialized().await?);
        
        // Check memory usage
        assert!(memory_tracker.check_budget_compliance().await?);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_daa_with_time_series_data() -> Result<()> {
        let market_hours = create_test_market_hours();
        let predictor = create_test_neural_predictor(None).await?;
        let coordinator = DaaCoordinator::new(predictor, market_hours).await?;
        
        // Test with Phase 3 TimeSeriesData structure
        let timestamp = chrono::Utc::now();
        let data = create_test_time_series_data("AAPL", timestamp);
        
        // Process data through DAA coordinator
        let decision = coordinator.process_market_data(&data).await?;
        
        // Verify decision structure
        assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0);
        
        Ok(())
    }
}