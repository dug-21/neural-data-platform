//! Data Quality rule type definitions for NDP.
//!
//! This module defines DQ rule types, actions, and validation structures.
//! These are the single source of truth for all DQ-related enums used
//! throughout the NDP validation pipeline.
//!
//! # Rule Types (11 total)
//!
//! - Value-level: `range_check`, `null_check`, `enum_check`, `pattern_check`
//! - Temporal: `freshness_check`, `monotonic_check`, `rate_of_change`
//! - Cross-field: `cross_field_check`, `conditional_check`
//! - Batch-level: `completeness_check`, `cardinality_check`
//!
//! # Actions (5 total)
//!
//! - `flag`: Add to dq_flags but keep value
//! - `reject`: Set value to NULL
//! - `clamp`: Clamp value to valid range
//! - `drop`: Drop entire row
//! - `warn`: Log warning only

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

/// DQ rule type discriminator for semantic validation.
///
/// This enum provides the rule type names without parameters,
/// used for validating rule type strings before full parsing.
/// Corresponds to the 11 DQ rule types defined in DQ-FRAMEWORK-DESIGN.md.
///
/// # Example
///
/// ```rust
/// use ndp_types::DqRuleType;
///
/// let rule_type = DqRuleType::RangeCheck;
/// assert_eq!(rule_type.as_ref(), "range_check");
///
/// // Get all valid rule types for error messages
/// let all_types = DqRuleType::all_names();
/// assert!(all_types.contains(&"range_check"));
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    EnumIter,
    EnumString,
    Display,
    AsRefStr,
    VariantNames,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum DqRuleType {
    // =========================================================================
    // Value-Level Rules
    // =========================================================================
    /// Value must be within min/max bounds.
    ///
    /// Supports both inclusive and exclusive bounds.
    /// At least one of min or max must be specified.
    RangeCheck,

    /// Value must not be null/empty.
    ///
    /// Used for required fields that cannot be NULL.
    NullCheck,

    /// Value must be one of allowed values.
    ///
    /// Supports case-sensitive and case-insensitive matching.
    EnumCheck,

    /// Value must match regex pattern.
    ///
    /// Pattern must be a valid regex expression.
    PatternCheck,

    // =========================================================================
    // Temporal Rules
    // =========================================================================
    /// Data must arrive within time threshold.
    ///
    /// Checks that timestamps are not too old (max_age) or in the future (max_future).
    FreshnessCheck,

    /// Values must be monotonically increasing/decreasing.
    ///
    /// Used for cumulative counters and sequential data.
    MonotonicCheck,

    /// Rate of change must be within bounds.
    ///
    /// Detects sudden spikes or drops in time-series data.
    RateOfChange,

    // =========================================================================
    // Cross-Field Rules
    // =========================================================================
    /// Relationship between fields must hold.
    ///
    /// Uses SQL-like expressions to validate relationships.
    CrossFieldCheck,

    /// Conditional validation based on other field values.
    ///
    /// Applies a rule only when a condition is met.
    ConditionalCheck,

    // =========================================================================
    // Batch-Level Rules
    // =========================================================================
    /// Required percentage of non-null values.
    ///
    /// Validates batch completeness (e.g., 95% of values must be present).
    CompletenessCheck,

    /// Distinct value count within bounds.
    ///
    /// Validates cardinality constraints.
    CardinalityCheck,
}

impl DqRuleType {
    /// Returns all rule type names as a static slice.
    ///
    /// This is the single source of truth for supported DQ rule types.
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// Returns an iterator over all rule type variants.
    pub fn all() -> impl Iterator<Item = Self> {
        use strum::IntoEnumIterator;
        Self::iter()
    }

    /// Check if this is a value-level rule.
    pub fn is_value_level(&self) -> bool {
        matches!(
            self,
            DqRuleType::RangeCheck
                | DqRuleType::NullCheck
                | DqRuleType::EnumCheck
                | DqRuleType::PatternCheck
        )
    }

    /// Check if this is a temporal rule.
    pub fn is_temporal(&self) -> bool {
        matches!(
            self,
            DqRuleType::FreshnessCheck | DqRuleType::MonotonicCheck | DqRuleType::RateOfChange
        )
    }

