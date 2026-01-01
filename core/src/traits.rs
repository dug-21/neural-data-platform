use crate::error::CoreResult;
use crate::types::RawDataPoint;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub location_id: String,
    pub value: f64,
    pub tags: HashMap<String, String>,
    /// Stable source identifier from config (AIR-009)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ndp_id: Option<String>,
    /// Mutable attributes as JSON blob (AIR-009)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AggregatedPoint {
    pub timestamp: DateTime<Utc>,
    pub location_id: String,
    pub value: f64,
    pub aggregation_type: AggregationType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AggregationType {
    Mean,
    Median,
    Min,
    Max,
    Sum,
    Count,
    Percentile(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForecastedPoint {
    pub timestamp: DateTime<Utc>,
    pub location_id: String,
    pub predicted_value: f64,
    pub confidence_lower: f64,
    pub confidence_upper: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    pub mae: f64,
    pub rmse: f64,
    pub mape: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthStatus {
    pub healthy: bool,
    pub message: String,
    pub details: HashMap<String, String>,
}

#[async_trait]
pub trait Store: Send + Sync {
    async fn write(&self, point: TimeSeriesPoint) -> CoreResult<()>;
    async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> CoreResult<()>;

    async fn query(
        &self,
        location_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        filters: Option<HashMap<String, String>>,
    ) -> CoreResult<Vec<TimeSeriesPoint>>;

    async fn aggregate(
        &self,
        location_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        aggregation: AggregationType,
        interval: chrono::Duration,
    ) -> CoreResult<Vec<AggregatedPoint>>;

    async fn health_check(&self) -> CoreResult<HealthStatus>;
}

#[async_trait]
pub trait Source: Send + Sync {
    async fn fetch(&self) -> CoreResult<Vec<TimeSeriesPoint>>;
    async fn health_check(&self) -> CoreResult<HealthStatus>;
}

/// Source trait for raw JSON data (Bronze layer)
///
/// This trait defines the interface for fetching raw, untransformed data
/// from sources. The raw payload is stored exactly as received without
/// any parsing or field extraction.
///
/// # Design Rationale (DP-004 / ADR-001)
///
/// Sources implementing RawSource return RawDataPoint containing:
/// - `source_id`: Generated from stream_id + source type (e.g., "air-quality-Http")
/// - `ndp_id`: Stable identifier from source configuration
/// - `context`: Mutable metadata from source configuration
/// - `raw_payload`: Exact JSON response from the source
///
/// # Example
///
/// ```ignore
/// let source = HttpPollingSource::with_raw_config(...);
/// let raw_point = source.fetch_raw().await?;
/// println!("Source: {}, Payload: {}", raw_point.source_id, raw_point.raw_payload);
/// ```
#[async_trait]
pub trait RawSource: Send + Sync {
    /// Fetch raw data from source, returning unmodified payload.
    ///
    /// Returns a single RawDataPoint with the exact JSON response.
    /// For batch operations, use `fetch_raw_batch`.
    async fn fetch_raw(&self) -> CoreResult<RawDataPoint>;

    /// Fetch raw data from source as a batch of points.
    ///
    /// Default implementation calls fetch_raw once and wraps in Vec.
    /// Override for sources that naturally produce multiple points.
    async fn fetch_raw_batch(&self) -> CoreResult<Vec<RawDataPoint>> {
        let point = self.fetch_raw().await?;
        Ok(vec![point])
    }
}

/// Store trait for raw JSON data (Bronze layer)
///
/// This trait defines the interface for storing raw, untransformed data
/// from sources before any parsing or field extraction occurs.
///
/// # Design Rationale (ADR-001)
///
/// The Bronze layer stores exact source payloads as JSON blobs. This enables:
/// - Schema evolution without data loss
/// - Reprocessing historical data with new parsers
/// - Debugging and auditing source data
///
/// # Example
///
/// ```ignore
/// let store: Arc<dyn RawStore> = /* ... */;
/// let point = RawDataPoint::new("air-quality-Http", json!({"pm25": 12.5}))
///     .with_ndp_id("sensor-001")
///     .with_context(json!({"room": "office"}));
/// store.write_raw(point).await?;
/// ```
#[async_trait]
pub trait RawStore: Send + Sync {
    /// Write a single raw data point to Bronze storage
    async fn write_raw(&self, point: RawDataPoint) -> CoreResult<()>;

    /// Write a batch of raw data points to Bronze storage
    ///
    /// Implementations should optimize for batch writes (e.g., single Parquet file).
    async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> CoreResult<()>;

    /// Query raw data points by time range and optional source filter
    ///
    /// Returns all raw data points within the specified time range.
    /// If source_filter is provided, only returns points from that source.
    async fn query_raw(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        source_filter: Option<String>,
    ) -> CoreResult<Vec<RawDataPoint>>;
}

#[async_trait]
pub trait Forecast: Send + Sync {
    async fn train(&mut self, data: Vec<TimeSeriesPoint>) -> CoreResult<ModelMetrics>;

    async fn predict(&self, location_id: &str, horizon: usize) -> CoreResult<Vec<ForecastedPoint>>;

    async fn evaluate(&self, actual: Vec<TimeSeriesPoint>) -> CoreResult<ModelMetrics>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CoreError;
    use mockall::mock;
    use mockall::predicate::*;

    // ========== LONDON SCHOOL TDD: MOCK DEFINITIONS ==========

    mock! {
        pub Store {}

        #[async_trait]
        impl Store for Store {
            async fn write(&self, point: TimeSeriesPoint) -> CoreResult<()>;
            async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> CoreResult<()>;
            async fn query(
                &self,
                location_id: &str,
                start: DateTime<Utc>,
                end: DateTime<Utc>,
                filters: Option<HashMap<String, String>>,
            ) -> CoreResult<Vec<TimeSeriesPoint>>;
            async fn aggregate(
                &self,
                location_id: &str,
                start: DateTime<Utc>,
                end: DateTime<Utc>,
                aggregation: AggregationType,
                interval: chrono::Duration,
            ) -> CoreResult<Vec<AggregatedPoint>>;
            async fn health_check(&self) -> CoreResult<HealthStatus>;
        }
    }

    mock! {
        pub Source {}

        #[async_trait]
        impl Source for Source {
            async fn fetch(&self) -> CoreResult<Vec<TimeSeriesPoint>>;
            async fn health_check(&self) -> CoreResult<HealthStatus>;
        }
    }

    mock! {
        pub Forecast {}

        #[async_trait]
        impl Forecast for Forecast {
            async fn train(&mut self, data: Vec<TimeSeriesPoint>) -> CoreResult<ModelMetrics>;
            async fn predict(
                &self,
                location_id: &str,
                horizon: usize,
            ) -> CoreResult<Vec<ForecastedPoint>>;
            async fn evaluate(&self, actual: Vec<TimeSeriesPoint>) -> CoreResult<ModelMetrics>;
        }
    }

    mock! {
        pub RawStore {}

        #[async_trait]
        impl RawStore for RawStore {
            async fn write_raw(&self, point: RawDataPoint) -> CoreResult<()>;
            async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> CoreResult<()>;
            async fn query_raw(
                &self,
                start: DateTime<Utc>,
                end: DateTime<Utc>,
                source_filter: Option<String>,
            ) -> CoreResult<Vec<RawDataPoint>>;
        }
    }

    mock! {
        pub RawSource {}

        #[async_trait]
        impl RawSource for RawSource {
            async fn fetch_raw(&self) -> CoreResult<RawDataPoint>;
            async fn fetch_raw_batch(&self) -> CoreResult<Vec<RawDataPoint>>;
        }
    }

    // ========== UNIT TESTS: DATA STRUCTURES ==========

    #[test]
    fn test_aggregation_type_equality() {
        assert_eq!(AggregationType::Mean, AggregationType::Mean);
        assert_ne!(AggregationType::Mean, AggregationType::Median);
        assert_eq!(
            AggregationType::Percentile(95.0),
            AggregationType::Percentile(95.0)
        );
    }

    #[test]
    fn test_aggregation_type_variants() {
        let types = vec![
            AggregationType::Mean,
            AggregationType::Median,
            AggregationType::Min,
            AggregationType::Max,
            AggregationType::Sum,
            AggregationType::Count,
            AggregationType::Percentile(95.0),
        ];

        for agg_type in types {
            assert_eq!(agg_type, agg_type);
        }
    }

    #[test]
    fn test_health_status_creation() {
        let status = HealthStatus {
            healthy: true,
            message: "All systems operational".to_string(),
            details: HashMap::new(),
        };

        assert!(status.healthy);
        assert_eq!(status.message, "All systems operational");
    }

    #[test]
    fn test_health_status_unhealthy_with_details() {
        let mut details = HashMap::new();
        details.insert("error".to_string(), "Connection timeout".to_string());
        details.insert("retry_count".to_string(), "3".to_string());

        let status = HealthStatus {
            healthy: false,
            message: "Service degraded".to_string(),
            details,
        };

        assert!(!status.healthy);
        assert_eq!(status.details.get("error").unwrap(), "Connection timeout");
    }

    #[test]
    fn test_time_series_point_with_tags() {
        let mut tags = HashMap::new();
        tags.insert("sensor_type".to_string(), "PM2.5".to_string());

        let point = TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "sensor-001".to_string(),
            value: 25.5,
            tags,
            ndp_id: None,
            context: None,
        };

        assert_eq!(point.location_id, "sensor-001");
        assert_eq!(point.value, 25.5);
        assert_eq!(point.tags.get("sensor_type"), Some(&"PM2.5".to_string()));
    }

    #[test]
    fn test_time_series_point_equality() {
        let now = Utc::now();
        let point1 = TimeSeriesPoint {
            timestamp: now,
            location_id: "sensor-001".to_string(),
            value: 25.5,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        };

        let point2 = TimeSeriesPoint {
            timestamp: now,
            location_id: "sensor-001".to_string(),
            value: 25.5,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        };

        assert_eq!(point1, point2);
    }

    #[test]
    fn test_aggregated_point_creation() {
        let now = Utc::now();
        let point = AggregatedPoint {
            timestamp: now,
            location_id: "sensor-001".to_string(),
            value: 42.5,
            aggregation_type: AggregationType::Mean,
        };

        assert_eq!(point.timestamp, now);
        assert_eq!(point.location_id, "sensor-001");
        assert_eq!(point.value, 42.5);
        assert_eq!(point.aggregation_type, AggregationType::Mean);
    }

    #[test]
    fn test_forecasted_point_creation() {
        let future = Utc::now() + chrono::Duration::hours(1);
        let point = ForecastedPoint {
            timestamp: future,
            location_id: "sensor-001".to_string(),
            predicted_value: 50.0,
            confidence_lower: 45.0,
            confidence_upper: 55.0,
        };

        assert_eq!(point.predicted_value, 50.0);
        assert_eq!(point.confidence_lower, 45.0);
        assert_eq!(point.confidence_upper, 55.0);
        assert!(point.confidence_lower < point.predicted_value);
        assert!(point.confidence_upper > point.predicted_value);
    }

    #[test]
    fn test_model_metrics_creation() {
        let metrics = ModelMetrics {
            mae: 0.5,
            rmse: 0.7,
            mape: 5.0,
        };

        assert_eq!(metrics.mae, 0.5);
        assert_eq!(metrics.rmse, 0.7);
        assert_eq!(metrics.mape, 5.0);
    }

    #[test]
    fn test_time_series_point_serde() {
        let now = Utc::now();
        let mut tags = HashMap::new();
        tags.insert("sensor_type".to_string(), "PM2.5".to_string());

        let point = TimeSeriesPoint {
            timestamp: now,
            location_id: "sensor-001".to_string(),
            value: 25.5,
            tags: tags.clone(),
            ndp_id: None,
            context: None,
        };

        let json = serde_json::to_string(&point).unwrap();
        let deserialized: TimeSeriesPoint = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.location_id, point.location_id);
        assert_eq!(deserialized.value, point.value);
    }

    #[test]
    fn test_time_series_point_with_ndp_id() {
        let point = TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "sensor-001".to_string(),
            value: 25.5,
            tags: HashMap::new(),
            ndp_id: Some("air-quality-office-001".to_string()),
            context: None,
        };

        assert_eq!(point.ndp_id, Some("air-quality-office-001".to_string()));
    }

    #[test]
    fn test_time_series_point_with_context() {
        let context = serde_json::json!({
            "room": "office",
            "floor": 2
        });

        let point = TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "sensor-001".to_string(),
            value: 25.5,
            tags: HashMap::new(),
            ndp_id: None,
            context: Some(context.clone()),
        };

        assert_eq!(point.context, Some(context));
    }

    #[test]
    fn test_time_series_point_serde_with_new_fields() {
        let context = serde_json::json!({"key": "value"});
        let now = Utc::now();

        let point = TimeSeriesPoint {
            timestamp: now,
            location_id: "sensor-001".to_string(),
            value: 25.5,
            tags: HashMap::new(),
            ndp_id: Some("ndp-001".to_string()),
            context: Some(context.clone()),
        };

        let json = serde_json::to_string(&point).unwrap();
        let deserialized: TimeSeriesPoint = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.ndp_id, Some("ndp-001".to_string()));
        assert_eq!(deserialized.context, Some(context));
    }

    #[test]
    fn test_time_series_point_serde_skip_none_fields() {
        let point = TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "sensor-001".to_string(),
            value: 25.5,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        };

        let json = serde_json::to_string(&point).unwrap();

        // When ndp_id and context are None, they should not appear in JSON
        assert!(!json.contains("ndp_id"));
        assert!(!json.contains("context"));
    }

    #[test]
    fn test_time_series_point_backward_compatible_deserialization() {
        // Old JSON format without ndp_id and context
        let old_json = r#"{
            "timestamp": "2024-01-15T10:30:00Z",
            "location_id": "sensor-001",
            "value": 25.5,
            "tags": {}
        }"#;

        let deserialized: TimeSeriesPoint = serde_json::from_str(old_json).unwrap();

        assert_eq!(deserialized.location_id, "sensor-001");
        assert_eq!(deserialized.value, 25.5);
        assert_eq!(deserialized.ndp_id, None);
        assert_eq!(deserialized.context, None);
    }

    // ========== LONDON SCHOOL TDD: BEHAVIOR VERIFICATION TESTS ==========

    // Store trait behavior tests

    #[tokio::test]
    async fn test_store_write_single_point_interaction() {
        let mut mock_store = MockStore::new();

        let point = TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "sensor-001".to_string(),
            value: 23.5,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        };

        mock_store.expect_write().times(1).returning(|_| Ok(()));

        let result = mock_store.write(point).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_store_write_batch_interaction() {
        let mut mock_store = MockStore::new();

        let points = vec![
            TimeSeriesPoint {
                timestamp: Utc::now(),
                location_id: "sensor-001".to_string(),
                value: 23.5,
                tags: HashMap::new(),
                ndp_id: None,
                context: None,
            },
            TimeSeriesPoint {
                timestamp: Utc::now(),
                location_id: "sensor-002".to_string(),
                value: 65.0,
                tags: HashMap::new(),
                ndp_id: None,
                context: None,
            },
        ];

        mock_store
            .expect_write_batch()
            .times(1)
            .withf(|points| points.len() == 2)
            .returning(|_| Ok(()));

        let result = mock_store.write_batch(points).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_store_query_with_filters_interaction() {
        let mut mock_store = MockStore::new();

        let start = Utc::now();
        let end = start + chrono::Duration::hours(1);
        let mut filters = HashMap::new();
        filters.insert("sensor_type".to_string(), "PM2.5".to_string());

        let expected_point = TimeSeriesPoint {
            timestamp: start,
            location_id: "sensor-001".to_string(),
            value: 23.5,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        };

        mock_store
            .expect_query()
            .with(
                eq("sensor-001"),
                eq(start),
                eq(end),
                eq(Some(filters.clone())),
            )
            .times(1)
            .returning(move |_, _, _, _| Ok(vec![expected_point.clone()]));

        let result = mock_store
            .query("sensor-001", start, end, Some(filters))
            .await;
        assert!(result.is_ok());
        let points = result.unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 23.5);
    }

    #[tokio::test]
    async fn test_store_query_without_filters_interaction() {
        let mut mock_store = MockStore::new();

        let start = Utc::now();
        let end = start + chrono::Duration::hours(1);

        mock_store
            .expect_query()
            .with(eq("sensor-001"), eq(start), eq(end), eq(None))
            .times(1)
            .returning(|_, _, _, _| Ok(vec![]));

        let result = mock_store.query("sensor-001", start, end, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_store_aggregate_mean_interaction() {
        let mut mock_store = MockStore::new();

        let start = Utc::now();
        let end = start + chrono::Duration::hours(1);
        let interval = chrono::Duration::minutes(15);

        let expected_aggregated = AggregatedPoint {
            timestamp: start,
            location_id: "sensor-001".to_string(),
            value: 42.5,
            aggregation_type: AggregationType::Mean,
        };

        mock_store
            .expect_aggregate()
            .with(
                eq("sensor-001"),
                eq(start),
                eq(end),
                eq(AggregationType::Mean),
                eq(interval),
            )
            .times(1)
            .returning(move |_, _, _, _, _| Ok(vec![expected_aggregated.clone()]));

        let result = mock_store
            .aggregate("sensor-001", start, end, AggregationType::Mean, interval)
            .await;

        assert!(result.is_ok());
        let points = result.unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 42.5);
        assert_eq!(points[0].aggregation_type, AggregationType::Mean);
    }

    #[tokio::test]
    async fn test_store_aggregate_percentile_interaction() {
        let mut mock_store = MockStore::new();

        let start = Utc::now();
        let end = start + chrono::Duration::hours(1);
        let interval = chrono::Duration::minutes(15);

        let expected_aggregated = AggregatedPoint {
            timestamp: start,
            location_id: "sensor-001".to_string(),
            value: 95.0,
            aggregation_type: AggregationType::Percentile(95.0),
        };

        mock_store
            .expect_aggregate()
            .times(1)
            .returning(move |_, _, _, _, _| Ok(vec![expected_aggregated.clone()]));

        let result = mock_store
            .aggregate(
                "sensor-001",
                start,
                end,
                AggregationType::Percentile(95.0),
                interval,
            )
            .await;

        assert!(result.is_ok());
        let points = result.unwrap();
        assert_eq!(
            points[0].aggregation_type,
            AggregationType::Percentile(95.0)
        );
    }

    #[tokio::test]
    async fn test_store_health_check_healthy_interaction() {
        let mut mock_store = MockStore::new();

        let expected_status = HealthStatus {
            healthy: true,
            message: "All systems operational".to_string(),
            details: HashMap::new(),
        };

        mock_store
            .expect_health_check()
            .times(1)
            .returning(move || Ok(expected_status.clone()));

        let result = mock_store.health_check().await;
        assert!(result.is_ok());
        let status = result.unwrap();
        assert!(status.healthy);
    }

    #[tokio::test]
    async fn test_store_health_check_unhealthy_interaction() {
        let mut mock_store = MockStore::new();

        let mut details = HashMap::new();
        details.insert("error".to_string(), "Connection timeout".to_string());

        let expected_status = HealthStatus {
            healthy: false,
            message: "Service degraded".to_string(),
            details,
        };

        mock_store
            .expect_health_check()
            .times(1)
            .returning(move || Ok(expected_status.clone()));

        let result = mock_store.health_check().await;
        assert!(result.is_ok());
        let status = result.unwrap();
        assert!(!status.healthy);
        assert_eq!(status.details.get("error").unwrap(), "Connection timeout");
    }

    #[tokio::test]
    async fn test_store_complete_workflow_interaction() {
        let mut mock_store = MockStore::new();

        let start = Utc::now();
        let end = start + chrono::Duration::hours(1);
        let interval = chrono::Duration::minutes(15);

        let point = TimeSeriesPoint {
            timestamp: start,
            location_id: "sensor-001".to_string(),
            value: 23.5,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        };

        let point_for_query = point.clone();
        let point_for_write = point.clone();

        let mut seq = mockall::Sequence::new();

        // Write data
        mock_store
            .expect_write()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(()));

        // Query data
        mock_store
            .expect_query()
            .times(1)
            .in_sequence(&mut seq)
            .returning(move |_, _, _, _| Ok(vec![point_for_query.clone()]));

        // Aggregate data
        mock_store
            .expect_aggregate()
            .times(1)
            .in_sequence(&mut seq)
            .returning(move |_, _, _, _, _| {
                Ok(vec![AggregatedPoint {
                    timestamp: start,
                    location_id: "sensor-001".to_string(),
                    value: 23.5,
                    aggregation_type: AggregationType::Mean,
                }])
            });

        // Health check
        mock_store
            .expect_health_check()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|| {
                Ok(HealthStatus {
                    healthy: true,
                    message: "OK".to_string(),
                    details: HashMap::new(),
                })
            });

        // Execute workflow
        mock_store.write(point_for_write).await.unwrap();
        let queried = mock_store
            .query("sensor-001", start, end, None)
            .await
            .unwrap();
        assert_eq!(queried.len(), 1);

        let aggregated = mock_store
            .aggregate("sensor-001", start, end, AggregationType::Mean, interval)
            .await
            .unwrap();
        assert_eq!(aggregated.len(), 1);

        let health = mock_store.health_check().await.unwrap();
        assert!(health.healthy);
    }

    // Source trait behavior tests

    #[tokio::test]
    async fn test_source_fetch_interaction() {
        let mut mock_source = MockSource::new();

        let expected_points = vec![TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "sensor-001".to_string(),
            value: 23.5,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        }];

        mock_source
            .expect_fetch()
            .times(1)
            .returning(move || Ok(expected_points.clone()));

        let result = mock_source.fetch().await;
        assert!(result.is_ok());
        let points = result.unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 23.5);
    }

    #[tokio::test]
    async fn test_source_fetch_empty_interaction() {
        let mut mock_source = MockSource::new();

        mock_source.expect_fetch().times(1).returning(|| Ok(vec![]));

        let result = mock_source.fetch().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_source_health_check_interaction() {
        let mut mock_source = MockSource::new();

        let expected_status = HealthStatus {
            healthy: true,
            message: "Source operational".to_string(),
            details: HashMap::new(),
        };

        mock_source
            .expect_health_check()
            .times(1)
            .returning(move || Ok(expected_status.clone()));

        let result = mock_source.health_check().await;
        assert!(result.is_ok());
        assert!(result.unwrap().healthy);
    }

    #[tokio::test]
    async fn test_source_fetch_and_health_workflow() {
        let mut mock_source = MockSource::new();

        let mut seq = mockall::Sequence::new();

        mock_source
            .expect_health_check()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|| {
                Ok(HealthStatus {
                    healthy: true,
                    message: "OK".to_string(),
                    details: HashMap::new(),
                })
            });

        mock_source
            .expect_fetch()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|| {
                Ok(vec![TimeSeriesPoint {
                    timestamp: Utc::now(),
                    location_id: "sensor-001".to_string(),
                    value: 23.5,
                    tags: HashMap::new(),
                    ndp_id: None,
                    context: None,
                }])
            });

        // Execute workflow
        let health = mock_source.health_check().await.unwrap();
        assert!(health.healthy);

        let points = mock_source.fetch().await.unwrap();
        assert_eq!(points.len(), 1);
    }

    // Forecast trait behavior tests

    #[tokio::test]
    async fn test_forecast_train_interaction() {
        let mut mock_forecast = MockForecast::new();

        let training_data = vec![TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "sensor-001".to_string(),
            value: 23.5,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        }];

        let expected_metrics = ModelMetrics {
            mae: 0.5,
            rmse: 0.7,
            mape: 5.0,
        };

        mock_forecast
            .expect_train()
            .times(1)
            .withf(|data| data.len() == 1)
            .returning(move |_| Ok(expected_metrics.clone()));

        let result = mock_forecast.train(training_data).await;
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert_eq!(metrics.mae, 0.5);
    }

    #[tokio::test]
    async fn test_forecast_predict_interaction() {
        let mut mock_forecast = MockForecast::new();

        let future = Utc::now() + chrono::Duration::hours(1);
        let expected_forecast = ForecastedPoint {
            timestamp: future,
            location_id: "sensor-001".to_string(),
            predicted_value: 50.0,
            confidence_lower: 45.0,
            confidence_upper: 55.0,
        };

        mock_forecast
            .expect_predict()
            .with(eq("sensor-001"), eq(10))
            .times(1)
            .returning(move |_, _| Ok(vec![expected_forecast.clone()]));

        let result = mock_forecast.predict("sensor-001", 10).await;
        assert!(result.is_ok());
        let forecasts = result.unwrap();
        assert_eq!(forecasts.len(), 1);
        assert_eq!(forecasts[0].predicted_value, 50.0);
    }

    #[tokio::test]
    async fn test_forecast_predict_multiple_horizons() {
        let mut mock_forecast = MockForecast::new();

        let horizon = 5;
        let base_time = Utc::now();

        mock_forecast
            .expect_predict()
            .times(1)
            .returning(move |location_id, h| {
                let forecasts = (0..h)
                    .map(|i| ForecastedPoint {
                        timestamp: base_time + chrono::Duration::hours(i as i64),
                        location_id: location_id.to_string(),
                        predicted_value: 50.0 + i as f64,
                        confidence_lower: 45.0,
                        confidence_upper: 55.0,
                    })
                    .collect();
                Ok(forecasts)
            });

        let result = mock_forecast.predict("sensor-001", horizon).await;
        assert!(result.is_ok());
        let forecasts = result.unwrap();
        assert_eq!(forecasts.len(), horizon);
    }

    #[tokio::test]
    async fn test_forecast_evaluate_interaction() {
        let mut mock_forecast = MockForecast::new();

        let actual_data = vec![TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "sensor-001".to_string(),
            value: 23.5,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        }];

        let expected_metrics = ModelMetrics {
            mae: 0.3,
            rmse: 0.5,
            mape: 3.0,
        };

        mock_forecast
            .expect_evaluate()
            .times(1)
            .returning(move |_| Ok(expected_metrics.clone()));

        let result = mock_forecast.evaluate(actual_data).await;
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert_eq!(metrics.mae, 0.3);
    }

    #[tokio::test]
    async fn test_forecast_complete_ml_workflow() {
        let mut mock_forecast = MockForecast::new();

        let training_data = vec![TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "sensor-001".to_string(),
            value: 23.5,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        }];

        let expected_metrics_train = ModelMetrics {
            mae: 0.5,
            rmse: 0.7,
            mape: 5.0,
        };

        let expected_metrics_eval = ModelMetrics {
            mae: 0.5,
            rmse: 0.7,
            mape: 5.0,
        };

        let future = Utc::now() + chrono::Duration::hours(1);
        let expected_forecast = ForecastedPoint {
            timestamp: future,
            location_id: "sensor-001".to_string(),
            predicted_value: 50.0,
            confidence_lower: 45.0,
            confidence_upper: 55.0,
        };

        let mut seq = mockall::Sequence::new();

        // Train model
        mock_forecast
            .expect_train()
            .times(1)
            .in_sequence(&mut seq)
            .returning(move |_| Ok(expected_metrics_train.clone()));

        // Generate predictions
        mock_forecast
            .expect_predict()
            .times(1)
            .in_sequence(&mut seq)
            .returning(move |_, _| Ok(vec![expected_forecast.clone()]));

        // Evaluate model
        mock_forecast
            .expect_evaluate()
            .times(1)
            .in_sequence(&mut seq)
            .returning(move |_| Ok(expected_metrics_eval.clone()));

        // Execute workflow
        let metrics = mock_forecast.train(training_data.clone()).await.unwrap();
        assert_eq!(metrics.mae, 0.5);

        let predictions = mock_forecast.predict("sensor-001", 10).await.unwrap();
        assert_eq!(predictions.len(), 1);
        assert_eq!(predictions[0].predicted_value, 50.0);

        let eval_metrics = mock_forecast.evaluate(training_data).await.unwrap();
        assert_eq!(eval_metrics.rmse, 0.7);
    }

    // ========== ERROR HANDLING TESTS ==========

    #[tokio::test]
    async fn test_store_write_error_handling() {
        let mut mock_store = MockStore::new();

        let point = TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "sensor-001".to_string(),
            value: 23.5,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        };

        mock_store
            .expect_write()
            .times(1)
            .returning(|_| Err(CoreError::Storage("Connection failed".to_string())));

        let result = mock_store.write(point).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_source_fetch_error_handling() {
        let mut mock_source = MockSource::new();

        mock_source
            .expect_fetch()
            .times(1)
            .returning(|| Err(CoreError::Source("MQTT connection refused".to_string())));

        let result = mock_source.fetch().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_forecast_train_error_handling() {
        let mut mock_forecast = MockForecast::new();

        let training_data = vec![];

        mock_forecast.expect_train().times(1).returning(|_| {
            Err(CoreError::Forecast(
                "Insufficient training data".to_string(),
            ))
        });

        let result = mock_forecast.train(training_data).await;
        assert!(result.is_err());
    }

    // ========== CONTRACT VERIFICATION TESTS ==========

    #[tokio::test]
    async fn test_store_query_contract_empty_results() {
        let mut mock_store = MockStore::new();

        let start = Utc::now();
        let end = start + chrono::Duration::hours(1);

        mock_store
            .expect_query()
            .times(1)
            .returning(|_, _, _, _| Ok(vec![]));

        let result = mock_store.query("sensor-001", start, end, None).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_store_aggregate_contract_multiple_windows() {
        let mut mock_store = MockStore::new();

        let start = Utc::now();
        let end = start + chrono::Duration::hours(1);
        let interval = chrono::Duration::minutes(15);

        mock_store
            .expect_aggregate()
            .times(1)
            .returning(move |_, _, _, _, _| {
                Ok(vec![
                    AggregatedPoint {
                        timestamp: start,
                        location_id: "sensor-001".to_string(),
                        value: 40.0,
                        aggregation_type: AggregationType::Mean,
                    },
                    AggregatedPoint {
                        timestamp: start + chrono::Duration::minutes(15),
                        location_id: "sensor-001".to_string(),
                        value: 42.0,
                        aggregation_type: AggregationType::Mean,
                    },
                    AggregatedPoint {
                        timestamp: start + chrono::Duration::minutes(30),
                        location_id: "sensor-001".to_string(),
                        value: 44.0,
                        aggregation_type: AggregationType::Mean,
                    },
                    AggregatedPoint {
                        timestamp: start + chrono::Duration::minutes(45),
                        location_id: "sensor-001".to_string(),
                        value: 46.0,
                        aggregation_type: AggregationType::Mean,
                    },
                ])
            });

        let result = mock_store
            .aggregate("sensor-001", start, end, AggregationType::Mean, interval)
            .await;

        assert!(result.is_ok());
        let points = result.unwrap();
        assert_eq!(points.len(), 4);
    }

    #[tokio::test]
    async fn test_forecast_predict_contract_horizon_matches_output() {
        let mut mock_forecast = MockForecast::new();

        let horizon = 24;
        let base_time = Utc::now();

        mock_forecast
            .expect_predict()
            .times(1)
            .returning(move |location_id, h| {
                let forecasts = (0..h)
                    .map(|i| ForecastedPoint {
                        timestamp: base_time + chrono::Duration::hours(i as i64),
                        location_id: location_id.to_string(),
                        predicted_value: 50.0,
                        confidence_lower: 45.0,
                        confidence_upper: 55.0,
                    })
                    .collect();
                Ok(forecasts)
            });

        let result = mock_forecast.predict("sensor-001", horizon).await;
        assert!(result.is_ok());
        let forecasts = result.unwrap();
        assert_eq!(forecasts.len(), horizon);
    }

    // ========== RAW STORE BEHAVIOR TESTS (DP-004) ==========

    #[tokio::test]
    async fn test_raw_store_write_single_point_interaction() {
        let mut mock_store = MockRawStore::new();

        let point = RawDataPoint::new("test-Http", serde_json::json!({"pm25": 12.5}))
            .with_ndp_id("sensor-001")
            .with_context(serde_json::json!({"room": "office"}));

        mock_store.expect_write_raw().times(1).returning(|_| Ok(()));

        let result = mock_store.write_raw(point).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_raw_store_write_batch_interaction() {
        let mut mock_store = MockRawStore::new();

        let points = vec![
            RawDataPoint::new("source-1-Http", serde_json::json!({"value": 1})),
            RawDataPoint::new("source-2-Http", serde_json::json!({"value": 2})),
            RawDataPoint::new("source-3-Http", serde_json::json!({"value": 3})),
        ];

        mock_store
            .expect_write_raw_batch()
            .times(1)
            .withf(|points| points.len() == 3)
            .returning(|_| Ok(()));

        let result = mock_store.write_raw_batch(points).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_raw_store_write_error_handling() {
        let mut mock_store = MockRawStore::new();

        let point = RawDataPoint::new("test-Http", serde_json::json!({"value": 1}));

        mock_store
            .expect_write_raw()
            .times(1)
            .returning(|_| Err(CoreError::Storage("Disk full".to_string())));

        let result = mock_store.write_raw(point).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_raw_store_batch_write_error_handling() {
        let mut mock_store = MockRawStore::new();

        let points = vec![RawDataPoint::new(
            "test-Http",
            serde_json::json!({"value": 1}),
        )];

        mock_store
            .expect_write_raw_batch()
            .times(1)
            .returning(|_| Err(CoreError::Storage("Write failed".to_string())));

        let result = mock_store.write_raw_batch(points).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_raw_store_preserves_point_data() {
        use std::sync::{Arc, Mutex};

        let mut mock_store = MockRawStore::new();
        let captured_point: Arc<Mutex<Option<RawDataPoint>>> = Arc::new(Mutex::new(None));
        let captured_clone = captured_point.clone();

        let point = RawDataPoint::new(
            "capture-test-Http",
            serde_json::json!({
                "pm25": 12.5,
                "status": "active",
                "nested": {"key": "value"}
            }),
        )
        .with_ndp_id("capture-001")
        .with_context(serde_json::json!({"room": "lab"}));

        let expected_source_id = point.source_id.clone();
        let expected_ndp_id = point.ndp_id.clone();

        mock_store.expect_write_raw().times(1).returning(move |p| {
            *captured_clone.lock().unwrap() = Some(p);
            Ok(())
        });

        mock_store.write_raw(point).await.unwrap();

        let captured = captured_point.lock().unwrap();
        let captured_ref = captured.as_ref().unwrap();
        assert_eq!(captured_ref.source_id, expected_source_id);
        assert_eq!(captured_ref.ndp_id, expected_ndp_id);
        assert_eq!(captured_ref.raw_payload["pm25"], 12.5);
        assert_eq!(captured_ref.raw_payload["status"], "active");
        assert_eq!(captured_ref.raw_payload["nested"]["key"], "value");
    }

    #[tokio::test]
    async fn test_raw_store_workflow_single_then_batch() {
        let mut mock_store = MockRawStore::new();

        let mut seq = mockall::Sequence::new();

        // First: single write
        mock_store
            .expect_write_raw()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(()));

        // Then: batch write
        mock_store
            .expect_write_raw_batch()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| Ok(()));

        // Execute workflow
        let single_point = RawDataPoint::new("single-Http", serde_json::json!({"value": 1}));
        mock_store.write_raw(single_point).await.unwrap();

        let batch_points = vec![
            RawDataPoint::new("batch-1-Http", serde_json::json!({"value": 2})),
            RawDataPoint::new("batch-2-Http", serde_json::json!({"value": 3})),
        ];
        mock_store.write_raw_batch(batch_points).await.unwrap();
    }

    #[tokio::test]
    async fn test_raw_store_empty_batch_succeeds() {
        let mut mock_store = MockRawStore::new();

        mock_store
            .expect_write_raw_batch()
            .times(1)
            .withf(|points| points.is_empty())
            .returning(|_| Ok(()));

        let result = mock_store.write_raw_batch(vec![]).await;
        assert!(result.is_ok());
    }

    // ========== RAW SOURCE BEHAVIOR TESTS (DP-004 Cycle 8) ==========

    #[tokio::test]
    async fn test_raw_source_fetch_raw_interaction() {
        let mut mock_source = MockRawSource::new();

        let expected_point = RawDataPoint::new(
            "test-stream-Http",
            serde_json::json!({"pm25": 12.5, "status": "active"}),
        )
        .with_ndp_id("sensor-001")
        .with_context(serde_json::json!({"room": "office"}));

        mock_source.expect_fetch_raw().times(1).returning(move || {
            Ok(RawDataPoint::new(
                "test-stream-Http",
                serde_json::json!({"pm25": 12.5, "status": "active"}),
            )
            .with_ndp_id("sensor-001")
            .with_context(serde_json::json!({"room": "office"})))
        });

        let result = mock_source.fetch_raw().await;
        assert!(result.is_ok());

        let point = result.unwrap();
        assert_eq!(point.source_id, expected_point.source_id);
        assert_eq!(point.ndp_id, expected_point.ndp_id);
        assert_eq!(point.raw_payload["pm25"], 12.5);
        assert_eq!(point.raw_payload["status"], "active");
    }

    #[tokio::test]
    async fn test_raw_source_fetch_raw_batch_interaction() {
        let mut mock_source = MockRawSource::new();

        mock_source.expect_fetch_raw_batch().times(1).returning(|| {
            Ok(vec![
                RawDataPoint::new("source-1-Http", serde_json::json!({"value": 1})),
                RawDataPoint::new("source-2-Http", serde_json::json!({"value": 2})),
            ])
        });

        let result = mock_source.fetch_raw_batch().await;
        assert!(result.is_ok());

        let points = result.unwrap();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].source_id, "source-1-Http");
        assert_eq!(points[1].source_id, "source-2-Http");
    }

    #[tokio::test]
    async fn test_raw_source_fetch_raw_error_handling() {
        let mut mock_source = MockRawSource::new();

        mock_source
            .expect_fetch_raw()
            .times(1)
            .returning(|| Err(CoreError::Source("Connection timeout".to_string())));

        let result = mock_source.fetch_raw().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_raw_source_preserves_all_json_types() {
        let mut mock_source = MockRawSource::new();

        // Test that complex JSON payloads are preserved
        let complex_payload = serde_json::json!({
            "string": "hello",
            "number": 42,
            "float": 3.14159,
            "boolean": true,
            "null": null,
            "array": [1, "two", false, null],
            "nested": {
                "deep": {
                    "value": "preserved"
                }
            }
        });

        let expected_payload = complex_payload.clone();

        mock_source
            .expect_fetch_raw()
            .times(1)
            .returning(move || Ok(RawDataPoint::new("complex-Http", complex_payload.clone())));

        let result = mock_source.fetch_raw().await.unwrap();

        assert_eq!(result.raw_payload["string"], expected_payload["string"]);
        assert_eq!(result.raw_payload["number"], expected_payload["number"]);
        assert_eq!(result.raw_payload["float"], expected_payload["float"]);
        assert_eq!(result.raw_payload["boolean"], expected_payload["boolean"]);
        assert!(result.raw_payload["null"].is_null());
        assert_eq!(result.raw_payload["array"][1], "two");
        assert_eq!(
            result.raw_payload["nested"]["deep"]["value"],
            expected_payload["nested"]["deep"]["value"]
        );
    }

    #[tokio::test]
    async fn test_raw_source_workflow_fetch_then_store() {
        let mut mock_source = MockRawSource::new();
        let mut mock_store = MockRawStore::new();

        let mut seq = mockall::Sequence::new();

        // Fetch from source
        mock_source
            .expect_fetch_raw()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|| {
                Ok(RawDataPoint::new(
                    "workflow-test-Http",
                    serde_json::json!({"value": 42}),
                ))
            });

        // Store the fetched point
        mock_store
            .expect_write_raw()
            .times(1)
            .in_sequence(&mut seq)
            .withf(|point| point.source_id == "workflow-test-Http")
            .returning(|_| Ok(()));

        // Execute workflow
        let fetched = mock_source.fetch_raw().await.unwrap();
        mock_store.write_raw(fetched).await.unwrap();
    }
}
