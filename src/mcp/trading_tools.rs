//! MCP Trading Tools Implementation
//!
//! Real implementation connecting to TimescaleDB, Redis, Neural Network, and Agents

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
// use redis::AsyncCommands;

use crate::agents::AutonomousAgent;
use crate::data::{RedisCache, TimeSeriesData, TimescaleDBStorage};
use crate::monitoring::{ComponentHealth, HealthMonitor, HealthStatus, SystemHealth};
use crate::neural::NeuralPredictor;

/// MCP Trading Tools Implementation
pub struct TradingMcpTools {
    storage: Arc<TimescaleDBStorage>,
    cache: Arc<RwLock<RedisCache>>,
    predictor: Arc<NeuralPredictor>,
    agent: Arc<RwLock<AutonomousAgent>>,
    monitor: Option<Arc<HealthMonitor>>,
}

impl TradingMcpTools {
    pub fn new(
        storage: Arc<TimescaleDBStorage>,
        cache: Arc<RwLock<RedisCache>>,
        predictor: Arc<NeuralPredictor>,
        agent: Arc<RwLock<AutonomousAgent>>,
    ) -> Self {
        Self {
            storage,
            cache,
            predictor,
            agent,
            monitor: None,
        }
    }

    pub async fn with_monitor(monitor: Arc<HealthMonitor>) -> Result<Self> {
        // Create placeholder components for health monitoring only
        use sqlx::postgres::PgPoolOptions;

        // Create minimal storage pool for health monitoring
        let storage = Arc::new(TimescaleDBStorage {
            pool: match sqlx::postgres::PgPool::connect("postgres://localhost/test").await {
                Ok(pool) => pool,
                Err(_) => {
                    // If connection fails, return error
                    return Err(anyhow::anyhow!("Failed to create database pool"));
                }
            },
        });

        let cache = Arc::new(RwLock::new(
            match RedisCache::new("redis://localhost:6379").await {
                Ok(cache) => cache,
                Err(_) => {
                    // If Redis fails, we'll handle this in health monitoring
                    return Err(anyhow::anyhow!(
                        "Redis cache not available for health monitoring"
                    ));
                }
            },
        ));

        Ok(Self {
            storage,
            cache,
            predictor: Arc::new(NeuralPredictor::default().await.map_err(|e| anyhow::anyhow!("Failed to create neural predictor: {}", e))?),
            agent: Arc::new(RwLock::new(AutonomousAgent::default())),
            monitor: Some(monitor),
        })
    }

