//! Training Notification System
//!
//! Monitors performance events and triggers training notifications when thresholds are exceeded.
//! Integrates with the performance channel to provide real-time feedback for autonomous training.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, instrument, warn};

use super::super::performance_channel::{PerformanceEvent, PerformanceEventType, EventPriority};

/// Training notification payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingNotification {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub trigger_reason: TrainingTriggerReason,
    pub model_type: String,
    pub priority: TrainingPriority,
    pub recommended_action: TrainingAction,
    pub performance_context: PerformanceContext,
    pub metadata: HashMap<String, String>,
}

/// Reasons that can trigger training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingTriggerReason {
    LowAccuracy {
        current_accuracy: f64,
        threshold: f64,
        window_size: usize,
    },
    LowConfidence {
        current_confidence: f64,
        threshold: f64,
        consecutive_occurrences: u32,
    },
    HighErrorRate {
        current_error_rate: f64,
        threshold: f64,
        time_window: Duration,
    },
    ModelDivergence {
        divergence_score: f64,
        threshold: f64,
        models_disagreeing: Vec<String>,
    },
    PerformanceDegradation {
        metric_name: String,
        degradation_percent: f64,
        threshold_percent: f64,
    },
    ConsecutiveFailures {
        failure_count: u32,
        threshold: u32,
        failure_window: Duration,
    },
    TradingLosses {
        cumulative_loss: f64,
        loss_threshold: f64,
        loss_window: Duration,
    },
    DataDrift {
        drift_score: f64,
        threshold: f64,
        affected_features: Vec<String>,
    },
    ScheduledRetraining {
        last_training: DateTime<Utc>,
        training_interval: Duration,
    },
    ManualTrigger {
        triggered_by: String,
        reason: String,
    },
}

/// Training priority levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrainingPriority {
    Critical = 0,
    High = 1,
    Medium = 2,
    Low = 3,
}

/// Recommended training actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingAction {
    FullRetrain {
        data_points: usize,
        estimated_duration: Duration,
        resource_requirements: ResourceRequirements,
    },
    IncrementalUpdate {
        new_samples: usize,
        update_strategy: String,
        estimated_duration: Duration,
    },
    HyperparameterTuning {
        parameters_to_tune: Vec<String>,
        search_space: HashMap<String, (f64, f64)>,
        max_trials: u32,
    },
    EnsembleRebalancing {
        models_to_retrain: Vec<String>,
        new_weights: HashMap<String, f64>,
    },
    DataAugmentation {
        augmentation_strategies: Vec<String>,
        target_samples: usize,
    },
    ModelArchitectureChange {
        current_architecture: String,
        recommended_architecture: String,
        migration_required: bool,
    },
}

/// Resource requirements for training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub cpu_cores: u32,
    pub memory_gb: f64,
    pub gpu_memory_gb: Option<f64>,
    pub disk_space_gb: f64,
    pub estimated_cost: Option<f64>,
}

/// Performance context for training decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceContext {
    pub recent_accuracy: f64,
    pub recent_confidence: f64,
    pub error_rate: f64,
    pub prediction_latency: f64,
    pub model_usage_stats: HashMap<String, u64>,
    pub trading_performance: Option<TradingPerformance>,
    pub system_health: SystemHealthSnapshot,
}

/// Trading performance snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingPerformance {
    pub profit_loss: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub win_rate: f64,
    pub trades_count: u32,
}

/// System health snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthSnapshot {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub network_latency: f64,
    pub active_models: u32,
    pub error_count_last_hour: u64,
}

/// Training thresholds configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingThresholds {
    pub accuracy_threshold: f64,
    pub confidence_threshold: f64,
    pub error_rate_threshold: f64,
    pub divergence_threshold: f64,
    pub degradation_threshold: f64,
    pub consecutive_failures_threshold: u32,
    pub loss_threshold: f64,
    pub drift_threshold: f64,
    
    // Time windows for evaluation
    pub accuracy_window_minutes: u32,
    pub error_rate_window_minutes: u32,
    pub loss_window_minutes: u32,
    pub failure_window_minutes: u32,
    
    // Model-specific thresholds
    pub model_specific_thresholds: HashMap<String, ModelThresholds>,
    
    // Notification settings
    pub min_notification_interval: Duration,
    pub max_notifications_per_hour: u32,
    pub enable_rate_limiting: bool,
}

/// Model-specific training thresholds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelThresholds {
    pub accuracy_threshold: f64,
    pub confidence_threshold: f64,
    pub retraining_interval: Duration,
    pub performance_weight: f64,
}

