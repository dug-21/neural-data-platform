# C4 Architecture Diagrams - Neural Trading System

## Level 1: System Context Diagram

```mermaid
graph TB
    subgraph "External Systems"
        MS[Market Data Sources<br/>Exchanges, Reuters, Bloomberg]
        BR[Brokers & Exchanges<br/>Interactive Brokers, Binance]
        NEWS[News Sources<br/>Financial APIs]
        REG[Regulatory Systems<br/>Compliance Reporting]
    end
    
    subgraph "Users"
        TR[Traders<br/>Monitor & Override]
        RM[Risk Managers<br/>Set Limits & Controls]
        DO[DevOps<br/>System Management]
        QR[Quant Researchers<br/>Model Development]
    end
    
    NT[Neural Trading System<br/>Automated Trading Platform]
    
    MS -->|Market Data| NT
    NEWS -->|News & Signals| NT
    NT -->|Orders| BR
    BR -->|Executions| NT
    NT -->|Reports| REG
    
    TR -->|Monitoring| NT
    RM -->|Risk Controls| NT
    DO -->|Operations| NT
    QR -->|Models| NT
    
    style NT fill:#4A90E2,stroke:#333,stroke-width:3px
    style MS fill:#E8F4F8
    style BR fill:#E8F4F8
    style NEWS fill:#E8F4F8
    style REG fill:#E8F4F8
    style TR fill:#F0F8E8
    style RM fill:#F0F8E8
    style DO fill:#F0F8E8
    style QR fill:#F0F8E8
```

## Level 2: Container Diagram

```mermaid
graph TB
    subgraph "Neural Trading System"
        subgraph "Frontend Layer"
            WEB[Web Dashboard<br/>React/TypeScript]
            MOB[Mobile App<br/>React Native]
        end
        
        subgraph "API Layer"
            GW[API Gateway<br/>Kong/Envoy]
            WS[WebSocket Server<br/>Real-time Updates]
        end
        
        subgraph "Core Services"
            DRS[Domain Registry<br/>Service Discovery]
            DI[Data Ingestion<br/>Market Data Processing]
            EB[Event Bus<br/>Kafka/Pulsar]
            MLO[ML Ops Platform<br/>Model Training & Management]
            MEX[Model Execution<br/>Inference Engine]
            ACT[Action Layer<br/>Trade Execution]
        end
        
        subgraph "Data Layer"
            TS[Time Series DB<br/>TimescaleDB]
            PG[PostgreSQL<br/>Transactional Data]
            S3[Object Storage<br/>Model Artifacts]
            RED[Redis<br/>Cache & Sessions]
        end
        
        subgraph "Infrastructure"
            K8S[Kubernetes<br/>Orchestration]
            PROM[Prometheus<br/>Metrics]
            ELK[ELK Stack<br/>Logging]
        end
    end
    
    WEB --> GW
    MOB --> GW
    GW --> DRS
    GW --> ACT
    
    DI --> EB
    EB --> MLO
    MLO --> MEX
    MEX --> ACT
    
    DRS --> RED
    DI --> TS
    MLO --> S3
    ACT --> PG
    
    K8S --> |Manages| DRS
    K8S --> |Manages| DI
    K8S --> |Manages| MLO
    K8S --> |Manages| MEX
    K8S --> |Manages| ACT
    
    PROM --> |Monitors| K8S
    ELK --> |Logs| K8S
    
    style EB fill:#FF6B6B,stroke:#333,stroke-width:2px
    style MLO fill:#4ECDC4,stroke:#333,stroke-width:2px
    style MEX fill:#45B7D1,stroke:#333,stroke-width:2px
    style ACT fill:#96CEB4,stroke:#333,stroke-width:2px
```

## Level 3: Component Diagram - ML Ops Platform

```mermaid
graph TB
    subgraph "ML Ops Platform"
        subgraph "Feature Engineering"
            FE[Feature Extractor<br/>Stream Processing]
            FV[Feature Validator<br/>Quality Checks]
            FS[Feature Store<br/>Feast/Tecton]
        end
        
        subgraph "Model Training"
            TP[Training Pipeline<br/>Kubeflow/MLflow]
            EX[Experiment Tracker<br/>MLflow/W&B]
            HP[Hyperparameter Tuner<br/>Optuna/Ray Tune]
        end
        
        subgraph "Model Registry"
            MR[Model Registry<br/>MLflow Registry]
            MV[Model Versioning<br/>DVC/Git LFS]
            MA[Model Artifacts<br/>S3 Compatible]
        end
        
        subgraph "Model Serving"
            MS[Model Server<br/>TorchServe/TF Serving]
            AB[A/B Testing<br/>Feature Flags]
            MM[Model Monitor<br/>Drift Detection]
        end
        
        subgraph "Data Quality"
            DV[Data Validator<br/>Great Expectations]
            DD[Drift Detector<br/>Evidently AI]
            DQ[Quality Metrics<br/>Custom Rules]
        end
    end
    
    FE --> FV
    FV --> FS
    FS --> TP
    
    TP --> EX
    TP --> HP
    TP --> MR
    
    MR --> MV
    MV --> MA
    MA --> MS
    
    MS --> AB
    MS --> MM
    
    FV --> DV
    DV --> DD
    DD --> DQ
    DQ --> MM
    
    style FE fill:#E8BBF0
    style TP fill:#BBE8F0
    style MR fill:#F0E8BB
    style MS fill:#BBF0E8
    style DV fill:#F0BBBB
```

