//! Performance Benchmark Tests for Phase 3
//!
//! Tests ensuring Phase 3 system meets performance requirements

pub mod benchmarks;
pub mod latency;
pub mod throughput;

#[cfg(test)]
mod tests {
    use super::super::utilities::*;
    use anyhow::Result;
    use std::time::Instant;

    #[tokio::test]
    async fn test_prediction_latency_benchmark() -> Result<()> {
        let predictor = create_test_neural_predictor(None).await?;
        let timestamp = chrono::Utc::now();
        let data = create_test_time_series_data("AAPL", timestamp);
        
        // Warmup
        for _ in 0..10 {
            let _result = predictor.predict(&data).await?;
        }
        
        // Benchmark
        let start = Instant::now();
        let iterations = 1000;
        
        for _ in 0..iterations {
            let _result = predictor.predict(&data).await?;
        }
        
        let duration = start.elapsed();
        let avg_latency_ms = duration.as_millis() / iterations;
        
        println!("Average prediction latency: {}ms", avg_latency_ms);
        assert!(avg_latency_ms < 50, "Latency too high: {}ms", avg_latency_ms);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_system_throughput_benchmark() -> Result<()> {
        let predictor = create_test_neural_predictor(None).await?;
        let timestamp = chrono::Utc::now();
        
        let start = Instant::now();
        let mut handles = Vec::new();
        
        // Spawn concurrent predictions
        for i in 0..100 {
            let predictor_clone = std::sync::Arc::new(predictor.clone());
            let data = create_test_time_series_data(&format!("SYMBOL{}", i), timestamp);
            
            let handle = tokio::spawn(async move {
                predictor_clone.predict(&data).await
            });
            handles.push(handle);
        }
        
        // Wait for all to complete
        for handle in handles {
            handle.await??;
        }
        
        let duration = start.elapsed();
        let throughput = 100.0 / duration.as_secs_f64();
        
        println!("System throughput: {:.2} predictions/sec", throughput);
        assert!(throughput > 50.0, "Throughput too low: {:.2} predictions/sec", throughput);
        
        Ok(())
    }
}