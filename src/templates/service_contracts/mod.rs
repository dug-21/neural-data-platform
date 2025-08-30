//! Service Contract Definition Template
//! 
//! This template provides a comprehensive framework for defining service contracts
//! that enforce module isolation and type safety across the platform.
//! 
//! Key Features:
//! - Strong typing for all interactions
//! - Version compatibility checking
//! - Contract validation and compliance
//! - Schema evolution support
//! - API documentation generation
//! - Mock implementations for testing

use std::collections::HashMap;
use std::fmt;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::{Result, anyhow};

/// Service contract version for compatibility checking
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContractVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl ContractVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }

    /// Check if this version is compatible with another version
    /// Compatible if major versions match and this version is >= other version
    pub fn is_compatible_with(&self, other: &ContractVersion) -> bool {
        self.major == other.major && 
        (self.minor > other.minor || 
         (self.minor == other.minor && self.patch >= other.patch))
    }
}

impl fmt::Display for ContractVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Service capability definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceCapability {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub version: ContractVersion,
}

/// Domain boundaries for module isolation enforcement
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServiceDomain {
    DataIngestion,
    CoreDataPlatform,
    TradingDecision,
    TradingExecution,
    SystemOpsDecision,
    SystemOpsExecution,
    Observability,
    Configuration,
}

impl ServiceDomain {
    /// Check if this domain can interact with another domain
    pub fn can_interact_with(&self, other: &ServiceDomain) -> bool {
        match (self, other) {
            // Data ingestion can only publish to core data platform
            (ServiceDomain::DataIngestion, ServiceDomain::CoreDataPlatform) => true,
            
            // Core data platform can consume from ingestion and publish to decision layers
            (ServiceDomain::CoreDataPlatform, ServiceDomain::TradingDecision) => true,
            (ServiceDomain::CoreDataPlatform, ServiceDomain::SystemOpsDecision) => true,
            
            // Decision layers can consume from core platform and publish to execution
            (ServiceDomain::TradingDecision, ServiceDomain::TradingExecution) => true,
            (ServiceDomain::SystemOpsDecision, ServiceDomain::SystemOpsExecution) => true,
            
            // All domains can interact with observability and configuration
            (_, ServiceDomain::Observability) => true,
            (_, ServiceDomain::Configuration) => true,
            (ServiceDomain::Observability, _) => true,
            (ServiceDomain::Configuration, _) => true,
            
            // Same domain interactions are allowed
            (a, b) if a == b => true,
            
            // All other interactions are forbidden
            _ => false,
        }
    }

    /// Get allowed input stream patterns for this domain
    pub fn allowed_input_patterns(&self) -> Vec<String> {
        match self {
            ServiceDomain::DataIngestion => vec![], // External inputs only
            ServiceDomain::CoreDataPlatform => vec!["data.*.*.*".to_string()],
            ServiceDomain::TradingDecision => vec![
                "data.trading.*.processed".to_string(),
                "features.trading.*".to_string(),
            ],
            ServiceDomain::TradingExecution => vec![
                "decisions.trading.*".to_string(),
            ],
            ServiceDomain::SystemOpsDecision => vec![
                "data.system-ops.*.processed".to_string(),
                "features.system-ops.*".to_string(),
            ],
            ServiceDomain::SystemOpsExecution => vec![
                "decisions.system-ops.*".to_string(),
            ],
            ServiceDomain::Observability => vec!["*.*.*.*".to_string()], // Can read all
            ServiceDomain::Configuration => vec![], // Configuration only
        }
    }

    /// Get allowed output stream patterns for this domain
    pub fn allowed_output_patterns(&self) -> Vec<String> {
        match self {
            ServiceDomain::DataIngestion => vec!["data.*.*.*".to_string()],
            ServiceDomain::CoreDataPlatform => vec![
                "data.*.*.processed".to_string(),
                "features.*.*".to_string(),
            ],
            ServiceDomain::TradingDecision => vec![
                "decisions.trading.*".to_string(),
            ],
            ServiceDomain::TradingExecution => vec![
                "executions.trading.*".to_string(),
                "metrics.trading.*".to_string(),
            ],
            ServiceDomain::SystemOpsDecision => vec![
                "decisions.system-ops.*".to_string(),
            ],
            ServiceDomain::SystemOpsExecution => vec![
                "executions.system-ops.*".to_string(),
                "metrics.system-ops.*".to_string(),
            ],
            ServiceDomain::Observability => vec![
                "metrics.*.*".to_string(),
                "traces.*.*".to_string(),
            ],
            ServiceDomain::Configuration => vec![], // Configuration only
        }
    }
}

