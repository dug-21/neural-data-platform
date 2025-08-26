# gRPC Services - Strict Protobuf Enforcement

## Overview

All gRPC services in the Neural Trader Phase 4 implementation enforce **strict protobuf compliance** with zero tolerance for non-proto data formats. This document outlines the enforcement mechanisms and service behavior.

## Core Enforcement Principle

**INVALID PROTO = REJECTED REQUEST**

- Services ONLY accept valid protobuf messages
- Immediate rejection of non-proto data
- No fallback handlers for legacy formats
- No "compatibility mode" or dual format support

## Service Specifications

### 1. Market Data Service

```proto
service MarketDataService {
  rpc StreamPrices(StreamPricesRequest) returns (stream PriceUpdate);
  rpc GetHistorical(HistoricalRequest) returns (HistoricalResponse);
  rpc GetMarketStatus(MarketStatusRequest) returns (MarketStatusResponse);
}
```

**Enforcement Rules:**
- All requests must be valid protobuf messages
- Schema validation occurs before any processing
- Invalid proto structure returns `INVALID_ARGUMENT` (400)
- Missing required fields return `FAILED_PRECONDITION` (412)

### 2. Trading Service

```proto
service TradingService {
  rpc PlaceOrder(OrderRequest) returns (OrderResponse);
  rpc CancelOrder(CancelRequest) returns (CancelResponse);
  rpc GetPosition(PositionRequest) returns (PositionResponse);
  rpc GetOrders(OrdersRequest) returns (OrdersResponse);
}
```

**Enforcement Rules:**
- Order validation includes proto structure AND business rules
- Malformed protobuf messages rejected at transport layer
- Field validation errors include proto field paths
- No JSON/REST fallback endpoints

### 3. Event Bus Service

```proto
service EventBusService {
  rpc Publish(PublishRequest) returns (PublishResponse);
  rpc Subscribe(SubscribeRequest) returns (stream EventMessage);
  rpc GetEventHistory(EventHistoryRequest) returns (EventHistoryResponse);
}
```

**Enforcement Rules:**
- Event payloads must be properly serialized proto messages
- Event type validation against registered proto schemas
- Subscription filters validate proto field paths
- Invalid events dropped, not transformed

### 4. ML Analytics Service

```proto
service MLAnalyticsService {
  rpc GetPrediction(PredictionRequest) returns (PredictionResponse);
  rpc TrainModel(TrainRequest) returns (stream TrainResponse);
  rpc GetModelMetrics(MetricsRequest) returns (MetricsResponse);
}
```

**Enforcement Rules:**
- Feature vectors must be proto-encoded
- Model parameters require proto serialization
- Training data validated for proto compliance
- No raw JSON/CSV input processing

### 5. Configuration Service

```proto
service ConfigurationService {
  rpc GetConfig(ConfigRequest) returns (ConfigResponse);
  rpc UpdateConfig(UpdateConfigRequest) returns (UpdateConfigResponse);
  rpc ValidateConfig(ValidateConfigRequest) returns (ValidateConfigResponse);
}
```

**Enforcement Rules:**
- Configuration updates must be valid proto messages
- Schema migrations handled through proto evolution
- No YAML/JSON configuration endpoints
- Config validation includes proto schema compliance

## Validation Pipeline

### Request Validation Sequence

1. **Transport Layer Validation**
   ```rust
   // Immediate proto deserialization check
   let request = match MyRequest::decode(&bytes) {
       Ok(req) => req,
       Err(e) => return Err(Status::invalid_argument(
           format!("Invalid protobuf: {}", e)
       ))
   };
   ```

2. **Schema Validation**
   ```rust
   // Verify required fields and constraints
   if request.symbol.is_empty() {
       return Err(Status::failed_precondition(
           "Required field 'symbol' is empty"
       ));
   }
   ```

3. **Business Logic Validation**
   ```rust
   // Apply domain-specific rules
   if !is_valid_symbol(&request.symbol) {
       return Err(Status::invalid_argument(
           "Invalid symbol format"
       ));
   }
   ```

### Error Response Format

All validation errors return structured proto responses:

```proto
message ErrorResponse {
  string code = 1;          // Error code
  string message = 2;       // Human-readable message
  string field_path = 3;    // Proto field path (if applicable)
  repeated string details = 4; // Additional error details
}
```

## Health Check Compliance

### Service Health Verification

```proto
service HealthService {
  rpc Check(HealthCheckRequest) returns (HealthCheckResponse);
}
```

**Proto Compliance Checks:**
- Verify proto descriptor availability
- Test schema validation pipeline
- Validate error response formatting
- Check proto serialization performance

