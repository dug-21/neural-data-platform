//! CSV data source implementation (dp-013)
//!
//! Provides file-based CSV data ingestion with:
//! - Configurable timestamp parsing (ISO8601, epoch, custom)
//! - Configurable delimiter and encoding
//! - Error handling strategies (skip, fail, log)
//! - Async file reading via tokio

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use csv_async::AsyncReaderBuilder;
use serde_json::{Map, Value};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::BufReader;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::error::{CoreError, CoreResult};
use crate::sources::generate_source_id;
use crate::traits::RawSource;
use crate::types::raw_data_point::RawDataPoint;
use crate::types::stream_config::{CsvSourceConfig, OnError, SourceType, TimestampFormat};

/// CSV data source for reading CSV files as RawDataPoints
///
/// This source reads CSV files asynchronously and produces RawDataPoints
/// for each row, with the entire row stored as a JSON object in raw_payload.
///
/// # Configuration-Driven Behavior
///
/// All behavior is driven by CsvSourceConfig:
/// - `path`: Path to the CSV file
/// - `timestamp_field`: Column name containing timestamps
/// - `timestamp_format`: How to parse timestamp values
/// - `delimiter`: Field separator character
/// - `on_error`: How to handle parsing errors
///
/// # Example
///
/// ```ignore
/// use neural_core::sources::CsvSource;
/// use neural_core::types::stream_config::CsvSourceConfig;
///
/// let config = CsvSourceConfig::new("/data/readings.csv", "timestamp");
/// let source = CsvSource::new("sensor-data", config);
/// let points = source.fetch_raw_batch().await?;
/// ```
pub struct CsvSource {
    /// Stream identifier
    stream_id: String,
    /// CSV configuration
    config: CsvSourceConfig,
    /// Platform-assigned stable identifier
    ndp_id: Option<String>,
    /// Config-derived metadata
    context: Option<Value>,
    /// Current row index (for fetch_raw state)
    current_row: Arc<Mutex<usize>>,
    /// Cached rows after first read
    cached_rows: Arc<Mutex<Option<Vec<RawDataPoint>>>>,
}

impl CsvSource {
    /// Create a new CsvSource with required configuration
    pub fn new(stream_id: impl Into<String>, config: CsvSourceConfig) -> Self {
        Self {
            stream_id: stream_id.into(),
            config,
            ndp_id: None,
            context: None,
            current_row: Arc::new(Mutex::new(0)),
            cached_rows: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a CsvSource using the builder pattern
    pub fn builder(stream_id: impl Into<String>, config: CsvSourceConfig) -> CsvSourceBuilder {
        CsvSourceBuilder::new(stream_id, config)
    }

    /// Set the platform-assigned stable identifier
    pub fn with_ndp_id(mut self, ndp_id: impl Into<String>) -> Self {
        self.ndp_id = Some(ndp_id.into());
        self
    }

    /// Set context metadata
    pub fn with_context(mut self, context: Value) -> Self {
        self.context = Some(context);
        self
    }

    /// Get the generated source_id
    pub fn source_id(&self) -> String {
        generate_source_id(&self.stream_id, &SourceType::Csv)
    }

    /// Get the file path
    pub fn path(&self) -> &PathBuf {
        &self.config.path
    }

    /// Parse a timestamp string according to the configured format
    fn parse_timestamp(&self, value: &str) -> CoreResult<DateTime<Utc>> {
        match &self.config.timestamp_format {
            TimestampFormat::Iso8601 => {
                // Try RFC3339 first (includes timezone)
                if let Ok(dt) = DateTime::parse_from_rfc3339(value) {
                    return Ok(dt.with_timezone(&Utc));
                }
                // Try ISO8601 without timezone (assume UTC)
                if let Ok(ndt) = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S") {
                    return Ok(ndt.and_utc());
                }
                // Try with fractional seconds
                if let Ok(ndt) = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f") {
                    return Ok(ndt.and_utc());
                }
                Err(CoreError::Parser(format!(
                    "Failed to parse ISO8601 timestamp: {}",
                    value
                )))
            }
            TimestampFormat::EpochSeconds => {
                let epoch: i64 = value.parse().map_err(|e| {
                    CoreError::Parser(format!("Failed to parse epoch seconds '{}': {}", value, e))
                })?;
                DateTime::from_timestamp(epoch, 0)
                    .ok_or_else(|| CoreError::Parser(format!("Invalid epoch seconds: {}", value)))
            }
            TimestampFormat::EpochMillis => {
                let epoch_ms: i64 = value.parse().map_err(|e| {
                    CoreError::Parser(format!("Failed to parse epoch millis '{}': {}", value, e))
                })?;
                let secs = epoch_ms / 1000;
                let nanos = ((epoch_ms % 1000) * 1_000_000) as u32;
                DateTime::from_timestamp(secs, nanos).ok_or_else(|| {
                    CoreError::Parser(format!("Invalid epoch milliseconds: {}", value))
                })
            }
            TimestampFormat::Custom(format_str) => {
                let ndt = NaiveDateTime::parse_from_str(value, format_str).map_err(|e| {
                    CoreError::Parser(format!(
                        "Failed to parse timestamp '{}' with format '{}': {}",
                        value, format_str, e
                    ))
                })?;
                Ok(ndt.and_utc())
            }
        }
    }

