//! Performance Event Definitions and Aggregation
//!
//! This module defines performance events and provides aggregation logic to convert
//! streams of events into performance snapshots for the training engine.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::daa::autonomous_training::PerformanceSnapshot;
use super::performance_channel::{PerformanceEvent, PerformanceEventType, PerformanceSource};

/// Configuration for performance aggregation
#[derive(Debug, Clone)]
pub struct AggregationConfig {
    /// Time window for aggregating events
    pub aggregation_window: Duration,
    /// Maximum number of events to buffer
    pub max_buffer_size: usize,
    /// Minimum events required for aggregation
    pub min_events_for_snapshot: usize,
    /// Weight decay for older events
    pub time_decay_factor: f64,
}

impl Default for AggregationConfig {
    fn default() -> Self {
        Self {
            aggregation_window: Duration::minutes(5),
            max_buffer_size: 10000,
            min_events_for_snapshot: 10,
            time_decay_factor: 0.95,
        }
    }
}

/// Aggregates performance events into snapshots
pub struct PerformanceAggregator {
    /// Configuration
    config: AggregationConfig,
    /// Buffer of recent events
    event_buffer: Arc<RwLock<VecDeque<PerformanceEvent>>>,
    /// Channel to send aggregated snapshots
    snapshot_sender: mpsc::UnboundedSender<PerformanceSnapshot>,
    /// Running statistics
    stats: Arc<RwLock<AggregatorStats>>,
    /// Model-specific metrics tracking
    model_metrics: Arc<RwLock<HashMap<String, ModelMetrics>>>,
}

/// Statistics for the aggregator
#[derive(Debug, Default)]
struct AggregatorStats {
    total_events_processed: u64,
    total_snapshots_generated: u64,
    last_snapshot_time: Option<DateTime<Utc>>,
    events_per_source: HashMap<String, u64>,
}

/// Metrics tracked per model
#[derive(Debug, Default)]
struct ModelMetrics {
    predictions: u64,
    total_confidence: f64,
    total_accuracy: f64,
    errors: u64,
    last_prediction_time: Option<DateTime<Utc>>,
}

impl PerformanceAggregator {
    /// Create a new performance aggregator
    pub fn new(
        config: AggregationConfig,
        snapshot_sender: mpsc::UnboundedSender<PerformanceSnapshot>,
    ) -> Self {
        Self {
            config,
            event_buffer: Arc::new(RwLock::new(VecDeque::with_capacity(config.max_buffer_size))),
            snapshot_sender,
            stats: Arc::new(RwLock::new(AggregatorStats::default())),
            model_metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Process incoming events and aggregate into snapshots
    pub async fn start_processing(
        &self,
        mut event_receiver: mpsc::UnboundedReceiver<PerformanceEvent>,
    ) -> Result<()> {
        let mut interval = tokio::time::interval(self.config.aggregation_window.to_std()?);
        
        info!("Starting performance aggregator with window: {:?}", self.config.aggregation_window);
        
        loop {
            tokio::select! {
                Some(event) = event_receiver.recv() => {
                    self.process_event(event).await?;
                }
                _ = interval.tick() => {
                    if let Err(e) = self.aggregate_and_emit().await {
                        error!("Failed to aggregate performance snapshot: {}", e);
                    }
                }
            }
        }
    }
    
    /// Process a single event
    async fn process_event(&self, event: PerformanceEvent) -> Result<()> {
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_events_processed += 1;
            
            let source_key = format!("{:?}", event.source);
            *stats.events_per_source.entry(source_key).or_insert(0) += 1;
        }
        
        // Update model-specific metrics
        if let PerformanceSource::NeuralPredictor { model_name } = &event.source {
            let mut model_metrics = self.model_metrics.write().await;
            let metrics = model_metrics.entry(model_name.clone()).or_default();
            
            match &event.event_type {
                PerformanceEventType::PredictionCompleted { accuracy, confidence, .. } => {
                    metrics.predictions += 1;
                    metrics.total_confidence += confidence;
                    metrics.total_accuracy += accuracy;
                    metrics.last_prediction_time = Some(event.timestamp);
                }
                PerformanceEventType::PredictionError { .. } => {
                    metrics.errors += 1;
                }
                _ => {}
            }
        }
        
        // Add to buffer
        {
            let mut buffer = self.event_buffer.write().await;
            buffer.push_back(event);
            
            // Trim buffer if too large
            while buffer.len() > self.config.max_buffer_size {
                buffer.pop_front();
            }
        }
        
        Ok(())
    }
    
    /// Aggregate buffered events into a performance snapshot
    async fn aggregate_and_emit(&self) -> Result<()> {
        let events = self.event_buffer.read().await.clone();
        
        if events.len() < self.config.min_events_for_snapshot {
            debug!("Not enough events for snapshot: {} < {}", 
                   events.len(), self.config.min_events_for_snapshot);
            return Ok(());
        }
        
        // Calculate time window
        let now = Utc::now();
        let window_start = now - self.config.aggregation_window;
        
        // Filter events within window
        let recent_events: Vec<&PerformanceEvent> = events
            .iter()
            .filter(|e| e.timestamp >= window_start)
            .collect();
        
        if recent_events.is_empty() {
            return Ok(());
        }
        
        // Calculate aggregated metrics
        let snapshot = self.calculate_snapshot(&recent_events, now).await?;
        
        // Send snapshot
        self.snapshot_sender.send(snapshot)?;
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_snapshots_generated += 1;
            stats.last_snapshot_time = Some(now);
        }
        
        // Clear old events from buffer
        {
            let mut buffer = self.event_buffer.write().await;
            buffer.retain(|e| e.timestamp >= window_start);
        }
        
        Ok(())
    }
    
