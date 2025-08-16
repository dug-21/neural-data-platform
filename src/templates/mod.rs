//! Template Structures for Neural Time Series Platform
//! 
//! This module provides comprehensive templates that enforce the architectural
//! principles defined in the HIGH-LEVEL-ARCHITECTURE.md document.
//! 
//! ## Template Categories
//! 
//! ### 1. Module Boilerplate (`module_boilerplate`)
//! - Base module implementation following the Module trait
//! - Lifecycle management (initialize, health_check, shutdown)
//! - Observability integration (metrics, tracing)
//! - Message handling with proper isolation
//! - Configuration validation
//! 
//! ### 2. Redis Streams Handlers (`redis_handlers`)
//! - Event-driven message processing
//! - Stream pattern subscription with isolation enforcement
//! - Circuit breaker patterns for fault tolerance
//! - Backpressure and flow control
//! - Performance monitoring and metrics
//! 
//! ### 3. Service Contracts (`service_contracts`)
//! - Type-safe service interfaces
//! - Domain boundary enforcement
//! - Version compatibility checking
//! - Contract validation and compliance
//! - Schema evolution support
//! 
//! ### 4. Test Harness (`test_harness`)
//! - Module isolation verification
//! - Message contract testing
//! - Performance boundary validation
//! - Fault injection and error simulation
//! - Integration test orchestration
//! 
//! ### 5. Configuration Management (`configuration`)
//! - Hierarchical configuration structure
//! - Schema validation with JSON Schema
//! - Hot-reload capabilities
//! - Environment-specific overrides
//! - Audit logging for changes
//! 
//! ## Architecture Compliance
//! 
//! All templates enforce the core architectural principles:
//! 
//! - **Strict Module Isolation**: No unintended interactions between modules
//! - **Decision Accuracy First**: Correctness over performance
//! - **Observable by Design**: Comprehensive metrics and tracing
//! - **Progressive Scalability**: From Docker to Kubernetes
//! - **Domain Agnostic Core**: Generic platform with domain-specific implementations
//! - **Fail-Safe Autonomy**: Safe autonomous operation with human oversight
//! 
//! ## Usage Examples
//! 
//! ### Creating a New Module
//! 
//! ```rust
//! use neural_trader::templates::module_boilerplate::{BaseModule, ModuleConfig, Module};
//! use neural_trader::templates::service_contracts::ServiceDomain;
//! 
//! // Define your module configuration
//! #[derive(Debug, Clone)]
//! struct MyModuleConfig {
//!     name: String,
//!     domain: String,
//!     // ... other config fields
//! }
//! 
//! impl ModuleConfig for MyModuleConfig {
//!     fn validate(&self) -> anyhow::Result<()> {
//!         // Validation logic
//!         Ok(())
//!     }
//!     
//!     fn module_name(&self) -> &str { &self.name }
//!     fn domain(&self) -> &str { &self.domain }
//!     // ... implement other required methods
//! }
//! 
//! // Create your module using the base template
//! let module = BaseModule::<MyModuleConfig, MyPayloadType>::new(
//!     "my-module".to_string(),
//!     metrics_exporter,
//!     trace_exporter,
//! );
//! ```
//! 
//! ### Implementing a Message Handler
//! 
//! ```rust
//! use neural_trader::templates::redis_handlers::{MessageHandler, StreamPattern};
//! 
//! struct MyMessageHandler;
//! 
//! #[async_trait]
//! impl MessageHandler for MyMessageHandler {
//!     type PayloadType = MyPayload;
//!     
//!     async fn handle_message(&self, event: Event<Self::PayloadType>) -> Result<()> {
//!         // Process the message
//!         Ok(())
//!     }
//!     
//!     fn subscription_patterns(&self) -> Vec<StreamPattern> {
//!         vec![StreamPattern::new("data", "trading", "*", "processed")]
//!     }
//!     
//!     fn publication_patterns(&self) -> Vec<StreamPattern> {
//!         vec![StreamPattern::new("decisions", "trading", "my-strategy", "*")]
//!     }
//! }
//! ```
//! 
//! ### Setting Up Configuration
//! 
//! ```rust
//! use neural_trader::templates::configuration::{HierarchicalConfigManager, ConfigLevel};
//! 
//! // Create configuration manager
//! let config_manager = HierarchicalConfigManager::new(
//!     PathBuf::from("/config"),
//!     "production".to_string(),
//! ).await?;
//! 
//! // Set module-specific configuration
//! config_manager.set(
//!     ConfigLevel::Module("my-module".to_string()),
//!     "worker_threads",
//!     &8u32,
//!     "admin",
//!     "Performance optimization",
//! ).await?;
//! ```
//! 
//! ### Running Isolation Tests
//! 
//! ```rust
//! use neural_trader::templates::test_harness::{TestRunner, TestHarnessConfig};
//! 
//! let test_config = TestHarnessConfig::default();
//! let test_runner = TestRunner::new(test_config);
//! 
//! let report = test_runner.run_test_suite(
//!     "module-isolation-test",
//!     &module,
//!     &config,
//!     &handler,
//!     &service,
//!     &contracts,
//! ).await;
//! 
//! println!("{}", report.to_markdown());
//! assert!(report.passed());
//! ```
//! 
//! ## Stream Naming Convention
//! 
//! All templates follow the stream naming convention:
//! `{category}.{domain}.{source}.{type}`
//! 
//! Examples:
//! - `data.trading.alpaca.raw` - Raw market data from Alpaca
//! - `features.trading.rsi.15m` - RSI indicator calculated on 15-minute intervals
//! - `decisions.trading.momentum` - Trading decisions from momentum strategy
//! - `executions.trading.confirmed` - Confirmed trade executions
//! - `metrics.system-ops.performance` - System performance metrics
//! 
//! ## Error Handling
//! 
//! All templates implement consistent error handling:
//! 
//! - **Circuit Breakers**: Prevent cascade failures
//! - **Retry Logic**: Exponential backoff with jitter
//! - **Dead Letter Queues**: Handle unprocessable messages
//! - **Graceful Degradation**: Maintain core functionality under stress
//! - **Error Propagation**: Proper error context and correlation
//! 
//! ## Performance Considerations
//! 
//! Templates are designed for high performance:
//! 
//! - **Async/Await**: Non-blocking I/O operations
//! - **Message Batching**: Efficient batch processing
//! - **Connection Pooling**: Reuse Redis connections
//! - **Metrics Sampling**: Configurable sampling rates
//! - **Memory Management**: Bounded memory usage
//! 
//! ## Security Features
//! 
//! - **Input Validation**: Schema-based validation
//! - **Access Control**: Domain-based permissions
//! - **Audit Logging**: Complete audit trail
//! - **Secret Management**: No hardcoded secrets
//! - **Network Isolation**: Module boundary enforcement

