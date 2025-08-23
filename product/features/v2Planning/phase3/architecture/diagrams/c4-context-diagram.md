# C4 Context Diagram - Neural Trader V2

## System Context (Level 1)

```mermaid
graph TB
    subgraph "Trading Ecosystem"
        User[("Traders<br/>Professional traders and<br/>algorithmic trading teams")]
        Admin[("System Administrators<br/>Platform operations and<br/>monitoring teams")]
        Analyst[("Analysts<br/>Performance analysis and<br/>strategy optimization")]
    end
    
    subgraph "Neural Trader Platform"
        NT["<b>Neural Trader System</b><br/>Automated trading platform with<br/>ML-powered strategy execution"]
    end
    
    subgraph "External Systems"
        Exchange1["Exchange APIs<br/>Binance, Coinbase, Kraken"]
        DataProvider["Market Data Providers<br/>Bloomberg, Reuters, Polygon"]
        MLInfra["ML Infrastructure<br/>Model training clusters,<br/>GPU compute"]
        Monitor["Monitoring Services<br/>DataDog, PagerDuty"]
        Auth["Authentication Provider<br/>Auth0/Okta"]
    end
    
    User -->|"Submit orders,<br/>Configure strategies,<br/>View performance"| NT
    Admin -->|"Monitor system,<br/>Deploy updates,<br/>Manage infrastructure"| NT
    Analyst -->|"Analyze performance,<br/>Backtest strategies,<br/>Generate reports"| NT
    
    NT -->|"Execute orders,<br/>Subscribe market data"| Exchange1
    NT -->|"Fetch historical data,<br/>Real-time feeds"| DataProvider
    NT -->|"Train models,<br/>Run inference"| MLInfra
    NT -->|"Send metrics,<br/>Trigger alerts"| Monitor
    NT -->|"Authenticate users,<br/>Manage permissions"| Auth
    
    style NT fill:#1168bd,stroke:#0b4884,color:#ffffff
    style User fill:#08427b,stroke:#052e56,color:#ffffff
    style Admin fill:#08427b,stroke:#052e56,color:#ffffff
    style Analyst fill:#08427b,stroke:#052e56,color:#ffffff
```

## Container Diagram (Level 2)

```mermaid
graph TB
    subgraph "Neural Trader Platform"
        subgraph "Edge Layer"
            WebApp["Web Application<br/>(React/TypeScript)<br/>Trading UI and dashboards"]
            APIGateway["API Gateway<br/>(Kong/Envoy)<br/>Request routing and auth"]
            WSGateway["WebSocket Gateway<br/>(Node.js)<br/>Real-time streaming"]
        end
        
        subgraph "Platform Services"
            EventBus["Event Bus<br/>(Redis Streams)<br/>Message broker"]
            Registry["Domain Registry<br/>(etcd/Consul)<br/>Service discovery"]
            MLOps["ML Ops Platform<br/>(MLflow/Kubeflow)<br/>Model management"]
        end
        
        subgraph "Trading Domain"
            MarketData["Market Data Service<br/>(Rust)<br/>Data ingestion & normalization"]
            Strategy["Strategy Engine<br/>(Rust)<br/>Signal generation & execution"]
            OrderMgmt["Order Management<br/>(Rust)<br/>Order lifecycle & routing"]
        end
        
        subgraph "Analytics Domain"
            Performance["Performance Analytics<br/>(Python)<br/>P&L and metrics calculation"]
            Backtest["Backtesting Engine<br/>(Rust/Python)<br/>Historical simulation"]
        end
        
        subgraph "ML Domain"
            Neural["Neural Predictor<br/>(Python/FANN)<br/>Price prediction models"]
            Feature["Feature Engineering<br/>(Python)<br/>Feature computation"]
        end
        
        subgraph "Data Layer"
            TimeSeries[("TimescaleDB<br/>Time-series data")]
            Cache[("Redis Cluster<br/>Cache & sessions")]
            ObjectStore[("MinIO/S3<br/>Model & file storage")]
        end
    end
    
    WebApp --> APIGateway
    APIGateway --> Strategy
    APIGateway --> OrderMgmt
    APIGateway --> Performance
    
    WSGateway --> EventBus
    
    MarketData --> EventBus
    EventBus --> Strategy
    Strategy --> OrderMgmt
    Strategy --> EventBus
    
    MarketData --> TimeSeries
    MarketData --> Cache
    
    Feature --> Neural
    Neural --> Strategy
    
    Strategy --> Performance
    Performance --> Backtest
    
    MLOps --> Neural
    MLOps --> ObjectStore
    
    Registry -.-> MarketData
    Registry -.-> Strategy
    Registry -.-> OrderMgmt
    
    style EventBus fill:#1168bd,stroke:#0b4884,color:#ffffff
    style Strategy fill:#1168bd,stroke:#0b4884,color:#ffffff
    style Neural fill:#1168bd,stroke:#0b4884,color:#ffffff
```

## Component Diagram - Strategy Engine (Level 3)

