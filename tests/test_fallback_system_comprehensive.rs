use anyhow::Result;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

// Import the modules under test
use neural_trader::neural::fallback_system::{
    EmergencyFallbackSystem, SimpleMovingAverage, FallbackMetrics
};

#[cfg(test)]
mod fallback_system_tests {
    use super::*;

    #[tokio::test]
    async fn test_fallback_system_creation() {
        let fallback = EmergencyFallbackSystem::new(5);
        
        // Initially should not be enabled
        assert!(!fallback.is_enabled());
        assert_eq!(fallback.get_total_fallbacks(), 0);
        
        // Metrics should be empty
        let metrics = fallback.get_metrics().await;
        assert_eq!(metrics.total_activations, 0);
        assert!(metrics.last_activation.is_none());
        assert!(metrics.fallback_reasons.is_empty());
    }

    #[tokio::test]
    async fn test_fallback_activation() {
        let fallback = EmergencyFallbackSystem::new(5);
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        
        let result = fallback.calculate_fallback(&data).await.unwrap();
        assert_eq!(result, 3.0);
        
        // Should be enabled after activation
        assert!(fallback.is_enabled());
        assert_eq!(fallback.get_total_fallbacks(), 1);
        
        // Verify metrics updated
        let metrics = fallback.get_metrics().await;
        assert_eq!(metrics.total_activations, 1);
        assert!(metrics.last_activation.is_some());
    }

    #[tokio::test]
    async fn test_fallback_metrics_tracking() {
        let fallback = EmergencyFallbackSystem::new(3);
        
        // Activate multiple times with different data
        for i in 0..5 {
            let data = vec![i as f64; 3];
            fallback.calculate_fallback(&data).await.unwrap();
        }
        
        let metrics = fallback.get_metrics().await;
        assert_eq!(metrics.total_activations, 5);
        assert_eq!(fallback.get_total_fallbacks(), 5);
        assert!(metrics.last_activation.is_some());
    }

    #[tokio::test]
    async fn test_fallback_with_empty_data() {
        let fallback = EmergencyFallbackSystem::new(5);
        
        let result = fallback.calculate_fallback(&[]).await.unwrap();
        assert_eq!(result, 0.0);
        
        // Should still be enabled and counted
        assert!(fallback.is_enabled());
        assert_eq!(fallback.get_total_fallbacks(), 1);
    }

    #[tokio::test]
    async fn test_fallback_window_sizes() {
        let test_data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        
        // Window size 3 - should take last 3 values
        let fallback = EmergencyFallbackSystem::new(3);
        let result = fallback.calculate_fallback(&test_data).await.unwrap();
        assert_eq!(result, 9.0); // Average of [8, 9, 10]
        
        // Window size 1 - should take last value
        let fallback = EmergencyFallbackSystem::new(1);
        let result = fallback.calculate_fallback(&test_data).await.unwrap();
        assert_eq!(result, 10.0);
        
        // Window larger than data - should take all values
        let fallback = EmergencyFallbackSystem::new(20);
        let result = fallback.calculate_fallback(&test_data).await.unwrap();
        assert_eq!(result, 5.5); // Average of all 10 values
    }

    #[tokio::test]
    async fn test_predict_with_fallback_success() {
        let fallback = EmergencyFallbackSystem::new(3);
        let data = vec![10.0, 20.0, 30.0];
        
        // Test with successful neural prediction
        let result = fallback.predict_with_fallback(
            "TEST",
            &data,
            || async { Ok(42.0) }
        ).await.unwrap();
        
        assert_eq!(result, 42.0); // Should return neural prediction
        // Fallback should NOT be enabled for successful prediction
        assert!(!fallback.is_enabled());
    }

    #[tokio::test]
    async fn test_predict_with_fallback_failure() {
        let fallback = EmergencyFallbackSystem::new(3);
        let data = vec![10.0, 20.0, 30.0];
        
        // Test with failing neural prediction
        let result = fallback.predict_with_fallback(
            "TEST",
            &data,
            || async { Err(anyhow::anyhow!("Neural model failed")) }
        ).await.unwrap();
        
        assert_eq!(result, 20.0); // Should return SMA fallback
        assert!(fallback.is_enabled());
        
        // Check that failure reason was recorded
        let metrics = fallback.get_metrics().await;
        assert!(!metrics.fallback_reasons.is_empty());
        assert!(metrics.fallback_reasons.contains_key("Neural model failed"));
    }

    #[tokio::test]
    async fn test_multiple_failure_reasons() {
        let fallback = EmergencyFallbackSystem::new(2);
        let data = vec![5.0, 10.0];
        
        // Test different failure reasons
        let failures = [
            "Connection timeout",
            "Model not loaded",
            "Invalid input",
            "Connection timeout", // Duplicate to test counting
        ];
        
        for failure in failures {
            let _ = fallback.predict_with_fallback(
                "TEST",
                &data,
                || async { Err(anyhow::anyhow!("{}", failure)) }
            ).await;
        }
        
        let metrics = fallback.get_metrics().await;
        assert_eq!(metrics.total_activations, 4);
        
        // Connection timeout should appear twice
        assert_eq!(metrics.fallback_reasons.get("Connection timeout"), Some(&2));
        assert_eq!(metrics.fallback_reasons.get("Model not loaded"), Some(&1));
        assert_eq!(metrics.fallback_reasons.get("Invalid input"), Some(&1));
    }

