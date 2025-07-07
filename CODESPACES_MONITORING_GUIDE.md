# 📊 Neural Trader Monitoring in GitHub Codespaces

## 🚀 Quick Start

After starting the Docker stack with `./scripts/start_full_stock_simulation.sh`, you'll have access to several monitoring interfaces.

## 🌐 Available Monitoring Interfaces

### 1. **Grafana Dashboard** (Port 3000) - Main Visualization
- **What it shows**: Real-time trading metrics, P&L, positions, performance charts
- **Access**: 
  - Click on the "Ports" tab in Codespaces
  - Find port 3000 (labeled "Grafana Dashboard")
  - Click the globe icon to open in browser
- **Login**: 
  - Username: `admin`
  - Password: Check `GRAFANA_ADMIN_PASSWORD` in your environment

### 2. **Prometheus** (Port 9090) - Raw Metrics
- **What it shows**: Time-series metrics, query interface, targets status
- **Access**: Open port 9090 from the Ports tab
- **Useful queries**:
  ```promql
  # Trading performance
  trading_pnl_total
  trading_positions_open
  trading_win_rate
  
  # System health
  up
  process_cpu_seconds_total
  ```

### 3. **Redis Commander** (Port 8081) - Real-time Data Viewer
- **What it shows**: Live market data, trading signals, agent decisions
- **Access**: Open port 8081 from the Ports tab
- **Key patterns to explore**:
  - `market:*` - Live market data
  - `signals:*` - Trading signals
  - `agents:*` - Agent decisions
  - `positions:*` - Open positions

### 4. **pgAdmin** (Port 8082) - Database Explorer
- **What it shows**: Historical data, trades, performance metrics
- **Access**: Open port 8082 from the Ports tab
- **Login**: 
  - Email: `admin@neuraltrader.local`
  - Password: Check `PGADMIN_DEFAULT_PASSWORD`

### 5. **Neural Trader API** (Port 3030) - REST API
- **Endpoints**:
  - `/health` - System health check
  - `/metrics` - Prometheus metrics
  - `/api/positions` - Current positions
  - `/api/performance` - Trading performance

### 6. **Data Ingestion Metrics** (Port 8001)
- **What it shows**: Data provider status, API usage, rate limits
- **Access**: Open port 8001 from the Ports tab

## 🎯 Codespaces Port Access

### Method 1: Ports Tab (Recommended)
1. Click on the "Ports" tab at the bottom of Codespaces
2. Find the port you want to access
3. Click the globe icon (🌐) to open in a new tab
4. Or click the address to copy the URL

### Method 2: Command Palette
1. Press `Cmd/Ctrl + Shift + P`
2. Type "Forward a Port"
3. Enter the port number
4. Choose visibility (private/public)

### Method 3: Direct URL
Codespaces URLs follow this pattern:
```
https://CODESPACE_NAME-PORT.preview.app.github.dev
```

Example:
```
https://mycodespace-3000.preview.app.github.dev  # Grafana
https://mycodespace-9090.preview.app.github.dev  # Prometheus
```

## 📈 What to Monitor During Trading

### In Grafana:
- **Trading Dashboard**: Overall P&L, win rate, positions
- **Risk Dashboard**: Exposure, drawdown, risk metrics
- **Performance Dashboard**: Strategy performance comparison
- **System Dashboard**: API latency, data feed status

### In Redis Commander:
- **Live Prices**: `market:AAPL:price`, `market:MSFT:price`
- **Signals**: `signals:momentum:*`, `signals:entry:*`
- **Decisions**: `agents:decisions:*`

### In Prometheus:
- **Trading Metrics**:
  ```promql
  rate(trading_trades_total[5m])  # Trades per minute
  trading_pnl_unrealized          # Unrealized P&L
  trading_sharpe_ratio            # Sharpe ratio
  ```

## 🛠️ Troubleshooting

### Ports Not Accessible?
1. Check that services are running:
   ```bash
   docker-compose ps
   ```

2. Manually forward a port:
   ```bash
   gh codespace ports forward 3000:3000
   ```

3. Check port visibility:
   ```bash
   gh codespace ports list
   ```

### Can't Login to Grafana/pgAdmin?
Check your environment variables:
```bash
echo $GRAFANA_ADMIN_PASSWORD
echo $PGADMIN_DEFAULT_PASSWORD
```

### No Data Showing?
1. Ensure data ingestion is running:
   ```bash
   docker-compose logs -f data-ingestion
   ```

2. Check if market is open (US stocks: Mon-Fri 9:30 AM - 4:00 PM ET)

3. Verify API keys are set:
   ```bash
   docker-compose exec data-ingestion env | grep API_KEY
   ```

## 📱 Mobile Access

Codespaces ports can be accessed from mobile devices:
1. Make the port public in the Ports tab
2. Access via the generated URL
3. Grafana and Redis Commander work well on mobile

## 🔒 Security Note

- Keep ports **private** by default
- Only make ports public temporarily when needed
- Grafana and pgAdmin have authentication
- Use strong passwords for production