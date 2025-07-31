# Technical Debt Cleanup Phase 3 - Pseudocode

## Overview

This document provides pseudocode for both Phase 3A (completing current work) and Phase 3B (integration). Phase 3A MUST complete before Phase 3B begins.

## Phase 3A: Complete Current Work

### 1. Module Refactoring Completion

```pseudocode
FUNCTION complete_module_refactoring():
    // Step 1: Identify remaining large modules
    large_modules = FIND_FILES_WITH_LINES > 500
    
    FOR EACH module IN large_modules:
        // Analyze module structure
        components = ANALYZE_MODULE_COMPONENTS(module)
        
        // Create sub-modules
        FOR EACH component IN components:
            sub_module = CREATE_SUB_MODULE(component.name)
            MOVE_CODE(component.code, sub_module)
            UPDATE_VISIBILITY(sub_module.items)
        
        // Update module exports
        UPDATE_MODULE_EXPORTS(module, sub_modules)
        
        // Create mod.rs with proper re-exports
        CREATE_MODULE_INDEX(module, sub_modules)
```

### 2. Fix Compilation Errors

```pseudocode
FUNCTION fix_compilation_errors():
    // Step 1: Categorize errors
    errors = COLLECT_COMPILATION_ERRORS()
    error_categories = {
        imports: [],
        types: [],
        modules: [],
        other: []
    }
    
    CATEGORIZE_ERRORS(errors, error_categories)
    
    // Step 2: Fix import errors first (most common)
    FOR EACH error IN error_categories.imports:
        IF error.type == "unresolved import":
            new_path = FIND_NEW_MODULE_PATH(error.import)
            UPDATE_IMPORT(error.file, error.line, new_path)
        ELSE IF error.type == "duplicate import":
            REMOVE_DUPLICATE(error.file, error.line)
    
    // Step 3: Fix type errors
    FOR EACH error IN error_categories.types:
        IF error.type == "type not found":
            correct_import = FIND_TYPE_MODULE(error.type_name)
            ADD_IMPORT(error.file, correct_import)
    
    // Step 4: Fix module structure errors
    FOR EACH error IN error_categories.modules:
        FIX_MODULE_VISIBILITY(error)
        UPDATE_MODULE_EXPORTS(error)
```

### 3. Complete Performance Channel

```pseudocode
FUNCTION complete_performance_channel():
    // Current state: partial implementation
    // Goal: complete event bus with full functionality
    
    // Step 1: Complete event emission
    FUNCTION emit_performance_event(event):
        // Add to circular buffer
        buffer_index = (self.event_count % BUFFER_SIZE)
        self.event_buffer[buffer_index] = event
        self.event_count += 1
        
        // Broadcast to subscribers
        FOR EACH subscriber IN self.subscribers:
            IF subscriber.is_active():
                TRY:
                    subscriber.send(event.clone())
                CATCH SendError:
                    self.remove_inactive_subscriber(subscriber)
        
        // Update statistics
        self.stats.total_events += 1
        self.stats.events_by_type[event.type] += 1
        
        // Check for performance degradation
        IF event.accuracy < CRITICAL_THRESHOLD:
            self.emit_critical_alert(event)
    
    // Step 2: Complete metrics pipeline
    FUNCTION process_metrics():
        WHILE running:
            batch = COLLECT_EVENTS(batch_size = 100, timeout = 1s)
            
            // Aggregate metrics
            aggregated = self.aggregator.process(batch)
            
            // Export to monitoring systems
            FOR EACH exporter IN self.exporters:
                exporter.export(aggregated)
            
            // Check training triggers
            IF aggregated.avg_accuracy < TRAINING_THRESHOLD:
                self.trigger_training_notification(aggregated)
```

### 4. Build Training Notification System

