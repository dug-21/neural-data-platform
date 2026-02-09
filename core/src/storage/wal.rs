use crate::error::{CoreError, CoreResult};
use crate::types::RawDataPoint;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use tracing::warn;

/// A structured WAL entry containing a data point with sequencing metadata.
///
/// Each entry is serialized as a single JSON line in the WAL file.
/// The `sequence` field provides total ordering for replay and commit tracking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WalEntry {
    /// Monotonically increasing sequence number assigned at append time.
    pub sequence: u64,
    /// Source identifier for the data point (e.g., "air-quality-Mqtt").
    pub source_id: String,
    /// Timestamp of the data point.
    pub timestamp: DateTime<Utc>,
    /// The raw data point being durably stored.
    pub point: RawDataPoint,
}

/// Watermark header stored as the first line of the WAL file.
/// Allows recovery of committed watermark on restart.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WalHeader {
    __watermark: u64,
}

pub struct WriteAheadLog {
    path: PathBuf,
    file: File,
    next_sequence: u64,
    committed_watermark: u64,
}

impl WriteAheadLog {
    /// Create or recover a WriteAheadLog at the given path.
    ///
    /// On fresh creation: next_sequence=1, committed_watermark=0.
    /// On recovery: scans the existing file to restore watermark and next_sequence.
    pub fn new<P: AsRef<Path>>(path: P) -> CoreResult<Self> {
        let path = path.as_ref().to_path_buf();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Recover state from existing file if present
        let (next_sequence, committed_watermark) = if path.exists() {
            Self::recover_state(&path)?
        } else {
            (1, 0)
        };

        let file = OpenOptions::new().create(true).append(true).open(&path)?;

        Ok(Self {
            path,
            file,
            next_sequence,
            committed_watermark,
        })
    }

    /// Scan an existing WAL file to recover watermark and next sequence number.
    fn recover_state(path: &Path) -> CoreResult<(u64, u64)> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut max_sequence: u64 = 0;
        let mut watermark: u64 = 0;

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Check for watermark header
            if let Ok(header) = serde_json::from_str::<WalHeader>(trimmed) {
                watermark = header.__watermark;
                continue;
            }

