# Neural Trading Platform (MVP)

🚧 **MVP Status**: A working autonomous trading platform MVP with real-time data ingestion, WebSocket streaming, and foundational neural network architecture. Currently focused on data collection and processing with trading capabilities in development.

**Current State**: Production-ready data ingestion service with Alpaca WebSocket streaming, TimescaleDB storage, and monitoring infrastructure. Neural trading engine architecture implemented with basic autonomous decision-making capabilities.

## 🚀 Key Features (MVP)

### ✅ **Currently Working**
- **Real-time Data Ingestion**: WebSocket streaming from Alpaca Markets with <1 second latency
- **Time-Series Storage**: TimescaleDB with automated data compression and indexing
- **Event-Driven Architecture**: Redis pub/sub for real-time market data distribution
- **Production Infrastructure**: Docker deployment with Prometheus + Grafana monitoring
- **WebSocket Streaming**: Real-time market data with automatic reconnection

### 🔄 **In Development**
- **Neural Network Ensemble**: Architecture implemented, training pipeline in progress
- **Autonomous Trading**: Basic DAA framework functional, advanced strategies developing
- **Multi-Provider Support**: Alpaca fully operational, other providers configured

### 📋 **MVP Limitations**
- Currently optimized for Alpaca Markets data (paper trading recommended)
- Neural models functional but training with limited historical data
- Additional data providers available but not primary focus

## 📋 Architecture Overview

```
┌─────────────────────┐      ┌─────────────────────┐      ┌─────────────────────┐
│   Data Ingestion    │      │   Neural Trader     │      │   Data Platform     │
│     (Python)        │ ──▶  │      (Rust)         │ ◀──▶ │                     │
│                     │      │                     │      │ • TimescaleDB       │
│ • Market Providers  │      │ • DAA Coordinator   │      │ • Redis             │
│ • Rate Limiting     │      │ • Neural Predictor  │      └─────────────────────┘
│ • Data Processing   │      │ • Trading Engine    │
└─────────────────────┘      └─────────────────────┘
```

For detailed architecture documentation, see [ARCHITECTURE.md](ARCHITECTURE.md).

## 🛠️ Technology Stack

- **Core Engine**: Rust with Tokio async runtime
- **Neural Networks**: ruv_fann (Fast Artificial Neural Network Library)
- **Data Ingestion**: Python 3.11+ with asyncio
- **Time-Series DB**: TimescaleDB (PostgreSQL extension)
- **Caching/Streaming**: Redis 7+
- **Monitoring**: Prometheus + Grafana
- **Deployment**: Docker Compose

## 📦 Quick Start

### Prerequisites