pub mod module_boilerplate;
pub mod redis_handlers;
pub mod service_contracts;
pub mod test_harness;
pub mod configuration;

// Re-export commonly used types for convenience
pub use module_boilerplate::{Module, ModuleConfig, Event, BaseModule};
pub use redis_handlers::{MessageHandler, StreamPattern, RedisStreamHandler};
pub use service_contracts::{ServiceContract, ServiceDomain, ContractVersion, ServiceInterface};
pub use test_harness::{IsolationTestHarness, TestRunner, TestResult};
pub use configuration::{ConfigurationManager, HierarchicalConfigManager, ConfigLevel};

/// Template validation utilities
pub mod validation {
    use super::*;
    use anyhow::{Result, anyhow};

    /// Validate that a module follows architectural principles
    pub async fn validate_module_architecture<M, C>(module: &M, config: &C) -> Result<Vec<String>>
    where
        M: Module<Config = C>,
        C: ModuleConfig,
    {
        let mut violations = Vec::new();

        // Check naming conventions
        let module_name = config.module_name();
        if !module_name.contains(config.domain()) {
            violations.push(format!(
                "Module name '{}' should include domain '{}'",
                module_name,
                config.domain()
            ));
        }

        // Check stream patterns
        let input_streams = config.input_streams();
        let output_streams = config.output_streams();

        for stream in &input_streams {
            if !is_valid_stream_name(stream) {
                violations.push(format!("Invalid input stream pattern: {}", stream));
            }
        }

        for stream in &output_streams {
            if !is_valid_stream_name(stream) {
                violations.push(format!("Invalid output stream pattern: {}", stream));
            }
        }

        // Test health check response time
        let start = std::time::Instant::now();
        let _health = module.health_check().await;
        let duration = start.elapsed();

        if duration > std::time::Duration::from_millis(1000) {
            violations.push(format!(
                "Health check took {}ms, exceeds 1000ms limit",
                duration.as_millis()
            ));
        }

        Ok(violations)
    }

