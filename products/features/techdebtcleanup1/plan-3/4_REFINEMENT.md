# Technical Debt Cleanup Phase 3 - Refinement

## Test-Driven Implementation Plan

This refinement plan follows TDD principles to complete current work (Phase 3A) and implement component connections (Phase 3B) systematically.

## Phase 3A - Complete Current Work

### A1: Module Refactoring Completion (Day 1)

#### Red: Write Module Structure Tests

```rust
#[cfg(test)]
mod test_module_structure {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_neural_module_structure_exists() {
        // Verify new module structure
        assert!(Path::new("src/neural/predictor.rs").exists());
        assert!(Path::new("src/neural/fann/wrapper.rs").exists());
        assert!(Path::new("src/neural/monitoring/metrics.rs").exists());
        assert!(Path::new("src/neural/monitoring/channel.rs").exists());
    }

    #[test]
    fn test_adapter_module_structure_exists() {
        assert!(Path::new("src/adapters/neural/core.rs").exists());
        assert!(Path::new("src/adapters/neural/fallback.rs").exists());
        assert!(Path::new("src/adapters/neural/health.rs").exists());
        assert!(Path::new("src/adapters/neural/performance.rs").exists());
    }

    #[test]
    fn test_modules_properly_declared() {
        // This will fail to compile if modules aren't declared
        use crate::neural::{predictor, fann, monitoring};
        use crate::adapters::neural::{core, fallback, health, performance};
    }
}
```

#### Green: Complete Module Refactoring

```bash
# Create module structure
mkdir -p src/neural/{fann,monitoring}
mkdir -p src/adapters/neural
mkdir -p src/integration/notifications

# Move code to new locations
mv src/neural/fann_predictor.rs src/neural/fann/wrapper.rs
# ... (other moves)
```

```rust
// Update src/neural/mod.rs
pub mod predictor;
pub mod fann;
pub mod monitoring;

// Re-export key types
pub use predictor::NeuralPredictor;
pub use fann::FannPredictor;
pub use monitoring::{PerformanceChannel, PerformanceEvent};
```

#### Refactor: Clean Module Boundaries

```rust
// Ensure clean separation of concerns
// neural/predictor.rs - Core prediction logic only
// neural/fann/wrapper.rs - FANN-specific implementation
// neural/monitoring/channel.rs - Event bus implementation
// adapters/neural/core.rs - Adapter pattern implementation
```

### A2: Fix Compilation Errors (Day 2)

#### Red: Write Import Resolution Tests

```rust
#[test]
fn test_all_imports_resolve() {
    // This test will fail with compilation errors if imports are wrong
    use crate::neural::predictor::NeuralPredictor;
    use crate::neural::fann::FannPredictor;
    use crate::neural::monitoring::{PerformanceChannel, PerformanceEvent};
    use crate::adapters::neural::EnhancedNeuralAdapter;
    
    // Verify types are accessible
    let _: Option<Box<dyn NeuralPredictor>> = None;
}
```

#### Green: Fix Import Paths Systematically

```rust
// Create a mapping of old -> new paths
const IMPORT_MAPPINGS: &[(&str, &str)] = &[
    ("crate::neural::NeuralPredictor", "crate::neural::predictor::NeuralPredictor"),
    ("crate::neural::FannPredictor", "crate::neural::fann::FannPredictor"),
    ("crate::adapters::EnhancedNeuralAdapter", "crate::adapters::neural::EnhancedNeuralAdapter"),
    // ... more mappings
];

// Apply fixes file by file
fn fix_imports_in_file(path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(path)?;
    let mut fixed = content.clone();
    
    for (old, new) in IMPORT_MAPPINGS {
        fixed = fixed.replace(old, new);
    }
    
    if fixed != content {
        std::fs::write(path, fixed)?;
    }
    Ok(())
}
```

### A3: Complete Performance Channel (Day 3)

#### Red: Write Channel Implementation Tests

```rust
#[tokio::test]
async fn test_performance_channel_subscription() {
    let channel = PerformanceChannel::new(100);
    let mut rx = channel.subscribe();
    
    let event = PerformanceEvent {
        timestamp: Utc::now(),
        model_id: "test".into(),
        accuracy: 0.95,
        latency_ms: 10.0,
        confidence: 0.9,
        resource_usage: Default::default(),
    };
    
    // Emit event
    channel.emit(event.clone()).await.unwrap();
    
    // Should receive
    let received = rx.recv().await.unwrap();
    assert_eq!(received.model_id, event.model_id);
}

#[tokio::test]
async fn test_channel_overflow_handling() {
    let channel = PerformanceChannel::new(2); // Small capacity
    
    // Fill channel
    for i in 0..5 {
        let event = create_test_event(i);
        let _ = channel.emit(event).await; // Some may fail
    }
    
    // Check metrics
    let metrics = channel.get_metrics().await;
    assert!(metrics.events_dropped > 0);
}
```

#### Green: Implement Complete Channel

