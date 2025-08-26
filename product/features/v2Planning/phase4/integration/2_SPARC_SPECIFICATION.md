# EventBus Integration SPARC Specification

## Executive Summary

This document provides a comprehensive SPARC (Specification, Pseudocode, Architecture, Refinement, Completion) methodology implementation for integrating EventBus into the Neural Trader V2 Phase 4 architecture. The specification defines the complete migration path from Redis-based messaging to a hybrid EventBus system while maintaining backwards compatibility.

---

## Phase 1: SPECIFICATION

### 1.1 Functional Requirements

#### Core Integration Requirements
- **REQ-001**: EventBus must provide real-time market data distribution with <100ms latency
- **REQ-002**: System must support 10,000+ events per second throughput
- **REQ-003**: All event types must be strongly typed with schema validation
- **REQ-004**: EventBus must integrate seamlessly with existing Redis infrastructure
- **REQ-005**: Zero-downtime migration path must be available

#### Event Schema Requirements
- **REQ-006**: Market data events must include timestamp, symbol, price, volume
- **REQ-007**: Trading signal events must include signal type, confidence, target price
- **REQ-008**: System health events must include component status, metrics, alerts
- **REQ-009**: All events must support correlation IDs for tracing

#### Service Integration Requirements
- **REQ-010**: data-ingestion service must publish to both Redis and EventBus during migration
- **REQ-011**: neural-ml-ops must consume from Redis and publish to EventBus as central hub
- **REQ-012**: neural-trading must support dual-channel consumption (Redis + EventBus)
- **REQ-013**: EventBus must support topic-based routing and filtering

### 1.2 Non-Functional Requirements

#### Performance Requirements
- **NFR-001**: Event processing latency < 50ms (p95)
- **NFR-002**: System availability > 99.9%
- **NFR-003**: Memory usage < 2GB per service instance
- **NFR-004**: CPU utilization < 70% under normal load

#### Scalability Requirements
- **NFR-005**: Horizontal scaling up to 10 instances per service
- **NFR-006**: Support for 100+ concurrent event consumers
- **NFR-007**: Event retention for 24 hours minimum

#### Security Requirements
- **NFR-008**: All events must be authenticated and authorized
- **NFR-009**: Sensitive data must be encrypted in transit
- **NFR-010**: Access controls must be role-based

### 1.3 Interface Contracts

#### EventBus Core Interface
```typescript
interface EventBus {
  publish<T>(topic: string, event: Event<T>): Promise<void>
  subscribe<T>(topic: string, handler: EventHandler<T>): Promise<Subscription>
  unsubscribe(subscription: Subscription): Promise<void>
  getHealth(): Promise<HealthStatus>
}
```

#### Event Base Schema
```typescript
interface Event<T> {
  id: string
  type: string
  timestamp: number
  correlationId?: string
  source: string
  data: T
}
```

#### Market Data Event Schema
```typescript
interface MarketDataEvent {
  symbol: string
  price: number
  volume: number
  bid: number
  ask: number
  lastUpdate: number
}
```

#### Trading Signal Event Schema
```typescript
interface TradingSignalEvent {
  symbol: string
  signalType: 'BUY' | 'SELL' | 'HOLD'
  confidence: number
  targetPrice: number
  stopLoss: number
  reasoning: string
  modelVersion: string
}
```

### 1.4 Success Criteria
- All services successfully integrated with EventBus
- Zero data loss during migration
- Performance benchmarks met
- Complete backwards compatibility maintained
- Comprehensive monitoring and alerting in place

---

## Phase 2: PSEUDOCODE

### 2.1 Data Ingestion Service Integration

