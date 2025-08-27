# Implementation Templates for V2 MVP Phases
## Development Blueprints and Code Templates

### Executive Summary

This document provides **concrete implementation templates** for each phase of the V2 MVP development. These templates serve as **development blueprints** that teams can follow to implement shared components and domain-specific features according to the architectural specifications.

---

## Template Organization

### Directory Structure Template
```
src/
├── shared/                     # Phase 1: Shared Infrastructure
│   ├── event_bus/             # Redis Streams implementation
│   ├── storage/               # TimescaleDB repositories
│   ├── configuration/         # Config management
│   ├── monitoring/            # Metrics and health checks
│   └── interfaces/            # Trait definitions
├── data/                      # Phase 2: Data Layer
│   ├── ingestion/             # Data ingestion service
│   ├── features/              # Feature engineering
│   └── neural/                # ML model implementation
├── services/                  # Phase 3: Service Layer
│   ├── action_layer/          # Trading execution
│   ├── risk_management/       # Risk controls
│   └── api/                   # REST/WebSocket APIs
└── integration/               # Phase 4: Integration
    ├── testing/               # Integration tests
    └── deployment/            # Deployment scripts
```

---

## Phase 1 Templates: Shared Infrastructure

### 1.1 Redis Streams Event Bus Template

#### Producer Implementation Template
```rust
// src/shared/event_bus/redis_producer.rs
use crate::shared::{
    interfaces::{EventBus, StreamMessage, MessageId, EventBusError},
    configuration::ConfigurationProvider,
    monitoring::MetricsProvider,
};
use deadpool_redis::{Config, Pool, Runtime};
use async_trait::async_trait;
use std::sync::Arc;

pub struct RedisStreamProducer {
    pool: Pool,
    metrics: Arc<dyn MetricsProvider>,
    config: ProducerConfig,
}

#[derive(Debug, Clone)]
pub struct ProducerConfig {
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
}

impl RedisStreamProducer {
    pub async fn new(
        redis_url: &str,
        metrics: Arc<dyn MetricsProvider>,
        config: ProducerConfig,
    ) -> Result<Self, EventBusError> {
        let cfg = Config::from_url(redis_url)?;
        let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
        
        // Test connection
        let conn = pool.get().await?;
        drop(conn);
        
        Ok(Self {
            pool,
            metrics,
            config,
        })
    }
}

#[async_trait]
impl EventBus for RedisStreamProducer {
    async fn publish(
        &self,
        stream: &str,
        message: StreamMessage,
    ) -> Result<MessageId, EventBusError> {
        let start = std::time::Instant::now();
        
        // TODO: Implement message serialization
        let fields = self.serialize_message(&message)?;
        
        // TODO: Implement Redis XADD command
        let mut conn = self.pool.get().await?;
        let id: String = conn.xadd(stream, "*", &fields).await?;
        
        // Record metrics
        self.metrics.record_timing(
            "stream_publish_latency_seconds",
            [
                ("stream".to_string(), stream.to_string()),
                ("priority".to_string(), format!("{:?}", message.metadata.priority)),
            ].into(),
            start.elapsed(),
        ).await?;
        
        self.metrics.increment_counter(
            "stream_messages_published_total",
            [
                ("stream".to_string(), stream.to_string()),
                ("source".to_string(), message.source.clone()),
            ].into(),
            1.0,
        ).await?;
        
        Ok(id)
    }
    
    // TODO: Implement other EventBus methods
    
    async fn health_check(&self) -> Result<HealthStatus, EventBusError> {
        match self.pool.get().await {
            Ok(mut conn) => {
                match conn.ping().await {
                    Ok(_) => Ok(HealthStatus::Healthy),
                    Err(_) => Ok(HealthStatus::Unhealthy),
                }
            }
            Err(_) => Ok(HealthStatus::Critical),
        }
    }
    
    // TODO: Implement remaining methods
}

impl RedisStreamProducer {
    fn serialize_message(&self, message: &StreamMessage) -> Result<Vec<(String, String)>, EventBusError> {
        // TODO: Implement message serialization
        // Convert StreamMessage to Redis field-value pairs
        let mut fields = vec![
            ("message_id".to_string(), message.message_id.to_string()),
            ("timestamp".to_string(), message.timestamp.to_rfc3339()),
            ("event_type".to_string(), message.event_type.clone()),
            ("source".to_string(), message.source.clone()),
        ];
        
        // Serialize payload
        let payload_json = serde_json::to_string(&message.payload)?;
        fields.push(("payload".to_string(), payload_json));
        
        // Serialize metadata
        let metadata_json = serde_json::to_string(&message.metadata)?;
        fields.push(("metadata".to_string(), metadata_json));
        
        Ok(fields)
    }
}

// TODO: Add error handling, retry logic, and batch processing
```

#### Consumer Implementation Template
```rust
// src/shared/event_bus/redis_consumer.rs
use crate::shared::interfaces::{MessageStream, ReceivedMessage, EventBusError};
use deadpool_redis::Pool;
use async_trait::async_trait;

pub struct RedisStreamConsumer {
    pool: Pool,
    stream: String,
    group: String,
    consumer: String,
    batch_size: usize,
}

impl RedisStreamConsumer {
    pub fn new(
        pool: Pool,
        stream: String,
        group: String,
        consumer: String,
        batch_size: usize,
    ) -> Self {
        Self {
            pool,
            stream,
            group,
            consumer,
            batch_size,
        }
    }
    
    async fn ensure_consumer_group(&self) -> Result<(), EventBusError> {
        // TODO: Implement consumer group creation
        let mut conn = self.pool.get().await?;
        
        // Create consumer group if it doesn't exist
        let result: Result<String, redis::RedisError> = conn
            .xgroup_create_mkstream(&self.stream, &self.group, "0")
            .await;
            
        match result {
            Ok(_) => Ok(()),
            Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()), // Group exists
            Err(e) => Err(EventBusError::Redis(e)),
        }
    }
}

#[async_trait]
impl MessageStream for RedisStreamConsumer {
    async fn next_batch(&mut self) -> Result<Vec<ReceivedMessage>, EventBusError> {
        // TODO: Implement XREADGROUP command
        self.ensure_consumer_group().await?;
        
        let mut conn = self.pool.get().await?;
        
        // Read messages from stream
        let opts = redis::streams::StreamReadOptions::default()
            .group(&self.group, &self.consumer)
            .count(self.batch_size)
            .block(1000); // 1 second timeout
            
        let reply: redis::streams::StreamReadReply = conn
            .xread_options(&[&self.stream], &[">"], &opts)
            .await?;
        
        let mut messages = Vec::new();
        
        for stream_data in reply.keys {
            for stream_id in stream_data.ids {
                // TODO: Deserialize message
                let message = self.deserialize_message(&stream_id.map)?;
                messages.push(ReceivedMessage {
                    id: stream_id.id,
                    stream: self.stream.clone(),
                    message,
                    received_at: chrono::Utc::now(),
                });
            }
        }
        
        Ok(messages)
    }
    
    // TODO: Implement other MessageStream methods
}

impl RedisStreamConsumer {
    fn deserialize_message(&self, fields: &std::collections::HashMap<String, String>) -> Result<StreamMessage, EventBusError> {
        // TODO: Implement message deserialization
        // Convert Redis fields back to StreamMessage
        todo!("Implement message deserialization")
    }
}
```

