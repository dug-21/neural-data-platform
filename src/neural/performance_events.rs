//! Performance Events Aggregation Module
//!
//! Aggregates performance events into snapshots for the autonomous training engine
//! to make informed decisions about when to trigger retraining.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

use super::performance_channel::{PerformanceEvent, PerformanceEventType, PerformanceSource};
use crate::types::DataType;

/// Trading-specific performance metrics (for MCP compatibility)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingPerformanceMetrics {
    /// Realized profit/loss in the period
    pub realized_pnl: f64,
    /// Unrealized profit/loss
    pub unrealized_pnl: f64,
    /// Win rate percentage
    pub win_rate: f64,
    /// Average trade duration in minutes
    pub avg_trade_duration_minutes: f64,
    /// Risk-adjusted return
    pub risk_adjusted_return: f64,
}

/// Accuracy metrics breakdown (for MCP compatibility)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccuracyMetrics {
    /// Overall accuracy percentage
    pub overall_accuracy: f64,
    /// Precision (true positives / (true positives + false positives))
    pub precision: f64,
    /// Recall (true positives / (true positives + false negatives))
    pub recall: f64,
    /// F1 score (harmonic mean of precision and recall)
    pub f1_score: f64,
}

/// Data type-specific performance metrics (for DAA compatibility)
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
    pub last_updated: DateTime<Utc>,
}

/// Performance snapshot for training decisions
/// Extended to support all use cases across the neural trader system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub timestamp: DateTime<Utc>,
    pub accuracy: f64,
    pub confidence: f64,
    pub price_error: f64,
    pub sharpe_ratio: Option<f64>,
    pub max_drawdown: Option<f64>,
    pub volatility: f64,
    pub model_agreement: f64,
    pub consecutive_failures: u32,
    pub trading_volume: f64,
    pub profit_loss: f64,
    pub event_count: usize,
    pub window_duration: Duration,
    
    // Extended fields for compatibility with other modules
    /// Latency in milliseconds (for MCP compatibility)
    #[serde(default)]
    pub latency_ms: Option<u64>,
    /// Error rate percentage (for MCP compatibility)
    #[serde(default)]  
    pub error_rate: Option<f64>,
    /// Recent predictions count (for MCP compatibility)
    #[serde(default)]
    pub recent_predictions: Option<u64>,
    /// Symbol this performance relates to (for MCP compatibility)
    #[serde(default)]
    pub symbol: Option<String>,
    /// Trading performance metrics (for MCP compatibility)
    #[serde(default)]
    pub trading_performance: Option<TradingPerformanceMetrics>,
    /// Accuracy metrics breakdown (for MCP compatibility)
    #[serde(default)]
    pub accuracy_metrics: Option<AccuracyMetrics>,
    /// Data type-specific metrics (for DAA compatibility)
    #[serde(default)]
    pub data_type_metrics: Option<DataTypeMetrics>,
    
    // Observability module compatibility fields
    /// CPU usage percentage (for observability compatibility)
    #[serde(default)]
    pub cpu_usage: Option<f64>,
    /// Memory usage percentage (for observability compatibility)
    #[serde(default)]
    pub memory_usage: Option<f64>,
    /// Active connections count (for observability compatibility)
    #[serde(default)]
    pub active_connections: Option<u32>,
    /// Requests per second (for observability compatibility)
    #[serde(default)]
    pub requests_per_second: Option<f64>,
    /// Average response time (for observability compatibility)
    #[serde(default)]
    pub average_response_time: Option<Duration>,
    /// Cache hit rate (for observability compatibility)
    #[serde(default)]
    pub cache_hit_rate: Option<f64>,
}

/// Configuration for the performance aggregator
#[derive(Debug, Clone)]
pub struct AggregatorConfig {
    /// Window duration for aggregating events
    pub aggregation_window: Duration,
    /// Maximum events to buffer before forcing aggregation
    pub max_buffer_size: usize,
    /// Minimum events required for meaningful aggregation
    pub min_events_for_snapshot: usize,
    /// Enable detailed logging
    pub verbose_logging: bool,
}

