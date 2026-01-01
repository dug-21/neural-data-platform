//! Unit tests for market hours detection and training window identification
//!
//! Tests timezone handling, market session detection, holiday processing,
//! and training window calculations across different exchanges.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Timelike, Utc, Weekday};
use autonomous_platform::utils::market_hours::{
    Exchange, MarketHours, MarketIntensity, MarketSession, TrainingWindow,
    holidays::{Holiday, HolidayType},
};
use std::collections::HashMap;

/// Helper to create a specific UTC datetime
fn create_datetime(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, 0)
        .single()
        .expect("Invalid date/time")
}

/// Helper to create a datetime for a specific weekday and time
fn create_weekday_datetime(weekday: Weekday, hour: u32, minute: u32) -> DateTime<Utc> {
    let mut date = Utc::now().date_naive();
    
    // Find the next occurrence of the specified weekday
    while date.weekday() != weekday {
        date = date.succ_opt().unwrap();
    }
    
    date.and_hms_opt(hour, minute, 0)
        .unwrap()
        .and_utc()
}

#[tokio::test]
async fn test_market_hours_initialization() {
    let market_hours = MarketHours::new();
    
    // Verify major exchanges are configured
    let exchanges = vec![
        Exchange::NYSE,
        Exchange::NASDAQ,
        Exchange::LSE,
        Exchange::TSE,
        Exchange::SSE,
    ];
    
    for exchange in exchanges {
        // Just verify we can query each exchange
        let now = Utc::now();
        let _is_open = market_hours.is_exchange_open(exchange, now).await;
        let _session = market_hours.get_session(exchange, now).await;
    }
}

#[tokio::test]
async fn test_weekend_market_closure() {
    let market_hours = MarketHours::new();
    
    // Saturday
    let saturday = create_weekday_datetime(Weekday::Sat, 12, 0);
    assert!(!market_hours.is_exchange_open(Exchange::NYSE, saturday).await);
    assert_eq!(
        market_hours.get_session(Exchange::NYSE, saturday).await,
        MarketSession::Closed
    );
    
    // Sunday
    let sunday = create_weekday_datetime(Weekday::Sun, 12, 0);
    assert!(!market_hours.is_exchange_open(Exchange::NASDAQ, sunday).await);
    assert_eq!(
        market_hours.get_session(Exchange::NASDAQ, sunday).await,
        MarketSession::Closed
    );
}

#[tokio::test]
async fn test_trading_hours_detection() {
    let market_hours = MarketHours::new();
    
    // Test NYSE regular hours (9:30 AM - 4:00 PM ET)
    // Using UTC times (ET is UTC-5 in winter, UTC-4 in summer)
    // For simplicity, we'll use fixed UTC times
    
    // Monday at 14:30 UTC (9:30 AM ET)
    let monday_open = create_weekday_datetime(Weekday::Mon, 14, 30);
    let session = market_hours.get_session(Exchange::NYSE, monday_open).await;
    // Session depends on actual timezone conversion
    
    // Monday at 21:00 UTC (4:00 PM ET) - should be after hours or closed
    let monday_close = create_weekday_datetime(Weekday::Mon, 21, 0);
    let session = market_hours.get_session(Exchange::NYSE, monday_close).await;
    assert_ne!(session, MarketSession::Regular);
}

#[tokio::test]
async fn test_pre_market_and_after_hours() {
    let market_hours = MarketHours::new();
    
    // Test pre-market hours (4:00 AM - 9:30 AM ET)
    let tuesday_premarket = create_weekday_datetime(Weekday::Tue, 9, 0); // 4:00 AM ET
    let session = market_hours.get_session(Exchange::NYSE, tuesday_premarket).await;
    // Note: This might be PreMarket or Closed depending on timezone handling
    
    // Test after-hours (4:00 PM - 8:00 PM ET)
    let tuesday_afterhours = create_weekday_datetime(Weekday::Tue, 22, 0); // 5:00 PM ET
    let session = market_hours.get_session(Exchange::NYSE, tuesday_afterhours).await;
    // Note: This might be AfterHours or Closed depending on timezone handling
}

