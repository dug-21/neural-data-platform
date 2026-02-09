//! Shared constants for the NDP platform
//!
//! Single source of truth for metrics, schema names, and column names
//! used across Gold config validation, semantic validation, and DDL generation.

/// Valid aggregate metrics per SPEC-A01-gold-etl-schema.md
pub const VALID_METRICS: &[&str] = &[
    "mean", "std", "min", "max", "count", "p95", "p99", "first", "last",
];

/// Valid rolling window statistics (subset of VALID_METRICS)
pub const VALID_ROLLING_STATS: &[&str] = &["mean", "std", "min", "max"];

/// Gold schema name. All Gold layer objects are created in this schema.
pub const GOLD_SCHEMA: &str = "gold";

/// Silver schema name. All Silver layer tables live here.
pub const SILVER_SCHEMA: &str = "silver";

/// Default entity identifier column used across NDP streams.
pub const NDP_ENTITY_COLUMN: &str = "ndp_id";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_metrics_contains_all_expected() {
        let expected = &[
            "mean", "std", "min", "max", "count", "p95", "p99", "first", "last",
        ];
        for metric in expected {
            assert!(
                VALID_METRICS.contains(metric),
                "VALID_METRICS missing expected metric '{}'",
                metric
            );
        }
        // Also verify no unexpected items
        for metric in VALID_METRICS {
            assert!(
                expected.contains(metric),
                "VALID_METRICS contains unexpected metric '{}'",
                metric
            );
        }
    }

    #[test]
    fn test_valid_metrics_count() {
        assert_eq!(VALID_METRICS.len(), 9);
    }

    #[test]
    fn test_valid_rolling_stats_contains_all_expected() {
        let expected = &["mean", "std", "min", "max"];
        for stat in expected {
            assert!(
                VALID_ROLLING_STATS.contains(stat),
                "VALID_ROLLING_STATS missing expected stat '{}'",
                stat
            );
        }
        // Also verify no unexpected items
        for stat in VALID_ROLLING_STATS {
            assert!(
                expected.contains(stat),
                "VALID_ROLLING_STATS contains unexpected stat '{}'",
                stat
            );
        }
    }

    #[test]
    fn test_valid_rolling_stats_count() {
        assert_eq!(VALID_ROLLING_STATS.len(), 4);
    }

    #[test]
    fn test_gold_schema_value() {
        assert_eq!(GOLD_SCHEMA, "gold");
    }

    #[test]
    fn test_silver_schema_value() {
        assert_eq!(SILVER_SCHEMA, "silver");
    }

    #[test]
    fn test_ndp_entity_column_value() {
        assert_eq!(NDP_ENTITY_COLUMN, "ndp_id");
    }
}