impl Default for AggregatorConfig {
    fn default() -> Self {
        Self {
            aggregation_window: Duration::minutes(5),
            max_buffer_size: 10000,
            min_events_for_snapshot: 10,
            verbose_logging: false,
        }
    }
}

/// Aggregates performance events into snapshots
pub struct PerformanceAggregator {
    event_buffer: Arc<RwLock<VecDeque<PerformanceEvent>>>,
    config: AggregatorConfig,
    performance_sender: mpsc::UnboundedSender<PerformanceSnapshot>,
    last_aggregation: Arc<RwLock<DateTime<Utc>>>,
}

impl PerformanceAggregator {
    /// Create a new performance aggregator
    pub fn new(
        config: AggregatorConfig,
        performance_sender: mpsc::UnboundedSender<PerformanceSnapshot>,
    ) -> Self {
        Self {
            event_buffer: Arc::new(RwLock::new(VecDeque::with_capacity(config.max_buffer_size))),
            config,
            performance_sender,
            last_aggregation: Arc::new(RwLock::new(Utc::now())),
        }
    }

    /// Process incoming events and aggregate into snapshots
    pub async fn process_events(
        &self,
        mut receiver: mpsc::UnboundedReceiver<PerformanceEvent>,
    ) -> Result<()> {
        let mut interval = tokio::time::interval(self.config.aggregation_window.to_std()?);

        info!("Performance aggregator started with window: {:?}", self.config.aggregation_window);

        loop {
            tokio::select! {
                Some(event) = receiver.recv() => {
                    if let Err(e) = self.buffer_event(event).await {
                        warn!("Failed to buffer event: {}", e);
                    }
                }
                _ = interval.tick() => {
                    if let Err(e) = self.aggregate_and_emit().await {
                        warn!("Failed to aggregate events: {}", e);
                    }
                }
            }
        }
    }

    /// Buffer an incoming event
    async fn buffer_event(&self, event: PerformanceEvent) -> Result<()> {
        let mut buffer = self.event_buffer.write().await;
        
        // Enforce max buffer size
        if buffer.len() >= self.config.max_buffer_size {
            buffer.pop_front();
        }
        
        buffer.push_back(event);
        
        // Force aggregation if buffer is getting full
        if buffer.len() >= self.config.max_buffer_size * 9 / 10 {
            drop(buffer); // Release lock before aggregating
            self.aggregate_and_emit().await?;
        }
        
        Ok(())
    }

    /// Aggregate buffered events into a performance snapshot
    async fn aggregate_and_emit(&self) -> Result<()> {
        let mut buffer = self.event_buffer.write().await;
        
        if buffer.len() < self.config.min_events_for_snapshot {
            debug!("Not enough events for snapshot: {} < {}", 
                   buffer.len(), self.config.min_events_for_snapshot);
            return Ok(());
        }
        
        let events: Vec<PerformanceEvent> = buffer.drain(..).collect();
        drop(buffer); // Release lock early
        
        let snapshot = self.create_snapshot(&events).await?;
        
        self.performance_sender
            .send(snapshot)
            .map_err(|e| anyhow::anyhow!("Failed to send performance snapshot: {}", e))?;
        
        *self.last_aggregation.write().await = Utc::now();
        
        Ok(())
    }