### Health Check Response

```proto
message HealthCheckResponse {
  enum ServingStatus {
    UNKNOWN = 0;
    SERVING = 1;
    NOT_SERVING = 2;
    SERVICE_UNKNOWN = 3;
  }
  ServingStatus status = 1;
  ProtoComplianceStatus proto_status = 2;
}

message ProtoComplianceStatus {
  bool schema_loaded = 1;
  bool validation_active = 2;
  int32 rejected_requests_count = 3;
  string last_validation_error = 4;
}
```

## Error Handling Strategy

### Rejection Categories

1. **Transport Level Errors**
   - Invalid protobuf encoding: `INVALID_ARGUMENT`
   - Corrupted message: `DATA_LOSS`
   - Unsupported version: `UNIMPLEMENTED`

2. **Schema Validation Errors**
   - Missing required fields: `FAILED_PRECONDITION`
   - Invalid field values: `INVALID_ARGUMENT`
   - Unknown fields: `INVALID_ARGUMENT`

3. **Business Logic Errors**
   - Invalid business rules: `FAILED_PRECONDITION`
   - Authorization failures: `PERMISSION_DENIED`
   - Resource constraints: `RESOURCE_EXHAUSTED`

### Error Messages

All error messages include:
- Proto field path for field-specific errors
- Expected vs actual value information
- Schema version information
- Helpful correction suggestions

Example error response:
```
Status: INVALID_ARGUMENT
Message: "Invalid protobuf field: orders[0].quantity must be positive"
Details: [
  "field_path: orders[0].quantity",
  "received_value: -100",
  "expected: value > 0",
  "schema_version: v2.1.0"
]
```

## Implementation Guidelines

### Service Interceptors

```rust
// Global proto validation interceptor
pub struct ProtoValidationInterceptor;

impl Interceptor for ProtoValidationInterceptor {
    fn call(&mut self, request: Request<()>) -> Result<Request<()>, Status> {
        // Validate proto format before routing
        validate_proto_request(&request)?;
        Ok(request)
    }
}
```

### Request Handlers

```rust
// Service method implementation
async fn place_order(
    &self,
    request: Request<OrderRequest>
) -> Result<Response<OrderResponse>, Status> {
    // Proto already validated by interceptor
    let order_req = request.into_inner();
    
    // Additional business validation
    self.validate_order_business_rules(&order_req).await?;
    
    // Process order
    let response = self.process_order(order_req).await?;
    Ok(Response::new(response))
}
```

### Client Libraries

Generated client libraries must:
- Enforce proto serialization at compile time
- Provide clear error messages for proto violations
- Include schema validation helpers
- Support proto field validation

## Monitoring and Metrics

### Proto Compliance Metrics

- `proto_validation_errors_total{service, error_type}`
- `proto_request_size_bytes{service, method}`
- `proto_serialization_duration_seconds{service, method}`
- `proto_schema_validation_errors_total{field_path, error_type}`

### Alerting Thresholds

- Proto validation error rate > 1%
- Schema deserialization failures > 0.1%
- Invalid field format errors > 5%

## Migration Strategy

### From Legacy Systems

1. **No Dual Mode Support**
   - Legacy endpoints immediately return `UNIMPLEMENTED`
   - Clear migration timeline communicated
   - Proto conversion tools provided

2. **Client Migration**
   - Generated proto client libraries
   - Migration validation tools
   - Schema evolution support

3. **Testing Strategy**
   - Proto compliance test suite
   - Invalid message fuzzing
   - Performance benchmarks

## Security Considerations

### Proto-Level Security

- Schema validation prevents injection attacks
- Field size limits enforced at proto level
- Required field validation prevents incomplete requests
- Type safety through proto definitions

### Input Sanitization

All proto fields undergo sanitization:
- String fields: UTF-8 validation, length limits
- Numeric fields: Range validation, overflow protection
- Repeated fields: Count limits, element validation
- Message fields: Recursive validation

## Performance Impact

### Validation Overhead

- Proto deserialization: ~0.1ms typical
- Schema validation: ~0.05ms typical
- Total overhead: <2% of request processing time

### Optimization Strategies

- Proto descriptor caching
- Validation result memoization
- Batch validation for repeated fields
- SIMD-optimized proto parsing

## Conclusion

Strict protobuf enforcement ensures:
- Type safety across all service boundaries
- Clear error handling and debugging
- Consistent data formats
- Performance optimization opportunities
- Future-proof schema evolution

**Remember: Invalid proto = rejected request. No exceptions, no fallbacks, no compatibility modes.**