```pseudocode
class DataIngestionService:
    initialize():
        redis_client = create_redis_connection()
        eventbus_client = create_eventbus_connection()
        migration_mode = get_migration_mode()  // REDIS_ONLY, DUAL, EVENTBUS_ONLY
        
    process_market_data(raw_data):
        processed_data = transform_market_data(raw_data)
        market_event = create_market_data_event(processed_data)
        
        switch migration_mode:
            case REDIS_ONLY:
                publish_to_redis(market_event)
            case DUAL:
                parallel_publish(redis_client, eventbus_client, market_event)
            case EVENTBUS_ONLY:
                publish_to_eventbus(market_event)
                
        log_publishing_metrics(market_event)
        
    parallel_publish(redis_client, eventbus_client, event):
        redis_future = async_publish_redis(redis_client, event)
        eventbus_future = async_publish_eventbus(eventbus_client, event)
        
        wait_for_both(redis_future, eventbus_future)
        handle_publishing_errors()
```

### 2.2 Neural ML-Ops Hub Integration

```pseudocode
class NeuralMLOpsService:
    initialize():
        redis_subscriber = create_redis_subscriber()
        eventbus_client = create_eventbus_connection()
        signal_processor = initialize_signal_processor()
        
    run_processing_loop():
        while service_active:
            market_data = consume_from_redis()
            if market_data:
                trading_signals = process_market_data(market_data)
                for signal in trading_signals:
                    publish_trading_signal(signal)
                    
            health_status = check_system_health()
            publish_health_event(health_status)
            
            sleep(processing_interval)
            
    process_market_data(market_data):
        features = extract_features(market_data)
        predictions = ml_model.predict(features)
        signals = convert_predictions_to_signals(predictions)
        return signals
        
    publish_trading_signal(signal):
        trading_event = create_trading_signal_event(signal)
        eventbus_client.publish('trading.signals', trading_event)
        update_signal_metrics(trading_event)
```

### 2.3 Neural Trading Dual-Channel Consumer

```pseudocode
class NeuralTradingService:
    initialize():
        redis_subscriber = create_redis_subscriber()
        eventbus_subscriber = create_eventbus_subscriber()
        consumption_strategy = get_consumption_strategy()  // REDIS_PRIMARY, EVENTBUS_PRIMARY, HYBRID
        
    start_consumption():
        switch consumption_strategy:
            case REDIS_PRIMARY:
                start_redis_consumer()
                start_eventbus_backup_consumer()
            case EVENTBUS_PRIMARY:
                start_eventbus_consumer()
                start_redis_backup_consumer()
            case HYBRID:
                start_parallel_consumers()
                
    start_parallel_consumers():
        redis_thread = spawn_thread(consume_redis_signals)
        eventbus_thread = spawn_thread(consume_eventbus_signals)
        
        while service_active:
            signals = merge_signal_streams()
            deduplicated_signals = remove_duplicates(signals)
            execute_trading_decisions(deduplicated_signals)
            
    merge_signal_streams():
        redis_signals = get_redis_signals()
        eventbus_signals = get_eventbus_signals()
        
        merged = []
        for signal in redis_signals + eventbus_signals:
            if not is_duplicate(signal, merged):
                merged.append(signal)
                
        return sort_by_priority(merged)
```

### 2.4 Migration Orchestration

```pseudocode
class MigrationOrchestrator:
    initialize():
        services = [data_ingestion, neural_ml_ops, neural_trading]
        migration_phases = [PREPARATION, DUAL_MODE, VALIDATION, CUTOVER, CLEANUP]
        
    execute_migration():
        for phase in migration_phases:
            execute_phase(phase)
            validate_phase_completion(phase)
            wait_for_stabilization()
            
    execute_phase(phase):
        switch phase:
            case PREPARATION:
                setup_eventbus_infrastructure()
                deploy_dual_mode_configurations()
                run_connectivity_tests()
                
            case DUAL_MODE:
                enable_dual_publishing()
                monitor_data_consistency()
                validate_performance_metrics()
                
            case VALIDATION:
                run_end_to_end_tests()
                compare_redis_vs_eventbus_data()
                validate_zero_data_loss()
                
            case CUTOVER:
                switch_consumers_to_eventbus()
                disable_redis_publishing()
                monitor_system_stability()
                
            case CLEANUP:
                remove_redis_dependencies()
                cleanup_dual_mode_code()
                update_monitoring_dashboards()
```

---

## Phase 3: ARCHITECTURE

### 3.1 System Design Patterns

