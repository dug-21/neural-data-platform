use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    pub symbol: String,
    pub price: f64,
    pub timestamp: DateTime<Utc>,
    pub volume: Option<f64>,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    // OHLC fields for compatibility
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub close: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OHLCV {
    pub timestamp: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderbookLevel {
    pub price: f64,
    pub quantity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Orderbook {
    pub symbol: String,
    pub bids: Vec<OrderbookLevel>,
    pub asks: Vec<OrderbookLevel>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketStats {
    pub symbol: String,
    pub period: String,
    pub volume: f64,
    pub high: f64,
    pub low: f64,
    pub open: f64,
    pub close: f64,
    pub change_amount: f64,
    pub change_percent: f64,
    pub trade_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalIndicators {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub rsi: Option<f64>,
    pub macd: Option<MACDValues>,
    pub bollinger_bands: Option<BollingerBands>,
    pub moving_averages: Option<MovingAverages>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MACDValues {
    pub macd: f64,
    pub signal: f64,
    pub histogram: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BollingerBands {
    pub upper: f64,
    pub middle: f64,
    pub lower: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovingAverages {
    pub sma_9: f64,
    pub sma_20: f64,
    pub sma_50: f64,
    pub sma_200: f64,
    pub ema_9: f64,
    pub ema_20: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePrediction {
    pub symbol: String,
    pub timeframe: String,
    pub predictions: Vec<PredictionPoint>,
    pub confidence: f64,
    pub model_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionPoint {
    pub timestamp: DateTime<Utc>,
    pub predicted_price: f64,
    pub upper_bound: f64,
    pub lower_bound: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    pub symbol: String,
    pub trend: String, // bullish, bearish, neutral
    pub strength: f64, // 0.0 to 1.0
    pub support_levels: Vec<f64>,
    pub resistance_levels: Vec<f64>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartPattern {
    pub name: String,
    pub confidence: f64,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub target_price: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSignal {
    pub symbol: String,
    pub action: String, // buy, sell, hold
    pub confidence: f64,
    pub price: f64,
    pub reasoning: String,
    pub timestamp: DateTime<Utc>,
    pub entry_price: f64,
    pub take_profit: Option<f64>,
    pub stop_loss: Option<f64>,
    pub risk_reward: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub order_id: String,
    pub symbol: String,
    pub side: String, // buy, sell
    pub order_type: String, // market, limit
    pub quantity: f64,
    pub price: Option<f64>,
    pub take_profit: Option<f64>,
    pub stop_loss: Option<f64>,
    pub status: String, // pending, filled, cancelled
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub quantity: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub opened_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub total_value: f64,
    pub cash_balance: f64,
    pub positions: Vec<Position>,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub daily_change: f64,
    pub daily_change_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub overall_status: String, // healthy, degraded, unhealthy
    pub components: ComponentHealthMap,
    pub timestamp: DateTime<Utc>,
}

pub type ComponentHealthMap = std::collections::HashMap<String, ComponentHealth>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub component: String,
    pub status: String,
    pub latency_ms: f64,
    pub last_check: DateTime<Utc>,
    pub error_count: u64,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub timeframe: String,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub network_io: NetworkIO,
    pub api_latency: APILatency,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkIO {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct APILatency {
    pub p50: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
    pub mean: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub component: String,
    pub message: String,
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub severity: String, // critical, warning, info
    pub component: String,
    pub message: String,
    pub triggered_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub checks_passed: usize,
    pub checks_failed: usize,
    pub details: Vec<HealthCheckDetail>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckDetail {
    pub check: String,
    pub status: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeDecision {
    pub symbol: String,
    pub action: String, // buy, sell, hold
    pub quantity: f64,
    pub price: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub reasoning: String,
    pub timestamp: DateTime<Utc>,
    pub confidence: f64,
    pub reasons: Vec<String>,
    pub entry_price: f64,
    pub position_size: f64,
    pub risk_reward_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub symbol: String,
    pub risk_score: f64, // 0.0 to 1.0
    pub max_loss: f64,
    pub probability_of_loss: f64,
    pub risk_reward_ratio: f64,
    pub recommendation: String,
    pub risk_level: String,
    pub exposure_percentage: f64,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionSize {
    pub symbol: String,
    pub recommended_size: f64,
    pub max_size: f64,
    pub risk_per_trade: f64,
    pub account_risk_percent: f64,
    pub recommended_shares: f64,
    pub position_value: f64,
    pub risk_amount: f64,
    pub percentage_of_capital: f64,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub risk_per_share: f64,
}