    #[tokio::test]
    async fn test_concurrent_fallback_activations() {
        let fallback = std::sync::Arc::new(EmergencyFallbackSystem::new(3));
        let data = vec![1.0, 2.0, 3.0];
        
        // Create multiple concurrent tasks
        let mut handles = vec![];
        for i in 0..10 {
            let fallback_clone = fallback.clone();
            let data_clone = data.clone();
            let handle = tokio::spawn(async move {
                fallback_clone.predict_with_fallback(
                    &format!("TEST{}", i),
                    &data_clone,
                    || async { Err(anyhow::anyhow!("Concurrent failure {}", i)) }
                ).await
            });
            handles.push(handle);
        }
        
        // Wait for all tasks to complete
        for handle in handles {
            let result = handle.await.unwrap().unwrap();
            assert_eq!(result, 2.0); // Should all return SMA of [1,2,3]
        }
        
        // Verify all activations were recorded
        let metrics = fallback.get_metrics().await;
        assert_eq!(metrics.total_activations, 10);
        assert_eq!(fallback.get_total_fallbacks(), 10);
    }

    #[tokio::test]
    async fn test_simple_moving_average_calculator() {
        let sma = SimpleMovingAverage::new(3);
        
        // Test basic calculation
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = sma.calculate(&data);
        assert_eq!(result, 4.0); // Average of last 3: [3, 4, 5]
        
        // Test with empty data
        let result = sma.calculate(&[]);
        assert_eq!(result, 0.0);
        
        // Test with less data than window
        let data = vec![10.0, 20.0];
        let result = sma.calculate(&data);
        assert_eq!(result, 15.0); // Average of [10, 20]
        
        // Test with exact window size
        let data = vec![100.0, 200.0, 300.0];
        let result = sma.calculate(&data);
        assert_eq!(result, 200.0); // Average of [100, 200, 300]
    }

    #[tokio::test]
    async fn test_fallback_with_various_data_types() {
        let fallback = EmergencyFallbackSystem::new(2);
        
        // Test with integers converted to f64
        let int_data: Vec<f64> = vec![1, 2, 3, 4, 5].into_iter().map(|x| x as f64).collect();
        let result = fallback.calculate_fallback(&int_data).await.unwrap();
        assert_eq!(result, 4.5); // Average of last 2: [4, 5]
        
        // Test with very small numbers
        let small_data = vec![0.001, 0.002, 0.003];
        let result = fallback.calculate_fallback(&small_data).await.unwrap();
        assert_eq!(result, 0.0025); // Average of [0.002, 0.003]
        
        // Test with very large numbers
        let large_data = vec![1_000_000.0, 2_000_000.0];
        let result = fallback.calculate_fallback(&large_data).await.unwrap();
        assert_eq!(result, 1_500_000.0);
    }

    #[tokio::test]
    async fn test_fallback_metrics_persistence() {
        let fallback = EmergencyFallbackSystem::new(3);
        let data = vec![1.0, 2.0, 3.0];
        
        // Activate fallback
        let _ = fallback.calculate_fallback(&data).await.unwrap();
        
        // Wait a bit
        sleep(Duration::from_millis(10)).await;
        
        // Activate again
        let _ = fallback.calculate_fallback(&data).await.unwrap();
        
        let metrics = fallback.get_metrics().await;
        assert_eq!(metrics.total_activations, 2);
        
        // Last activation should be recent
        assert!(metrics.last_activation.is_some());
        let last_activation = metrics.last_activation.unwrap();
        let elapsed = last_activation.elapsed().unwrap();
        assert!(elapsed.as_millis() < 100); // Should be very recent
    }

    #[tokio::test]
    async fn test_fallback_negative_numbers() {
        let fallback = EmergencyFallbackSystem::new(3);
        
        // Test with negative numbers
        let data = vec![-10.0, -5.0, 0.0, 5.0, 10.0];
        let result = fallback.calculate_fallback(&data).await.unwrap();
        assert_eq!(result, 5.0); // Average of last 3: [0, 5, 10]
        
        // Test with all negative
        let data = vec![-1.0, -2.0, -3.0];
        let result = fallback.calculate_fallback(&data).await.unwrap();
        assert_eq!(result, -2.0); // Average of [-1, -2, -3]
    }

    #[tokio::test]
    async fn test_fallback_system_reset_behavior() {
        let fallback = EmergencyFallbackSystem::new(2);
        
        // Test that each fallback calculation is independent
        let data1 = vec![1.0, 2.0];
        let result1 = fallback.calculate_fallback(&data1).await.unwrap();
        assert_eq!(result1, 1.5);
        
        let data2 = vec![10.0, 20.0];
        let result2 = fallback.calculate_fallback(&data2).await.unwrap();
        assert_eq!(result2, 15.0);
        
        // Total fallbacks should accumulate
        assert_eq!(fallback.get_total_fallbacks(), 2);
        
        // But each calculation should be independent (not using previous data)
        let data3 = vec![100.0];
        let result3 = fallback.calculate_fallback(&data3).await.unwrap();
        assert_eq!(result3, 100.0); // Just the single value
    }
}