    /// Calculate performance snapshot from events
    async fn calculate_snapshot(
        &self,
        events: &[&PerformanceEvent],
        timestamp: DateTime<Utc>,
    ) -> Result<PerformanceSnapshot> {
        let mut accuracy_sum = 0.0;
        let mut confidence_sum = 0.0;
        let mut prediction_count = 0;
        let mut error_count = 0;
        let mut sharpe_ratios = Vec::new();
        let mut drawdowns = Vec::new();
        let mut profit_losses = Vec::new();
        let mut model_agreements = Vec::new();
        let mut volatilities = Vec::new();
        let mut trading_volumes = Vec::new();
        
        // Process events with time decay
        for (i, event) in events.iter().enumerate() {
            let age_factor = (i as f64 / events.len() as f64) * self.config.time_decay_factor;
            let weight = 1.0 - age_factor;
            
            match &event.event_type {
                PerformanceEventType::PredictionCompleted { 
                    accuracy, confidence, model_agreement, .. 
                } => {
                    accuracy_sum += accuracy * weight;
                    confidence_sum += confidence * weight;
                    prediction_count += 1;
                    
                    if let Some(agreement) = model_agreement {
                        model_agreements.push(*agreement);
                    }
                }
                PerformanceEventType::TradingSignal { 
                    profit_loss, sharpe_ratio, max_drawdown, .. 
                } => {
                    profit_losses.push(*profit_loss);
                    sharpe_ratios.push(*sharpe_ratio);
                    drawdowns.push(*max_drawdown);
                }
                PerformanceEventType::PredictionError { .. } => {
                    error_count += 1;
                }
                PerformanceEventType::MarketRegimeChange { .. } => {
                    // Extract volatility from metrics if available
                    if let Some(vol) = event.metrics.get("volatility") {
                        volatilities.push(*vol);
                    }
                }
                _ => {}
            }
            
            // Extract trading volume from metrics
            if let Some(volume) = event.metrics.get("trading_volume") {
                trading_volumes.push(*volume);
            }
        }
        
        // Calculate averages and aggregates
        let avg_accuracy = if prediction_count > 0 {
            accuracy_sum / prediction_count as f64
        } else {
            0.5 // Default when no predictions
        };
        
        let avg_confidence = if prediction_count > 0 {
            confidence_sum / prediction_count as f64
        } else {
            0.5
        };
        
        let price_error = 1.0 - avg_accuracy;
        
        let sharpe_ratio = if !sharpe_ratios.is_empty() {
            sharpe_ratios.iter().sum::<f64>() / sharpe_ratios.len() as f64
        } else {
            0.0
        };
        
        let max_drawdown = drawdowns.iter().cloned().fold(0.0, f64::max);
        
        let volatility = if !volatilities.is_empty() {
            volatilities.iter().sum::<f64>() / volatilities.len() as f64
        } else {
            self.calculate_volatility_from_prices(&profit_losses)
        };
        
        let model_agreement = if !model_agreements.is_empty() {
            model_agreements.iter().sum::<f64>() / model_agreements.len() as f64
        } else {
            1.0 // Default to full agreement if not measured
        };
        
        let consecutive_failures = self.count_consecutive_errors(events);
        
        let trading_volume = if !trading_volumes.is_empty() {
            trading_volumes.iter().sum::<f64>() / trading_volumes.len() as f64
        } else {
            1000000.0 // Default volume
        };
        
        let profit_loss = profit_losses.iter().sum::<f64>();
        
        Ok(PerformanceSnapshot {
            timestamp,
            accuracy: avg_accuracy,
            confidence: avg_confidence,
            price_error,
            sharpe_ratio,
            max_drawdown,
            volatility,
            model_agreement,
            consecutive_failures,
            trading_volume,
            profit_loss,
        })
    }
    
