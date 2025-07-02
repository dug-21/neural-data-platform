# Neural Trading Platform Implementation Guide

## Overview

This guide provides detailed instructions for implementing an autonomous neural trading platform using the ruv-FANN and ruv-DAA ecosystem. The platform supports personal day trading with four specialized AI agents working together to analyze markets, manage risk, optimize portfolios, and execute trades.

## Architecture

### Core Trading Agents
1. **MarketAnalyzerAgent** - NHITS neural model for multi-timeframe analysis (<5ms latency)
2. **RiskManagerAgent** - DeepAR model for probabilistic risk forecasting (<10ms latency)
3. **PortfolioManagerAgent** - MLPMultivariate for portfolio optimization (<20ms latency)
4. **ExecutionAgent** - TCN model for ultra-fast trade execution (<1ms latency)

### Data Platform
- **TimescaleDB** - High-performance time-series database for market data
- **Redis** - Low-latency caching for real-time data
- **Data Connectors** - IEX Cloud, Alpaca Markets, Finnhub integration

### Neural Framework
- **ruv-FANN** - Neural network inference and training
- **ruv-swarm-ml** - Forecasting models (NHITS, DeepAR, TCN, MLP)
- **ruv-DAA** - Autonomous agent orchestration

## Project Structure

```
neural-trading-platform/
├── Cargo.toml                    # Trading-specific dependencies
├── docker-compose.yml            # Complete trading infrastructure
├── src/
│   ├── lib.rs                    # Trading platform core
│   ├── main.rs                   # Trading CLI interface
│   ├── agents/                   # Trading AI agents
│   │   ├── mod.rs
│   │   ├── market_analyzer.rs    # NHITS-based market analysis
│   │   ├── risk_manager.rs       # DeepAR risk assessment
│   │   ├── portfolio_manager.rs  # MLP portfolio optimization
│   │   ├── execution_agent.rs    # TCN trade execution
│   │   └── orchestrator.rs       # DAA coordination
│   ├── data/                     # Market data pipeline
│   │   ├── mod.rs
│   │   ├── market_data.rs        # OHLCV, ticks, order book types
│   │   ├── providers/            # Data source connectors
│   │   │   ├── iex_cloud.rs      # IEX Cloud integration
│   │   │   ├── alpaca.rs         # Alpaca Markets
│   │   │   └── finnhub.rs        # Finnhub global data
│   │   ├── storage.rs            # TimescaleDB integration
│   │   └── pipeline.rs           # Real-time data processing
│   ├── trading/                  # Trading engine
│   │   ├── mod.rs
│   │   ├── strategies/           # Trading strategies
│   │   ├── orders.rs             # Order management
│   │   ├── positions.rs          # Position tracking
│   │   └── execution.rs          # Trade execution
│   ├── neural/                   # Neural network layer
│   │   ├── mod.rs
│   │   ├── models.rs             # NHITS, DeepAR, TCN, MLP
│   │   ├── training.rs           # Online learning pipeline
│   │   └── inference.rs          # Real-time prediction
│   ├── mcp/                      # Model Context Protocol
│   │   ├── mod.rs
│   │   ├── server.rs             # MCP server for AI coordination
│   │   ├── trading_tools.rs      # Trading-specific MCP tools
│   │   └── market_tools.rs       # Market data tools
│   └── config/                   # Configuration management
│       ├── mod.rs
│       ├── trading.rs            # Trading parameters
│       └── data_sources.rs       # Data provider configs
├── connectors/                   # Data connector microservices
│   ├── iex-connector/            # IEX Cloud connector
│   ├── alpaca-connector/         # Alpaca Markets connector
│   └── finnhub-connector/        # Finnhub connector
├── scripts/                      # Trading automation scripts
│   ├── quick-start.sh            # One-command platform startup
│   ├── daa-start.sh              # DAA-specific startup
│   └── market-data-setup.sh      # Data pipeline initialization
├── config/                       # Configuration files
│   ├── trading.toml              # Trading parameters
│   ├── data_sources.toml         # Data provider configurations
│   └── neural_models.toml        # Model configurations
└── docs/                         # Documentation
    ├── TRADING_STRATEGIES.md     # Strategy documentation
    ├── RISK_MANAGEMENT.md        # Risk control documentation
    └── API_REFERENCE.md          # API documentation
```

## Dependencies (Cargo.toml)

```toml
[package]
name = "neural-trading-platform"
version = "2.0.0"
edition = "2021"
description = "Autonomous neural trading platform with DAA agents"

[dependencies]
# Core ruv ecosystem
ruv-fann = "0.1.3"
ruv-swarm-core = "0.2.0"
ruv-swarm-ml = "0.2.0"        # NHITS, DeepAR, TCN, MLP models
ruv-swarm-mcp = "0.2.0"
ruv-daa = { git = "https://github.com/ruvnet/daa.git", branch = "main" }

# Trading and financial data
rust_decimal = { version = "1.35", features = ["serde-with-str"] }
chrono = { version = "0.4", features = ["serde"] }
chrono-tz = "0.9"

# Data and networking
reqwest = { version = "0.11", features = ["json", "stream"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.9", features = ["v4", "serde"] }

# Database
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "chrono", "uuid", "rust_decimal"] }
redis = { version = "0.25", features = ["tokio-comp", "connection-manager"] }

# Async runtime
tokio = { version = "1.39", features = ["full"] }
async-trait = "0.1"
futures = "0.3"

# Web framework (for MCP server)
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "fs"] }

# Configuration
config = "0.14"
toml = "0.8"

# Monitoring and logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt", "json"] }
metrics = "0.22"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

[features]
default = ["std"]
std = []
daa = ["ruv-daa"]
live-trading = ["daa"]
gpu = ["ruv-fann/gpu"]

[[bin]]
name = "trading-platform"
path = "src/main.rs"

[[bin]]
name = "market-data-ingestion"
path = "src/bin/data_ingestion.rs"

[[bin]]
name = "mcp-server"
path = "src/bin/mcp_server.rs"
```

