# Neural Trader Platform - Startup Flow Documentation

## Overview
The Neural Trader platform follows a sophisticated multi-phase initialization process that sets up distributed AI trading capabilities, real-time market data processing, and autonomous training systems.

## Startup Sequence

### Phase 1: Core Initialization (Lines 318-376)

#### 1.1 Logging Setup
```rust
tracing_subscriber::fmt()
    .with_max_level(Level::INFO)
    .with_thread_ids(true)
    .with_file(true)
    .with_line_number(true)
    .init();
```
- Initializes structured logging with file/line numbers
- Thread IDs enabled for debugging concurrent operations

#### 1.2 Configuration Loading
```rust
let config = load_default_config()
```
**Loads:**
- Database connection settings (TimescaleDB)
- Redis cache configuration
- Neural network memory allocation (default: 4GB)
- Supported models: ["NHITS", "DeepAR", "TCN", "MLP", "Transformer"]

#### 1.3 Symbol Configuration
**Three symbol categories loaded:**
1. **Stock Symbols**: AAPL, MSFT, GOOGL, AMZN, NVDA, DDOG
2. **Sector ETFs**: XLK, XLF, XLV, XLE, XLY, XLP, XLI, XLB, XLU, XLRE
3. **All Symbols**: Combined list for comprehensive market coverage

#### 1.4 Feature Flag Detection
**Environment Variables Checked:**
- `ENABLE_SECTOR_MODELS`: ETF-based sector modeling
- `ENABLE_AUTONOMOUS_TRAINING`: Self-improving neural networks
- `ENABLE_REALTIME_ADAPTATION`: Live model adjustments
- `ENABLE_DATA_DISCOVERY`: Automatic data source detection
- `TRAINING_HISTORY_DAYS`: Historical data window (default: 90 days)

### Phase 2: Storage Layer Setup (Lines 377-408)

#### 2.1 TimescaleDB Initialization
- Creates connection pool to time-series database
- Optimized for high-frequency market data storage
- Handles continuous aggregates for efficient queries

#### 2.2 Redis Cache Setup
- Establishes Redis connection for hot data caching
- Pool size: 10 connections
- Used for real-time data streaming and model caching

#### 2.3 Data Access Layer (DAL)
- Unified interface for data operations
- Manages database queries and cache coordination
- Semaphore-limited training queries (max: 10 concurrent)

#### 2.4 Training Data Service
- Loads and prepares market data for model training
- Handles data normalization and feature engineering
- Symbol isolation for accurate model training

### Phase 3: Neural System Bootstrap (Lines 411-583)

#### 3.1 Neural Predictor Initialization
**Two modes based on `ENABLE_SECTOR_MODELS`:**

##### Mode A: Sector Model Support (Enabled)
1. Loads `sector_models.toml` from multiple paths:
   - `/var/lib/neural-trader/config/sector_models.toml` (Docker)
   - `neural-trader-config/sector_models.toml` (Local dev)
   - `config/sector_models.toml` (Alternative)
2. Creates sector-specific neural models
3. Initializes cluster pools for each sector
4. Maps symbols to appropriate sectors

##### Mode B: Standard Mode (Disabled)
- Uses default symbol list from environment
- Creates generic models without sector specialization

#### 3.2 Autonomous Training Setup
**If `ENABLE_AUTONOMOUS_TRAINING` is true:**
1. Sets training threshold (default: 1000 samples)
2. Spawns monitoring loop (5-minute intervals)
3. Prioritizes ETF sector models for training
4. Triggers automatic retraining when thresholds met

#### 3.3 Real-time Adaptation
- Enables live model weight adjustments
- Responds to market condition changes
- Maintains performance metrics

#### 3.4 Data Discovery
- Automatically identifies new data sources
- Validates data quality and consistency
- Integrates discovered data into training pipeline

### Phase 4: DAA Coordinator Setup (Lines 585-646)

#### 4.1 Market Hours Tracker
- Monitors NYSE, NASDAQ, and other exchange schedules
- Adjusts training intensity based on market hours
- Optimizes resource usage during off-hours

