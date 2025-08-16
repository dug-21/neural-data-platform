# Neural Trader - Data Flow Architecture

## Data Flow Overview

The Neural Trader platform processes data through multiple interconnected pipelines, from raw market data ingestion to autonomous trading decisions. This document details the current data flows as implemented in the system.

## Primary Data Flows

### 1. Real-Time Market Data Flow

```mermaid
flowchart TD
    %% Data Sources
    subgraph "Market Data Sources"
        WS_ALPACA[Alpaca WebSocket<br/>Real-time trades/quotes]
        API_POLYGON[Polygon API<br/>Market data]
        API_YAHOO[Yahoo Finance<br/>Price data]
        API_NEWS[News APIs<br/>Sentiment data]
    end

    %% Ingestion Layer
    subgraph "Data Ingestion (Python)"
        PROVIDER_MGR[Provider Manager]
        STREAM_COORD[Stream Coordinator]
        DATA_PROC[Data Processor]
        VALIDATOR[Data Validator]
        NORMALIZER[Data Normalizer]
    end

    %% Storage Layer
    subgraph "Storage Systems"
        REDIS_PUB[Redis Pub/Sub<br/>Channel: market:*]
        REDIS_CACHE[Redis Cache<br/>TTL: 3600s]
        TIMESCALE[TimescaleDB<br/>Hypertables]
    end

    %% Processing Layer
    subgraph "Neural Processing (Rust)"
        REDIS_SUB[Redis Subscriber]
        FEATURE_EXT[Feature Extractor]
        NEURAL_ENS[Neural Ensemble]
        PREDICTOR[Vendor Predictor]
    end

    %% Decision Layer
    subgraph "Decision Making"
        DAA_COORD[DAA Coordinator]
        STRATEGY_MGR[Strategy Manager]
        RISK_MGR[Risk Manager]
        DECISION_LOG[Decision Logger]
    end

    %% Data Flow Connections
    WS_ALPACA --> PROVIDER_MGR
    API_POLYGON --> PROVIDER_MGR
    API_YAHOO --> PROVIDER_MGR
    API_NEWS --> PROVIDER_MGR

    PROVIDER_MGR --> STREAM_COORD
    STREAM_COORD --> DATA_PROC
    DATA_PROC --> VALIDATOR
    VALIDATOR --> NORMALIZER

    NORMALIZER --> REDIS_PUB
    NORMALIZER --> REDIS_CACHE
    NORMALIZER --> TIMESCALE

    REDIS_PUB --> REDIS_SUB
    REDIS_SUB --> FEATURE_EXT
    FEATURE_EXT --> NEURAL_ENS
    NEURAL_ENS --> PREDICTOR

    PREDICTOR --> DAA_COORD
    DAA_COORD --> STRATEGY_MGR
    STRATEGY_MGR --> RISK_MGR
    RISK_MGR --> DECISION_LOG

    DECISION_LOG --> REDIS_CACHE
    DECISION_LOG --> TIMESCALE
```

### 2. Historical Data Processing Flow

```mermaid
flowchart LR
    %% Historical Sources
    subgraph "Historical Data Sources"
        BACKFILL[Backfill Requests]
        BATCH_API[Batch API Calls]
        FILE_DATA[File Data Sources]
    end

    %% Processing Pipeline
    subgraph "Batch Processing"
        SCHEDULER[Batch Scheduler]
        PROCESSOR[Batch Processor]
        AGGREGATOR[Data Aggregator]
        QUALITY_CHECK[Quality Assurance]
    end

    %% Storage
    subgraph "Storage & Indexing"
        TS_HYPERTABLE[TimescaleDB Hypertables]
        CONTINUOUS_AGG[Continuous Aggregates]
        COMPRESSION[Data Compression]
        INDEXING[Performance Indexing]
    end

    %% Analysis
    subgraph "Analysis Pipeline"
        FEATURE_STORE[Feature Store]
        MODEL_TRAINING[Model Training Data]
        BACKTESTING[Backtesting Data]
    end

    %% Flow connections
    BACKFILL --> SCHEDULER
    BATCH_API --> SCHEDULER
    FILE_DATA --> SCHEDULER

    SCHEDULER --> PROCESSOR
    PROCESSOR --> AGGREGATOR
    AGGREGATOR --> QUALITY_CHECK

    QUALITY_CHECK --> TS_HYPERTABLE
    TS_HYPERTABLE --> CONTINUOUS_AGG
    CONTINUOUS_AGG --> COMPRESSION
    COMPRESSION --> INDEXING

    INDEXING --> FEATURE_STORE
    INDEXING --> MODEL_TRAINING
    INDEXING --> BACKTESTING
```

