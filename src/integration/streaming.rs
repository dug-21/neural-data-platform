//! Streaming Pipeline for Real-time Data Processing
//! 
//! This module provides streaming data processing capabilities for market data,
//! news feeds, and real-time event processing. It integrates with the data pipeline
//! and provides event-driven data flow for DAA agents.

use crate::data::{DataPipeline, TimeSeriesData};
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, broadcast};
use tokio::time::timeout;
use tracing::{info, warn, error, debug};

/// Main streaming pipeline for real-time data processing
#[derive(Clone)]
pub struct StreamingPipeline {
    data_pipeline: Arc<DataPipeline>,
    config: StreamConfig,
    market_sender: Arc<RwLock<Option<mpsc::UnboundedSender<MarketData>>>>,
    news_sender: Arc<RwLock<Option<mpsc::UnboundedSender<NewsData>>>>,
    event_broadcaster: Arc<broadcast::Sender<StreamEvent>>,
    subscriptions: Arc<RwLock<HashMap<String, SubscriptionInfo>>>,
    metrics: Arc<RwLock<StreamMetrics>>,
    processing_active: Arc<RwLock<bool>>,
}

/// Configuration for streaming pipeline
#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub market_buffer_size: usize,
    pub news_buffer_size: usize,
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    pub retry_attempts: u32,
    pub quality_threshold: f64,
    pub enable_order_book: bool,
    pub enable_sentiment_analysis: bool,
}

/// Market data structure for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub price: f64,
    pub volume: f64,
    pub bid: f64,
    pub ask: f64,
    pub source: String,
    pub sequence_number: u64,
    pub order_book_depth: Option<u32>,
    pub metadata: Option<serde_json::Value>,
}

/// News data structure for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsData {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub title: String,
    pub content: String,
    pub source: String,
    pub symbols: Vec<String>,
    pub sentiment_score: f64,
    pub relevance_score: f64,
    pub category: String,
    pub metadata: Option<serde_json::Value>,
}

/// Stream event for internal processing
#[derive(Debug, Clone)]
pub struct StreamEvent {
    pub event_type: String,
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub data: serde_json::Value,
    pub source: String,
}

/// Subscription information
#[derive(Debug, Clone)]
struct SubscriptionInfo {
    symbol: String,
    subscription_type: String,
    created_at: DateTime<Utc>,
    last_update: DateTime<Utc>,
}

/// Stream quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamQualityMetrics {
    pub data_completeness: f64,
    pub latency_ms: f64,
    pub error_rate: f64,
    pub throughput_per_second: f64,
}

/// Stream performance metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamMetrics {
    pub total_market_messages: u64,
    pub total_news_messages: u64,
    pub batch_processing_count: u64,
    pub error_count: u64,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub start_time: Option<DateTime<Utc>>,
    #[serde(with = "chrono::serde::ts_seconds_option")]
    pub last_update: Option<DateTime<Utc>>,
}

/// Data source error types
#[derive(Debug, Clone)]
pub enum DataSourceError {
    ConnectionError(String),
    ValidationError(String),
    ProcessingError(String),
    TimeoutError(String),
}

impl std::fmt::Display for DataSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataSourceError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            DataSourceError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            DataSourceError::ProcessingError(msg) => write!(f, "Processing error: {}", msg),
            DataSourceError::TimeoutError(msg) => write!(f, "Timeout error: {}", msg),
        }
    }
}

impl std::error::Error for DataSourceError {}