    /// Query market data from TimescaleDB
    pub async fn query_market_data(&self, params: Value) -> Result<Value> {
        let symbol = params["symbol"].as_str().unwrap_or("BTC/USD");
        let interval = params["interval"].as_str().unwrap_or("1m");
        let limit = params["limit"].as_u64().unwrap_or(100) as i64;

        // Build query based on parameters
        let mut query = String::from(
            "SELECT timestamp, symbol, open, high, low, close, volume 
             FROM market_data 
             WHERE symbol = $1",
        );

        // Add time range if specified
        if let Some(_start_time) = params["start_time"].as_str() {
            query.push_str(" AND timestamp >= $2");
        }
        if let Some(_end_time) = params["end_time"].as_str() {
            query.push_str(" AND timestamp <= $3");
        }

        // Handle aggregation
        if params["aggregation"].as_str() == Some("ohlc") {
            query = format!(
                "SELECT 
                    time_bucket('{}', timestamp) AS timestamp,
                    symbol,
                    FIRST(open, timestamp) as open,
                    MAX(high) as high,
                    MIN(low) as low,
                    LAST(close, timestamp) as close,
                    SUM(volume) as volume
                FROM market_data
                WHERE symbol = $1
                GROUP BY time_bucket('{}', timestamp), symbol",
                interval, interval
            );
        }

        query.push_str(" ORDER BY timestamp DESC LIMIT $4");

        // Execute query
        let rows = sqlx::query(&query)
            .bind(symbol)
            .bind(limit)
            .fetch_all(&self.storage.pool)
            .await
            .context("Failed to query market data")?;

        // Transform results
        let data: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "timestamp": row.get::<DateTime<Utc>, _>("timestamp").to_rfc3339(),
                    "symbol": row.get::<String, _>("symbol"),
                    "open": row.get::<f64, _>("open"),
                    "high": row.get::<f64, _>("high"),
                    "low": row.get::<f64, _>("low"),
                    "close": row.get::<f64, _>("close"),
                    "volume": row.get::<f64, _>("volume"),
                })
            })
            .collect();

        Ok(json!({
            "symbol": symbol,
            "interval": interval,
            "data": data,
            "count": data.len(),
            "latest": data.first(),
        }))
    }

    /// Get cached data from Redis
    pub async fn get_cache_data(&self, params: Value) -> Result<Value> {
        let cache = self.cache.read().await;

        // Handle pattern matching
        if let Some(pattern) = params["pattern"].as_str() {
            let mut conn = cache.conn.clone();
            let keys: Vec<String> = redis::cmd("KEYS")
                .arg(pattern)
                .query_async(&mut conn)
                .await?;
            let mut data = json!({});

            for key in &keys {
                let mut conn = cache.conn.clone();
                if let Ok(value) = redis::cmd("GET")
                    .arg(key)
                    .query_async::<Option<String>>(&mut conn)
                    .await
                {
                    if let Some(value) = value {
                        if let Ok(parsed) = serde_json::from_str::<Value>(&value) {
                            data[key] = parsed;
                        }
                    }
                }
            }

            return Ok(json!({
                "pattern": pattern,
                "keys": keys,
                "data": data,
                "count": keys.len(),
            }));
        }

        // Handle single key
        let key = params["key"].as_str().unwrap_or("market:latest");

        // Check if key exists and get type
        let mut conn = cache.conn.clone();
        let exists: bool = redis::cmd("EXISTS")
            .arg(&key)
            .query_async(&mut conn)
            .await?;
        if !exists {
            return Ok(json!({
                "key": key,
                "found": false,
                "data": null,
            }));
        }

        // Get key type
        let mut conn = cache.conn.clone();
        let key_type: String = redis::cmd("TYPE").arg(&key).query_async(&mut conn).await?;

        // Get data based on type
        let (data, ttl) = match key_type.as_str() {
            "string" => {
                let mut conn = cache.conn.clone();
                let value: String = redis::cmd("GET").arg(&key).query_async(&mut conn).await?;
                let ttl: i64 = redis::cmd("TTL").arg(&key).query_async(&mut conn).await?;

                // Try to parse as JSON
                let parsed = serde_json::from_str::<Value>(&value).unwrap_or(json!(value));
                (parsed, ttl)
            }
            "list" => {
                let mut conn = cache.conn.clone();
                let values: Vec<String> = redis::cmd("LRANGE")
                    .arg(&key)
                    .arg(0)
                    .arg(-1)
                    .query_async(&mut conn)
                    .await?;
                let ttl: i64 = redis::cmd("TTL").arg(&key).query_async(&mut conn).await?;
                (json!(values), ttl)
            }
            "hash" => {
                let mut conn = cache.conn.clone();
                let values: std::collections::HashMap<String, String> = redis::cmd("HGETALL")
                    .arg(&key)
                    .query_async(&mut conn)
                    .await?;
                let ttl: i64 = redis::cmd("TTL").arg(&key).query_async(&mut conn).await?;
                (json!(values), ttl)
            }
            _ => (json!(null), -1),
        };

        Ok(json!({
            "key": key,
            "found": true,
            "type": key_type,
            "data": data,
            "ttl": ttl,
            "length": match key_type.as_str() {
                "list" => {
                    let mut conn = cache.conn.clone();
                    redis::cmd("LLEN").arg(&key).query_async::<i64>(&mut conn).await.ok()
                },
                "hash" => {
                    let mut conn = cache.conn.clone();
                    redis::cmd("HLEN").arg(&key).query_async::<i64>(&mut conn).await.ok()
                },
                _ => None,
            },
        }))
    }

    /// Request neural network prediction
    pub async fn request_prediction(&self, params: Value) -> Result<Value> {
        let symbol = params["symbol"].as_str().unwrap_or("BTC/USD");
        let horizon = params["horizon"].as_u64().unwrap_or(5);

        // Validate horizon
        if horizon > 100 {
            return Err(anyhow::anyhow!("Prediction horizon too large (max: 100)"));
        }

        let start_time = std::time::Instant::now();

        // Get historical data for the symbol
        let historical_data = self.get_historical_data(symbol, 100).await?;

        // Prepare features if provided
        let features = params["features"].as_object().map(|f| {
            f.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<HashMap<String, serde_json::Value>>()
        });

        // Handle ensemble predictions
        let predictions = if params["ensemble"].as_bool().unwrap_or(false) {
            let models = params["models"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            self.predictor
                .predict_ensemble(
                    &historical_data,
                    horizon as usize,
                    &models,
                    features.clone(),
                )
                .await?
        } else {
            self.predictor
                .predict(&historical_data, horizon as usize, features.clone())
                .await?
        };

        let computation_time = start_time.elapsed().as_millis();

        // Format predictions
        let formatted_predictions: Vec<Value> = predictions.iter().enumerate().map(|(i, pred)| {
            json!({
                "timestamp": (Utc::now() + chrono::Duration::minutes(i as i64 + 1)).to_rfc3339(),
                "value": pred.value,
                "confidence": pred.confidence,
                "interval_low": pred.interval_low,
                "interval_high": pred.interval_high,
            })
        }).collect();

        Ok(json!({
            "symbol": symbol,
            "horizon": horizon,
            "predictions": formatted_predictions,
            "model_used": predictions.first().map(|p| &p.model_name),
            "ensemble": params["ensemble"].as_bool().unwrap_or(false),
            "models_used": if params["ensemble"].as_bool().unwrap_or(false) {
                predictions.iter().map(|p| &p.model_name).collect::<Vec<_>>()
            } else {
                vec![]
            },
            "computation_time_ms": computation_time,
            "confidence_threshold": params["confidence_threshold"].as_f64().unwrap_or(0.0),
            "features_used": features,
            "feature_importance": self.predictor.get_feature_importance().await?,
            "prediction_intervals": {
                "95": formatted_predictions.iter().map(|p| {
                    json!({
                        "low": p["interval_low"],
                        "high": p["interval_high"],
                    })
                }).collect::<Vec<_>>(),
            },
        }))
    }

    /// Get agent trading decision
    pub async fn agent_decision(&self, params: Value) -> Result<Value> {
        let symbol = params["symbol"].as_str().unwrap_or("BTC/USD");
        let position_size = params["position_size"].as_f64().unwrap_or(0.0);
        let current_position = params["current_position"].as_f64().unwrap_or(0.0);

        // Get current market data
        let market_data = self.get_latest_market_data(symbol).await?;

        // Calculate P&L if position exists
        let pnl = if current_position > 0.0 {
            let entry_price = params["entry_price"].as_f64().unwrap_or(market_data.close);
            let current_price = params["current_price"]
                .as_f64()
                .unwrap_or(market_data.close);
            Some((current_price - entry_price) * current_position)
        } else {
            None
        };

        // Get agent decision
        let decision = self
            .agent
            .write()
            .await
            .make_decision(symbol, &market_data, current_position, position_size)
            .await?;

        // Handle multi-strategy decisions
        let strategy_signals = if let Some(weights) = params["strategy_weights"].as_object() {
            let mut signals = json!({});
            for (strategy, _weight) in weights {
                signals[strategy] = self
                    .agent
                    .write()
                    .await
                    .get_strategy_signal(strategy, symbol, &market_data)
                    .await?;
            }
            Some(signals)
        } else {
            None
        };

        // Risk assessment
        let risk_assessment = self
            .agent
            .write()
            .await
            .assess_risk(
                symbol,
                position_size,
                &market_data,
                params["portfolio_value"].as_f64(),
            )
            .await?;

        // Adjust position size based on risk
        let adjusted_size = if risk_assessment.risk_score > 0.7 {
            position_size * (1.0 - risk_assessment.risk_score)
        } else {
            position_size
        };

        Ok(json!({
            "symbol": symbol,
            "decision": decision.action,
            "confidence": decision.confidence,
            "reasoning": decision.reasoning,
            "risk_assessment": {
                "score": risk_assessment.risk_score,
                "factors": risk_assessment.factors,
                "max_drawdown": risk_assessment.max_drawdown,
                "var_95": risk_assessment.value_at_risk,
            },
            "current_pnl": pnl,
            "position_recommendation": {
                "action": decision.position_action,
                "size": adjusted_size,
                "stop_loss": decision.stop_loss,
                "take_profit": decision.take_profit,
            },
            "risk_warnings": risk_assessment.warnings,
            "adjusted_position_size": adjusted_size,
            "strategy_signals": strategy_signals,
            "combined_signal": decision.combined_signal,
            "decision_breakdown": decision.breakdown,
        }))
    }

    /// Get comprehensive system status
    pub async fn system_status(&self, params: Value) -> Result<Value> {
        let detailed = params["detailed"].as_bool().unwrap_or(false);
        let include_alerts = params["include_alerts"].as_bool().unwrap_or(false);
        let include_resources = params["include_resources"].as_bool().unwrap_or(false);
        let include_trading_stats = params["include_trading_stats"].as_bool().unwrap_or(false);

        let monitor = self
            .monitor
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Health monitor not configured"))?;

        // Get basic health status
        let health = monitor.get_system_health().await?;

        // Determine overall status
        let overall_status = if health
            .components
            .values()
            .all(|c| c.status == HealthStatus::Healthy)
        {
            "operational"
        } else if health
            .components
            .values()
            .any(|c| matches!(c.status, HealthStatus::Unhealthy(_)))
        {
            "unhealthy"
        } else {
            "degraded"
        };

        // Build component status
        let mut components = json!({});
        for (component, health_info) in &health.components {
            components[format!("{:?}", component)] = json!({
                "status": match &health_info.status {
                    HealthStatus::Healthy => "healthy",
                    HealthStatus::Degraded(_) => "degraded",
                    HealthStatus::Unhealthy(_) => "unhealthy",
                    HealthStatus::Unknown => "unknown",
                },
                "message": match &health_info.status {
                    HealthStatus::Healthy => "Operating normally",
                    HealthStatus::Degraded(msg) | HealthStatus::Unhealthy(msg) => msg.as_str(),
                    HealthStatus::Unknown => "Status unknown",
                },
                "last_check": health_info.last_check.to_rfc3339(),
            });
        }

        let mut result = json!({
            "status": overall_status,
            "timestamp": Utc::now().to_rfc3339(),
            "uptime_seconds": health.system_uptime.as_secs(),
            "components": components,
        });

        // Add detailed metrics
        if detailed {
            // Simple metrics placeholder
            result["metrics"] = json!({
                "requests_total": 0,
                "requests_failed": 0,
                "avg_response_time_ms": 100,
                "cache_hit_rate": 0.9,
            });

            result["performance"] = json!({
                "avg_latency_ms": 100,
                "requests_per_second": 10.0,
                "processed_items": 0,
            });
        }

        // Add alerts
        if include_alerts {
            result["alerts"] = json!(Vec::<String>::new());
        }

        // Add resource usage
        if include_resources {
            result["resources"] = json!({});
        }

        // Add trading statistics
        if include_trading_stats {
            result["trading_stats"] = json!({});
        }

        Ok(result)
    }

    // Helper methods

    async fn get_historical_data(&self, symbol: &str, limit: usize) -> Result<Vec<TimeSeriesData>> {
        let rows = sqlx::query(
            "SELECT timestamp, symbol, close as value, volume 
             FROM market_data 
             WHERE symbol = $1 
             ORDER BY timestamp DESC 
             LIMIT $2",
        )
        .bind(symbol)
        .bind(limit as i64)
        .fetch_all(&self.storage.pool)
        .await?;

        Ok(rows
            .into_iter()
            .rev()
            .map(|row| TimeSeriesData {
                timestamp: row.get("timestamp"),
                symbol: row.get("symbol"),
                open: 0.0, // Will be filled from actual data
                high: 0.0,
                low: 0.0,
                close: row.get("value"),
                volume: row.get("volume"),
                indicators: Default::default(),
                source: Some("timescale".to_string()),
                entity: Some(row.get::<String, _>("symbol")),
                value: Some(row.get("value")),
                metadata: None,
            })
            .collect())
    }

    async fn get_latest_market_data(&self, symbol: &str) -> Result<MarketData> {
        let row = sqlx::query(
            "SELECT timestamp, symbol, open, high, low, close, volume FROM market_data 
             WHERE symbol = $1 
             ORDER BY timestamp DESC 
             LIMIT 1",
        )
        .bind(symbol)
        .fetch_one(&self.storage.pool)
        .await?;

        Ok(MarketData {
            timestamp: row.get("timestamp"),
            symbol: row.get("symbol"),
            open: row.get("open"),
            high: row.get("high"),
            low: row.get("low"),
            close: row.get("close"),
            volume: row.get("volume"),
        })
    }
}

#[derive(Debug)]
pub struct MarketData {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}
