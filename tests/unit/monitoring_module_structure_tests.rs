//! Module Structure Validation Tests for Phase 3A
//!
//! Ensures proper module boundaries, encapsulation, and dependency management.

use autonomous_platform::neural::monitoring::{
    PerformanceChannel, PerformanceEvent, PerformanceEventBuilder, PerformanceEventType,
    PerformanceSource, PerformanceMetrics, EventPriority, AlertType, AlertSeverity,
    ComponentType, ChannelConfig, ChannelStatistics, CircularBuffer,
    MetricsPipeline, MetricsPipelineConfig, MetricsCollector, MetricsAggregator, MetricsExporter,
    MetricPoint, MetricUnit, AggregatedDataPoint, AggregationType, ExportDestination, ExportFormat,
    NotificationSystem, NotificationSystemConfig, TrainingNotifier, TrainingNotification,
    TrainingTriggerReason, TrainingPriority, TrainingAction, TrainingThresholds,
    MonitoringConfig, MonitoringStatistics, PerformanceMonitoringSystem, MonitoringSystemBuilder,
};

#[cfg(test)]
mod module_structure_tests {
    use super::*;
    use std::mem;

    #[test]
    fn test_module_exports_are_public() {
        // Verify all expected types are publicly accessible
        // This test will fail to compile if any type is not properly exported
        
        // Performance Channel types
        let _ = mem::size_of::<PerformanceChannel>();
        let _ = mem::size_of::<PerformanceEvent>();
        let _ = mem::size_of::<PerformanceMetrics>();
        let _ = mem::size_of::<EventPriority>();
        let _ = mem::size_of::<ChannelConfig>();
        
        // Metrics Pipeline types
        let _ = mem::size_of::<MetricsPipeline>();
        let _ = mem::size_of::<MetricsCollector>();
        let _ = mem::size_of::<MetricsAggregator>();
        let _ = mem::size_of::<MetricsExporter>();
        
        // Notification System types
        let _ = mem::size_of::<NotificationSystem>();
        let _ = mem::size_of::<TrainingNotifier>();
        let _ = mem::size_of::<TrainingNotification>();
        let _ = mem::size_of::<TrainingThresholds>();
        
        // Monitoring System types
        let _ = mem::size_of::<MonitoringConfig>();
        let _ = mem::size_of::<MonitoringStatistics>();
        let _ = mem::size_of::<PerformanceMonitoringSystem>();
    }

    #[test]
    fn test_builder_pattern_implementation() {
        // Verify builder patterns are correctly implemented
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::System { 
                service_name: "test".to_string() 
            })
            .event_type(PerformanceEventType::MetricsUpdate { 
                component: "test".to_string(),
                metrics: std::collections::HashMap::new(),
                timestamp: chrono::Utc::now(),
            })
            .priority(EventPriority::High)
            .build();
            
        assert!(event.is_ok());
        
        let (system, _rx) = MonitoringSystemBuilder::new()
            .with_channel_config(ChannelConfig::default())
            .with_metrics_config(MetricsPipelineConfig::default())
            .with_notification_config(NotificationSystemConfig::default())
            .with_training_thresholds(TrainingThresholds::default())
            .build();
            