    /// Convert a CSV record to a RawDataPoint
    fn record_to_raw_point(
        &self,
        headers: &[String],
        record: &csv_async::StringRecord,
        row_index: usize,
    ) -> CoreResult<RawDataPoint> {
        // Build JSON object from record
        let mut map = Map::new();
        for (i, header) in headers.iter().enumerate() {
            if let Some(value) = record.get(i) {
                // Try to parse as number, otherwise store as string
                let json_value = if let Ok(num) = value.parse::<i64>() {
                    Value::Number(num.into())
                } else if let Ok(num) = value.parse::<f64>() {
                    Value::Number(
                        serde_json::Number::from_f64(num)
                            .unwrap_or_else(|| serde_json::Number::from(0)),
                    )
                } else if value.eq_ignore_ascii_case("true") {
                    Value::Bool(true)
                } else if value.eq_ignore_ascii_case("false") {
                    Value::Bool(false)
                } else if value.is_empty() {
                    Value::Null
                } else {
                    Value::String(value.to_string())
                };
                map.insert(header.clone(), json_value);
            }
        }

        // Extract timestamp from the configured field
        let timestamp_str = record
            .get(
                headers
                    .iter()
                    .position(|h| h == &self.config.timestamp_field)
                    .ok_or_else(|| {
                        CoreError::Parser(format!(
                            "Timestamp field '{}' not found in CSV headers",
                            self.config.timestamp_field
                        ))
                    })?,
            )
            .ok_or_else(|| {
                CoreError::Parser(format!("Missing timestamp value at row {}", row_index))
            })?;

        let timestamp = self.parse_timestamp(timestamp_str)?;

        // Build RawDataPoint
        let mut point =
            RawDataPoint::new(self.source_id(), Value::Object(map)).with_timestamp(timestamp);

        if let Some(ref ndp_id) = self.ndp_id {
            point = point.with_ndp_id(ndp_id.clone());
        }

        if let Some(ref context) = self.context {
            point = point.with_context(context.clone());
        }

        Ok(point)
    }