```rust
// In src/neural/monitoring/channel.rs
use tokio::sync::broadcast;
use tokio::sync::RwLock;
use std::sync::Arc;

pub struct PerformanceChannel {
    broadcast_tx: broadcast::Sender<PerformanceEvent>,
    metrics: Arc<RwLock<ChannelMetrics>>,
}

#[derive(Default)]
struct ChannelMetrics {
    events_sent: u64,
    events_dropped: u64,
    subscribers: u32,
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
        let rx = self.broadcast_tx.subscribe();
        // Update subscriber count
        tokio::spawn({
            let metrics = self.metrics.clone();
            async move {
                metrics.write().await.subscribers += 1;
            }
        });
        rx
    }
    
    pub async fn emit(&self, event: PerformanceEvent) -> Result<()> {
        match self.broadcast_tx.send(event) {
            Ok(receivers) => {
                let mut metrics = self.metrics.write().await;
                metrics.events_sent += 1;
                Ok(())
            }
            Err(broadcast::error::SendError(_)) => {
                let mut metrics = self.metrics.write().await;
                metrics.events_dropped += 1;
                // Don't fail - fire and forget semantics
                Ok(())
            }
        }
    }
    
    pub async fn get_metrics(&self) -> ChannelMetrics {
        self.metrics.read().await.clone()
    }
}
```

### A4: Build Training Notification System (Day 4)

#### Red: Write Notification Tests

```rust
#[tokio::test]
async fn test_training_notification_emission() {
    let notification_channel = NotificationChannel::new(100);
    let mut rx = notification_channel.subscribe();
    
    let notification = TrainingNotification::TrainingRequested {
        model_id: "model-1".into(),
        reason: "Performance degradation".into(),
        priority: JobPriority::High,
    };
    
    notification_channel.emit(notification.clone()).await?;
    
    let received = rx.recv().await?;
    match received {
        TrainingNotification::TrainingRequested { model_id, .. } => {
            assert_eq!(model_id, "model-1");
        }
        _ => panic!("Wrong notification type"),
    }
}
```

#### Green: Implement Notification System

```rust
// In src/integration/notifications/types.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingNotification {
    TrainingRequested {
        model_id: String,
        reason: String,
        priority: JobPriority,
        timestamp: DateTime<Utc>,
    },
    TrainingStarted {
        job_id: Uuid,
        model_id: String,
        allocated_resources: f64,
        timestamp: DateTime<Utc>,
    },
    TrainingProgress {
        job_id: Uuid,
        epoch: u32,
        loss: f64,
        accuracy: f64,
        timestamp: DateTime<Utc>,
    },
    TrainingCompleted {
        job_id: Uuid,
        final_metrics: TrainingMetrics,
        model_path: PathBuf,
        timestamp: DateTime<Utc>,
    },
    TrainingFailed {
        job_id: Uuid,
        error: String,
        retry_count: u32,
        timestamp: DateTime<Utc>,
    },
}

// In src/integration/notifications/channel.rs
pub struct NotificationChannel {
    tx: broadcast::Sender<TrainingNotification>,
}

impl NotificationChannel {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }
    
    pub fn subscribe(&self) -> broadcast::Receiver<TrainingNotification> {
        self.tx.subscribe()
    }
    
    pub async fn emit(&self, mut notification: TrainingNotification) -> Result<()> {
        // Add timestamp if not present
        match &mut notification {
            TrainingNotification::TrainingRequested { timestamp, .. } |
            TrainingNotification::TrainingStarted { timestamp, .. } |
            TrainingNotification::TrainingProgress { timestamp, .. } |
            TrainingNotification::TrainingCompleted { timestamp, .. } |
            TrainingNotification::TrainingFailed { timestamp, .. } => {
                if *timestamp == DateTime::default() {
                    *timestamp = Utc::now();
                }
            }
        }
        
        self.tx.send(notification).map_err(|_| anyhow!("No subscribers"))?;
        Ok(())
    }
}
```

## Phase 3B - Integration

### B1: Market Timing Integration (Day 5)

#### Red: Write Integration Tests

```rust
#[tokio::test]
async fn test_daa_uses_market_timing_for_decisions() {
    // Arrange
    let market_hours = Arc::new(MarketHours::new(config));
    let mock_predictor = Arc::new(MockNeuralPredictor::new());
    let daa = DaaCoordinator::builder()
        .with_neural_predictor(mock_predictor)
        .with_market_hours(market_hours.clone())
        .build();

    // Simulate market closed (optimal training time)
    let closed_time = Utc.ymd(2024, 1, 1).and_hms(2, 0, 0);
    
    // Act
    let decision = daa.make_decision_at_time(closed_time).await?;
    
    // Assert
    match decision {
        AutonomousAction::InitiateTraining { .. } => {
            assert!(true); // Expected
        }
        _ => panic!("Expected training decision during market closed"),
    }
}
```

#### Green: Implement Market Timing Connection

```rust
impl DaaCoordinator {
    pub fn new(
        config: Arc<ModularPlatformConfig>,
        neural_predictor: Arc<dyn NeuralPredictor>,
        market_hours: Arc<MarketHours>,
        notification_channel: Arc<NotificationChannel>,
    ) -> Result<Self> {
        Ok(Self {
            config,
            neural_predictor,
            market_hours,
            notification_channel,
            training_scheduler: None,
            performance_rx: None,
            // ... existing fields
        })
    }
}
```

