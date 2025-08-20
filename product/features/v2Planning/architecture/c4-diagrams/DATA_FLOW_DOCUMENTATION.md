# V2 Neural Trader - Data Flow Documentation

## Architecture Overview

The V2 Neural Trader platform implements a dual-plane architecture:
- **Control Plane**: MCP-based administrative operations (low volume)
- **Data Plane**: Event streaming for high-volume data processing (millions of events/minute)

## Component Layer Summary

### 1. MCP Interface Container
- **Purpose**: Central control plane gateway
- **Technology**: TypeScript/Node.js with MCP Protocol
- **Key Components**: MCP Server Core, Tool Registry, Request Router, Session Manager

### 2. Data Ingestion Container  
- **Purpose**: Market data extraction and streaming
- **Technology**: Rust with Tokio async runtime
- **Key Components**: Market Data Connectors, Data Validators, Stream Publishers

### 3. Event Bus Platform Container
- **Purpose**: High-throughput event streaming backbone
- **Technology**: Redis Streams with Redis Cluster
- **Key Components**: Stream Partitioner, Consumer Groups, Event Store, Message Router

### 4. ML Ops Platform Container
- **Purpose**: Feature engineering and model training
- **Technology**: Rust with ruv-FANN neural library
- **Key Components**: Feature Pipeline, Feature Store, Training Manager, Model Registry

### 5. Model Execution Container
- **Purpose**: Autonomous decision making
- **Technology**: Rust with ruv-FANN and DAA frameworks
- **Key Components**: Neural Engine, DAA Coordinator, Consensus Manager, Strategy Engine

### 6. Action Layer Container
- **Purpose**: Trade execution and risk management
- **Technology**: Rust with broker-specific APIs
- **Key Components**: Trade Execution Engine, Risk Management, Portfolio Manager, Order Router

## Data Flow Patterns

### Primary Data Flow (Market Data → Trading) - CORRECTED

```
[External Market Data Sources]
         ↓ (WebSocket/REST)
[Data Ingestion Container]
    • Validates and normalizes data
    • Publishes to Redis Streams
         ↓ (Redis Streams - millions/min)
[Event Bus Platform]
    • Partitions by symbol/timestamp
    • Routes to consumers
    ↙ (Stream 1)        ↘ (Stream 2)
[ML Ops Platform]     [Model Execution Container]
 • Calculates features    ↑ (Trained Models)
 • Trains models          ↑ (From ML Ops)
 • Stores feature vectors  • Real-time data
         ↓ (Trained Models)   • Neural inference
[Model Registry/Store]     • DAA consensus
                          • Strategy execution
                               ↓ (Trading Decisions)
                    [Action Layer Container]
                          • Risk validation
                          • Order execution
                               ↓ (Orders)
                    [External Brokers]
```

**CRITICAL DATA FLOW CORRECTIONS:**

1. **SPLIT EVENT BUS OUTPUT**: Event Bus sends data to BOTH ML Ops and Model Execution
2. **ML OPS ROLE**: Processes EventBus data → Extracts features → Trains models → Stores models
3. **MODEL EXECUTION ROLE**: Gets trained models from ML Ops + real-time data from EventBus → Makes predictions
4. **NO BIDIRECTIONAL FLOW**: ML Ops does NOT directly communicate with Model Execution during runtime
5. **SEQUENTIAL DEPENDENCY**: ML Ops must complete feature extraction and training BEFORE Model Execution can predict

### Control Flow (MCP Administrative)

```
[Human/Claude]
         ↓ (MCP Protocol)
[MCP Interface Container]
    • Authenticates requests
    • Routes to appropriate container
         ↓ (MCP Tools)
[All Containers]
    • Configuration changes
    • Start/stop operations
    • Status queries
    • Emergency interventions
```

## Inter-Container Communication

### Data Plane Connections

| Source Container | Target Container | Protocol | Volume | Latency |
|-----------------|------------------|----------|--------|---------|
| Data Ingestion | Event Bus | Redis Streams | 10M+ events/min | <1ms |
| Event Bus | ML Ops Platform | Redis Streams | 10M+ events/min | <1ms |
| ML Ops Platform | Model Execution | Redis Streams | 1M+ features/min | <5ms |
| Model Execution | Action Layer | Direct RPC | 100k decisions/min | <10ms |
| Action Layer | External Brokers | FIX/REST | 50k orders/min | <50ms |

### Control Plane Connections

