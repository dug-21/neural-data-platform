//! Market scheduling logic and session management
//! 
//! Provides core scheduling functionality including market session detection,
//! intensity calculations, and training window optimization.

use chrono::{DateTime, Duration, Utc, Weekday, Datelike, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;
use anyhow::{Context, Result};

use crate::utils::market_hours::{
    exchanges::{Exchange, ExchangeManager},
    timezone::TimezoneConverter,
    holidays::HolidayManager,
    config::{
        TrainingWindow, MarketIntensity, ResourceAllocationPolicy, 
        EmergencyOverrideManager, SchedulingRecommendations, VolatilityLevel
    },
};

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

    pub fn update_correlations(&mut self, correlations: HashMap<(Exchange, Exchange), f64>) {
        self.correlations = correlations;
        self.last_update = Utc::now();
    }

    pub fn update_impact_scores(&mut self, impact_scores: HashMap<Exchange, f64>) {
        self.impact_scores = impact_scores;
        self.last_update = Utc::now();
    }

    pub fn update_volatility_indices(&mut self, volatility_indices: HashMap<Exchange, f64>) {
        self.volatility_indices = volatility_indices;
        self.last_update = Utc::now();
    }

    pub fn get_correlation(&self, exchange1: Exchange, exchange2: Exchange) -> Option<f64> {
        self.correlations.get(&(exchange1, exchange2))
            .or_else(|| self.correlations.get(&(exchange2, exchange1)))
            .copied()
    }

    pub fn get_volatility(&self, exchange: Exchange) -> Option<f64> {
        self.volatility_indices.get(&exchange).copied()
    }

    pub fn get_impact_score(&self, exchange: Exchange) -> Option<f64> {
        self.impact_scores.get(&exchange).copied()
    }
}

/// Market scheduler with comprehensive timing logic
pub struct MarketScheduler {
    exchange_manager: ExchangeManager,
    timezone_converter: TimezoneConverter,
    holiday_manager: Arc<HolidayManager>,
    intensity_cache: Arc<RwLock<HashMap<DateTime<Utc>, MarketIntensity>>>,
    resource_allocation_policy: Arc<RwLock<ResourceAllocationPolicy>>,
    emergency_override_manager: Arc<RwLock<EmergencyOverrideManager>>,
    market_correlation_matrix: Arc<RwLock<MarketCorrelationMatrix>>,
    market_status_cache: Arc<RwLock<HashMap<Exchange, MarketStatus>>>,
}

impl MarketScheduler {
    pub fn new() -> Self {
        Self {
            exchange_manager: ExchangeManager::new(),
            timezone_converter: TimezoneConverter::new(),
            holiday_manager: Arc::new(HolidayManager::new()),
            intensity_cache: Arc::new(RwLock::new(HashMap::new())),
            resource_allocation_policy: Arc::new(RwLock::new(ResourceAllocationPolicy::default())),
            emergency_override_manager: Arc::new(RwLock::new(EmergencyOverrideManager::new(3))),
            market_correlation_matrix: Arc::new(RwLock::new(MarketCorrelationMatrix::new())),
            market_status_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Check if a specific exchange is open
    pub async fn is_exchange_open(&self, exchange: Exchange, time: DateTime<Utc>) -> bool {
        if let Some(schedule) = self.exchange_manager.get_schedule(exchange) {
            // Check if it's a holiday
            if self.holiday_manager.is_holiday(exchange, time).await {
                return false;
            }
            
            // Convert to exchange local time with DST adjustment
            let local_time = self.timezone_converter.convert_with_dst(time, exchange);
            
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

    /// Get current market session for an exchange
    pub async fn get_session(&self, exchange: Exchange, time: DateTime<Utc>) -> MarketSession {
        if let Some(schedule) = self.exchange_manager.get_schedule(exchange) {
            if self.holiday_manager.is_holiday(exchange, time).await {
                return MarketSession::Closed;
            }
            
            let local_time = self.timezone_converter.convert_with_dst(time, exchange);
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
        
        for exchange in self.exchange_manager.get_all_exchanges() {
            match self.get_session(exchange, time).await {
                MarketSession::Regular => {
                    active_regular += 1;
                    active_exchanges.push(exchange);
                },
                MarketSession::PreMarket | MarketSession::AfterHours => {
                    active_extended += 1;
                    active_exchanges.push(exchange);
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
                if let Some(correlation) = matrix.get_correlation(active_exchanges[i], active_exchanges[j]) {
                    // High correlation means overlapping impact
                    if correlation > 0.7 {
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
        {
            let manager = self.emergency_override_manager.read().await;
            if manager.has_active_override(time) {
                return TrainingWindow::Optimal; // Emergency overrides get optimal window
            }
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
        
        // Classify based on intensity
        match intensity.score {
            s if s < 0.1 => TrainingWindow::Optimal,
            s if s < 0.3 => TrainingWindow::Good,
            s if s < 0.5 => TrainingWindow::Acceptable,
            s if s < 0.8 => TrainingWindow::Poor,
            _ => TrainingWindow::Restricted,
        }
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

    /// Get resource limit recommendation based on market intensity and policy
    pub async fn get_resource_limit(&self, time: DateTime<Utc>) -> f64 {
        // Check for emergency override
        {
            let manager = self.emergency_override_manager.read().await;
            if let Some(override_) = manager.get_active_override(time) {
                return override_.requested_resources.min(1.0);
            }
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

    /// Get all active exchanges at a given time with their status
    pub async fn get_active_exchanges(&self, time: DateTime<Utc>) -> Vec<(Exchange, MarketSession)> {
        let mut active = Vec::new();
        
        for exchange in self.exchange_manager.get_all_exchanges() {
            let session = self.get_session(exchange, time).await;
            if session != MarketSession::Closed {
                active.push((exchange, session));
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
        
        if let Some(schedule) = self.exchange_manager.get_schedule(exchange) {
            let session = self.get_session(exchange, time).await;
            let is_trading = matches!(session, MarketSession::Regular);
            
            // Calculate next state change
            let local_time = self.timezone_converter.convert_to_exchange_time(time, exchange);
            let next_change = self.calculate_next_state_change(schedule, local_time);
            
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
    fn calculate_next_state_change(&self, schedule: &crate::utils::market_hours::exchanges::ExchangeSchedule, local_time: DateTime<Utc>) -> DateTime<Utc> {
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
        
        if let Some(volatility) = matrix.get_volatility(exchange) {
            match volatility {
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
        matrix.update_correlations(correlations);
        matrix.update_impact_scores(impact_scores);
        matrix.update_volatility_indices(volatility_indices);
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
            recommendation: SchedulingRecommendations::generate_recommendation(training_window, resource_limit),
        }
    }

    /// Request emergency override
    pub async fn request_emergency_override(
        &self,
        reason: String,
        priority: crate::utils::market_hours::config::EmergencyPriority,
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
        if !manager.can_add_override() {
            return Err(anyhow::anyhow!("Maximum concurrent overrides reached"));
        }
        
        let override_ = crate::utils::market_hours::config::EmergencyOverride {
            id: uuid::Uuid::new_v4().to_string(),
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
        manager.add_override(override_);
        
        Ok(id)
    }

    /// Cancel emergency override
    pub async fn cancel_emergency_override(&self, override_id: &str) -> Result<()> {
        let mut manager = self.emergency_override_manager.write().await;
        
        if manager.remove_override(override_id) {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Override not found"))
        }
    }
}

impl Default for MarketScheduler {
    fn default() -> Self {
        Self::new()
    }
}