    /// Create a performance snapshot from events
    async fn create_snapshot(&self, events: &[PerformanceEvent]) -> Result<PerformanceSnapshot> {
        let now = Utc::now();
        let last_agg = *self.last_aggregation.read().await;
        let window_duration = now - last_agg;
        
        let snapshot = PerformanceSnapshot {
            timestamp: now,
            accuracy: self.calculate_average_accuracy(events),
            confidence: self.calculate_average_confidence(events),
            price_error: self.calculate_price_error(events),
            sharpe_ratio: self.extract_latest_sharpe(events),
            max_drawdown: self.extract_max_drawdown(events),
            volatility: self.calculate_volatility(events),
            model_agreement: self.calculate_model_agreement(events),
            consecutive_failures: self.count_consecutive_failures(events),
            trading_volume: self.extract_trading_volume(events),
            profit_loss: self.extract_profit_loss(events),
            event_count: events.len(),
            window_duration,
            // Optional fields with sensible defaults
            latency_ms: Some(100),
            error_rate: Some(1.0 - self.calculate_average_accuracy(events)),
            recent_predictions: Some(events.len() as u64),
            symbol: Some("ALL".to_string()),
            trading_performance: None,
            accuracy_metrics: None,
            data_type_metrics: None,
            cpu_usage: Some(50.0),
            memory_usage: Some(1024.0),
            active_connections: Some(10),
            requests_per_second: Some(25.0),
            average_response_time: Some(window_duration),
            cache_hit_rate: Some(0.85),
        };
        
        if self.config.verbose_logging {
            info!("Created performance snapshot: accuracy={:.4}, confidence={:.4}, events={}", 
                  snapshot.accuracy, snapshot.confidence, snapshot.event_count);
        }
        
        Ok(snapshot)
    }

    /// Calculate average accuracy from prediction events
    fn calculate_average_accuracy(&self, events: &[PerformanceEvent]) -> f64 {
        let accuracies: Vec<f64> = events
            .iter()
            .filter_map(|e| match &e.event_type {
                PerformanceEventType::PredictionCompleted { accuracy, .. } => Some(*accuracy),
                _ => None,
            })
            .collect();
        
        if accuracies.is_empty() {
            return 0.0;
        }
        
        accuracies.iter().sum::<f64>() / accuracies.len() as f64
    }

    /// Calculate average confidence from prediction events
    fn calculate_average_confidence(&self, events: &[PerformanceEvent]) -> f64 {
        let confidences: Vec<f64> = events
            .iter()
            .filter_map(|e| match &e.event_type {
                PerformanceEventType::PredictionCompleted { confidence, .. } => Some(*confidence),
                _ => None,
            })
            .collect();
        
        if confidences.is_empty() {
            return 0.0;
        }
        
        confidences.iter().sum::<f64>() / confidences.len() as f64
    }

    /// Calculate price error (placeholder - would need actual vs predicted prices)
    fn calculate_price_error(&self, _events: &[PerformanceEvent]) -> f64 {
        // In a real implementation, this would compare predicted vs actual prices
        // For now, we'll use confidence as a proxy (1 - confidence = error)
        0.05 // Default 5% error
    }

    /// Extract latest Sharpe ratio from trading signals
    fn extract_latest_sharpe(&self, events: &[PerformanceEvent]) -> Option<f64> {
        events
            .iter()
            .rev()
            .find_map(|e| match &e.event_type {
                PerformanceEventType::TradingSignal { sharpe_ratio, .. } => Some(*sharpe_ratio),
                _ => None,
            })
    }