    /// Calculate volatility from price movements
    fn calculate_volatility_from_prices(&self, profit_losses: &[f64]) -> f64 {
        if profit_losses.len() < 2 {
            return 0.02; // Default volatility
        }
        
        let mean = profit_losses.iter().sum::<f64>() / profit_losses.len() as f64;
        let variance = profit_losses.iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f64>() / profit_losses.len() as f64;
        
        variance.sqrt()
    }
    
    /// Count consecutive prediction errors
    fn count_consecutive_errors(&self, events: &[&PerformanceEvent]) -> usize {
        let mut consecutive = 0;
        let mut current_streak = 0;
        
        for event in events.iter().rev() {
            match &event.event_type {
                PerformanceEventType::PredictionError { .. } => {
                    current_streak += 1;
                    consecutive = consecutive.max(current_streak);
                }
                PerformanceEventType::PredictionCompleted { accuracy, .. } => {
                    if *accuracy < 0.5 {
                        current_streak += 1;
                        consecutive = consecutive.max(current_streak);
                    } else {
                        current_streak = 0;
                    }
                }
                _ => {}
            }
        }
        
        consecutive
    }
    
    /// Get aggregator statistics
    pub async fn get_statistics(&self) -> AggregatorStatistics {
        let stats = self.stats.read().await;
        let model_metrics = self.model_metrics.read().await;
        
        let model_stats: HashMap<String, ModelStatistics> = model_metrics
            .iter()
            .map(|(name, metrics)| {
                let avg_confidence = if metrics.predictions > 0 {
                    metrics.total_confidence / metrics.predictions as f64
                } else {
                    0.0
                };
                
                let avg_accuracy = if metrics.predictions > 0 {
                    metrics.total_accuracy / metrics.predictions as f64
                } else {
                    0.0
                };
                
                (name.clone(), ModelStatistics {
                    predictions: metrics.predictions,
                    average_confidence: avg_confidence,
                    average_accuracy: avg_accuracy,
                    error_rate: metrics.errors as f64 / metrics.predictions.max(1) as f64,
                    last_prediction_time: metrics.last_prediction_time,
                })
            })
            .collect();
        
        AggregatorStatistics {
            total_events_processed: stats.total_events_processed,
            total_snapshots_generated: stats.total_snapshots_generated,
            last_snapshot_time: stats.last_snapshot_time,
            events_per_source: stats.events_per_source.clone(),
            model_statistics: model_stats,
        }
    }
}

