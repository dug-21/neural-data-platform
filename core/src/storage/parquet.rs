use crate::error::{CoreError, CoreResult};
use crate::storage::wal::WriteAheadLog;
use crate::traits::{AggregatedPoint, AggregationType, HealthStatus, Store, TimeSeriesPoint};
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use polars::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ParquetStore {
    base_path: PathBuf,
    wal: Arc<Mutex<WriteAheadLog>>,
}

impl ParquetStore {
    pub fn new<P: AsRef<Path>>(base_path: P) -> CoreResult<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        std::fs::create_dir_all(&base_path)?;

        let wal_path = base_path.join("wal.log");
        let wal = WriteAheadLog::new(wal_path)?;

        Ok(Self {
            base_path,
            wal: Arc::new(Mutex::new(wal)),
        })
    }

    pub async fn replay_wal(&self) -> CoreResult<()> {
        let wal = self.wal.lock().await;
        let entries = wal.replay()?;

        if entries.is_empty() {
            return Ok(());
        }

        let mut points = Vec::new();
        for entry in entries {
            let point: TimeSeriesPoint = serde_json::from_slice(&entry).map_err(|e| {
                CoreError::Storage(format!("Failed to deserialize WAL entry: {}", e))
            })?;
            points.push(point);
        }

        drop(wal);

        if !points.is_empty() {
            self.write_batch(points).await?;
        }

        let mut wal = self.wal.lock().await;
        wal.commit()?;

        Ok(())
    }

    /// Build partition path using stream_id (preferred) or location_id as fallback
    /// This aligns storage structure with stream configuration for better discoverability
    fn partition_path(&self, stream_id: &str, timestamp: DateTime<Utc>) -> PathBuf {
        self.base_path
            .join("data")
            .join(stream_id)
            .join(format!("year={}", timestamp.year()))
            .join(format!("month={:02}", timestamp.month()))
            .join(format!("day={:02}", timestamp.day()))
            .join("readings.parquet")
    }

    /// Extract partition key from point: use stream_id tag if present, else location_id
    fn get_partition_key(point: &TimeSeriesPoint) -> String {
        point
            .tags
            .get("stream_id")
            .cloned()
            .unwrap_or_else(|| point.location_id.clone())
    }

    async fn write_parquet(&self, points: Vec<TimeSeriesPoint>, path: &Path) -> CoreResult<()> {
        if points.is_empty() {
            return Ok(());
        }

        let parent = path
            .parent()
            .ok_or_else(|| CoreError::Storage("Invalid path: no parent directory".to_string()))?;
        std::fs::create_dir_all(parent)?;

        let timestamps: Vec<i64> = points
            .iter()
            .map(|p| p.timestamp.timestamp_micros())
            .collect();

        let location_ids: Vec<String> = points.iter().map(|p| p.location_id.clone()).collect();

        let metrics: Vec<String> = points
            .iter()
            .map(|p| {
                p.tags
                    .get("metric")
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string())
            })
            .collect();

        let values: Vec<f64> = points.iter().map(|p| p.value).collect();

        let _tag_keys: Vec<Vec<String>> = points
            .iter()
            .map(|p| p.tags.keys().cloned().collect())
            .collect();

        let _tag_values: Vec<Vec<String>> = points
            .iter()
            .map(|p| p.tags.values().cloned().collect())
            .collect();

        let timestamp_series = Series::new("timestamp", timestamps);
        let location_series = Series::new("location_id", location_ids);
        let metric_series = Series::new("metric", metrics);
        let value_series = Series::new("value", values);

        let mut df = DataFrame::new(vec![
            timestamp_series,
            location_series,
            metric_series,
            value_series,
        ])
        .map_err(|e| CoreError::Storage(format!("Failed to create DataFrame: {}", e)))?;

        let file = std::fs::File::create(path)?;
        ParquetWriter::new(file)
            .with_compression(ParquetCompression::Snappy)
            .finish(&mut df)
            .map_err(|e| CoreError::Storage(format!("Failed to write Parquet: {}", e)))?;

        Ok(())
    }

    async fn append_to_parquet(&self, points: Vec<TimeSeriesPoint>, path: &Path) -> CoreResult<()> {
        let mut all_points = points;

        if path.exists() {
            let file = std::fs::File::open(path)?;
            let df = ParquetReader::new(file).finish().map_err(|e| {
                CoreError::Storage(format!("Failed to read existing Parquet: {}", e))
            })?;

            let timestamps = df
                .column("timestamp")
                .map_err(|e| CoreError::Storage(format!("Missing timestamp column: {}", e)))?
                .i64()
                .map_err(|e| CoreError::Storage(format!("Invalid timestamp type: {}", e)))?;

            let location_ids = df
                .column("location_id")
                .map_err(|e| CoreError::Storage(format!("Missing location_id column: {}", e)))?
                .utf8()
                .map_err(|e| CoreError::Storage(format!("Invalid location_id type: {}", e)))?;

            let metrics = df
                .column("metric")
                .map_err(|e| CoreError::Storage(format!("Missing metric column: {}", e)))?
                .utf8()
                .map_err(|e| CoreError::Storage(format!("Invalid metric type: {}", e)))?;

            let values = df
                .column("value")
                .map_err(|e| CoreError::Storage(format!("Missing value column: {}", e)))?
                .f64()
                .map_err(|e| CoreError::Storage(format!("Invalid value type: {}", e)))?;

            for i in 0..df.height() {
                if let (Some(ts), Some(loc), Some(metric), Some(val)) = (
                    timestamps.get(i),
                    location_ids.get(i),
                    metrics.get(i),
                    values.get(i),
                ) {
                    let timestamp = DateTime::from_timestamp_micros(ts)
                        .ok_or_else(|| CoreError::Storage("Invalid timestamp".to_string()))?;

                    let mut tags = HashMap::new();
                    tags.insert("metric".to_string(), metric.to_string());

                    all_points.push(TimeSeriesPoint {
                        timestamp,
                        location_id: loc.to_string(),
                        value: val,
                        tags,
                    });
                }
            }
        }

        self.write_parquet(all_points, path).await
    }
}

