//! Join clause generation for aligned views
//!
//! Generates SQL JOIN clauses based on join strategy:
//! - Full Outer: All rows from all streams
//! - Left: All rows from primary stream
//! - Inner: Only rows present in all streams
//!
//! Also handles forecast streams with LATERAL joins per ADR-FE001-003.
//!
//! Each non-forecast CA is wrapped in a subquery that collapses `ndp_id` by
//! grouping on `bucket` only. Underlying CAs group by `(bucket, ndp_id)`,
//! but the aligned view must produce exactly one row per bucket for
//! deterministic context enrichment in event detection.

use crate::gold::config::{AlignedStream, JoinStrategy, StreamType};

/// Trait for building JOIN clauses
pub trait JoinBuilder: Send + Sync {
    /// Build FROM and JOIN clauses for aligned streams
    ///
    /// # Arguments
    /// * `streams` - List of streams sorted by role (primary first)
    /// * `strategy` - Join strategy to use
    ///
    /// # Returns
    /// SQL FROM clause with all JOINs
    fn build_joins(&self, streams: &[AlignedStream], strategy: JoinStrategy) -> String;
}

/// Default join builder implementation
pub struct DefaultJoinBuilder;

impl JoinBuilder for DefaultJoinBuilder {
    fn build_joins(&self, streams: &[AlignedStream], strategy: JoinStrategy) -> String {
        if streams.is_empty() {
            return String::new();
        }

        let mut sql = String::new();

        // First stream is the FROM clause — wrapped in bucket subquery
        let primary = &streams[0];
        let table_expr = Self::build_bucket_subquery(primary);
        sql.push_str(&format!("FROM {} {}", table_expr, primary.alias));

        // Join remaining streams
        for (i, stream) in streams.iter().skip(1).enumerate() {
            sql.push('\n');

            // Handle forecast streams specially with LATERAL join
            if stream.stream_type == StreamType::Forecast {
                sql.push_str(&self.build_forecast_lateral_join(stream, streams, i));
            } else {
                sql.push_str(&self.build_standard_join(stream, streams, i, strategy));
            }
        }

        sql
    }
}

impl DefaultJoinBuilder {
    /// Wrap a CA table in a subquery that collapses ndp_id by grouping on bucket.
    ///
    /// Underlying CAs group by `(bucket, ndp_id)`. The aligned view needs exactly
    /// one row per bucket, so we re-aggregate each CA by bucket only, using
    /// appropriate aggregate functions derived from the column name suffix.
    fn build_bucket_subquery(stream: &AlignedStream) -> String {
        let mut agg_columns: Vec<String> = vec!["bucket".to_string()];

        for col in &stream.columns {
            if col == "bucket" {
                continue;
            }
            let agg_fn = Self::aggregate_for_column(col);
            agg_columns.push(format!("{}({}) AS {}", agg_fn, col, col));
        }

        format!(
            "(SELECT {} FROM {} GROUP BY bucket)",
            agg_columns.join(", "),
            stream.gold_table
        )
    }