#### Event-Driven Architecture Pattern
- **Publisher-Subscriber**: Loose coupling between event producers and consumers
- **Event Sourcing**: Complete event history for audit and replay capabilities
- **CQRS**: Separate read/write models for optimal performance
- **Saga Pattern**: Distributed transaction management across services

#### Integration Patterns
- **Adapter Pattern**: Seamless integration with existing Redis infrastructure
- **Bridge Pattern**: Abstract EventBus implementation details
- **Strategy Pattern**: Configurable migration strategies
- **Observer Pattern**: Real-time monitoring and alerting

### 3.2 Component Architecture

#### EventBus Core Components
```
EventBus Core
├── Publisher
│   ├── Topic Router
│   ├── Schema Validator
│   └── Persistence Layer
├── Subscriber
│   ├── Consumer Groups
│   ├── Message Filtering
│   └── Dead Letter Queue
├── Management
│   ├── Topic Manager
│   ├── Schema Registry
│   └── Metrics Collector
└── Infrastructure
    ├── Storage Backend
    ├── Network Layer
    └── Security Module
```

#### Service Integration Architecture
```
Single Data Flow Architecture
┌─────────────────┐    ┌──────────────────┐
│  Data Ingestion │───▶│   Redis (Fast)   │───┐
│     Service     │    └──────────────────┘   │
└─────────────────┘                           │
         │                                    ▼
         │                            ┌─────────────────┐    ┌─────────────────┐
         │                            │ Neural ML-Ops   │───▶│    EventBus     │
         │                            │    Service      │    │  (ML Features)  │
         │                            └─────────────────┘    └─────────────────┘
         │                                    ▲                       │
         ▼                                    │                       ▼
┌─────────────────┐                          │               ┌─────────────────┐
│  TimescaleDB    │──────────────────────────┘               │ Neural Trading  │
│  (Historical)   │                                          │    Service      │
└─────────────────┘                                          └─────────────────┘

Data Types:
- Fast Data: Redis (real-time market data)
- Slow Data: TimescaleDB (historical features)
- ML Features: EventBus (processed features only)
```

### 3.3 Data Flow Design

#### Event Flow Topology
```
Market Data Sources
    │
    ▼
┌─────────────────────┐
│  Data Ingestion     │ ──┐
│  - WebSocket feeds  │   │
│  - REST API polls   │   │
│  - File imports     │   │
└─────────────────────┘   │
                          │
    ┌─────────────────────┘
    │
    ▼
┌─────────────────────┐    ┌─────────────────────┐
│      Redis          │    │     EventBus        │
│   (Transition)      │    │    (Target State)   │
└─────────────────────┘    └─────────────────────┘
    │                           │
    ▼                           ▼
┌─────────────────────┐    ┌─────────────────────┐
│  Neural ML-Ops      │    │  Neural Trading     │
│  - Feature engine   │    │  - Signal consumer  │
│  - Model inference  │    │  - Risk management  │
│  - Signal generation│    │  - Order execution  │
└─────────────────────┘    └─────────────────────┘
    │                           ▲
    └───────────────────────────┘
```

### 3.4 Deployment Architecture

#### Container Orchestration
```yaml
EventBus Infrastructure:
  - EventBus Core: 3 replicas (HA)
  - Schema Registry: 2 replicas
  - Management API: 2 replicas
  - Monitoring: 1 replica

Service Instances:
  - Data Ingestion: 2-5 replicas (auto-scale)
  - Neural ML-Ops: 1-3 replicas (CPU intensive)
  - Neural Trading: 1-2 replicas (stateful)
```

#### Network Topology
```
Load Balancer
    │
    ├── EventBus Cluster (Port 9092)
    ├── Redis Cluster (Port 6379)
    ├── Management API (Port 8080)
    └── Monitoring (Port 3000)

Internal Networks:
    - EventBus: eventbus-network (10.1.0.0/16)
    - Redis: redis-network (10.2.0.0/16)
    - Services: services-network (10.3.0.0/16)
```

---

## Phase 4: REFINEMENT

### 4.1 Iterative Development Strategy

