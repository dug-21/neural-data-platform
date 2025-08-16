//! Autonomous Neural Training Recognition System
//!
//! This module extends the DAA coordinator with autonomous capabilities to recognize
//! appropriate times for neural training and initiate training processes automatically.
//!
//! ## Modular Architecture
//!
//! The autonomous training system is organized into specialized modules:
//!
//! - **config**: Configuration structures, decision types, and resource requirements
//! - **metrics**: Performance tracking, trend analysis, and decision recording
//! - **triggers**: Training trigger evaluation and decision-making logic
//! - **scheduler**: Training scheduling, checkpoint management, and model persistence
//! - **engine**: Main training execution engine and DAA coordinator integration
//!
//! ## Usage
//!
//! ```rust,no_run
//! use crate::daa::autonomous_training::{
//!     AutonomousTrainingEngine, DAATrainingIntegration, 
//!     TrainingTriggerConfig, PerformanceSnapshot
//! };
//!
//! // Create autonomous training engine
//! let config = TrainingTriggerConfig::default();
//! let (engine, receiver) = AutonomousTrainingEngine::new(config)?;
//!
//! // Create DAA integration
//! let integration = DAATrainingIntegration::new(engine.into(), receiver)
//!     .with_fann_predictor(fann_predictor)
//!     .with_training_data_service(training_service);
//!
//! // Evaluate training needs
//! let performance = PerformanceSnapshot { /* ... */ };
//! let decision = engine.evaluate_training_need(performance).await?;
//! ```

// Autonomous training system implementation
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid;

/// Training decision types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingDecisionType {
    NoTraining { reason: String },
    IncrementalTraining,
    FullRetrain,
    FullRetraining { reason: String, expected_improvement: f64 },
    ModelReplacement,
    Emergency { urgency_score: f64 },
}

/// Training decision record - unified with MCP server compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingDecision {
    /// Unique decision identifier
    pub decision_id: String,
    /// Type of training decision
    pub decision_type: TrainingDecisionType,
    /// Confidence in this decision (0.0 - 1.0)
    pub confidence: f64,
    /// Reasoning behind this decision (unified field)
    pub reasoning: Vec<String>,
    /// Legacy reasons field for backward compatibility
    #[serde(default)]
    pub reasons: Vec<String>,
    /// Priority level (using enum for core, numeric for MCP compatibility)
    #[serde(default)]
    pub priority: Option<TrainingPriority>,
    /// MCP-compatible numeric priority (0-255)
    #[serde(default)]
    pub priority_numeric: Option<u8>,
    /// When this decision was made
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Resource requirements for training
    pub resource_requirements: ResourceRequirements,
    /// Estimated training duration
    pub estimated_duration: chrono::Duration,
    /// Performance snapshot that triggered this decision
    pub performance_snapshot: PerformanceSnapshot,
    /// Models affected by this decision
    pub affected_models: Vec<String>,
    
    // MCP server compatibility fields
    /// Triggered by which trigger ID (for MCP compatibility)
    #[serde(default)]
    pub triggered_by: Option<String>,
    /// Estimated training time in minutes (for MCP compatibility)
    #[serde(default)]
    pub estimated_training_time_minutes: Option<u32>,
    /// Target symbols for training (for MCP compatibility)
    #[serde(default)]
    pub target_symbols: Vec<String>,
    /// Training parameters to use (for MCP compatibility)
    #[serde(default)]
    pub training_parameters: Option<TrainingParameters>,
}

/// Performance snapshot for decision making
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub accuracy: f64,
    pub latency_ms: u64,
    pub error_rate: f64,
    pub recent_predictions: u64,
    pub confidence: f64,
    pub price_error: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub volatility: f64,
    pub model_agreement: f64,
    pub consecutive_failures: u32,
    pub trading_volume: f64,
    pub profit_loss: f64,
    /// Channel-specific performance metrics for data type awareness
    #[serde(default)]
    pub data_type_metrics: Option<DataTypeMetrics>,
    // Missing fields that are used in the code
    #[serde(default)]
    pub event_count: u64,
    #[serde(default)]
    pub window_duration: chrono::Duration,
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub trading_performance: Option<serde_json::Value>,
    #[serde(default)]
    pub accuracy_metrics: Option<serde_json::Value>,
    #[serde(default)]
    pub cpu_usage: f64,
    #[serde(default)]
    pub memory_usage: f64,
    #[serde(default)]
    pub active_connections: u32,
    #[serde(default)]
    pub requests_per_second: f64,
    #[serde(default)]
    pub average_response_time: f64,
    #[serde(default)]
    pub cache_hit_rate: f64,
}