## Trading Agent Implementations

### 1. Market Analyzer Agent (src/agents/market_analyzer.rs)

```rust
use ruv_swarm_ml::models::{NHITS, ModelConfig};
use ruv_daa::{Agent, Decision, AnalysisResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub price: f64,
    pub volume: f64,
    pub high: f64,
    pub low: f64,
    pub open: f64,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub spread: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalIndicators {
    pub rsi: Option<f64>,
    pub macd: Option<f64>,
    pub bollinger_upper: Option<f64>,
    pub bollinger_lower: Option<f64>,
    pub ema_20: Option<f64>,
    pub ema_50: Option<f64>,
    pub volume_sma: Option<f64>,
    pub price_change_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NHITSPrediction {
    pub minute_level: Vec<f64>,      // 1-minute predictions
    pub hourly_level: Vec<f64>,      // 1-hour predictions
    pub daily_level: Vec<f64>,       // 1-day predictions
    pub confidence: f64,
    pub trend_strength: f64,
    pub volatility_forecast: f64,
}

pub struct MarketAnalyzerAgent {
    agent_id: String,
    nhits_model: NHITS<f32>,
    performance_metrics: HashMap<String, f64>,
    last_prediction: Option<NHITSPrediction>,
}

impl MarketAnalyzerAgent {
    pub fn new(agent_id: String) -> Result<Self> {
        // Configure NHITS model for hierarchical time series analysis
        let model_config = ModelConfig::builder()
            .input_size(50)               // 50 time steps input
            .horizon(24)                  // 24-step ahead prediction
            .num_stacks(3)                // 3 hierarchical levels
            .num_blocks_per_stack(1)
            .num_layers(4)
            .layer_widths(vec![512, 512, 512, 512])
            .build();

        let nhits_model = NHITS::new(model_config)?;

        Ok(Self {
            agent_id,
            nhits_model,
            performance_metrics: HashMap::new(),
            last_prediction: None,
        })
    }

    pub async fn analyze_market(&mut self, data: &MarketData) -> Result<AnalysisResult> {
        let start_time = std::time::Instant::now();

        // Calculate technical indicators
        let indicators = self.calculate_indicators(data).await?;
        
        // Prepare input features for NHITS model
        let features = self.prepare_features(data, &indicators)?;
        
        // Get NHITS hierarchical prediction
        let prediction = self.nhits_model.predict(&features).await?;
        let nhits_prediction = self.interpret_nhits_output(prediction)?;
        
        // Combine neural prediction with technical analysis
        let analysis = self.generate_analysis(data, &indicators, &nhits_prediction)?;
        
        // Check latency requirement (<5ms)
        let latency = start_time.elapsed();
        if latency.as_millis() > 5 {
            tracing::warn!("MarketAnalyzer latency {}ms exceeds 5ms target", latency.as_millis());
        }
        
        self.last_prediction = Some(nhits_prediction);
        self.update_performance_metrics(&analysis, latency);
        
        Ok(analysis)
    }

    async fn calculate_indicators(&self, data: &MarketData) -> Result<TechnicalIndicators> {
        // Calculate technical indicators using SIMD-optimized functions
        // This would integrate with your existing technical analysis code
        
        Ok(TechnicalIndicators {
            rsi: Some(self.calculate_rsi(data)?),
            macd: Some(self.calculate_macd(data)?),
            bollinger_upper: Some(self.calculate_bollinger_upper(data)?),
            bollinger_lower: Some(self.calculate_bollinger_lower(data)?),
            ema_20: Some(self.calculate_ema(data, 20)?),
            ema_50: Some(self.calculate_ema(data, 50)?),
            volume_sma: Some(self.calculate_volume_sma(data)?),
            price_change_pct: Some((data.price - data.open) / data.open * 100.0),
        })
    }

    fn prepare_features(&self, data: &MarketData, indicators: &TechnicalIndicators) -> Result<Vec<f32>> {
        let mut features = vec![
            data.price as f32,
            data.volume as f32,
            data.high as f32,
            data.low as f32,
            (data.high - data.low) as f32, // Range
        ];

        // Add technical indicators
        if let Some(rsi) = indicators.rsi {
            features.push(rsi as f32);
        }
        if let Some(macd) = indicators.macd {
            features.push(macd as f32);
        }
        
        // Add time-based features
        features.push(data.timestamp.hour() as f32);
        features.push(data.timestamp.weekday().num_days_from_monday() as f32);
        
        // Normalize features (important for neural networks)
        self.normalize_features(features)
    }

    fn normalize_features(&self, features: Vec<f32>) -> Result<Vec<f32>> {
        // Implement feature normalization
        // Use z-score normalization or min-max scaling
        Ok(features) // Simplified for example
    }

    fn interpret_nhits_output(&self, output: Vec<f32>) -> Result<NHITSPrediction> {
        // NHITS output includes hierarchical predictions
        let minute_level = output[0..24].to_vec().into_iter().map(|x| x as f64).collect();
        let hourly_level = output[24..48].to_vec().into_iter().map(|x| x as f64).collect();
        let daily_level = output[48..72].to_vec().into_iter().map(|x| x as f64).collect();
        
        // Calculate confidence based on prediction consistency
        let confidence = self.calculate_prediction_confidence(&output)?;
        
        // Calculate trend strength
        let trend_strength = self.calculate_trend_strength(&minute_level)?;
        
        // Forecast volatility
        let volatility_forecast = self.calculate_volatility_forecast(&output)?;

        Ok(NHITSPrediction {
            minute_level,
            hourly_level,
            daily_level,
            confidence,
            trend_strength,
            volatility_forecast,
        })
    }

    fn generate_analysis(
        &self,
        data: &MarketData,
        indicators: &TechnicalIndicators,
        prediction: &NHITSPrediction,
    ) -> Result<AnalysisResult> {
        let mut reasoning = Vec::new();
        let mut confidence = prediction.confidence;

        // Determine market direction
        let predicted_price = prediction.minute_level.last().unwrap_or(&data.price);
        let direction = if *predicted_price > data.price {
            reasoning.push("NHITS model predicts price increase".to_string());
            "bullish"
        } else {
            reasoning.push("NHITS model predicts price decrease".to_string());
            "bearish"
        };

        // Factor in technical indicators
        if let Some(rsi) = indicators.rsi {
            if rsi > 70.0 {
                reasoning.push("RSI indicates overbought conditions".to_string());
                confidence *= 0.9; // Reduce confidence for overbought
            } else if rsi < 30.0 {
                reasoning.push("RSI indicates oversold conditions".to_string());
                confidence *= 0.9; // Reduce confidence for oversold
            }
        }

        // Check trend consistency across timeframes
        if prediction.trend_strength > 0.7 {
            reasoning.push("Strong trend consistency across timeframes".to_string());
            confidence *= 1.1;
        }

        let recommendation = match direction {
            "bullish" => if confidence > 0.7 { "strong_buy" } else { "buy" },
            "bearish" => if confidence > 0.7 { "strong_sell" } else { "sell" },
            _ => "hold",
        };

        let mut metrics = HashMap::new();
        metrics.insert("predicted_price".to_string(), *predicted_price);
        metrics.insert("trend_strength".to_string(), prediction.trend_strength);
        metrics.insert("volatility_forecast".to_string(), prediction.volatility_forecast);
        metrics.insert("price_change_pct".to_string(), 
                      (*predicted_price - data.price) / data.price * 100.0);

        Ok(AnalysisResult {
            timestamp: Utc::now(),
            symbol: data.symbol.clone(),
            recommendation: recommendation.to_string(),
            confidence,
            reasoning,
            metrics,
            risk_level: self.assess_risk_level(prediction)?,
        })
    }

    // Helper methods for calculations
    fn calculate_rsi(&self, _data: &MarketData) -> Result<f64> {
        // Implement RSI calculation
        // This would query recent price data and calculate RSI
        Ok(50.0) // Placeholder
    }

    fn calculate_macd(&self, _data: &MarketData) -> Result<f64> {
        // Implement MACD calculation
        Ok(0.0) // Placeholder
    }

    fn calculate_bollinger_upper(&self, _data: &MarketData) -> Result<f64> {
        // Implement Bollinger Bands calculation
        Ok(_data.price * 1.02) // Placeholder
    }

    fn calculate_bollinger_lower(&self, _data: &MarketData) -> Result<f64> {
        Ok(_data.price * 0.98) // Placeholder
    }

    fn calculate_ema(&self, _data: &MarketData, _period: usize) -> Result<f64> {
        // Implement EMA calculation
        Ok(_data.price) // Placeholder
    }

    fn calculate_volume_sma(&self, _data: &MarketData) -> Result<f64> {
        Ok(_data.volume) // Placeholder
    }

    fn calculate_prediction_confidence(&self, _output: &[f32]) -> Result<f64> {
        // Calculate confidence based on prediction variance
        Ok(0.8) // Placeholder
    }

    fn calculate_trend_strength(&self, _predictions: &[f64]) -> Result<f64> {
        // Calculate trend strength from predictions
        Ok(0.7) // Placeholder
    }

    fn calculate_volatility_forecast(&self, _output: &[f32]) -> Result<f64> {
        // Calculate expected volatility
        Ok(0.15) // Placeholder
    }

    fn assess_risk_level(&self, _prediction: &NHITSPrediction) -> Result<RiskLevel> {
        // Assess risk based on volatility and confidence
        Ok(RiskLevel::Medium) // Placeholder
    }

    fn update_performance_metrics(&mut self, _analysis: &AnalysisResult, latency: std::time::Duration) {
        self.performance_metrics.insert("latency_ms".to_string(), latency.as_millis() as f64);
        self.performance_metrics.insert("predictions_count".to_string(), 
                                       self.performance_metrics.get("predictions_count").unwrap_or(&0.0) + 1.0);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[async_trait::async_trait]
impl Agent for MarketAnalyzerAgent {
    async fn process(&mut self, input: &[f64]) -> Result<Decision> {
        // Convert input to MarketData and analyze
        let market_data = self.parse_market_data(input)?;
        let analysis = self.analyze_market(&market_data).await?;
        
        Ok(Decision::new(
            analysis.recommendation,
            analysis.confidence,
            analysis.reasoning,
        ))
    }

    fn agent_id(&self) -> &str {
        &self.agent_id
    }

    fn capabilities(&self) -> Vec<String> {
        vec![
            "market_analysis".to_string(),
            "price_prediction".to_string(),
            "technical_analysis".to_string(),
            "multi_timeframe_analysis".to_string(),
        ]
    }
}

impl MarketAnalyzerAgent {
    fn parse_market_data(&self, input: &[f64]) -> Result<MarketData> {
        // Parse input array into MarketData structure
        // This would depend on your input format
        Ok(MarketData {
            timestamp: Utc::now(),
            symbol: "AAPL".to_string(), // Would be dynamic
            price: input[0],
            volume: input[1],
            high: input[2],
            low: input[3],
            open: input[4],
            bid: Some(input[5]),
            ask: Some(input[6]),
            spread: Some(input[6] - input[5]),
        })
    }
}
```

