//! Streaming Data Connector for Real-time Market Data Integration
//!
//! This module provides real-time streaming data integration capabilities for the neural trader,
//! enabling continuous online learning from live market data feeds.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Instant};
use tracing::{debug, error, info, warn};
use rand;

use crate::data::TimeSeriesData;
use crate::neural::FannPredictor;

/// Configuration for streaming data connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    /// WebSocket endpoint for market data
    pub websocket_url: String,
    /// API key for authentication
    pub api_key: Option<String>,
    /// Symbols to subscribe to
    pub symbols: Vec<String>,
    /// Update interval in milliseconds
    pub update_interval_ms: u64,
    /// Buffer size for streaming data
    pub buffer_size: usize,
    /// Enable real-time processing
    pub real_time_processing: bool,
    /// Batch size for neural network updates
    pub batch_size: usize,
    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,
    /// Reconnection attempts
    pub max_reconnect_attempts: usize,
    /// Quality threshold for data validation
    pub quality_threshold: f64,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            websocket_url: "wss://stream.binance.com:9443/ws/btcusdt@ticker".to_string(),
            api_key: None,
            symbols: vec!["BTCUSD".to_string(), "ETHUSD".to_string()],
            update_interval_ms: 1000,
            buffer_size: 10000,
            real_time_processing: true,
            batch_size: 32,
            connection_timeout_secs: 30,
            max_reconnect_attempts: 5,
            quality_threshold: 0.95,
        }
    }
}

/// Market data message from streaming feed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataMessage {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub price: f64,
    pub volume: f64,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub high_24h: Option<f64>,
    pub low_24h: Option<f64>,
    pub change_24h: Option<f64>,
    pub source: String,
}

/// Data quality metrics for streaming validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQualityMetrics {
    pub completeness: f64,
    pub timeliness: f64,
    pub accuracy: f64,
    pub consistency: f64,
    pub overall_quality: f64,
    pub last_update: DateTime<Utc>,
}

/// Streaming data connector for real-time market data integration
pub struct StreamingConnector {
    config: StreamingConfig,
    predictor: Arc<FannPredictor>,
    data_buffer: Arc<RwLock<VecDeque<TimeSeriesData>>>,
    quality_metrics: Arc<RwLock<HashMap<String, DataQualityMetrics>>>,
    connection_status: Arc<RwLock<ConnectionStatus>>,
    message_sender: Option<mpsc::UnboundedSender<MarketDataMessage>>,
    is_running: Arc<RwLock<bool>>,
}

/// Connection status tracking
#[derive(Debug, Clone)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub last_message_time: DateTime<Utc>,
    pub messages_received: u64,
    pub connection_errors: u64,
    pub reconnection_attempts: usize,
    pub latency_ms: f64,
}

