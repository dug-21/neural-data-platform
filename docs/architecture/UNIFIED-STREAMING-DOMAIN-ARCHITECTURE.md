# Unified Streaming Architecture with Domain Deployment

## Executive Summary

This document presents the unified architecture that integrates high-throughput streaming capabilities (millions of events/second) with clean domain deployment patterns. The design combines Kafka-based event streaming with standardized gRPC interfaces and clear deployment boundaries to create a production-ready trading system.

## Architecture Overview

### Core Design Principles

1. **Streaming-First**: Kafka as primary event backbone for millions of events/second
2. **Domain Isolation**: Clear boundaries between generic platform and domain-specific services
3. **Interface Standardization**: gRPC contracts for all cross-layer communication
4. **Independent Scaling**: Each layer scales according to its specific requirements
5. **Migration Path**: Gradual transition from Redis Streams to Kafka

## Three-Layer Architecture

### Layer 1: Generic Platform Services (Shared Infrastructure)

**Single Deployment - Domain Agnostic**

```yaml
Components:
  - EventBus Platform (Kafka + Redis Streams)
  - ML Ops Platform (ruv-FANN)
  - Domain Registry (gRPC/REST)
  - TimescaleDB (Shared Storage)
  - Monitoring Platform (Prometheus/Grafana)

Characteristics:
  - Deployed once for entire system
  - Handles millions of events/second
  - Domain-agnostic business logic
  - Horizontal scaling by workload
```

#### EventBus Platform - Hybrid Streaming Architecture

**Current State (MVP)**: Redis Streams (1K msgs/sec)
**Target State**: Kafka Primary + Redis Compatibility (1-10M msgs/sec)

```yaml
Kafka Configuration:
  partitions_per_topic: 50-100
  replication_factor: 3
  min_insync_replicas: 2
  acks: all
  enable_idempotence: true
  max_in_flight_requests: 1

Topic Strategy:
  trading.orders.v1: 100 partitions (by symbol)
  trading.positions.v1: 50 partitions (by account_id)
  risk.events.v1: 30 partitions (by domain)
  ml.predictions.v1: 20 partitions (by model_id)
  monitoring.metrics.v1: 10 partitions (by service)
```

**Partitioning Strategies:**

1. **Symbol-Based Partitioning** (Trading Events)
   ```rust
   fn calculate_partition(symbol: &str, num_partitions: u32) -> u32 {
       use std::collections::hash_map::DefaultHasher;
       use std::hash::{Hash, Hasher};
       
       let mut hasher = DefaultHasher::new();
       symbol.hash(&mut hasher);
       (hasher.finish() % num_partitions as u64) as u32
   }
   ```

2. **Domain-Based Partitioning** (Cross-Domain Events)
   ```rust
   fn domain_partition(domain: &str, entity_id: &str, num_partitions: u32) -> u32 {
       let mut hasher = DefaultHasher::new();
       format!("{}:{}", domain, entity_id).hash(&mut hasher);
       (hasher.finish() % num_partitions as u64) as u32
   }
   ```

3. **Time-Based Partitioning** (Analytics)
   ```rust
   fn time_partition(timestamp: DateTime<Utc>, num_partitions: u32) -> u32 {
       (timestamp.hour() % num_partitions) as u32
   }
   ```

**Migration Strategy:**
```yaml
Phase 1: Kafka cluster deployment alongside Redis
Phase 2: Dual-write to both systems
Phase 3: Read migration to Kafka
Phase 4: Redis Streams deprecation
Timeline: 4-6 months
```

### Layer 2: Standardized Interface Contracts

**Per-Interface Implementation - Domain Specific**

```protobuf
// Standard interfaces that all domain services must implement

service DataIngestionService {
  rpc RegisterSource(SourceConfig) returns (RegistrationResponse);
  rpc StreamData(stream DataPoint) returns (StreamResponse);
  rpc GetSchema(SchemaRequest) returns (SchemaDefinition);
  rpc HealthCheck(Empty) returns (HealthStatus);
}

service ModelExecutionService {
  rpc LoadModel(ModelConfig) returns (LoadResponse);
  rpc Predict(PredictionRequest) returns (PredictionResponse);
  rpc GetMetrics(MetricsRequest) returns (ModelMetrics);
  rpc UnloadModel(UnloadRequest) returns (UnloadResponse);
}

service ActionExecutionService {
  rpc ExecuteAction(ActionRequest) returns (ActionResponse);
  rpc GetCapabilities(CapabilityRequest) returns (CapabilityResponse);
  rpc ValidateAction(ValidationRequest) returns (ValidationResponse);
  rpc GetActionStatus(StatusRequest) returns (ActionStatus);
}
```

