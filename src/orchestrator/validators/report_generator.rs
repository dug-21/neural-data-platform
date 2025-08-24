// Report Generator for Phase 3 Production Validation
// Generates comprehensive validation reports in multiple formats

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use chrono::{DateTime, Utc};

/// Report format options
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReportFormat {
    Console,
    Json,
    Html,
    Markdown,
    Csv,
    Xml,
}

/// Overall validation status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationStatus {
    Passed,
    Failed,
    Warning,
    Critical,
}

/// Individual validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub validator_name: String,
    pub status: ValidationStatus,
    pub score: f64, // 0.0 to 100.0
    pub message: String,
    pub details: Vec<ValidationDetail>,
    pub execution_time_ms: u64,
    pub timestamp: DateTime<Utc>,
}

/// Detailed validation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationDetail {
    pub category: String,
    pub file_path: Option<String>,
    pub line_number: Option<usize>,
    pub severity: String, // "error", "warning", "info"
    pub message: String,
    pub suggestion: Option<String>,
}

/// Comprehensive validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub metadata: ReportMetadata,
    pub summary: ValidationSummary,
    pub results: Vec<ValidationResult>,
    pub quality_gates: QualityGateResults,
    pub deployment_decision: DeploymentDecision,
    pub recommendations: Vec<String>,
}

/// Report metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMetadata {
    pub generated_at: DateTime<Utc>,
    pub generator_version: String,
    pub project_name: String,
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
    pub validation_mode: String,
    pub total_execution_time_ms: u64,
}

/// Validation summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub total_validators: usize,
    pub passed_count: usize,
    pub failed_count: usize,
    pub warning_count: usize,
    pub critical_count: usize,
    pub overall_score: f64,
    pub overall_status: ValidationStatus,
}

/// Quality gate results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGateResults {
    pub pre_commit_gate: bool,
    pub interface_contract_gate: bool,
    pub test_coverage_gate: bool,
    pub performance_gate: bool,
    pub security_gate: bool,
    pub deployment_gate: bool,
}

/// Final deployment decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentDecision {
    pub approved: bool,
    pub reason: String,
    pub required_actions: Vec<String>,
    pub risk_level: String, // "low", "medium", "high", "critical"
}

/// Report generator implementation
pub struct ReportGenerator {
    project_name: String,
    version: String,
}

impl Default for ReportGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl ReportGenerator {
    pub fn new() -> Self {
        Self {
            project_name: "Neural Trader Phase 3".to_string(),
            version: "1.0.0".to_string(),
        }
    }

    /// Generate a complete validation report
    pub fn generate_report(
        &self,
        results: Vec<ValidationResult>,
        mode: &str,
        execution_time_ms: u64,
    ) -> Result<ValidationReport, String> {
        let summary = self.calculate_summary(&results);
        let quality_gates = self.evaluate_quality_gates(&results);
        let deployment_decision = self.make_deployment_decision(&summary, &quality_gates, mode);
        let recommendations = self.generate_recommendations(&results, &deployment_decision);

        let metadata = ReportMetadata {
            generated_at: Utc::now(),
            generator_version: self.version.clone(),
            project_name: self.project_name.clone(),
            git_commit: self.get_git_commit(),
            git_branch: self.get_git_branch(),
            validation_mode: mode.to_string(),
            total_execution_time_ms: execution_time_ms,
        };

        Ok(ValidationReport {
            metadata,
            summary,
            results,
            quality_gates,
            deployment_decision,
            recommendations,
        })
    }

    /// Calculate validation summary statistics
    fn calculate_summary(&self, results: &[ValidationResult]) -> ValidationSummary {
        let total_validators = results.len();
        let mut passed_count = 0;
        let mut failed_count = 0;
        let mut warning_count = 0;
        let mut critical_count = 0;
        let mut total_score = 0.0;

        for result in results {
            total_score += result.score;
            match result.status {
                ValidationStatus::Passed => passed_count += 1,
                ValidationStatus::Failed => failed_count += 1,
                ValidationStatus::Warning => warning_count += 1,
                ValidationStatus::Critical => critical_count += 1,
            }
        }

        let overall_score = if total_validators > 0 {
            total_score / total_validators as f64
        } else {
            0.0
        };

        let overall_status = if critical_count > 0 {
            ValidationStatus::Critical
        } else if failed_count > 0 {
            ValidationStatus::Failed
        } else if warning_count > 0 {
            ValidationStatus::Warning
        } else {
            ValidationStatus::Passed
        };

        ValidationSummary {
            total_validators,
            passed_count,
            failed_count,
            warning_count,
            critical_count,
            overall_score,
            overall_status,
        }
    }

