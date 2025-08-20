# MVP Architecture Decisions and Rationale

## Executive Summary

The MVP architecture represents a **radical simplification** of the V2 design, focusing on proving the core neural trading concept with minimal complexity. This approach validates the fundamental hypothesis - that neural networks can generate profitable trading signals - before investing in complex infrastructure.

## Core Architecture Decisions

### 1. Single Data Source (Alpaca Markets)
**Decision**: Use only Alpaca for both market data and paper trading execution.

**Rationale**:
- Free tier with comprehensive API
- WebSocket real-time feeds
- Built-in paper trading with $100k virtual capital
- No need for complex multi-source orchestration
- Reduces integration complexity by 80%

**Trade-offs**:
- Limited to US equities initially
- Single point of failure (mitigated by paper trading)
- No redundancy or failover

### 2. Single Neural Model (MLP)
**Decision**: Deploy one 20→64→32→1 MLP network instead of ensemble/consensus systems.

**Rationale**:
- Proves neural integration works
- Faster training iterations (2-5 minutes)
- Easier to debug and understand
- Clear performance attribution
- Reduces compute requirements by 5x

**Trade-offs**:
- Lower prediction accuracy
- No consensus validation
- Higher variance in predictions

### 3. Redis Streams Event Bus (MVP Choice)
**Decision**: Use Redis Streams as MVP EventBus with clear Kafka migration path.

**Rationale**:
- **Production Ready**: Consumer groups, persistence, monitoring
- **Target Performance**: 100K messages/second (meets MVP requirements)
- **Low Latency**: <10ms for trading, <50ms for analytics
- **Operational Simplicity**: Single Redis instance deployment
- **Cost Effective**: Minimal infrastructure overhead
- **Future Proof**: Clean migration path to Kafka when scaling needs exceed Redis

**MVP Stream Design**:
- Domain-based stream keys (trading:market-data, trading:signals, trading:actions)
- Service-based consumer groups (ingestion-group, model-exec-group, action-group)
- Proper error handling with dead letter queues
- Comprehensive monitoring and metrics

**Migration Triggers to Kafka**:
- Stream length consistently > 1M messages
- Consumer lag > 1 second
- Memory usage > 80% of Redis capacity
- Multi-datacenter replication needs

