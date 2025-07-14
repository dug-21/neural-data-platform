# Neural Trading Platform

An autonomous trading platform that leverages advanced neural networks (ruv_fann) and Decentralized Autonomous Agents (DAA) to make intelligent trading decisions. Built with Rust for high-performance core components and Python for flexible data ingestion.

## 🚀 Key Features

- **Neural Network Ensemble**: Multiple FANN-based models (NHITS, TCN, DeepAR, Transformer, MLP) with weighted consensus predictions
- **Autonomous Trading**: DAA coordination system for multi-agent decision making with risk management
- **Real-time Data**: Multi-provider data ingestion supporting 9+ market data sources
- **Time-Series Optimized**: TimescaleDB for historical data with compression and continuous aggregates
- **Event-Driven Architecture**: Redis pub/sub and streams for real-time market data flow
- **Production Ready**: Docker deployment with monitoring stack (Prometheus + Grafana)

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

### 2. Configure Environment

Create a `.env` file with your API keys and passwords:

```bash
# Database
POSTGRES_USER=neural_trader
POSTGRES_PASSWORD=your_secure_password
POSTGRES_DB=neural_trader_db

# Redis
REDIS_PASSWORD=your_redis_password

# API Keys (at least one required)
YAHOO_API_KEY=your_yahoo_key
FINNHUB_API_KEY=your_finnhub_key
POLYGON_API_KEY=your_polygon_key
# ... see docker-compose.yml for all supported providers
```

### 3. Start the Platform

```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f neural-trader

# Check service health
docker-compose ps
```

### 4. Access Monitoring

- Grafana Dashboard: http://localhost:3000 (admin/admin)
- Prometheus Metrics: http://localhost:9090

## 📊 Supported Data Providers

| Provider | Type | Priority | Features |
|----------|------|----------|----------|
| Polygon | Market Data | 1 | Professional real-time data |
| IEX Cloud | Market Data | 2 | Institutional-grade |
| Finnhub | Market Data | 3 | Comprehensive coverage |
| Alpha Vantage | Market Data | 4 | Technical indicators |
| Yahoo Finance | Market Data | 5 | Free tier fallback |
| NASDAQ Data | Market Data | - | Official exchange data |
| NewsAPI | Alternative | - | News sentiment |
| Reddit | Alternative | - | Social sentiment |
| FRED | Economic | - | Federal Reserve data |

## 🧠 Neural Network Models

The platform uses an ensemble of neural network models:

- **NHITS**: Neural Hierarchical Interpolation for Time Series (128→64→32→16 neurons)
- **TCN**: Temporal Convolutional Networks (96→48→24 neurons)
- **DeepAR**: Probabilistic forecasting with uncertainty (100→50→25 neurons)
- **Transformer**: Attention-based architecture (256→128→64→32 neurons)
- **MLP**: Multi-Layer Perceptron baseline (64→32→16 neurons)

Models are trained continuously with online learning and weighted based on performance.

## 🤖 Autonomous Trading System

The DAA (Decentralized Autonomous Agents) system coordinates trading decisions:

1. **Neural Consensus**: Aggregates predictions from multiple models
2. **Strategy Signals**: Collects votes from trading strategies (momentum, neural-enhanced)
3. **Risk Assessment**: Evaluates market, position, and portfolio risk
4. **Decision Synthesis**: Combines inputs with confidence weighting
5. **Parameter Adaptation**: Updates based on performance feedback

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

## 📈 Development

### Project Structure

```
neural-trader/
├── src/                    # Rust source code
│   ├── main.rs            # Main application entry
│   ├── neural/            # Neural network integration
│   ├── integration/       # DAA coordination
│   ├── strategies/        # Trading strategies
│   └── adapters/          # Data source adapters
├── data_ingestion/        # Python data ingestion service
│   ├── providers/         # Market data providers
│   ├── processors/        # Data processing pipeline
│   └── schedulers/        # Data collection scheduling
├── config/                # Configuration files
├── docker/                # Docker-related files
└── docker-compose.yml     # Service orchestration
```

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

### Performance Characteristics

- Market data ingestion: < 100ms latency
- Neural predictions: < 500ms for ensemble
- Trading decisions: < 1s end-to-end
- Throughput: 10,000+ market updates/second

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

## ⚠️ Disclaimer

This software is for educational and research purposes only. Trading financial instruments carries risk. Always do your own research and never trade with money you cannot afford to lose.