            // Try to parse as WalEntry to get sequence
            if let Ok(entry) = serde_json::from_str::<WalEntry>(trimmed) {
                if entry.sequence > max_sequence {
                    max_sequence = entry.sequence;
                }
            }
            // If line doesn't parse as either, skip it (could be old-format or corrupted)
        }

        Ok((max_sequence + 1, watermark))
    }

    // ========== NEW V2 API ==========

    /// Append a RawDataPoint to the WAL, returning the assigned sequence number.
    ///
    /// The caller must treat a failed append as "data not durable" and must NOT
    /// add the point to the accumulator.
    pub fn append_point(&mut self, point: &RawDataPoint) -> CoreResult<u64> {
        let seq = self.next_sequence;
        let entry = WalEntry {
            sequence: seq,
            source_id: point.source_id.clone(),
            timestamp: point.timestamp,
            point: point.clone(),
        };

        let json_str = serde_json::to_string(&entry)
            .map_err(|e| CoreError::Storage(format!("Failed to serialize WAL entry: {}", e)))?;

        writeln!(self.file, "{}", json_str)?;
        self.file.flush()?;

        self.next_sequence += 1;
        Ok(seq)
    }

    /// Replay all entries with sequence > watermark.
    ///
    /// Corrupted trailing lines (e.g., from a crash) are skipped with a warning.
    pub fn replay_since(&self, watermark: u64) -> CoreResult<Vec<WalEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();
        let mut line_number = 0;
        let mut last_line_was_corrupt = false;

        for line_result in reader.lines() {
            line_number += 1;
            let line = line_result?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Skip watermark header lines
            if serde_json::from_str::<WalHeader>(trimmed).is_ok() {
                last_line_was_corrupt = false;
                continue;
            }

            match serde_json::from_str::<WalEntry>(trimmed) {
                Ok(entry) => {
                    last_line_was_corrupt = false;
                    if entry.sequence > watermark {
                        entries.push(entry);
                    }
                }
                Err(e) => {
                    // Only warn for corrupted lines; they may be old-format or partial writes
                    last_line_was_corrupt = true;
                    warn!(
                        line = line_number,
                        error = %e,
                        "Skipping corrupted WAL line"
                    );
                }
            }
        }

        let _ = last_line_was_corrupt; // suppress unused warning; logged above

        Ok(entries)
    }

    /// Commit all entries up to and including the given watermark.
    ///
    /// Rewrites the WAL file keeping only entries with sequence > watermark.
    /// Uses atomic temp-file + rename for crash safety.
    /// No-op if watermark <= current committed_watermark.
    pub fn commit_to(&mut self, watermark: u64) -> CoreResult<()> {
        if watermark <= self.committed_watermark {
            return Ok(());
        }

        // Read all current entries with sequence > watermark
        let surviving = self.replay_since(watermark)?;

        // Write to a temp file, then atomic rename
        let temp_path = self.path.with_extension("tmp");
        {
            let mut temp_file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&temp_path)?;

            // Write watermark header
            let header = WalHeader {
                __watermark: watermark,
            };
            let header_json = serde_json::to_string(&header).map_err(|e| {
                CoreError::Storage(format!("Failed to serialize WAL header: {}", e))
            })?;
            writeln!(temp_file, "{}", header_json)?;

            // Write surviving entries
            for entry in &surviving {
                let json_str = serde_json::to_string(entry).map_err(|e| {
                    CoreError::Storage(format!("Failed to serialize WAL entry: {}", e))
                })?;
                writeln!(temp_file, "{}", json_str)?;
            }
            temp_file.flush()?;
        }

        // Atomic rename
        std::fs::rename(&temp_path, &self.path)?;

        // Reopen file for appending
        self.file = OpenOptions::new().append(true).open(&self.path)?;
        self.committed_watermark = watermark;

        Ok(())
    }

    /// Return the current committed watermark.
    pub fn current_watermark(&self) -> u64 {
        self.committed_watermark
    }

    /// Return the next sequence number that will be assigned.
    pub fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    // ========== LEGACY V1 API (used by parquet.rs) ==========

    /// Append raw bytes as a single line to the WAL.
    ///
    /// This is the legacy V1 API preserved for backward compatibility with
    /// `ParquetStore` which serializes `TimeSeriesPoint` as bytes.
    pub fn append(&mut self, entry: &[u8]) -> CoreResult<()> {
        let json_str = std::str::from_utf8(entry)
            .map_err(|e| CoreError::Storage(format!("Invalid UTF-8 in WAL entry: {}", e)))?;

        writeln!(self.file, "{}", json_str)?;
        self.file.flush()?;

        Ok(())
    }

    /// Replay all non-empty lines as raw bytes.
    ///
    /// This is the legacy V1 API preserved for backward compatibility with
    /// `ParquetStore` which deserializes `TimeSeriesPoint` from bytes.
    pub fn replay(&self) -> CoreResult<Vec<Vec<u8>>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if !line.trim().is_empty() {
                // Skip watermark header lines in legacy replay
                if serde_json::from_str::<WalHeader>(line.trim()).is_ok() {
                    continue;
                }
                entries.push(line.into_bytes());
            }
        }

        Ok(entries)
    }

    /// Delete the WAL file and recreate it (legacy full commit).
    ///
    /// This is the legacy V1 API preserved for backward compatibility with
    /// `ParquetStore`.
    pub fn commit(&mut self) -> CoreResult<()> {
        std::fs::remove_file(&self.path)?;

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        self.file = file;
        self.next_sequence = 1;
        self.committed_watermark = 0;

        Ok(())
    }

    /// Return the WAL file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Return the WAL file size in bytes (0 if file doesn't exist).
    pub fn file_size_bytes(&self) -> u64 {
        std::fs::metadata(&self.path)
            .map(|m| m.len())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;
    use std::fs;
    use std::io::Write as IoWrite;

    fn temp_wal_path() -> PathBuf {
        let temp_dir = std::env::temp_dir();
        temp_dir.join(format!("test_wal_{}.log", uuid::Uuid::new_v4()))
    }

    fn make_test_point(source_id: &str, value: f64) -> RawDataPoint {
        RawDataPoint::new(source_id, json!({"value": value}))
            .with_timestamp(Utc.with_ymd_and_hms(2026, 2, 8, 10, 0, 0).unwrap())
    }

    fn make_test_point_at(source_id: &str, value: f64, ts: DateTime<Utc>) -> RawDataPoint {
        RawDataPoint::new(source_id, json!({"value": value})).with_timestamp(ts)
    }

    // ========== CYCLE 1: WalEntry serialization ==========

    #[test]
    fn test_wal_entry_serialization_round_trip() {
        let point = make_test_point("air-quality-Mqtt", 42.0);
        let entry = WalEntry {
            sequence: 1,
            source_id: "air-quality-Mqtt".to_string(),
            timestamp: point.timestamp,
            point: point.clone(),
        };

        let json_str = serde_json::to_string(&entry).unwrap();
        let restored: WalEntry = serde_json::from_str(&json_str).unwrap();

        assert_eq!(restored.sequence, 1);
        assert_eq!(restored.source_id, "air-quality-Mqtt");
        assert_eq!(restored.timestamp, entry.timestamp);
        assert_eq!(restored.point, point);
    }

    // ========== CYCLE 2: WAL creation (fresh) ==========

    #[test]
    fn test_wal_creation() {
        let path = temp_wal_path();
        let wal = WriteAheadLog::new(&path).unwrap();

        assert!(path.exists());
        assert_eq!(wal.next_sequence(), 1);
        assert_eq!(wal.current_watermark(), 0);

        let _ = fs::remove_file(&path);
    }

    // ========== CYCLE 3: append returns sequence ==========

    #[test]
    fn test_wal_append_returns_incrementing_sequences() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();

        let p1 = make_test_point("src-a", 1.0);
        let p2 = make_test_point("src-a", 2.0);
        let p3 = make_test_point("src-a", 3.0);

        assert_eq!(wal.append_point(&p1).unwrap(), 1);
        assert_eq!(wal.append_point(&p2).unwrap(), 2);
        assert_eq!(wal.append_point(&p3).unwrap(), 3);
        assert_eq!(wal.next_sequence(), 4);

        let _ = fs::remove_file(&path);
    }

    // ========== CYCLE 4: replay_since(0) returns all ==========

    #[test]
    fn test_wal_replay_since_zero_returns_all() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();

        for i in 0..5 {
            let point = make_test_point("src", i as f64);
            wal.append_point(&point).unwrap();
        }

        let entries = wal.replay_since(0).unwrap();
        assert_eq!(entries.len(), 5);

        // Verify sequences
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.sequence, (i + 1) as u64);
            assert_eq!(entry.point.raw_payload["value"], i as f64);
        }

        let _ = fs::remove_file(&path);
    }

    // ========== CYCLE 5: replay_since(N) filters correctly ==========

    #[test]
    fn test_wal_replay_since_filters_by_watermark() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();

        for i in 0..5 {
            let point = make_test_point("src", (i + 1) as f64);
            wal.append_point(&point).unwrap();
        }

        // replay_since(3) should return entries with sequence > 3 (i.e., 4 and 5)
        let entries = wal.replay_since(3).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sequence, 4);
        assert_eq!(entries[1].sequence, 5);

        let _ = fs::remove_file(&path);
    }

    // ========== CYCLE 6: commit_to with atomic rename ==========

    #[test]
    fn test_wal_commit_to_removes_committed_entries() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();

        for i in 0..5 {
            let point = make_test_point("src", (i + 1) as f64);
            wal.append_point(&point).unwrap();
        }

        wal.commit_to(3).unwrap();

        assert_eq!(wal.current_watermark(), 3);

        // Only entries 4 and 5 should survive
        let entries = wal.replay_since(0).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sequence, 4);
        assert_eq!(entries[1].sequence, 5);

        let _ = fs::remove_file(&path);
    }

    // ========== CYCLE 7: commit_to is no-op if watermark hasn't advanced ==========

    #[test]
    fn test_wal_commit_to_noop_for_stale_watermark() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();

        for i in 0..5 {
            let point = make_test_point("src", (i + 1) as f64);
            wal.append_point(&point).unwrap();
        }

        wal.commit_to(3).unwrap();
        assert_eq!(wal.current_watermark(), 3);

        // Attempting to commit to a lower watermark is a no-op
        wal.commit_to(2).unwrap();
        assert_eq!(wal.current_watermark(), 3);

        // Same watermark is also a no-op
        wal.commit_to(3).unwrap();
        assert_eq!(wal.current_watermark(), 3);

        // Entries 4 and 5 still present
        let entries = wal.replay_since(0).unwrap();
        assert_eq!(entries.len(), 2);

        let _ = fs::remove_file(&path);
    }

    // ========== CYCLE 8: WAL persistence across instances (recovery) ==========

    #[test]
    fn test_wal_persistence_across_instances() {
        let path = temp_wal_path();

        // Create WAL, append 3 entries, drop
        {
            let mut wal = WriteAheadLog::new(&path).unwrap();
            for i in 0..3 {
                let point = make_test_point("src", (i + 1) as f64);
                wal.append_point(&point).unwrap();
            }
        }

        // Reopen WAL at same path
        let wal2 = WriteAheadLog::new(&path).unwrap();
        assert_eq!(wal2.next_sequence(), 4);

        let entries = wal2.replay_since(0).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].sequence, 1);
        assert_eq!(entries[2].sequence, 3);

        let _ = fs::remove_file(&path);
    }

    // ========== CYCLE 9: Watermark persistence ==========

    #[test]
    fn test_wal_watermark_persistence_across_instances() {
        let path = temp_wal_path();

        // Create WAL, append 5, commit_to(3), drop
        {
            let mut wal = WriteAheadLog::new(&path).unwrap();
            for i in 0..5 {
                let point = make_test_point("src", (i + 1) as f64);
                wal.append_point(&point).unwrap();
            }
            wal.commit_to(3).unwrap();
        }

        // Reopen WAL at same path
        let wal2 = WriteAheadLog::new(&path).unwrap();
        assert_eq!(wal2.current_watermark(), 3);
        assert_eq!(wal2.next_sequence(), 6); // max(4,5) + 1

        let entries = wal2.replay_since(3).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].sequence, 4);
        assert_eq!(entries[1].sequence, 5);

        let _ = fs::remove_file(&path);
    }

    // ========== CYCLE 10: Corrupted trailing line (crash simulation) ==========

    #[test]
    fn test_wal_skips_corrupted_trailing_line() {
        let path = temp_wal_path();

        // Create WAL, append 3 entries
        {
            let mut wal = WriteAheadLog::new(&path).unwrap();
            for i in 0..3 {
                let point = make_test_point("src", (i + 1) as f64);
                wal.append_point(&point).unwrap();
            }
        }

        // Manually append a partial/corrupted JSON line simulating a crash
        {
            let mut file = OpenOptions::new().append(true).open(&path).unwrap();
            writeln!(file, "{{\"sequence\":4,\"source_id\":\"src\",\"truncated").unwrap();
        }

        // Reopen WAL -- should recover 3 valid entries, skip corrupted one
        let wal2 = WriteAheadLog::new(&path).unwrap();
        let entries = wal2.replay_since(0).unwrap();
        assert_eq!(entries.len(), 3);

        let _ = fs::remove_file(&path);
    }

    // ========== CYCLE 11: Backward compatibility (legacy API) ==========

    #[test]
    fn test_wal_legacy_append_and_replay() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();

        // Use legacy byte-based append (as ParquetStore does)
        let entry = json!({"timestamp": "2024-01-15T10:30:00Z", "value": 42.0});
        wal.append(&entry.to_string().into_bytes()).unwrap();

        let replayed = wal.replay().unwrap();
        assert_eq!(replayed.len(), 1);

        let replayed_json: serde_json::Value = serde_json::from_slice(&replayed[0]).unwrap();
        assert_eq!(replayed_json, entry);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_legacy_commit_clears_log() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();

        for i in 0..3 {
            let entry = json!({"id": i});
            wal.append(&entry.to_string().into_bytes()).unwrap();
        }

        assert_eq!(wal.replay().unwrap().len(), 3);

        wal.commit().unwrap();

        assert_eq!(wal.replay().unwrap().len(), 0);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_legacy_append_after_commit() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();

        let entry1 = json!({"id": 1});
        wal.append(&entry1.to_string().into_bytes()).unwrap();
        wal.commit().unwrap();

        let entry2 = json!({"id": 2});
        wal.append(&entry2.to_string().into_bytes()).unwrap();

        let replayed = wal.replay().unwrap();
        assert_eq!(replayed.len(), 1);

        let replayed_json: serde_json::Value = serde_json::from_slice(&replayed[0]).unwrap();
        assert_eq!(replayed_json, entry2);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_legacy_empty_replay() {
        let path = temp_wal_path();
        let wal = WriteAheadLog::new(&path).unwrap();

        let replayed = wal.replay().unwrap();
        assert_eq!(replayed.len(), 0);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_legacy_invalid_utf8() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();

        let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
        let result = wal.append(&invalid_utf8);

        assert!(result.is_err());
        if let Err(CoreError::Storage(msg)) = result {
            assert!(msg.contains("Invalid UTF-8"));
        }

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_legacy_persistence_across_instances() {
        let path = temp_wal_path();

        {
            let mut wal1 = WriteAheadLog::new(&path).unwrap();
            let entry = json!({"data": "persistent"});
            wal1.append(&entry.to_string().into_bytes()).unwrap();
        }

        let wal2 = WriteAheadLog::new(&path).unwrap();
        let replayed = wal2.replay().unwrap();

        assert_eq!(replayed.len(), 1);
        let replayed_json: serde_json::Value = serde_json::from_slice(&replayed[0]).unwrap();
        assert_eq!(replayed_json, json!({"data": "persistent"}));

        let _ = fs::remove_file(&path);
    }

    // ========== CYCLE 12: Empty WAL replay_since ==========

    #[test]
    fn test_wal_empty_replay_since() {
        let path = temp_wal_path();
        let wal = WriteAheadLog::new(&path).unwrap();

        let entries = wal.replay_since(0).unwrap();
        assert!(entries.is_empty());

        let _ = fs::remove_file(&path);
    }

    // ========== EXTRA: Legacy replay skips watermark headers ==========

    #[test]
    fn test_wal_legacy_replay_skips_watermark_headers() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();

        // Use v2 API to append and commit (creates watermark header)
        for i in 0..3 {
            let point = make_test_point("src", (i + 1) as f64);
            wal.append_point(&point).unwrap();
        }
        wal.commit_to(1).unwrap();

        // Legacy replay should not include the watermark header as an entry
        let legacy_entries = wal.replay().unwrap();
        for entry_bytes in &legacy_entries {
            let line = std::str::from_utf8(entry_bytes).unwrap();
            assert!(!line.contains("__watermark"), "Legacy replay should skip watermark headers");
        }

        let _ = fs::remove_file(&path);
    }

    // ========== EXTRA: Can append after commit_to ==========

    #[test]
    fn test_wal_append_after_commit_to() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();

        for i in 0..3 {
            let point = make_test_point("src", (i + 1) as f64);
            wal.append_point(&point).unwrap();
        }

        wal.commit_to(2).unwrap();

        // Append new entry after partial commit
        let point = make_test_point("src", 99.0);
        let seq = wal.append_point(&point).unwrap();
        assert_eq!(seq, 4);

        let entries = wal.replay_since(0).unwrap();
        assert_eq!(entries.len(), 2); // seq 3 (survived commit) + seq 4 (new)
        assert_eq!(entries[0].sequence, 3);
        assert_eq!(entries[1].sequence, 4);

        let _ = fs::remove_file(&path);
    }

    // ========== EXTRA: commit_to all entries ==========

    #[test]
    fn test_wal_commit_to_all_entries() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();

        for i in 0..3 {
            let point = make_test_point("src", (i + 1) as f64);
            wal.append_point(&point).unwrap();
        }

        // Commit all entries
        wal.commit_to(3).unwrap();

        let entries = wal.replay_since(0).unwrap();
        assert!(entries.is_empty());
        assert_eq!(wal.current_watermark(), 3);

        let _ = fs::remove_file(&path);
    }

    // ========== EXTRA: source_id tracking in WalEntry ==========

    #[test]
    fn test_wal_entry_preserves_source_id() {
        let path = temp_wal_path();
        let mut wal = WriteAheadLog::new(&path).unwrap();

        let p1 = make_test_point("air-quality-Mqtt", 1.0);
        let p2 = make_test_point("outdoor-weather-Http", 2.0);
        wal.append_point(&p1).unwrap();
        wal.append_point(&p2).unwrap();

        let entries = wal.replay_since(0).unwrap();
        assert_eq!(entries[0].source_id, "air-quality-Mqtt");
        assert_eq!(entries[1].source_id, "outdoor-weather-Http");

        let _ = fs::remove_file(&path);
    }
}