| Source | Target | Protocol | Purpose |
|--------|--------|----------|---------|
| MCP Interface | Data Ingestion | MCP | Start/stop feeds, configure sources |
| MCP Interface | Event Bus | MCP | Create streams, configure consumers |
| MCP Interface | ML Ops | MCP | Trigger training, deploy models |
| MCP Interface | Model Execution | MCP | Switch models, adjust strategies |
| MCP Interface | Action Layer | MCP | Emergency stops, risk overrides |

## Stream Topology

### Event Bus Stream Structure

```yaml
Market Data Streams:
  - market-data:{symbol}
  - market-data:{exchange}:{symbol}
  - market-data:aggregated

Feature Streams:
  - features:{symbol}:realtime
  - features:{symbol}:batch
  - features:technical-indicators

Prediction Streams:
  - predictions:{model}:{symbol}
  - predictions:ensemble
  - predictions:consensus

Decision Streams:
  - decisions:trading
  - decisions:risk
  - decisions:portfolio

Execution Streams:
  - executions:orders
  - executions:fills
  - executions:settlements
```

## Data Flow Characteristics

### Volume Metrics

- **Market Data Ingestion**: 10M+ ticks/minute
- **Feature Calculation**: 5M+ features/minute  
- **Model Predictions**: 1M+ predictions/minute
- **Trading Decisions**: 100k+ decisions/minute
- **Order Execution**: 50k+ orders/minute

### Latency Requirements

- **Market Data → Features**: <10ms P99
- **Features → Predictions**: <20ms P99
- **Predictions → Decisions**: <10ms P99
- **Decisions → Execution**: <50ms P99
- **End-to-End**: <100ms P99

### Scaling Patterns

#### Horizontal Scaling
- **Data Ingestion**: Scale by symbol/exchange
- **Event Bus**: Partition by key (symbol+timestamp)
- **ML Ops**: Distribute feature calculation
- **Model Execution**: Multiple prediction instances
- **Action Layer**: Load balance across brokers

#### Vertical Scaling
- **Neural Engine**: SIMD optimization
- **Feature Pipeline**: In-memory computation
- **Stream Processing**: Redis Cluster sharding

## Failure Handling

### Circuit Breakers
1. **Data Ingestion**: Fallback to backup feeds
2. **Event Bus**: Dead letter queues for failed messages
3. **ML Ops**: Cached features during outages
4. **Model Execution**: Fallback to simpler models
5. **Action Layer**: Emergency stop on anomalies

### Recovery Mechanisms
- **Event Replay**: Point-in-time recovery from Event Store
- **Model Rollback**: Quick reversion to previous versions
- **Position Recovery**: Reconciliation from broker state
- **Session Persistence**: Resume from last checkpoint

## Security & Compliance

### Data Security
- **Encryption**: TLS for all external connections
- **Authentication**: JWT tokens for MCP access
- **Authorization**: Role-based access control
- **Audit Trail**: All operations logged

### Regulatory Compliance
- **Trade Reporting**: Real-time regulatory feeds
- **Best Execution**: Smart order routing logs
- **Market Manipulation**: Pattern detection
- **Data Retention**: 7 years cold storage

## Monitoring & Observability

### Key Metrics
- **Throughput**: Events/second per stream
- **Latency**: P50, P95, P99 percentiles
- **Error Rates**: Failed messages, rejected orders
- **Resource Usage**: CPU, memory, network
- **Model Performance**: Accuracy, drift metrics

### Alerting Thresholds
- **Data Loss**: >0.01% message drop rate
- **Latency Spike**: >2x baseline P99
- **Model Drift**: >5% accuracy degradation
- **Risk Breach**: Position limit violations
- **System Health**: Component availability <99.9%

## Future Enhancements

### Planned Improvements
1. **Kafka Integration**: Alternative to Redis Streams for higher scale
2. **Multi-Region**: Geographic distribution for latency reduction
3. **Quantum Models**: Integration of quantum computing for complex strategies
4. **Blockchain Settlement**: DeFi integration for crypto trading
5. **Advanced DAA**: Self-improving autonomous agent networks

### Scalability Roadmap
- **Phase 1**: 10M events/minute (current design)
- **Phase 2**: 100M events/minute (Kafka + horizontal scaling)
- **Phase 3**: 1B events/minute (multi-region + edge computing)
- **Phase 4**: Unlimited (fully distributed, serverless)

---

This data flow documentation serves as the authoritative reference for understanding how data and control signals flow through the V2 Neural Trader platform. All component interactions must adhere to these patterns for consistency and maintainability.