impl Default for TrainingThresholds {
    fn default() -> Self {
        Self {
            accuracy_threshold: 0.80,         // 80% accuracy minimum
            confidence_threshold: 0.75,       // 75% confidence minimum
            error_rate_threshold: 15.0,       // 15% error rate maximum
            divergence_threshold: 0.70,       // 70% model agreement minimum
            degradation_threshold: 20.0,      // 20% performance degradation trigger
            consecutive_failures_threshold: 5, // 5 consecutive failures
            loss_threshold: 1000.0,           // $1000 loss threshold
            drift_threshold: 0.85,            // 85% drift score
            
            accuracy_window_minutes: 30,
            error_rate_window_minutes: 15,
            loss_window_minutes: 60,
            failure_window_minutes: 10,
            
            model_specific_thresholds: HashMap::new(),
            
            min_notification_interval: Duration::minutes(10),
            max_notifications_per_hour: 6,
            enable_rate_limiting: true,
        }
    }
}

/// Training notifier that monitors performance and triggers notifications
pub struct TrainingNotifier {
    performance_rx: broadcast::Receiver<PerformanceEvent>,
    training_tx: mpsc::Sender<TrainingNotification>,
    thresholds: TrainingThresholds,
    
    // State tracking
    event_history: Arc<RwLock<VecDeque<PerformanceEvent>>>,
    notification_history: Arc<RwLock<VecDeque<TrainingNotification>>>,
    model_stats: Arc<RwLock<HashMap<String, ModelStatistics>>>,
    
    // Rate limiting
    last_notification_time: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
    notifications_this_hour: Arc<RwLock<HashMap<String, u32>>>,
    
    config: NotifierConfig,
}

/// Model statistics for tracking performance trends
#[derive(Debug, Default)]
struct ModelStatistics {
    accuracy_history: VecDeque<(DateTime<Utc>, f64)>,
    confidence_history: VecDeque<(DateTime<Utc>, f64)>,
    error_count: u64,
    prediction_count: u64,
    last_training: Option<DateTime<Utc>>,
    consecutive_failures: u32,
    performance_trend: PerformanceTrend,
}

/// Performance trend indicators
#[derive(Debug, Default)]
struct PerformanceTrend {
    accuracy_slope: f64,
    confidence_slope: f64,
    error_rate_slope: f64,
    trend_confidence: f64,
}

/// Notifier configuration
#[derive(Debug, Clone)]
pub struct NotifierConfig {
    pub history_buffer_size: usize,
    pub statistics_window_hours: u32,
    pub enable_trend_analysis: bool,
    pub enable_predictive_notifications: bool,
    pub notification_batch_size: u32,
    pub performance_check_interval: Duration,
}

impl Default for NotifierConfig {
    fn default() -> Self {
        Self {
            history_buffer_size: 10000,
            statistics_window_hours: 24,
            enable_trend_analysis: true,
            enable_predictive_notifications: true,
            notification_batch_size: 10,
            performance_check_interval: Duration::seconds(30),
        }
    }
}