    /// Derive the appropriate SQL aggregate function for a column
    /// based on its naming suffix convention.
    ///
    /// The CAs already contain per-entity aggregates (e.g. `co2_mean` is
    /// `AVG(co2)` per ndp_id). Re-aggregating across entities uses:
    /// - `_mean` → `AVG` (average of per-entity means)
    /// - `_min`  → `MIN` (true minimum across entities)
    /// - `_max`  → `MAX` (true maximum across entities)
    /// - `_std`  → `AVG` (approximate — not statistically precise)
    /// - `_p95`  → `MAX` (conservative — take highest p95)
    /// - `_count` / `sample_count` → `SUM` (total across entities)
    /// - `_first` → `MIN` (deterministic text-safe choice)
    /// - `_last`  → `MAX` (deterministic text-safe choice)
    fn aggregate_for_column(column: &str) -> &'static str {
        if column == "sample_count" {
            return "SUM";
        }
        if column.ends_with("_mean") {
            return "AVG";
        }
        if column.ends_with("_min") {
            return "MIN";
        }
        if column.ends_with("_max") {
            return "MAX";
        }
        if column.ends_with("_std") {
            return "AVG";
        }
        if column.ends_with("_p95") {
            return "MAX";
        }
        if column.ends_with("_count") {
            return "SUM";
        }
        if column.ends_with("_first") {
            return "MIN";
        }
        if column.ends_with("_last") {
            return "MAX";
        }
        "AVG" // safe default for numeric columns
    }

    /// Build a standard join clause (non-forecast)
    fn build_standard_join(
        &self,
        stream: &AlignedStream,
        all_streams: &[AlignedStream],
        index: usize,
        strategy: JoinStrategy,
    ) -> String {
        let join_keyword = strategy.sql_keyword();
        let table_expr = Self::build_bucket_subquery(stream);

        // Build the join condition
        let condition = match strategy {
            JoinStrategy::FullOuter => {
                // For full outer, coalesce all previous buckets
                if index == 0 {
                    format!("{}.bucket = {}.bucket", all_streams[0].alias, stream.alias)
                } else {
                    let previous_buckets: Vec<String> = all_streams[..=index]
                        .iter()
                        .map(|s| format!("{}.bucket", s.alias))
                        .collect();
                    format!(
                        "COALESCE({}) = {}.bucket",
                        previous_buckets.join(", "),
                        stream.alias
                    )
                }
            }
            JoinStrategy::Left | JoinStrategy::Inner => {
                // Simple equality with primary stream
                format!("{}.bucket = {}.bucket", all_streams[0].alias, stream.alias)
            }
        };

        format!(
            "{} {} {}\n    ON {}",
            join_keyword, table_expr, stream.alias, condition
        )
    }

    /// Build a LATERAL join for forecast streams (ADR-FE001-003)
    fn build_forecast_lateral_join(
        &self,
        stream: &AlignedStream,
        all_streams: &[AlignedStream],
        index: usize,
    ) -> String {
        // Build bucket expression from previous streams
        let bucket_expr = if index == 0 {
            format!("{}.bucket", all_streams[0].alias)
        } else {
            let buckets: Vec<String> = all_streams[..=index]
                .iter()
                .filter(|s| s.stream_type != StreamType::Forecast)
                .map(|s| format!("{}.bucket", s.alias))
                .collect();
            if buckets.len() == 1 {
                buckets[0].clone()
            } else {
                format!("COALESCE({})", buckets.join(", "))
            }
        };

        format!(
            r#"LEFT JOIN LATERAL (
    SELECT * FROM {} f
    WHERE f.issued_at <= {}
    ORDER BY f.issued_at DESC
    LIMIT 1
) {} ON TRUE"#,
            stream.gold_table, bucket_expr, stream.alias
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gold::config::{NullHandling, StreamRole};

    fn create_test_stream(
        stream_id: &str,
        alias: &str,
        role: StreamRole,
        stream_type: StreamType,
    ) -> AlignedStream {
        AlignedStream {
            stream_id: stream_id.to_string(),
            alias: alias.to_string(),
            role,
            stream_type,
            gold_table: format!("gold.{}_hourly", stream_id.replace("-", "_")),
            columns: vec!["bucket".to_string()],
            null_handling: NullHandling::Preserve,
        }
    }

    /// Helper to create a test stream with realistic columns
    fn create_stream_with_columns(
        stream_id: &str,
        alias: &str,
        role: StreamRole,
        stream_type: StreamType,
        columns: Vec<&str>,
    ) -> AlignedStream {
        AlignedStream {
            stream_id: stream_id.to_string(),
            alias: alias.to_string(),
            role,
            stream_type,
            gold_table: format!("gold.{}_hourly", stream_id.replace("-", "_")),
            columns: columns.into_iter().map(String::from).collect(),
            null_handling: NullHandling::Preserve,
        }
    }

    #[test]
    fn test_build_joins_single_stream() {
        let builder = DefaultJoinBuilder;
        let streams = vec![create_test_stream(
            "air-quality",
            "indoor",
            StreamRole::Primary,
            StreamType::Observation,
        )];

        let sql = builder.build_joins(&streams, JoinStrategy::FullOuter);

        // Single stream wrapped in bucket subquery
        assert!(sql.contains("FROM (SELECT bucket FROM gold.air_quality_hourly GROUP BY bucket) indoor"));
    }

    #[test]
    fn test_generates_full_outer_join_sql() {
        let builder = DefaultJoinBuilder;
        let streams = vec![
            create_test_stream(
                "air-quality",
                "indoor",
                StreamRole::Primary,
                StreamType::Observation,
            ),
            create_test_stream(
                "outdoor-weather",
                "outdoor",
                StreamRole::Context,
                StreamType::Observation,
            ),
        ];

        let sql = builder.build_joins(&streams, JoinStrategy::FullOuter);

        assert!(sql.contains("FULL OUTER JOIN"));
        assert!(sql.contains("gold.outdoor_weather_hourly"));
        assert!(sql.contains("indoor.bucket = outdoor.bucket"));
    }

    #[test]
    fn test_aligned_view_joins_on_bucket() {
        let builder = DefaultJoinBuilder;
        let streams = vec![
            create_test_stream(
                "air-quality",
                "aq",
                StreamRole::Primary,
                StreamType::Observation,
            ),
            create_test_stream(
                "outdoor-weather",
                "ow",
                StreamRole::Context,
                StreamType::Observation,
            ),
        ];

        let sql = builder.build_joins(&streams, JoinStrategy::FullOuter);

        assert!(sql.contains("ON aq.bucket = ow.bucket"));
    }

    #[test]
    fn test_full_outer_join_three_streams_coalesce() {
        let builder = DefaultJoinBuilder;
        let streams = vec![
            create_test_stream(
                "air-quality",
                "indoor",
                StreamRole::Primary,
                StreamType::Observation,
            ),
            create_test_stream(
                "outdoor-weather",
                "outdoor",
                StreamRole::Context,
                StreamType::Observation,
            ),
            create_test_stream(
                "home-assistant-state",
                "state",
                StreamRole::Actuator,
                StreamType::StateEvent,
            ),
        ];

        let sql = builder.build_joins(&streams, JoinStrategy::FullOuter);

        // Third stream should join on COALESCE of first two buckets
        assert!(sql.contains("COALESCE(indoor.bucket, outdoor.bucket) = state.bucket"));
    }

    #[test]
    fn test_left_join_strategy() {
        let builder = DefaultJoinBuilder;
        let streams = vec![
            create_test_stream(
                "air-quality",
                "indoor",
                StreamRole::Primary,
                StreamType::Observation,
            ),
            create_test_stream(
                "outdoor-weather",
                "outdoor",
                StreamRole::Context,
                StreamType::Observation,
            ),
        ];

        let sql = builder.build_joins(&streams, JoinStrategy::Left);

        assert!(sql.contains("LEFT JOIN"));
        assert!(!sql.contains("FULL OUTER JOIN"));
        // Left join uses simple equality with primary
        assert!(sql.contains("indoor.bucket = outdoor.bucket"));
    }

    #[test]
    fn test_inner_join_strategy() {
        let builder = DefaultJoinBuilder;
        let streams = vec![
            create_test_stream(
                "air-quality",
                "indoor",
                StreamRole::Primary,
                StreamType::Observation,
            ),
            create_test_stream(
                "outdoor-weather",
                "outdoor",
                StreamRole::Context,
                StreamType::Observation,
            ),
        ];

        let sql = builder.build_joins(&streams, JoinStrategy::Inner);

        assert!(sql.contains("INNER JOIN"));
        assert!(!sql.contains("FULL OUTER JOIN"));
    }

    #[test]
    fn test_forecast_stream_lateral_join() {
        let builder = DefaultJoinBuilder;
        let streams = vec![
            create_test_stream(
                "air-quality",
                "indoor",
                StreamRole::Primary,
                StreamType::Observation,
            ),
            create_test_stream(
                "nws-forecast",
                "forecast",
                StreamRole::Context,
                StreamType::Forecast,
            ),
        ];

        let sql = builder.build_joins(&streams, JoinStrategy::FullOuter);

        assert!(sql.contains("LEFT JOIN LATERAL"));
        assert!(sql.contains("WHERE f.issued_at <="));
        assert!(sql.contains("ORDER BY f.issued_at DESC"));
        assert!(sql.contains("LIMIT 1"));
        assert!(sql.contains(") forecast ON TRUE"));
    }

    #[test]
    fn test_forecast_with_multiple_observation_streams() {
        let builder = DefaultJoinBuilder;
        let streams = vec![
            create_test_stream(
                "air-quality",
                "indoor",
                StreamRole::Primary,
                StreamType::Observation,
            ),
            create_test_stream(
                "outdoor-weather",
                "outdoor",
                StreamRole::Context,
                StreamType::Observation,
            ),
            create_test_stream(
                "nws-forecast",
                "forecast",
                StreamRole::Context,
                StreamType::Forecast,
            ),
        ];

        let sql = builder.build_joins(&streams, JoinStrategy::FullOuter);

        // Forecast should use COALESCE of non-forecast buckets
        assert!(sql.contains("WHERE f.issued_at <= COALESCE(indoor.bucket, outdoor.bucket)"));
    }

    // ========== Bucket subquery tests ==========

    #[test]
    fn test_bucket_subquery_collapses_ndp_id() {
        let stream = create_stream_with_columns(
            "air-quality",
            "indoor",
            StreamRole::Primary,
            StreamType::Observation,
            vec!["bucket", "co2_mean", "co2_min", "co2_max", "sample_count"],
        );

        let subquery = DefaultJoinBuilder::build_bucket_subquery(&stream);

        assert!(subquery.contains("GROUP BY bucket"));
        assert!(subquery.contains("AVG(co2_mean) AS co2_mean"));
        assert!(subquery.contains("MIN(co2_min) AS co2_min"));
        assert!(subquery.contains("MAX(co2_max) AS co2_max"));
        assert!(subquery.contains("SUM(sample_count) AS sample_count"));
        // Should NOT include ndp_id
        assert!(!subquery.contains("ndp_id"));
    }

    #[test]
    fn test_bucket_subquery_state_event_columns() {
        let stream = create_stream_with_columns(
            "home-assistant-state",
            "state",
            StreamRole::Actuator,
            StreamType::StateEvent,
            vec!["bucket", "state_count", "state_first", "state_last", "sample_count"],
        );

        let subquery = DefaultJoinBuilder::build_bucket_subquery(&stream);

        assert!(subquery.contains("SUM(state_count) AS state_count"));
        assert!(subquery.contains("MIN(state_first) AS state_first"));
        assert!(subquery.contains("MAX(state_last) AS state_last"));
        assert!(subquery.contains("SUM(sample_count) AS sample_count"));
    }

    #[test]
    fn test_bucket_subquery_percentile_and_std() {
        let stream = create_stream_with_columns(
            "air-quality",
            "indoor",
            StreamRole::Primary,
            StreamType::Observation,
            vec!["bucket", "pm25_std", "pm25_p95"],
        );

        let subquery = DefaultJoinBuilder::build_bucket_subquery(&stream);

        assert!(subquery.contains("AVG(pm25_std) AS pm25_std"));
        assert!(subquery.contains("MAX(pm25_p95) AS pm25_p95"));
    }

    #[test]
    fn test_aggregate_for_column_mapping() {
        assert_eq!(DefaultJoinBuilder::aggregate_for_column("co2_mean"), "AVG");
        assert_eq!(DefaultJoinBuilder::aggregate_for_column("co2_min"), "MIN");
        assert_eq!(DefaultJoinBuilder::aggregate_for_column("co2_max"), "MAX");
        assert_eq!(DefaultJoinBuilder::aggregate_for_column("co2_std"), "AVG");
        assert_eq!(DefaultJoinBuilder::aggregate_for_column("pm25_p95"), "MAX");
        assert_eq!(DefaultJoinBuilder::aggregate_for_column("state_count"), "SUM");
        assert_eq!(DefaultJoinBuilder::aggregate_for_column("state_first"), "MIN");
        assert_eq!(DefaultJoinBuilder::aggregate_for_column("state_last"), "MAX");
        assert_eq!(DefaultJoinBuilder::aggregate_for_column("sample_count"), "SUM");
    }

    #[test]
    fn test_subquery_used_in_full_join() {
        let builder = DefaultJoinBuilder;
        let streams = vec![
            create_stream_with_columns(
                "air-quality",
                "indoor",
                StreamRole::Primary,
                StreamType::Observation,
                vec!["bucket", "co2_mean", "sample_count"],
            ),
            create_stream_with_columns(
                "home-assistant-state",
                "state",
                StreamRole::Actuator,
                StreamType::StateEvent,
                vec!["bucket", "state_count", "sample_count"],
            ),
        ];

        let sql = builder.build_joins(&streams, JoinStrategy::FullOuter);

        // Both sources should be subqueries with GROUP BY
        assert!(sql.contains("FROM (SELECT bucket, AVG(co2_mean) AS co2_mean, SUM(sample_count) AS sample_count FROM gold.air_quality_hourly GROUP BY bucket) indoor"));
        assert!(sql.contains("FULL OUTER JOIN (SELECT bucket, SUM(state_count) AS state_count, SUM(sample_count) AS sample_count FROM gold.home_assistant_state_hourly GROUP BY bucket) state"));
        assert!(sql.contains("indoor.bucket = state.bucket"));
    }
}
