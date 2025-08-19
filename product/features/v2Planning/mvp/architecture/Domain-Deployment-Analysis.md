# Domain Deployment Pattern Analysis

## Current Architecture Misalignments

### 1. Component Classification Issues

**CURRENT PROBLEMS:**
- Event Bus shows direct domain connections (should be fully generic)
- ML Ops Platform mixed with domain-specific model execution
- Data Ingestion missing standardized interface contracts
- Action Platform directly connected to domains without interface layer
- Missing clear deployment boundaries in C4 diagrams

### 2. Required Deployment Boundaries

#### GENERIC COMPONENTS (Shared Infrastructure)
```
┌─────────────────────────────────────────────┐
│           SHARED PLATFORM LAYER            │
├─────────────────────────────────────────────┤
│ • EventBus Platform (Redis Streams)        │
│ • ML Ops Platform (ruv-FANN)              │
│ • Domain Registry (gRPC/REST)             │
│ • Monitoring Collector (Prometheus)        │
│ • TimescaleDB (Shared Storage)            │
└─────────────────────────────────────────────┘
```

#### STANDARDIZED INTERFACES (Per-Domain Deployment)
```
┌─────────────────────────────────────────────┐
│        STANDARDIZED INTERFACE LAYER        │
├─────────────────────────────────────────────┤
│ • Data Ingestion Interface                 │
│   - Standard gRPC contracts                │
│   - Domain-specific implementations        │
│                                            │
│ • Model Execution Interface               │
│   - Standard ML Ops API                   │
│   - Domain-specific model runners         │
│                                            │
│ • Action Interface                         │
│   - Standard Model Execution API          │
│   - Domain-specific action executors      │
└─────────────────────────────────────────────┘
```

#### DOMAIN-SPECIFIC COMPONENTS (Per-Domain)
```
┌─────────────────────────────────────────────┐
│         TRADING DOMAIN DEPLOYMENT          │
├─────────────────────────────────────────────┤
│ • Trading Data Ingestion                   │
│ • Trading Model Execution                  │
│ • Trading Action Layer                     │
│ • Alpaca Connector                         │
│ • Trading Risk Controller                  │
└─────────────────────────────────────────────┘
```

### 3. Interface Contracts Required

#### Data Ingestion Standard Interface
```protobuf
service DataIngestionService {
  rpc RegisterSource(SourceConfig) returns (RegistrationResponse);
  rpc StreamData(stream DataPoint) returns (StreamResponse);
  rpc GetSchema(SchemaRequest) returns (SchemaDefinition);
}

message DataPoint {
  string domain = 1;
  string source = 2;
  string symbol = 3;
  int64 timestamp = 4;
  map<string, double> values = 5;
  string schema_version = 6;
}
```

#### Model Execution Standard Interface
```protobuf
service ModelExecutionService {
  rpc LoadModel(ModelConfig) returns (LoadResponse);
  rpc Predict(PredictionRequest) returns (PredictionResponse);
  rpc GetMetrics(MetricsRequest) returns (ModelMetrics);
}

message PredictionRequest {
  string domain = 1;
  string model_id = 2;
  repeated double features = 3;
  map<string, string> metadata = 4;
}
```

#### Action Execution Standard Interface
```protobuf
service ActionExecutionService {
  rpc ExecuteAction(ActionRequest) returns (ActionResponse);
  rpc GetCapabilities(CapabilityRequest) returns (CapabilityResponse);
  rpc ValidateAction(ValidationRequest) returns (ValidationResponse);
}

message ActionRequest {
  string domain = 1;
  string action_type = 2;
  map<string, double> parameters = 3;
  double confidence = 4;
}
```

### 4. Deployment Strategy

#### Shared Infrastructure (Single Deployment)
- **EventBus Platform**: Single Redis cluster, domain-agnostic topics
- **ML Ops Platform**: Single ruv-FANN service, domain-agnostic training
- **Domain Registry**: Single service managing all domain configs
- **Monitoring**: Single Prometheus/Grafana stack

#### Per-Domain Services (Multiple Deployments)
- **Data Ingestion**: Trading-specific ingestion service implementing standard interface
- **Model Execution**: Trading-specific model runner implementing standard interface  
- **Action Layer**: Trading-specific action executor implementing standard interface

### 5. Trading Domain Interaction Patterns

```
External Data -> Trading Data Ingestion -> EventBus (Generic)
                                            ↓
EventBus (Generic) -> Trading Model Execution -> Generic ML Ops
                                            ↓
Trading Model Execution -> Trading Action Layer -> External Brokers
```

### 6. Required Architecture Updates

1. **Separate Model Execution from ML Ops in diagrams**
2. **Add standardized interface layers to container diagram**
3. **Show clear deployment boundaries with different colors/patterns**
4. **Define gRPC interface contracts in component diagrams**
5. **Update Trading Service to show interface implementations**
6. **Add Domain Registry as central coordination point**

### 7. Interface Validation Rules

#### Data Ingestion Interface
- MUST implement standard gRPC contract
- MUST register schema with Domain Registry
- MUST use standard EventBus topics format
- CAN have domain-specific source connectors

#### Model Execution Interface  
- MUST implement standard ML Ops API
- MUST use generic ML Ops Platform for training
- MUST report standard metrics format
- CAN have domain-specific feature engineering

#### Action Layer Interface
- MUST implement standard Model Execution API
- MUST validate actions through generic framework
- MUST audit all actions to generic monitoring
- CAN have domain-specific execution logic

This analysis shows the current architecture needs significant restructuring to properly separate generic platform services from domain-specific implementations while maintaining clean, standardized interfaces.