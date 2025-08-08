/*!
 * Tests for multi-channel Redis subscription system
 */

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_fair_processing_scheduler_creation() {
        let scheduler = FairProcessingScheduler::new(
            Duration::from_secs(60),
            0.20 // 20% max
        );
        
        assert_eq!(scheduler.max_symbol_percentage, 0.20);
        assert_eq!(scheduler.fairness_window, Duration::from_secs(60));
    }

    #[tokio::test]
    async fn test_fair_processing_scheduler_throttling() {
        let mut scheduler = FairProcessingScheduler::new(
            Duration::from_millis(100), // Short window for testing
            0.20 // 20% max
        );

        // Simulate processing that would exceed the limit
        scheduler.record_processing_completion("NVDA", Duration::from_millis(50));
        scheduler.record_processing_completion("AAPL", Duration::from_millis(10));

        // NVDA should be throttled (50ms out of 60ms = 83% > 20%)
        assert!(!scheduler.should_process("NVDA"));
        // AAPL should still be allowed
        assert!(scheduler.should_process("AAPL"));
    }

    #[tokio::test]
    async fn test_fair_processing_compliance_rate() {
        let mut scheduler = FairProcessingScheduler::new(
            Duration::from_secs(60),
            0.20
        );

        // All processing within limits - should have 100% compliance
        scheduler.record_processing_completion("AAPL", Duration::from_millis(10));
        scheduler.record_processing_completion("MSFT", Duration::from_millis(10));
        scheduler.record_processing_completion("GOOGL", Duration::from_millis(10));
        
        let compliance_rate = scheduler.get_compliance_rate();
        assert!(compliance_rate > 0.99); // Should be close to 100%
    }

    #[tokio::test]
    async fn test_multi_channel_config_defaults() {
        let config = MultiChannelConfig::default();
        
        assert_eq!(config.max_symbol_percentage, 0.20);
        assert_eq!(config.fairness_window_seconds, 60);
        assert_eq!(config.worker_queue_size, 1000);
        assert!(config.enabled_symbols.contains(&"AAPL".to_string()));
        assert!(config.enabled_symbols.contains(&"NVDA".to_string()));
    }

    #[tokio::test]
    async fn test_work_item_creation() {
        use crate::adapters::MarketData;
        
        let market_data = MarketData {
            symbol: "AAPL".to_string(),
            timestamp: 1000000000,
            open: 150.0,
            high: 152.0,
            low: 149.0,
            close: 151.0,
            volume: 1000,
        };

        let work_item = WorkItem {
            symbol: "AAPL".to_string(),
            channel: "market:AAPL".to_string(),
            market_data,
            received_at: std::time::Instant::now(),
            priority: 1.0,
        };

        assert_eq!(work_item.symbol, "AAPL");
        assert_eq!(work_item.channel, "market:AAPL");
        assert_eq!(work_item.priority, 1.0);
    }

    #[tokio::test]
    async fn test_symbol_stats_initialization() {
        let stats = SymbolStats::default();
        
        assert_eq!(stats.messages_processed, 0);
        assert_eq!(stats.total_processing_time, Duration::ZERO);
        assert_eq!(stats.throttle_count, 0);
        assert!(stats.last_processed.is_none());
    }

    #[tokio::test]
    async fn test_processing_priority_conversion() {
        let high_priority: f64 = ProcessingPriority::High.into();
        let normal_priority: f64 = ProcessingPriority::Normal.into();
        let low_priority: f64 = ProcessingPriority::Low.into();
        let throttled_priority: f64 = ProcessingPriority::Throttled.into();

        assert_eq!(high_priority, 3.0);
        assert_eq!(normal_priority, 2.0);
        assert_eq!(low_priority, 1.0);
        assert_eq!(throttled_priority, 0.0);
    }

    #[tokio::test]
    async fn test_time_window_functionality() {
        use crate::multi_channel::fair_scheduler::TimeWindow;
        
        let mut window = TimeWindow::new(Duration::from_secs(1));
        
        // Add some processing time
        window.add_processing_time(Duration::from_millis(100));
        assert_eq!(window.processing_time, Duration::from_millis(100));
        
        // Test percentage calculation
        let percentage = window.get_percentage(Duration::from_millis(500));
        assert!((percentage - 0.2).abs() < 0.01); // Should be ~20%
        
        // Test reset
        window.reset();
        assert_eq!(window.processing_time, Duration::ZERO);
    }
}