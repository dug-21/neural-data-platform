//! Enhanced Performance Snapshot with Data Type Tracking
//!
//! This module provides an enhanced version of PerformanceSnapshot that embeds
//! the existing structure while adding data type discovery and pattern tracking
//! capabilities for the DAA extension system.
//!
//! ## Backward Compatibility
//!
//! The EnhancedPerformanceSnapshot embeds the original PerformanceSnapshot to
//! ensure all existing DAA decision flows continue to work unchanged. The new
//! fields are additive and optional.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

use super::autonomous_training::PerformanceSnapshot;

/// Data type patterns discovered during neural processing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataTypePattern {
    /// Numerical data with statistical properties
    Numerical {
        mean: f64,
        std_dev: f64,
        min: f64,
        max: f64,
        distribution_type: DistributionType,
    },
    /// Categorical data with value frequencies
    Categorical {
        unique_values: Vec<String>,
        value_counts: HashMap<String, u32>,
        entropy: f64,
    },
    /// Time series data with temporal characteristics
    TimeSeries {
        seasonality_detected: bool,
        trend_strength: f64,
        autocorrelation: f64,
        stationarity_p_value: Option<f64>,
    },
    /// Text data with linguistic properties
    Textual {
        vocabulary_size: usize,
        average_length: f64,
        language_detected: Option<String>,
        sentiment_polarity: Option<f64>,
    },
    /// Complex nested data structures
    Structured {
        depth_levels: u32,
        field_types: HashMap<String, String>,
        schema_stability: f64,
    },
}

/// Statistical distribution types for numerical data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DistributionType {
    Normal,
    LogNormal,
    Exponential,
    Uniform,
    Skewed { skewness: f64 },
    Multimodal { modes: u32 },
    Unknown,
}

/// Metrics for tracking data type patterns and completeness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTypeMetrics {
    /// Discovered patterns by data source/field
    pub discovered_patterns: HashMap<String, DataTypePattern>,
    /// Data completeness scores by field (0.0 = no data, 1.0 = complete)
    pub field_completeness: HashMap<String, f64>,
    /// Pattern stability over time (how consistent patterns remain)
    pub pattern_stability_score: f64,
    /// Number of new patterns discovered in this snapshot
    pub new_patterns_count: u32,
    /// Total data points processed for pattern discovery
    pub total_data_points: u64,
    /// Timestamp when patterns were last updated
    pub last_pattern_update: DateTime<Utc>,
    /// Confidence in the discovered patterns (0.0 to 1.0)
    pub pattern_confidence: f64,
    /// Data quality issues detected
    pub quality_issues: Vec<DataQualityIssue>,
}

/// Data quality issues that may affect neural training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQualityIssue {
    /// Type of quality issue
    pub issue_type: QualityIssueType,
    /// Affected field or data source
    pub affected_field: String,
    /// Severity level (0.0 = minor, 1.0 = critical)
    pub severity: f64,
    /// Human-readable description
    pub description: String,
    /// Suggested remediation if available
    pub remediation: Option<String>,
    /// When this issue was detected
    pub detected_at: DateTime<Utc>,
}

/// Types of data quality issues
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QualityIssueType {
    /// Missing data beyond acceptable thresholds
    MissingData,
    /// Data format inconsistencies
    FormatInconsistency,
    /// Statistical outliers that may indicate errors
    OutlierDetection,
    /// Schema changes that break compatibility
    SchemaChange,
    /// Data distribution drift over time
    DistributionDrift,
    /// Encoding or character set issues
    EncodingIssues,
    /// Temporal inconsistencies (future dates, etc.)
    TemporalInconsistency,
}

/// Enhanced performance snapshot that embeds the original structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedPerformanceSnapshot {
    /// Original performance snapshot - preserved for backward compatibility
    pub base_snapshot: PerformanceSnapshot,
    
    /// Enhanced data type metrics for pattern tracking
    pub data_type_metrics: DataTypeMetrics,
    
    /// Overall data completeness score (0.0 to 1.0)
    pub data_completeness_score: f64,
    
    /// Enhancement metadata
    pub enhancement_metadata: EnhancementMetadata,
}

/// Metadata about the enhancement process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancementMetadata {
    /// Version of the enhancement system
    pub enhancement_version: String,
    /// Time when enhancement was applied
    pub enhanced_at: DateTime<Utc>,
    /// Processing time for enhancement in milliseconds
    pub processing_time_ms: u64,
    /// Source of the enhancement data
    pub data_sources: Vec<String>,
    /// Any warnings or notes from the enhancement process
    pub processing_notes: Vec<String>,
}