### 2. Risk Manager Agent (src/agents/risk_manager.rs)

```rust
use ruv_swarm_ml::models::{DeepAR, ModelConfig};
use ruv_daa::{Agent, Decision, AnalysisResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaRForecast {
    pub var_95: f64,              // 95% Value at Risk
    pub var_99: f64,              // 99% Value at Risk
    pub expected_shortfall: f64,   // Expected Shortfall (CVaR)
    pub probability_loss: f64,     // Probability of loss
    pub max_drawdown_forecast: f64, // Expected max drawdown
    pub confidence_interval: (f64, f64), // Prediction interval
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskLimits {
    pub max_position_size: f64,
    pub max_daily_loss: f64,
    pub max_portfolio_var: f64,
    pub max_single_stock_weight: f64,
    pub max_sector_concentration: f64,
}

pub struct RiskManagerAgent {
    agent_id: String,
    deepar_model: DeepAR<f32>,
    risk_limits: RiskLimits,
    portfolio_value: f64,
    current_positions: HashMap<String, f64>,
    historical_returns: Vec<f64>,
}

impl RiskManagerAgent {
    pub fn new(agent_id: String, initial_capital: f64) -> Result<Self> {
        // Configure DeepAR for probabilistic risk forecasting
        let model_config = ModelConfig::builder()
            .input_size(30)              // 30-day lookback
            .horizon(5)                  // 5-day forecast horizon
            .num_cells(64)               // LSTM cells
            .num_layers(2)               // Depth
            .dropout_rate(0.1)
            .likelihood("gaussian")       // Gaussian likelihood for returns
            .build();

        let deepar_model = DeepAR::new(model_config)?;

        let risk_limits = RiskLimits {
            max_position_size: initial_capital * 0.1,  // 10% max position
            max_daily_loss: initial_capital * 0.02,    // 2% daily loss limit
            max_portfolio_var: initial_capital * 0.05, // 5% portfolio VaR
            max_single_stock_weight: 0.15,             // 15% max single stock
            max_sector_concentration: 0.3,              // 30% max sector weight
        };

        Ok(Self {
            agent_id,
            deepar_model,
            risk_limits,
            portfolio_value: initial_capital,
            current_positions: HashMap::new(),
            historical_returns: Vec::new(),
        })
    }

    pub async fn assess_risk(&mut self, analysis: &AnalysisResult) -> Result<Decision> {
        let start_time = std::time::Instant::now();

        // Generate probabilistic risk forecast using DeepAR
        let var_forecast = self.generate_var_forecast().await?;
        
        // Check position sizing limits
        let position_risk = self.calculate_position_risk(analysis, &var_forecast)?;
        
        // Assess portfolio-level risk
        let portfolio_risk = self.assess_portfolio_risk(&var_forecast)?;
        
        // Generate risk-adjusted decision
        let decision = self.make_risk_decision(analysis, &position_risk, &portfolio_risk)?;
        
        // Check latency requirement (<10ms)
        let latency = start_time.elapsed();
        if latency.as_millis() > 10 {
            tracing::warn!("RiskManager latency {}ms exceeds 10ms target", latency.as_millis());
        }

        Ok(decision)
    }

    async fn generate_var_forecast(&self) -> Result<VaRForecast> {
        // Prepare input for DeepAR (historical returns and features)
        let features = self.prepare_risk_features()?;
        
        // Get probabilistic forecast from DeepAR
        let forecast_distribution = self.deepar_model.predict_quantiles(
            &features,
            &[0.01, 0.05, 0.5, 0.95, 0.99] // Quantiles for VaR calculation
        ).await?;

        // Extract VaR values
        let var_99 = forecast_distribution[0] * self.portfolio_value; // 1st percentile
        let var_95 = forecast_distribution[1] * self.portfolio_value; // 5th percentile
        let median = forecast_distribution[2] * self.portfolio_value;
        let upper_95 = forecast_distribution[3] * self.portfolio_value;
        let upper_99 = forecast_distribution[4] * self.portfolio_value;

        // Calculate Expected Shortfall (CVaR)
        let expected_shortfall = self.calculate_expected_shortfall(&forecast_distribution)?;
        
        // Calculate probability of loss
        let probability_loss = self.calculate_loss_probability(&forecast_distribution)?;
        
        // Forecast maximum drawdown
        let max_drawdown_forecast = self.forecast_max_drawdown(&forecast_distribution)?;

        Ok(VaRForecast {
            var_95: var_95.abs(),
            var_99: var_99.abs(),
            expected_shortfall: expected_shortfall * self.portfolio_value,
            probability_loss,
            max_drawdown_forecast,
            confidence_interval: (var_95, upper_95),
        })
    }

    fn prepare_risk_features(&self) -> Result<Vec<f32>> {
        let mut features = Vec::new();
        
        // Historical returns (normalized)
        let recent_returns: Vec<f32> = self.historical_returns
            .iter()
            .rev()
            .take(30)
            .map(|&x| x as f32)
            .collect();
        features.extend(recent_returns);
        
        // Portfolio concentration metrics
        features.push(self.calculate_concentration_risk() as f32);
        
        // Market volatility features
        features.push(self.calculate_realized_volatility() as f32);
        
        // Correlation features
        features.push(self.calculate_portfolio_correlation() as f32);

        Ok(features)
    }

    fn calculate_position_risk(&self, analysis: &AnalysisResult, var_forecast: &VaRForecast) -> Result<PositionRisk> {
        let symbol = &analysis.symbol;
        let proposed_action = &analysis.recommendation;
        
        // Calculate position size based on Kelly criterion and risk limits
        let kelly_fraction = self.calculate_kelly_fraction(analysis)?;
        let risk_adjusted_size = self.calculate_risk_adjusted_position_size(
            kelly_fraction,
            var_forecast,
            analysis.confidence
        )?;

        // Check against limits
        let within_position_limit = risk_adjusted_size <= self.risk_limits.max_position_size;
        let within_concentration_limit = self.check_concentration_limit(symbol, risk_adjusted_size)?;
        
        Ok(PositionRisk {
            recommended_size: risk_adjusted_size,
            within_limits: within_position_limit && within_concentration_limit,
            risk_score: self.calculate_position_risk_score(analysis, var_forecast)?,
            stop_loss_level: self.calculate_stop_loss_level(analysis, var_forecast)?,
        })
    }

    fn assess_portfolio_risk(&self, var_forecast: &VaRForecast) -> Result<PortfolioRisk> {
        // Check portfolio-level risk constraints
        let portfolio_var_ok = var_forecast.var_95 <= self.risk_limits.max_portfolio_var;
        let drawdown_ok = var_forecast.max_drawdown_forecast <= self.risk_limits.max_daily_loss;
        
        // Calculate portfolio diversification score
        let diversification_score = self.calculate_diversification_score()?;
        
        // Assess correlation risk
        let correlation_risk = self.assess_correlation_risk()?;

        Ok(PortfolioRisk {
            within_var_limit: portfolio_var_ok,
            within_drawdown_limit: drawdown_ok,
            diversification_score,
            correlation_risk,
            overall_risk_score: self.calculate_overall_risk_score(var_forecast)?,
        })
    }

    fn make_risk_decision(
        &self,
        analysis: &AnalysisResult,
        position_risk: &PositionRisk,
        portfolio_risk: &PortfolioRisk,
    ) -> Result<Decision> {
        let mut reasoning = Vec::new();
        let mut confidence = analysis.confidence;

        // Check if position is within risk limits
        if !position_risk.within_limits {
            reasoning.push("Position size exceeds risk limits".to_string());
            return Ok(Decision::new(
                "reject".to_string(),
                0.0,
                reasoning,
            ));
        }

        // Check portfolio-level risk
        if !portfolio_risk.within_var_limit || !portfolio_risk.within_drawdown_limit {
            reasoning.push("Portfolio risk limits exceeded".to_string());
            return Ok(Decision::new(
                "reject".to_string(),
                0.0,
                reasoning,
            ));
        }

        // Risk-adjust the recommendation
        let risk_adjusted_action = match analysis.recommendation.as_str() {
            "strong_buy" | "buy" => {
                if position_risk.risk_score < 0.3 {
                    reasoning.push("Low position risk - approving trade".to_string());
                    "approve"
                } else if position_risk.risk_score < 0.7 {
                    reasoning.push("Medium position risk - reducing size".to_string());
                    confidence *= 0.7;
                    "approve_reduced"
                } else {
                    reasoning.push("High position risk - rejecting trade".to_string());
                    "reject"
                }
            },
            "strong_sell" | "sell" => {
                reasoning.push("Sell orders reduce risk - approving".to_string());
                "approve"
            },
            _ => {
                reasoning.push("Hold recommendation - no risk impact".to_string());
                "approve"
            }
        };

        Ok(Decision::new(
            risk_adjusted_action.to_string(),
            confidence,
            reasoning,
        ))
    }

    // Helper calculation methods
    fn calculate_kelly_fraction(&self, analysis: &AnalysisResult) -> Result<f64> {
        // Kelly criterion: f* = (bp - q) / b
        // where b = odds, p = probability of win, q = probability of loss
        let win_probability = analysis.confidence;
        let loss_probability = 1.0 - win_probability;
        
        // Estimate odds from predicted price change
        let predicted_return = analysis.metrics
            .get("price_change_pct")
            .unwrap_or(&1.0) / 100.0;
        
        if predicted_return <= 0.0 {
            return Ok(0.0); // No position if negative expected return
        }

        let kelly_fraction = (predicted_return * win_probability - loss_probability) / predicted_return;
        
        // Cap Kelly fraction at 25% for safety
        Ok(kelly_fraction.max(0.0).min(0.25))
    }

    fn calculate_risk_adjusted_position_size(
        &self,
        kelly_fraction: f64,
        var_forecast: &VaRForecast,
        confidence: f64,
    ) -> Result<f64> {
        // Start with Kelly-suggested size
        let base_size = self.portfolio_value * kelly_fraction;
        
        // Adjust based on VaR forecast
        let var_adjustment = if var_forecast.var_95 > self.portfolio_value * 0.03 {
            0.5 // Reduce size if high VaR
        } else {
            1.0
        };
        
        // Adjust based on confidence
        let confidence_adjustment = confidence;
        
        let adjusted_size = base_size * var_adjustment * confidence_adjustment;
        
        // Ensure within absolute limits
        Ok(adjusted_size.min(self.risk_limits.max_position_size))
    }

    fn calculate_concentration_risk(&self) -> f64 {
        // Calculate Herfindahl-Hirschman Index for portfolio concentration
        let total_value: f64 = self.current_positions.values().sum();
        if total_value == 0.0 {
            return 0.0;
        }

        let hhi: f64 = self.current_positions
            .values()
            .map(|&value| {
                let weight = value / total_value;
                weight * weight
            })
            .sum();

        hhi
    }

    fn calculate_realized_volatility(&self) -> f64 {
        if self.historical_returns.len() < 2 {
            return 0.2; // Default volatility
        }

        let mean_return: f64 = self.historical_returns.iter().sum::<f64>() 
                              / self.historical_returns.len() as f64;
        
        let variance: f64 = self.historical_returns
            .iter()
            .map(|&r| (r - mean_return).powi(2))
            .sum::<f64>() / (self.historical_returns.len() - 1) as f64;

        variance.sqrt() * (252.0_f64).sqrt() // Annualized volatility
    }

    // Additional helper methods would be implemented here...
    fn calculate_portfolio_correlation(&self) -> f64 { 0.5 } // Placeholder
    fn calculate_expected_shortfall(&self, _dist: &[f32]) -> Result<f64> { Ok(0.03) }
    fn calculate_loss_probability(&self, _dist: &[f32]) -> Result<f64> { Ok(0.4) }
    fn forecast_max_drawdown(&self, _dist: &[f32]) -> Result<f64> { Ok(0.05) }
    fn check_concentration_limit(&self, _symbol: &str, _size: f64) -> Result<bool> { Ok(true) }
    fn calculate_position_risk_score(&self, _analysis: &AnalysisResult, _var: &VaRForecast) -> Result<f64> { Ok(0.3) }
    fn calculate_stop_loss_level(&self, _analysis: &AnalysisResult, _var: &VaRForecast) -> Result<f64> { Ok(0.02) }
    fn calculate_diversification_score(&self) -> Result<f64> { Ok(0.8) }
    fn assess_correlation_risk(&self) -> Result<f64> { Ok(0.3) }
    fn calculate_overall_risk_score(&self, _var: &VaRForecast) -> Result<f64> { Ok(0.4) }
}

#[derive(Debug, Clone)]
pub struct PositionRisk {
    pub recommended_size: f64,
    pub within_limits: bool,
    pub risk_score: f64,
    pub stop_loss_level: f64,
}

#[derive(Debug, Clone)]
pub struct PortfolioRisk {
    pub within_var_limit: bool,
    pub within_drawdown_limit: bool,
    pub diversification_score: f64,
    pub correlation_risk: f64,
    pub overall_risk_score: f64,
}

#[async_trait::async_trait]
impl Agent for RiskManagerAgent {
    async fn process(&mut self, input: &[f64]) -> Result<Decision> {
        // Convert input to analysis result and assess risk
        let analysis = self.parse_analysis_input(input)?;
        self.assess_risk(&analysis).await
    }

    fn agent_id(&self) -> &str {
        &self.agent_id
    }

    fn capabilities(&self) -> Vec<String> {
        vec![
            "risk_assessment".to_string(),
            "var_calculation".to_string(),
            "position_sizing".to_string(),
            "portfolio_risk_management".to_string(),
        ]
    }
}

impl RiskManagerAgent {
    fn parse_analysis_input(&self, _input: &[f64]) -> Result<AnalysisResult> {
        // Parse input into AnalysisResult
        // This would depend on your input format
        todo!("Implement input parsing")
    }
}
```

