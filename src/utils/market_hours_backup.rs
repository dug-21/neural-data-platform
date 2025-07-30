//! Market hours tracking and training window detection
//! 
//! Provides comprehensive market schedule information for major exchanges
//! and identifies optimal training windows based on market activity.

use chrono::{DateTime, Datelike, Duration, NaiveTime, TimeZone, Timelike, Utc, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::{Context, Result};
use uuid::Uuid;

// Note: Using UTC-based timezone offset calculations since chrono-tz is not available

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

/// Market session types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketSession {
    /// Pre-market trading session
    PreMarket,
    /// Regular trading hours
    Regular,
    /// After-hours trading session
    AfterHours,
    /// Market is closed
    Closed,
}

/// Trading window suitability for model training
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrainingWindow {
    /// Optimal window (weekends, holidays, deep night)
    Optimal,
    /// Good window (early morning, late night)
    Good,
    /// Acceptable window (extended hours)
    Acceptable,
    /// Poor window (active trading)
    Poor,
    /// Restricted window (critical trading periods)
    Restricted,
}

/// Market intensity level
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MarketIntensity {
    /// Overall intensity score (0.0 = no activity, 1.0 = peak activity)
    pub score: f64,
    /// Number of active major exchanges
    pub active_exchanges: usize,
    /// Primary session type across exchanges
    pub dominant_session: MarketSession,
    /// Estimated global trading volume percentage
    pub volume_estimate: f64,
}

/// Resource allocation policy based on market conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocationPolicy {
    pub market_hours_allocation: f64,      // 0.25 (25%) during market hours
    pub off_hours_allocation: f64,         // 0.90 (90%) during off hours
    pub weekend_allocation: f64,           // 0.95 (95%) during weekends
    pub holiday_allocation: f64,           // 0.95 (95%) during holidays
    pub emergency_allocation: f64,         // 1.00 (100%) during emergencies
    pub multi_market_penalty: f64,         // Reduction per active market (0.05)
    pub volatility_adjustment: f64,        // Adjustment based on market volatility
    pub min_allocation: f64,               // Minimum allocation regardless of conditions (0.10)
}

impl Default for ResourceAllocationPolicy {
    fn default() -> Self {
        Self {
            market_hours_allocation: 0.25,
            off_hours_allocation: 0.90,
            weekend_allocation: 0.95,
            holiday_allocation: 0.95,
            emergency_allocation: 1.00,
            multi_market_penalty: 0.05,
            volatility_adjustment: 0.0,
            min_allocation: 0.10,
        }
    }
}

/// Emergency override manager
#[derive(Debug, Clone)]
pub struct EmergencyOverrideManager {
    active_overrides: Vec<EmergencyOverride>,
    override_history: Vec<EmergencyOverride>,
    max_concurrent_overrides: usize,
    circuit_breaker_active: bool,
}

impl EmergencyOverrideManager {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            active_overrides: Vec::new(),
            override_history: Vec::new(),
            max_concurrent_overrides: max_concurrent,
            circuit_breaker_active: false,
        }
    }
}

/// Emergency override details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyOverride {
    pub id: String,
    pub reason: String,
    pub priority: EmergencyPriority,
    pub requested_resources: f64,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub approved_by: String,
    pub affected_exchanges: Vec<Exchange>,
    pub metadata: HashMap<String, String>,
}

/// Emergency priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EmergencyPriority {
    Low,
    Medium,
    High,
    Critical,
    SystemCritical,
}

/// Market correlation matrix for intelligent scheduling
#[derive(Debug, Clone)]
pub struct MarketCorrelationMatrix {
    correlations: HashMap<(Exchange, Exchange), f64>,
    impact_scores: HashMap<Exchange, f64>,
    volatility_indices: HashMap<Exchange, f64>,
    last_update: DateTime<Utc>,
}

impl MarketCorrelationMatrix {
    pub fn new() -> Self {
        Self {
            correlations: HashMap::new(),
            impact_scores: HashMap::new(),
            volatility_indices: HashMap::new(),
            last_update: Utc::now(),
        }
    }
}

