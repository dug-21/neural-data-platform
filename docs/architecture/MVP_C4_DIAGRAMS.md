# MVP Data Architecture - C4 Model Diagrams

## C4 Context Diagram (Level 1)

```mermaid
graph TB
    subgraph "External Systems"
        ALPACA[Alpaca Markets API<br/>Market Data Provider]
        MONITORING[Operations Team<br/>System Monitoring]
    end
    
    subgraph "Neural Trading Platform MVP"
        PLATFORM[Neural Trading System<br/>Real-time trading signals]
    end
    
    subgraph "Users"
        ANALYST[Trading Analyst<br/>Views signals & performance]
    end
    
    ALPACA -->|WebSocket/REST<br/>Market Data| PLATFORM
    PLATFORM -->|Metrics & Logs| MONITORING
    ANALYST -->|Views Predictions| PLATFORM
    
    style PLATFORM fill:#08427b,color:#fff
    style ALPACA fill:#999,color:#fff
    style MONITORING fill:#999,color:#fff
    style ANALYST fill:#999,color:#fff
```

## C4 Container Diagram (Level 2)

```mermaid
graph TB
    subgraph "External"
        ALPACA[Alpaca Markets<br/>Market Data API]
        GRAFANA_USER[Monitoring User<br/>Views Dashboards]
    end
    
    subgraph "Neural Trading Platform MVP"
        subgraph "Data Ingestion Layer"
            WEBSOCKET[WebSocket Handler<br/>Python AsyncIO<br/>Receives real-time data]
            VALIDATOR[Data Validator<br/>Python<br/>Validates & normalizes]
        end
        
        subgraph "Message Bus"
            REDIS[Redis Streams<br/>In-Memory DB<br/>Event streaming]
        end
        
        subgraph "Processing Layer"
            CONSUMER[Stream Consumer<br/>Rust<br/>Processes events]
            PREDICTOR[Neural Predictor<br/>Rust + ONNX<br/>Generates signals]
        end
        
        subgraph "Storage Layer"
            TIMESCALE[TimescaleDB<br/>PostgreSQL<br/>Time-series storage]
            FILESYSTEM[Model Storage<br/>Local FS<br/>Model artifacts]
        end
        
        subgraph "Monitoring"
            PROMETHEUS[Prometheus<br/>Metrics DB<br/>Scrapes metrics]
            GRAFANA[Grafana<br/>Dashboards<br/>Visualization]
        end
    end
    
    %% Data Flow
    ALPACA -->|WebSocket| WEBSOCKET
    WEBSOCKET -->|Validates| VALIDATOR
    VALIDATOR -->|XADD| REDIS
    REDIS -->|XREAD| CONSUMER
    CONSUMER -->|Features| PREDICTOR
    PREDICTOR -->|Signals| REDIS
    
    %% Storage
    VALIDATOR -->|Batch Insert| TIMESCALE
    PREDICTOR -->|Load/Save| FILESYSTEM
    
    %% Monitoring
    WEBSOCKET -.->|Metrics| PROMETHEUS
    CONSUMER -.->|Metrics| PROMETHEUS
    PROMETHEUS -->|Query| GRAFANA
    GRAFANA_USER -->|Views| GRAFANA
    
    style REDIS fill:#dc382d,color:#fff
    style TIMESCALE fill:#336791,color:#fff
    style PROMETHEUS fill:#e6522c,color:#fff
    style GRAFANA fill:#f46800,color:#fff
```

## C4 Component Diagram - Data Ingestion (Level 3)

