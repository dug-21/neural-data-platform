# Interface Contracts for Domain Deployment

## Standard gRPC Interface Definitions

### 1. Data Ingestion Service Interface

```protobuf
syntax = "proto3";

package neural_platform.data_ingestion;

service DataIngestionService {
  // Register a new data source with the platform
  rpc RegisterSource(SourceConfig) returns (RegistrationResponse);
  
  // Stream data points to the platform
  rpc StreamData(stream DataPoint) returns (StreamResponse);
  
  // Get schema definition for validation
  rpc GetSchema(SchemaRequest) returns (SchemaDefinition);
  
  // Health check for the service
  rpc HealthCheck(Empty) returns (HealthStatus);
}

message SourceConfig {
  string domain = 1;                    // e.g., "trading"
  string source_id = 2;                 // e.g., "alpaca_market_data"
  string source_type = 3;               // e.g., "real_time", "historical"
  map<string, string> connection_params = 4;
  string schema_version = 5;
  repeated string symbols = 6;          // assets to track
}

message DataPoint {
  string domain = 1;                    // Must match registered domain
  string source = 2;                    // Must match registered source_id
  string symbol = 3;                    // Asset identifier
  int64 timestamp = 4;                  // Unix timestamp (microseconds)
  map<string, double> values = 5;       // Field name -> value mapping
  string schema_version = 6;            // For validation
  map<string, string> metadata = 7;    // Additional context
}

message RegistrationResponse {
  bool success = 1;
  string source_id = 2;
  string error_message = 3;
  repeated string required_fields = 4;
}

message StreamResponse {
  bool success = 1;
  int64 points_processed = 2;
  string error_message = 3;
}

message SchemaRequest {
  string domain = 1;
  string version = 2;
}

message SchemaDefinition {
  string domain = 1;
  string version = 2;
  repeated FieldDefinition fields = 3;
  map<string, string> validation_rules = 4;
}

message FieldDefinition {
  string name = 1;
  string type = 2;                      // "double", "string", "timestamp"
  bool required = 3;
  string description = 4;
  double min_value = 5;
  double max_value = 6;
}
```

### 2. Model Execution Service Interface

```protobuf
syntax = "proto3";

package neural_platform.model_execution;

service ModelExecutionService {
  // Load a model for inference
  rpc LoadModel(ModelConfig) returns (LoadResponse);
  
  // Make predictions using loaded model
  rpc Predict(PredictionRequest) returns (PredictionResponse);
  
  // Get model performance metrics
  rpc GetMetrics(MetricsRequest) returns (ModelMetrics);
  
  // Unload model to free resources
  rpc UnloadModel(UnloadRequest) returns (UnloadResponse);
  
  // Health check for the service
  rpc HealthCheck(Empty) returns (HealthStatus);
}

message ModelConfig {
  string domain = 1;                    // e.g., "trading"
  string model_id = 2;                  // Unique model identifier
  string model_path = 3;                // Path to model file
  string model_type = 4;                // e.g., "mlp", "lstm", "transformer"
  map<string, string> parameters = 5;   // Model-specific config
  int32 max_batch_size = 6;
  int32 timeout_ms = 7;
}

message PredictionRequest {
  string domain = 1;                    // Must match loaded model domain
  string model_id = 2;                  // Must match loaded model
  repeated double features = 3;         // Input feature vector
  map<string, string> metadata = 4;    // Request context
  int64 timestamp = 5;                  // Request timestamp
}

message PredictionResponse {
  bool success = 1;
  repeated double predictions = 2;      // Model output vector
  double confidence = 3;                // Confidence score [0-1]
  int64 inference_time_us = 4;          // Inference latency
  string model_version = 5;
  string error_message = 6;
  map<string, double> debug_info = 7;   // Additional diagnostics
}

message LoadResponse {
  bool success = 1;
  string model_id = 2;
  string model_version = 3;
  string error_message = 4;
  int64 load_time_ms = 5;
}

message MetricsRequest {
  string domain = 1;
  string model_id = 2;
  int64 time_range_start = 3;           // Unix timestamp
  int64 time_range_end = 4;
}

message ModelMetrics {
  string model_id = 1;
  int64 total_predictions = 2;
  double avg_inference_time_us = 3;
  double avg_confidence = 4;
  int64 errors = 5;
  double accuracy = 6;                  // If ground truth available
  map<string, double> custom_metrics = 7;
}
```

### 3. Action Execution Service Interface

