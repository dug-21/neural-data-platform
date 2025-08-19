# MVP Data Architecture for V2 Neural Trading Platform

## Executive Summary

This document outlines a **minimal viable product (MVP)** data architecture for the V2 neural trading platform. The design prioritizes simplicity, low latency, and a clear migration path to the full V2 architecture while maintaining production readiness.

## Architecture Overview

### Core Design Principles
- **Single responsibility per component**
- **Direct data paths (minimal hops)**
- **Fail-fast with simple recovery**
- **Observable by default**
- **Minimal external dependencies**

## 1. Data Ingestion MVP

### 1.1 Single Market Data Source Architecture

```yaml
Primary Data Source:
  provider: Alpaca Markets
  rationale:
    - Free tier available (IEX data)
    - Real-time WebSocket for trades/quotes
    - REST API for historical data
    - Built-in paper trading support
    - No complex authentication flow
  
  data_types:
    - real_time_trades
    - real_time_quotes
    - 1min_bars
    - daily_bars
```

### 1.2 Simplified Data Pipeline

```
[Alpaca WebSocket] --> [Data Validator] --> [Redis Stream] --> [Consumer]
         |
         v
   [Health Monitor]
```

**Components:**

```python
# Minimal WebSocket Handler
class MinimalAlpacaHandler:
    """Direct WebSocket to Redis pipeline"""
    
    async def handle_trade(self, trade):
        # 1. Basic validation
        if not self._validate_trade(trade):
            metrics.increment("invalid_trades")
            return
            
        # 2. Transform to standard format
        normalized = {
            "t": trade.timestamp,
            "s": trade.symbol,
            "p": float(trade.price),
            "v": int(trade.size),
            "src": "alpaca"
        }
        
        # 3. Direct publish to Redis
        await redis.xadd(
            f"trades:{trade.symbol}",
            normalized,
            maxlen=10000  # Ring buffer
        )
```

### 1.3 Data Validation Layer

```yaml
Validation Rules:
  - Required fields present
  - Price > 0 and < 1_000_000
  - Volume >= 0
  - Timestamp within 24 hours
  - Symbol in whitelist
  
Error Handling:
  - Log and drop invalid records
  - Increment error metrics
  - No retry logic in MVP
```

## 2. Event Bus MVP

### 2.1 Redis Streams Configuration

```yaml
Stream Structure:
  trades:{symbol}:
    - Capped at 10,000 messages
    - TTL: 24 hours
    - Consumer groups for multiple readers
  
  quotes:{symbol}:
    - Capped at 5,000 messages  
    - TTL: 1 hour
    - Best bid/ask only
    
  signals:{symbol}:
    - Model predictions
    - Capped at 1,000 messages
    - TTL: 1 hour
```

### 2.2 Simple Pub/Sub Pattern

```python
# Publisher
async def publish_market_data(symbol: str, data: dict):
    """Direct publish with no transformation"""
    stream_key = f"trades:{symbol}"
    await redis.xadd(stream_key, data)

# Consumer  
async def consume_market_data(symbol: str):
    """Simple blocking consumer"""
    stream_key = f"trades:{symbol}"
    last_id = "0"
    
    while True:
        messages = await redis.xread(
            {stream_key: last_id},
            block=1000  # 1 second timeout
        )
        
        for message in messages:
            yield message
            last_id = message.id
```

### 2.3 No Complex Routing

- Direct streams per symbol
- No topic exchanges or routing rules
- Consumer subscribes to specific symbols only
- No message transformation in transit

## 3. Data Flow Design

### 3.1 Real-Time Flow (Latency Target: <50ms)

```
WebSocket Trade Event (T+0ms)
    ↓
Basic Validation (T+5ms)
    ↓
Redis XADD (T+10ms)
    ↓
Consumer XREAD (T+15ms)
    ↓
Feature Calculation (T+25ms)
    ↓
Model Prediction (T+40ms)
    ↓
Signal Published (T+45ms)
```

