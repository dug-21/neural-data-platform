//! Transform type definitions for Silver ETL.
//!
//! This module defines transformation types used in Bronze-to-Silver ETL,
//! including timestamp transforms and field conversion formulas.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

/// Timestamp transform types for Silver ETL.
///
/// These transforms convert various timestamp formats in Bronze data
/// to PostgreSQL TIMESTAMPTZ in the Silver layer.
///
/// # Variants
///
/// - `MicrosecondsToTimestamp`: Convert microseconds since Unix epoch
/// - `Iso8601`: Parse ISO 8601 formatted string
/// - `UnixSeconds`: Convert Unix seconds to timestamp
/// - `NwsDuration`: Parse NWS duration format (ISO 8601 with duration suffix)
///
/// # Example
///
/// ```rust
/// use ndp_types::TimestampTransform;
///
/// let transform = TimestampTransform::MicrosecondsToTimestamp;
/// assert_eq!(transform.as_ref(), "microseconds_to_timestamp");
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
pub enum TimestampTransform {
    /// Convert microseconds since Unix epoch to timestamp.
    ///
    /// Input: integer (e.g., 1704067200000000)
    /// Output: TIMESTAMPTZ
    MicrosecondsToTimestamp,

    /// Parse ISO 8601 formatted string.
    ///
    /// Input: string (e.g., "2024-01-01T00:00:00Z")
    /// Output: TIMESTAMPTZ
    Iso8601,

    /// Convert Unix seconds to timestamp.
    ///
    /// Input: integer or float (e.g., 1704067200)
    /// Output: TIMESTAMPTZ
    UnixSeconds,

    /// Parse NWS duration format.
    ///
    /// Input: ISO 8601 duration string (e.g., "PT3H")
    /// Used for NWS forecast valid times relative to issue time.
    NwsDuration,
}

impl TimestampTransform {
    /// Returns all timestamp transform names as a static slice.
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// Returns an iterator over all timestamp transform variants.
    pub fn all() -> impl Iterator<Item = Self> {
        use strum::IntoEnumIterator;
        Self::iter()
    }

    /// Get the SQL expression pattern for this transform.
    ///
    /// The placeholder `{field}` should be replaced with the actual field name.
    pub fn sql_pattern(&self) -> &'static str {
        match self {
            TimestampTransform::MicrosecondsToTimestamp => {
                "TO_TIMESTAMP({field}::BIGINT / 1000000.0)"
            }
            TimestampTransform::Iso8601 => "{field}::TIMESTAMPTZ",
            TimestampTransform::UnixSeconds => "TO_TIMESTAMP({field}::BIGINT)",
            TimestampTransform::NwsDuration => "({issue_time} + {field}::INTERVAL)",
        }
    }
}

/// Transform configuration types for field mappings.
///
/// These are the tagged enum variants for field-level transforms
/// in Silver ETL configuration.
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
pub enum TransformType {
    /// Unit conversion (e.g., Kelvin to Celsius).
    UnitConversion,

    /// SQL expression transform.
    Expression,

    /// Lookup table for categorical mappings.
    Lookup,

    /// JSON path extraction from nested payloads.
    JsonExtract,

    /// Timestamp format conversion.
    Timestamp,

    /// Computed field based on other columns.
    Computed,
}

impl TransformType {
    /// Returns all transform type names as a static slice.
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// Returns an iterator over all transform type variants.
    pub fn all() -> impl Iterator<Item = Self> {
        use strum::IntoEnumIterator;
        Self::iter()
    }
}

/// Conversion formula types for unit conversion transforms.
///
/// Used with `UnitConversion` transform to define how values
/// are converted between units.
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
pub enum ConversionFormulaType {
    /// Linear transformation: (value * scale) + offset.
    ///
    /// Example: Kelvin to Celsius: scale=1.0, offset=-273.15
    Linear,

    /// Custom code expression (future enhancement).
    Custom,
}

impl ConversionFormulaType {
    /// Returns all formula type names as a static slice.
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn test_timestamp_transform_roundtrip() {
        for variant in TimestampTransform::iter() {
            let serialized = serde_json::to_string(&variant).unwrap();
            let deserialized: TimestampTransform = serde_json::from_str(&serialized).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn test_timestamp_transform_count() {
        assert_eq!(TimestampTransform::all_names().len(), 4);
    }

    #[test]
    fn test_timestamp_transform_names() {
        let names = TimestampTransform::all_names();
        assert!(names.contains(&"microseconds_to_timestamp"));
        assert!(names.contains(&"iso8601"));
        assert!(names.contains(&"unix_seconds"));
        assert!(names.contains(&"nws_duration"));
    }

    #[test]
    fn test_transform_type_roundtrip() {
        for variant in TransformType::iter() {
            let serialized = serde_json::to_string(&variant).unwrap();
            let deserialized: TransformType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn test_transform_type_count() {
        assert_eq!(TransformType::all_names().len(), 6);
    }

    #[test]
    fn test_transform_type_names() {
        let names = TransformType::all_names();
        assert!(names.contains(&"unit_conversion"));
        assert!(names.contains(&"expression"));
        assert!(names.contains(&"lookup"));
        assert!(names.contains(&"json_extract"));
        assert!(names.contains(&"timestamp"));
        assert!(names.contains(&"computed"));
    }

    #[test]
    fn test_conversion_formula_type_roundtrip() {
        for variant in ConversionFormulaType::iter() {
            let serialized = serde_json::to_string(&variant).unwrap();
            let deserialized: ConversionFormulaType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn test_sql_pattern() {
        assert!(TimestampTransform::MicrosecondsToTimestamp
            .sql_pattern()
            .contains("TO_TIMESTAMP"));
        assert!(TimestampTransform::Iso8601
            .sql_pattern()
            .contains("TIMESTAMPTZ"));
    }
}