    /// Validate stream naming convention
    fn is_valid_stream_name(stream: &str) -> bool {
        let parts: Vec<&str> = stream.split('.').collect();
        if parts.len() != 4 {
            return false;
        }

        let (category, _domain, _source, _type) = (parts[0], parts[1], parts[2], parts[3]);

        // Valid categories from architecture
        matches!(category, "data" | "features" | "decisions" | "executions" | "metrics")
    }

    /// Validate service contract compliance
    pub fn validate_service_contract(contract: &ServiceContract) -> Result<Vec<String>> {
        let mut violations = Vec::new();

        // Check domain-specific stream patterns
        let allowed_inputs = contract.domain.allowed_input_patterns();
        let allowed_outputs = contract.domain.allowed_output_patterns();

        for input_pattern in contract.input_schemas.keys() {
            if !allowed_inputs.iter().any(|pattern| pattern_matches(pattern, input_pattern)) {
                violations.push(format!(
                    "Input pattern '{}' not allowed for domain {:?}",
                    input_pattern,
                    contract.domain
                ));
            }
        }

        for output_pattern in contract.output_schemas.keys() {
            if !allowed_outputs.iter().any(|pattern| pattern_matches(pattern, output_pattern)) {
                violations.push(format!(
                    "Output pattern '{}' not allowed for domain {:?}",
                    output_pattern,
                    contract.domain
                ));
            }
        }

        // Validate SLA requirements
        if contract.sla_requirements.max_latency_ms == 0 {
            violations.push("SLA max latency must be greater than 0".to_string());
        }

        if contract.sla_requirements.min_availability_percent < 0.0 
            || contract.sla_requirements.min_availability_percent > 100.0 {
            violations.push("SLA availability must be between 0% and 100%".to_string());
        }

        Ok(violations)
    }

    fn pattern_matches(allowed_pattern: &str, actual_pattern: &str) -> bool {
        allowed_pattern == "*" || 
        allowed_pattern == actual_pattern ||
        (allowed_pattern.ends_with("*") && 
         actual_pattern.starts_with(&allowed_pattern[..allowed_pattern.len()-1]))
    }
}

/// Template generation utilities
pub mod generators {
    use super::*;
    use std::collections::HashMap;

