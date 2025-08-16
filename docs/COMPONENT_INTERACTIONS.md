# Neural Trader - Component Interaction Diagrams

## System Component Interactions

### High-Level Component Architecture

```mermaid
graph TB
    %% External Services
    subgraph "External Data Sources"
        EXT_ALPACA[Alpaca WebSocket API]
        EXT_POLYGON[Polygon.io REST API]
        EXT_YAHOO[Yahoo Finance API]
        EXT_NEWS[NewsAPI/Reddit API]
    end

    %% Data Ingestion Layer
    subgraph "Data Ingestion Service (Python:8000)"
        DI_MAIN[Main Coordinator]
        DI_PROVIDERS[Data Providers]
        DI_SCHEDULERS[Stream Schedulers]
        DI_PROCESSORS[Data Processors]
        DI_STORAGE[Storage Adapters]
        DI_HEALTH[Health Monitor]
    end

    %% Storage Layer
    subgraph "Data Platform"
        STORAGE_TS[(TimescaleDB:5432)]
        STORAGE_REDIS[(Redis:6379)]
        STORAGE_MODELS[Model Files]
    end

    %% Neural Processing
    subgraph "Neural Trader Engine (Rust:8080)"
        NT_MAIN[Main Application]
        NT_NEURAL[Neural Ensemble]
        NT_DAA[DAA Coordinator]
        NT_ADAPTERS[Data Adapters]
        NT_STRATEGIES[Trading Strategies]
        NT_HEALTH[Health Monitor]
    end

    %% Monitoring
    subgraph "Monitoring Stack"
        MON_PROMETHEUS[Prometheus:9090]
        MON_GRAFANA[Grafana:3000]
    end

    %% External connections
    EXT_ALPACA --> DI_PROVIDERS
    EXT_POLYGON --> DI_PROVIDERS
    EXT_YAHOO --> DI_PROVIDERS
    EXT_NEWS --> DI_PROVIDERS

    %% Data Ingestion internal flow
    DI_MAIN --> DI_PROVIDERS
    DI_MAIN --> DI_SCHEDULERS
    DI_MAIN --> DI_HEALTH
    DI_PROVIDERS --> DI_PROCESSORS
    DI_PROCESSORS --> DI_STORAGE
    DI_SCHEDULERS --> DI_STORAGE

    %% Storage connections
    DI_STORAGE --> STORAGE_TS
    DI_STORAGE --> STORAGE_REDIS
    NT_ADAPTERS --> STORAGE_TS
    NT_ADAPTERS --> STORAGE_REDIS
    NT_NEURAL --> STORAGE_MODELS

    %% Neural Trader internal flow
    NT_MAIN --> NT_ADAPTERS
    NT_MAIN --> NT_HEALTH
    NT_ADAPTERS --> NT_NEURAL
    NT_NEURAL --> NT_DAA
    NT_STRATEGIES --> NT_DAA
    NT_DAA --> NT_ADAPTERS

    %% Monitoring connections
    DI_HEALTH --> MON_PROMETHEUS
    NT_HEALTH --> MON_PROMETHEUS
    MON_PROMETHEUS --> MON_GRAFANA

    %% Cross-service communication via Redis
    DI_STORAGE -.->|pub/sub| NT_ADAPTERS
    NT_DAA -.->|decisions| DI_STORAGE
```

## Detailed Component Interactions

### 1. Data Ingestion Service Internal Architecture

```mermaid
graph LR
    subgraph "Data Providers Layer"
        ALPACA[AlpacaProvider]
        POLYGON[PolygonProvider]
        YAHOO[YahooProvider]
        NEWS[NewsProvider]
    end

    subgraph "Processing Layer"
        AGG[Aggregator]
        CLEAN[Cleaner]
        TRANS[Transformer]
        VALID[Validator]
    end

    subgraph "Storage Layer"
        TS_ADAPTER[TimescaleAdapter]
        REDIS_ADAPTER[RedisAdapter]
    end

    subgraph "Coordination Layer"
        REALTIME[RealtimeCoordinator]
        BATCH[BatchScheduler]
        STREAM[StreamManager]
    end

    %% Provider interactions
    ALPACA --> AGG
    POLYGON --> AGG
    YAHOO --> AGG
    NEWS --> AGG

    %% Processing pipeline
    AGG --> CLEAN
    CLEAN --> TRANS
    TRANS --> VALID

    %% Storage routing
    VALID --> TS_ADAPTER
    VALID --> REDIS_ADAPTER

    %% Coordination management
    REALTIME --> ALPACA
    REALTIME --> REDIS_ADAPTER
    BATCH --> POLYGON
    BATCH --> TS_ADAPTER
    STREAM --> AGG
```

