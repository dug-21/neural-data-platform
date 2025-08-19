# Production Interface Contracts
## Layer-by-Layer Interface Specifications

### Overview

This document defines **production-grade interface contracts** between all architectural layers. Every interface is designed for reliability, observability, and maintainability from Day 1 - **no shortcuts or temporary solutions**.

## Core Interface Principles

### 1. Contract-First Design
- Interfaces defined before implementation
- Comprehensive error handling
- Full observability integration
- Version compatibility guarantees

### 2. Failure Mode Handling
- Graceful degradation patterns
- Circuit breaker integration
- Timeout and retry specifications
- Dead letter queue handling

### 3. Performance SLAs
- Latency requirements per operation
- Throughput guarantees
- Resource utilization limits
- Backpressure mechanisms

---

## Layer 1: Data Ingestion → Event Bus Contract

### Interface Definition

```rust
#[async_trait]
pub trait DataPlatformInterface {
    /// Publish validated event to event bus
    /// SLA: <10ms p95 latency, >99.9% success rate
    async fn publish_event(
        &self,
        event: ValidatedEvent,
        schema_version: SchemaVersion,
        routing_config: RoutingConfig,
        options: PublishOptions
    ) -> Result<PublishConfirmation, PublishError>;
    
    /// Batch publish for high-throughput scenarios  
    /// SLA: <50ms for 1000 events, >99.95% success rate
    async fn publish_batch(
        &self,
        events: Vec<ValidatedEvent>,
        batch_options: BatchOptions
    ) -> Result<BatchResult, PublishError>;
    
    /// Register/update schema with compatibility checking
    /// SLA: <100ms, schema compatibility verified
    async fn register_schema(
        &self,
        schema: Schema,
        compatibility: CompatibilityLevel
    ) -> Result<SchemaVersion, SchemaError>;
    
    /// Get current schema for validation
    /// SLA: <5ms (cached), <50ms (database)
    async fn get_schema(
        &self,
        event_type: &str,
        version: Option<SchemaVersion>
    ) -> Result<Schema, SchemaError>;
    
    /// Create/configure event stream
    /// SLA: <1s creation, idempotent operation
    async fn configure_stream(
        &self,
        stream_config: StreamConfig
    ) -> Result<StreamId, StreamError>;
    
    /// Health check with detailed status
    /// SLA: <10ms, comprehensive health data
    async fn healthcheck(&self) -> HealthStatus;
    
    /// Get detailed interface metrics
    /// SLA: <20ms, real-time metrics
    async fn get_metrics(
        &self,
        time_range: TimeRange
    ) -> Result<InterfaceMetrics, MetricsError>;
}
```

### Data Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedEvent {
    pub id: EventId,
    pub timestamp: DateTime<Utc>,
    pub domain: String,
    pub source: String,
    pub event_type: String,
    pub schema_version: SchemaVersion,
    pub payload: serde_json::Value,
    pub metadata: EventMetadata,
    pub correlation_id: CorrelationId,
}

#[derive(Debug, Clone)]
pub struct PublishOptions {
    pub priority: Priority,          // High, Normal, Low
    pub durability: Durability,      // Persistent, Memory
    pub timeout: Duration,           // Max wait time
    pub retry_policy: RetryPolicy,   // Exponential backoff config
}

#[derive(Debug, Clone)]
pub struct PublishConfirmation {
    pub event_id: EventId,
    pub stream_position: StreamPosition,
    pub timestamp: DateTime<Utc>,
    pub latency: Duration,
}

#[derive(Debug, Error)]
pub enum PublishError {
    #[error("Schema validation failed: {details}")]
    SchemaValidation { 
        event_id: EventId,
        schema_version: SchemaVersion,
        details: String,
    },
    
    #[error("Stream not available: {stream_id}")]
    StreamUnavailable { 
        stream_id: String,
        retry_after: Option<Duration>,
    },
    
