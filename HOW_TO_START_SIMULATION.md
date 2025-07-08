# 🚀 How to Start Neural Trader Simulation

This guide covers how to start the Neural Trader stock trading simulation in two environments:
1. **GitHub Codespaces** (cloud development with disk limitations)
2. **Local Docker** (your home computer with full resources)

## 📋 Prerequisites

### Required API Keys (at least one):
- **Finnhub** (Recommended): https://finnhub.io/register - 60 calls/min free
- **Alpha Vantage**: https://www.alphavantage.co/support/#api-key - 5 calls/min free
- **IEX Cloud**: https://iexcloud.io/console/tokens
- **Polygon**: https://polygon.io/stocks

### Required Secrets:
- Database passwords (can be auto-generated)
- JWT secrets (can be auto-generated)

---

## 🌐 Starting in GitHub Codespaces

### Step 1: Set Up Codespaces Secrets

1. Go to your GitHub repository settings
2. Navigate to **Secrets and variables** → **Codespaces**
3. Add these secrets:
   ```
   FINNHUB_API_KEY=your_actual_key
   ALPHA_VANTAGE_API_KEY=your_actual_key
   POSTGRES_PASSWORD=your_secure_password
   REDIS_PASSWORD=your_secure_password
   JWT_SECRET=your_jwt_secret
   GRAFANA_ADMIN_PASSWORD=your_admin_password
   ```

### Step 2: Open in Codespaces

1. Click the green **Code** button on your repository
2. Select **Codespaces** tab
3. Click **Create codespace on main** (or your branch)
4. Wait for the environment to build (~2-3 minutes)

### Step 3: Start the Simulation (Hybrid Mode - Recommended)

Due to disk space limitations in Codespaces, use the hybrid approach:

```bash
# 1. Navigate to the project root
cd /workspaces/neural-trader

# 2. Check your environment variables are loaded
env | grep -E "FINNHUB|POSTGRES_PASSWORD" | sed 's/=.*/=[SET]/'

# 3. Start the minimal database services
./scripts/start_minimal_footprint.sh

# 4. Build and run the application locally
cargo build --release --bin neural-trader
./target/release/neural-trader
```

### Step 4: Monitor the System

Open multiple terminal tabs in Codespaces:

**Tab 1 - System Monitor:**
```bash
./monitor_trading_system.sh
```

**Tab 2 - Live Logs:**
```bash
tail -f logs/neural-trader.log
```

**Tab 3 - Database Queries:**
```bash
# Check for market data
docker exec -it neural_trader_stocks-timescaledb-1 psql -U neural_trader -c "SELECT COUNT(*) FROM market_data;"

# View recent trades
docker exec -it neural_trader_stocks-timescaledb-1 psql -U neural_trader -c "SELECT * FROM trades ORDER BY created_at DESC LIMIT 10;"
```

### Step 5: Access Web Interfaces

In Codespaces, ports are automatically forwarded. Click on the **Ports** tab:

- **3030**: Trading API
- **3000**: Grafana Dashboard (admin/your_password)
- **9090**: Prometheus Metrics
- **8081**: Redis Commander (if enabled)
- **8082**: pgAdmin (if enabled)

### Troubleshooting Codespaces

If you run out of disk space:
```bash
# Clean up Docker
docker system prune -af --volumes

# Remove build artifacts
rm -rf target/debug
cargo clean

# Use the ultra-minimal setup
docker-compose -f docker-compose.ultramin.yml up -d
```

---

## 🖥️ Starting on Your Home Computer (Docker)

### Step 1: Prerequisites

1. **Install Docker Desktop**:
   - Windows/Mac: https://www.docker.com/products/docker-desktop
   - Linux: `curl -fsSL https://get.docker.com | sh`

2. **Clone the repository**:
   ```bash
   git clone https://github.com/yourusername/neural-trader.git
   cd neural-trader
   ```

3. **Set up environment variables**:
   ```bash
   # Copy the example environment file
   cp .env.stock-simulation.example .env.stock-simulation
   
   # Export your API keys
   export FINNHUB_API_KEY='your_actual_key'
   export ALPHA_VANTAGE_API_KEY='your_actual_key'
   export POSTGRES_PASSWORD='secure_password'
   export REDIS_PASSWORD='secure_password'
   export JWT_SECRET='your_jwt_secret'
   export GRAFANA_ADMIN_PASSWORD='admin_password'
   ```

### Step 2: Start with Full Docker Stack

Since you have no disk limitations at home, use the full stack:

```bash
# 1. Load environment variables
source .env.stock-simulation

# 2. Start all services
./scripts/start_full_stock_simulation.sh

# When prompted to view logs, press 'y'
```

### Alternative: Manual Docker Compose

```bash
# 1. Build all images
docker-compose -f docker-compose.dev.yml build

# 2. Start all services
docker-compose -f docker-compose.dev.yml up -d

# 3. View logs
docker-compose -f docker-compose.dev.yml logs -f
```

### Step 3: Verify Everything is Running

```bash
# Check all containers
docker ps

# Expected output should show:
# - neural_trader_app
# - neural_trader_timescaledb
# - neural_trader_redis
# - neural_trader_data_ingestion
# - neural_trader_prometheus
# - neural_trader_grafana
```

