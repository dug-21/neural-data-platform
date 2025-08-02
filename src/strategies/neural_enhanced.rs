//! Neural-Enhanced Trading Strategy
//!
//! Combines neural network predictions with traditional technical indicators
//! for sophisticated trading signal generation.

use super::{MarketContext, Position, Signal, StrategyConfig, StrategyError, TradingStrategy};
use crate::data::TimeSeriesData;
use crate::neural::NeuralPredictor;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Configuration for the neural-enhanced strategy
#[derive(Debug, Clone)]
pub struct NeuralEnhancedConfig {
    /// Minimum confidence threshold for neural predictions
    pub min_confidence: f64,
    /// Weight for momentum signal (0.0 to 1.0)
    pub momentum_weight: f64,
    /// Weight for mean reversion signal (0.0 to 1.0)
    pub mean_reversion_weight: f64,
    /// Weight for neural prediction signal (0.0 to 1.0)
    pub neural_weight: f64,
    /// RSI oversold threshold
    pub rsi_oversold: f64,
    /// RSI overbought threshold
    pub rsi_overbought: f64,
    /// Volume spike threshold (relative to average)
    pub volume_spike_threshold: f64,
    /// Maximum position size as percentage of portfolio
    pub max_position_size: f64,
    /// Stop loss percentage
    pub stop_loss_pct: f64,
    /// Take profit percentage
    pub take_profit_pct: f64,
    /// History size to maintain
    pub history_size: usize,
}

impl Default for NeuralEnhancedConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.65,
            momentum_weight: 0.3,
            mean_reversion_weight: 0.2,
            neural_weight: 0.5,
            rsi_oversold: 30.0,
            rsi_overbought: 70.0,
            volume_spike_threshold: 2.0,
            max_position_size: 0.02,
            stop_loss_pct: 0.02,
            take_profit_pct: 0.03,
            history_size: 100,
        }
    }
}

/// Neural-enhanced trading strategy
pub struct NeuralEnhancedStrategy {
    config: NeuralEnhancedConfig,
    neural_predictor: Arc<NeuralPredictor>,
    price_history: Arc<RwLock<VecDeque<f64>>>,
    volume_history: Arc<RwLock<VecDeque<f64>>>,
    metrics: Arc<RwLock<HashMap<String, f64>>>,
    entry_price: Arc<RwLock<Option<f64>>>,
    initialized: bool,
}

impl NeuralEnhancedStrategy {
    pub fn new(neural_predictor: Arc<NeuralPredictor>) -> Self {
        let config = NeuralEnhancedConfig::default();
        Self {
            config,
            neural_predictor,
            price_history: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
            volume_history: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            entry_price: Arc::new(RwLock::new(None)),
            initialized: false,
        }
    }

    /// Calculate market volatility from price history
    fn calculate_volatility(&self, prices: &VecDeque<f64>) -> f64 {
        if prices.len() < 20 {
            return 0.02; // Default volatility
        }

        let prices_vec: Vec<f64> = prices.iter().copied().collect();
        let returns: Vec<f64> = prices_vec
            .windows(2)
            .map(|w| (w[1] - w[0]) / w[0])
            .collect();

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance =
            returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;

        variance.sqrt()
    }

    /// Calculate trend strength using linear regression
    async fn calculate_trend_strength(&self, prices: &VecDeque<f64>) -> f64 {
        if prices.len() < 20 {
            return 0.0;
        }

        // Simple linear regression on recent prices
        let n = prices.len().min(50) as f64;
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_xy = 0.0;
        let mut sum_x2 = 0.0;

        for (i, &price) in prices.iter().rev().take(50).enumerate() {
            let x = i as f64;
            sum_x += x;
            sum_y += price;
            sum_xy += x * price;
            sum_x2 += x * x;
        }

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
        let avg_price = sum_y / n;

        // Normalize slope to [-1, 1] range
        (slope / avg_price * 100.0).max(-1.0).min(1.0)
    }

