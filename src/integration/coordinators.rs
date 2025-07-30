//! Integration Coordinators
//!
//! Specialized coordinators that implement specific integration patterns between
//! system components, providing autonomous coordination capabilities.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, warn};

use super::integration_hub::{IntegrationEvent, MarketSignalType, TrainingUrgency, HealthStatus};
use crate::neural::monitoring::{PerformanceEvent, PerformanceEventType, TrainingNotification, TrainingPriority, TrainingAction};
use crate::data::TimeSeriesData;
use crate::strategies::MarketContext;

/// Result type for coordinator operations
pub type CoordinationResult<T> = Result<T, CoordinationError>;

/// Coordination error types
#[derive(Debug, thiserror::Error)]
pub enum CoordinationError {
    #[error("Communication error: {0}")]
    Communication(String),
    
    #[error("Timeout error: operation timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Processing error: {0}")]
    Processing(String),
    
    #[error("Integration error: {component} failed with {reason}")]
    Integration { component: String, reason: String },
}

/// Performance analysis result
#[derive(Debug, Clone)]
pub struct PerformanceTrend {
    pub trend_type: PerformanceTrendType,
    pub severity: f64,
    pub confidence: f64,
    pub requires_action: bool,
    pub recommended_action: Option<String>,
}

/// Performance trend types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceTrendType {
    AccuracyDegrading(f64),
    ConfidenceDropping(f64),
    LatencyIncreasing(u64),
    ErrorIncreasing(String),
    Stable,
    Improving,
    Unknown,
}

/// Market timing analysis result
#[derive(Debug, Clone)]
pub struct TimingAnalysis {
    pub signal_strength: f64,
    pub market_context: MarketContext,
    pub risk_factors: Vec<String>,
    pub confidence: f64,
    pub recommended_urgency: CoordinationUrgency,
}

/// Coordination urgency levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum CoordinationUrgency {
    Immediate = 0,
    High = 1,
    Medium = 2,
    Low = 3,
}

/// Training thresholds for performance evaluation
#[derive(Debug, Clone)]
pub struct TrainingThresholds {
    pub min_accuracy: f64,
    pub min_confidence: f64,
    pub max_latency_ms: u64,
    pub max_error_rate: f64,
    pub lookback_window_mins: u64,
}

impl Default for TrainingThresholds {
    fn default() -> Self {
        Self {
            min_accuracy: 0.7,
            min_confidence: 0.65,
            max_latency_ms: 1000,
            max_error_rate: 0.1,
            lookback_window_mins: 60,
        }
    }
}

/// Performance degradation analysis state
#[derive(Debug, Clone, Default)]
pub struct PerformanceDecisionState {
    pub model_performance_history: HashMap<String, Vec<PerformanceSnapshot>>,
    pub last_training_decisions: HashMap<String, DateTime<Utc>>,
    pub active_alerts: Vec<PerformanceAlert>,
}

/// Performance snapshot for trend analysis
#[derive(Debug, Clone)]
pub struct PerformanceSnapshot {
    pub timestamp: DateTime<Utc>,
    pub accuracy: f64,
    pub confidence: f64,
    pub latency_ms: u64,
    pub error_count: u32,
}

/// Performance alert structure
#[derive(Debug, Clone)]
pub struct PerformanceAlert {
    pub model_name: String,
    pub alert_type: String,
    pub severity: f64,
    pub created_at: DateTime<Utc>,
    pub acknowledged: bool,
}

/// Training command for autonomous training
#[derive(Debug, Clone)]
pub struct TrainingCommand {
    pub model_name: String,
    pub urgency: TrainingUrgency,
    pub trigger_reason: String,
    pub performance_context: HashMap<String, f64>,
    pub estimated_duration_mins: u32,
    pub priority: TrainingPriority,
}

/// DAA coordination event types
#[derive(Debug, Clone)]
pub enum DaaCoordinationEvent {
    StrongBuySignal {
        confidence: f64,
        market_context: MarketContext,
        urgency: CoordinationUrgency,
    },
    StrongSellSignal {
        confidence: f64,
        market_context: MarketContext,
        risk_factors: Vec<String>,
    },
    VolatilityAlert {
        current_volatility: f64,
        threshold: f64,
        recommended_action: String,
    },
    MarketStateChange {
        from_state: String,
        to_state: String,
        confidence: f64,
    },
}