**Interface Characteristics:**
- Standard gRPC contracts across all domains
- Domain-specific implementations
- Registry-managed service discovery
- Automated compliance testing

### Layer 3: Domain-Specific Services (Trading)

**Per-Domain Deployment - Interface Compliant**

```yaml
Trading Domain Services:
  - Trading Data Ingestion (implements DataIngestionService)
  - Trading Model Execution (implements ModelExecutionService)  
  - Trading Action Execution (implements ActionExecutionService)
  - Risk Controller (domain-specific logic)
  - Alpaca Connector (external integration)
  - Trading Web UI (domain-specific interface)

Deployment Pattern:
  - Independent deployment per domain
  - Must implement standard interfaces
  - Can have domain-specific components
  - Scales based on domain load
```

## Data Flow Architecture

### High-Frequency Trading Flow

```
External Market Data 
  ↓ (WebSocket/REST)
Trading Data Ingestion (gRPC)
  ↓ (Kafka: trading.market_data.v1, partitioned by symbol)
EventBus Platform 
  ↓ (Kafka: trading.signals.v1)
Trading Model Execution (gRPC)
  ↓ (ML Ops Platform integration)
Prediction Results
  ↓ (Kafka: trading.predictions.v1)
Trading Action Execution (gRPC)
  ↓ (Risk validation + execution)
External Broker (Alpaca)
```

### Event Sourcing Pattern

```rust
#[derive(Serialize, Deserialize)]
pub struct TradingEvent {
    pub event_id: Uuid,
    pub aggregate_id: String,  // symbol, account_id, etc.
    pub event_type: String,
    pub event_version: u32,
    pub timestamp: DateTime<Utc>,
    pub data: serde_json::Value,
    pub metadata: HashMap<String, String>,
}

// Example event types
pub enum TradingEventType {
    OrderPlaced,
    OrderExecuted,
    PositionOpened,
    RiskLimitBreached,
    ModelPredictionGenerated,
    PerformanceCalculated,
}
```

## Performance Specifications

### Throughput Requirements

| Component | Current MVP | Production Target | Technology |
|-----------|-------------|-------------------|------------|
| Data Ingestion | 100 msgs/sec | 500K msgs/sec | Kafka partitioning |
| EventBus | 1K msgs/sec | 1-10M msgs/sec | Kafka cluster |
| Model Execution | 10 predictions/sec | 10K predictions/sec | Horizontal scaling |
| Action Execution | 1 action/sec | 1K actions/sec | Risk validation pipeline |

### Latency Requirements

| Use Case | Target Latency | Implementation |
|----------|---------------|----------------|
| Market Data Ingestion | <5ms | Kafka optimized producers |
| Model Inference | <10ms | ruv-FANN + GPU acceleration |
| Action Execution | <20ms | Risk pre-validation |
| Analytics Queries | <100ms | TimescaleDB optimization |

### Delivery Guarantees

**Exactly-Once (Trading Operations)**:
```rust
// Kafka configuration for exactly-once semantics
pub fn exactly_once_config() -> HashMap<String, String> {
    let mut config = HashMap::new();
    config.insert("enable.idempotence".to_string(), "true".to_string());
    config.insert("transactional.id".to_string(), "trading-processor-1".to_string());
    config.insert("acks".to_string(), "all".to_string());
    config.insert("retries".to_string(), "2147483647".to_string());
    config.insert("max.in.flight.requests.per.connection".to_string(), "1".to_string());
    config
}
```

**At-Least-Once (Analytics)**:
```rust
// Idempotent consumer pattern for analytics
pub struct IdempotentProcessor {
    processed_events: Arc<RwLock<HashSet<Uuid>>>,
}

impl IdempotentProcessor {
    pub async fn process_event(&self, event: AnalyticsEvent) -> Result<(), ProcessingError> {
        let event_id = event.event_id;
        
        if self.processed_events.read().await.contains(&event_id) {
            return Ok(()); // Skip duplicate
        }
        
        self.handle_event(event).await?;
        self.processed_events.write().await.insert(event_id);
        Ok(())
    }
}
```

## Domain Registry Architecture

### Service Registration

```yaml
Registration Process:
  1. Domain service starts up
  2. Registers with Domain Registry
  3. Receives domain configuration
  4. Connects to generic platform services
  5. Begins serving domain-specific requests

Schema Management:
  1. Domain service registers schemas
  2. Registry validates schema compatibility
  3. EventBus updates schema registry
  4. Other services retrieve schemas for validation
```

### Service Discovery

