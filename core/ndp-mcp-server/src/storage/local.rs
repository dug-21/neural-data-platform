//! Local filesystem implementation of BronzeStorage.
//!
//! Reads Parquet files from local Hive-style partition directories.
//! This is the primary implementation for Raspberry Pi edge deployment.
//!
//! # Directory Structure
//!
//! ```text
//! {base_path}/
//!     {stream_id}/
//!         year=YYYY/
//!             month=MM/
//!                 day=DD/
//!                     data.parquet
//! ```
//!
//! # Example
//!
//! ```ignore
//! let storage = LocalParquetStorage::new("/data/raw");
//! let streams = storage.list_streams().await?;
//! let schema = storage.get_schema("air-quality").await?;
//! ```

use crate::error::{McpError, McpResult};
use crate::storage::traits::BronzeStorage;
use crate::storage::types::{
    FieldInfo, ParquetSchemaInfo, RawPayloadStructure, StreamStorageInfo,
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parquet::file::reader::{FileReader, SerializedFileReader};
use serde_json::{json, Value};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// Local filesystem implementation of BronzeStorage.
///
/// Reads Parquet files from a local directory with Hive-style partitioning.
/// Optimized for edge deployment on Raspberry Pi with minimal memory footprint.
///
/// # Partition Discovery
///
/// Uses reverse chronological traversal to find the latest partition:
/// 1. Sort year directories descending (year=2026 before year=2025)
/// 2. Within each year, sort months descending
/// 3. Within each month, sort days descending
/// 4. Return first directory containing data.parquet
///
/// # Thread Safety
///
/// This implementation is Send + Sync as it only holds a PathBuf and
/// performs file I/O through thread-safe std::fs operations.
pub struct LocalParquetStorage {
    /// Base path for raw data (e.g., /data/raw)
    base_path: PathBuf,
}

impl LocalParquetStorage {
    /// Create a new LocalParquetStorage instance.
    ///
    /// # Arguments
    ///
    /// * `base_path` - Path to the raw data directory containing stream subdirectories
    ///
    /// # Example
    ///
    /// ```ignore
    /// let storage = LocalParquetStorage::new("/data/raw");
    /// ```
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Get the base path.
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// Find the latest partition for a stream using reverse chronological traversal.
    ///
    /// Walks the Hive-style partition tree (year=YYYY/month=MM/day=DD) in
    /// descending order to find the most recent partition containing data.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier
    ///
    /// # Returns
    ///
    /// Path to data.parquet in the latest partition, or None if no partitions exist.
    ///
    /// # Algorithm
    ///
    /// 1. List year directories under stream, sort descending
    /// 2. For each year, list month directories, sort descending
    /// 3. For each month, list day directories, sort descending
    /// 4. Check for data.parquet in each day directory
    /// 5. Return first found (most recent)
    fn find_latest_partition(&self, stream_id: &str) -> McpResult<Option<PathBuf>> {
        let stream_path = self.base_path.join(stream_id);

        if !stream_path.exists() {
            return Ok(None);
        }

        // Get and sort year directories descending
        let mut year_dirs = self.list_partition_dirs(&stream_path, "year=")?;
        year_dirs.sort_by_key(|d| {
            Reverse(
                d.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
            )
        });

        for year_dir in year_dirs {
            // Get and sort month directories descending
            let mut month_dirs = self.list_partition_dirs(&year_dir, "month=")?;
            month_dirs.sort_by_key(|d| {
                Reverse(
                    d.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                )
            });

            for month_dir in month_dirs {
                // Get and sort day directories descending
                let mut day_dirs = self.list_partition_dirs(&month_dir, "day=")?;
                day_dirs.sort_by_key(|d| {
                    Reverse(
                        d.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                    )
                });

                for day_dir in day_dirs {
                    let parquet_file = day_dir.join("data.parquet");
                    if parquet_file.exists() {
                        return Ok(Some(parquet_file));
                    }
                }
            }
        }

        Ok(None)
    }

    /// List directories matching a Hive partition prefix.
    ///
    /// # Arguments
    ///
    /// * `parent` - Parent directory to search
    /// * `prefix` - Partition prefix (e.g., "year=", "month=", "day=")
    ///
    /// # Returns
    ///
    /// Vector of directory paths matching the prefix.
    fn list_partition_dirs(&self, parent: &Path, prefix: &str) -> McpResult<Vec<PathBuf>> {
        let entries = fs::read_dir(parent).map_err(|e| {
            McpError::StorageError(format!("Failed to read directory {}: {}", parent.display(), e))
        })?;

        let dirs: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                    && e.file_name().to_string_lossy().starts_with(prefix)
            })
            .map(|e| e.path())
            .collect();

        Ok(dirs)
    }

    /// Get the partition path string relative to stream directory.
    ///
    /// Extracts "year=YYYY/month=MM/day=DD" from a full path.
    fn extract_partition_path(&self, parquet_path: &Path, stream_id: &str) -> Option<String> {
        let stream_path = self.base_path.join(stream_id);
        let parent = parquet_path.parent()?;

        parent
            .strip_prefix(&stream_path)
            .ok()
            .map(|p| p.to_string_lossy().to_string())
    }

    /// Get file metadata for a Parquet file.
    ///
    /// # Returns
    ///
    /// Tuple of (file_size_bytes, modified_time)
    fn get_file_metadata(&self, path: &Path) -> McpResult<(u64, DateTime<Utc>)> {
        let metadata = fs::metadata(path).map_err(|e| {
            McpError::StorageError(format!("Failed to get metadata for {}: {}", path.display(), e))
        })?;

        let size = metadata.len();
        let modified = metadata.modified().map_err(|e| {
            McpError::StorageError(format!("Failed to get modified time: {}", e))
        })?;

        let modified_dt = DateTime::<Utc>::from(modified);

        Ok((size, modified_dt))
    }

    /// Scan stream storage and collect metadata.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier
    ///
    /// # Returns
    ///
    /// StreamStorageInfo with all available metadata, or None if no data exists.
    fn scan_stream_storage(&self, stream_id: &str) -> McpResult<Option<StreamStorageInfo>> {
        let parquet_path = match self.find_latest_partition(stream_id)? {
            Some(p) => p,
            None => return Ok(None),
        };

        let (file_size, modified) = self.get_file_metadata(&parquet_path)?;
        let partition_path = self.extract_partition_path(&parquet_path, stream_id);

        // Read row count from Parquet metadata
        let row_count = self.get_row_count(&parquet_path)?;

        Ok(Some(
            StreamStorageInfo::new(stream_id)
                .with_latest_partition(partition_path.unwrap_or_default())
                .with_file_size(file_size)
                .with_modified(modified)
                .with_row_count(row_count),
        ))
    }

    /// Get the row count from Parquet metadata.
    ///
    /// Reads only the file metadata (footer), not the actual row data.
    fn get_row_count(&self, path: &Path) -> McpResult<u64> {
        let file = File::open(path).map_err(|e| {
            McpError::StorageError(format!("Failed to open {}: {}", path.display(), e))
        })?;

        let reader = SerializedFileReader::new(file).map_err(|e| {
            McpError::StorageError(format!("Failed to read Parquet file: {}", e))
        })?;

        let metadata = reader.metadata();
        let row_count: u64 = metadata
            .row_groups()
            .iter()
            .map(|rg| rg.num_rows() as u64)
            .sum();

        Ok(row_count)
    }

    /// Read the Arrow schema from a Parquet file.
    ///
    /// Opens the file and reads the schema from Parquet metadata.
    fn read_parquet_schema(&self, path: &Path) -> McpResult<Vec<FieldInfo>> {
        let file = File::open(path).map_err(|e| {
            McpError::StorageError(format!("Failed to open {}: {}", path.display(), e))
        })?;

        let reader = SerializedFileReader::new(file).map_err(|e| {
            McpError::StorageError(format!("Failed to read Parquet file: {}", e))
        })?;

        let schema_descr = reader.metadata().file_metadata().schema_descr();
        let arrow_schema = parquet::arrow::parquet_to_arrow_schema(
            schema_descr,
            reader.metadata().file_metadata().key_value_metadata(),
        )
        .map_err(|e| McpError::StorageError(format!("Failed to convert schema: {}", e)))?;

        let fields: Vec<FieldInfo> = arrow_schema
            .fields()
            .iter()
            .map(|f| {
                FieldInfo::new(f.name().clone(), format!("{:?}", f.data_type()))
                    .with_nullable(f.is_nullable())
            })
            .collect();

        Ok(fields)
    }

    /// Read N rows from a Parquet file.
    ///
    /// Returns rows as JSON objects with all columns.
    fn read_rows(&self, path: &Path, n: usize) -> McpResult<Vec<Value>> {
        let file = File::open(path).map_err(|e| {
            McpError::StorageError(format!("Failed to open {}: {}", path.display(), e))
        })?;

        let reader = SerializedFileReader::new(file).map_err(|e| {
            McpError::StorageError(format!("Failed to read Parquet file: {}", e))
        })?;

        let metadata = reader.metadata();
        let schema = metadata.file_metadata().schema_descr();
        let total_rows: usize = metadata
            .row_groups()
            .iter()
            .map(|rg| rg.num_rows() as usize)
            .sum();

        // Calculate which rows to read (last N rows)
        let skip_rows = total_rows.saturating_sub(n);

        let mut rows = Vec::with_capacity(n.min(total_rows));
        let row_iter = reader.get_row_iter(None).map_err(|e| {
            McpError::StorageError(format!("Failed to create row iterator: {}", e))
        })?;

        let mut current_row = 0;
        for row_result in row_iter {
            let row = row_result.map_err(|e| {
                McpError::StorageError(format!("Failed to read row: {}", e))
            })?;

            if current_row >= skip_rows {
                // Convert row to JSON
                let json_row = self.row_to_json(&row, schema)?;
                rows.push(json_row);

                if rows.len() >= n {
                    break;
                }
            }
            current_row += 1;
        }

        Ok(rows)
    }

    /// Convert a Parquet row to JSON.
    ///
    /// Handles the Bronze schema columns: timestamp, source_id, ndp_id, context, raw_payload
    fn row_to_json(
        &self,
        row: &parquet::record::Row,
        _schema: &parquet::schema::types::SchemaDescriptor,
    ) -> McpResult<Value> {
        let mut obj = serde_json::Map::new();

        // Iterate over the row's columns directly using get_column_iter
        for (name, field) in row.get_column_iter() {
            let value = self.field_to_json_value(field)?;
            obj.insert(name.clone(), value);
        }

        Ok(Value::Object(obj))
    }

    /// Convert a single field value to JSON.
    fn field_to_json_value(&self, field: &parquet::record::Field) -> McpResult<Value> {
        use parquet::record::Field;

        match field {
            Field::Null => Ok(Value::Null),
            Field::Bool(b) => Ok(Value::Bool(*b)),
            Field::Byte(b) => Ok(json!(b)),
            Field::Short(s) => Ok(json!(s)),
            Field::Int(i) => Ok(json!(i)),
            Field::Long(l) => Ok(json!(l)),
            Field::UByte(b) => Ok(json!(b)),
            Field::UShort(s) => Ok(json!(s)),
            Field::UInt(i) => Ok(json!(i)),
            Field::ULong(l) => Ok(json!(l)),
            Field::Float(f) => Ok(json!(f)),
            Field::Float16(f) => Ok(json!(f32::from(*f))),
            Field::Double(d) => Ok(json!(d)),
            Field::Decimal(d) => Ok(json!(format!("{:?}", d))),
            Field::Str(s) => {
                // Try to parse as JSON for raw_payload and context columns
                if let Ok(parsed) = serde_json::from_str::<Value>(s) {
                    Ok(parsed)
                } else {
                    Ok(Value::String(s.clone()))
                }
            }
            Field::Bytes(b) => Ok(json!(base64_encode(b.data()))),
            Field::Date(d) => Ok(json!(d)),
            Field::TimeMillis(t) => Ok(json!(t)),
            Field::TimeMicros(t) => Ok(json!(t)),
            Field::TimestampMillis(ts) => {
                let dt = DateTime::from_timestamp_millis(*ts);
                match dt {
                    Some(d) => Ok(Value::String(d.to_rfc3339())),
                    None => Ok(json!(ts)),
                }
            }
            Field::TimestampMicros(ts) => {
                let dt = DateTime::from_timestamp_micros(*ts);
                match dt {
                    Some(d) => Ok(Value::String(d.to_rfc3339())),
                    None => Ok(json!(ts)),
                }
            }
            Field::Group(g) => {
                // Recursively handle nested groups
                let mut map = serde_json::Map::new();
                for (name, f) in g.get_column_iter() {
                    let val = self.field_to_json_value(f)?;
                    map.insert(name.clone(), val);
                }
                Ok(Value::Object(map))
            }
            Field::ListInternal(list) => {
                let elements: Vec<Value> = list
                    .elements()
                    .iter()
                    .map(|f| self.field_to_json_value(f))
                    .collect::<McpResult<Vec<_>>>()?;
                Ok(Value::Array(elements))
            }
            Field::MapInternal(_) => {
                // Maps are complex; return as string representation
                Ok(Value::String(format!("{:?}", field)))
            }
        }
    }


    /// Extract structure from raw_payload JSON values.
    ///
    /// Analyzes multiple samples to build a complete picture of
    /// the raw_payload structure (capturing optional fields).
    fn extract_raw_payload_structure(&self, rows: &[Value]) -> RawPayloadStructure {
        let mut keys = Vec::new();
        let mut nested: HashMap<String, Vec<String>> = HashMap::new();

        for row in rows {
            if let Some(raw_payload) = row.get("raw_payload") {
                if let Some(obj) = raw_payload.as_object() {
                    for (key, val) in obj {
                        if !keys.contains(key) {
                            keys.push(key.clone());
                        }

                        // Track nested object keys
                        if let Some(nested_obj) = val.as_object() {
                            let nested_keys: Vec<String> = nested_obj.keys().cloned().collect();
                            let entry = nested.entry(key.clone()).or_default();
                            for k in nested_keys {
                                if !entry.contains(&k) {
                                    entry.push(k);
                                }
                            }
                        } else if let Some(arr) = val.as_array() {
                            // Check if array contains objects
                            for elem in arr {
                                if let Some(elem_obj) = elem.as_object() {
                                    let elem_keys: Vec<String> = elem_obj.keys().cloned().collect();
                                    let entry = nested.entry(key.clone()).or_default();
                                    for k in elem_keys {
                                        if !entry.contains(&k) {
                                            entry.push(k);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        RawPayloadStructure { keys, nested }
    }
}

#[async_trait]
impl BronzeStorage for LocalParquetStorage {
    async fn list_streams(&self) -> McpResult<Vec<StreamStorageInfo>> {
        if !self.base_path.exists() {
            return Err(McpError::StorageError(format!(
                "Base path does not exist: {}",
                self.base_path.display()
            )));
        }

        let entries = fs::read_dir(&self.base_path).map_err(|e| {
            McpError::StorageError(format!(
                "Failed to read directory {}: {}",
                self.base_path.display(),
                e
            ))
        })?;

        let mut streams = Vec::new();

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();

            // Only process directories (stream_id directories)
            if !path.is_dir() {
                continue;
            }

            let stream_id = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Skip hidden directories
            if stream_id.starts_with('.') {
                continue;
            }

            // Scan stream storage for metadata
            match self.scan_stream_storage(&stream_id)? {
                Some(info) => streams.push(info),
                None => {
                    // Stream directory exists but no partitions yet
                    streams.push(StreamStorageInfo::new(stream_id));
                }
            }
        }

        // Sort by stream_id for consistent ordering
        streams.sort_by(|a, b| a.stream_id.cmp(&b.stream_id));

        Ok(streams)
    }

    async fn get_schema(&self, stream_id: &str) -> McpResult<ParquetSchemaInfo> {
        let parquet_path = self
            .find_latest_partition(stream_id)?
            .ok_or_else(|| McpError::StreamNotFound(stream_id.to_string()))?;

        let fields = self.read_parquet_schema(&parquet_path)?;

        // Sample rows to analyze raw_payload structure
        let rows = self.read_rows(&parquet_path, 10)?;
        let raw_payload_structure = if !rows.is_empty() {
            Some(self.extract_raw_payload_structure(&rows))
        } else {
            None
        };

        Ok(ParquetSchemaInfo::new(
            stream_id,
            parquet_path.to_string_lossy().to_string(),
        )
        .with_fields(fields)
        .with_payload_structure(raw_payload_structure.unwrap_or_default()))
    }

    async fn sample(&self, stream_id: &str, n: usize) -> McpResult<Vec<Value>> {
        // Clamp n to valid range (1-100)
        let n = n.clamp(1, 100);

        let parquet_path = self
            .find_latest_partition(stream_id)?
            .ok_or_else(|| McpError::StreamNotFound(stream_id.to_string()))?;

        self.read_rows(&parquet_path, n)
    }

    async fn latest_partition(&self, stream_id: &str) -> McpResult<Option<String>> {
        let parquet_path = self.find_latest_partition(stream_id)?;

        match parquet_path {
            Some(path) => Ok(self.extract_partition_path(&path, stream_id)),
            None => Ok(None),
        }
    }
}

// Note: base64 encoding helper (inline to avoid extra dependency)
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut result = String::new();
    let chunks = data.chunks(3);

    for chunk in chunks {
        let b0 = chunk.first().copied().unwrap_or(0);
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        result.push(ALPHABET[(b0 >> 2) as usize] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4 | (b1 >> 4)) as usize] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((b1 & 0x0f) << 2 | (b2 >> 6)) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[(b2 & 0x3f) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::{Int64Array, StringArray};
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow::record_batch::RecordBatch;
    use parquet::arrow::ArrowWriter;
    use parquet::file::properties::WriterProperties;
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Create a test directory structure with Parquet files.
    fn create_test_storage() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();
        (temp_dir, base_path)
    }

    /// Create a test Parquet file with Bronze schema.
    fn create_test_parquet(path: &Path, rows: &[(i64, &str, &str)]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();

        let timestamp_array = Int64Array::from(rows.iter().map(|r| r.0).collect::<Vec<_>>());
        let source_id_array =
            StringArray::from(rows.iter().map(|r| r.1).collect::<Vec<&str>>());
        let raw_payload_array =
            StringArray::from(rows.iter().map(|r| r.2).collect::<Vec<&str>>());

        let schema = Arc::new(Schema::new(vec![
            Field::new("timestamp", DataType::Int64, false),
            Field::new("source_id", DataType::Utf8, false),
            Field::new("raw_payload", DataType::Utf8, false),
        ]));

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(timestamp_array),
                Arc::new(source_id_array),
                Arc::new(raw_payload_array),
            ],
        )
        .unwrap();

        let file = File::create(path).unwrap();
        let props = WriterProperties::builder().build();
        let mut writer = ArrowWriter::try_new(file, schema, Some(props)).unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();
    }

    /// Create Hive-style partition directories.
    fn create_partition_dirs(
        base: &Path,
        stream_id: &str,
        year: i32,
        month: u32,
        day: u32,
    ) -> PathBuf {
        let partition_path = base
            .join(stream_id)
            .join(format!("year={}", year))
            .join(format!("month={:02}", month))
            .join(format!("day={:02}", day));
        fs::create_dir_all(&partition_path).unwrap();
        partition_path.join("data.parquet")
    }

    #[tokio::test]
    async fn test_new() {
        let storage = LocalParquetStorage::new("/data/raw");
        assert_eq!(storage.base_path(), Path::new("/data/raw"));
    }

    #[tokio::test]
    async fn test_list_streams_empty_directory() {
        let (_temp_dir, base_path) = create_test_storage();
        let storage = LocalParquetStorage::new(&base_path);

        let streams = storage.list_streams().await.unwrap();
        assert!(streams.is_empty());
    }

    #[tokio::test]
    async fn test_list_streams_with_data() {
        let (_temp_dir, base_path) = create_test_storage();

        // Create test stream with partition
        let parquet_path = create_partition_dirs(&base_path, "air-quality", 2026, 1, 3);
        create_test_parquet(
            &parquet_path,
            &[(1704268800000000, "air-quality-Http", r#"{"pm25": 12.5}"#)],
        );

        let storage = LocalParquetStorage::new(&base_path);
        let streams = storage.list_streams().await.unwrap();

        assert_eq!(streams.len(), 1);
        assert_eq!(streams[0].stream_id, "air-quality");
        assert!(streams[0].latest_partition.is_some());
        assert!(streams[0].file_size_bytes.is_some());
        assert!(streams[0].row_count.is_some());
        assert_eq!(streams[0].row_count, Some(1));
    }

    #[tokio::test]
    async fn test_find_latest_partition_descending_order() {
        let (_temp_dir, base_path) = create_test_storage();

        // Create multiple partitions (not in chronological order)
        let path1 = create_partition_dirs(&base_path, "test-stream", 2026, 1, 1);
        let path2 = create_partition_dirs(&base_path, "test-stream", 2026, 1, 3);
        let path3 = create_partition_dirs(&base_path, "test-stream", 2025, 12, 31);

        create_test_parquet(&path1, &[(1, "test", r#"{"old": true}"#)]);
        create_test_parquet(&path2, &[(2, "test", r#"{"latest": true}"#)]);
        create_test_parquet(&path3, &[(3, "test", r#"{"older": true}"#)]);

        let storage = LocalParquetStorage::new(&base_path);
        let latest = storage.find_latest_partition("test-stream").unwrap();

        assert!(latest.is_some());
        let latest_path = latest.unwrap();
        assert!(latest_path.to_string_lossy().contains("year=2026"));
        assert!(latest_path.to_string_lossy().contains("month=01"));
        assert!(latest_path.to_string_lossy().contains("day=03"));
    }

    #[tokio::test]
    async fn test_get_schema() {
        let (_temp_dir, base_path) = create_test_storage();

        let parquet_path = create_partition_dirs(&base_path, "test-stream", 2026, 1, 1);
        create_test_parquet(&parquet_path, &[(1, "test", r#"{"value": 42}"#)]);

        let storage = LocalParquetStorage::new(&base_path);
        let schema = storage.get_schema("test-stream").await.unwrap();

        assert_eq!(schema.stream_id, "test-stream");
        assert_eq!(schema.fields.len(), 3);
        assert!(schema.fields.iter().any(|f| f.name == "timestamp"));
        assert!(schema.fields.iter().any(|f| f.name == "source_id"));
        assert!(schema.fields.iter().any(|f| f.name == "raw_payload"));
    }

    #[tokio::test]
    async fn test_get_schema_stream_not_found() {
        let (_temp_dir, base_path) = create_test_storage();
        let storage = LocalParquetStorage::new(&base_path);

        let result = storage.get_schema("nonexistent").await;
        assert!(result.is_err());
        assert!(matches!(result, Err(McpError::StreamNotFound(_))));
    }

    #[tokio::test]
    async fn test_sample() {
        let (_temp_dir, base_path) = create_test_storage();

        let parquet_path = create_partition_dirs(&base_path, "test-stream", 2026, 1, 1);
        create_test_parquet(
            &parquet_path,
            &[
                (1, "test", r#"{"pm25": 10.0, "temperature": 20.0}"#),
                (2, "test", r#"{"pm25": 11.0, "temperature": 21.0}"#),
                (3, "test", r#"{"pm25": 12.0, "temperature": 22.0}"#),
            ],
        );

        let storage = LocalParquetStorage::new(&base_path);
        let rows = storage.sample("test-stream", 2).await.unwrap();

        assert_eq!(rows.len(), 2);
        // Should return last 2 rows (most recent)
        assert!(rows[0].get("raw_payload").is_some());
    }

    #[tokio::test]
    async fn test_sample_clamps_to_max() {
        let (_temp_dir, base_path) = create_test_storage();

        let parquet_path = create_partition_dirs(&base_path, "test-stream", 2026, 1, 1);
        create_test_parquet(&parquet_path, &[(1, "test", r#"{"value": 1}"#)]);

        let storage = LocalParquetStorage::new(&base_path);
        // Request more than max (100) - should be clamped
        let rows = storage.sample("test-stream", 200).await.unwrap();

        // Only 1 row in file, so should return 1
        assert_eq!(rows.len(), 1);
    }

    #[tokio::test]
    async fn test_latest_partition() {
        let (_temp_dir, base_path) = create_test_storage();

        let parquet_path = create_partition_dirs(&base_path, "test-stream", 2026, 1, 15);
        create_test_parquet(&parquet_path, &[(1, "test", r#"{"value": 1}"#)]);

        let storage = LocalParquetStorage::new(&base_path);
        let partition = storage.latest_partition("test-stream").await.unwrap();

        assert!(partition.is_some());
        assert_eq!(partition.unwrap(), "year=2026/month=01/day=15");
    }

    #[tokio::test]
    async fn test_latest_partition_none() {
        let (_temp_dir, base_path) = create_test_storage();
        let storage = LocalParquetStorage::new(&base_path);

        let partition = storage.latest_partition("nonexistent").await.unwrap();
        assert!(partition.is_none());
    }

    #[tokio::test]
    async fn test_raw_payload_structure_extraction() {
        let (_temp_dir, base_path) = create_test_storage();

        let parquet_path = create_partition_dirs(&base_path, "test-stream", 2026, 1, 1);
        create_test_parquet(
            &parquet_path,
            &[
                (1, "test", r#"{"pm25": 10.0, "sensor": {"id": "abc"}}"#),
                (2, "test", r#"{"pm25": 11.0, "humidity": 65}"#),
            ],
        );

        let storage = LocalParquetStorage::new(&base_path);
        let schema = storage.get_schema("test-stream").await.unwrap();

        // Check raw_payload structure
        let structure = schema.raw_payload_structure.unwrap();
        assert!(structure.keys.contains(&"pm25".to_string()));
        assert!(structure.nested.contains_key("sensor"));
    }

    #[tokio::test]
    async fn test_multiple_streams_sorted() {
        let (_temp_dir, base_path) = create_test_storage();

        // Create streams in non-alphabetical order
        for stream in &["zebra", "alpha", "middle"] {
            let parquet_path = create_partition_dirs(&base_path, stream, 2026, 1, 1);
            create_test_parquet(&parquet_path, &[(1, stream, r#"{"v": 1}"#)]);
        }

        let storage = LocalParquetStorage::new(&base_path);
        let streams = storage.list_streams().await.unwrap();

        assert_eq!(streams.len(), 3);
        assert_eq!(streams[0].stream_id, "alpha");
        assert_eq!(streams[1].stream_id, "middle");
        assert_eq!(streams[2].stream_id, "zebra");
    }

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
        assert_eq!(base64_encode(b"a"), "YQ==");
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64_encode(b"abc"), "YWJj");
    }
}
