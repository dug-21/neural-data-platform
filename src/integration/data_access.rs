//! Data Access Layer for DAA Agent Integration
//!
//! This module provides a high-level interface for DAA agents to access
//! stored market data, enabling autonomous decision-making based on historical
//! and real-time data analysis.

use crate::data::{RedisCache, TimeSeriesData, TimescaleDBStorage};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info, warn};

/// Main data access layer for DAA agents
pub struct DataAccessLayer {
    pub storage: Arc<TimescaleDBStorage>,
    pub cache: Arc<RedisCache>,
    metrics: Arc<RwLock<AccessMetrics>>,
    active_subscriptions: Arc<RwLock<HashMap<String, AgentSubscription>>>,
    // Connection pooling for training workloads
    training_semaphore: Arc<Semaphore>,
    max_concurrent_training_queries: usize,
}

/// Data request from DAA agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRequest {
    pub agent_id: String,
    pub request_type: String,
    pub symbol: String,
    pub timeframe: Timeframe,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    pub metadata: HashMap<String, String>,
}

/// Response to agent data requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataResponse {
    pub agent_id: String,
    pub success: bool,
    pub data: Vec<TimeSeriesData>,
    pub metadata: HashMap<String, String>,
    pub error_message: Option<String>,
    pub response_time_ms: u64,
    pub data_source: String,
}

/// Timeframe for data requests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Timeframe {
    Minute,
    FiveMinute,
    FifteenMinute,
    Hourly,
    Daily,
    Weekly,
}

/// Training data request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingDataRequest {
    pub symbol: String,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub granularity: Timeframe,
    pub features: Vec<String>,
    pub include_indicators: bool,
}

/// Feature data configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    pub price_features: Vec<String>, // open, high, low, close, volume
    pub technical_indicators: Vec<String>, // sma, ema, rsi, macd, etc.
    pub lookback_window: usize,
    pub normalize: bool,
}

/// Price information for latest price lookups
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceInfo {
    pub price: f64,
    pub timestamp: DateTime<Utc>,
    pub volume: f64,
    pub source: String,
}

/// Map of symbol to price information
pub type PriceMap = HashMap<String, PriceInfo>;

/// Agent subscription for real-time data
#[derive(Debug, Clone)]
struct AgentSubscription {
    agent_id: String,
    symbol: String,
    timeframe: Timeframe,
    subscription_type: String,
    created_at: DateTime<Utc>,
    last_update: DateTime<Utc>,
}

/// Performance metrics for monitoring
#[derive(Debug, Clone, Default)]
struct AccessMetrics {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    cache_hits: u64,
    database_queries: u64,
    total_response_time_ms: u64,
    active_agents: HashMap<String, DateTime<Utc>>,
}

/// Performance metrics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_requests: u64,
    pub success_rate: f64,
    pub cache_hit_rate: f64,
    pub average_response_time_ms: f64,
    pub active_agent_count: usize,
    pub requests_per_second: f64,
}

impl DataAccessLayer {
    /// Create a new DataAccessLayer
    pub async fn new(storage: Arc<TimescaleDBStorage>, cache: Arc<RedisCache>) -> Result<Self> {
        let max_concurrent_training_queries = 10; // Configurable based on DB capacity
        Ok(Self {
            storage,
            cache,
            metrics: Arc::new(RwLock::new(AccessMetrics::default())),
            active_subscriptions: Arc::new(RwLock::new(HashMap::new())),
            training_semaphore: Arc::new(Semaphore::new(max_concurrent_training_queries)),
            max_concurrent_training_queries,
        })
    }

    /// Handle data request from DAA agent
    pub async fn handle_agent_data_request(&self, request: DataRequest) -> Result<DataResponse> {
        let start_time = std::time::Instant::now();
        debug!(
            "Handling data request from agent {}: {:?}",
            request.agent_id, request
        );

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_requests += 1;
            metrics
                .active_agents
                .insert(request.agent_id.clone(), Utc::now());
        }