#[async_trait]
impl Store for ParquetStore {
    async fn write(&self, point: TimeSeriesPoint) -> CoreResult<()> {
        let mut wal = self.wal.lock().await;
        let entry = serde_json::to_vec(&point)
            .map_err(|e| CoreError::Storage(format!("Failed to serialize point: {}", e)))?;
        wal.append(&entry)?;
        drop(wal);

        let partition_key = Self::get_partition_key(&point);
        let path = self.partition_path(&partition_key, point.timestamp);
        self.append_to_parquet(vec![point], &path).await
    }

    async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> CoreResult<()> {
        if points.is_empty() {
            return Ok(());
        }

        let mut wal = self.wal.lock().await;
        for point in &points {
            let entry = serde_json::to_vec(point)
                .map_err(|e| CoreError::Storage(format!("Failed to serialize point: {}", e)))?;
            wal.append(&entry)?;
        }
        drop(wal);

        let mut grouped: HashMap<PathBuf, Vec<TimeSeriesPoint>> = HashMap::new();
        for point in points {
            let partition_key = Self::get_partition_key(&point);
            let path = self.partition_path(&partition_key, point.timestamp);
            grouped.entry(path).or_insert_with(Vec::new).push(point);
        }

        for (path, partition_points) in grouped {
            self.append_to_parquet(partition_points, &path).await?;
        }

        let mut wal = self.wal.lock().await;
        wal.commit()?;

        Ok(())
    }

    async fn query(
        &self,
        location_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        _filters: Option<HashMap<String, String>>,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let mut all_points = Vec::new();

        let mut current = start;
        while current <= end {
            let path = self.partition_path(location_id, current);

            if path.exists() {
                let file = std::fs::File::open(&path)?;
                let mut df = ParquetReader::new(file)
                    .finish()
                    .map_err(|e| CoreError::Storage(format!("Failed to read Parquet: {}", e)))?;

                df = df
                    .lazy()
                    .filter(
                        col("timestamp")
                            .gt_eq(lit(start.timestamp_micros()))
                            .and(col("timestamp").lt_eq(lit(end.timestamp_micros()))),
                    )
                    .collect()
                    .map_err(|e| CoreError::Storage(format!("Failed to filter data: {}", e)))?;

                let timestamps = df.column("timestamp")?.i64()?;
                let location_ids = df.column("location_id")?.utf8()?;
                let metrics = df.column("metric")?.utf8()?;
                let values = df.column("value")?.f64()?;

                for i in 0..df.height() {
                    if let (Some(ts), Some(loc), Some(metric), Some(val)) = (
                        timestamps.get(i),
                        location_ids.get(i),
                        metrics.get(i),
                        values.get(i),
                    ) {
                        let timestamp = DateTime::from_timestamp_micros(ts)
                            .ok_or_else(|| CoreError::Storage("Invalid timestamp".to_string()))?;

                        let mut tags = HashMap::new();
                        tags.insert("metric".to_string(), metric.to_string());

                        all_points.push(TimeSeriesPoint {
                            timestamp,
                            location_id: loc.to_string(),
                            value: val,
                            tags,
                        });
                    }
                }
            }

            current = current + chrono::Duration::days(1);
        }

        Ok(all_points)
    }

