# Technical Debt Cleanup Phase 1 - Pseudocode

## Overview

This document provides high-level pseudocode for implementing the critical fixes identified in the technical debt analysis.

## 1. Centralized Model Routing

### 1.1 Updated FannPredictor

```pseudocode
class FannPredictor:
    // Central routing method - ALL predictions go through here
    function execute_model(model_type: string, data: TimeSeriesData[], config: ModelConfig) -> PredictionResult[]:
        // Step 1: Validate model type
        if model_type not in supported_models:
            throw UnsupportedModelError(model_type)
        
        // Step 2: Route to appropriate FANN implementation
        match model_type:
            case "DeepAR", "TCN", "NHITS", "LSTM", "GRU":
                // Convert to FANN network representation
                network = get_or_create_fann_network(model_type, config)
                
                // Prepare data for FANN
                input_data = prepare_fann_input(data)
                
                // Execute through ruv-fann
                raw_output = network.run(input_data)
                
                // Convert back to PredictionResult
                return format_predictions(raw_output, model_type)
            
            case "MLP":
                // Direct FANN execution
                return execute_standard_fann(data, config)
            
            default:
                throw UnknownModelError(model_type)
    
    // Private method - prevents external access
    private function get_or_create_fann_network(model_type: string, config: ModelConfig) -> Network:
        cache_key = generate_cache_key(model_type, config)
        
        if cache_key in network_cache:
            return network_cache[cache_key]
        
        // Create FANN network with architecture matching model type
        network = match model_type:
            case "DeepAR":
                create_deepar_fann_architecture(config)
            case "TCN":
                create_tcn_fann_architecture(config)
            case "LSTM":
                create_lstm_fann_architecture(config)
            // ... other models
        
        network_cache[cache_key] = network
        return network
```

### 1.2 Updated EnhancedNeuralAdapter

```pseudocode
class EnhancedNeuralAdapter:
    fann_predictor: FannPredictor  // ONLY predictor used
    // REMOVED: neuro_divergent_adapter
    
    function predict(model_name: string, data: TimeSeriesData[], horizon: int) -> PredictionResult[]:
        // ALWAYS route through FannPredictor
        try:
            return fann_predictor.execute_model(model_name, data, config)
        catch ModelError as e:
            if fallback_enabled:
                return fallback_manager.handle_prediction_failure(e, data)
            else:
                throw e
    
    // REMOVED: Any direct adapter access methods
```

## 2. DAA Training Orchestration

### 2.1 Enhanced DaaCoordinator

```pseudocode
class DaaCoordinator:
    autonomous_training: AutonomousTrainingEngine  // No longer Optional!
    training_scheduler: DAATrainingScheduler       // No longer Optional!
    market_hours: MarketHours
    performance_bridge: PerformanceTrainingBridge
    
    // Main orchestration loop
    async function orchestrate_autonomous_operations():
        while running:
            // Step 1: Gather current state
            market_state = analyze_market_conditions()
            performance_state = collect_performance_metrics()
            
            // Step 2: Make autonomous decision
            decision = decide_action(market_state, performance_state)
            
            // Step 3: Execute decision
            match decision:
                case AutonomousAction.InitiateTraining:
                    submit_training_job(performance_state)
                case AutonomousAction.ContinueTrading:
                    continue_trading_operations()
                case AutonomousAction.EmergencyTraining:
                    submit_emergency_training()
            
            // Step 4: Wait before next evaluation
            sleep(evaluation_interval)
    
    private function decide_action(market: MarketState, performance: PerformanceState) -> AutonomousAction:
        // Market timing awareness
        training_window = market_hours.get_current_training_window()
        market_intensity = market.intensity_score
        
        // Performance evaluation
        accuracy = performance.recent_accuracy
        degradation = performance.degradation_score
        
        // Decision matrix
        if accuracy < critical_threshold:
            return AutonomousAction.EmergencyTraining
        
        if training_window == TrainingWindow.Optimal and accuracy < target_threshold:
            return AutonomousAction.InitiateTraining
        
        if training_window == TrainingWindow.Restricted:
            return AutonomousAction.ContinueTrading  // Never train during restricted
        
        if market_intensity < 0.2 and needs_retraining(performance):
            return AutonomousAction.InitiateTraining
        
        return AutonomousAction.ContinueTrading
    
    private function submit_training_job(performance: PerformanceState):
        // Create training job with market awareness
        job = DAATrainingJob {
            id: generate_uuid(),
            priority: calculate_priority(performance),
            decision: create_training_decision(performance),
            resources: estimate_resources(),
            market_constraints: market_hours.get_constraints()
        }
        
        // Submit to scheduler
        training_scheduler.submit_job(job)
```

### 2.2 PerformanceTrainingBridge