#### Sprint Planning (2-week sprints)
1. **Sprint 1-2**: EventBus core infrastructure and basic publishing
2. **Sprint 3-4**: Service integration and dual-mode implementation
3. **Sprint 5-6**: Migration tooling and validation frameworks
4. **Sprint 7-8**: Performance optimization and monitoring
5. **Sprint 9-10**: Production deployment and stabilization

#### Continuous Integration Pipeline
```yaml
CI/CD Pipeline:
  - Unit Tests (>90% coverage)
  - Integration Tests (E2E scenarios)
  - Performance Tests (load/stress)
  - Security Scans (SAST/DAST)
  - Deployment Validation
```

### 4.2 Test-Driven Development Approach

#### Unit Testing Strategy
```python
# Event publishing tests
def test_eventbus_publish_success():
    eventbus = MockEventBus()
    event = create_market_data_event()
    
    result = eventbus.publish('market.data', event)
    
    assert result.success == True
    assert result.latency < 50  # ms

# Migration strategy tests
def test_dual_mode_consistency():
    redis_events = capture_redis_events()
    eventbus_events = capture_eventbus_events()
    
    assert len(redis_events) == len(eventbus_events)
    assert events_are_equivalent(redis_events, eventbus_events)
```

#### Integration Testing Framework
```python
# End-to-end testing
class EventBusIntegrationTest:
    def setup_test_environment(self):
        self.eventbus = start_test_eventbus()
        self.services = start_test_services()
        
    def test_market_data_flow(self):
        # Inject test market data
        test_data = create_test_market_data()
        self.services.data_ingestion.process(test_data)
        
        # Verify event propagation
        trading_signals = self.services.neural_trading.get_signals()
        assert len(trading_signals) > 0
        assert all(signal.is_valid() for signal in trading_signals)
```

### 4.3 Performance Optimization

#### Latency Optimization
- **Batch Processing**: Group events for efficient publishing
- **Connection Pooling**: Reuse EventBus connections
- **Async Processing**: Non-blocking event handling
- **Memory Mapping**: Fast event serialization/deserialization

#### Throughput Optimization
```python
class OptimizedEventPublisher:
    def __init__(self):
        self.connection_pool = create_connection_pool(size=10)
        self.batch_size = 100
        self.batch_timeout = 10  # ms
        
    async def publish_batch(self, events):
        serialized = await asyncio.gather(*[
            serialize_event(event) for event in events
        ])
        
        connection = await self.connection_pool.acquire()
        try:
            await connection.publish_batch(serialized)
        finally:
            await self.connection_pool.release(connection)
```

### 4.4 Error Handling and Resilience

#### Circuit Breaker Pattern
```python
class EventBusCircuitBreaker:
    def __init__(self, failure_threshold=5, timeout=60):
        self.failure_count = 0
        self.failure_threshold = failure_threshold
        self.timeout = timeout
        self.state = 'CLOSED'  # CLOSED, OPEN, HALF_OPEN
        
    async def call(self, func, *args, **kwargs):
        if self.state == 'OPEN':
            if time.time() - self.last_failure < self.timeout:
                raise CircuitBreakerOpenError()
            self.state = 'HALF_OPEN'
            
        try:
            result = await func(*args, **kwargs)
            self.reset()
            return result
        except Exception as e:
            self.record_failure()
            raise e
```

#### Retry and Backoff Strategy
```python
@retry(
    stop=stop_after_attempt(3),
    wait=wait_exponential(multiplier=1, min=4, max=10)
)
async def publish_with_retry(eventbus, topic, event):
    return await eventbus.publish(topic, event)
```

---

## Phase 5: COMPLETION

### 5.1 Success Validation Criteria

#### Functional Validation
- **✓ Event Publishing**: All events successfully published to EventBus
- **✓ Event Consumption**: All services consuming events correctly
- **✓ Data Consistency**: No data loss during migration
- **✓ Schema Validation**: All events conform to defined schemas
- **✓ Error Handling**: Proper error recovery and alerting

#### Performance Validation
- **✓ Latency**: P95 latency < 50ms
- **✓ Throughput**: 10,000+ events/second sustained
- **✓ Availability**: 99.9% uptime achieved
- **✓ Resource Usage**: Within defined limits
- **✓ Scalability**: Horizontal scaling validated