    async fn aggregate(
        &self,
        location_id: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        aggregation: AggregationType,
        interval: chrono::Duration,
    ) -> CoreResult<Vec<AggregatedPoint>> {
        let points = self.query(location_id, start, end, None).await?;

        if points.is_empty() {
            return Ok(Vec::new());
        }

        let mut buckets: HashMap<DateTime<Utc>, Vec<f64>> = HashMap::new();

        for point in points {
            let bucket_ts =
                (point.timestamp.timestamp() / interval.num_seconds()) * interval.num_seconds();
            let bucket_time = DateTime::from_timestamp(bucket_ts, 0)
                .ok_or_else(|| CoreError::Storage("Invalid bucket timestamp".to_string()))?;

            buckets
                .entry(bucket_time)
                .or_insert_with(Vec::new)
                .push(point.value);
        }

        let mut results = Vec::new();
        for (timestamp, values) in buckets {
            let aggregated_value = match &aggregation {
                AggregationType::Mean => values.iter().sum::<f64>() / values.len() as f64,
                AggregationType::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
                AggregationType::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                AggregationType::Sum => values.iter().sum(),
                AggregationType::Count => values.len() as f64,
                AggregationType::Median => {
                    let mut sorted = values.clone();
                    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let mid = sorted.len() / 2;
                    if sorted.len() % 2 == 0 {
                        (sorted[mid - 1] + sorted[mid]) / 2.0
                    } else {
                        sorted[mid]
                    }
                }
                AggregationType::Percentile(p) => {
                    let mut sorted = values.clone();
                    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                    let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
                    sorted[idx]
                }
            };

            results.push(AggregatedPoint {
                timestamp,
                location_id: location_id.to_string(),
                value: aggregated_value,
                aggregation_type: aggregation.clone(),
            });
        }

        results.sort_by_key(|p| p.timestamp);

        Ok(results)
    }