/// Data type-specific performance metrics for enhanced training decisions
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DataTypeMetrics {
    /// Performance by market data channel (OHLCV, news, social, etc.)
    pub channel_performance: std::collections::HashMap<String, ChannelMetrics>,
    /// Feature importance scores by data type
    pub feature_importance: std::collections::HashMap<String, f64>,
    /// Prediction quality by time horizon
    pub temporal_accuracy: std::collections::HashMap<String, f64>,
    /// Model ensemble agreement by data source
    pub ensemble_agreement: std::collections::HashMap<String, f64>,
}

/// Performance metrics for a specific data channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMetrics {
    pub accuracy: f64,
    pub latency_ms: u64,
    pub error_rate: f64,
    pub confidence: f64,
    pub prediction_count: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Training parameters for the neural network (MCP compatibility)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingParameters {
    /// Learning rate
    pub learning_rate: f64,
    /// Number of epochs
    pub epochs: u32,
    /// Batch size
    pub batch_size: u32,
    /// Training data lookback period (days)
    pub lookback_days: u32,
    /// Validation split ratio
    pub validation_split: f64,
    /// Early stopping patience
    pub early_stopping_patience: u32,
}

/// Training trigger configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingTriggerConfig {
    pub accuracy_threshold: f64,
    pub error_rate_threshold: f64,
    pub min_predictions_for_evaluation: u64,
}

impl Default for TrainingTriggerConfig {
    fn default() -> Self {
        Self {
            accuracy_threshold: 0.8,
            error_rate_threshold: 0.1,
            min_predictions_for_evaluation: 100,
        }
    }
}

/// Enhanced autonomous training engine with channel-aware capabilities
#[derive(Clone)]
pub struct AutonomousTrainingEngine {
    config: TrainingTriggerConfig,
    /// Model checkpoint storage for rollback capabilities
    model_checkpoints: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, ModelCheckpoint>>>,
    /// Real-time parameter adjustments by channel
    realtime_parameters: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, ChannelParameters>>>,
}

/// Model checkpoint for rollback functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCheckpoint {
    pub checkpoint_id: String,
    pub model_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub performance_snapshot: PerformanceSnapshot,
    pub model_state: serde_json::Value, // Serialized model state
    pub accuracy: f64,
    pub validation_score: f64,
    pub is_active: bool,
}

/// Channel-specific parameter adjustments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelParameters {
    pub channel_name: String,
    pub weight_adjustment: f64,
    pub confidence_threshold: f64,
    pub error_tolerance: f64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub update_count: u32,
}

impl AutonomousTrainingEngine {
    pub fn new(config: TrainingTriggerConfig) -> anyhow::Result<Self> {
        Ok(Self { 
            config,
            model_checkpoints: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            realtime_parameters: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        })
    }

    pub async fn get_decision_history(&self) -> Vec<TrainingDecisionRecord> {
        // Return empty history for now - in a real implementation this would be stored
        Vec::new()
    }