- Docker and Docker Compose
- API keys for data providers (see [Configuration](#configuration))
- At least 8GB RAM and 4 CPU cores

### 1. Clone the Repository

```bash
git clone https://github.com/yourusername/neural-trader.git
cd neural-trader
```

### 2. Configure Environment (MVP Setup)

Create a `.env` file with your API keys and passwords:

```bash
# Database Configuration
POSTGRES_USER=neural_trader
POSTGRES_PASSWORD=your_secure_password
POSTGRES_DB=neural_trader_db

# Redis Configuration
REDIS_PASSWORD=your_redis_password

# Primary Data Provider (Required for MVP)
ALPACA_API_KEY=your_alpaca_key
ALPACA_API_SECRET=your_alpaca_secret
ALPACA_WS_ENABLED=true

# Trading Configuration
TRADING_SYMBOLS_PRIMARY=AAPL,MSFT,GOOGL,AMZN,NVDA
PRIMARY_PROVIDER=alpaca
USE_SIMPLE_MODE=false

# Optional: Additional Providers (for future use)
FINNHUB_API_KEY=your_finnhub_key
POLYGON_API_KEY=your_polygon_key
ALPHA_VANTAGE_API_KEY=your_av_key
```

### 3. Start the Platform (MVP)

```bash
# Start infrastructure services
docker-compose up -d timescaledb redis prometheus grafana

# Start data ingestion service (primary MVP component)
docker-compose up -d data-ingestion

# Start neural trader (basic autonomous decisions)
docker-compose up -d neural-trader

# View real-time data ingestion logs
docker-compose logs -f data-ingestion

# Check service health
docker-compose ps
```

### 4. Verify MVP Operation

```bash
# Check WebSocket streaming is working
docker-compose logs data-ingestion | grep "WebSocket"

# Verify market data ingestion
docker-compose logs data-ingestion | grep "AAPL"

# Check neural trader decisions
docker-compose logs neural-trader | grep "decision"
```

### 4. Access Monitoring

- Grafana Dashboard: http://localhost:3000 (admin/admin)
- Prometheus Metrics: http://localhost:9090

## 📊 Data Providers (MVP)

| Provider | Status | Features | Notes |
|----------|--------|----------|-------|
| **Alpaca Markets** | ✅ **Active** | Real-time WebSocket, Paper Trading | Primary provider, fully integrated |
| Polygon | 🔄 Configured | Professional real-time data | Available but not primary focus |
| IEX Cloud | 🔄 Configured | Institutional-grade | Available but not primary focus |
| Finnhub | 🔄 Configured | Comprehensive coverage | Available but not primary focus |
| Alpha Vantage | 🔄 Configured | Technical indicators | Available but not primary focus |
| Yahoo Finance | 🔄 Configured | Free tier fallback | Available but not primary focus |
| NewsAPI | 🔄 Configured | News sentiment | Available but not primary focus |
| Reddit | 🔄 Configured | Social sentiment | Available but not primary focus |
| FRED | 🔄 Configured | Federal Reserve data | Available but not primary focus |

**MVP Focus**: The system is currently optimized for Alpaca Markets with real-time WebSocket streaming. Other providers are configured and available but not the primary development focus.

## 🧠 Neural Network Models (MVP)

The platform implements an ensemble architecture with the following models:

### ✅ **Architecture Implemented**
- **NHITS**: Neural Hierarchical Interpolation for Time Series (128→64→32→16 neurons)
- **TCN**: Temporal Convolutional Networks (96→48→24 neurons) 
- **DeepAR**: Probabilistic forecasting with uncertainty (100→50→25 neurons)
- **Transformer**: Attention-based architecture (256→128→64→32 neurons)
- **MLP**: Multi-Layer Perceptron baseline (64→32→16 neurons)

### 🔄 **Current Development Status**
- **Model Framework**: Fully implemented with ruv_fann integration
- **Training Pipeline**: Functional with limited historical data (<1 day)
- **Ensemble Weighting**: Confidence-based consensus implemented
- **Online Learning**: Continuous model updates as data accumulates

### 📊 **MVP Performance**
- **Data Requirements**: Models need 30-80 samples minimum for operation
- **Training Time**: Real-time incremental learning (seconds per update)
- **Prediction Latency**: <500ms for ensemble consensus
- **Current Limitation**: Limited historical data affects model accuracy initially

## 🤖 Autonomous Trading System (MVP)

The DAA (Decentralized Autonomous Agents) system coordinates trading decisions:

### ✅ **Currently Functional**
1. **Neural Consensus**: Aggregates predictions from ensemble models with confidence weighting
2. **Strategy Integration**: Momentum and neural-enhanced strategies operational
3. **Risk Management**: Position sizing (2% max per trade) and stop-loss (2%) implemented
4. **Real-time Decisions**: 1-second decision cycles with adaptive thresholds
5. **Event Processing**: Redis pub/sub for market data and decision coordination

### 🔄 **MVP Status**
- **Decision Frequency**: Every 1 second during market hours
- **Entry Threshold**: 0.3 combined signal + 0.75 confidence required
- **Current Focus**: Data accumulation and pattern recognition
- **Trading Mode**: Paper trading recommended for MVP

### 📊 **Decision Process**
1. **Neural Consensus** (60% weight): Ensemble model predictions
2. **Strategy Signals** (40% weight): Traditional indicators
3. **Risk Assessment**: Volatility-adjusted position sizing
4. **Confidence Check**: Must exceed 75% threshold for entry
5. **Execution**: Currently optimized for Alpaca paper trading

## 🔧 Configuration

### Platform Configuration

Configuration files are located in the `config/` directory:

- `platform.toml`: Base configuration
- `development.toml`: Development settings
- `production.toml`: Production settings

Example configuration:

```toml
[neural]
memory_gb = 1.0
models = ["NHITS", "DeepAR", "TCN", "MLP"]
prediction_cache_ttl = 300

[database]
url = "postgres://neural_trader:password@localhost/neural_trader_db"
max_connections = 20

[redis]
url = "redis://localhost:6379"
default_ttl_seconds = 3600
```

### Trading Strategies

Configure trading strategies in `config/trading.yaml`:

```yaml
strategies:
  - name: momentum
    enabled: true
    risk_limit: 0.02
    position_size: 0.1
    
  - name: neural_enhanced
    enabled: true
    risk_limit: 0.02
    position_size: 0.1
```

## 📈 Development (MVP)

### Project Structure

```
neural-trader/
├── src/                    # Rust neural trading engine
│   ├── main.rs            # Main application entry
│   ├── neural/            # Neural network integration (ruv_fann)
│   ├── integration/       # DAA coordination system
│   ├── strategies/        # Trading strategies (momentum, neural-enhanced)
│   └── adapters/          # Database adapters (TimescaleDB, Redis)
├── data_ingestion/        # Python data ingestion service (PRIMARY)
│   ├── providers/         # Market data providers (Alpaca primary)
│   ├── schedulers/        # WebSocket streaming coordination
│   └── storage/           # Data storage and processing
├── config/                # Configuration files (TOML/YAML)
├── docker/                # Docker deployment files
└── docker-compose.yml     # Service orchestration
```

### MVP Development Focus
- **Primary Service**: Python data ingestion with real-time WebSocket streaming
- **Neural Engine**: Rust application with basic autonomous decision-making
- **Data Pipeline**: TimescaleDB → Redis → Trading Engine
- **Monitoring**: Prometheus metrics and Grafana dashboards

### Building from Source

#### Rust Application

```bash
# Build release version
cargo build --release

# Run tests
cargo test

# Run with custom config
cargo run -- --config config/development.toml
```

#### Python Service

```bash
cd data_ingestion

# Install dependencies
pip install -r requirements.txt

# Run service
python main.py start --providers yahoo_finance finnhub --symbols AAPL MSFT
```

### Running Tests

```bash
# Rust tests
cargo test

# Python tests
cd data_ingestion && pytest

# Integration tests
docker-compose -f docker-compose.test.yml up --abort-on-container-exit
```

## 📊 Monitoring & Performance

### Key Metrics

- **Trading Performance**: P&L, win rate, Sharpe ratio
- **Neural Accuracy**: Model prediction accuracy by horizon
- **System Health**: CPU, memory, latency metrics
- **Data Quality**: Provider uptime, data completeness

### Performance Characteristics (MVP)

- **Market Data Ingestion**: <1 second latency via WebSocket
- **Neural Predictions**: <500ms for ensemble consensus
- **Trading Decisions**: 1-second cycles with adaptive thresholds
- **Data Throughput**: Real-time processing with Redis pub/sub
- **Storage**: TimescaleDB with automatic compression

### MVP Benchmarks
- **WebSocket Latency**: Sub-second market data updates
- **Decision Frequency**: Every 1 second during market hours
- **Memory Usage**: ~500MB for full stack
- **Storage Growth**: ~1GB per day for primary symbols

## 🔒 Security

- **API Keys**: Environment variable based, never in code
- **Network**: Isolated Docker network with limited exposure
- **Authentication**: Password-protected Redis and PostgreSQL
- **Validation**: Input validation on all data adapters

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [ruv_fann](https://github.com/ruv-fann/ruv-fann) - Neural network framework
- [DAA](https://github.com/daa/daa) - Autonomous agent coordination
- [TimescaleDB](https://www.timescale.com/) - Time-series database
- All the data provider APIs that make this possible

## ⚠️ Disclaimer & MVP Status

### Current MVP Status
This is a **working MVP** focused on:
- ✅ Real-time data ingestion and storage
- ✅ Basic neural network architecture
- ✅ Simple autonomous decision-making
- 🔄 Limited historical data for training
- 📋 Paper trading recommended

### Important Notices
- **Educational Purpose**: This software is for educational and research purposes
- **Trading Risk**: Financial trading carries substantial risk of loss
- **MVP Limitations**: Currently optimized for data collection and basic trading
- **Data Requirements**: System needs time to accumulate sufficient historical data
- **Paper Trading**: Recommended for MVP evaluation

### Next Steps
1. **Data Accumulation**: Let system collect 7-30 days of historical data
2. **Model Training**: Neural networks improve with more data
3. **Strategy Development**: Additional trading strategies in development
4. **Risk Management**: Enhanced risk controls planned

**Always do your own research and never trade with money you cannot afford to lose.**