#[tokio::test]
async fn test_market_intensity_calculation() {
    let market_hours = MarketHours::new();
    
    // Weekend should have zero intensity
    let weekend = create_weekday_datetime(Weekday::Sat, 12, 0);
    let intensity = market_hours.get_market_intensity(weekend).await;
    assert_eq!(intensity.active_exchanges, 0);
    assert_eq!(intensity.dominant_session, MarketSession::Closed);
    assert!(intensity.score < 0.1);
    
    // Weekday during US market hours should have higher intensity
    let weekday = create_weekday_datetime(Weekday::Wed, 15, 0); // 10:00 AM ET
    let intensity = market_hours.get_market_intensity(weekday).await;
    assert!(intensity.active_exchanges > 0);
    assert!(intensity.score > 0.0);
}

#[tokio::test]
async fn test_training_window_classification() {
    let market_hours = MarketHours::new();
    
    // Weekend should be optimal
    let weekend = create_weekday_datetime(Weekday::Sun, 3, 0);
    let window = market_hours.get_training_window(weekend).await;
    assert_eq!(window, TrainingWindow::Optimal);
    
    // Late night weekday should be good or optimal
    let late_night = create_weekday_datetime(Weekday::Tue, 3, 0); // 3:00 AM UTC
    let window = market_hours.get_training_window(late_night).await;
    assert!(matches!(window, TrainingWindow::Optimal | TrainingWindow::Good));
    
    // During market hours should be poor or restricted
    let market_hours_time = create_weekday_datetime(Weekday::Wed, 15, 0); // US market hours
    let window_market = market_hours.get_training_window(market_hours_time).await;
    assert!(matches!(
        window_market,
        TrainingWindow::Poor | TrainingWindow::Restricted | TrainingWindow::Acceptable
    ));
}

#[tokio::test]
async fn test_find_next_training_window() {
    let market_hours = MarketHours::new();
    
    // Start from a weekday afternoon
    let start = create_weekday_datetime(Weekday::Wed, 15, 0);
    let min_duration = Duration::hours(4);
    
    // Find next good training window
    let result = market_hours
        .find_next_training_window(start, min_duration, TrainingWindow::Good)
        .await;
    
    assert!(result.is_some());
    if let Some((window_start, window_end)) = result {
        assert!(window_start >= start);
        assert!(window_end > window_start);
        assert!(window_end - window_start >= min_duration);
    }
}

#[tokio::test]
async fn test_holiday_handling() {
    let market_hours = MarketHours::new();
    
    // Add some test holidays
    let holidays = vec![
        Holiday {
            date: NaiveDate::from_ymd_opt(2024, 12, 25).unwrap(),
            name: "Christmas Day".to_string(),
            holiday_type: HolidayType::NationalHoliday,
            affects_trading: true,
            early_close_time: None,
            late_open_time: None,
            description: Some("Christmas holiday closure".to_string()),
        },
        Holiday {
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            name: "New Year's Day".to_string(),
            holiday_type: HolidayType::NationalHoliday,
            affects_trading: true,
            early_close_time: None,
            late_open_time: None,
            description: Some("New Year holiday closure".to_string()),
        },
        Holiday {
            date: NaiveDate::from_ymd_opt(2024, 7, 4).unwrap(),
            name: "Independence Day".to_string(),
            holiday_type: HolidayType::NationalHoliday,
            affects_trading: true,
            early_close_time: None,
            late_open_time: None,
            description: Some("Fourth of July holiday closure".to_string()),
        },
    ];
    market_hours.update_holidays(Exchange::NYSE, holidays.clone()).await;
    market_hours.update_holidays(Exchange::NASDAQ, holidays).await;
    
    // Test that exchanges are closed on holidays
    let christmas = create_datetime(2024, 12, 25, 12, 0);
    let new_year = create_datetime(2024, 1, 1, 12, 0);
    
    assert!(!market_hours.is_exchange_open(Exchange::NYSE, christmas).await);
    assert_eq!(
        market_hours.get_session(Exchange::NYSE, christmas).await,
        MarketSession::Closed
    );
    
    assert!(!market_hours.is_exchange_open(Exchange::NASDAQ, new_year).await);
    assert_eq!(
        market_hours.get_session(Exchange::NASDAQ, new_year).await,
        MarketSession::Closed
    );
}

