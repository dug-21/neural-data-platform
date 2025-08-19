//! Emergency Trading Controls
//!
//! Provides immediate stop/resume capabilities for trading operations

use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct EmergencyControls {
    state: Arc<RwLock<EmergencyState>>,
}

#[derive(Debug, Clone)]
struct EmergencyState {
    is_stopped: bool,
    stop_reason: Option<String>,
    stop_timestamp: Option<DateTime<Utc>>,
    stop_count: u32,
    last_resume_timestamp: Option<DateTime<Utc>>,
}

impl Default for EmergencyState {
    fn default() -> Self {
        Self {
            is_stopped: false,
            stop_reason: None,
            stop_timestamp: None,
            stop_count: 0,
            last_resume_timestamp: None,
        }
    }
}

impl EmergencyControls {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(EmergencyState::default())),
        }
    }
    
    /// Activate emergency stop
    pub async fn activate_stop(&self, reason: &str) {
        let mut state = self.state.write().await;
        state.is_stopped = true;
        state.stop_reason = Some(reason.to_string());
        state.stop_timestamp = Some(Utc::now());
        state.stop_count += 1;
        
        // Log the emergency stop
        tracing::error!("🚨 EMERGENCY STOP ACTIVATED: {} (Stop #{:?})", reason, state.stop_count);
    }
    
    /// Deactivate emergency stop and resume trading
    pub async fn deactivate_stop(&self) {
        let mut state = self.state.write().await;
        if state.is_stopped {
            state.is_stopped = false;
            state.last_resume_timestamp = Some(Utc::now());
            
            let stop_duration = state.last_resume_timestamp.unwrap()
                .signed_duration_since(state.stop_timestamp.unwrap_or(Utc::now()))
                .num_seconds();
            
            tracing::info!("✅ Trading resumed after {} seconds", stop_duration);
        }
    }
    
    /// Check if emergency stop is active
    pub async fn is_stopped(&self) -> bool {
        let state = self.state.read().await;
        state.is_stopped
    }
    
    /// Get current emergency state
    pub async fn get_state(&self) -> EmergencyStatus {
        let state = self.state.read().await;
        EmergencyStatus {
            is_stopped: state.is_stopped,
            stop_reason: state.stop_reason.clone(),
            stop_timestamp: state.stop_timestamp,
            stop_count: state.stop_count,
            last_resume_timestamp: state.last_resume_timestamp,
            uptime_seconds: state.last_resume_timestamp
                .or(state.stop_timestamp)
                .map(|ts| Utc::now().signed_duration_since(ts).num_seconds())
                .unwrap_or(0),
        }
    }
    
    /// Force immediate stop with critical priority
    pub async fn force_stop(&self, reason: &str) {
        self.activate_stop(&format!("FORCE STOP: {}", reason)).await;
    }
    
    /// Check if system can resume (basic safety checks)
    pub async fn can_resume(&self) -> (bool, Vec<String>) {
        let state = self.state.read().await;
        let mut issues = Vec::new();
        
        // Basic safety checks
        if let Some(stop_time) = state.stop_timestamp {
            let minutes_since_stop = Utc::now()
                .signed_duration_since(stop_time)
                .num_minutes();
            
            if minutes_since_stop < 1 {
                issues.push("Must wait at least 1 minute before resuming".to_string());
            }
        }
        
        // Check if too many stops in short period (circuit breaker)
        if state.stop_count >= 5 {
            issues.push("Too many emergency stops - manual intervention required".to_string());
        }
        
        (issues.is_empty(), issues)
    }
    
    /// Reset emergency controls (for maintenance/testing)
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        *state = EmergencyState::default();
        tracing::info!("Emergency controls reset");
    }
    
    /// Get stop history summary
    pub async fn get_stop_history(&self) -> StopHistory {
        let state = self.state.read().await;
        StopHistory {
            total_stops: state.stop_count,
            current_stop_reason: state.stop_reason.clone(),
            last_stop_time: state.stop_timestamp,
            last_resume_time: state.last_resume_timestamp,
            is_currently_stopped: state.is_stopped,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmergencyStatus {
    pub is_stopped: bool,
    pub stop_reason: Option<String>,
    pub stop_timestamp: Option<DateTime<Utc>>,
    pub stop_count: u32,
    pub last_resume_timestamp: Option<DateTime<Utc>>,
    pub uptime_seconds: i64,
}

#[derive(Debug, Clone)]
pub struct StopHistory {
    pub total_stops: u32,
    pub current_stop_reason: Option<String>,
    pub last_stop_time: Option<DateTime<Utc>>,
    pub last_resume_time: Option<DateTime<Utc>>,
    pub is_currently_stopped: bool,
}

/// Emergency stop triggers
pub enum StopTrigger {
    ManualStop,
    RiskLimitBreach,
    SystemError,
    MarketConditions,
    ConnectionLoss,
    UnknownError,
}

impl StopTrigger {
    pub fn to_reason(&self) -> &'static str {
        match self {
            StopTrigger::ManualStop => "Manual emergency stop activated",
            StopTrigger::RiskLimitBreach => "Risk limits breached - emergency stop",
            StopTrigger::SystemError => "Critical system error detected",
            StopTrigger::MarketConditions => "Extreme market conditions detected",
            StopTrigger::ConnectionLoss => "Connection to broker lost",
            StopTrigger::UnknownError => "Unknown error - emergency stop",
        }
    }
}

impl EmergencyControls {
    /// Convenience method to stop with predefined triggers
    pub async fn stop_with_trigger(&self, trigger: StopTrigger) {
        self.activate_stop(trigger.to_reason()).await;
    }
}