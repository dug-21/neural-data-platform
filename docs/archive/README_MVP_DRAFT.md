# Neural Trading Platform (MVP)

> **MVP Status**: This is a Minimum Viable Product implementation featuring real-time data ingestion and autonomous trading coordination. The system is in active development with core features operational.

## 🎯 Current MVP Features

### ✅ **Operational Components**
- **Real-time Data Ingestion**: WebSocket streaming from Alpaca Markets
- **TimescaleDB Integration**: Time-series data storage with hypertables
- **Redis Pub/Sub**: Real-time market data distribution
- **Autonomous Trading Framework**: DAA (Decentralized Autonomous Agents) coordination
- **Monitoring Stack**: Prometheus metrics + Grafana dashboards
- **Docker Deployment**: Production-ready containerized services

### 🚧 **In Development**
- **Neural Network Models**: Integration with ruv-FANN library (dependency resolution in progress)
- **Trading Strategies**: Momentum and neural-enhanced strategies (foundation implemented)
- **Paper Trading**: Simulation mode for strategy testing
- **Risk Management**: Position sizing and loss limits (framework in place)

## 📊 Architecture Overview

```
┌─────────────────────┐      ┌─────────────────────┐      ┌─────────────────────┐
│   Data Ingestion    │      │   Neural Trader     │      │   Data Platform     │
│     (Python)        │ ──▶  │      (Rust)         │ ◀──▶ │                     │
│                     │      │                     │      │ • TimescaleDB       │
│ • Alpaca WebSocket  │      │ • DAA Coordinator   │      │ • Redis Streams     │
│ • Rate Limiting     │      │ • Strategy Engine   │      │ • Prometheus        │
│ • Data Validation   │      │ • Risk Management   │      │ • Grafana           │
└─────────────────────┘      └─────────────────────┘      └─────────────────────┘
```

## 🛠️ Technology Stack

### **Core Components**
- **Data Ingestion**: Python 3.11+ with asyncio, Alpaca SDK
- **Trading Engine**: Rust with Tokio async runtime
- **Database**: TimescaleDB (PostgreSQL with time-series optimization)
- **Caching**: Redis 7+ with pub/sub and streams
- **Monitoring**: Prometheus + Grafana
- **Deployment**: Docker Compose

### **Neural Networks** (In Progress)
- **Framework**: ruv-FANN (Fast Artificial Neural Network Library)
- **Models**: NHITS, TCN, DeepAR, Transformer, MLP ensemble
- **Integration**: DAA autonomous coordination system

## 🚀 Quick Start

### Prerequisites
- Docker and Docker Compose
- Alpaca Markets API credentials (paper trading supported)
- 8GB RAM, 4 CPU cores recommended

### 1. Clone and Setup

```bash
git clone https://github.com/yourusername/neural-trader.git
cd neural-trader
git checkout real-time-ingestion  # Current working branch
```

### 2. Configure Environment

Create `.env` file with required settings:

```bash
# Database Configuration
POSTGRES_USER=neural_trader
POSTGRES_PASSWORD=your_secure_password
POSTGRES_DB=neural_trader_db

# Redis Configuration
REDIS_PASSWORD=your_redis_password

# Alpaca API (Paper Trading)
ALPACA_API_KEY=your_alpaca_api_key
ALPACA_API_SECRET=your_alpaca_api_secret
ALPACA_WS_ENABLED=true
PRIMARY_PROVIDER=alpaca

# Optional: Additional providers
POLYGON_API_KEY=your_polygon_key
FINNHUB_API_KEY=your_finnhub_key
ALPHA_VANTAGE_API_KEY=your_alpha_vantage_key
```

### 3. Start Services

```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f data-ingestion
docker-compose logs -f neural-trader

# Check health
docker-compose ps
```

### 4. Access Interfaces

- **Grafana Dashboard**: http://localhost:3000 (admin/admin)
- **Prometheus Metrics**: http://localhost:9090
- **Redis Commander**: http://localhost:8081 (development only)
- **pgAdmin**: http://localhost:8082 (development only)

## 📈 Data Providers

### **Primary Provider: Alpaca Markets**
- **Status**: ✅ Fully Operational
- **Features**: WebSocket streaming, historical data, paper trading
- **Symbols**: US equities
- **Rate Limits**: 200 requests/minute (basic), 10,000/minute (unlimited)
- **Data Feed**: IEX (basic), SIP (unlimited)

### **Secondary Providers** (Configuration Ready)
| Provider | Status | Features |
|----------|---------|----------|
| Polygon | 🔧 Configured | Real-time market data, high-frequency |
| Finnhub | 🔧 Configured | Stock data, news, earnings |
| Alpha Vantage | 🔧 Configured | Technical indicators, fundamentals |
| Yahoo Finance | 🔧 Configured | Free tier fallback |

## 🤖 Autonomous Trading System

### **DAA Coordination** (Operational)
The system uses Decentralized Autonomous Agents for decision-making:

1. **Market Data Processing**: Real-time WebSocket data ingestion
2. **Event Bus Integration**: Redis pub/sub for inter-agent communication
3. **Decision Coordination**: Multi-agent consensus for trading actions
4. **Risk Management**: Built-in position and portfolio risk controls