### 3.2 Batch Processing Flow

```python
class SimpleBatchProcessor:
    """Process historical data in fixed windows"""
    
    async def process_1min_bars(self, symbol: str):
        # Fetch last 100 bars
        bars = await alpaca.get_bars(
            symbol, 
            timeframe="1Min",
            limit=100
        )
        
        # Calculate simple features
        features = {
            "sma_20": calculate_sma(bars, 20),
            "rsi_14": calculate_rsi(bars, 14),
            "volume_ratio": bars[-1].volume / mean_volume
        }
        
        # Store in Redis with TTL
        await redis.setex(
            f"features:{symbol}",
            json.dumps(features),
            expire=300  # 5 minutes
        )
```

## 4. Storage Architecture

### 4.1 Time-Series Storage (Simplified)

```sql
-- Single hypertable for all market data
CREATE TABLE market_data (
    time        TIMESTAMPTZ NOT NULL,
    symbol      VARCHAR(10) NOT NULL,
    price       DECIMAL(10,2),
    volume      BIGINT,
    data_type   VARCHAR(20),  -- 'trade', 'quote', 'bar'
    metadata    JSONB
);

-- Convert to hypertable with daily partitions
SELECT create_hypertable('market_data', 'time');

-- Single index for symbol+time queries
CREATE INDEX idx_symbol_time ON market_data (symbol, time DESC);

-- Automatic data retention (7 days for MVP)
SELECT add_retention_policy('market_data', INTERVAL '7 days');
```

### 4.2 Model Artifact Storage

```yaml
Storage Structure:
  /models/
    /{symbol}/
      /current/
        - model.pkl (pickled sklearn model)
        - metadata.json (version, metrics, timestamp)
      /archive/
        - model_v1.pkl
        - model_v2.pkl
        
  Implementation:
    - Local filesystem for MVP
    - Simple versioning (v1, v2, v3...)
    - No model registry
    - Manual rollback via file copy
```

### 4.3 Minimal Metadata

```python
# Simple metadata tracking
metadata = {
    "version": 1,
    "created_at": datetime.utcnow().isoformat(),
    "symbol": "AAPL",
    "features": ["sma_20", "rsi_14", "volume_ratio"],
    "accuracy": 0.65,
    "training_samples": 1000
}
```

## 5. Technology Stack

### 5.1 Core Components

| Component | Technology | Rationale |
|-----------|------------|-----------|
| Market Data | Alpaca WebSocket | Free, reliable, simple API |
| Message Bus | Redis Streams | Low latency, simple, built-in persistence |
| Time-Series DB | TimescaleDB | PostgreSQL-based, SQL familiar, auto-partitioning |
| Model Storage | Local Filesystem | Simple, no dependencies, easy debugging |
| Metrics | Prometheus | Standard, simple scraping, good ecosystem |
| Monitoring | Grafana | Works with Prometheus, simple dashboards |

### 5.2 Language Choices

```yaml
Data Ingestion Layer:
  language: Python 3.11+
  rationale:
    - Alpaca SDK available
    - AsyncIO for concurrent operations
    - Simple Redis client
    - Fast development
    
Neural Processing Layer:
  language: Rust
  rationale:
    - Performance for model inference
    - Memory safety
    - Existing codebase
    - Direct Redis integration
```

## 6. Performance Targets

### 6.1 MVP Metrics

```yaml
Latency:
  - Market data ingestion: < 10ms
  - Redis publish: < 5ms
  - Feature calculation: < 20ms
  - Model prediction: < 25ms
  - End-to-end: < 50ms

Throughput:
  - Messages/second: 1,000
  - Symbols tracked: 10
  - Concurrent models: 10

Reliability:
  - Uptime target: 95% (allows for maintenance)
  - Data loss tolerance: 1% (non-critical)
  - Recovery time: < 5 minutes
```

### 6.2 Resource Requirements

