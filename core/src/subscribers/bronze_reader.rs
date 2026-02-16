//! BronzeReader implementations for Silver catch-up processing.
//!
//! Provides three implementations of the `BronzeReader` trait:
//!
//! - `ParquetBronzeReader`: reads from Bronze Parquet files (historical days)
//! - `WalBronzeReader`: reads from the current day's WAL (today's data)
//! - `HybridBronzeReader`: composes both for full time-range coverage
//!
//! # Usage
//!
//! Wire `HybridBronzeReader` into `SilverSubscriber` to enable Silver catch-up
//! on restart. The hybrid reader queries Parquet for past days and WAL for today.

use crate::error::CoreResult;
use crate::storage::wal::WriteAheadLog;
use crate::subscribers::BronzeReader;
use crate::traits::RawStore;
use crate::types::RawDataPoint;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::Path;
use std::sync::Arc;
use tracing::debug;

/// Reads historical Bronze data from Parquet files via `RawStore::query_raw()`.
///
/// Covers completed days where Parquet snapshots have been written.
pub struct ParquetBronzeReader {
    store: Arc<dyn RawStore>,
}

impl ParquetBronzeReader {
    pub fn new(store: Arc<dyn RawStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl BronzeReader for ParquetBronzeReader {
    async fn read_since(
        &self,
        since: DateTime<Utc>,
        stream_filter: Option<&str>,
    ) -> CoreResult<Vec<RawDataPoint>> {
        let end = Utc::now();
        let source_filter = stream_filter.map(|s| s.to_string());
        debug!(
            since = %since,
            end = %end,
            stream_filter = ?stream_filter,
            "ParquetBronzeReader: querying Parquet store"
        );
        self.store.query_raw(since, end, source_filter).await
    }

    async fn get_latest_timestamp(
        &self,
        stream_filter: Option<&str>,
    ) -> CoreResult<Option<DateTime<Utc>>> {
        // Query a wide range and find the max timestamp
        let start = DateTime::<Utc>::MIN_UTC;
        let end = Utc::now();
        let source_filter = stream_filter.map(|s| s.to_string());
        let points = self.store.query_raw(start, end, source_filter).await?;
        Ok(points.iter().map(|p| p.timestamp).max())
    }
}

/// Reads today's Bronze data from the WAL via `WriteAheadLog::replay_since()`.
///
/// The WAL reader opens the WAL file read-only for replay. It creates its own
/// `WriteAheadLog` instance on the same file path — safe because `replay_since()`
/// opens a fresh read handle each time (no conflict with append writes).
pub struct WalBronzeReader {
    wal_path: std::path::PathBuf,
}

impl WalBronzeReader {
    pub fn new(wal_path: impl AsRef<Path>) -> Self {
        Self {
            wal_path: wal_path.as_ref().to_path_buf(),
        }
    }

    /// Replay WAL entries, filtering by timestamp and optional stream.
    fn replay_filtered(
        &self,
        since: DateTime<Utc>,
        stream_filter: Option<&str>,
    ) -> CoreResult<Vec<RawDataPoint>> {
        // Open a read-only WAL instance to replay entries
        let wal = WriteAheadLog::new(&self.wal_path)?;
        let entries = wal.replay_since(0)?;

        debug!(
            wal_entries = entries.len(),
            since = %since,
            stream_filter = ?stream_filter,
            "WalBronzeReader: replaying WAL"
        );

        let points: Vec<RawDataPoint> = entries
            .into_iter()
            .filter(|e| e.point.timestamp >= since)
            .filter(|e| match stream_filter {
                Some(filter) => e.source_id.starts_with(filter),
                None => true,
            })
            .map(|e| e.point)
            .collect();

        Ok(points)
    }
}

#[async_trait]
impl BronzeReader for WalBronzeReader {
    async fn read_since(
        &self,
        since: DateTime<Utc>,
        stream_filter: Option<&str>,
    ) -> CoreResult<Vec<RawDataPoint>> {
        self.replay_filtered(since, stream_filter)
    }

    async fn get_latest_timestamp(
        &self,
        stream_filter: Option<&str>,
    ) -> CoreResult<Option<DateTime<Utc>>> {
        let points = self.replay_filtered(DateTime::<Utc>::MIN_UTC, stream_filter)?;
        Ok(points.iter().map(|p| p.timestamp).max())
    }
}

/// Composite BronzeReader that combines Parquet (historical) and WAL (today).
///
/// Queries both sources and merges results, deduplicating by timestamp + source_id.
/// This enables full Bronze catch-up: Parquet covers completed days, WAL covers
/// the current day's data that hasn't been finalized into Parquet yet.
pub struct HybridBronzeReader {
    parquet: ParquetBronzeReader,
    wal: WalBronzeReader,
}

impl HybridBronzeReader {
    pub fn new(store: Arc<dyn RawStore>, wal_path: impl AsRef<Path>) -> Self {
        Self {
            parquet: ParquetBronzeReader::new(store),
            wal: WalBronzeReader::new(wal_path),
        }
    }
}

#[async_trait]
impl BronzeReader for HybridBronzeReader {
    async fn read_since(
        &self,
        since: DateTime<Utc>,
        stream_filter: Option<&str>,
    ) -> CoreResult<Vec<RawDataPoint>> {
        // Query both sources
        let parquet_points = self.parquet.read_since(since, stream_filter).await?;
        let wal_points = self.wal.read_since(since, stream_filter).await?;

        debug!(
            parquet_count = parquet_points.len(),
            wal_count = wal_points.len(),
            "HybridBronzeReader: merging Parquet + WAL results"
        );

        if parquet_points.is_empty() {
            return Ok(wal_points);
        }
        if wal_points.is_empty() {
            return Ok(parquet_points);
        }

        // Merge and deduplicate: WAL entries are authoritative for today.
        // Parquet may contain older snapshots of the same data.
        // Dedup key: (source_id, timestamp) — keep WAL version if duplicate.
        use std::collections::HashSet;
        let mut seen: HashSet<(String, DateTime<Utc>)> = HashSet::new();
        let mut merged = Vec::with_capacity(parquet_points.len() + wal_points.len());

        // WAL entries first (authoritative for today)
        for point in wal_points {
            let key = (point.source_id.clone(), point.timestamp);
            seen.insert(key);
            merged.push(point);
        }

        // Parquet entries (skip duplicates already in WAL)
        for point in parquet_points {
            let key = (point.source_id.clone(), point.timestamp);
            if !seen.contains(&key) {
                merged.push(point);
            }
        }

        // Sort by timestamp ascending for consistent ordering
        merged.sort_by_key(|p| p.timestamp);

        Ok(merged)
    }

    async fn get_latest_timestamp(
        &self,
        stream_filter: Option<&str>,
    ) -> CoreResult<Option<DateTime<Utc>>> {
        let parquet_ts = self.parquet.get_latest_timestamp(stream_filter).await?;
        let wal_ts = self.wal.get_latest_timestamp(stream_filter).await?;

        Ok(match (parquet_ts, wal_ts) {
            (Some(p), Some(w)) => Some(p.max(w)),
            (Some(p), None) => Some(p),
            (None, Some(w)) => Some(w),
            (None, None) => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CoreError;
    use crate::traits::MockRawStore;
    use chrono::TimeZone;
    use serde_json::json;

    fn make_point(source_id: &str, hour: u32) -> RawDataPoint {
        let ts = Utc.with_ymd_and_hms(2026, 2, 14, hour, 0, 0).unwrap();
        RawDataPoint::new(source_id, json!({"value": hour}))
            .with_timestamp(ts)
            .with_ndp_id("test-device")
    }

    // ========== ParquetBronzeReader Tests ==========

    #[tokio::test]
    async fn test_parquet_reader_delegates_to_store() {
        let mut mock_store = MockRawStore::new();
        let p1 = make_point("air-quality-Mqtt", 10);
        let p2 = make_point("air-quality-Mqtt", 11);
        let expected = vec![p1.clone(), p2.clone()];

        mock_store
            .expect_query_raw()
            .times(1)
            .returning(move |_, _, _| Ok(vec![p1.clone(), p2.clone()]));

        let reader = ParquetBronzeReader::new(Arc::new(mock_store));
        let since = Utc.with_ymd_and_hms(2026, 2, 14, 0, 0, 0).unwrap();
        let result = reader.read_since(since, None).await.unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].source_id, expected[0].source_id);
        assert_eq!(result[1].source_id, expected[1].source_id);
    }

    #[tokio::test]
    async fn test_parquet_reader_passes_stream_filter() {
        let mut mock_store = MockRawStore::new();
        let p1 = make_point("air-quality-Mqtt", 10);

        mock_store
            .expect_query_raw()
            .times(1)
            .withf(|_, _, filter| filter.as_deref() == Some("air-quality-Mqtt"))
            .returning(move |_, _, _| Ok(vec![p1.clone()]));

        let reader = ParquetBronzeReader::new(Arc::new(mock_store));
        let since = Utc.with_ymd_and_hms(2026, 2, 14, 0, 0, 0).unwrap();
        let result = reader
            .read_since(since, Some("air-quality-Mqtt"))
            .await
            .unwrap();

        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_parquet_reader_empty_store() {
        let mut mock_store = MockRawStore::new();
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(|_, _, _| Ok(vec![]));

        let reader = ParquetBronzeReader::new(Arc::new(mock_store));
        let since = Utc.with_ymd_and_hms(2026, 2, 14, 0, 0, 0).unwrap();
        let result = reader.read_since(since, None).await.unwrap();

        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_parquet_reader_get_latest_timestamp() {
        let mut mock_store = MockRawStore::new();
        let p1 = make_point("air-quality-Mqtt", 10);
        let p2 = make_point("air-quality-Mqtt", 15);
        let expected_ts = p2.timestamp;

        mock_store
            .expect_query_raw()
            .times(1)
            .returning(move |_, _, _| Ok(vec![p1.clone(), p2.clone()]));

        let reader = ParquetBronzeReader::new(Arc::new(mock_store));
        let ts = reader.get_latest_timestamp(None).await.unwrap();

        assert_eq!(ts, Some(expected_ts));
    }

    // ========== WalBronzeReader Tests ==========

    fn create_wal_with_entries(entries: &[(&str, u32)]) -> (tempfile::TempDir, std::path::PathBuf) {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("wal.log");
        let mut wal = WriteAheadLog::new(&wal_path).unwrap();

        for &(source_id, hour) in entries {
            let point = make_point(source_id, hour);
            wal.append_point(&point).unwrap();
        }

        (temp_dir, wal_path)
    }

    #[tokio::test]
    async fn test_wal_reader_replays_all_entries() {
        let (_temp_dir, wal_path) = create_wal_with_entries(&[
            ("air-quality-Mqtt", 10),
            ("air-quality-Mqtt", 11),
            ("outdoor-weather-Http", 12),
        ]);

        let reader = WalBronzeReader::new(&wal_path);
        let since = Utc.with_ymd_and_hms(2026, 2, 14, 0, 0, 0).unwrap();
        let result = reader.read_since(since, None).await.unwrap();

        assert_eq!(result.len(), 3);
    }

    #[tokio::test]
    async fn test_wal_reader_filters_by_timestamp() {
        let (_temp_dir, wal_path) = create_wal_with_entries(&[
            ("air-quality-Mqtt", 8),
            ("air-quality-Mqtt", 10),
            ("air-quality-Mqtt", 14),
        ]);

        let reader = WalBronzeReader::new(&wal_path);
        // Only entries at or after 10:00
        let since = Utc.with_ymd_and_hms(2026, 2, 14, 10, 0, 0).unwrap();
        let result = reader.read_since(since, None).await.unwrap();

        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn test_wal_reader_filters_by_stream() {
        let (_temp_dir, wal_path) = create_wal_with_entries(&[
            ("air-quality-Mqtt", 10),
            ("outdoor-weather-Http", 11),
            ("air-quality-Mqtt", 12),
        ]);

        let reader = WalBronzeReader::new(&wal_path);
        let since = Utc.with_ymd_and_hms(2026, 2, 14, 0, 0, 0).unwrap();
        let result = reader.read_since(since, Some("air-quality")).await.unwrap();

        assert_eq!(result.len(), 2);
        for p in &result {
            assert!(p.source_id.starts_with("air-quality"));
        }
    }

    #[tokio::test]
    async fn test_wal_reader_empty_wal() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("wal.log");
        // Create empty WAL
        let _wal = WriteAheadLog::new(&wal_path).unwrap();

        let reader = WalBronzeReader::new(&wal_path);
        let since = Utc.with_ymd_and_hms(2026, 2, 14, 0, 0, 0).unwrap();
        let result = reader.read_since(since, None).await.unwrap();

        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_wal_reader_get_latest_timestamp() {
        let (_temp_dir, wal_path) =
            create_wal_with_entries(&[("air-quality-Mqtt", 10), ("air-quality-Mqtt", 15)]);

        let reader = WalBronzeReader::new(&wal_path);
        let ts = reader.get_latest_timestamp(None).await.unwrap();

        let expected = Utc.with_ymd_and_hms(2026, 2, 14, 15, 0, 0).unwrap();
        assert_eq!(ts, Some(expected));
    }

    // ========== HybridBronzeReader Tests ==========

    #[tokio::test]
    async fn test_hybrid_reader_merges_parquet_and_wal() {
        let mut mock_store = MockRawStore::new();

        // Parquet has yesterday's data (hour 8, 9)
        let p1 = make_point("air-quality-Mqtt", 8);
        let p2 = make_point("air-quality-Mqtt", 9);
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(move |_, _, _| Ok(vec![p1.clone(), p2.clone()]));

        // WAL has today's data (hour 14, 15)
        let (_temp_dir, wal_path) =
            create_wal_with_entries(&[("air-quality-Mqtt", 14), ("air-quality-Mqtt", 15)]);

        let reader = HybridBronzeReader::new(Arc::new(mock_store), &wal_path);
        let since = Utc.with_ymd_and_hms(2026, 2, 14, 0, 0, 0).unwrap();
        let result = reader.read_since(since, None).await.unwrap();

        assert_eq!(result.len(), 4);
        // Should be sorted by timestamp
        for i in 1..result.len() {
            assert!(result[i].timestamp >= result[i - 1].timestamp);
        }
    }

    #[tokio::test]
    async fn test_hybrid_reader_deduplicates_overlapping_data() {
        let mut mock_store = MockRawStore::new();

        // Parquet has data at hour 10 (from previous snapshot)
        let p1 = make_point("air-quality-Mqtt", 10);
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(move |_, _, _| Ok(vec![p1.clone()]));

        // WAL also has data at hour 10 (not yet truncated)
        let (_temp_dir, wal_path) =
            create_wal_with_entries(&[("air-quality-Mqtt", 10), ("air-quality-Mqtt", 11)]);

        let reader = HybridBronzeReader::new(Arc::new(mock_store), &wal_path);
        let since = Utc.with_ymd_and_hms(2026, 2, 14, 0, 0, 0).unwrap();
        let result = reader.read_since(since, None).await.unwrap();

        // Should be 2 (not 3) — hour 10 is deduplicated
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn test_hybrid_reader_parquet_only_when_wal_empty() {
        let mut mock_store = MockRawStore::new();

        let p1 = make_point("air-quality-Mqtt", 10);
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(move |_, _, _| Ok(vec![p1.clone()]));

        // Empty WAL
        let temp_dir = tempfile::TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("wal.log");
        let _wal = WriteAheadLog::new(&wal_path).unwrap();

        let reader = HybridBronzeReader::new(Arc::new(mock_store), &wal_path);
        let since = Utc.with_ymd_and_hms(2026, 2, 14, 0, 0, 0).unwrap();
        let result = reader.read_since(since, None).await.unwrap();

        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_hybrid_reader_wal_only_when_parquet_empty() {
        let mut mock_store = MockRawStore::new();
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(|_, _, _| Ok(vec![]));

        let (_temp_dir, wal_path) = create_wal_with_entries(&[("air-quality-Mqtt", 14)]);

        let reader = HybridBronzeReader::new(Arc::new(mock_store), &wal_path);
        let since = Utc.with_ymd_and_hms(2026, 2, 14, 0, 0, 0).unwrap();
        let result = reader.read_since(since, None).await.unwrap();

        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn test_hybrid_reader_get_latest_timestamp() {
        let mut mock_store = MockRawStore::new();

        // Parquet latest: hour 9
        let p1 = make_point("air-quality-Mqtt", 9);
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(move |_, _, _| Ok(vec![p1.clone()]));

        // WAL latest: hour 15
        let (_temp_dir, wal_path) = create_wal_with_entries(&[("air-quality-Mqtt", 15)]);

        let reader = HybridBronzeReader::new(Arc::new(mock_store), &wal_path);
        let ts = reader.get_latest_timestamp(None).await.unwrap();

        let expected = Utc.with_ymd_and_hms(2026, 2, 14, 15, 0, 0).unwrap();
        assert_eq!(ts, Some(expected));
    }

    #[tokio::test]
    async fn test_hybrid_reader_both_empty() {
        let mut mock_store = MockRawStore::new();
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(|_, _, _| Ok(vec![]));

        let temp_dir = tempfile::TempDir::new().unwrap();
        let wal_path = temp_dir.path().join("wal.log");
        let _wal = WriteAheadLog::new(&wal_path).unwrap();

        let reader = HybridBronzeReader::new(Arc::new(mock_store), &wal_path);
        let since = Utc.with_ymd_and_hms(2026, 2, 14, 0, 0, 0).unwrap();
        let result = reader.read_since(since, None).await.unwrap();

        assert!(result.is_empty());

        // Note: get_latest_timestamp would need a new mock_store,
        // so we test that separately
    }
}