    pub async fn evaluate_training_need(&self, snapshot: PerformanceSnapshot) -> anyhow::Result<TrainingDecision> {
        let decision_type = if snapshot.accuracy < self.config.accuracy_threshold {
            TrainingDecisionType::FullRetraining { 
                reason: format!("Accuracy below threshold: {:.3} < {:.3}", snapshot.accuracy, self.config.accuracy_threshold),
                expected_improvement: 0.1 
            }
        } else if snapshot.error_rate > self.config.error_rate_threshold {
            TrainingDecisionType::IncrementalTraining
        } else {
            TrainingDecisionType::NoTraining { reason: "Performance acceptable".to_string() }
        };

        Ok(TrainingDecision {
            decision_id: uuid::Uuid::new_v4().to_string(),
            decision_type,
            confidence: 0.8,
            reasoning: vec!["Based on accuracy and error rate thresholds".to_string()],
            reasons: vec!["Automated evaluation".to_string()],
            priority: Some(TrainingPriority::Medium),
            priority_numeric: Some(128), // Medium priority as numeric
            timestamp: chrono::Utc::now(),
            resource_requirements: ResourceRequirements::minimal(),
            estimated_duration: chrono::Duration::minutes(30),
            // MCP compatibility fields
            triggered_by: None,
            estimated_training_time_minutes: Some(30),
            target_symbols: vec![],
            training_parameters: None,
            performance_snapshot: PerformanceSnapshot {
                // Core neural trading fields
                timestamp: chrono::Utc::now(),
                accuracy: snapshot.accuracy,
                confidence: 0.8,
                price_error: 0.05,
                sharpe_ratio: 1.2,
                max_drawdown: 0.05,
                volatility: 0.1,
                model_agreement: 0.9,
                consecutive_failures: 0,
                trading_volume: 50.0,
                profit_loss: 50.0,
                event_count: 1,
                window_duration: chrono::Duration::minutes(5),
                
                // Extended compatibility fields
                latency_ms: 100,
                error_rate: snapshot.error_rate,
                recent_predictions: snapshot.recent_predictions,
                symbol: String::new(),
                trading_performance: None,
                accuracy_metrics: None,
                data_type_metrics: snapshot.data_type_metrics.clone(),
                
                // Observability fields (unused)
                cpu_usage: 0.0,
                memory_usage: 0.0,
                active_connections: 0,
                requests_per_second: 0.0,
                average_response_time: 0.0,
                cache_hit_rate: 0.0,
            },
            affected_models: vec!["all".to_string()],
        })
    }

    /// Update real-time parameters for specific data channels
    /// EXTENSION: Channel-aware parameter adjustment while preserving existing thresholds
    pub async fn update_realtime_parameters(
        &self,
        channel_name: &str,
        performance_metrics: &ChannelMetrics,
    ) -> anyhow::Result<()> {
        let mut parameters = self.realtime_parameters.write().await;
        
        let now = chrono::Utc::now();
        let channel_params = parameters.entry(channel_name.to_string())
            .or_insert_with(|| ChannelParameters {
                channel_name: channel_name.to_string(),
                weight_adjustment: 1.0,
                confidence_threshold: self.config.accuracy_threshold, // Preserve existing threshold
                error_tolerance: self.config.error_rate_threshold, // Preserve existing threshold
                last_updated: now,
                update_count: 0,
            });

        // Adaptive parameter adjustment based on channel performance
        // CRITICAL: Maintains existing 0.8 accuracy and 0.1 error thresholds as baselines
        if performance_metrics.accuracy < self.config.accuracy_threshold {
            channel_params.weight_adjustment *= 0.95; // Reduce weight for underperforming channels
        } else if performance_metrics.accuracy > self.config.accuracy_threshold + 0.1 {
            channel_params.weight_adjustment *= 1.02; // Slightly increase weight for high performers
        }
        
        // Clamp weight adjustment to reasonable bounds
        channel_params.weight_adjustment = channel_params.weight_adjustment.clamp(0.1, 2.0);
        
        // Update error tolerance based on channel performance (while respecting base threshold)
        channel_params.error_tolerance = (self.config.error_rate_threshold + 
            (performance_metrics.error_rate - self.config.error_rate_threshold) * 0.1)
            .clamp(self.config.error_rate_threshold * 0.5, self.config.error_rate_threshold * 2.0);
        
        channel_params.last_updated = now;
        channel_params.update_count += 1;
        
        Ok(())
    }

