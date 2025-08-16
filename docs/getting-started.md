# Getting Started with Neural Trading Platform

This guide will get you up and running with the Neural Trading Platform in under 10 minutes. By the end, you'll have a fully functional autonomous trading system processing real market data.

## 🎯 What You'll Achieve

After completing this guide, you'll have:
- ✅ A running neural trading platform
- ✅ Real-time market data ingestion
- ✅ Neural network predictions
- ✅ Autonomous trading decisions (paper trading mode)
- ✅ Monitoring dashboards

## 📋 Prerequisites

Before starting, ensure you have:

### System Requirements
- **Docker** 20.10+ and **Docker Compose** 2.0+
- **4+ CPU cores** and **8GB+ RAM**
- **50GB+ available storage** (SSD recommended)
- **Stable internet connection** for real-time data

### Required Accounts
You'll need API keys from at least one market data provider:
- **Alpaca Markets** (recommended for beginners - free paper trading)
- **Finnhub** (free tier available)
- **Alpha Vantage** (free tier available)

### Check Your System
```bash
# Verify Docker installation
docker --version
docker-compose --version

# Check available resources
docker system info | grep -E "(CPUs|Total Memory)"
```

## 🚀 Quick Setup (5 minutes)

### Step 1: Clone the Repository
```bash
git clone https://github.com/yourusername/neural-trader.git
cd neural-trader
```

### Step 2: Configure Environment
```bash
# Copy the example environment file
cp .env.example .env

# Edit the configuration (use your preferred editor)
nano .env
```

**Minimum configuration for Alpaca (recommended for beginners):**
```bash
# Database Configuration
POSTGRES_USER=neural_trader
POSTGRES_PASSWORD=your_secure_password_here
POSTGRES_DB=neural_trader_db

# Redis Configuration  
REDIS_PASSWORD=your_redis_password_here

# Alpaca Configuration (Paper Trading)
ALPACA_API_KEY=your_alpaca_key_here
ALPACA_API_SECRET=your_alpaca_secret_here
ALPACA_WS_ENABLED=true
ALPACA_PAPER_TRADING=true

# Trading Configuration
TRADING_SYMBOLS_PRIMARY=AAPL,MSFT,GOOGL,TSLA,NVDA
PRIMARY_PROVIDER=alpaca
USE_PAPER_TRADING=true
```

### Step 3: Start the Platform
```bash
# Start all services
docker-compose up -d

# Check that all services are running
docker-compose ps
```

You should see all services in "Up" status:
```
       Name                     Command               State           Ports         
-----------------------------------------------------------------------------------
neural-trader_data-ingestion_1   python main.py                   Up      
neural-trader_grafana_1          /run.sh                          Up      0.0.0.0:3000->3000/tcp
neural-trader_neural-trader_1    ./target/release/neural-trader   Up      0.0.0.0:8080->8080/tcp
neural-trader_prometheus_1       /bin/prometheus                  Up      0.0.0.0:9090->9090/tcp
neural-trader_redis_1            docker-entrypoint.sh redis-s... Up      6379/tcp
neural-trader_timescaledb_1      docker-entrypoint.sh postgres   Up      5432/tcp
```

### Step 4: Verify Operation
```bash
# Check system health
curl http://localhost:8080/health

# View real-time logs
docker-compose logs -f data-ingestion | head -20
```

**Expected output:**
```
data-ingestion_1  | INFO: Connected to Alpaca WebSocket
data-ingestion_1  | INFO: Subscribed to symbols: AAPL, MSFT, GOOGL, TSLA, NVDA
data-ingestion_1  | INFO: Received market data for AAPL: $150.25
neural-trader_1   | INFO: Neural prediction for AAPL: 0.75 confidence
neural-trader_1   | INFO: DAA decision: HOLD (insufficient confidence for entry)
```

## 🎉 Success! You're Now Running

Congratulations! Your Neural Trading Platform is now running. Here's what's happening:

1. **Data Ingestion**: Collecting real-time market data from Alpaca
2. **Neural Processing**: Making predictions using ensemble models
3. **Autonomous Decisions**: DAA system evaluating trading opportunities
4. **Risk Management**: Monitoring positions and market conditions

## 📊 Access Your Dashboards

### Grafana Monitoring Dashboard
- **URL**: http://localhost:3000
- **Login**: admin / admin
- **Features**: Real-time trading metrics, system health, neural model performance