impl Default for DataTypeMetrics {
    fn default() -> Self {
        Self {
            discovered_patterns: HashMap::new(),
            field_completeness: HashMap::new(),
            pattern_stability_score: 1.0,
            new_patterns_count: 0,
            total_data_points: 0,
            last_pattern_update: Utc::now(),
            pattern_confidence: 0.0,
            quality_issues: Vec::new(),
        }
    }
}

impl Default for EnhancementMetadata {
    fn default() -> Self {
        Self {
            enhancement_version: "1.0.0".to_string(),
            enhanced_at: Utc::now(),
            processing_time_ms: 0,
            data_sources: Vec::new(),
            processing_notes: Vec::new(),
        }
    }
}

impl EnhancedPerformanceSnapshot {
    /// Create from an existing PerformanceSnapshot with default enhancements
    pub fn from_base_snapshot(base_snapshot: PerformanceSnapshot) -> Self {
        Self {
            base_snapshot,
            data_type_metrics: DataTypeMetrics::default(),
            data_completeness_score: 0.0,
            enhancement_metadata: EnhancementMetadata::default(),
        }
    }
    
    /// Create with full enhancement data
    pub fn new(
        base_snapshot: PerformanceSnapshot,
        data_type_metrics: DataTypeMetrics,
        data_completeness_score: f64,
    ) -> Self {
        Self {
            base_snapshot,
            data_type_metrics,
            data_completeness_score,
            enhancement_metadata: EnhancementMetadata::default(),
        }
    }
    
    /// Get the embedded base snapshot for backward compatibility
    pub fn base(&self) -> &PerformanceSnapshot {
        &self.base_snapshot
    }
    
    /// Consume self and extract the base snapshot
    pub fn into_base(self) -> PerformanceSnapshot {
        self.base_snapshot
    }
    
    /// Calculate an overall enhancement score (0.0 to 1.0)
    pub fn enhancement_score(&self) -> f64 {
        let pattern_score = if self.data_type_metrics.discovered_patterns.is_empty() {
            0.0
        } else {
            self.data_type_metrics.pattern_confidence
        };
        
        let quality_score = if self.data_type_metrics.quality_issues.is_empty() {
            1.0
        } else {
            let total_severity: f64 = self.data_type_metrics.quality_issues
                .iter()
                .map(|issue| issue.severity)
                .sum();
            (1.0 - (total_severity / self.data_type_metrics.quality_issues.len() as f64)).max(0.0)
        };
        
        (self.data_completeness_score + pattern_score + quality_score) / 3.0
    }
    
    /// Check if the snapshot indicates data type discovery is needed
    pub fn needs_pattern_discovery(&self) -> bool {
        self.data_type_metrics.new_patterns_count > 0 ||
        self.data_type_metrics.pattern_confidence < 0.7 ||
        self.data_completeness_score < 0.8
    }
    
    /// Get critical quality issues that require immediate attention
    pub fn critical_quality_issues(&self) -> Vec<&DataQualityIssue> {
        self.data_type_metrics.quality_issues
            .iter()
            .filter(|issue| issue.severity >= 0.8)
            .collect()
    }
    
    /// Add a new data quality issue
    pub fn add_quality_issue(&mut self, issue: DataQualityIssue) {
        self.data_type_metrics.quality_issues.push(issue);
    }
    
    /// Update pattern discovery results
    pub fn update_patterns(&mut self, patterns: HashMap<String, DataTypePattern>) {
        let old_count = self.data_type_metrics.discovered_patterns.len();
        self.data_type_metrics.discovered_patterns.extend(patterns);
        let new_count = self.data_type_metrics.discovered_patterns.len();
        
        self.data_type_metrics.new_patterns_count = (new_count - old_count) as u32;
        self.data_type_metrics.last_pattern_update = Utc::now();
        
        // Update pattern confidence based on stability
        if self.data_type_metrics.pattern_stability_score > 0.8 {
            self.data_type_metrics.pattern_confidence = 
                (self.data_type_metrics.pattern_confidence + 0.1).min(1.0);
        }
    }
}

/// Backward compatibility wrapper for converting to/from base PerformanceSnapshot
impl From<PerformanceSnapshot> for EnhancedPerformanceSnapshot {
    fn from(base_snapshot: PerformanceSnapshot) -> Self {
        Self::from_base_snapshot(base_snapshot)
    }
}

