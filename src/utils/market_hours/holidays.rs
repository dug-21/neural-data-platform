//! Holiday calendar management for market closures
//! 
//! Provides comprehensive holiday tracking for major exchanges including
//! national holidays, bank holidays, and market-specific closures.

use chrono::{DateTime, Utc, NaiveDate, Datelike};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

use crate::utils::market_hours::exchanges::Exchange;

/// Type of market holiday
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HolidayType {
    NationalHoliday,
    BankHoliday,
    MarketHoliday,
    EarlyClose,
    LateOpen,
    EmergencyClosure,
}

/// Holiday entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Holiday {
    pub date: NaiveDate,
    pub name: String,
    pub holiday_type: HolidayType,
    pub affects_trading: bool,
    pub early_close_time: Option<chrono::NaiveTime>,
    pub late_open_time: Option<chrono::NaiveTime>,
    pub description: Option<String>,
}

/// Holiday calendar for market closures
#[derive(Debug, Clone)]
pub struct HolidayCalendar {
    holidays: HashMap<Exchange, Vec<Holiday>>,
    last_update: DateTime<Utc>,
    holiday_cache: HashMap<(Exchange, NaiveDate), HolidayType>,
}

impl HolidayCalendar {
    pub fn new() -> Self {
        let mut calendar = Self {
            holidays: HashMap::new(),
            last_update: Utc::now(),
            holiday_cache: HashMap::new(),
        };
        
        // Initialize with common holidays for major exchanges
        calendar.initialize_holidays();
        calendar
    }

    /// Initialize common holidays for major exchanges
    fn initialize_holidays(&mut self) {
        self.add_us_holidays();
        self.add_uk_holidays();
        self.add_eu_holidays();
        self.add_asia_holidays();
        self.rebuild_cache();
    }

    /// Add US market holidays (NYSE, NASDAQ)
    fn add_us_holidays(&mut self) {
        let us_exchanges = vec![Exchange::NYSE, Exchange::NASDAQ, Exchange::TORONTO];
        
        for exchange in us_exchanges {
            let mut holidays = Vec::new();
            
            // Fixed date holidays
            holidays.push(Holiday {
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                name: "New Year's Day".to_string(),
                holiday_type: HolidayType::NationalHoliday,
                affects_trading: true,
                early_close_time: None,
                late_open_time: None,
                description: Some("New Year's Day market closure".to_string()),
            });
            
            holidays.push(Holiday {
                date: NaiveDate::from_ymd_opt(2024, 7, 4).unwrap(),
                name: "Independence Day".to_string(),
                holiday_type: HolidayType::NationalHoliday,
                affects_trading: true,
                early_close_time: None,
                late_open_time: None,
                description: Some("US Independence Day".to_string()),
            });
            
            holidays.push(Holiday {
                date: NaiveDate::from_ymd_opt(2024, 12, 25).unwrap(),
                name: "Christmas Day".to_string(),
                holiday_type: HolidayType::NationalHoliday,
                affects_trading: true,
                early_close_time: None,
                late_open_time: None,
                description: Some("Christmas Day market closure".to_string()),
            });
            
            // Early close days
            holidays.push(Holiday {
                date: NaiveDate::from_ymd_opt(2024, 11, 29).unwrap(),
                name: "Black Friday".to_string(),
                holiday_type: HolidayType::EarlyClose,
                affects_trading: false,
                early_close_time: Some(chrono::NaiveTime::from_hms_opt(13, 0, 0).unwrap()),
                late_open_time: None,
                description: Some("Early close at 1:00 PM ET".to_string()),
            });
            
            holidays.push(Holiday {
                date: NaiveDate::from_ymd_opt(2024, 12, 24).unwrap(),
                name: "Christmas Eve".to_string(),
                holiday_type: HolidayType::EarlyClose,
                affects_trading: false,
                early_close_time: Some(chrono::NaiveTime::from_hms_opt(13, 0, 0).unwrap()),
                late_open_time: None,
                description: Some("Early close at 1:00 PM ET".to_string()),
            });
            
            self.holidays.insert(exchange, holidays);
        }
    }

