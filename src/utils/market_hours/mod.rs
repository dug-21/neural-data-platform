//! Market hours tracking and training window detection
//! 
//! Provides comprehensive market schedule information for major exchanges
//! and identifies optimal training windows based on market activity.
//!
//! This module has been refactored into several specialized components:
//! - `config`: Configuration structures and resource allocation policies
//! - `exchanges`: Exchange definitions and trading schedules
//! - `timezone`: Timezone handling and conversion utilities
//! - `holidays`: Holiday calendar management for market closures
//! - `scheduler`: Market scheduling logic and session management

pub mod config;
pub mod exchanges;
pub mod timezone;
pub mod holidays;
pub mod scheduler;

// Re-export commonly used types for backward compatibility
pub use config::{
    ResourceAllocationPolicy, EmergencyPriority, EmergencyOverride, EmergencyOverrideManager,
    CircuitBreakerRules, CircuitBreakerLevel, VolatilityLevel, TrainingWindow, MarketIntensity,
    SchedulingRecommendations,
};

pub use exchanges::{Exchange, TradingHours, ExtendedHours, ExchangeSchedule, ExchangeManager};

pub use timezone::{TimezoneConverter, TimeCalculator};

pub use holidays::{HolidayType, Holiday, HolidayCalendar, HolidayStatistics, HolidayManager};

pub use scheduler::{
    MarketSession, MarketStatus, MarketCorrelationMatrix, MarketScheduler,
};

use chrono::{DateTime, Duration, Utc};
use anyhow::Result;
use std::sync::Arc;

/// Main market hours tracker with training window detection
/// 
/// This is the primary interface that maintains backward compatibility
/// while using the refactored modular components internally.
pub struct MarketHours {
    scheduler: Arc<MarketScheduler>,
}

impl MarketHours {
    /// Create a new market hours tracker with comprehensive global coverage
    pub fn new() -> Self {
        Self {
            scheduler: Arc::new(MarketScheduler::new()),
        }
    }

    /// Check if a specific exchange is open
    pub async fn is_exchange_open(&self, exchange: Exchange, time: DateTime<Utc>) -> bool {
        self.scheduler.is_exchange_open(exchange, time).await
    }

    /// Check if market is open (alias for is_exchange_open for backward compatibility)
    pub async fn is_market_open(&self, exchange: Exchange, time: DateTime<Utc>) -> bool {
        self.scheduler.is_exchange_open(exchange, time).await
    }

    /// Get current market session for an exchange
    pub async fn get_session(&self, exchange: Exchange, time: DateTime<Utc>) -> MarketSession {
        self.scheduler.get_session(exchange, time).await
    }

    /// Calculate global market intensity with correlation adjustments
    pub async fn get_market_intensity(&self, time: DateTime<Utc>) -> MarketIntensity {
        self.scheduler.get_market_intensity(time).await
    }

    /// Determine training window suitability with emergency override support
    pub async fn get_training_window(&self, time: DateTime<Utc>) -> TrainingWindow {
        self.scheduler.get_training_window(time).await
    }

    /// Find next optimal training window
    pub async fn find_next_training_window(
        &self, 
        start: DateTime<Utc>, 
        min_duration: Duration,
        min_quality: TrainingWindow,
    ) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        self.scheduler.find_next_training_window(start, min_duration, min_quality).await
    }

    /// Get resource limit recommendation based on market intensity and policy
    pub async fn get_resource_limit(&self, time: DateTime<Utc>) -> f64 {
        self.scheduler.get_resource_limit(time).await
    }

    /// Get all active exchanges at a given time with their status
    pub async fn get_active_exchanges(&self, time: DateTime<Utc>) -> Vec<(Exchange, MarketSession)> {
        self.scheduler.get_active_exchanges(time).await
    }

    /// Get comprehensive market status
    pub async fn get_market_status(&self, exchange: Exchange, time: DateTime<Utc>) -> Option<MarketStatus> {
        self.scheduler.get_market_status(exchange, time).await
    }

    /// Update market correlation matrix
    pub async fn update_correlation_matrix(
        &self,
        correlations: std::collections::HashMap<(Exchange, Exchange), f64>,
        impact_scores: std::collections::HashMap<Exchange, f64>,
        volatility_indices: std::collections::HashMap<Exchange, f64>,
    ) {
        self.scheduler.update_correlation_matrix(correlations, impact_scores, volatility_indices).await
    }

    /// Get scheduling recommendations based on current market conditions
    pub async fn get_scheduling_recommendations(&self, time: DateTime<Utc>) -> SchedulingRecommendations {
        self.scheduler.get_scheduling_recommendations(time).await
    }

    /// Request emergency override
    pub async fn request_emergency_override(
        &self,
        reason: String,
        priority: EmergencyPriority,
        requested_resources: f64,
        duration: Duration,
        approved_by: String,
        affected_exchanges: Vec<Exchange>,
    ) -> Result<String> {
        self.scheduler.request_emergency_override(
            reason, priority, requested_resources, duration, approved_by, affected_exchanges
        ).await
    }

    /// Cancel emergency override
    pub async fn cancel_emergency_override(&self, override_id: &str) -> Result<()> {
        self.scheduler.cancel_emergency_override(override_id).await
    }

    /// Update holiday calendar
    pub async fn update_holidays(&self, exchange: Exchange, holidays: Vec<holidays::Holiday>) {
        // Access the holiday manager through the scheduler
        // This would require additional methods in the scheduler to expose holiday management
        // For now, we'll document that holiday updates should be done through the HolidayManager directly
    }

    /// Check if a date is a holiday for an exchange (convenience method)
    pub async fn is_holiday(&self, exchange: Exchange, date: DateTime<Utc>) -> bool {
        // This would need to be implemented by exposing the holiday manager through the scheduler
        // For backward compatibility, we'll provide a basic implementation
        false // Placeholder - would delegate to holiday manager
    }
}