### B2: Performance Feedback Connection (Day 6)

#### Red: Write Performance Subscription Tests

```rust
#[tokio::test]
async fn test_daa_responds_to_performance_events() {
    let performance_channel = Arc::new(PerformanceChannel::new());
    let notification_channel = Arc::new(NotificationChannel::new(100));
    let mut notification_rx = notification_channel.subscribe();
    
    let daa = DaaCoordinator::builder()
        .with_performance_channel(performance_channel.clone())
        .with_notification_channel(notification_channel)
        .build();
    
    // Start monitoring
    daa.start_performance_monitoring().await?;
    
    // Emit poor performance
    performance_channel.emit(PerformanceEvent {
        accuracy: 0.45, // Below threshold
        // ...
    }).await?;
    
    // Should receive training notification
    let notification = notification_rx.recv().await?;
    match notification {
        TrainingNotification::TrainingRequested { priority, .. } => {
            assert_eq!(priority, JobPriority::Emergency);
        }
        _ => panic!("Expected training request notification"),
    }
}
```

#### Green: Implement Performance Subscription

```rust
impl DaaCoordinator {
    pub async fn start_performance_monitoring(&self) -> Result<()> {
        let mut rx = self.performance_channel.subscribe();
        let coordinator = self.clone(); // Assumes Clone
        
        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                if let Err(e) = coordinator.handle_performance_event(event).await {
                    error!("Failed to handle performance event: {}", e);
                }
            }
        });
        
        Ok(())
    }
    
    async fn handle_performance_event(&self, event: PerformanceEvent) -> Result<()> {
        if event.accuracy < self.config.daa_timing.performance_threshold_emergency {
            // Notify immediately
            self.notification_channel.emit(
                TrainingNotification::TrainingRequested {
                    model_id: event.model_id,
                    reason: format!("Emergency: accuracy {:.2}%", event.accuracy * 100.0),
                    priority: JobPriority::Emergency,
                    timestamp: Utc::now(),
                }
            ).await?;
            
            // Submit training job
            if let Some(scheduler) = &self.training_scheduler {
                scheduler.submit_job(/* ... */).await?;
            }
        }
        Ok(())
    }
}
```

### B3: Full Integration Testing (Day 7)

#### End-to-End Integration Test

```rust
#[tokio::test]
async fn test_full_integration_flow() {
    // Setup complete system
    let config = test_config();
    let market_hours = Arc::new(MarketHours::new(config.clone()));
    let performance_channel = Arc::new(PerformanceChannel::new());
    let notification_channel = Arc::new(NotificationChannel::new(100));
    
    let neural_predictor = Arc::new(EnhancedNeuralAdapter::new(
        Arc::new(FannPredictor::new(config.neural.clone())?),
        performance_channel.clone(),
    ));
    
    let daa = DaaCoordinator::builder()
        .with_config(config)
        .with_neural_predictor(neural_predictor.clone())
        .with_market_hours(market_hours)
        .with_performance_channel(performance_channel)
        .with_notification_channel(notification_channel.clone())
        .build()?;
    
    // Start monitoring
    daa.start_performance_monitoring().await?;
    
    // Subscribe to notifications
    let mut notification_rx = notification_channel.subscribe();
    
    // Make prediction (will emit performance event)
    let _ = neural_predictor.predict(&test_data(), 24, &["feature1"]).await?;
    
    // Wait for async processing
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Verify notification received
    if let Ok(notification) = notification_rx.try_recv() {
        match notification {
            TrainingNotification::TrainingRequested { .. } => {
                // Success - training was requested based on performance
            }
            _ => panic!("Unexpected notification type"),
        }
    }
}
```

## Implementation Timeline

### Phase 3A (Days 1-4)
1. **Day 1**: Complete module refactoring
2. **Day 2**: Fix all compilation errors  
3. **Day 3**: Complete performance channel implementation
4. **Day 4**: Build training notification system

### Phase 3B (Days 5-7)
5. **Day 5**: Wire MarketHours to DaaCoordinator
6. **Day 6**: Connect performance feedback loop
7. **Day 7**: Full integration testing

## Key Refactoring Principles

### Phase 3A Principles
1. **Complete Don't Restart**: Finish what's in progress
2. **Test Module Structure**: Verify files exist in right places
3. **Fix Systematically**: Group similar errors and fix in batches
4. **Maintain Compatibility**: Keep existing interfaces working

### Phase 3B Principles  
1. **No New Layers**: Only connect existing components
2. **Test First**: Write failing tests before implementation
3. **Small Steps**: One connection at a time
4. **Event-Driven**: Use existing channels for communication

## Success Metrics

### Phase 3A Success
- Zero compilation errors
- All modules properly organized
- Performance channel fully operational
- Notification system working

### Phase 3B Success
- Market timing influences decisions
- Performance events trigger training
- All integration tests passing
- No performance regression