    /// Evaluate quality gates based on validation results
    fn evaluate_quality_gates(&self, results: &[ValidationResult]) -> QualityGateResults {
        let mut gates = QualityGateResults {
            pre_commit_gate: true,
            interface_contract_gate: true,
            test_coverage_gate: true,
            performance_gate: true,
            security_gate: true,
            deployment_gate: true,
        };

        for result in results {
            match result.validator_name.as_str() {
                "code-completeness" => {
                    gates.pre_commit_gate = matches!(result.status, ValidationStatus::Passed);
                }
                "interface-contract" => {
                    gates.interface_contract_gate = matches!(result.status, ValidationStatus::Passed);
                }
                "test-coverage" => {
                    gates.test_coverage_gate = matches!(result.status, ValidationStatus::Passed);
                }
                "performance-benchmark" => {
                    gates.performance_gate = matches!(result.status, ValidationStatus::Passed);
                }
                "security-standards" => {
                    gates.security_gate = matches!(result.status, ValidationStatus::Passed);
                }
                _ => {}
            }
        }

        // Deployment gate passes only if all other gates pass
        gates.deployment_gate = gates.pre_commit_gate
            && gates.interface_contract_gate
            && gates.test_coverage_gate
            && gates.performance_gate
            && gates.security_gate;

        gates
    }

    /// Make deployment decision based on validation results
    fn make_deployment_decision(
        &self,
        summary: &ValidationSummary,
        gates: &QualityGateResults,
        mode: &str,
    ) -> DeploymentDecision {
        let mut required_actions = Vec::new();
        let mut risk_level = "low".to_string();
        let mut approved = true;
        let mut reason = "All validation gates passed".to_string();

        // Check critical failures (ZERO TOLERANCE)
        if summary.critical_count > 0 {
            approved = false;
            risk_level = "critical".to_string();
            reason = format!(
                "CRITICAL: {} critical validation failures detected",
                summary.critical_count
            );
            required_actions.push("Fix all critical issues immediately".to_string());
        }

        // Check failed validations
        if summary.failed_count > 0 && approved {
            if mode == "production" {
                // Production mode: ZERO TOLERANCE for failures
                approved = false;
                risk_level = "high".to_string();
                reason = format!(
                    "PRODUCTION BLOCKED: {} validation failures (ZERO TOLERANCE)",
                    summary.failed_count
                );
            } else {
                risk_level = "medium".to_string();
                reason = format!("{} validation failures detected", summary.failed_count);
            }
            required_actions.push("Address all validation failures".to_string());
        }

        // Check quality gates
        if !gates.deployment_gate {
            if !gates.pre_commit_gate {
                required_actions.push("Fix code completeness issues (TODOs, stubs)".to_string());
            }
            if !gates.interface_contract_gate {
                required_actions.push("Implement missing interface contracts".to_string());
            }
            if !gates.test_coverage_gate {
                required_actions.push("Increase test coverage to minimum 95%".to_string());
            }
            if !gates.performance_gate {
                required_actions.push("Optimize performance to meet SLA requirements".to_string());
            }
            if !gates.security_gate {
                required_actions.push("Address security vulnerabilities".to_string());
            }

            if mode == "production" {
                approved = false;
                risk_level = "critical".to_string();
                reason = "Quality gates failed - Production deployment blocked".to_string();
            }
        }

        // Check overall score thresholds
        match mode {
            "production" => {
                if summary.overall_score < 95.0 {
                    approved = false;
                    risk_level = "critical".to_string();
                    reason = format!(
                        "Overall score {:.1}% below production threshold (95%)",
                        summary.overall_score
                    );
                    required_actions.push("Achieve minimum 95% validation score".to_string());
                }
            }
            "staging" => {
                if summary.overall_score < 85.0 {
                    risk_level = "medium".to_string();
                    required_actions.push("Improve validation score for production readiness".to_string());
                }
            }
            _ => {
                // Development mode - more lenient
                if summary.overall_score < 70.0 {
                    required_actions.push("Consider improving code quality before merge".to_string());
                }
            }
        }

        if approved && required_actions.is_empty() {
            reason = format!(
                "All validations passed - Deployment approved (Score: {:.1}%)",
                summary.overall_score
            );
        }

        DeploymentDecision {
            approved,
            reason,
            required_actions,
            risk_level,
        }
    }

