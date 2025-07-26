"""
Validation report generation and formatting.

Provides comprehensive reporting functionality for all validation results including:
- HTML report generation
- JSON export
- Summary dashboards
- Email notifications
"""

import asyncio
import json
from typing import List, Dict, Any, Optional, Union
from datetime import datetime, timedelta
from dataclasses import dataclass, field, asdict
from pathlib import Path
import pandas as pd
from jinja2 import Template

from .pre_loader import PreValidationResult
from .post_loader import PostValidationResult
from .gap_detector import GapAnalysisResult
from .checksum_validator import DataIntegrityReport
from .data_quality import DataQualityReport
from ..utils.logging import get_logger
from ..utils.metrics import metrics


@dataclass
class ValidationReport:
    """Comprehensive validation report combining all validation results."""
    report_id: str
    symbol: str
    start_date: datetime
    end_date: datetime
    validation_timestamp: datetime
    
    # Validation results
    pre_validation: Optional[PreValidationResult] = None
    post_validation: Optional[PostValidationResult] = None
    gap_analysis: Optional[GapAnalysisResult] = None
    integrity_check: Optional[DataIntegrityReport] = None
    quality_analysis: Optional[DataQualityReport] = None
    
    # Summary metrics
    overall_status: str = "PENDING"  # PASSED, FAILED, WARNING
    overall_score: float = 0.0
    critical_issues: List[str] = field(default_factory=list)
    warnings: List[str] = field(default_factory=list)
    recommendations: List[str] = field(default_factory=list)
    
    # Performance metrics
    validation_duration_seconds: float = 0.0
    data_volume_mb: float = 0.0
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert report to dictionary format."""
        return {
            'report_id': self.report_id,
            'symbol': self.symbol,
            'date_range': {
                'start': self.start_date.isoformat(),
                'end': self.end_date.isoformat()
            },
            'validation_timestamp': self.validation_timestamp.isoformat(),
            'overall_status': self.overall_status,
            'overall_score': self.overall_score,
            'critical_issues': self.critical_issues,
            'warnings': self.warnings,
            'recommendations': self.recommendations,
            'validation_results': {
                'pre_validation': {
                    'is_valid': self.pre_validation.is_valid,
                    'validation_score': self.pre_validation.validation_score,
                    'error_rate': self.pre_validation.error_rate,
                    'total_records': self.pre_validation.total_records
                } if self.pre_validation else None,
                'post_validation': {
                    'is_valid': self.post_validation.is_valid,
                    'success_rate': self.post_validation.success_rate,
                    'checks_performed': self.post_validation.checks_performed
                } if self.post_validation else None,
                'gap_analysis': {
                    'total_gaps': self.gap_analysis.total_gaps,
                    'coverage_percentage': self.gap_analysis.coverage_percentage,
                    'largest_gap': str(self.gap_analysis.largest_gap.duration) if self.gap_analysis.largest_gap else None
                } if self.gap_analysis else None,
                'integrity_check': {
                    'overall_integrity_score': self.integrity_check.overall_integrity_score,
                    'validations_performed': self.integrity_check.validations_performed
                } if self.integrity_check else None,
                'quality_analysis': {
                    'overall_quality_score': self.quality_analysis.overall_quality_score,
                    'quality_grade': self.quality_analysis.quality_grade
                } if self.quality_analysis else None
            },
            'performance': {
                'validation_duration_seconds': self.validation_duration_seconds,
                'data_volume_mb': self.data_volume_mb
            }
        }


class ValidationReportGenerator:
    """Generates validation reports in various formats."""
    
    # HTML template for report
    HTML_TEMPLATE = """