### 1.2 TimescaleDB Storage Template

#### Repository Implementation Template
```rust
// src/shared/storage/timescale_repository.rs
use crate::shared::interfaces::{
    Repository, TimeSeriesRepository, TimeSeriesKey, TimeSeriesEntry,
    SearchCriteria, StorageError,
};
use deadpool_postgres::{Pool, Client};
use async_trait::async_trait;
use serde::{Serialize, Deserialize};

pub struct TimescaleRepository<T> 
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    pool: Pool,
    table_name: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> TimescaleRepository<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync,
{
    pub fn new(pool: Pool, table_name: String) -> Self {
        Self {
            pool,
            table_name,
            _phantom: std::marker::PhantomData,
        }
    }
    
    async fn get_client(&self) -> Result<Client, StorageError> {
        self.pool.get().await.map_err(StorageError::ConnectionError)
    }
}

#[async_trait]
impl<T> Repository<T, TimeSeriesKey> for TimescaleRepository<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static,
{
    async fn insert(&self, entity: &T) -> Result<TimeSeriesKey, StorageError> {
        // TODO: Implement single insert
        let client = self.get_client().await?;
        
        let timestamp = chrono::Utc::now();
        let data_json = serde_json::to_value(entity)?;
        
        // TODO: Extract symbol from entity or use default
        let symbol = "DEFAULT".to_string(); // This should be extracted from entity
        
        let query = format!(
            "INSERT INTO {} (time, symbol, data) VALUES ($1, $2, $3) RETURNING time, symbol",
            self.table_name
        );
        
        let row = client.query_one(&query, &[&timestamp, &symbol, &data_json]).await?;
        
        Ok(TimeSeriesKey {
            timestamp: row.get("time"),
            symbol: row.get("symbol"),
            id: None,
        })
    }
    
    async fn insert_batch(&self, entities: &[T]) -> Result<Vec<TimeSeriesKey>, StorageError> {
        // TODO: Implement batch insert using COPY
        let client = self.get_client().await?;
        
        // Use COPY for high-performance batch inserts
        let copy_sql = format!(
            "COPY {} (time, symbol, data) FROM STDIN BINARY",
            self.table_name
        );
        
        let stmt = client.prepare(&copy_sql).await?;
        let sink = client.copy_in(&stmt).await?;
        
        // TODO: Stream entities to COPY sink
        // This requires implementing BinaryCopyInWriter usage
        
        todo!("Implement batch insert using COPY")
    }
    
    async fn find_by_id(&self, id: &TimeSeriesKey) -> Result<Option<T>, StorageError> {
        // TODO: Implement find by timestamp and symbol
        let client = self.get_client().await?;
        
        let query = format!(
            "SELECT data FROM {} WHERE time = $1 AND symbol = $2",
            self.table_name
        );
        
        match client.query_opt(&query, &[&id.timestamp, &id.symbol]).await? {
            Some(row) => {
                let data_json: serde_json::Value = row.get("data");
                let entity: T = serde_json::from_value(data_json)?;
                Ok(Some(entity))
            }
            None => Ok(None),
        }
    }
    
    // TODO: Implement other Repository methods
}

#[async_trait]
impl<T> TimeSeriesRepository<T> for TimescaleRepository<T>
where
    T: Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static,
{
    async fn query_time_range(
        &self,
        symbol: &str,
        start_time: chrono::DateTime<chrono::Utc>,
        end_time: chrono::DateTime<chrono::Utc>,
        limit: Option<u32>,
    ) -> Result<Vec<TimeSeriesEntry<T>>, StorageError> {
        // TODO: Implement time range query
        let client = self.get_client().await?;
        
        let mut query = format!(
            "SELECT time, symbol, data FROM {} WHERE symbol = $1 AND time >= $2 AND time <= $3 ORDER BY time DESC",
            self.table_name
        );
        
        if let Some(limit_value) = limit {
            query.push_str(&format!(" LIMIT {}", limit_value));
        }
        
        let rows = client.query(&query, &[&symbol, &start_time, &end_time]).await?;
        
        let mut entries = Vec::new();
        for row in rows {
            let data_json: serde_json::Value = row.get("data");
            let entity: T = serde_json::from_value(data_json)?;
            
            entries.push(TimeSeriesEntry {
                timestamp: row.get("time"),
                symbol: row.get("symbol"),
                data: entity,
                metadata: std::collections::HashMap::new(), // TODO: Add metadata support
            });
        }
        
        Ok(entries)
    }
    
    // TODO: Implement other TimeSeriesRepository methods
}
```

### 1.3 Configuration Management Template