```mermaid
graph TB
    subgraph "Data Ingestion Container"
        subgraph "Connection Management"
            CONN_MGR[Connection Manager<br/>Handles WebSocket lifecycle]
            RECONN[Reconnection Handler<br/>Exponential backoff]
        end
        
        subgraph "Data Processing"
            PARSER[Message Parser<br/>Parses Alpaca format]
            VALIDATOR[Field Validator<br/>Validates data quality]
            NORMALIZER[Data Normalizer<br/>Standardizes format]
        end
        
        subgraph "Publishing"
            REDIS_PUB[Redis Publisher<br/>Async publishing]
            BATCH_WRITER[Batch Writer<br/>TimescaleDB inserts]
        end
        
        subgraph "Monitoring"
            METRICS[Metrics Collector<br/>Prometheus metrics]
            HEALTH[Health Check<br/>Liveness/Readiness]
        end
    end
    
    %% Internal flows
    CONN_MGR -->|Raw Message| PARSER
    CONN_MGR -->|On Disconnect| RECONN
    RECONN -->|Retry| CONN_MGR
    
    PARSER -->|Parsed Data| VALIDATOR
    VALIDATOR -->|Valid Data| NORMALIZER
    VALIDATOR -->|Invalid| METRICS
    
    NORMALIZER -->|Trade/Quote| REDIS_PUB
    NORMALIZER -->|Batch| BATCH_WRITER
    
    REDIS_PUB -->|Success/Fail| METRICS
    BATCH_WRITER -->|Success/Fail| METRICS
    
    style CONN_MGR fill:#08427b,color:#fff
    style VALIDATOR fill:#08427b,color:#fff
    style REDIS_PUB fill:#08427b,color:#fff
```

## C4 Component Diagram - Neural Processing (Level 3)

```mermaid
graph TB
    subgraph "Neural Processing Container"
        subgraph "Data Consumption"
            STREAM_READER[Stream Reader<br/>Redis consumer]
            EVENT_BUFFER[Event Buffer<br/>Ring buffer]
        end
        
        subgraph "Feature Engineering"
            FEATURE_EXT[Feature Extractor<br/>Technical indicators]
            FEATURE_CACHE[Feature Cache<br/>LRU cache]
            WINDOW_AGG[Window Aggregator<br/>Time windows]
        end
        
        subgraph "Model Inference"
            MODEL_LOADER[Model Loader<br/>Load from disk]
            PREDICTOR[Prediction Engine<br/>ONNX Runtime]
            ENSEMBLE[Ensemble Manager<br/>Multiple models]
        end
        
        subgraph "Signal Generation"
            SIGNAL_GEN[Signal Generator<br/>Buy/Sell/Hold]
            CONFIDENCE[Confidence Scorer<br/>Probability calc]
            PUBLISHER[Signal Publisher<br/>Redis streams]
        end
    end
    
    %% Internal flows
    STREAM_READER -->|Market Data| EVENT_BUFFER
    EVENT_BUFFER -->|Batch| FEATURE_EXT
    
    FEATURE_EXT -->|Features| FEATURE_CACHE
    FEATURE_CACHE -->|Cached| WINDOW_AGG
    WINDOW_AGG -->|Feature Vector| PREDICTOR
    
    MODEL_LOADER -->|Model| PREDICTOR
    PREDICTOR -->|Raw Output| ENSEMBLE
    ENSEMBLE -->|Prediction| SIGNAL_GEN
    
    SIGNAL_GEN -->|Signal| CONFIDENCE
    CONFIDENCE -->|Scored Signal| PUBLISHER
    
    style PREDICTOR fill:#08427b,color:#fff
    style FEATURE_EXT fill:#08427b,color:#fff
    style SIGNAL_GEN fill:#08427b,color:#fff
```

## Data Flow Sequence Diagram

```mermaid
sequenceDiagram
    participant A as Alpaca API
    participant W as WebSocket Handler
    participant V as Validator
    participant R as Redis Streams
    participant C as Consumer
    participant F as Feature Extractor
    participant M as Model
    participant S as Signal Publisher
    participant T as TimescaleDB
    
    A->>W: Trade Event
    W->>V: Parse & Validate
    
    alt Valid Data
        V->>R: XADD trades:AAPL
        V-->>T: Batch Insert (async)
        R->>C: XREAD (blocking)
        C->>F: Calculate Features
        F->>M: Predict
        M->>S: Generate Signal
        S->>R: XADD signals:AAPL
    else Invalid Data
        V-->>V: Log & Drop
        V-->>W: Continue
    end
    
    Note over W,S: Target Latency < 50ms
```