```protobuf
syntax = "proto3";

package neural_platform.action_execution;

service ActionExecutionService {
  // Execute an action based on model prediction
  rpc ExecuteAction(ActionRequest) returns (ActionResponse);
  
  // Get available actions for this domain
  rpc GetCapabilities(CapabilityRequest) returns (CapabilityResponse);
  
  // Validate action before execution
  rpc ValidateAction(ValidationRequest) returns (ValidationResponse);
  
  // Get action execution status
  rpc GetActionStatus(StatusRequest) returns (ActionStatus);
  
  // Health check for the service
  rpc HealthCheck(Empty) returns (HealthStatus);
}

message ActionRequest {
  string domain = 1;                    // e.g., "trading"
  string action_type = 2;               // e.g., "buy", "sell", "hold"
  map<string, double> parameters = 3;   // Action-specific parameters
  double confidence = 4;                // Model confidence [0-1]
  string symbol = 5;                    // Asset identifier
  int64 timestamp = 6;                  // Action timestamp
  string request_id = 7;                // For tracking
  map<string, string> metadata = 8;
}

message ActionResponse {
  bool success = 1;
  string action_id = 2;                 // For tracking execution
  string status = 3;                    // "queued", "executing", "completed", "failed"
  string error_message = 4;
  int64 execution_time_ms = 5;
  map<string, string> result_data = 6;  // Action-specific results
}

message CapabilityRequest {
  string domain = 1;
}

message CapabilityResponse {
  repeated ActionCapability capabilities = 1;
}

message ActionCapability {
  string action_type = 1;
  string description = 2;
  repeated ParameterSpec required_parameters = 3;
  repeated ParameterSpec optional_parameters = 4;
  double min_confidence = 5;            // Minimum confidence required
}

message ParameterSpec {
  string name = 1;
  string type = 2;                      // "double", "string", "int"
  string description = 3;
  bool required = 4;
  double min_value = 5;
  double max_value = 6;
}

message ValidationRequest {
  ActionRequest action = 1;
  bool dry_run = 2;                     // Don't execute, just validate
}

message ValidationResponse {
  bool valid = 1;
  repeated string errors = 2;
  repeated string warnings = 3;
  double estimated_cost = 4;            // If applicable
  double risk_score = 5;                // Risk assessment [0-1]
}

message StatusRequest {
  string action_id = 1;
}

message ActionStatus {
  string action_id = 1;
  string status = 2;
  double progress = 3;                  // Completion percentage [0-1]
  string current_step = 4;
  map<string, string> status_data = 5;
  int64 last_updated = 6;
}
```

### 4. Common Types

```protobuf
syntax = "proto3";

package neural_platform.common;

message Empty {}

message HealthStatus {
  string status = 1;                    // "healthy", "degraded", "unhealthy"
  string version = 2;
  int64 uptime_seconds = 3;
  map<string, string> details = 4;
}
```

## Implementation Requirements

### Data Ingestion Service Implementation

**Trading Domain Must:**
1. Implement `DataIngestionService` interface
2. Register "trading" domain with required schemas
3. Stream market data in standardized `DataPoint` format
4. Handle connection failures gracefully
5. Report metrics to shared monitoring platform

**Interface Validation:**
- All `DataPoint` messages must have valid schema_version
- `timestamp` must be within reasonable bounds (not future, not too old)
- `values` must contain all required fields per schema
- `symbol` must be in registered symbols list

### Model Execution Service Implementation

**Trading Domain Must:**
1. Implement `ModelExecutionService` interface
2. Load models from shared ML Ops Platform
3. Accept standardized feature vectors
4. Return predictions with confidence scores
5. Report performance metrics

**Interface Validation:**
- `features` array length must match model input size
- `model_id` must be loaded before prediction requests
- `confidence` score must be between 0 and 1
- `inference_time_us` must be tracked accurately

### Action Execution Service Implementation

**Trading Domain Must:**
1. Implement `ActionExecutionService` interface
2. Support standard action types: "buy", "sell", "hold"
3. Validate actions against risk constraints
4. Execute through domain-specific brokers
5. Audit all actions to shared storage

**Interface Validation:**
- `action_type` must be in declared capabilities
- `parameters` must include all required fields
- `confidence` must meet minimum threshold
- Actions must pass risk validation before execution

## Domain Registry Integration

### Registration Process

1. **Service Startup:**
   ```
   Domain Service -> Domain Registry: RegisterDomain
   Domain Registry -> Domain Service: DomainConfig
   Domain Service -> Shared Services: Connect using config
   ```

2. **Schema Management:**
   ```
   Domain Service -> Domain Registry: RegisterSchema
   Domain Registry -> EventBus: Update schema registry
   Other Services -> Domain Registry: GetSchema for validation
   ```

3. **Service Discovery:**
   ```
   Client -> Domain Registry: DiscoverServices
   Domain Registry -> Client: Service endpoints + health status
   Client -> Domain Service: Direct gRPC calls
   ```

## Interface Compliance Testing

### Automated Validation

Each domain service MUST pass:

1. **Contract Tests**: gRPC interface compliance
2. **Schema Tests**: Data format validation  
3. **Performance Tests**: Latency and throughput requirements
4. **Integration Tests**: End-to-end workflow validation
5. **Failure Tests**: Error handling and recovery

### Continuous Monitoring

- Interface response times tracked
- Error rates monitored per interface method
- Schema validation failures logged
- Performance degradation alerts

This interface design ensures clean separation between generic platform services and domain-specific implementations while maintaining standardized contracts for interoperability.