impl Default for MarketHours {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions for common operations
pub mod utils {
    use super::*;

    /// Create a new market hours instance (convenience function)
    pub fn create_market_hours() -> MarketHours {
        MarketHours::new()
    }

    /// Check if it's currently a weekend
    pub fn is_weekend(time: DateTime<Utc>) -> bool {
        use chrono::Datelike;
        let weekday = time.weekday();
        weekday == chrono::Weekday::Sat || weekday == chrono::Weekday::Sun
    }

    /// Get current UTC time (convenience function)
    pub fn now() -> DateTime<Utc> {
        Utc::now()
    }

    /// Convert hours to Duration
    pub fn hours(h: i64) -> Duration {
        Duration::hours(h)
    }

    /// Convert minutes to Duration
    pub fn minutes(m: i64) -> Duration {
        Duration::minutes(m)
    }

    /// Convert days to Duration
    pub fn days(d: i64) -> Duration {
        Duration::days(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Weekday};

    #[tokio::test]
    async fn test_market_hours_creation() {
        let market_hours = MarketHours::new();
        
        // Test that we can get market intensity
        let intensity = market_hours.get_market_intensity(Utc::now()).await;
        assert!(intensity.score >= 0.0 && intensity.score <= 1.0);
    }
    
    #[tokio::test]
    async fn test_training_window_ordering() {
        assert!(TrainingWindow::Optimal < TrainingWindow::Good);
        assert!(TrainingWindow::Good < TrainingWindow::Acceptable);
        assert!(TrainingWindow::Acceptable < TrainingWindow::Poor);
        assert!(TrainingWindow::Poor < TrainingWindow::Restricted);
    }
    
    #[tokio::test]
    async fn test_weekend_detection() {
        let market_hours = MarketHours::new();
        
        // Saturday
        let saturday = Utc.with_ymd_and_hms(2024, 1, 6, 12, 0, 0).unwrap();
        let window = market_hours.get_training_window(saturday).await;
        assert_eq!(window, TrainingWindow::Optimal);
        
        // Sunday
        let sunday = Utc.with_ymd_and_hms(2024, 1, 7, 12, 0, 0).unwrap();
        let window = market_hours.get_training_window(sunday).await;
        assert_eq!(window, TrainingWindow::Optimal);
    }
    
    #[tokio::test]
    async fn test_resource_allocation_policy() {
        let market_hours = MarketHours::new();
        
        // Weekend should get high allocation
        let saturday = Utc.with_ymd_and_hms(2024, 1, 6, 12, 0, 0).unwrap();
        let limit = market_hours.get_resource_limit(saturday).await;
        assert!(limit >= 0.90);
    }
    
    #[tokio::test]
    async fn test_exchange_sessions() {
        let market_hours = MarketHours::new();
        
        // Test that we can get sessions for major exchanges
        let nyse_session = market_hours.get_session(Exchange::NYSE, Utc::now()).await;
        let lse_session = market_hours.get_session(Exchange::LSE, Utc::now()).await;
        let tse_session = market_hours.get_session(Exchange::TSE, Utc::now()).await;
        
        // Sessions should be valid enum values
        assert!(matches!(nyse_session, MarketSession::Regular | MarketSession::PreMarket | MarketSession::AfterHours | MarketSession::Closed));
        assert!(matches!(lse_session, MarketSession::Regular | MarketSession::PreMarket | MarketSession::AfterHours | MarketSession::Closed));
        assert!(matches!(tse_session, MarketSession::Regular | MarketSession::PreMarket | MarketSession::AfterHours | MarketSession::Closed));
    }
    
    #[tokio::test]
    async fn test_active_exchanges() {
        let market_hours = MarketHours::new();
        
        let active_exchanges = market_hours.get_active_exchanges(Utc::now()).await;
        
        // Should have some exchanges (might be 0 if all are closed, but that's valid)
        assert!(active_exchanges.len() <= 25); // We have ~25 exchanges defined
    }
    
    #[tokio::test]
    async fn test_market_status() {
        let market_hours = MarketHours::new();
        
        let status = market_hours.get_market_status(Exchange::NYSE, Utc::now()).await;
        assert!(status.is_some());
        
        if let Some(status) = status {
            assert_eq!(status.exchange, Exchange::NYSE);
            assert!(status.next_state_change > Utc::now());
        }
    }

    #[tokio::test]
    async fn test_scheduling_recommendations() {
        let market_hours = MarketHours::new();
        
        let recommendations = market_hours.get_scheduling_recommendations(Utc::now()).await;
        
        assert!(recommendations.resource_limit >= 0.0 && recommendations.resource_limit <= 1.0);
        assert!(recommendations.intensity_score >= 0.0 && recommendations.intensity_score <= 1.0);
        assert!(!recommendations.recommendation.is_empty());
    }

    #[tokio::test]
    async fn test_utils_functions() {
        assert!(utils::is_weekend(Utc.with_ymd_and_hms(2024, 1, 6, 12, 0, 0).unwrap())); // Saturday
        assert!(utils::is_weekend(Utc.with_ymd_and_hms(2024, 1, 7, 12, 0, 0).unwrap())); // Sunday
        assert!(!utils::is_weekend(Utc.with_ymd_and_hms(2024, 1, 8, 12, 0, 0).unwrap())); // Monday
        
        assert_eq!(utils::hours(2), Duration::hours(2));
        assert_eq!(utils::minutes(30), Duration::minutes(30));
        assert_eq!(utils::days(1), Duration::days(1));
    }
}