#### 4.2 DAA Coordinator Initialization
- Creates decision-making pipeline
- Channel capacity: 1000 decisions
- Integrates with neural predictor and market hours

#### 4.3 Strategy Registration
**Two primary strategies:**

1. **Momentum Strategy**
   - Risk limit: 2%
   - Position size: 10%
   - Traditional technical analysis

2. **Neural-Enhanced Strategy**
   - Risk limit: 2%
   - Position size: 10%
   - AI-powered predictions

### Phase 5: Communication Layer (Lines 648-712)

#### 5.1 Redis Adapter Setup
- Parses Redis URL for connection parameters
- Creates connection pool
- Enables pub/sub for market data streaming

#### 5.2 Event Bus Initialization
- Central message broker for system events
- Handles market data distribution
- Coordinates component communication

#### 5.3 Historical Data Loading
```rust
load_historical_data(&storage, &event_bus)
```
- Queries `market_data_1h` continuous aggregate
- Loads configurable time window (default: 90 days)
- Publishes historical events to event bus
- Cast volume data to prevent type mismatches

### Phase 6: ETF Model Bootstrap (Lines 715-761)

**Only runs if autonomous training enabled:**
1. Waits 30 seconds for system stabilization
2. Checks each ETF sector model for training status
3. Bootstrap process for untrained models:
   - Verifies model file existence
   - Checks for placeholder models
   - Triggers initial training via DAA
4. Skips already-trained models

### Phase 7: Health Monitoring (Lines 763-788)

#### 7.1 Component Registration
- Database health checks
- Redis connectivity monitoring
- Neural system status
- DAA orchestrator health

#### 7.2 Health Server
- HTTP endpoint: `http://0.0.0.0:9092/health`
- Request timeout: 30 seconds
- Provides system-wide health status

### Phase 8: Background Tasks (Lines 790-1335)

#### 8.1 Shutdown Handler
- Graceful shutdown on Ctrl+C
- Saves ETF model checkpoints before exit
- MCP server panic protection

#### 8.2 Decision Processing Loop
- Processes DAA trading decisions
- Executes via trading adapters
- Respects shutdown signals

#### 8.3 Checkpoint Scheduler
**Market-aware checkpoint saving:**
- **During Market Hours**: Light checkpointing (3 ETFs max)
- **After Hours**: Full checkpointing (all ETFs)
- 30-minute intervals

#### 8.4 Market Data Streaming
**Two modes based on `ENABLE_MULTI_CHANNEL`:**

##### Multi-Channel Mode (Enabled)
- Creates separate subscription per symbol
- Fair processing across all symbols
- Prevents single symbol from monopolizing resources
- Channel format: `market:{symbol}`

##### Single-Channel Mode (Disabled)
- Subscribes to general `market:updates` channel
- Processes all symbols sequentially
- Simpler but less scalable

#### 8.5 Event Bus Processing
- Monitors event bus for market updates
- Aggregates time-series data (10+ data points required)
- Calculates market metrics:
  - Volatility (standard deviation of returns)
  - Trend detection (moving average comparison)
  - Volume aggregation
- Triggers DAA decision-making
- Updates position tracking

### Phase 9: Main Event Loop (Lines 1335+)

**Continuous operation waiting for shutdown signal:**
1. Processes incoming market events
2. Makes trading decisions via DAA
3. Manages position lifecycle
4. Triggers model retraining on low confidence
5. Maintains system health

## Startup Validation Checks

### Critical Checks
1. ✅ Database connectivity
2. ✅ Redis cache availability
3. ✅ Neural model initialization
4. ✅ DAA coordinator setup
5. ✅ Health monitoring activation

### Non-Critical Checks (Warnings Only)
1. ⚠️ Sector configuration file presence
2. ⚠️ Historical data availability
3. ⚠️ Autonomous training enablement
4. ⚠️ Real-time adaptation setup

## Environment Variables

