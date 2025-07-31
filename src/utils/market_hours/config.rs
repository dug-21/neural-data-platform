//! Market hours configuration and resource allocation policies
//! 
//! Provides configuration structures for market timing systems and resource 
//! allocation strategies based on market conditions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// Emergency priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EmergencyPriority {
    Low,
    Medium,
    High,
    Critical,
    SystemCritical,
}

/// Emergency override details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergencyOverride {
    pub id: String,
    pub reason: String,
    pub priority: EmergencyPriority,
    pub requested_resources: f64,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub approved_by: String,
    pub affected_exchanges: Vec<crate::utils::market_hours::exchanges::Exchange>,
    pub metadata: HashMap<String, String>,
}

/// Emergency override manager
#[derive(Debug, Clone)]
pub struct EmergencyOverrideManager {
    pub(crate) active_overrides: Vec<EmergencyOverride>,
    pub(crate) override_history: Vec<EmergencyOverride>,
    pub(crate) max_concurrent_overrides: usize,
    pub(crate) circuit_breaker_active: bool,
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

    pub fn has_active_override(&self, time: chrono::DateTime<chrono::Utc>) -> bool {
        self.active_overrides.iter().any(|override_| {
            override_.start_time <= time && 
            override_.end_time.map_or(true, |end| end > time)
        })
    }

    pub fn get_active_override(&self, time: chrono::DateTime<chrono::Utc>) -> Option<EmergencyOverride> {
        self.active_overrides.iter()
            .find(|o| o.start_time <= time && o.end_time.map_or(true, |end| end > time))
            .cloned()
    }

    pub fn can_add_override(&self) -> bool {
        !self.circuit_breaker_active && 
        self.active_overrides.len() < self.max_concurrent_overrides
    }

    pub fn add_override(&mut self, override_: EmergencyOverride) {
        self.active_overrides.push(override_.clone());
        self.override_history.push(override_);
    }

    pub fn remove_override(&mut self, override_id: &str) -> bool {
        if let Some(pos) = self.active_overrides.iter().position(|o| o.id == override_id) {
            let mut override_ = self.active_overrides.remove(pos);
            override_.end_time = Some(chrono::Utc::now());
            self.override_history.push(override_);
            true
        } else {
            false
        }
    }
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
    pub applies_after_time: chrono::NaiveTime,
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

/// Training window suitability for model training
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

/// Market intensity level
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MarketIntensity {
    /// Overall intensity score (0.0 = no activity, 1.0 = peak activity)
    pub score: f64,
    /// Number of active major exchanges
    pub active_exchanges: usize,
    /// Primary session type across exchanges
    pub dominant_session: crate::utils::market_hours::scheduler::MarketSession,
    /// Estimated global trading volume percentage
    pub volume_estimate: f64,
}

/// Scheduling recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulingRecommendations {
    pub current_window: TrainingWindow,
    pub resource_limit: f64,
    pub intensity_score: f64,
    pub active_markets: usize,
    pub next_optimal_window: Option<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>,
    pub next_good_window: Option<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>,
    pub recommendation: String,
}

impl SchedulingRecommendations {
    /// Generate scheduling recommendation message
    pub fn generate_recommendation(window: TrainingWindow, resource_limit: f64) -> String {
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