```mermaid
graph TB
    subgraph "Strategy Engine Service"
        subgraph "API Layer"
            gRPC["gRPC Server<br/>Service interface"]
            REST["REST API<br/>HTTP interface"]
        end
        
        subgraph "Core Components"
            Manager["Strategy Manager<br/>Lifecycle management"]
            Executor["Strategy Executor<br/>Execution orchestration"]
            Scheduler["Task Scheduler<br/>Time-based triggers"]
        end
        
        subgraph "Processing Components"
            Signal["Signal Processor<br/>Signal generation"]
            Aggregator["Signal Aggregator<br/>Multi-strategy aggregation"]
            Risk["Risk Manager<br/>Risk checks & limits"]
            Position["Position Tracker<br/>Position management"]
        end
        
        subgraph "Strategy Types"
            MA["Moving Average<br/>Strategy"]
            ML["ML-Based<br/>Strategy"]
            Arb["Arbitrage<br/>Strategy"]
            Custom["Custom<br/>Strategies"]
        end
        
        subgraph "External Interfaces"
            MDClient["Market Data<br/>Client"]
            OMClient["Order Management<br/>Client"]
            MLClient["ML Platform<br/>Client"]
            EventClient["Event Bus<br/>Client"]
        end
    end
    
    gRPC --> Manager
    REST --> Manager
    
    Manager --> Executor
    Manager --> Scheduler
    
    Executor --> MA
    Executor --> ML
    Executor --> Arb
    Executor --> Custom
    
    MA --> Signal
    ML --> Signal
    Arb --> Signal
    Custom --> Signal
    
    Signal --> Aggregator
    Aggregator --> Risk
    Risk --> Position
    Position --> OMClient
    
    MDClient --> MA
    MDClient --> ML
    MDClient --> Arb
    
    MLClient --> ML
    
    Signal --> EventClient
    Position --> EventClient
    
    style Executor fill:#1168bd,stroke:#0b4884,color:#ffffff
    style Signal fill:#1168bd,stroke:#0b4884,color:#ffffff
    style Risk fill:#1168bd,stroke:#0b4884,color:#ffffff
```

## Code Structure Diagram (Level 4)

```mermaid
classDiagram
    class StrategyEngine {
        -strategies: HashMap~StrategyId, Strategy~
        -executor: StrategyExecutor
        -scheduler: TaskScheduler
        -risk_manager: RiskManager
        +register_strategy(strategy: Strategy)
        +execute_strategy(id: StrategyId)
        +stop_strategy(id: StrategyId)
        +get_positions() Positions
    }
    
    class Strategy {
        <<interface>>
        +initialize(context: Context)
        +on_tick(tick: Tick) Signal
        +on_signal(signal: Signal) Order
        +on_order_update(update: OrderUpdate)
        +shutdown()
    }
    
    class MLStrategy {
        -model: NeuralModel
        -feature_buffer: FeatureBuffer
        -config: StrategyConfig
        +compute_features() Features
        +generate_signals(prediction: Prediction) Signal
    }
    
    class SignalProcessor {
        -filters: Vec~SignalFilter~
        -validators: Vec~SignalValidator~
        +process(signal: Signal) ProcessedSignal
        +validate(signal: Signal) bool
        +apply_filters(signal: Signal) Signal
    }
    
    class RiskManager {
        -limits: RiskLimits
        -portfolio: Portfolio
        -exposure_tracker: ExposureTracker
        +check_limits(signal: Signal) bool
        +calculate_position_size(signal: Signal) Decimal
        +update_exposure(position: Position)
    }
    
    class OrderManager {
        -order_book: OrderBook
        -venue_router: VenueRouter
        -execution_algos: HashMap~String, ExecutionAlgo~
        +submit_order(order: Order) OrderId
        +cancel_order(id: OrderId)
        +get_order_status(id: OrderId) OrderStatus
    }
    
    Strategy <|-- MLStrategy
    Strategy <|-- ArbitrageStrategy
    Strategy <|-- MovingAverageStrategy
    
    StrategyEngine --> Strategy
    StrategyEngine --> SignalProcessor
    StrategyEngine --> RiskManager
    StrategyEngine --> OrderManager
    
    MLStrategy --> NeuralModel
    SignalProcessor --> Signal
    RiskManager --> Portfolio
    OrderManager --> Order
```

## Deployment Diagram

```mermaid
graph TB
    subgraph "Google Cloud Platform - US East 1"
        subgraph "GKE Cluster - Production"
            subgraph "System Node Pool"
                Ingress["Ingress Controller<br/>NGINX"]
                Prometheus["Prometheus<br/>Metrics"]
                Grafana["Grafana<br/>Dashboards"]
            end
            
            subgraph "Compute Node Pool"
                subgraph "Trading Namespace"
                    MD1["Market Data<br/>Pod 1"]
                    MD2["Market Data<br/>Pod 2"]
                    MD3["Market Data<br/>Pod 3"]
                    
                    SE1["Strategy Engine<br/>Pod 1"]
                    SE2["Strategy Engine<br/>Pod 2"]
                    
                    OM1["Order Mgmt<br/>Pod 1"]
                    OM2["Order Mgmt<br/>Pod 2"]
                end
            end
            
            subgraph "ML Node Pool (GPU)"
                subgraph "ML Namespace"
                    ML1["ML Inference<br/>Pod 1"]
                    ML2["ML Training<br/>Pod 1"]
                    FE1["Feature Eng<br/>Pod 1"]
                end
            end
            
            subgraph "Data Node Pool"
                Redis["Redis Cluster<br/>StatefulSet"]
                Postgres["PostgreSQL<br/>StatefulSet"]
                Minio["MinIO<br/>StatefulSet"]
            end
        end
        
        subgraph "External Services"
            LB["Cloud Load<br/>Balancer"]
            CDN["Cloud CDN"]
            Storage["Cloud Storage<br/>Backups"]
        end
    end
    
    Internet -->|HTTPS| LB
    LB --> Ingress
    Ingress --> MD1
    Ingress --> SE1
    Ingress --> OM1
    
    MD1 -.-> Redis
    SE1 -.-> Redis
    OM1 -.-> Postgres
    ML1 -.-> Minio
    
    style MD1 fill:#1168bd,stroke:#0b4884,color:#ffffff
    style SE1 fill:#1168bd,stroke:#0b4884,color:#ffffff
    style ML1 fill:#1168bd,stroke:#0b4884,color:#ffffff
```