/// Service contract metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceContract {
    pub name: String,
    pub domain: ServiceDomain,
    pub version: ContractVersion,
    pub description: String,
    pub capabilities: Vec<ServiceCapability>,
    pub dependencies: Vec<ContractDependency>,
    pub input_schemas: HashMap<String, String>, // Stream pattern -> JSON schema
    pub output_schemas: HashMap<String, String>, // Stream pattern -> JSON schema
    pub error_codes: HashMap<String, String>,
    pub sla_requirements: SlaRequirements,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Contract dependency specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractDependency {
    pub service_name: String,
    pub domain: ServiceDomain,
    pub min_version: ContractVersion,
    pub max_version: Option<ContractVersion>,
    pub required: bool,
}

/// SLA requirements for the service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlaRequirements {
    pub max_latency_ms: u64,
    pub min_availability_percent: f64,
    pub max_error_rate_percent: f64,
    pub throughput_per_second: u64,
}

impl Default for SlaRequirements {
    fn default() -> Self {
        Self {
            max_latency_ms: 1000,
            min_availability_percent: 99.9,
            max_error_rate_percent: 1.0,
            throughput_per_second: 1000,
        }
    }
}

/// Standard request/response types for service interactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceRequest<T> {
    pub id: Uuid,
    pub correlation_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub service_name: String,
    pub operation: String,
    pub version: ContractVersion,
    pub payload: T,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceResponse<T> {
    pub id: Uuid,
    pub correlation_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub service_name: String,
    pub operation: String,
    pub version: ContractVersion,
    pub success: bool,
    pub payload: Option<T>,
    pub error: Option<ServiceError>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Standard error format for service interactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub retryable: bool,
}

/// Service interface trait that all services must implement
#[async_trait]
pub trait ServiceInterface: Send + Sync {
    /// Get the service contract
    fn contract(&self) -> &ServiceContract;

    /// Validate that this service can interact with another service
    fn can_interact_with(&self, other_domain: &ServiceDomain) -> bool {
        self.contract().domain.can_interact_with(other_domain)
    }

    /// Validate a request against the contract
    async fn validate_request<T>(&self, request: &ServiceRequest<T>) -> Result<()>
    where
        T: Serialize + for<'de> Deserialize<'de>;

    /// Health check for the service
    async fn health_check(&self) -> ServiceResponse<HealthStatus>;

    /// Get service metrics
    async fn metrics(&self) -> ServiceResponse<HashMap<String, f64>>;

