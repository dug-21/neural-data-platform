//! Field type definitions for NDP schemas.
//!
//! This module defines data types for Bronze and Silver layer fields.
//! Bronze types are logical types, while Silver types map to PostgreSQL.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

/// Field data types for Bronze layer schema definitions.
///
/// These types represent the logical data types in the raw data
/// before transformation to Silver layer PostgreSQL types.
///
/// # Variants
///
/// - `Float`: 64-bit floating point number
/// - `Int`: 64-bit signed integer
/// - `String`: UTF-8 text string
/// - `Bool`: Boolean true/false
/// - `Json`: JSON object or array
///
/// # Example
///
/// ```rust
/// use ndp_types::FieldType;
///
/// let field_type = FieldType::Float;
/// assert_eq!(field_type.as_ref(), "float");
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
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum FieldType {
    /// 64-bit floating point number.
    ///
    /// Supports `range` and `display_precision` field attributes.
    Float,

    /// 64-bit signed integer.
    ///
    /// Supports `range` field attribute but not `display_precision`.
    Int,

    /// UTF-8 text string.
    ///
    /// Does not support `range` or `display_precision` attributes.
    String,

    /// Boolean true/false value.
    ///
    /// Does not support `range` or `display_precision` attributes.
    Bool,

    /// JSON object or array.
    ///
    /// Stored as JSONB in Silver layer. Does not support range attributes.
    Json,
}

impl FieldType {
    /// Returns all field type names as a static slice.
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// Returns an iterator over all field type variants.
    pub fn all() -> impl Iterator<Item = Self> {
        use strum::IntoEnumIterator;
        Self::iter()
    }

    /// Check if this field type supports numeric range validation.
    pub fn supports_range(&self) -> bool {
        matches!(self, FieldType::Float | FieldType::Int)
    }

    /// Check if this field type supports display precision.
    pub fn supports_precision(&self) -> bool {
        matches!(self, FieldType::Float)
    }

    /// Get the default Silver layer type for this Bronze type.
    pub fn default_silver_type(&self) -> SilverFieldType {
        match self {
            FieldType::Float => SilverFieldType::DoublePrecision,
            FieldType::Int => SilverFieldType::Bigint,
            FieldType::String => SilverFieldType::Text,
            FieldType::Bool => SilverFieldType::Boolean,
            FieldType::Json => SilverFieldType::Jsonb,
        }
    }
}

/// PostgreSQL column types for Silver layer tables.
///
/// These types map directly to TimescaleDB/PostgreSQL types.
/// They are used in Silver ETL field mappings to define target column types.
///
/// # Variants
///
/// Numeric types:
/// - `DoublePrecision`: 64-bit floating point (DOUBLE PRECISION)
/// - `Real`: 32-bit floating point (REAL)
/// - `Integer`: 32-bit signed integer (INTEGER)
/// - `Bigint`: 64-bit signed integer (BIGINT)
/// - `Smallint`: 16-bit signed integer (SMALLINT)
///
/// String types:
/// - `Text`: Variable-length text (TEXT)
/// - `Varchar`: Variable-length text with limit (VARCHAR)
///
/// Other types:
/// - `Boolean`: Boolean (BOOLEAN)
/// - `Timestamptz`: Timestamp with timezone (TIMESTAMPTZ)
/// - `Jsonb`: Binary JSON (JSONB)
/// - `TextArray`: Text array (TEXT[])
///
/// # Example
///
/// ```rust
/// use ndp_types::SilverFieldType;
///
/// let silver_type = SilverFieldType::DoublePrecision;
/// assert_eq!(silver_type.as_ref(), "double_precision");
/// assert_eq!(silver_type.postgres_type(), "DOUBLE PRECISION");
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
pub enum SilverFieldType {
    /// 64-bit floating point (PostgreSQL DOUBLE PRECISION).
    DoublePrecision,

    /// 32-bit floating point (PostgreSQL REAL).
    Real,

    /// 32-bit signed integer (PostgreSQL INTEGER).
    Integer,

    /// 64-bit signed integer (PostgreSQL BIGINT).
    Bigint,

    /// 16-bit signed integer (PostgreSQL SMALLINT).
    Smallint,

    /// Variable-length text (PostgreSQL TEXT).
    Text,

    /// Variable-length text with limit (PostgreSQL VARCHAR).
    Varchar,

    /// Boolean (PostgreSQL BOOLEAN).
    Boolean,

    /// Timestamp with timezone (PostgreSQL TIMESTAMPTZ).
    Timestamptz,

