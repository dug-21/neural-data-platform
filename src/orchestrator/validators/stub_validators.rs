// Stub Validators for Testing Compilation
// These are minimal implementations to ensure the framework compiles

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Validation result for any validator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub validator_name: String,
    pub status: ValidationStatus,
    pub score: f64,
    pub message: String,
    pub details: Vec<ValidationDetail>,
    pub execution_time_ms: u64,
    pub timestamp: DateTime<Utc>,
}

/// Validation status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationStatus {
    Passed,
    Failed,
    Warning,
    Critical,
}

/// Detailed validation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationDetail {
    pub category: String,
    pub file_path: Option<String>,
    pub line_number: Option<usize>,
    pub severity: String,
    pub message: String,
    pub suggestion: Option<String>,
}

// Stub validators for compilation testing

pub struct CodeCompletenessValidator;

impl CodeCompletenessValidator {
    pub fn new() -> Result<Self, String> {
        Ok(Self)
    }
    
    pub fn validate(&self, _project_root: &Path) -> Result<ValidationResult, String> {
        Ok(ValidationResult {
            validator_name: "code-completeness".to_string(),
            status: ValidationStatus::Passed,
            score: 95.0,
            message: "Code completeness validation passed".to_string(),
            details: vec![],
            execution_time_ms: 500,
            timestamp: Utc::now(),
        })
    }
}

pub struct InterfaceContractValidator;

impl InterfaceContractValidator {
    pub fn new(_proto_paths: Vec<PathBuf>, _impl_paths: Vec<PathBuf>) -> Result<Self, String> {
        Ok(Self)
    }
    
    pub fn validate(&self) -> Result<ValidationResult, String> {
        Ok(ValidationResult {
            validator_name: "interface-contract".to_string(),
            status: ValidationStatus::Passed,
            score: 92.0,
            message: "Interface contract validation passed".to_string(),
            details: vec![],
            execution_time_ms: 800,
            timestamp: Utc::now(),
        })
    }
}

pub struct TestCoverageValidator;

impl TestCoverageValidator {
    pub fn new(_project_root: &Path) -> Self {
        Self
    }
    
    pub async fn validate(&self, _project_root: &Path) -> Result<ValidationResult, String> {
        Ok(ValidationResult {
            validator_name: "test-coverage".to_string(),
            status: ValidationStatus::Passed,
            score: 96.5,
            message: "Test coverage validation passed".to_string(),
            details: vec![],
            execution_time_ms: 2000,
            timestamp: Utc::now(),
        })
    }
}

pub struct PerformanceBenchmarkValidator;

impl PerformanceBenchmarkValidator {
    pub fn new() -> Self {
        Self
    }
    
    pub async fn validate(&self, _project_root: &Path) -> Result<ValidationResult, String> {
        Ok(ValidationResult {
            validator_name: "performance-benchmark".to_string(),
            status: ValidationStatus::Passed,
            score: 88.0,
            message: "Performance benchmark validation passed".to_string(),
            details: vec![],
            execution_time_ms: 5000,
            timestamp: Utc::now(),
        })
    }
}

pub struct SecurityStandardsValidator;

impl SecurityStandardsValidator {
    pub fn new() -> Result<Self, String> {
        Ok(Self)
    }
    
    pub async fn validate(&self, _project_root: &Path) -> Result<ValidationResult, String> {
        Ok(ValidationResult {
            validator_name: "security-standards".to_string(),
            status: ValidationStatus::Passed,
            score: 94.0,
            message: "Security standards validation passed".to_string(),
            details: vec![],
            execution_time_ms: 3000,
            timestamp: Utc::now(),
        })
    }
}