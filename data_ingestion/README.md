# Neural Trader Data Ingestion Service

A production-ready, scalable Python data ingestion service for financial market data. Supports multiple data providers, real-time streaming, and batch processing with comprehensive error handling and monitoring.

## Features

- **Multiple Data Providers**:
  - IEX Cloud (premium)
  - Polygon.io (premium)
  - Alpha Vantage (free tier)
  - Yahoo Finance (free)
  - Finnhub (free tier)

- **Data Types**:
  - Real-time market data streaming
  - Historical OHLCV data
  - Tick-level trade data
  - Order book snapshots
  - Technical indicators

- **Storage Backends**:
  - TimescaleDB for time-series data
  - Redis for real-time caching and pub/sub

- **Processing Features**:
  - Data validation and cleaning
  - Outlier detection
  - Multi-provider aggregation
  - Technical indicator calculation
  - Data transformation and normalization

- **Production Features**:
  - Async/await for high performance
  - Comprehensive error handling
  - Retry logic with exponential backoff
  - Circuit breaker pattern
  - Rate limiting
  - Prometheus metrics
  - Structured logging
  - Docker support

## Quick Start

### Using Docker Compose (Recommended)

1. Clone the repository and navigate to the data ingestion directory:
```bash
cd neural-trader/data_ingestion
```

2. Create a `.env` file with your API keys:
```bash
# API Keys (optional - will use free providers if not set)
IEX_CLOUD_API_KEY=your_key_here
ALPHA_VANTAGE_API_KEY=your_key_here
POLYGON_API_KEY=your_key_here
FINNHUB_API_KEY=your_key_here

# Database passwords
TIMESCALE_PASSWORD=secure_password
REDIS_PASSWORD=secure_password
GRAFANA_PASSWORD=admin_password
```

3. Start the services:
```bash
docker-compose up -d
```

4. Access the services:
- Grafana: http://localhost:3000 (admin/admin_password)
- Prometheus: http://localhost:9091
- TimescaleDB: localhost:5432
- Redis: localhost:6379

### Manual Installation

1. Install dependencies:
```bash
pip install -r requirements.txt
```

2. Set up TimescaleDB and Redis (or use Docker):
```bash
docker run -d --name timescaledb -p 5432:5432 \
  -e POSTGRES_PASSWORD=postgres \
  timescale/timescaledb:latest-pg16

docker run -d --name redis -p 6379:6379 redis:7-alpine
```

3. Configure environment variables:
```bash
export TIMESCALE_HOST=localhost
export TIMESCALE_DATABASE=neural_trader
export TIMESCALE_USER=postgres
export TIMESCALE_PASSWORD=postgres
export REDIS_HOST=localhost
export REDIS_PORT=6379
```

4. Run the service:
```bash
python -m data_ingestion.main start --symbols AAPL MSFT GOOGL
```

## Usage

### Command Line Interface

```bash
# Start data ingestion with specific providers and symbols
python -m data_ingestion.main start \
  --providers yahoo_finance finnhub \
  --symbols AAPL GOOGL MSFT AMZN TSLA \
  --realtime \
  --batch

# Backfill historical data
python -m data_ingestion.main backfill \
  --symbols AAPL MSFT \
  --start-date 2024-01-01 \
  --end-date 2024-07-03 \
  --providers yahoo_finance

# List available providers
python -m data_ingestion.main list-providers
```

### Python API

```python
import asyncio
from data_ingestion import RealtimeCoordinator, BatchScheduler

async def main():
    # Real-time streaming
    coordinator = RealtimeCoordinator()
    await coordinator.initialize(['yahoo_finance', 'finnhub'])
    await coordinator.subscribe(['AAPL', 'MSFT', 'GOOGL'])
    
    # Add callback for real-time data
    async def handle_data(data):
        print(f"Received: {data['symbol']} @ {data['close']}")
    
    coordinator.add_data_callback(handle_data)
    await coordinator.start()
    
    # Batch processing
    scheduler = BatchScheduler()
    await scheduler.initialize()
    
    # Schedule daily data collection
    await scheduler.schedule_job(
        'daily_update',
        '0 6 * * *',  # 6 AM daily
        {
            'symbols': ['AAPL', 'MSFT'],
            'lookback_days': 1,
            'interval': '1day'
        }
    )

asyncio.run(main())
```

### Data Access

