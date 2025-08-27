# Neural Trader V2 Architecture - Phase 3 (CORRECTED)

## Architecture Overview

This directory contains the CORRECTED architecture documents for Neural Trader V2, implementing the ACTUAL MVP architecture with binary separation, embedded ruv-FANN, and DAA Coordinators.

## Critical Architecture Decisions

### 1. Binary Separation (NOT layers within one binary)

```
neural-ml-ops      (Training Binary)
├── Feature engineering (Rust)
├── ruv-FANN model training
├── Model registry (config-store)
└── Event publishing (Redis Streams)

neural-trading     (Execution Binary)
├── DAA Coordinator (decision making)
├── Embedded ruv-FANN inference
├── Market data processing
└── Order execution (Alpaca)

neural-core        (Shared Library)
├── Common data types
├── Event streaming traits
├── ruv-FANN integration
└── Redis Streams client
```

### 2. Event Backbone: Redis Streams (NOT microservices)

```
Redis Streams:
├── features:computed     (ML Ops → Trading)
├── models:updates        (ML Ops → Trading)
├── trading:signals       (Trading → Monitoring)
└── orders:executed       (Trading → Audit)
```

### 3. Embedded Neural Networks (NO Python ML platform)

- ruv-FANN models trained in `neural-ml-ops`
- ruv-FANN inference embedded in `neural-trading` 
- < 1ms inference latency (no network calls)
- Models stored in config-store, cached in-memory

### 4. DAA Coordinators (ONLY in domain binaries)

- DAA Coordinator embedded in `neural-trading` binary
- Drives ALL trading decisions
- NO DAA in `neural-ml-ops` (training only)

## Architecture Documents

### [system-architecture.md](system-architecture.md)
- Complete system overview with binary interactions
- Event-driven communication patterns
- Infrastructure services (Redis, TimescaleDB, etc.)

### [component-design.md](component-design.md)  
- Detailed component design for each binary
- ruv-FANN integration patterns
- DAA Coordinator implementation

### [rust-layer-separation.md](rust-layer-separation.md)
- Rust workspace structure
- Binary separation implementation
- Shared library (`neural-core`) design

### [integration-patterns.md](integration-patterns.md)
- Redis Streams event patterns
- Binary communication protocols
- Data flow between binaries

### [deployment-architecture.md](deployment-architecture.md)
- Kubernetes deployment for 3 binaries
- Container orchestration patterns
- Infrastructure as code

## Key Principles

### ✅ CORRECT Architecture
1. **Binary Separation**: 3 Rust binaries with clear boundaries
2. **Embedded ML**: ruv-FANN models embedded in binaries
3. **Event Backbone**: Redis Streams for inter-binary communication
4. **Quality First**: New build from scratch, not migration
5. **DAA in Domains**: Decision coordinators only in execution binaries

### ❌ WRONG Architecture (Avoided)
1. ~~Microservices~~ → Binary separation
2. ~~Python ML platform~~ → Embedded ruv-FANN
3. ~~Service mesh~~ → Redis Streams
4. ~~Layered monolith~~ → Binary separation
5. ~~Migration complexity~~ → Quality-first new build

## Implementation Flow

### Phase 1: Core Infrastructure
1. Setup Redis Streams event backbone
2. Implement config-store service (already exists)
3. Create neural-core shared library

### Phase 2: ML Operations Binary
1. Build neural-ml-ops binary
2. Implement ruv-FANN training pipeline
3. Create feature engineering pipeline
4. Setup model registry integration

### Phase 3: Trading Binary
1. Build neural-trading binary  
2. Implement DAA Coordinator
3. Embed ruv-FANN inference engine
4. Create Alpaca execution integration

### Phase 4: Integration & Testing
1. Wire up Redis Streams communication
2. Implement end-to-end testing
3. Setup monitoring and observability
4. Performance optimization

## Success Metrics

- **Latency**: < 1ms inference, < 5ms trading decisions
- **Throughput**: > 1000 trades/sec, > 100k features/sec  
- **Availability**: 99.9% uptime with binary-level resilience
- **Maintainability**: Clear separation enables independent development
- **Performance**: Full Rust stack with embedded inference

## Migration from Current State

This is a **QUALITY-FIRST NEW BUILD**, not a migration:

1. Build new architecture in parallel
2. Migrate data and configurations
3. Run both systems during validation
4. Switch traffic to new system
5. Decommission old architecture

No gradual migration - clean cutover to ensure architectural integrity.