#### Configuration Provider Template
```rust
// src/shared/configuration/provider.rs
use crate::shared::interfaces::{
    ConfigurationProvider, ConfigError, ValidationResult, ConfigWatcher,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;

pub struct HierarchicalConfigProvider {
    config_data: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    watchers: Arc<RwLock<HashMap<String, Vec<tokio::sync::mpsc::UnboundedSender<ConfigChange>>>>>,
}

impl HierarchicalConfigProvider {
    pub async fn new() -> Result<Self, ConfigError> {
        let mut config_data = HashMap::new();
        
        // TODO: Load configuration from multiple sources
        Self::load_base_config(&mut config_data)?;
        Self::load_environment_config(&mut config_data)?;
        Self::apply_environment_variables(&mut config_data)?;
        
        Ok(Self {
            config_data: Arc::new(RwLock::new(config_data)),
            watchers: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    fn load_base_config(config_data: &mut HashMap<String, serde_json::Value>) -> Result<(), ConfigError> {
        // TODO: Load base configuration from embedded config
        let base_config = include_str!("../../../config/base.toml");
        let parsed: toml::Value = toml::from_str(base_config)?;
        let json_value: serde_json::Value = serde_json::to_value(parsed)?;
        
        Self::flatten_config("", &json_value, config_data);
        Ok(())
    }
    
    fn load_environment_config(config_data: &mut HashMap<String, serde_json::Value>) -> Result<(), ConfigError> {
        // TODO: Load environment-specific configuration
        let env = std::env::var("NEURAL_TRADER_ENV").unwrap_or_else(|_| "development".to_string());
        
        let config_content = match env.as_str() {
            "production" => include_str!("../../../config/production.toml"),
            "staging" => include_str!("../../../config/staging.toml"),
            _ => include_str!("../../../config/development.toml"),
        };
        
        let parsed: toml::Value = toml::from_str(config_content)?;
        let json_value: serde_json::Value = serde_json::to_value(parsed)?;
        
        // Override base config with environment-specific values
        Self::flatten_config("", &json_value, config_data);
        Ok(())
    }
    
    fn apply_environment_variables(config_data: &mut HashMap<String, serde_json::Value>) -> Result<(), ConfigError> {
        // TODO: Override with environment variables
        // Map environment variables to config paths
        let env_mappings = [
            ("REDIS_URL", "redis.url"),
            ("DATABASE_URL", "database.url"),
            ("ENABLE_LIVE_TRADING", "features.enable_live_trading"),
            ("LOG_LEVEL", "logging.level"),
        ];
        
        for (env_var, config_path) in env_mappings {
            if let Ok(value) = std::env::var(env_var) {
                // TODO: Parse value based on expected type
                let json_value = match config_path {
                    path if path.contains("enable_") => {
                        serde_json::Value::Bool(value.parse().unwrap_or(false))
                    }
                    _ => serde_json::Value::String(value),
                };
                
                config_data.insert(config_path.to_string(), json_value);
            }
        }
        
        Ok(())
    }
    
    fn flatten_config(
        prefix: &str,
        value: &serde_json::Value,
        result: &mut HashMap<String, serde_json::Value>,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, val) in map {
                    let new_prefix = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", prefix, key)
                    };
                    Self::flatten_config(&new_prefix, val, result);
                }
            }
            _ => {
                result.insert(prefix.to_string(), value.clone());
            }
        }
    }
}

#[async_trait]
impl ConfigurationProvider for HierarchicalConfigProvider {
    async fn get<T>(&self, key_path: &str) -> Result<Option<T>, ConfigError>
    where
        T: for<'de> Deserialize<'de> + Send,
    {
        let config_data = self.config_data.read().await;
        
        match config_data.get(key_path) {
            Some(value) => {
                let parsed: T = serde_json::from_value(value.clone())?;
                Ok(Some(parsed))
            }
            None => Ok(None),
        }
    }
    
    async fn get_or_default<T>(&self, key_path: &str, default: T) -> Result<T, ConfigError>
    where
        T: for<'de> Deserialize<'de> + Send,
    {
        match self.get(key_path).await? {
            Some(value) => Ok(value),
            None => Ok(default),
        }
    }
    
    async fn validate(&self) -> Result<ValidationResult, ConfigError> {
        // TODO: Implement configuration validation
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        
        // Check required fields
        let required_fields = [
            "redis.url",
            "database.url",
        ];
        
        for field in required_fields {
            if self.get::<String>(field).await?.is_none() {
                errors.push(ConfigValidationError {
                    key_path: field.to_string(),
                    error_message: "Required field is missing".to_string(),
                    suggested_fix: Some("Set this field in configuration or environment variable".to_string()),
                });
            }
        }
        
        // Check for dangerous combinations
        let live_trading: bool = self.get_or_default("features.enable_live_trading", false).await?;
        let environment: String = self.get_or_default("environment", "development".to_string()).await?;
        
        if live_trading && environment == "development" {
            errors.push(ConfigValidationError {
                key_path: "features.enable_live_trading".to_string(),
                error_message: "Live trading not allowed in development environment".to_string(),
                suggested_fix: Some("Set environment to production or disable live trading".to_string()),
            });
        }
        
        Ok(ValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
        })
    }
    
    // TODO: Implement other ConfigurationProvider methods
}
```

---

## Phase 2 Templates: Data Layer Foundation

### 2.1 Feature Engineering Template