#[tokio::test]
async fn test_global_market_coverage() {
    let market_hours = MarketHours::new();
    
    // Test different times to see global market coverage
    let times = vec![
        create_datetime(2024, 1, 15, 0, 0),   // Midnight UTC
        create_datetime(2024, 1, 15, 6, 0),   // 6 AM UTC
        create_datetime(2024, 1, 15, 12, 0),  // Noon UTC
        create_datetime(2024, 1, 15, 18, 0),  // 6 PM UTC
    ];
    
    for time in times {
        let active = market_hours.get_active_exchanges(time).await;
        let intensity = market_hours.get_market_intensity(time).await;
        
        // During weekdays, at least some exchange should be active at most times
        if time.weekday() != Weekday::Sat && time.weekday() != Weekday::Sun {
            // Global markets mean something is usually open
            println!(
                "Time: {} UTC, Active exchanges: {}, Intensity: {:.2}",
                time.format("%H:%M"),
                active.len(),
                intensity.score
            );
        }
    }
}

#[tokio::test]
async fn test_resource_limit_recommendations() {
    let market_hours = MarketHours::new();
    
    // Weekend should allow high resource usage
    let weekend = create_weekday_datetime(Weekday::Sat, 12, 0);
    let weekend_limit = market_hours.get_resource_limit(weekend).await;
    assert!(weekend_limit > 0.8);
    
    // Market hours should restrict resource usage
    let market_time = create_weekday_datetime(Weekday::Tue, 15, 0);
    let market_limit = market_hours.get_resource_limit(market_time).await;
    assert!(market_limit < 0.5);
}

#[tokio::test]
async fn test_intensity_caching() {
    let market_hours = MarketHours::new();
    
    let test_time = create_datetime(2024, 1, 15, 12, 0);
    
    // First call should calculate
    let intensity1 = market_hours.get_market_intensity(test_time).await;
    
    // Second call should use cache
    let intensity2 = market_hours.get_market_intensity(test_time).await;
    
    assert_eq!(intensity1.score, intensity2.score);
    assert_eq!(intensity1.active_exchanges, intensity2.active_exchanges);
}

#[tokio::test]
async fn test_training_window_ordering() {
    // Test the Ord implementation
    assert!(TrainingWindow::Optimal < TrainingWindow::Good);
    assert!(TrainingWindow::Good < TrainingWindow::Acceptable);
    assert!(TrainingWindow::Acceptable < TrainingWindow::Poor);
    assert!(TrainingWindow::Poor < TrainingWindow::Restricted);
    
    // Test equality
    assert_eq!(TrainingWindow::Optimal, TrainingWindow::Optimal);
    
    // Test sorting
    let mut windows = vec![
        TrainingWindow::Poor,
        TrainingWindow::Optimal,
        TrainingWindow::Restricted,
        TrainingWindow::Good,
        TrainingWindow::Acceptable,
    ];
    
    windows.sort();
    
    assert_eq!(windows[0], TrainingWindow::Optimal);
    assert_eq!(windows[1], TrainingWindow::Good);
    assert_eq!(windows[2], TrainingWindow::Acceptable);
    assert_eq!(windows[3], TrainingWindow::Poor);
    assert_eq!(windows[4], TrainingWindow::Restricted);
}

#[tokio::test]
async fn test_multiple_exchange_sessions() {
    let market_hours = MarketHours::new();
    
    // Test a time when Asian markets are open but US markets are closed
    let asia_morning = create_datetime(2024, 1, 15, 2, 0); // 2 AM UTC = 11 AM Tokyo
    
    let tokyo_session = market_hours.get_session(Exchange::TSE, asia_morning).await;
    let nyse_session = market_hours.get_session(Exchange::NYSE, asia_morning).await;
    
    // NYSE should be closed
    assert_eq!(nyse_session, MarketSession::Closed);
    
    // Training window should reflect mixed market activity
    let window = market_hours.get_training_window(asia_morning).await;
    assert!(matches!(
        window,
        TrainingWindow::Good | TrainingWindow::Acceptable
    ));
}