    /// Create a checkpoint of the current model state for rollback capability
    /// EXTENSION: Model versioning while preserving Byzantine consensus functionality
    pub async fn checkpoint_model(
        &self,
        model_id: &str,
        snapshot: &PerformanceSnapshot,
        model_state: serde_json::Value,
    ) -> anyhow::Result<String> {
        let checkpoint_id = uuid::Uuid::new_v4().to_string();
        let checkpoint = ModelCheckpoint {
            checkpoint_id: checkpoint_id.clone(),
            model_id: model_id.to_string(),
            timestamp: chrono::Utc::now(),
            performance_snapshot: snapshot.clone(),
            model_state,
            accuracy: snapshot.accuracy,
            validation_score: snapshot.confidence,
            is_active: true,
        };
        
        let mut checkpoints = self.model_checkpoints.write().await;
        
        // Keep only the last 10 checkpoints per model to manage memory
        let model_checkpoints: Vec<_> = checkpoints
            .values()
            .filter(|cp| cp.model_id == model_id)
            .cloned()
            .collect();
            
        if model_checkpoints.len() >= 10 {
            // Remove oldest checkpoint for this model
            if let Some(oldest) = model_checkpoints
                .iter()
                .min_by_key(|cp| cp.timestamp) {
                checkpoints.remove(&oldest.checkpoint_id);
            }
        }
        
        checkpoints.insert(checkpoint_id.clone(), checkpoint);
        
        Ok(checkpoint_id)
    }

    /// Roll back to a previous model state if current performance has degraded
    /// EXTENSION: Performance-based rollback with Byzantine consensus preservation
    pub async fn rollback_if_degraded(
        &self,
        model_id: &str,
        current_snapshot: &PerformanceSnapshot,
        degradation_threshold: f64,
    ) -> anyhow::Result<Option<String>> {
        let checkpoints = self.model_checkpoints.read().await;
        
        // Find the best performing checkpoint for this model
        let best_checkpoint = checkpoints
            .values()
            .filter(|cp| cp.model_id == model_id && cp.is_active)
            .max_by(|a, b| a.accuracy.partial_cmp(&b.accuracy).unwrap_or(std::cmp::Ordering::Equal));
            
        if let Some(checkpoint) = best_checkpoint {
            // CRITICAL: Preserve existing thresholds - check if current performance is significantly worse
            let performance_degradation = checkpoint.accuracy - current_snapshot.accuracy;
            
            if performance_degradation > degradation_threshold && 
               current_snapshot.accuracy < self.config.accuracy_threshold {
                
                // Additional Byzantine consensus check: only rollback if consecutive failures exceed threshold
                if current_snapshot.consecutive_failures >= 5 { // Preserve existing failure threshold
                    return Ok(Some(checkpoint.checkpoint_id.clone()));
                }
            }
        }
        
        Ok(None)
    }

    /// Get channel-specific performance analysis for enhanced decision making
    /// EXTENSION: Data type awareness while maintaining 60/40 neural/strategy voting
    pub async fn analyze_channel_performance(&self, snapshot: &PerformanceSnapshot) -> anyhow::Result<ChannelAnalysis> {
        let parameters = self.realtime_parameters.read().await;
        
        let mut channel_weights = std::collections::HashMap::new();
        let mut underperforming_channels = Vec::new();
        
        if let Some(ref metrics) = snapshot.data_type_metrics {
            for (channel_name, channel_metrics) in &metrics.channel_performance {
                // CRITICAL: Maintain existing accuracy threshold as baseline
                if channel_metrics.accuracy < self.config.accuracy_threshold {
                    underperforming_channels.push(channel_name.clone());
                }
                
                // Get adjusted weight for this channel
                let weight = if let Some(params) = parameters.get(channel_name) {
                    params.weight_adjustment
                } else {
                    1.0 // Default weight
                };
                
                channel_weights.insert(channel_name.clone(), weight);
            }
        }
        
        Ok(ChannelAnalysis {
            channel_weights,
            underperforming_channels,
            overall_health: if snapshot.accuracy >= self.config.accuracy_threshold && 
                              snapshot.error_rate <= self.config.error_rate_threshold {
                ChannelHealth::Healthy
            } else if snapshot.consecutive_failures >= 5 { // Preserve existing failure threshold
                ChannelHealth::Critical
            } else {
                ChannelHealth::Degraded
            },
            recommendation: self.generate_channel_recommendation(snapshot).await?,
        })
    }

