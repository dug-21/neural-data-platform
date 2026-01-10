//! Prometheus metrics for Silver ETL
//!
//! Exposes metrics for monitoring ETL execution:
//! - Rows processed per stream
//! - DQ flag counts
//! - Execution duration
//! - Watermark positions

use prometheus::{Counter, Histogram, IntCounterVec, Registry};
use std::sync::OnceLock;

/// Global metrics registry
static METRICS: OnceLock<EtlMetrics> = OnceLock::new();

/// ETL metrics collection
pub struct EtlMetrics {
    /// Total rows processed per stream
    pub rows_processed: IntCounterVec,

    /// Rows with DQ flags per stream
    pub rows_flagged: IntCounterVec,

    /// Rows rejected per stream
    pub rows_rejected: IntCounterVec,

    /// ETL execution duration histogram
    pub duration_seconds: Histogram,

    /// ETL run counter
    pub runs_total: Counter,
}

impl EtlMetrics {
    /// Initialize metrics with the given registry
    pub fn init(registry: &Registry) -> &'static Self {
        METRICS.get_or_init(|| {
            let rows_processed = IntCounterVec::new(
                prometheus::opts!(
                    "silver_etl_rows_processed_total",
                    "Total rows processed per stream"
                ),
                &["stream_id"],
            )
            .expect("metric creation");

            let rows_flagged = IntCounterVec::new(
                prometheus::opts!(
                    "silver_etl_rows_flagged_total",
                    "Rows with DQ flags per stream"
                ),
                &["stream_id"],
            )
            .expect("metric creation");

            let rows_rejected = IntCounterVec::new(
                prometheus::opts!("silver_etl_rows_rejected_total", "Rows rejected per stream"),
                &["stream_id"],
            )
            .expect("metric creation");

            let duration_seconds = Histogram::with_opts(
                prometheus::HistogramOpts::new(
                    "silver_etl_duration_seconds",
                    "ETL execution duration in seconds",
                )
                .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]),
            )
            .expect("metric creation");

            let runs_total =
                Counter::new("silver_etl_runs_total", "Total ETL runs").expect("metric creation");

            // Register all metrics
            registry.register(Box::new(rows_processed.clone())).ok();
            registry.register(Box::new(rows_flagged.clone())).ok();
            registry.register(Box::new(rows_rejected.clone())).ok();
            registry.register(Box::new(duration_seconds.clone())).ok();
            registry.register(Box::new(runs_total.clone())).ok();

            Self {
                rows_processed,
                rows_flagged,
                rows_rejected,
                duration_seconds,
                runs_total,
            }
        })
    }

    /// Get the global metrics instance
    pub fn get() -> Option<&'static Self> {
        METRICS.get()
    }
}
