# Integration Specifications

## Overview

This document defines the integration specifications for the Neural-Trader platform, including API interfaces, data contracts, communication protocols, and system integration patterns. The specifications ensure seamless interaction between neural networks, trading strategies, data pipelines, and external systems.

## System Architecture Integration

### Core Integration Points

```rust
// Main integration flow
Market Data → Data Pipeline → Neural Networks → Trading Strategies → Order Management → Execution
     ↓             ↓              ↓               ↓                ↓              ↓
  [WebSocket]  [Event Bus]   [Prediction API]  [Strategy API]  [Order API]  [Broker API]
  [REST API]   [Redis Pub/Sub] [Model Registry] [Risk Engine]  [Portfolio]  [Execution]
```

### Integration Layers

1. **Data Layer**: Market data ingestion and processing
2. **Neural Layer**: Machine learning model execution
3. **Strategy Layer**: Trading strategy implementation
4. **Execution Layer**: Order management and execution
5. **Monitoring Layer**: System health and performance tracking

## API Specifications

### Neural Prediction API

#### Core Interface
```rust
#[async_trait]
pub trait NeuralPredictorTrait {
    /// Generate predictions for a single symbol
    async fn predict(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        features: Option<HashMap<String, serde_json::Value>>
    ) -> Result<Vec<PredictionResult>>;
    
    /// Generate ensemble predictions using multiple models
    async fn predict_ensemble(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        models: &[String],
        features: Option<HashMap<String, serde_json::Value>>
    ) -> Result<Vec<PredictionResult>>;
    
    /// Get feature importance for interpretability
    async fn get_feature_importance(&self) -> Result<HashMap<String, f64>>;
    
    /// Update model with new training data
    async fn update_with_feedback(
        &self,
        predictions: &[PredictionResult],
        actual: &[f64]
    ) -> Result<()>;
}
```

#### Data Structures
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResult {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub confidence: f64,
    pub interval_low: f64,
    pub interval_high: f64,
    pub model_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesData {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub indicators: HashMap<String, f64>,
}

#[derive(Debug, Clone)]
pub struct NeuralConfig {
    pub memory_gb: f64,
    pub models: Vec<String>,
    pub prediction_cache_ttl: u64,
    pub model_load_timeout: u64,
    pub max_concurrent_predictions: usize,
    pub enable_model_monitoring: bool,
    pub accuracy_threshold: f64,
}
```

### Trading Strategy API

#### Strategy Interface
```rust
#[async_trait]
pub trait TradingStrategy {
    /// Generate trading signals based on market data
    async fn generate_signal(
        &self,
        market_data: &MarketContext,
        portfolio: &Portfolio
    ) -> Result<TradingSignal>;
    
    /// Update strategy parameters
    async fn update_parameters(
        &mut self,
        parameters: HashMap<String, f64>
    ) -> Result<()>;
    
    /// Get strategy performance metrics
    async fn get_performance_metrics(&self) -> Result<PerformanceMetrics>;
    
    /// Handle strategy initialization
    async fn initialize(&mut self, config: &StrategyConfig) -> Result<()>;
}
```

#### Trading Signal Structure
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSignal {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub signal_type: SignalType,
    pub strength: f64,
    pub confidence: f64,
    pub entry_price: Option<f64>,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub position_size: f64,
    pub time_horizon: Duration,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SignalType {
    Buy,
    Sell,
    Hold,
    Close,
}
```

### Market Data API

#### Data Provider Interface
```rust
#[async_trait]
pub trait MarketDataProvider {
    /// Stream real-time market data
    async fn stream_data(
        &self,
        symbols: Vec<String>,
        data_types: Vec<DataType>
    ) -> Result<Pin<Box<dyn Stream<Item = MarketData>>>>;
    
    /// Get historical data
    async fn get_historical_data(
        &self,
        symbol: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        interval: Duration
    ) -> Result<Vec<TimeSeriesData>>;
    
    /// Get real-time quotes
    async fn get_quotes(
        &self,
        symbols: Vec<String>
    ) -> Result<Vec<Quote>>;
    
    /// Check provider health
    async fn health_check(&self) -> Result<ProviderHealth>;
}
```

#### Market Data Structures
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub data_type: DataType,
    pub price: f64,
    pub volume: f64,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    Trade,
    Quote,
    Bar,
    News,
    Fundamental,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub bid: f64,
    pub ask: f64,
    pub bid_size: f64,
    pub ask_size: f64,
    pub spread: f64,
}
```

## Event-Driven Architecture

### Event Bus Implementation
```rust
pub struct EventBus {
    subscribers: Arc<RwLock<HashMap<String, Vec<Box<dyn EventHandler + Send + Sync>>>>>,
    redis_client: Arc<redis::Client>,
    metrics: Arc<Metrics>,
}