### 2. Neural Trader Engine Internal Architecture

```mermaid
graph TB
    subgraph "Data Access Layer"
        TS_CLIENT[TimescaleClient]
        REDIS_CLIENT[RedisClient]
        MODEL_STORE[ModelStorage]
    end

    subgraph "Neural Processing Layer"
        VENDOR_PRED[VendorPredictor]
        MODEL_FACTORY[ModelFactory]
        ENSEMBLE[EnsembleManager]
        PERF_TRACK[PerformanceTracker]
    end

    subgraph "Decision Layer"
        DAA_COORD[DaaCoordinator]
        RISK_MGR[RiskManager]
        STRATEGY_EXEC[StrategyExecutor]
    end

    subgraph "Strategy Layer"
        MOMENTUM[MomentumStrategy]
        NEURAL_ENH[NeuralEnhancedStrategy]
    end

    %% Data flow
    TS_CLIENT --> VENDOR_PRED
    REDIS_CLIENT --> VENDOR_PRED
    MODEL_STORE --> MODEL_FACTORY

    %% Neural processing
    MODEL_FACTORY --> VENDOR_PRED
    VENDOR_PRED --> ENSEMBLE
    ENSEMBLE --> PERF_TRACK

    %% Decision making
    ENSEMBLE --> DAA_COORD
    MOMENTUM --> DAA_COORD
    NEURAL_ENH --> DAA_COORD
    DAA_COORD --> RISK_MGR
    RISK_MGR --> STRATEGY_EXEC

    %% Feedback loops
    PERF_TRACK --> MODEL_FACTORY
    STRATEGY_EXEC --> REDIS_CLIENT
    STRATEGY_EXEC --> TS_CLIENT
```

## Inter-Service Communication Patterns

### 1. Real-Time Data Flow

```mermaid
sequenceDiagram
    participant Alpaca as Alpaca WebSocket
    participant DI as Data Ingestion
    participant Redis as Redis Pub/Sub
    participant NT as Neural Trader
    participant TS as TimescaleDB

    Note over Alpaca,TS: Real-time market data processing

    Alpaca->>DI: WebSocket: Trade data
    DI->>DI: Process & validate
    
    par Parallel Storage
        DI->>Redis: PUBLISH market:AAPL
        DI->>TS: INSERT market_data
    end

    Redis->>NT: SUBSCRIBE market:*
    NT->>NT: Neural prediction
    NT->>NT: Trading decision
    
    par Decision Distribution
        NT->>Redis: PUBLISH decisions:AAPL
        NT->>TS: INSERT trading_decisions
    end
```

### 2. Neural Model Training Flow

```mermaid
sequenceDiagram
    participant NT as Neural Trader
    participant TS as TimescaleDB
    participant Models as Model Storage
    participant Redis as Redis Cache

    Note over NT,Redis: Model training and deployment

    NT->>TS: Query historical data
    TS->>NT: Return time series
    NT->>NT: Feature extraction
    NT->>NT: Model training
    NT->>Models: Save trained model
    NT->>Redis: Cache predictions
    NT->>Redis: Update model metrics
```

### 3. Health Check Coordination

```mermaid
sequenceDiagram
    participant Prometheus as Prometheus
    participant DI as Data Ingestion
    participant NT as Neural Trader
    participant TS as TimescaleDB
    participant Redis as Redis

    Note over Prometheus,Redis: System health monitoring

    loop Every 30 seconds
        Prometheus->>DI: GET /health
        DI->>TS: Check DB connection
        DI->>Redis: Check cache connection
        DI->>Prometheus: Health status + metrics
        
        Prometheus->>NT: GET /health
        NT->>TS: Check data access
        NT->>Redis: Check pub/sub
        NT->>Prometheus: Health status + metrics
    end
```

## Data Transformation Pipeline

### 1. Market Data Processing Chain

```mermaid
graph LR
    subgraph "Input Sources"
        RAW_WS[WebSocket Stream]
        RAW_API[REST API Data]
        RAW_FILE[File Data]
    end

    subgraph "Validation Layer"
        SCHEMA[Schema Validation]
        QUALITY[Quality Checks]
        DEDUPE[Deduplication]
    end

    subgraph "Transformation Layer"
        NORMALIZE[Normalization]
        ENRICH[Enrichment]
        FORMAT[Format Conversion]
    end

    subgraph "Output Destinations"
        TS_STORE[(TimescaleDB)]
        REDIS_CACHE[(Redis Cache)]
        METRICS[Prometheus Metrics]
    end

    %% Processing flow
    RAW_WS --> SCHEMA
    RAW_API --> SCHEMA
    RAW_FILE --> SCHEMA

    SCHEMA --> QUALITY
    QUALITY --> DEDUPE

    DEDUPE --> NORMALIZE
    NORMALIZE --> ENRICH
    ENRICH --> FORMAT

    FORMAT --> TS_STORE
    FORMAT --> REDIS_CACHE
    FORMAT --> METRICS
```