## Deployment Architecture Diagram

```mermaid
graph TB
    subgraph "Development Environment"
        subgraph "Docker Compose Stack"
            REDIS_DEV[Redis Container<br/>6379:6379]
            TIMESCALE_DEV[TimescaleDB Container<br/>5432:5432]
            APP_DEV[Application Container<br/>Data Ingestion + Neural]
            GRAFANA_DEV[Grafana Container<br/>3000:3000]
        end
    end
    
    subgraph "Production Environment (Phase 1)"
        subgraph "Single VM/Cloud Instance"
            subgraph "System Services"
                REDIS_PROD[Redis Service<br/>Persistent AOF]
                TIMESCALE_PROD[TimescaleDB Service<br/>With Backups]
            end
            
            subgraph "Application Services"
                INGESTION[Data Ingestion<br/>SystemD Service]
                NEURAL[Neural Processor<br/>SystemD Service]
            end
            
            subgraph "Monitoring Stack"
                PROMETHEUS_PROD[Prometheus<br/>Local Storage]
                GRAFANA_PROD[Grafana<br/>Dashboard Service]
            end
        end
    end
    
    subgraph "Production Environment (Phase 2+)"
        subgraph "Kubernetes Cluster"
            subgraph "Data Plane"
                POD1[Ingestion Pod<br/>Replicas: 2]
                POD2[Neural Pod<br/>Replicas: 3]
            end
            
            subgraph "Control Plane"
                SVC[Services<br/>LoadBalancer]
                CONFIG[ConfigMaps<br/>Secrets]
            end
            
            subgraph "Storage"
                PV[Persistent Volumes<br/>Model Storage]
            end
        end
    end
    
    style REDIS_DEV fill:#dc382d,color:#fff
    style REDIS_PROD fill:#dc382d,color:#fff
    style TIMESCALE_DEV fill:#336791,color:#fff
    style TIMESCALE_PROD fill:#336791,color:#fff
```

## Technology Decision Rationale

```mermaid
graph LR
    subgraph "Decision: Message Bus"
        KAFKA[Apache Kafka<br/>❌ Too complex for MVP]
        RABBITMQ[RabbitMQ<br/>❌ Extra dependency]
        REDIS_CHOICE[Redis Streams<br/>✅ Simple, fast, sufficient]
    end
    
    subgraph "Decision: Time-Series DB"
        INFLUX[InfluxDB<br/>❌ New query language]
        CLICKHOUSE[ClickHouse<br/>❌ Complex setup]
        TIMESCALE_CHOICE[TimescaleDB<br/>✅ PostgreSQL-based]
    end
    
    subgraph "Decision: Model Storage"
        S3[AWS S3<br/>❌ Cloud dependency]
        MINIO[MinIO<br/>❌ Extra service]
        FS_CHOICE[Local Filesystem<br/>✅ Simple for MVP]
    end
    
    style REDIS_CHOICE fill:#4CAF50,color:#fff
    style TIMESCALE_CHOICE fill:#4CAF50,color:#fff
    style FS_CHOICE fill:#4CAF50,color:#fff
    style KAFKA fill:#f44336,color:#fff
    style RABBITMQ fill:#f44336,color:#fff
    style INFLUX fill:#f44336,color:#fff
    style CLICKHOUSE fill:#f44336,color:#fff
    style S3 fill:#f44336,color:#fff
    style MINIO fill:#f44336,color:#fff
```

## Migration Path Visualization

