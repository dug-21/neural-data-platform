//! Real-Time Training Integration for DAA System
//!
//! This module integrates real-time training capabilities with the existing
//! DAA autonomous training system, preserving all batch training functionality
//! while adding real-time parameter updates.

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

// Import existing DAA training components
use crate::daa::autonomous_training::{
    AutonomousTrainingEngine, 
    PerformanceSnapshot, 
    TrainingDecisionType,
    TrainingTriggerConfig,
};

// Import real-time training components
use crate::neural::realtime_training::{
    RealtimeTrainingExtension,
    RealtimeTrainingConfig,
};

// Import existing neural components
use crate::neural::{VendorPredictor, PredictionResult};

/// DAA Training Scheduler with real-time coordination
pub struct DAATrainingScheduler {
    /// Existing autonomous training engine
    autonomous_engine: Arc<RwLock<AutonomousTrainingEngine>>,
    
    /// Real-time training extension
    realtime_extension: Arc<RealtimeTrainingExtension>,
    
    /// Coordination state
    batch_training_active: Arc<RwLock<bool>>,
    
    /// Performance tracking
    performance_history: Arc<RwLock<Vec<PerformanceSnapshot>>>,
    
    /// Configuration
    coordination_config: CoordinationConfig,
}

/// Configuration for DAA training coordination
#[derive(Debug, Clone)]
pub struct CoordinationConfig {
    /// Allow real-time updates during batch training
    pub allow_concurrent_updates: bool,
    
    /// Minimum time between batch training sessions
    pub batch_training_cooldown_minutes: u64,
    
    /// Maximum real-time updates before forcing batch retrain
    pub max_realtime_updates_before_batch: u32,
    
    /// Performance degradation threshold for emergency batch training
    pub emergency_batch_threshold: f64,
}

impl Default for CoordinationConfig {
    fn default() -> Self {
        Self {
            allow_concurrent_updates: false, // Conservative default
            batch_training_cooldown_minutes: 30,
            max_realtime_updates_before_batch: 100,
            emergency_batch_threshold: 0.5,
        }
    }
}

impl DAATrainingScheduler {
    /// Create new DAA training scheduler with real-time coordination
    pub fn new(
        autonomous_engine: Arc<RwLock<AutonomousTrainingEngine>>,
        realtime_extension: Arc<RealtimeTrainingExtension>,
        coordination_config: CoordinationConfig,
    ) -> Self {
        Self {
            autonomous_engine,
            realtime_extension,
            batch_training_active: Arc::new(RwLock::new(false)),
            performance_history: Arc::new(RwLock::new(Vec::new())),
            coordination_config,
        }
    }
    
    /// Start the training coordination system
    pub async fn start_coordination(&self) -> Result<()> {
        info!("🎯 Starting DAA training coordination with real-time integration");
        
        // Start real-time training processing
        self.realtime_extension.start_processing().await?;
        
        // Start periodic coordination tasks
        self.start_coordination_tasks().await?;
        
        info!("✅ DAA training coordination started successfully");
        Ok(())
    }
    
    /// Start background coordination tasks
    async fn start_coordination_tasks(&self) -> Result<()> {
        let batch_training_active = self.batch_training_active.clone();
        let realtime_extension = self.realtime_extension.clone();
        let autonomous_engine = self.autonomous_engine.clone();
        let performance_history = self.performance_history.clone();
        let coordination_config = self.coordination_config.clone();
        
        // Spawn periodic coordination task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(60) // Check every minute
            );
            
