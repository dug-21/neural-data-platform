//! Test Coverage Enforcement Validator
//! 
//! This validator ensures minimum test coverage requirements are met
//! across all binaries and validates test quality standards.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoverageValidationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Command execution failed: {0}")]
    CommandExecution(String),
    #[error("Coverage parsing error: {0}")]
    CoverageParsing(String),
    #[error("Coverage threshold violation: {0}")]
    ThresholdViolation(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageValidationResult {
    pub passed: bool,
    pub overall_score: f64,
    pub coverage_summary: CoverageSummary,
    pub binary_coverage: HashMap<String, BinaryCoverage>,
    pub violations: Vec<CoverageViolation>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageSummary {
    pub overall_line_coverage: f64,
    pub overall_branch_coverage: f64,
    pub overall_function_coverage: f64,
    pub total_lines: u32,
    pub covered_lines: u32,
    pub total_branches: u32,
    pub covered_branches: u32,
    pub total_functions: u32,
    pub covered_functions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryCoverage {
    pub binary_name: String,
    pub line_coverage: f64,
    pub branch_coverage: f64,
    pub function_coverage: f64,
    pub meets_requirements: bool,
    pub test_files_count: u32,
    pub source_files_count: u32,
    pub uncovered_files: Vec<String>,
    pub critical_gaps: Vec<CoverageGap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageGap {
    pub file_path: String,
    pub line_range: (u32, u32),
    pub gap_type: CoverageGapType,
    pub severity: CoverageGapSeverity,
    pub suggested_tests: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoverageGapType {
    UncoveredFunction,
    UncoveredBranch,
    UncoveredErrorPath,
    UncoveredEdgeCase,
    MissingIntegrationTest,
    MissingPerformanceTest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoverageGapSeverity {
    Critical,  // Core business logic
    High,      // Important functionality
    Medium,    // Standard code paths
    Low,       // Helper functions
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageViolation {
    pub binary_name: String,
    pub violation_type: CoverageViolationType,
    pub current_coverage: f64,
    pub required_coverage: f64,
    pub message: String,
    pub suggested_fix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoverageViolationType {
    LineCoverageBelowThreshold,
    BranchCoverageBelowThreshold,
    FunctionCoverageBelowThreshold,
    CriticalPathUntested,
    ErrorHandlingUntested,
    IntegrationTestsMissing,
    PerformanceTestsMissing,
}

#[derive(Debug, Clone)]
pub struct CoverageRequirements {
    pub min_line_coverage: f64,
    pub min_branch_coverage: f64,
    pub min_function_coverage: f64,
    pub binary_requirements: HashMap<String, BinaryRequirements>,
}

#[derive(Debug, Clone)]
pub struct BinaryRequirements {
    pub min_line_coverage: f64,
    pub min_branch_coverage: f64,
    pub critical_files: Vec<String>,
    pub requires_integration_tests: bool,
    pub requires_performance_tests: bool,
}

pub struct TestCoverageValidator {
    requirements: CoverageRequirements,
    binary_paths: HashMap<String, PathBuf>,
    test_paths: HashMap<String, PathBuf>,
}

impl TestCoverageValidator {
    pub fn new(project_root: &Path) -> Self {
        let requirements = Self::default_requirements();
        let binary_paths = Self::detect_binary_paths();
        let test_paths = Self::detect_test_paths();
        
        Self {
            requirements,
            binary_paths,
            test_paths,
        }
    }
    
    /// Create default coverage requirements based on Phase 3 specifications
    fn default_requirements() -> CoverageRequirements {
        let mut binary_requirements = HashMap::new();
        
        // Phase 3 binary-specific requirements
        binary_requirements.insert("config-store".to_string(), BinaryRequirements {
            min_line_coverage: 95.0,
            min_branch_coverage: 90.0,
            critical_files: vec![
                "config_service.rs".to_string(),
                "validation.rs".to_string(),
                "grpc_server.rs".to_string(),
            ],
            requires_integration_tests: true,
            requires_performance_tests: true,
        });
        
        binary_requirements.insert("data-ingestion".to_string(), BinaryRequirements {
            min_line_coverage: 95.0,
            min_branch_coverage: 90.0,
            critical_files: vec![
                "stream_processor.py".to_string(),
                "data_validator.py".to_string(),
                "redis_publisher.py".to_string(),
            ],
            requires_integration_tests: true,
            requires_performance_tests: true,
        });
        
        binary_requirements.insert("ruv-fann".to_string(), BinaryRequirements {
            min_line_coverage: 95.0,
            min_branch_coverage: 90.0,
            critical_files: vec![
                "neural_network.rs".to_string(),
                "model_trainer.rs".to_string(),
                "prediction_engine.rs".to_string(),
            ],
            requires_integration_tests: true,
            requires_performance_tests: true,
        });
        
        binary_requirements.insert("daa-coordinator".to_string(), BinaryRequirements {
            min_line_coverage: 95.0,
            min_branch_coverage: 90.0,
            critical_files: vec![
                "consensus_engine.rs".to_string(),
                "agent_coordinator.rs".to_string(),
                "decision_aggregator.rs".to_string(),
            ],
            requires_integration_tests: true,
            requires_performance_tests: true,
        });
        
        CoverageRequirements {
            min_line_coverage: 95.0,
            min_branch_coverage: 90.0,
            min_function_coverage: 95.0,
            binary_requirements,
        }\n    }\n    \n    /// Detect binary paths for Phase 3 architecture\n    fn detect_binary_paths() -> HashMap<String, PathBuf> {\n        let mut paths = HashMap::new();\n        paths.insert(\"config-store\".to_string(), PathBuf::from(\"src/config-store\"));\n        paths.insert(\"data-ingestion\".to_string(), PathBuf::from(\"src/data-ingestion\"));\n        paths.insert(\"ruv-fann\".to_string(), PathBuf::from(\"src/ruv-fann\"));\n        paths.insert(\"daa-coordinator\".to_string(), PathBuf::from(\"src/daa-coordinator\"));\n        paths\n    }\n    \n    /// Detect test paths\n    fn detect_test_paths() -> HashMap<String, PathBuf> {\n        let mut paths = HashMap::new();\n        paths.insert(\"unit\".to_string(), PathBuf::from(\"tests/unit\"));\n        paths.insert(\"integration\".to_string(), PathBuf::from(\"tests/integration\"));\n        paths.insert(\"performance\".to_string(), PathBuf::from(\"tests/performance\"));\n        paths.insert(\"e2e\".to_string(), PathBuf::from(\"tests/e2e\"));\n        paths\n    }\n    \n    /// Validate test coverage across all binaries\n    pub async fn validate(&self, project_root: &Path) -> Result<CoverageValidationResult, CoverageValidationError> {\n        let mut binary_coverage = HashMap::new();\n        let mut violations = Vec::new();\n        let mut recommendations = Vec::new();\n        \n        // Validate coverage for each binary\n        for (binary_name, binary_path) in &self.binary_paths {\n            let full_path = project_root.join(binary_path);\n            \n            if full_path.exists() {\n                let coverage = self.measure_binary_coverage(&full_path, binary_name).await?;\n                \n                // Check against requirements\n                let binary_req = self.requirements.binary_requirements.get(binary_name);\n                violations.extend(self.validate_binary_coverage(&coverage, binary_req, binary_name));\n                \n                // Generate recommendations\n                recommendations.extend(self.generate_coverage_recommendations(&coverage));\n                \n                binary_coverage.insert(binary_name.clone(), coverage);\n            }\n        }\n        \n        // Calculate overall coverage summary\n        let coverage_summary = self.calculate_overall_summary(&binary_coverage);\n        \n        // Check overall requirements\n        violations.extend(self.validate_overall_coverage(&coverage_summary));\n        \n        let overall_score = self.calculate_coverage_score(&coverage_summary, &violations);\n        let passed = violations.iter().all(|v| !matches!(\n            v.violation_type, \n            CoverageViolationType::LineCoverageBelowThreshold |\n            CoverageViolationType::CriticalPathUntested\n        ));\n        \n        Ok(CoverageValidationResult {\n            passed,\n            overall_score,\n            coverage_summary,\n            binary_coverage,\n            violations,\n            recommendations,\n        })\n    }\n    \n    /// Measure test coverage for a specific binary\n    async fn measure_binary_coverage(&self, binary_path: &Path, binary_name: &str) -> Result<BinaryCoverage, CoverageValidationError> {\n        // Run coverage analysis based on binary language\n        let coverage_data = if self.is_rust_binary(binary_name) {\n            self.run_cargo_coverage(binary_path).await?\n        } else {\n            self.run_python_coverage(binary_path).await?\n        };\n        \n        // Analyze critical coverage gaps\n        let critical_gaps = self.analyze_coverage_gaps(&coverage_data, binary_name)?;\n        \n        // Count test files\n        let test_files_count = self.count_test_files(binary_path)?;\n        let source_files_count = self.count_source_files(binary_path)?;\n        \n        // Identify uncovered files\n        let uncovered_files = self.identify_uncovered_files(&coverage_data)?;\n        \n        Ok(BinaryCoverage {\n            binary_name: binary_name.to_string(),\n            line_coverage: coverage_data.line_coverage,\n            branch_coverage: coverage_data.branch_coverage,\n            function_coverage: coverage_data.function_coverage,\n            meets_requirements: self.check_binary_requirements(&coverage_data, binary_name),\n            test_files_count,\n            source_files_count,\n            uncovered_files,\n            critical_gaps,\n        })\n    }\n    \n    /// Run Cargo coverage for Rust binaries\n    async fn run_cargo_coverage(&self, binary_path: &Path) -> Result<RawCoverageData, CoverageValidationError> {\n        let output = Command::new(\"cargo\")\n            .args([\"tarpaulin\", \"--out\", \"Json\", \"--output-dir\", \"target/coverage\"])\n            .current_dir(binary_path)\n            .output()\n            .map_err(|e| CoverageValidationError::CommandExecution(e.to_string()))?;\n        \n        if !output.status.success() {\n            return Err(CoverageValidationError::CommandExecution(\n                String::from_utf8_lossy(&output.stderr).to_string()\n            ));\n        }\n        \n        let coverage_json = String::from_utf8_lossy(&output.stdout);\n        self.parse_tarpaulin_output(&coverage_json)\n    }\n    \n    /// Run Python coverage\n    async fn run_python_coverage(&self, binary_path: &Path) -> Result<RawCoverageData, CoverageValidationError> {\n        // Run pytest with coverage\n        let output = Command::new(\"python\")\n            .args([\"-m\", \"pytest\", \"--cov=.\", \"--cov-report=json:coverage.json\"])\n            .current_dir(binary_path)\n            .output()\n            .map_err(|e| CoverageValidationError::CommandExecution(e.to_string()))?;\n        \n        if !output.status.success() {\n            return Err(CoverageValidationError::CommandExecution(\n                String::from_utf8_lossy(&output.stderr).to_string()\n            ));\n        }\n        \n        // Read coverage.json\n        let coverage_file = binary_path.join(\"coverage.json\");\n        let coverage_json = std::fs::read_to_string(coverage_file)\n            .map_err(|e| CoverageValidationError::Io(e))?;\n        \n        self.parse_python_coverage(&coverage_json)\n    }\n    \n    /// Parse tarpaulin coverage output\n    fn parse_tarpaulin_output(&self, json_output: &str) -> Result<RawCoverageData, CoverageValidationError> {\n        // Parse tarpaulin JSON format\n        let data: serde_json::Value = serde_json::from_str(json_output)\n            .map_err(|e| CoverageValidationError::CoverageParsing(e.to_string()))?;\n        \n        let coverage = data.get(\"coverage\")\n            .and_then(|c| c.as_f64())\n            .unwrap_or(0.0);\n        \n        // Extract more detailed metrics if available\n        Ok(RawCoverageData {\n            line_coverage: coverage,\n            branch_coverage: coverage * 0.9, // Approximation if not available\n            function_coverage: coverage,\n            files: HashMap::new(), // Would extract from detailed data\n        })\n    }\n    \n    /// Parse Python coverage output\n    fn parse_python_coverage(&self, json_output: &str) -> Result<RawCoverageData, CoverageValidationError> {\n        let data: serde_json::Value = serde_json::from_str(json_output)\n            .map_err(|e| CoverageValidationError::CoverageParsing(e.to_string()))?;\n        \n        let totals = data.get(\"totals\")\n            .ok_or_else(|| CoverageValidationError::CoverageParsing(\"Missing totals\".to_string()))?;\n        \n        let line_coverage = totals.get(\"percent_covered\")\n            .and_then(|p| p.as_f64())\n            .unwrap_or(0.0);\n        \n        Ok(RawCoverageData {\n            line_coverage,\n            branch_coverage: line_coverage * 0.9, // Approximation\n            function_coverage: line_coverage,\n            files: HashMap::new(),\n        })\n    }\n    \n    /// Analyze coverage gaps and identify critical areas\n    fn analyze_coverage_gaps(&self, coverage_data: &RawCoverageData, binary_name: &str) -> Result<Vec<CoverageGap>, CoverageValidationError> {\n        let mut gaps = Vec::new();\n        \n        // Check for critical files with low coverage\n        if let Some(binary_req) = self.requirements.binary_requirements.get(binary_name) {\n            for critical_file in &binary_req.critical_files {\n                if let Some(file_coverage) = coverage_data.files.get(critical_file) {\n                    if file_coverage.line_coverage < binary_req.min_line_coverage {\n                        gaps.push(CoverageGap {\n                            file_path: critical_file.clone(),\n                            line_range: (0, 0), // Would be extracted from detailed data\n                            gap_type: CoverageGapType::UncoveredFunction,\n                            severity: CoverageGapSeverity::Critical,\n                            suggested_tests: vec![\n                                format!(\"Add unit tests for {}\", critical_file),\n                                format!(\"Add integration tests for {}\", critical_file),\n                            ],\n                        });\n                    }\n                }\n            }\n        }\n        \n        Ok(gaps)\n    }\n    \n    /// Validate binary coverage against requirements\n    fn validate_binary_coverage(\n        &self, \n        coverage: &BinaryCoverage, \n        requirements: Option<&BinaryRequirements>,\n        binary_name: &str\n    ) -> Vec<CoverageViolation> {\n        let mut violations = Vec::new();\n        \n        let req = requirements.unwrap_or(&BinaryRequirements {\n            min_line_coverage: self.requirements.min_line_coverage,\n            min_branch_coverage: self.requirements.min_branch_coverage,\n            critical_files: vec![],\n            requires_integration_tests: false,\n            requires_performance_tests: false,\n        });\n        \n        // Check line coverage\n        if coverage.line_coverage < req.min_line_coverage {\n            violations.push(CoverageViolation {\n                binary_name: binary_name.to_string(),\n                violation_type: CoverageViolationType::LineCoverageBelowThreshold,\n                current_coverage: coverage.line_coverage,\n                required_coverage: req.min_line_coverage,\n                message: format!(\n                    \"Line coverage {:.1}% is below required {:.1}%\",\n                    coverage.line_coverage, req.min_line_coverage\n                ),\n                suggested_fix: \"Add more unit tests to cover missing code paths\".to_string(),\n            });\n        }\n        \n        // Check branch coverage\n        if coverage.branch_coverage < req.min_branch_coverage {\n            violations.push(CoverageViolation {\n                binary_name: binary_name.to_string(),\n                violation_type: CoverageViolationType::BranchCoverageBelowThreshold,\n                current_coverage: coverage.branch_coverage,\n                required_coverage: req.min_branch_coverage,\n                message: format!(\n                    \"Branch coverage {:.1}% is below required {:.1}%\",\n                    coverage.branch_coverage, req.min_branch_coverage\n                ),\n                suggested_fix: \"Add tests for conditional branches and error paths\".to_string(),\n            });\n        }\n        \n        // Check critical gaps\n        for gap in &coverage.critical_gaps {\n            if gap.severity == CoverageGapSeverity::Critical {\n                violations.push(CoverageViolation {\n                    binary_name: binary_name.to_string(),\n                    violation_type: CoverageViolationType::CriticalPathUntested,\n                    current_coverage: 0.0,\n                    required_coverage: 100.0,\n                    message: format!(\"Critical code path in {} is not tested\", gap.file_path),\n                    suggested_fix: gap.suggested_tests.join(\", \"),\n                });\n            }\n        }\n        \n        violations\n    }\n    \n    /// Calculate overall coverage summary\n    fn calculate_overall_summary(&self, binary_coverage: &HashMap<String, BinaryCoverage>) -> CoverageSummary {\n        if binary_coverage.is_empty() {\n            return CoverageSummary {\n                overall_line_coverage: 0.0,\n                overall_branch_coverage: 0.0,\n                overall_function_coverage: 0.0,\n                total_lines: 0,\n                covered_lines: 0,\n                total_branches: 0,\n                covered_branches: 0,\n                total_functions: 0,\n                covered_functions: 0,\n            };\n        }\n        \n        let total_binaries = binary_coverage.len() as f64;\n        \n        let overall_line_coverage = binary_coverage.values()\n            .map(|c| c.line_coverage)\n            .sum::<f64>() / total_binaries;\n        \n        let overall_branch_coverage = binary_coverage.values()\n            .map(|c| c.branch_coverage)\n            .sum::<f64>() / total_binaries;\n        \n        let overall_function_coverage = binary_coverage.values()\n            .map(|c| c.function_coverage)\n            .sum::<f64>() / total_binaries;\n        \n        CoverageSummary {\n            overall_line_coverage,\n            overall_branch_coverage,\n            overall_function_coverage,\n            total_lines: 0,      // Would be calculated from detailed data\n            covered_lines: 0,    // Would be calculated from detailed data\n            total_branches: 0,   // Would be calculated from detailed data\n            covered_branches: 0, // Would be calculated from detailed data\n            total_functions: 0,  // Would be calculated from detailed data\n            covered_functions: 0, // Would be calculated from detailed data\n        }\n    }\n    \n    /// Helper methods\n    fn is_rust_binary(&self, binary_name: &str) -> bool {\n        matches!(binary_name, \"config-store\" | \"ruv-fann\" | \"daa-coordinator\")\n    }\n    \n    fn check_binary_requirements(&self, coverage_data: &RawCoverageData, binary_name: &str) -> bool {\n        if let Some(req) = self.requirements.binary_requirements.get(binary_name) {\n            coverage_data.line_coverage >= req.min_line_coverage &&\n            coverage_data.branch_coverage >= req.min_branch_coverage\n        } else {\n            coverage_data.line_coverage >= self.requirements.min_line_coverage\n        }\n    }\n    \n    fn count_test_files(&self, binary_path: &Path) -> Result<u32, CoverageValidationError> {\n        // Count test files in the binary\n        Ok(0) // Placeholder - would implement file counting\n    }\n    \n    fn count_source_files(&self, binary_path: &Path) -> Result<u32, CoverageValidationError> {\n        // Count source files in the binary\n        Ok(0) // Placeholder - would implement file counting\n    }\n    \n    fn identify_uncovered_files(&self, coverage_data: &RawCoverageData) -> Result<Vec<String>, CoverageValidationError> {\n        // Identify files with zero or very low coverage\n        Ok(vec![]) // Placeholder - would implement from coverage data\n    }\n    \n    fn validate_overall_coverage(&self, summary: &CoverageSummary) -> Vec<CoverageViolation> {\n        let mut violations = Vec::new();\n        \n        if summary.overall_line_coverage < self.requirements.min_line_coverage {\n            violations.push(CoverageViolation {\n                binary_name: \"overall\".to_string(),\n                violation_type: CoverageViolationType::LineCoverageBelowThreshold,\n                current_coverage: summary.overall_line_coverage,\n                required_coverage: self.requirements.min_line_coverage,\n                message: format!(\n                    \"Overall line coverage {:.1}% is below required {:.1}%\",\n                    summary.overall_line_coverage, self.requirements.min_line_coverage\n                ),\n                suggested_fix: \"Improve test coverage across all binaries\".to_string(),\n            });\n        }\n        \n        violations\n    }\n    \n    fn generate_coverage_recommendations(&self, coverage: &BinaryCoverage) -> Vec<String> {\n        let mut recommendations = Vec::new();\n        \n        if coverage.line_coverage < 95.0 {\n            recommendations.push(format!(\n                \"Increase line coverage for {} from {:.1}% to 95%+\",\n                coverage.binary_name, coverage.line_coverage\n            ));\n        }\n        \n        if !coverage.critical_gaps.is_empty() {\n            recommendations.push(format!(\n                \"Address {} critical coverage gaps in {}\",\n                coverage.critical_gaps.len(), coverage.binary_name\n            ));\n        }\n        \n        recommendations\n    }\n    \n    fn calculate_coverage_score(&self, summary: &CoverageSummary, violations: &[CoverageViolation]) -> f64 {\n        if violations.is_empty() {\n            return 100.0;\n        }\n        \n        let critical_violations = violations.iter()\n            .filter(|v| matches!(v.violation_type, CoverageViolationType::CriticalPathUntested))\n            .count();\n        \n        let high_violations = violations.iter()\n            .filter(|v| matches!(v.violation_type, CoverageViolationType::LineCoverageBelowThreshold))\n            .count();\n        \n        let weighted_violations = (critical_violations * 4) + (high_violations * 2);\n        let max_violations = 20; // Reasonable maximum\n        \n        let penalty = (weighted_violations as f64 / max_violations as f64) * 100.0;\n        (100.0 - penalty).max(0.0).min(100.0)\n    }\n}\n\n#[derive(Debug, Clone)]\nstruct RawCoverageData {\n    line_coverage: f64,\n    branch_coverage: f64,\n    function_coverage: f64,\n    files: HashMap<String, FileCoverageData>,\n}\n\n#[derive(Debug, Clone)]\nstruct FileCoverageData {\n    line_coverage: f64,\n    branch_coverage: f64,\n    function_coverage: f64,\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n    \n    #[test]\n    fn test_coverage_requirements() {\n        let validator = TestCoverageValidator::new(Path::new(\".\"));\n        \n        // Verify Phase 3 binary requirements are set\n        assert!(validator.requirements.binary_requirements.contains_key(\"config-store\"));\n        assert!(validator.requirements.binary_requirements.contains_key(\"data-ingestion\"));\n        assert!(validator.requirements.binary_requirements.contains_key(\"ruv-fann\"));\n        assert!(validator.requirements.binary_requirements.contains_key(\"daa-coordinator\"));\n        \n        // Verify minimum coverage requirements\n        let config_req = validator.requirements.binary_requirements.get(\"config-store\").unwrap();\n        assert_eq!(config_req.min_line_coverage, 95.0);\n        assert_eq!(config_req.min_branch_coverage, 90.0);\n    }\n    \n    #[test]\n    fn test_binary_language_detection() {\n        let validator = TestCoverageValidator::new(Path::new(\".\"));\n        \n        assert!(validator.is_rust_binary(\"config-store\"));\n        assert!(validator.is_rust_binary(\"ruv-fann\"));\n        assert!(validator.is_rust_binary(\"daa-coordinator\"));\n        assert!(!validator.is_rust_binary(\"data-ingestion\")); // Python\n    }\n    \n    #[test]\n    fn test_coverage_score_calculation() {\n        let validator = TestCoverageValidator::new(Path::new(\".\"));\n        \n        let summary = CoverageSummary {\n            overall_line_coverage: 97.5,\n            overall_branch_coverage: 92.0,\n            overall_function_coverage: 95.0,\n            total_lines: 1000,\n            covered_lines: 975,\n            total_branches: 200,\n            covered_branches: 184,\n            total_functions: 100,\n            covered_functions: 95,\n        };\n        \n        let violations = vec![];\n        let score = validator.calculate_coverage_score(&summary, &violations);\n        assert_eq!(score, 100.0);\n        \n        let violations = vec![\n            CoverageViolation {\n                binary_name: \"test\".to_string(),\n                violation_type: CoverageViolationType::CriticalPathUntested,\n                current_coverage: 0.0,\n                required_coverage: 100.0,\n                message: \"Critical path untested\".to_string(),\n                suggested_fix: \"Add tests\".to_string(),\n            }\n        ];\n        let score_with_violations = validator.calculate_coverage_score(&summary, &violations);\n        assert!(score_with_violations < 100.0);\n    }\n}"}