**Trade-offs**:
- Single-node throughput limit (~100K msgs/sec vs Kafka's 1M+ msgs/sec)
- No built-in multi-datacenter replication
- Manual consumer group management vs Kafka's automatic rebalancing

### 4. File-Based Model Storage
**Decision**: Store models as timestamped files on local filesystem.

**Rationale**:
- Zero additional infrastructure
- Simple versioning by timestamp
- Easy backup and restore
- Direct file access for debugging
- No database dependencies

**Trade-offs**:
- No metadata management
- Manual version tracking
- Limited to single server

### 5. Paper Trading Only
**Decision**: Default to paper trading mode with optional live trading later.

**Rationale**:
- Zero financial risk during validation
- Realistic market conditions
- Same API as live trading
- Builds confidence before going live
- Regulatory compliance simplified

**Trade-offs**:
- No real P&L validation
- Potential differences from live execution
- Limited psychological validation

### 6. Basic Feature Set (20 Indicators)
**Decision**: Use only 20 proven technical indicators as features.

**Rationale**:
- Well-understood feature space
- Fast computation (<100ms)
- Proven predictive value
- Easy to validate
- Reduces overfitting risk

**Trade-offs**:
- Limited signal diversity
- No complex features
- Potentially lower alpha

### 7. Conservative Risk Management
**Decision**: Implement strict position and loss limits.

**Limits**:
- 5% max position size
- 2% daily loss limit
- 5% stop loss per trade
- Emergency stop <1 second

**Rationale**:
- Protects against model errors
- Builds trust with stakeholders
- Allows safe experimentation
- Easy to monitor and enforce
- Regulatory best practice

### 8. REST API + WebSocket Interface
**Decision**: Simple REST API for control, WebSocket for real-time updates.

**Rationale**:
- Industry standard patterns
- Easy client integration
- Clear separation of concerns
- Built-in authentication
- Wide tooling support

**Trade-offs**:
- No GraphQL flexibility
- No gRPC performance
- Manual subscription management

## Technology Stack Decisions

### Core Runtime: Rust
- Memory safety without GC
- Predictable latency
- Excellent async support
- Small binary size
- C-compatible FFI

### Neural Network: ruv-FANN
- SIMD optimizations
- Proven in production
- Low memory footprint
- Fast inference (<10ms)
- Simple API

### Data Storage: TimescaleDB
- PostgreSQL compatible
- Time-series optimized
- SQL familiarity
- Excellent tooling
- Easy backup/restore

### Monitoring: Prometheus + Grafana
- Industry standard
- Zero-code instrumentation
- Rich visualization
- Alert management
- Low overhead

## Performance Targets

### Latency Requirements
- **EventBus latency**: <10ms (Redis Streams)
- **Data ingestion**: <50ms (Alpaca WebSocket to Redis)
- **Feature calculation**: <100ms (20 technical indicators)
- **Neural prediction**: <10ms (ruv-FANN inference)
- **Order execution**: <1 second (Alpaca API call)
- **End-to-end pipeline**: <2 seconds (market data to order placement)

### Throughput Requirements
- **EventBus**: 100,000 messages/second (Redis Streams)
- **Market Data**: 1,000 quotes/second (10 symbols @ 100Hz)
- **Model Predictions**: 100 predictions/minute
- **Order Execution**: 50 orders/day max
- **System Events**: 500 events/second (monitoring, health checks)

### Resource Requirements
- **CPU**: 4 cores (2 for Redis, 2 for services)
- **Memory**: 8GB RAM (4GB for Redis, 4GB for services)
- **Storage**: 100GB SSD (Redis persistence + TimescaleDB)
- **Network**: 100Mbps (adequate for 100K msgs/sec)
- **Redis Specific**: 
  - Memory usage: ~4GB for 1M message backlog
  - Persistence: RDB + AOF for durability
  - Connection pooling: 50-100 connections

## Migration Path to V2

### Phase 1: MVP (Current)
- Prove neural trading works
- Validate performance metrics
- Build operational confidence
- Gather real-world data

### Phase 2: Enhanced MVP (Month 2-3)
- Add second data source
- Implement model ensemble
- Live trading with small capital
- Advanced risk management
- **EventBus**: Continue with Redis Streams (adequate for enhanced MVP)

### Phase 3: Scale Out (Month 4-6)
- Kubernetes deployment
- Multiple asset classes
- Advanced ML Ops
- Full MCP integration
- **EventBus Migration**: Evaluate Kafka migration based on throughput metrics

### Phase 4: Full V2 (Month 7-12)
- Complete feature parity
- DAA agents
- Multi-strategy
- Production scale
- **EventBus**: Full Kafka deployment for 1M+ messages/second

## Risk Mitigation

### Technical Risks
- **Model failure**: Paper trading limits impact
- **EventBus failure**: Redis persistence (RDB+AOF) + automatic failover
- **Message loss**: Consumer group acknowledgments + dead letter queues
- **Redis memory overflow**: Stream trimming + monitoring alerts
- **Consumer lag**: Monitoring + automatic scaling alerts
- **Data loss**: Redis persistence + TimescaleDB backup
- **System crash**: Automatic restart with position recovery
- **Network issues**: Graceful degradation, cached data

### Operational Risks
- **Human error**: Limited permissions, audit logging
- **Configuration drift**: Version control, infrastructure as code
- **Monitoring blind spots**: Comprehensive metrics, alerting

### Financial Risks
- **Large losses**: Strict position and loss limits
- **Regulatory issues**: Paper trading default, full audit trail
- **Model degradation**: Performance monitoring, automatic alerts

## Success Criteria

### Technical Success
- [ ] **EventBus Performance**: <10ms Redis Streams latency
- [ ] **Throughput**: Handle 100K messages/second without consumer lag
- [ ] **End-to-end latency**: <2 seconds (market data to order execution)
- [ ] **Uptime**: 99% during market hours (including Redis)
- [ ] **Data Durability**: Zero message loss with consumer group acknowledgments
- [ ] **Model Performance**: <1% prediction errors with <10ms inference time
- [ ] **Consumer Health**: All consumer groups lag <100 messages

### Business Success
- [ ] Positive Sharpe ratio (>0.3)
- [ ] Win rate >52%
- [ ] Max drawdown <15%
- [ ] Profitable backtests

### Operational Success
- [ ] Fully automated operation
- [ ] Clear monitoring dashboards
- [ ] Complete audit trail
- [ ] Disaster recovery tested

## Conclusion

This Redis Streams-based MVP architecture **validates the core neural trading hypothesis with production-ready messaging infrastructure**. The choice of Redis Streams over Kafka for MVP provides:

### Key Benefits:
1. **Rapid Deployment**: Deploy in 2-4 weeks with single Redis instance
2. **Production Ready**: 100K msgs/sec throughput with <10ms latency
3. **Operational Simplicity**: No complex cluster management required
4. **Cost Effective**: Minimal infrastructure overhead for MVP validation
5. **Future Proof**: Clean interface abstraction enables seamless Kafka migration

### Strategic Advantages:
1. **Prove neural trading viability** with real market data and production messaging
2. **Build incrementally** toward full V2 vision with maintained interfaces
3. **Minimize complexity** while ensuring production reliability
4. **Learn quickly** through rapid iteration and monitoring
5. **Scale confidently** with clear migration path to Kafka

The architecture maintains **clean EventBus interfaces** and **clear upgrade paths**, ensuring MVP investments directly contribute to the final production system capable of handling 1M+ messages/second when needed.

**Redis Streams provides the perfect MVP foundation**: production-ready messaging with operational simplicity, allowing the team to focus on proving the neural trading concept rather than managing complex infrastructure.