    async fn health_check(&self) -> CoreResult<HealthStatus> {
        let mut details = HashMap::new();
        details.insert("storage_type".to_string(), "parquet".to_string());
        details.insert(
            "base_path".to_string(),
            self.base_path.display().to_string(),
        );

        let wal_exists = self.wal.lock().await.path().exists();
        details.insert("wal_exists".to_string(), wal_exists.to_string());

        let base_path_writable = self.base_path.exists()
            && std::fs::metadata(&self.base_path)
                .map(|m| !m.permissions().readonly())
                .unwrap_or(false);

        Ok(HealthStatus {
            healthy: base_path_writable,
            message: if base_path_writable {
                "Storage is healthy".to_string()
            } else {
                "Storage path not writable".to_string()
            },
            details,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn create_test_store() -> (ParquetStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store = ParquetStore::new(temp_dir.path()).unwrap();
        (store, temp_dir)
    }

    fn create_test_point(
        timestamp: DateTime<Utc>,
        location_id: &str,
        value: f64,
    ) -> TimeSeriesPoint {
        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), "test_metric".to_string());

        TimeSeriesPoint {
            timestamp,
            location_id: location_id.to_string(),
            value,
            tags,
        }
    }

    #[tokio::test]
    async fn test_partition_path_generation() {
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        let path = store.partition_path("sensor-001", timestamp);

        assert!(path.to_string_lossy().contains("data"));
        assert!(path.to_string_lossy().contains("sensor-001"));
        assert!(path.to_string_lossy().contains("year=2024"));
        assert!(path.to_string_lossy().contains("month=01"));
        assert!(path.to_string_lossy().contains("day=15"));
        assert!(path.to_string_lossy().ends_with("readings.parquet"));
    }

    #[tokio::test]
    async fn test_write_single_point() {
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let point = create_test_point(timestamp, "sensor-001", 42.5);

        let result = store.write(point.clone()).await;
        assert!(result.is_ok());

        let path = store.partition_path("sensor-001", timestamp);
        assert!(path.exists());
    }

    #[tokio::test]
    async fn test_write_batch() {
        let (store, _temp) = create_test_store();
        let base_time = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();

        let points: Vec<TimeSeriesPoint> = (0..5)
            .map(|i| {
                create_test_point(
                    base_time + chrono::Duration::minutes(i),
                    "sensor-001",
                    10.0 * i as f64,
                )
            })
            .collect();

        let result = store.write_batch(points).await;
        assert!(result.is_ok());

        let path = store.partition_path("sensor-001", base_time);
        assert!(path.exists());
    }

    #[tokio::test]
    async fn test_query_time_range() {
        let (store, _temp) = create_test_store();
        let base_time = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();

        let points: Vec<TimeSeriesPoint> = (0..10)
            .map(|i| {
                create_test_point(
                    base_time + chrono::Duration::minutes(i),
                    "sensor-001",
                    i as f64,
                )
            })
            .collect();

        store.write_batch(points).await.unwrap();

        let start = base_time + chrono::Duration::minutes(3);
        let end = base_time + chrono::Duration::minutes(7);

        let results = store.query("sensor-001", start, end, None).await.unwrap();

        assert!(results.len() >= 4 && results.len() <= 5);
        for point in results {
            assert!(point.timestamp >= start && point.timestamp <= end);
        }
    }

    #[tokio::test]
    async fn test_query_with_filters() {
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        let mut tags = HashMap::new();
        tags.insert("sensor_type".to_string(), "PM2.5".to_string());

        let point = TimeSeriesPoint {
            timestamp,
            location_id: "sensor-001".to_string(),
            value: 25.5,
            tags,
        };

        store.write(point).await.unwrap();

        let mut filters = HashMap::new();
        filters.insert("sensor_type".to_string(), "PM2.5".to_string());

        let start = timestamp - chrono::Duration::hours(1);
        let end = timestamp + chrono::Duration::hours(1);

        let results = store.query("sensor-001", start, end, Some(filters)).await;
        assert!(results.is_ok());
    }

    #[tokio::test]
    async fn test_aggregate_mean() {
        let (store, _temp) = create_test_store();
        let base_time = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();

        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let points: Vec<TimeSeriesPoint> = values
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                create_test_point(
                    base_time + chrono::Duration::minutes(i as i64),
                    "sensor-001",
                    v,
                )
            })
            .collect();

        store.write_batch(points).await.unwrap();