/// Holiday calendar for market closures
#[derive(Debug, Clone)]
pub struct HolidayCalendar {
    holidays: HashMap<Exchange, Vec<DateTime<Utc>>>,
    last_update: DateTime<Utc>,
    holiday_types: HashMap<(Exchange, DateTime<Utc>), HolidayType>,
}

/// Type of market holiday
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HolidayType {
    NationalHoliday,
    BankHoliday,
    MarketHoliday,
    EarlyClose,
    LateOpen,
    EmergencyClosure,
}

/// Exchange trading schedule
#[derive(Debug, Clone)]
pub struct ExchangeSchedule {
    pub exchange: Exchange,
    pub timezone_name: String,             // e.g., "America/New_York"
    pub utc_offset: i32,                   // UTC offset in hours
    pub regular_hours: TradingHours,
    pub extended_hours: Option<ExtendedHours>,
    pub holidays: Vec<DateTime<Utc>>,
    pub half_days: Vec<(DateTime<Utc>, NaiveTime)>, // Date and early close time
    pub settlement_time: Option<NaiveTime>, // Daily settlement time
    pub circuit_breaker_rules: CircuitBreakerRules,
}

/// Circuit breaker rules for exchanges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerRules {
    pub enabled: bool,
    pub levels: Vec<CircuitBreakerLevel>,
}

impl Default for CircuitBreakerRules {
    fn default() -> Self {
        Self {
            enabled: false,
            levels: Vec::new(),
        }
    }
}

/// Individual circuit breaker level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerLevel {
    pub percentage: f64,       // e.g., 0.07 for 7%
    pub halt_duration_minutes: u32,
    pub applies_after_time: NaiveTime,
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

/// Market hours tracker with training window detection
pub struct MarketHours {
    schedules: HashMap<Exchange, ExchangeSchedule>,
    holiday_calendar: Arc<RwLock<HolidayCalendar>>,
    intensity_cache: Arc<RwLock<HashMap<DateTime<Utc>, MarketIntensity>>>,
    resource_allocation_policy: Arc<RwLock<ResourceAllocationPolicy>>,
    emergency_override_manager: Arc<RwLock<EmergencyOverrideManager>>,
    market_correlation_matrix: Arc<RwLock<MarketCorrelationMatrix>>,
    timezone_offsets: HashMap<Exchange, i32>,  // UTC offset in hours
    market_status_cache: Arc<RwLock<HashMap<Exchange, MarketStatus>>>,
}

/// Current market status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStatus {
    pub exchange: Exchange,
    pub session: MarketSession,
    pub is_trading: bool,
    pub next_state_change: DateTime<Utc>,
    pub volatility_level: VolatilityLevel,
    pub circuit_breaker_active: bool,
    pub special_conditions: Vec<String>,
}

/// Market volatility levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VolatilityLevel {
    Low,
    Normal,
    Elevated,
    High,
    Extreme,
}