```yaml
Minimal Deployment:
  - 1 VM/Container: 4 CPU, 8GB RAM
  - Redis: 2GB RAM
  - TimescaleDB: 10GB disk, 2GB RAM
  - Total: ~15GB disk, 12GB RAM

Storage Growth:
  - ~100MB/day per symbol
  - 7-day retention = 700MB per symbol
  - 10 symbols = 7GB total
```

## 7. Migration Path to Full V2

### 7.1 Phase 1: MVP (Current)
- Single data source (Alpaca)
- Direct Redis streams
- Simple filesystem storage
- Manual deployment

### 7.2 Phase 2: Multi-Source
- Add Polygon.io for better data
- Implement source fallback
- Add data reconciliation
- Containerize components

### 7.3 Phase 3: Scalability
- Implement domain registry
- Add Kafka for higher throughput
- S3 for model storage
- Kubernetes deployment

### 7.4 Phase 4: Full V2
- MCP gateway layer
- Multiple data sources
- Complex event routing
- Auto-scaling
- Multi-tenancy

## 8. Data Schemas

### 8.1 Trade Event Schema

```json
{
  "timestamp": "2024-01-15T09:30:00.123Z",
  "symbol": "AAPL",
  "price": 195.50,
  "volume": 100,
  "source": "alpaca",
  "conditions": ["regular"]
}
```

### 8.2 Model Signal Schema

```json
{
  "timestamp": "2024-01-15T09:30:01.000Z",
  "symbol": "AAPL",
  "signal": "BUY",
  "confidence": 0.75,
  "model_version": "v1",
  "features": {
    "sma_20": 194.30,
    "rsi_14": 65.5,
    "volume_ratio": 1.2
  }
}
```

### 8.3 Feature Vector Schema

```json
{
  "symbol": "AAPL",
  "timestamp": "2024-01-15T09:30:00.000Z",
  "features": [
    194.30,  // sma_20
    65.5,    // rsi_14
    1.2,     // volume_ratio
    0.015,   // return_1min
    0.023    // volatility_5min
  ],
  "labels": {
    "price_5min": 196.00,
    "direction": 1
  }
}
```

## 9. Error Handling Strategy

### 9.1 Fail-Fast Approach

```python
class MVPErrorHandler:
    """Simple error handling - fail fast, recover quick"""
    
    async def handle_connection_error(self, error):
        # Log error
        logger.error(f"Connection failed: {error}")
        
        # Increment metric
        metrics.increment("connection_errors")
        
        # Wait and retry (exponential backoff)
        await asyncio.sleep(2 ** self.retry_count)
        self.retry_count += 1
        
        # Restart connection
        if self.retry_count < 5:
            await self.reconnect()
        else:
            # Alert and halt
            await self.alert_critical()
            sys.exit(1)
```

### 9.2 Data Quality Issues