<!DOCTYPE html>
<html>
<head>
    <title>Data Validation Report - {{ symbol }}</title>
    <style>
        body { 
            font-family: Arial, sans-serif; 
            margin: 20px;
            background-color: #f5f5f5;
        }
        .header {
            background-color: #2c3e50;
            color: white;
            padding: 20px;
            border-radius: 5px;
            margin-bottom: 20px;
        }
        .summary {
            background-color: white;
            padding: 20px;
            border-radius: 5px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            margin-bottom: 20px;
        }
        .status-passed { color: #27ae60; font-weight: bold; }
        .status-failed { color: #e74c3c; font-weight: bold; }
        .status-warning { color: #f39c12; font-weight: bold; }
        .metric-card {
            display: inline-block;
            background-color: #ecf0f1;
            padding: 15px;
            margin: 10px;
            border-radius: 5px;
            min-width: 200px;
        }
        .metric-value {
            font-size: 24px;
            font-weight: bold;
            color: #2c3e50;
        }
        .metric-label {
            color: #7f8c8d;
            font-size: 14px;
        }
        .section {
            background-color: white;
            padding: 20px;
            margin-bottom: 20px;
            border-radius: 5px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .section h2 {
            color: #2c3e50;
            border-bottom: 2px solid #ecf0f1;
            padding-bottom: 10px;
        }
        .issue-list {
            list-style-type: none;
            padding: 0;
        }
        .issue-item {
            padding: 10px;
            margin: 5px 0;
            border-left: 4px solid #e74c3c;
            background-color: #ffe6e6;
        }
        .warning-item {
            padding: 10px;
            margin: 5px 0;
            border-left: 4px solid #f39c12;
            background-color: #fff3cd;
        }
        .recommendation-item {
            padding: 10px;
            margin: 5px 0;
            border-left: 4px solid #3498db;
            background-color: #e3f2fd;
        }
        table {
            width: 100%;
            border-collapse: collapse;
            margin-top: 10px;
        }
        th, td {
            text-align: left;
            padding: 10px;
            border-bottom: 1px solid #ecf0f1;
        }
        th {
            background-color: #ecf0f1;
            font-weight: bold;
        }
        .progress-bar {
            width: 100%;
            height: 20px;
            background-color: #ecf0f1;
            border-radius: 10px;
            overflow: hidden;
        }
        .progress-fill {
            height: 100%;
            background-color: #3498db;
            transition: width 0.3s ease;
        }
        .quality-grade {
            display: inline-block;
            font-size: 48px;
            font-weight: bold;
            padding: 20px;
            border-radius: 50%;
            width: 80px;
            height: 80px;
            text-align: center;
            line-height: 80px;
        }
        .grade-a { background-color: #27ae60; color: white; }
        .grade-b { background-color: #3498db; color: white; }
        .grade-c { background-color: #f39c12; color: white; }
        .grade-d { background-color: #e67e22; color: white; }
        .grade-f { background-color: #e74c3c; color: white; }
    </style>
</head>
<body>
    <div class="header">
        <h1>Data Validation Report</h1>
        <p>Symbol: {{ symbol }} | Date Range: {{ start_date }} to {{ end_date }}</p>
        <p>Generated: {{ timestamp }}</p>
    </div>
    
    <div class="summary">
        <h2>Executive Summary</h2>
        <div>
            <div class="metric-card">
                <div class="metric-label">Overall Status</div>
                <div class="metric-value status-{{ status_class }}">{{ overall_status }}</div>
            </div>
            <div class="metric-card">
                <div class="metric-label">Overall Score</div>
                <div class="metric-value">{{ overall_score }}%</div>
            </div>
            {% if quality_grade %}
            <div class="metric-card">
                <div class="metric-label">Quality Grade</div>
                <div class="quality-grade grade-{{ quality_grade|lower }}">{{ quality_grade }}</div>
            </div>
            {% endif %}
        </div>
    </div>
    
    {% if critical_issues %}
    <div class="section">
        <h2>Critical Issues ({{ critical_issues|length }})</h2>
        <ul class="issue-list">
            {% for issue in critical_issues %}
            <li class="issue-item">{{ issue }}</li>
            {% endfor %}
        </ul>
    </div>
    {% endif %}
    
    {% if warnings %}
    <div class="section">
        <h2>Warnings ({{ warnings|length }})</h2>
        <ul class="issue-list">
            {% for warning in warnings %}
            <li class="warning-item">{{ warning }}</li>
            {% endfor %}
        </ul>
    </div>
    {% endif %}
    
    <div class="section">
        <h2>Validation Results</h2>
        
        {% if pre_validation %}
        <h3>Pre-Load Validation</h3>
        <table>
            <tr>
                <th>Metric</th>
                <th>Value</th>
            </tr>
            <tr>
                <td>Total Records</td>
                <td>{{ pre_validation.total_records|number_format }}</td>
            </tr>
            <tr>
                <td>Valid Records</td>
                <td>{{ pre_validation.valid_records|number_format }}</td>
            </tr>
            <tr>
                <td>Validation Score</td>
                <td>
                    <div class="progress-bar">
                        <div class="progress-fill" style="width: {{ pre_validation.validation_score }}%"></div>
                    </div>
                    {{ pre_validation.validation_score }}%
                </td>
            </tr>
        </table>
        {% endif %}
        
        {% if post_validation %}
        <h3>Post-Load Validation</h3>
        <table>
            <tr>
                <th>Check</th>
                <th>Status</th>
            </tr>
            <tr>
                <td>Checks Performed</td>
                <td>{{ post_validation.checks_performed }}</td>
            </tr>
            <tr>
                <td>Checks Passed</td>
                <td>{{ post_validation.checks_passed }}</td>
            </tr>
            <tr>
                <td>Success Rate</td>
                <td>{{ post_validation.success_rate }}%</td>
            </tr>
        </table>
        {% endif %}
        
        {% if gap_analysis %}
        <h3>Gap Analysis</h3>
        <table>
            <tr>
                <th>Metric</th>
                <th>Value</th>
            </tr>
            <tr>
                <td>Total Gaps</td>
                <td>{{ gap_analysis.total_gaps }}</td>
            </tr>
            <tr>
                <td>Coverage Percentage</td>
                <td>{{ gap_analysis.coverage_percentage }}%</td>
            </tr>
            <tr>
                <td>Largest Gap</td>
                <td>{{ gap_analysis.largest_gap_duration }}</td>
            </tr>
        </table>
        {% endif %}
        
        {% if quality_dimensions %}
        <h3>Quality Dimensions</h3>
        <table>
            <tr>
                <th>Dimension</th>
                <th>Score</th>
                <th>Weight</th>
                <th>Contribution</th>
            </tr>
            {% for dim in quality_dimensions %}
            <tr>
                <td>{{ dim.name|title }}</td>
                <td>{{ dim.score }}%</td>
                <td>{{ dim.weight }}</td>
                <td>{{ dim.contribution }}%</td>
            </tr>
            {% endfor %}
        </table>
        {% endif %}
    </div>
    
    {% if recommendations %}
    <div class="section">
        <h2>Recommendations</h2>
        <ul class="issue-list">
            {% for rec in recommendations %}
            <li class="recommendation-item">{{ rec }}</li>
            {% endfor %}
        </ul>
    </div>
    {% endif %}
    
    <div class="section">
        <h2>Performance Metrics</h2>
        <table>
            <tr>
                <td>Validation Duration</td>
                <td>{{ validation_duration }} seconds</td>
            </tr>
            <tr>
                <td>Data Volume Processed</td>
                <td>{{ data_volume }} MB</td>
            </tr>
        </table>
    </div>
</body>
</html>
"""
    
    def __init__(self, output_dir: Optional[Path] = None):
        self.logger = get_logger(__name__)
        self.output_dir = output_dir or Path("validation_reports")
        self.output_dir.mkdir(exist_ok=True)
        
    def generate_report(
        self,
        report: ValidationReport,
        output_format: str = "html",
        include_details: bool = True
    ) -> Path:
        """Generate validation report in specified format."""
        self.logger.info(
            f"Generating {output_format} report for {report.symbol}"
        )
        
        if output_format == "html":
            return self._generate_html_report(report, include_details)
        elif output_format == "json":
            return self._generate_json_report(report, include_details)
        elif output_format == "summary":
            return self._generate_summary_report(report)
        else:
            raise ValueError(f"Unsupported output format: {output_format}")
            
    def _generate_html_report(
        self,
        report: ValidationReport,
        include_details: bool
    ) -> Path:
        """Generate HTML validation report."""
        # Prepare template data
        template_data = {
            'symbol': report.symbol,
            'start_date': report.start_date.strftime('%Y-%m-%d'),
            'end_date': report.end_date.strftime('%Y-%m-%d'),
            'timestamp': report.validation_timestamp.strftime('%Y-%m-%d %H:%M:%S'),
            'overall_status': report.overall_status,
            'overall_score': f"{report.overall_score:.1f}",
            'status_class': self._get_status_class(report.overall_status),
            'critical_issues': report.critical_issues,
            'warnings': report.warnings[:20],  # Limit to 20 warnings
            'recommendations': report.recommendations,
            'validation_duration': f"{report.validation_duration_seconds:.2f}",
            'data_volume': f"{report.data_volume_mb:.2f}"
        }
        
        # Add validation results
        if report.pre_validation:
            template_data['pre_validation'] = {
                'total_records': report.pre_validation.total_records,
                'valid_records': report.pre_validation.valid_records,
                'validation_score': f"{report.pre_validation.validation_score:.1f}"
            }
            
        if report.post_validation:
            template_data['post_validation'] = {
                'checks_performed': report.post_validation.checks_performed,
                'checks_passed': report.post_validation.checks_passed,
                'success_rate': f"{report.post_validation.success_rate:.1f}"
            }
            
        if report.gap_analysis:
            template_data['gap_analysis'] = {
                'total_gaps': report.gap_analysis.total_gaps,
                'coverage_percentage': f"{report.gap_analysis.coverage_percentage:.1f}",
                'largest_gap_duration': str(report.gap_analysis.largest_gap.duration) if report.gap_analysis.largest_gap else "N/A"
            }
            
        if report.quality_analysis:
            template_data['quality_grade'] = report.quality_analysis.quality_grade
            template_data['quality_dimensions'] = [
                {
                    'name': name,
                    'score': f"{dim.score:.1f}",
                    'weight': f"{dim.weight:.2f}",
                    'contribution': f"{dim.weighted_score:.1f}"
                }
                for name, dim in report.quality_analysis.dimensions.items()
            ]
            
        # Custom filter for number formatting
        def number_format(value):
            return f"{value:,}"
            
        # Generate HTML
        template = Template(self.HTML_TEMPLATE)
        template.globals['number_format'] = number_format
        html_content = template.render(**template_data)
        
        # Save report
        filename = f"validation_report_{report.symbol}_{report.report_id}.html"
        output_path = self.output_dir / filename
        
        with open(output_path, 'w') as f:
            f.write(html_content)
            
        self.logger.info(f"HTML report saved to {output_path}")
        return output_path
        
    def _generate_json_report(
        self,
        report: ValidationReport,
        include_details: bool
    ) -> Path:
        """Generate JSON validation report."""
        # Convert report to dictionary
        report_dict = report.to_dict()
        
        if include_details:
            # Add detailed validation results
            if report.pre_validation:
                report_dict['pre_validation_details'] = {
                    'validation_errors': report.pre_validation.validation_errors[:100],
                    'warnings': report.pre_validation.warnings[:50],
                    'statistics': report.pre_validation.statistics
                }
                
            if report.post_validation:
                report_dict['post_validation_details'] = {
                    'validation_errors': report.post_validation.validation_errors[:100],
                    'query_results': report.post_validation.query_results
                }
                
            if report.gap_analysis:
                report_dict['gap_analysis_details'] = {
                    'gaps_by_severity': {
                        k.value: v for k, v in report.gap_analysis.gaps_by_severity.items()
                    },
                    'gaps_by_type': report.gap_analysis.gaps_by_type,
                    'summary_statistics': report.gap_analysis.summary_statistics
                }
                
            if report.integrity_check:
                report_dict['integrity_check_details'] = {
                    'checksum_results': {
                        k: {
                            'is_valid': v.is_valid,
                            'match_percentage': v.match_percentage,
                            'errors': v.errors
                        }
                        for k, v in report.integrity_check.checksum_results.items()
                    },
                    'row_count_validation': report.integrity_check.row_count_validation,
                    'statistical_validation': report.integrity_check.statistical_validation
                }
                
            if report.quality_analysis:
                report_dict['quality_analysis_details'] = report.quality_analysis.to_dict()
                
        # Save report
        filename = f"validation_report_{report.symbol}_{report.report_id}.json"
        output_path = self.output_dir / filename
        
        with open(output_path, 'w') as f:
            json.dump(report_dict, f, indent=2, default=str)
            
        self.logger.info(f"JSON report saved to {output_path}")
        return output_path
        
    def _generate_summary_report(self, report: ValidationReport) -> Path:
        """Generate summary validation report."""
        summary_lines = []
        summary_lines.append(f"VALIDATION SUMMARY REPORT")
        summary_lines.append("=" * 50)
        summary_lines.append(f"Symbol: {report.symbol}")
        summary_lines.append(f"Date Range: {report.start_date.date()} to {report.end_date.date()}")
        summary_lines.append(f"Report ID: {report.report_id}")
        summary_lines.append(f"Generated: {report.validation_timestamp}")
        summary_lines.append("")
        summary_lines.append(f"OVERALL STATUS: {report.overall_status}")
        summary_lines.append(f"OVERALL SCORE: {report.overall_score:.1f}%")
        
        if report.quality_analysis:
            summary_lines.append(f"QUALITY GRADE: {report.quality_analysis.quality_grade}")
            
        summary_lines.append("")
        summary_lines.append("VALIDATION RESULTS:")
        
        if report.pre_validation:
            summary_lines.append(f"  Pre-Load: {'PASSED' if report.pre_validation.is_valid else 'FAILED'} "
                               f"({report.pre_validation.validation_score:.1f}% valid)")
                               
        if report.post_validation:
            summary_lines.append(f"  Post-Load: {'PASSED' if report.post_validation.is_valid else 'FAILED'} "
                               f"({report.post_validation.success_rate:.1f}% checks passed)")
                               
        if report.gap_analysis:
            summary_lines.append(f"  Gap Analysis: {report.gap_analysis.total_gaps} gaps found "
                               f"({report.gap_analysis.coverage_percentage:.1f}% coverage)")
                               
        if report.integrity_check:
            summary_lines.append(f"  Integrity: {report.integrity_check.overall_integrity_score:.1f}% score")
            
        if report.quality_analysis:
            summary_lines.append(f"  Quality: {report.quality_analysis.overall_quality_score:.1f}% "
                               f"(Grade: {report.quality_analysis.quality_grade})")
                               
        if report.critical_issues:
            summary_lines.append("")
            summary_lines.append(f"CRITICAL ISSUES ({len(report.critical_issues)}):")
            for issue in report.critical_issues[:5]:
                summary_lines.append(f"  - {issue}")
                
        if len(report.critical_issues) > 5:
            summary_lines.append(f"  ... and {len(report.critical_issues) - 5} more")
            
        if report.recommendations:
            summary_lines.append("")
            summary_lines.append(f"TOP RECOMMENDATIONS:")
            for rec in report.recommendations[:3]:
                summary_lines.append(f"  - {rec}")
                
        summary_lines.append("")
        summary_lines.append(f"Validation completed in {report.validation_duration_seconds:.2f} seconds")
        summary_lines.append(f"Processed {report.data_volume_mb:.2f} MB of data")
        
        # Save report
        filename = f"validation_summary_{report.symbol}_{report.report_id}.txt"
        output_path = self.output_dir / filename
        
        with open(output_path, 'w') as f:
            f.write("\n".join(summary_lines))
            
        self.logger.info(f"Summary report saved to {output_path}")
        return output_path
        
    def _get_status_class(self, status: str) -> str:
        """Get CSS class for status."""
        status_map = {
            'PASSED': 'passed',
            'FAILED': 'failed',
            'WARNING': 'warning'
        }
        return status_map.get(status, 'warning')
        
    async def send_email_report(
        self,
        report: ValidationReport,
        recipients: List[str],
        smtp_config: Dict[str, Any]
    ):
        """Send validation report via email."""
        # This would integrate with email service
        # Placeholder for email functionality
        self.logger.info(f"Email report would be sent to {recipients}")
        
    def generate_dashboard_data(
        self,
        reports: List[ValidationReport]
    ) -> Dict[str, Any]:
        """Generate dashboard data from multiple reports."""
        dashboard_data = {
            'total_reports': len(reports),
            'symbols': list(set(r.symbol for r in reports)),
            'date_range': {
                'start': min(r.start_date for r in reports),
                'end': max(r.end_date for r in reports)
            },
            'status_summary': {
                'passed': sum(1 for r in reports if r.overall_status == 'PASSED'),
                'failed': sum(1 for r in reports if r.overall_status == 'FAILED'),
                'warning': sum(1 for r in reports if r.overall_status == 'WARNING')
            },
            'average_score': sum(r.overall_score for r in reports) / len(reports) if reports else 0,
            'total_issues': sum(len(r.critical_issues) for r in reports),
            'common_issues': self._find_common_issues(reports),
            'quality_distribution': self._calculate_quality_distribution(reports)
        }
        
        return dashboard_data
        
    def _find_common_issues(
        self,
        reports: List[ValidationReport],
        top_n: int = 10
    ) -> List[Dict[str, Any]]:
        """Find most common issues across reports."""
        issue_counts = {}
        
        for report in reports:
            for issue in report.critical_issues:
                # Normalize issue text for grouping
                normalized = issue.lower().strip()
                issue_counts[normalized] = issue_counts.get(normalized, 0) + 1
                
        # Sort by frequency
        common_issues = sorted(
            issue_counts.items(),
            key=lambda x: x[1],
            reverse=True
        )[:top_n]
        
        return [
            {'issue': issue, 'count': count}
            for issue, count in common_issues
        ]
        
    def _calculate_quality_distribution(
        self,
        reports: List[ValidationReport]
    ) -> Dict[str, int]:
        """Calculate quality grade distribution."""
        distribution = {'A': 0, 'B': 0, 'C': 0, 'D': 0, 'F': 0}
        
        for report in reports:
            if report.quality_analysis:
                grade = report.quality_analysis.quality_grade
                distribution[grade] = distribution.get(grade, 0) + 1
                
        return distribution