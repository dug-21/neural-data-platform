# Neural Time Series Platform - Template Documentation

This document provides comprehensive guidance for using the template structures to accelerate development while maintaining strict architectural compliance.

## Overview

The template system provides five core template categories that enforce the architectural principles defined in the HIGH-LEVEL-ARCHITECTURE.md:

1. **Module Boilerplate** - Foundation for all platform modules
2. **Redis Streams Handlers** - Event-driven message processing
3. **Service Contracts** - Type-safe service interfaces
4. **Test Harness** - Module isolation validation
5. **Configuration Management** - Hierarchical configuration system

## Quick Start

### 1. Creating a New Module

```bash
# Generate module template
cd /workspaces/neural-trader/src/templates
cargo run --bin generate-module -- \
  --name "market-data-ingestion" \
  --domain "DataIngestion" \
  --input-patterns "external.market.*" \
  --output-patterns "data.trading.*.raw"
```

### 2. Implementing Message Processing

```rust
use neural_trader::templates::redis_handlers::{MessageHandler, StreamPattern};

#[derive(Debug)]
pub struct MarketDataHandler;

#[async_trait]
impl MessageHandler for MarketDataHandler {
    type PayloadType = MarketDataEvent;
    
    async fn handle_message(&self, event: Event<Self::PayloadType>) -> Result<()> {
        // 1. Validate message format
        self.validate_message(&event).await?;
        
        // 2. Process market data
        let processed_data = self.process_market_data(&event.payload).await?;
        
        // 3. Publish to processing streams
        self.publish_processed_data(processed_data).await?;
        
        Ok(())
    }
    
    fn subscription_patterns(&self) -> Vec<StreamPattern> {
        vec![StreamPattern::new("external", "market", "*", "*")]
    }
    
    fn publication_patterns(&self) -> Vec<StreamPattern> {
        vec![StreamPattern::new("data", "trading", "*", "raw")]
    }
}
```

### 3. Setting Up Configuration

```rust
use neural_trader::templates::configuration::{
    HierarchicalConfigManager, ConfigLevel, ConfigEntry
};

// Initialize configuration manager
let config_manager = HierarchicalConfigManager::new(
    PathBuf::from("/config"),
    "production".to_string(),
).await?;

// Set module configuration
config_manager.set(
    ConfigLevel::Module("market-data-ingestion".to_string()),
    "batch_size",
    &1000u32,
    "admin",
    "Optimize throughput",
).await?;
```

### 4. Running Isolation Tests

```rust
use neural_trader::templates::test_harness::{TestRunner, TestHarnessConfig};

let test_config = TestHarnessConfig {
    test_duration_seconds: 300,
    max_message_rate: 1000,
    fault_injection_enabled: true,
    ..TestHarnessConfig::default()
};

let test_runner = TestRunner::new(test_config);
let report = test_runner.run_test_suite(
    "market-data-isolation",
    &module,
    &config,
    &handler,
    &service,
    &contracts,
).await;

assert!(report.passed(), "Isolation tests failed: {}", report.to_markdown());
```

## Architecture Compliance

### Domain Isolation Enforcement

The templates automatically enforce domain boundaries:

```rust
// ✅ ALLOWED: Data ingestion publishing to core platform
StreamPattern::new("data", "trading", "alpaca", "raw")

// ❌ FORBIDDEN: Data ingestion directly to execution
StreamPattern::new("executions", "trading", "orders", "confirmed")

// ✅ ALLOWED: Decision service consuming processed data
StreamPattern::new("data", "trading", "*", "processed")

// ❌ FORBIDDEN: Decision service accessing raw data
StreamPattern::new("data", "trading", "*", "raw")
```

### Stream Naming Convention

All stream names follow the strict convention: `{category}.{domain}.{source}.{type}`

**Valid Categories:**
- `data` - Raw and processed data streams
- `features` - Computed features and indicators
- `decisions` - Autonomous decision outputs
- `executions` - Execution confirmations and results
- `metrics` - Performance and system metrics

**Domain Examples:**
- `trading` - Trading-related streams
- `system-ops` - System operations streams
- `risk-management` - Risk control streams

### Configuration Hierarchy

Configuration follows a three-level hierarchy with proper precedence:

```
/config
  /global                    # Platform-wide settings (precedence: 1)
    - platform.yaml
    - observability.yaml
  /domains                   # Domain-specific settings (precedence: 2)
    /trading
      - strategies.yaml
      - risk.yaml
    /system-ops
      - thresholds.yaml
  /modules                   # Module-specific settings (precedence: 3)
    /market-data-ingestion
      - sources.yaml
      - processing.yaml
```

## Template Reference

### Module Boilerplate Template

**Key Features:**
- Lifecycle management (initialize, health_check, shutdown)
- Observability integration (metrics, tracing)
- Message handling with correlation tracking
- Configuration validation
- Error handling with circuit breakers

**Usage:**
```rust
impl Module for MyModule {
    type Config = MyModuleConfig;
    type PayloadType = MyPayload;

    async fn initialize(&self, config: Self::Config) -> Result<()> {
        // Validate configuration
        config.validate()?;
        
        // Initialize connections
        self.setup_redis_connection(&config).await?;
        
        // Start health monitoring
        self.start_health_monitor().await?;
        
        Ok(())
    }

    async fn handle_message(&self, msg: Event<Self::PayloadType>) -> Result<()> {
        // Start distributed trace
        let span_id = self.traces().start_span("handle_message", None).await;
        
        // Process with metrics
        let start = Instant::now();
        let result = self.process_message(msg).await;
        
        // Record latency
        self.metrics().record_histogram(
            "message_processing_latency_ms",
            start.elapsed().as_millis() as f64,
            HashMap::new(),
        ).await;
        
        self.traces().end_span(&span_id).await;
        result
    }
}
```