    /// Read all rows from the CSV file
    async fn read_all_rows(&self) -> CoreResult<Vec<RawDataPoint>> {
        debug!(
            path = %self.config.path.display(),
            stream_id = %self.stream_id,
            "Reading CSV file"
        );

        let file = File::open(&self.config.path).await.map_err(|e| {
            CoreError::Source(format!(
                "Failed to open CSV file '{}': {}",
                self.config.path.display(),
                e
            ))
        })?;

        let reader = BufReader::new(file);

        let mut csv_reader = AsyncReaderBuilder::new()
            .delimiter(self.config.delimiter as u8)
            .has_headers(self.config.has_header)
            .create_reader(reader);

        // Get headers
        let headers: Vec<String> = if self.config.has_header {
            let header_record = csv_reader
                .headers()
                .await
                .map_err(|e| CoreError::Source(format!("Failed to read CSV headers: {}", e)))?;
            header_record.iter().map(|s| s.to_string()).collect()
        } else {
            // Generate numeric headers if no header row
            (0..100).map(|i| format!("col_{}", i)).collect()
        };

        // Verify timestamp field exists in headers
        if !headers.contains(&self.config.timestamp_field) {
            return Err(CoreError::Config(format!(
                "Timestamp field '{}' not found in CSV headers: {:?}",
                self.config.timestamp_field, headers
            )));
        }

        let mut points = Vec::new();
        let mut row_index = 0;
        let mut records = csv_reader.records();

        use futures::stream::StreamExt;
        while let Some(result) = records.next().await {
            // Check row limit
            if self.config.row_limit > 0 && row_index >= self.config.row_limit {
                debug!(
                    row_limit = self.config.row_limit,
                    "Row limit reached, stopping"
                );
                break;
            }

            match result {
                Ok(record) => match self.record_to_raw_point(&headers, &record, row_index) {
                    Ok(point) => {
                        points.push(point);
                    }
                    Err(e) => match self.config.on_error {
                        OnError::Fail => {
                            return Err(e);
                        }
                        OnError::Skip => {
                            debug!(row = row_index, error = %e, "Skipping invalid row");
                        }
                        OnError::Log => {
                            warn!(row = row_index, error = %e, "Invalid row in CSV");
                        }
                    },
                },
                Err(e) => match self.config.on_error {
                    OnError::Fail => {
                        return Err(CoreError::Source(format!(
                            "CSV parse error at row {}: {}",
                            row_index, e
                        )));
                    }
                    OnError::Skip => {
                        debug!(row = row_index, error = %e, "Skipping malformed row");
                    }
                    OnError::Log => {
                        warn!(row = row_index, error = %e, "Malformed row in CSV");
                    }
                },
            }

            row_index += 1;
        }

        info!(
            path = %self.config.path.display(),
            rows_read = points.len(),
            "CSV file read complete"
        );

        Ok(points)
    }
}

/// Builder for CsvSource with fluent API
pub struct CsvSourceBuilder {
    stream_id: String,
    config: CsvSourceConfig,
    ndp_id: Option<String>,
    context: Option<Value>,
}

impl CsvSourceBuilder {
    /// Create a new builder
    pub fn new(stream_id: impl Into<String>, config: CsvSourceConfig) -> Self {
        Self {
            stream_id: stream_id.into(),
            config,
            ndp_id: None,
            context: None,
        }
    }

    /// Set the platform-assigned stable identifier
    pub fn ndp_id(mut self, ndp_id: impl Into<String>) -> Self {
        self.ndp_id = Some(ndp_id.into());
        self
    }

    /// Set context metadata
    pub fn context(mut self, context: Value) -> Self {
        self.context = Some(context);
        self
    }

    /// Build the CsvSource
    pub fn build(self) -> CsvSource {
        let mut source = CsvSource::new(self.stream_id, self.config);
        source.ndp_id = self.ndp_id;
        source.context = self.context;
        source
    }
}

/// RawSource implementation for CsvSource (dp-013)
///
/// Reads CSV files and produces RawDataPoints for each row.
/// The entire row is stored as a JSON object in raw_payload.
#[async_trait]
impl RawSource for CsvSource {
    /// Fetch the next raw data point from the CSV file.
    ///
    /// This method maintains state and returns one row at a time.
    /// On first call, reads the entire file and caches rows.
    /// Subsequent calls return cached rows sequentially.
    async fn fetch_raw(&self) -> CoreResult<RawDataPoint> {
        // Ensure rows are cached
        let mut cached = self.cached_rows.lock().await;
        if cached.is_none() {
            let rows = self.read_all_rows().await?;
            *cached = Some(rows);
        }

        let rows = cached.as_ref().unwrap();
        let mut current = self.current_row.lock().await;

        if *current >= rows.len() {
            return Err(CoreError::Source(format!(
                "No more rows in CSV file (read {} rows)",
                rows.len()
            )));
        }

        let point = rows[*current].clone();
        *current += 1;
        Ok(point)
    }