    /// Add UK market holidays (LSE)
    fn add_uk_holidays(&mut self) {
        let mut holidays = Vec::new();
        
        holidays.push(Holiday {
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            name: "New Year's Day".to_string(),
            holiday_type: HolidayType::BankHoliday,
            affects_trading: true,
            early_close_time: None,
            late_open_time: None,
            description: Some("New Year's Day bank holiday".to_string()),
        });
        
        holidays.push(Holiday {
            date: NaiveDate::from_ymd_opt(2024, 3, 29).unwrap(),
            name: "Good Friday".to_string(),
            holiday_type: HolidayType::BankHoliday,
            affects_trading: true,
            early_close_time: None,
            late_open_time: None,
            description: Some("Good Friday bank holiday".to_string()),
        });
        
        holidays.push(Holiday {
            date: NaiveDate::from_ymd_opt(2024, 4, 1).unwrap(),
            name: "Easter Monday".to_string(),
            holiday_type: HolidayType::BankHoliday,
            affects_trading: true,
            early_close_time: None,
            late_open_time: None,
            description: Some("Easter Monday bank holiday".to_string()),
        });
        
        holidays.push(Holiday {
            date: NaiveDate::from_ymd_opt(2024, 12, 25).unwrap(),
            name: "Christmas Day".to_string(),
            holiday_type: HolidayType::BankHoliday,
            affects_trading: true,
            early_close_time: None,
            late_open_time: None,
            description: Some("Christmas Day bank holiday".to_string()),
        });
        
        holidays.push(Holiday {
            date: NaiveDate::from_ymd_opt(2024, 12, 26).unwrap(),
            name: "Boxing Day".to_string(),
            holiday_type: HolidayType::BankHoliday,
            affects_trading: true,
            early_close_time: None,
            late_open_time: None,
            description: Some("Boxing Day bank holiday".to_string()),
        });
        
        self.holidays.insert(Exchange::LSE, holidays);
    }

    /// Add European market holidays
    fn add_eu_holidays(&mut self) {
        let eu_exchanges = vec![
            Exchange::FRANKFURT, Exchange::PARIS, Exchange::MILAN,
            Exchange::MADRID, Exchange::AMSTERDAM, Exchange::ZURICH
        ];
        
        for exchange in eu_exchanges {
            let mut holidays = Vec::new();
            
            holidays.push(Holiday {
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                name: "New Year's Day".to_string(),
                holiday_type: HolidayType::NationalHoliday,
                affects_trading: true,
                early_close_time: None,
                late_open_time: None,
                description: Some("New Year's Day closure".to_string()),
            });
            
            holidays.push(Holiday {
                date: NaiveDate::from_ymd_opt(2024, 3, 29).unwrap(),
                name: "Good Friday".to_string(),
                holiday_type: HolidayType::NationalHoliday,
                affects_trading: true,
                early_close_time: None,
                late_open_time: None,
                description: Some("Good Friday closure".to_string()),
            });
            
            holidays.push(Holiday {
                date: NaiveDate::from_ymd_opt(2024, 4, 1).unwrap(),
                name: "Easter Monday".to_string(),
                holiday_type: HolidayType::NationalHoliday,
                affects_trading: true,
                early_close_time: None,
                late_open_time: None,
                description: Some("Easter Monday closure".to_string()),
            });
            
            holidays.push(Holiday {
                date: NaiveDate::from_ymd_opt(2024, 5, 1).unwrap(),
                name: "Labour Day".to_string(),
                holiday_type: HolidayType::NationalHoliday,
                affects_trading: true,
                early_close_time: None,
                late_open_time: None,
                description: Some("Labour Day closure".to_string()),
            });
            
            holidays.push(Holiday {
                date: NaiveDate::from_ymd_opt(2024, 12, 25).unwrap(),
                name: "Christmas Day".to_string(),
                holiday_type: HolidayType::NationalHoliday,
                affects_trading: true,
                early_close_time: None,
                late_open_time: None,
                description: Some("Christmas Day closure".to_string()),
            });
            
            holidays.push(Holiday {
                date: NaiveDate::from_ymd_opt(2024, 12, 26).unwrap(),
                name: "Boxing Day".to_string(),
                holiday_type: HolidayType::NationalHoliday,
                affects_trading: true,
                early_close_time: None,
                late_open_time: None,
                description: Some("Boxing Day closure".to_string()),
            });
            
            self.holidays.insert(exchange, holidays);
        }
    }