    #[error("Rate limit exceeded: {current}/{limit} rps")]
    RateLimitExceeded { 
        current: u32, 
        limit: u32,
        reset_time: DateTime<Utc>,
    },
    
    #[error("Event too large: {size}/{max_size} bytes")]
    EventTooLarge { 
        size: usize, 
        max_size: usize,
    },
    
    #[error("Downstream service timeout: {service}")]
    ServiceTimeout { 
        service: String,
        timeout: Duration,
    },
    
    #[error("Internal service error: {code}")]
    InternalError { 
        code: ErrorCode,
        message: String,
        retry_after: Option<Duration>,
    },
}
```

### Routing Configuration

```rust
#[derive(Debug, Clone)]
pub struct RoutingConfig {
    pub stream_pattern: StreamPattern,
    pub partition_key: Option<String>,
    pub tags: HashMap<String, String>,
    pub expiry: Option<Duration>,
}

#[derive(Debug, Clone)]  
pub enum StreamPattern {
    /// data.{domain}.{source}.{event_type}
    Standard { domain: String, source: String, event_type: String },
    /// Custom stream name
    Custom(String),
    /// Computed from event content
    Dynamic(Box<dyn Fn(&ValidatedEvent) -> String>),
}
```

### Health and Metrics

```rust
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub status: ServiceStatus,
    pub timestamp: DateTime<Utc>,
    pub components: HashMap<String, ComponentHealth>,
    pub dependencies: Vec<DependencyHealth>,
}

#[derive(Debug, Clone)]
pub enum ServiceStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
    Unknown,
}

#[derive(Debug, Clone)]
pub struct InterfaceMetrics {
    pub requests_per_second: f64,
    pub success_rate: f64,
    pub error_rate: f64,
    pub latency_percentiles: LatencyMetrics,
    pub queue_depth: u64,
    pub throughput_bytes_per_second: u64,
    pub schema_cache_hit_rate: f64,
}
```

---

## Layer 2: Event Bus → ML Ops Contract

### Interface Definition

```rust
#[async_trait]
pub trait MLOpsInterface {
    /// Retrieve features for model input
    /// SLA: <10ms p95 (cached), <100ms p95 (computed)
    async fn get_features(
        &self,
        request: FeatureRequest
    ) -> Result<FeatureResponse, FeatureError>;
    
    /// Batch feature retrieval for efficiency
    /// SLA: <50ms for 100 entities
    async fn get_features_batch(
        &self,
        requests: Vec<FeatureRequest>
    ) -> Result<Vec<FeatureResponse>, FeatureError>;
    
    /// Execute model prediction
    /// SLA: <50ms p95, >99.9% availability
    async fn predict(
        &self,
        model_id: ModelId,
        features: FeatureVector,
        options: PredictionOptions
    ) -> Result<Prediction, PredictionError>;
    
    /// Batch prediction for throughput
    /// SLA: <100ms for 1000 predictions
    async fn predict_batch(
        &self,
        requests: Vec<PredictionRequest>
    ) -> Result<Vec<Prediction>, PredictionError>;
    
    /// Deploy new model version
    /// SLA: <30s deployment, zero-downtime
    async fn deploy_model(
        &self,
        deployment: ModelDeployment
    ) -> Result<DeploymentResult, DeploymentError>;
    
    /// A/B test model versions
    /// SLA: Traffic split within 1s
    async fn create_ab_test(
        &self,
        test_config: ABTestConfig
    ) -> Result<ABTestId, ABTestError>;
    
    /// Get model performance metrics
    /// SLA: <100ms, real-time metrics
    async fn get_model_performance(
        &self,
        model_id: ModelId,
        time_range: TimeRange,
        metrics: Vec<MetricType>
    ) -> Result<PerformanceReport, MetricsError>;
    