    /// Generate a complete module from template
    pub fn generate_module_template(
        module_name: &str,
        domain: ServiceDomain,
        input_patterns: Vec<String>,
        output_patterns: Vec<String>,
    ) -> String {
        format!(
            r#"//! {} Module
//! 
//! Auto-generated module following the Neural Time Series Platform architecture.
//! Domain: {:?}

use std::collections::HashMap;
use async_trait::async_trait;
use serde::{{Deserialize, Serialize}};
use anyhow::Result;
use neural_trader::templates::module_boilerplate::{{
    BaseModule, Module, ModuleConfig, Event, HealthStatus,
    MetricsExporter, TraceExporter
}};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {}Config {{
    pub module_name: String,
    pub domain: String,
    pub redis_url: String,
    pub input_streams: Vec<String>,
    pub output_streams: Vec<String>,
    // Add module-specific configuration fields here
}}

impl ModuleConfig for {}Config {{
    fn validate(&self) -> Result<()> {{
        if self.module_name.is_empty() {{
            return Err(anyhow::anyhow!("Module name cannot be empty"));
        }}
        if self.domain.is_empty() {{
            return Err(anyhow::anyhow!("Domain cannot be empty"));
        }}
        Ok(())
    }}

    fn module_name(&self) -> &str {{ &self.module_name }}
    fn domain(&self) -> &str {{ &self.domain }}
    fn input_streams(&self) -> Vec<String> {{ self.input_streams.clone() }}
    fn output_streams(&self) -> Vec<String> {{ self.output_streams.clone() }}
}}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct {}Payload {{
    // Define your payload structure here
    pub data: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}}

pub struct {} {{
    base: BaseModule<{}Config, {}Payload>,
}}

impl {} {{
    pub fn new(
        metrics_exporter: Box<dyn MetricsExporter>,
        trace_exporter: Box<dyn TraceExporter>,
    ) -> Self {{
        Self {{
            base: BaseModule::new(
                "{}".to_string(),
                metrics_exporter,
                trace_exporter,
            ),
        }}
    }}
}}

#[async_trait]
impl Module for {} {{
    type Config = {}Config;
    type PayloadType = {}Payload;

    async fn initialize(&self, config: Self::Config) -> Result<()> {{
        // Add module-specific initialization logic here
        self.base.initialize(config).await
    }}

    async fn health_check(&self) -> HealthStatus {{
        // Add module-specific health checks here
        self.base.health_check().await
    }}

    async fn shutdown(&self) -> Result<()> {{
        // Add module-specific cleanup logic here
        self.base.shutdown().await
    }}

    fn metrics(&self) -> Box<dyn MetricsExporter> {{
        self.base.metrics()
    }}

    fn traces(&self) -> Box<dyn TraceExporter> {{
        self.base.traces()
    }}

    async fn handle_message(&self, msg: Event<Self::PayloadType>) -> Result<()> {{
        // Add module-specific message processing logic here
        self.base.handle_message(msg).await
    }}

    fn name(&self) -> &str {{
        self.base.name()
    }}
}}

impl Default for {}Config {{
    fn default() -> Self {{
        Self {{
            module_name: "{}".to_string(),
            domain: "{:?}".to_string(),
            redis_url: "redis://localhost:6379".to_string(),
            input_streams: vec!{:?},
            output_streams: vec!{:?},
        }}
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;
    use neural_trader::templates::module_boilerplate::{{NoOpMetricsExporter, NoOpTraceExporter}};

    #[tokio::test]
    async fn test_{}_lifecycle() {{
        let module = {}::new(
            Box::new(NoOpMetricsExporter),
            Box::new(NoOpTraceExporter),
        );

        let config = {}Config::default();
        assert!(config.validate().is_ok());

        assert!(module.initialize(config).await.is_ok());
        assert!(matches!(module.health_check().await, HealthStatus::Healthy));
        assert!(module.shutdown().await.is_ok());
    }}
}}
"#,
            module_name,
            domain,
            module_name,
            module_name,
            module_name,
            module_name,
            module_name,
            module_name,
            module_name,
            module_name,
            module_name,
            module_name,
            module_name,
            module_name,
            module_name,
            domain,
            input_patterns,
            output_patterns,
            module_name.to_lowercase().replace("-", "_"),
            module_name,
            module_name,
        )
    }

    /// Generate service contract template
    pub fn generate_service_contract_template(
        service_name: &str,
        domain: ServiceDomain,
        capabilities: Vec<&str>,
    ) -> String {
        let capability_list = capabilities
            .iter()
            .map(|cap| format!(
                r#"            ServiceCapability {{
                name: "{}".to_string(),
                description: "{} capability".to_string(),
                required: true,
                version: ContractVersion::new(1, 0, 0),
            }}"#,
                cap, cap
            ))
            .collect::<Vec<_>>()
            .join(",\n");

        format!(
            r#"//! {} Service Contract
//! 
//! Auto-generated service contract for {:?} domain.

use neural_trader::templates::service_contracts::{{
    ServiceContract, ServiceDomain, ContractVersion, ServiceCapability,
    ContractDependency, SlaRequirements
}};
use std::collections::HashMap;
use chrono::Utc;

pub fn create_{}_contract() -> ServiceContract {{
    ServiceContract {{
        name: "{}".to_string(),
        domain: ServiceDomain::{:?},
        version: ContractVersion::new(1, 0, 0),
        description: "{} service for {:?} domain".to_string(),
        capabilities: vec![
{}
        ],
        dependencies: vec![
            ContractDependency {{
                service_name: "redis-streams".to_string(),
                domain: ServiceDomain::CoreDataPlatform,
                min_version: ContractVersion::new(1, 0, 0),
                max_version: None,
                required: true,
            }},
        ],
        input_schemas: HashMap::new(),
        output_schemas: HashMap::new(),
        error_codes: HashMap::new(),
        sla_requirements: SlaRequirements::default(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }}
}}
"#,
            service_name,
            domain,
            service_name.to_lowercase().replace("-", "_"),
            service_name,
            domain,
            service_name,
            domain,
            capability_list
        )
    }

    /// Generate test template
    pub fn generate_test_template(module_name: &str) -> String {
        format!(
            r#"//! Integration Tests for {}
//! 
//! Auto-generated test suite for module isolation validation.

use neural_trader::templates::test_harness::{{
    IsolationTestHarness, TestHarnessConfig, TestRunner
}};
use neural_trader::templates::module_boilerplate::Module;

#[tokio::test]
async fn test_{}_isolation() {{
    let test_config = TestHarnessConfig::default();
    let harness = IsolationTestHarness::new(test_config);

    // Create your module instance
    let module = create_test_module();
    let config = create_test_config();

    let result = harness.test_module_isolation(&module, &config).await;
    
    assert!(result.success, "Module isolation test failed: {{:?}}", result.violations);
    assert!(result.violations.is_empty(), "Found isolation violations: {{:#?}}", result.violations);
}}

#[tokio::test]
async fn test_{}_performance_boundaries() {{
    let test_config = TestHarnessConfig {{
        performance_monitoring: true,
        ..TestHarnessConfig::default()
    }};
    
    let test_runner = TestRunner::new(test_config);
    
    // Create test components
    let module = create_test_module();
    let config = create_test_config();
    let handler = create_test_handler();
    let service = create_test_service();
    let contracts = vec![];

    let report = test_runner.run_test_suite(
        "{}-performance-test",
        &module,
        &config,
        &handler,
        &service,
        &contracts,
    ).await;

    println!("{{}}", report.to_markdown());
    assert!(report.passed(), "Performance tests failed");
}}

// Helper functions - implement these based on your module
fn create_test_module() -> impl Module {{
    // Return your module instance
    todo!("Implement module creation")
}}

fn create_test_config() -> impl neural_trader::templates::module_boilerplate::ModuleConfig {{
    // Return your config instance
    todo!("Implement config creation")
}}

fn create_test_handler() -> impl neural_trader::templates::redis_handlers::MessageHandler {{
    // Return your handler instance
    todo!("Implement handler creation")
}}

fn create_test_service() -> impl neural_trader::templates::service_contracts::ServiceInterface {{
    // Return your service instance
    todo!("Implement service creation")
}}
"#,
            module_name,
            module_name.to_lowercase().replace("-", "_"),
            module_name.to_lowercase().replace("-", "_"),
            module_name
        )
    }
}