    /// Fetch all rows from the CSV file as a batch.
    ///
    /// Returns a Vec<RawDataPoint> with one entry per valid CSV row.
    /// This is the preferred method for CSV sources as it reads the
    /// entire file in one operation.
    async fn fetch_raw_batch(&self) -> CoreResult<Vec<RawDataPoint>> {
        // Check if already cached
        let cached = self.cached_rows.lock().await;
        if let Some(ref rows) = *cached {
            return Ok(rows.clone());
        }
        drop(cached);

        // Read all rows
        let rows = self.read_all_rows().await?;

        // Cache for future calls
        let mut cached = self.cached_rows.lock().await;
        *cached = Some(rows.clone());

        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ========== dp-013: CSV Source Configuration Tests ==========

    #[test]
    fn test_csv_source_config_new() {
        let config = CsvSourceConfig::new("/path/to/file.csv", "timestamp");

        assert_eq!(config.path, PathBuf::from("/path/to/file.csv"));
        assert_eq!(config.timestamp_field, "timestamp");
        assert_eq!(config.delimiter, ',');
        assert_eq!(config.encoding, "utf-8");
        assert!(config.has_header);
        assert_eq!(config.row_limit, 0);
    }

    #[test]
    fn test_csv_source_config_builder_pattern() {
        let config = CsvSourceConfig::new("/data/sensors.csv", "time")
            .with_delimiter(';')
            .with_timestamp_format(TimestampFormat::EpochSeconds)
            .with_on_error(OnError::Fail)
            .with_has_header(false)
            .with_row_limit(1000);

        assert_eq!(config.delimiter, ';');
        assert_eq!(config.timestamp_format, TimestampFormat::EpochSeconds);
        assert_eq!(config.on_error, OnError::Fail);
        assert!(!config.has_header);
        assert_eq!(config.row_limit, 1000);
    }

    #[test]
    fn test_csv_source_config_default() {
        let config = CsvSourceConfig::default();

        assert_eq!(config.timestamp_field, "timestamp");
        assert_eq!(config.delimiter, ',');
        assert_eq!(config.encoding, "utf-8");
        assert_eq!(config.timestamp_format, TimestampFormat::Iso8601);
        assert_eq!(config.on_error, OnError::Skip);
        assert!(config.has_header);
        assert_eq!(config.row_limit, 0);
    }

    // ========== dp-013: CSV Source Construction Tests ==========

    #[test]
    fn test_csv_source_new() {
        let config = CsvSourceConfig::new("/data/test.csv", "timestamp");
        let source = CsvSource::new("test-stream", config);

        assert_eq!(source.stream_id, "test-stream");
        assert_eq!(source.source_id(), "test-stream-Csv");
        assert!(source.ndp_id.is_none());
        assert!(source.context.is_none());
    }

    #[test]
    fn test_csv_source_with_metadata() {
        let config = CsvSourceConfig::new("/data/test.csv", "timestamp");
        let context = serde_json::json!({"location": "office"});

        let source = CsvSource::new("test-stream", config)
            .with_ndp_id("sensor-001")
            .with_context(context.clone());

        assert_eq!(source.ndp_id, Some("sensor-001".to_string()));
        assert_eq!(source.context, Some(context));
    }

    #[test]
    fn test_csv_source_builder() {
        let config = CsvSourceConfig::new("/data/test.csv", "timestamp");
        let context = serde_json::json!({"room": "lab"});

        let source = CsvSource::builder("my-stream", config)
            .ndp_id("device-001")
            .context(context.clone())
            .build();

        assert_eq!(source.stream_id, "my-stream");
        assert_eq!(source.ndp_id, Some("device-001".to_string()));
        assert_eq!(source.context, Some(context));
    }

    // ========== dp-013: Timestamp Parsing Tests ==========

    #[test]
    fn test_parse_timestamp_iso8601() {
        let config =
            CsvSourceConfig::new("/test.csv", "ts").with_timestamp_format(TimestampFormat::Iso8601);
        let source = CsvSource::new("test", config);

        // RFC3339 with timezone
        let result = source.parse_timestamp("2024-01-15T10:30:00Z");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().timestamp(), 1705314600);

        // ISO8601 without timezone
        let result = source.parse_timestamp("2024-01-15T10:30:00");
        assert!(result.is_ok());

        // With milliseconds
        let result = source.parse_timestamp("2024-01-15T10:30:00.123");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_timestamp_epoch_seconds() {
        let config = CsvSourceConfig::new("/test.csv", "ts")
            .with_timestamp_format(TimestampFormat::EpochSeconds);
        let source = CsvSource::new("test", config);

        let result = source.parse_timestamp("1705315800");
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert_eq!(dt.timestamp(), 1705315800);
    }

    #[test]
    fn test_parse_timestamp_epoch_millis() {
        let config = CsvSourceConfig::new("/test.csv", "ts")
            .with_timestamp_format(TimestampFormat::EpochMillis);
        let source = CsvSource::new("test", config);

        let result = source.parse_timestamp("1705315800123");
        assert!(result.is_ok());
        let dt = result.unwrap();
        assert_eq!(dt.timestamp(), 1705315800);
        assert_eq!(dt.timestamp_subsec_millis(), 123);
    }

    #[test]
    fn test_parse_timestamp_custom_format() {
        let config = CsvSourceConfig::new("/test.csv", "ts")
            .with_timestamp_format(TimestampFormat::Custom("%Y/%m/%d %H:%M:%S".to_string()));
        let source = CsvSource::new("test", config);

        let result = source.parse_timestamp("2024/01/15 10:30:00");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        let config =
            CsvSourceConfig::new("/test.csv", "ts").with_timestamp_format(TimestampFormat::Iso8601);
        let source = CsvSource::new("test", config);

        let result = source.parse_timestamp("not-a-timestamp");
        assert!(result.is_err());
    }

    // ========== dp-013: CSV Reading Integration Tests ==========

    fn create_test_csv_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        file.write_all(content.as_bytes())
            .expect("Failed to write test data");
        file.flush().expect("Failed to flush");
        file
    }

