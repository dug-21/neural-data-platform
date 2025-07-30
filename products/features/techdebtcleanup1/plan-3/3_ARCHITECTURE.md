# Technical Debt Cleanup Phase 3 - Architecture

## Executive Summary

This architecture document describes the two-phase approach for completing the technical debt cleanup:
- **Phase 3A**: Complete current work (module refactoring, fix compilation errors, complete performance channel, build training notifications)
- **Phase 3B**: Integration design for connecting existing neural-trader components without adding new architectural layers

## Phase 3A Architecture - Complete Current Work

### Module Structure After Refactoring

```
src/
├── neural/
│   ├── mod.rs (minimal, only re-exports)
│   ├── predictor.rs (main prediction logic)
│   ├── fann/
│   │   ├── mod.rs
│   │   ├── wrapper.rs (FANN-specific code)
│   │   └── types.rs
│   └── monitoring/
│       ├── mod.rs
│       ├── metrics.rs (performance tracking)
│       └── channel.rs (event bus implementation)
│
├── adapters/
│   ├── mod.rs
│   └── neural/
│       ├── mod.rs
│       ├── core.rs (core adapter logic)
│       ├── fallback.rs (fallback mechanisms)
│       ├── health.rs (health monitoring)
│       └── performance.rs (performance event emission)
│
└── integration/
    ├── notifications/
    │   ├── mod.rs
    │   ├── channel.rs (notification bus)
    │   └── types.rs (notification types)
    └── daa_coordinator.rs
```

### Phase 3A Component Architecture

#### 1. Completed Performance Channel

```rust
// In neural/monitoring/channel.rs
pub struct PerformanceChannel {
    broadcast_tx: broadcast::Sender<PerformanceEvent>,
    metrics: Arc<RwLock<ChannelMetrics>>,
}

impl PerformanceChannel {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            broadcast_tx: tx,
            metrics: Arc::new(RwLock::new(ChannelMetrics::default())),
        }
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<PerformanceEvent> {
        self.broadcast_tx.subscribe()
    }
    
    pub async fn emit(&self, event: PerformanceEvent) -> Result<()> {
        match self.broadcast_tx.send(event) {
            Ok(_) => {
                self.metrics.write().await.events_sent += 1;
                Ok(())
            }
            Err(_) => {
                self.metrics.write().await.events_dropped += 1;
                Err(anyhow!("Channel full"))
            }
        }
    }
}
```

#### 2. Training Notification System

```rust
// In integration/notifications/types.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingNotification {
    TrainingRequested {
        model_id: String,
        reason: String,
        priority: JobPriority,
    },
    TrainingStarted {
        job_id: Uuid,
        model_id: String,
        allocated_resources: f64,
    },
    TrainingProgress {
        job_id: Uuid,
        epoch: u32,
        loss: f64,
        accuracy: f64,
    },
    TrainingCompleted {
        job_id: Uuid,
        final_metrics: TrainingMetrics,
        model_path: PathBuf,
    },
    TrainingFailed {
        job_id: Uuid,
        error: String,
        retry_count: u32,
    },
}
```

### Phase 3A Completion Criteria

1. **Zero Compilation Errors**: All imports resolved, modules properly declared
2. **Performance Channel Working**: Events flow from adapter to subscribers
3. **Notifications Implemented**: Training events properly communicated
4. **Tests Pass**: All existing tests updated for new structure

## Phase 3B Architecture - Integration

### Current Architecture Overview (After Phase 3A)

```
┌─────────────────────────────────────────────────────────────┐
│                        Client Applications                     │
└────────────────────────┬───────────────────────────────────┘
                         │
┌────────────────────────▼───────────────────────────────────┐
│                    NeuralPredictor                          │
│                  (Thin routing layer)                       │
└────────────────────────┬───────────────────────────────────┘
                         │
┌────────────────────────▼───────────────────────────────────┐
│               EnhancedNeuralAdapter                         │
│    (Circuit breakers, fallback, health monitoring)         │
│                  ↓ emits events ↓                           │
└────────────────────────┬───────────────────────────────────┘
                         │                    │
┌────────────────────────▼────────┐  ┌───────▼──────────────┐
│         FannPredictor           │  │  PerformanceChannel   │
│    (FANN neural networks)       │  │  (Event bus <1ms)     │
└─────────────────────────────────┘  └───────┬──────────────┘
                                             │ subscribes
┌────────────────────────────────────────────▼──────────────┐
│                    DaaCoordinator                          │
│         (Autonomous decision orchestration)                │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────────┐  │
│  │MarketHours  │  │TrainingScheduler│  │Risk Management│  │
│  │(Timing)     │  │(Job Queue)      │  │(Position Mgmt)│  │
│  └─────────────┘  └──────────────┘  └─────────────────┘  │
└────────────────────────────────────────────────────────────┘
```

### Phase 3B Integration Points

#### 1. Market Timing Integration

**Current State**: MarketHours exists as a standalone utility
**Target State**: Direct integration with DaaCoordinator