```mermaid
graph TD
    subgraph "MVP Phase"
        MVP[Single Source<br/>Redis Streams<br/>Local Storage]
    end
    
    subgraph "Phase 2: Multi-Source"
        PHASE2[Multiple Sources<br/>Source Fallback<br/>Containerized]
    end
    
    subgraph "Phase 3: Scalability"
        PHASE3[Domain Registry<br/>Kafka Option<br/>S3 Storage<br/>Kubernetes]
    end
    
    subgraph "Phase 4: Full V2"
        V2[MCP Gateway<br/>Event Routing<br/>Auto-scaling<br/>Multi-tenant]
    end
    
    MVP -->|2-4 weeks| PHASE2
    PHASE2 -->|4-6 weeks| PHASE3
    PHASE3 -->|6-8 weeks| V2
    
    style MVP fill:#4CAF50,color:#fff
    style PHASE2 fill:#2196F3,color:#fff
    style PHASE3 fill:#FF9800,color:#fff
    style V2 fill:#9C27B0,color:#fff
```

## Performance Profile

```mermaid
graph LR
    subgraph "Latency Breakdown"
        direction TB
        INGEST[Data Ingestion<br/>5-10ms]
        VALIDATE[Validation<br/>2-3ms]
        PUBLISH[Redis Publish<br/>3-5ms]
        CONSUME[Consumer Read<br/>2-3ms]
        FEATURE[Feature Calc<br/>10-15ms]
        PREDICT[Model Inference<br/>10-20ms]
        SIGNAL[Signal Gen<br/>2-3ms]
    end
    
    INGEST --> VALIDATE
    VALIDATE --> PUBLISH
    PUBLISH --> CONSUME
    CONSUME --> FEATURE
    FEATURE --> PREDICT
    PREDICT --> SIGNAL
    
    Note1[Total: 35-50ms target]
```

## Resource Allocation

```mermaid
pie title "MVP Resource Distribution"
    "Redis Streams" : 20
    "TimescaleDB" : 25
    "Data Ingestion" : 15
    "Neural Processing" : 30
    "Monitoring" : 10
```

## Error Handling Flow

```mermaid
stateDiagram-v2
    [*] --> Running
    Running --> ConnectionError: Network Issue
    Running --> DataError: Invalid Data
    Running --> SystemError: Resource Issue
    
    ConnectionError --> Reconnecting: Retry Logic
    Reconnecting --> Running: Success
    Reconnecting --> Failed: Max Retries
    
    DataError --> Logging: Log & Skip
    Logging --> Running: Continue
    
    SystemError --> HealthCheck: Check Resources
    HealthCheck --> Recovery: Recoverable
    HealthCheck --> Shutdown: Critical
    
    Recovery --> Running: Recovered
    Failed --> [*]
    Shutdown --> [*]
```

## Monitoring Dashboard Layout

```mermaid
graph TB
    subgraph "Dashboard 1: Data Pipeline Health"
        M1[Ingestion Rate<br/>msgs/sec]
        M2[Validation Errors<br/>errors/min]
        M3[Redis Lag<br/>ms]
        M4[Connection Status<br/>UP/DOWN]
    end
    
    subgraph "Dashboard 2: Model Performance"
        M5[Predictions/min<br/>by symbol]
        M6[Avg Confidence<br/>0-1 score]
        M7[Latency p95<br/>ms]
        M8[Model Version<br/>current]
    end
    
    subgraph "Dashboard 3: System Resources"
        M9[CPU Usage<br/>%]
        M10[Memory Usage<br/>GB]
        M11[Redis Memory<br/>MB]
        M12[Disk I/O<br/>MB/s]
    end
```

These C4 diagrams provide a comprehensive visual representation of the MVP data architecture, clearly showing:

1. **System context** and external dependencies
2. **Container relationships** and data flow
3. **Component interactions** within each container
4. **Deployment options** from development to production
5. **Technology decisions** with rationale
6. **Migration path** to full V2 architecture
7. **Performance characteristics** and resource allocation

The diagrams complement the detailed MVP architecture document and provide stakeholders with clear visual understanding of the system design.