/// Market coordination state tracking
#[derive(Debug, Clone, Default)]
pub struct MarketCoordinationState {
    pub current_market_state: String,
    pub last_signal_time: Option<DateTime<Utc>>,
    pub signal_history: Vec<MarketSignalHistory>,
    pub active_positions: HashMap<String, f64>,
}

/// Market signal history for analysis
#[derive(Debug, Clone)]
pub struct MarketSignalHistory {
    pub timestamp: DateTime<Utc>,
    pub signal_type: MarketSignalType,
    pub confidence: f64,
    pub outcome: Option<String>,
}

/// Performance → Training Integration Coordinator
pub struct PerformanceCoordinator {
    performance_rx: broadcast::Receiver<PerformanceEvent>,
    training_tx: mpsc::Sender<TrainingCommand>,
    thresholds: TrainingThresholds,
    decision_state: Arc<RwLock<PerformanceDecisionState>>,
}

impl PerformanceCoordinator {
    pub fn new(
        performance_rx: broadcast::Receiver<PerformanceEvent>,
        training_tx: mpsc::Sender<TrainingCommand>,
        thresholds: TrainingThresholds,
    ) -> Self {
        Self {
            performance_rx,
            training_tx,
            thresholds,
            decision_state: Arc::new(RwLock::new(PerformanceDecisionState::default())),
        }
    }
    
