# Neural Trader Paper Trading Quick Start Guide

## 🚀 Running Paper Trading with $10,000 Capital

### Prerequisites
- GitHub Codespaces or local development environment
- Docker and Docker Compose installed
- At least one free API key for market data

### Step 1: Set Up Environment Variables

```bash
# Option A: Use the interactive setup script
./scripts/setup_stock_env.sh

# Option B: Manually set environment variables
export FINNHUB_API_KEY="your_actual_key"  # Get from https://finnhub.io/register
export PRIMARY_PROVIDER="finnhub"

# Load secure passwords if using .env.generated
source .env.generated  # This contains secure DB passwords
```

### Step 2: Choose Your Trading Mode

#### For Stock Trading (Recommended)
```bash
# Use the stock trading configuration
cp .env.stock-simulation .env

# Start the full stack with monitoring
./scripts/start_full_stock_simulation.sh
```

#### For Crypto Trading
```bash
# Use the minimal configuration
cp .env.minimal .env

# Update for crypto symbols
sed -i 's/TRADING_SYMBOLS_PRIMARY=.*/TRADING_SYMBOLS_PRIMARY=BTC\/USD,ETH\/USD,SOL\/USD/' .env
```

### Step 3: Start the Full Trading Stack

```bash
# This starts everything: TimescaleDB, Redis, Data Ingestion, 
# Neural Trader, Prometheus, and Grafana
docker-compose up -d

# Check that all services are running
docker-compose ps

# View logs (optional)
docker-compose logs -f
```

### Step 4: Access Monitoring Dashboards

In GitHub Codespaces, all ports are automatically forwarded. Access them via the **Ports** tab:

| Service | Port | Description | Login |
|---------|------|-------------|-------|
| **Grafana** | 3000 | 📊 Main trading dashboard | admin / $GRAFANA_ADMIN_PASSWORD |
| **Prometheus** | 9090 | 📈 Metrics and queries | No login required |
| **Redis Commander** | 8081 | 🔴 Live data viewer | No login required |
| **pgAdmin** | 8082 | 🗄️ Database explorer | admin@neuraltrader.local / $PGADMIN_DEFAULT_PASSWORD |
| **Trading API** | 3030 | 🔌 REST API endpoints | No login required |

### Step 5: Monitor Your Trading

#### Via Grafana (Recommended)
1. Open port 3000 in your browser
2. Login with admin credentials
3. Navigate to dashboards:
   - **Trading Performance** - P&L, positions, win rate
   - **Risk Metrics** - Exposure, drawdown, VaR
   - **System Health** - API status, data feeds

#### Via Command Line
```bash
# Check system health
curl http://localhost:3030/health

# View current positions
curl http://localhost:3030/api/positions

# Check performance metrics
curl http://localhost:3030/api/performance
```

#### Via MCP Tools
```bash
# Check system status
cargo run --bin mcp_server -- query system_status

# Get latest predictions
cargo run --bin mcp_server -- query request_prediction --symbol AAPL

# Check agent decisions
cargo run --bin mcp_server -- query agent_decision --agent-id market_analyzer
```

## 📊 Paper Trading Configuration

### Stock Trading Configuration (config/stock_trading.yaml)
```yaml
paper_trading:
  initial_capital: 10000.00
  currency: USD
  
  # Stock-specific settings
  trading_hours:
    market_open: "09:30"
    market_close: "16:00"
    active_start: "09:35"    # Start 5 min after open
    active_end: "15:45"      # End 15 min before close
  
  risk_management:
    max_position_size_pct: 0.25      # 25% max per position
    max_total_exposure_pct: 1.00     # 100% max total exposure
    stop_loss_pct: 0.02              # 2% stop loss
    take_profit_pct: 0.03            # 3% take profit
    max_daily_drawdown_pct: 0.05     # 5% daily loss limit
  
  execution:
    commission_per_share: 0.005      # $0.005 per share
    min_commission: 1.00             # $1 minimum
    enable_slippage: true
    slippage_bps: 10                 # 10 basis points
```

## 🎯 Trading Scenarios

### Scenario 1: Conservative Stock Day Trading
```bash
# Ensure you're using stock configuration
export TRADING_ASSET_CLASS=stocks
export TRADING_SYMBOLS_PRIMARY=AAPL,MSFT,GOOGL,AMZN

# Run with conservative settings
docker-compose up -d
```

### Scenario 2: Crypto Trading
```bash
# Switch to crypto configuration
export TRADING_ASSET_CLASS=crypto
export TRADING_SYMBOLS_PRIMARY=BTC/USD,ETH/USD,SOL/USD

# Run 24/7 crypto trading
docker-compose up -d
```

### Scenario 3: Multi-Strategy Portfolio
```bash
# Use the full configuration with multiple strategies
docker-compose -f docker-compose.yml up -d
```