    /// Rollback model to previous version
    /// SLA: <10s rollback time
    async fn rollback_model(
        &self,
        model_id: ModelId,
        target_version: ModelVersion,
        rollback_strategy: RollbackStrategy
    ) -> Result<RollbackResult, RollbackError>;
}
```

### Feature Serving

```rust
#[derive(Debug, Clone)]
pub struct FeatureRequest {
    pub entity_id: String,
    pub feature_group: String,
    pub feature_names: Vec<String>,
    pub version: Option<FeatureVersion>,
    pub point_in_time: Option<DateTime<Utc>>,
    pub fallback_strategy: FallbackStrategy,
}

#[derive(Debug, Clone)]
pub struct FeatureResponse {
    pub entity_id: String,
    pub features: HashMap<String, FeatureValue>,
    pub computed_at: DateTime<Utc>,
    pub cache_hit: bool,
    pub quality_score: f64,
}

#[derive(Debug, Clone)]
pub enum FallbackStrategy {
    /// Return error if features unavailable
    Strict,
    /// Use default values for missing features
    UseDefaults(HashMap<String, FeatureValue>),
    /// Use cached values (with staleness tolerance)
    UseCached { max_staleness: Duration },
}
```

### Model Execution

```rust
#[derive(Debug, Clone)]
pub struct PredictionRequest {
    pub model_id: ModelId,
    pub features: FeatureVector,
    pub prediction_id: PredictionId,
    pub context: PredictionContext,
}

#[derive(Debug, Clone)]
pub struct Prediction {
    pub prediction_id: PredictionId,
    pub model_id: ModelId,
    pub model_version: ModelVersion,
    pub result: PredictionResult,
    pub confidence: f64,
    pub features_used: Vec<String>,
    pub latency: Duration,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum PredictionResult {
    Regression(f64),
    Classification { class: String, probabilities: HashMap<String, f64> },
    Ranking(Vec<RankingItem>),
    Embedding(Vec<f64>),
}

#[derive(Debug, Clone)]
pub struct PredictionOptions {
    pub explain: bool,              // Include prediction explanation
    pub uncertainty: bool,          // Include uncertainty estimates  
    pub feature_importance: bool,   // Include feature importance scores
    pub timeout: Duration,          // Maximum wait time
}
```

### Model Management

```rust
#[derive(Debug, Clone)]
pub struct ModelDeployment {
    pub model_id: ModelId,
    pub model_version: ModelVersion,
    pub deployment_strategy: DeploymentStrategy,
    pub resource_requirements: ResourceRequirements,
    pub health_check_config: HealthCheckConfig,
}

#[derive(Debug, Clone)]
pub enum DeploymentStrategy {
    /// Replace all instances immediately
    Immediate,
    /// Gradual rollout with traffic shifting
    Rolling { 
        max_unavailable: u32,
        traffic_split_steps: Vec<u32>,
    },
    /// Blue-green deployment
    BlueGreen { 
        validation_duration: Duration,
    },
    /// Canary deployment
    Canary { 
        canary_percentage: u32,
        success_threshold: f64,
        monitoring_duration: Duration,
    },
}
```

---

## Layer 3: ML Ops → Model Execution Contract

### Interface Definition

```rust
#[async_trait]
pub trait ModelExecutionInterface {
    /// Make decision based on model predictions
    /// SLA: <100ms p95, includes risk validation
    async fn make_decision(
        &self,
        decision_request: DecisionRequest
    ) -> Result<Decision, DecisionError>;
    
    /// Batch decision making
    /// SLA: <500ms for 100 decisions
    async fn make_decisions_batch(
        &self,
        requests: Vec<DecisionRequest>
    ) -> Result<Vec<Decision>, DecisionError>;
    
    /// Execute consensus decision across multiple models
    /// SLA: <200ms p95, Byzantine fault tolerance
    async fn consensus_decision(
        &self,
        consensus_request: ConsensusRequest
    ) -> Result<ConsensusDecision, ConsensusError>;
    
    /// Explain decision reasoning
    /// SLA: <50ms, human-readable explanation
    async fn explain_decision(
        &self,
        decision_id: DecisionId
    ) -> Result<DecisionExplanation, ExplanationError>;
    