### Step 4: Access the Services

Open your browser to:

- **Trading API**: http://localhost:3030
- **Grafana Dashboard**: http://localhost:3000 (admin/your_password)
- **Prometheus**: http://localhost:9090
- **pgAdmin**: http://localhost:8082 (admin@neural-trader.local/admin)
- **Redis Commander**: http://localhost:8081

### Step 5: Monitor Trading Activity

**Real-time monitoring dashboard:**
```bash
# Create a monitoring script
cat > monitor_local.sh << 'EOF'
#!/bin/bash
while true; do
  clear
  echo "🚀 Neural Trader Monitor - $(date)"
  echo "=================================="
  
  # Container status
  echo -e "\n📦 Containers:"
  docker ps --format "table {{.Names}}\t{{.Status}}" | grep neural_trader
  
  # Database stats
  echo -e "\n📊 Database Stats:"
  docker exec neural_trader_timescaledb psql -U neural_trader -t -c "SELECT 'Market Data:', COUNT(*) FROM market_data;"
  docker exec neural_trader_timescaledb psql -U neural_trader -t -c "SELECT 'Trades:', COUNT(*) FROM trades;"
  
  # Redis stats
  echo -e "\n💾 Redis Cache:"
  docker exec neural_trader_redis redis-cli -a ${REDIS_PASSWORD} INFO keyspace | grep -E "keys=|expires="
  
  sleep 5
done
EOF

chmod +x monitor_local.sh
./monitor_local.sh
```

---

## 📊 Market Hours & Data Collection

### When to Expect Data:

- **US Market Hours**: 9:30 AM - 4:00 PM ET (Monday-Friday)
- **Pre-market**: 4:00 AM - 9:30 AM ET
- **After-hours**: 4:00 PM - 8:00 PM ET

### Data Collection Patterns:

1. **During Market Hours**:
   - Real-time quotes every 1-5 seconds
   - Trade executions logged immediately
   - Technical indicators calculated continuously

2. **After Hours**:
   - Reduced data frequency
   - End-of-day summaries
   - Next-day predictions calculated

---

## 🛠️ Common Commands

### Stop Everything:
```bash
# Codespaces
docker-compose -f docker-compose.ultramin.yml down
pkill -f neural-trader

# Local Docker
docker-compose -f docker-compose.dev.yml down
```

### View Logs:
```bash
# Application logs
tail -f logs/neural-trader.log

# Docker logs
docker-compose logs -f neural-trader
docker-compose logs -f data-ingestion
```

### Database Queries:
```bash
# Connect to database
docker exec -it neural_trader_timescaledb psql -U neural_trader

# Useful queries:
SELECT * FROM market_data ORDER BY timestamp DESC LIMIT 10;
SELECT * FROM trades WHERE created_at > NOW() - INTERVAL '1 hour';
SELECT symbol, COUNT(*) FROM market_data GROUP BY symbol;
```

### Clean Up:
```bash
# Remove all data and start fresh
docker-compose down -v
docker system prune -af
```

---

## 🚨 Troubleshooting

### Issue: No market data appearing
- **Check**: Is the market open? (9:30 AM - 4:00 PM ET)
- **Verify**: API keys are set correctly
- **Test**: Run `./test_finnhub_direct.py` to verify API access

### Issue: Containers won't start
- **Check**: Ports already in use? `lsof -i :5432`
- **Fix**: `docker-compose down` then try again
- **Clean**: `docker system prune -af`

### Issue: Out of disk space (Codespaces)
- **Use**: Hybrid mode (databases in Docker, app local)
- **Clean**: `docker system prune -af --volumes`
- **Alternative**: Use minimal compose file

### Issue: Can't connect to services
- **Local**: Check firewall settings
- **Codespaces**: Check port forwarding in Ports tab

---

## 📈 Expected Results

When running successfully during market hours, you should see:

1. **Logs showing data ingestion**:
   ```
   INFO: Fetching quote for AAPL: $209.62
   INFO: Storing market data for 6 symbols
   INFO: Technical indicators calculated
   ```

2. **Database filling with data**:
   ```
   neural_trader=# SELECT COUNT(*) FROM market_data;
    count 
   -------
     1248
   ```

3. **Trading decisions being made**:
   ```
   INFO: Signal generated for NVDA: BUY
   INFO: Paper trade executed: BUY 10 shares @ $160.00
   ```

4. **Grafana dashboards updating** with real-time metrics

---

## 🎯 Next Steps

1. **Customize Trading Strategy**: Edit `config/trading_config.toml`
2. **Add More Symbols**: Update `TRADING_SYMBOLS_PRIMARY` in `.env.stock-simulation`
3. **Enable More Providers**: Add API keys for multiple data sources
4. **Train Neural Models**: Collect data for a few days, then run training
5. **Backtest Strategies**: Use historical data to test performance

---

## 📞 Support

- **Logs**: Always check `logs/neural-trader.log` first
- **Documentation**: See `/docs` folder for detailed guides
- **Issues**: Report bugs at GitHub Issues
- **Community**: Join discussions in GitHub Discussions

Happy Trading! 🚀📈