## Detailed Data Transformations

### 1. Market Data Normalization Pipeline

```mermaid
graph TB
    subgraph "Raw Data Formats"
        ALPACA_RAW[Alpaca Format<br/>{symbol, price, size, timestamp}]
        POLYGON_RAW[Polygon Format<br/>{T, sym, p, s, t}]
        YAHOO_RAW[Yahoo Format<br/>{Open, High, Low, Close, Volume}]
    end

    subgraph "Validation Layer"
        SCHEMA_VAL[Schema Validation<br/>- Required fields<br/>- Type checking<br/>- Range validation]
        BUSINESS_VAL[Business Logic Validation<br/>- OHLC consistency<br/>- Volume >= 0<br/>- Timestamp validity]
        QUALITY_VAL[Quality Checks<br/>- Data gaps detection<br/>- Outlier identification<br/>- Duplicate removal]
    end

    subgraph "Transformation Layer"
        NORMALIZE[Standardization<br/>- Decimal precision<br/>- Timezone conversion<br/>- Symbol mapping]
        ENRICH[Data Enrichment<br/>- Provider metadata<br/>- Data quality scores<br/>- Calculation fields]
        AGGREGATE[Aggregation<br/>- OHLCV bars<br/>- Volume-weighted prices<br/>- Time bucketing]
    end

    subgraph "Standard Format"
        STANDARD[Normalized Market Data<br/>{<br/>  time: timestamptz,<br/>  symbol: varchar(10),<br/>  open/high/low/close: decimal(10,4),<br/>  volume: bigint,<br/>  provider: varchar(50),<br/>  metadata: jsonb<br/>}]
    end

    %% Validation flow
    ALPACA_RAW --> SCHEMA_VAL
    POLYGON_RAW --> SCHEMA_VAL
    YAHOO_RAW --> SCHEMA_VAL

    SCHEMA_VAL --> BUSINESS_VAL
    BUSINESS_VAL --> QUALITY_VAL

    %% Transformation flow
    QUALITY_VAL --> NORMALIZE
    NORMALIZE --> ENRICH
    ENRICH --> AGGREGATE

    %% Output
    AGGREGATE --> STANDARD
```

### 2. Neural Feature Engineering Pipeline