    /// Generate recommendations based on validation results
    fn generate_recommendations(&self, results: &[ValidationResult], decision: &DeploymentDecision) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Analyze results and provide specific recommendations
        for result in results {
            match result.validator_name.as_str() {
                "code-completeness" => {
                    if !matches!(result.status, ValidationStatus::Passed) {
                        recommendations.push("Review and complete all TODO comments and stub implementations".to_string());
                        recommendations.push("Consider using TDD approach for remaining implementations".to_string());
                    }
                }
                "test-coverage" => {
                    if result.score < 95.0 {
                        recommendations.push(format!(
                            "Increase test coverage from {:.1}% to minimum 95%",
                            result.score
                        ));
                        recommendations.push("Focus on testing edge cases and error handling paths".to_string());
                    }
                }
                "performance-benchmark" => {
                    if !matches!(result.status, ValidationStatus::Passed) {
                        recommendations.push("Profile and optimize performance bottlenecks".to_string());
                        recommendations.push("Consider implementing caching or connection pooling".to_string());
                    }
                }
                "security-standards" => {
                    if !matches!(result.status, ValidationStatus::Passed) {
                        recommendations.push("Review and address all security vulnerabilities".to_string());
                        recommendations.push("Update dependencies to latest secure versions".to_string());
                        recommendations.push("Consider implementing additional security controls".to_string());
                    }
                }
                _ => {}
            }
        }

        // Add general recommendations based on deployment decision
        if !decision.approved {
            recommendations.push("Run validation locally before pushing changes".to_string());
            recommendations.push("Use 'make validate-production' to test production readiness".to_string());
        } else {
            recommendations.push("Monitor application performance after deployment".to_string());
            recommendations.push("Continue maintaining high code quality standards".to_string());
        }