    /// JSON binary (PostgreSQL JSONB).
    Jsonb,

    /// Text array (PostgreSQL TEXT[]).
    #[serde(rename = "text[]")]
    #[strum(serialize = "text[]")]
    TextArray,
}

impl SilverFieldType {
    /// Returns all Silver field type names as a static slice.
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// Returns an iterator over all Silver field type variants.
    pub fn all() -> impl Iterator<Item = Self> {
        use strum::IntoEnumIterator;
        Self::iter()
    }

    /// Get the PostgreSQL type name for this Silver type.
    pub fn postgres_type(&self) -> &'static str {
        match self {
            SilverFieldType::DoublePrecision => "DOUBLE PRECISION",
            SilverFieldType::Real => "REAL",
            SilverFieldType::Integer => "INTEGER",
            SilverFieldType::Bigint => "BIGINT",
            SilverFieldType::Smallint => "SMALLINT",
            SilverFieldType::Text => "TEXT",
            SilverFieldType::Varchar => "VARCHAR",
            SilverFieldType::Boolean => "BOOLEAN",
            SilverFieldType::Timestamptz => "TIMESTAMPTZ",
            SilverFieldType::Jsonb => "JSONB",
            SilverFieldType::TextArray => "TEXT[]",
        }
    }

    /// Check if this is a numeric type.
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            SilverFieldType::DoublePrecision
                | SilverFieldType::Real
                | SilverFieldType::Integer
                | SilverFieldType::Bigint
                | SilverFieldType::Smallint
        )
    }

    /// Check if this is a text type.
    pub fn is_text(&self) -> bool {
        matches!(self, SilverFieldType::Text | SilverFieldType::Varchar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn test_field_type_roundtrip() {
        for variant in FieldType::iter() {
            let serialized = serde_json::to_string(&variant).unwrap();
            let deserialized: FieldType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn test_field_type_all_names() {
        let names = FieldType::all_names();
        assert_eq!(names.len(), 5);
        assert!(names.contains(&"float"));
        assert!(names.contains(&"int"));
        assert!(names.contains(&"string"));
        assert!(names.contains(&"bool"));
        assert!(names.contains(&"json"));
    }

    #[test]
    fn test_silver_field_type_roundtrip() {
        for variant in SilverFieldType::iter() {
            let serialized = serde_json::to_string(&variant).unwrap();
            let deserialized: SilverFieldType = serde_json::from_str(&serialized).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn test_silver_field_type_all_names() {
        let names = SilverFieldType::all_names();
        assert_eq!(names.len(), 11);
        assert!(names.contains(&"double_precision"));
        assert!(names.contains(&"text[]"));
    }

    #[test]
    fn test_text_array_serialization() {
        let field_type = SilverFieldType::TextArray;
        let json = serde_json::to_string(&field_type).unwrap();
        assert_eq!(json, "\"text[]\"");

        let parsed: SilverFieldType = serde_json::from_str("\"text[]\"").unwrap();
        assert_eq!(parsed, SilverFieldType::TextArray);
    }

    #[test]
    fn test_supports_range() {
        assert!(FieldType::Float.supports_range());
        assert!(FieldType::Int.supports_range());
        assert!(!FieldType::String.supports_range());
        assert!(!FieldType::Bool.supports_range());
        assert!(!FieldType::Json.supports_range());
    }

    #[test]
    fn test_default_silver_type() {
        assert_eq!(
            FieldType::Float.default_silver_type(),
            SilverFieldType::DoublePrecision
        );
        assert_eq!(
            FieldType::Int.default_silver_type(),
            SilverFieldType::Bigint
        );
        assert_eq!(
            FieldType::String.default_silver_type(),
            SilverFieldType::Text
        );
        assert_eq!(
            FieldType::Bool.default_silver_type(),
            SilverFieldType::Boolean
        );
        assert_eq!(
            FieldType::Json.default_silver_type(),
            SilverFieldType::Jsonb
        );
    }

    #[test]
    fn test_postgres_type() {
        assert_eq!(
            SilverFieldType::DoublePrecision.postgres_type(),
            "DOUBLE PRECISION"
        );
        assert_eq!(SilverFieldType::TextArray.postgres_type(), "TEXT[]");
    }

    #[test]
    fn test_is_numeric() {
        assert!(SilverFieldType::DoublePrecision.is_numeric());
        assert!(SilverFieldType::Integer.is_numeric());
        assert!(!SilverFieldType::Text.is_numeric());
        assert!(!SilverFieldType::Jsonb.is_numeric());
    }
}