```pseudocode
FUNCTION build_training_notification_system():
    // New component to create
    
    CLASS TrainingNotificationSystem:
        performance_rx: Receiver<PerformanceEvent>
        notification_tx: Sender<TrainingNotification>
        model_states: HashMap<ModelId, ModelState>
        thresholds: TrainingThresholds
        
        FUNCTION start():
            SPAWN async {
                WHILE let event = performance_rx.recv():
                    self.process_performance_event(event)
            }
        
        FUNCTION process_performance_event(event):
            // Update model state
            state = self.model_states.get_or_insert(event.model_id)
            state.update_metrics(event)
            
            // Check triggers
            notification = CHECK_TRIGGERS(state, self.thresholds)
            
            IF notification IS NOT None:
                self.notification_tx.send(notification)
        
        FUNCTION check_triggers(state, thresholds):
            // Accuracy degradation
            IF state.accuracy < thresholds.accuracy_min:
                RETURN TrainingNotification {
                    trigger: AccuracyDegradation,
                    urgency: HIGH,
                    model_id: state.model_id
                }
            
            // Data drift detection
            IF state.drift_score > thresholds.drift_max:
                RETURN TrainingNotification {
                    trigger: DataDrift,
                    urgency: MEDIUM,
                    model_id: state.model_id
                }
            
            // Time-based retraining
            IF state.time_since_training > thresholds.max_age:
                RETURN TrainingNotification {
                    trigger: ScheduledRetrain,
                    urgency: LOW,
                    model_id: state.model_id
                }
            
            RETURN None
```

## Phase 3B: Integration

### 1. Market Timing Integration

```pseudocode
FUNCTION integrate_market_timing_with_daa():
    // Simple connection - no new layers
    
    // Update DaaCoordinator constructor
    FUNCTION DaaCoordinator::new(config, predictor, market_hours):
        self.config = config
        self.neural_predictor = predictor
        self.market_hours = market_hours  // NEW: Add field
        self.training_scheduler = Some(Arc::new(
            DaaTrainingScheduler::new(market_hours.clone())
        ))
        RETURN self
    
    // Modify decision method
    FUNCTION make_decision(context, position, data):
        // Get timing context
        current_time = NOW()
        market_state = self.market_hours.get_market_intensity(current_time)
        training_window = self.market_hours.get_training_window(current_time)
        
        // Get performance
        performance = self.get_current_performance()
        
        // Timing-aware decisions
        IF market_state.intensity < 0.3 AND training_window == OPTIMAL:
            IF performance.accuracy < 0.8:
                RETURN InitiateTraining
        
        // Continue existing logic
        RETURN self.make_trading_decision(context, position, data)
```

### 2. Performance Event Subscription

```pseudocode
FUNCTION connect_performance_to_training():
    // Subscribe DAA to performance events
    
    FUNCTION DaaCoordinator::start_monitoring():
        // Subscribe to performance channel
        performance_rx = self.performance_channel.subscribe()
        
        // Spawn monitoring task
        SPAWN async {
            WHILE let event = performance_rx.recv():
                self.handle_performance_event(event)
        }
    
    FUNCTION handle_performance_event(event):
        // Check thresholds
        IF event.accuracy < EMERGENCY_THRESHOLD:
            self.submit_training_job(EMERGENCY, "Critical performance")
        ELSE IF event.accuracy < NORMAL_THRESHOLD:
            // Check market timing
            IF self.should_train_now():
                self.submit_training_job(NORMAL, "Performance degradation")
```

### 3. Training Scheduler Initialization

```pseudocode
FUNCTION initialize_training_scheduler():
    // Already has field, just needs initialization
    
    // In DaaCoordinator constructor
    IF self.training_scheduler IS None:
        self.training_scheduler = Some(Arc::new(
            DaaTrainingScheduler::new(
                market_hours: self.market_hours.clone(),
                config: self.config.training
            )
        ))
```

### 4. Complete Integration Validation

```pseudocode
FUNCTION validate_integration():
    // Test complete flow
    
    TEST prediction_to_training_flow:
        // Make prediction
        prediction = neural_predictor.predict(data)
        
        // Verify performance event emitted
        ASSERT performance_channel.last_event IS NOT None
        
        // Verify DAA received event
        WAIT_FOR daa.last_performance_event == performance_channel.last_event
        
        // Verify training decision made
        IF prediction.accuracy < THRESHOLD:
            ASSERT daa.training_jobs.len() > 0
        
        // Verify market timing respected
        IF market_hours.is_trading_hours():
            ASSERT daa.training_jobs.last().scheduled_for > market_close
```

## Key Implementation Notes

1. **Phase 3A Must Complete First**: No integration until all components compile and work
2. **No New Layers**: Phase 3B only connects existing components
3. **Simple Wiring**: Direct field additions and subscriptions
4. **Test Each Phase**: Validate 3A before starting 3B