```mermaid
graph LR
    subgraph "Input Data"
        MARKET_DATA[Market Data<br/>OHLCV + Metadata]
        SENTIMENT_DATA[Sentiment Data<br/>News + Social]
        MACRO_DATA[Macro Data<br/>Economic indicators]
    end

    subgraph "Technical Indicators"
        PRICE_IND[Price Indicators<br/>- SMA/EMA<br/>- Bollinger Bands<br/>- RSI/MACD]
        VOLUME_IND[Volume Indicators<br/>- Volume SMA<br/>- OBV<br/>- Volume Profile]
        VOLATILITY_IND[Volatility Indicators<br/>- ATR<br/>- Historical Vol<br/>- GARCH estimates]
    end

    subgraph "Statistical Features"
        RETURNS[Returns<br/>- Log returns<br/>- Rolling statistics<br/>- Autocorrelation]
        MOMENTUM[Momentum<br/>- Price momentum<br/>- Volume momentum<br/>- Cross-sectional ranking]
        REGIME[Regime Detection<br/>- Trend classification<br/>- Volatility regimes<br/>- Market state]
    end

    subgraph "Cross-Asset Features"
        CORRELATION[Cross-Correlations<br/>- Pair correlations<br/>- Sector correlations<br/>- Market beta]
        SPILLOVER[Spillover Effects<br/>- Lead-lag relationships<br/>- Information flow<br/>- Network effects]
    end

    subgraph "Feature Vectors"
        SYMBOL_FEATURES[Symbol-Specific<br/>- Individual stock features<br/>- Company fundamentals<br/>- Technical patterns]
        SECTOR_FEATURES[Sector Features<br/>- Sector indices<br/>- Sector rotation<br/>- Industry factors]
        MARKET_FEATURES[Market Features<br/>- VIX/volatility<br/>- Interest rates<br/>- Economic calendar]
    end

    %% Feature engineering flow
    MARKET_DATA --> PRICE_IND
    MARKET_DATA --> VOLUME_IND
    MARKET_DATA --> VOLATILITY_IND
    MARKET_DATA --> RETURNS
    MARKET_DATA --> MOMENTUM
    MARKET_DATA --> REGIME

    SENTIMENT_DATA --> MOMENTUM
    MACRO_DATA --> MARKET_FEATURES

    PRICE_IND --> CORRELATION
    VOLUME_IND --> SPILLOVER
    RETURNS --> CORRELATION

    %% Feature aggregation
    PRICE_IND --> SYMBOL_FEATURES
    VOLUME_IND --> SYMBOL_FEATURES
    VOLATILITY_IND --> SYMBOL_FEATURES
    CORRELATION --> SECTOR_FEATURES
    SPILLOVER --> SECTOR_FEATURES
    REGIME --> MARKET_FEATURES
```

### 3. Neural Model Data Flow

```mermaid
sequenceDiagram
    participant FS as Feature Store
    participant VB as Vendor Bridge
    participant VP as Vendor Predictor
    participant MF as Model Factory
    participant EN as Ensemble
    participant DC as DAA Coordinator

    Note over FS,DC: Neural prediction pipeline

    FS->>VB: Request features for symbol
    VB->>VB: Convert to vendor format
    VB->>VP: TimeSeriesDataset<f32>

    VP->>MF: Request model for symbol/sector
    MF->>VP: Return BaseModel instance

    VP->>VP: Generate prediction
    VP->>EN: Individual model result

    Note over EN: Collect all model predictions

    EN->>EN: Calculate ensemble weights
    EN->>EN: Compute consensus prediction
    EN->>EN: Estimate confidence intervals

    EN->>DC: Final prediction + confidence
    DC->>DC: Combine with other signals
    DC->>DC: Make trading decision
```

## Data Storage Architecture

### 1. TimescaleDB Hypertable Structure

```mermaid
erDiagram
    MARKET_DATA {
        timestamptz time PK
        varchar symbol PK
        decimal open
        decimal high
        decimal low
        decimal close
        bigint volume
        varchar provider PK
        jsonb metadata
    }

    TICK_DATA {
        timestamptz time PK
        varchar symbol PK
        decimal price
        bigint size
        varchar exchange
        text conditions
        varchar provider PK
    }

    ORDER_BOOK {
        timestamptz time PK
        varchar symbol PK
        decimal bid_price
        bigint bid_size
        decimal ask_price
        bigint ask_size
        decimal mid_price
        decimal spread
        varchar provider PK
    }

    PREDICTIONS {
        timestamptz time PK
        varchar symbol
        varchar model_name
        int horizon
        decimal predicted_value
        decimal confidence
        decimal interval_low
        decimal interval_high
        jsonb metadata
    }

    TRADING_DECISIONS {
        timestamptz time PK
        uuid decision_id
        varchar symbol
        varchar action
        decimal confidence
        decimal position_size
        text reasoning
        varchar agent_id
        jsonb metadata
    }

    MARKET_DATA ||--o{ PREDICTIONS : generates
    PREDICTIONS ||--o{ TRADING_DECISIONS : influences
```

### 2. Redis Data Structures

