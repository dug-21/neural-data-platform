//! Column expression generation for aligned views
//!
//! Generates SELECT column expressions with:
//! - Stream alias prefixing (e.g., indoor_pm25_mean)
//! - NULL handling based on stream type
//! - Bucket coalescing for FULL OUTER JOIN
//! - Sample count aggregation

use crate::gold::config::AlignedStream;
use crate::gold::generators::null_handler::get_null_handler;

/// Trait for building SELECT column expressions
pub trait ColumnBuilder: Send + Sync {
    /// Build all SELECT column expressions for aligned streams
    ///
    /// # Arguments
    /// * `streams` - List of aligned streams with their columns
    ///
    /// # Returns
    /// Vector of SQL column expressions
    fn build_select_columns(&self, streams: &[AlignedStream]) -> Vec<String>;
}

/// Default column builder implementation
pub struct DefaultColumnBuilder;

impl ColumnBuilder for DefaultColumnBuilder {
    fn build_select_columns(&self, streams: &[AlignedStream]) -> Vec<String> {
        let mut expressions = Vec::new();

        // 1. Build bucket COALESCE expression
        expressions.push(format!("    {}", self.build_bucket_expression(streams)));

        // 2. Build columns for each stream
        for stream in streams {
            // Add comment as part of the first column expression for this stream
            let stream_columns = self.build_stream_columns(stream, streams);
            if !stream_columns.is_empty() {
                // Add a blank line comment before each stream section
                expressions.push(format!(
                    "\n    -- {} ({:?})",
                    stream.alias, stream.stream_type
                ));
                expressions.extend(stream_columns);
            }
        }

        // 3. Add total samples column
        expressions.push("\n    -- Total samples".to_string());
        expressions.push(self.build_total_samples_expression(streams));

        expressions
    }
}

impl DefaultColumnBuilder {
    /// Build the bucket COALESCE expression
    fn build_bucket_expression(&self, streams: &[AlignedStream]) -> String {
        if streams.len() == 1 {
            format!("{}.bucket AS bucket", streams[0].alias)
        } else {
            let bucket_aliases: Vec<String> = streams
                .iter()
                .map(|s| format!("{}.bucket", s.alias))
                .collect();
            format!("COALESCE({}) AS bucket", bucket_aliases.join(", "))
        }
    }

    /// Build column expressions for a single stream
    fn build_stream_columns(
        &self,
        stream: &AlignedStream,
        all_streams: &[AlignedStream],
    ) -> Vec<String> {
        let null_handler = get_null_handler(stream.null_handling);
        let bucket_expr = self.get_bucket_expression(all_streams);

        let mut expressions = Vec::new();

        for column in &stream.columns {
            // Skip bucket column (handled separately)
            if column == "bucket" {
                continue;
            }

            let source_expr = format!("{}.{}", stream.alias, column);
            let target_alias = format!("{}_{}", stream.alias, column);

            let expr = null_handler.wrap_column(&source_expr, &target_alias, &bucket_expr);
            expressions.push(format!("    {}", expr));
        }

        // Add sample count column for this stream
        expressions.push(format!(
            "    COALESCE({}.sample_count, 0) AS {}_samples",
            stream.alias, stream.alias
        ));

        expressions
    }

    /// Get the bucket expression for window ordering
    fn get_bucket_expression(&self, streams: &[AlignedStream]) -> String {
        if streams.len() == 1 {
            format!("{}.bucket", streams[0].alias)
        } else {
            let bucket_aliases: Vec<String> = streams
                .iter()
                .map(|s| format!("{}.bucket", s.alias))
                .collect();
            format!("COALESCE({})", bucket_aliases.join(", "))
        }
    }