/// Public statistics for the aggregator
#[derive(Debug, Clone, Serialize)]
pub struct AggregatorStatistics {
    pub total_events_processed: u64,
    pub total_snapshots_generated: u64,
    pub last_snapshot_time: Option<DateTime<Utc>>,
    pub events_per_source: HashMap<String, u64>,
    pub model_statistics: HashMap<String, ModelStatistics>,
}

/// Statistics for individual models
#[derive(Debug, Clone, Serialize)]
pub struct ModelStatistics {
    pub predictions: u64,
    pub average_confidence: f64,
    pub average_accuracy: f64,
    pub error_rate: f64,
    pub last_prediction_time: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::health::ComponentType;
    
    #[tokio::test]
    async fn test_aggregator_creation() {
        let (sender, _receiver) = mpsc::unbounded_channel();
        let config = AggregationConfig::default();
        let aggregator = PerformanceAggregator::new(config, sender);
        
        let stats = aggregator.get_statistics().await;
        assert_eq!(stats.total_events_processed, 0);
        assert_eq!(stats.total_snapshots_generated, 0);
    }
    
    #[tokio::test]
    async fn test_event_processing() {
        let (snapshot_sender, mut snapshot_receiver) = mpsc::unbounded_channel();
        let config = AggregationConfig {
            aggregation_window: Duration::seconds(1),
            min_events_for_snapshot: 2,
            ..Default::default()
        };
        let aggregator = PerformanceAggregator::new(config, snapshot_sender);
        
        // Process some events
        let event1 = PerformanceEvent::prediction_completed(
            "TestModel".to_string(),
            0.85,
            0.9,
            100,
        );
        let event2 = PerformanceEvent::trading_signal(
            "TestStrategy".to_string(),
            0.05,
            1.2,
            0.1,
        );
        
        aggregator.process_event(event1).await.unwrap();
        aggregator.process_event(event2).await.unwrap();
        
        // Trigger aggregation
        aggregator.aggregate_and_emit().await.unwrap();
        
        // Check snapshot was generated
        let snapshot = snapshot_receiver.try_recv().unwrap();
        assert!(snapshot.accuracy > 0.0);
        assert!(snapshot.confidence > 0.0);
        assert_eq!(snapshot.sharpe_ratio, 1.2);
        assert_eq!(snapshot.max_drawdown, 0.1);
    }
    
    #[tokio::test]
    async fn test_model_metrics_tracking() {
        let (sender, _receiver) = mpsc::unbounded_channel();
        let aggregator = PerformanceAggregator::new(AggregationConfig::default(), sender);
        
        // Process predictions from different models
        for i in 0..5 {
            let event = PerformanceEvent::prediction_completed(
                "Model1".to_string(),
                0.8 + (i as f64 * 0.01),
                0.85 + (i as f64 * 0.01),
                100 + i * 10,
            );
            aggregator.process_event(event).await.unwrap();
        }
        
        for i in 0..3 {
            let event = PerformanceEvent::prediction_completed(
                "Model2".to_string(),
                0.7 + (i as f64 * 0.02),
                0.75 + (i as f64 * 0.02),
                150 + i * 20,
            );
            aggregator.process_event(event).await.unwrap();
        }
        
        // Check model statistics
        let stats = aggregator.get_statistics().await;
        assert_eq!(stats.model_statistics.len(), 2);
        
        let model1_stats = &stats.model_statistics["Model1"];
        assert_eq!(model1_stats.predictions, 5);
        assert!(model1_stats.average_accuracy > 0.8);
        
        let model2_stats = &stats.model_statistics["Model2"];
        assert_eq!(model2_stats.predictions, 3);
        assert!(model2_stats.average_accuracy > 0.7);
    }
}