#### Technical Indicators Calculator
```rust
// src/data/features/technical_indicators.rs
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalIndicators {
    pub sma_20: f64,
    pub ema_12: f64,
    pub ema_26: f64,
    pub rsi_14: f64,
    pub macd: f64,
    pub macd_signal: f64,
    pub bollinger_upper: f64,
    pub bollinger_lower: f64,
    pub atr_14: f64,
    pub volume_sma_20: f64,
}

pub struct IndicatorCalculator {
    // Price data buffers
    prices: VecDeque<f64>,
    volumes: VecDeque<f64>,
    
    // EMA state
    ema_12_state: Option<f64>,
    ema_26_state: Option<f64>,
    macd_signal_state: Option<f64>,
    
    // RSI state
    rsi_gains: VecDeque<f64>,
    rsi_losses: VecDeque<f64>,
    
    // ATR state
    true_ranges: VecDeque<f64>,
    prev_close: Option<f64>,
}

impl IndicatorCalculator {
    pub fn new() -> Self {
        Self {
            prices: VecDeque::with_capacity(100),
            volumes: VecDeque::with_capacity(100),
            ema_12_state: None,
            ema_26_state: None,
            macd_signal_state: None,
            rsi_gains: VecDeque::with_capacity(14),
            rsi_losses: VecDeque::with_capacity(14),
            true_ranges: VecDeque::with_capacity(14),
            prev_close: None,
        }
    }
    
    pub fn add_price_data(&mut self, price: f64, volume: f64, high: f64, low: f64) {
        // TODO: Implement price data buffering
        self.prices.push_back(price);
        self.volumes.push_back(volume);
        
        // Keep only what we need
        if self.prices.len() > 100 {
            self.prices.pop_front();
        }
        if self.volumes.len() > 100 {
            self.volumes.pop_front();
        }
        
        // Update RSI state
        if let Some(prev_price) = self.prices.get(self.prices.len().saturating_sub(2)) {
            let change = price - prev_price;
            if change > 0.0 {
                self.rsi_gains.push_back(change);
                self.rsi_losses.push_back(0.0);
            } else {
                self.rsi_gains.push_back(0.0);
                self.rsi_losses.push_back(-change);
            }
            
            if self.rsi_gains.len() > 14 {
                self.rsi_gains.pop_front();
                self.rsi_losses.pop_front();
            }
        }
        
        // Update ATR state
        if let Some(prev_close) = self.prev_close {
            let tr1 = high - low;
            let tr2 = (high - prev_close).abs();
            let tr3 = (low - prev_close).abs();
            let true_range = tr1.max(tr2).max(tr3);
            
            self.true_ranges.push_back(true_range);
            if self.true_ranges.len() > 14 {
                self.true_ranges.pop_front();
            }
        }
        self.prev_close = Some(price);
    }
    
    pub fn calculate_indicators(&mut self) -> Option<TechnicalIndicators> {
        if self.prices.len() < 26 {
            return None; // Need enough data
        }
        
        // TODO: Implement all indicator calculations
        Some(TechnicalIndicators {
            sma_20: self.calculate_sma(20),
            ema_12: self.calculate_ema(12),
            ema_26: self.calculate_ema(26),
            rsi_14: self.calculate_rsi(),
            macd: self.calculate_macd(),
            macd_signal: self.calculate_macd_signal(),
            bollinger_upper: self.calculate_bollinger_upper(),
            bollinger_lower: self.calculate_bollinger_lower(),
            atr_14: self.calculate_atr(),
            volume_sma_20: self.calculate_volume_sma(20),
        })
    }
    
    fn calculate_sma(&self, period: usize) -> f64 {
        if self.prices.len() < period {
            return 0.0;
        }
        
        let sum: f64 = self.prices.iter().rev().take(period).sum();
        sum / period as f64
    }
    
    fn calculate_ema(&mut self, period: usize) -> f64 {
        if self.prices.is_empty() {
            return 0.0;
        }
        
        let current_price = *self.prices.back().unwrap();
        let multiplier = 2.0 / (period as f64 + 1.0);
        
        match period {
            12 => {
                if let Some(prev_ema) = self.ema_12_state {
                    let ema = (current_price * multiplier) + (prev_ema * (1.0 - multiplier));
                    self.ema_12_state = Some(ema);
                    ema
                } else {
                    // Initialize with SMA
                    let ema = self.calculate_sma(12);
                    self.ema_12_state = Some(ema);
                    ema
                }
            }
            26 => {
                if let Some(prev_ema) = self.ema_26_state {
                    let ema = (current_price * multiplier) + (prev_ema * (1.0 - multiplier));
                    self.ema_26_state = Some(ema);
                    ema
                } else {
                    let ema = self.calculate_sma(26);
                    self.ema_26_state = Some(ema);
                    ema
                }
            }
            _ => 0.0, // TODO: Support other periods
        }
    }
    
    fn calculate_rsi(&self) -> f64 {
        if self.rsi_gains.len() < 14 {
            return 50.0; // Neutral RSI
        }
        
        let avg_gain: f64 = self.rsi_gains.iter().sum::<f64>() / 14.0;
        let avg_loss: f64 = self.rsi_losses.iter().sum::<f64>() / 14.0;
        
        if avg_loss == 0.0 {
            return 100.0;
        }
        
        let rs = avg_gain / avg_loss;
        100.0 - (100.0 / (1.0 + rs))
    }
    
    fn calculate_macd(&self) -> f64 {
        // MACD = EMA(12) - EMA(26)
        let ema_12 = self.ema_12_state.unwrap_or(0.0);
        let ema_26 = self.ema_26_state.unwrap_or(0.0);
        ema_12 - ema_26
    }
    
    fn calculate_macd_signal(&mut self) -> f64 {
        let macd = self.calculate_macd();
        let multiplier = 2.0 / 10.0; // 9-period EMA
        
        if let Some(prev_signal) = self.macd_signal_state {
            let signal = (macd * multiplier) + (prev_signal * (1.0 - multiplier));
            self.macd_signal_state = Some(signal);
            signal
        } else {
            self.macd_signal_state = Some(macd);
            macd
        }
    }
    
    // TODO: Implement remaining indicator calculations
    fn calculate_bollinger_upper(&self) -> f64 { 0.0 }
    fn calculate_bollinger_lower(&self) -> f64 { 0.0 }
    fn calculate_atr(&self) -> f64 { 0.0 }
    fn calculate_volume_sma(&self, _period: usize) -> f64 { 0.0 }
}

/// Feature extraction service
pub struct FeatureExtractor {
    calculators: std::collections::HashMap<String, IndicatorCalculator>,
}

impl FeatureExtractor {
    pub fn new() -> Self {
        Self {
            calculators: std::collections::HashMap::new(),
        }
    }
    
    pub fn process_market_data(
        &mut self,
        symbol: &str,
        price: f64,
        volume: f64,
        high: f64,
        low: f64,
    ) -> Option<TechnicalIndicators> {
        let calculator = self.calculators
            .entry(symbol.to_string())
            .or_insert_with(IndicatorCalculator::new);
            
        calculator.add_price_data(price, volume, high, low);
        calculator.calculate_indicators()
    }
}
```

### 2.2 Neural Model Template