    /// Add Asian market holidays
    fn add_asia_holidays(&mut self) {
        // Tokyo Stock Exchange
        let mut tse_holidays = Vec::new();
        
        tse_holidays.push(Holiday {
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            name: "New Year's Day".to_string(),
            holiday_type: HolidayType::NationalHoliday,
            affects_trading: true,
            early_close_time: None,
            late_open_time: None,
            description: Some("Japanese New Year".to_string()),
        });
        
        tse_holidays.push(Holiday {
            date: NaiveDate::from_ymd_opt(2024, 2, 11).unwrap(),
            name: "National Foundation Day".to_string(),
            holiday_type: HolidayType::NationalHoliday,
            affects_trading: true,
            early_close_time: None,
            late_open_time: None,
            description: Some("National Foundation Day".to_string()),
        });
        
        self.holidays.insert(Exchange::TSE, tse_holidays);
        
        // Hong Kong Exchange
        let mut hkex_holidays = Vec::new();
        
        hkex_holidays.push(Holiday {
            date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            name: "New Year's Day".to_string(),
            holiday_type: HolidayType::NationalHoliday,
            affects_trading: true,
            early_close_time: None,
            late_open_time: None,
            description: Some("New Year's Day".to_string()),
        });
        
        hkex_holidays.push(Holiday {
            date: NaiveDate::from_ymd_opt(2024, 2, 10).unwrap(),
            name: "Chinese New Year".to_string(),
            holiday_type: HolidayType::NationalHoliday,
            affects_trading: true,
            early_close_time: None,
            late_open_time: None,
            description: Some("Chinese New Year".to_string()),
        });
        
        self.holidays.insert(Exchange::HKEX, hkex_holidays);
    }

    /// Rebuild the holiday cache for faster lookups
    fn rebuild_cache(&mut self) {
        self.holiday_cache.clear();
        
        for (exchange, holidays) in &self.holidays {
            for holiday in holidays {
                self.holiday_cache.insert((*exchange, holiday.date), holiday.holiday_type);
            }
        }
    }

    /// Check if a date is a holiday for an exchange
    pub fn is_holiday(&self, exchange: Exchange, date: NaiveDate) -> bool {
        self.holiday_cache.contains_key(&(exchange, date))
    }

    /// Get holiday information for a specific date and exchange
    pub fn get_holiday(&self, exchange: Exchange, date: NaiveDate) -> Option<&Holiday> {
        if let Some(holidays) = self.holidays.get(&exchange) {
            holidays.iter().find(|h| h.date == date)
        } else {
            None
        }
    }

    /// Get all holidays for an exchange
    pub fn get_holidays(&self, exchange: Exchange) -> Option<&Vec<Holiday>> {
        self.holidays.get(&exchange)
    }

    /// Add a new holiday
    pub fn add_holiday(&mut self, exchange: Exchange, holiday: Holiday) {
        self.holidays.entry(exchange).or_insert_with(Vec::new).push(holiday.clone());
        self.holiday_cache.insert((exchange, holiday.date), holiday.holiday_type);
        self.last_update = Utc::now();
    }

    /// Remove a holiday
    pub fn remove_holiday(&mut self, exchange: Exchange, date: NaiveDate) -> bool {
        if let Some(holidays) = self.holidays.get_mut(&exchange) {
            if let Some(pos) = holidays.iter().position(|h| h.date == date) {
                holidays.remove(pos);
                self.holiday_cache.remove(&(exchange, date));
                self.last_update = Utc::now();
                return true;
            }
        }
        false
    }

    /// Update holidays for an exchange
    pub fn update_holidays(&mut self, exchange: Exchange, holidays: Vec<Holiday>) {
        // Remove old cache entries for this exchange
        self.holiday_cache.retain(|(ex, _), _| *ex != exchange);
        
        // Add new cache entries
        for holiday in &holidays {
            self.holiday_cache.insert((exchange, holiday.date), holiday.holiday_type);
        }
        
        self.holidays.insert(exchange, holidays);
        self.last_update = Utc::now();
    }