    /// Start coordination between performance events and training decisions
    pub async fn coordinate_training_decisions(&mut self) -> CoordinationResult<()> {
        info!("Starting Performance-Training coordination");
        
        while let Ok(performance_event) = self.performance_rx.recv().await {
            // Analyze performance trend
            let analysis = self.analyze_performance_trend(&performance_event).await?;
            
            // Check if training is needed based on analysis
            if let Some(training_command) = self.evaluate_training_need(analysis).await? {
                // Send training command
                match self.training_tx.send(training_command.clone()).await {
                    Ok(_) => {
                        info!("Training command sent for model: {}", training_command.model_name);
                        
                        // Update decision state
                        self.update_decision_state(&performance_event, &training_command).await;
                    }
                    Err(e) => {
                        error!("Failed to send training command: {}", e);
                        return Err(CoordinationError::Communication(e.to_string()));
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Analyze performance trends to determine training needs
    async fn analyze_performance_trend(&self, event: &PerformanceEvent) -> CoordinationResult<PerformanceTrend> {
        let trend = match &event.event_type {
            PerformanceEventType::PredictionCompleted { model, accuracy, confidence, latency_ms, .. } => {
                // Store performance snapshot
                self.store_performance_snapshot(model, *accuracy, *confidence, *latency_ms).await;
                
                // Analyze trends
                if *accuracy < self.thresholds.min_accuracy {
                    PerformanceTrend {
                        trend_type: PerformanceTrendType::AccuracyDegrading(*accuracy),
                        severity: (self.thresholds.min_accuracy - accuracy) / self.thresholds.min_accuracy,
                        confidence: 0.9,
                        requires_action: true,
                        recommended_action: Some("retrain_model".to_string()),
                    }
                } else if *confidence < self.thresholds.min_confidence {
                    PerformanceTrend {
                        trend_type: PerformanceTrendType::ConfidenceDropping(*confidence),
                        severity: (self.thresholds.min_confidence - confidence) / self.thresholds.min_confidence,
                        confidence: 0.8,
                        requires_action: true,
                        recommended_action: Some("increase_training_data".to_string()),
                    }
                } else if *latency_ms > self.thresholds.max_latency_ms {
                    PerformanceTrend {
                        trend_type: PerformanceTrendType::LatencyIncreasing(*latency_ms),
                        severity: (*latency_ms as f64 - self.thresholds.max_latency_ms as f64) / self.thresholds.max_latency_ms as f64,
                        confidence: 0.95,
                        requires_action: true,
                        recommended_action: Some("optimize_model".to_string()),
                    }
                } else {
                    PerformanceTrend {
                        trend_type: PerformanceTrendType::Stable,
                        severity: 0.0,
                        confidence: 0.9,
                        requires_action: false,
                        recommended_action: None,
                    }
                }
            }
            
            PerformanceEventType::Alert { message, .. } => {
                PerformanceTrend {
                    trend_type: PerformanceTrendType::ErrorIncreasing(message.clone()),
                    severity: 0.8,
                    confidence: 0.7,
                    requires_action: true,
                    recommended_action: Some("investigate_errors".to_string()),
                }
            }
            
            _ => PerformanceTrend {
                trend_type: PerformanceTrendType::Unknown,
                severity: 0.0,
                confidence: 0.5,
                requires_action: false,
                recommended_action: None,
            }
        };
        
        Ok(trend)
    }
    
    /// Evaluate if training is needed based on performance trend
    async fn evaluate_training_need(&self, trend: PerformanceTrend) -> CoordinationResult<Option<TrainingCommand>> {
        if !trend.requires_action {
            return Ok(None);
        }
        
        let training_command = match trend.trend_type {
            PerformanceTrendType::AccuracyDegrading(accuracy) => {
                let urgency = if accuracy < 0.5 {
                    TrainingUrgency::Critical
                } else if accuracy < 0.6 {
                    TrainingUrgency::High
                } else {
                    TrainingUrgency::Medium
                };
                
                Some(TrainingCommand {
                    model_name: "accuracy_degraded_model".to_string(), // Would be extracted from event
                    urgency,
                    trigger_reason: format!("Accuracy degraded to {:.3}", accuracy),
                    performance_context: HashMap::from([
                        ("current_accuracy".to_string(), accuracy),
                        ("threshold".to_string(), self.thresholds.min_accuracy),
                        ("severity".to_string(), trend.severity),
                    ]),
                    estimated_duration_mins: match urgency {
                        TrainingUrgency::Critical => 15,
                        TrainingUrgency::High => 30,
                        _ => 60,
                    },
                    priority: match urgency {
                        TrainingUrgency::Critical => TrainingPriority::Critical,
                        TrainingUrgency::High => TrainingPriority::High,
                        _ => TrainingPriority::Medium,
                    },
                })
            }
            
            PerformanceTrendType::ConfidenceDropping(confidence) => {
                Some(TrainingCommand {
                    model_name: "confidence_dropped_model".to_string(),
                    urgency: TrainingUrgency::Medium,
                    trigger_reason: format!("Confidence dropped to {:.3}", confidence),
                    performance_context: HashMap::from([
                        ("current_confidence".to_string(), confidence),
                        ("threshold".to_string(), self.thresholds.min_confidence),
                    ]),
                    estimated_duration_mins: 45,
                    priority: TrainingPriority::Medium,
                })
            }
            
            PerformanceTrendType::LatencyIncreasing(latency_ms) => {
                Some(TrainingCommand {
                    model_name: "high_latency_model".to_string(),
                    urgency: TrainingUrgency::Low,
                    trigger_reason: format!("Latency increased to {}ms", latency_ms),
                    performance_context: HashMap::from([
                        ("current_latency_ms".to_string(), latency_ms as f64),
                        ("threshold_ms".to_string(), self.thresholds.max_latency_ms as f64),
                    ]),
                    estimated_duration_mins: 120,
                    priority: TrainingPriority::Low,
                })
            }
            
            _ => None,
        };
        
        Ok(training_command)
    }
    
    /// Store performance snapshot for trend analysis
    async fn store_performance_snapshot(&self, model_name: &str, accuracy: f64, confidence: f64, latency_ms: u64) {
        if let Ok(mut state) = self.decision_state.write().await {
            let snapshot = PerformanceSnapshot {
                timestamp: Utc::now(),
                accuracy,
                confidence,
                latency_ms,
                error_count: 0,
            };
            
            state.model_performance_history
                .entry(model_name.to_string())
                .or_insert_with(Vec::new)
                .push(snapshot);
                
            // Keep only recent snapshots (last 100)
            if let Some(history) = state.model_performance_history.get_mut(model_name) {
                if history.len() > 100 {
                    history.drain(0..history.len()-100);
                }
            }
        }
    }
    
    /// Update decision state after sending training command
    async fn update_decision_state(&self, event: &PerformanceEvent, command: &TrainingCommand) {
        if let Ok(mut state) = self.decision_state.write().await {
            state.last_training_decisions.insert(command.model_name.clone(), Utc::now());
            
            // Add performance alert
            let alert = PerformanceAlert {
                model_name: command.model_name.clone(),
                alert_type: command.trigger_reason.clone(),
                severity: match command.urgency {
                    TrainingUrgency::Critical => 1.0,
                    TrainingUrgency::High => 0.8,
                    TrainingUrgency::Medium => 0.6,
                    TrainingUrgency::Low => 0.4,
                },
                created_at: Utc::now(),
                acknowledged: false,
            };
            
            state.active_alerts.push(alert);
        }
    }
}

/// Market Timing → DAA Integration Coordinator
pub struct MarketTimingCoordinator {
    market_data_rx: broadcast::Receiver<TimeSeriesData>,
    daa_coordination_tx: mpsc::Sender<DaaCoordinationEvent>,
    coordination_state: Arc<RwLock<MarketCoordinationState>>,
}

impl MarketTimingCoordinator {
    pub fn new(
        market_data_rx: broadcast::Receiver<TimeSeriesData>,
        daa_coordination_tx: mpsc::Sender<DaaCoordinationEvent>,
    ) -> Self {
        Self {
            market_data_rx,
            daa_coordination_tx,
            coordination_state: Arc::new(RwLock::new(MarketCoordinationState::default())),
        }
    }
    
    /// Start coordination between market timing and DAA decisions
    pub async fn coordinate_market_decisions(&mut self) -> CoordinationResult<()> {
        info!("Starting Market-DAA coordination");
        
        while let Ok(market_data) = self.market_data_rx.recv().await {
            // Analyze market timing signals
            let timing_analysis = self.analyze_market_timing(&market_data).await?;
            
            // Generate DAA coordination events based on analysis
            if let Some(coordination_event) = self.generate_coordination_event(timing_analysis).await? {
                match self.daa_coordination_tx.send(coordination_event.clone()).await {
                    Ok(_) => {
                        debug!("DAA coordination event sent: {:?}", coordination_event);
                        self.update_coordination_state(&market_data, &coordination_event).await;
                    }
                    Err(e) => {
                        error!("Failed to send DAA coordination event: {}", e);
                        return Err(CoordinationError::Communication(e.to_string()));
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Analyze market data for timing signals
    async fn analyze_market_timing(&self, data: &TimeSeriesData) -> CoordinationResult<TimingAnalysis> {
        // Simplified market timing analysis
        let price_change = (data.close - data.open) / data.open;
        let volume_factor = (data.volume / 1000000.0).min(2.0); // Normalize volume
        
        // Calculate signal strength based on price change and volume
        let signal_strength = price_change * volume_factor;
        
        // Create market context
        let market_context = MarketContext {
            symbol: data.symbol.clone(),
            current_price: data.close,
            bid: data.close - 0.01,
            ask: data.close + 0.01,
            volume_24h: data.volume,
            volatility: (data.high - data.low) / data.close,
            timestamp: data.timestamp.timestamp(),
        };
        
        // Assess risk factors
        let mut risk_factors = Vec::new();
        if market_context.volatility > 0.05 {
            risk_factors.push("High volatility".to_string());
        }
        if data.volume < 100000.0 {
            risk_factors.push("Low volume".to_string());
        }
        
        // Determine urgency
        let recommended_urgency = match signal_strength.abs() {
            s if s > 0.1 => CoordinationUrgency::Immediate,
            s if s > 0.05 => CoordinationUrgency::High,
            s if s > 0.02 => CoordinationUrgency::Medium,
            _ => CoordinationUrgency::Low,
        };
        
        Ok(TimingAnalysis {
            signal_strength,
            market_context,
            risk_factors,
            confidence: 0.8, // Would be calculated based on historical accuracy
            recommended_urgency,
        })
    }
    
    /// Generate DAA coordination event from timing analysis
    async fn generate_coordination_event(&self, analysis: TimingAnalysis) -> CoordinationResult<Option<DaaCoordinationEvent>> {
        let event = match analysis.signal_strength {
            strength if strength > 0.05 => {
                Some(DaaCoordinationEvent::StrongBuySignal {
                    confidence: analysis.confidence,
                    market_context: analysis.market_context,
                    urgency: analysis.recommended_urgency,
                })
            }
            
            strength if strength < -0.05 => {
                Some(DaaCoordinationEvent::StrongSellSignal {
                    confidence: analysis.confidence,
                    market_context: analysis.market_context,
                    risk_factors: analysis.risk_factors,
                })
            }
            
            _ => {
                // No strong signal, check for volatility alerts
                if analysis.market_context.volatility > 0.1 {
                    Some(DaaCoordinationEvent::VolatilityAlert {
                        current_volatility: analysis.market_context.volatility,
                        threshold: 0.1,
                        recommended_action: "reduce_position_size".to_string(),
                    })
                } else {
                    None
                }
            }
        };
        
        Ok(event)
    }
    
    /// Update coordination state after processing market data
    async fn update_coordination_state(&self, data: &TimeSeriesData, event: &DaaCoordinationEvent) {
        if let Ok(mut state) = self.coordination_state.write().await {
            state.last_signal_time = Some(data.timestamp);
            
            // Record signal in history
            let signal_type = match event {
                DaaCoordinationEvent::StrongBuySignal { .. } => MarketSignalType::BuySignal,
                DaaCoordinationEvent::StrongSellSignal { .. } => MarketSignalType::SellSignal,
                DaaCoordinationEvent::VolatilityAlert { .. } => MarketSignalType::VolatilityAlert,
                DaaCoordinationEvent::MarketStateChange { .. } => MarketSignalType::TrendReversal,
            };
            
            let history_entry = MarketSignalHistory {
                timestamp: data.timestamp,
                signal_type,
                confidence: 0.8, // Would be extracted from event
                outcome: None, // Would be updated later with actual outcome
            };
            
            state.signal_history.push(history_entry);
            
            // Keep only recent history (last 1000 signals)
            if state.signal_history.len() > 1000 {
                state.signal_history.drain(0..state.signal_history.len()-1000);
            }
        }
    }
}

/// Training Coordination Service
pub struct TrainingCoordinator {
    training_command_rx: mpsc::Receiver<TrainingCommand>,
    training_notification_tx: mpsc::Sender<TrainingNotification>,
}

impl TrainingCoordinator {
    pub fn new(
        training_command_rx: mpsc::Receiver<TrainingCommand>,
        training_notification_tx: mpsc::Sender<TrainingNotification>,
    ) -> Self {
        Self {
            training_command_rx,
            training_notification_tx,
        }
    }
    
    /// Start training coordination service
    pub async fn coordinate_training(&mut self) -> CoordinationResult<()> {
        info!("Starting Training coordination");
        
        while let Some(training_command) = self.training_command_rx.recv().await {
            info!("Processing training command for model: {}", training_command.model_name);
            
            // Create training notification
            let notification = TrainingNotification {
                model_type: training_command.model_name.clone(),
                trigger_reason: match training_command.urgency {
                    TrainingUrgency::Critical => crate::neural::monitoring::TrainingTriggerReason::CriticalPerformanceDrop,
                    TrainingUrgency::High => crate::neural::monitoring::TrainingTriggerReason::AccuracyThresholdBreached,
                    TrainingUrgency::Medium => crate::neural::monitoring::TrainingTriggerReason::ScheduledMaintenance,
                    TrainingUrgency::Low => crate::neural::monitoring::TrainingTriggerReason::DataDriftDetected,
                },
                priority: training_command.priority,
                recommended_action: match training_command.recommended_action.as_deref() {
                    Some("retrain_model") => TrainingAction::FullRetrain,
                    Some("increase_training_data") => TrainingAction::IncrementalTrain,
                    Some("optimize_model") => TrainingAction::HyperparameterTune,
                    _ => TrainingAction::FullRetrain,
                },
                performance_context: training_command.performance_context,
                estimated_duration_mins: training_command.estimated_duration_mins,
                created_at: Utc::now(),
            };
            
            // Send training notification
            match self.training_notification_tx.send(notification).await {
                Ok(_) => {
                    info!("Training notification sent for model: {}", training_command.model_name);
                }
                Err(e) => {
                    error!("Failed to send training notification: {}", e);
                    return Err(CoordinationError::Communication(e.to_string()));
                }
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;
    use tokio::time::{timeout, Duration};
    
    #[tokio::test]
    async fn test_performance_coordinator_creation() {
        let (performance_tx, performance_rx) = broadcast::channel(100);
        let (training_tx, _training_rx) = mpsc::channel(100);
        let thresholds = TrainingThresholds::default();
        
        let coordinator = PerformanceCoordinator::new(performance_rx, training_tx, thresholds);
        
        // Test that coordinator can be created successfully
        assert!(coordinator.decision_state.read().await.model_performance_history.is_empty());
    }
    
    #[tokio::test]
    async fn test_market_timing_coordinator_creation() {
        let (market_tx, market_rx) = broadcast::channel(100);
        let (daa_tx, _daa_rx) = mpsc::channel(100);
        
        let coordinator = MarketTimingCoordinator::new(market_rx, daa_tx);
        
        // Test that coordinator can be created successfully
        assert!(coordinator.coordination_state.read().await.signal_history.is_empty());
    }
    
    #[tokio::test]
    async fn test_training_coordinator_creation() {
        let (training_tx, training_rx) = mpsc::channel(100);
        let (notification_tx, _notification_rx) = mpsc::channel(100);
        
        let coordinator = TrainingCoordinator::new(training_rx, notification_tx);
        
        // Test that coordinator structure is valid
        // (No direct state to check, but creation should succeed)
    }
}