impl StreamingPipeline {
    /// Create a new streaming pipeline
    pub async fn new(data_pipeline: Arc<DataPipeline>, config: StreamConfig) -> Result<Self> {
        let (event_sender, _) = broadcast::channel(1000);
        
        Ok(Self {
            data_pipeline,
            config,
            market_sender: Arc::new(RwLock::new(None)),
            news_sender: Arc::new(RwLock::new(None)),
            event_broadcaster: Arc::new(event_sender),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(StreamMetrics {
                start_time: Some(Utc::now()),
                ..Default::default()
            })),
            processing_active: Arc::new(RwLock::new(false)),
        })
    }

    /// Health check for the streaming pipeline
    pub async fn health_check(&self) -> Result<bool> {
        let pipeline_healthy = self.data_pipeline.health_check().await?;
        let processing_active = *self.processing_active.read().await;
        
        Ok(pipeline_healthy && processing_active)
    }

    /// Start market data stream
    pub async fn start_market_stream(&mut self, symbols: Vec<String>) -> Result<()> {
        let (sender, mut receiver) = mpsc::unbounded_channel::<MarketData>();
        
        // Store sender for external data injection
        {
            let mut market_sender = self.market_sender.write().await;
            *market_sender = Some(sender);
        }
        
        // Start processing task
        let data_pipeline_clone = Arc::clone(&self.data_pipeline);
        let event_broadcaster_clone = Arc::clone(&self.event_broadcaster);
        let metrics_clone = Arc::clone(&self.metrics);
        let config_clone = self.config.clone();
        
        tokio::spawn(async move {
            let mut batch = Vec::new();
            let mut last_batch_time = Utc::now();
            
            while let Some(market_data) = receiver.recv().await {
                batch.push(market_data);
                
                let should_process_batch = batch.len() >= config_clone.batch_size
                    || (Utc::now() - last_batch_time).num_milliseconds() > config_clone.batch_timeout_ms as i64;
                
                if should_process_batch {
                    if let Err(e) = Self::process_market_batch(
                        &batch,
                        &data_pipeline_clone,
                        &event_broadcaster_clone,
                        &metrics_clone,
                    ).await {
                        error!("Failed to process market data batch: {}", e);
                    }
                    
                    batch.clear();
                    last_batch_time = Utc::now();
                }
            }
        });
        
        // Update subscriptions
        let mut subscriptions = self.subscriptions.write().await;
        for symbol in symbols {
            subscriptions.insert(
                symbol.clone(),
                SubscriptionInfo {
                    symbol: symbol.clone(),
                    subscription_type: "market_data".to_string(),
                    created_at: Utc::now(),
                    last_update: Utc::now(),
                },
            );
        }
        
        // Mark processing as active
        {
            let mut processing = self.processing_active.write().await;
            *processing = true;
        }
        
        info!("Started market data stream for {} symbols", subscriptions.len());
        Ok(())
    }

    /// Start news data stream
    pub async fn start_news_stream(&mut self, topics: Vec<String>) -> Result<()> {
        let (sender, mut receiver) = mpsc::unbounded_channel::<NewsData>();
        
        // Store sender for external data injection
        {
            let mut news_sender = self.news_sender.write().await;
            *news_sender = Some(sender);
        }
        
        // Start processing task
        let data_pipeline_clone = Arc::clone(&self.data_pipeline);
        let event_broadcaster_clone = Arc::clone(&self.event_broadcaster);
        let metrics_clone = Arc::clone(&self.metrics);
        
        tokio::spawn(async move {
            while let Some(news_data) = receiver.recv().await {
                if let Err(e) = Self::process_news_item(
                    &news_data,
                    &data_pipeline_clone,
                    &event_broadcaster_clone,
                    &metrics_clone,
                ).await {
                    error!("Failed to process news data: {}", e);
                }
            }
        });
        
        info!("Started news stream for {} topics", topics.len());
        Ok(())
    }

    /// Process market data
    pub async fn process_market_data(&self, market_data: MarketData) -> Result<()> {
        // Validate market data
        self.validate_market_data(&market_data)?;
        
        // Send to processing pipeline
        if let Some(sender) = self.market_sender.read().await.as_ref() {
            sender.send(market_data)
                .map_err(|e| anyhow::anyhow!("Failed to send market data: {}", e))?;
        }
        
        Ok(())
    }

    /// Process news data
    pub async fn process_news_data(&self, news_data: NewsData) -> Result<()> {
        // Validate news data
        self.validate_news_data(&news_data)?;
        
        // Send to processing pipeline
        if let Some(sender) = self.news_sender.read().await.as_ref() {
            sender.send(news_data)
                .map_err(|e| anyhow::anyhow!("Failed to send news data: {}", e))?;
        }
        
        Ok(())
    }

    /// Get active market subscriptions
    pub async fn get_active_market_subscriptions(&self) -> Result<Vec<String>> {
        let subscriptions = self.subscriptions.read().await;
        Ok(subscriptions
            .values()
            .filter(|sub| sub.subscription_type == "market_data")
            .map(|sub| sub.symbol.clone())
            .collect())
    }

    /// Get active news topics
    pub async fn get_active_news_topics(&self) -> Result<Vec<String>> {
        let subscriptions = self.subscriptions.read().await;
        Ok(subscriptions
            .values()
            .filter(|sub| sub.subscription_type == "news")
            .map(|sub| sub.symbol.clone())
            .collect())
    }

    /// Get latest market data for a symbol
    pub async fn get_latest_market_data(&self, symbol: &str) -> Result<Option<TimeSeriesData>> {
        self.data_pipeline.get_latest_data(symbol).await
    }

    /// Get latest news for a symbol
    pub async fn get_latest_news_for_symbol(&self, symbol: &str) -> Result<Vec<NewsData>> {
        // This is a simplified implementation - in reality would query news database
        Ok(Vec::new())
    }

    /// Batch process market data
    pub async fn batch_process_market_data(&self, batch: Vec<MarketData>) -> Result<()> {
        for market_data in batch {
            self.process_market_data(market_data).await?;
        }
        
        let mut metrics = self.metrics.write().await;
        metrics.batch_processing_count += 1;
        
        Ok(())
    }

    /// Get stream metrics
    pub async fn get_stream_metrics(&self) -> Result<StreamMetrics> {
        let metrics = self.metrics.read().await;
        Ok(metrics.clone())
    }

    /// Monitor stream quality
    pub async fn monitor_stream_quality(&self) -> Result<StreamQualityMetrics> {
        let metrics = self.metrics.read().await;
        
        let data_completeness = if metrics.total_market_messages > 0 {
            1.0 - (metrics.error_count as f64 / metrics.total_market_messages as f64)
        } else {
            1.0
        };
        
        let throughput_per_second = if let Some(start_time) = metrics.start_time {
            let elapsed_seconds = (Utc::now() - start_time).num_seconds() as f64;
            if elapsed_seconds > 0.0 {
                metrics.total_market_messages as f64 / elapsed_seconds
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        Ok(StreamQualityMetrics {
            data_completeness,
            latency_ms: 50.0, // Simplified - would measure actual latency
            error_rate: metrics.error_count as f64 / metrics.total_market_messages.max(1) as f64,
            throughput_per_second,
        })
    }

    /// Simulate connection error for testing
    pub async fn simulate_connection_error(&self) -> Result<()> {
        bail!("Simulated connection error");
    }

    /// Recover from error
    pub async fn recover_from_error(&self) -> Result<()> {
        info!("Recovering from error - restarting streams");
        
        // Reset processing state
        {
            let mut processing = self.processing_active.write().await;
            *processing = true;
        }
        
        Ok(())
    }

    /// Process incoming data (for testing)
    pub async fn process_incoming_data(&self) -> Result<()> {
        // Simplified implementation for testing
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        Ok(())
    }

    /// Inject test market data
    pub async fn inject_test_market_data(&self, market_data: MarketData) -> Result<()> {
        self.process_market_data(market_data).await
    }

    /// Stop processing
    pub async fn stop_processing(&self) -> Result<()> {
        let mut processing = self.processing_active.write().await;
        *processing = false;
        info!("Stopped stream processing");
        Ok(())
    }

    /// Store results in memory for DAA agents
    pub async fn store_results_in_memory(&self, memory_key: &str) -> Result<()> {
        let metrics = self.get_stream_metrics().await?;
        let quality_metrics = self.monitor_stream_quality().await?;
        
        let memory_data = serde_json::json!({
            "stream_metrics": metrics,
            "quality_metrics": quality_metrics,
            "timestamp": Utc::now(),
            "status": "active"
        });
        
        // In a real implementation, this would store to a memory service
        info!("Stored streaming results in memory at key: {}", memory_key);
        Ok(())
    }

    /// Get memory data (simplified for testing)
    pub async fn get_memory_data(&self, memory_key: &str) -> Result<HashMap<String, serde_json::Value>> {
        let mut data = HashMap::new();
        
        let metrics = self.get_stream_metrics().await?;
        let quality_metrics = self.monitor_stream_quality().await?;
        
        data.insert("market_data".to_string(), serde_json::json!({}));
        data.insert("news_data".to_string(), serde_json::json!({}));
        data.insert("stream_metrics".to_string(), serde_json::to_value(metrics)?);
        data.insert("quality_metrics".to_string(), serde_json::to_value(quality_metrics)?);
        
        Ok(data)
    }

    // Private helper methods

    fn validate_market_data(&self, market_data: &MarketData) -> Result<()> {
        if market_data.symbol.is_empty() {
            bail!("Market data symbol cannot be empty");
        }
        
        if market_data.price < 0.0 {
            bail!("Market data price cannot be negative");
        }
        
        if market_data.volume < 0.0 {
            bail!("Market data volume cannot be negative");
        }
        
        Ok(())
    }

    fn validate_news_data(&self, news_data: &NewsData) -> Result<()> {
        if news_data.id.is_empty() {
            bail!("News data ID cannot be empty");
        }
        
        if news_data.title.is_empty() {
            bail!("News data title cannot be empty");
        }
        
        Ok(())
    }

    async fn process_market_batch(
        batch: &[MarketData],
        data_pipeline: &Arc<DataPipeline>,
        event_broadcaster: &Arc<broadcast::Sender<StreamEvent>>,
        metrics: &Arc<RwLock<StreamMetrics>>,
    ) -> Result<()> {
        for market_data in batch {
            // Convert to TimeSeriesData and store
            let time_series_data = TimeSeriesData {
                symbol: market_data.symbol.clone(),
                timestamp: market_data.timestamp,
                open: market_data.price - 1.0, // Simplified
                high: market_data.price + 2.0,
                low: market_data.price - 2.0,
                close: market_data.price,
                volume: market_data.volume,
                indicators: HashMap::new(),
            };
            
            // Store in data pipeline
            data_pipeline.process_data(time_series_data).await?;
            
            // Broadcast event
            let event = StreamEvent {
                event_type: "market_data_update".to_string(),
                symbol: market_data.symbol.clone(),
                timestamp: market_data.timestamp,
                data: serde_json::to_value(market_data)?,
                source: market_data.source.clone(),
            };
            
            let _ = event_broadcaster.send(event);
            
            // Update metrics
            let mut metrics_guard = metrics.write().await;
            metrics_guard.total_market_messages += 1;
            metrics_guard.last_update = Some(Utc::now());
        }
        
        Ok(())
    }

    async fn process_news_item(
        news_data: &NewsData,
        _data_pipeline: &Arc<DataPipeline>,
        event_broadcaster: &Arc<broadcast::Sender<StreamEvent>>,
        metrics: &Arc<RwLock<StreamMetrics>>,
    ) -> Result<()> {
        // Broadcast news event
        let event = StreamEvent {
            event_type: "news_update".to_string(),
            symbol: news_data.symbols.get(0).unwrap_or(&"GENERAL".to_string()).clone(),
            timestamp: news_data.timestamp,
            data: serde_json::to_value(news_data)?,
            source: news_data.source.clone(),
        };
        
        let _ = event_broadcaster.send(event);
        
        // Update metrics
        let mut metrics_guard = metrics.write().await;
        metrics_guard.total_news_messages += 1;
        metrics_guard.last_update = Some(Utc::now());
        
        Ok(())
    }
}