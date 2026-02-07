//! Join clause generation for aligned views
//!
//! Generates SQL JOIN clauses based on join strategy:
//! - Full Outer: All rows from all streams
//! - Left: All rows from primary stream
//! - Inner: Only rows present in all streams
//!
//! Also handles forecast streams with LATERAL joins per ADR-FE001-003.

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

        // First stream is the FROM clause
        let primary = &streams[0];
        sql.push_str(&format!("FROM {} {}", primary.gold_table, primary.alias));

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
    /// Build a standard join clause (non-forecast)
    fn build_standard_join(
        &self,
        stream: &AlignedStream,
        all_streams: &[AlignedStream],
        index: usize,
        strategy: JoinStrategy,
    ) -> String {
        let join_keyword = strategy.sql_keyword();

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
            join_keyword, stream.gold_table, stream.alias, condition
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

        assert_eq!(sql, "FROM gold.air_quality_hourly indoor");
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
        assert!(sql.contains("gold.outdoor_weather_hourly outdoor"));
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
}