#### Single MLP Predictor Template
```rust
// src/data/neural/mlp_predictor.rs
use crate::data::features::TechnicalIndicators;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLPConfig {
    pub input_size: usize,
    pub hidden_layers: Vec<usize>,
    pub output_size: usize,
    pub learning_rate: f64,
    pub dropout_rate: f64,
}

impl Default for MLPConfig {
    fn default() -> Self {
        Self {
            input_size: 20,       // Number of technical indicators
            hidden_layers: vec![64, 32], // Two hidden layers
            output_size: 1,       // Single output (price direction)
            learning_rate: 0.001,
            dropout_rate: 0.2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    pub symbol: String,
    pub prediction: f64,
    pub confidence: Option<f64>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub model_version: String,
}

pub struct MLPPredictor {
    config: MLPConfig,
    model: Option<Box<dyn NeuralModel>>,
    model_path: String,
}

// Abstract neural model interface
trait NeuralModel: Send + Sync {
    fn predict(&self, features: &[f64]) -> Result<f64, PredictionError>;
    fn train(&mut self, features: &[Vec<f64>], targets: &[f64]) -> Result<(), PredictionError>;
    fn save(&self, path: &str) -> Result<(), PredictionError>;
    fn load(&mut self, path: &str) -> Result<(), PredictionError>;
}

impl MLPPredictor {
    pub fn new(config: MLPConfig, model_path: String) -> Self {
        Self {
            config,
            model: None,
            model_path,
        }
    }
    
    pub async fn initialize(&mut self) -> Result<(), PredictionError> {
        // TODO: Load or create neural model
        // For MVP, we'll use a simple placeholder implementation
        
        if std::path::Path::new(&self.model_path).exists() {
            // Load existing model
            self.load_model().await?;
        } else {
            // Create new model
            self.create_model().await?;
        }
        
        Ok(())
    }
    
    pub async fn predict(&self, indicators: &TechnicalIndicators) -> Result<Prediction, PredictionError> {
        let model = self.model.as_ref()
            .ok_or_else(|| PredictionError::ModelNotInitialized)?;
        
        // Convert indicators to feature vector
        let features = self.indicators_to_features(indicators);
        
        // Generate prediction
        let prediction = model.predict(&features)?;
        
        // TODO: Calculate confidence score
        let confidence = self.calculate_confidence(&features, prediction);
        
        Ok(Prediction {
            symbol: "UNKNOWN".to_string(), // TODO: Extract from context
            prediction,
            confidence: Some(confidence),
            timestamp: chrono::Utc::now(),
            model_version: "mlp-v1.0".to_string(),
        })
    }
    
    pub async fn train(
        &mut self,
        training_data: &[(TechnicalIndicators, f64)],
    ) -> Result<TrainingResult, PredictionError> {
        // TODO: Implement training logic
        let model = self.model.as_mut()
            .ok_or_else(|| PredictionError::ModelNotInitialized)?;
        
        // Convert training data to feature matrices
        let (features, targets): (Vec<Vec<f64>>, Vec<f64>) = training_data
            .iter()
            .map(|(indicators, target)| (self.indicators_to_features(indicators), *target))
            .unzip();
        
        // Train model
        model.train(&features, &targets)?;
        
        // Save trained model
        model.save(&self.model_path)?;
        
        Ok(TrainingResult {
            epochs_trained: 100, // TODO: Track actual epochs
            final_loss: 0.01,   // TODO: Track actual loss
            accuracy: 0.85,     // TODO: Calculate actual accuracy
        })
    }
    
    fn indicators_to_features(&self, indicators: &TechnicalIndicators) -> Vec<f64> {
        // TODO: Normalize features for neural network input
        vec![
            indicators.sma_20,
            indicators.ema_12,
            indicators.ema_26,
            indicators.rsi_14 / 100.0,  // Normalize RSI to 0-1
            indicators.macd,
            indicators.macd_signal,
            indicators.bollinger_upper,
            indicators.bollinger_lower,
            indicators.atr_14,
            indicators.volume_sma_20,
            // TODO: Add more features and proper normalization
        ]
    }
    
    fn calculate_confidence(&self, _features: &[f64], _prediction: f64) -> f64 {
        // TODO: Implement confidence calculation
        // Could use prediction variance, model uncertainty, etc.
        0.75 // Placeholder
    }
    
    async fn load_model(&mut self) -> Result<(), PredictionError> {
        // TODO: Implement model loading
        // For MVP, create a simple placeholder model
        self.model = Some(Box::new(PlaceholderModel::new()));
        Ok(())
    }
    
    async fn create_model(&mut self) -> Result<(), PredictionError> {
        // TODO: Create new model with config
        self.model = Some(Box::new(PlaceholderModel::new()));
        Ok(())
    }
}

// Placeholder model implementation for MVP
struct PlaceholderModel {
    weights: Vec<f64>,
}

impl PlaceholderModel {
    fn new() -> Self {
        Self {
            weights: vec![0.0; 20], // Initialize with zeros
        }
    }
}

impl NeuralModel for PlaceholderModel {
    fn predict(&self, features: &[f64]) -> Result<f64, PredictionError> {
        // Simple linear combination for MVP
        if features.len() != self.weights.len() {
            return Err(PredictionError::InvalidInput("Feature size mismatch".to_string()));
        }
        
        let prediction: f64 = features.iter()
            .zip(self.weights.iter())
            .map(|(f, w)| f * w)
            .sum();
            
        // Apply sigmoid to get probability
        Ok(1.0 / (1.0 + (-prediction).exp()))
    }
    
    fn train(&mut self, features: &[Vec<f64>], targets: &[f64]) -> Result<(), PredictionError> {
        // TODO: Implement simple gradient descent
        // For MVP, just update weights randomly
        for weight in &mut self.weights {
            *weight += (rand::random::<f64>() - 0.5) * 0.01;
        }
        Ok(())
    }
    
    fn save(&self, path: &str) -> Result<(), PredictionError> {
        // TODO: Implement model serialization
        let serialized = serde_json::to_string(&self.weights)?;
        std::fs::write(path, serialized)?;
        Ok(())
    }
    
    fn load(&mut self, path: &str) -> Result<(), PredictionError> {
        // TODO: Implement model deserialization
        let content = std::fs::read_to_string(path)?;
        self.weights = serde_json::from_str(&content)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingResult {
    pub epochs_trained: u32,
    pub final_loss: f64,
    pub accuracy: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum PredictionError {
    #[error("Model not initialized")]
    ModelNotInitialized,
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Training error: {0}")]
    TrainingError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
```

---

## Phase 3 Templates: Service Layer

### 3.1 Action Layer Template

#### Trading Decision Engine
```rust
// src/services/action_layer/decision_engine.rs
use crate::{
    data::neural::Prediction,
    shared::interfaces::{EventBus, StreamMessage},
    services::risk_management::RiskValidator,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingDecision {
    pub decision_id: uuid::Uuid,
    pub symbol: String,
    pub action: TradingAction,
    pub quantity: u32,
    pub price: Option<rust_decimal::Decimal>,
    pub reasoning: String,
    pub confidence: f64,
    pub risk_score: f64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradingAction {
    Buy,
    Sell,
    Hold,
}

pub struct DecisionEngine {
    event_bus: Box<dyn EventBus>,
    risk_validator: Box<dyn RiskValidator>,
    config: DecisionConfig,
}

#[derive(Debug, Clone)]
pub struct DecisionConfig {
    pub confidence_threshold: f64,
    pub max_position_size: u32,
    pub enable_live_trading: bool,
}

impl DecisionEngine {
    pub fn new(
        event_bus: Box<dyn EventBus>,
        risk_validator: Box<dyn RiskValidator>,
        config: DecisionConfig,
    ) -> Self {
        Self {
            event_bus,
            risk_validator,
            config,
        }
    }
    
    pub async fn process_prediction(&self, prediction: Prediction) -> Result<Option<TradingDecision>, DecisionError> {
        // TODO: Implement decision logic based on prediction
        
        // Skip if confidence too low
        if let Some(confidence) = prediction.confidence {
            if confidence < self.config.confidence_threshold {
                return Ok(None);
            }
        }
        
        // Determine action based on prediction
        let action = self.prediction_to_action(&prediction)?;
        
        if matches!(action, TradingAction::Hold) {
            return Ok(None);
        }
        
        // Calculate position size
        let quantity = self.calculate_position_size(&prediction)?;
        
        // Create decision
        let decision = TradingDecision {
            decision_id: uuid::Uuid::new_v4(),
            symbol: prediction.symbol.clone(),
            action,
            quantity,
            price: None, // Market order
            reasoning: format!("Neural prediction: {:.4}", prediction.prediction),
            confidence: prediction.confidence.unwrap_or(0.5),
            risk_score: 0.0, // Will be calculated by risk validator
            timestamp: chrono::Utc::now(),
        };
        
        // Validate with risk manager
        let validated_decision = self.risk_validator.validate_decision(decision).await?;
        
        // Publish decision
        if let Some(ref final_decision) = validated_decision {
            self.publish_decision(final_decision).await?;
        }
        
        Ok(validated_decision)
    }
    
    fn prediction_to_action(&self, prediction: &Prediction) -> Result<TradingAction, DecisionError> {
        // TODO: Implement prediction interpretation logic
        // Simple threshold-based approach for MVP
        
        match prediction.prediction {
            p if p > 0.6 => Ok(TradingAction::Buy),
            p if p < 0.4 => Ok(TradingAction::Sell),
            _ => Ok(TradingAction::Hold),
        }
    }
    
    fn calculate_position_size(&self, prediction: &Prediction) -> Result<u32, DecisionError> {
        // TODO: Implement position sizing logic
        // Simple fixed-size approach for MVP
        
        let base_size = 100u32; // Base position size
        let confidence_multiplier = prediction.confidence.unwrap_or(0.5);
        
        let size = (base_size as f64 * confidence_multiplier) as u32;
        Ok(size.min(self.config.max_position_size))
    }
    
    async fn publish_decision(&self, decision: &TradingDecision) -> Result<(), DecisionError> {
        let message = StreamMessage {
            message_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            stream: "trading.decisions".to_string(),
            event_type: "trading_decision".to_string(),
            source: "decision_engine".to_string(),
            payload: serde_json::to_value(decision)?,
            metadata: crate::shared::interfaces::MessageMetadata::default(),
        };
        
        self.event_bus.publish("trading.decisions", message).await?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DecisionError {
    #[error("Risk validation failed: {0}")]
    RiskValidation(String),
    
    #[error("Event bus error: {0}")]
    EventBus(#[from] crate::shared::interfaces::EventBusError),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Invalid prediction: {0}")]
    InvalidPrediction(String),
}
```