impl EventBus {
    /// Publish event to all subscribers
    pub async fn publish<T: Event + Send + Sync>(&self, event: T) -> Result<()> {
        let event_type = event.event_type();
        let serialized = serde_json::to_string(&event)?;
        
        // Publish to Redis for distributed processing
        self.redis_client.publish(&event_type, &serialized).await?;
        
        // Notify local subscribers
        let subscribers = self.subscribers.read().await;
        if let Some(handlers) = subscribers.get(&event_type) {
            for handler in handlers {
                handler.handle(&event).await?;
            }
        }
        
        // Record metrics
        self.metrics.increment_counter("events_published", &[&event_type]);
        
        Ok(())
    }
    
    /// Subscribe to events
    pub async fn subscribe<T: EventHandler + Send + Sync + 'static>(
        &self,
        event_type: String,
        handler: T
    ) -> Result<()> {
        let mut subscribers = self.subscribers.write().await;
        let handlers = subscribers.entry(event_type).or_insert_with(Vec::new);
        handlers.push(Box::new(handler));
        Ok(())
    }
}
```

### Event Types
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    MarketDataReceived(MarketDataEvent),
    PredictionGenerated(PredictionEvent),
    SignalGenerated(SignalEvent),
    OrderExecuted(OrderEvent),
    RiskAlert(RiskEvent),
    SystemHealth(HealthEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketDataEvent {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub data: MarketData,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionEvent {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub predictions: Vec<PredictionResult>,
    pub model_name: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalEvent {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub signal: TradingSignal,
    pub strategy_name: String,
}
```

## Configuration Management

### Configuration Schema
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    pub data_sources: DataSourceConfig,
    pub neural_models: NeuralConfig,
    pub trading_strategies: StrategyConfig,
    pub risk_management: RiskConfig,
    pub execution: ExecutionConfig,
    pub monitoring: MonitoringConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceConfig {
    pub providers: Vec<ProviderConfig>,
    pub symbols: Vec<String>,
    pub update_interval: Duration,
    pub buffer_size: usize,
    pub quality_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub enabled: bool,
    pub api_key: String,
    pub rate_limit: RateLimit,
    pub timeout: Duration,
    pub retry_attempts: usize,
}
```

### Dynamic Configuration Updates
```rust
pub struct ConfigManager {
    config: Arc<RwLock<PlatformConfig>>,
    watchers: Vec<Box<dyn ConfigWatcher + Send + Sync>>,
}

impl ConfigManager {
    /// Update configuration at runtime
    pub async fn update_config(&self, updates: ConfigUpdate) -> Result<()> {
        let mut config = self.config.write().await;
        
        // Apply updates
        match updates {
            ConfigUpdate::DataSource(ds_config) => {
                config.data_sources = ds_config;
            }
            ConfigUpdate::Neural(neural_config) => {
                config.neural_models = neural_config;
            }
            ConfigUpdate::Strategy(strategy_config) => {
                config.trading_strategies = strategy_config;
            }
        }
        
        // Notify watchers
        for watcher in &self.watchers {
            watcher.on_config_changed(&config).await?;
        }
        
        Ok(())
    }
}
```

## Database Integration

### Repository Pattern
```rust
#[async_trait]
pub trait Repository<T> {
    async fn save(&self, entity: &T) -> Result<()>;
    async fn find_by_id(&self, id: &str) -> Result<Option<T>>;
    async fn find_by_criteria(&self, criteria: &QueryCriteria) -> Result<Vec<T>>;
    async fn update(&self, entity: &T) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<()>;
}