    /// Check if this is a cross-field rule.
    pub fn is_cross_field(&self) -> bool {
        matches!(
            self,
            DqRuleType::CrossFieldCheck | DqRuleType::ConditionalCheck
        )
    }

    /// Check if this is a batch-level rule.
    pub fn is_batch_level(&self) -> bool {
        matches!(
            self,
            DqRuleType::CompletenessCheck | DqRuleType::CardinalityCheck
        )
    }
}

/// Data quality actions when a rule fails.
///
/// Actions determine what happens to data that fails DQ validation.
/// The default action is `Flag`.
///
/// # Example
///
/// ```rust
/// use ndp_types::DqAction;
///
/// let action = DqAction::Flag;
/// assert_eq!(action.as_ref(), "flag");
///
/// // Default is Flag
/// let default_action = DqAction::default();
/// assert_eq!(default_action, DqAction::Flag);
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Default,
    Serialize,
    Deserialize,
    JsonSchema,
    EnumIter,
    EnumString,
    Display,
    AsRefStr,
    VariantNames,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum DqAction {
    /// Add DQ flag column but keep record value.
    ///
    /// This is the default action. The original value is preserved
    /// and a flag is added to the dq_flags column.
    #[default]
    Flag,

    /// Reject record - set value to NULL.
    ///
    /// The value is replaced with NULL and flagged.
    Reject,

    /// Clamp value to valid range.
    ///
    /// For range_check rules, the value is adjusted to the nearest bound.
    Clamp,

    /// Drop the field value (set to null).
    ///
    /// Similar to reject but specifically for dropping the value.
    Drop,

    /// Log warning but process normally.
    ///
    /// Typically used for batch-level rules.
    Warn,
}

impl DqAction {
    /// Returns all action names as a static slice.
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// Returns an iterator over all action variants.
    pub fn all() -> impl Iterator<Item = Self> {
        use strum::IntoEnumIterator;
        Self::iter()
    }

    /// Check if this action modifies the value.
    pub fn modifies_value(&self) -> bool {
        matches!(self, DqAction::Reject | DqAction::Clamp | DqAction::Drop)
    }

    /// Check if this action is blocking (prevents row from being processed).
    pub fn is_blocking(&self) -> bool {
        matches!(self, DqAction::Drop)
    }
}

/// Direction constraint for monotonic checks.
///
/// Specifies whether values should increase or decrease monotonically.
///
/// # Example
///
/// ```rust
/// use ndp_types::MonotonicDirection;
///
/// let dir = MonotonicDirection::Increasing;
/// assert_eq!(dir.as_ref(), "increasing");
/// ```
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    EnumIter,
    EnumString,
    Display,
    AsRefStr,
    VariantNames,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum MonotonicDirection {
    /// Values must be >= previous value.
    Increasing,

    /// Values must be <= previous value.
    Decreasing,

    /// Values must be > previous value (no equality).
    StrictIncreasing,
}

impl MonotonicDirection {
    /// Returns all direction names as a static slice.
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// Check if this direction allows equal values.
    pub fn allows_equal(&self) -> bool {
        matches!(
            self,
            MonotonicDirection::Increasing | MonotonicDirection::Decreasing
        )
    }
}