    /// Validate decision against constraints
    /// SLA: <20ms, comprehensive validation
    async fn validate_decision(
        &self,
        decision: &Decision,
        constraints: &[Constraint]
    ) -> Result<ValidationResult, ValidationError>;
}
```

### Decision Making

```rust
#[derive(Debug, Clone)]
pub struct DecisionRequest {
    pub request_id: RequestId,
    pub context: DecisionContext,
    pub models: Vec<ModelSpec>,
    pub constraints: Vec<Constraint>,
    pub options: DecisionOptions,
}

#[derive(Debug, Clone)]
pub struct DecisionContext {
    pub domain: String,
    pub entity_id: String,
    pub timestamp: DateTime<Utc>,
    pub features: FeatureVector,
    pub external_signals: HashMap<String, serde_json::Value>,
    pub risk_parameters: RiskParameters,
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub decision_id: DecisionId,
    pub request_id: RequestId,
    pub action: ActionType,
    pub confidence: f64,
    pub reasoning: String,
    pub model_votes: Vec<ModelVote>,
    pub risk_score: f64,
    pub constraints_satisfied: bool,
    pub timestamp: DateTime<Utc>,
    pub expiry: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ModelVote {
    pub model_id: ModelId,
    pub model_version: ModelVersion,
    pub vote: ActionType,
    pub confidence: f64,
    pub weight: f64,
    pub features_used: Vec<String>,
}
```

### Consensus Mechanism

```rust
#[derive(Debug, Clone)]
pub struct ConsensusRequest {
    pub request_id: RequestId,
    pub participating_models: Vec<ModelId>,
    pub consensus_algorithm: ConsensusAlgorithm,
    pub minimum_agreement: f64,
    pub timeout: Duration,
    pub context: DecisionContext,
}

#[derive(Debug, Clone)]
pub enum ConsensusAlgorithm {
    /// Simple majority voting
    Majority,
    /// Weighted voting by model performance
    Weighted { weights: HashMap<ModelId, f64> },
    /// Byzantine fault tolerance
    Byzantine { fault_tolerance: u32 },
    /// Confidence-weighted consensus
    ConfidenceWeighted { min_confidence: f64 },
}

#[derive(Debug, Clone)]
pub struct ConsensusDecision {
    pub consensus_id: ConsensusId,
    pub final_decision: Decision,
    pub agreement_level: f64,
    pub participating_votes: Vec<ModelVote>,
    pub dissenting_votes: Vec<ModelVote>,
    pub consensus_reached: bool,
    pub algorithm_used: ConsensusAlgorithm,
}
```

---

## Layer 4: Model Execution → Action Layer Contract

### Interface Definition

```rust
#[async_trait]
pub trait ActionPlatformInterface {
    /// Validate proposed action against rules and constraints
    /// SLA: <50ms p95, comprehensive validation
    async fn validate_action(
        &self,
        action: ProposedAction,
        validation_context: ValidationContext
    ) -> Result<ValidationResult, ValidationError>;
    
    /// Execute validated action with confirmation
    /// SLA: <1s p95, guaranteed confirmation
    async fn execute_action(
        &self,
        validated_action: ValidatedAction,
        execution_options: ExecutionOptions
    ) -> Result<ExecutionResult, ExecutionError>;
    
    /// Execute multiple actions atomically
    /// SLA: <2s p95, all-or-nothing semantics
    async fn execute_atomic_batch(
        &self,
        actions: Vec<ValidatedAction>,
        batch_options: BatchExecutionOptions
    ) -> Result<BatchExecutionResult, ExecutionError>;
    
    /// Get real-time execution status
    /// SLA: <10ms, real-time status
    async fn get_execution_status(
        &self,
        execution_id: ExecutionId
    ) -> Result<ExecutionStatus, StatusError>;
    
