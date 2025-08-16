//! Market schedule tracking for major exchanges
//! 
//! Provides utilities to determine market hours, holidays, and optimal training windows.

use chrono::{DateTime, Datelike, Local, NaiveTime, TimeZone, Utc, Weekday};
use chrono_tz::Tz;
use std::collections::HashMap;
use std::sync::RwLock;

/// Represents different market exchanges and their timezones
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Exchange {
    /// New York Stock Exchange
    NYSE,
    /// NASDAQ
    NASDAQ,
    /// London Stock Exchange
    LSE,
    /// Tokyo Stock Exchange
    TSE,
    /// Shanghai Stock Exchange
    SSE,
    /// Hong Kong Exchange
    HKEX,
}

impl Exchange {
    /// Get the timezone for this exchange
    pub fn timezone(&self) -> Tz {
        match self {
            Exchange::NYSE | Exchange::NASDAQ => chrono_tz::America::New_York,
            Exchange::LSE => chrono_tz::Europe::London,
            Exchange::TSE => chrono_tz::Asia::Tokyo,
            Exchange::SSE => chrono_tz::Asia::Shanghai,
            Exchange::HKEX => chrono_tz::Asia::Hong_Kong,
        }
    }

    /// Get regular market hours (in local timezone)
    pub fn regular_hours(&self) -> (NaiveTime, NaiveTime) {
        match self {
            Exchange::NYSE | Exchange::NASDAQ => (
                NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
                NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
            ),
            Exchange::LSE => (
                NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
                NaiveTime::from_hms_opt(16, 30, 0).unwrap(),
            ),
            Exchange::TSE => (
                NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
                NaiveTime::from_hms_opt(15, 0, 0).unwrap(),
            ),
            Exchange::SSE => (
                NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
                NaiveTime::from_hms_opt(15, 0, 0).unwrap(),
            ),
            Exchange::HKEX => (
                NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
                NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
            ),
        }
    }

    /// Check if extended hours trading is available
    pub fn has_extended_hours(&self) -> bool {
        matches!(self, Exchange::NYSE | Exchange::NASDAQ)
    }

    /// Get extended hours (pre-market and after-hours) if available
    pub fn extended_hours(&self) -> Option<(NaiveTime, NaiveTime)> {
        match self {
            Exchange::NYSE | Exchange::NASDAQ => Some((
                NaiveTime::from_hms_opt(4, 0, 0).unwrap(),  // Pre-market starts
                NaiveTime::from_hms_opt(20, 0, 0).unwrap(), // After-hours ends
            )),
            _ => None,
        }
    }
}

/// Market status for a given exchange
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketStatus {
    /// Market is open for regular trading
    Open,
    /// Pre-market trading session
    PreMarket,
    /// After-hours trading session
    AfterHours,
    /// Market is closed
    Closed,
    /// Market is closed for holiday
    Holiday,
    /// Weekend (Saturday or Sunday)
    Weekend,
}

/// Market schedule tracker
pub struct MarketSchedule {
    /// Holiday calendar cache
    holidays: RwLock<HashMap<(Exchange, i32), Vec<DateTime<Utc>>>>,
}

impl MarketSchedule {
    /// Create a new market schedule tracker
    pub fn new() -> Self {
        Self {
            holidays: RwLock::new(HashMap::new()),
        }
    }

    /// Check if a given date is a holiday for an exchange
    pub fn is_holiday(&self, exchange: Exchange, date: DateTime<Utc>) -> bool {
        let year = date.year();
        let holidays = self.holidays.read().unwrap();
        
        if let Some(holiday_list) = holidays.get(&(exchange, year)) {
            holiday_list.iter().any(|h| h.date() == date.date())
        } else {
            // For now, return false if we don't have holiday data
            // In production, this would fetch from a holiday calendar API
            false
        }
    }

    /// Get current market status for an exchange
    pub fn get_market_status(&self, exchange: Exchange, time: Option<DateTime<Utc>>) -> MarketStatus {
        let now = time.unwrap_or_else(Utc::now);
        
        // Convert to exchange's local time
        let tz = exchange.timezone();
        let local_time = now.with_timezone(&tz);
        
        // Check if it's a weekend
        match local_time.weekday() {
            Weekday::Sat | Weekday::Sun => return MarketStatus::Weekend,
            _ => {}
        }
        
        // Check if it's a holiday
        if self.is_holiday(exchange, now) {
            return MarketStatus::Holiday;
        }
        
        // Get current time of day
        let current_time = local_time.time();
        let (open_time, close_time) = exchange.regular_hours();
        
        // Check regular hours
        if current_time >= open_time && current_time < close_time {
            return MarketStatus::Open;
        }
        
        // Check extended hours if available
        if let Some((extended_start, extended_end)) = exchange.extended_hours() {
            if current_time >= extended_start && current_time < open_time {
                return MarketStatus::PreMarket;
            }
            if current_time >= close_time && current_time < extended_end {
                return MarketStatus::AfterHours;
            }
        }
        
        MarketStatus::Closed
    }