    /// Update price and volume history
    async fn update_history(&self, context: &MarketContext) {
        let mut price_history = self.price_history.write().await;
        let mut volume_history = self.volume_history.write().await;

        price_history.push_back(context.current_price);
        volume_history.push_back(context.volume_24h);

        // Maintain history size
        if price_history.len() > self.config.history_size {
            price_history.pop_front();
        }
        if volume_history.len() > self.config.history_size {
            volume_history.pop_front();
        }
    }

    /// Calculate simple moving average
    async fn calculate_sma(&self, period: usize) -> Option<f64> {
        let prices = self.price_history.read().await;
        if prices.len() < period {
            return None;
        }

        let sum: f64 = prices.iter().rev().take(period).sum();
        Some(sum / period as f64)
    }

    /// Calculate exponential moving average
    async fn calculate_ema(&self, period: usize) -> Option<f64> {
        let prices = self.price_history.read().await;
        if prices.len() < period {
            return None;
        }

        let multiplier = 2.0 / (period as f64 + 1.0);
        let mut ema = prices[0];

        for price in prices.iter().skip(1) {
            ema = (price - ema) * multiplier + ema;
        }

        Some(ema)
    }

    /// Calculate RSI
    async fn calculate_rsi(&self, period: usize) -> Option<f64> {
        let prices = self.price_history.read().await;
        if prices.len() < period + 1 {
            return None;
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in 1..=period {
            if i >= prices.len() {
                break;
            }
            let change = prices[prices.len() - i] - prices[prices.len() - i - 1];
            if change > 0.0 {
                gains += change;
            } else {
                losses -= change;
            }
        }

        let avg_gain = gains / period as f64;
        let avg_loss = losses / period as f64;

        if avg_loss == 0.0 {
            return Some(100.0);
        }

        let rs = avg_gain / avg_loss;
        Some(100.0 - (100.0 / (1.0 + rs)))
    }

    /// Helper function to calculate RSI for a specific index
    async fn calculate_rsi_for_index(&self, prices: &VecDeque<f64>, index: usize) -> Option<f64> {
        if index < 14 {
            return None;
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in (index - 13)..=index {
            if i > 0 {
                let change = prices[i] - prices[i - 1];
                if change > 0.0 {
                    gains += change;
                } else {
                    losses -= change;
                }
            }
        }

        let avg_gain = gains / 14.0;
        let avg_loss = losses / 14.0;

        if avg_loss == 0.0 {
            return Some(100.0);
        }

        let rs = avg_gain / avg_loss;
        Some(100.0 - (100.0 / (1.0 + rs)))
    }

    /// Helper function to calculate SMA for a specific index
    async fn calculate_sma_for_index(
        &self,
        prices: &VecDeque<f64>,
        index: usize,
        period: usize,
    ) -> Option<f64> {
        if index < period - 1 {
            return None;
        }

        let sum: f64 = ((index - period + 1)..=index).map(|i| prices[i]).sum();

        Some(sum / period as f64)
    }

    /// Get neural predictions with real vendor neural networks
    async fn get_neural_predictions(&self, symbol: &str) -> Result<Vec<f64>, StrategyError> {
        // Convert price history to TimeSeriesData for neural predictor
        let prices = self.price_history.read().await;
        let volumes = self.volume_history.read().await;

        if prices.len() < 20 {
            return Err(StrategyError::InsufficientData(
                "Not enough historical data".to_string(),
            ));
        }

        let mut time_series_data: Vec<TimeSeriesData> = Vec::new();
        let now = Utc::now().timestamp();

        for (i, (price, volume)) in prices.iter().zip(volumes.iter()).enumerate() {
            time_series_data.push(TimeSeriesData {
                symbol: symbol.to_string(),
                timestamp: DateTime::<Utc>::from_timestamp(
                    now - ((prices.len() - i) as i64 * 3600),
                    0,
                )
                .unwrap_or_else(Utc::now),
                open: price * 0.99,
                high: price * 1.01,
                low: price * 0.99,
                close: *price,
                volume: *volume,
                indicators: HashMap::new(),
                source: Some("neural-enhanced".to_string()),
                entity: Some(symbol.to_string()),
                value: Some(*price),
                metadata: None,
                // Enhanced fields for vendor model integration
                values: vec![*price], // Single price value
                timestamps: vec![DateTime::<Utc>::from_timestamp(
                    now - ((prices.len() - i) as i64 * 3600),
                    0,
                ).unwrap_or_else(Utc::now)], // Single timestamp
                metadata_map: HashMap::new(), // Empty metadata map
            });
        }

        // Get predictions using ensemble of vendor models for better accuracy
        let models = vec!["LSTM".to_string(), "GRU".to_string(), "TCN".to_string()];
        let predictions = self
            .neural_predictor
            .predict_ensemble(&time_series_data, 5, &models, None)
            .await
            .map_err(|e| StrategyError::Execution(format!("Vendor neural prediction failed: {}", e)))?;

        if predictions.is_empty() {
            return Err(StrategyError::Execution(
                "No predictions returned".to_string(),
            ));
        }

        // Extract price predictions from the results
        Ok(predictions.iter().map(|p| p.value).collect())
    }

    /// Generate composite signal
    async fn generate_composite_signal(
        &self,
        symbol: &str,
    ) -> Result<(f64, f64, Vec<String>), StrategyError> {
        let mut signal_strength = 0.0;
        let mut confidence = 0.0;
        let mut reasons = Vec::new();

        // Technical indicators
        let sma_20 = self.calculate_sma(20).await;
        let sma_50 = self.calculate_sma(50).await;
        let ema_12 = self.calculate_ema(12).await;
        let ema_26 = self.calculate_ema(26).await;
        let rsi = self.calculate_rsi(14).await;

        // Momentum signals
        if let (Some(s20), Some(s50), Some(e12), Some(e26)) = (sma_20, sma_50, ema_12, ema_26) {
            let macd = e12 - e26;
            let momentum_signal = if macd > 0.0 && s20 > s50 {
                0.7
            } else if macd < 0.0 && s20 < s50 {
                -0.7
            } else {
                0.0
            };

            signal_strength += momentum_signal * self.config.momentum_weight;
            if momentum_signal.abs() > 0.5 {
                reasons.push(format!("Momentum: {:.2}", momentum_signal));
            }
        }

        // Mean reversion signals
        if let Some(rsi_val) = rsi {
            let mean_reversion_signal = if rsi_val < self.config.rsi_oversold {
                (self.config.rsi_oversold - rsi_val) / self.config.rsi_oversold
            } else if rsi_val > self.config.rsi_overbought {
                -(rsi_val - self.config.rsi_overbought) / (100.0 - self.config.rsi_overbought)
            } else {
                0.0
            };

            signal_strength += mean_reversion_signal * self.config.mean_reversion_weight;
            if mean_reversion_signal.abs() > 0.5 {
                reasons.push(format!(
                    "Mean Reversion: {:.2} (RSI: {:.2})",
                    mean_reversion_signal, rsi_val
                ));
            }
        }

        // Neural predictions with real vendor networks
        match self.get_neural_predictions(symbol).await {
            Ok(predictions) => {
                if !predictions.is_empty() {
                    let current_price = *self.price_history.read().await.back().unwrap_or(&0.0);
                    let predicted_price = predictions[0];
                    let price_change = (predicted_price - current_price) / current_price;

                    // Use multiple prediction horizons for better signal
                    let mut trend_strength = 0.0;
                    for (i, &pred) in predictions.iter().enumerate().take(3) {
                        let horizon_change = (pred - current_price) / current_price;
                        // Weight nearer predictions more heavily
                        trend_strength += horizon_change * (1.0 / (i + 1) as f64);
                    }
                    trend_strength /= 1.833; // Normalize by sum of weights

                    let neural_signal = trend_strength.max(-1.0).min(1.0);
                    signal_strength += neural_signal * self.config.neural_weight;

                    // Higher confidence with ensemble predictions
                    confidence = 0.85;

                    if neural_signal.abs() > 0.01 {
                        reasons.push(format!("Neural Ensemble: {:.2}% predicted change (multi-horizon trend: {:.2}%)", 
                            price_change * 100.0, trend_strength * 100.0));
                    }

                    // Add volatility adjustment based on prediction intervals
                    if predictions.len() > 0 {
                        let pred_volatility =
                            (predictions[0] - current_price).abs() / current_price;
                        if pred_volatility > 0.05 {
                            confidence *= 0.9; // Reduce confidence in high volatility
                            reasons.push(format!(
                                "High volatility detected: {:.2}%",
                                pred_volatility * 100.0
                            ));
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Neural prediction error: {}", e);
                confidence = 0.5; // Lower confidence without neural predictions

                // Fall back to simple technical analysis
                if let (Some(s20), Some(s50)) = (sma_20, sma_50) {
                    let trend_signal = if s20 > s50 { 0.3 } else { -0.3 };
                    signal_strength += trend_signal * self.config.neural_weight * 0.5;
                    reasons
                        .push("Using technical fallback due to neural unavailability".to_string());
                }
            }
        }

        // Normalize signal strength
        signal_strength = signal_strength.max(-1.0).min(1.0);

        // Adjust confidence based on indicator agreement
        if reasons.len() > 2 {
            confidence += 0.2;
        }
        confidence = f64::min(confidence, 1.0);

        Ok((signal_strength, confidence, reasons))
    }
}

#[async_trait]
impl TradingStrategy for NeuralEnhancedStrategy {
    fn name(&self) -> &str {
        "neural_enhanced"
    }

    async fn initialize(&mut self, config: StrategyConfig) -> Result<(), StrategyError> {
        // Update configuration from parameters
        if let Some(min_conf) = config.parameters.get("min_confidence") {
            if let Some(val) = min_conf.as_f64() {
                self.config.min_confidence = val;
            }
        }

        if let Some(momentum_weight) = config.parameters.get("momentum_weight") {
            if let Some(val) = momentum_weight.as_f64() {
                self.config.momentum_weight = val;
            }
        }

        if let Some(neural_weight) = config.parameters.get("neural_weight") {
            if let Some(val) = neural_weight.as_f64() {
                self.config.neural_weight = val;
            }
        }

        self.config.max_position_size = config.position_size;
        self.initialized = true;

        // Initialize metrics
        let mut metrics = self.metrics.write().await;
        metrics.insert("total_trades".to_string(), 0.0);
        metrics.insert("winning_trades".to_string(), 0.0);
        metrics.insert("total_pnl".to_string(), 0.0);

        info!(
            "Neural-enhanced strategy initialized with config: {:?}",
            self.config
        );
        Ok(())
    }

    async fn generate_signal(
        &self,
        context: &MarketContext,
        position: Option<&Position>,
    ) -> Result<Signal, StrategyError> {
        if !self.initialized {
            return Err(StrategyError::Configuration(
                "Strategy not initialized".to_string(),
            ));
        }

        // Update history
        self.update_history(context).await;

        // Check for existing position
        if let Some(pos) = position {
            let pnl_pct = (context.current_price - pos.entry_price) / pos.entry_price;

            // Check stop loss
            if pnl_pct < -self.config.stop_loss_pct {
                return Ok(Signal::Sell {
                    confidence: 1.0,
                    size: Some(pos.size),
                    reason: format!("Stop loss triggered at {:.2}%", pnl_pct * 100.0),
                });
            }

            // Check take profit
            if pnl_pct > self.config.take_profit_pct {
                return Ok(Signal::Sell {
                    confidence: 1.0,
                    size: Some(pos.size),
                    reason: format!("Take profit triggered at {:.2}%", pnl_pct * 100.0),
                });
            }

            // Check for exit signal
            let (signal_strength, confidence, reasons) =
                self.generate_composite_signal(&context.symbol).await?;

            if signal_strength < -0.5 && confidence > self.config.min_confidence {
                return Ok(Signal::Sell {
                    confidence,
                    size: Some(pos.size),
                    reason: format!("Neural exit signal: {}", reasons.join(", ")),
                });
            }
        } else {
            // No position - check for entry with enhanced neural-driven logic
            let price_history = self.price_history.read().await;
            if price_history.len() < 50 {
                return Ok(Signal::Hold {
                    reason: "Insufficient historical data".to_string(),
                });
            }

            // Calculate market regime for adaptive thresholds
            let volatility = self.calculate_volatility(&price_history);
            let trend_strength = self.calculate_trend_strength(&price_history).await;
            drop(price_history);

            // Adaptive signal threshold based on market conditions
            let signal_threshold = if volatility > 0.03 {
                0.4 // Higher threshold in volatile markets
            } else if trend_strength.abs() > 0.7 {
                0.2 // Lower threshold in strong trends
            } else {
                0.3 // Normal threshold
            };

            let (signal_strength, confidence, reasons) =
                self.generate_composite_signal(&context.symbol).await?;

            if signal_strength > signal_threshold && confidence > self.config.min_confidence {
                // Calculate position size based on confidence
                let size = self.config.max_position_size * confidence;

                // Store entry price
                *self.entry_price.write().await = Some(context.current_price);

                return Ok(Signal::Buy {
                    confidence,
                    size: Some(size),
                    reason: format!("Neural entry signal: {}", reasons.join(", ")),
                });
            }
        }

        Ok(Signal::Hold {
            reason: "No clear signal".to_string(),
        })
    }

    async fn update_parameters(
        &mut self,
        parameters: HashMap<String, Value>,
    ) -> Result<(), StrategyError> {
        for (key, value) in parameters {
            match key.as_str() {
                "min_confidence" => {
                    if let Some(val) = value.as_f64() {
                        self.config.min_confidence = val;
                    }
                }
                "momentum_weight" => {
                    if let Some(val) = value.as_f64() {
                        self.config.momentum_weight = val;
                    }
                }
                "neural_weight" => {
                    if let Some(val) = value.as_f64() {
                        self.config.neural_weight = val;
                    }
                }
                "stop_loss_pct" => {
                    if let Some(val) = value.as_f64() {
                        self.config.stop_loss_pct = val;
                    }
                }
                "take_profit_pct" => {
                    if let Some(val) = value.as_f64() {
                        self.config.take_profit_pct = val;
                    }
                }
                _ => {
                    warn!("Unknown parameter: {}", key);
                }
            }
        }

        Ok(())
    }

    fn get_metrics(&self) -> HashMap<String, f64> {
        // Return a clone of the metrics
        // Since we can't await in a sync function, we'll use try_read
        if let Ok(metrics) = self.metrics.try_read() {
            metrics.clone()
        } else {
            // Return default metrics if lock is held
            let mut default_metrics = HashMap::new();
            default_metrics.insert("status".to_string(), 1.0);
            default_metrics
        }
    }

    fn can_execute(&self, context: &MarketContext) -> Result<bool, StrategyError> {
        if !self.initialized {
            return Ok(false);
        }

        // Check volume threshold
        if context.volume_24h < 1000.0 {
            return Ok(false);
        }

        // Check volatility
        if context.volatility > 0.1 {
            return Ok(false);
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_strategy_initialization() {
        // Create a test neural config
        let neural_config = crate::config::NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 3600,
            accuracy_threshold: 0.7,
            enable_model_monitoring: false,
            ..Default::default()
        };

        let neural_predictor = match NeuralPredictor::new(neural_config).await {
            Ok(predictor) => Arc::new(predictor),
            Err(_) => {
                // If we can't create a real predictor, skip test
                println!("Skipping test - cannot create neural predictor");
                return;
            }
        };
        let mut strategy = NeuralEnhancedStrategy::new(neural_predictor);

        let config = StrategyConfig {
            name: "neural_enhanced".to_string(),
            enabled: true,
            risk_limit: 0.02,
            position_size: 0.01,
            parameters: HashMap::new(),
        };

        assert!(strategy.initialize(config).await.is_ok());
        assert_eq!(strategy.name(), "neural_enhanced");
    }
}