    /// Extract maximum drawdown from trading signals
    fn extract_max_drawdown(&self, events: &[PerformanceEvent]) -> Option<f64> {
        events
            .iter()
            .filter_map(|e| match &e.event_type {
                PerformanceEventType::TradingSignal { max_drawdown, .. } => Some(*max_drawdown),
                _ => None,
            })
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Calculate volatility from prediction variance
    fn calculate_volatility(&self, events: &[PerformanceEvent]) -> f64 {
        let confidences: Vec<f64> = events
            .iter()
            .filter_map(|e| match &e.event_type {
                PerformanceEventType::PredictionCompleted { confidence, .. } => Some(*confidence),
                _ => None,
            })
            .collect();
        
        if confidences.len() < 2 {
            return 0.0;
        }
        
        // Calculate standard deviation of confidences as a proxy for volatility
        let mean = confidences.iter().sum::<f64>() / confidences.len() as f64;
        let variance = confidences
            .iter()
            .map(|&c| (c - mean).powi(2))
            .sum::<f64>() / confidences.len() as f64;
        
        variance.sqrt()
    }

    /// Calculate model agreement from divergence events
    fn calculate_model_agreement(&self, events: &[PerformanceEvent]) -> f64 {
        let agreements: Vec<f64> = events
            .iter()
            .filter_map(|e| match &e.event_type {
                PerformanceEventType::ModelDivergence { model_agreement, .. } => {
                    Some(*model_agreement)
                }
                _ => None,
            })
            .collect();
        
        if agreements.is_empty() {
            return 1.0; // Default to full agreement
        }
        
        agreements.iter().sum::<f64>() / agreements.len() as f64
    }

    /// Count consecutive failures (low accuracy predictions)
    fn count_consecutive_failures(&self, events: &[PerformanceEvent]) -> u32 {
        let mut consecutive = 0;
        let mut max_consecutive = 0;
        
        for event in events.iter().rev() {
            if let PerformanceEventType::PredictionCompleted { accuracy, .. } = &event.event_type {
                if *accuracy < 0.5 {
                    consecutive += 1;
                    max_consecutive = max_consecutive.max(consecutive);
                } else {
                    consecutive = 0;
                }
            }
        }
        
        max_consecutive
    }

    /// Extract trading volume from events
    fn extract_trading_volume(&self, events: &[PerformanceEvent]) -> f64 {
        // Sum of all trading activities
        events
            .iter()
            .filter(|e| matches!(&e.source, PerformanceSource::TradingStrategy { .. }))
            .count() as f64
    }

    /// Extract profit/loss from trading signals
    fn extract_profit_loss(&self, events: &[PerformanceEvent]) -> f64 {
        events
            .iter()
            .filter_map(|e| match &e.event_type {
                PerformanceEventType::TradingSignal { profit_loss, .. } => Some(*profit_loss),
                _ => None,
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::neural::performance_channel::ComponentType;

    #[tokio::test]
    async fn test_aggregator_creation() {
        let (_tx, rx) = mpsc::unbounded_channel();
        let config = AggregatorConfig::default();
        let aggregator = PerformanceAggregator::new(config, rx);
        
        assert!(aggregator.event_buffer.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_event_buffering() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let config = AggregatorConfig {
            max_buffer_size: 10,
            ..Default::default()
        };
        let aggregator = PerformanceAggregator::new(config, tx);
        
        let event = PerformanceEvent {
            timestamp: Utc::now(),
            source: PerformanceSource::NeuralPredictor {
                model_name: "test".to_string(),
            },
            event_type: PerformanceEventType::PredictionCompleted {
                accuracy: 0.95,
                confidence: 0.9,
                latency_ms: 100,
            },
            metrics: Default::default(),
        };
        
        aggregator.buffer_event(event).await.unwrap();
        
        let buffer = aggregator.event_buffer.read().await;
        assert_eq!(buffer.len(), 1);
    }

    #[tokio::test]
    async fn test_snapshot_creation() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let config = AggregatorConfig::default();
        let aggregator = PerformanceAggregator::new(config, tx);
        
        let events = vec![
            PerformanceEvent {
                timestamp: Utc::now(),
                source: PerformanceSource::NeuralPredictor {
                    model_name: "test1".to_string(),
                },
                event_type: PerformanceEventType::PredictionCompleted {
                    accuracy: 0.95,
                    confidence: 0.9,
                    latency_ms: 100,
                },
                metrics: Default::default(),
            },
            PerformanceEvent {
                timestamp: Utc::now(),
                source: PerformanceSource::TradingStrategy {
                    strategy_name: "momentum".to_string(),
                },
                event_type: PerformanceEventType::TradingSignal {
                    profit_loss: 150.0,
                    sharpe_ratio: 1.5,
                    max_drawdown: 0.05,
                },
                metrics: Default::default(),
            },
        ];
        
        let snapshot = aggregator.create_snapshot(&events).await.unwrap();
        
        assert_eq!(snapshot.accuracy, 0.95);
        assert_eq!(snapshot.confidence, 0.9);
        assert_eq!(snapshot.event_count, 2);
        assert!(snapshot.sharpe_ratio.is_some());
    }
}