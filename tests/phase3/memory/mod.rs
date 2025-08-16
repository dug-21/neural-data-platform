//! Memory Budget Compliance Tests for Phase 3
//!
//! Tests ensuring Phase 3 system operates within memory constraints

pub mod budget;
pub mod optimization;
pub mod tracking;

#[cfg(test)]
mod tests {
    use super::super::utilities::*;
    use anyhow::Result;

    #[tokio::test]
    async fn test_memory_baseline() -> Result<()> {
        let memory_tracker = MemoryTracker::new(256); // 256MB budget
        
        // Basic memory usage test
        let predictor = create_test_neural_predictor(None).await?;
        assert!(memory_tracker.check_budget_compliance().await?);
        
        let usage = memory_tracker.get_memory_usage_mb().await;
        println!("Baseline memory usage: {}MB", usage);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_memory_under_load() -> Result<()> {
        let memory_tracker = MemoryTracker::new(512); // 512MB budget
        
        // Create multiple components
        let predictor = create_test_neural_predictor(None).await?;
        let market_hours = create_test_market_hours();
        
        // Process multiple data points
        let timestamp = chrono::Utc::now();
        for i in 0..100 {
            let data = create_test_time_series_data(&format!("SYMBOL{}", i), timestamp);
            let _result = predictor.predict(&data).await?;
            
            // Check memory periodically
            if i % 10 == 0 {
                assert!(memory_tracker.check_budget_compliance().await?);
            }
        }
        
        let final_usage = memory_tracker.get_memory_usage_mb().await;
        println!("Memory usage under load: {}MB", final_usage);
        assert!(final_usage <= 512);
        
        Ok(())
    }
}