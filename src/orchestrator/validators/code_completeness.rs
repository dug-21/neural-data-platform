//! Code Completeness Validator
//! 
//! This validator enforces ZERO tolerance for incomplete implementations.
//! It scans the entire codebase for stub functions, TODO comments, 
//! and other incomplete implementations that should never reach production.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
    #[error("Validation failed: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationViolation {
    pub file: PathBuf,
    pub line_number: usize,
    pub line_content: String,
    pub violation_type: ViolationType,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ViolationType {
    TodoMacro,
    UnimplementedMacro,
    PanicNotImplemented,
    EmptyOkReturn,
    EmptyFunctionBody,
    MockInProduction,
    TestDoubleInProduction,
    PlaceholderValue,
    IncompleteErrorHandling,
    MissingValidation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    Critical,  // Blocks deployment
    High,      // Must be fixed before release
    Medium,    // Should be fixed
    Low,       // Nice to fix
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub passed: bool,
    pub score: f64,
    pub total_files_scanned: usize,
    pub total_lines_scanned: usize,
    pub violations: Vec<ValidationViolation>,
    pub summary: ValidationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub critical_violations: usize,
    pub high_violations: usize,
    pub medium_violations: usize,
    pub low_violations: usize,
    pub binaries_validated: HashMap<String, BinaryValidationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryValidationResult {
    pub binary_name: String,
    pub files_scanned: usize,
    pub violations: usize,
    pub passed: bool,
    pub specific_violations: Vec<ViolationType>,
}

pub struct CodeCompletenessValidator {
    forbidden_patterns: Vec<ForbiddenPattern>,
    binary_paths: HashMap<String, PathBuf>,
    exclusions: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct ForbiddenPattern {
    name: String,
    regex: Regex,
    violation_type: ViolationType,
    severity: Severity,
    message_template: String,
}

impl CodeCompletenessValidator {
    pub fn new() -> Result<Self, ValidationError> {
        let forbidden_patterns = Self::create_forbidden_patterns()?;
        let binary_paths = Self::detect_binary_paths();
        let exclusions = Self::create_exclusions();
        
        Ok(Self {
            forbidden_patterns,
            binary_paths,
            exclusions,
        })
    }
    
    /// Create patterns for forbidden code constructs
    fn create_forbidden_patterns() -> Result<Vec<ForbiddenPattern>, ValidationError> {
        let patterns = vec![
            // Rust TODO macros
            ForbiddenPattern {
                name: "todo_macro".to_string(),
                regex: Regex::new(r"todo!\s*\(")?,
                violation_type: ViolationType::TodoMacro,
                severity: Severity::Critical,
                message_template: "TODO macro found - implementation incomplete".to_string(),
            },
            
            // Rust unimplemented macros  
            ForbiddenPattern {
                name: "unimplemented_macro".to_string(),
                regex: Regex::new(r"unimplemented!\s*\(")?,
                violation_type: ViolationType::UnimplementedMacro,
                severity: Severity::Critical,
                message_template: "unimplemented! macro found - missing implementation".to_string(),
            },
            
            // Panic with not implemented
            ForbiddenPattern {
                name: "panic_not_implemented".to_string(),
                regex: Regex::new(r#"panic!\s*\(\s*["'].*not implemented.*["']\s*\)"#)?,
                violation_type: ViolationType::PanicNotImplemented,
                severity: Severity::Critical,
                message_template: "Panic with 'not implemented' message found".to_string(),
            },
            
            // Empty Ok returns (likely stubs)
            ForbiddenPattern {
                name: "empty_ok_return".to_string(),
                regex: Regex::new(r"^\s*Ok\s*\(\s*\(\s*\)\s*\)\s*$")?,
                violation_type: ViolationType::EmptyOkReturn,
                severity: Severity::High,
                message_template: "Empty Ok(()) return - likely stub implementation".to_string(),
            },
            
            // Python TODO/FIXME comments in production paths
            ForbiddenPattern {
                name: "python_todo".to_string(),
                regex: Regex::new(r"#\s*(TODO|FIXME|XXX).*")?,
                violation_type: ViolationType::TodoMacro,
                severity: Severity::High,
                message_template: "TODO/FIXME comment found in Python code".to_string(),
            },
            
            // Mock services in production config
            ForbiddenPattern {
                name: "mock_in_production".to_string(),
                regex: Regex::new(r"(Mock|Fake|Stub)[A-Z]\w+")?,
                violation_type: ViolationType::MockInProduction,
                severity: Severity::Critical,
                message_template: "Mock/Fake/Stub service found in production code".to_string(),
            },
            
            // Test doubles in production
            ForbiddenPattern {
                name: "test_double_production".to_string(),
                regex: Regex::new(r"(test_double|TestDouble)")?,
                violation_type: ViolationType::TestDoubleInProduction,
                severity: Severity::Critical,
                message_template: "Test double found in production code".to_string(),
            },
            
            // Placeholder values
            ForbiddenPattern {
                name: "placeholder_values".to_string(),
                regex: Regex::new(r"(PLACEHOLDER|placeholder|CHANGEME|changeme|FIXME)")?,
                violation_type: ViolationType::PlaceholderValue,
                severity: Severity::Medium,
                message_template: "Placeholder value found".to_string(),
            },
            
            // Missing error handling patterns
            ForbiddenPattern {
                name: "unwrap_in_production".to_string(),
                regex: Regex::new(r"\.unwrap\(\)")?,
                violation_type: ViolationType::IncompleteErrorHandling,
                severity: Severity::High,
                message_template: "unwrap() found - missing proper error handling".to_string(),
            },
        ];
        
        Ok(patterns)
    }
    
    /// Detect binary paths for Phase 3 architecture
    fn detect_binary_paths() -> HashMap<String, PathBuf> {
        let mut paths = HashMap::new();
        
        // Phase 3 binary structure
        paths.insert("config-store".to_string(), PathBuf::from("src/config-store"));
        paths.insert("data-ingestion".to_string(), PathBuf::from("src/data-ingestion"));
        paths.insert("ruv-fann".to_string(), PathBuf::from("src/ruv-fann"));
        paths.insert("daa-coordinator".to_string(), PathBuf::from("src/daa-coordinator"));
        
        // Shared components
        paths.insert("shared".to_string(), PathBuf::from("src/shared"));
        paths.insert("common".to_string(), PathBuf::from("src/common"));
        
        paths
    }
    
    /// Create exclusion patterns for test files and development tools
    fn create_exclusions() -> Vec<PathBuf> {
        vec![
            PathBuf::from("tests/"),
            PathBuf::from(".git/"),
            PathBuf::from("target/"),
            PathBuf::from("node_modules/"),
            PathBuf::from("__pycache__/"),
            PathBuf::from(".pytest_cache/"),
            PathBuf::from("build/"),
            PathBuf::from("dist/"),
        ]
    }
    
    /// Validate code completeness across all binaries
    pub fn validate(&self, root_path: &Path) -> Result<ValidationResult, ValidationError> {
        let mut violations = Vec::new();
        let mut total_files = 0;
        let mut total_lines = 0;
        let mut binary_results = HashMap::new();
        
        // Validate each binary separately
        for (binary_name, binary_path) in &self.binary_paths {
            let full_path = root_path.join(binary_path);
            
            if full_path.exists() {
                let binary_result = self.validate_binary(&full_path, binary_name)?;
                violations.extend(binary_result.violations.clone());
                total_files += binary_result.files_scanned;
                
                binary_results.insert(binary_name.clone(), BinaryValidationResult {
                    binary_name: binary_name.clone(),
                    files_scanned: binary_result.files_scanned,
                    violations: binary_result.violations.len(),
                    passed: binary_result.violations.is_empty(),
                    specific_violations: binary_result.violations
                        .iter()
                        .map(|v| v.violation_type.clone())
                        .collect(),
                });
            }
        }
        
        // Calculate summary statistics
        let summary = ValidationSummary {
            critical_violations: violations.iter()
                .filter(|v| v.severity == Severity::Critical)
                .count(),
            high_violations: violations.iter()
                .filter(|v| v.severity == Severity::High)
                .count(),
            medium_violations: violations.iter()
                .filter(|v| v.severity == Severity::Medium)
                .count(),
            low_violations: violations.iter()
                .filter(|v| v.severity == Severity::Low)
                .count(),
            binaries_validated: binary_results,
        };
        
        // Calculate overall score
        let score = self.calculate_score(&summary, total_files);
        
        // Determine if validation passed (no critical or high violations)
        let passed = summary.critical_violations == 0 && summary.high_violations == 0;
        
        Ok(ValidationResult {
            passed,
            score,
            total_files_scanned: total_files,
            total_lines_scanned: total_lines,
            violations,
            summary,
        })
    }
    
    /// Validate a specific binary
    fn validate_binary(
        &self, 
        binary_path: &Path, 
        binary_name: &str
    ) -> Result<BinaryValidationResult, ValidationError> {
        let mut violations = Vec::new();
        let mut files_scanned = 0;
        
        // Get all source files in the binary
        let source_files = self.get_source_files(binary_path)?;
        
        for file_path in source_files {
            if self.should_exclude_file(&file_path) {
                continue;
            }
            
            files_scanned += 1;
            let file_violations = self.validate_file(&file_path)?;
            violations.extend(file_violations);
        }
        
        Ok(BinaryValidationResult {
            binary_name: binary_name.to_string(),
            files_scanned,
            violations: violations.len(),
            passed: violations.is_empty(),
            specific_violations: violations
                .iter()
                .map(|v| v.violation_type.clone())
                .collect(),
        })
    }
    
    /// Validate a single file
    fn validate_file(&self, file_path: &Path) -> Result<Vec<ValidationViolation>, ValidationError> {
        let content = fs::read_to_string(file_path)?;
        let lines: Vec<&str> = content.lines().collect();
        let mut violations = Vec::new();
        
        for (line_number, line) in lines.iter().enumerate() {
            for pattern in &self.forbidden_patterns {
                if pattern.regex.is_match(line) {
                    violations.push(ValidationViolation {
                        file: file_path.to_path_buf(),
                        line_number: line_number + 1, // 1-indexed
                        line_content: line.to_string(),
                        violation_type: pattern.violation_type.clone(),
                        severity: pattern.severity.clone(),
                        message: pattern.message_template.clone(),
                    });
                }
            }
        }
        
        // Additional file-level validations
        violations.extend(self.validate_file_structure(&content, file_path)?);
        
        Ok(violations)
    }
    
    /// Validate file structure and implementation completeness
    fn validate_file_structure(
        &self, 
        content: &str, 
        file_path: &Path
    ) -> Result<Vec<ValidationViolation>, ValidationError> {
        let mut violations = Vec::new();
        
        // Check for empty trait implementations
        if file_path.extension().and_then(|s| s.to_str()) == Some("rs") {
            violations.extend(self.check_empty_trait_implementations(content, file_path)?);
            violations.extend(self.check_incomplete_error_handling(content, file_path)?);
            violations.extend(self.check_missing_validation(content, file_path)?);
        }
        
        // Check Python-specific patterns
        if file_path.extension().and_then(|s| s.to_str()) == Some("py") {
            violations.extend(self.check_python_implementations(content, file_path)?);
        }
        
        Ok(violations)
    }
    
    /// Check for empty trait implementations in Rust
    fn check_empty_trait_implementations(
        &self,
        content: &str,
        file_path: &Path
    ) -> Result<Vec<ValidationViolation>, ValidationError> {
        let mut violations = Vec::new();
        
        // Pattern for trait implementations with empty bodies
        let impl_pattern = Regex::new(r"impl\s+\w+\s+for\s+\w+\s*\{([^}]*)\}")?;
        
        for captures in impl_pattern.captures_iter(content) {
            let impl_body = captures.get(1).unwrap().as_str().trim();
            
            // If implementation body is empty or only contains comments
            if impl_body.is_empty() || 
               impl_body.lines().all(|line| line.trim().is_empty() || line.trim().starts_with("//")) {
                violations.push(ValidationViolation {
                    file: file_path.to_path_buf(),
                    line_number: 1, // Would need more complex parsing for exact line
                    line_content: captures.get(0).unwrap().as_str().to_string(),
                    violation_type: ViolationType::EmptyFunctionBody,
                    severity: Severity::Critical,
                    message: "Empty trait implementation found".to_string(),
                });
            }
        }
        
        Ok(violations)
    }
    
    /// Check for incomplete error handling patterns
    fn check_incomplete_error_handling(
        &self,
        content: &str,
        file_path: &Path
    ) -> Result<Vec<ValidationViolation>, ValidationError> {
        let mut violations = Vec::new();
        
        // Look for functions that return Results but don't handle errors properly
        let result_fn_pattern = Regex::new(
            r"fn\s+\w+\s*\([^)]*\)\s*->\s*Result<[^>]+,\s*[^>]+>\s*\{[^}]*\}"
        )?;
        
        for captures in result_fn_pattern.captures_iter(content) {
            let fn_body = captures.get(0).unwrap().as_str();
            
            // Check if function uses unwrap(), expect(), or other unsafe error handling
            if fn_body.contains(".unwrap()") || fn_body.contains(".expect(") {
                violations.push(ValidationViolation {
                    file: file_path.to_path_buf(),
                    line_number: 1,
                    line_content: "Function with unsafe error handling".to_string(),
                    violation_type: ViolationType::IncompleteErrorHandling,
                    severity: Severity::High,
                    message: "Function returning Result uses unsafe error handling".to_string(),
                });
            }
        }
        
        Ok(violations)
    }
    
    /// Check for missing input validation
    fn check_missing_validation(
        &self,
        content: &str,
        file_path: &Path
    ) -> Result<Vec<ValidationViolation>, ValidationError> {
        let mut violations = Vec::new();
        
        // Check for public functions that don't validate inputs
        let pub_fn_pattern = Regex::new(r"pub\s+fn\s+(\w+)\s*\([^)]*\)")?;
        
        for captures in pub_fn_pattern.captures_iter(content) {
            let fn_name = captures.get(1).unwrap().as_str();
            
            // Skip test functions and certain safe functions
            if fn_name.starts_with("test_") || 
               fn_name.starts_with("new") ||
               fn_name == "default" {
                continue;
            }
            
            // Look for validation patterns in the function
            let fn_start = captures.get(0).unwrap().start();
            let remaining_content = &content[fn_start..];
            
            // Find the function body (simplified - would need proper parsing)
            if let Some(body_start) = remaining_content.find('{') {
                let body_content = &remaining_content[body_start..];
                
                // Check for validation patterns
                let has_validation = body_content.contains(".validate(") ||
                                   body_content.contains("validate_") ||
                                   body_content.contains("if ") ||
                                   body_content.contains("match ");
                
                if !has_validation && body_content.len() > 50 {
                    violations.push(ValidationViolation {
                        file: file_path.to_path_buf(),
                        line_number: 1,
                        line_content: format!("pub fn {} lacks input validation", fn_name),
                        violation_type: ViolationType::MissingValidation,
                        severity: Severity::Medium,
                        message: format!("Public function '{}' may lack input validation", fn_name),
                    });
                }
            }
        }
        
        Ok(violations)
    }
    
    /// Check Python-specific implementation patterns
    fn check_python_implementations(
        &self,
        content: &str,
        file_path: &Path
    ) -> Result<Vec<ValidationViolation>, ValidationError> {
        let mut violations = Vec::new();
        
        // Check for pass statements in production code
        let pass_pattern = Regex::new(r"^\s*pass\s*$")?;
        
        for (line_number, line) in content.lines().enumerate() {
            if pass_pattern.is_match(line) {
                violations.push(ValidationViolation {
                    file: file_path.to_path_buf(),
                    line_number: line_number + 1,
                    line_content: line.to_string(),
                    violation_type: ViolationType::EmptyFunctionBody,
                    severity: Severity::High,
                    message: "Empty implementation with 'pass' statement".to_string(),
                });
            }
        }
        
        // Check for NotImplementedError raises
        let not_impl_pattern = Regex::new(r"raise\s+NotImplementedError")?;
        
        for (line_number, line) in content.lines().enumerate() {
            if not_impl_pattern.is_match(line) {
                violations.push(ValidationViolation {
                    file: file_path.to_path_buf(),
                    line_number: line_number + 1,
                    line_content: line.to_string(),
                    violation_type: ViolationType::UnimplementedMacro,
                    severity: Severity::Critical,
                    message: "NotImplementedError found - implementation incomplete".to_string(),
                });
            }
        }
        
        Ok(violations)
    }
    
    /// Get all source files in a directory recursively
    fn get_source_files(&self, dir: &Path) -> Result<Vec<PathBuf>, ValidationError> {
        let mut files = Vec::new();
        
        if dir.is_file() {
            files.push(dir.to_path_buf());
            return Ok(files);
        }
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() && !self.should_exclude_file(&path) {
                files.extend(self.get_source_files(&path)?);
            } else if path.is_file() && self.is_source_file(&path) {
                files.push(path);
            }
        }
        
        Ok(files)
    }
    
    /// Check if a file should be excluded from validation
    fn should_exclude_file(&self, file_path: &Path) -> bool {
        for exclusion in &self.exclusions {
            if file_path.starts_with(exclusion) ||
               file_path.to_string_lossy().contains(&exclusion.to_string_lossy().to_string()) {
                return true;
            }
        }
        
        false
    }
    
    /// Check if a file is a source file we want to validate
    fn is_source_file(&self, file_path: &Path) -> bool {
        if let Some(extension) = file_path.extension().and_then(|s| s.to_str()) {
            matches!(extension, "rs" | "py" | "toml" | "yaml" | "yml" | "json")
        } else {
            false
        }
    }
    
    /// Calculate validation score based on violations and file count
    fn calculate_score(&self, summary: &ValidationSummary, total_files: usize) -> f64 {
        if total_files == 0 {
            return 100.0;
        }
        
        let total_violations = summary.critical_violations + 
                              summary.high_violations + 
                              summary.medium_violations + 
                              summary.low_violations;
        
        if total_violations == 0 {
            return 100.0;
        }
        
        // Weighted scoring: critical violations have highest impact
        let weighted_violations = (summary.critical_violations * 4) +
                                 (summary.high_violations * 2) +
                                 (summary.medium_violations * 1) +
                                 (summary.low_violations * 0);
        
        let max_possible_score = total_files * 4; // Assuming worst case all critical
        let score = 100.0 - ((weighted_violations as f64 / max_possible_score as f64) * 100.0);
        
        score.max(0.0).min(100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    
    #[test]
    fn test_detects_todo_macro() {
        let validator = CodeCompletenessValidator::new().unwrap();
        
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "fn test() { todo!(\"implement this\") }").unwrap();
        
        let violations = validator.validate_file(&file_path).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].violation_type, ViolationType::TodoMacro);
        assert_eq!(violations[0].severity, Severity::Critical);
    }
    
    #[test]
    fn test_detects_unimplemented_macro() {
        let validator = CodeCompletenessValidator::new().unwrap();
        
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "fn test() { unimplemented!() }").unwrap();
        
        let violations = validator.validate_file(&file_path).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].violation_type, ViolationType::UnimplementedMacro);
        assert_eq!(violations[0].severity, Severity::Critical);
    }
    
    #[test]
    fn test_detects_mock_in_production() {
        let validator = CodeCompletenessValidator::new().unwrap();
        
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, "let service = MockTradingService::new();").unwrap();
        
        let violations = validator.validate_file(&file_path).unwrap();
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].violation_type, ViolationType::MockInProduction);
        assert_eq!(violations[0].severity, Severity::Critical);
    }
    
    #[test]
    fn test_clean_code_passes() {
        let validator = CodeCompletenessValidator::new().unwrap();
        
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        fs::write(&file_path, r#"
            fn calculate_trading_signal(data: &MarketData) -> Result<Signal, Error> {
                data.validate()?;
                let prediction = self.model.predict(data)?;
                Ok(Signal::new(prediction))
            }
        "#).unwrap();
        
        let violations = validator.validate_file(&file_path).unwrap();
        assert_eq!(violations.len(), 0);
    }
    
    #[test]
    fn test_python_not_implemented_error() {
        let validator = CodeCompletenessValidator::new().unwrap();
        
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        fs::write(&file_path, "def process_data():\n    raise NotImplementedError()").unwrap();
        
        let violations = validator.validate_file(&file_path).unwrap();
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.violation_type == ViolationType::UnimplementedMacro));
    }
    
    #[test]
    fn test_scoring_system() {
        let validator = CodeCompletenessValidator::new().unwrap();
        
        let summary = ValidationSummary {
            critical_violations: 1,
            high_violations: 2,
            medium_violations: 3,
            low_violations: 4,
            binaries_validated: HashMap::new(),
        };
        
        let score = validator.calculate_score(&summary, 10);
        assert!(score < 100.0);
        assert!(score > 0.0);
    }
}