```mermaid
graph TB
    subgraph "Redis Pub/Sub Channels"
        PUB_MARKET[Channel: market:SYMBOL<br/>Real-time market data]
        PUB_PREDICTIONS[Channel: predictions:SYMBOL<br/>Neural predictions]
        PUB_DECISIONS[Channel: decisions:SYMBOL<br/>Trading decisions]
        PUB_HEALTH[Channel: health:*<br/>System health events]
    end

    subgraph "Redis Cache Keys"
        CACHE_MARKET[Key: market:SYMBOL:latest<br/>TTL: 60s<br/>Latest market data]
        CACHE_PRED[Key: predictions:SYMBOL:MODEL<br/>TTL: 300s<br/>Model predictions]
        CACHE_FEATURES[Key: features:SYMBOL<br/>TTL: 900s<br/>Feature vectors]
        CACHE_CONFIG[Key: config:*<br/>TTL: 3600s<br/>Configuration cache]
    end

    subgraph "Redis Streams"
        STREAM_EVENTS[Stream: events<br/>System events log]
        STREAM_METRICS[Stream: metrics<br/>Performance metrics]
        STREAM_TRADES[Stream: trades<br/>Trading activity log]
    end

    %% Data flow through Redis
    PUB_MARKET -.->|consume| CACHE_MARKET
    CACHE_FEATURES -.->|input| PUB_PREDICTIONS
    PUB_PREDICTIONS -.->|consume| PUB_DECISIONS
    PUB_DECISIONS -.->|log| STREAM_TRADES
```

## Performance and Monitoring Data Flow

### 1. Metrics Collection Pipeline

```mermaid
flowchart TD
    subgraph "Application Metrics"
        DI_METRICS[Data Ingestion Metrics<br/>- WebSocket latency<br/>- Message throughput<br/>- Provider uptime]
        NT_METRICS[Neural Trader Metrics<br/>- Prediction latency<br/>- Model accuracy<br/>- Decision frequency]
        SYS_METRICS[System Metrics<br/>- CPU/Memory usage<br/>- Database connections<br/>- Cache hit rates]
    end

    subgraph "Metrics Aggregation"
        PROMETHEUS[Prometheus<br/>- Scrape endpoints<br/>- Store time series<br/>- Apply rules]
        ALERTING[Alert Manager<br/>- Threshold monitoring<br/>- Notification routing<br/>- Escalation policies]
    end

    subgraph "Visualization"
        GRAFANA[Grafana Dashboards<br/>- Real-time charts<br/>- Historical analysis<br/>- Custom alerts]
        API_METRICS[Metrics API<br/>- JSON endpoints<br/>- Custom queries<br/>- Export functionality]
    end

    %% Metrics flow
    DI_METRICS --> PROMETHEUS
    NT_METRICS --> PROMETHEUS
    SYS_METRICS --> PROMETHEUS

    PROMETHEUS --> ALERTING
    PROMETHEUS --> GRAFANA
    PROMETHEUS --> API_METRICS
```

### 2. Log Aggregation Flow

```mermaid
graph LR
    subgraph "Log Sources"
        APP_LOGS[Application Logs<br/>- Structured JSON<br/>- Error tracking<br/>- Performance logs]
        CONTAINER_LOGS[Container Logs<br/>- Docker stdout<br/>- System messages<br/>- Health checks]
        DB_LOGS[Database Logs<br/>- Query performance<br/>- Connection events<br/>- Error messages]
    end

    subgraph "Log Processing"
        COLLECTOR[Log Collector<br/>- Parse formats<br/>- Filter messages<br/>- Add metadata]
        AGGREGATOR[Log Aggregator<br/>- Merge streams<br/>- Deduplicate<br/>- Enrich context]
        INDEXER[Log Indexer<br/>- Full-text search<br/>- Time-based indexing<br/>- Tag classification]
    end

    subgraph "Log Storage & Analysis"
        STORAGE[Log Storage<br/>- Time-series DB<br/>- Compressed storage<br/>- Retention policies]
        SEARCH[Search Interface<br/>- Query builder<br/>- Real-time search<br/>- Export tools]
        ALERTS[Log-based Alerts<br/>- Pattern matching<br/>- Anomaly detection<br/>- Threshold alerts]
    end

    %% Log processing flow
    APP_LOGS --> COLLECTOR
    CONTAINER_LOGS --> COLLECTOR
    DB_LOGS --> COLLECTOR

    COLLECTOR --> AGGREGATOR
    AGGREGATOR --> INDEXER

    INDEXER --> STORAGE
    STORAGE --> SEARCH
    STORAGE --> ALERTS
```