        let results = store
            .aggregate(
                "sensor-001",
                base_time,
                base_time + chrono::Duration::hours(1),
                AggregationType::Mean,
                chrono::Duration::hours(1),
            )
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].value, 30.0);
    }

    #[tokio::test]
    async fn test_aggregate_percentile() {
        let (store, _temp) = create_test_store();
        let base_time = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();

        let values: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        let points: Vec<TimeSeriesPoint> = values
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                create_test_point(
                    base_time + chrono::Duration::seconds(i as i64),
                    "sensor-001",
                    v,
                )
            })
            .collect();

        store.write_batch(points).await.unwrap();

        let results = store
            .aggregate(
                "sensor-001",
                base_time,
                base_time + chrono::Duration::hours(1),
                AggregationType::Percentile(95.0),
                chrono::Duration::hours(1),
            )
            .await
            .unwrap();

        assert!(!results.is_empty());
        assert!(results[0].value >= 95.0 && results[0].value <= 96.0);
    }

    #[tokio::test]
    async fn test_health_check() {
        let (store, _temp) = create_test_store();

        let health = store.health_check().await.unwrap();

        assert!(health.healthy);
        assert_eq!(
            health.details.get("storage_type"),
            Some(&"parquet".to_string())
        );
    }

    #[tokio::test]
    async fn test_wal_write() {
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let point = create_test_point(timestamp, "sensor-001", 42.5);

        store.write(point).await.unwrap();

        let wal = store.wal.lock().await;
        let entries = wal.replay().unwrap();
        assert!(!entries.is_empty());
    }

    #[tokio::test]
    async fn test_wal_replay_on_startup() {
        let temp_dir = TempDir::new().unwrap();

        {
            let store = ParquetStore::new(temp_dir.path()).unwrap();
            let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
            let point = create_test_point(timestamp, "sensor-001", 99.9);

            let mut wal = store.wal.lock().await;
            let entry = serde_json::to_vec(&point).unwrap();
            wal.append(&entry).unwrap();
        }

        let store = ParquetStore::new(temp_dir.path()).unwrap();
        store.replay_wal().await.unwrap();

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let start = timestamp - chrono::Duration::hours(1);
        let end = timestamp + chrono::Duration::hours(1);

        let results = store.query("sensor-001", start, end, None).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].value, 99.9);
    }

    #[tokio::test]
    async fn test_partition_pruning() {
        let (store, _temp) = create_test_store();

        let day1 = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
        let day2 = Utc.with_ymd_and_hms(2024, 1, 16, 10, 0, 0).unwrap();

        store
            .write(create_test_point(day1, "sensor-001", 10.0))
            .await
            .unwrap();
        store
            .write(create_test_point(day2, "sensor-001", 20.0))
            .await
            .unwrap();

        let path1 = store.partition_path("sensor-001", day1);
        let path2 = store.partition_path("sensor-001", day2);

        assert!(path1.exists());
        assert!(path2.exists());
        assert_ne!(path1, path2);
    }

    #[tokio::test]
    async fn test_multiple_locations() {
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        store
            .write(create_test_point(timestamp, "sensor-001", 10.0))
            .await
            .unwrap();
        store
            .write(create_test_point(timestamp, "sensor-002", 20.0))
            .await
            .unwrap();

        let start = timestamp - chrono::Duration::hours(1);
        let end = timestamp + chrono::Duration::hours(1);

        let results1 = store.query("sensor-001", start, end, None).await.unwrap();
        let results2 = store.query("sensor-002", start, end, None).await.unwrap();

        assert_eq!(results1.len(), 1);
        assert_eq!(results2.len(), 1);
        assert_eq!(results1[0].value, 10.0);
        assert_eq!(results2[0].value, 20.0);
    }

    #[tokio::test]
    async fn test_metric_column_persistence() {
        let (store, _temp) = create_test_store();
        let base_time = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();

        // Create points with different metrics
        let mut points = Vec::new();

        let mut temp_point = TimeSeriesPoint {
            timestamp: base_time,
            location_id: "sensor-001".to_string(),
            value: 23.5,
            tags: HashMap::new(),
        };
        temp_point
            .tags
            .insert("metric".to_string(), "temperature".to_string());
        points.push(temp_point);

        let mut humidity_point = TimeSeriesPoint {
            timestamp: base_time + chrono::Duration::minutes(1),
            location_id: "sensor-001".to_string(),
            value: 65.0,
            tags: HashMap::new(),
        };
        humidity_point
            .tags
            .insert("metric".to_string(), "humidity".to_string());
        points.push(humidity_point);

        let mut pm25_point = TimeSeriesPoint {
            timestamp: base_time + chrono::Duration::minutes(2),
            location_id: "sensor-001".to_string(),
            value: 12.3,
            tags: HashMap::new(),
        };
        pm25_point
            .tags
            .insert("metric".to_string(), "pm2_5".to_string());
        points.push(pm25_point);

        // Write batch
        store.write_batch(points).await.unwrap();

        // Query back
        let start = base_time - chrono::Duration::hours(1);
        let end = base_time + chrono::Duration::hours(1);
        let results = store.query("sensor-001", start, end, None).await.unwrap();

        // Verify we got all 3 points back
        assert_eq!(results.len(), 3);

        // Verify each metric was persisted correctly
        let temp_result = results
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"temperature".to_string()))
            .unwrap();
        assert_eq!(temp_result.value, 23.5);

        let humidity_result = results
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"humidity".to_string()))
            .unwrap();
        assert_eq!(humidity_result.value, 65.0);

        let pm25_result = results
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"pm2_5".to_string()))
            .unwrap();
        assert_eq!(pm25_result.value, 12.3);
    }

    #[tokio::test]
    async fn test_metric_column_default_to_unknown() {
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        // Create point without metric tag
        let point = TimeSeriesPoint {
            timestamp,
            location_id: "sensor-001".to_string(),
            value: 42.0,
            tags: HashMap::new(), // No metric tag
        };

        store.write(point).await.unwrap();

        // Query back
        let start = timestamp - chrono::Duration::hours(1);
        let end = timestamp + chrono::Duration::hours(1);
        let results = store.query("sensor-001", start, end, None).await.unwrap();

        assert_eq!(results.len(), 1);
        // Should default to "unknown" when metric tag is missing
        assert_eq!(results[0].tags.get("metric"), Some(&"unknown".to_string()));
        assert_eq!(results[0].value, 42.0);
    }

    // ========== MQTT ROUTING PARTITION KEY TESTS (REGRESSION PREVENTION) ==========

    #[tokio::test]
    async fn test_partition_key_uses_stream_id_over_location_id() {
        // CRITICAL REGRESSION TEST: Verify partition path uses stream_id, not device MAC
        // This is the storage-layer fix for MQTT routing
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        // Create point with device MAC as location_id but stream_id in tags
        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), "pm25".to_string());
        tags.insert("stream_id".to_string(), "air-quality".to_string()); // Router adds this

        let point = TimeSeriesPoint {
            timestamp,
            location_id: "d83bda1cd074".to_string(), // Device MAC
            value: 25.5,
            tags,
        };

        // Act - write point
        store.write(point.clone()).await.unwrap();

        // Assert - verify partition path uses "air-quality", NOT "d83bda1cd074"
        let path = store.partition_path("air-quality", timestamp);
        assert!(path.exists(), "File should exist in air-quality directory");
        assert!(path.to_string_lossy().contains("air-quality"));
        assert!(!path.to_string_lossy().contains("d83bda1cd074"));

        // Verify wrong path does NOT exist
        let wrong_path = store.partition_path("d83bda1cd074", timestamp);
        assert!(!wrong_path.exists(), "File should NOT exist in device MAC directory");
    }

    #[tokio::test]
    async fn test_partition_key_falls_back_to_location_id() {
        // When stream_id tag is missing, use location_id (backward compatibility)
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        // Point WITHOUT stream_id tag (old behavior)
        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), "temperature".to_string());

        let point = TimeSeriesPoint {
            timestamp,
            location_id: "legacy-sensor-001".to_string(),
            value: 22.5,
            tags,
        };

        store.write(point.clone()).await.unwrap();

        // Should use location_id as partition key
        let path = store.partition_path("legacy-sensor-001", timestamp);
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("legacy-sensor-001"));
    }

    #[tokio::test]
    async fn test_mqtt_points_written_to_stream_directory() {
        // End-to-end test: MQTT points go to correct directory
        let (store, temp_dir) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        // Simulate MQTT point after router enrichment
        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), "pm25".to_string());
        tags.insert("stream_id".to_string(), "air-quality".to_string());
        tags.insert("source_id".to_string(), "air-quality-Mqtt".to_string());
        tags.insert("device_mac".to_string(), "d83bda1cd074".to_string());

        let point = TimeSeriesPoint {
            timestamp,
            location_id: "d83bda1cd074".to_string(), // Device MAC from MQTT
            value: 25.5,
            tags,
        };

        // Write point
        store.write(point).await.unwrap();

        // Verify file structure
        let expected_path = temp_dir.path()
            .join("data")
            .join("air-quality") // Stream ID, NOT device MAC
            .join("year=2024")
            .join("month=01")
            .join("day=15")
            .join("readings.parquet");

        assert!(expected_path.exists(), "File should be in air-quality directory");

        // Verify wrong path does NOT exist
        let wrong_path = temp_dir.path()
            .join("data")
            .join("d83bda1cd074") // Device MAC directory should NOT exist
            .join("year=2024")
            .join("month=01")
            .join("day=15")
            .join("readings.parquet");

        assert!(!wrong_path.exists(), "File should NOT be in device MAC directory");
    }

    #[tokio::test]
    async fn test_get_partition_key_function() {
        // Unit test for get_partition_key helper
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        // Test 1: stream_id tag present (preferred)
        let mut tags1 = HashMap::new();
        tags1.insert("stream_id".to_string(), "air-quality".to_string());
        let point1 = TimeSeriesPoint {
            timestamp,
            location_id: "d83bda1cd074".to_string(),
            value: 25.5,
            tags: tags1,
        };
        assert_eq!(ParquetStore::get_partition_key(&point1), "air-quality");

        // Test 2: No stream_id tag (fallback to location_id)
        let point2 = TimeSeriesPoint {
            timestamp,
            location_id: "sensor-001".to_string(),
            value: 22.0,
            tags: HashMap::new(),
        };
        assert_eq!(ParquetStore::get_partition_key(&point2), "sensor-001");

        // Test 3: Both present (stream_id wins)
        let mut tags3 = HashMap::new();
        tags3.insert("stream_id".to_string(), "outdoor-weather".to_string());
        let point3 = TimeSeriesPoint {
            timestamp,
            location_id: "old-location".to_string(),
            value: 18.5,
            tags: tags3,
        };
        assert_eq!(ParquetStore::get_partition_key(&point3), "outdoor-weather");
    }
}