impl From<EnhancedPerformanceSnapshot> for PerformanceSnapshot {
    fn from(enhanced: EnhancedPerformanceSnapshot) -> Self {
        enhanced.base_snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_base_snapshot() -> PerformanceSnapshot {
        PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.85,
            confidence: 0.8,
            price_error: 0.05,
            sharpe_ratio: Some(1.2),
            max_drawdown: Some(0.05),
            volatility: 0.1,
            model_agreement: 0.9,
            consecutive_failures: 0,
            trading_volume: 1000.0,
            profit_loss: 50.0,
            event_count: 50,
            window_duration: chrono::Duration::minutes(1),
            // Extended fields for compatibility with other modules
            latency_ms: 100,
            error_rate: 0.15,
            recent_predictions: 50,
            symbol: Some("TEST".to_string()),
            trading_performance: None,
            accuracy_metrics: None,
            data_type_metrics: None,
            // Observability module compatibility fields
            cpu_usage: Some(50.0),
            memory_usage: Some(512.0),
            active_connections: Some(10),
            requests_per_second: Some(30.0),
            average_response_time: Some(chrono::Duration::milliseconds(25).into()),
            cache_hit_rate: Some(0.85),
        }
    }

    #[test]
    fn test_enhanced_snapshot_creation() {
        let base = create_test_base_snapshot();
        let enhanced = EnhancedPerformanceSnapshot::from_base_snapshot(base.clone());
        
        assert_eq!(enhanced.base_snapshot.accuracy, base.accuracy);
        assert_eq!(enhanced.data_completeness_score, 0.0);
        assert!(enhanced.data_type_metrics.discovered_patterns.is_empty());
    }

    #[test]
    fn test_backward_compatibility_conversion() {
        let base = create_test_base_snapshot();
        let original_accuracy = base.accuracy;
        
        // Convert to enhanced and back
        let enhanced = EnhancedPerformanceSnapshot::from(base);
        let recovered: PerformanceSnapshot = enhanced.into();
        
        assert_eq!(recovered.accuracy, original_accuracy);
    }

    #[test]
    fn test_pattern_update() {
        let base = create_test_base_snapshot();
        let mut enhanced = EnhancedPerformanceSnapshot::from_base_snapshot(base);
        
        let mut patterns = HashMap::new();
        patterns.insert("price_data".to_string(), DataTypePattern::Numerical {
            mean: 100.0,
            std_dev: 10.0,
            min: 80.0,
            max: 120.0,
            distribution_type: DistributionType::Normal,
        });
        
        enhanced.update_patterns(patterns);
        
        assert_eq!(enhanced.data_type_metrics.new_patterns_count, 1);
        assert!(enhanced.data_type_metrics.discovered_patterns.contains_key("price_data"));
    }

    #[test]
    fn test_quality_issue_management() {
        let base = create_test_base_snapshot();
        let mut enhanced = EnhancedPerformanceSnapshot::from_base_snapshot(base);
        
        let critical_issue = DataQualityIssue {
            issue_type: QualityIssueType::MissingData,
            affected_field: "price_feed".to_string(),
            severity: 0.9,
            description: "Critical data missing".to_string(),
            remediation: Some("Check data source connectivity".to_string()),
            detected_at: Utc::now(),
        };
        
        enhanced.add_quality_issue(critical_issue);
        
        let critical_issues = enhanced.critical_quality_issues();
        assert_eq!(critical_issues.len(), 1);
        assert_eq!(critical_issues[0].severity, 0.9);
    }

    #[test]
    fn test_enhancement_score_calculation() {
        let base = create_test_base_snapshot();
        let mut enhanced = EnhancedPerformanceSnapshot::from_base_snapshot(base);
        
        enhanced.data_completeness_score = 0.9;
        enhanced.data_type_metrics.pattern_confidence = 0.8;
        
        let score = enhanced.enhancement_score();
        assert!(score > 0.8);
        assert!(score <= 1.0);
    }

    #[test]
    fn test_needs_pattern_discovery() {
        let base = create_test_base_snapshot();
        let mut enhanced = EnhancedPerformanceSnapshot::from_base_snapshot(base);
        
        // Should need discovery initially
        assert!(enhanced.needs_pattern_discovery());
        
        // Set high confidence and completeness
        enhanced.data_type_metrics.pattern_confidence = 0.9;
        enhanced.data_completeness_score = 0.9;
        enhanced.data_type_metrics.new_patterns_count = 0;
        
        // Should not need discovery now
        assert!(!enhanced.needs_pattern_discovery());
    }

    #[test]
    fn test_serialization_compatibility() {
        let base = create_test_base_snapshot();
        let enhanced = EnhancedPerformanceSnapshot::from_base_snapshot(base);
        
        // Test JSON serialization
        let json = serde_json::to_string(&enhanced).unwrap();
        let deserialized: EnhancedPerformanceSnapshot = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.base_snapshot.accuracy, enhanced.base_snapshot.accuracy);
        assert_eq!(deserialized.data_completeness_score, enhanced.data_completeness_score);
    }
}