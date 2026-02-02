//! Strategy type definitions for Silver ETL.
//!
//! This module defines strategy enums for deduplication and partitioning
//! in the Silver layer.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, Display, EnumIter, EnumString, VariantNames};

/// Deduplication strategy for Silver ETL.
///
/// Determines how duplicate records (based on key columns) are handled
/// during Bronze-to-Silver ETL processing.
///
/// # Variants
///
/// - `Upsert`: Update existing row with new values (default)
/// - `Skip`: Skip new row if key exists
/// - `Replace`: Replace existing row entirely
///
/// # Example
///
/// ```rust
/// use ndp_types::DeduplicationStrategy;
///
/// let strategy = DeduplicationStrategy::default();
/// assert_eq!(strategy, DeduplicationStrategy::Upsert);
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
pub enum DeduplicationStrategy {
    /// Update existing row with new values.
    ///
    /// When a duplicate key is found, merge the new values
    /// with the existing row. This is the default strategy
    /// and works well for time-series data where later
    /// measurements should update earlier ones.
    #[default]
    Upsert,

    /// Skip new row if key exists.
    ///
    /// When a duplicate key is found, keep the existing row
    /// and discard the new one. Useful for idempotent processing
    /// where the first value seen should be preserved.
    Skip,

    /// Replace existing row entirely.
    ///
    /// When a duplicate key is found, delete the existing row
    /// and insert the new one. Unlike Upsert, this does not
    /// merge values.
    Replace,
}

impl DeduplicationStrategy {
    /// Returns all strategy names as a static slice.
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// Returns an iterator over all strategy variants.
    pub fn all() -> impl Iterator<Item = Self> {
        use strum::IntoEnumIterator;
        Self::iter()
    }

    /// Get the SQL ON CONFLICT clause for this strategy.
    ///
    /// The placeholder `{columns}` should be replaced with the update column list.
    pub fn sql_on_conflict(&self) -> &'static str {
        match self {
            DeduplicationStrategy::Upsert => "ON CONFLICT DO UPDATE SET {columns}",
            DeduplicationStrategy::Skip => "ON CONFLICT DO NOTHING",
            DeduplicationStrategy::Replace => "ON CONFLICT DO UPDATE SET {columns}",
        }
    }
}

/// Partitioning strategy for Bronze layer Parquet files.
///
/// Determines how data is partitioned in the Bronze layer storage.
///
/// # Variants
///
/// - `Daily`: Partition by day (default)
/// - `Hourly`: Partition by hour
/// - `Monthly`: Partition by month
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
pub enum PartitioningStrategy {
    /// Partition by day (YYYY-MM-DD).
    #[default]
    Daily,

    /// Partition by hour (YYYY-MM-DD-HH).
    Hourly,

    /// Partition by month (YYYY-MM).
    Monthly,
}

impl PartitioningStrategy {
    /// Returns all strategy names as a static slice.
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// Returns an iterator over all strategy variants.
    pub fn all() -> impl Iterator<Item = Self> {
        use strum::IntoEnumIterator;
        Self::iter()
    }

    /// Get the chrono format string for this partitioning strategy.
    pub fn format_string(&self) -> &'static str {
        match self {
            PartitioningStrategy::Daily => "%Y-%m-%d",
            PartitioningStrategy::Hourly => "%Y-%m-%d-%H",
            PartitioningStrategy::Monthly => "%Y-%m",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn test_deduplication_strategy_roundtrip() {
        for variant in DeduplicationStrategy::iter() {
            let serialized = serde_json::to_string(&variant).unwrap();
            let deserialized: DeduplicationStrategy = serde_json::from_str(&serialized).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn test_deduplication_strategy_count() {
        assert_eq!(DeduplicationStrategy::all_names().len(), 3);
    }

    #[test]
    fn test_deduplication_strategy_names() {
        let names = DeduplicationStrategy::all_names();
        assert!(names.contains(&"upsert"));
        assert!(names.contains(&"skip"));
        assert!(names.contains(&"replace"));
    }

    #[test]
    fn test_deduplication_strategy_default() {
        assert_eq!(
            DeduplicationStrategy::default(),
            DeduplicationStrategy::Upsert
        );
    }

    #[test]
    fn test_partitioning_strategy_roundtrip() {
        for variant in PartitioningStrategy::iter() {
            let serialized = serde_json::to_string(&variant).unwrap();
            let deserialized: PartitioningStrategy = serde_json::from_str(&serialized).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn test_partitioning_strategy_count() {
        assert_eq!(PartitioningStrategy::all_names().len(), 3);
    }

    #[test]
    fn test_partitioning_strategy_names() {
        let names = PartitioningStrategy::all_names();
        assert!(names.contains(&"daily"));
        assert!(names.contains(&"hourly"));
        assert!(names.contains(&"monthly"));
    }

    #[test]
    fn test_partitioning_strategy_default() {
        assert_eq!(PartitioningStrategy::default(), PartitioningStrategy::Daily);
    }

    #[test]
    fn test_format_string() {
        assert_eq!(PartitioningStrategy::Daily.format_string(), "%Y-%m-%d");
        assert_eq!(PartitioningStrategy::Hourly.format_string(), "%Y-%m-%d-%H");
        assert_eq!(PartitioningStrategy::Monthly.format_string(), "%Y-%m");
    }

    #[test]
    fn test_sql_on_conflict() {
        assert!(DeduplicationStrategy::Upsert
            .sql_on_conflict()
            .contains("DO UPDATE"));
        assert!(DeduplicationStrategy::Skip
            .sql_on_conflict()
            .contains("DO NOTHING"));
        assert!(DeduplicationStrategy::Replace
            .sql_on_conflict()
            .contains("DO UPDATE"));
    }
}
