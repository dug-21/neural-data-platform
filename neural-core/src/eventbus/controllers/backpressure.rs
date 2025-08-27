use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::eventbus::error::EventBusError;

/// Backpressure controller for managing channel load and throttling
pub struct BackpressureController {
    channel_limits: Arc<RwLock<HashMap<String, ChannelLimits>>>,
    current_metrics: Arc<RwLock<HashMap<String, ChannelMetrics>>>,
    throttle_states: Arc<RwLock<HashMap<String, ThrottleState>>>,
}

#[derive(Debug, Clone)]
pub struct ChannelLimits {
    pub max_pending_messages: usize,
    pub max_memory_mb: usize,
    pub max_consumer_lag_ms: u64,
    pub warning_threshold: f64,  // 0.0 to 1.0
    pub critical_threshold: f64, // 0.0 to 1.0
}

impl Default for ChannelLimits {
    fn default() -> Self {
        Self {
            max_pending_messages: 10000,
            max_memory_mb: 100,
            max_consumer_lag_ms: 5000,
            warning_threshold: 0.7,
            critical_threshold: 0.9,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChannelMetrics {
    pub pending_messages: usize,
    pub memory_mb: usize,
    pub consumer_lag_ms: u64,
    pub message_rate_per_sec: f64,
    pub error_rate: f64,
}

#[derive(Debug, Clone)]
pub struct ThrottleState {
    pub rate_limit: f64,  // 0.0 to 1.0 (1.0 = no throttle)
    pub batch_size_multiplier: f64,
    pub consumer_scaling_factor: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackpressureStatus {
    Normal,
    Warning,
    Critical,
}

impl BackpressureController {
    pub fn new() -> Self {
        Self {
            channel_limits: Arc::new(RwLock::new(HashMap::new())),
            current_metrics: Arc::new(RwLock::new(HashMap::new())),
            throttle_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set_channel_limits(&self, channel: &str, limits: ChannelLimits) {
        let mut channel_limits = self.channel_limits.write().await;
        channel_limits.insert(channel.to_string(), limits);
    }

    pub async fn update_metrics(&self, channel: &str, metrics: ChannelMetrics) {
        let mut current_metrics = self.current_metrics.write().await;
        current_metrics.insert(channel.to_string(), metrics);
    }

    pub async fn check_pressure(&self, channel: &str) -> Result<BackpressureStatus, EventBusError> {
        let limits = self.get_limits(channel).await;
        let metrics = self.get_metrics(channel).await?;
        
        let pressure = self.calculate_pressure(&limits, &metrics);
        
        let status = if pressure >= limits.critical_threshold {
            self.apply_critical_throttling(channel).await?;
            BackpressureStatus::Critical
        } else if pressure >= limits.warning_threshold {
            self.apply_warning_throttling(channel).await?;
            BackpressureStatus::Warning
        } else {
            self.clear_throttling(channel).await?;
            BackpressureStatus::Normal
        };
        
        Ok(status)
    }

    async fn get_limits(&self, channel: &str) -> ChannelLimits {
        let limits = self.channel_limits.read().await;
        limits.get(channel).cloned().unwrap_or_default()
    }

    async fn get_metrics(&self, channel: &str) -> Result<ChannelMetrics, EventBusError> {
        let metrics = self.current_metrics.read().await;
        metrics.get(channel).cloned().ok_or_else(|| {
            EventBusError::Internal(format!("No metrics available for channel {}", channel))
        })
    }

    fn calculate_pressure(&self, limits: &ChannelLimits, metrics: &ChannelMetrics) -> f64 {
        let message_pressure = metrics.pending_messages as f64 / limits.max_pending_messages as f64;
        let memory_pressure = metrics.memory_mb as f64 / limits.max_memory_mb as f64;
        let lag_pressure = metrics.consumer_lag_ms as f64 / limits.max_consumer_lag_ms as f64;
        
        // Return the maximum pressure value
        message_pressure.max(memory_pressure).max(lag_pressure)
    }

    async fn apply_critical_throttling(&self, channel: &str) -> Result<(), EventBusError> {
        warn!("Applying critical throttling to channel: {}", channel);
        
        let mut throttle_states = self.throttle_states.write().await;
        throttle_states.insert(
            channel.to_string(),
            ThrottleState {
                rate_limit: 0.25,  // Reduce to 25% of normal rate
                batch_size_multiplier: 4.0,  // Increase batch size
                consumer_scaling_factor: 2.0,  // Double consumers
            },
        );
        
        Ok(())
    }

    async fn apply_warning_throttling(&self, channel: &str) -> Result<(), EventBusError> {
        info!("Applying warning throttling to channel: {}", channel);
        
        let mut throttle_states = self.throttle_states.write().await;
        throttle_states.insert(
            channel.to_string(),
            ThrottleState {
                rate_limit: 0.75,  // Reduce to 75% of normal rate
                batch_size_multiplier: 2.0,  // Double batch size
                consumer_scaling_factor: 1.0,  // No change in consumers
            },
        );
        
        Ok(())
    }

    async fn clear_throttling(&self, channel: &str) -> Result<(), EventBusError> {
        let mut throttle_states = self.throttle_states.write().await;
        throttle_states.remove(channel);
        Ok(())
    }

    pub async fn get_throttle_state(&self, channel: &str) -> Option<ThrottleState> {
        let throttle_states = self.throttle_states.read().await;
        throttle_states.get(channel).cloned()
    }

    pub async fn should_throttle(&self, channel: &str) -> bool {
        let throttle_states = self.throttle_states.read().await;
        throttle_states.contains_key(channel)
    }

    pub async fn get_rate_limit(&self, channel: &str) -> f64 {
        let throttle_states = self.throttle_states.read().await;
        throttle_states
            .get(channel)
            .map(|state| state.rate_limit)
            .unwrap_or(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_backpressure_normal() {
        let controller = BackpressureController::new();
        
        controller.set_channel_limits(
            "stream:symbol:AAPL",
            ChannelLimits {
                max_pending_messages: 1000,
                max_memory_mb: 50,
                max_consumer_lag_ms: 1000,
                warning_threshold: 0.7,
                critical_threshold: 0.9,
            },
        ).await;
        
        controller.update_metrics(
            "stream:symbol:AAPL",
            ChannelMetrics {
                pending_messages: 100,  // 10% of limit
                memory_mb: 5,  // 10% of limit
                consumer_lag_ms: 100,  // 10% of limit
                message_rate_per_sec: 100.0,
                error_rate: 0.01,
            },
        ).await;
        
        let status = controller.check_pressure("stream:symbol:AAPL").await.unwrap();
        assert_eq!(status, BackpressureStatus::Normal);
        assert!(!controller.should_throttle("stream:symbol:AAPL").await);
    }

    #[tokio::test]
    async fn test_backpressure_warning() {
        let controller = BackpressureController::new();
        
        controller.set_channel_limits(
            "stream:symbol:AAPL",
            ChannelLimits::default(),
        ).await;
        
        controller.update_metrics(
            "stream:symbol:AAPL",
            ChannelMetrics {
                pending_messages: 7500,  // 75% of limit
                memory_mb: 10,
                consumer_lag_ms: 100,
                message_rate_per_sec: 100.0,
                error_rate: 0.01,
            },
        ).await;
        
        let status = controller.check_pressure("stream:symbol:AAPL").await.unwrap();
        assert_eq!(status, BackpressureStatus::Warning);
        assert!(controller.should_throttle("stream:symbol:AAPL").await);
        assert_eq!(controller.get_rate_limit("stream:symbol:AAPL").await, 0.75);
    }

    #[tokio::test]
    async fn test_backpressure_critical() {
        let controller = BackpressureController::new();
        
        controller.set_channel_limits(
            "stream:symbol:AAPL",
            ChannelLimits::default(),
        ).await;
        
        controller.update_metrics(
            "stream:symbol:AAPL",
            ChannelMetrics {
                pending_messages: 9500,  // 95% of limit
                memory_mb: 10,
                consumer_lag_ms: 100,
                message_rate_per_sec: 100.0,
                error_rate: 0.01,
            },
        ).await;
        
        let status = controller.check_pressure("stream:symbol:AAPL").await.unwrap();
        assert_eq!(status, BackpressureStatus::Critical);
        assert!(controller.should_throttle("stream:symbol:AAPL").await);
        assert_eq!(controller.get_rate_limit("stream:symbol:AAPL").await, 0.25);
    }
}