        // Validate request
        if let Err(e) = self.validate_request(&request).await {
            warn!("Invalid request from agent {}: {}", request.agent_id, e);
            return Ok(self.create_error_response(&request, e.to_string(), start_time));
        }

        // Route request based on type
        let result = match request.request_type.as_str() {
            "historical_data" => self.handle_historical_data_request(&request).await,
            "latest_prices" => self.handle_latest_prices_request(&request).await,
            "aggregated_stats" => self.handle_aggregated_stats_request(&request).await,
            "subscribe_stream" => self.handle_subscription_request(&request).await,
            "unsubscribe_stream" => self.handle_unsubscription_request(&request).await,
            _ => {
                bail!("Unsupported request type: {}", request.request_type);
            }
        };

        let response = match result {
            Ok(mut response) => {
                response.response_time_ms = start_time.elapsed().as_millis() as u64;

                // Update success metrics
                let mut metrics = self.metrics.write().await;
                metrics.successful_requests += 1;
                if response.data_source == "cache" {
                    metrics.cache_hits += 1;
                } else {
                    metrics.database_queries += 1;
                }
                metrics.total_response_time_ms += response.response_time_ms;

                response
            }
            Err(e) => {
                error!(
                    "Failed to handle request from agent {}: {}",
                    request.agent_id, e
                );

                // Update failure metrics
                let mut metrics = self.metrics.write().await;
                metrics.failed_requests += 1;

                self.create_error_response(&request, e.to_string(), start_time)
            }
        };