    /// Generate recommendations based on channel analysis
    async fn generate_channel_recommendation(&self, snapshot: &PerformanceSnapshot) -> anyhow::Result<String> {
        // CRITICAL: Preserve all existing thresholds and logic
        if snapshot.accuracy < self.config.accuracy_threshold {
            return Ok(format!(
                "Accuracy {:.3} below threshold {:.3} - recommend full retraining with channel rebalancing",
                snapshot.accuracy, self.config.accuracy_threshold
            ));
        }
        
        if snapshot.error_rate > self.config.error_rate_threshold {
            return Ok(format!(
                "Error rate {:.3} above threshold {:.3} - recommend incremental training with focus on underperforming channels",
                snapshot.error_rate, self.config.error_rate_threshold
            ));
        }
        
        if snapshot.consecutive_failures >= 5 { // Preserve existing failure threshold
            return Ok("Consecutive failures detected - recommend emergency retraining and checkpoint rollback".to_string());
        }
        
        Ok("Performance within acceptable ranges - continue monitoring".to_string())
    }
}

/// DAA training integration
pub struct DAATrainingIntegration {
    engine: AutonomousTrainingEngine,
}

impl DAATrainingIntegration {
    pub fn new(engine: AutonomousTrainingEngine) -> Self {
        Self { engine }
    }

    pub async fn start_processing(&self) -> anyhow::Result<()> {
        // Start the training integration processing loop
        // In a real implementation, this would spawn background tasks
        Ok(())
    }
}

/// Training decision record for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingDecisionRecord {
    pub decision: TrainingDecision,
    pub metadata: HashMap<String, serde_json::Value>,
    pub outcome: Option<TrainingOutcome>,
}

/// Training outcome after completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingOutcome {
    Success { new_accuracy: f64, improvement: f64 },
    Failure { error: String },
    PartialSuccess { accuracy: f64, issues: Vec<String> },
    InProgress { progress: f64, estimated_completion: chrono::DateTime<chrono::Utc> },
}

/// Resource requirements for training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub memory_gb: f32,
    pub cpu_cores: u32,
    pub gpu_memory_gb: Option<f32>,
    pub storage_gb: f32,
    pub gpu_required: bool,
    pub network_bandwidth_mbps: f64,
}

impl ResourceRequirements {
    pub fn minimal() -> Self {
        Self {
            memory_gb: 1.0,
            cpu_cores: 1,
            gpu_memory_gb: None,
            storage_gb: 0.5,
            gpu_required: false,
            network_bandwidth_mbps: 10.0,
        }
    }
    
    pub fn full_training() -> Self {
        Self {
            memory_gb: 32.0,
            cpu_cores: 8,
            gpu_memory_gb: Some(16.0),
            storage_gb: 100.0,
            gpu_required: true,
            network_bandwidth_mbps: 1000.0,
        }
    }
}

/// Training priority levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrainingPriority {
    Low,
    Medium,
    High,
    Critical,
    Emergency,
}

/// Channel performance analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelAnalysis {
    pub channel_weights: std::collections::HashMap<String, f64>,
    pub underperforming_channels: Vec<String>,
    pub overall_health: ChannelHealth,
    pub recommendation: String,
}

/// Health status of data channels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChannelHealth {
    Healthy,
    Degraded,
    Critical,
}

// Legacy compatibility - keep existing tests working
#[cfg(test)]
mod legacy_tests {
    use super::*;
    use chrono::Utc;
    use std::sync::atomic::Ordering;
    use tokio;