/// Time-series specific repository
#[async_trait]
pub trait TimeSeriesRepository<T> {
    async fn save_time_series(&self, data: &[T]) -> Result<()>;
    async fn query_time_range(
        &self,
        symbol: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>
    ) -> Result<Vec<T>>;
    async fn get_latest(&self, symbol: &str, limit: usize) -> Result<Vec<T>>;
}
```

### Database Implementations
```rust
pub struct TimescaleRepository {
    pool: Arc<sqlx::PgPool>,
}

impl TimescaleRepository {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = sqlx::PgPool::connect(database_url).await?;
        Ok(Self { pool: Arc::new(pool) })
    }
}

#[async_trait]
impl TimeSeriesRepository<TimeSeriesData> for TimescaleRepository {
    async fn save_time_series(&self, data: &[TimeSeriesData]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        
        for item in data {
            sqlx::query!(
                "INSERT INTO market_data (symbol, timestamp, open, high, low, close, volume) 
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
                item.symbol, item.timestamp, item.open, item.high, 
                item.low, item.close, item.volume
            )
            .execute(&mut *tx)
            .await?;
        }
        
        tx.commit().await?;
        Ok(())
    }
    
    async fn query_time_range(
        &self,
        symbol: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>
    ) -> Result<Vec<TimeSeriesData>> {
        let rows = sqlx::query!(
            "SELECT * FROM market_data 
             WHERE symbol = $1 AND timestamp >= $2 AND timestamp <= $3
             ORDER BY timestamp ASC",
            symbol, start, end
        )
        .fetch_all(&self.pool)
        .await?;
        
        let data = rows.into_iter()
            .map(|row| TimeSeriesData {
                timestamp: row.timestamp,
                symbol: row.symbol,
                open: row.open.to_f64().unwrap_or(0.0),
                high: row.high.to_f64().unwrap_or(0.0),
                low: row.low.to_f64().unwrap_or(0.0),
                close: row.close.to_f64().unwrap_or(0.0),
                volume: row.volume as f64,
                indicators: HashMap::new(),
            })
            .collect();
        
        Ok(data)
    }
}
```

## External Service Integration

### Broker Integration
```rust
#[async_trait]
pub trait BrokerIntegration {
    /// Place order
    async fn place_order(&self, order: OrderRequest) -> Result<OrderResponse>;
    
    /// Cancel order
    async fn cancel_order(&self, order_id: &str) -> Result<()>;
    
    /// Get order status
    async fn get_order_status(&self, order_id: &str) -> Result<OrderStatus>;
    
    /// Get account information
    async fn get_account_info(&self) -> Result<AccountInfo>;
    
    /// Get positions
    async fn get_positions(&self) -> Result<Vec<Position>>;
}

/// Alpaca broker implementation
pub struct AlpacaBroker {
    client: reqwest::Client,
    api_key: String,
    secret_key: String,
    base_url: String,
}

impl AlpacaBroker {
    pub fn new(api_key: String, secret_key: String, paper_trading: bool) -> Self {
        let base_url = if paper_trading {
            "https://paper-api.alpaca.markets"
        } else {
            "https://api.alpaca.markets"
        };
        
        Self {
            client: reqwest::Client::new(),
            api_key,
            secret_key,
            base_url: base_url.to_string(),
        }
    }
}

#[async_trait]
impl BrokerIntegration for AlpacaBroker {
    async fn place_order(&self, order: OrderRequest) -> Result<OrderResponse> {
        let response = self.client
            .post(&format!("{}/v2/orders", self.base_url))
            .header("APCA-API-KEY-ID", &self.api_key)
            .header("APCA-API-SECRET-KEY", &self.secret_key)
            .json(&order)
            .send()
            .await?;
        
        let order_response: OrderResponse = response.json().await?;
        Ok(order_response)
    }
    