#### Migration Validation
- **✓ Zero Downtime**: No service interruptions
- **✓ Backwards Compatibility**: Legacy systems continue working
- **✓ Data Integrity**: All historical data preserved
- **✓ Rollback Capability**: Ability to revert if needed

### 5.2 Acceptance Testing

#### User Acceptance Test Scenarios
```gherkin
Feature: Real-time Trading Signal Delivery
  Scenario: Market data triggers trading signal
    Given the EventBus is running
    And neural-ml-ops service is subscribed to market data
    And neural-trading service is subscribed to trading signals
    When market data event is published
    Then trading signal should be generated within 100ms
    And neural-trading should receive the signal
    And trading decision should be executed

Feature: System Resilience
  Scenario: EventBus temporary unavailability
    Given services are running in dual mode
    When EventBus becomes unavailable
    Then services should continue using Redis
    And no data should be lost
    When EventBus becomes available
    Then services should resume EventBus usage
```

#### Load Testing Scenarios
```python
class LoadTest:
    async def test_high_throughput(self):
        """Test system under 15,000 events/second"""
        event_rate = 15000  # events per second
        duration = 300      # 5 minutes
        
        async with EventGenerator(rate=event_rate) as generator:
            await generator.run_for(duration)
            
        metrics = await self.collect_performance_metrics()
        assert metrics.avg_latency < 50
        assert metrics.error_rate < 0.1
        assert metrics.throughput >= event_rate * 0.95
```

### 5.3 Production Readiness Checklist

#### Infrastructure Readiness
- [ ] EventBus cluster deployed and configured
- [ ] Monitoring and alerting configured
- [ ] Log aggregation and analysis setup
- [ ] Backup and disaster recovery tested
- [ ] Security policies implemented
- [ ] Network policies and firewall rules configured

#### Application Readiness
- [ ] All services updated with EventBus integration
- [ ] Configuration management implemented
- [ ] Health checks and readiness probes configured
- [ ] Graceful shutdown handling implemented
- [ ] Resource limits and requests defined
- [ ] Auto-scaling policies configured

#### Operational Readiness
- [ ] Runbooks and troubleshooting guides created
- [ ] On-call procedures defined
- [ ] Incident response playbooks prepared
- [ ] Performance baselines established
- [ ] Capacity planning completed
- [ ] Team training completed

### 5.4 Migration Execution Plan

#### Phase 1: Preparation (Week 1)
```bash
# Infrastructure setup
kubectl apply -f eventbus-infrastructure/
kubectl apply -f monitoring-stack/
kubectl apply -f service-configs/

# Validation
./scripts/validate-infrastructure.sh
./scripts/run-connectivity-tests.sh
```

#### Phase 2: Dual Mode (Week 2-3)
```bash
# Enable dual publishing
kubectl patch deployment data-ingestion -p '{"spec":{"template":{"spec":{"containers":[{"name":"app","env":[{"name":"MIGRATION_MODE","value":"DUAL"}]}]}}}}'

# Monitor and validate
./scripts/monitor-dual-mode.sh
./scripts/validate-data-consistency.sh
```

#### Phase 3: Consumer Migration (Week 4)
```bash
# Switch consumers to EventBus
kubectl patch deployment neural-trading -p '{"spec":{"template":{"spec":{"containers":[{"name":"app","env":[{"name":"CONSUMPTION_MODE","value":"EVENTBUS_PRIMARY"}]}]}}}}'

# Validate and monitor
./scripts/validate-consumer-migration.sh
```

#### Phase 4: Cleanup (Week 5)
```bash
# Remove Redis dependencies
kubectl patch deployment data-ingestion -p '{"spec":{"template":{"spec":{"containers":[{"name":"app","env":[{"name":"MIGRATION_MODE","value":"EVENTBUS_ONLY"}]}]}}}}'

# Cleanup resources
./scripts/cleanup-legacy-resources.sh
```

### 5.5 Success Metrics and KPIs