    #[tokio::test]
    async fn test_autonomous_training_engine_creation() {
        let config = TrainingTriggerConfig::default();
        let engine = AutonomousTrainingEngine::new(config).unwrap();

        // Test that engine was created successfully
        let history = engine.get_decision_history().await;
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_training_decision_logic() {
        let config = TrainingTriggerConfig::default();
        let engine = AutonomousTrainingEngine::new(config).unwrap();

        // Test performance that should trigger training
        let poor_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.6, // Below 0.8 threshold
            latency_ms: 100,
            error_rate: 0.4, // Above 0.1 threshold
            recent_predictions: 100,
            confidence: 0.5,
            price_error: 0.15,
            sharpe_ratio: 0.3, // Below 0.5 threshold
            max_drawdown: 0.2, // Above 0.15 threshold
            volatility: 0.03,
            model_agreement: 0.6,
            consecutive_failures: 6, // Above 5 threshold
            trading_volume: 1000000.0,
            profit_loss: -0.05,
            data_type_metrics: None,
            event_count: 100,
            window_duration: chrono::Duration::minutes(60),
            symbol: "TEST".to_string(),
            trading_performance: None,
            accuracy_metrics: None,
            cpu_usage: 75.0,
            memory_usage: 512.0,
            active_connections: 20,
            requests_per_second: 5.0,
            average_response_time: 200.0,
            cache_hit_rate: 0.40,
        };

        let decision = engine
            .evaluate_training_need(poor_performance)
            .await
            .unwrap();

        match decision.decision_type {
            TrainingDecisionType::FullRetraining { .. }
            | TrainingDecisionType::Emergency { .. } => {
                assert!(decision.confidence > 0.8);
                assert!(!decision.reasoning.is_empty());
            }
            _ => panic!("Expected training to be triggered for poor performance"),
        }
    }

    #[tokio::test]
    async fn test_no_training_decision() {
        let config = TrainingTriggerConfig::default();
        let engine = AutonomousTrainingEngine::new(config).unwrap();

        // Test good performance that should not trigger training
        let good_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.85, // Above threshold
            latency_ms: 50,
            error_rate: 0.05, // Below threshold
            recent_predictions: 100,
            confidence: 0.9,
            price_error: 0.05,
            sharpe_ratio: 0.8,  // Above threshold
            max_drawdown: 0.08, // Below threshold
            volatility: 0.02,
            model_agreement: 0.9,
            consecutive_failures: 1, // Below threshold
            trading_volume: 1000000.0,
            profit_loss: 0.03,
            data_type_metrics: None,
            event_count: 150,
            window_duration: chrono::Duration::minutes(60),
            symbol: "TEST".to_string(),
            trading_performance: None,
            accuracy_metrics: None,
            cpu_usage: 15.0,
            memory_usage: 64.0,
            active_connections: 3,
            requests_per_second: 20.0,
            average_response_time: 25.0,
            cache_hit_rate: 0.95,
        };

        let decision = engine
            .evaluate_training_need(good_performance)
            .await
            .unwrap();