### Prometheus Metrics
- **URL**: http://localhost:9090
- **Purpose**: Raw metrics and system monitoring

### Trading Engine Health
- **URL**: http://localhost:8080/health
- **Purpose**: System status and component health checks

## 🔍 Verify Everything is Working

### Check Data Flow
```bash
# Verify market data ingestion
docker-compose logs data-ingestion | grep -i "received market data" | tail -5

# Check neural predictions
docker-compose logs neural-trader | grep -i "neural prediction" | tail -5

# View trading decisions
docker-compose logs neural-trader | grep -i "daa decision" | tail -5
```

### Monitor Resource Usage
```bash
# Check resource usage
docker stats --no-stream

# View disk usage
docker system df
```

## 🎯 What's Next?

Now that your system is running, here are your next steps:

### 1. Explore the Monitoring (5 minutes)
- Open Grafana at http://localhost:3000
- Explore the pre-built dashboards
- Watch real-time market data and trading decisions

### 2. Understand the System (15 minutes)
- Read the [Architecture Overview](architecture.md)
- Learn about [Neural Networks](neural-networks.md)
- Understand [Risk Management](risk-management.md)

### 3. Customize Configuration (30 minutes)
- Add more [data providers](data-provider-reference.md)
- Adjust [trading strategies](configuration.md#trading-strategies)
- Configure [risk parameters](risk-management.md)

### 4. Advanced Features (1 hour)
- Set up [real trading](deployment.md#production-trading) (when ready)
- Configure [advanced monitoring](monitoring.md)
- Explore [API endpoints](api-reference.md)

## 🛠️ Troubleshooting

### Common Issues

#### Services Won't Start
```bash
# Check Docker resources
docker system df
docker system prune  # If needed

# Restart specific service
docker-compose restart neural-trader
```

#### No Market Data
```bash
# Check API key configuration
docker-compose logs data-ingestion | grep -i "api"

# Verify provider status
curl -H "APCA-API-KEY-ID: your_key" -H "APCA-API-SECRET-KEY: your_secret" \
  https://paper-api.alpaca.markets/v2/account
```

#### Neural Predictions Not Working
```bash
# Check neural engine logs
docker-compose logs neural-trader | grep -i "neural"

# Verify model initialization
docker-compose logs neural-trader | grep -i "model"
```

#### Can't Access Dashboards
```bash
# Check port binding
docker-compose ps | grep -E "(3000|9090|8080)"

# Restart monitoring services
docker-compose restart grafana prometheus
```

### Getting Help

If you encounter issues:

1. **Check Logs**: `docker-compose logs [service-name]`
2. **Review Configuration**: Verify your `.env` file
3. **Consult Documentation**: [Troubleshooting Guide](troubleshooting.md)
4. **Ask for Help**: [GitHub Discussions](https://github.com/yourusername/neural-trader/discussions)

## 📚 Learning Path

### Beginner (First Week)
1. ✅ Complete this quick start guide
2. 📖 Read [Architecture Overview](architecture.md)
3. 🔧 Explore [Configuration Options](configuration.md)
4. 📊 Learn [Monitoring Basics](monitoring.md)

### Intermediate (First Month)
1. 🧠 Understand [Neural Networks](neural-networks.md)
2. 🤖 Learn [DAA System](daa-system.md)
3. ⚠️ Master [Risk Management](risk-management.md)
4. 🔧 Try [Custom Strategies](development.md)

### Advanced (Ongoing)
1. 🏭 Set up [Production Deployment](deployment.md)
2. ⚡ Optimize [Performance](performance.md)
3. 🔐 Implement [Security Best Practices](security.md)
4. 🤝 [Contribute](contributing.md) to the project

## 🚨 Important Reminders

### Always Use Paper Trading First
- The default configuration uses paper trading
- Never use real money until you understand the system
- Monitor performance for at least a week before considering real trading

### Risk Management
- Start with small position sizes
- Set appropriate stop-losses
- Monitor the system actively during market hours
- Have an exit strategy

### Continuous Learning
- Market conditions change constantly
- Neural models need regular retraining
- Stay updated with system improvements
- Join the community for best practices

---

**🎉 Congratulations!** You now have a running autonomous trading system. Take time to understand how it works before making any modifications.

**Next recommended reading**: [Architecture Overview](architecture.md)