```pseudocode
class PerformanceTrainingBridge:
    performance_monitor: PerformanceMonitor
    training_engine: AutonomousTrainingEngine
    market_hours: MarketHours
    conversion_cache: HashMap<string, PerformanceSnapshot>
    
    // Continuous evaluation loop
    async function continuous_evaluation_loop():
        while running:
            // Step 1: Collect performance metrics
            raw_metrics = performance_monitor.get_latest_metrics()
            
            // Step 2: Convert to training format
            snapshot = convert_metrics_to_snapshot(raw_metrics)
            
            // Step 3: Evaluate with market awareness
            market_window = market_hours.get_current_window()
            
            // Step 4: Check if training needed
            if should_trigger_training(snapshot, market_window):
                trigger_training(snapshot)
            
            // Step 5: Store for historical analysis
            store_performance_history(snapshot)
            
            sleep(evaluation_interval)
    
    private function convert_metrics_to_snapshot(metrics: PerformanceStats) -> PerformanceSnapshot:
        // Map incompatible structures
        return PerformanceSnapshot {
            accuracy: metrics.success_rate,
            confidence: calculate_confidence_from_metrics(metrics),
            price_error: estimate_price_error(metrics),
            sharpe_ratio: calculate_sharpe_from_history(),
            max_drawdown: calculate_drawdown(),
            volatility: calculate_volatility(),
            model_agreement: calculate_model_agreement(metrics),
            consecutive_failures: count_recent_failures(metrics),
            trading_volume: get_recent_volume(),
            profit_loss: calculate_pnl()
        }
    
    private function should_trigger_training(snapshot: PerformanceSnapshot, window: TrainingWindow) -> bool:
        // Never train during restricted windows
        if window == TrainingWindow.Restricted:
            return false
        
        // Always train if critical performance issues
        if snapshot.accuracy < 0.5 or snapshot.consecutive_failures > 10:
            return true
        
        // Consider window quality
        window_multiplier = match window:
            case TrainingWindow.Optimal: 1.0
            case TrainingWindow.Good: 0.7
            case TrainingWindow.Acceptable: 0.4
            case TrainingWindow.Poor: 0.1
        
        // Calculate training score
        training_score = calculate_training_need_score(snapshot)
        return training_score * window_multiplier > training_threshold
```

## 3. Mock Adapter Removal

### 3.1 Removal Process

```pseudocode
// Step 1: Update all imports
foreach file in codebase:
    if file contains "use.*neuro_divergent":
        if import is "adapters::neuro_divergent":
            remove import
            add "use neural::fann_predictor"
    
// Step 2: Update all instantiations
foreach usage of NeuroDivergentAdapter:
    replace with fann_predictor usage
    
// Step 3: Update method calls
foreach call to adapter.predict_deepar() or adapter.predict_tcn():
    replace with fann_predictor.execute_model("DeepAR", ...) or
                 fann_predictor.execute_model("TCN", ...)

// Step 4: Delete mock files
delete src/adapters/neuro_divergent.rs
remove from src/adapters/mod.rs

// Step 5: Update tests
foreach test using mock adapter:
    update to use fann_predictor with test data
```

### 3.2 Compile-Time Enforcement

```pseudocode
// In src/adapters/mod.rs
// Make NeuroDivergentAdapter private to prevent imports
mod neuro_divergent {
    // Content moved to private module if needed for migration
}

// In src/neural/mod.rs
// Export only the approved interface
pub use fann_predictor::{FannPredictor, PredictionResult};
// Do NOT export adapters

// Add compiler directive
#[deprecated(note = "Use FannPredictor.execute_model() instead")]
struct NeuroDivergentAdapter;  // If needed for gradual migration
```

## 4. Integration Flow

### 4.1 Complete Prediction Flow

```pseudocode
// Market Data → Neural Prediction → Trading Decision

async function complete_prediction_flow(market_event: MarketEvent):
    // Step 1: Convert market event to time series
    time_series = convert_to_time_series(market_event)
    
    // Step 2: Get prediction through FannPredictor ONLY
    predictions = fann_predictor.execute_model(
        model_type="ensemble",
        data=time_series,
        config=model_config
    )
    
    // Step 3: Emit performance metrics
    performance_event = create_performance_event(predictions)
    performance_channel.emit(performance_event)
    
    // Step 4: DAA makes decision
    trading_decision = daa_coordinator.evaluate_predictions(predictions)
    
    // Step 5: Execute decision
    if trading_decision.should_trade:
        execute_trade(trading_decision)
    else:
        log_decision("Holding due to: " + trading_decision.reason)
```

### 4.2 Complete Training Flow

```pseudocode
// Performance Degradation → Training Decision → Model Update

async function complete_training_flow():
    // Step 1: Bridge collects metrics
    metrics = performance_bridge.collect_metrics()
    
    // Step 2: Convert to snapshot
    snapshot = performance_bridge.convert_to_snapshot(metrics)
    
    // Step 3: DAA evaluates need
    training_decision = daa_coordinator.evaluate_training_need(snapshot)
    
    // Step 4: Submit if needed
    if training_decision.should_train:
        job = create_training_job(training_decision)
        training_scheduler.submit_job(job)
    
    // Step 5: Execute training
    training_result = await execute_training(job)
    
    // Step 6: Update models in FannPredictor
    fann_predictor.update_model(training_result.model_name, training_result.weights)
```

## 5. Error Handling Strategy

```pseudocode
// Comprehensive error handling for all components

enum ErrorSeverity:
    Critical    // System must stop
    High        // Fallback required
    Medium      // Log and continue
    Low         // Monitor only

function handle_prediction_error(error: Error, context: Context) -> Result:
    severity = classify_error_severity(error)
    
    match severity:
        case Critical:
            // Stop trading, alert operators
            emergency_shutdown(error, context)
            
        case High:
            // Use fallback predictor
            fallback_result = fallback_manager.get_fallback_prediction(context)
            log_error_with_fallback(error, fallback_result)
            return fallback_result
            
        case Medium:
            // Log and use last known good
            log_error(error)
            return cache.get_last_valid_prediction()
            
        case Low:
            // Just monitor
            metrics.increment_error_count(error.type)
            return continue_with_defaults()
```