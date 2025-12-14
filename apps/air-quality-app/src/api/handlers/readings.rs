use crate::{error::ApiError, response::ApiResponse, ApiResult};
use axum::{
    extract::{Query, State},
    Json,
};
use neural_core::{AggregatedPoint, AggregationType, Store, TimeSeriesPoint};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Deserialize)]
pub struct LatestQuery {
    pub location_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ReadingsQuery {
    pub location_id: String,
    pub start: String, // ISO 8601 timestamp
    pub end: String,   // ISO 8601 timestamp
}

#[derive(Debug, Deserialize)]
pub struct AggregateQuery {
    pub location_id: String,
    pub start: String,
    pub end: String,
    pub interval: String, // 1m, 5m, 1h, 1d
    pub agg: String,      // mean, min, max, p50, p95
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reading {
    pub timestamp: String,
    pub location_id: String,
    pub value: f64,
    pub tags: HashMap<String, String>,
}

impl From<TimeSeriesPoint> for Reading {
    fn from(point: TimeSeriesPoint) -> Self {
        Self {
            timestamp: point.timestamp.to_rfc3339(),
            location_id: point.location_id,
            value: point.value,
            tags: point.tags,
        }
    }
}

pub async fn latest_readings_handler(
    State(store): State<Arc<dyn Store>>,
    Query(query): Query<LatestQuery>,
) -> ApiResult<Json<ApiResponse<Reading>>> {
    let end = chrono::Utc::now();
    let start = end - chrono::Duration::hours(1);

    let points = store
        .query(&query.location_id, start, end, None)
        .await
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    let latest = points
        .into_iter()
        .max_by_key(|p| p.timestamp)
        .ok_or_else(|| ApiError::NotFound(format!("No readings found for location {}", query.location_id)))?;

    Ok(Json(ApiResponse::success(Reading::from(latest))))
}

pub async fn readings_handler(
    State(store): State<Arc<dyn Store>>,
    Query(query): Query<ReadingsQuery>,
) -> ApiResult<Json<ApiResponse<Vec<Reading>>>> {
    let start = chrono::DateTime::parse_from_rfc3339(&query.start)
        .map_err(|_| ApiError::BadRequest("Invalid start timestamp".to_string()))?
        .with_timezone(&chrono::Utc);

    let end = chrono::DateTime::parse_from_rfc3339(&query.end)
        .map_err(|_| ApiError::BadRequest("Invalid end timestamp".to_string()))?
        .with_timezone(&chrono::Utc);

    let points = store
        .query(&query.location_id, start, end, None)
        .await
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    let readings: Vec<Reading> = points.into_iter().map(Reading::from).collect();

    Ok(Json(ApiResponse::success(readings)))
}

pub async fn aggregate_handler(
    State(store): State<Arc<dyn Store>>,
    Query(query): Query<AggregateQuery>,
) -> ApiResult<Json<ApiResponse<Vec<AggregatedPoint>>>> {
    let start = chrono::DateTime::parse_from_rfc3339(&query.start)
        .map_err(|_| ApiError::BadRequest("Invalid start timestamp".to_string()))?
        .with_timezone(&chrono::Utc);

    let end = chrono::DateTime::parse_from_rfc3339(&query.end)
        .map_err(|_| ApiError::BadRequest("Invalid end timestamp".to_string()))?
        .with_timezone(&chrono::Utc);

    let interval = parse_interval(&query.interval)
        .ok_or_else(|| ApiError::BadRequest("Invalid interval".to_string()))?;

    let aggregation = parse_aggregation(&query.agg)
        .ok_or_else(|| ApiError::BadRequest("Invalid aggregation type".to_string()))?;

    let points = store
        .aggregate(&query.location_id, start, end, aggregation, interval)
        .await
        .map_err(|e| ApiError::InternalError(e.to_string()))?;

    Ok(Json(ApiResponse::success(points)))
}

fn parse_interval(interval: &str) -> Option<chrono::Duration> {
    match interval {
        "1m" => Some(chrono::Duration::minutes(1)),
        "5m" => Some(chrono::Duration::minutes(5)),
        "1h" => Some(chrono::Duration::hours(1)),
        "1d" => Some(chrono::Duration::days(1)),
        _ => None,
    }
}

fn parse_aggregation(agg: &str) -> Option<AggregationType> {
    match agg {
        "mean" => Some(AggregationType::Mean),
        "min" => Some(AggregationType::Min),
        "max" => Some(AggregationType::Max),
        "p50" => Some(AggregationType::Percentile(50.0)),
        "p95" => Some(AggregationType::Percentile(95.0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neural_core::CoreError;
    use mockall::mock;

    mock! {
        pub TestStore {}

        #[async_trait::async_trait]
        impl Store for TestStore {
            async fn write(&self, point: TimeSeriesPoint) -> Result<(), CoreError>;
            async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> Result<(), CoreError>;
            async fn query(
                &self,
                location_id: &str,
                start: chrono::DateTime<chrono::Utc>,
                end: chrono::DateTime<chrono::Utc>,
                filters: Option<HashMap<String, String>>,
            ) -> Result<Vec<TimeSeriesPoint>, CoreError>;
            async fn aggregate(
                &self,
                location_id: &str,
                start: chrono::DateTime<chrono::Utc>,
                end: chrono::DateTime<chrono::Utc>,
                aggregation: AggregationType,
                interval: chrono::Duration,
            ) -> Result<Vec<AggregatedPoint>, CoreError>;
            async fn health_check(&self) -> Result<neural_core::HealthStatus, CoreError>;
        }
    }

    #[tokio::test]
    async fn test_latest_readings_success() {
        let mut mock = MockTestStore::new();
        let now = chrono::Utc::now();

        mock.expect_query().returning(move |_loc, _start, _end, _filters| {
            Ok(vec![
                TimeSeriesPoint {
                    timestamp: now - chrono::Duration::minutes(10),
                    location_id: "test-loc".to_string(),
                    value: 10.0,
                    tags: HashMap::new(),
                },
                TimeSeriesPoint {
                    timestamp: now - chrono::Duration::minutes(5),
                    location_id: "test-loc".to_string(),
                    value: 20.0,
                    tags: HashMap::new(),
                },
            ])
        });

        let query = LatestQuery {
            location_id: "test-loc".to_string(),
        };

        let result = latest_readings_handler(State(Arc::new(mock)), Query(query)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.value, 20.0);
        assert_eq!(response.data.location_id, "test-loc");
    }

    #[tokio::test]
    async fn test_latest_readings_not_found() {
        let mut mock = MockTestStore::new();

        mock.expect_query()
            .returning(|_loc, _start, _end, _filters| Ok(vec![]));

        let query = LatestQuery {
            location_id: "nonexistent".to_string(),
        };

        let result = latest_readings_handler(State(Arc::new(mock)), Query(query)).await;

        assert!(result.is_err());
        match result {
            Err(ApiError::NotFound(msg)) => assert!(msg.contains("nonexistent")),
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test]
    async fn test_readings_handler_success() {
        let mut mock = MockTestStore::new();
        let now = chrono::Utc::now();

        mock.expect_query().returning(move |_loc, _start, _end, _filters| {
            Ok(vec![TimeSeriesPoint {
                timestamp: now,
                location_id: "test-loc".to_string(),
                value: 15.0,
                tags: HashMap::new(),
            }])
        });

        let query = ReadingsQuery {
            location_id: "test-loc".to_string(),
            start: (now - chrono::Duration::hours(1)).to_rfc3339(),
            end: now.to_rfc3339(),
        };

        let result = readings_handler(State(Arc::new(mock)), Query(query)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].value, 15.0);
    }

    #[tokio::test]
    async fn test_readings_handler_invalid_timestamp() {
        let mock = MockTestStore::new();

        let query = ReadingsQuery {
            location_id: "test-loc".to_string(),
            start: "invalid".to_string(),
            end: "also-invalid".to_string(),
        };

        let result = readings_handler(State(Arc::new(mock)), Query(query)).await;

        assert!(result.is_err());
        match result {
            Err(ApiError::BadRequest(msg)) => assert!(msg.contains("Invalid")),
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[tokio::test]
    async fn test_aggregate_handler_mean() {
        let mut mock = MockTestStore::new();
        let now = chrono::Utc::now();

        mock.expect_aggregate().returning(move |_loc, _start, _end, agg, _interval| {
            assert_eq!(agg, AggregationType::Mean);
            Ok(vec![AggregatedPoint {
                timestamp: now,
                location_id: "test-loc".to_string(),
                value: 25.0,
                aggregation_type: AggregationType::Mean,
            }])
        });

        let query = AggregateQuery {
            location_id: "test-loc".to_string(),
            start: (now - chrono::Duration::hours(1)).to_rfc3339(),
            end: now.to_rfc3339(),
            interval: "5m".to_string(),
            agg: "mean".to_string(),
        };

        let result = aggregate_handler(State(Arc::new(mock)), Query(query)).await;

        assert!(result.is_ok());
        let response = result.unwrap().0;
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].value, 25.0);
    }

    #[tokio::test]
    async fn test_aggregate_handler_percentile() {
        let mut mock = MockTestStore::new();
        let now = chrono::Utc::now();

        mock.expect_aggregate().returning(move |_loc, _start, _end, agg, _interval| {
            assert_eq!(agg, AggregationType::Percentile(95.0));
            Ok(vec![AggregatedPoint {
                timestamp: now,
                location_id: "test-loc".to_string(),
                value: 95.0,
                aggregation_type: AggregationType::Percentile(95.0),
            }])
        });

        let query = AggregateQuery {
            location_id: "test-loc".to_string(),
            start: (now - chrono::Duration::hours(1)).to_rfc3339(),
            end: now.to_rfc3339(),
            interval: "1h".to_string(),
            agg: "p95".to_string(),
        };

        let result = aggregate_handler(State(Arc::new(mock)), Query(query)).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_aggregate_handler_invalid_interval() {
        let mock = MockTestStore::new();
        let now = chrono::Utc::now();

        let query = AggregateQuery {
            location_id: "test-loc".to_string(),
            start: (now - chrono::Duration::hours(1)).to_rfc3339(),
            end: now.to_rfc3339(),
            interval: "invalid".to_string(),
            agg: "mean".to_string(),
        };

        let result = aggregate_handler(State(Arc::new(mock)), Query(query)).await;

        assert!(result.is_err());
        match result {
            Err(ApiError::BadRequest(msg)) => assert!(msg.contains("interval")),
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[tokio::test]
    async fn test_aggregate_handler_invalid_aggregation() {
        let mock = MockTestStore::new();
        let now = chrono::Utc::now();

        let query = AggregateQuery {
            location_id: "test-loc".to_string(),
            start: (now - chrono::Duration::hours(1)).to_rfc3339(),
            end: now.to_rfc3339(),
            interval: "1h".to_string(),
            agg: "invalid".to_string(),
        };

        let result = aggregate_handler(State(Arc::new(mock)), Query(query)).await;

        assert!(result.is_err());
        match result {
            Err(ApiError::BadRequest(msg)) => assert!(msg.contains("aggregation")),
            _ => panic!("Expected BadRequest error"),
        }
    }

    #[test]
    fn test_parse_interval() {
        assert_eq!(parse_interval("1m"), Some(chrono::Duration::minutes(1)));
        assert_eq!(parse_interval("5m"), Some(chrono::Duration::minutes(5)));
        assert_eq!(parse_interval("1h"), Some(chrono::Duration::hours(1)));
        assert_eq!(parse_interval("1d"), Some(chrono::Duration::days(1)));
        assert_eq!(parse_interval("invalid"), None);
    }

    #[test]
    fn test_parse_aggregation() {
        assert_eq!(parse_aggregation("mean"), Some(AggregationType::Mean));
        assert_eq!(parse_aggregation("min"), Some(AggregationType::Min));
        assert_eq!(parse_aggregation("max"), Some(AggregationType::Max));
        assert_eq!(parse_aggregation("p50"), Some(AggregationType::Percentile(50.0)));
        assert_eq!(parse_aggregation("p95"), Some(AggregationType::Percentile(95.0)));
        assert_eq!(parse_aggregation("invalid"), None);
    }
}