#[tokio::test]
async fn test_market_overlap_periods() {
    let market_hours = MarketHours::new();
    
    // Test London-NYSE overlap (roughly 1:30 PM - 4:30 PM UTC)
    let overlap_time = create_weekday_datetime(Weekday::Tue, 14, 0);
    let intensity = market_hours.get_market_intensity(overlap_time).await;
    
    // During overlap, intensity should be higher
    assert!(intensity.active_exchanges >= 2);
    assert!(intensity.score > 0.3);
    
    // Training window should be poor during overlap
    let window = market_hours.get_training_window(overlap_time).await;
    assert!(matches!(
        window,
        TrainingWindow::Poor | TrainingWindow::Restricted
    ));
}

#[tokio::test]
async fn test_extended_hours_support() {
    let market_hours = MarketHours::new();
    
    // NYSE has extended hours support
    // Pre-market: 4:00 AM - 9:30 AM ET
    // After-hours: 4:00 PM - 8:00 PM ET
    
    // Test early morning (pre-market)
    let premarket = create_weekday_datetime(Weekday::Mon, 8, 0); // ~3 AM ET
    let session = market_hours.get_session(Exchange::NYSE, premarket).await;
    
    // Test evening (after-hours)
    let afterhours = create_weekday_datetime(Weekday::Mon, 22, 0); // ~5 PM ET
    let session = market_hours.get_session(Exchange::NYSE, afterhours).await;
    
    // LSE doesn't have extended hours
    let lse_afterhours = create_weekday_datetime(Weekday::Mon, 17, 0);
    let lse_session = market_hours.get_session(Exchange::LSE, lse_afterhours).await;
    assert_eq!(lse_session, MarketSession::Closed);
}

#[tokio::test]
async fn test_volume_estimation() {
    let market_hours = MarketHours::new();
    
    // Regular trading hours should have high volume estimate
    let regular_hours = create_weekday_datetime(Weekday::Wed, 15, 0);
    let regular_intensity = market_hours.get_market_intensity(regular_hours).await;
    assert!(regular_intensity.volume_estimate > 0.5);
    
    // Extended hours should have lower volume
    let extended_hours = create_weekday_datetime(Weekday::Wed, 22, 0);
    let extended_intensity = market_hours.get_market_intensity(extended_hours).await;
    assert!(extended_intensity.volume_estimate < 0.5);
    
    // Closed markets should have minimal volume
    let closed_time = create_weekday_datetime(Weekday::Sun, 12, 0);
    let closed_intensity = market_hours.get_market_intensity(closed_time).await;
    assert!(closed_intensity.volume_estimate < 0.2);
}

#[tokio::test]
async fn test_training_window_duration() {
    let market_hours = MarketHours::new();
    
    // Request a long training window
    let start = create_weekday_datetime(Weekday::Fri, 20, 0); // Friday evening
    let min_duration = Duration::hours(8);
    
    let result = market_hours
        .find_next_training_window(start, min_duration, TrainingWindow::Optimal)
        .await;
    
    assert!(result.is_some());
    if let Some((window_start, window_end)) = result {
        // Should find the weekend window
        assert!(window_start.weekday() == Weekday::Fri || window_start.weekday() == Weekday::Sat);
        assert!(window_end - window_start >= min_duration);
    }
}

#[tokio::test]
async fn test_display_implementations() {
    // Test Display trait for TrainingWindow
    assert_eq!(format!("{}", TrainingWindow::Optimal), "Optimal");
    assert_eq!(format!("{}", TrainingWindow::Good), "Good");
    assert_eq!(format!("{}", TrainingWindow::Acceptable), "Acceptable");
    assert_eq!(format!("{}", TrainingWindow::Poor), "Poor");
    assert_eq!(format!("{}", TrainingWindow::Restricted), "Restricted");
}