### 3.2 Risk Management Template

#### Risk Validator Implementation
```rust
// src/services/risk_management/validator.rs
use crate::services::action_layer::{TradingDecision, TradingAction};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait RiskValidator: Send + Sync {
    async fn validate_decision(&self, decision: TradingDecision) -> Result<Option<TradingDecision>, RiskError>;
    async fn check_position_limits(&self, symbol: &str, quantity: u32) -> Result<bool, RiskError>;
    async fn check_daily_loss_limit(&self) -> Result<bool, RiskError>;
    async fn emergency_stop(&self) -> Result<(), RiskError>;
}

pub struct BasicRiskValidator {
    config: RiskConfig,
    position_tracker: Box<dyn PositionTracker>,
    performance_tracker: Box<dyn PerformanceTracker>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub max_position_size_percent: f64,    // 5% max per position
    pub daily_loss_limit_percent: f64,     // 10% daily loss limit
    pub stop_loss_percent: f64,            // 5% stop loss per trade
    pub max_total_exposure_percent: f64,   // 80% max total exposure
    pub enable_emergency_stop: bool,
    pub paper_trading_mode: bool,
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_position_size_percent: 0.05,  // 5%
            daily_loss_limit_percent: 0.10,   // 10%
            stop_loss_percent: 0.05,          // 5%
            max_total_exposure_percent: 0.80, // 80%
            enable_emergency_stop: true,
            paper_trading_mode: true,         // Safe default
        }
    }
}

impl BasicRiskValidator {
    pub fn new(
        config: RiskConfig,
        position_tracker: Box<dyn PositionTracker>,
        performance_tracker: Box<dyn PerformanceTracker>,
    ) -> Self {
        Self {
            config,
            position_tracker,
            performance_tracker,
        }
    }
}

#[async_trait]
impl RiskValidator for BasicRiskValidator {
    async fn validate_decision(&self, mut decision: TradingDecision) -> Result<Option<TradingDecision>, RiskError> {
        // TODO: Implement comprehensive risk validation
        
        // Check if trading is allowed
        if !self.is_trading_allowed().await? {
            return Ok(None);
        }
        
        // Check position size limits
        if !self.check_position_limits(&decision.symbol, decision.quantity).await? {
            return Ok(None);
        }
        
        // Check daily loss limits
        if !self.check_daily_loss_limit().await? {
            return Ok(None);
        }
        
        // Check market hours
        if !self.is_market_open().await? {
            return Ok(None);
        }
        
        // Calculate risk score
        decision.risk_score = self.calculate_risk_score(&decision).await?;
        
        // Apply position sizing adjustments
        decision.quantity = self.adjust_position_size(decision.quantity, decision.risk_score)?;
        
        Ok(Some(decision))
    }
    
    async fn check_position_limits(&self, symbol: &str, quantity: u32) -> Result<bool, RiskError> {
        // TODO: Check against current positions
        let current_position = self.position_tracker.get_position(symbol).await?;
        let portfolio_value = self.position_tracker.get_portfolio_value().await?;
        
        // Calculate position value (simplified)
        let estimated_value = quantity as f64 * 100.0; // Assume $100 per share
        let position_percent = estimated_value / portfolio_value;
        
        // Check if within limits
        Ok(position_percent <= self.config.max_position_size_percent)
    }
    
    async fn check_daily_loss_limit(&self) -> Result<bool, RiskError> {
        let daily_pnl = self.performance_tracker.get_daily_pnl().await?;
        let portfolio_value = self.position_tracker.get_portfolio_value().await?;
        
        let loss_percent = -daily_pnl / portfolio_value;
        Ok(loss_percent <= self.config.daily_loss_limit_percent)
    }
    
    async fn emergency_stop(&self) -> Result<(), RiskError> {
        // TODO: Implement emergency stop logic
        // Close all positions, halt trading, send alerts
        if self.config.enable_emergency_stop {
            // Implementation would close all positions
            tracing::warn!("Emergency stop activated");
        }
        Ok(())
    }
}

impl BasicRiskValidator {
    async fn is_trading_allowed(&self) -> Result<bool, RiskError> {
        // TODO: Check various conditions that might halt trading
        // - Emergency stop active
        // - System health issues
        // - External halt signals
        Ok(true)
    }
    
    async fn is_market_open(&self) -> Result<bool, RiskError> {
        // TODO: Implement market hours checking
        // For MVP, assume market is always open
        Ok(true)
    }
    
    async fn calculate_risk_score(&self, decision: &TradingDecision) -> Result<f64, RiskError> {
        // TODO: Implement risk scoring algorithm
        // Consider factors like:
        // - Volatility
        // - Correlation
        // - Position concentration
        // - Market conditions
        
        // Simple scoring for MVP
        let base_risk = match decision.action {
            TradingAction::Buy | TradingAction::Sell => 0.5,
            TradingAction::Hold => 0.0,
        };
        
        // Adjust for confidence
        let confidence_adjustment = 1.0 - decision.confidence;
        
        Ok(base_risk + confidence_adjustment * 0.3)
    }
    
    fn adjust_position_size(&self, quantity: u32, risk_score: f64) -> Result<u32, RiskError> {
        // TODO: Adjust position size based on risk score
        let risk_multiplier = (1.0 - risk_score).max(0.1); // Minimum 10% of original size
        Ok((quantity as f64 * risk_multiplier) as u32)
    }
}

// Supporting traits
#[async_trait]
pub trait PositionTracker: Send + Sync {
    async fn get_position(&self, symbol: &str) -> Result<Position, RiskError>;
    async fn get_portfolio_value(&self) -> Result<f64, RiskError>;
    async fn get_total_exposure(&self) -> Result<f64, RiskError>;
}

#[async_trait]
pub trait PerformanceTracker: Send + Sync {
    async fn get_daily_pnl(&self) -> Result<f64, RiskError>;
    async fn get_drawdown(&self) -> Result<f64, RiskError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub quantity: i32, // Positive for long, negative for short
    pub avg_price: rust_decimal::Decimal,
    pub market_value: f64,
    pub unrealized_pnl: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum RiskError {
    #[error("Position limit exceeded")]
    PositionLimitExceeded,
    
    #[error("Daily loss limit exceeded")]
    DailyLossLimitExceeded,
    
    #[error("Emergency stop active")]
    EmergencyStopActive,
    
    #[error("Market closed")]
    MarketClosed,
    
    #[error("Insufficient data: {0}")]
    InsufficientData(String),
    
    #[error("Calculation error: {0}")]
    CalculationError(String),
}
```

