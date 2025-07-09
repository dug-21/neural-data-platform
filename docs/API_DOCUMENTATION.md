# Neural Trader API Documentation

## Table of Contents
1. [Neural Prediction API](#neural-prediction-api)
2. [DAA Integration API](#daa-integration-api)
3. [Trading Strategy API](#trading-strategy-api)
4. [MCP Trading Server API](#mcp-trading-server-api)
5. [Data Ingestion API](#data-ingestion-api)

## Neural Prediction API

### FANNPredictor

The core neural network prediction interface using vendored FANN library.

```rust
use neural_trader::neural::FANNPredictor;
```

#### Methods

##### `new(config: NeuralConfig) -> Result<Self>`
Creates a new FANN predictor instance.

**Parameters:**
- `config`: Neural network configuration

**Example:**
```rust
let config = NeuralConfig {
    layers: vec![20, 40, 20, 1],
    learning_rate: 0.001,
    training_epochs: 1000,
    prediction_horizon: Duration::minutes(5),
};
let predictor = FANNPredictor::new(config)?;
```

##### `predict(symbol: &str, data: &MarketData) -> Result<Prediction>`
Generates price prediction for a given symbol.

**Parameters:**
- `symbol`: Stock symbol (e.g., "AAPL")
- `data`: Current market data

**Returns:**
```rust
pub struct Prediction {
    pub symbol: String,
    pub current_price: f64,
    pub predicted_price: f64,
    pub confidence: f64,        // 0.0 to 1.0
    pub horizon: Duration,
    pub timestamp: DateTime<Utc>,
}
```

##### `train(training_data: &TrainingData) -> Result<()>`
Trains the neural network with historical data.

**Parameters:**
- `training_data`: Historical price and volume data

##### `evaluate(test_data: &TestData) -> Result<ModelMetrics>`
Evaluates model performance.

**Returns:**
```rust
pub struct ModelMetrics {
    pub mse: f64,              // Mean Squared Error
    pub mae: f64,              // Mean Absolute Error
    pub accuracy: f64,         // Directional accuracy
    pub sharpe_ratio: f64,
}
```

## DAA Integration API

### DAABridge

Interface to the Distributed Autonomous Agents system.

```rust
use neural_trader::agents::DAABridge;
```

#### Methods

##### `new(config: DAAConfig) -> Result<Self>`
Initializes DAA bridge with configuration.

**Parameters:**
```rust
pub struct DAAConfig {
    pub agent_count: usize,
    pub consensus_threshold: f64,
    pub risk_tolerance: RiskLevel,
    pub decision_timeout: Duration,
}
```

##### `get_consensus_decision(symbol: &str, context: &MarketContext) -> Result<TradingDecision>`
Gets consensus trading decision from DAA network.

**Returns:**
```rust
pub struct TradingDecision {
    pub action: Action,           // Buy, Sell, Hold
    pub symbol: String,
    pub quantity: u32,
    pub confidence: f64,
    pub risk_score: f64,
    pub agents_agreed: usize,
    pub total_agents: usize,
    pub reasoning: Vec<String>,
}
```

##### `spawn_agent(agent_type: AgentType) -> Result<AgentId>`
Spawns a new autonomous agent.

**Agent Types:**
- `RiskAnalyst`: Focuses on risk assessment
- `TechnicalAnalyst`: Technical indicators analysis
- `FundamentalAnalyst`: Fundamental analysis
- `SentimentAnalyst`: Market sentiment analysis
- `Arbitrageur`: Arbitrage opportunity detection

### AutonomousDecisions

Advanced decision-making interface.

```rust
use neural_trader::integration::AutonomousDecisions;
```

#### Methods

##### `analyze_opportunity(market_data: &MarketData) -> Result<Opportunity>`
Analyzes market for trading opportunities.

**Returns:**
```rust
pub struct Opportunity {
    pub id: Uuid,
    pub symbol: String,
    pub opportunity_type: OpportunityType,
    pub expected_return: f64,
    pub risk_level: RiskLevel,
    pub time_horizon: Duration,
    pub entry_price: f64,
    pub target_price: f64,
    pub stop_loss: f64,
}
```

## Trading Strategy API

### NeuralEnhancedStrategy

Neural network enhanced trading strategy.

```rust
use neural_trader::strategies::NeuralEnhancedStrategy;
```

#### Methods

##### `execute(market_data: &MarketData, position: &Position) -> Result<Vec<Order>>`
Executes trading strategy and returns orders.

**Parameters:**
- `market_data`: Current market state
- `position`: Current position information

**Returns:**
```rust
pub struct Order {
    pub id: Uuid,
    pub symbol: String,
    pub order_type: OrderType,    // Market, Limit, Stop
    pub side: OrderSide,          // Buy, Sell
    pub quantity: u32,
    pub price: Option<f64>,       // For limit orders
    pub time_in_force: TimeInForce,
    pub metadata: HashMap<String, Value>,
}
```

## MCP Trading Server API

### REST Endpoints

#### `GET /health`
Health check endpoint.

**Response:**
```json
{
  "status": "healthy",
  "version": "1.0.0",
  "uptime_seconds": 3600,
  "components": {
    "neural_engine": "healthy",
    "daa_network": "healthy",
    "data_pipeline": "healthy"
  }
}
```

#### `POST /predict`
Get neural network prediction.

**Request:**
```json
{
  "symbol": "AAPL",
  "horizon": "5min"
}
```

**Response:**
```json
{
  "symbol": "AAPL",
  "current_price": 150.25,
  "predicted_price": 150.75,
  "confidence": 0.82,
  "horizon": "5min",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

#### `POST /decision`
Get trading decision from DAA.

**Request:**
```json
{
  "symbol": "AAPL",
  "include_reasoning": true
}
```

**Response:**
```json
{
  "action": "buy",
  "symbol": "AAPL",
  "quantity": 100,
  "confidence": 0.75,
  "risk_score": 0.3,
  "agents_agreed": 4,
  "total_agents": 5,
  "reasoning": [
    "Strong momentum detected",
    "Support level holding",
    "Positive market sentiment"
  ]
}
```

#### `GET /metrics`
System metrics and performance.

**Response:**
```json
{
  "performance": {
    "total_trades": 1523,
    "win_rate": 0.68,
    "average_return": 0.023,
    "sharpe_ratio": 1.85
  },
  "neural_metrics": {
    "prediction_accuracy": 0.82,
    "model_version": "2.1.0",
    "last_training": "2024-01-15T09:00:00Z"
  },
  "system_metrics": {
    "cpu_usage": 0.45,
    "memory_usage": 0.62,
    "request_latency_ms": 12
  }
}
```

### WebSocket Endpoints

#### `/ws/stream`
Real-time market data and trading signals.

**Subscribe:**
```json
{
  "type": "subscribe",
  "channels": ["predictions", "decisions", "executions"],
  "symbols": ["AAPL", "GOOGL"]
}
```

**Messages:**
```json
{
  "type": "prediction",
  "data": {
    "symbol": "AAPL",
    "predicted_price": 150.75,
    "confidence": 0.82,
    "timestamp": "2024-01-15T10:30:00Z"
  }
}
```

## Data Ingestion API

### Python API

#### DataProvider Base Class
```python
from data_ingestion.providers.base import DataProvider

class CustomProvider(DataProvider):
    async def fetch_market_data(self, symbol: str) -> MarketData:
        # Implementation
        pass
```

#### Market Data Models
```python
from data_ingestion.models import MarketData, OHLCV

# OHLCV data
ohlcv = OHLCV(
    timestamp=datetime.now(),
    open=150.0,
    high=151.0,
    low=149.5,
    close=150.5,
    volume=1000000
)

# Market data with indicators
market_data = MarketData(
    symbol="AAPL",
    ohlcv=ohlcv,
    indicators={
        "rsi": 65.5,
        "macd": 0.25,
        "moving_avg_20": 149.8
    }
)
```

### Error Handling

All APIs use consistent error responses:

```json
{
  "error": {
    "code": "INVALID_SYMBOL",
    "message": "Symbol INVALID not found",
    "details": {
      "valid_symbols": ["AAPL", "GOOGL", "MSFT"]
    }
  }
}
```

Error codes:
- `INVALID_SYMBOL`: Invalid trading symbol
- `INSUFFICIENT_DATA`: Not enough data for prediction
- `MODEL_ERROR`: Neural network error
- `CONSENSUS_TIMEOUT`: DAA consensus timeout
- `RATE_LIMIT`: API rate limit exceeded

## Rate Limits

- REST API: 100 requests/minute
- WebSocket: 1000 messages/minute
- Predictions: 20 requests/minute per symbol

## Authentication

API key required in header:
```
X-API-Key: your-api-key
```

Or Bearer token:
```
Authorization: Bearer your-jwt-token
```