    /// Cancel pending execution
    /// SLA: <100ms, best-effort cancellation
    async fn cancel_execution(
        &self,
        execution_id: ExecutionId,
        reason: String
    ) -> Result<CancellationResult, CancellationError>;
    
    /// Compensate/rollback completed action
    /// SLA: <5s, compensating transaction
    async fn compensate_action(
        &self,
        execution_id: ExecutionId,
        compensation_strategy: CompensationStrategy
    ) -> Result<CompensationResult, CompensationError>;
    
    /// Get comprehensive audit trail
    /// SLA: <200ms for 1000 records
    async fn get_audit_trail(
        &self,
        filters: AuditFilters,
        pagination: PaginationOptions
    ) -> Result<AuditTrail, AuditError>;
}
```

### Action Validation

```rust
#[derive(Debug, Clone)]
pub struct ProposedAction {
    pub action_id: ActionId,
    pub action_type: ActionType,
    pub parameters: HashMap<String, serde_json::Value>,
    pub requester: String,
    pub priority: ActionPriority,
    pub deadline: Option<DateTime<Utc>>,
    pub dependencies: Vec<ActionId>,
}

#[derive(Debug, Clone)]
pub struct ValidationContext {
    pub domain: String,
    pub current_state: SystemState,
    pub risk_limits: RiskLimits,
    pub compliance_rules: Vec<ComplianceRule>,
    pub market_conditions: MarketConditions,
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub action_id: ActionId,
    pub is_valid: bool,
    pub validation_score: f64,
    pub rule_results: Vec<RuleResult>,
    pub risk_assessment: RiskAssessment,
    pub required_approvals: Vec<Approval>,
    pub estimated_impact: ImpactEstimate,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RuleResult {
    pub rule_id: String,
    pub rule_name: String,
    pub passed: bool,
    pub score: f64,
    pub message: String,
    pub severity: Severity,
}
```

### Action Execution

```rust
#[derive(Debug, Clone)]
pub struct ValidatedAction {
    pub action: ProposedAction,
    pub validation: ValidationResult,
    pub approvals: Vec<Approval>,
    pub validated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub execution_id: ExecutionId,
    pub action_id: ActionId,
    pub status: ExecutionStatus,
    pub result: ActionResult,
    pub executed_at: DateTime<Utc>,
    pub execution_time: Duration,
    pub confirmation: ExecutionConfirmation,
    pub side_effects: Vec<SideEffect>,
}

#[derive(Debug, Clone)]
pub enum ExecutionStatus {
    Pending,
    InProgress { progress: f64 },
    Completed { success: bool },
    Failed { error: ExecutionError, retry_count: u32 },
    Cancelled { reason: String },
    Timeout,
}

#[derive(Debug, Clone)]
pub struct ExecutionConfirmation {
    pub confirmation_id: String,
    pub external_reference: Option<String>,
    pub confirmed_by: String,
    pub confirmation_data: HashMap<String, serde_json::Value>,
    pub timestamp: DateTime<Utc>,
}
```

### Compensation and Rollback

```rust
#[derive(Debug, Clone)]
pub enum CompensationStrategy {
    /// Reverse the exact action
    Reverse,
    /// Execute predefined compensation action
    Compensate { compensation_action: ProposedAction },
    /// Manual intervention required
    Manual { instructions: String },
    /// No compensation possible
    None { reason: String },
}

#[derive(Debug, Clone)]
pub struct CompensationResult {
    pub compensation_id: CompensationId,
    pub original_execution_id: ExecutionId,
    pub strategy_used: CompensationStrategy,
    pub compensation_status: ExecutionStatus,
    pub compensation_actions: Vec<ExecutionResult>,
    pub net_effect: NetEffect,
    pub timestamp: DateTime<Utc>,
}
```

### Audit and Compliance

```rust
#[derive(Debug, Clone)]
pub struct AuditTrail {
    pub records: Vec<AuditRecord>,
    pub total_count: u64,
    pub pagination: PaginationInfo,
    pub query_time: Duration,
}

#[derive(Debug, Clone)]
pub struct AuditRecord {
    pub record_id: String,
    pub timestamp: DateTime<Utc>,
    pub action_id: ActionId,
    pub execution_id: Option<ExecutionId>,
    pub event_type: AuditEventType,
    pub actor: String,
    pub details: HashMap<String, serde_json::Value>,
    pub before_state: Option<SystemState>,
    pub after_state: Option<SystemState>,
}

#[derive(Debug, Clone)]
pub enum AuditEventType {
    ActionProposed,
    ActionValidated,
    ActionApproved,
    ActionExecuted,
    ActionCompleted,
    ActionFailed,
    ActionCancelled,
    ActionCompensated,
    RuleViolation,
    SystemStateChange,
}
```

---

## Cross-Layer Concerns

### Error Handling Standards

```rust
pub trait PlatformError: std::error::Error + Send + Sync + 'static {
    /// Unique error code for logging/monitoring
    fn error_code(&self) -> ErrorCode;
    
