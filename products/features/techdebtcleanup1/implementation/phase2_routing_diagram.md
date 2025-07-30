# Central Routing Flow Diagram (Updated Target State)

## Simplified Component Architecture

```mermaid
graph TB
    %% External API Layer
    subgraph "Public API"
        A[Client Code]
        B[NeuralPredictor::predict]
        C[NeuralPredictor::predict_ensemble]
    end
    
    %% Enhanced Adapter - Single Implementation
    subgraph "EnhancedNeuralAdapter (Primary Implementation)"
        D[predict_enhanced - MAIN ENTRY]
        E[Health Monitor]
        F[Circuit Breaker]
        G[Performance Tracker]
        H[Fallback Manager]
        I[Training Notifier]
    end
    
    %% FANN Layer (Internal)
    subgraph "FANN Integration (Internal)"
        J[FannPredictor::predict]
        K[Model Router - ALL to FANN]
        L[ruv-FANN Networks]
    end
    
    %% Performance & Training
    subgraph "Monitoring & Feedback"
        M[Performance Channel]
        N[Training Notification Channel]
        O[Health Metrics]
        P[Performance Analytics]
    end
    
    %% Connections
    A --> B
    A --> C
    B --> D
    C --> D
    
    D --> E
    D --> F
    D --> G
    
    E --> O
    F --> H
    
    D --> J
    J --> K
    K --> L
    
    G --> M
    G --> I
    I --> N
    
    M --> P
    
    %% Styling
    style D fill:#4CAF50,stroke:#333,stroke-width:4px
    style L fill:#2196F3,stroke:#333,stroke-width:3px
    style M fill:#FF9800,stroke:#333,stroke-width:2px
    style N fill:#9C27B0,stroke:#333,stroke-width:2px
```

## Data Flow Sequence (Simplified)

```mermaid
sequenceDiagram
    participant Client
    participant NeuralPredictor
    participant Enhanced as EnhancedAdapter
    participant Health as Health Monitor
    participant Perf as Performance
    participant FANN as FannPredictor
    participant Network as ruv-FANN
    participant Training as Training System
    
    Client->>NeuralPredictor: predict(data, horizon)
    NeuralPredictor->>Enhanced: predict_enhanced()
    
    Note over Enhanced: Production Features
    Enhanced->>Health: check_system_health()
    Health-->>Enhanced: OK/Degraded
    
    alt Circuit Open
        Enhanced->>Enhanced: execute_fallback()
    else Circuit Closed
        Enhanced->>FANN: predict()
        Note over FANN: All models route here
        
        FANN->>Network: run(input_vector)
        Note over Network: Real Neural Network
        Network-->>FANN: output
        FANN-->>Enhanced: PredictionResult
    end
    
    Enhanced->>Perf: emit_performance_event()
    Perf->>Perf: calculate_metrics()
    
    alt Low Performance
        Perf->>Training: notify(metrics)
        Note over Training: Trigger retraining
    end
    
    Enhanced-->>Client: Vec<PredictionResult>
```

## Model Routing (All Through FANN)

```mermaid
graph LR
    subgraph "Model Types"
        MLP[MLP]
        LSTM[LSTM]
        GRU[GRU]
        TRANS[Transformer]
        DEEPAR[DeepAR]
        TCN[TCN]
    end
    
    subgraph "Single Route"
        FANN[FannPredictor]
    end
    
    subgraph "ruv-FANN Networks"
        NET[Neural Network Execution]
    end
    
    MLP --> FANN
    LSTM --> FANN
    GRU --> FANN
    TRANS --> FANN
    DEEPAR --> FANN
    TCN --> FANN
    
    FANN --> NET
    
    style FANN fill:#4CAF50,stroke:#333,stroke-width:3px
    style NET fill:#2196F3,stroke:#333,stroke-width:3px
```

## Performance Event Flow with Training Integration

```mermaid
graph TD
    A[Prediction Start] --> B[Enhanced Adapter]
    B --> C[Start Timer]
    C --> D[Health Check]
    D --> E{Healthy?}
    
    E -->|Yes| F[Execute FANN Prediction]
    E -->|No| G[Execute Fallback]
    
    F --> H[Calculate Metrics]
    G --> H
    
    H --> I[Build Performance Event]
    I --> J[Emit to Channel]
    
    J --> K[Performance Analytics]
    J --> L{Performance OK?}
    
    L -->|Yes| M[Store Metrics]
    L -->|No| N[Training Notification]
    
    N --> O[Training System]
    O --> P[Schedule Retraining]
    
    style J fill:#FF9800,stroke:#333,stroke-width:2px
    style N fill:#9C27B0,stroke:#333,stroke-width:2px
    style O fill:#E91E63,stroke:#333,stroke-width:2px
```

## Training Notification Triggers

```mermaid
stateDiagram-v2
    [*] --> Monitoring: Continuous
    
    Monitoring --> CheckAccuracy: Each Prediction
    CheckAccuracy --> LowAccuracy: accuracy < threshold
    CheckAccuracy --> CheckConfidence: accuracy OK
    
    CheckConfidence --> LowConfidence: confidence < threshold
    CheckConfidence --> CheckErrors: confidence OK
    
    CheckErrors --> HighErrors: error_rate > threshold
    CheckErrors --> Normal: errors OK
    
    LowAccuracy --> NotifyTraining
    LowConfidence --> NotifyTraining
    HighErrors --> NotifyTraining
    
    NotifyTraining --> TrainingSystem: Send Notification
    TrainingSystem --> ScheduleRetrain
    
    Normal --> [*]
    ScheduleRetrain --> [*]
```

## Component Responsibilities

| Component | Responsibility | Key Features |
|-----------|---------------|--------------|
| NeuralPredictor | Public API | Simple interface, delegates to Enhanced |
| EnhancedNeuralAdapter | Main Implementation | Health, fallbacks, performance, routing |
| FannPredictor | FANN Integration | Network management, execution |
| Health Monitor | System Health | Circuit breakers, degradation detection |
| Performance Tracker | Metrics Collection | Latency, accuracy, confidence tracking |
| Training Notifier | Training Integration | Threshold monitoring, notifications |
| Fallback Manager | Resilience | Multiple strategies, graceful degradation |

## Key Design Improvements

### 1. **Single Implementation Path**
- No more enhanced vs FANN confusion
- All models route through same path
- Clear, simple architecture

### 2. **Integrated Features**
- Health monitoring built-in
- Performance tracking automatic
- Training notifications direct
- Fallbacks always available

### 3. **Simplified Configuration**
- No use_real_models flag
- Single configuration object
- Consistent behavior

### 4. **Better Observability**
- Every prediction tracked
- Performance metrics aggregated
- Training feedback loop
- Health status visible

## Migration Benefits

### Before (Complex)
```
Client → NeuralPredictor → Route Decision
         ├→ Enhanced? → EnhancedAdapter → FannPredictor → FANN
         └→ FANN only? → FannPredictor → FANN
```

### After (Simple)
```
Client → NeuralPredictor → EnhancedAdapter → FannPredictor → FANN
         (thin wrapper)     (all features)     (FANN mgmt)    (execution)
```

## Performance Characteristics

- **Latency**: ~10-50ms per prediction
- **Overhead**: <1ms for routing
- **Memory**: Efficient with Arc sharing
- **Throughput**: 1000+ predictions/sec
- **Training Notifications**: <1ms async

## Success Metrics

1. **Code Reduction**: ~40% less routing code
2. **Test Coverage**: 85%+ on new paths
3. **Performance**: No regression
4. **Features**: All production features active
5. **Reliability**: Circuit breakers functional
6. **Observability**: 100% prediction tracking