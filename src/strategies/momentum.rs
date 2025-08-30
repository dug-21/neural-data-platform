//! Momentum trading strategy
//!
//! Implements a momentum-based trading strategy that identifies trends
//! and generates signals based on price momentum indicators.

use super::{MarketContext, Position, Signal, StrategyConfig, StrategyError, TradingStrategy};
use async_trait::async_trait;
use std::collections::{HashMap, VecDeque};

/// Momentum strategy configuration
#[derive(Debug, Clone)]
pub struct MomentumConfig {
    /// Short-term moving average period
    pub fast_period: usize,
    /// Long-term moving average period
    pub slow_period: usize,
    /// RSI period
    pub rsi_period: usize,
    /// RSI overbought threshold
    pub rsi_overbought: f64,
    /// RSI oversold threshold
    pub rsi_oversold: f64,
    /// Minimum momentum threshold
    pub momentum_threshold: f64,
    /// Stop loss percentage
    pub stop_loss_pct: f64,
    /// Take profit percentage
    pub take_profit_pct: f64,
}

impl Default for MomentumConfig {
    fn default() -> Self {
        Self {
            fast_period: 12,
            slow_period: 26,
            rsi_period: 14,
            rsi_overbought: 70.0,
            rsi_oversold: 30.0,
            momentum_threshold: 0.02,
            stop_loss_pct: 0.02,
            take_profit_pct: 0.05,
        }
    }
}

/// Momentum trading strategy
pub struct MomentumStrategy {
    config: MomentumConfig,
    price_history: VecDeque<f64>,
    metrics: HashMap<String, f64>,
}

impl MomentumStrategy {
    /// Create a new momentum strategy
    pub fn new() -> Self {
        Self {
            config: MomentumConfig::default(),
            price_history: VecDeque::with_capacity(100),
            metrics: HashMap::new(),
        }
    }

    /// Calculate simple moving average
    fn calculate_sma(&self, period: usize) -> Option<f64> {
        if self.price_history.len() < period {
            return None;
        }

        let sum: f64 = self.price_history.iter().rev().take(period).sum();

        Some(sum / period as f64)
    }

    /// Calculate RSI (Relative Strength Index)
    fn calculate_rsi(&self) -> Option<f64> {
        if self.price_history.len() < self.config.rsi_period + 1 {
            return None;
        }

        let mut gains = Vec::new();
        let mut losses = Vec::new();

        for i in 1..=self.config.rsi_period {
            let current = self.price_history[self.price_history.len() - i];
            let previous = self.price_history[self.price_history.len() - i - 1];
            let change = current - previous;

            if change > 0.0 {
                gains.push(change);
                losses.push(0.0);
            } else {
                gains.push(0.0);
                losses.push(-change);
            }
        }

        let avg_gain = gains.iter().sum::<f64>() / self.config.rsi_period as f64;
        let avg_loss = losses.iter().sum::<f64>() / self.config.rsi_period as f64;

        if avg_loss == 0.0 {
            return Some(100.0);
        }

        let rs = avg_gain / avg_loss;
        let rsi = 100.0 - (100.0 / (1.0 + rs));

        Some(rsi)
    }

    /// Calculate momentum
    fn calculate_momentum(&self, period: usize) -> Option<f64> {
        if self.price_history.len() < period {
            return None;
        }

        let current = self.price_history.back()?;
        let past = self.price_history[self.price_history.len() - period];

        Some((current - past) / past)
    }

    /// Evaluate stop loss condition
    fn check_stop_loss(&self, position: &Position) -> bool {
        let pnl_pct = (position.current_price - position.entry_price) / position.entry_price;

        match position.side {
            super::PositionSide::Long => pnl_pct <= -self.config.stop_loss_pct,
            super::PositionSide::Short => pnl_pct >= self.config.stop_loss_pct,
        }
    }

    /// Evaluate take profit condition
    fn check_take_profit(&self, position: &Position) -> bool {
        let pnl_pct = (position.current_price - position.entry_price) / position.entry_price;

        match position.side {
            super::PositionSide::Long => pnl_pct >= self.config.take_profit_pct,
            super::PositionSide::Short => pnl_pct <= -self.config.take_profit_pct,
        }
    }
}

#[async_trait]
impl TradingStrategy for MomentumStrategy {
    fn name(&self) -> &str {
        "Momentum Strategy"
    }

    async fn initialize(&mut self, config: StrategyConfig) -> Result<(), StrategyError> {
        // Parse configuration parameters
        if let Some(fast_period) = config.parameters.get("fast_period") {
            self.config.fast_period = fast_period
                .as_u64()
                .ok_or_else(|| StrategyError::Configuration("Invalid fast_period".to_string()))?
                as usize;
        }

        if let Some(slow_period) = config.parameters.get("slow_period") {
            self.config.slow_period = slow_period
                .as_u64()
                .ok_or_else(|| StrategyError::Configuration("Invalid slow_period".to_string()))?
                as usize;
        }

        if let Some(rsi_period) = config.parameters.get("rsi_period") {
            self.config.rsi_period = rsi_period
                .as_u64()
                .ok_or_else(|| StrategyError::Configuration("Invalid rsi_period".to_string()))?
                as usize;
        }

        if let Some(momentum_threshold) = config.parameters.get("momentum_threshold") {
            self.config.momentum_threshold = momentum_threshold.as_f64().ok_or_else(|| {
                StrategyError::Configuration("Invalid momentum_threshold".to_string())
            })?;
        }

        // Validate configuration
        if self.config.fast_period >= self.config.slow_period {
            return Err(StrategyError::Configuration(
                "Fast period must be less than slow period".to_string(),
            ));
        }

        Ok(())
    }