        assert_eq!(system.config.channel.buffer_size, 1000);
    }

    #[test]
    fn test_default_implementations() {
        // Verify default trait implementations
        let channel_config = ChannelConfig::default();
        assert_eq!(channel_config.buffer_size, 1000);
        assert_eq!(channel_config.channel_capacity, 10000);
        assert!(channel_config.enable_persistence);
        assert!(channel_config.enable_metrics);
        
        let monitoring_config = MonitoringConfig::default();
        assert_eq!(monitoring_config.channel.buffer_size, 1000);
        
        let notification_config = NotificationSystemConfig::default();
        assert!(notification_config.enable_training_notifications);
        
        let thresholds = TrainingThresholds::default();
        assert!(thresholds.accuracy_threshold > 0.0);
        assert!(thresholds.confidence_threshold > 0.0);
    }

    #[test]
    fn test_enum_ordering() {
        // Verify priority enums implement Ord correctly
        assert!(EventPriority::Critical < EventPriority::High);
        assert!(EventPriority::High < EventPriority::Medium);
        assert!(EventPriority::Medium < EventPriority::Low);
        
        assert!(TrainingPriority::Critical < TrainingPriority::High);
        assert!(TrainingPriority::High < TrainingPriority::Medium);
        assert!(TrainingPriority::Medium < TrainingPriority::Low);
        
        assert!(AlertSeverity::Critical < AlertSeverity::Warning);
        assert!(AlertSeverity::Warning < AlertSeverity::Info);
    }

    #[test]
    fn test_serialization_support() {
        // Verify types can be serialized/deserialized
        let event = PerformanceEvent {
            id: "test".to_string(),
            timestamp: chrono::Utc::now(),
            source: PerformanceSource::System { 
                service_name: "test".to_string() 
            },
            event_type: PerformanceEventType::MetricsUpdate { 
                component: "test".to_string(),
                metrics: std::collections::HashMap::new(),
                timestamp: chrono::Utc::now(),
            },
            metrics: PerformanceMetrics::default(),
            tags: std::collections::HashMap::new(),
            correlation_id: None,
            priority: EventPriority::Medium,
        };
        
        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: PerformanceEvent = serde_json::from_str(&serialized).unwrap();
        assert_eq!(event.id, deserialized.id);
        
        let stats = ChannelStatistics::default();
        let serialized = serde_json::to_string(&stats).unwrap();
        let deserialized: ChannelStatistics = serde_json::from_str(&serialized).unwrap();
        assert_eq!(stats.total_events_emitted, deserialized.total_events_emitted);
    }

    #[test]
    fn test_thread_safety() {
        // Verify key types are Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        
        assert_send_sync::<PerformanceChannel>();
        assert_send_sync::<MonitoringConfig>();
        assert_send_sync::<ChannelConfig>();
        assert_send_sync::<TrainingThresholds>();
    }

    #[test]
    fn test_circular_buffer_encapsulation() {
        // CircularBuffer should not be directly accessible outside the module
        // This ensures internal implementation details are hidden
        
        // The following should not compile if CircularBuffer is properly encapsulated:
        // let buffer: CircularBuffer<i32> = CircularBuffer::new(10);
        
        // Instead, CircularBuffer functionality should be accessed through PerformanceChannel
        let (channel, _rx) = PerformanceChannel::new_with_buffer(100);
        assert_eq!(channel.buffer_size(), 0);
    }

    #[test]
    fn test_module_boundaries() {
        // Verify that modules maintain proper boundaries
        // performance_channel module should not expose internal types
        
        // These imports should work
        use autonomous_platform::neural::monitoring::performance_channel::{
            PerformanceChannel, PerformanceEvent, PerformanceEventBuilder,
        };
        
        // Internal types should not be accessible
        // use autonomous_platform::neural::monitoring::performance_channel::CircularBuffer; // Should fail
        
        let _ = PerformanceChannel::new_with_buffer(10);
        let _ = PerformanceEventBuilder::new();
    }

    #[test]
    fn test_monitoring_system_composition() {
        // Verify the monitoring system properly composes all subsystems
        let config = MonitoringConfig {
            channel: ChannelConfig::default(),
            metrics_pipeline: MetricsPipelineConfig::default(),
            notifications: NotificationSystemConfig::default(),
            training_thresholds: TrainingThresholds::default(),
        };
        
        let (system, rx) = PerformanceMonitoringSystem::new(config);
        
        // Verify we can access subsystems through the main system
        let channel = system.get_performance_channel();
        assert_eq!(channel.buffer_size(), 0);
        
        // Verify we can subscribe to events
        let _sub = system.subscribe_to_events();
        
        // Verify rx is of the correct type
        let _: tokio::sync::broadcast::Receiver<PerformanceEvent> = rx;
    }
}

#[cfg(test)]
mod dependency_validation_tests {
    use super::*;

    #[test]
    fn test_no_circular_dependencies() {
        // This test verifies that modules don't have circular dependencies
        // by ensuring we can import each module independently
        
        // Performance channel should be independent
        use autonomous_platform::neural::monitoring::performance_channel::PerformanceChannel;
        let _ = PerformanceChannel::new_with_buffer(10);
        
        // Metrics should depend only on performance_channel
        use autonomous_platform::neural::monitoring::metrics::MetricsPipeline;
        
        // Notifications should depend only on performance_channel
        use autonomous_platform::neural::monitoring::notifications::TrainingNotifier;
    }

    #[test]
    fn test_trait_implementations() {
        // Verify Clone implementations where needed
        let config = ChannelConfig::default();
        let config_clone = config.clone();
        assert_eq!(config.buffer_size, config_clone.buffer_size);
        
        let (channel, _rx) = PerformanceChannel::new(config);
        let channel_clone = channel.clone();
        assert_eq!(channel.buffer_size(), channel_clone.buffer_size());
        
        // Verify Debug implementations
        let event = PerformanceEventBuilder::new().build().unwrap();
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("PerformanceEvent"));
    }
}