impl MarketHours {
    /// Create a new market hours tracker with comprehensive global coverage
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
            holiday_calendar: Arc::new(RwLock::new(HolidayCalendar {
                holidays: HashMap::new(),
                last_update: Utc::now(),
                holiday_types: HashMap::new(),
            })),
            intensity_cache: Arc::new(RwLock::new(HashMap::new())),
            resource_allocation_policy: Arc::new(RwLock::new(ResourceAllocationPolicy::default())),
            emergency_override_manager: Arc::new(RwLock::new(EmergencyOverrideManager::new(3))),
            market_correlation_matrix: Arc::new(RwLock::new(MarketCorrelationMatrix::new())),
            timezone_offsets,
            market_status_cache: Arc::new(RwLock::new(HashMap::new())),
        }
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
            circuit_breaker_rules: CircuitBreakerRules {
                enabled: true,
                levels: vec![
                    CircuitBreakerLevel {
                        percentage: 0.07,
                        halt_duration_minutes: 15,
                        applies_after_time: NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
                    },
                    CircuitBreakerLevel {
                        percentage: 0.13,
                        halt_duration_minutes: 15,
                        applies_after_time: NaiveTime::from_hms_opt(9, 30, 0).unwrap(),
                    },
                    CircuitBreakerLevel {
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
            circuit_breaker_rules: CircuitBreakerRules::default(),
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
            circuit_breaker_rules: CircuitBreakerRules::default(),
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
            circuit_breaker_rules: CircuitBreakerRules::default(),
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
            circuit_breaker_rules: CircuitBreakerRules::default(),
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
            circuit_breaker_rules: CircuitBreakerRules::default(),
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
            circuit_breaker_rules: CircuitBreakerRules::default(),
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
            circuit_breaker_rules: CircuitBreakerRules::default(),
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
            circuit_breaker_rules: CircuitBreakerRules::default(),
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
            circuit_breaker_rules: CircuitBreakerRules::default(),
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
            circuit_breaker_rules: CircuitBreakerRules::default(),
        });
        offsets.insert(Exchange::JOHANNESBURG, 2);
    }
    
    /// Check if a specific exchange is open
    pub async fn is_exchange_open(&self, exchange: Exchange, time: DateTime<Utc>) -> bool {
        if let Some(schedule) = self.schedules.get(&exchange) {
            // Check if it's a holiday
            if self.is_holiday(exchange, time).await {
                return false;
            }
            
            // Convert to exchange local time
            let local_time = self.convert_to_exchange_time(time, exchange);
            
            // Check if it's a trading day
            if !schedule.regular_hours.trading_days.contains(&local_time.weekday()) {
                return false;
            }
            
            // Check if within trading hours
            let current_time = local_time.time();
            current_time >= schedule.regular_hours.open && 
            current_time < schedule.regular_hours.close
        } else {
            false
        }
    }
    
    /// Convert UTC time to exchange local time
    fn convert_to_exchange_time(&self, utc_time: DateTime<Utc>, exchange: Exchange) -> DateTime<Utc> {
        if let Some(offset) = self.timezone_offsets.get(&exchange) {
            utc_time + Duration::hours(*offset as i64)
        } else {
            utc_time
        }
    }
    
    /// Get current market session for an exchange
    pub async fn get_session(&self, exchange: Exchange, time: DateTime<Utc>) -> MarketSession {
        if let Some(schedule) = self.schedules.get(&exchange) {
            if self.is_holiday(exchange, time).await {
                return MarketSession::Closed;
            }
            
            let local_time = self.convert_to_exchange_time(time, exchange);
            let weekday = local_time.weekday();
            
            if !schedule.regular_hours.trading_days.contains(&weekday) {
                return MarketSession::Closed;
            }
            
            let current_time = local_time.time();
            
            // Check extended hours if available
            if let Some(extended) = &schedule.extended_hours {
                if current_time >= extended.pre_market_open && 
                   current_time < schedule.regular_hours.open {
                    return MarketSession::PreMarket;
                }
                
                if current_time >= schedule.regular_hours.close && 
                   current_time < extended.after_hours_close {
                    return MarketSession::AfterHours;
                }
            }
            
            // Check regular hours
            if current_time >= schedule.regular_hours.open && 
               current_time < schedule.regular_hours.close {
                return MarketSession::Regular;
            }
            
            MarketSession::Closed
        } else {
            MarketSession::Closed
        }
    }
    
    /// Calculate global market intensity with correlation adjustments
    pub async fn get_market_intensity(&self, time: DateTime<Utc>) -> MarketIntensity {
        // Check cache first
        {
            let cache = self.intensity_cache.read().await;
            if let Some(cached) = cache.get(&time) {
                return *cached;
            }
        }
        
        let mut active_regular = 0;
        let mut active_extended = 0;
        let mut closed = 0;
        let mut active_exchanges = Vec::new();
        
        for exchange in self.schedules.keys() {
            match self.get_session(*exchange, time).await {
                MarketSession::Regular => {
                    active_regular += 1;
                    active_exchanges.push(*exchange);
                },
                MarketSession::PreMarket | MarketSession::AfterHours => {
                    active_extended += 1;
                    active_exchanges.push(*exchange);
                },
                MarketSession::Closed => closed += 1,
            }
        }
        
        let total_exchanges = active_regular + active_extended + closed;
        let active_count = active_regular + active_extended;
        
        // Calculate base intensity score
        let regular_weight = 1.0;
        let extended_weight = 0.3;
        let mut score = (active_regular as f64 * regular_weight + 
                        active_extended as f64 * extended_weight) / 
                       total_exchanges as f64;
        
        // Apply correlation adjustments
        score = self.apply_correlation_adjustments(score, &active_exchanges).await;
        
        // Determine dominant session
        let dominant_session = if active_regular > 0 {
            MarketSession::Regular
        } else if active_extended > 0 {
            MarketSession::PreMarket
        } else {
            MarketSession::Closed
        };
        
        // Estimate volume with correlation adjustments
        let volume_estimate = match dominant_session {
            MarketSession::Regular => 0.8 + score * 0.2,
            MarketSession::PreMarket | MarketSession::AfterHours => 0.2 + score * 0.3,
            MarketSession::Closed => score * 0.1,
        };
        
        let intensity = MarketIntensity {
            score,
            active_exchanges: active_count,
            dominant_session,
            volume_estimate,
        };
        
        // Cache the result
        {
            let mut cache = self.intensity_cache.write().await;
            cache.insert(time, intensity);
            
            // Clean old entries if cache is too large
            if cache.len() > 10000 {
                let cutoff = time - Duration::hours(24);
                cache.retain(|k, _| *k > cutoff);
            }
        }
        
        intensity
    }
    
    /// Apply market correlation adjustments to intensity score
    async fn apply_correlation_adjustments(&self, base_score: f64, active_exchanges: &[Exchange]) -> f64 {
        let matrix = self.market_correlation_matrix.read().await;
        let mut adjusted_score = base_score;
        
        // Apply correlation penalties for highly correlated active markets
        for i in 0..active_exchanges.len() {
            for j in i+1..active_exchanges.len() {
                let key = (active_exchanges[i], active_exchanges[j]);
                if let Some(correlation) = matrix.correlations.get(&key) {
                    // High correlation means overlapping impact
                    if *correlation > 0.7 {
                        adjusted_score *= 1.0 - (correlation - 0.7) * 0.5;
                    }
                }
            }
        }
        
        adjusted_score
    }
    
    /// Determine training window suitability with emergency override support
    pub async fn get_training_window(&self, time: DateTime<Utc>) -> TrainingWindow {
        // Check for emergency overrides first
        if self.has_emergency_override(time).await {
            return TrainingWindow::Optimal; // Emergency overrides get optimal window
        }
        
        let intensity = self.get_market_intensity(time).await;
        
        // Weekend check
        let weekday = time.weekday();
        if weekday == Weekday::Sat || weekday == Weekday::Sun {
            return TrainingWindow::Optimal;
        }
        
        // Check if all major exchanges are closed
        if intensity.active_exchanges == 0 {
            return TrainingWindow::Optimal;
        }
        
        // Classify based on intensity with policy adjustments
        let policy = self.resource_allocation_policy.read().await;
        match intensity.score {
            s if s < 0.1 => TrainingWindow::Optimal,
            s if s < 0.3 => TrainingWindow::Good,
            s if s < 0.5 => TrainingWindow::Acceptable,
            s if s < 0.8 => TrainingWindow::Poor,
            _ => TrainingWindow::Restricted,
        }
    }
    
    /// Check if there's an active emergency override
    async fn has_emergency_override(&self, time: DateTime<Utc>) -> bool {
        let manager = self.emergency_override_manager.read().await;
        manager.active_overrides.iter().any(|override_| {
            override_.start_time <= time && 
            override_.end_time.map_or(true, |end| end > time)
        })
    }
    
    /// Find next optimal training window
    pub async fn find_next_training_window(
        &self, 
        start: DateTime<Utc>, 
        min_duration: Duration,
        min_quality: TrainingWindow,
    ) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        let mut current = start;
        let end_search = start + Duration::days(7); // Search up to 7 days ahead
        
        while current < end_search {
            let window = self.get_training_window(current).await;
            
            if window <= min_quality {
                // Found start of potential window
                let mut window_end = current;
                
                // Find how long the window lasts
                while window_end < end_search {
                    let next_window = self.get_training_window(window_end).await;
                    if next_window > min_quality {
                        break;
                    }
                    window_end = window_end + Duration::minutes(15);
                }
                
                // Check if window is long enough
                if window_end - current >= min_duration {
                    return Some((current, window_end));
                }
            }
            
            current = current + Duration::minutes(15);
        }
        
        None
    }
    
    /// Check if a date is a holiday for an exchange
    async fn is_holiday(&self, exchange: Exchange, date: DateTime<Utc>) -> bool {
        let calendar = self.holiday_calendar.read().await;
        if let Some(holidays) = calendar.holidays.get(&exchange) {
            holidays.iter().any(|h| h.date_naive() == date.date_naive())
        } else {
            false
        }
    }
    
    /// Update holiday calendar
    pub async fn update_holidays(&self, exchange: Exchange, holidays: Vec<DateTime<Utc>>) {
        let mut calendar = self.holiday_calendar.write().await;
        calendar.holidays.insert(exchange, holidays);
        calendar.last_update = Utc::now();
    }
    
    /// Get resource limit recommendation based on market intensity and policy
    pub async fn get_resource_limit(&self, time: DateTime<Utc>) -> f64 {
        // Check for emergency override
        if let Some(override_) = self.get_active_emergency_override(time).await {
            return override_.requested_resources.min(1.0);
        }
        
        let intensity = self.get_market_intensity(time).await;
        let policy = self.resource_allocation_policy.read().await;
        
        // Base allocation based on market state
        let mut allocation = match intensity.dominant_session {
            MarketSession::Regular => policy.market_hours_allocation,
            MarketSession::PreMarket | MarketSession::AfterHours => {
                (policy.market_hours_allocation + policy.off_hours_allocation) / 2.0
            },
            MarketSession::Closed => {
                if time.weekday() == Weekday::Sat || time.weekday() == Weekday::Sun {
                    policy.weekend_allocation
                } else {
                    policy.off_hours_allocation
                }
            },
        };
        
        // Apply multi-market penalty
        if intensity.active_exchanges > 1 {
            let penalty = policy.multi_market_penalty * (intensity.active_exchanges - 1) as f64;
            allocation = (allocation - penalty).max(policy.min_allocation);
        }
        
        // Apply volatility adjustment
        allocation = (allocation + policy.volatility_adjustment).clamp(policy.min_allocation, 1.0);
        
        allocation
    }
    
    /// Get active emergency override if any
    async fn get_active_emergency_override(&self, time: DateTime<Utc>) -> Option<EmergencyOverride> {
        let manager = self.emergency_override_manager.read().await;
        manager.active_overrides.iter()
            .find(|o| o.start_time <= time && o.end_time.map_or(true, |end| end > time))
            .cloned()
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
        let mut manager = self.emergency_override_manager.write().await;
        
        // Check if circuit breaker is active
        if manager.circuit_breaker_active {
            return Err(anyhow::anyhow!("Circuit breaker active, cannot create new overrides"));
        }
        
        // Check concurrent override limit
        if manager.active_overrides.len() >= manager.max_concurrent_overrides {
            return Err(anyhow::anyhow!("Maximum concurrent overrides reached"));
        }
        
        let override_ = EmergencyOverride {
            id: Uuid::new_v4().to_string(),
            reason,
            priority,
            requested_resources: requested_resources.clamp(0.0, 1.0),
            start_time: Utc::now(),
            end_time: Some(Utc::now() + duration),
            approved_by,
            affected_exchanges,
            metadata: HashMap::new(),
        };
        
        let id = override_.id.clone();
        manager.active_overrides.push(override_.clone());
        manager.override_history.push(override_);
        
        Ok(id)
    }
    
    /// Cancel emergency override
    pub async fn cancel_emergency_override(&self, override_id: &str) -> Result<()> {
        let mut manager = self.emergency_override_manager.write().await;
        
        if let Some(pos) = manager.active_overrides.iter().position(|o| o.id == override_id) {
            let mut override_ = manager.active_overrides.remove(pos);
            override_.end_time = Some(Utc::now());
            manager.override_history.push(override_);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Override not found"))
        }
    }
    
    /// Get all active exchanges at a given time with their status
    pub async fn get_active_exchanges(&self, time: DateTime<Utc>) -> Vec<(Exchange, MarketSession)> {
        let mut active = Vec::new();
        
        for exchange in self.schedules.keys() {
            let session = self.get_session(*exchange, time).await;
            if session != MarketSession::Closed {
                active.push((*exchange, session));
            }
        }
        
        active
    }
    
    /// Get comprehensive market status
    pub async fn get_market_status(&self, exchange: Exchange, time: DateTime<Utc>) -> Option<MarketStatus> {
        // Check cache first
        {
            let cache = self.market_status_cache.read().await;
            if let Some(status) = cache.get(&exchange) {
                if status.next_state_change > time {
                    return Some(status.clone());
                }
            }
        }
        
        if let Some(schedule) = self.schedules.get(&exchange) {
            let session = self.get_session(exchange, time).await;
            let is_trading = matches!(session, MarketSession::Regular);
            let is_holiday = self.is_holiday(exchange, time).await;
            
            // Calculate next state change
            let local_time = self.convert_to_exchange_time(time, exchange);
            let next_change = self.calculate_next_state_change(&schedule, local_time);
            
            // Get volatility level from correlation matrix
            let volatility_level = self.get_volatility_level(exchange).await;
            
            // Check circuit breaker status
            let circuit_breaker_active = false; // Would check actual market data in production
            
            let status = MarketStatus {
                exchange,
                session,
                is_trading,
                next_state_change: next_change,
                volatility_level,
                circuit_breaker_active,
                special_conditions: vec![],
            };
            
            // Update cache
            {
                let mut cache = self.market_status_cache.write().await;
                cache.insert(exchange, status.clone());
            }
            
            Some(status)
        } else {
            None
        }
    }
    
    /// Calculate next state change for an exchange
    fn calculate_next_state_change(&self, schedule: &ExchangeSchedule, local_time: DateTime<Utc>) -> DateTime<Utc> {
        let current_time = local_time.time();
        let current_date = local_time.date_naive();
        
        // If before market open
        if current_time < schedule.regular_hours.open {
            return Utc.from_utc_datetime(&current_date.and_time(schedule.regular_hours.open));
        }
        
        // If during market hours
        if current_time < schedule.regular_hours.close {
            return Utc.from_utc_datetime(&current_date.and_time(schedule.regular_hours.close));
        }
        
        // If after market close, next open is next trading day
        let mut next_date = current_date + Duration::days(1);
        while !schedule.regular_hours.trading_days.contains(&next_date.weekday()) {
            next_date = next_date + Duration::days(1);
        }
        
        Utc.from_utc_datetime(&next_date.and_time(schedule.regular_hours.open))
    }
    
    /// Get volatility level for an exchange
    async fn get_volatility_level(&self, exchange: Exchange) -> VolatilityLevel {
        let matrix = self.market_correlation_matrix.read().await;
        
        if let Some(volatility) = matrix.volatility_indices.get(&exchange) {
            match *volatility {
                v if v < 0.1 => VolatilityLevel::Low,
                v if v < 0.2 => VolatilityLevel::Normal,
                v if v < 0.3 => VolatilityLevel::Elevated,
                v if v < 0.5 => VolatilityLevel::High,
                _ => VolatilityLevel::Extreme,
            }
        } else {
            VolatilityLevel::Normal
        }
    }
    
    /// Update market correlation matrix
    pub async fn update_correlation_matrix(
        &self,
        correlations: HashMap<(Exchange, Exchange), f64>,
        impact_scores: HashMap<Exchange, f64>,
        volatility_indices: HashMap<Exchange, f64>,
    ) {
        let mut matrix = self.market_correlation_matrix.write().await;
        matrix.correlations = correlations;
        matrix.impact_scores = impact_scores;
        matrix.volatility_indices = volatility_indices;
        matrix.last_update = Utc::now();
    }
    
    /// Get scheduling recommendations based on current market conditions
    pub async fn get_scheduling_recommendations(&self, time: DateTime<Utc>) -> SchedulingRecommendations {
        let intensity = self.get_market_intensity(time).await;
        let resource_limit = self.get_resource_limit(time).await;
        let training_window = self.get_training_window(time).await;
        let active_exchanges = self.get_active_exchanges(time).await;
        
        // Find next optimal windows
        let next_optimal = self.find_next_training_window(
            time, 
            Duration::hours(2), 
            TrainingWindow::Optimal
        ).await;
        
        let next_good = self.find_next_training_window(
            time, 
            Duration::hours(1), 
            TrainingWindow::Good
        ).await;
        
        SchedulingRecommendations {
            current_window: training_window,
            resource_limit,
            intensity_score: intensity.score,
            active_markets: active_exchanges.len(),
            next_optimal_window: next_optimal,
            next_good_window: next_good,
            recommendation: Self::generate_recommendation(training_window, resource_limit),
        }
    }
    
    /// Generate scheduling recommendation message
    fn generate_recommendation(window: TrainingWindow, resource_limit: f64) -> String {
        match window {
            TrainingWindow::Optimal => {
                format!("Optimal training window. Recommended resource allocation: {:.0}%", resource_limit * 100.0)
            },
            TrainingWindow::Good => {
                format!("Good training window. Recommended resource allocation: {:.0}%", resource_limit * 100.0)
            },
            TrainingWindow::Acceptable => {
                format!("Acceptable training window. Limited to {:.0}% resources due to market activity", resource_limit * 100.0)
            },
            TrainingWindow::Poor => {
                format!("Poor training window. Restrict to {:.0}% resources. Consider postponing non-critical training", resource_limit * 100.0)
            },
            TrainingWindow::Restricted => {
                format!("Restricted window - peak market hours. Only {:.0}% resources available. Defer training if possible", resource_limit * 100.0)
            },
        }
    }
}