// =============================================================================
// LONDON SCHOOL TDD TESTS - TC-104, TC-105, TC-106 Series
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // =========================================================================
    // TC-104: DqRuleType Serialization/Deserialization
    // Description: Verify DqRuleType all variants serialize correctly
    // Priority: High
    // =========================================================================
    #[test]
    fn test_dq_rule_type_serializes_to_snake_case() {
        let test_cases = vec![
            (DqRuleType::RangeCheck, "\"range_check\""),
            (DqRuleType::NullCheck, "\"null_check\""),
            (DqRuleType::EnumCheck, "\"enum_check\""),
            (DqRuleType::PatternCheck, "\"pattern_check\""),
            (DqRuleType::FreshnessCheck, "\"freshness_check\""),
            (DqRuleType::MonotonicCheck, "\"monotonic_check\""),
            (DqRuleType::RateOfChange, "\"rate_of_change\""),
            (DqRuleType::CrossFieldCheck, "\"cross_field_check\""),
            (DqRuleType::ConditionalCheck, "\"conditional_check\""),
            (DqRuleType::CompletenessCheck, "\"completeness_check\""),
            (DqRuleType::CardinalityCheck, "\"cardinality_check\""),
        ];

        for (rule_type, expected_json) in test_cases {
            let json = serde_json::to_string(&rule_type).unwrap();
            assert_eq!(json, expected_json, "Failed for {:?}", rule_type);
        }
    }

    #[test]
    fn test_dq_rule_type_deserializes_from_snake_case() {
        let test_cases = vec![
            ("\"range_check\"", DqRuleType::RangeCheck),
            ("\"null_check\"", DqRuleType::NullCheck),
            ("\"enum_check\"", DqRuleType::EnumCheck),
            ("\"pattern_check\"", DqRuleType::PatternCheck),
            ("\"freshness_check\"", DqRuleType::FreshnessCheck),
            ("\"monotonic_check\"", DqRuleType::MonotonicCheck),
            ("\"rate_of_change\"", DqRuleType::RateOfChange),
            ("\"cross_field_check\"", DqRuleType::CrossFieldCheck),
            ("\"conditional_check\"", DqRuleType::ConditionalCheck),
            ("\"completeness_check\"", DqRuleType::CompletenessCheck),
            ("\"cardinality_check\"", DqRuleType::CardinalityCheck),
        ];

        for (json, expected) in test_cases {
            let deserialized: DqRuleType = serde_json::from_str(json).unwrap();
            assert_eq!(deserialized, expected);
        }
    }

    #[test]
    fn test_dq_rule_type_round_trip() {
        for variant in DqRuleType::all() {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: DqRuleType = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    // =========================================================================
    // TC-105: DqAction Serialization
    // Description: Verify all DqAction variants serialize correctly
    // Priority: High
    // =========================================================================
    #[test]
    fn test_dq_action_serialization() {
        let test_cases = vec![
            (DqAction::Flag, "\"flag\""),
            (DqAction::Reject, "\"reject\""),
            (DqAction::Clamp, "\"clamp\""),
            (DqAction::Drop, "\"drop\""),
            (DqAction::Warn, "\"warn\""),
        ];

        for (action, expected) in test_cases {
            let json = serde_json::to_string(&action).unwrap();
            assert_eq!(json, expected);
        }
    }

    #[test]
    fn test_dq_action_deserialization() {
        let test_cases = vec![
            ("\"flag\"", DqAction::Flag),
            ("\"reject\"", DqAction::Reject),
            ("\"clamp\"", DqAction::Clamp),
            ("\"drop\"", DqAction::Drop),
            ("\"warn\"", DqAction::Warn),
        ];

        for (json, expected) in test_cases {
            let deserialized: DqAction = serde_json::from_str(json).unwrap();
            assert_eq!(deserialized, expected);
        }
    }

    #[test]
    fn test_dq_action_round_trip() {
        for variant in DqAction::all() {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: DqAction = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    // =========================================================================
    // TC-106: Unknown variant deserialization fails gracefully
    // Description: Verify invalid values are rejected
    // Priority: High
    // =========================================================================
    #[test]
    fn test_dq_rule_type_rejects_invalid() {
        let invalid_values = vec![
            "\"invalid_rule\"",
            "\"custom\"",
            "\"RANGE_CHECK\"", // uppercase
            "\"rangeCheck\"",  // camelCase
        ];

        for invalid in invalid_values {
            let result: Result<DqRuleType, _> = serde_json::from_str(invalid);
            assert!(result.is_err(), "Should reject: {}", invalid);
        }
    }

    #[test]
    fn test_dq_action_rejects_invalid() {
        let invalid_values = vec![
            "\"invalid_action\"",
            "\"skip\"",
            "\"FLAG\"", // uppercase
            "\"Warn\"", // mixed case
        ];

        for invalid in invalid_values {
            let result: Result<DqAction, _> = serde_json::from_str(invalid);
            assert!(result.is_err(), "Should reject: {}", invalid);
        }
    }

    // =========================================================================
    // TC-107: Case sensitivity
    // Priority: High
    // =========================================================================
    #[test]
    fn test_case_sensitivity() {
        // Uppercase should fail
        assert!(serde_json::from_str::<DqRuleType>("\"RANGE_CHECK\"").is_err());
        assert!(serde_json::from_str::<DqAction>("\"FLAG\"").is_err());

        // Lowercase should succeed
        assert!(serde_json::from_str::<DqRuleType>("\"range_check\"").is_ok());
        assert!(serde_json::from_str::<DqAction>("\"flag\"").is_ok());
    }

    // =========================================================================
    // TC-301: all_names() completeness
    // Description: Verify all_names() returns all variants
    // Priority: High
    // =========================================================================
    #[test]
    fn test_dq_rule_type_all_names_complete() {
        let names = DqRuleType::all_names();

        assert_eq!(names.len(), 11, "Should have exactly 11 DQ rule types");
        assert!(names.contains(&"range_check"));
        assert!(names.contains(&"null_check"));
        assert!(names.contains(&"enum_check"));
        assert!(names.contains(&"pattern_check"));
        assert!(names.contains(&"freshness_check"));
        assert!(names.contains(&"monotonic_check"));
        assert!(names.contains(&"rate_of_change"));
        assert!(names.contains(&"cross_field_check"));
        assert!(names.contains(&"conditional_check"));
        assert!(names.contains(&"completeness_check"));
        assert!(names.contains(&"cardinality_check"));
    }

    #[test]
    fn test_dq_action_all_names_complete() {
        let names = DqAction::all_names();

        assert_eq!(names.len(), 5, "Should have exactly 5 DQ actions");
        assert!(names.contains(&"flag"));
        assert!(names.contains(&"reject"));
        assert!(names.contains(&"clamp"));
        assert!(names.contains(&"drop"));
        assert!(names.contains(&"warn"));
    }

    // =========================================================================
    // TC-302: EnumString parsing
    // Priority: High
    // =========================================================================
    #[test]
    fn test_dq_rule_type_from_string() {
        assert_eq!(
            DqRuleType::from_str("range_check").unwrap(),
            DqRuleType::RangeCheck
        );
        assert_eq!(
            DqRuleType::from_str("null_check").unwrap(),
            DqRuleType::NullCheck
        );
        assert!(DqRuleType::from_str("invalid").is_err());
    }

    #[test]
    fn test_dq_action_from_string() {
        assert_eq!(DqAction::from_str("flag").unwrap(), DqAction::Flag);
        assert_eq!(DqAction::from_str("reject").unwrap(), DqAction::Reject);
        assert_eq!(DqAction::from_str("clamp").unwrap(), DqAction::Clamp);
        assert!(DqAction::from_str("invalid").is_err());
    }

    // =========================================================================
    // TC-303: AsRefStr conversion
    // Priority: Medium
    // =========================================================================
    #[test]
    fn test_dq_rule_type_as_ref_str() {
        assert_eq!(DqRuleType::RangeCheck.as_ref(), "range_check");
        assert_eq!(DqRuleType::NullCheck.as_ref(), "null_check");
        assert_eq!(DqRuleType::CrossFieldCheck.as_ref(), "cross_field_check");
    }

    #[test]
    fn test_dq_action_as_ref_str() {
        assert_eq!(DqAction::Flag.as_ref(), "flag");
        assert_eq!(DqAction::Reject.as_ref(), "reject");
        assert_eq!(DqAction::Clamp.as_ref(), "clamp");
    }

    // =========================================================================
    // TC-304: VariantNames
    // Priority: Medium
    // =========================================================================
    #[test]
    fn test_variant_names_matches_all_names() {
        assert_eq!(DqRuleType::VARIANTS.len(), DqRuleType::all_names().len());
        assert_eq!(DqAction::VARIANTS.len(), DqAction::all_names().len());
    }

    // =========================================================================
    // Rule category tests
    // =========================================================================
    #[test]
    fn test_dq_rule_type_categories() {
        // Value-level rules
        assert!(DqRuleType::RangeCheck.is_value_level());
        assert!(DqRuleType::NullCheck.is_value_level());
        assert!(DqRuleType::EnumCheck.is_value_level());
        assert!(DqRuleType::PatternCheck.is_value_level());

        // Temporal rules
        assert!(DqRuleType::FreshnessCheck.is_temporal());
        assert!(DqRuleType::MonotonicCheck.is_temporal());
        assert!(DqRuleType::RateOfChange.is_temporal());

        // Cross-field rules
        assert!(DqRuleType::CrossFieldCheck.is_cross_field());
        assert!(DqRuleType::ConditionalCheck.is_cross_field());

        // Batch-level rules
        assert!(DqRuleType::CompletenessCheck.is_batch_level());
        assert!(DqRuleType::CardinalityCheck.is_batch_level());
    }

    // =========================================================================
    // DqAction behavior tests
    // =========================================================================
    #[test]
    fn test_dq_action_default() {
        let default_action = DqAction::default();
        assert_eq!(default_action, DqAction::Flag);
    }

    #[test]
    fn test_dq_action_modifies_value() {
        assert!(!DqAction::Flag.modifies_value());
        assert!(DqAction::Reject.modifies_value());
        assert!(DqAction::Clamp.modifies_value());
        assert!(DqAction::Drop.modifies_value());
        assert!(!DqAction::Warn.modifies_value());
    }

    #[test]
    fn test_dq_action_is_blocking() {
        assert!(!DqAction::Flag.is_blocking());
        assert!(!DqAction::Reject.is_blocking());
        assert!(!DqAction::Clamp.is_blocking());
        assert!(DqAction::Drop.is_blocking());
        assert!(!DqAction::Warn.is_blocking());
    }

    // =========================================================================
    // MonotonicDirection tests
    // =========================================================================
    #[test]
    fn test_monotonic_direction_serialization() {
        let test_cases = vec![
            (MonotonicDirection::Increasing, "\"increasing\""),
            (MonotonicDirection::Decreasing, "\"decreasing\""),
            (
                MonotonicDirection::StrictIncreasing,
                "\"strict_increasing\"",
            ),
        ];

        for (direction, expected) in test_cases {
            let json = serde_json::to_string(&direction).unwrap();
            assert_eq!(json, expected);
        }
    }

    #[test]
    fn test_monotonic_direction_round_trip() {
        for variant in MonotonicDirection::all_names() {
            let json = format!("\"{}\"", variant);
            let deserialized: MonotonicDirection = serde_json::from_str(&json).unwrap();
            let reserialized = serde_json::to_string(&deserialized).unwrap();
            assert_eq!(json, reserialized);
        }
    }

    #[test]
    fn test_monotonic_direction_allows_equal() {
        assert!(MonotonicDirection::Increasing.allows_equal());
        assert!(MonotonicDirection::Decreasing.allows_equal());
        assert!(!MonotonicDirection::StrictIncreasing.allows_equal());
    }

    // =========================================================================
    // JSON Schema generation tests
    // =========================================================================
    #[test]
    fn test_dq_rule_type_schema_generated() {
        let schema = schemars::schema_for!(DqRuleType);
        let schema_json = serde_json::to_value(&schema).unwrap();

        // Verify schema is generated and contains all variants
        let schema_str = serde_json::to_string(&schema_json).unwrap();
        assert!(schema_str.contains("\"range_check\""));
        assert!(schema_str.contains("\"null_check\""));
        assert!(schema_str.contains("\"cardinality_check\""));
    }

    #[test]
    fn test_dq_action_schema_generated() {
        let schema = schemars::schema_for!(DqAction);
        let schema_json = serde_json::to_value(&schema).unwrap();

        // Verify schema is generated and contains all action variants
        let schema_str = serde_json::to_string(&schema_json).unwrap();
        assert!(schema_str.contains("\"flag\""));
        assert!(schema_str.contains("\"reject\""));
        assert!(schema_str.contains("\"warn\""));
    }

    // =========================================================================
    // Hash and Clone tests
    // =========================================================================
    #[test]
    fn test_dq_rule_type_hashable() {
        use std::collections::HashMap;

        let mut map: HashMap<DqRuleType, &str> = HashMap::new();
        map.insert(DqRuleType::RangeCheck, "Range validation");
        map.insert(DqRuleType::NullCheck, "Null validation");

        assert_eq!(map.get(&DqRuleType::RangeCheck), Some(&"Range validation"));
        assert_eq!(map.get(&DqRuleType::FreshnessCheck), None);
    }

    #[test]
    fn test_dq_action_hashable() {
        use std::collections::HashMap;

        let mut map: HashMap<DqAction, &str> = HashMap::new();
        map.insert(DqAction::Flag, "Flag action");
        map.insert(DqAction::Reject, "Reject action");

        assert_eq!(map.get(&DqAction::Flag), Some(&"Flag action"));
    }
}