    /// Whether this error is retryable
    fn is_retryable(&self) -> bool;
    
    /// Suggested retry delay
    fn retry_after(&self) -> Option<Duration>;
    
    /// Error severity for alerting
    fn severity(&self) -> Severity;
    
    /// Additional context for debugging
    fn context(&self) -> HashMap<String, String>;
}

#[derive(Debug, Clone, Copy)]
pub enum Severity {
    Critical,   // Page immediately
    High,       // Alert within 5 minutes  
    Medium,     // Alert within 1 hour
    Low,        // Log only
}
```

### Observability Standards

```rust
pub trait ObservableInterface {
    /// Get interface health status
    fn health(&self) -> HealthStatus;
    
    /// Get real-time metrics
    fn metrics(&self) -> InterfaceMetrics;
    
    /// Get distributed trace information
    fn trace_context(&self) -> TraceContext;
    
    /// Record custom metric
    fn record_metric(&self, metric: CustomMetric);
    
    /// Create child span for operation
    fn create_span(&self, operation: &str) -> Span;
}
```

### Security Standards

```rust
pub trait SecureInterface {
    /// Authenticate caller
    async fn authenticate(&self, credentials: Credentials) -> Result<Identity>;
    
    /// Authorize specific operation
    async fn authorize(&self, identity: &Identity, operation: &str) -> Result<bool>;
    
    /// Audit security events
    fn audit_security_event(&self, event: SecurityEvent);
    
    /// Rate limiting
    async fn check_rate_limit(&self, identity: &Identity) -> Result<RateLimitStatus>;
}
```

## Implementation Guidelines

### 1. Contract Testing
- Every interface must have comprehensive contract tests
- Consumer-driven contract testing between layers
- Breaking change detection in CI/CD pipeline

### 2. Backward Compatibility  
- All interfaces must maintain backward compatibility
- Deprecation warnings with 3-month notice period
- Versioned APIs with support for N-1 versions

### 3. Performance Testing
- Load testing for all SLA requirements
- Chaos engineering for failure scenarios
- Resource usage profiling under load

### 4. Monitoring and Alerting
- RED metrics (Rate, Errors, Duration) for all interfaces
- SLA violation alerting with severity levels
- Distributed tracing across all layer boundaries

### 5. Documentation
- OpenAPI specs for all REST interfaces
- gRPC protobuf definitions for binary interfaces
- Example code and integration guides

## Conclusion

These production interface contracts ensure:

1. **Reliability**: Comprehensive error handling and graceful degradation
2. **Observability**: Full monitoring, tracing, and metrics
3. **Security**: Authentication, authorization, and audit trails
4. **Performance**: Defined SLAs with measurement and alerting
5. **Maintainability**: Versioned interfaces with backward compatibility

Each interface is designed to handle real-world production scenarios from Day 1, eliminating the technical debt that comes from "MVP shortcuts".