#### Technical KPIs
- **ML Feature Processing Latency**: P50 < 20ms, P95 < 50ms, P99 < 100ms
- **ML Feature Throughput**: 5,000+ ML features/second sustained
- **System Availability**: 99.9% uptime
- **TimescaleDB Query Performance**: Historical queries < 500ms
- **Single Data Flow Efficiency**: 40% reduction in data duplication

#### Business KPIs
- **Trading Signal Accuracy**: Maintain or improve current accuracy
- **Time to Market**: Reduce feature deployment time by 40%
- **Operational Costs**: 25% reduction in infrastructure costs
- **System Scalability**: Support 3x current trading volume

#### Implementation KPIs
- **Data Flow Integrity**: 100% data flow from ingestion to execution
- **ML Feature Accuracy**: Maintain current ML model performance
- **System Simplification**: 50% reduction in data path complexity
- **Team Productivity**: Improved development velocity with single data flow

---

## Appendices

### Appendix A: Event Schema Registry

#### Complete Event Type Definitions
```typescript
// Raw Market Data (Redis only)
interface RawMarketDataEvent extends BaseEvent {
  data: {
    symbol: string;
    timestamp: number;
    price: number;
    volume: number;
    bid: number;
    ask: number;
    high: number;
    low: number;
    open: number;
    close: number;
  }
}

// ML Feature Events (EventBus only)
interface MLFeatureEvent extends BaseEvent {
  data: {
    symbol: string;
    timestamp: number;
    technical_indicators: {
      rsi: number;
      macd: number;
      bollinger_bands: { upper: number; middle: number; lower: number };
      moving_averages: { sma_20: number; ema_50: number };
    };
    price_movements: {
      price_change_1h: number;
      price_change_24h: number;
      volatility: number;
    };
    volume_metrics: {
      volume_sma: number;
      volume_ratio: number;
    };
    predictions: {
      price_direction: number;
      confidence: number;
      volatility_forecast: number;
    };
    model_version: string;
  }
}

// Trading Signal Events
interface TradingSignalEvent extends BaseEvent {
  data: {
    symbol: string;
    signalType: 'BUY' | 'SELL' | 'HOLD';
    confidence: number;
    targetPrice: number;
    stopLoss: number;
    takeProfit: number;
    reasoning: string;
    modelVersion: string;
    risk_score: number;
  }
}

// System Health Events
interface SystemHealthEvent extends BaseEvent {
  data: {
    service: string;
    status: 'healthy' | 'degraded' | 'unhealthy';
    metrics: {
      cpu_usage: number;
      memory_usage: number;
      latency: number;
      error_rate: number;
    };
    timestamp: number;
  }
}
```

### Appendix B: Configuration Templates

#### TimescaleDB Configuration
```yaml
timescaledb:
  cluster:
    nodes: 2
    replication: enabled
    backup_retention: "30d"
  
  hypertables:
    - name: "market_data_history"
      time_column: "timestamp"
      chunk_time_interval: "1h"
      retention_policy: "7d"
    - name: "ml_features_history"
      time_column: "timestamp" 
      chunk_time_interval: "6h"
      retention_policy: "30d"
      
  performance:
    shared_preload_libraries: "timescaledb"
    max_connections: 200
    work_mem: "256MB"
    maintenance_work_mem: "512MB"
```

#### EventBus Configuration
```yaml
eventbus:
  cluster:
    nodes: 3
    replication_factor: 2
    min_insync_replicas: 1
  
  topics:
    - name: "ml.features"
      partitions: 12
      retention: "24h"
      description: "Processed ML features from ML-Ops service"
    - name: "trading.signals"
      partitions: 6
      retention: "168h"  # 7 days
      description: "Trading signals generated by trading services"
    - name: "trading.executed"
      partitions: 4
      retention: "168h"  # 7 days
      description: "Executed trades for monitoring and coordination"
    - name: "system.health"
      partitions: 3
      retention: "72h"
      description: "System health and monitoring events"
      
  performance:
    batch_size: 100
    linger_ms: 5
    compression_type: "snappy"
    max_request_size: "1MB"
```