impl StreamingConnector {
    /// Create a new streaming connector
    pub fn new(config: StreamingConfig, predictor: Arc<FannPredictor>) -> Self {
        let connection_status = ConnectionStatus {
            connected: false,
            last_message_time: Utc::now(),
            messages_received: 0,
            connection_errors: 0,
            reconnection_attempts: 0,
            latency_ms: 0.0,
        };

        Self {
            config,
            predictor,
            data_buffer: Arc::new(RwLock::new(VecDeque::with_capacity(10000))),
            quality_metrics: Arc::new(RwLock::new(HashMap::new())),
            connection_status: Arc::new(RwLock::new(connection_status)),
            message_sender: None,
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the streaming data connection
    pub async fn start(&mut self) -> Result<()> {
        *self.is_running.write().await = true;
        
        info!("🚀 Starting streaming data connector for symbols: {:?}", self.config.symbols);

        // Create message channel
        let (tx, rx) = mpsc::unbounded_channel::<MarketDataMessage>();
        self.message_sender = Some(tx.clone());

        // Start connection monitor
        let connection_monitor = self.start_connection_monitor().await;
        
        // Start data processing task
        let data_processor = self.start_data_processor(rx).await;
        
        // Start quality monitoring
        let quality_monitor = self.start_quality_monitor().await;
        
        // Start mock data feed (simulating real market data)
        let data_feed = self.start_mock_data_feed(tx).await;

        // Wait for all tasks
        tokio::select! {
            _ = connection_monitor => warn!("Connection monitor stopped"),
            _ = data_processor => warn!("Data processor stopped"),
            _ = quality_monitor => warn!("Quality monitor stopped"),
            _ = data_feed => warn!("Data feed stopped"),
        }

        Ok(())
    }

    /// Stop the streaming connector
    pub async fn stop(&self) -> Result<()> {
        *self.is_running.write().await = false;
        info!("🛑 Stopping streaming data connector");
        Ok(())
    }

    /// Start connection monitoring task
    async fn start_connection_monitor(&self) -> tokio::task::JoinHandle<()> {
        let connection_status = Arc::clone(&self.connection_status);
        let is_running = Arc::clone(&self.is_running);
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));
            
            while *is_running.read().await {
                interval.tick().await;
                
                let mut status = connection_status.write().await;
                let time_since_last_message = Utc::now().timestamp() - status.last_message_time.timestamp();
                
                if time_since_last_message > 30 {
                    warn!("⚠️ No market data received for {} seconds", time_since_last_message);
                    status.connected = false;
                    status.connection_errors += 1;
                } else {
                    status.connected = true;
                }
                
                debug!(
                    "📊 Connection status: connected={}, messages={}, errors={}, latency={:.2}ms",
                    status.connected, status.messages_received, status.connection_errors, status.latency_ms
                );
            }
        })
    }

    /// Start data processing task
    async fn start_data_processor(&self, mut rx: mpsc::UnboundedReceiver<MarketDataMessage>) -> tokio::task::JoinHandle<()> {
        let predictor = Arc::clone(&self.predictor);
        let data_buffer = Arc::clone(&self.data_buffer);
        let quality_metrics = Arc::clone(&self.quality_metrics);
        let is_running = Arc::clone(&self.is_running);
        let batch_size = self.config.batch_size;
        
        tokio::spawn(async move {
            let mut batch_buffer = Vec::new();
            let mut last_processing = Instant::now();
            
            while *is_running.read().await {
                // Process incoming messages
                while let Ok(message) = rx.try_recv() {
                    // Convert to TimeSeriesData
                    if let Ok(time_series_data) = Self::convert_to_time_series(&message) {
                        // Add to buffer
                        {
                            let mut buffer = data_buffer.write().await;
                            buffer.push_back(time_series_data.clone());
                            
                            // Maintain buffer size
                            if buffer.len() > 10000 {
                                buffer.pop_front();
                            }
                        }
                        
                        // Add to batch
                        batch_buffer.push(time_series_data);
                        
                        // Update quality metrics
                        Self::update_quality_metrics(&quality_metrics, &message).await;
                    }
                }
                
                // Process batch if ready or timeout reached
                let should_process = batch_buffer.len() >= batch_size || 
                    last_processing.elapsed() > Duration::from_millis(5000);
                
                if should_process && !batch_buffer.is_empty() {
                    info!("🔄 Processing streaming batch: {} samples", batch_buffer.len());
                    
                    // Process each sample for online learning
                    for data in &batch_buffer {
                        if let Err(e) = predictor.process_streaming_data(data.clone()).await {
                            warn!("Failed to process streaming data: {}", e);
                        }
                    }
                    
                    batch_buffer.clear();
                    last_processing = Instant::now();
                }
                
                // Small delay to prevent busy loop
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
    }

    /// Start quality monitoring task
    async fn start_quality_monitor(&self) -> tokio::task::JoinHandle<()> {
        let quality_metrics = Arc::clone(&self.quality_metrics);
        let is_running = Arc::clone(&self.is_running);
        let quality_threshold = self.config.quality_threshold;
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            
            while *is_running.read().await {
                interval.tick().await;
                
                let metrics = quality_metrics.read().await;
                for (symbol, quality) in metrics.iter() {
                    if quality.overall_quality < quality_threshold {
                        warn!(
                            "⚠️ Data quality below threshold for {}: {:.3} < {:.3}",
                            symbol, quality.overall_quality, quality_threshold
                        );
                    } else {
                        debug!(
                            "✅ Data quality good for {}: {:.3}",
                            symbol, quality.overall_quality
                        );
                    }
                }
            }
        })
    }

    /// Start mock data feed (simulating real market data WebSocket)
    async fn start_mock_data_feed(&self, tx: mpsc::UnboundedSender<MarketDataMessage>) -> tokio::task::JoinHandle<()> {
        let symbols = self.config.symbols.clone();
        let update_interval = self.config.update_interval_ms;
        let is_running = Arc::clone(&self.is_running);
        let connection_status = Arc::clone(&self.connection_status);
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(update_interval));
            let mut prices = HashMap::new();
            
            // Initialize prices
            for symbol in &symbols {
                prices.insert(symbol.clone(), match symbol.as_str() {
                    "BTCUSD" => 50000.0,
                    "ETHUSD" => 3000.0,
                    _ => 100.0,
                });
            }
            
            while *is_running.read().await {
                interval.tick().await;
                
                for symbol in &symbols {
                    let current_price = prices.get(symbol).copied().unwrap_or(100.0);
                    
                    // Simulate price movement
                    let change_percent = (rand::random::<f64>() - 0.5) * 0.02; // ±1% change
                    let new_price = current_price * (1.0 + change_percent);
                    prices.insert(symbol.clone(), new_price);
                    
                    let message = MarketDataMessage {
                        symbol: symbol.clone(),
                        timestamp: Utc::now(),
                        price: new_price,
                        volume: 1000.0 + (rand::random::<f64>() * 9000.0),
                        bid: Some(new_price * 0.9995),
                        ask: Some(new_price * 1.0005),
                        high_24h: Some(new_price * 1.05),
                        low_24h: Some(new_price * 0.95),
                        change_24h: Some(change_percent),
                        source: "mock_feed".to_string(),
                    };
                    
                    if let Err(e) = tx.send(message) {
                        error!("Failed to send market data message: {}", e);
                        break;
                    }
                    
                    // Update connection status
                    {
                        let mut status = connection_status.write().await;
                        status.last_message_time = Utc::now();
                        status.messages_received += 1;
                        status.latency_ms = 10.0 + (rand::random::<f64>() * 20.0); // 10-30ms simulated latency
                    }
                }
            }
        })
    }

    /// Convert market data message to time series data
    fn convert_to_time_series(message: &MarketDataMessage) -> Result<TimeSeriesData> {
        let mut indicators = HashMap::new();
        
        // Calculate simple indicators
        indicators.insert("spread".to_string(), 
            message.ask.unwrap_or(message.price) - message.bid.unwrap_or(message.price));
        
        if let Some(change) = message.change_24h {
            indicators.insert("change_24h".to_string(), change);
        }
        
        // Simulate RSI
        indicators.insert("rsi".to_string(), 45.0 + (rand::random::<f64>() * 20.0));
        
        Ok(TimeSeriesData {
            symbol: message.symbol.clone(),
            timestamp: message.timestamp,
            open: message.price * 0.999,
            high: message.high_24h.unwrap_or(message.price * 1.001),
            low: message.low_24h.unwrap_or(message.price * 0.999),
            close: message.price,
            volume: message.volume,
            indicators,
            source: Some(message.source.clone()),
            entity: Some(message.symbol.clone()),
            value: Some(message.price),
            metadata: Some(serde_json::json!({
                "bid": message.bid,
                "ask": message.ask,
                "source": message.source
            })),
            // Required fields for vendor model integration
            values: vec![message.price],
            timestamps: vec![message.timestamp],
            metadata_map: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!(message.symbol));
                map.insert("source".to_string(), serde_json::json!(message.source));
                if let Some(bid) = message.bid {
                    map.insert("bid".to_string(), serde_json::json!(bid));
                }
                if let Some(ask) = message.ask {
                    map.insert("ask".to_string(), serde_json::json!(ask));
                }
                map
            },
        })
    }

    /// Update quality metrics for streaming data
    async fn update_quality_metrics(
        quality_metrics: &Arc<RwLock<HashMap<String, DataQualityMetrics>>>,
        message: &MarketDataMessage,
    ) {
        let mut metrics = quality_metrics.write().await;
        
        let current_metrics = metrics.entry(message.symbol.clone()).or_insert_with(|| {
            DataQualityMetrics {
                completeness: 1.0,
                timeliness: 1.0,
                accuracy: 1.0,
                consistency: 1.0,
                overall_quality: 1.0,
                last_update: Utc::now(),
            }
        });
        
        // Update timeliness based on message age
        let message_age_ms = (Utc::now().timestamp_millis() - message.timestamp.timestamp_millis()) as f64;
        let timeliness = (1.0 - (message_age_ms / 10000.0).min(1.0)).max(0.0);
        
        // Update completeness based on field availability
        let fields_present = [
            message.bid.is_some(),
            message.ask.is_some(),
            message.high_24h.is_some(),
            message.low_24h.is_some(),
            message.change_24h.is_some(),
        ].iter().filter(|&&x| x).count() as f64;
        let completeness = fields_present / 5.0;
        
        // Update accuracy (simplified - based on price reasonableness)
        let accuracy = if message.price > 0.0 && message.volume >= 0.0 { 1.0 } else { 0.0 };
        
        // Update consistency (simplified - based on bid/ask spread)
        let consistency = if let (Some(bid), Some(ask)) = (message.bid, message.ask) {
            if ask >= bid && (ask - bid) / message.price < 0.01 { 1.0 } else { 0.5 }
        } else { 0.8 };
        
        // Apply exponential smoothing
        let alpha = 0.1;
        current_metrics.completeness = current_metrics.completeness * (1.0 - alpha) + completeness * alpha;
        current_metrics.timeliness = current_metrics.timeliness * (1.0 - alpha) + timeliness * alpha;
        current_metrics.accuracy = current_metrics.accuracy * (1.0 - alpha) + accuracy * alpha;
        current_metrics.consistency = current_metrics.consistency * (1.0 - alpha) + consistency * alpha;
        
        // Calculate overall quality
        current_metrics.overall_quality = (
            current_metrics.completeness * 0.3 +
            current_metrics.timeliness * 0.3 +
            current_metrics.accuracy * 0.3 +
            current_metrics.consistency * 0.1
        );
        
        current_metrics.last_update = Utc::now();
    }

    /// Get current connection status
    pub async fn get_connection_status(&self) -> ConnectionStatus {
        self.connection_status.read().await.clone()
    }

    /// Get data quality metrics
    pub async fn get_quality_metrics(&self) -> HashMap<String, DataQualityMetrics> {
        self.quality_metrics.read().await.clone()
    }

    /// Get current buffer size
    pub async fn get_buffer_size(&self) -> usize {
        self.data_buffer.read().await.len()
    }

    /// Get recent data from buffer
    pub async fn get_recent_data(&self, limit: usize) -> Vec<TimeSeriesData> {
        let buffer = self.data_buffer.read().await;
        buffer.iter().rev().take(limit).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NeuralConfig;

    #[tokio::test]
    async fn test_streaming_connector_creation() {
        let config = StreamingConfig::default();
        
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 60,
            max_retries: 3,
            error_threshold: 0.1,
        };
        
        let predictor = Arc::new(FannPredictor::new(neural_config).unwrap());
        let connector = StreamingConnector::new(config, predictor);
        
        assert!(!*connector.is_running.read().await);
        assert_eq!(connector.get_buffer_size().await, 0);
    }

    #[tokio::test]
    async fn test_market_data_conversion() {
        let message = MarketDataMessage {
            symbol: "BTCUSD".to_string(),
            timestamp: Utc::now(),
            price: 50000.0,
            volume: 1000.0,
            bid: Some(49995.0),
            ask: Some(50005.0),
            high_24h: Some(51000.0),
            low_24h: Some(49000.0),
            change_24h: Some(0.02),
            source: "test".to_string(),
        };
        
        let time_series = StreamingConnector::convert_to_time_series(&message).unwrap();
        
        assert_eq!(time_series.symbol, "BTCUSD");
        assert_eq!(time_series.close, 50000.0);
        assert_eq!(time_series.volume, 1000.0);
        assert!(time_series.indicators.contains_key("spread"));
        assert!(time_series.indicators.contains_key("change_24h"));
    }
}