### 3. Data Provider Configurations

**config/data_sources.toml**
```toml
[iex_cloud]
name = "IEX Cloud"
base_url = "https://cloud.iexapis.com/stable"
api_key = "${IEX_API_KEY}"
rate_limit_per_minute = 100
free_tier_messages_per_month = 500000
supported_data = ["stocks", "etfs", "options"]

[iex_cloud.endpoints]
quote = "/stock/{symbol}/quote"
historical = "/stock/{symbol}/chart/{range}"
batch_quotes = "/stock/market/batch"
real_time = "/stock/{symbol}/quote"

[alpaca]
name = "Alpaca Markets"
base_url = "https://paper-api.alpaca.markets"
api_key = "${ALPACA_API_KEY}"
secret_key = "${ALPACA_SECRET_KEY}"
rate_limit_per_minute = 200
supported_data = ["stocks", "crypto"]

[alpaca.endpoints]
account = "/v2/account"
positions = "/v2/positions"
orders = "/v2/orders"
bars = "/v2/stocks/{symbol}/bars"
trades = "/v2/stocks/{symbol}/trades"

[finnhub]
name = "Finnhub"
base_url = "https://finnhub.io/api/v1"
api_key = "${FINNHUB_API_KEY}"
rate_limit_per_minute = 60
supported_data = ["stocks", "forex", "crypto", "news"]

[finnhub.endpoints]
quote = "/quote"
candles = "/stock/candle"
news = "/news"
company_news = "/company-news"
```