/// Scheduling recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulingRecommendations {
    pub current_window: TrainingWindow,
    pub resource_limit: f64,
    pub intensity_score: f64,
    pub active_markets: usize,
    pub next_optimal_window: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub next_good_window: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub recommendation: String,
}

impl Default for MarketHours {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TrainingWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrainingWindow::Optimal => write!(f, "Optimal"),
            TrainingWindow::Good => write!(f, "Good"),
            TrainingWindow::Acceptable => write!(f, "Acceptable"),
            TrainingWindow::Poor => write!(f, "Poor"),
            TrainingWindow::Restricted => write!(f, "Restricted"),
        }
    }
}

impl std::cmp::PartialOrd for TrainingWindow {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for TrainingWindow {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Lower values are better
        let self_val = match self {
            TrainingWindow::Optimal => 0,
            TrainingWindow::Good => 1,
            TrainingWindow::Acceptable => 2,
            TrainingWindow::Poor => 3,
            TrainingWindow::Restricted => 4,
        };
        
        let other_val = match other {
            TrainingWindow::Optimal => 0,
            TrainingWindow::Good => 1,
            TrainingWindow::Acceptable => 2,
            TrainingWindow::Poor => 3,
            TrainingWindow::Restricted => 4,
        };
        
        self_val.cmp(&other_val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_market_hours_creation() {
        let market_hours = MarketHours::new();
        assert!(market_hours.schedules.contains_key(&Exchange::NYSE));
        assert!(market_hours.schedules.contains_key(&Exchange::LSE));
        assert!(market_hours.schedules.contains_key(&Exchange::TSE));
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
        
        // Market hours should get low allocation
        let tuesday_market = Utc.with_ymd_and_hms(2024, 1, 9, 14, 30, 0).unwrap(); // 9:30 AM EST
        let limit = market_hours.get_resource_limit(tuesday_market).await;
        assert!(limit <= 0.30);
    }
    
    #[tokio::test]
    async fn test_emergency_override() {
        let market_hours = MarketHours::new();
        
        // Request emergency override
        let override_id = market_hours.request_emergency_override(
            "Critical model update required".to_string(),
            EmergencyPriority::Critical,
            0.8,
            Duration::hours(2),
            "system_admin".to_string(),
            vec![Exchange::NYSE, Exchange::NASDAQ],
        ).await.unwrap();
        
        // Check that override is active
        let now = Utc::now();
        let has_override = market_hours.has_emergency_override(now).await;
        assert!(has_override);
        
        // Cancel override
        market_hours.cancel_emergency_override(&override_id).await.unwrap();
        
        // Check that override is no longer active
        let has_override = market_hours.has_emergency_override(now).await;
        assert!(!has_override);
    }
}