## Level 3: Component Diagram - Action Layer

```mermaid
graph TB
    subgraph "Action Layer"
        subgraph "Decision Processing"
            DV[Decision Validator<br/>Risk Rules]
            DO[Decision Optimizer<br/>Position Sizing]
            DQ[Decision Queue<br/>Priority Management]
        end
        
        subgraph "Risk Management"
            RV[Risk Validator<br/>Limit Checks]
            RC[Risk Calculator<br/>VaR, Sharpe]
            RM[Risk Monitor<br/>Real-time Tracking]
            EC[Emergency Control<br/>Kill Switch]
        end
        
        subgraph "Order Management"
            OM[Order Manager<br/>Lifecycle Management]
            OR[Order Router<br/>Smart Routing]
            OE[Order Executor<br/>Broker Interface]
        end
        
        subgraph "Position Management"
            PM[Position Manager<br/>Portfolio Tracking]
            PL[P&L Calculator<br/>Real-time P&L]
            PR[Position Reconciler<br/>Consistency Checks]
        end
        
        subgraph "Execution Feedback"
            FB[Feedback Collector<br/>Performance Metrics]
            FA[Feedback Analyzer<br/>Slippage Analysis]
            FL[Feedback Loop<br/>Model Updates]
        end
    end
    
    DV --> DO
    DO --> DQ
    DQ --> RV
    
    RV --> RC
    RC --> RM
    RM --> EC
    
    RV --> OM
    OM --> OR
    OR --> OE
    
    OE --> PM
    PM --> PL
    PL --> PR
    
    OE --> FB
    FB --> FA
    FA --> FL
    
    EC --> |Emergency Stop| OM
    EC --> |Close All| PM
    
    style DV fill:#FFE5B4
    style RV fill:#FFB4B4
    style OM fill:#B4E5FF
    style PM fill:#B4FFB4
    style FB fill:#E5B4FF
```

## Data Flow Sequence Diagram

```mermaid
sequenceDiagram
    participant MS as Market Source
    participant DI as Data Ingestion
    participant EB as Event Bus
    participant MLO as ML Ops
    participant MEX as Model Execution
    participant ACT as Action Layer
    participant BR as Broker
    
    MS->>DI: Market Data Stream
    DI->>DI: Validate & Normalize
    DI->>EB: Publish Event
    
    EB->>MLO: Stream Events
    MLO->>MLO: Extract Features
    MLO->>MLO: Validate Quality
    
    MLO->>MEX: Feature Vector
    MEX->>MEX: Run Inference
    MEX->>MEX: Generate Decision
    
    MEX->>ACT: Trading Decision
    ACT->>ACT: Validate Risk
    ACT->>ACT: Size Position
    
    ACT->>BR: Submit Order
    BR->>BR: Execute Trade
    BR->>ACT: Execution Report
    
    ACT->>EB: Feedback Event
    EB->>MLO: Performance Data
    MLO->>MLO: Update Model
```

## Deployment Architecture

```mermaid
graph TB
    subgraph "Production Environment"
        subgraph "Region 1 - Primary"
            subgraph "Kubernetes Cluster 1"
                NG1[Node Group 1<br/>CPU Optimized]
                NG2[Node Group 2<br/>GPU Enabled]
                NG3[Node Group 3<br/>Memory Optimized]
            end
            
            LB1[Load Balancer]
            DB1[(Primary Database)]
            CACHE1[(Redis Primary)]
        end
        
        subgraph "Region 2 - DR"
            subgraph "Kubernetes Cluster 2"
                NG4[Node Group 4<br/>CPU Optimized]
                NG5[Node Group 5<br/>GPU Enabled]
            end
            
            LB2[Load Balancer]
            DB2[(Standby Database)]
            CACHE2[(Redis Replica)]
        end
        
        subgraph "Shared Services"
            CDN[CDN<br/>CloudFlare]
            S3[Object Storage<br/>Models & Artifacts]
            KAFKA[Kafka Cluster<br/>Multi-AZ]
        end
    end
    
    CDN --> LB1
    CDN --> LB2
    
    LB1 --> NG1
    LB1 --> NG2
    LB1 --> NG3
    
    LB2 --> NG4
    LB2 --> NG5
    
    NG1 --> DB1
    NG2 --> DB1
    NG3 --> CACHE1
    
    NG4 --> DB2
    NG5 --> CACHE2
    
    DB1 -.->|Replication| DB2
    CACHE1 -.->|Replication| CACHE2
    
    NG1 --> KAFKA
    NG2 --> S3
    NG4 --> KAFKA
    NG5 --> S3
    
    style LB1 fill:#90EE90
    style LB2 fill:#90EE90
    style KAFKA fill:#FFD700
    style S3 fill:#87CEEB
```