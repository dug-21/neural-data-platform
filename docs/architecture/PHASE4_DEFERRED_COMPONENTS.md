# Phase 4 - Deferred Components

## Executive Summary
Certain components from the legacy codebase have been identified as valuable but not critical for the initial 3-binary architecture rollout. These will be rebuilt as separate services in Phase 4.

## Deferred to Phase 4

### 1. Backtesting Service
**Current Location**: `src/backtesting/`
**Decision**: DELETE now, rebuild later
**Rationale**: 
- Not required for live trading operation
- Can be developed independently without affecting core system
- Better implemented as a separate service that can run offline
- Reduces initial migration complexity

**Future Architecture**:
```yaml
backtesting-service/
├── src/
│   ├── engines/
│   │   ├── monte_carlo.rs
│   │   ├── walk_forward.rs
│   │   └── historical.rs
│   ├── strategies/
│   │   └── strategy_loader.rs
│   └── main.rs
└── Cargo.toml
```

**Key Features to Rebuild**:
- Monte Carlo simulations with configurable parameters
- Walk-forward optimization
- A/B testing framework
- Integration with neural-ml-ops for model evaluation
- Historical data replay from config-store

### 2. Redis Streams Integration
**Current Status**: Deferred per initial requirements
**Decision**: Implement in Phase 4
**Rationale**:
- Current in-memory event bus sufficient for MVP
- Allows focus on core functionality first
- Can be added transparently through existing EventBus traits

**Future Implementation**:
- Replace in-memory event bus in neural-core
- Add Redis Streams backend
- Enable inter-binary communication
- Implement event persistence and replay

### 3. Advanced Monitoring Dashboard
**Current Location**: `src/monitoring/`
**Decision**: Basic health checks only for now
**Rationale**:
- Current logging and metrics sufficient for MVP
- Can leverage existing observability tools initially
- Custom dashboard is nice-to-have, not critical

## Benefits of Deferring

1. **Reduced Complexity**: Focus on core trading functionality
2. **Faster Delivery**: Ship Phase 3 sooner
3. **Clean Separation**: Each Phase 4 component can be a separate service
4. **Independent Development**: Teams can work on these in parallel later
5. **Better Design**: Learn from Phase 3 before building these components

## Phase 4 Timeline (Tentative)

### Sprint 1: Redis Streams
- Implement Redis backend for EventBus
- Add persistence layer
- Enable cross-binary communication

### Sprint 2: Backtesting Service
- Create new backtesting service
- Implement Monte Carlo engine
- Add walk-forward optimization
- Integrate with neural-ml-ops

### Sprint 3: Monitoring Dashboard
- Build custom monitoring UI
- Add real-time metrics visualization
- Implement alerting system

## Migration Impact

### Code Reduction
By deferring backtesting:
- Remove 6 additional files (~2,000 lines)
- Total removal increases from 77% to 80%
- Simpler initial deployment

### Risk Assessment
- **Low Risk**: Backtesting not required for production trading
- **Medium Risk**: Redis Streams delay means temporary scaling limitations
- **Mitigation**: In-memory bus sufficient for initial load

## Success Criteria for Phase 3 (Without These Components)

✅ Core trading system operational
✅ ML training pipeline functional
✅ DAA coordination working
✅ Basic health monitoring active
✅ Configuration management integrated

## Conclusion

Deferring the backtesting engine and other non-critical components to Phase 4 allows us to:
1. Ship the core 3-binary architecture faster
2. Reduce migration risk
3. Focus on essential trading functionality
4. Design better services based on Phase 3 learnings

The backtesting engine, while valuable, is not required for the system to begin trading. It can be added later as an independent service without disrupting the core architecture.