---

## Phase 4 Templates: Integration & Testing

### 4.1 Integration Test Template

#### End-to-End Test Implementation
```rust
// src/integration/testing/end_to_end_test.rs
use crate::{
    shared::{
        event_bus::RedisStreamProducer,
        storage::TimescaleRepository,
        interfaces::{EventBus, Repository},
    },
    data::{
        features::FeatureExtractor,
        neural::MLPPredictor,
    },
    services::{
        action_layer::DecisionEngine,
        risk_management::BasicRiskValidator,
    },
};
use tokio;

pub struct EndToEndTestSuite {
    event_bus: Box<dyn EventBus>,
    storage: Box<dyn Repository<MarketDataPoint, TimeSeriesKey>>,
    feature_extractor: FeatureExtractor,
    predictor: MLPPredictor,
    decision_engine: DecisionEngine,
}

impl EndToEndTestSuite {
    pub async fn new() -> Result<Self, TestError> {
        // TODO: Initialize test components
        // Set up test database, Redis, etc.
        
        let event_bus = Self::setup_test_event_bus().await?;
        let storage = Self::setup_test_storage().await?;
        let feature_extractor = FeatureExtractor::new();
        let predictor = Self::setup_test_predictor().await?;
        let decision_engine = Self::setup_test_decision_engine(&event_bus).await?;
        
        Ok(Self {
            event_bus,
            storage,
            feature_extractor,
            predictor,
            decision_engine,
        })
    }
    
    pub async fn test_complete_data_flow(&mut self) -> Result<(), TestError> {
        // TODO: Test complete flow from market data to trading decision
        
        // 1. Simulate market data
        let market_data = self.create_test_market_data();
        
        // 2. Process through feature extraction
        let features = self.feature_extractor.process_market_data(
            &market_data.symbol,
            market_data.price.to_f64().unwrap(),
            market_data.volume as f64,
            market_data.high.unwrap().to_f64().unwrap(),
            market_data.low.unwrap().to_f64().unwrap(),
        );
        
        // 3. Generate neural prediction
        if let Some(indicators) = features {
            let prediction = self.predictor.predict(&indicators).await?;
            
            // 4. Process through decision engine
            let decision = self.decision_engine.process_prediction(prediction).await?;
            
            // 5. Verify decision was made
            assert!(decision.is_some(), "Expected a trading decision");
            
            // 6. Verify event was published
            // TODO: Consume from event bus and verify
        }
        
        Ok(())
    }
    
    pub async fn test_error_recovery(&self) -> Result<(), TestError> {
        // TODO: Test error scenarios and recovery
        
        // Test Redis failure recovery
        self.test_redis_failure_recovery().await?;
        
        // Test database failure recovery
        self.test_database_failure_recovery().await?;
        
        // Test model failure recovery
        self.test_model_failure_recovery().await?;
        
        Ok(())
    }
    
    pub async fn test_performance_under_load(&self) -> Result<PerformanceResults, TestError> {
        // TODO: Test system performance under load
        
        let start = std::time::Instant::now();
        let message_count = 10000;
        
        // Send many market data messages
        for i in 0..message_count {
            let market_data = self.create_test_market_data_with_sequence(i);
            self.simulate_market_data_ingestion(market_data).await?;
        }
        
        let duration = start.elapsed();
        
        Ok(PerformanceResults {
            messages_processed: message_count,
            total_duration: duration,
            throughput: message_count as f64 / duration.as_secs_f64(),
            avg_latency: duration / message_count,
        })
    }
    
    // Helper methods
    async fn setup_test_event_bus() -> Result<Box<dyn EventBus>, TestError> {
        // TODO: Set up Redis for testing
        todo!("Setup test Redis instance")
    }
    
    async fn setup_test_storage() -> Result<Box<dyn Repository<MarketDataPoint, TimeSeriesKey>>, TestError> {
        // TODO: Set up test database
        todo!("Setup test TimescaleDB")
    }
    
    async fn setup_test_predictor() -> Result<MLPPredictor, TestError> {
        // TODO: Set up test model
        todo!("Setup test neural model")
    }
    
    async fn setup_test_decision_engine(event_bus: &Box<dyn EventBus>) -> Result<DecisionEngine, TestError> {
        // TODO: Set up test decision engine
        todo!("Setup test decision engine")
    }
    
    fn create_test_market_data(&self) -> MarketDataPoint {
        MarketDataPoint {
            symbol: "TEST".to_string(),
            price: rust_decimal::Decimal::new(10050, 2), // $100.50
            volume: 1000,
            bid: Some(rust_decimal::Decimal::new(10045, 2)),
            ask: Some(rust_decimal::Decimal::new(10055, 2)),
            high: Some(rust_decimal::Decimal::new(10060, 2)),
            low: Some(rust_decimal::Decimal::new(10040, 2)),
            timestamp: chrono::Utc::now(),
            source: "test".to_string(),
        }
    }
    
    // Additional test helper methods...
}

#[derive(Debug, Clone)]
pub struct PerformanceResults {
    pub messages_processed: u32,
    pub total_duration: std::time::Duration,
    pub throughput: f64,
    pub avg_latency: std::time::Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum TestError {
    #[error("Setup error: {0}")]
    Setup(String),
    
    #[error("Test execution error: {0}")]
    Execution(String),
    
    #[error("Assertion failed: {0}")]
    AssertionFailed(String),
    
    #[error("Performance test failed: {0}")]
    Performance(String),
}

/// Test runner for all integration tests
pub async fn run_all_integration_tests() -> Result<(), TestError> {
    println!("Starting integration test suite...");
    
    let mut test_suite = EndToEndTestSuite::new().await?;
    
    // Run all tests
    println!("Testing complete data flow...");
    test_suite.test_complete_data_flow().await?;
    
    println!("Testing error recovery...");
    test_suite.test_error_recovery().await?;
    
    println!("Testing performance under load...");
    let perf_results = test_suite.test_performance_under_load().await?;
    println!("Performance results: {:.2} msg/sec, {:.2}ms avg latency", 
             perf_results.throughput, 
             perf_results.avg_latency.as_millis());
    
    println!("All integration tests passed!");
    Ok(())
}
```