    #[tokio::test]
    async fn test_fetch_raw_batch_basic() {
        let csv_content = "timestamp,temperature,humidity\n\
                          2024-01-15T10:00:00Z,22.5,45.0\n\
                          2024-01-15T11:00:00Z,23.1,44.0\n\
                          2024-01-15T12:00:00Z,24.0,43.5\n";

        let file = create_test_csv_file(csv_content);
        let config = CsvSourceConfig::new(file.path(), "timestamp");
        let source = CsvSource::new("sensor-data", config);

        let points = source.fetch_raw_batch().await.expect("Should read CSV");

        assert_eq!(points.len(), 3);
        assert_eq!(points[0].source_id, "sensor-data-Csv");
        assert_eq!(points[0].raw_payload["temperature"], 22.5);
        assert_eq!(points[0].raw_payload["humidity"], 45.0);
        assert_eq!(points[1].raw_payload["temperature"], 23.1);
        assert_eq!(points[2].raw_payload["temperature"], 24.0);
    }

    #[tokio::test]
    async fn test_fetch_raw_batch_with_metadata() {
        let csv_content = "timestamp,value\n\
                          2024-01-15T10:00:00Z,100\n";

        let file = create_test_csv_file(csv_content);
        let config = CsvSourceConfig::new(file.path(), "timestamp");
        let context = serde_json::json!({"sensor": "primary"});

        let source = CsvSource::new("test", config)
            .with_ndp_id("device-001")
            .with_context(context.clone());

        let points = source.fetch_raw_batch().await.expect("Should read CSV");

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].ndp_id, Some("device-001".to_string()));
        assert_eq!(points[0].context, Some(context));
    }

    #[tokio::test]
    async fn test_fetch_raw_batch_custom_delimiter() {
        let csv_content = "timestamp;value;label\n\
                          2024-01-15T10:00:00Z;42;test\n";

        let file = create_test_csv_file(csv_content);
        let config = CsvSourceConfig::new(file.path(), "timestamp").with_delimiter(';');
        let source = CsvSource::new("test", config);

        let points = source.fetch_raw_batch().await.expect("Should read CSV");

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].raw_payload["value"], 42);
        assert_eq!(points[0].raw_payload["label"], "test");
    }

    #[tokio::test]
    async fn test_fetch_raw_batch_epoch_timestamps() {
        let csv_content = "ts,reading\n\
                          1705315800,100\n\
                          1705319400,101\n";

        let file = create_test_csv_file(csv_content);
        let config = CsvSourceConfig::new(file.path(), "ts")
            .with_timestamp_format(TimestampFormat::EpochSeconds);
        let source = CsvSource::new("test", config);

        let points = source.fetch_raw_batch().await.expect("Should read CSV");

        assert_eq!(points.len(), 2);
        assert_eq!(points[0].timestamp.timestamp(), 1705315800);
        assert_eq!(points[1].timestamp.timestamp(), 1705319400);
    }

    #[tokio::test]
    async fn test_fetch_raw_batch_row_limit() {
        let csv_content = "timestamp,value\n\
                          2024-01-15T10:00:00Z,1\n\
                          2024-01-15T11:00:00Z,2\n\
                          2024-01-15T12:00:00Z,3\n\
                          2024-01-15T13:00:00Z,4\n\
                          2024-01-15T14:00:00Z,5\n";

        let file = create_test_csv_file(csv_content);
        let config = CsvSourceConfig::new(file.path(), "timestamp").with_row_limit(3);
        let source = CsvSource::new("test", config);

        let points = source.fetch_raw_batch().await.expect("Should read CSV");

        assert_eq!(points.len(), 3);
        assert_eq!(points[0].raw_payload["value"], 1);
        assert_eq!(points[2].raw_payload["value"], 3);
    }

    #[tokio::test]
    async fn test_fetch_raw_batch_skip_invalid_rows() {
        let csv_content = "timestamp,value\n\
                          2024-01-15T10:00:00Z,100\n\
                          invalid-timestamp,200\n\
                          2024-01-15T12:00:00Z,300\n";

        let file = create_test_csv_file(csv_content);
        let config = CsvSourceConfig::new(file.path(), "timestamp").with_on_error(OnError::Skip);
        let source = CsvSource::new("test", config);

        let points = source.fetch_raw_batch().await.expect("Should read CSV");

        // Should have 2 valid rows, skipping the invalid one
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].raw_payload["value"], 100);
        assert_eq!(points[1].raw_payload["value"], 300);
    }

    #[tokio::test]
    async fn test_fetch_raw_batch_fail_on_invalid_row() {
        let csv_content = "timestamp,value\n\
                          2024-01-15T10:00:00Z,100\n\
                          invalid-timestamp,200\n\
                          2024-01-15T12:00:00Z,300\n";

        let file = create_test_csv_file(csv_content);
        let config = CsvSourceConfig::new(file.path(), "timestamp").with_on_error(OnError::Fail);
        let source = CsvSource::new("test", config);

        let result = source.fetch_raw_batch().await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_raw_sequential() {
        let csv_content = "timestamp,value\n\
                          2024-01-15T10:00:00Z,1\n\
                          2024-01-15T11:00:00Z,2\n\
                          2024-01-15T12:00:00Z,3\n";

        let file = create_test_csv_file(csv_content);
        let config = CsvSourceConfig::new(file.path(), "timestamp");
        let source = CsvSource::new("test", config);

        // Fetch rows one at a time
        let p1 = source.fetch_raw().await.expect("First row");
        let p2 = source.fetch_raw().await.expect("Second row");
        let p3 = source.fetch_raw().await.expect("Third row");

        assert_eq!(p1.raw_payload["value"], 1);
        assert_eq!(p2.raw_payload["value"], 2);
        assert_eq!(p3.raw_payload["value"], 3);

        // Fourth fetch should error (no more rows)
        let result = source.fetch_raw().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_raw_batch_missing_timestamp_field() {
        let csv_content = "time,value\n\
                          2024-01-15T10:00:00Z,100\n";

        let file = create_test_csv_file(csv_content);
        // Config expects "timestamp" but CSV has "time"
        let config = CsvSourceConfig::new(file.path(), "timestamp");
        let source = CsvSource::new("test", config);

        let result = source.fetch_raw_batch().await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("timestamp"));
    }

    #[tokio::test]
    async fn test_fetch_raw_batch_file_not_found() {
        let config = CsvSourceConfig::new("/nonexistent/path/file.csv", "timestamp");
        let source = CsvSource::new("test", config);

        let result = source.fetch_raw_batch().await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to open"));
    }

    #[tokio::test]
    async fn test_fetch_raw_batch_type_inference() {
        let csv_content = "timestamp,int_val,float_val,bool_val,empty_val,string_val\n\
                          2024-01-15T10:00:00Z,42,3.14,true,,hello\n\
                          2024-01-15T11:00:00Z,-10,0.0,false,,world\n";

        let file = create_test_csv_file(csv_content);
        let config = CsvSourceConfig::new(file.path(), "timestamp");
        let source = CsvSource::new("test", config);

        let points = source.fetch_raw_batch().await.expect("Should read CSV");

        // Check first row
        assert_eq!(points[0].raw_payload["int_val"], 42);
        assert_eq!(points[0].raw_payload["float_val"], 3.14);
        assert_eq!(points[0].raw_payload["bool_val"], true);
        assert!(points[0].raw_payload["empty_val"].is_null());
        assert_eq!(points[0].raw_payload["string_val"], "hello");

        // Check second row
        assert_eq!(points[1].raw_payload["int_val"], -10);
        assert_eq!(points[1].raw_payload["bool_val"], false);
    }

    // ========== dp-013: Serialization Tests ==========

    #[test]
    fn test_csv_source_config_serialization() {
        let config = CsvSourceConfig::new("/data/test.csv", "timestamp")
            .with_delimiter(';')
            .with_timestamp_format(TimestampFormat::EpochSeconds)
            .with_on_error(OnError::Log);

        let json = serde_json::to_string(&config).expect("Should serialize");
        let restored: CsvSourceConfig = serde_json::from_str(&json).expect("Should deserialize");

        assert_eq!(config, restored);
    }

    #[test]
    fn test_timestamp_format_serialization() {
        // Test all variants
        let formats = vec![
            TimestampFormat::Iso8601,
            TimestampFormat::EpochSeconds,
            TimestampFormat::EpochMillis,
            TimestampFormat::Custom("%Y-%m-%d".to_string()),
        ];

        for format in formats {
            let json = serde_json::to_string(&format).expect("Should serialize");
            let restored: TimestampFormat =
                serde_json::from_str(&json).expect("Should deserialize");
            assert_eq!(format, restored);
        }
    }

    #[test]
    fn test_on_error_serialization() {
        let variants = vec![OnError::Skip, OnError::Fail, OnError::Log];

        for variant in variants {
            let json = serde_json::to_string(&variant).expect("Should serialize");
            let restored: OnError = serde_json::from_str(&json).expect("Should deserialize");
            assert_eq!(variant, restored);
        }
    }
}