    async fn generate_signal(
        &self,
        _context: &MarketContext,
        position: Option<&Position>,
    ) -> Result<Signal, StrategyError> {
        // Check if we have an open position
        if let Some(pos) = position {
            // Check stop loss
            if self.check_stop_loss(pos) {
                return Ok(Signal::Sell {
                    confidence: 1.0,
                    size: Some(pos.size),
                    reason: "Stop loss triggered".to_string(),
                });
            }

            // Check take profit
            if self.check_take_profit(pos) {
                return Ok(Signal::Sell {
                    confidence: 1.0,
                    size: Some(pos.size),
                    reason: "Take profit reached".to_string(),
                });
            }
        }

        // Calculate indicators
        let fast_sma = self.calculate_sma(self.config.fast_period);
        let slow_sma = self.calculate_sma(self.config.slow_period);
        let rsi = self.calculate_rsi();
        let momentum = self.calculate_momentum(self.config.slow_period);

        // Check if we have enough data
        if fast_sma.is_none() || slow_sma.is_none() || rsi.is_none() || momentum.is_none() {
            return Ok(Signal::Hold {
                reason: "Insufficient data for analysis".to_string(),
            });
        }

        let fast_sma = fast_sma.unwrap();
        let slow_sma = slow_sma.unwrap();
        let rsi = rsi.unwrap();
        let momentum = momentum.unwrap();

        // Generate signal based on momentum indicators
        if fast_sma > slow_sma
            && rsi < self.config.rsi_overbought
            && momentum > self.config.momentum_threshold
            && position.is_none()
        {
            return Ok(Signal::Buy {
                confidence: (momentum / self.config.momentum_threshold).min(1.0),
                size: Some(1.0),
                reason: format!(
                    "Bullish momentum: SMA crossover, RSI={:.2}, Momentum={:.2}%",
                    rsi,
                    momentum * 100.0
                ),
            });
        }

        if fast_sma < slow_sma
            && rsi > self.config.rsi_oversold
            && momentum < -self.config.momentum_threshold
            && position.is_some()
        {
            return Ok(Signal::Sell {
                confidence: (-momentum / self.config.momentum_threshold).min(1.0),
                size: Some(position.unwrap().size),
                reason: format!(
                    "Bearish momentum: SMA crossunder, RSI={:.2}, Momentum={:.2}%",
                    rsi,
                    momentum * 100.0
                ),
            });
        }

        Ok(Signal::Hold {
            reason: format!(
                "No clear momentum signal: RSI={:.2}, Momentum={:.2}%",
                rsi,
                momentum * 100.0
            ),
        })
    }

    async fn update_parameters(
        &mut self,
        parameters: HashMap<String, serde_json::Value>,
    ) -> Result<(), StrategyError> {
        let mut temp_config = self.config.clone();

        // Update parameters
        for (key, value) in parameters {
            match key.as_str() {
                "fast_period" => {
                    temp_config.fast_period = value.as_u64().ok_or_else(|| {
                        StrategyError::Configuration("Invalid fast_period".to_string())
                    })? as usize;
                }
                "slow_period" => {
                    temp_config.slow_period = value.as_u64().ok_or_else(|| {
                        StrategyError::Configuration("Invalid slow_period".to_string())
                    })? as usize;
                }
                "momentum_threshold" => {
                    temp_config.momentum_threshold = value.as_f64().ok_or_else(|| {
                        StrategyError::Configuration("Invalid momentum_threshold".to_string())
                    })?;
                }
                _ => {
                    return Err(StrategyError::Configuration(format!(
                        "Unknown parameter: {}",
                        key
                    )));
                }
            }
        }

        // Validate new configuration
        if temp_config.fast_period >= temp_config.slow_period {
            return Err(StrategyError::Configuration(
                "Fast period must be less than slow period".to_string(),
            ));
        }

        self.config = temp_config;
        Ok(())
    }

    fn get_metrics(&self) -> HashMap<String, f64> {
        self.metrics.clone()
    }

    fn can_execute(&self, _context: &MarketContext) -> Result<bool, StrategyError> {
        // Check if market conditions allow trading
        if _context.volume_24h == 0.0 {
            return Ok(false);
        }

        // Check volatility
        if _context.volatility > 0.5 {
            // Too volatile
            return Ok(false);
        }

        Ok(true)
    }
}

impl Default for MomentumStrategy {
    fn default() -> Self {
        Self::new()
    }
}
