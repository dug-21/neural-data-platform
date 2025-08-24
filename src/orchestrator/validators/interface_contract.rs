//! Interface Contract Validator
//! 
//! This validator ensures that all interface contracts are fully implemented
//! and compliant with their specifications. It validates gRPC services,
//! Redis Streams integration, and API contract adherence.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContractValidationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
    #[error("Proto parsing error: {0}")]
    ProtoParsing(String),
    #[error("Contract violation: {0}")]
    ContractViolation(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractValidationResult {
    pub passed: bool,
    pub score: f64,
    pub total_contracts_checked: usize,
    pub violations: Vec<ContractViolation>,
    pub summary: ContractValidationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractViolation {
    pub contract_name: String,
    pub violation_type: ContractViolationType,
    pub severity: ContractSeverity,
    pub message: String,
    pub file_path: Option<PathBuf>,
    pub line_number: Option<usize>,
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContractViolationType {
    MissingGrpcMethod,
    IncompleteGrpcImplementation,
    MissingErrorHandling,
    InvalidMessageField,
    MissingStreamHandling,
    RedisStreamSchemaViolation,
    BackpressureNotHandled,
    TimeoutNotConfigured,
    CircuitBreakerMissing,
    RetryLogicMissing,
    ValidationMissing,
    SerializationError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContractSeverity {
    Critical,  // Breaks contract compatibility
    High,      // Degraded functionality
    Medium,    // Best practice violation
    Low,       // Style/convention issue
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractValidationSummary {
    pub grpc_services_checked: usize,
    pub grpc_methods_validated: usize,
    pub redis_streams_validated: usize,
    pub critical_violations: usize,
    pub high_violations: usize,
    pub medium_violations: usize,
    pub low_violations: usize,
    pub contract_compliance_by_service: HashMap<String, ServiceComplianceResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceComplianceResult {
    pub service_name: String,
    pub total_methods: usize,
    pub implemented_methods: usize,
    pub compliance_percentage: f64,
    pub missing_methods: Vec<String>,
    pub violations: Vec<ContractViolation>,
}

#[derive(Debug, Clone)]
pub struct GrpcServiceContract {
    pub service_name: String,
    pub methods: Vec<GrpcMethodContract>,
    pub proto_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GrpcMethodContract {
    pub method_name: String,
    pub request_type: String,
    pub response_type: String,
    pub is_streaming: bool,
    pub is_client_streaming: bool,
    pub is_server_streaming: bool,
}

#[derive(Debug, Clone)]
pub struct RedisStreamContract {
    pub stream_name: String,
    pub schema: HashMap<String, String>,
    pub consumer_groups: Vec<String>,
    pub required_fields: Vec<String>,
}

pub struct InterfaceContractValidator {
    grpc_contracts: Vec<GrpcServiceContract>,
    redis_contracts: Vec<RedisStreamContract>,
    proto_paths: Vec<PathBuf>,
    implementation_paths: Vec<PathBuf>,
}

impl InterfaceContractValidator {
    pub fn new(proto_paths: Vec<PathBuf>, implementation_paths: Vec<PathBuf>) -> Result<Self, ContractValidationError> {
        let grpc_contracts = Self::discover_grpc_contracts(&proto_paths)?;
        let redis_contracts = Self::discover_redis_contracts(&implementation_paths)?;
        
        Ok(Self {
            grpc_contracts,
            redis_contracts,
            proto_paths,
            implementation_paths,
        })
    }
    
    /// Discover gRPC service contracts from .proto files
    fn discover_grpc_contracts(proto_paths: &[PathBuf]) -> Result<Vec<GrpcServiceContract>, ContractValidationError> {
        let mut contracts = Vec::new();
        
        for proto_path in proto_paths {
            if proto_path.is_file() && proto_path.extension().map_or(false, |ext| ext == "proto") {
                contracts.extend(Self::parse_proto_file(proto_path)?);
            } else if proto_path.is_dir() {
                contracts.extend(Self::discover_proto_files_in_dir(proto_path)?);
            }
        }
        
        Ok(contracts)
    }
    
    /// Parse a single .proto file to extract service contracts
    fn parse_proto_file(proto_file: &Path) -> Result<Vec<GrpcServiceContract>, ContractValidationError> {
        let content = fs::read_to_string(proto_file)?;
        let mut contracts = Vec::new();
        
        // Parse service definitions
        let service_pattern = Regex::new(r"service\s+(\w+)\s*\{([^}]*)\}")?;
        
        for service_match in service_pattern.captures_iter(&content) {
            let service_name = service_match.get(1).unwrap().as_str().to_string();
            let service_body = service_match.get(2).unwrap().as_str();
            
            let methods = Self::parse_service_methods(service_body)?;
            
            contracts.push(GrpcServiceContract {
                service_name,
                methods,
                proto_file: proto_file.to_path_buf(),
            });
        }
        
        Ok(contracts)
    }
    
    /// Parse service methods from service body
    fn parse_service_methods(service_body: &str) -> Result<Vec<GrpcMethodContract>, ContractValidationError> {
        let mut methods = Vec::new();
        
        // Match RPC method definitions
        let rpc_pattern = Regex::new(
            r"rpc\s+(\w+)\s*\(\s*(stream\s+)?(\w+)\s*\)\s*returns\s*\(\s*(stream\s+)?(\w+)\s*\)\s*;"
        )?;
        
        for method_match in rpc_pattern.captures_iter(service_body) {
            let method_name = method_match.get(1).unwrap().as_str().to_string();
            let is_client_streaming = method_match.get(2).is_some();
            let request_type = method_match.get(3).unwrap().as_str().to_string();
            let is_server_streaming = method_match.get(4).is_some();
            let response_type = method_match.get(5).unwrap().as_str().to_string();
            
            methods.push(GrpcMethodContract {
                method_name,
                request_type,
                response_type,
                is_streaming: is_client_streaming || is_server_streaming,
                is_client_streaming,
                is_server_streaming,
            });
        }
        
        Ok(methods)
    }
    
    /// Discover .proto files in a directory
    fn discover_proto_files_in_dir(dir: &Path) -> Result<Vec<GrpcServiceContract>, ContractValidationError> {
        let mut contracts = Vec::new();
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "proto") {
                contracts.extend(Self::parse_proto_file(&path)?);
            } else if path.is_dir() {
                contracts.extend(Self::discover_proto_files_in_dir(&path)?);
            }
        }
        
        Ok(contracts)
    }
    
    /// Discover Redis Stream contracts from implementation files
    fn discover_redis_contracts(implementation_paths: &[PathBuf]) -> Result<Vec<RedisStreamContract>, ContractValidationError> {
        let mut contracts = Vec::new();
        
        for impl_path in implementation_paths {
            contracts.extend(Self::extract_redis_contracts(impl_path)?);
        }
        
        Ok(contracts)
    }
    
    /// Extract Redis Stream contracts from implementation files
    fn extract_redis_contracts(path: &Path) -> Result<Vec<RedisStreamContract>, ContractValidationError> {
        let mut contracts = Vec::new();
        
        if path.is_file() {
            contracts.extend(Self::parse_redis_contracts_from_file(path)?);
        } else if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                contracts.extend(Self::extract_redis_contracts(&entry.path())?);
            }
        }
        
        Ok(contracts)
    }
    
    /// Parse Redis Stream contracts from a single file
    fn parse_redis_contracts_from_file(file_path: &Path) -> Result<Vec<RedisStreamContract>, ContractValidationError> {
        let content = fs::read_to_string(file_path)?;
        let mut contracts = Vec::new();
        
        // Look for Redis Stream definitions
        // This is a simplified parser - real implementation would be more sophisticated
        let stream_pattern = Regex::new(r#"stream_name\s*[:=]\s*["']([^"']+)["']"#)?;
        
        for stream_match in stream_pattern.captures_iter(&content) {
            let stream_name = stream_match.get(1).unwrap().as_str().to_string();
            
            // Extract schema information (simplified)
            let contract = RedisStreamContract {
                stream_name,
                schema: HashMap::new(), // Would be extracted from actual usage
                consumer_groups: Vec::new(),
                required_fields: Vec::new(),
            };
            
            contracts.push(contract);
        }
        
        Ok(contracts)
    }
    
    /// Validate all interface contracts
    pub fn validate(&self) -> Result<ContractValidationResult, ContractValidationError> {
        let mut violations = Vec::new();\n        let mut service_compliance = HashMap::new();\n        \n        // Validate gRPC service implementations\n        for contract in &self.grpc_contracts {\n            let compliance = self.validate_grpc_service(contract)?;\n            violations.extend(compliance.violations.clone());\n            service_compliance.insert(contract.service_name.clone(), compliance);\n        }\n        \n        // Validate Redis Stream implementations\n        for contract in &self.redis_contracts {\n            violations.extend(self.validate_redis_stream(contract)?);\n        }\n        \n        let summary = ContractValidationSummary {\n            grpc_services_checked: self.grpc_contracts.len(),\n            grpc_methods_validated: self.grpc_contracts.iter()\n                .map(|c| c.methods.len())\n                .sum(),\n            redis_streams_validated: self.redis_contracts.len(),\n            critical_violations: violations.iter()\n                .filter(|v| v.severity == ContractSeverity::Critical)\n                .count(),\n            high_violations: violations.iter()\n                .filter(|v| v.severity == ContractSeverity::High)\n                .count(),\n            medium_violations: violations.iter()\n                .filter(|v| v.severity == ContractSeverity::Medium)\n                .count(),\n            low_violations: violations.iter()\n                .filter(|v| v.severity == ContractSeverity::Low)\n                .count(),\n            contract_compliance_by_service: service_compliance,\n        };\n        \n        let score = self.calculate_compliance_score(&summary);\n        let passed = summary.critical_violations == 0 && summary.high_violations == 0;\n        \n        Ok(ContractValidationResult {\n            passed,\n            score,\n            total_contracts_checked: self.grpc_contracts.len() + self.redis_contracts.len(),\n            violations,\n            summary,\n        })\n    }\n    \n    /// Validate a gRPC service implementation against its contract\n    fn validate_grpc_service(&self, contract: &GrpcServiceContract) -> Result<ServiceComplianceResult, ContractValidationError> {\n        let mut violations = Vec::new();\n        let mut implemented_methods = 0;\n        let mut missing_methods = Vec::new();\n        \n        // Find implementation files for this service\n        let impl_files = self.find_service_implementation_files(&contract.service_name)?;\n        \n        for method in &contract.methods {\n            if self.is_method_implemented(&impl_files, &method.method_name)? {\n                implemented_methods += 1;\n                \n                // Validate method implementation quality\n                violations.extend(self.validate_method_implementation(\n                    &impl_files, \n                    method,\n                    &contract.service_name\n                )?);\n            } else {\n                missing_methods.push(method.method_name.clone());\n                violations.push(ContractViolation {\n                    contract_name: contract.service_name.clone(),\n                    violation_type: ContractViolationType::MissingGrpcMethod,\n                    severity: ContractSeverity::Critical,\n                    message: format!(\"Method '{}' is not implemented\", method.method_name),\n                    file_path: None,\n                    line_number: None,\n                    suggested_fix: Some(format!(\n                        \"Implement the '{}' method in the service implementation\",\n                        method.method_name\n                    )),\n                });\n            }\n        }\n        \n        let compliance_percentage = if contract.methods.is_empty() {\n            100.0\n        } else {\n            (implemented_methods as f64 / contract.methods.len() as f64) * 100.0\n        };\n        \n        Ok(ServiceComplianceResult {\n            service_name: contract.service_name.clone(),\n            total_methods: contract.methods.len(),\n            implemented_methods,\n            compliance_percentage,\n            missing_methods,\n            violations,\n        })\n    }\n    \n    /// Find implementation files for a service\n    fn find_service_implementation_files(&self, service_name: &str) -> Result<Vec<PathBuf>, ContractValidationError> {\n        let mut impl_files = Vec::new();\n        \n        for impl_path in &self.implementation_paths {\n            impl_files.extend(self.search_for_service_impl(impl_path, service_name)?);\n        }\n        \n        Ok(impl_files)\n    }\n    \n    /// Search for service implementation files\n    fn search_for_service_impl(&self, path: &Path, service_name: &str) -> Result<Vec<PathBuf>, ContractValidationError> {\n        let mut files = Vec::new();\n        \n        if path.is_file() {\n            let content = fs::read_to_string(path)?;\n            \n            // Look for service implementation patterns\n            let impl_pattern = format!(r\"impl.*{}.*Service\", service_name);\n            let regex = Regex::new(&impl_pattern)?;\n            \n            if regex.is_match(&content) {\n                files.push(path.to_path_buf());\n            }\n        } else if path.is_dir() {\n            for entry in fs::read_dir(path)? {\n                let entry = entry?;\n                files.extend(self.search_for_service_impl(&entry.path(), service_name)?);\n            }\n        }\n        \n        Ok(files)\n    }\n    \n    /// Check if a method is implemented\n    fn is_method_implemented(&self, impl_files: &[PathBuf], method_name: &str) -> Result<bool, ContractValidationError> {\n        for file in impl_files {\n            let content = fs::read_to_string(file)?;\n            \n            // Look for method implementation\n            let method_pattern = format!(r\"(?:async\\s+)?fn\\s+{}\\s*\\(\", method_name);\n            let regex = Regex::new(&method_pattern)?;\n            \n            if regex.is_match(&content) {\n                return Ok(true);\n            }\n        }\n        \n        Ok(false)\n    }\n    \n    /// Validate method implementation quality\n    fn validate_method_implementation(\n        &self, \n        impl_files: &[PathBuf], \n        method: &GrpcMethodContract,\n        service_name: &str\n    ) -> Result<Vec<ContractViolation>, ContractValidationError> {\n        let mut violations = Vec::new();\n        \n        for file in impl_files {\n            let content = fs::read_to_string(file)?;\n            violations.extend(self.check_method_error_handling(file, &content, method, service_name)?);\n            violations.extend(self.check_method_validation(file, &content, method, service_name)?);\n            \n            if method.is_streaming {\n                violations.extend(self.check_streaming_implementation(file, &content, method, service_name)?);\n            }\n        }\n        \n        Ok(violations)\n    }\n    \n    /// Check method error handling\n    fn check_method_error_handling(\n        &self,\n        file_path: &Path,\n        content: &str,\n        method: &GrpcMethodContract,\n        service_name: &str\n    ) -> Result<Vec<ContractViolation>, ContractValidationError> {\n        let mut violations = Vec::new();\n        \n        // Find method implementation\n        let method_pattern = format!(\n            r\"(?:async\\s+)?fn\\s+{}\\s*\\([^{{]*\\)\\s*->[^{{]*\\{{([^{{}}]*(?:\\{{[^{{}}]*\\}}[^{{}}]*)*?)\\}}\",\n            method.method_name\n        );\n        let regex = Regex::new(&method_pattern)?;\n        \n        if let Some(method_match) = regex.captures(content) {\n            let method_body = method_match.get(1).unwrap().as_str();\n            \n            // Check for proper gRPC error handling\n            if !method_body.contains(\"Status::\") && !method_body.contains(\".map_err\") {\n                violations.push(ContractViolation {\n                    contract_name: service_name.to_string(),\n                    violation_type: ContractViolationType::MissingErrorHandling,\n                    severity: ContractSeverity::High,\n                    message: format!(\n                        \"Method '{}' lacks proper gRPC error handling\", \n                        method.method_name\n                    ),\n                    file_path: Some(file_path.to_path_buf()),\n                    line_number: None,\n                    suggested_fix: Some(\n                        \"Add proper error handling with gRPC Status codes\".to_string()\n                    ),\n                });\n            }\n            \n            // Check for timeout handling\n            if !method_body.contains(\"timeout\") && !method_body.contains(\"deadline\") {\n                violations.push(ContractViolation {\n                    contract_name: service_name.to_string(),\n                    violation_type: ContractViolationType::TimeoutNotConfigured,\n                    severity: ContractSeverity::Medium,\n                    message: format!(\n                        \"Method '{}' does not handle timeouts\", \n                        method.method_name\n                    ),\n                    file_path: Some(file_path.to_path_buf()),\n                    line_number: None,\n                    suggested_fix: Some(\n                        \"Add timeout handling for long-running operations\".to_string()\n                    ),\n                });\n            }\n        }\n        \n        Ok(violations)\n    }\n    \n    /// Check method input validation\n    fn check_method_validation(\n        &self,\n        file_path: &Path,\n        content: &str,\n        method: &GrpcMethodContract,\n        service_name: &str\n    ) -> Result<Vec<ContractViolation>, ContractValidationError> {\n        let mut violations = Vec::new();\n        \n        // Find method implementation\n        let method_pattern = format!(\n            r\"(?:async\\s+)?fn\\s+{}\\s*\\([^{{]*request[^{{]*\\)\\s*->[^{{]*\\{{([^{{}}]*(?:\\{{[^{{}}]*\\}}[^{{}}]*)*?)\\}}\",\n            method.method_name\n        );\n        let regex = Regex::new(&method_pattern)?;\n        \n        if let Some(method_match) = regex.captures(content) {\n            let method_body = method_match.get(1).unwrap().as_str();\n            \n            // Check for input validation\n            if !method_body.contains(\".validate\") && !method_body.contains(\"validate_\") {\n                violations.push(ContractViolation {\n                    contract_name: service_name.to_string(),\n                    violation_type: ContractViolationType::ValidationMissing,\n                    severity: ContractSeverity::Medium,\n                    message: format!(\n                        \"Method '{}' does not validate input\", \n                        method.method_name\n                    ),\n                    file_path: Some(file_path.to_path_buf()),\n                    line_number: None,\n                    suggested_fix: Some(\n                        \"Add input validation for request parameters\".to_string()\n                    ),\n                });\n            }\n        }\n        \n        Ok(violations)\n    }\n    \n    /// Check streaming implementation\n    fn check_streaming_implementation(\n        &self,\n        file_path: &Path,\n        content: &str,\n        method: &GrpcMethodContract,\n        service_name: &str\n    ) -> Result<Vec<ContractViolation>, ContractValidationError> {\n        let mut violations = Vec::new();\n        \n        // Find method implementation\n        let method_pattern = format!(\n            r\"(?:async\\s+)?fn\\s+{}\\s*\\([^{{]*\\)\\s*->[^{{]*\\{{([^{{}}]*(?:\\{{[^{{}}]*\\}}[^{{}}]*)*?)\\}}\",\n            method.method_name\n        );\n        let regex = Regex::new(&method_pattern)?;\n        \n        if let Some(method_match) = regex.captures(content) {\n            let method_body = method_match.get(1).unwrap().as_str();\n            \n            // Check for backpressure handling\n            if method.is_server_streaming && !method_body.contains(\"backpressure\") {\n                violations.push(ContractViolation {\n                    contract_name: service_name.to_string(),\n                    violation_type: ContractViolationType::BackpressureNotHandled,\n                    severity: ContractSeverity::High,\n                    message: format!(\n                        \"Streaming method '{}' does not handle backpressure\", \n                        method.method_name\n                    ),\n                    file_path: Some(file_path.to_path_buf()),\n                    line_number: None,\n                    suggested_fix: Some(\n                        \"Implement backpressure handling for streaming responses\".to_string()\n                    ),\n                });\n            }\n        }\n        \n        Ok(violations)\n    }\n    \n    /// Validate Redis Stream implementation\n    fn validate_redis_stream(&self, contract: &RedisStreamContract) -> Result<Vec<ContractViolation>, ContractValidationError> {\n        let mut violations = Vec::new();\n        \n        // Find Redis Stream usage in implementation files\n        for impl_path in &self.implementation_paths {\n            violations.extend(self.check_redis_stream_usage(impl_path, contract)?);\n        }\n        \n        Ok(violations)\n    }\n    \n    /// Check Redis Stream usage implementation\n    fn check_redis_stream_usage(\n        &self,\n        path: &Path,\n        contract: &RedisStreamContract\n    ) -> Result<Vec<ContractViolation>, ContractValidationError> {\n        let mut violations = Vec::new();\n        \n        if path.is_file() {\n            let content = fs::read_to_string(path)?;\n            \n            // Check if the stream is used\n            if content.contains(&contract.stream_name) {\n                // Check for proper error handling\n                if !content.contains(\"redis\") || !content.contains(\"Error\") {\n                    violations.push(ContractViolation {\n                        contract_name: contract.stream_name.clone(),\n                        violation_type: ContractViolationType::MissingErrorHandling,\n                        severity: ContractSeverity::High,\n                        message: format!(\n                            \"Redis Stream '{}' usage lacks error handling\", \n                            contract.stream_name\n                        ),\n                        file_path: Some(path.to_path_buf()),\n                        line_number: None,\n                        suggested_fix: Some(\n                            \"Add proper error handling for Redis operations\".to_string()\n                        ),\n                    });\n                }\n                \n                // Check for serialization handling\n                if !content.contains(\"serialize\") && !content.contains(\"serde\") {\n                    violations.push(ContractViolation {\n                        contract_name: contract.stream_name.clone(),\n                        violation_type: ContractViolationType::SerializationError,\n                        severity: ContractSeverity::Medium,\n                        message: format!(\n                            \"Redis Stream '{}' may lack proper serialization\", \n                            contract.stream_name\n                        ),\n                        file_path: Some(path.to_path_buf()),\n                        line_number: None,\n                        suggested_fix: Some(\n                            \"Ensure proper message serialization/deserialization\".to_string()\n                        ),\n                    });\n                }\n            }\n        } else if path.is_dir() {\n            for entry in fs::read_dir(path)? {\n                let entry = entry?;\n                violations.extend(self.check_redis_stream_usage(&entry.path(), contract)?);\n            }\n        }\n        \n        Ok(violations)\n    }\n    \n    /// Calculate overall compliance score\n    fn calculate_compliance_score(&self, summary: &ContractValidationSummary) -> f64 {\n        if summary.grpc_services_checked == 0 {\n            return 100.0;\n        }\n        \n        let total_violations = summary.critical_violations + \n                              summary.high_violations + \n                              summary.medium_violations + \n                              summary.low_violations;\n        \n        if total_violations == 0 {\n            return 100.0;\n        }\n        \n        // Weighted scoring\n        let weighted_violations = (summary.critical_violations * 4) +\n                                 (summary.high_violations * 2) +\n                                 (summary.medium_violations * 1);\n        \n        let max_possible_violations = summary.grpc_methods_validated * 4;\n        \n        if max_possible_violations == 0 {\n            return 100.0;\n        }\n        \n        let score = 100.0 - ((weighted_violations as f64 / max_possible_violations as f64) * 100.0);\n        score.max(0.0).min(100.0)\n    }\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n    use std::fs;\n    use tempfile::TempDir;\n    \n    #[test]\n    fn test_proto_parsing() {\n        let temp_dir = TempDir::new().unwrap();\n        let proto_file = temp_dir.path().join(\"test.proto\");\n        \n        fs::write(&proto_file, r#\"\n            service TestService {\n                rpc GetData(DataRequest) returns (DataResponse);\n                rpc StreamData(stream DataRequest) returns (stream DataResponse);\n            }\n        \"#).unwrap();\n        \n        let contracts = InterfaceContractValidator::parse_proto_file(&proto_file).unwrap();\n        assert_eq!(contracts.len(), 1);\n        assert_eq!(contracts[0].service_name, \"TestService\");\n        assert_eq!(contracts[0].methods.len(), 2);\n        \n        let get_data = &contracts[0].methods[0];\n        assert_eq!(get_data.method_name, \"GetData\");\n        assert!(!get_data.is_streaming);\n        \n        let stream_data = &contracts[0].methods[1];\n        assert_eq!(stream_data.method_name, \"StreamData\");\n        assert!(stream_data.is_streaming);\n        assert!(stream_data.is_client_streaming);\n        assert!(stream_data.is_server_streaming);\n    }\n    \n    #[test]\n    fn test_service_implementation_detection() {\n        let temp_dir = TempDir::new().unwrap();\n        let impl_file = temp_dir.path().join(\"service.rs\");\n        \n        fs::write(&impl_file, r#\"\n            impl TestService for TestServiceImpl {\n                async fn get_data(&self, request: Request<DataRequest>) -> Result<Response<DataResponse>, Status> {\n                    Ok(Response::new(DataResponse::default()))\n                }\n            }\n        \"#).unwrap();\n        \n        let validator = InterfaceContractValidator::new(\n            vec![],\n            vec![temp_dir.path().to_path_buf()]\n        ).unwrap();\n        \n        let impl_files = validator.find_service_implementation_files(\"TestService\").unwrap();\n        assert_eq!(impl_files.len(), 1);\n        \n        let is_implemented = validator.is_method_implemented(&impl_files, \"get_data\").unwrap();\n        assert!(is_implemented);\n        \n        let not_implemented = validator.is_method_implemented(&impl_files, \"missing_method\").unwrap();\n        assert!(!not_implemented);\n    }\n}"}