    /// Build the total samples expression
    fn build_total_samples_expression(&self, streams: &[AlignedStream]) -> String {
        let sample_exprs: Vec<String> = streams
            .iter()
            .map(|s| format!("COALESCE({}.sample_count, 0)", s.alias))
            .collect();
        format!("    {} AS total_samples", sample_exprs.join(" + "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gold::config::{NullHandling, StreamRole, StreamType};

    fn create_test_stream(
        alias: &str,
        stream_type: StreamType,
        null_handling: NullHandling,
        columns: Vec<&str>,
    ) -> AlignedStream {
        AlignedStream {
            stream_id: format!("{}-stream", alias),
            alias: alias.to_string(),
            role: StreamRole::Primary,
            stream_type,
            gold_table: format!("gold.{}_hourly", alias),
            columns: columns.iter().map(|s| s.to_string()).collect(),
            null_handling,
        }
    }

    #[test]
    fn test_column_aliasing_uses_stream_alias() {
        let builder = DefaultColumnBuilder;
        let streams = vec![create_test_stream(
            "indoor",
            StreamType::Observation,
            NullHandling::Preserve,
            vec!["bucket", "pm25_mean", "co2_mean"],
        )];

        let columns = builder.build_select_columns(&streams);
        let joined = columns.join("\n");

        assert!(joined.contains("indoor_pm25_mean"));
        assert!(joined.contains("indoor_co2_mean"));
    }

    #[test]
    fn test_bucket_coalesce_multiple_streams() {
        let builder = DefaultColumnBuilder;
        let streams = vec![
            create_test_stream(
                "indoor",
                StreamType::Observation,
                NullHandling::Preserve,
                vec!["bucket", "pm25_mean"],
            ),
            create_test_stream(
                "outdoor",
                StreamType::Observation,
                NullHandling::Preserve,
                vec!["bucket", "temp_mean"],
            ),
        ];

        let columns = builder.build_select_columns(&streams);
        let bucket_expr = &columns[0];

        assert!(bucket_expr.contains("COALESCE(indoor.bucket, outdoor.bucket)"));
    }

    #[test]
    fn test_null_handling_for_state_events() {
        let builder = DefaultColumnBuilder;
        let streams = vec![create_test_stream(
            "state",
            StreamType::StateEvent,
            NullHandling::CarryForward,
            vec!["bucket", "window_state"],
        )];

        let columns = builder.build_select_columns(&streams);
        let joined = columns.join("\n");

        assert!(joined.contains("COALESCE"));
        // PostgreSQL-compatible: cascading LAG instead of IGNORE NULLS
        assert!(joined.contains("LAG(state.window_state, 1)"));
        assert!(!joined.contains("IGNORE NULLS")); // Not PostgreSQL compatible
        assert!(joined.contains("state_window_state"));
    }

    #[test]
    fn test_observation_preserves_null() {
        let builder = DefaultColumnBuilder;
        let streams = vec![create_test_stream(
            "indoor",
            StreamType::Observation,
            NullHandling::Preserve,
            vec!["bucket", "pm25_mean"],
        )];

        let columns = builder.build_select_columns(&streams);
        let joined = columns.join("\n");

        // Should be simple passthrough, no COALESCE around the value
        assert!(joined.contains("indoor.pm25_mean AS indoor_pm25_mean"));
    }

    #[test]
    fn test_total_samples_column() {
        let builder = DefaultColumnBuilder;
        let streams = vec![
            create_test_stream(
                "indoor",
                StreamType::Observation,
                NullHandling::Preserve,
                vec!["bucket", "pm25_mean"],
            ),
            create_test_stream(
                "outdoor",
                StreamType::Observation,
                NullHandling::Preserve,
                vec!["bucket", "temp_mean"],
            ),
        ];

        let columns = builder.build_select_columns(&streams);
        let joined = columns.join("\n");

        assert!(joined.contains(
            "COALESCE(indoor.sample_count, 0) + COALESCE(outdoor.sample_count, 0) AS total_samples"
        ));
    }

    #[test]
    fn test_stream_samples_column() {
        let builder = DefaultColumnBuilder;
        let streams = vec![create_test_stream(
            "indoor",
            StreamType::Observation,
            NullHandling::Preserve,
            vec!["bucket", "pm25_mean"],
        )];

        let columns = builder.build_select_columns(&streams);
        let joined = columns.join("\n");

        assert!(joined.contains("COALESCE(indoor.sample_count, 0) AS indoor_samples"));
    }
}
