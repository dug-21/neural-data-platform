//! Exchange definitions and trading schedules
//! 
//! Provides comprehensive exchange schedules for major global stock exchanges
//! with their trading hours, timezone information, and extended trading sessions.

use chrono::{NaiveTime, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Major stock exchanges with their trading hours
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Exchange {
    // Americas
    NYSE,
    NASDAQ,
    TORONTO,
    MEXICO,
    SAOPAULO,
    BUENOSAIRES,
    SANTIAGO,
    
    // Europe
    LSE,         // London Stock Exchange
    FRANKFURT,
    PARIS,
    MILAN,
    MADRID,
    AMSTERDAM,
    ZURICH,
    STOCKHOLM,
    OSLO,
    COPENHAGEN,
    HELSINKI,
    MOSCOW,
    
    // Asia-Pacific
    TSE,         // Tokyo Stock Exchange
    SSE,         // Shanghai Stock Exchange
    BSE,         // Bombay Stock Exchange  
    HKEX,        // Hong Kong Exchange
    SINGAPORE,
    SEOUL,
    TAIWAN,
    SYDNEY,
    WELLINGTON,
    BANGKOK,
    JAKARTA,
    KUALALUMPUR,
    
    // Africa
    JOHANNESBURG,
    
    // Custom
    CUSTOM,
}

/// Regular trading hours
#[derive(Debug, Clone)]
pub struct TradingHours {
    pub open: NaiveTime,
    pub close: NaiveTime,
    pub trading_days: Vec<Weekday>,
}

/// Extended trading hours
#[derive(Debug, Clone)]
pub struct ExtendedHours {
    pub pre_market_open: NaiveTime,
    pub pre_market_close: NaiveTime,
    pub after_hours_open: NaiveTime,
    pub after_hours_close: NaiveTime,
}

/// Exchange trading schedule
#[derive(Debug, Clone)]
pub struct ExchangeSchedule {
    pub exchange: Exchange,
    pub timezone_name: String,             // e.g., "America/New_York"
    pub utc_offset: i32,                   // UTC offset in hours
    pub regular_hours: TradingHours,
    pub extended_hours: Option<ExtendedHours>,
    pub holidays: Vec<chrono::DateTime<chrono::Utc>>,
    pub half_days: Vec<(chrono::DateTime<chrono::Utc>, NaiveTime)>, // Date and early close time
    pub settlement_time: Option<NaiveTime>, // Daily settlement time
    pub circuit_breaker_rules: crate::utils::market_hours::config::CircuitBreakerRules,
}

/// Exchange schedule manager
pub struct ExchangeManager {
    schedules: HashMap<Exchange, ExchangeSchedule>,
    timezone_offsets: HashMap<Exchange, i32>,
}

impl ExchangeManager {
    pub fn new() -> Self {
        let mut schedules = HashMap::new();
        let mut timezone_offsets = HashMap::new();
        
        // Initialize all exchanges with their schedules
        Self::init_americas_exchanges(&mut schedules, &mut timezone_offsets);
        Self::init_europe_exchanges(&mut schedules, &mut timezone_offsets);
        Self::init_asia_pacific_exchanges(&mut schedules, &mut timezone_offsets);
        Self::init_africa_exchanges(&mut schedules, &mut timezone_offsets);
        
        Self {
            schedules,
            timezone_offsets,
        }
    }

    pub fn get_schedule(&self, exchange: Exchange) -> Option<&ExchangeSchedule> {
        self.schedules.get(&exchange)
    }

    pub fn get_timezone_offset(&self, exchange: Exchange) -> Option<i32> {
        self.timezone_offsets.get(&exchange).copied()
    }

    pub fn get_all_exchanges(&self) -> Vec<Exchange> {
        self.schedules.keys().copied().collect()
    }

    /// Initialize Americas exchanges
    fn init_americas_exchanges(schedules: &mut HashMap<Exchange, ExchangeSchedule>, offsets: &mut HashMap<Exchange, i32>) {
        // NYSE and NASDAQ
        let nyse_schedule = ExchangeSchedule {
            exchange: Exchange::NYSE,
            timezone_name: "America/New_York".to_string(),
            utc_offset: -5, // EST
            regular_hours: TradingHours {
                open: NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
                close: NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
                trading_days: vec![
                    Weekday::Mon, Weekday::Tue, Weekday::Wed, 
                    Weekday::Thu, Weekday::Fri
                ],
            },
            extended_hours: Some(ExtendedHours {
                pre_market_open: NaiveTime::from_hms_opt(4, 0, 0).unwrap(),
                pre_market_close: NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
                after_hours_open: NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
                after_hours_close: NaiveTime::from_hms_opt(20, 0, 0).unwrap(),
            }),
            holidays: vec![],
            half_days: vec![],
            settlement_time: Some(NaiveTime::from_hms_opt(16, 30, 0).unwrap()),
            circuit_breaker_rules: crate::utils::market_hours::config::CircuitBreakerRules {
                enabled: true,
                levels: vec![
                    crate::utils::market_hours::config::CircuitBreakerLevel {
                        percentage: 0.07,
                        halt_duration_minutes: 15,
                        applies_after_time: NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
                    },
                    crate::utils::market_hours::config::CircuitBreakerLevel {
                        percentage: 0.13,
                        halt_duration_minutes: 15,
                        applies_after_time: NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
                    },
                    crate::utils::market_hours::config::CircuitBreakerLevel {
                        percentage: 0.20,
                        halt_duration_minutes: 0, // Halt for rest of day
                        applies_after_time: NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
                    },
                ],
            },
        };
        
        schedules.insert(Exchange::NYSE, nyse_schedule.clone());
        schedules.insert(Exchange::NASDAQ, nyse_schedule);
        offsets.insert(Exchange::NYSE, -5);
        offsets.insert(Exchange::NASDAQ, -5);
        
        // Toronto Stock Exchange
        schedules.insert(Exchange::TORONTO, ExchangeSchedule {
            exchange: Exchange::TORONTO,
            timezone_name: "America/Toronto".to_string(),
            utc_offset: -5, // EST
            regular_hours: TradingHours {
                open: NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
                close: NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
                trading_days: vec![
                    Weekday::Mon, Weekday::Tue, Weekday::Wed, 
                    Weekday::Thu, Weekday::Fri
                ],
            },
            extended_hours: None,
            holidays: vec![],
            half_days: vec![],
            settlement_time: Some(NaiveTime::from_hms_opt(16, 15, 0).unwrap()),
            circuit_breaker_rules: crate::utils::market_hours::config::CircuitBreakerRules::default(),
        });
        offsets.insert(Exchange::TORONTO, -5);
        
        // São Paulo Stock Exchange (B3)
        schedules.insert(Exchange::SAOPAULO, ExchangeSchedule {
            exchange: Exchange::SAOPAULO,
            timezone_name: "America/Sao_Paulo".to_string(),
            utc_offset: -3, // BRT
            regular_hours: TradingHours {
                open: NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
                close: NaiveTime::from_hms_opt(17, 30, 0).unwrap(),
                trading_days: vec![
                    Weekday::Mon, Weekday::Tue, Weekday::Wed, 
                    Weekday::Thu, Weekday::Fri
                ],
            },
            extended_hours: Some(ExtendedHours {
                pre_market_open: NaiveTime::from_hms_opt(9, 45, 0).unwrap(),
                pre_market_close: NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
                after_hours_open: NaiveTime::from_hms_opt(17, 30, 0).unwrap(),
                after_hours_close: NaiveTime::from_hms_opt(18, 0, 0).unwrap(),
            }),
            holidays: vec![],
            half_days: vec![],
            settlement_time: Some(NaiveTime::from_hms_opt(18, 0, 0).unwrap()),
            circuit_breaker_rules: crate::utils::market_hours::config::CircuitBreakerRules::default(),
        });
        offsets.insert(Exchange::SAOPAULO, -3);
    }
    
    /// Initialize European exchanges
    fn init_europe_exchanges(schedules: &mut HashMap<Exchange, ExchangeSchedule>, offsets: &mut HashMap<Exchange, i32>) {
        // London Stock Exchange
        schedules.insert(Exchange::LSE, ExchangeSchedule {
            exchange: Exchange::LSE,
            timezone_name: "Europe/London".to_string(),
            utc_offset: 0, // GMT
            regular_hours: TradingHours {
                open: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
                close: NaiveTime::from_hms_opt(16, 30, 0).unwrap(),
                trading_days: vec![
                    Weekday::Mon, Weekday::Tue, Weekday::Wed, 
                    Weekday::Thu, Weekday::Fri
                ],
            },
            extended_hours: None,
            holidays: vec![],
            half_days: vec![],
            settlement_time: Some(NaiveTime::from_hms_opt(17, 0, 0).unwrap()),
            circuit_breaker_rules: crate::utils::market_hours::config::CircuitBreakerRules::default(),
        });
        offsets.insert(Exchange::LSE, 0);
        
        // Frankfurt Stock Exchange
        schedules.insert(Exchange::FRANKFURT, ExchangeSchedule {
            exchange: Exchange::FRANKFURT,
            timezone_name: "Europe/Berlin".to_string(),
            utc_offset: 1, // CET
            regular_hours: TradingHours {
                open: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
                close: NaiveTime::from_hms_opt(17, 30, 0).unwrap(),
                trading_days: vec![
                    Weekday::Mon, Weekday::Tue, Weekday::Wed, 
                    Weekday::Thu, Weekday::Fri
                ],
            },
            extended_hours: Some(ExtendedHours {
                pre_market_open: NaiveTime::from_hms_opt(8, 0, 0).unwrap(),
                pre_market_close: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
                after_hours_open: NaiveTime::from_hms_opt(17, 30, 0).unwrap(),
                after_hours_close: NaiveTime::from_hms_opt(20, 0, 0).unwrap(),
            }),
            holidays: vec![],
            half_days: vec![],
            settlement_time: Some(NaiveTime::from_hms_opt(18, 0, 0).unwrap()),
            circuit_breaker_rules: crate::utils::market_hours::config::CircuitBreakerRules::default(),
        });
        offsets.insert(Exchange::FRANKFURT, 1);
        
        // Paris Stock Exchange (Euronext)
        schedules.insert(Exchange::PARIS, ExchangeSchedule {
            exchange: Exchange::PARIS,
            timezone_name: "Europe/Paris".to_string(),
            utc_offset: 1, // CET
            regular_hours: TradingHours {
                open: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
                close: NaiveTime::from_hms_opt(17, 30, 0).unwrap(),
                trading_days: vec![
                    Weekday::Mon, Weekday::Tue, Weekday::Wed, 
                    Weekday::Thu, Weekday::Fri
                ],
            },
            extended_hours: None,
            holidays: vec![],
            half_days: vec![],
            settlement_time: Some(NaiveTime::from_hms_opt(17, 35, 0).unwrap()),
            circuit_breaker_rules: crate::utils::market_hours::config::CircuitBreakerRules::default(),
        });
        offsets.insert(Exchange::PARIS, 1);
    }
    
    /// Initialize Asia-Pacific exchanges
    fn init_asia_pacific_exchanges(schedules: &mut HashMap<Exchange, ExchangeSchedule>, offsets: &mut HashMap<Exchange, i32>) {
        // Tokyo Stock Exchange
        schedules.insert(Exchange::TSE, ExchangeSchedule {
            exchange: Exchange::TSE,
            timezone_name: "Asia/Tokyo".to_string(),
            utc_offset: 9, // JST
            regular_hours: TradingHours {
                open: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
                close: NaiveTime::from_hms_opt(15, 0, 0).unwrap(),
                trading_days: vec![
                    Weekday::Mon, Weekday::Tue, Weekday::Wed, 
                    Weekday::Thu, Weekday::Fri
                ],
            },
            extended_hours: None,
            holidays: vec![],
            half_days: vec![],
            settlement_time: Some(NaiveTime::from_hms_opt(15, 15, 0).unwrap()),
            circuit_breaker_rules: crate::utils::market_hours::config::CircuitBreakerRules::default(),
        });
        offsets.insert(Exchange::TSE, 9);
        
        // Shanghai Stock Exchange
        schedules.insert(Exchange::SSE, ExchangeSchedule {
            exchange: Exchange::SSE,
            timezone_name: "Asia/Shanghai".to_string(),
            utc_offset: 8, // CST
            regular_hours: TradingHours {
                open: NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
                close: NaiveTime::from_hms_opt(15, 0, 0).unwrap(),
                trading_days: vec![
                    Weekday::Mon, Weekday::Tue, Weekday::Wed, 
                    Weekday::Thu, Weekday::Fri
                ],
            },
            extended_hours: None,
            holidays: vec![],
            half_days: vec![],
            settlement_time: Some(NaiveTime::from_hms_opt(15, 30, 0).unwrap()),
            circuit_breaker_rules: crate::utils::market_hours::config::CircuitBreakerRules::default(),
        });
        offsets.insert(Exchange::SSE, 8);
        
        // Hong Kong Exchange
        schedules.insert(Exchange::HKEX, ExchangeSchedule {
            exchange: Exchange::HKEX,
            timezone_name: "Asia/Hong_Kong".to_string(),
            utc_offset: 8, // HKT
            regular_hours: TradingHours {
                open: NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
                close: NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
                trading_days: vec![
                    Weekday::Mon, Weekday::Tue, Weekday::Wed, 
                    Weekday::Thu, Weekday::Fri
                ],
            },
            extended_hours: None,
            holidays: vec![],
            half_days: vec![],
            settlement_time: Some(NaiveTime::from_hms_opt(16, 15, 0).unwrap()),
            circuit_breaker_rules: crate::utils::market_hours::config::CircuitBreakerRules::default(),
        });
        offsets.insert(Exchange::HKEX, 8);
        
        // Australian Securities Exchange
        schedules.insert(Exchange::SYDNEY, ExchangeSchedule {
            exchange: Exchange::SYDNEY,
            timezone_name: "Australia/Sydney".to_string(),
            utc_offset: 10, // AEST
            regular_hours: TradingHours {
                open: NaiveTime::from_hms_opt(10, 0, 0).unwrap(),
                close: NaiveTime::from_hms_opt(16, 0, 0).unwrap(),
                trading_days: vec![
                    Weekday::Mon, Weekday::Tue, Weekday::Wed, 
                    Weekday::Thu, Weekday::Fri
                ],
            },
            extended_hours: None,
            holidays: vec![],
            half_days: vec![],
            settlement_time: Some(NaiveTime::from_hms_opt(16, 15, 0).unwrap()),
            circuit_breaker_rules: crate::utils::market_hours::config::CircuitBreakerRules::default(),
        });
        offsets.insert(Exchange::SYDNEY, 10);
    }
    
    /// Initialize African exchanges
    fn init_africa_exchanges(schedules: &mut HashMap<Exchange, ExchangeSchedule>, offsets: &mut HashMap<Exchange, i32>) {
        // Johannesburg Stock Exchange
        schedules.insert(Exchange::JOHANNESBURG, ExchangeSchedule {
            exchange: Exchange::JOHANNESBURG,
            timezone_name: "Africa/Johannesburg".to_string(),
            utc_offset: 2, // SAST
            regular_hours: TradingHours {
                open: NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
                close: NaiveTime::from_hms_opt(17, 0, 0).unwrap(),
                trading_days: vec![
                    Weekday::Mon, Weekday::Tue, Weekday::Wed, 
                    Weekday::Thu, Weekday::Fri
                ],
            },
            extended_hours: None,
            holidays: vec![],
            half_days: vec![],
            settlement_time: Some(NaiveTime::from_hms_opt(17, 15, 0).unwrap()),
            circuit_breaker_rules: crate::utils::market_hours::config::CircuitBreakerRules::default(),
        });
        offsets.insert(Exchange::JOHANNESBURG, 2);
    }
}

impl Default for ExchangeManager {
    fn default() -> Self {
        Self::new()
    }
}