### 2. Neural Feature Pipeline

```mermaid
graph TB
    subgraph "Data Sources"
        MARKET[Market Data]
        SENTIMENT[Sentiment Data]
        MACRO[Macro Indicators]
    end

    subgraph "Feature Engineering"
        TECHNICAL[Technical Indicators]
        STATISTICAL[Statistical Features]
        TEMPORAL[Temporal Features]
        CROSS_ASSET[Cross-Asset Features]
    end

    subgraph "Feature Store"
        SHARED[Shared Features]
        SYMBOL[Symbol-Specific]
        SECTOR[Sector Features]
    end

    subgraph "Model Input"
        VECTOR[Feature Vectors]
        SEQUENCE[Sequence Data]
        METADATA[Metadata]
    end

    %% Feature extraction
    MARKET --> TECHNICAL
    MARKET --> STATISTICAL
    MARKET --> TEMPORAL
    SENTIMENT --> STATISTICAL
    MACRO --> CROSS_ASSET

    %% Feature storage
    TECHNICAL --> SHARED
    STATISTICAL --> SYMBOL
    TEMPORAL --> SYMBOL
    CROSS_ASSET --> SECTOR

    %% Model preparation
    SHARED --> VECTOR
    SYMBOL --> SEQUENCE
    SECTOR --> METADATA
```

## Decision Making Architecture

### 1. DAA Coordination Flow

```mermaid
graph TB
    subgraph "Input Signals"
        NEURAL[Neural Predictions]
        MOMENTUM[Momentum Signals]
        SENTIMENT[Sentiment Analysis]
        RISK[Risk Metrics]
    end

    subgraph "DAA Coordinator"
        CONSENSUS[Consensus Builder]
        CONFIDENCE[Confidence Scorer]
        THRESHOLD[Threshold Check]
        POSITION[Position Sizer]
    end

    subgraph "Risk Management"
        PORTFOLIO[Portfolio Check]
        EXPOSURE[Exposure Limits]
        VOLATILITY[Volatility Filter]
        STOP_LOSS[Stop Loss Logic]
    end

    subgraph "Execution"
        DECISION[Trading Decision]
        LOGGING[Decision Logging]
        MONITORING[Performance Tracking]
    end

    %% Signal processing
    NEURAL --> CONSENSUS
    MOMENTUM --> CONSENSUS
    SENTIMENT --> CONSENSUS
    RISK --> CONFIDENCE

    %% DAA coordination
    CONSENSUS --> CONFIDENCE
    CONFIDENCE --> THRESHOLD
    THRESHOLD --> POSITION

    %% Risk checks
    POSITION --> PORTFOLIO
    PORTFOLIO --> EXPOSURE
    EXPOSURE --> VOLATILITY
    VOLATILITY --> STOP_LOSS

    %% Final execution
    STOP_LOSS --> DECISION
    DECISION --> LOGGING
    DECISION --> MONITORING
```

### 2. Model Ensemble Coordination

```mermaid
graph LR
    subgraph "Individual Models"
        NHITS[NHITS Model]
        TCN[TCN Model]
        DEEPAR[DeepAR Model]
        TRANSFORMER[Transformer Model]
        MLP[MLP Baseline]
    end

    subgraph "Ensemble Layer"
        WEIGHTS[Weight Calculator]
        CONSENSUS[Consensus Builder]
        CONFIDENCE[Confidence Aggregator]
        UNCERTAINTY[Uncertainty Estimator]
    end

    subgraph "Output"
        PREDICTION[Final Prediction]
        INTERVALS[Confidence Intervals]
        METADATA[Prediction Metadata]
    end

    %% Model outputs
    NHITS --> WEIGHTS
    TCN --> WEIGHTS
    DEEPAR --> WEIGHTS
    TRANSFORMER --> WEIGHTS
    MLP --> WEIGHTS

    %% Ensemble processing
    WEIGHTS --> CONSENSUS
    CONSENSUS --> CONFIDENCE
    CONFIDENCE --> UNCERTAINTY

    %% Final output
    CONSENSUS --> PREDICTION
    UNCERTAINTY --> INTERVALS
    CONFIDENCE --> METADATA
```

---

*This document provides detailed component interaction diagrams for the Neural Trader platform, showing how the various services, models, and data flows work together in the current implementation.*