### **Trading Strategies** (Framework Ready)
- **Momentum Strategy**: Trend-following with risk management
- **Neural-Enhanced Strategy**: ML-based prediction integration
- **Risk Controls**: 2% position limit, 3% daily loss limit
- **Position Sizing**: Dynamic sizing based on volatility

## 📊 Monitoring & Metrics

### **Real-time Metrics**
- Market data ingestion rate
- WebSocket connection health
- Trading decision latency
- Risk metrics and position tracking
- System resource utilization

### **Performance Characteristics** (Target)
- **Data Latency**: <100ms from source to storage
- **Decision Speed**: <1s for trading decisions
- **Throughput**: 10,000+ market updates/second
- **Uptime**: 99.9% target with auto-recovery

## 🔧 Development

### **Current Branch Structure**
```
neural-trader/
├── data_ingestion/           # Python data ingestion service
│   ├── providers/           # Market data providers
│   ├── storage/             # TimescaleDB and Redis integration
│   └── utils/               # Logging, metrics, validation
├── src/                     # Rust trading engine
│   ├── adapters/            # Database and cache adapters
│   ├── strategies/          # Trading strategies
│   └── integration/         # DAA coordination
├── docker/                  # Docker configuration
└── config/                  # System configuration
```

### **Building from Source**

**Data Ingestion Service:**
```bash
cd data_ingestion
pip install -r requirements.txt
python main.py start --providers alpaca --symbols AAPL MSFT
```

**Trading Engine:**
```bash
cargo build --release
cargo run
```

### **Testing**
```bash
# Python tests
cd data_ingestion && python -m pytest tests/

# Rust tests
cargo test

# Integration tests
docker-compose -f docker-compose.test.yml up --abort-on-container-exit
```

## 🛡️ Security & Production

### **Security Features**
- **Environment Variables**: All secrets in .env files
- **Docker Networks**: Isolated internal networks
- **Non-root Containers**: Security-hardened containers
- **Rate Limiting**: API rate limiting and backoff

### **Production Considerations**
- **Resource Limits**: Memory and CPU limits configured
- **Health Checks**: HTTP health endpoints
- **Logging**: Structured logging with ELK stack compatibility
- **Monitoring**: Comprehensive Prometheus metrics

## 🚨 Known Limitations (MVP)

### **Current Limitations**
1. **Neural Models**: ruv-FANN integration pending dependency resolution
2. **Live Trading**: Currently paper trading only (easily switchable)
3. **Market Coverage**: US equities only via Alpaca
4. **Strategy Count**: 2 basic strategies implemented
5. **Backtesting**: Not yet implemented

### **Upcoming Features**
- **Neural Model Integration**: Complete ruv-FANN implementation
- **Live Trading**: Real trading with proper risk controls
- **Multi-Asset Support**: Options, futures, crypto
- **Advanced Strategies**: Machine learning-based strategies
- **Backtesting Engine**: Historical strategy validation

## 🔄 Roadmap

### **Phase 1 (Current - MVP)**
- [x] Real-time data ingestion
- [x] Docker deployment
- [x] Basic monitoring
- [x] DAA framework
- [ ] Neural model integration

### **Phase 2 (Next 2 weeks)**
- [ ] Complete neural model integration
- [ ] Paper trading validation
- [ ] Enhanced risk management
- [ ] Performance optimization

### **Phase 3 (Month 2)**
- [ ] Live trading capabilities
- [ ] Advanced strategies
- [ ] Backtesting engine
- [ ] Multi-asset support

## 📝 Configuration

### **Data Ingestion Configuration**
```yaml
# config/data_ingestion.yaml
providers:
  alpaca:
    enabled: true
    websocket: true
    symbols: ["AAPL", "MSFT", "GOOGL"]
    
monitoring:
  prometheus_port: 9091
  metrics_enabled: true
```

### **Trading Configuration**
```yaml
# config/trading.yaml
strategies:
  momentum:
    enabled: true
    risk_limit: 0.02
    position_size: 0.1
    
  neural_enhanced:
    enabled: false  # Pending neural integration
    risk_limit: 0.02
    position_size: 0.1
```

## 🆘 Support & Troubleshooting

### **Common Issues**
1. **WebSocket Connection**: Check Alpaca API credentials
2. **Database Connection**: Verify TimescaleDB is running
3. **Redis Connection**: Ensure Redis is accessible
4. **Docker Issues**: Check Docker and Docker Compose versions

### **Debugging**
```bash
# Check service logs
docker-compose logs -f data-ingestion

# Monitor metrics
curl http://localhost:9091/metrics

# Database connection
docker-compose exec timescaledb psql -U neural_trader -d neural_trader_db
```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## ⚠️ Disclaimer

**This is MVP software for educational and research purposes.** 
- Currently supports paper trading only
- Not recommended for live trading without thorough testing
- Trading financial instruments involves risk
- Always do your own research

## 🙏 Acknowledgments

- [Alpaca Markets](https://alpaca.markets/) - Primary data provider
- [TimescaleDB](https://www.timescale.com/) - Time-series database
- [ruv-FANN](https://github.com/ruv-fann) - Neural network framework
- [DAA Framework](https://github.com/daa) - Autonomous agent coordination

---

**Current Status**: MVP with real-time data ingestion and autonomous trading framework operational. Neural model integration in progress.