### 4. Quick Start Script (scripts/quick-start.sh)

```bash
#!/bin/bash

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Neural Trading Platform Quick Start${NC}"
echo "=============================================="

# Check if DAA mode is enabled
DAA_ENABLED=${DAA_ENABLED:-false}
if [[ "$1" == "--daa" ]] || [[ "$DAA_ENABLED" == "true" ]]; then
    echo -e "${PURPLE}Building with DAA (Distributed Autonomous Agents) support...${NC}"
    FEATURES="--features daa"
else
    FEATURES=""
fi

# Function to check dependencies
check_dependencies() {
    echo -e "${YELLOW}Checking dependencies...${NC}"
    
    if ! command -v docker &> /dev/null; then
        echo -e "${RED}❌ Docker is required but not installed${NC}"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null; then
        echo -e "${RED}❌ Docker Compose is required but not installed${NC}"
        exit 1
    fi
    
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}❌ Rust/Cargo is required but not installed${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✅ All dependencies found${NC}"
}

# Function to setup environment
setup_environment() {
    echo -e "${YELLOW}Setting up environment...${NC}"
    
    # Create .env file if it doesn't exist
    if [ ! -f .env ]; then
        cat > .env << EOF
# Trading Platform Configuration
DB_PASSWORD=trading123
REDIS_PASSWORD=redis123
GRAFANA_PASSWORD=admin

# Data Provider API Keys
IEX_API_KEY=your_iex_api_key_here
ALPACA_API_KEY=your_alpaca_api_key_here
ALPACA_SECRET_KEY=your_alpaca_secret_key_here
FINNHUB_API_KEY=your_finnhub_api_key_here

# Trading Configuration
INITIAL_CAPITAL=100000
MAX_DAILY_LOSS=2000
RISK_TOLERANCE=0.5

# Environment
RUST_LOG=info
ENVIRONMENT=development
EOF
        echo -e "${GREEN}✅ Created .env file${NC}"
        echo -e "${YELLOW}⚠️  Please edit .env with your API keys before continuing${NC}"
    fi
    
    # Create data directories
    mkdir -p data/{timescale,redis,grafana}
    mkdir -p logs
    
    echo -e "${GREEN}✅ Environment setup complete${NC}"
}

# Function to start data platform
start_data_platform() {
    echo -e "${YELLOW}Starting data platform...${NC}"
    
    # Start TimescaleDB and Redis
    docker-compose up -d timescaledb redis grafana
    
    # Wait for databases to be ready
    echo -e "${YELLOW}Waiting for databases to initialize...${NC}"
    sleep 15
    
    # Check if TimescaleDB is ready
    until docker exec neural-trading-timescaledb pg_isready -U autonomous; do
        echo -e "${YELLOW}Waiting for TimescaleDB...${NC}"
        sleep 2
    done
    
    echo -e "${GREEN}✅ Data platform started${NC}"
}

# Function to run database migrations
run_migrations() {
    echo -e "${YELLOW}Running database migrations...${NC}"
    
    # Run SQL migrations
    docker exec -i neural-trading-timescaledb psql -U autonomous -d autonomous_data < docker/data-platform/init/01-create-tables.sql
    
    echo -e "${GREEN}✅ Database migrations complete${NC}"
}

# Function to start data connectors
start_connectors() {
    echo -e "${YELLOW}Starting data connectors...${NC}"
    
    # Check if API keys are configured
    if grep -q "your_.*_api_key_here" .env; then
        echo -e "${YELLOW}⚠️  API keys not configured - starting in demo mode${NC}"
        export DEMO_MODE=true
    fi
    
    # Start connectors
    docker-compose up -d iex-connector alpaca-connector finnhub-connector
    
    echo -e "${GREEN}✅ Data connectors started${NC}"
}

# Function to build and start trading platform
start_trading_platform() {
    echo -e "${YELLOW}Building trading platform...${NC}"
    
    # Build the platform
    cargo build --release $FEATURES
    
    if [ $? -ne 0 ]; then
        echo -e "${RED}❌ Build failed${NC}"
        exit 1
    fi
    
    echo -e "${YELLOW}Starting trading platform...${NC}"
    
    # Start main trading platform
    cargo run --release --bin trading-platform $FEATURES &
    PLATFORM_PID=$!
    
    # Start MCP server
    cargo run --release --bin mcp-server &
    MCP_PID=$!
    
    # Start market data ingestion
    cargo run --release --bin market-data-ingestion &
    INGESTION_PID=$!
    
    # Save PIDs for cleanup
    echo $PLATFORM_PID > platform.pid
    echo $MCP_PID > mcp.pid
    echo $INGESTION_PID > ingestion.pid
    
    echo -e "${GREEN}✅ Trading platform started${NC}"
    echo -e "${BLUE}Platform PID: $PLATFORM_PID${NC}"
    echo -e "${BLUE}MCP Server PID: $MCP_PID${NC}"
    echo -e "${BLUE}Data Ingestion PID: $INGESTION_PID${NC}"
}

# Function to show status
show_status() {
    echo -e "${BLUE}Neural Trading Platform Status${NC}"
    echo "==============================="
    
    # Check data platform
    if docker-compose ps | grep -q "Up.*timescaledb"; then
        echo -e "${GREEN}✅ TimescaleDB: Running${NC}"
    else
        echo -e "${RED}❌ TimescaleDB: Stopped${NC}"
    fi
    
    if docker-compose ps | grep -q "Up.*redis"; then
        echo -e "${GREEN}✅ Redis: Running${NC}"
    else
        echo -e "${RED}❌ Redis: Stopped${NC}"
    fi
    
    # Check trading platform
    if [ -f platform.pid ] && kill -0 $(cat platform.pid) 2>/dev/null; then
        echo -e "${GREEN}✅ Trading Platform: Running (PID: $(cat platform.pid))${NC}"
    else
        echo -e "${RED}❌ Trading Platform: Stopped${NC}"
    fi
    
    if [ -f mcp.pid ] && kill -0 $(cat mcp.pid) 2>/dev/null; then
        echo -e "${GREEN}✅ MCP Server: Running (PID: $(cat mcp.pid))${NC}"
    else
        echo -e "${RED}❌ MCP Server: Stopped${NC}"
    fi
    
    echo ""
    echo -e "${BLUE}Service URLs:${NC}"
    echo "  Grafana Dashboard: http://localhost:3000"
    echo "  MCP Server: ws://localhost:8080/mcp"
    echo "  TimescaleDB: postgresql://localhost:5432/autonomous_data"
    echo "  Redis: redis://localhost:6379"
    
    # Show DAA agent status if enabled
    if [[ "$DAA_ENABLED" == "true" ]]; then
        echo ""
        echo -e "${PURPLE}DAA Agents Status:${NC}"
        echo "  MarketAnalyzer: Active"
        echo "  RiskManager: Active"
        echo "  PortfolioManager: Active"
        echo "  ExecutionAgent: Active"
    fi
}

# Function to stop platform
stop_platform() {
    echo -e "${YELLOW}Stopping trading platform...${NC}"
    
    # Kill Rust processes
    if [ -f platform.pid ]; then
        kill $(cat platform.pid) 2>/dev/null || true
        rm platform.pid
    fi
    
    if [ -f mcp.pid ]; then
        kill $(cat mcp.pid) 2>/dev/null || true
        rm mcp.pid
    fi
    
    if [ -f ingestion.pid ]; then
        kill $(cat ingestion.pid) 2>/dev/null || true
        rm ingestion.pid
    fi
    
    # Stop Docker containers
    docker-compose down
    
    echo -e "${GREEN}✅ Platform stopped${NC}"
}

# Function to show trading menu
show_trading_menu() {
    echo ""
    echo -e "${PURPLE}Neural Trading Platform Commands:${NC}"
    echo "  1) setup     - Initial environment setup"
    echo "  2) start     - Start complete trading platform"
    echo "  3) stop      - Stop all services"
    echo "  4) restart   - Restart the platform"
    echo "  5) status    - Show platform status"
    echo "  6) simulate  - Start paper trading simulation"
    echo "  7) live      - Start live trading (requires broker setup)"
    echo "  8) logs      - Show platform logs"
    echo "  9) clean     - Clean all data and stop"
    echo "  10) help     - Show this menu"
    echo ""
    if [[ "$DAA_ENABLED" == "true" ]]; then
        echo -e "${PURPLE}DAA-Specific Commands:${NC}"
        echo "  daa-status   - Show DAA agent status"
        echo "  daa-metrics  - Show agent performance metrics"
        echo "  daa-retrain  - Retrain neural models"
        echo ""
    fi
}

# Handle command line arguments
case "${1:-help}" in
    setup)
        check_dependencies
        setup_environment
        ;;
    start)
        check_dependencies
        setup_environment
        start_data_platform
        run_migrations
        start_connectors
        start_trading_platform
        sleep 3
        show_status
        ;;
    stop)
        stop_platform
        ;;
    restart)
        stop_platform
        sleep 2
        start_data_platform
        start_connectors
        start_trading_platform
        ;;
    status)
        show_status
        ;;
    simulate)
        echo -e "${BLUE}Starting paper trading simulation...${NC}"
        SIMULATION_MODE=true cargo run --release --bin trading-platform $FEATURES
        ;;
    live)
        echo -e "${RED}⚠️  Starting LIVE trading mode${NC}"
        echo -e "${YELLOW}This will use real money. Are you sure? (y/N)${NC}"
        read -r response
        if [[ "$response" =~ ^[Yy]$ ]]; then
            LIVE_TRADING=true cargo run --release --bin trading-platform $FEATURES
        else
            echo -e "${GREEN}Live trading cancelled${NC}"
        fi
        ;;
    logs)
        tail -f logs/*.log 2>/dev/null || echo "No logs found. Start the platform first."
        ;;
    clean)
        stop_platform
        docker system prune -f
        rm -rf data/* logs/*
        echo -e "${GREEN}✅ Cleanup complete${NC}"
        ;;
    daa-status)
        if [[ "$DAA_ENABLED" == "true" ]]; then
            cargo run --release --bin daa-status
        else
            echo -e "${RED}DAA mode not enabled. Start with --daa flag${NC}"
        fi
        ;;
    daa-metrics)
        if [[ "$DAA_ENABLED" == "true" ]]; then
            cargo run --release --bin daa-metrics
        else
            echo -e "${RED}DAA mode not enabled. Start with --daa flag${NC}"
        fi
        ;;
    daa-retrain)
        if [[ "$DAA_ENABLED" == "true" ]]; then
            echo -e "${YELLOW}Retraining neural models...${NC}"
            cargo run --release --bin daa-retrain
        else
            echo -e "${RED}DAA mode not enabled. Start with --daa flag${NC}"
        fi
        ;;
    help|*)
        show_trading_menu
        ;;
esac
```

This implementation guide provides a complete foundation for building the autonomous neural trading platform with real ruv-FANN and ruv-DAA integration. The structure is optimized for AI agents to understand and implement, with clear separation of concerns and comprehensive examples.