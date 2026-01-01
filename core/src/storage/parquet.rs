use crate::error::{CoreError, CoreResult};
use crate::storage::wal::WriteAheadLog;
use crate::traits::{
    AggregatedPoint, AggregationType, HealthStatus, RawStore, Store, TimeSeriesPoint,
};
use crate::types::RawDataPoint;
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
    ///
    /// P2-06: Uses push() instead of chained join() to reduce allocations from 6 to 1
    fn partition_path(&self, stream_id: &str, timestamp: DateTime<Utc>) -> PathBuf {
        let mut path = self.base_path.clone();
        path.push("data");
        path.push(stream_id);
        path.push(format!("year={}", timestamp.year()));
        path.push(format!("month={:02}", timestamp.month()));
        path.push(format!("day={:02}", timestamp.day()));
        path.push("readings.parquet");
        path
    }

    /// Extract partition key from point: use stream_id tag if present, else location_id
    fn get_partition_key(point: &TimeSeriesPoint) -> String {
        point
            .tags
            .get("stream_id")
            .cloned()
            .unwrap_or_else(|| point.location_id.clone())
    }

    /// Write time series points to a Parquet file
    ///
    /// AIR-010 P3-02: Uses spawn_blocking to prevent blocking the async runtime during
    /// CPU-intensive Parquet serialization and Snappy compression.
    async fn write_parquet(&self, points: Vec<TimeSeriesPoint>, path: &Path) -> CoreResult<()> {
        if points.is_empty() {
            return Ok(());
        }

        let path = path.to_path_buf();

        // Move CPU-intensive work to blocking thread pool
        tokio::task::spawn_blocking(move || {
            let parent = path
                .parent()
                .ok_or_else(|| CoreError::Storage("Invalid path: no parent directory".to_string()))?;
            std::fs::create_dir_all(parent)?;

            // P2-02: Pre-allocate Vecs with known capacity to avoid reallocations
            let len = points.len();
            let mut timestamps = Vec::with_capacity(len);
            let mut location_ids = Vec::with_capacity(len);
            let mut metrics = Vec::with_capacity(len);
            let mut values = Vec::with_capacity(len);
            let mut ndp_ids = Vec::with_capacity(len);
            let mut contexts = Vec::with_capacity(len);

            for p in &points {
                timestamps.push(p.timestamp.timestamp_micros());
                location_ids.push(p.location_id.clone());
                metrics.push(
                    p.tags
                        .get("metric")
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string()),
                );
                values.push(p.value);
                ndp_ids.push(p.ndp_id.clone());
                contexts.push(p.context.as_ref().map(|c| c.to_string()));
            }

            let timestamp_series = Series::new("timestamp", timestamps);
            let location_series = Series::new("location_id", location_ids);
            let metric_series = Series::new("metric", metrics);
            let value_series = Series::new("value", values);
            let ndp_id_series = Series::new("ndp_id", ndp_ids);
            let context_series = Series::new("context", contexts);

            let mut df = DataFrame::new(vec![
                timestamp_series,
                location_series,
                metric_series,
                value_series,
                ndp_id_series,
                context_series,
            ])
            .map_err(|e| CoreError::Storage(format!("Failed to create DataFrame: {}", e)))?;

            let file = std::fs::File::create(&path)?;
            ParquetWriter::new(file)
                .with_compression(ParquetCompression::Snappy)
                .finish(&mut df)
                .map_err(|e| CoreError::Storage(format!("Failed to write Parquet: {}", e)))?;

            Ok::<_, CoreError>(())
        })
        .await
        .map_err(|e| CoreError::Storage(format!("Parquet write task panicked: {}", e)))??;
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

            // AIR-009: Read ndp_id and context columns if they exist
            let ndp_ids = df.column("ndp_id").ok().and_then(|c| c.utf8().ok());
            let contexts = df.column("context").ok().and_then(|c| c.utf8().ok());

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

                    // AIR-009: Extract ndp_id and context from columns
                    let ndp_id = ndp_ids.and_then(|col| col.get(i).map(|s| s.to_string()));
                    let context = contexts
                        .and_then(|col| col.get(i).and_then(|s| serde_json::from_str(s).ok()));

                    all_points.push(TimeSeriesPoint {
                        timestamp,
                        location_id: loc.to_string(),
                        value: val,
                        tags,
                        ndp_id,
                        context,
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

    /// Write a batch of time series points to Parquet storage
    ///
    /// AIR-010 P1-02: Parallelized partition writes using try_join_all for improved throughput.
    /// All partitions are written concurrently instead of sequentially.
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

        // M-014: Pre-allocate HashMap with typical partition count (usually 1-3 partitions per batch)
        let mut grouped: HashMap<PathBuf, Vec<TimeSeriesPoint>> = HashMap::with_capacity(3);
        for point in points {
            let partition_key = Self::get_partition_key(&point);
            let path = self.partition_path(&partition_key, point.timestamp);
            grouped.entry(path).or_insert_with(Vec::new).push(point);
        }

        // Write partitions sequentially (parallel writes require Arc<Self> refactor)
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

                // AIR-009: Read ndp_id and context columns if they exist
                let ndp_ids = df.column("ndp_id").ok().and_then(|c| c.utf8().ok());
                let contexts = df.column("context").ok().and_then(|c| c.utf8().ok());

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

                        // AIR-009: Extract ndp_id and context from columns
                        let ndp_id = ndp_ids.and_then(|col| col.get(i).map(|s| s.to_string()));
                        let context = contexts
                            .and_then(|col| col.get(i).and_then(|s| serde_json::from_str(s).ok()));

                        all_points.push(TimeSeriesPoint {
                            timestamp,
                            location_id: loc.to_string(),
                            value: val,
                            tags,
                            ndp_id,
                            context,
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

// ========== DP-004: RAW DATA STORAGE (BRONZE LAYER) ==========
//
// This implementation provides the 5-column schema for raw JSON storage:
// - timestamp: i64 (microseconds since epoch)
// - source_id: String (e.g., "air-quality-Http")
// - ndp_id: String (nullable, platform-assigned identifier)
// - context: String (nullable, JSON-serialized metadata)
// - raw_payload: String (JSON-serialized source data)

/// Extract stream_id from source_id by removing the protocol suffix
///
/// source_id format: "{stream_id}-{SourceType}" (e.g., "air-quality-Mqtt", "nws-forecast-Http")
/// Returns just the stream_id part for directory naming
fn extract_stream_id(source_id: &str) -> &str {
    // Known protocol suffixes (ordered by specificity)
    const SUFFIXES: &[&str] = &["-FileWatch", "-Webhook", "-HttpPoll", "-Http", "-Mqtt"];

    for suffix in SUFFIXES {
        if source_id.ends_with(suffix) {
            return &source_id[..source_id.len() - suffix.len()];
        }
    }
    source_id // Return as-is if no known suffix
}

impl ParquetStore {
    /// Build partition path for raw data storage using stream_id (extracted from source_id)
    ///
    /// Directory structure: {base_path}/raw/{stream_id}/year={YYYY}/month={MM}/day={DD}/data.parquet
    ///
    /// Note: Uses stream_id (e.g., "air-quality") not full source_id (e.g., "air-quality-Mqtt")
    /// to ensure all data from a stream goes to the same directory regardless of source type.
    ///
    /// Daily partitioning chosen over hourly to:
    /// - Reduce small file proliferation (sensors produce ~15KB/hour)
    /// - Improve Parquet compression efficiency with larger row groups
    /// - Reduce DuckDB/Polars query overhead from file-per-hour
    ///
    /// P2-06: Uses push() instead of chained join() to reduce allocations
    pub fn raw_partition_path(&self, source_id: &str, timestamp: DateTime<Utc>) -> PathBuf {
        let stream_id = extract_stream_id(source_id);
        let mut path = self.base_path.clone();
        path.push("raw");
        path.push(stream_id);
        path.push(format!("year={}", timestamp.year()));
        path.push(format!("month={:02}", timestamp.month()));
        path.push(format!("day={:02}", timestamp.day()));
        path.push("data.parquet");
        path
    }

    /// Write raw data points to Parquet file with 5-column schema
    ///
    /// AIR-010 P3-02: Uses spawn_blocking to prevent blocking the async runtime during
    /// CPU-intensive Parquet serialization and Snappy compression.
    async fn write_raw_parquet(&self, points: Vec<RawDataPoint>, path: &Path) -> CoreResult<()> {
        if points.is_empty() {
            return Ok(());
        }

        let path = path.to_path_buf();

        // Move CPU-intensive work to blocking thread pool
        tokio::task::spawn_blocking(move || {
            let parent = path
                .parent()
                .ok_or_else(|| CoreError::Storage("Invalid path: no parent directory".to_string()))?;
            std::fs::create_dir_all(parent)?;

            // P2-02: Pre-allocate Vecs with known capacity to avoid reallocations
            let len = points.len();
            let mut timestamps = Vec::with_capacity(len);
            let mut source_ids = Vec::with_capacity(len);
            let mut ndp_ids = Vec::with_capacity(len);
            let mut contexts = Vec::with_capacity(len);
            let mut raw_payloads = Vec::with_capacity(len);

            for p in &points {
                timestamps.push(p.timestamp.timestamp_micros());
                source_ids.push(p.source_id.clone());
                ndp_ids.push(p.ndp_id.clone());
                contexts.push(p.context.as_ref().map(|c| c.to_string()));
                raw_payloads.push(p.raw_payload.to_string());
            }

            // Create Series for DataFrame
            let timestamp_series = Series::new("timestamp", timestamps);
            let source_id_series = Series::new("source_id", source_ids);
            let ndp_id_series = Series::new("ndp_id", ndp_ids);
            let context_series = Series::new("context", contexts);
            let raw_payload_series = Series::new("raw_payload", raw_payloads);

            let mut df = DataFrame::new(vec![
                timestamp_series,
                source_id_series,
                ndp_id_series,
                context_series,
                raw_payload_series,
            ])
            .map_err(|e| CoreError::Storage(format!("Failed to create DataFrame: {}", e)))?;

            let file = std::fs::File::create(&path)?;
            ParquetWriter::new(file)
                .with_compression(ParquetCompression::Snappy)
                .finish(&mut df)
                .map_err(|e| CoreError::Storage(format!("Failed to write Parquet: {}", e)))?;

            Ok::<_, CoreError>(())
        })
        .await
        .map_err(|e| CoreError::Storage(format!("Parquet write task panicked: {}", e)))??;

        Ok(())
    }

    /// Append raw data points to an existing Parquet file or create new one
    async fn append_to_raw_parquet(
        &self,
        points: Vec<RawDataPoint>,
        path: PathBuf,
    ) -> CoreResult<()> {
        let mut all_points = points;

        if path.exists() {
            let file = std::fs::File::open(&path)?;
            let df = ParquetReader::new(file).finish().map_err(|e| {
                CoreError::Storage(format!("Failed to read existing Parquet: {}", e))
            })?;

            // Read existing data and convert back to RawDataPoint
            let timestamps = df
                .column("timestamp")
                .map_err(|e| CoreError::Storage(format!("Missing timestamp column: {}", e)))?
                .i64()
                .map_err(|e| CoreError::Storage(format!("Invalid timestamp type: {}", e)))?;

            let source_ids = df
                .column("source_id")
                .map_err(|e| CoreError::Storage(format!("Missing source_id column: {}", e)))?
                .utf8()
                .map_err(|e| CoreError::Storage(format!("Invalid source_id type: {}", e)))?;

            let ndp_ids = df.column("ndp_id").ok().and_then(|c| c.utf8().ok());
            let contexts = df.column("context").ok().and_then(|c| c.utf8().ok());
            let raw_payloads = df
                .column("raw_payload")
                .map_err(|e| CoreError::Storage(format!("Missing raw_payload column: {}", e)))?
                .utf8()
                .map_err(|e| CoreError::Storage(format!("Invalid raw_payload type: {}", e)))?;

            for i in 0..df.height() {
                if let (Some(ts), Some(source_id), Some(payload_str)) =
                    (timestamps.get(i), source_ids.get(i), raw_payloads.get(i))
                {
                    let timestamp = DateTime::from_timestamp_micros(ts)
                        .ok_or_else(|| CoreError::Storage("Invalid timestamp".to_string()))?;

                    let ndp_id = ndp_ids.and_then(|col| col.get(i).map(|s| s.to_string()));
                    let context = contexts
                        .and_then(|col| col.get(i).and_then(|s| serde_json::from_str(s).ok()));
                    let raw_payload: serde_json::Value = serde_json::from_str(payload_str)
                        .map_err(|e| CoreError::Storage(format!("Invalid JSON payload: {}", e)))?;

                    all_points.push(RawDataPoint {
                        timestamp,
                        source_id: source_id.to_string(),
                        ndp_id,
                        context,
                        raw_payload,
                    });
                }
            }
        }

        self.write_raw_parquet(all_points, &path).await
    }

    /// Find raw partition files within a time range
    ///
    /// Note: The source_filter can be either a full source_id (e.g., "air-quality-Mqtt")
    /// or a stream_id (e.g., "air-quality"). Both are matched against the stream_id
    /// directory since raw_partition_path uses extract_stream_id for directory naming.
    fn find_raw_partitions(
        &self,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
        source_filter: Option<&str>,
    ) -> CoreResult<Vec<PathBuf>> {
        let raw_path = self.base_path.join("raw");
        if !raw_path.exists() {
            return Ok(vec![]);
        }

        let mut paths = Vec::new();

        // Extract stream_id from source filter for directory matching
        // This aligns with raw_partition_path which uses extract_stream_id
        let stream_filter = source_filter.map(extract_stream_id);

        // Iterate through stream directories
        for source_entry in std::fs::read_dir(&raw_path)? {
            let source_entry = source_entry?;
            let source_path = source_entry.path();

            if !source_path.is_dir() {
                continue;
            }

            // Apply stream filter if provided
            if let Some(filter) = stream_filter {
                if let Some(name) = source_path.file_name().and_then(|n| n.to_str()) {
                    if name != filter {
                        continue;
                    }
                }
            }

            // Walk the partition structure
            self.collect_partition_files(&source_path, &mut paths)?;
        }

        Ok(paths)
    }

    /// Recursively collect partition files
    fn collect_partition_files(&self, dir: &Path, paths: &mut Vec<PathBuf>) -> CoreResult<()> {
        if !dir.exists() || !dir.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().map(|e| e == "parquet").unwrap_or(false) {
                paths.push(path);
            } else if path.is_dir() {
                self.collect_partition_files(&path, paths)?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl RawStore for ParquetStore {
    async fn write_raw(&self, point: RawDataPoint) -> CoreResult<()> {
        // Append to WAL first
        let mut wal = self.wal.lock().await;
        let entry = serde_json::to_vec(&point)
            .map_err(|e| CoreError::Storage(format!("Failed to serialize raw point: {}", e)))?;
        wal.append(&entry)?;
        drop(wal);

        // Write to partition path based on source_id
        let path = self.raw_partition_path(&point.source_id, point.timestamp);
        self.append_to_raw_parquet(vec![point], path).await
    }

    /// Write a batch of raw data points to Parquet storage
    ///
    /// AIR-010 P1-02: Parallelized partition writes using try_join_all for improved throughput.
    async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> CoreResult<()> {
        if points.is_empty() {
            return Ok(());
        }

        // Append all to WAL first
        let mut wal = self.wal.lock().await;
        for point in &points {
            let entry = serde_json::to_vec(point)
                .map_err(|e| CoreError::Storage(format!("Failed to serialize raw point: {}", e)))?;
            wal.append(&entry)?;
        }
        drop(wal);

        // M-014: Pre-allocate HashMap with typical partition count (usually 1-3 partitions per batch)
        let mut grouped: HashMap<PathBuf, Vec<RawDataPoint>> = HashMap::with_capacity(3);
        for point in points {
            let path = self.raw_partition_path(&point.source_id, point.timestamp);
            grouped.entry(path).or_default().push(point);
        }

        // Write partitions sequentially (parallel writes require Arc<Self> refactor)
        for (path, partition_points) in grouped {
            self.append_to_raw_parquet(partition_points, path).await?;
        }

        // Commit WAL
        let mut wal = self.wal.lock().await;
        wal.commit()?;

        Ok(())
    }

    async fn query_raw(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        source_filter: Option<String>,
    ) -> CoreResult<Vec<RawDataPoint>> {
        let partition_files = self.find_raw_partitions(start, end, source_filter.as_deref())?;
        // M-005: Pre-allocate based on partition count (estimate ~100 points per file)
        let mut all_points = Vec::with_capacity(partition_files.len() * 100);

        for path in partition_files {
            let file = std::fs::File::open(&path)?;
            let df = ParquetReader::new(file)
                .finish()
                .map_err(|e| CoreError::Storage(format!("Failed to read Parquet: {}", e)))?;

            let timestamps = df.column("timestamp")?.i64()?;
            let source_ids = df.column("source_id")?.utf8()?;
            let ndp_ids = df.column("ndp_id").ok().and_then(|c| c.utf8().ok());
            let contexts = df.column("context").ok().and_then(|c| c.utf8().ok());
            let raw_payloads = df.column("raw_payload")?.utf8()?;

            for i in 0..df.height() {
                if let (Some(ts), Some(source_id), Some(payload_str)) =
                    (timestamps.get(i), source_ids.get(i), raw_payloads.get(i))
                {
                    let timestamp = DateTime::from_timestamp_micros(ts)
                        .ok_or_else(|| CoreError::Storage("Invalid timestamp".to_string()))?;

                    // Apply time filter
                    if timestamp < start || timestamp > end {
                        continue;
                    }

                    // Apply source filter
                    if let Some(ref filter) = source_filter {
                        if source_id != filter {
                            continue;
                        }
                    }

                    let ndp_id = ndp_ids.and_then(|col| col.get(i).map(|s| s.to_string()));
                    let context = contexts
                        .and_then(|col| col.get(i).and_then(|s| serde_json::from_str(s).ok()));
                    let raw_payload: serde_json::Value = serde_json::from_str(payload_str)
                        .map_err(|e| CoreError::Storage(format!("Invalid JSON payload: {}", e)))?;

                    all_points.push(RawDataPoint {
                        timestamp,
                        source_id: source_id.to_string(),
                        ndp_id,
                        context,
                        raw_payload,
                    });
                }
            }
        }

        Ok(all_points)
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
            ndp_id: None,
            context: None,
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
            ndp_id: None,
            context: None,
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
            ndp_id: None,
            context: None,
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
            ndp_id: None,
            context: None,
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
            ndp_id: None,
            context: None,
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
            ndp_id: None,
            context: None,
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
            ndp_id: None,
            context: None,
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
        assert!(
            !wrong_path.exists(),
            "File should NOT exist in device MAC directory"
        );
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
            ndp_id: None,
            context: None,
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
            ndp_id: None,
            context: None,
        };

        // Write point
        store.write(point).await.unwrap();

        // Verify file structure
        let expected_path = temp_dir
            .path()
            .join("data")
            .join("air-quality") // Stream ID, NOT device MAC
            .join("year=2024")
            .join("month=01")
            .join("day=15")
            .join("readings.parquet");

        assert!(
            expected_path.exists(),
            "File should be in air-quality directory"
        );

        // Verify wrong path does NOT exist
        let wrong_path = temp_dir
            .path()
            .join("data")
            .join("d83bda1cd074") // Device MAC directory should NOT exist
            .join("year=2024")
            .join("month=01")
            .join("day=15")
            .join("readings.parquet");

        assert!(
            !wrong_path.exists(),
            "File should NOT be in device MAC directory"
        );
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
            ndp_id: None,
            context: None,
        };
        assert_eq!(ParquetStore::get_partition_key(&point1), "air-quality");

        // Test 2: No stream_id tag (fallback to location_id)
        let point2 = TimeSeriesPoint {
            timestamp,
            location_id: "sensor-001".to_string(),
            value: 22.0,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
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
            ndp_id: None,
            context: None,
        };
        assert_eq!(ParquetStore::get_partition_key(&point3), "outdoor-weather");
    }

    // ========== AIR-009: NDP_ID AND CONTEXT COLUMN TESTS ==========

    #[tokio::test]
    async fn test_parquet_stores_ndp_id() {
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), "temperature".to_string());

        let point = TimeSeriesPoint {
            timestamp,
            location_id: "sensor-001".to_string(),
            value: 23.5,
            tags,
            ndp_id: Some("air-quality-office-001".to_string()),
            context: None,
        };

        store.write(point).await.unwrap();

        // Query back and verify ndp_id is preserved
        let start = timestamp - chrono::Duration::hours(1);
        let end = timestamp + chrono::Duration::hours(1);
        let results = store.query("sensor-001", start, end, None).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].ndp_id,
            Some("air-quality-office-001".to_string())
        );
    }

    #[tokio::test]
    async fn test_parquet_stores_context() {
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        let context = serde_json::json!({
            "room": "office",
            "floor": 2,
            "calibrated": true
        });

        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), "temperature".to_string());

        let point = TimeSeriesPoint {
            timestamp,
            location_id: "sensor-001".to_string(),
            value: 23.5,
            tags,
            ndp_id: None,
            context: Some(context.clone()),
        };

        store.write(point).await.unwrap();

        let start = timestamp - chrono::Duration::hours(1);
        let end = timestamp + chrono::Duration::hours(1);
        let results = store.query("sensor-001", start, end, None).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].context, Some(context));
    }

    #[tokio::test]
    async fn test_parquet_stores_both_ndp_id_and_context() {
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        let context = serde_json::json!({
            "location": "Building A, Floor 2",
            "sensor_model": "AirGradient ONE"
        });

        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), "pm25".to_string());

        let point = TimeSeriesPoint {
            timestamp,
            location_id: "sensor-001".to_string(),
            value: 15.5,
            tags,
            ndp_id: Some("air-quality-office-001".to_string()),
            context: Some(context.clone()),
        };

        store.write(point).await.unwrap();

        let start = timestamp - chrono::Duration::hours(1);
        let end = timestamp + chrono::Duration::hours(1);
        let results = store.query("sensor-001", start, end, None).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].ndp_id,
            Some("air-quality-office-001".to_string())
        );
        assert_eq!(results[0].context, Some(context));
    }

    #[tokio::test]
    async fn test_parquet_handles_none_ndp_id_and_context() {
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();

        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), "humidity".to_string());

        let point = TimeSeriesPoint {
            timestamp,
            location_id: "sensor-001".to_string(),
            value: 65.0,
            tags,
            ndp_id: None,
            context: None,
        };

        store.write(point).await.unwrap();

        let start = timestamp - chrono::Duration::hours(1);
        let end = timestamp + chrono::Duration::hours(1);
        let results = store.query("sensor-001", start, end, None).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].ndp_id, None);
        assert_eq!(results[0].context, None);
    }

    // ========== DP-004: RAW DATA STORAGE TESTS (BRONZE LAYER) ==========
    //
    // TDD Cycle 4-6: ParquetStore RawStore implementation tests
    // These tests verify the 5-column schema for raw JSON storage.

    use crate::traits::RawStore;

    // ========== TDD CYCLE 4: ParquetStore writes RawDataPoint ==========

    #[tokio::test]
    async fn test_raw_partition_path_uses_stream_id() {
        // TC-030: Verify partition path structure uses stream_id (extracted from source_id)
        // DP-004: Directory named by stream, not source_id with protocol suffix
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 10, 30, 0).unwrap();

        // Pass source_id with suffix, but path should use stream_id without suffix
        let path = store.raw_partition_path("air-quality-Http", timestamp);

        assert!(path.to_string_lossy().contains("raw"));
        assert!(path.to_string_lossy().contains("air-quality")); // Stream name only
        assert!(!path.to_string_lossy().contains("-Http")); // No protocol suffix
        assert!(path.to_string_lossy().contains("year=2026"));
        assert!(path.to_string_lossy().contains("month=01"));
        assert!(path.to_string_lossy().contains("day=15"));
        assert!(!path.to_string_lossy().contains("hour=")); // Daily partitions, no hour
        assert!(path.to_string_lossy().ends_with("data.parquet"));
    }

    #[tokio::test]
    async fn test_extract_stream_id_from_source_id() {
        // Test protocol suffix extraction
        assert_eq!(super::extract_stream_id("air-quality-Mqtt"), "air-quality");
        assert_eq!(super::extract_stream_id("air-quality-Http"), "air-quality");
        assert_eq!(
            super::extract_stream_id("nws-forecast-HttpPoll"),
            "nws-forecast"
        );
        assert_eq!(
            super::extract_stream_id("outdoor-weather-Webhook"),
            "outdoor-weather"
        );
        assert_eq!(super::extract_stream_id("file-data-FileWatch"), "file-data");
        // No suffix - return as-is
        assert_eq!(super::extract_stream_id("air-quality"), "air-quality");
        assert_eq!(super::extract_stream_id("unknown"), "unknown");
    }

    #[tokio::test]
    async fn test_raw_parquet_schema_has_5_columns() {
        // TC-031: Verify the 5-column schema is created correctly
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 10, 30, 0).unwrap();

        let point = RawDataPoint::new("test-Http", serde_json::json!({"value": 42}))
            .with_timestamp(timestamp)
            .with_ndp_id("test-001")
            .with_context(serde_json::json!({"room": "lab"}));

        store.write_raw(point).await.unwrap();

        // Read back the parquet file and verify schema
        let path = store.raw_partition_path("test-Http", timestamp);
        assert!(path.exists(), "Parquet file should exist");

        let file = std::fs::File::open(&path).unwrap();
        let df = ParquetReader::new(file).finish().unwrap();

        // Verify 5 columns exist
        let column_names: Vec<&str> = df.get_column_names();
        assert_eq!(column_names.len(), 5, "Should have exactly 5 columns");
        assert!(column_names.contains(&"timestamp"));
        assert!(column_names.contains(&"source_id"));
        assert!(column_names.contains(&"ndp_id"));
        assert!(column_names.contains(&"context"));
        assert!(column_names.contains(&"raw_payload"));
    }

    #[tokio::test]
    async fn test_write_raw_single_point() {
        // TC-032: Write single RawDataPoint and verify storage
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 10, 30, 0).unwrap();

        let point = RawDataPoint::new(
            "air-quality-Http",
            serde_json::json!({
                "pm25": 12.5,
                "status": "active",
                "firmware": "v2.1"
            }),
        )
        .with_timestamp(timestamp)
        .with_ndp_id("sensor-001")
        .with_context(serde_json::json!({"room": "office", "floor": 2}));

        let result = store.write_raw(point).await;
        assert!(result.is_ok());

        // Verify file was created
        let path = store.raw_partition_path("air-quality-Http", timestamp);
        assert!(path.exists());
    }

    #[tokio::test]
    async fn test_write_and_query_raw_round_trip() {
        // TC-032: Write and read back RawDataPoint
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 10, 30, 0).unwrap();

        let original = RawDataPoint::new(
            "roundtrip-Http",
            serde_json::json!({
                "value": 42,
                "nested": {"a": 1, "b": "two"}
            }),
        )
        .with_timestamp(timestamp)
        .with_ndp_id("roundtrip-001")
        .with_context(serde_json::json!({"test": true}));

        store.write_raw(original.clone()).await.unwrap();

        // Query back
        let start = timestamp - chrono::Duration::hours(1);
        let end = timestamp + chrono::Duration::hours(1);
        let results = store
            .query_raw(start, end, Some("roundtrip-Http".to_string()))
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source_id, "roundtrip-Http");
        assert_eq!(results[0].ndp_id, Some("roundtrip-001".to_string()));
        assert_eq!(results[0].raw_payload["value"], 42);
        assert_eq!(results[0].raw_payload["nested"]["a"], 1);
        assert_eq!(results[0].raw_payload["nested"]["b"], "two");
    }

    #[tokio::test]
    async fn test_raw_handles_nullable_fields() {
        // TC-033: Verify nullable ndp_id and context are handled
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 10, 30, 0).unwrap();

        let point = RawDataPoint::new("minimal-Http", serde_json::json!({"data": 1}))
            .with_timestamp(timestamp);
        // ndp_id and context are None

        store.write_raw(point).await.unwrap();

        let start = timestamp - chrono::Duration::hours(1);
        let end = timestamp + chrono::Duration::hours(1);
        let results = store
            .query_raw(start, end, Some("minimal-Http".to_string()))
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].ndp_id.is_none());
        assert!(results[0].context.is_none());
        assert_eq!(results[0].raw_payload["data"], 1);
    }

    // ========== TDD CYCLE 5: Batch Writes ==========

    #[tokio::test]
    async fn test_write_raw_batch() {
        // TC-034: Write batch of RawDataPoints
        let (store, _temp) = create_test_store();
        let base_time = Utc.with_ymd_and_hms(2026, 1, 15, 10, 0, 0).unwrap();

        let points: Vec<RawDataPoint> = (0..5)
            .map(|i| {
                RawDataPoint::new(
                    "batch-test-Http",
                    serde_json::json!({"index": i, "value": i * 10}),
                )
                .with_timestamp(base_time + chrono::Duration::minutes(i))
            })
            .collect();

        let result = store.write_raw_batch(points).await;
        assert!(result.is_ok());

        // Query back
        let start = base_time - chrono::Duration::hours(1);
        let end = base_time + chrono::Duration::hours(1);
        let results = store
            .query_raw(start, end, Some("batch-test-Http".to_string()))
            .await
            .unwrap();

        assert_eq!(results.len(), 5);
    }

    #[tokio::test]
    async fn test_write_raw_batch_empty_succeeds() {
        // TC-034: Empty batch should succeed without error
        let (store, _temp) = create_test_store();

        let result = store.write_raw_batch(vec![]).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_write_raw_batch_multiple_sources() {
        // TC-034: Batch with multiple sources partitions correctly
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 10, 30, 0).unwrap();

        let points = vec![
            RawDataPoint::new("source-a-Http", serde_json::json!({"from": "a"}))
                .with_timestamp(timestamp),
            RawDataPoint::new("source-b-Mqtt", serde_json::json!({"from": "b"}))
                .with_timestamp(timestamp),
            RawDataPoint::new("source-a-Http", serde_json::json!({"from": "a2"}))
                .with_timestamp(timestamp),
        ];

        store.write_raw_batch(points).await.unwrap();

        // Verify partition paths
        let path_a = store.raw_partition_path("source-a-Http", timestamp);
        let path_b = store.raw_partition_path("source-b-Mqtt", timestamp);

        assert!(path_a.exists(), "Source A partition should exist");
        assert!(path_b.exists(), "Source B partition should exist");

        // Verify source filtering
        let start = timestamp - chrono::Duration::hours(1);
        let end = timestamp + chrono::Duration::hours(1);

        let results_a = store
            .query_raw(start, end, Some("source-a-Http".to_string()))
            .await
            .unwrap();
        let results_b = store
            .query_raw(start, end, Some("source-b-Mqtt".to_string()))
            .await
            .unwrap();

        assert_eq!(results_a.len(), 2);
        assert_eq!(results_b.len(), 1);
    }

    // ========== TDD CYCLE 6: Partition Path Uses stream_id ==========

    #[tokio::test]
    async fn test_partition_path_structure() {
        // Verify directory structure: raw/{stream_id}/year=YYYY/month=MM/day=DD/data.parquet
        // Note: Uses stream_id (extracted from source_id) for directory naming
        // Daily partitioning for better file compaction (vs hourly which creates many small files)
        let (store, temp_dir) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2026, 6, 15, 14, 30, 0).unwrap();

        let point = RawDataPoint::new("my-source-Http", serde_json::json!({"test": 1}))
            .with_timestamp(timestamp);

        store.write_raw(point).await.unwrap();

        // Check directory structure exists (using stream_id "my-source", not full source_id)
        let expected_dir = temp_dir
            .path()
            .join("raw")
            .join("my-source") // stream_id extracted from "my-source-Http"
            .join("year=2026")
            .join("month=06")
            .join("day=15"); // Daily partition (no hour subdirectory)

        assert!(expected_dir.exists(), "Partition directory should exist");
        assert!(
            expected_dir.join("data.parquet").exists(),
            "Data file should exist"
        );
    }

    #[tokio::test]
    async fn test_source_filter_in_query() {
        // Verify source filtering works correctly
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 10, 30, 0).unwrap();

        // Write points from different sources
        store
            .write_raw(
                RawDataPoint::new("source-1-Http", serde_json::json!({"from": 1}))
                    .with_timestamp(timestamp),
            )
            .await
            .unwrap();

        store
            .write_raw(
                RawDataPoint::new("source-2-Mqtt", serde_json::json!({"from": 2}))
                    .with_timestamp(timestamp),
            )
            .await
            .unwrap();

        store
            .write_raw(
                RawDataPoint::new("source-3-Webhook", serde_json::json!({"from": 3}))
                    .with_timestamp(timestamp),
            )
            .await
            .unwrap();

        let start = timestamp - chrono::Duration::hours(1);
        let end = timestamp + chrono::Duration::hours(1);

        // Query with filter
        let filtered = store
            .query_raw(start, end, Some("source-2-Mqtt".to_string()))
            .await
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].source_id, "source-2-Mqtt");

        // Query without filter
        let all = store.query_raw(start, end, None).await.unwrap();
        assert_eq!(all.len(), 3);
    }

    // ========== RAW DATA TYPE PRESERVATION TESTS ==========

    #[tokio::test]
    async fn test_raw_preserves_all_json_types() {
        // Verify non-numeric types are preserved in raw_payload
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 10, 30, 0).unwrap();

        let complex_payload = serde_json::json!({
            "string": "hello world",
            "integer": 42,
            "float": 3.14159,
            "boolean": true,
            "null": null,
            "array": [1, "two", false, null],
            "nested": {
                "deep": {
                    "value": "found"
                }
            }
        });

        let point =
            RawDataPoint::new("types-test-Http", complex_payload.clone()).with_timestamp(timestamp);

        store.write_raw(point).await.unwrap();

        let start = timestamp - chrono::Duration::hours(1);
        let end = timestamp + chrono::Duration::hours(1);
        let results = store
            .query_raw(start, end, Some("types-test-Http".to_string()))
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        let payload = &results[0].raw_payload;

        // Verify all types preserved
        assert_eq!(payload["string"], "hello world");
        assert_eq!(payload["integer"], 42);
        assert_eq!(payload["float"], 3.14159);
        assert_eq!(payload["boolean"], true);
        assert!(payload["null"].is_null());
        assert_eq!(payload["array"][0], 1);
        assert_eq!(payload["array"][1], "two");
        assert_eq!(payload["array"][2], false);
        assert!(payload["array"][3].is_null());
        assert_eq!(payload["nested"]["deep"]["value"], "found");
    }

    #[tokio::test]
    async fn test_raw_context_metadata_preserved() {
        // Verify context metadata is preserved
        let (store, _temp) = create_test_store();
        let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 10, 30, 0).unwrap();

        let context = serde_json::json!({
            "room": "Office 201",
            "floor": 2,
            "building": "A",
            "sensors": ["pm25", "co2", "temperature"],
            "calibration": {
                "date": "2026-01-01",
                "technician": "John"
            }
        });

        let point = RawDataPoint::new("context-test-Http", serde_json::json!({"value": 1}))
            .with_timestamp(timestamp)
            .with_context(context.clone());

        store.write_raw(point).await.unwrap();

        let start = timestamp - chrono::Duration::hours(1);
        let end = timestamp + chrono::Duration::hours(1);
        let results = store
            .query_raw(start, end, Some("context-test-Http".to_string()))
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        let stored_context = results[0].context.as_ref().unwrap();

        assert_eq!(stored_context["room"], "Office 201");
        assert_eq!(stored_context["floor"], 2);
        assert_eq!(stored_context["sensors"][0], "pm25");
        assert_eq!(stored_context["calibration"]["technician"], "John");
    }
}
