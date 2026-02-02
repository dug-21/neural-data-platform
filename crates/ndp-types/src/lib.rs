//! NDP Types - Single Source of Truth for NDP Configuration Types
//!
//! This crate provides the authoritative type definitions for all NDP
//! configuration structures. It serves as the single source of truth
//! to prevent validation drift between runtime types, validators, and
//! JSON Schema.
//!
//! # Overview
//!
//! NDP uses a config-driven architecture where behavior is defined in YAML
//! configuration files. This crate defines the types that those configurations
//! deserialize into, with full support for:
//!
//! - **Serde serialization**: All types implement `Serialize` and `Deserialize`
//! - **JSON Schema generation**: All types implement `JsonSchema` (via schemars)
//! - **Enum utilities**: String parsing, iteration, display (via strum)
//! - **Validation**: The `NdpValidate` trait for semantic validation
//!
//! # Type Categories
//!
//! ## Core Types (CRITICAL)
//!
//! - [`SourceType`]: Data source types for Bronze layer ingestion
//! - [`FieldType`]: Logical field types for Bronze schema
//! - [`SilverFieldType`]: PostgreSQL column types for Silver layer
//!
//! ## DQ Types (HIGH)
//!
//! - [`DqRuleType`]: DQ rule type discriminator
//! - [`DqAction`]: Actions when DQ rules fail
//! - [`MonotonicDirection`]: Direction for monotonic checks
//!
//! ## Transform Types (MEDIUM)
//!
//! - [`TimestampTransform`]: Timestamp conversion types
//! - [`TransformType`]: Field transform types
//! - [`ConversionFormulaType`]: Unit conversion formula types
//!
//! ## Strategy Types (LOW)
//!
//! - [`DeduplicationStrategy`]: Duplicate handling strategies
//! - [`PartitioningStrategy`]: Parquet partitioning strategies
//!
//! ## Validation Framework
//!
//! - [`NdpValidate`]: Trait for semantic validation
//! - [`ValidationError`]: Unified error structure
//! - [`ValidationContext`]: Context for cross-reference validation
//! - [`ErrorCode`]: Machine-readable error codes
//!
//! # Usage
//!
//! ```rust
//! use ndp_types::{SourceType, FieldType, DqRuleType, DqAction};
//!
//! // Types derive Serialize, Deserialize, JsonSchema
//! let source = SourceType::HttpPoll;
//! let json = serde_json::to_string(&source).unwrap();
//! assert_eq!(json, "\"http_poll\"");
//!
//! // Enum iteration via strum
//! for name in SourceType::all_names() {
//!     println!("Supported source type: {}", name);
//! }
//!
//! // Parse from strings
//! let parsed: SourceType = "mqtt".parse().unwrap();
//! assert_eq!(parsed, SourceType::Mqtt);
//! ```
//!
//! # Schema Generation
//!
//! ```rust
//! use schemars::schema_for;
//! use ndp_types::SourceType;
//!
//! let schema = schema_for!(SourceType);
//! // Schema includes all variants with descriptions from doc comments
//! ```

// =============================================================================
// Module declarations
// =============================================================================

// Core types - CRITICAL priority
mod field_type;
mod source_type;

// DQ types - HIGH priority
mod dq_rule;

// Transform types - MEDIUM priority
mod transform;

// Strategy types - LOW priority
mod strategy;

// Validation framework
mod validate;

// Error types
mod error;

// =============================================================================
// Core Types - CRITICAL priority
// =============================================================================

/// Data source types for Bronze layer ingestion.
pub use source_type::SourceType;

/// Logical field types for Bronze schema definitions.
pub use field_type::FieldType;

/// PostgreSQL column types for Silver layer tables.
pub use field_type::SilverFieldType;

// =============================================================================
// DQ Types - HIGH priority
// =============================================================================

/// DQ rule type discriminator for validation.
pub use dq_rule::DqRuleType;

/// Actions when DQ rules fail.
pub use dq_rule::DqAction;

/// Direction for monotonic check rules.
pub use dq_rule::MonotonicDirection;

// =============================================================================
// Transform Types - MEDIUM priority
// =============================================================================

/// Timestamp transform types for Silver ETL.
pub use transform::TimestampTransform;

/// Field transform types for Silver ETL.
pub use transform::TransformType;

/// Conversion formula types for unit transforms.
pub use transform::ConversionFormulaType;

// =============================================================================
// Strategy Types - LOW priority
// =============================================================================

/// Deduplication strategies for Silver ETL.
pub use strategy::DeduplicationStrategy;

/// Partitioning strategies for Bronze Parquet files.
pub use strategy::PartitioningStrategy;

// =============================================================================
// Validation Framework
// =============================================================================

/// Trait for NDP configuration validation.
pub use validate::NdpValidate;

/// Unified validation error structure.
pub use validate::ValidationError;

/// Context for cross-reference validation.
pub use validate::ValidationContext;

/// Validation layer indicator.
pub use validate::ValidationLayer;

/// Error severity level.
pub use validate::Severity;

/// Machine-readable error codes.
pub use error::ErrorCode;

// =============================================================================
// Re-export strum traits for enum iteration
// =============================================================================

/// Re-export strum's IntoEnumIterator for convenient enum iteration.
pub use strum::IntoEnumIterator;

// =============================================================================
// Version Information
// =============================================================================

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Crate name.
pub const NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_types_exported() {
        // Core types
        let _ = SourceType::Mqtt;
        let _ = FieldType::Float;
        let _ = SilverFieldType::DoublePrecision;

        // DQ types
        let _ = DqRuleType::RangeCheck;
        let _ = DqAction::Flag;
        let _ = MonotonicDirection::Increasing;

        // Transform types
        let _ = TimestampTransform::MicrosecondsToTimestamp;
        let _ = TransformType::UnitConversion;
        let _ = ConversionFormulaType::Linear;

        // Strategy types
        let _ = DeduplicationStrategy::Upsert;
        let _ = PartitioningStrategy::Daily;

        // Validation types
        let _ = ValidationLayer::Semantic;
        let _ = Severity::Error;
        let _ = ErrorCode::InvalidSourceType;
    }

    #[test]
    fn test_version_info() {
        assert!(!VERSION.is_empty());
        assert_eq!(NAME, "ndp-types");
    }

    #[test]
    fn test_all_names_methods() {
        // Verify all_names() is available on all enums
        assert!(!SourceType::all_names().is_empty());
        assert!(!FieldType::all_names().is_empty());
        assert!(!SilverFieldType::all_names().is_empty());
        assert!(!DqRuleType::all_names().is_empty());
        assert!(!DqAction::all_names().is_empty());
        assert!(!MonotonicDirection::all_names().is_empty());
        assert!(!TimestampTransform::all_names().is_empty());
        assert!(!TransformType::all_names().is_empty());
        assert!(!ConversionFormulaType::all_names().is_empty());
        assert!(!DeduplicationStrategy::all_names().is_empty());
        assert!(!PartitioningStrategy::all_names().is_empty());
    }

    #[test]
    fn test_enum_iteration() {
        // Verify IntoEnumIterator works
        let count: usize = SourceType::iter().count();
        assert_eq!(count, 5);
    }
}