    /// Validate contract compatibility with another service
    fn validate_compatibility(&self, other_contract: &ServiceContract) -> Result<()> {
        // Check domain interaction rules
        if !self.contract().domain.can_interact_with(&other_contract.domain) {
            return Err(anyhow!(
                "Service {} (domain: {:?}) cannot interact with service {} (domain: {:?})",
                self.contract().name,
                self.contract().domain,
                other_contract.name,
                other_contract.domain
            ));
        }

        // Check version compatibility for dependencies
        for dependency in &self.contract().dependencies {
            if dependency.service_name == other_contract.name {
                if !other_contract.version.is_compatible_with(&dependency.min_version) {
                    return Err(anyhow!(
                        "Service {} version {} is not compatible with required minimum version {}",
                        other_contract.name,
                        other_contract.version,
                        dependency.min_version
                    ));
                }

                if let Some(max_version) = &dependency.max_version {
                    if other_contract.version.major > max_version.major {
                        return Err(anyhow!(
                            "Service {} version {} exceeds maximum supported version {}",
                            other_contract.name,
                            other_contract.version,
                            max_version
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}

/// Contract registry for managing service contracts
pub struct ContractRegistry {
    contracts: HashMap<String, ServiceContract>,
    compatibility_matrix: HashMap<(String, String), bool>,
}

impl ContractRegistry {
    pub fn new() -> Self {
        Self {
            contracts: HashMap::new(),
            compatibility_matrix: HashMap::new(),
        }
    }

    /// Register a service contract
    pub fn register_contract(&mut self, contract: ServiceContract) -> Result<()> {
        // Validate contract
        self.validate_contract(&contract)?;

        // Check for conflicts with existing contracts
        if let Some(existing) = self.contracts.get(&contract.name) {
            if existing.version.major != contract.version.major {
                return Err(anyhow!(
                    "Major version change for service {} requires explicit migration",
                    contract.name
                ));
            }
        }

        self.contracts.insert(contract.name.clone(), contract);
        Ok(())
    }

    /// Get a service contract by name
    pub fn get_contract(&self, name: &str) -> Option<&ServiceContract> {
        self.contracts.get(name)
    }

    /// Validate all registered contracts for compatibility
    pub fn validate_all_contracts(&mut self) -> Result<()> {
        let contract_names: Vec<String> = self.contracts.keys().cloned().collect();

        for service_a in &contract_names {
            for service_b in &contract_names {
                if service_a != service_b {
                    let contract_a = self.contracts.get(service_a).unwrap();
                    let contract_b = self.contracts.get(service_b).unwrap();

                    let compatible = self.check_contract_compatibility(contract_a, contract_b);
                    self.compatibility_matrix.insert(
                        (service_a.clone(), service_b.clone()),
                        compatible,
                    );
                }
            }
        }

        Ok(())
    }

    /// Check if two services are compatible
    pub fn are_compatible(&self, service_a: &str, service_b: &str) -> bool {
        self.compatibility_matrix
            .get(&(service_a.to_string(), service_b.to_string()))
            .copied()
            .unwrap_or(false)
    }

    /// Generate contract documentation
    pub fn generate_documentation(&self) -> String {
        let mut docs = String::new();
        docs.push_str("# Service Contract Documentation\n\n");

        for contract in self.contracts.values() {
            docs.push_str(&format!("## {} ({})\n\n", contract.name, contract.version));
            docs.push_str(&format!("**Domain:** {:?}\n\n", contract.domain));
            docs.push_str(&format!("**Description:** {}\n\n", contract.description));

            docs.push_str("### Capabilities\n");
            for capability in &contract.capabilities {
                docs.push_str(&format!(
                    "- **{}** ({}): {} {}\n",
                    capability.name,
                    capability.version,
                    capability.description,
                    if capability.required { "(Required)" } else { "" }
                ));
            }
            docs.push_str("\n");

            docs.push_str("### Dependencies\n");
            for dependency in &contract.dependencies {
                docs.push_str(&format!(
                    "- **{}** ({:?}): {} - {} {}\n",
                    dependency.service_name,
                    dependency.domain,
                    dependency.min_version,
                    dependency.max_version
                        .as_ref()
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "latest".to_string()),
                    if dependency.required { "(Required)" } else { "" }
                ));
            }
            docs.push_str("\n");

            docs.push_str("### SLA Requirements\n");
            docs.push_str(&format!(
                "- Max Latency: {}ms\n- Min Availability: {}%\n- Max Error Rate: {}%\n- Throughput: {}/sec\n\n",
                contract.sla_requirements.max_latency_ms,
                contract.sla_requirements.min_availability_percent,
                contract.sla_requirements.max_error_rate_percent,
                contract.sla_requirements.throughput_per_second
            ));

            docs.push_str("---\n\n");
        }

        docs
    }

    fn validate_contract(&self, contract: &ServiceContract) -> Result<()> {
        // Validate stream patterns against domain rules
        let allowed_inputs = contract.domain.allowed_input_patterns();
        let allowed_outputs = contract.domain.allowed_output_patterns();

        for input_pattern in contract.input_schemas.keys() {
            if !allowed_inputs.iter().any(|pattern| self.pattern_matches(pattern, input_pattern)) {
                return Err(anyhow!(
                    "Input pattern '{}' not allowed for domain {:?}",
                    input_pattern,
                    contract.domain
                ));
            }
        }

        for output_pattern in contract.output_schemas.keys() {
            if !allowed_outputs.iter().any(|pattern| self.pattern_matches(pattern, output_pattern)) {
                return Err(anyhow!(
                    "Output pattern '{}' not allowed for domain {:?}",
                    output_pattern,
                    contract.domain
                ));
            }
        }

        Ok(())
    }

    fn check_contract_compatibility(&self, contract_a: &ServiceContract, contract_b: &ServiceContract) -> bool {
        // Check domain interaction rules
        if !contract_a.domain.can_interact_with(&contract_b.domain) {
            return false;
        }

        // Check if A depends on B
        for dependency in &contract_a.dependencies {
            if dependency.service_name == contract_b.name {
                if !contract_b.version.is_compatible_with(&dependency.min_version) {
                    return false;
                }

                if let Some(max_version) = &dependency.max_version {
                    if contract_b.version.major > max_version.major {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn pattern_matches(&self, allowed_pattern: &str, actual_pattern: &str) -> bool {
        // Simple pattern matching - in a real implementation, use a proper pattern matcher
        allowed_pattern == "*" || 
        allowed_pattern == actual_pattern ||
        (allowed_pattern.ends_with("*") && 
         actual_pattern.starts_with(&allowed_pattern[..allowed_pattern.len()-1]))
    }
}

/// Health status for service health checks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

/// Example implementations for common service types

/// Data Ingestion Service Contract Template
pub fn create_data_ingestion_contract(
    service_name: String,
    data_sources: Vec<String>,
    output_domain: String,
) -> ServiceContract {
    ServiceContract {
        name: service_name.clone(),
        domain: ServiceDomain::DataIngestion,
        version: ContractVersion::new(1, 0, 0),
        description: format!("Data ingestion service for {}", data_sources.join(", ")),
        capabilities: vec![
            ServiceCapability {
                name: "data_ingestion".to_string(),
                description: "Ingest data from external sources".to_string(),
                required: true,
                version: ContractVersion::new(1, 0, 0),
            },
            ServiceCapability {
                name: "data_normalization".to_string(),
                description: "Normalize data to standard format".to_string(),
                required: true,
                version: ContractVersion::new(1, 0, 0),
            },
        ],
        dependencies: vec![
            ContractDependency {
                service_name: "redis-streams".to_string(),
                domain: ServiceDomain::CoreDataPlatform,
                min_version: ContractVersion::new(1, 0, 0),
                max_version: None,
                required: true,
            },
        ],
        input_schemas: HashMap::new(), // External inputs
        output_schemas: {
            let mut schemas = HashMap::new();
            for source in &data_sources {
                schemas.insert(
                    format!("data.{}.{}.raw", output_domain, source),
                    r#"{"type": "object", "properties": {"timestamp": {"type": "string"}, "data": {"type": "object"}}}"#.to_string(),
                );
            }
            schemas
        },
        error_codes: {
            let mut codes = HashMap::new();
            codes.insert("INGESTION_001".to_string(), "Source connection failed".to_string());
            codes.insert("INGESTION_002".to_string(), "Data validation failed".to_string());
            codes.insert("INGESTION_003".to_string(), "Rate limit exceeded".to_string());
            codes
        },
        sla_requirements: SlaRequirements {
            max_latency_ms: 1000,
            min_availability_percent: 99.5,
            max_error_rate_percent: 2.0,
            throughput_per_second: 10000,
        },
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

/// Decision Service Contract Template
pub fn create_decision_service_contract(
    service_name: String,
    domain: String,
    strategies: Vec<String>,
) -> ServiceContract {
    let service_domain = match domain.as_str() {
        "trading" => ServiceDomain::TradingDecision,
        "system-ops" => ServiceDomain::SystemOpsDecision,
        _ => ServiceDomain::TradingDecision, // Default
    };

    ServiceContract {
        name: service_name.clone(),
        domain: service_domain,
        version: ContractVersion::new(1, 0, 0),
        description: format!("Decision service for {} domain with strategies: {}", domain, strategies.join(", ")),
        capabilities: vec![
            ServiceCapability {
                name: "decision_making".to_string(),
                description: "Make autonomous decisions based on data".to_string(),
                required: true,
                version: ContractVersion::new(1, 0, 0),
            },
            ServiceCapability {
                name: "strategy_voting".to_string(),
                description: "Consensus voting between strategies".to_string(),
                required: true,
                version: ContractVersion::new(1, 0, 0),
            },
        ],
        dependencies: vec![
            ContractDependency {
                service_name: "core-data-platform".to_string(),
                domain: ServiceDomain::CoreDataPlatform,
                min_version: ContractVersion::new(1, 0, 0),
                max_version: None,
                required: true,
            },
        ],
        input_schemas: {
            let mut schemas = HashMap::new();
            schemas.insert(
                format!("data.{}.*.processed", domain),
                r#"{"type": "object", "properties": {"symbol": {"type": "string"}, "price": {"type": "number"}}}"#.to_string(),
            );
            schemas.insert(
                format!("features.{}.{}", domain, "*"),
                r#"{"type": "object", "properties": {"indicator": {"type": "string"}, "value": {"type": "number"}}}"#.to_string(),
            );
            schemas
        },
        output_schemas: {
            let mut schemas = HashMap::new();
            for strategy in &strategies {
                schemas.insert(
                    format!("decisions.{}.{}", domain, strategy),
                    r#"{"type": "object", "properties": {"action": {"type": "string"}, "confidence": {"type": "number"}, "reasoning": {"type": "string"}}}"#.to_string(),
                );
            }
            schemas
        },
        error_codes: {
            let mut codes = HashMap::new();
            codes.insert("DECISION_001".to_string(), "Insufficient data for decision".to_string());
            codes.insert("DECISION_002".to_string(), "Strategy consensus failed".to_string());
            codes.insert("DECISION_003".to_string(), "Confidence threshold not met".to_string());
            codes
        },
        sla_requirements: SlaRequirements {
            max_latency_ms: 100,
            min_availability_percent: 99.9,
            max_error_rate_percent: 0.5,
            throughput_per_second: 1000,
        },
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_version_compatibility() {
        let v1_0_0 = ContractVersion::new(1, 0, 0);
        let v1_1_0 = ContractVersion::new(1, 1, 0);
        let v1_0_1 = ContractVersion::new(1, 0, 1);
        let v2_0_0 = ContractVersion::new(2, 0, 0);

        assert!(v1_1_0.is_compatible_with(&v1_0_0));
        assert!(v1_0_1.is_compatible_with(&v1_0_0));
        assert!(!v1_0_0.is_compatible_with(&v1_1_0));
        assert!(!v2_0_0.is_compatible_with(&v1_0_0));
        assert!(!v1_0_0.is_compatible_with(&v2_0_0));
    }

    #[test]
    fn test_service_domain_interactions() {
        assert!(ServiceDomain::DataIngestion.can_interact_with(&ServiceDomain::CoreDataPlatform));
        assert!(!ServiceDomain::DataIngestion.can_interact_with(&ServiceDomain::TradingDecision));
        
        assert!(ServiceDomain::TradingDecision.can_interact_with(&ServiceDomain::TradingExecution));
        assert!(!ServiceDomain::TradingDecision.can_interact_with(&ServiceDomain::SystemOpsExecution));
        
        // All domains can interact with observability
        assert!(ServiceDomain::DataIngestion.can_interact_with(&ServiceDomain::Observability));
        assert!(ServiceDomain::TradingDecision.can_interact_with(&ServiceDomain::Observability));
    }

    #[test]
    fn test_contract_registry() {
        let mut registry = ContractRegistry::new();

        let contract = create_data_ingestion_contract(
            "test-ingestion".to_string(),
            vec!["alpaca".to_string(), "binance".to_string()],
            "trading".to_string(),
        );

        assert!(registry.register_contract(contract).is_ok());
        assert!(registry.get_contract("test-ingestion").is_some());
    }

    #[test]
    fn test_decision_service_contract() {
        let contract = create_decision_service_contract(
            "trading-decision".to_string(),
            "trading".to_string(),
            vec!["momentum".to_string(), "mean-reversion".to_string()],
        );

        assert_eq!(contract.domain, ServiceDomain::TradingDecision);
        assert_eq!(contract.capabilities.len(), 2);
        assert!(contract.output_schemas.contains_key("decisions.trading.momentum"));
        assert!(contract.output_schemas.contains_key("decisions.trading.mean-reversion"));
    }

    #[test]
    fn test_domain_stream_patterns() {
        let trading_decision = ServiceDomain::TradingDecision;
        let input_patterns = trading_decision.allowed_input_patterns();
        let output_patterns = trading_decision.allowed_output_patterns();

        assert!(input_patterns.contains(&"data.trading.*.processed".to_string()));
        assert!(input_patterns.contains(&"features.trading.*".to_string()));
        assert!(output_patterns.contains(&"decisions.trading.*".to_string()));
    }
}