    async fn get_account_info(&self) -> Result<AccountInfo> {
        let response = self.client
            .get(&format!("{}/v2/account", self.base_url))
            .header("APCA-API-KEY-ID", &self.api_key)
            .header("APCA-API-SECRET-KEY", &self.secret_key)
            .send()
            .await?;
        
        let account_info: AccountInfo = response.json().await?;
        Ok(account_info)
    }
}
```

### News and Sentiment Integration
```rust
#[async_trait]
pub trait NewsProvider {
    async fn get_news(&self, symbol: &str, limit: usize) -> Result<Vec<NewsItem>>;
    async fn get_sentiment(&self, symbol: &str) -> Result<SentimentScore>;
    async fn stream_news(&self, symbols: Vec<String>) -> Result<Pin<Box<dyn Stream<Item = NewsItem>>>>;
}

pub struct NewsAPIProvider {
    client: reqwest::Client,
    api_key: String,
}

impl NewsAPIProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
        }
    }
}

#[async_trait]
impl NewsProvider for NewsAPIProvider {
    async fn get_news(&self, symbol: &str, limit: usize) -> Result<Vec<NewsItem>> {
        let response = self.client
            .get("https://newsapi.org/v2/everything")
            .query(&[
                ("q", symbol),
                ("apiKey", &self.api_key),
                ("pageSize", &limit.to_string()),
                ("sortBy", "publishedAt"),
                ("language", "en")
            ])
            .send()
            .await?;
        
        let news_response: NewsResponse = response.json().await?;
        Ok(news_response.articles)
    }
    
    async fn get_sentiment(&self, symbol: &str) -> Result<SentimentScore> {
        let news_items = self.get_news(symbol, 50).await?;
        let sentiment = self.analyze_sentiment(&news_items).await?;
        Ok(sentiment)
    }
}
```

## Monitoring and Observability Integration

### Metrics Collection
```rust
pub struct MetricsCollector {
    prometheus_registry: Arc<prometheus::Registry>,
    counters: HashMap<String, prometheus::Counter>,
    histograms: HashMap<String, prometheus::Histogram>,
    gauges: HashMap<String, prometheus::Gauge>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let registry = Arc::new(prometheus::Registry::new());
        
        Self {
            prometheus_registry: registry,
            counters: HashMap::new(),
            histograms: HashMap::new(),
            gauges: HashMap::new(),
        }
    }
    
    pub fn increment_counter(&self, name: &str, labels: &[&str]) {
        if let Some(counter) = self.counters.get(name) {
            counter.inc();
        }
    }
    
    pub fn record_histogram(&self, name: &str, value: f64) {
        if let Some(histogram) = self.histograms.get(name) {
            histogram.observe(value);
        }
    }
    
    pub fn set_gauge(&self, name: &str, value: f64) {
        if let Some(gauge) = self.gauges.get(name) {
            gauge.set(value);
        }
    }
}
```

### Health Check Integration
```rust
#[async_trait]
pub trait HealthCheck {
    async fn health_check(&self) -> Result<HealthStatus>;
    fn component_name(&self) -> &str;
}

pub struct HealthMonitor {
    components: Vec<Box<dyn HealthCheck + Send + Sync>>,
}

