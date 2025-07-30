//! Performance Monitoring Module
//!
//! Comprehensive performance monitoring system with event bus, metrics collection,
//! aggregation, export, and intelligent notifications for autonomous training decisions.
//!
//! ARCHITECTURE:
//! ```
//! PerformanceChannel (Event Bus) 
//!       ↓
//! MetricsCollector (Raw data collection)
//!       ↓  
//! MetricsAggregator (Statistical analysis)
//!       ↓
//! MetricsExporter (External systems)
//!
//! PerformanceChannel → TrainingNotifier (Intelligent training triggers)
//! ```

pub mod performance_channel;
pub mod metrics;
pub mod notifications;

pub use performance_channel::{
    PerformanceChannel, PerformanceEvent, PerformanceEventBuilder, PerformanceEventType,
    PerformanceSource, PerformanceMetrics, EventPriority, AlertType, AlertSeverity,
    ComponentType, ChannelConfig, ChannelStatistics, CircularBuffer,
};

pub use metrics::{
    MetricsPipeline, MetricsPipelineConfig, MetricsCollector, MetricsAggregator, MetricsExporter,
    MetricPoint, MetricUnit, AggregatedDataPoint, AggregationType, ExportDestination, ExportFormat,
    CollectorConfig, MetricStatistics, AggregatorConfig, RealTimeStatistics, ExporterConfig, 
    ExportResult, ExportStatistics, CollectionStatistics,
};

pub use notifications::{
    NotificationSystem, NotificationSystemConfig, TrainingNotifier, TrainingNotification,
    TrainingTriggerReason, TrainingPriority, TrainingAction, TrainingThresholds,
};

use anyhow::Result;
use tokio::sync::{broadcast, mpsc};
use tracing::info;

/// Complete performance monitoring system configuration
#[derive(Debug, Clone)]
pub struct MonitoringConfig {
    pub channel: ChannelConfig,
    pub metrics_pipeline: MetricsPipelineConfig,
    pub notifications: NotificationSystemConfig,
    pub training_thresholds: TrainingThresholds,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            channel: ChannelConfig::default(),
            metrics_pipeline: MetricsPipelineConfig::default(),
            notifications: NotificationSystemConfig::default(),
            training_thresholds: TrainingThresholds::default(),
        }
    }
}

/// Performance monitoring statistics
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MonitoringStatistics {
    pub channel_stats: ChannelStatistics,
    pub collection_stats: Option<metrics::CollectionStatistics>,
    pub real_time_stats: Option<std::collections::HashMap<String, metrics::RealTimeStatistics>>,
    pub export_stats: Option<metrics::ExportStatistics>,
    pub notification_stats: Option<notifications::NotificationStats>,
}

/// Complete performance monitoring system
pub struct PerformanceMonitoringSystem {
    performance_channel: PerformanceChannel,
    metrics_pipeline: MetricsPipeline,
    notification_system: NotificationSystem,
    config: MonitoringConfig,
    
    // Communication channels
    training_notifications_rx: mpsc::Receiver<TrainingNotification>,
}

impl PerformanceMonitoringSystem {
    /// Create a new performance monitoring system
    pub fn new(config: MonitoringConfig) -> (Self, broadcast::Receiver<PerformanceEvent>) {
        // Create performance channel (event bus)
        let (performance_channel, performance_rx) = PerformanceChannel::new(config.channel.clone());
        
        // Create training notification channel
        let (training_tx, training_notifications_rx) = mpsc::channel(1000);
        
        // Create metrics pipeline with performance events
        let performance_rx_for_metrics = performance_channel.subscribe();
        let (performance_events_tx, performance_events_rx) = mpsc::unbounded_channel();
        
        // Spawn task to bridge broadcast to unbounded channel for metrics
        let bridge_tx = performance_events_tx.clone();
        tokio::spawn(async move {
            let mut rx = performance_rx_for_metrics;
            while let Ok(event) = rx.recv().await {
                if bridge_tx.send(event).is_err() {
                    break;
                }
            }
        });
        
        let metrics_pipeline = MetricsPipeline::new(
            config.metrics_pipeline.clone(),
            performance_events_rx,
        );
        
        // Create notification system
        let notification_system = NotificationSystem::new(
            config.notifications.clone(),
            performance_channel.subscribe(),
            training_tx,
            config.training_thresholds.clone(),
        );
        
        let system = Self {
            performance_channel,
            metrics_pipeline,
            notification_system,
            config,
            training_notifications_rx,
        };
        
        (system, performance_rx)
    }