```rust
// Domain Registry interface
pub struct DomainRegistry {
    services: HashMap<String, ServiceEndpoint>,
    schemas: HashMap<String, SchemaDefinition>,
    health_checker: HealthChecker,
}

impl DomainRegistry {
    pub async fn register_service(&mut self, domain: &str, service: ServiceConfig) -> Result<(), RegistryError> {
        // Validate service implements required interfaces
        self.validate_interfaces(&service).await?;
        
        // Register service endpoint
        self.services.insert(domain.to_string(), service.endpoint);
        
        // Register schemas
        for schema in service.schemas {
            self.register_schema(domain, schema).await?;
        }
        
        Ok(())
    }
    
    pub async fn discover_services(&self, client_domain: &str) -> Vec<ServiceEndpoint> {
        // Return services this domain can access
        self.services.values()
            .filter(|service| self.can_access(client_domain, &service.domain))
            .cloned()
            .collect()
    }
}
```

## Scaling Patterns

### Horizontal Scaling Strategy

**EventBus Platform**: Scale by adding Kafka brokers
```yaml
Current: 3 brokers (development)
Production: 9+ brokers (3 per availability zone)
Scaling trigger: >70% CPU or >80% disk usage
```

**Data Ingestion**: Scale by domain and data source
```yaml
Scaling pattern: One instance per high-volume data source
Auto-scaling trigger: >1000 msgs/sec per instance
Max instances: 20 per domain
```

**Model Execution**: Scale by prediction load
```yaml
Scaling pattern: Load-based horizontal pod autoscaling
CPU target: 70%
Memory target: 80%
Prediction queue depth trigger: >100 pending
```

**Action Execution**: Scale by action throughput
```yaml
Scaling pattern: Independent per domain
Risk validation pre-scaling: Validate 10x expected load
Circuit breaker: Halt scaling on risk limit breaches
```

### Resource Allocation

```yaml
Production Resource Requirements:

EventBus Platform (Kafka):
  CPU: 16 cores per broker
  Memory: 64GB per broker
  Disk: 10TB NVMe per broker
  Network: 10Gbps

ML Ops Platform:
  CPU: 32 cores
  Memory: 128GB
  GPU: 4x V100 (optional)
  Disk: 5TB SSD

Domain Services (each):
  CPU: 4-8 cores
  Memory: 16-32GB  
  Disk: 1TB SSD
```

## Monitoring and Observability

### Streaming Metrics

```yaml
EventBus Monitoring:
  - Messages per second (per topic/partition)
  - Consumer lag (per consumer group)
  - Broker disk usage and CPU
  - Network throughput
  - Replication lag

Interface Metrics:
  - gRPC request latency (p50, p95, p99)
  - Error rates per interface method
  - Request volume per domain service
  - Circuit breaker states

Domain Metrics:
  - Trading performance (PnL, accuracy)
  - Risk limit utilization
  - Model prediction accuracy
  - Action execution success rates
```

### Alerting Strategy

```yaml
Critical Alerts:
  - Kafka cluster health degradation
  - Domain service interface failures
  - Risk limit breaches
  - Model prediction accuracy drops

Warning Alerts:
  - High consumer lag (>1 minute)
  - Interface latency increases (>p95)
  - Resource utilization (>80%)
  - Schema validation failures
```

## Implementation Roadmap

### Phase 1: Foundation (Month 1-2)
- Deploy Kafka cluster (3 brokers)
- Implement Domain Registry
- Create standard gRPC interface definitions
- Migrate Trading Data Ingestion to new architecture

### Phase 2: Streaming Migration (Month 2-3)
- Implement dual-write (Redis + Kafka)
- Deploy Kafka-based EventBus Platform
- Migrate domain services to standard interfaces
- Performance testing and optimization

### Phase 3: Full Migration (Month 3-4)
- Complete migration from Redis to Kafka
- Deploy production monitoring stack
- Implement exactly-once delivery for trading
- Load testing at target volumes

### Phase 4: Production Optimization (Month 4-6)
- Performance tuning for millions of events/second
- Advanced partitioning strategies
- Cross-region replication
- Compliance and audit features

## Conclusion

This unified architecture provides:

1. **Scale**: Kafka handles millions of events/second with proper partitioning
2. **Domain Isolation**: Clear boundaries with standardized interfaces
3. **Migration Path**: Gradual transition from current MVP to production scale
4. **Operational Simplicity**: Layer-appropriate technology choices
5. **Production Readiness**: Exactly-once delivery, monitoring, and alerting

The architecture supports the neural trader's evolution from MVP (1K msgs/sec) to production scale (millions of events/sec) while maintaining clean domain boundaries and standardized interfaces throughout the platform.