### Required
- `DATABASE_URL`: PostgreSQL connection string
- `REDIS_URL`: Redis connection string

### Optional with Defaults
- `TRAINING_HISTORY_DAYS`: 90
- `TRAINING_SAMPLE_THRESHOLD`: 1000
- `ENABLE_SECTOR_MODELS`: false
- `ENABLE_AUTONOMOUS_TRAINING`: false
- `ENABLE_REALTIME_ADAPTATION`: false
- `ENABLE_DATA_DISCOVERY`: false
- `ENABLE_MULTI_CHANNEL`: false
- `LOG_LEVEL`: INFO

## Startup Time Estimates

### Fast Path (Minimal Features)
- Configuration loading: ~100ms
- Storage setup: ~500ms
- Neural predictor: ~2s
- Total: **~3-5 seconds**

### Full Path (All Features)
- Configuration loading: ~100ms
- Storage setup: ~500ms
- Neural predictor with sectors: ~5s
- Historical data loading: ~2-10s (depends on data volume)
- ETF bootstrap: ~30s delay + training time
- Total: **~40-60 seconds**

## Error Recovery

### Database Connection Failure
- Immediate exit with error
- No fallback mode available

### Redis Connection Failure
- System continues with degraded performance
- No real-time streaming
- Falls back to database queries

### Model Loading Failure
- Creates new untrained models
- Triggers bootstrap training
- Continues with reduced accuracy

### Historical Data Loading Failure
- Warning logged
- System continues without historical context
- Real-time data processing still functional

## Memory Requirements

### Minimum (Single Model)
- Neural models: 1GB
- Data buffers: 512MB
- System overhead: 512MB
- **Total: 2GB**

### Recommended (Full Features)
- Neural models: 4GB
- Data buffers: 2GB
- Cache: 1GB
- System overhead: 1GB
- **Total: 8GB**

## Performance Optimizations

1. **Lazy Model Loading**: Individual symbol models created on-demand
2. **ETF Priority**: Sector models initialized first for broader coverage
3. **Market Hours Awareness**: Resource-intensive operations deferred to off-hours
4. **Multi-Channel Streaming**: Fair processing prevents data bottlenecks
5. **Continuous Aggregates**: Pre-computed hourly/daily data for faster queries

## Monitoring During Startup

### Key Metrics to Watch
- Database connection pool usage
- Redis memory consumption
- Model initialization success rate
- Historical data loading speed
- Event bus message throughput

### Log Patterns Indicating Issues
- `Failed to initialize`: Critical component failure
- `Failed to load sector config`: Missing configuration file
- `Insufficient data`: Not enough training samples
- `Failed to subscribe`: Redis streaming issues
- `Low confidence decision`: Model accuracy problems

## Post-Startup Validation

### Health Check Endpoint
```bash
curl http://localhost:9092/health
```

Expected response:
```json
{
  "status": "healthy",
  "components": {
    "database": "healthy",
    "redis": "healthy",
    "neural_system": "healthy",
    "daa_orchestrator": "healthy"
  }
}
```

### Verify Model Training
- Check logs for: "Successfully triggered autonomous retrain"
- Monitor checkpoint saves: "Saved checkpoint for ETF"
- Validate predictions: Confidence > 70% indicates good training

## Troubleshooting Guide

### Slow Startup
1. Check historical data volume
2. Verify network latency to database
3. Review model complexity settings
4. Consider disabling non-critical features

### Training Not Triggering
1. Verify `ENABLE_AUTONOMOUS_TRAINING=true`
2. Check data availability (need 100+ samples minimum)
3. Review `TRAINING_SAMPLE_THRESHOLD` setting
4. Inspect DAA coordinator logs

### Market Data Not Flowing
1. Confirm Redis connectivity
2. Check channel subscriptions
3. Verify data ingestion service running
4. Review event bus logs

### High Memory Usage
1. Reduce `NEURAL_MEMORY_GB` setting
2. Limit concurrent model count
3. Decrease cache sizes
4. Enable model compression