        // Remove duplicates and sort
        recommendations.sort();
        recommendations.dedup();
        recommendations
    }

    /// Write report to file in specified format
    pub fn write_report(
        &self,
        report: &ValidationReport,
        format: ReportFormat,
        output_path: &Path,
    ) -> Result<(), String> {
        let content = match format {
            ReportFormat::Console => self.format_console(report),
            ReportFormat::Json => self.format_json(report)?,
            ReportFormat::Html => self.format_html(report),
            ReportFormat::Markdown => self.format_markdown(report),
            ReportFormat::Csv => self.format_csv(report),
            ReportFormat::Xml => self.format_xml(report),
        };

        fs::write(output_path, content)
            .map_err(|e| format!("Failed to write report to {}: {}", output_path.display(), e))?;

        Ok(())
    }

    /// Format report as JSON
    fn format_json(&self, report: &ValidationReport) -> Result<String, String> {
        serde_json::to_string_pretty(report)
            .map_err(|e| format!("Failed to serialize report to JSON: {}", e))
    }

    /// Format report as console output
    fn format_console(&self, report: &ValidationReport) -> String {
        let mut output = String::new();

        // Header
        output.push_str("================================================================\n");
        output.push_str("  PHASE 3 PRODUCTION VALIDATION REPORT\n");
        output.push_str("  ZERO TOLERANCE FOR INCOMPLETE IMPLEMENTATIONS\n");
        output.push_str("================================================================\n\n");

        // Metadata
        output.push_str(&format!("Generated: {}\n", report.metadata.generated_at.format("%Y-%m-%d %H:%M:%S UTC")));
        output.push_str(&format!("Project: {}\n", report.metadata.project_name));
        output.push_str(&format!("Mode: {}\n", report.metadata.validation_mode));
        if let Some(ref branch) = report.metadata.git_branch {
            output.push_str(&format!("Branch: {}\n", branch));
        }
        if let Some(ref commit) = report.metadata.git_commit {
            output.push_str(&format!("Commit: {}\n", commit));
        }
        output.push_str(&format!("Execution Time: {}ms\n\n", report.metadata.total_execution_time_ms));

        // Summary
        output.push_str("VALIDATION SUMMARY\n");
        output.push_str("------------------\n");
        output.push_str(&format!("Overall Status: {:?}\n", report.summary.overall_status));
        output.push_str(&format!("Overall Score: {:.1}%\n", report.summary.overall_score));
        output.push_str(&format!("Total Validators: {}\n", report.summary.total_validators));
        output.push_str(&format!("✅ Passed: {}\n", report.summary.passed_count));
        output.push_str(&format!("❌ Failed: {}\n", report.summary.failed_count));
        output.push_str(&format!("⚠️  Warnings: {}\n", report.summary.warning_count));
        output.push_str(&format!("🚨 Critical: {}\n\n", report.summary.critical_count));

        // Quality Gates
        output.push_str("QUALITY GATES\n");
        output.push_str("-------------\n");
        output.push_str(&format!("Pre-Commit Gate: {}\n", if report.quality_gates.pre_commit_gate { "✅ PASS" } else { "❌ FAIL" }));
        output.push_str(&format!("Interface Contract Gate: {}\n", if report.quality_gates.interface_contract_gate { "✅ PASS" } else { "❌ FAIL" }));
        output.push_str(&format!("Test Coverage Gate: {}\n", if report.quality_gates.test_coverage_gate { "✅ PASS" } else { "❌ FAIL" }));
        output.push_str(&format!("Performance Gate: {}\n", if report.quality_gates.performance_gate { "✅ PASS" } else { "❌ FAIL" }));
        output.push_str(&format!("Security Gate: {}\n", if report.quality_gates.security_gate { "✅ PASS" } else { "❌ FAIL" }));
        output.push_str(&format!("Deployment Gate: {}\n\n", if report.quality_gates.deployment_gate { "✅ PASS" } else { "❌ FAIL" }));

        // Deployment Decision
        output.push_str("DEPLOYMENT DECISION\n");
        output.push_str("-------------------\n");
        output.push_str(&format!("Status: {}\n", if report.deployment_decision.approved { "🚀 APPROVED" } else { "🚨 BLOCKED" }));
        output.push_str(&format!("Reason: {}\n", report.deployment_decision.reason));
        output.push_str(&format!("Risk Level: {}\n", report.deployment_decision.risk_level.to_uppercase()));
        
        if !report.deployment_decision.required_actions.is_empty() {
            output.push_str("\nRequired Actions:\n");
            for action in &report.deployment_decision.required_actions {
                output.push_str(&format!("  • {}\n", action));
            }
        }
        output.push('\n');

        // Individual Results
        output.push_str("VALIDATION RESULTS\n");
        output.push_str("------------------\n");
        for result in &report.results {
            let status_icon = match result.status {
                ValidationStatus::Passed => "✅",
                ValidationStatus::Failed => "❌",
                ValidationStatus::Warning => "⚠️ ",
                ValidationStatus::Critical => "🚨",
            };
            
            output.push_str(&format!(
                "{} {} (Score: {:.1}%, Time: {}ms)\n",
                status_icon, result.validator_name, result.score, result.execution_time_ms
            ));
            output.push_str(&format!("   {}\n", result.message));
            
            if !result.details.is_empty() {
                for detail in &result.details {
                    output.push_str(&format!("     • {}: {}\n", detail.category, detail.message));
                }
            }
            output.push('\n');
        }

        // Recommendations
        if !report.recommendations.is_empty() {
            output.push_str("RECOMMENDATIONS\n");
            output.push_str("---------------\n");
            for (i, rec) in report.recommendations.iter().enumerate() {
                output.push_str(&format!("{}. {}\n", i + 1, rec));
            }
        }

        output
    }

    /// Format report as HTML
    fn format_html(&self, report: &ValidationReport) -> String {
        let status_color = match report.deployment_decision.approved {
            true => "#28a745",  // green
            false => "#dc3545", // red
        };

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Phase 3 Production Validation Report</title>
    <style>
        body {{ font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; margin: 0; padding: 20px; background-color: #f8f9fa; }}
        .container {{ max-width: 1200px; margin: 0 auto; background: white; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
        .header {{ background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; padding: 30px; border-radius: 8px 8px 0 0; }}
        .header h1 {{ margin: 0; font-size: 2.5em; }}
        .header .subtitle {{ margin: 10px 0 0 0; opacity: 0.9; font-size: 1.2em; }}
        .content {{ padding: 30px; }}
        .summary {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 20px; margin-bottom: 30px; }}
        .metric {{ background: #f8f9fa; padding: 20px; border-radius: 8px; text-align: center; border-left: 4px solid #007bff; }}
        .metric .value {{ font-size: 2em; font-weight: bold; color: #495057; }}
        .metric .label {{ color: #6c757d; font-size: 0.9em; text-transform: uppercase; }}
        .status-approved {{ background: #d4edda; border-color: #28a745; }}
        .status-blocked {{ background: #f8d7da; border-color: #dc3545; }}
        .quality-gates {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 15px; margin: 30px 0; }}
        .gate {{ padding: 15px; border-radius: 8px; display: flex; justify-content: space-between; align-items: center; }}
        .gate.pass {{ background: #d4edda; color: #155724; }}
        .gate.fail {{ background: #f8d7da; color: #721c24; }}
        .results {{ margin-top: 30px; }}
        .result {{ margin-bottom: 20px; padding: 20px; border-radius: 8px; border-left: 4px solid; }}
        .result.passed {{ background: #d4edda; border-color: #28a745; }}
        .result.failed {{ background: #f8d7da; border-color: #dc3545; }}
        .result.warning {{ background: #fff3cd; border-color: #ffc107; }}
        .result.critical {{ background: #f5c6cb; border-color: #dc3545; }}
        .deployment-decision {{ padding: 25px; border-radius: 8px; margin: 30px 0; text-align: center; }}
        .recommendations {{ background: #e2e3e5; padding: 20px; border-radius: 8px; margin-top: 30px; }}
        .recommendations ul {{ margin: 10px 0; padding-left: 20px; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>🚀 Phase 3 Production Validation</h1>
            <div class="subtitle">ZERO TOLERANCE FOR INCOMPLETE IMPLEMENTATIONS</div>
        </div>
        
        <div class="content">
            <div class="summary">
                <div class="metric">
                    <div class="value">{:.1}%</div>
                    <div class="label">Overall Score</div>
                </div>
                <div class="metric">
                    <div class="value">{}</div>
                    <div class="label">Total Validators</div>
                </div>
                <div class="metric">
                    <div class="value">{}</div>
                    <div class="label">Passed</div>
                </div>
                <div class="metric">
                    <div class="value">{}</div>
                    <div class="label">Failed</div>
                </div>
            </div>
            
            <h2>📊 Quality Gates</h2>
            <div class="quality-gates">
                <div class="gate {}">
                    <span>Pre-Commit Gate</span>
                    <span>{}</span>
                </div>
                <div class="gate {}">
                    <span>Interface Contract Gate</span>
                    <span>{}</span>
                </div>
                <div class="gate {}">
                    <span>Test Coverage Gate</span>
                    <span>{}</span>
                </div>
                <div class="gate {}">
                    <span>Performance Gate</span>
                    <span>{}</span>
                </div>
                <div class="gate {}">
                    <span>Security Gate</span>
                    <span>{}</span>
                </div>
                <div class="gate {}">
                    <span>Deployment Gate</span>
                    <span>{}</span>
                </div>
            </div>
            
            <div class="deployment-decision {}" style="background-color: {}; color: white;">
                <h2 style="margin: 0 0 10px 0;">{}</h2>
                <p style="margin: 0; font-size: 1.1em;">{}</p>
                <p style="margin: 10px 0 0 0; opacity: 0.9;">Risk Level: {}</p>
            </div>
            
            <h2>📋 Validation Results</h2>
            <div class="results">
                {}
            </div>
            
            {}
            
            <div style="margin-top: 30px; padding: 20px; background: #f8f9fa; border-radius: 8px; font-size: 0.9em; color: #6c757d;">
                <strong>Report Details:</strong><br>
                Generated: {} | Mode: {} | Execution Time: {}ms<br>
                {}{}
            </div>
        </div>
    </div>
</body>
</html>"#,
            report.summary.overall_score,
            report.summary.total_validators,
            report.summary.passed_count,
            report.summary.failed_count,
            if report.quality_gates.pre_commit_gate { "pass" } else { "fail" },
            if report.quality_gates.pre_commit_gate { "✅ PASS" } else { "❌ FAIL" },
            if report.quality_gates.interface_contract_gate { "pass" } else { "fail" },
            if report.quality_gates.interface_contract_gate { "✅ PASS" } else { "❌ FAIL" },
            if report.quality_gates.test_coverage_gate { "pass" } else { "fail" },
            if report.quality_gates.test_coverage_gate { "✅ PASS" } else { "❌ FAIL" },
            if report.quality_gates.performance_gate { "pass" } else { "fail" },
            if report.quality_gates.performance_gate { "✅ PASS" } else { "❌ FAIL" },
            if report.quality_gates.security_gate { "pass" } else { "fail" },
            if report.quality_gates.security_gate { "✅ PASS" } else { "❌ FAIL" },
            if report.quality_gates.deployment_gate { "pass" } else { "fail" },
            if report.quality_gates.deployment_gate { "✅ PASS" } else { "❌ FAIL" },
            if report.deployment_decision.approved { "status-approved" } else { "status-blocked" },
            status_color,
            if report.deployment_decision.approved { "🚀 DEPLOYMENT APPROVED" } else { "🚨 DEPLOYMENT BLOCKED" },
            report.deployment_decision.reason,
            report.deployment_decision.risk_level.to_uppercase(),
            self.format_html_results(&report.results),
            if !report.recommendations.is_empty() {
                format!(
                    r#"<div class="recommendations">
                        <h3>💡 Recommendations</h3>
                        <ul>
                            {}
                        </ul>
                    </div>"#,
                    report.recommendations.iter()
                        .map(|r| format!("<li>{}</li>", r))
                        .collect::<Vec<_>>()
                        .join("")
                )
            } else { String::new() },
            report.metadata.generated_at.format("%Y-%m-%d %H:%M:%S UTC"),
            report.metadata.validation_mode,
            report.metadata.total_execution_time_ms,
            report.metadata.git_branch.as_ref().map(|b| format!("Branch: {} | ", b)).unwrap_or_default(),
            report.metadata.git_commit.as_ref().map(|c| format!("Commit: {}", c)).unwrap_or_default()
        )
    }

    fn format_html_results(&self, results: &[ValidationResult]) -> String {
        results.iter().map(|result| {
            let class = match result.status {
                ValidationStatus::Passed => "passed",
                ValidationStatus::Failed => "failed",
                ValidationStatus::Warning => "warning",
                ValidationStatus::Critical => "critical",
            };
            let icon = match result.status {
                ValidationStatus::Passed => "✅",
                ValidationStatus::Failed => "❌",
                ValidationStatus::Warning => "⚠️",
                ValidationStatus::Critical => "🚨",
            };
            
            format!(
                r#"<div class="result {}">
                    <h4>{} {} (Score: {:.1}%, Time: {}ms)</h4>
                    <p>{}</p>
                    {}
                </div>"#,
                class,
                icon,
                result.validator_name,
                result.score,
                result.execution_time_ms,
                result.message,
                if !result.details.is_empty() {
                    format!(
                        "<ul>{}</ul>",
                        result.details.iter()
                            .map(|d| format!("<li><strong>{}:</strong> {}</li>", d.category, d.message))
                            .collect::<Vec<_>>()
                            .join("")
                    )
                } else { String::new() }
            )
        }).collect::<Vec<_>>().join("")
    }

    /// Format report as Markdown
    fn format_markdown(&self, report: &ValidationReport) -> String {
        let mut output = String::new();

        // Header
        output.push_str("# Phase 3 Production Validation Report\n\n");
        output.push_str("**ZERO TOLERANCE FOR INCOMPLETE IMPLEMENTATIONS**\n\n");

        // Metadata
        output.push_str("## 📋 Report Information\n\n");
        output.push_str(&format!("- **Generated:** {}\n", report.metadata.generated_at.format("%Y-%m-%d %H:%M:%S UTC")));
        output.push_str(&format!("- **Project:** {}\n", report.metadata.project_name));
        output.push_str(&format!("- **Validation Mode:** {}\n", report.metadata.validation_mode));
        output.push_str(&format!("- **Execution Time:** {}ms\n", report.metadata.total_execution_time_ms));
        if let Some(ref branch) = report.metadata.git_branch {
            output.push_str(&format!("- **Branch:** {}\n", branch));
        }
        if let Some(ref commit) = report.metadata.git_commit {
            output.push_str(&format!("- **Commit:** {}\n", commit));
        }
        output.push_str("\n");

        // Summary
        output.push_str("## 📊 Validation Summary\n\n");
        output.push_str(&format!("| Metric | Value |\n"));
        output.push_str(&format!("|--------|-------|\n"));
        output.push_str(&format!("| Overall Status | {:?} |\n", report.summary.overall_status));
        output.push_str(&format!("| Overall Score | {:.1}% |\n", report.summary.overall_score));
        output.push_str(&format!("| Total Validators | {} |\n", report.summary.total_validators));
        output.push_str(&format!("| ✅ Passed | {} |\n", report.summary.passed_count));
        output.push_str(&format!("| ❌ Failed | {} |\n", report.summary.failed_count));
        output.push_str(&format!("| ⚠️ Warnings | {} |\n", report.summary.warning_count));
        output.push_str(&format!("| 🚨 Critical | {} |\n\n", report.summary.critical_count));

        // Quality Gates
        output.push_str("## 🚪 Quality Gates\n\n");
        output.push_str("| Gate | Status |\n");
        output.push_str("|------|--------|\n");
        output.push_str(&format!("| Pre-Commit | {} |\n", if report.quality_gates.pre_commit_gate { "✅ PASS" } else { "❌ FAIL" }));
        output.push_str(&format!("| Interface Contract | {} |\n", if report.quality_gates.interface_contract_gate { "✅ PASS" } else { "❌ FAIL" }));
        output.push_str(&format!("| Test Coverage | {} |\n", if report.quality_gates.test_coverage_gate { "✅ PASS" } else { "❌ FAIL" }));
        output.push_str(&format!("| Performance | {} |\n", if report.quality_gates.performance_gate { "✅ PASS" } else { "❌ FAIL" }));
        output.push_str(&format!("| Security | {} |\n", if report.quality_gates.security_gate { "✅ PASS" } else { "❌ FAIL" }));
        output.push_str(&format!("| **Deployment** | **{}** |\n\n", if report.quality_gates.deployment_gate { "✅ PASS" } else { "❌ FAIL" }));

        // Deployment Decision
        output.push_str("## 🚀 Deployment Decision\n\n");
        output.push_str(&format!(
            "**Status:** {}\n\n",
            if report.deployment_decision.approved { "🚀 APPROVED" } else { "🚨 BLOCKED" }
        ));
        output.push_str(&format!("**Reason:** {}\n\n", report.deployment_decision.reason));
        output.push_str(&format!("**Risk Level:** {}\n\n", report.deployment_decision.risk_level.to_uppercase()));

        if !report.deployment_decision.required_actions.is_empty() {
            output.push_str("**Required Actions:**\n\n");
            for action in &report.deployment_decision.required_actions {
                output.push_str(&format!("- {}\n", action));
            }
            output.push_str("\n");
        }

        // Validation Results
        output.push_str("## 🔍 Validation Results\n\n");
        for result in &report.results {
            let status_icon = match result.status {
                ValidationStatus::Passed => "✅",
                ValidationStatus::Failed => "❌",
                ValidationStatus::Warning => "⚠️",
                ValidationStatus::Critical => "🚨",
            };
            
            output.push_str(&format!("### {} {}\n\n", status_icon, result.validator_name));
            output.push_str(&format!("- **Score:** {:.1}%\n", result.score));
            output.push_str(&format!("- **Execution Time:** {}ms\n", result.execution_time_ms));
            output.push_str(&format!("- **Status:** {:?}\n", result.status));
            output.push_str(&format!("- **Message:** {}\n", result.message));
            
            if !result.details.is_empty() {
                output.push_str("\n**Details:**\n\n");
                for detail in &result.details {
                    output.push_str(&format!("- **{}:** {}\n", detail.category, detail.message));
                    if let Some(ref suggestion) = detail.suggestion {
                        output.push_str(&format!("  - *Suggestion:* {}\n", suggestion));
                    }
                }
            }
            output.push_str("\n");
        }

        // Recommendations
        if !report.recommendations.is_empty() {
            output.push_str("## 💡 Recommendations\n\n");
            for (i, rec) in report.recommendations.iter().enumerate() {
                output.push_str(&format!("{}. {}\n", i + 1, rec));
            }
            output.push_str("\n");
        }

        output.push_str("---\n\n");
        output.push_str("*Report generated by Phase 3 Production Validation Framework*\n");

        output
    }

    /// Format report as CSV
    fn format_csv(&self, report: &ValidationReport) -> String {
        let mut output = String::new();
        
        // Header
        output.push_str("validator,status,score,execution_time_ms,message\n");
        
        // Data rows
        for result in &report.results {
            output.push_str(&format!(
                "{},{:?},{},{},{}\n",
                result.validator_name,
                result.status,
                result.score,
                result.execution_time_ms,
                result.message.replace(',', ';').replace('\n', ' ')
            ));
        }
        
        output
    }

    /// Format report as XML
    fn format_xml(&self, report: &ValidationReport) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<ValidationReport>
    <Metadata>
        <GeneratedAt>{}</GeneratedAt>
        <Project>{}</Project>
        <Mode>{}</Mode>
        <ExecutionTime>{}</ExecutionTime>
        <GitBranch>{}</GitBranch>
        <GitCommit>{}</GitCommit>
    </Metadata>
    <Summary>
        <OverallStatus>{:?}</OverallStatus>
        <OverallScore>{}</OverallScore>
        <TotalValidators>{}</TotalValidators>
        <PassedCount>{}</PassedCount>
        <FailedCount>{}</FailedCount>
        <WarningCount>{}</WarningCount>
        <CriticalCount>{}</CriticalCount>
    </Summary>
    <QualityGates>
        <PreCommitGate>{}</PreCommitGate>
        <InterfaceContractGate>{}</InterfaceContractGate>
        <TestCoverageGate>{}</TestCoverageGate>
        <PerformanceGate>{}</PerformanceGate>
        <SecurityGate>{}</SecurityGate>
        <DeploymentGate>{}</DeploymentGate>
    </QualityGates>
    <DeploymentDecision>
        <Approved>{}</Approved>
        <Reason>{}</Reason>
        <RiskLevel>{}</RiskLevel>
    </DeploymentDecision>
    <Results>
        {}
    </Results>
</ValidationReport>"#,
            report.metadata.generated_at.to_rfc3339(),
            report.metadata.project_name,
            report.metadata.validation_mode,
            report.metadata.total_execution_time_ms,
            report.metadata.git_branch.as_deref().unwrap_or("unknown"),
            report.metadata.git_commit.as_deref().unwrap_or("unknown"),
            report.summary.overall_status,
            report.summary.overall_score,
            report.summary.total_validators,
            report.summary.passed_count,
            report.summary.failed_count,
            report.summary.warning_count,
            report.summary.critical_count,
            report.quality_gates.pre_commit_gate,
            report.quality_gates.interface_contract_gate,
            report.quality_gates.test_coverage_gate,
            report.quality_gates.performance_gate,
            report.quality_gates.security_gate,
            report.quality_gates.deployment_gate,
            report.deployment_decision.approved,
            report.deployment_decision.reason,
            report.deployment_decision.risk_level,
            report.results.iter()
                .map(|r| format!(
                    r#"        <Result>
            <Validator>{}</Validator>
            <Status>{:?}</Status>
            <Score>{}</Score>
            <ExecutionTime>{}</ExecutionTime>
            <Message>{}</Message>
        </Result>"#,
                    r.validator_name, r.status, r.score, r.execution_time_ms, r.message
                ))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }

    /// Get git commit hash
    fn get_git_commit(&self) -> Option<String> {
        std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
    }

    /// Get git branch name
    fn get_git_branch(&self) -> Option<String> {
        std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    String::from_utf8(output.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_generation() {
        let generator = ReportGenerator::new();
        let results = vec![
            ValidationResult {
                validator_name: "test-validator".to_string(),
                status: ValidationStatus::Passed,
                score: 95.0,
                message: "All tests passed".to_string(),
                details: vec![],
                execution_time_ms: 1000,
                timestamp: Utc::now(),
            },
        ];

        let report = generator.generate_report(results, "development", 1000);
        assert!(report.is_ok());

        let report = report.unwrap();
        assert_eq!(report.summary.total_validators, 1);
        assert_eq!(report.summary.passed_count, 1);
        assert_eq!(report.summary.overall_score, 95.0);
        assert!(matches!(report.summary.overall_status, ValidationStatus::Passed));
    }

    #[test]
    fn test_deployment_decision_production_mode() {
        let generator = ReportGenerator::new();
        let summary = ValidationSummary {
            total_validators: 1,
            passed_count: 0,
            failed_count: 1,
            warning_count: 0,
            critical_count: 0,
            overall_score: 60.0,
            overall_status: ValidationStatus::Failed,
        };
        
        let gates = QualityGateResults {
            pre_commit_gate: false,
            interface_contract_gate: true,
            test_coverage_gate: true,
            performance_gate: true,
            security_gate: true,
            deployment_gate: false,
        };

        let decision = generator.make_deployment_decision(&summary, &gates, "production");
        assert!(!decision.approved);
        assert_eq!(decision.risk_level, "high");
    }

    #[test]
    fn test_json_serialization() {
        let generator = ReportGenerator::new();
        let results = vec![
            ValidationResult {
                validator_name: "test".to_string(),
                status: ValidationStatus::Passed,
                score: 100.0,
                message: "OK".to_string(),
                details: vec![],
                execution_time_ms: 500,
                timestamp: Utc::now(),
            },
        ];

        let report = generator.generate_report(results, "development", 500).unwrap();
        let json = generator.format_json(&report);
        assert!(json.is_ok());
        
        // Verify it's valid JSON
        let _: ValidationReport = serde_json::from_str(&json.unwrap()).unwrap();
    }

    #[test]
    fn test_console_format() {
        let generator = ReportGenerator::new();
        let results = vec![
            ValidationResult {
                validator_name: "test".to_string(),
                status: ValidationStatus::Passed,
                score: 100.0,
                message: "All good".to_string(),
                details: vec![],
                execution_time_ms: 500,
                timestamp: Utc::now(),
            },
        ];

        let report = generator.generate_report(results, "development", 500).unwrap();
        let console_output = generator.format_console(&report);
        
        assert!(console_output.contains("PHASE 3 PRODUCTION VALIDATION REPORT"));
        assert!(console_output.contains("✅ Passed: 1"));
        assert!(console_output.contains("🚀 APPROVED"));
    }
}