    /// Start the complete monitoring system
    pub async fn start(mut self) -> Result<()> {
        info!("Starting Performance Monitoring System");
        
        // Start all components concurrently
        let mut tasks = Vec::new();
        
        // Extract components to move into tasks
        let metrics_pipeline = self.metrics_pipeline;
        let notification_system = self.notification_system;
        let mut training_notifications_rx = self.training_notifications_rx;
        
        // Start metrics pipeline
        tasks.push(tokio::spawn(async move {
            if let Err(e) = metrics_pipeline.start().await {
                tracing::error!("Metrics pipeline failed: {}", e);
            }
        }));
        
        // Start notification system
        tasks.push(tokio::spawn(async move {
            if let Err(e) = notification_system.start().await {
                tracing::error!("Notification system failed: {}", e);
            }
        }));
        
        // Process training notifications
        tasks.push(tokio::spawn(async move {
            info!("Starting training notification processing");
            
            while let Some(notification) = training_notifications_rx.recv().await {
                // Log the notification
                match notification.priority {
                    TrainingPriority::Critical => {
                        tracing::error!("CRITICAL Training Notification: {:?} for model {}", 
                            notification.trigger_reason, notification.model_type);
                    }
                    TrainingPriority::High => {
                        tracing::warn!("HIGH Priority Training Notification: {:?} for model {}", 
                            notification.trigger_reason, notification.model_type);
                    }
                    TrainingPriority::Medium => {
                        tracing::info!("Training Notification: {:?} for model {}", 
                            notification.trigger_reason, notification.model_type);
                    }
                    TrainingPriority::Low => {
                        tracing::debug!("Low Priority Training Notification: {:?} for model {}", 
                            notification.trigger_reason, notification.model_type);
                    }
                }

                // Here you would integrate with actual training system
                info!("Recommended training action: {:?}", notification.recommended_action);
            }
        }));
        
        // Wait for all tasks
        for task in tasks {
            if let Err(e) = task.await {
                tracing::error!("Monitoring system task failed: {}", e);
            }
        }
        
        Ok(())
    }

    /// Get performance channel for event emission
    pub fn get_performance_channel(&self) -> &PerformanceChannel {
        &self.performance_channel
    }

    /// Subscribe to performance events
    pub fn subscribe_to_events(&self) -> broadcast::Receiver<PerformanceEvent> {
        self.performance_channel.subscribe()
    }

    /// Emit a performance event
    pub async fn emit_event(&self, event: PerformanceEvent) -> Result<()> {
        self.performance_channel.emit(event).await
    }

    /// Emit event with fire-and-forget semantics (maximum performance)
    pub fn emit_event_fast(&self, event: PerformanceEvent) {
        self.performance_channel.emit_fast(event);
    }

    /// Get comprehensive monitoring statistics
    pub async fn get_statistics(&self) -> Result<MonitoringStatistics> {
        Ok(MonitoringStatistics {
            channel_stats: self.performance_channel.get_statistics()?,
            collection_stats: self.metrics_pipeline.get_collection_statistics().await,
            real_time_stats: self.metrics_pipeline.get_real_time_statistics().await,
            export_stats: self.metrics_pipeline.get_export_statistics().await,
            notification_stats: self.notification_system.get_training_stats().await,
        })
    }

    /// Force immediate metrics export
    pub async fn force_metrics_export(&self) -> Result<()> {
        self.metrics_pipeline.force_export().await
    }

    /// Get recent performance events
    pub fn get_recent_events(&self, count: usize) -> Vec<PerformanceEvent> {
        self.performance_channel.get_recent_metrics(count)
    }

    /// Clear performance event buffer
    pub fn clear_event_buffer(&self) -> Result<()> {
        self.performance_channel.clear_buffer()
    }

}

/// Builder for easy monitoring system setup
pub struct MonitoringSystemBuilder {
    config: MonitoringConfig,
}

impl MonitoringSystemBuilder {
    pub fn new() -> Self {
        Self {
            config: MonitoringConfig::default(),
        }
    }

    pub fn with_channel_config(mut self, config: ChannelConfig) -> Self {
        self.config.channel = config;
        self
    }

    pub fn with_metrics_config(mut self, config: MetricsPipelineConfig) -> Self {
        self.config.metrics_pipeline = config;
        self
    }

    pub fn with_notification_config(mut self, config: NotificationSystemConfig) -> Self {
        self.config.notifications = config;
        self
    }

    pub fn with_training_thresholds(mut self, thresholds: TrainingThresholds) -> Self {
        self.config.training_thresholds = thresholds;
        self
    }

    pub fn build(self) -> (PerformanceMonitoringSystem, broadcast::Receiver<PerformanceEvent>) {
        PerformanceMonitoringSystem::new(self.config)
    }
}

impl Default for MonitoringSystemBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitoring_system_creation() {
        let (system, _rx) = MonitoringSystemBuilder::new().build();
        assert!(system.config.channel.buffer_size > 0);
    }

    #[tokio::test]
    async fn test_event_emission() {
        let (system, mut rx) = MonitoringSystemBuilder::new().build();
        
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "test".to_string(),
                predictor_id: "pred1".to_string(),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                model: "test".to_string(),
                accuracy: 0.95,
                confidence: 0.9,
                latency_ms: 100,
                input_features: 10,
                output_dimension: 1,
                timestamp: chrono::Utc::now(),
            })
            .build()
            .unwrap();
        
        system.emit_event_fast(event.clone());
        
        // Check that event was received
        let received = rx.try_recv().unwrap();
        assert_eq!(received.id, event.id);
    }
}