    /// Check if any major market is currently open
    pub fn any_market_open(&self, time: Option<DateTime<Utc>>) -> bool {
        let exchanges = vec![
            Exchange::NYSE,
            Exchange::NASDAQ,
            Exchange::LSE,
            Exchange::TSE,
            Exchange::SSE,
            Exchange::HKEX,
        ];
        
        exchanges.iter().any(|&exchange| {
            matches!(
                self.get_market_status(exchange, time),
                MarketStatus::Open | MarketStatus::PreMarket | MarketStatus::AfterHours
            )
        })
    }

    /// Get the next optimal training window (when markets are closed)
    pub fn next_training_window(&self, min_duration_hours: f64) -> (DateTime<Utc>, DateTime<Utc>) {
        let mut start_time = Utc::now();
        
        loop {
            // Check if current time is good for training
            if !self.any_market_open(Some(start_time)) {
                // Find how long markets will stay closed
                let mut end_time = start_time;
                while !self.any_market_open(Some(end_time)) {
                    end_time = end_time + chrono::Duration::minutes(30);
                    
                    // Limit search to 48 hours
                    if end_time - start_time > chrono::Duration::hours(48) {
                        break;
                    }
                }
                
                // Check if window is long enough
                let duration = end_time - start_time;
                if duration.num_minutes() as f64 >= min_duration_hours * 60.0 {
                    return (start_time, end_time);
                }
            }
            
            // Move to next 30-minute interval
            start_time = start_time + chrono::Duration::minutes(30);
            
            // Prevent infinite loop
            if start_time - Utc::now() > chrono::Duration::days(7) {
                // Return a default weekend window
                let next_saturday = start_time
                    + chrono::Duration::days((6 - start_time.weekday().num_days_from_monday()) as i64);
                return (next_saturday, next_saturday + chrono::Duration::hours(24));
            }
        }
    }

    /// Get market intensity score (0.0 = all closed, 1.0 = all major markets open)
    pub fn market_intensity(&self, time: Option<DateTime<Utc>>) -> f64 {
        let exchanges = vec![
            (Exchange::NYSE, 0.3),    // Weight US markets higher
            (Exchange::NASDAQ, 0.3),
            (Exchange::LSE, 0.15),
            (Exchange::TSE, 0.1),
            (Exchange::SSE, 0.075),
            (Exchange::HKEX, 0.075),
        ];
        
        exchanges.iter()
            .map(|&(exchange, weight)| {
                match self.get_market_status(exchange, time) {
                    MarketStatus::Open => weight,
                    MarketStatus::PreMarket | MarketStatus::AfterHours => weight * 0.3,
                    _ => 0.0,
                }
            })
            .sum()
    }

    /// Add holiday dates for an exchange (typically loaded from external source)
    pub fn add_holidays(&self, exchange: Exchange, year: i32, holidays: Vec<DateTime<Utc>>) {
        let mut holiday_cache = self.holidays.write().unwrap();
        holiday_cache.insert((exchange, year), holidays);
    }
}

impl Default for MarketSchedule {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_exchange_timezones() {
        assert_eq!(Exchange::NYSE.timezone(), chrono_tz::America::New_York);
        assert_eq!(Exchange::LSE.timezone(), chrono_tz::Europe::London);
        assert_eq!(Exchange::TSE.timezone(), chrono_tz::Asia::Tokyo);
    }

    #[test]
    fn test_market_hours() {
        let (open, close) = Exchange::NYSE.regular_hours();
        assert_eq!(open.hour(), 9);
        assert_eq!(open.minute(), 30);
        assert_eq!(close.hour(), 16);
        assert_eq!(close.minute(), 0);
    }

    #[test]
    fn test_weekend_detection() {
        let schedule = MarketSchedule::new();
        
        // Create a Saturday
        let saturday = Utc.ymd(2025, 1, 25).and_hms(12, 0, 0);
        assert_eq!(
            schedule.get_market_status(Exchange::NYSE, Some(saturday)),
            MarketStatus::Weekend
        );
    }

    #[test]
    fn test_market_intensity() {
        let schedule = MarketSchedule::new();
        
        // During NYSE market hours
        let nyse_open = chrono_tz::America::New_York
            .ymd(2025, 1, 27) // Monday
            .and_hms(10, 0, 0)
            .with_timezone(&Utc);
        
        let intensity = schedule.market_intensity(Some(nyse_open));
        assert!(intensity > 0.5); // Should be high during US market hours
    }
}