## 📈 Performance Tracking

### Real-time Monitoring
- **Grafana Dashboard** (Port 3000): Visual metrics and charts
- **Prometheus** (Port 9090): Query raw metrics
- **Redis Commander** (Port 8081): Live data streams

### Key Metrics to Watch
```promql
# In Prometheus (port 9090)
trading_pnl_total                    # Total P&L
trading_positions_open               # Current open positions
trading_win_rate                     # Win percentage
rate(trading_trades_total[5m])       # Trades per minute
trading_sharpe_ratio                 # Risk-adjusted returns
```

### Export Results
```bash
# Export trade history from database
docker exec -it neural_trader_timescaledb psql -U neural_trader -d neural_trader_db \
  -c "COPY (SELECT * FROM trades WHERE created_at > NOW() - INTERVAL '1 day') TO STDOUT WITH CSV HEADER" \
  > trades_$(date +%Y%m%d).csv

# View performance summary
docker exec -it neural_trader_timescaledb psql -U neural_trader -d neural_trader_db \
  -c "SELECT COUNT(*) as total_trades, 
             SUM(CASE WHEN pnl > 0 THEN 1 ELSE 0 END) as winning_trades,
             SUM(pnl) as total_pnl,
             AVG(pnl) as avg_pnl
      FROM trades 
      WHERE created_at > NOW() - INTERVAL '1 day'"
```

## 🛠️ Troubleshooting

### Issue: No market data showing
```bash
# Check data ingestion logs
docker-compose logs -f data-ingestion

# Verify API keys are loaded
docker-compose exec data-ingestion env | grep API_KEY

# For stocks, check if market is open (Mon-Fri 9:30 AM - 4:00 PM ET)
date
```

### Issue: Services not starting
```bash
# Check service status
docker-compose ps

# View specific service logs
docker-compose logs [service-name]

# Restart a specific service
docker-compose restart [service-name]
```

### Issue: Can't access dashboards in Codespaces
```bash
# Check forwarded ports
gh codespace ports list

# Manually forward a port if needed
gh codespace ports forward 3000:3000 --codespace $CODESPACE_NAME
```

### Issue: Database connection errors
```bash
# Test database connection
docker exec -it neural_trader_timescaledb psql -U neural_trader -c "SELECT 1;"

# Check Redis connection
docker exec -it neural_trader_redis redis-cli -a $REDIS_PASSWORD ping
```

## 🔍 Advanced Configuration

### Environment Variables
```bash
# Risk management overrides
export MAX_POSITION_SIZE=2000        # $2000 max position
export DAILY_LOSS_LIMIT=300          # $300 daily loss limit
export USE_TRAILING_STOPS=true       # Enable trailing stops

# Performance tuning
export WORKER_THREADS=4
export CACHE_TTL_SECONDS=60
export PROCESSING_INTERVAL_SECONDS=1
```

### Custom Strategy Parameters
Edit strategy configuration in:
- `config/stock_trading.yaml` for stocks
- `config/trading.yaml` for general settings

### Data Provider Selection
```bash
# Set primary provider (in order of recommendation for stocks)
export PRIMARY_PROVIDER=finnhub      # Best free tier (60 calls/min)
# export PRIMARY_PROVIDER=alpha_vantage  # Good for technical indicators
# export PRIMARY_PROVIDER=iex_cloud     # Reliable but limited free tier
# export PRIMARY_PROVIDER=polygon       # Professional features
```

## 📊 Expected Results

With $10,000 capital and default settings:

### For Stocks:
- **Average position size**: $2,000 - $2,500 (20-25% of capital)
- **Daily trades**: 5-10 (during market hours)
- **Risk per trade**: $100 (1% of capital)
- **Commission**: ~$1-5 per trade
- **Target daily return**: 1-2%

### For Crypto:
- **Average position size**: $1,500 - $2,000
- **Daily trades**: 10-20 (24/7 operation)
- **Risk per trade**: $100 (1% of capital)
- **Lower commissions**: 0.1% per trade
- **Higher volatility**: 3-7% daily swings

## 🚀 Next Steps

1. **Start with Paper Trading**: Always test strategies with paper trading first
2. **Monitor Closely**: Watch Grafana dashboards for the first few hours
3. **Adjust Risk Parameters**: Start conservative, increase gradually
4. **Analyze Performance**: Export trades weekly for detailed analysis
5. **Document Learnings**: Keep notes on what works and what doesn't

## 🔐 Security Reminders

- Never commit API keys to files
- Keep environment variables in your shell profile
- Use the provided scripts to manage secrets
- Keep Codespaces ports private unless necessary

---

Ready to start? Set your API keys and run `./scripts/start_full_stock_simulation.sh` to begin paper trading!