### 4.2 Deployment Template

#### Docker Compose Configuration
```yaml
# docker-compose.mvp.yml
version: '3.8'

services:
  # Redis Streams for event bus
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
      - ./config/redis.conf:/etc/redis/redis.conf
    command: redis-server /etc/redis/redis.conf
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 30s
      timeout: 10s
      retries: 3
    networks:
      - neural-trader

  # TimescaleDB for storage
  timescaledb:
    image: timescale/timescaledb:latest-pg14
    environment:
      POSTGRES_DB: neural_trader
      POSTGRES_USER: ${POSTGRES_USER:-neural_trader}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-password}
    ports:
      - "5432:5432"
    volumes:
      - timescale_data:/var/lib/postgresql/data
      - ./src/shared/storage/schema:/docker-entrypoint-initdb.d
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U neural_trader"]
      interval: 30s
      timeout: 10s
      retries: 3
    networks:
      - neural-trader

  # Main neural trader application
  neural-trader:
    build:
      context: .
      dockerfile: docker/Dockerfile.mvp
    environment:
      RUST_LOG: ${RUST_LOG:-info}
      REDIS_URL: redis://redis:6379
      DATABASE_URL: postgresql://neural_trader:${POSTGRES_PASSWORD:-password}@timescaledb:5432/neural_trader
      ENABLE_LIVE_TRADING: ${ENABLE_LIVE_TRADING:-false}
      ALPACA_API_KEY: ${ALPACA_API_KEY}
      ALPACA_SECRET_KEY: ${ALPACA_SECRET_KEY}
      ALPACA_BASE_URL: ${ALPACA_BASE_URL:-https://paper-api.alpaca.markets}
    depends_on:
      redis:
        condition: service_healthy
      timescaledb:
        condition: service_healthy
    volumes:
      - ./models:/app/models
      - ./config:/app/config
    ports:
      - "8080:8080"  # REST API
      - "8081:8081"  # WebSocket
      - "9090:9090"  # Metrics
    networks:
      - neural-trader

  # Data ingestion service (Python)
  data-ingestion:
    build:
      context: ./data_ingestion
      dockerfile: Dockerfile
    environment:
      REDIS_URL: redis://redis:6379
      DATABASE_URL: postgresql://neural_trader:${POSTGRES_PASSWORD:-password}@timescaledb:5432/neural_trader
      ALPACA_API_KEY: ${ALPACA_API_KEY}
      ALPACA_SECRET_KEY: ${ALPACA_SECRET_KEY}
      SYMBOLS: ${SYMBOLS:-AAPL,GOOGL,MSFT,TSLA}
    depends_on:
      redis:
        condition: service_healthy
      timescaledb:
        condition: service_healthy
    networks:
      - neural-trader

  # Prometheus for metrics
  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9091:9090"
    volumes:
      - ./config/prometheus.yml:/etc/prometheus/prometheus.yml
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'
    networks:
      - neural-trader

  # Grafana for visualization
  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      GF_SECURITY_ADMIN_PASSWORD: ${GRAFANA_PASSWORD:-admin}
    volumes:
      - grafana_data:/var/lib/grafana
      - ./config/grafana/dashboards:/etc/grafana/provisioning/dashboards
      - ./config/grafana/datasources:/etc/grafana/provisioning/datasources
    networks:
      - neural-trader

volumes:
  redis_data:
  timescale_data:
  grafana_data:

networks:
  neural-trader:
    driver: bridge
```

#### Dockerfile Template
```dockerfile
# docker/Dockerfile.mvp
FROM rust:1.70 as builder

WORKDIR /app

# Copy Cargo files for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY vendor/ ./vendor/

# Create src directory with dummy main for dependency build
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies
RUN cargo build --release

# Copy actual source code
COPY src/ ./src/

# Build application
RUN cargo build --release

# Runtime stage
FROM ubuntu:22.04

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN groupadd -r app && useradd -r -g app app

# Copy binary from builder
COPY --from=builder /app/target/release/neural-trader /usr/local/bin/neural-trader

# Copy configuration
COPY config/ /app/config/

# Create directories
RUN mkdir -p /app/models /app/logs && \
    chown -R app:app /app

USER app
WORKDIR /app

# Expose ports
EXPOSE 8080 8081 9090

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:8080/health || exit 1

# Run application
CMD ["neural-trader"]
```

---

## Template Usage Guidelines

### Development Workflow

1. **Phase 1 Implementation**:
   - Start with shared infrastructure templates
   - Implement Redis Streams producer/consumer
   - Set up TimescaleDB repositories
   - Add configuration management
   - Integrate monitoring

2. **Phase 2 Implementation**:
   - Implement feature extraction
   - Create neural model predictor
   - Integrate with data ingestion
   - Add performance monitoring

3. **Phase 3 Implementation**:
   - Build decision engine
   - Implement risk management
   - Create API interfaces
   - Add trading controls

4. **Phase 4 Implementation**:
   - Write integration tests
   - Set up deployment scripts
   - Performance testing
   - Production validation

### Template Customization

- **TODO Comments**: Mark areas requiring implementation
- **Configuration**: Adjust configs for specific environments
- **Error Handling**: Customize error types and handling
- **Metrics**: Add domain-specific metrics
- **Testing**: Extend test cases for specific requirements

These templates provide a **concrete foundation** for implementing the V2 MVP architecture, ensuring consistency across all development phases while maintaining the flexibility needed for specific requirements and optimizations.