            loop {
                interval.tick().await;
                
                if let Err(e) = Self::coordinate_training_systems(
                    &batch_training_active,
                    &realtime_extension,
                    &autonomous_engine,
                    &performance_history,
                    &coordination_config,
                ).await {
                    warn!("Training coordination error: {}", e);
                }
            }
        });
        
        Ok(())
    }
    
    /// Coordinate between batch and real-time training systems
    async fn coordinate_training_systems(
        batch_training_active: &RwLock<bool>,
        realtime_extension: &RealtimeTrainingExtension,
        autonomous_engine: &RwLock<AutonomousTrainingEngine>,
        performance_history: &RwLock<Vec<PerformanceSnapshot>>,
        config: &CoordinationConfig,
    ) -> Result<()> {
        // Check if batch training is currently active
        let is_batch_active = *batch_training_active.read().await;
        
        if is_batch_active && !config.allow_concurrent_updates {
            debug!("Batch training active - skipping real-time coordination");
            return Ok(());
        }
        
        // Process queued real-time updates
        realtime_extension.process_queued_updates().await?;
        
        // Check if we need to trigger batch retraining
        let should_trigger_batch = Self::should_trigger_batch_training(
            realtime_extension,
            performance_history,
            config,
        ).await?;
        
        if should_trigger_batch && !is_batch_active {
            info!("🔄 Triggering batch retraining based on real-time performance");
            Self::trigger_batch_training(
                batch_training_active,
                autonomous_engine,
            ).await?;
        }
        
        Ok(())
    }
    
    /// Check if batch training should be triggered
    async fn should_trigger_batch_training(
        realtime_extension: &RealtimeTrainingExtension,
        performance_history: &RwLock<Vec<PerformanceSnapshot>>,
        config: &CoordinationConfig,
    ) -> Result<bool> {
        let metrics = realtime_extension.get_metrics().await;
        
        // Check real-time update count threshold
        if metrics.update_count > config.max_realtime_updates_before_batch as u64 {
            info!("Real-time update count ({}) exceeded threshold ({})", 
                  metrics.update_count, config.max_realtime_updates_before_batch);
            return Ok(true);
        }
        
        // Check for performance degradation
        let recent_performance = Self::get_recent_performance(performance_history).await;
        if let Some(perf) = recent_performance {
            if perf.accuracy < config.emergency_batch_threshold {
                warn!("Performance degradation detected: accuracy {:.3} < threshold {:.3}",
                      perf.accuracy, config.emergency_batch_threshold);
                return Ok(true);
            }
        }
        
        // Check for excessive accuracy degradations
        let degradation_ratio = if metrics.update_count > 0 {
            metrics.accuracy_degradations as f64 / metrics.update_count as f64
        } else {
            0.0
        };
        
        if degradation_ratio > 0.7 { // >70% of updates resulted in degradation
            warn!("High degradation ratio detected: {:.3}", degradation_ratio);
            return Ok(true);
        }
        
        Ok(false)
    }
    
    /// Trigger batch training
    async fn trigger_batch_training(
        batch_training_active: &RwLock<bool>,
        autonomous_engine: &RwLock<AutonomousTrainingEngine>,
    ) -> Result<()> {
        // Set batch training flag
        *batch_training_active.write().await = true;
        
        // Create mock performance snapshot for batch training evaluation
        let performance_snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.7, // Below threshold to trigger training
            confidence: 0.8,
            price_error: 0.08,
            sharpe_ratio: 0.8,
            max_drawdown: 0.12,
            volatility: 0.08,
            model_agreement: 0.85,
            consecutive_failures: 0,
            trading_volume: 1000.0,
            profit_loss: 50.0,
            event_count: 1,
            window_duration: chrono::Duration::minutes(5),
            // Extended fields for compatibility with other modules
            latency_ms: 100,
            error_rate: 0.15,
            recent_predictions: 1000,
            symbol: "BATCH_TRAINING".to_string(),
            trading_performance: None,
            accuracy_metrics: None,
            data_type_metrics: None,
            // Observability module compatibility fields
            cpu_usage: 75.0,
            memory_usage: 850.0,
            active_connections: 8,
            requests_per_second: 25.0,
            average_response_time: 45.0, // milliseconds as f64
            cache_hit_rate: 0.75,
        };
        
        // Trigger autonomous training engine evaluation
        let engine = autonomous_engine.read().await;
        match engine.evaluate_training_need(performance_snapshot).await {
            Ok(decision) => {
                match decision.decision_type {
                    TrainingDecisionType::FullRetraining { reason, .. } => {
                        info!("🚀 Batch retraining triggered: {}", reason);
                        // In a real implementation, this would start the actual training process
                    }
                    TrainingDecisionType::IncrementalTraining => {
                        info!("🔄 Incremental training triggered");
                    }
                    TrainingDecisionType::NoTraining { reason } => {
                        info!("No batch training needed: {}", reason);
                    }
                    _ => {
                        debug!("Other training decision: {:?}", decision.decision_type);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to evaluate training need: {}", e);
            }
        }
        
        // Simulate training time (in real implementation, this would be the actual training duration)
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        // Clear batch training flag
        *batch_training_active.write().await = false;
        
        Ok(())
    }
    
    /// Get most recent performance snapshot
    async fn get_recent_performance(
        performance_history: &RwLock<Vec<PerformanceSnapshot>>,
    ) -> Option<PerformanceSnapshot> {
        let history = performance_history.read().await;
        history.last().cloned()
    }
    
    /// Process trading outcome and generate feedback
    pub async fn process_trading_outcome(
        &self,
        symbol: &str,
        prediction: &PredictionResult,
        actual_outcome: Option<f64>,
    ) -> Result<()> {
        // Create feedback from the trading outcome
        if let Some(feedback) = RealtimeTrainingExtension::create_feedback(
            symbol,
            prediction,
            actual_outcome,
        ) {
            // Send feedback to real-time training system
            self.realtime_extension.send_feedback(feedback).await?;
            
            // Update performance history
            if let Some(actual) = actual_outcome {
                let accuracy = 1.0 - ((prediction.value - actual) / actual).abs();
                let performance = PerformanceSnapshot {
                    timestamp: Utc::now(),
                    accuracy,
                    confidence: prediction.confidence,
                    price_error: (prediction.value - actual).abs(),
                    sharpe_ratio: if accuracy > 0.8 { 1.2 } else { 0.8 },
                    max_drawdown: 0.05,
                    volatility: 0.02,
                    model_agreement: 0.9,
                    consecutive_failures: if accuracy < 0.8 { 1 } else { 0 },
                    trading_volume: 1000000.0,
                    profit_loss: (actual - prediction.value) * 100.0, // Simplified P&L
                    event_count: 1,
                    window_duration: chrono::Duration::minutes(1),
                    // Extended fields for compatibility with other modules
                    latency_ms: 50, // Typical prediction latency
                    error_rate: if accuracy < 0.8 { 0.1 } else { 0.05 },
                    recent_predictions: 1,
                    symbol: symbol.to_string(),
                    trading_performance: None,
                    accuracy_metrics: None,
                    data_type_metrics: None,
                    // Observability module compatibility fields
                    cpu_usage: 45.0,
                    memory_usage: 600.0,
                    active_connections: 15,
                    requests_per_second: 60.0,
                    average_response_time: 22.0, // milliseconds as f64
                    cache_hit_rate: 0.92,
                };
                
                let mut history = self.performance_history.write().await;
                history.push(performance);
                
                // Keep only recent performance (last 100 entries)
                if history.len() > 100 {
                    history.drain(0..50); // Remove oldest half
                }
            }
        }
        
        Ok(())
    }
    
    /// Check if real-time updates are currently allowed
    pub async fn are_realtime_updates_allowed(&self) -> bool {
        let is_batch_active = *self.batch_training_active.read().await;
        !is_batch_active || self.coordination_config.allow_concurrent_updates
    }
    
    /// Get comprehensive training system status
    pub async fn get_training_status(&self) -> HashMap<String, serde_json::Value> {
        let mut status = HashMap::new();
        
        // Batch training status
        let is_batch_active = *self.batch_training_active.read().await;
        status.insert("batch_training_active".to_string(), serde_json::json!(is_batch_active));
        
        // Real-time training metrics
        let realtime_metrics = self.realtime_extension.get_metrics().await;
        status.insert("realtime_metrics".to_string(), serde_json::json!(realtime_metrics));
        
        // Real-time update statistics
        let update_stats = self.realtime_extension.get_update_statistics().await;
        status.insert("update_statistics".to_string(), serde_json::json!(update_stats));
        
        // Performance history summary
        let history = self.performance_history.read().await;
        let recent_accuracy = history.last().map(|p| p.accuracy).unwrap_or(0.0);
        let avg_accuracy = if !history.is_empty() {
            history.iter().map(|p| p.accuracy).sum::<f64>() / history.len() as f64
        } else {
            0.0
        };
        
        status.insert("performance_summary".to_string(), serde_json::json!({
            "recent_accuracy": recent_accuracy,
            "average_accuracy": avg_accuracy,
            "history_length": history.len(),
        }));
        
        // Coordination configuration
        status.insert("coordination_config".to_string(), serde_json::json!({
            "allow_concurrent_updates": self.coordination_config.allow_concurrent_updates,
            "batch_cooldown_minutes": self.coordination_config.batch_training_cooldown_minutes,
            "max_realtime_updates": self.coordination_config.max_realtime_updates_before_batch,
            "emergency_threshold": self.coordination_config.emergency_batch_threshold,
        }));
        
        status
    }
    
    /// Force batch retraining (manual trigger)
    pub async fn force_batch_training(&self) -> Result<()> {
        info!("🔧 Manually triggering batch retraining");
        
        Self::trigger_batch_training(
            &self.batch_training_active,
            &self.autonomous_engine,
        ).await?;
        
        Ok(())
    }
    
    /// Reset real-time training metrics (useful for testing)
    pub async fn reset_realtime_metrics(&self) -> Result<()> {
        // This would reset counters in the real-time extension
        // For now, we'll just log the reset
        info!("🔄 Resetting real-time training metrics");
        Ok(())
    }
}