- Drop invalid records (don't block pipeline)
- Log quality metrics for monitoring
- No complex reconciliation in MVP
- Manual intervention for persistent issues

## 10. Monitoring & Observability

### 10.1 Key Metrics

```yaml
System Health:
  - WebSocket connection status
  - Redis connection pool size
  - TimescaleDB query latency
  - Model prediction latency

Data Quality:
  - Records processed/second
  - Invalid record ratio
  - Data gaps detected
  - Symbol coverage

Business Metrics:
  - Predictions generated/minute
  - Model accuracy (simple)
  - Signal generation rate
```

### 10.2 Simple Dashboards

```yaml
Dashboard 1 - Data Pipeline:
  - Market data ingestion rate
  - Redis stream lag
  - Processing latency histogram
  - Error rate

Dashboard 2 - Model Performance:
  - Prediction count by symbol
  - Average confidence scores
  - Model version deployment
  - Simple P&L tracking
```

## 11. Security Considerations

### 11.1 MVP Security

```yaml
API Keys:
  - Environment variables only
  - Read-only market data access
  - No trading permissions in MVP

Network:
  - Private network for internal services
  - TLS for external connections
  - No public endpoints

Data:
  - No PII storage
  - Public market data only
  - Local storage (no cloud)
```

## 12. Development & Testing

### 12.1 Local Development

```bash
# Start minimal stack
docker-compose up -d redis timescaledb

# Run data ingestion
python -m data_ingestion.main --symbols AAPL,MSFT

# Monitor streams
redis-cli XREAD STREAMS trades:AAPL 0
```

### 12.2 Testing Strategy

```python
# Unit tests for validators
def test_trade_validation():
    trade = {"price": -1, "volume": 100}
    assert not validate_trade(trade)

# Integration test for pipeline
async def test_end_to_end_flow():
    # Publish test trade
    await publish_trade(test_trade)
    
    # Verify in Redis
    result = await redis.xread("trades:TEST")
    assert result[0]["price"] == test_trade.price
    
    # Verify in TimescaleDB
    row = await db.fetch_one(
        "SELECT * FROM market_data WHERE symbol = 'TEST'"
    )
    assert row.price == test_trade.price
```

## 13. Deployment Guide

### 13.1 Single Container Deployment

```dockerfile
FROM python:3.11-slim

# Install dependencies
COPY requirements.txt .
RUN pip install -r requirements.txt

# Copy application
COPY data_ingestion/ /app/data_ingestion/

# Set environment
ENV ALPACA_API_KEY=${ALPACA_API_KEY}
ENV REDIS_URL=redis://redis:6379

# Run application
CMD ["python", "-m", "data_ingestion.main"]
```

### 13.2 Docker Compose Stack

```yaml
version: '3.8'

services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    command: redis-server --appendonly yes

  timescaledb:
    image: timescale/timescaledb:latest-pg15
    environment:
      POSTGRES_PASSWORD: password
      POSTGRES_DB: market_data
    ports:
      - "5432:5432"
    volumes:
      - timescale_data:/var/lib/postgresql/data

  data_ingestion:
    build: .
    environment:
      ALPACA_API_KEY: ${ALPACA_API_KEY}
      REDIS_URL: redis://redis:6379
      DATABASE_URL: postgresql://postgres:password@timescaledb/market_data
    depends_on:
      - redis
      - timescaledb

volumes:
  redis_data:
  timescale_data:
```

## 14. Cost Analysis

### 14.1 MVP Infrastructure Costs

```yaml
Cloud Deployment (AWS/GCP):
  - t3.large instance: $60/month
  - 100GB SSD storage: $10/month
  - Network transfer: $5/month
  - Total: ~$75/month

Local/On-Premise:
  - Hardware: One-time $500-1000
  - Electricity: ~$10/month
  - Internet: Existing
  - Total: ~$10/month ongoing
```

## 15. Success Criteria

### 15.1 MVP Milestones

- [ ] Real-time data ingestion working for 1 symbol
- [ ] Data flowing through Redis streams
- [ ] Basic model making predictions
- [ ] Latency under 50ms end-to-end
- [ ] 24-hour stability test passed
- [ ] Basic monitoring dashboard operational

### 15.2 Production Readiness Checklist

- [ ] Error handling for all failure modes
- [ ] Metrics exposed for Prometheus
- [ ] Health check endpoints
- [ ] Graceful shutdown handling
- [ ] Data persistence verified
- [ ] Recovery procedures documented

## Conclusion

This MVP data architecture provides a simple, working foundation for the V2 neural trading platform. It prioritizes:

1. **Simplicity**: Minimal components and dependencies
2. **Performance**: Direct data paths for low latency
3. **Reliability**: Simple failure modes and recovery
4. **Observability**: Built-in metrics and monitoring
5. **Scalability Path**: Clear upgrade path to full V2

The architecture can be implemented and deployed within 1-2 weeks, providing immediate value while maintaining flexibility for future enhancements.