## Data Quality and Validation

### 1. Data Quality Framework

```mermaid
graph TB
    subgraph "Input Validation"
        SCHEMA_CHECK[Schema Validation<br/>- Required fields<br/>- Data types<br/>- Format compliance]
        RANGE_CHECK[Range Validation<br/>- Min/max values<br/>- Business rules<br/>- Consistency checks]
        FRESHNESS_CHECK[Freshness Validation<br/>- Timestamp checks<br/>- Lag detection<br/>- Real-time requirements]
    end

    subgraph "Quality Scoring"
        COMPLETENESS[Completeness Score<br/>- Missing data %<br/>- Field population<br/>- Time coverage]
        ACCURACY[Accuracy Score<br/>- Historical comparison<br/>- Cross-validation<br/>- Outlier detection]
        CONSISTENCY[Consistency Score<br/>- Cross-provider<br/>- OHLC validation<br/>- Volume checks]
    end

    subgraph "Quality Actions"
        ACCEPT[Accept Data<br/>- High quality score<br/>- Pass all validations<br/>- Normal processing]
        REPAIR[Repair Data<br/>- Fill missing values<br/>- Interpolate gaps<br/>- Apply corrections]
        REJECT[Reject Data<br/>- Critical failures<br/>- Inconsistent data<br/>- Alert operators]
    end

    %% Quality flow
    SCHEMA_CHECK --> COMPLETENESS
    RANGE_CHECK --> ACCURACY
    FRESHNESS_CHECK --> CONSISTENCY

    COMPLETENESS --> ACCEPT
    COMPLETENESS --> REPAIR
    ACCURACY --> ACCEPT
    ACCURACY --> REPAIR
    CONSISTENCY --> REJECT
```

### 2. Error Handling and Recovery

```mermaid
flowchart TD
    subgraph "Error Detection"
        DATA_ERROR[Data Errors<br/>- Invalid format<br/>- Missing fields<br/>- Range violations]
        SYSTEM_ERROR[System Errors<br/>- Connection failures<br/>- Timeout errors<br/>- Resource exhaustion]
        LOGIC_ERROR[Logic Errors<br/>- Business rule violations<br/>- Calculation errors<br/>- State inconsistencies]
    end

    subgraph "Error Classification"
        TRANSIENT[Transient Errors<br/>- Retry with backoff<br/>- Circuit breaker<br/>- Fallback providers]
        PERMANENT[Permanent Errors<br/>- Log and alert<br/>- Skip processing<br/>- Manual intervention]
        CRITICAL[Critical Errors<br/>- System shutdown<br/>- Data corruption<br/>- Security breaches]
    end

    subgraph "Recovery Actions"
        RETRY[Automatic Retry<br/>- Exponential backoff<br/>- Max retry limits<br/>- Success tracking]
        FALLBACK[Fallback Systems<br/>- Secondary providers<br/>- Cached data<br/>- Default values]
        ALERT[Alert & Monitor<br/>- Immediate notification<br/>- Escalation procedures<br/>- Recovery tracking]
    end

    %% Error handling flow
    DATA_ERROR --> TRANSIENT
    SYSTEM_ERROR --> TRANSIENT
    LOGIC_ERROR --> PERMANENT

    TRANSIENT --> RETRY
    TRANSIENT --> FALLBACK
    PERMANENT --> ALERT
    CRITICAL --> ALERT
```

---

*This document provides a comprehensive view of all major data flows in the Neural Trader platform, from raw data ingestion through neural processing to autonomous trading decisions. Each flow is designed for high performance, reliability, and scalability in production environments.*