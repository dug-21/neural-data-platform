use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::RwLock;
use std::time::Instant;
use anyhow::Result;
use std::collections::HashMap;
use tracing::{info, warn};

/// Metrics for tracking fallback system usage
#[derive(Debug, Default)]
pub struct FallbackMetrics {
    pub total_activations: u64,
    pub last_activation: Option<Instant>,
    pub fallback_reasons: HashMap<String, u64>,
}

/// Emergency fallback system for when neural predictions fail
pub struct EmergencyFallbackSystem {
    enabled: Arc<AtomicBool>,
    metrics: Arc<RwLock<FallbackMetrics>>,
    sma_window: usize,
    total_fallbacks: Arc<AtomicU64>,
}

impl EmergencyFallbackSystem {
    pub fn new(sma_window: usize) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(false)),
            metrics: Arc::new(RwLock::new(FallbackMetrics::default())),
            sma_window,
            total_fallbacks: Arc::new(AtomicU64::new(0)),
        }
    }
    
    /// Calculate fallback prediction using Simple Moving Average
    pub async fn calculate_fallback(&self, data: &[f64]) -> Result<f64> {
        // Mark fallback as enabled
        self.enabled.store(true, Ordering::Relaxed);
        self.total_fallbacks.fetch_add(1, Ordering::Relaxed);
        
        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_activations += 1;
            metrics.last_activation = Some(Instant::now());
        }
        
        // Simple moving average calculation
        if data.is_empty() {
            return Ok(0.0);
        }
        
        let window = self.sma_window.min(data.len());
        let sum: f64 = data.iter().rev().take(window).sum();
        let avg = sum / window as f64;
        
        info!("Fallback SMA calculated: {} (window: {})", avg, window);
        Ok(avg)
    }
    
    /// Predict with automatic fallback on failure
    pub async fn predict_with_fallback<F, Fut>(
        &self,
        symbol: &str,
        data: &[f64],
        neural_predict_fn: F,
    ) -> Result<f64>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<f64>>,
    {
        // Try neural prediction first
        match neural_predict_fn().await {
            Ok(prediction) => {
                info!("Neural prediction successful for {}: {}", symbol, prediction);
                Ok(prediction)
            }
            Err(e) => {
                // Neural prediction failed, use fallback
                warn!("Neural prediction failed for {}: {}, using SMA fallback", symbol, e);
                
                // Update failure reason metrics
                {
                    let mut metrics = self.metrics.write().await;
                    let reason = e.to_string();
                    *metrics.fallback_reasons.entry(reason).or_insert(0) += 1;
                }
                
                self.calculate_fallback(data).await
            }
        }
    }
    
    /// Get current fallback metrics
    pub async fn get_metrics(&self) -> FallbackMetrics {
        let metrics = self.metrics.read().await;
        FallbackMetrics {
            total_activations: metrics.total_activations,
            last_activation: metrics.last_activation,
            fallback_reasons: metrics.fallback_reasons.clone(),
        }
    }
    
    /// Check if fallback is currently enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
    
    /// Get total fallback count
    pub fn get_total_fallbacks(&self) -> u64 {
        self.total_fallbacks.load(Ordering::Relaxed)
    }
}

/// Simple Moving Average calculator for fallback predictions
pub struct SimpleMovingAverage {
    window_size: usize,
}

impl SimpleMovingAverage {
    pub fn new(window_size: usize) -> Self {
        Self { window_size }
    }
    
    pub fn calculate(&self, data: &[f64]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }
        
        let window = self.window_size.min(data.len());
        let sum: f64 = data.iter().rev().take(window).sum();
        sum / window as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_fallback_activation() {
        let fallback = EmergencyFallbackSystem::new(5);
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        
        let result = fallback.calculate_fallback(&data).await.unwrap();
        assert_eq!(result, 3.0);
        
        // Verify metrics updated
        let metrics = fallback.get_metrics().await;
        assert_eq!(metrics.total_activations, 1);
        assert!(metrics.last_activation.is_some());
    }
    
    #[tokio::test]
    async fn test_fallback_metrics_tracking() {
        let fallback = EmergencyFallbackSystem::new(5);
        
        // Activate multiple times
        for i in 0..5 {
            fallback.calculate_fallback(&vec![i as f64]).await.unwrap();
        }
        
        let metrics = fallback.get_metrics().await;
        assert_eq!(metrics.total_activations, 5);
        assert_eq!(fallback.get_total_fallbacks(), 5);
    }
    
    #[tokio::test]
    async fn test_predict_with_fallback() {
        let fallback = EmergencyFallbackSystem::new(3);
        let data = vec![10.0, 20.0, 30.0];
        
        // Test with failing neural prediction
        let result = fallback.predict_with_fallback(
            "TEST",
            &data,
            || async { Err(anyhow::anyhow!("Neural model failed")) }
        ).await.unwrap();
        
        assert_eq!(result, 20.0); // Average of last 3 values
        assert!(fallback.is_enabled());
    }
}