impl HealthMonitor {
    pub async fn check_system_health(&self) -> SystemHealth {
        let mut health_results = Vec::new();
        
        for component in &self.components {
            let component_name = component.component_name();
            match component.health_check().await {
                Ok(status) => health_results.push((component_name.to_string(), status)),
                Err(e) => {
                    health_results.push((
                        component_name.to_string(),
                        HealthStatus::Unhealthy(e.to_string())
                    ));
                }
            }
        }
        
        SystemHealth {
            timestamp: Utc::now(),
            overall_status: self.determine_overall_status(&health_results),
            components: health_results,
        }
    }
}
```

## Error Handling and Resilience

### Error Types
```rust
#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    #[error("Data source error: {0}")]
    DataSource(#[from] DataSourceError),
    
    #[error("Neural network error: {0}")]
    Neural(#[from] NeuralError),
    
    #[error("Trading strategy error: {0}")]
    Strategy(#[from] StrategyError),
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
}
```

### Circuit Breaker Pattern
```rust
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_threshold: usize,
    recovery_timeout: Duration,
    failure_count: Arc<AtomicUsize>,
    last_failure: Arc<RwLock<Option<Instant>>>,
}

impl CircuitBreaker {
    pub async fn call<F, T, E>(&self, operation: F) -> Result<T, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        match *self.state.read().await {
            CircuitState::Open => {
                let last_failure = *self.last_failure.read().await;
                if let Some(last_failure) = last_failure {
                    if last_failure.elapsed() > self.recovery_timeout {
                        *self.state.write().await = CircuitState::HalfOpen;
                    } else {
                        return Err(/* Circuit breaker open error */);
                    }
                }
            }
            CircuitState::HalfOpen => {
                match operation.await {
                    Ok(result) => {
                        *self.state.write().await = CircuitState::Closed;
                        self.failure_count.store(0, Ordering::SeqCst);
                        return Ok(result);
                    }
                    Err(e) => {
                        *self.state.write().await = CircuitState::Open;
                        *self.last_failure.write().await = Some(Instant::now());
                        return Err(e);
                    }
                }
            }
            CircuitState::Closed => {
                match operation.await {
                    Ok(result) => Ok(result),
                    Err(e) => {
                        let failure_count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                        if failure_count >= self.failure_threshold {
                            *self.state.write().await = CircuitState::Open;
                            *self.last_failure.write().await = Some(Instant::now());
                        }
                        Err(e)
                    }
                }
            }
        }
    }
}
```

## Testing Integration

### Integration Test Framework
```rust
pub struct IntegrationTestHarness {
    pub database: TestDatabase,
    pub mock_broker: MockBroker,
    pub mock_data_provider: MockDataProvider,
    pub event_bus: EventBus,
    pub config: PlatformConfig,
}

impl IntegrationTestHarness {
    pub async fn new() -> Result<Self> {
        let database = TestDatabase::new().await?;
        let mock_broker = MockBroker::new();
        let mock_data_provider = MockDataProvider::new();
        let event_bus = EventBus::new();
        let config = PlatformConfig::test_config();
        
        Ok(Self {
            database,
            mock_broker,
            mock_data_provider,
            event_bus,
            config,
        })
    }
    
    pub async fn create_test_scenario(&self) -> TestScenario {
        TestScenario::new(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_end_to_end_trading_flow() {
        let harness = IntegrationTestHarness::new().await.unwrap();
        let scenario = harness.create_test_scenario().await;
        
        // Inject market data
        let market_data = scenario.create_market_data("BTC/USD").await;
        scenario.inject_market_data(market_data).await.unwrap();
        
        // Wait for neural predictions
        let predictions = scenario.wait_for_predictions("BTC/USD").await.unwrap();
        assert!(!predictions.is_empty());
        
        // Wait for trading signals
        let signals = scenario.wait_for_signals("BTC/USD").await.unwrap();
        assert!(!signals.is_empty());
        
        // Verify order placement
        let orders = scenario.get_placed_orders().await.unwrap();
        assert!(!orders.is_empty());
    }
}
```

## Deployment Integration

### Docker Configuration
```dockerfile
# Multi-stage build for neural-trader
FROM rust:1.70 as builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release --bin neural-trader

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/neural-trader /usr/local/bin/
COPY config/ /app/config/

EXPOSE 8080 9090

CMD ["neural-trader"]
```

### Kubernetes Integration
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: neural-trader
spec:
  replicas: 3
  selector:
    matchLabels:
      app: neural-trader
  template:
    metadata:
      labels:
        app: neural-trader
    spec:
      containers:
      - name: neural-trader
        image: neural-trader:latest
        ports:
        - containerPort: 8080
        - containerPort: 9090
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: neural-trader-secrets
              key: database-url
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: neural-trader-secrets
              key: redis-url
        resources:
          requests:
            memory: "1Gi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "1000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
```

---

*This integration specification provides comprehensive guidelines for implementing and connecting all components of the Neural-Trader platform. For specific implementation details, refer to the source code and accompanying documentation.*