#### Service Configuration Templates
```yaml
# Data Ingestion Service
data-ingestion:
  data_flow_mode: "REDIS_TIMESCALE"  # Single flow to Redis + TimescaleDB
  redis:
    host: "redis-cluster"
    port: 6379
    channels: ["market_data"]
  timescaledb:
    host: "timescaledb-cluster"
    port: 5432
    database: "market_data_history"
  
# Neural ML-Ops Service  
neural-ml-ops:
  input:
    redis_channels: ["market_data"]
    timescaledb_config:
      historical_window: "24h"
      feature_queries: ["technical_indicators", "price_movements"]
  output:
    eventbus_topics: ["ml.features", "system.health"]
  processing:
    batch_size: 50
    processing_interval: 1000  # ms
    
# Neural Trading Service
neural-trading:
  input:
    eventbus_topics: ["ml.features", "trading.signals"]  # EventBus only
  processing:
    risk_management: true
    order_execution: true
```

### Appendix C: Monitoring and Alerting

#### Key Metrics to Monitor
```yaml
metrics:
  timescaledb:
    - query_response_time
    - historical_data_ingestion_rate
    - storage_usage
    - connection_pool_utilization
    - chunk_creation_rate
    
  eventbus:
    - ml_feature_publishing_rate
    - ml_feature_consumption_rate
    - publishing_latency_p95
    - consumption_latency_p95
    - error_rate
    - topic_lag
  
  services:
    - cpu_usage
    - memory_usage
    - ml_feature_processing_time
    - redis_connection_status
    - timescaledb_connection_status
    - health_check_status
    - single_data_flow_integrity

alerts:
  critical:
    - EventBus cluster down
    - TimescaleDB cluster down
    - ML feature processing stopped
    - Single data flow interrupted
    - ML feature publishing latency > 100ms
    - Error rate > 1%
    - Service health check failed
  warning:
    - TimescaleDB query latency > 500ms
    - ML feature publishing latency > 50ms
    - EventBus topic lag > 1000 messages
    - High memory usage > 80%
    - CPU usage > 70%
    - Redis connection degraded
```

### Appendix D: Troubleshooting Guide

#### Common Issues and Resolutions
```markdown
## Single Data Flow Issues
**Symptom**: ML features not reaching trading service
**Diagnosis**: Check ML-Ops processing and EventBus publishing
**Resolution**: 
1. Verify ML-Ops Redis consumption: `kubectl logs -l app=neural-ml-ops`
2. Check TimescaleDB connectivity: `kubectl exec -it ml-ops -- psql -h timescaledb`
3. Validate EventBus ML feature publishing: `kubectl logs -l app=eventbus`

## TimescaleDB Query Performance
**Symptom**: Historical feature queries taking >1s
**Diagnosis**: Check TimescaleDB query performance and indexing
**Resolution**:
1. Monitor TimescaleDB metrics and slow queries
2. Review time-series indexing strategy
3. Optimize historical window queries
4. Consider data retention policies

## EventBus ML Feature Processing
**Symptom**: Trading service not receiving ML features
**Diagnosis**: Check EventBus topic configuration and consumer groups
**Resolution**:
1. Verify EventBus topic creation: `kubectl exec -it eventbus -- kafka-topics --list`
2. Check consumer group status
3. Validate ML feature event schema
4. Review EventBus partition assignments
```

---

## Document Information

**Version**: 1.0  
**Last Updated**: 2025-08-26  
**Document Owner**: Neural Trader Development Team  
**Review Cycle**: Monthly  
**Next Review**: 2025-09-26  

**Approval**:
- [ ] Technical Lead
- [ ] Architecture Review Board  
- [ ] Product Owner
- [ ] DevOps Lead
- [ ] Security Team

---

This SPARC specification provides the complete blueprint for the Neural Trader V2 Phase 4 SINGLE DATA FLOW architecture. The specification ensures systematic development with clear data flow: data-ingestion → Redis → ML-Ops → EventBus → Execution, utilizing TimescaleDB for historical data and eliminating all dual-path mechanisms. This approach provides better separation of concerns, improved performance, and simplified system maintenance.