        info!(
            "Completed request for agent {} in {}ms",
            request.agent_id, response.response_time_ms
        );
        Ok(response)
    }

    /// Get market data for agents
    pub async fn get_market_data(
        &self,
        symbol: &str,
        timeframe: Timeframe,
    ) -> Result<Vec<TimeSeriesData>> {
        let cache_key = format!("market_data:{}:{:?}", symbol, timeframe);

        // Try cache first
        if let Ok(Some(cached_data)) = self.cache.get::<Vec<TimeSeriesData>>(&cache_key).await {
            debug!("Cache hit for market data: {}", symbol);
            return Ok(cached_data);
        }

        // Query from database
        debug!("Cache miss, querying database for market data: {}", symbol);
        let end_time = Utc::now();
        let start_time = match timeframe {
            Timeframe::Minute => end_time - Duration::hours(1),
            Timeframe::FiveMinute => end_time - Duration::hours(4),
            Timeframe::FifteenMinute => end_time - Duration::hours(12),
            Timeframe::Hourly => end_time - Duration::days(1),
            Timeframe::Daily => end_time - Duration::days(30),
            Timeframe::Weekly => end_time - Duration::days(180),
        };

        let raw_data = self
            .storage
            .query_range(symbol, start_time, end_time)
            .await
            .context("Failed to query time series data")?;

        // Convert to TimeSeriesData format
        let mut time_series_data = Vec::new();
        for data_point in raw_data {
            if let Some(metadata) = &data_point.metadata {
                time_series_data.push(TimeSeriesData {
                    symbol: data_point.entity.clone(),
                    timestamp: data_point.timestamp,
                    open: metadata
                        .get("open")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(data_point.value),
                    high: metadata
                        .get("high")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(data_point.value),
                    low: metadata
                        .get("low")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(data_point.value),
                    close: data_point.value,
                    volume: metadata
                        .get("volume")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0),
                    indicators: metadata
                        .get("indicators")
                        .and_then(|v| v.as_object())
                        .map(|obj| {
                            obj.iter()
                                .filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f)))
                                .collect()
                        })
                        .unwrap_or_default(),
                    source: Some(data_point.source.clone()),
                    entity: Some(data_point.entity.clone()),
                    value: Some(data_point.value),
                    metadata: data_point.metadata.clone(),
                });
            }
        }

        // Cache the result
        let ttl = match timeframe {
            Timeframe::Minute => 60,         // 1 minute
            Timeframe::FiveMinute => 300,    // 5 minutes
            Timeframe::FifteenMinute => 900, // 15 minutes
            Timeframe::Hourly => 3600,       // 1 hour
            Timeframe::Daily => 21600,       // 6 hours
            Timeframe::Weekly => 86400,      // 24 hours
        };

        if let Err(e) = self
            .cache
            .set(&cache_key, &time_series_data, Some(ttl))
            .await
        {
            warn!("Failed to cache market data: {}", e);
        }

        Ok(time_series_data)
    }

    /// Get latest prices for multiple symbols
    pub async fn get_latest_prices(&self, symbols: Vec<String>) -> Result<PriceMap> {
        let mut price_map = PriceMap::new();

        for symbol in symbols {
            let cache_key = format!("data:{}:latest", symbol);

            if let Ok(Some(latest_data)) = self.cache.get::<TimeSeriesData>(&cache_key).await {
                price_map.insert(
                    symbol,
                    PriceInfo {
                        price: latest_data.close,
                        timestamp: latest_data.timestamp,
                        volume: latest_data.volume,
                        source: "cache".to_string(),
                    },
                );
            } else {
                // Query from database for latest entry
                let end_time = Utc::now();
                let start_time = end_time - Duration::minutes(10);

                if let Ok(data) = self
                    .storage
                    .query_range(&symbol, start_time, end_time)
                    .await
                {
                    if let Some(latest) = data.last() {
                        let price_info = PriceInfo {
                            price: latest.value,
                            timestamp: latest.timestamp,
                            volume: latest
                                .metadata
                                .as_ref()
                                .and_then(|m| m.get("volume"))
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.0),
                            source: "database".to_string(),
                        };
                        price_map.insert(symbol, price_info);
                    }
                }
            }
        }

        Ok(price_map)
    }

    /// Health check for the data access layer
    pub async fn health_check(&self) -> Result<bool> {
        // TODO: Implement proper health checks for storage and cache
        // For now, return true if we have valid references
        Ok(true)
    }

    /// Get performance metrics
    pub async fn get_performance_metrics(&self) -> Result<PerformanceMetrics> {
        let metrics = self.metrics.read().await;

        let success_rate = if metrics.total_requests > 0 {
            metrics.successful_requests as f64 / metrics.total_requests as f64
        } else {
            0.0
        };

        let cache_hit_rate = if metrics.cache_hits + metrics.database_queries > 0 {
            metrics.cache_hits as f64 / (metrics.cache_hits + metrics.database_queries) as f64
        } else {
            0.0
        };

        let average_response_time_ms = if metrics.successful_requests > 0 {
            metrics.total_response_time_ms as f64 / metrics.successful_requests as f64
        } else {
            0.0
        };

        // Clean up old agent activity (older than 5 minutes)
        let cutoff = Utc::now() - Duration::minutes(5);
        let active_agent_count = metrics
            .active_agents
            .values()
            .filter(|&&last_seen| last_seen > cutoff)
            .count();

        // Simple requests per second calculation based on recent activity
        let requests_per_second = if metrics.total_requests > 0 {
            metrics.total_requests as f64 / 60.0 // Rough estimate over 1 minute
        } else {
            0.0
        };

        Ok(PerformanceMetrics {
            total_requests: metrics.total_requests,
            success_rate,
            cache_hit_rate,
            average_response_time_ms,
            active_agent_count,
            requests_per_second,
        })
    }

    // Private helper methods

    async fn validate_request(&self, request: &DataRequest) -> Result<()> {
        if request.agent_id.is_empty() {
            bail!("Agent ID cannot be empty");
        }

        if request.symbol.is_empty() {
            bail!("Symbol cannot be empty");
        }

        if let (Some(start), Some(end)) = (request.start_time, request.end_time) {
            if start >= end {
                bail!("Start time must be before end time");
            }
        }

        Ok(())
    }

    async fn handle_historical_data_request(&self, request: &DataRequest) -> Result<DataResponse> {
        let data = self
            .get_market_data(&request.symbol, request.timeframe.clone())
            .await?;

        // Apply time range filtering if specified
        let filtered_data = if let (Some(start), Some(end)) = (request.start_time, request.end_time)
        {
            data.into_iter()
                .filter(|d| d.timestamp >= start && d.timestamp <= end)
                .collect()
        } else {
            data
        };

        // Apply limit if specified
        let final_data = if let Some(limit) = request.limit {
            filtered_data.into_iter().take(limit).collect()
        } else {
            filtered_data
        };

        Ok(DataResponse {
            agent_id: request.agent_id.clone(),
            success: true,
            data: final_data,
            metadata: HashMap::new(),
            error_message: None,
            response_time_ms: 0, // Will be set by caller
            data_source: "database".to_string(),
        })
    }

    async fn handle_latest_prices_request(&self, request: &DataRequest) -> Result<DataResponse> {
        let symbols = if request.symbol.contains(',') {
            request
                .symbol
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        } else {
            vec![request.symbol.clone()]
        };

        let price_map = self.get_latest_prices(symbols).await?;

        // Convert price map to TimeSeriesData for consistent response format
        let mut data = Vec::new();
        for (symbol, price_info) in price_map {
            data.push(TimeSeriesData {
                symbol: symbol.clone(),
                timestamp: price_info.timestamp,
                open: price_info.price,
                high: price_info.price,
                low: price_info.price,
                close: price_info.price,
                volume: price_info.volume,
                indicators: HashMap::new(),
                source: Some("price_feed".to_string()),
                entity: Some(symbol.clone()),
                value: Some(price_info.price),
                metadata: None,
            });
        }

        Ok(DataResponse {
            agent_id: request.agent_id.clone(),
            success: true,
            data,
            metadata: HashMap::new(),
            error_message: None,
            response_time_ms: 0,
            data_source: "mixed".to_string(),
        })
    }

    async fn handle_aggregated_stats_request(&self, request: &DataRequest) -> Result<DataResponse> {
        let start_time = request
            .start_time
            .unwrap_or_else(|| Utc::now() - Duration::hours(24));
        let end_time = request.end_time.unwrap_or_else(|| Utc::now());

        // Get aggregated statistics from storage
        let interval = match request.timeframe {
            Timeframe::Minute => "1 minute",
            Timeframe::FiveMinute => "5 minutes",
            Timeframe::FifteenMinute => "15 minutes",
            Timeframe::Hourly => "1 hour",
            Timeframe::Daily => "1 day",
            Timeframe::Weekly => "1 week",
        };

        let stats = self
            .storage
            .get_statistics(&request.symbol, start_time, end_time, interval)
            .await?;

        // Convert aggregated stats to TimeSeriesData format
        let mut data = Vec::new();
        for stat in stats {
            let mut indicators = HashMap::new();
            if let Some(avg) = stat.avg_value {
                indicators.insert("avg".to_string(), avg);
            }
            if let Some(min) = stat.min_value {
                indicators.insert("min".to_string(), min);
            }
            if let Some(max) = stat.max_value {
                indicators.insert("max".to_string(), max);
            }
            if let Some(stddev) = stat.stddev {
                indicators.insert("stddev".to_string(), stddev);
            }
            indicators.insert("count".to_string(), stat.count as f64);
            indicators.insert("volume".to_string(), 1000.0); // Default volume

            data.push(TimeSeriesData {
                symbol: stat.entity.clone(),
                timestamp: stat.bucket,
                open: stat.avg_value.unwrap_or(0.0),
                high: stat.max_value.unwrap_or(0.0),
                low: stat.min_value.unwrap_or(0.0),
                close: stat.avg_value.unwrap_or(0.0),
                volume: 1000.0, // Default volume for aggregated data
                indicators,
                source: Some("aggregated_stats".to_string()),
                entity: Some(stat.entity.clone()),
                value: Some(stat.avg_value.unwrap_or(0.0)),
                metadata: None,
            });
        }

        Ok(DataResponse {
            agent_id: request.agent_id.clone(),
            success: true,
            data,
            metadata: HashMap::new(),
            error_message: None,
            response_time_ms: 0,
            data_source: "database".to_string(),
        })
    }

    async fn handle_subscription_request(&self, request: &DataRequest) -> Result<DataResponse> {
        let subscription_id = format!(
            "{}:{}:{}",
            request.agent_id,
            request.symbol,
            Utc::now().timestamp()
        );

        let subscription = AgentSubscription {
            agent_id: request.agent_id.clone(),
            symbol: request.symbol.clone(),
            timeframe: request.timeframe.clone(),
            subscription_type: request
                .metadata
                .get("stream_type")
                .cloned()
                .unwrap_or_default(),
            created_at: Utc::now(),
            last_update: Utc::now(),
        };

        let mut subscriptions = self.active_subscriptions.write().await;
        subscriptions.insert(subscription_id.clone(), subscription);

        let mut metadata = HashMap::new();
        metadata.insert("subscription_id".to_string(), subscription_id);
        metadata.insert("status".to_string(), "active".to_string());

        Ok(DataResponse {
            agent_id: request.agent_id.clone(),
            success: true,
            data: Vec::new(),
            metadata,
            error_message: None,
            response_time_ms: 0,
            data_source: "subscription".to_string(),
        })
    }

    async fn handle_unsubscription_request(&self, request: &DataRequest) -> Result<DataResponse> {
        let subscription_id = request
            .metadata
            .get("subscription_id")
            .ok_or_else(|| anyhow::anyhow!("Missing subscription_id in metadata"))?;

        let mut subscriptions = self.active_subscriptions.write().await;
        let removed = subscriptions.remove(subscription_id).is_some();

        let mut metadata = HashMap::new();
        metadata.insert("subscription_id".to_string(), subscription_id.clone());
        metadata.insert(
            "status".to_string(),
            if removed { "removed" } else { "not_found" }.to_string(),
        );

        Ok(DataResponse {
            agent_id: request.agent_id.clone(),
            success: removed,
            data: Vec::new(),
            metadata,
            error_message: if !removed {
                Some("Subscription not found".to_string())
            } else {
                None
            },
            response_time_ms: 0,
            data_source: "subscription".to_string(),
        })
    }

    fn create_error_response(
        &self,
        request: &DataRequest,
        error: String,
        start_time: std::time::Instant,
    ) -> DataResponse {
        DataResponse {
            agent_id: request.agent_id.clone(),
            success: false,
            data: Vec::new(),
            metadata: HashMap::new(),
            error_message: Some(error),
            response_time_ms: start_time.elapsed().as_millis() as u64,
            data_source: "error".to_string(),
        }
    }

    // Training Data Methods

    /// Get training data for machine learning models
    /// Supports any stock symbol (AAPL, GOOGL, MSFT, etc.)
    pub async fn get_training_data(
        &self,
        symbol: &str,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        granularity: Timeframe,
    ) -> Result<Vec<TimeSeriesData>> {
        // Acquire permit from semaphore to control concurrent training queries
        let _permit = self.training_semaphore.acquire().await?;

        info!(
            "Fetching training data for {} from {} to {} with {:?} granularity",
            symbol, start_date, end_date, granularity
        );

        // Build cache key with granularity
        let cache_key = format!(
            "training:{}:{:?}:{}:{}",
            symbol,
            granularity,
            start_date.timestamp(),
            end_date.timestamp()
        );

        // Try cache first
        if let Ok(Some(cached_data)) = self.cache.get::<Vec<TimeSeriesData>>(&cache_key).await {
            debug!("Training data cache hit for {}", symbol);
            return Ok(cached_data);
        }

        // Query from database with proper batching
        let mut all_data = Vec::new();
        let mut current_start = start_date;

        // Batch size based on granularity to avoid overwhelming the database
        let batch_duration = match granularity {
            Timeframe::Minute => Duration::hours(24), // 1 day batches for minute data
            Timeframe::FiveMinute => Duration::days(7), // 1 week batches for 5-min data
            Timeframe::FifteenMinute => Duration::days(30), // 1 month batches for 15-min data
            Timeframe::Hourly => Duration::days(90),  // 3 month batches for hourly data
            Timeframe::Daily => Duration::days(365),  // 1 year batches for daily data
            Timeframe::Weekly => Duration::days(1825), // 5 year batches for weekly data
        };

        while current_start < end_date {
            let batch_end = std::cmp::min(current_start + batch_duration, end_date);

            debug!(
                "Fetching batch for {} from {} to {}",
                symbol, current_start, batch_end
            );

            let batch_data = self
                .storage
                .query_range(symbol, current_start, batch_end)
                .await
                .with_context(|| format!("Failed to query training data for {}", symbol))?;

            // Convert and aggregate based on granularity
            let converted_data = self.aggregate_by_timeframe(batch_data, &granularity)?;
            all_data.extend(converted_data);

            current_start = batch_end;
        }

        // Cache the result with longer TTL for training data
        let ttl = 3600 * 24; // 24 hours for training data
        if let Err(e) = self.cache.set(&cache_key, &all_data, Some(ttl)).await {
            warn!("Failed to cache training data: {}", e);
        }

        info!(
            "Retrieved {} training samples for {}",
            all_data.len(),
            symbol
        );
        Ok(all_data)
    }

    /// Get feature data for a specific symbol with lookback window
    /// Supports any stock symbol and custom feature sets
    pub async fn get_feature_data(
        &self,
        symbol: &str,
        lookback_window: usize,
        features: Vec<String>,
    ) -> Result<HashMap<String, Vec<f64>>> {
        let _permit = self.training_semaphore.acquire().await?;

        info!(
            "Fetching feature data for {} with {} lookback window",
            symbol, lookback_window
        );

        // Calculate time range based on lookback window
        let end_time = Utc::now();
        let start_time = end_time - Duration::days(lookback_window as i64);

        // Get raw data
        let raw_data = self
            .storage
            .query_range(symbol, start_time, end_time)
            .await?;

        // Convert to time series format
        let time_series = self.convert_to_time_series(raw_data)?;

        // Extract requested features
        let mut feature_map = HashMap::new();

        for feature in features {
            let values = match feature.as_str() {
                "open" => time_series.iter().map(|d| d.open).collect(),
                "high" => time_series.iter().map(|d| d.high).collect(),
                "low" => time_series.iter().map(|d| d.low).collect(),
                "close" => time_series.iter().map(|d| d.close).collect(),
                "volume" => time_series.iter().map(|d| d.volume).collect(),
                // Technical indicators from metadata
                indicator => time_series
                    .iter()
                    .map(|d| d.indicators.get(indicator).copied().unwrap_or(0.0))
                    .collect(),
            };

            feature_map.insert(feature, values);
        }

        Ok(feature_map)
    }

    /// Get the latest training window for real-time predictions
    /// This method is optimized for low latency to support real-time inference
    pub async fn get_latest_training_window(
        &self,
        symbol: &str,
        window_size: usize,
    ) -> Result<Vec<TimeSeriesData>> {
        let _permit = self.training_semaphore.acquire().await?;

        debug!("Fetching latest {} data points for {}", window_size, symbol);

        // Use shorter cache TTL for latest data
        let cache_key = format!("latest_window:{}:{}", symbol, window_size);

        if let Ok(Some(cached_data)) = self.cache.get::<Vec<TimeSeriesData>>(&cache_key).await {
            return Ok(cached_data);
        }

        // Query with limit for efficiency
        let end_time = Utc::now();
        let start_time = end_time - Duration::hours(24 * 7); // Look back 1 week max

        let raw_data = self
            .storage
            .query_range(symbol, start_time, end_time)
            .await?;
        let time_series = self.convert_to_time_series(raw_data)?;

        // Get the latest window_size entries
        let latest_window: Vec<TimeSeriesData> = time_series
            .into_iter()
            .rev()
            .take(window_size)
            .rev()
            .collect();

        // Cache with short TTL (1 minute) for real-time data
        if let Err(e) = self.cache.set(&cache_key, &latest_window, Some(60)).await {
            warn!("Failed to cache latest window: {}", e);
        }

        Ok(latest_window)
    }

    /// Get training data with technical indicators calculated
    pub async fn get_enriched_training_data(
        &self,
        request: TrainingDataRequest,
    ) -> Result<Vec<TimeSeriesData>> {
        let _permit = self.training_semaphore.acquire().await?;

        // Get base training data
        let mut data = self
            .get_training_data(
                &request.symbol,
                request.start_date,
                request.end_date,
                request.granularity,
            )
            .await?;

        // Enrich with indicators if requested
        if request.include_indicators {
            data = self.calculate_technical_indicators(data, &request.features)?;
        }

        Ok(data)
    }

    // Helper methods for training data pipeline

    /// Aggregate data based on timeframe
    fn aggregate_by_timeframe(
        &self,
        raw_data: Vec<crate::data::storage::TimeSeriesData>,
        _timeframe: &Timeframe,
    ) -> Result<Vec<TimeSeriesData>> {
        // Convert storage format to unified format first
        self.convert_to_time_series(raw_data)
    }

    /// Convert storage time series data to unified time series format  
    fn convert_to_time_series(
        &self,
        raw_data: Vec<crate::data::storage::TimeSeriesData>,
    ) -> Result<Vec<TimeSeriesData>> {
        let time_series = raw_data
            .into_iter()
            .map(|data_point| TimeSeriesData::from_storage_format(&data_point))
            .collect();

        Ok(time_series)
    }

    /// Calculate technical indicators for training data
    fn calculate_technical_indicators(
        &self,
        mut data: Vec<TimeSeriesData>,
        requested_indicators: &[String],
    ) -> Result<Vec<TimeSeriesData>> {
        // Placeholder for technical indicator calculations
        // In a real implementation, this would calculate SMA, EMA, RSI, MACD, etc.

        for indicator in requested_indicators {
            match indicator.as_str() {
                "sma_20" => {
                    // Simple Moving Average 20-period
                    let data_len = data.len();
                    for i in 19..data_len {
                        let sum: f64 = data[i - 19..=i].iter().map(|d| d.close).sum::<f64>();
                        data[i].indicators.insert("sma_20".to_string(), sum / 20.0);
                    }
                }
                "sma_50" => {
                    // Simple Moving Average 50-period
                    let data_len = data.len();
                    for i in 49..data_len {
                        let sum: f64 = data[i - 49..=i].iter().map(|d| d.close).sum::<f64>();
                        data[i].indicators.insert("sma_50".to_string(), sum / 50.0);
                    }
                }
                "volume_ratio" => {
                    // Volume ratio compared to 20-day average
                    let data_len = data.len();
                    for i in 19..data_len {
                        let avg_volume: f64 =
                            data[i - 19..=i].iter().map(|d| d.volume).sum::<f64>() / 20.0;
                        let current_volume = data[i].volume;
                        if avg_volume > 0.0 {
                            data[i]
                                .indicators
                                .insert("volume_ratio".to_string(), current_volume / avg_volume);
                        }
                    }
                }
                _ => {
                    debug!("Indicator {} not implemented, skipping", indicator);
                }
            }
        }

        Ok(data)
    }

    /// Get connection pool statistics for monitoring
    pub async fn get_training_pool_stats(&self) -> HashMap<String, f64> {
        let mut stats = HashMap::new();
        stats.insert(
            "max_connections".to_string(),
            self.max_concurrent_training_queries as f64,
        );
        stats.insert(
            "available_permits".to_string(),
            self.training_semaphore.available_permits() as f64,
        );
        stats
    }
}