```python
import asyncio
from data_ingestion.storage import TimescaleDB, RedisStore

async def query_data():
    # Connect to storage
    db = TimescaleDB()
    redis = RedisStore()
    
    await db.connect()
    await redis.connect()
    
    # Get historical data
    df = await db.query_market_data(
        symbol='AAPL',
        start_time=datetime(2024, 7, 1),
        end_time=datetime(2024, 7, 3)
    )
    print(df.head())
    
    # Get latest price from cache
    latest = await redis.get_latest_price('AAPL')
    print(f"Latest AAPL price: ${latest['close']}")
    
    # Subscribe to real-time updates
    await redis.subscribe_to_updates(['AAPL'], ['price'])
    async for update in redis.get_updates():
        print(f"Update: {update}")

asyncio.run(query_data())
```

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Data Providers │     │  Data Providers │     │  Data Providers │
│   (IEX Cloud)   │     │   (Polygon.io)  │     │ (Yahoo Finance) │
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                       │
         └───────────────────────┴───────────────────────┘
                                 │
                    ┌────────────┴────────────┐
                    │   Stream Manager        │
                    │  (Load Balancing)       │
                    └────────────┬────────────┘
                                 │
                ┌────────────────┴────────────────┐
                │                                 │
    ┌───────────┴───────────┐       ┌────────────┴────────────┐
    │  Realtime Coordinator │       │    Batch Scheduler      │
    │   (WebSocket/SSE)     │       │    (Cron Jobs)         │
    └───────────┬───────────┘       └────────────┬────────────┘
                │                                 │
                └────────────┬────────────────────┘
                             │
                  ┌──────────┴──────────┐
                  │   Data Processors   │
                  │ • Validation        │
                  │ • Cleaning          │
                  │ • Transformation    │
                  │ • Aggregation       │
                  └──────────┬──────────┘
                             │
                ┌────────────┴────────────────┐
                │                             │
    ┌───────────┴───────────┐   ┌────────────┴────────────┐
    │     TimescaleDB       │   │        Redis            │
    │  (Historical Data)    │   │   (Real-time Cache)     │
    └───────────────────────┘   └─────────────────────────┘
```

## Configuration

### Environment Variables

```bash
# API Keys
IEX_CLOUD_API_KEY=pk_xxxxx
ALPHA_VANTAGE_API_KEY=xxxxx
POLYGON_API_KEY=xxxxx
FINNHUB_API_KEY=xxxxx

# Database Configuration
TIMESCALE_HOST=localhost
TIMESCALE_PORT=5432
TIMESCALE_DATABASE=neural_trader
TIMESCALE_USER=trader
TIMESCALE_PASSWORD=password

# Redis Configuration
REDIS_HOST=localhost
REDIS_PORT=6379
REDIS_PASSWORD=password
REDIS_DB=0

# Service Configuration
LOG_LEVEL=INFO
LOG_FORMAT=json
PROMETHEUS_ENABLED=true
PROMETHEUS_PORT=9090
MAX_REQUESTS_PER_MINUTE=60
MAX_CONCURRENT_REQUESTS=10
BATCH_SIZE=1000
PROCESSING_INTERVAL_SECONDS=60
```

### Provider Configuration

Each provider has specific features and limitations:

| Provider | Real-time | Historical | Free Tier | Rate Limit |
|----------|-----------|------------|-----------|------------|
| Yahoo Finance | Polling | Yes | Yes | Unlimited* |
| Finnhub | WebSocket | Yes | Yes | 60/min |
| Alpha Vantage | No | Yes | Yes | 5/min |
| IEX Cloud | SSE | Yes | Limited | 100/sec |
| Polygon | WebSocket | Yes | No | 5/sec |

*Yahoo Finance has no official rate limit but should be used respectfully

## Monitoring

### Prometheus Metrics

- `data_points_processed_total`: Total data points processed by provider
- `api_requests_total`: API requests by provider
- `validation_failures_total`: Data validation failures
- `storage_operations_duration_seconds`: Storage operation latency
- `active_connections`: Active connections by type
- `stream_health`: Health score of data streams
- `batch_job_duration_seconds`: Batch job execution time

### Grafana Dashboards

Pre-configured dashboards are available in `grafana/dashboards/`:
- Data Ingestion Overview
- Provider Performance
- Storage Metrics
- Real-time Stream Health

## Development

### Running Tests

```bash
# Install dev dependencies
pip install -r requirements-dev.txt

# Run tests
pytest tests/ -v

# Run with coverage
pytest tests/ --cov=data_ingestion --cov-report=html
```

### Code Quality

```bash
# Format code
black data_ingestion/

# Sort imports
isort data_ingestion/

# Type checking
mypy data_ingestion/

# Linting
flake8 data_ingestion/
```

## Troubleshooting

### Common Issues

1. **Rate Limit Errors**
   - Reduce `MAX_REQUESTS_PER_MINUTE` in settings
   - Use fewer providers simultaneously
   - Implement provider rotation

2. **Connection Timeouts**
   - Check network connectivity
   - Verify API keys are valid
   - Increase timeout settings

3. **Data Quality Issues**
   - Check validation logs
   - Review outlier detection settings
   - Verify provider data format hasn't changed

4. **Performance Issues**
   - Monitor Prometheus metrics
   - Check database indexes
   - Scale horizontally with multiple instances

## License

This project is part of the Neural Trader system.