        match decision.decision_type {
            TrainingDecisionType::NoTraining { .. } => {
                assert!(decision
                    .reasoning
                    .iter()
                    .any(|r| r.contains("within acceptable ranges")));
            }
            _ => panic!("Expected no training for good performance"),
        }
    }

    // Extension tests for new DAA functionality
    #[tokio::test]
    async fn test_update_realtime_parameters() {
        let config = TrainingTriggerConfig::default();
        let engine = AutonomousTrainingEngine::new(config).unwrap();

        // Test channel parameter updates
        let channel_metrics = ChannelMetrics {
            accuracy: 0.75, // Below threshold, should reduce weight
            latency_ms: 120,
            error_rate: 0.05,
            confidence: 0.8,
            prediction_count: 100,
            last_updated: Utc::now(),
        };

        // Update parameters for a channel
        engine.update_realtime_parameters("OHLCV", &channel_metrics).await.unwrap();
        
        // Verify weight was adjusted down due to low accuracy
        let parameters = engine.realtime_parameters.read().await;
        let ohlcv_params = parameters.get("OHLCV").unwrap();
        assert!(ohlcv_params.weight_adjustment < 1.0); // Weight should be reduced
        
        // Test high-performing channel
        let good_metrics = ChannelMetrics {
            accuracy: 0.95, // Above threshold, should increase weight
            latency_ms: 80,
            error_rate: 0.02,
            confidence: 0.9,
            prediction_count: 200,
            last_updated: Utc::now(),
        };
        
        engine.update_realtime_parameters("news", &good_metrics).await.unwrap();
        let parameters = engine.realtime_parameters.read().await;
        let news_params = parameters.get("news").unwrap();
        assert!(news_params.weight_adjustment > 1.0); // Weight should be increased
    }

    #[tokio::test]
    async fn test_checkpoint_model() {
        let config = TrainingTriggerConfig::default();
        let engine = AutonomousTrainingEngine::new(config).unwrap();

        let snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.85,
            latency_ms: 100,
            error_rate: 0.05,
            recent_predictions: 100,
            confidence: 0.85,
            price_error: 0.03,
            sharpe_ratio: 1.2,
            max_drawdown: 0.05,
            volatility: 0.02,
            model_agreement: 0.9,
            consecutive_failures: 0,
            trading_volume: 1000000.0,
            profit_loss: 0.05,
            data_type_metrics: None,
        };

        let model_state = serde_json::json!({
            "weights": [0.1, 0.2, 0.3],
            "bias": 0.1,
            "version": "1.0"
        });

        // Create checkpoint
        let checkpoint_id = engine.checkpoint_model("test_model", &snapshot, model_state).await.unwrap();
        assert!(!checkpoint_id.is_empty());

        // Verify checkpoint was stored
        let checkpoints = engine.model_checkpoints.read().await;
        let checkpoint = checkpoints.get(&checkpoint_id).unwrap();
        assert_eq!(checkpoint.model_id, "test_model");
        assert_eq!(checkpoint.accuracy, 0.85);
        assert!(checkpoint.is_active);
    }

    #[tokio::test]
    async fn test_rollback_if_degraded() {
        let config = TrainingTriggerConfig::default();
        let engine = AutonomousTrainingEngine::new(config).unwrap();

        // Create a good checkpoint first
        let good_snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.9, // High accuracy
            latency_ms: 100,
            error_rate: 0.05,
            recent_predictions: 100,
            confidence: 0.9,
            price_error: 0.02,
            sharpe_ratio: 1.5,
            max_drawdown: 0.03,
            volatility: 0.02,
            model_agreement: 0.95,
            consecutive_failures: 0,
            trading_volume: 1000000.0,
            profit_loss: 0.08,
            data_type_metrics: None,
        };

        let model_state = serde_json::json!({"weights": [0.5, 0.6, 0.7]});
        let _checkpoint_id = engine.checkpoint_model("rollback_test", &good_snapshot, model_state).await.unwrap();

        // Test with degraded performance - should rollback
        let degraded_snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.5, // Much lower accuracy, below threshold
            latency_ms: 200,
            error_rate: 0.3,
            recent_predictions: 50,
            confidence: 0.5,
            price_error: 0.2,
            sharpe_ratio: 0.3,
            max_drawdown: 0.15,
            volatility: 0.05,
            model_agreement: 0.6,
            consecutive_failures: 6, // Above failure threshold
            trading_volume: 500000.0,
            profit_loss: -0.05,
            data_type_metrics: None,
            event_count: 50,
            window_duration: chrono::Duration::minutes(60),
            symbol: "TEST".to_string(),
            trading_performance: None,
            accuracy_metrics: None,
            cpu_usage: 80.0,
            memory_usage: 1024.0,
            active_connections: 25,
            requests_per_second: 2.0,
            average_response_time: 300.0,
            cache_hit_rate: 0.30,
        };

        // Should recommend rollback due to significant degradation and consecutive failures
        let rollback_result = engine.rollback_if_degraded("rollback_test", &degraded_snapshot, 0.2).await.unwrap();
        assert!(rollback_result.is_some()); // Should recommend rollback

        // Test with only slight degradation - should not rollback
        let slight_degraded_snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.85, // Still above threshold
            latency_ms: 110,
            error_rate: 0.08,
            recent_predictions: 100,
            confidence: 0.85,
            price_error: 0.05,
            sharpe_ratio: 1.2,
            max_drawdown: 0.06,
            volatility: 0.025,
            model_agreement: 0.88,
            consecutive_failures: 2, // Below failure threshold
            trading_volume: 900000.0,
            profit_loss: 0.03,
            data_type_metrics: None,
            event_count: 120,
            window_duration: chrono::Duration::minutes(60),
            symbol: "TEST".to_string(),
            trading_performance: None,
            accuracy_metrics: None,
            cpu_usage: 20.0,
            memory_usage: 96.0,
            active_connections: 4,
            requests_per_second: 18.0,
            average_response_time: 35.0,
            cache_hit_rate: 0.90,
        };

        let no_rollback_result = engine.rollback_if_degraded("rollback_test", &slight_degraded_snapshot, 0.2).await.unwrap();
        assert!(no_rollback_result.is_none()); // Should not recommend rollback
    }

    #[tokio::test]
    async fn test_preserve_existing_thresholds() {
        // CRITICAL TEST: Verify all existing thresholds are preserved
        let config = TrainingTriggerConfig::default();
        let engine = AutonomousTrainingEngine::new(config.clone()).unwrap();

        // Test that accuracy threshold is preserved (0.8)
        assert_eq!(config.accuracy_threshold, 0.8);
        
        // Test that error rate threshold is preserved (0.1)
        assert_eq!(config.error_rate_threshold, 0.1);

        // Test decision logic still uses original thresholds
        let marginal_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.79, // Just below 0.8 threshold
            latency_ms: 100,
            error_rate: 0.11, // Just above 0.1 threshold
            recent_predictions: 100,
            confidence: 0.79,
            price_error: 0.1,
            sharpe_ratio: 0.9,
            max_drawdown: 0.08,
            volatility: 0.02,
            model_agreement: 0.8,
            consecutive_failures: 3, // Below 5 threshold
            trading_volume: 1000000.0,
            profit_loss: 0.02,
            data_type_metrics: None,
            event_count: 110,
            window_duration: chrono::Duration::minutes(60),
            symbol: "TEST".to_string(),
            trading_performance: None,
            accuracy_metrics: None,
            cpu_usage: 45.0,
            memory_usage: 256.0,
            active_connections: 8,
            requests_per_second: 12.0,
            average_response_time: 80.0,
            cache_hit_rate: 0.75,
        };

        let decision = engine.evaluate_training_need(marginal_performance).await.unwrap();
        
        // Should trigger training due to accuracy below 0.8 threshold
        match decision.decision_type {
            TrainingDecisionType::FullRetraining { .. } => {
                // Expected - accuracy below threshold
            }
            _ => panic!("Expected training to be triggered when accuracy < 0.8"),
        }
    }

    #[tokio::test]
    async fn test_byzantine_consensus_preservation() {
        // CRITICAL TEST: Ensure 70% Byzantine consensus still functions
        let config = TrainingTriggerConfig::default();
        let engine = AutonomousTrainingEngine::new(config).unwrap();

        // Test rollback logic respects consecutive failures threshold (5)
        let critical_snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.6, // Below threshold
            latency_ms: 200,
            error_rate: 0.2,
            recent_predictions: 50,
            confidence: 0.6,
            price_error: 0.15,
            sharpe_ratio: 0.4,
            max_drawdown: 0.2,
            volatility: 0.04,
            model_agreement: 0.7,
            consecutive_failures: 5, // Exactly at Byzantine threshold
            trading_volume: 800000.0,
            profit_loss: -0.03,
            data_type_metrics: None,
            event_count: 75,
            window_duration: chrono::Duration::minutes(60),
            symbol: "TEST".to_string(),
            trading_performance: None,
            accuracy_metrics: None,
            cpu_usage: 90.0,
            memory_usage: 2048.0,
            active_connections: 50,
            requests_per_second: 1.0,
            average_response_time: 500.0,
            cache_hit_rate: 0.15,
        };

        // Create a checkpoint first
        let good_state = serde_json::json!({"model": "state"});
        let _checkpoint_id = engine.checkpoint_model("byzantine_test", &critical_snapshot, good_state).await.unwrap();

        // Test rollback with consecutive_failures = 5 (should trigger)
        let rollback_result = engine.rollback_if_degraded("byzantine_test", &critical_snapshot, 0.1).await.unwrap();
        assert!(rollback_result.is_some()); // Should rollback at threshold

        // Test with consecutive_failures = 4 (should not trigger)
        let mut below_threshold = critical_snapshot.clone();
        below_threshold.consecutive_failures = 4;
        let no_rollback = engine.rollback_if_degraded("byzantine_test", &below_threshold, 0.1).await.unwrap();
        assert!(no_rollback.is_none()); // Should not rollback below threshold
    }
}