### Redis Streams Handler Template

**Key Features:**
- Circuit breaker patterns for fault tolerance
- Backpressure and flow control
- Dead letter queue support
- Performance monitoring
- Stream pattern matching

**Message Flow:**
1. Subscribe to input patterns with consumer groups
2. Validate messages against domain rules
3. Process with concurrency control
4. Publish to output streams
5. Handle errors with retry logic

### Service Contract Template

**Key Features:**
- Type-safe interfaces with JSON Schema validation
- Version compatibility checking
- Domain interaction rules
- SLA requirements specification
- Contract evolution support

**Example Contract:**
```yaml
name: "trading-decision-service"
domain: "TradingDecision"
version: "1.2.0"
capabilities:
  - name: "momentum_strategy"
    required: true
    version: "1.0.0"
  - name: "risk_assessment"
    required: true
    version: "1.1.0"
dependencies:
  - service: "core-data-platform"
    min_version: "1.0.0"
    required: true
input_schemas:
  "data.trading.*.processed":
    type: "object"
    properties:
      symbol: { type: "string" }
      price: { type: "number" }
      volume: { type: "number" }
sla_requirements:
  max_latency_ms: 100
  min_availability_percent: 99.9
  max_error_rate_percent: 0.5
```

### Test Harness Template

**Test Categories:**

1. **Module Isolation Tests**
   - Domain interaction validation
   - Stream access pattern verification
   - Configuration namespace isolation
   - Performance boundary compliance

2. **Message Contract Tests**
   - Schema validation
   - Pattern matching verification
   - Error handling validation

3. **Service Contract Tests**
   - Interface compatibility
   - Version compatibility
   - SLA compliance

4. **Integration Tests**
   - End-to-end workflow validation
   - Fault tolerance verification
   - Performance under load

### Configuration Management Template

**Features:**
- Hierarchical configuration with precedence
- JSON Schema validation
- Hot-reload capabilities
- Environment variable overrides
- Audit logging for changes
- Multi-format export (JSON, YAML, TOML, ENV)

**Environment Overrides:**
```bash
# Override any configuration with environment variables
NT_REDIS_URL="redis://production:6379"
NT_WORKER_THREADS="16"
NT_LOG_LEVEL="info"
```

## Best Practices

### 1. Module Development

- Always start with the module boilerplate template
- Implement all required traits completely
- Add comprehensive error handling
- Include detailed logging and metrics
- Write isolation tests before implementation

### 2. Message Processing

- Validate all incoming messages
- Use correlation IDs for tracing
- Implement idempotent processing
- Handle backpressure gracefully
- Monitor processing latency

### 3. Configuration Management

- Use the hierarchical structure consistently
- Validate all configuration changes
- Document configuration schemas
- Version configuration changes
- Test with different environments

### 4. Testing Strategy

- Run isolation tests in CI/CD pipeline
- Test with realistic message volumes
- Inject faults to test resilience
- Validate performance boundaries
- Monitor test metrics over time

## Common Patterns

### Error Handling
```rust
// Circuit breaker pattern
if !self.circuit_breaker.can_execute().await {
    return Err(anyhow!("Circuit breaker is open"));
}

match self.process_message(msg).await {
    Ok(result) => {
        self.circuit_breaker.record_success().await;
        Ok(result)
    }
    Err(e) => {
        self.circuit_breaker.record_failure().await;
        self.send_to_dlq(msg, &e).await?;
        Err(e)
    }
}
```

### Correlation Tracking
```rust
// Propagate correlation IDs through the system
let correlation_id = msg.correlation_id;
let output_msg = Event::new(domain, source, payload)
    .with_correlation_id(correlation_id);
```

### Performance Monitoring
```rust
// Track key performance metrics
self.metrics().record_histogram(
    "neural_platform_module_processing_latency_ms",
    latency_ms,
    [
        ("module", module_name),
        ("domain", domain),
        ("operation", operation_name),
    ].into(),
).await;
```

## Troubleshooting

### Common Issues

1. **Module Not Starting**
   - Check configuration validation
   - Verify Redis connectivity
   - Review health check implementation

2. **Message Processing Failures**
   - Validate message schemas
   - Check stream access permissions
   - Review error handling logic

3. **Isolation Test Failures**
   - Verify domain interaction rules
   - Check stream naming conventions
   - Review configuration namespacing

4. **Performance Issues**
   - Check concurrency limits
   - Review message batching
   - Monitor resource usage

### Debugging Tips

- Enable detailed logging for troubleshooting
- Use distributed tracing to follow message flows
- Monitor circuit breaker states
- Check configuration precedence resolution

## Migration Guide

### From Legacy Code

1. **Identify Module Boundaries**
   - Map existing code to domain boundaries
   - Identify current dependencies
   - Plan gradual migration

2. **Implement Templates Gradually**
   - Start with configuration management
   - Add observability to existing code
   - Implement message handlers
   - Add isolation tests

3. **Validate Compliance**
   - Run architecture validation
   - Test isolation boundaries
   - Verify performance requirements
   - Update documentation

## Support

For questions or issues with the templates:

1. Check the template documentation
2. Review example implementations
3. Run the validation utilities
4. Consult the architecture document
5. File issues in the project repository