```rust
// Simple wiring - no new components
pub struct DaaCoordinator {
    neural_predictor: Arc<dyn NeuralPredictor>,
    market_hours: Arc<MarketHours>,  // <- Add this field
    training_scheduler: Option<Arc<DaaTrainingScheduler>>, // <- Initialize this
    performance_rx: Option<Receiver<PerformanceEvent>>, // <- Subscribe to events
    notification_channel: Arc<NotificationChannel>, // <- From Phase 3A
    // ... existing fields remain
}
```

#### 2. Performance Feedback Loop

```
EnhancedNeuralAdapter
    │
    ├─► Performs prediction
    ├─► Calculates metrics
    └─► Emits PerformanceEvent ──► PerformanceChannel
                                        │
                                        ├─► DAA subscribes
                                        └─► Triggers training decisions
```

#### 3. Training Orchestration Flow

```
Performance Degradation Detected
    │
    ▼
DaaCoordinator evaluates:
    ├─► Market timing (via MarketHours)
    ├─► Resource availability
    └─► Performance severity
        │
        ▼
    Decision Matrix:
        ├─► Market closed + Poor performance = Train now
        ├─► Market slow + Degrading = Schedule training
        └─► Market active + Critical = Emergency training
            │
            ▼
        Submit to TrainingScheduler
            │
            ▼
        Emit TrainingNotification
```

### Phase 3B Data Flow Patterns

#### Performance Event Flow
```rust
PerformanceEvent {
    timestamp: DateTime<Utc>,
    model_id: String,
    prediction_accuracy: f64,
    latency_ms: f64,
    resource_usage: ResourceMetrics,
}
    │
    ├─► Statistical aggregation (existing)
    ├─► Metric export (existing)
    └─► DAA decision trigger (NEW CONNECTION)
```

#### Training Decision Flow
```rust
TrainingDecision {
    trigger: PerformanceDegradation | Scheduled | Emergency,
    market_window: TrainingWindow,
    priority: JobPriority,
    resource_allocation: f64,
}
    │
    ├─► TrainingScheduler.submit_job()
    └─► NotificationChannel.emit(TrainingRequested)
```

### Configuration Architecture

```yaml
# Existing ModularPlatformConfig extended with:
daa_timing:
  enable_market_aware_training: true
  performance_thresholds:
    emergency: 0.5
    high: 0.6
    normal: 0.7
  market_hours_training_allocation: 0.25
  off_hours_training_allocation: 0.90
```

## Architectural Decisions

### Phase 3A Decisions
1. **Module Boundaries**: Clear separation between neural, adapters, and integration
2. **Event-Driven Communication**: Use channels for loose coupling
3. **No Mock Removal**: Keep mocks in test modules only
4. **Incremental Refactoring**: One module at a time to maintain stability

### Phase 3B Decisions
1. **No New Coordination Layer**: PerformanceChannel already provides event-driven coordination
2. **Direct Component Wiring**: Simple field additions and subscriptions
3. **Reuse Existing Event Types**: No new event types or transformations
4. **Configuration Extension**: Maintain backward compatibility

## Implementation Timeline

### Phase 3A (Days 1-4)
1. **Day 1**: Complete module refactoring
2. **Day 2**: Fix all compilation errors
3. **Day 3**: Implement performance channel fully
4. **Day 4**: Build training notification system

### Phase 3B (Days 5-7)
5. **Day 5**: Wire MarketHours to DaaCoordinator
6. **Day 6**: Connect performance feedback loop
7. **Day 7**: Initialize training scheduler and test integration

## Performance Considerations

### Phase 3A
- **Compilation Time**: May increase slightly due to more modules
- **Runtime Performance**: No impact, same code reorganized
- **Memory Usage**: Unchanged

### Phase 3B
- **Event Processing**: < 1ms latency (PerformanceChannel already optimized)
- **Decision Making**: < 10ms for training decisions (simple logic)
- **Memory Overhead**: ~0 (reusing existing components)
- **CPU Impact**: Negligible (event-driven, not polling)

## Testing Strategy

### Phase 3A Tests
1. **Module Tests**: Verify each new module compiles and tests pass
2. **Import Tests**: Ensure all imports resolve correctly
3. **Channel Tests**: Verify event emission and subscription
4. **Notification Tests**: Confirm notification delivery

### Phase 3B Tests
1. **Integration Tests**: Verify event flow from prediction to training
2. **Timing Tests**: Ensure market hours influence decisions
3. **Performance Tests**: Confirm no latency regression
4. **Failure Tests**: Validate graceful degradation

## Migration Path

### Phase 3A Migration
1. Create new module structure
2. Move code incrementally
3. Update imports file by file
4. Run tests after each move

### Phase 3B Migration
1. Add MarketHours field to DaaCoordinator
2. Initialize TrainingScheduler on startup
3. Subscribe to PerformanceChannel
4. Update decision logic to consider timing
5. Deploy with feature flag for gradual rollout

## Conclusion

This phased architecture approach ensures systematic completion of technical debt cleanup:
- Phase 3A completes all in-progress work and establishes a clean foundation
- Phase 3B connects existing components without adding complexity
- Both phases maintain system stability through incremental changes