    /// Get upcoming holidays for an exchange
    pub fn get_upcoming_holidays(&self, exchange: Exchange, days_ahead: u32) -> Vec<&Holiday> {
        let today = Utc::now().date_naive();
        let end_date = today + chrono::Duration::days(days_ahead as i64);
        
        if let Some(holidays) = self.holidays.get(&exchange) {
            holidays
                .iter()
                .filter(|h| h.date >= today && h.date <= end_date)
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Check if market has early close
    pub fn is_early_close(&self, exchange: Exchange, date: NaiveDate) -> Option<chrono::NaiveTime> {
        if let Some(holiday) = self.get_holiday(exchange, date) {
            if holiday.holiday_type == HolidayType::EarlyClose {
                return holiday.early_close_time;
            }
        }
        None
    }

    /// Check if market has late open
    pub fn is_late_open(&self, exchange: Exchange, date: NaiveDate) -> Option<chrono::NaiveTime> {
        if let Some(holiday) = self.get_holiday(exchange, date) {
            if holiday.holiday_type == HolidayType::LateOpen {
                return holiday.late_open_time;
            }
        }
        None
    }

    /// Get holiday statistics
    pub fn get_statistics(&self) -> HolidayStatistics {
        let mut total_holidays = 0;
        let mut exchanges_with_holidays = 0;
        let mut holiday_types = HashMap::new();
        
        for (_, holidays) in &self.holidays {
            if !holidays.is_empty() {
                exchanges_with_holidays += 1;
            }
            
            for holiday in holidays {
                total_holidays += 1;
                *holiday_types.entry(holiday.holiday_type).or_insert(0) += 1;
            }
        }
        
        HolidayStatistics {
            total_holidays,
            exchanges_covered: self.holidays.len(),
            exchanges_with_holidays,
            holiday_types,
            last_update: self.last_update,
        }
    }

    /// Check if date affects trading (either full closure or modified hours)
    pub fn affects_trading(&self, exchange: Exchange, date: NaiveDate) -> bool {
        if let Some(holiday) = self.get_holiday(exchange, date) {
            holiday.affects_trading || 
            holiday.holiday_type == HolidayType::EarlyClose ||
            holiday.holiday_type == HolidayType::LateOpen
        } else {
            false
        }
    }
}

/// Holiday statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HolidayStatistics {
    pub total_holidays: usize,
    pub exchanges_covered: usize,
    pub exchanges_with_holidays: usize,
    pub holiday_types: HashMap<HolidayType, usize>,
    pub last_update: DateTime<Utc>,
}

/// Holiday manager with async support
pub struct HolidayManager {
    calendar: RwLock<HolidayCalendar>,
}

impl HolidayManager {
    pub fn new() -> Self {
        Self {
            calendar: RwLock::new(HolidayCalendar::new()),
        }
    }

    /// Check if a date is a holiday (async)
    pub async fn is_holiday(&self, exchange: Exchange, date: DateTime<Utc>) -> bool {
        let calendar = self.calendar.read().await;
        calendar.is_holiday(exchange, date.date_naive())
    }

    /// Get holiday information (async)
    pub async fn get_holiday(&self, exchange: Exchange, date: DateTime<Utc>) -> Option<Holiday> {
        let calendar = self.calendar.read().await;
        calendar.get_holiday(exchange, date.date_naive()).cloned()
    }

    /// Update holidays (async)
    pub async fn update_holidays(&self, exchange: Exchange, holidays: Vec<Holiday>) {
        let mut calendar = self.calendar.write().await;
        calendar.update_holidays(exchange, holidays);
    }

    /// Add holiday (async)
    pub async fn add_holiday(&self, exchange: Exchange, holiday: Holiday) {
        let mut calendar = self.calendar.write().await;
        calendar.add_holiday(exchange, holiday);
    }

    /// Get upcoming holidays (async)
    pub async fn get_upcoming_holidays(&self, exchange: Exchange, days_ahead: u32) -> Vec<Holiday> {
        let calendar = self.calendar.read().await;
        calendar.get_upcoming_holidays(exchange, days_ahead)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Check for early close (async)
    pub async fn is_early_close(&self, exchange: Exchange, date: DateTime<Utc>) -> Option<chrono::NaiveTime> {
        let calendar = self.calendar.read().await;
        calendar.is_early_close(exchange, date.date_naive())
    }

    /// Get statistics (async)
    pub async fn get_statistics(&self) -> HolidayStatistics {
        let calendar = self.calendar.read().await;
        calendar.get_statistics()
    }
}

impl Default for HolidayManager {
    fn default() -> Self {
        Self::new()
    }
}