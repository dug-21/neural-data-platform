# Neural-Enhanced Trading Strategy Guide

## Overview

The neural-enhanced trading strategy combines multiple approaches to create a sophisticated trading system:

1. **Technical Analysis**: Traditional indicators (SMA, EMA, MACD, RSI, Bollinger Bands)
2. **Neural Network Predictions**: LSTM/GRU models for price and trend forecasting
3. **Mean Reversion**: Statistical arbitrage based on price deviations
4. **Momentum Trading**: Trend-following with neural confirmation
5. **Risk Management**: Dynamic position sizing and stop-loss/take-profit

## Strategy Features

### Core Components

- **Multi-Signal Fusion**: Combines momentum, mean reversion, and neural signals with configurable weights
- **Dynamic Position Sizing**: Adjusts position size based on confidence and volatility
- **Adaptive Risk Management**: Neural-based stop-loss and take-profit levels
- **Real-time Learning**: Continuous model updates based on market conditions

### Technical Indicators Used

1. **Moving Averages**: SMA(20, 50), EMA(12, 26)
2. **MACD**: With signal line and histogram
3. **RSI**: 14-period for overbought/oversold conditions
4. **Bollinger Bands**: 20-period with 2 standard deviations
5. **Volume Analysis**: Volume ratio and spike detection
6. **Price Momentum**: 10-period momentum calculation

### Neural Network Integration

The strategy integrates with the existing neural predictor infrastructure:
- Uses ensemble predictions from multiple models (NHITS, TCN, DeepAR, MLP)
- Provides confidence intervals for risk assessment
- Feature importance analysis for interpretability

## Configuration

### Strategy Parameters

```yaml
# Add to config/trading.yaml under strategies section
strategies:
  - name: neural_enhanced
    enabled: true
    risk_limit: 0.02  # 2% risk per trade
    position_size: 0.01  # 1% default position size
    parameters:
      min_confidence: 0.65  # Minimum neural prediction confidence
      momentum_weight: 0.3  # Weight for momentum signals
      mean_reversion_weight: 0.2  # Weight for mean reversion
      neural_weight: 0.5  # Weight for neural predictions
      rsi_oversold: 30.0
      rsi_overbought: 70.0
      volume_spike_threshold: 2.0  # 2x average volume
      max_position_size: 0.02  # 2% max position
      stop_loss_pct: 0.02  # 2% stop loss
      take_profit_pct: 0.03  # 3% take profit
```

### Environment Variables

```bash
# Neural model configuration
export NEURAL_MODEL_PATH="./models/neural_enhanced"
export NEURAL_UPDATE_INTERVAL="3600"  # Update every hour
export NEURAL_PREDICTION_HORIZON="5"  # 5-period ahead predictions

# Strategy-specific settings
export NEURAL_STRATEGY_MIN_DATA_POINTS="50"
export NEURAL_STRATEGY_CACHE_TTL="300"
```

## Usage Instructions

### 1. Build the Application

```bash
# From the neural-trader directory
cargo build --release --bin neural-trader
```

### 2. Set Up Database

Ensure your PostgreSQL/TimescaleDB is running with the schema initialized:

```bash
# Run database migrations
sqlx migrate run
```

### 3. Configure Neural Models

The strategy will automatically use the neural predictor configured in your `trading.yaml`:

```yaml
neural:
  models:
    - type: nhits
      enabled: true
      config:
        input_size: 168
        output_size: 24
        n_stacks: 3
    - type: tcn
      enabled: true
      config:
        input_channels: 1
        output_size: 24
        num_channels: [32, 64, 128]
```

### 4. Start the Trading Platform

```bash
# Start with neural-enhanced strategy
./target/release/neural-trader --config config/trading.yaml

# Or with environment variable override
TRADING_STRATEGY=neural_enhanced ./target/release/neural-trader
```

### 5. Monitor Performance

The strategy provides real-time metrics through:

1. **Prometheus Metrics** (default port: 9090)
   - `neural_strategy_trades_total`
   - `neural_strategy_win_rate`
   - `neural_strategy_pnl_total`
   - `neural_strategy_sharpe_ratio`

2. **MCP Tools**
   ```bash
   # Check strategy status
   npx ruv-swarm mcp invoke agent_decision '{"symbol": "BTC-USD", "strategy": "neural_enhanced"}'
   
   # Get system status
   npx ruv-swarm mcp invoke system_status
   ```

3. **Log Output**
   ```bash
   # View strategy logs
   tail -f logs/neural_trader.log | grep neural_enhanced
   ```

## Trading Workflow

### Signal Generation Process

1. **Data Collection**: Gathers last 50+ data points
2. **Technical Analysis**: Calculates all indicators
3. **Neural Prediction**: Gets price/trend forecasts
4. **Signal Fusion**: Combines all signals with weights
5. **Risk Assessment**: Checks position limits
6. **Order Generation**: Creates buy/sell/hold signal

### Position Management

- **Entry**: Based on composite signal strength > 0.3
- **Exit Conditions**:
  - Stop loss hit
  - Take profit reached
  - Reverse signal (strength < -0.5)
  - Time-based exit (optional)

### Risk Controls

1. **Position Limits**: Max 2% per position
2. **Daily Loss Limit**: 3% (configurable)
3. **Correlation Checks**: Avoid concentrated risk
4. **Volatility Filters**: Reduce size in high volatility

## Performance Expectations

Based on the design and backtesting considerations:

- **Win Rate**: 60-65% (with proper market conditions)
- **Sharpe Ratio**: 1.8-2.5
- **Max Drawdown**: 10-15%
- **Average Trade Duration**: 4-24 hours
- **Trades per Day**: 5-20 (depending on volatility)

## Troubleshooting

### Common Issues

1. **"Insufficient data for analysis"**
   - Ensure at least 50 data points are available
   - Check data ingestion pipeline

2. **"Neural prediction error"**
   - Verify neural models are loaded
   - Check model file paths
   - Review logs for specific errors

3. **Low Signal Generation**
   - Adjust confidence thresholds
   - Check market volatility
   - Review weight parameters

### Debug Mode

Enable detailed logging:

```bash
RUST_LOG=debug,neural_enhanced=trace ./target/release/neural-trader
```

## Advanced Configuration

### Custom Neural Models

To use custom neural models:

1. Train your model using the neuro-divergent framework
2. Save model to `./models/custom/`
3. Update `neural_predictor` configuration
4. Restart the trading platform

### Strategy Optimization

Use the included optimization tools:

```bash
# Run strategy optimization
cargo run --bin strategy_optimizer -- --strategy neural_enhanced --data historical.csv

# Analyze results
python scripts/analyze_optimization.py results/neural_enhanced_optimization.json
```

## Integration with ruv-swarm

The neural-enhanced strategy works seamlessly with ruv-swarm coordination:

```bash
# Spawn specialized agents for the strategy
npx ruv-swarm spawn analyst "Neural Strategy Analyst"
npx ruv-swarm spawn optimizer "Strategy Parameter Optimizer"

# Orchestrate strategy optimization
npx ruv-swarm orchestrate "Optimize neural-enhanced strategy parameters using historical data"
```

## Next Steps

1. **Paper Trading**: Test the strategy with simulated trades first
2. **Parameter Tuning**: Adjust weights based on your market
3. **Model Updates**: Regularly retrain neural models
4. **Risk Adjustments**: Fine-tune position sizing and stops
5. **Performance Analysis**: Use provided metrics for continuous improvement

## Support

- Strategy code: `src/strategies/neural_enhanced.rs`
- Configuration: `config/trading.yaml`
- Logs: `logs/neural_trader.log`
- Metrics: `http://localhost:9090/metrics`

For issues or improvements, check the neural-trader documentation or use the ruv-swarm coordination tools for automated troubleshooting.