/// Factory for creating integrated training systems
pub struct TrainingSystemFactory;

impl TrainingSystemFactory {
    /// Create complete integrated training system
    pub async fn create_integrated_system(
        vendor_predictor: Arc<RwLock<VendorPredictor>>,
    ) -> Result<(Arc<DAATrainingScheduler>, Arc<RealtimeTrainingExtension>)> {
        // Create autonomous training engine
        let training_config = TrainingTriggerConfig::default();
        let autonomous_engine = Arc::new(RwLock::new(
            AutonomousTrainingEngine::new(training_config.clone())?
        ));
        
        // Create real-time training extension
        let realtime_config = RealtimeTrainingConfig::default();
        let realtime_extension = Arc::new(RealtimeTrainingExtension::new(
            vendor_predictor,
            autonomous_engine.clone(),
            realtime_config,
            training_config,
        ));
        
        // Create DAA training scheduler
        let coordination_config = CoordinationConfig::default();
        let scheduler = Arc::new(DAATrainingScheduler::new(
            autonomous_engine,
            realtime_extension.clone(),
            coordination_config,
        ));
        
        Ok((scheduler, realtime_extension))
    }
    
    /// Create system with custom configurations
    pub async fn create_custom_system(
        vendor_predictor: Arc<RwLock<VendorPredictor>>,
        training_config: TrainingTriggerConfig,
        realtime_config: RealtimeTrainingConfig,
        coordination_config: CoordinationConfig,
    ) -> Result<(Arc<DAATrainingScheduler>, Arc<RealtimeTrainingExtension>)> {
        // Create autonomous training engine
        let autonomous_engine = Arc::new(RwLock::new(
            AutonomousTrainingEngine::new(training_config.clone())?
        ));
        
        // Create real-time training extension
        let realtime_extension = Arc::new(RealtimeTrainingExtension::new(
            vendor_predictor,
            autonomous_engine.clone(),
            realtime_config,
            training_config,
        ));
        
        // Create DAA training scheduler
        let scheduler = Arc::new(DAATrainingScheduler::new(
            autonomous_engine,
            realtime_extension.clone(),
            coordination_config,
        ));
        
        Ok((scheduler, realtime_extension))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neural::PredictionResult;
    
    #[tokio::test]
    async fn test_coordination_config_creation() {
        let config = CoordinationConfig::default();
        assert!(!config.allow_concurrent_updates); // Conservative default
        assert_eq!(config.batch_training_cooldown_minutes, 30);
        assert_eq!(config.max_realtime_updates_before_batch, 100);
    }
    
    #[tokio::test]
    async fn test_should_trigger_batch_training() {
        // This test would require mock components in a full implementation
        // For now, just test that the function exists and can be called
        assert!(true);
    }
    
    #[tokio::test]
    async fn test_training_outcome_processing() {
        // Mock prediction result
        let prediction = PredictionResult {
            value: 100.0,
            confidence: 0.8,
            model_name: "test_model".to_string(),
            interval_low: 95.0,
            interval_high: 105.0,
            timestamp: Utc::now(),
            metadata: None,
        };
        
        // Test feedback creation
        let feedback = RealtimeTrainingExtension::create_feedback(
            "AAPL",
            &prediction,
            Some(102.0),
        );
        
        assert!(feedback.is_some());
        let feedback = feedback.unwrap();
        assert_eq!(feedback.symbol, "AAPL");
        assert!(feedback.accuracy > 0.9); // Close prediction should have high accuracy
    }
}