impl TrainingNotifier {
    /// Create a new training notifier
    pub fn new(
        performance_rx: broadcast::Receiver<PerformanceEvent>,
        training_tx: mpsc::Sender<TrainingNotification>,
        thresholds: TrainingThresholds,
        config: NotifierConfig,
    ) -> Self {
        info!("Initializing Training Notifier with thresholds: {:?}", thresholds);
        
        Self {
            performance_rx,
            training_tx,
            thresholds,
            event_history: Arc::new(RwLock::new(VecDeque::with_capacity(config.history_buffer_size))),
            notification_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            model_stats: Arc::new(RwLock::new(HashMap::new())),
            last_notification_time: Arc::new(RwLock::new(HashMap::new())),
            notifications_this_hour: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Start monitoring performance events and generating notifications
    #[instrument(skip(self))]
    pub async fn start_monitoring(&mut self) -> Result<()> {
        info!("Starting training notification monitoring");
        
        let mut check_interval = tokio::time::interval(self.config.performance_check_interval);
        
        loop {
            tokio::select! {
                // Process incoming performance events
                event_result = self.performance_rx.recv() => {
                    match event_result {
                        Ok(event) => {
                            if let Err(e) = self.process_performance_event(event).await {
                                error!("Failed to process performance event: {}", e);
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(missed)) => {
                            warn!("Training notifier lagged behind, missed {} events", missed);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            warn!("Performance channel closed, stopping training notifier");
                            break;
                        }
                    }
                }
                
                // Periodic performance checks and trend analysis
                _ = check_interval.tick() => {
                    if let Err(e) = self.perform_periodic_checks().await {
                        error!("Failed to perform periodic checks: {}", e);
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Process a single performance event
    #[instrument(skip(self, event), fields(event_id = %event.id))]
    async fn process_performance_event(&self, event: PerformanceEvent) -> Result<()> {
        // Add to history
        if let Ok(mut history) = self.event_history.write() {
            if history.len() >= self.config.history_buffer_size {
                history.pop_front();
            }
            history.push_back(event.clone());
        }

        // Update model statistics
        self.update_model_statistics(&event).await?;

        // Check for immediate triggers
        if let Some(notification) = self.evaluate_triggers(&event).await? {
            if self.should_send_notification(&notification).await {
                self.send_notification(notification).await?;
            }
        }

        Ok(())
    }

    /// Update model statistics based on performance event
    async fn update_model_statistics(&self, event: &PerformanceEvent) -> Result<()> {
        if let PerformanceEventType::PredictionCompleted { 
            model, accuracy, confidence, .. 
        } = &event.event_type {
            let mut stats = self.model_stats.write().unwrap();
            let model_stats = stats.entry(model.clone()).or_default();
            
            // Update accuracy history
            model_stats.accuracy_history.push_back((event.timestamp, *accuracy));
            if model_stats.accuracy_history.len() > 1000 {
                model_stats.accuracy_history.pop_front();
            }
            
            // Update confidence history  
            model_stats.confidence_history.push_back((event.timestamp, *confidence));
            if model_stats.confidence_history.len() > 1000 {
                model_stats.confidence_history.pop_front();
            }
            
            model_stats.prediction_count += 1;
            
            // Track consecutive failures
            if *accuracy < self.thresholds.accuracy_threshold {
                model_stats.consecutive_failures += 1;
                model_stats.error_count += 1;
            } else {
                model_stats.consecutive_failures = 0;
            }
            
            // Update performance trend if enabled
            if self.config.enable_trend_analysis {
                self.update_performance_trend(model_stats);
            }
        }
        
        Ok(())
    }

    /// Update performance trend analysis
    fn update_performance_trend(&self, stats: &mut ModelStatistics) {
        if stats.accuracy_history.len() < 10 {
            return; // Need minimum data points for trend analysis
        }
        
        // Simple linear regression for trend detection
        let accuracy_points: Vec<(f64, f64)> = stats.accuracy_history
            .iter()
            .enumerate()
            .map(|(i, (_, acc))| (i as f64, *acc))
            .collect();
        
        stats.performance_trend.accuracy_slope = self.calculate_slope(&accuracy_points);
        
        let confidence_points: Vec<(f64, f64)> = stats.confidence_history
            .iter()
            .enumerate()
            .map(|(i, (_, conf))| (i as f64, *conf))
            .collect();
        
        stats.performance_trend.confidence_slope = self.calculate_slope(&confidence_points);
        
        // Calculate trend confidence based on data consistency
        stats.performance_trend.trend_confidence = self.calculate_trend_confidence(&accuracy_points);
    }

    /// Calculate slope for linear trend
    fn calculate_slope(&self, points: &[(f64, f64)]) -> f64 {
        if points.len() < 2 {
            return 0.0;
        }
        
        let n = points.len() as f64;
        let sum_x: f64 = points.iter().map(|(x, _)| x).sum();
        let sum_y: f64 = points.iter().map(|(_, y)| y).sum();
        let sum_xy: f64 = points.iter().map(|(x, y)| x * y).sum();
        let sum_x_sq: f64 = points.iter().map(|(x, _)| x * x).sum();
        
        let denominator = n * sum_x_sq - sum_x * sum_x;
        if denominator.abs() < 1e-10 {
            return 0.0;
        }
        
        (n * sum_xy - sum_x * sum_y) / denominator
    }

    /// Calculate confidence in trend analysis
    fn calculate_trend_confidence(&self, points: &[(f64, f64)]) -> f64 {
        if points.len() < 3 {
            return 0.0;
        }
        
        // Calculate R-squared as a measure of trend confidence
        let mean_y = points.iter().map(|(_, y)| y).sum::<f64>() / points.len() as f64;
        let slope = self.calculate_slope(points);
        let intercept = mean_y - slope * (points.len() as f64 - 1.0) / 2.0;
        
        let ss_tot: f64 = points.iter().map(|(_, y)| (y - mean_y).powi(2)).sum();
        let ss_res: f64 = points.iter()
            .map(|(x, y)| {
                let predicted = slope * x + intercept;
                (y - predicted).powi(2)
            })
            .sum();
        
        if ss_tot < 1e-10 {
            return 1.0;
        }
        
        1.0 - (ss_res / ss_tot)
    }

    /// Evaluate if event should trigger training notification
    async fn evaluate_triggers(&self, event: &PerformanceEvent) -> Result<Option<TrainingNotification>> {
        match &event.event_type {
            PerformanceEventType::PredictionCompleted { 
                model, accuracy, confidence, .. 
            } => {
                // Check accuracy threshold
                if *accuracy < self.thresholds.accuracy_threshold {
                    return Ok(Some(self.create_training_notification(
                        TrainingTriggerReason::LowAccuracy {
                            current_accuracy: *accuracy,
                            threshold: self.thresholds.accuracy_threshold,
                            window_size: self.thresholds.accuracy_window_minutes as usize,
                        },
                        model.clone(),
                        TrainingPriority::High,
                        event,
                    ).await?));
                }
                
                // Check confidence threshold
                if *confidence < self.thresholds.confidence_threshold {
                    return Ok(Some(self.create_training_notification(
                        TrainingTriggerReason::LowConfidence {
                            current_confidence: *confidence,
                            threshold: self.thresholds.confidence_threshold,
                            consecutive_occurrences: 1, // Would need to track this
                        },
                        model.clone(),
                        TrainingPriority::Medium,
                        event,
                    ).await?));
                }
                
                // Check consecutive failures
                if let Ok(stats) = self.model_stats.read() {
                    if let Some(model_stats) = stats.get(model) {
                        if model_stats.consecutive_failures >= self.thresholds.consecutive_failures_threshold {
                            return Ok(Some(self.create_training_notification(
                                TrainingTriggerReason::ConsecutiveFailures {
                                    failure_count: model_stats.consecutive_failures,
                                    threshold: self.thresholds.consecutive_failures_threshold,
                                    failure_window: Duration::minutes(self.thresholds.failure_window_minutes as i64),
                                },
                                model.clone(),
                                TrainingPriority::Critical,
                                event,
                            ).await?));
                        }
                    }
                }
            }
            
            PerformanceEventType::ModelDivergence { 
                model_agreement, divergence_score, .. 
            } => {
                if *model_agreement < self.thresholds.divergence_threshold {
                    return Ok(Some(self.create_training_notification(
                        TrainingTriggerReason::ModelDivergence {
                            divergence_score: *divergence_score,
                            threshold: self.thresholds.divergence_threshold,
                            models_disagreeing: vec!["ensemble".to_string()], // Would need actual model list
                        },
                        "ensemble".to_string(),
                        TrainingPriority::High,
                        event,
                    ).await?));
                }
            }
            
            PerformanceEventType::TradingSignal { profit_loss, .. } => {
                if *profit_loss < -self.thresholds.loss_threshold {
                    return Ok(Some(self.create_training_notification(
                        TrainingTriggerReason::TradingLosses {
                            cumulative_loss: profit_loss.abs(),
                            loss_threshold: self.thresholds.loss_threshold,
                            loss_window: Duration::minutes(self.thresholds.loss_window_minutes as i64),
                        },
                        "trading_model".to_string(),
                        TrainingPriority::Critical,
                        event,
                    ).await?));
                }
            }
            
            _ => {}
        }
        
        Ok(None)
    }

    /// Create training notification from trigger
    async fn create_training_notification(
        &self,
        trigger_reason: TrainingTriggerReason,
        model_type: String,
        priority: TrainingPriority,
        context_event: &PerformanceEvent,
    ) -> Result<TrainingNotification> {
        let performance_context = self.build_performance_context(&model_type).await;
        
        let recommended_action = self.determine_training_action(&trigger_reason, &performance_context);
        
        Ok(TrainingNotification {
            id: format!("training_notif_{}", chrono::Utc::now().timestamp_nanos()),
            timestamp: Utc::now(),
            trigger_reason,
            model_type,
            priority,
            recommended_action,
            performance_context,
            metadata: HashMap::from([
                ("source_event_id".to_string(), context_event.id.clone()),
                ("notifier_version".to_string(), "1.0.0".to_string()),
            ]),
        })
    }

    /// Build performance context for notification
    async fn build_performance_context(&self, model_type: &str) -> PerformanceContext {
        let stats = self.model_stats.read().unwrap();
        let model_stats = stats.get(model_type);
        
        let (recent_accuracy, recent_confidence) = if let Some(stats) = model_stats {
            let accuracy = stats.accuracy_history.back().map(|(_, a)| *a).unwrap_or(0.0);
            let confidence = stats.confidence_history.back().map(|(_, c)| *c).unwrap_or(0.0);
            (accuracy, confidence)
        } else {
            (0.0, 0.0)
        };
        
        PerformanceContext {
            recent_accuracy,
            recent_confidence,
            error_rate: 0.05, // Would calculate from actual data
            prediction_latency: 100.0, // Would calculate from actual data
            model_usage_stats: HashMap::new(), // Would populate from actual data
            trading_performance: None, // Would populate from trading events
            system_health: SystemHealthSnapshot {
                cpu_usage: 0.5,
                memory_usage: 0.6,
                disk_usage: 0.3,
                network_latency: 10.0,
                active_models: 1,
                error_count_last_hour: 0,
            },
        }
    }

    /// Determine appropriate training action based on trigger
    fn determine_training_action(
        &self, 
        trigger_reason: &TrainingTriggerReason, 
        _context: &PerformanceContext
    ) -> TrainingAction {
        match trigger_reason {
            TrainingTriggerReason::LowAccuracy { .. } => {
                TrainingAction::FullRetrain {
                    data_points: 10000,
                    estimated_duration: Duration::hours(2),
                    resource_requirements: ResourceRequirements {
                        cpu_cores: 4,
                        memory_gb: 8.0,
                        gpu_memory_gb: Some(4.0),
                        disk_space_gb: 10.0,
                        estimated_cost: Some(50.0),
                    },
                }
            }
            
            TrainingTriggerReason::LowConfidence { .. } => {
                TrainingAction::HyperparameterTuning {
                    parameters_to_tune: vec!["learning_rate".to_string(), "batch_size".to_string()],
                    search_space: HashMap::from([
                        ("learning_rate".to_string(), (0.0001, 0.1)),
                        ("batch_size".to_string(), (16.0, 128.0)),
                    ]),
                    max_trials: 50,
                }
            }
            
            TrainingTriggerReason::ConsecutiveFailures { .. } => {
                TrainingAction::ModelArchitectureChange {
                    current_architecture: "MLP".to_string(),
                    recommended_architecture: "LSTM".to_string(),
                    migration_required: true,
                }
            }
            
            _ => {
                TrainingAction::IncrementalUpdate {
                    new_samples: 1000,
                    update_strategy: "online_learning".to_string(),
                    estimated_duration: Duration::minutes(30),
                }
            }
        }
    }

    /// Check if notification should be sent (rate limiting)
    async fn should_send_notification(&self, notification: &TrainingNotification) -> bool {
        if !self.thresholds.enable_rate_limiting {
            return true;
        }
        
        let now = Utc::now();
        let model_key = notification.model_type.clone();
        
        // Check minimum interval
        if let Ok(last_times) = self.last_notification_time.read() {
            if let Some(last_time) = last_times.get(&model_key) {
                if now - *last_time < self.thresholds.min_notification_interval {
                    debug!("Notification rate limited for model: {}", model_key);
                    return false;
                }
            }
        }
        
        // Check hourly limit
        if let Ok(hourly_counts) = self.notifications_this_hour.read() {
            if let Some(count) = hourly_counts.get(&model_key) {
                if *count >= self.thresholds.max_notifications_per_hour {
                    debug!("Hourly notification limit reached for model: {}", model_key);
                    return false;
                }
            }
        }
        
        true
    }

    /// Send training notification
    async fn send_notification(&self, notification: TrainingNotification) -> Result<()> {
        info!("Sending training notification: {:?} for model: {}", 
              notification.trigger_reason, notification.model_type);
        
        // Send notification
        self.training_tx
            .send(notification.clone())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send training notification: {}", e))?;
        
        // Update rate limiting tracking
        let model_key = notification.model_type.clone();
        let now = Utc::now();
        
        if let Ok(mut last_times) = self.last_notification_time.write() {
            last_times.insert(model_key.clone(), now);
        }
        
        if let Ok(mut hourly_counts) = self.notifications_this_hour.write() {
            *hourly_counts.entry(model_key).or_insert(0) += 1;
        }
        
        // Add to notification history
        if let Ok(mut history) = self.notification_history.write() {
            if history.len() >= 1000 {
                history.pop_front();
            }
            history.push_back(notification);
        }
        
        Ok(())
    }

    /// Perform periodic checks and maintenance
    async fn perform_periodic_checks(&self) -> Result<()> {
        let now = Utc::now();
        
        // Reset hourly counters
        if let Ok(mut hourly_counts) = self.notifications_this_hour.write() {
            hourly_counts.clear(); // Simple approach - could be more sophisticated
        }
        
        // Perform trend analysis if enabled
        if self.config.enable_trend_analysis {
            self.analyze_performance_trends().await?;
        }
        
        // Clean up old data
        self.cleanup_old_data(now).await?;
        
        Ok(())
    }

    /// Analyze performance trends for predictive notifications
    async fn analyze_performance_trends(&self) -> Result<()> {
        if !self.config.enable_predictive_notifications {
            return Ok(());
        }
        
        // Analyze trends and generate predictive notifications
        // This would be more sophisticated in a real implementation
        debug!("Performing trend analysis for predictive notifications");
        
        Ok(())
    }

    /// Clean up old data to prevent memory leaks
    async fn cleanup_old_data(&self, now: DateTime<Utc>) -> Result<()> {
        let cutoff = now - Duration::hours(self.config.statistics_window_hours as i64);
        
        // Clean up model statistics
        if let Ok(mut stats) = self.model_stats.write() {
            for model_stats in stats.values_mut() {
                model_stats.accuracy_history.retain(|(time, _)| *time > cutoff);
                model_stats.confidence_history.retain(|(time, _)| *time > cutoff);
            }
        }
        
        // Clean up event history
        if let Ok(mut history) = self.event_history.write() {
            history.retain(|event| event.timestamp > cutoff);
        }
        
        debug!("Cleaned up old performance data before {}", cutoff);
        Ok(())
    }

    /// Get notification statistics
    pub async fn get_notification_stats(&self) -> Result<NotificationStats> {
        let history = self.notification_history.read().unwrap();
        let stats = self.model_stats.read().unwrap();
        
        let total_notifications = history.len();
        let notifications_by_priority: HashMap<TrainingPriority, u32> = history
            .iter()
            .fold(HashMap::new(), |mut acc, notif| {
                *acc.entry(notif.priority.clone()).or_insert(0) += 1;
                acc
            });
        
        let models_monitored = stats.len();
        let total_predictions: u64 = stats.values().map(|s| s.prediction_count).sum();
        
        Ok(NotificationStats {
            total_notifications,
            notifications_by_priority,
            models_monitored,
            total_predictions,
            uptime: Utc::now(), // Would track actual uptime
        })
    }
}

/// Notification statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationStats {
    pub total_notifications: usize,
    pub notifications_by_priority: HashMap<TrainingPriority, u32>,
    pub models_monitored: usize,
    pub total_predictions: u64,
    pub uptime: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn test_training_notifier_creation() {
        let (_tx, rx) = broadcast::channel(100);
        let (training_tx, _training_rx) = mpsc::channel(100);
        
        let thresholds = TrainingThresholds::default();
        let config = NotifierConfig::default();
        
        let notifier = TrainingNotifier::new(rx, training_tx, thresholds, config);
        
        assert_eq!(notifier.config.history_buffer_size, 10000);
    }

    #[tokio::test]
    async fn test_threshold_triggering() {
        let (_tx, rx) = broadcast::channel(100);
        let (training_tx, mut training_rx) = mpsc::channel(100);
        
        let mut thresholds = TrainingThresholds::default();
        thresholds.accuracy_threshold = 0.90; // High threshold for testing
        
        let config = NotifierConfig::default();
        let notifier = TrainingNotifier::new(rx, training_tx, thresholds, config);
        
        // Create low accuracy event
        use crate::neural::monitoring::performance_channel::*;
        
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: "test_model".to_string(),
                predictor_id: "pred1".to_string(),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                model: "test_model".to_string(),
                accuracy: 0.85, // Below threshold
                confidence: 0.9,
                latency_ms: 100,
                input_features: 10,
                output_dimension: 1,
                timestamp: Utc::now(),
            })
            .priority(EventPriority::High)
            .build()
            .unwrap();
        
        // Process event
        let trigger = notifier.evaluate_triggers(&event).await.unwrap();
        assert!(trigger.is_some());
        
        if let Some(notification) = trigger {
            assert!(matches!(notification.trigger_reason, TrainingTriggerReason::LowAccuracy { .. }));
            assert_eq!(notification.model_type, "test_model");
        }
    }
}