//! In-memory accumulator for Bronze layer data points.
//!
//! Holds today's data in memory grouped by source_id. Provides efficient
//! insertion, deduplication (for WAL merge), and date-based partitioning.
//! Memory is estimated for backpressure signaling.

use crate::types::RawDataPoint;
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::HashMap;

/// In-memory accumulator holding RawDataPoints grouped by source_id.
///
/// Tracks earliest/latest timestamps and the active calendar date.
/// Used by the Bronze subscriber to buffer data before Parquet archival.
pub struct Accumulator {
    points: HashMap<String, Vec<RawDataPoint>>,
    count: usize,
    earliest: Option<DateTime<Utc>>,
    latest: Option<DateTime<Utc>>,
    active_date: NaiveDate,
}

impl Accumulator {
    /// Create a new empty accumulator for the given calendar date.
    pub fn new(date: NaiveDate) -> Self {
        Self {
            points: HashMap::new(),
            count: 0,
            earliest: None,
            latest: None,
            active_date: date,
        }
    }

    /// Add a single data point, updating count and timestamp bounds.
    pub fn add(&mut self, point: RawDataPoint) {
        let ts = point.timestamp;

        // Update earliest
        self.earliest = Some(match self.earliest {
            Some(e) if e <= ts => e,
            _ => ts,
        });

        // Update latest
        self.latest = Some(match self.latest {
            Some(l) if l >= ts => l,
            _ => ts,
        });

        self.points
            .entry(point.source_id.clone())
            .or_insert_with(Vec::new)
            .push(point);
        self.count += 1;
    }

    /// Returns a reference to all points grouped by source_id.
    pub fn all_points_by_source(&self) -> &HashMap<String, Vec<RawDataPoint>> {
        &self.points
    }

    /// Total number of data points across all sources.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Number of distinct source_ids.
    pub fn source_count(&self) -> usize {
        self.points.len()
    }

    /// Earliest timestamp seen, or None if empty.
    pub fn earliest(&self) -> Option<DateTime<Utc>> {
        self.earliest
    }

    /// Latest timestamp seen, or None if empty.
    pub fn latest(&self) -> Option<DateTime<Utc>> {
        self.latest
    }

    /// The calendar date this accumulator is tracking.
    pub fn active_date(&self) -> NaiveDate {
        self.active_date
    }

    /// Clear all data, resetting count and timestamp bounds. Active date is preserved.
    pub fn clear(&mut self) {
        self.points.clear();
        self.count = 0;
        self.earliest = None;
        self.latest = None;
    }

    /// Drain all points whose timestamp falls on the given date.
    ///
    /// Returns the removed points grouped by source_id. Points for other dates
    /// remain in the accumulator. Empty source buckets are removed.
    pub fn drain_for_date(&mut self, date: NaiveDate) -> HashMap<String, Vec<RawDataPoint>> {
        let mut drained: HashMap<String, Vec<RawDataPoint>> = HashMap::new();

        // Partition each source's points into kept vs drained
        let source_ids: Vec<String> = self.points.keys().cloned().collect();
        for source_id in source_ids {
            if let Some(points) = self.points.remove(&source_id) {
                let (matching, remaining): (Vec<_>, Vec<_>) = points
                    .into_iter()
                    .partition(|p| p.timestamp.date_naive() == date);

                if !matching.is_empty() {
                    drained.insert(source_id.clone(), matching);
                }
                if !remaining.is_empty() {
                    self.points.insert(source_id, remaining);
                }
            }
        }

        // Recompute count and timestamp bounds from remaining points
        self.recompute_stats();

        drained
    }

    /// Merge WAL-recovered entries, deduplicating by (source_id, timestamp).
    ///
    /// Callers extract RawDataPoint from WalEntry before calling this method.
    /// The accumulator does not depend on WAL types.
    pub fn merge_wal_entries(&mut self, entries: Vec<RawDataPoint>) {
        for entry in entries {
            let dominated = self
                .points
                .get(&entry.source_id)
                .map(|pts| {
                    pts.iter()
                        .any(|p| p.source_id == entry.source_id && p.timestamp == entry.timestamp)
                })
                .unwrap_or(false);

            if !dominated {
                self.add(entry);
            }
        }
    }

    /// Estimate memory usage in bytes.
    ///
    /// Accounts for HashMap overhead, String keys, Vec storage, and per-point
    /// serialized size estimates. This is a rough estimate for backpressure.
    pub fn memory_estimate_bytes(&self) -> usize {
        let hashmap_overhead = std::mem::size_of::<HashMap<String, Vec<RawDataPoint>>>();
        let per_bucket_overhead = self.points.len() * (
            std::mem::size_of::<String>()          // key
            + std::mem::size_of::<Vec<RawDataPoint>>() // value vec header
            + 64  // HashMap bucket metadata estimate
        );

        let per_point: usize = self.points.values().map(|pts| {
            pts.iter().map(|p| {
                std::mem::size_of::<RawDataPoint>()
                    + p.source_id.len()
                    + p.ndp_id.as_ref().map(|s| s.len()).unwrap_or(0)
                    + p.raw_payload.to_string().len()
                    + p.context.as_ref().map(|c| c.to_string().len()).unwrap_or(0)
            }).sum::<usize>()
        }).sum();

        hashmap_overhead + per_bucket_overhead + per_point
    }

    /// Recompute count, earliest, and latest from current contents.
    fn recompute_stats(&mut self) {
        self.count = self.points.values().map(|v| v.len()).sum();
        self.earliest = None;
        self.latest = None;

        for pts in self.points.values() {
            for p in pts {
                self.earliest = Some(match self.earliest {
                    Some(e) if e <= p.timestamp => e,
                    _ => p.timestamp,
                });
                self.latest = Some(match self.latest {
                    Some(l) if l >= p.timestamp => l,
                    _ => p.timestamp,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    /// Helper: create a RawDataPoint with given source_id and timestamp.
    fn make_point(source_id: &str, ts: DateTime<Utc>) -> RawDataPoint {
        RawDataPoint {
            timestamp: ts,
            source_id: source_id.to_string(),
            ndp_id: Some("test-device".to_string()),
            context: None,
            raw_payload: json!({"value": 42.0}),
        }
    }

    // ========== TDD CYCLE 1: Constructor ==========

    #[test]
    fn test_new_accumulator_is_empty() {
        let today = Utc::now().date_naive();
        let acc = Accumulator::new(today);

        assert_eq!(acc.count(), 0);
        assert_eq!(acc.source_count(), 0);
        assert!(acc.earliest().is_none());
        assert!(acc.latest().is_none());
        assert_eq!(acc.active_date(), today);
    }

    // ========== TDD CYCLE 2: Add single point ==========

    #[test]
    fn test_add_single_point() {
        let today = Utc::now().date_naive();
        let mut acc = Accumulator::new(today);
        let ts = Utc.with_ymd_and_hms(2026, 2, 8, 10, 0, 0).unwrap();

        acc.add(make_point("air-quality-Mqtt", ts));

        assert_eq!(acc.count(), 1);
        assert_eq!(acc.source_count(), 1);

        let by_source = acc.all_points_by_source();
        assert!(by_source.contains_key("air-quality-Mqtt"));
        assert_eq!(by_source["air-quality-Mqtt"].len(), 1);
        assert_eq!(by_source["air-quality-Mqtt"][0].timestamp, ts);
    }

    // ========== TDD CYCLE 3: Multiple points, same source ==========

    #[test]
    fn test_add_multiple_points_same_source() {
        let today = Utc::now().date_naive();
        let mut acc = Accumulator::new(today);
        let base = Utc.with_ymd_and_hms(2026, 2, 8, 10, 0, 0).unwrap();

        for i in 0..3 {
            let ts = base + chrono::Duration::minutes(i);
            acc.add(make_point("air-quality-Mqtt", ts));
        }

        assert_eq!(acc.count(), 3);
        assert_eq!(acc.source_count(), 1);
        assert_eq!(acc.all_points_by_source()["air-quality-Mqtt"].len(), 3);
    }

    // ========== TDD CYCLE 4: Multiple points, different sources ==========

    #[test]
    fn test_add_multiple_points_different_sources() {
        let today = Utc::now().date_naive();
        let mut acc = Accumulator::new(today);
        let ts = Utc.with_ymd_and_hms(2026, 2, 8, 10, 0, 0).unwrap();

        acc.add(make_point("air-quality-Mqtt", ts));
        acc.add(make_point("air-quality-Mqtt", ts + chrono::Duration::minutes(1)));
        acc.add(make_point("outdoor-weather-Http", ts + chrono::Duration::minutes(2)));

        assert_eq!(acc.count(), 3);
        assert_eq!(acc.source_count(), 2);

        let by_source = acc.all_points_by_source();
        assert_eq!(by_source["air-quality-Mqtt"].len(), 2);
        assert_eq!(by_source["outdoor-weather-Http"].len(), 1);
    }

    // ========== TDD CYCLE 5: Earliest/latest tracking ==========

    #[test]
    fn test_earliest_latest_in_order() {
        let today = Utc::now().date_naive();
        let mut acc = Accumulator::new(today);

        let t1 = Utc.with_ymd_and_hms(2026, 2, 8, 8, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 2, 8, 12, 0, 0).unwrap();
        let t3 = Utc.with_ymd_and_hms(2026, 2, 8, 16, 0, 0).unwrap();

        acc.add(make_point("src-a", t1));
        acc.add(make_point("src-a", t2));
        acc.add(make_point("src-a", t3));

        assert_eq!(acc.earliest(), Some(t1));
        assert_eq!(acc.latest(), Some(t3));
    }

    #[test]
    fn test_earliest_latest_reverse_order() {
        let today = Utc::now().date_naive();
        let mut acc = Accumulator::new(today);

        let t1 = Utc.with_ymd_and_hms(2026, 2, 8, 8, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2026, 2, 8, 12, 0, 0).unwrap();
        let t3 = Utc.with_ymd_and_hms(2026, 2, 8, 16, 0, 0).unwrap();

        acc.add(make_point("src-a", t3));
        acc.add(make_point("src-a", t1));
        acc.add(make_point("src-a", t2));

        assert_eq!(acc.earliest(), Some(t1));
        assert_eq!(acc.latest(), Some(t3));
    }

    // ========== TDD CYCLE 6: Clear ==========

    #[test]
    fn test_clear_resets_everything_except_date() {
        let today = Utc::now().date_naive();
        let mut acc = Accumulator::new(today);
        let ts = Utc.with_ymd_and_hms(2026, 2, 8, 10, 0, 0).unwrap();

        for i in 0..5 {
            acc.add(make_point("src", ts + chrono::Duration::seconds(i)));
        }

        assert_eq!(acc.count(), 5);

        acc.clear();

        assert_eq!(acc.count(), 0);
        assert_eq!(acc.source_count(), 0);
        assert!(acc.earliest().is_none());
        assert!(acc.latest().is_none());
        assert_eq!(acc.active_date(), today, "active_date should be preserved after clear");
    }

    // ========== TDD CYCLE 7: drain_for_date ==========

    #[test]
    fn test_drain_for_date_partitions_by_date() {
        let today = NaiveDate::from_ymd_opt(2026, 2, 8).unwrap();
        let yesterday = NaiveDate::from_ymd_opt(2026, 2, 7).unwrap();
        let mut acc = Accumulator::new(today);

        // 3 points for today
        let ts_today = Utc.with_ymd_and_hms(2026, 2, 8, 10, 0, 0).unwrap();
        for i in 0..3 {
            acc.add(make_point("src-a", ts_today + chrono::Duration::hours(i)));
        }

        // 2 points for yesterday
        let ts_yesterday = Utc.with_ymd_and_hms(2026, 2, 7, 22, 0, 0).unwrap();
        for i in 0..2 {
            acc.add(make_point("src-a", ts_yesterday + chrono::Duration::hours(i)));
        }

        assert_eq!(acc.count(), 5);

        let drained = acc.drain_for_date(yesterday);

        // Drained should have 2 yesterday points
        let drained_count: usize = drained.values().map(|v| v.len()).sum();
        assert_eq!(drained_count, 2);
        for pt in &drained["src-a"] {
            assert_eq!(pt.timestamp.date_naive(), yesterday);
        }

        // Accumulator should still have 3 today points
        assert_eq!(acc.count(), 3);
        for pt in &acc.all_points_by_source()["src-a"] {
            assert_eq!(pt.timestamp.date_naive(), today);
        }
    }

    // ========== TDD CYCLE 8: drain_for_date removes empty buckets ==========

    #[test]
    fn test_drain_for_date_removes_empty_source_buckets() {
        let today = NaiveDate::from_ymd_opt(2026, 2, 8).unwrap();
        let yesterday = NaiveDate::from_ymd_opt(2026, 2, 7).unwrap();
        let mut acc = Accumulator::new(today);

        // Source "A": 2 points yesterday only
        let ts_yesterday = Utc.with_ymd_and_hms(2026, 2, 7, 20, 0, 0).unwrap();
        acc.add(make_point("source-A", ts_yesterday));
        acc.add(make_point("source-A", ts_yesterday + chrono::Duration::hours(1)));

        // Source "B": 2 points today only
        let ts_today = Utc.with_ymd_and_hms(2026, 2, 8, 10, 0, 0).unwrap();
        acc.add(make_point("source-B", ts_today));
        acc.add(make_point("source-B", ts_today + chrono::Duration::hours(1)));

        assert_eq!(acc.source_count(), 2);

        let drained = acc.drain_for_date(yesterday);

        // Source "A" bucket should be fully drained
        assert!(drained.contains_key("source-A"));
        assert_eq!(drained["source-A"].len(), 2);

        // Source "A" should be gone from accumulator
        assert!(!acc.all_points_by_source().contains_key("source-A"));

        // Source "B" still present
        assert_eq!(acc.source_count(), 1);
        assert_eq!(acc.all_points_by_source()["source-B"].len(), 2);
        assert_eq!(acc.count(), 2);
    }

    // ========== TDD CYCLE 9: merge_wal_entries no duplicates ==========

    #[test]
    fn test_merge_wal_entries_no_duplicates() {
        let today = Utc::now().date_naive();
        let mut acc = Accumulator::new(today);
        let ts = Utc.with_ymd_and_hms(2026, 2, 8, 10, 0, 0).unwrap();

        let entries: Vec<RawDataPoint> = (0..3)
            .map(|i| make_point("src-a", ts + chrono::Duration::minutes(i)))
            .collect();

        acc.merge_wal_entries(entries);

        assert_eq!(acc.count(), 3);
        assert_eq!(acc.source_count(), 1);
    }

    // ========== TDD CYCLE 10: merge_wal_entries with duplicates ==========

    #[test]
    fn test_merge_wal_entries_deduplicates() {
        let today = Utc::now().date_naive();
        let mut acc = Accumulator::new(today);
        let t1 = Utc.with_ymd_and_hms(2026, 2, 8, 10, 0, 0).unwrap();

        // Seed with one point
        acc.add(make_point("source-A", t1));
        assert_eq!(acc.count(), 1);

        // Merge entries including a duplicate (same source_id + timestamp)
        let entries = vec![make_point("source-A", t1)]; // duplicate
        acc.merge_wal_entries(entries);

        // Should still be 1 -- duplicate was skipped
        assert_eq!(acc.count(), 1);
        assert_eq!(acc.all_points_by_source()["source-A"].len(), 1);
    }

    // ========== TDD CYCLE 11: merge_wal_entries mixed ==========

    #[test]
    fn test_merge_wal_entries_mixed_duplicate_and_new() {
        let today = Utc::now().date_naive();
        let mut acc = Accumulator::new(today);
        let base = Utc.with_ymd_and_hms(2026, 2, 8, 10, 0, 0).unwrap();

        // Seed with 3 points: t0, t1, t2
        for i in 0..3 {
            acc.add(make_point("src", base + chrono::Duration::minutes(i)));
        }
        assert_eq!(acc.count(), 3);

        // Merge 5 entries: t1 (dup), t2 (dup), t3 (new), t4 (new), t5 (new)
        let merge_entries: Vec<RawDataPoint> = (1..6)
            .map(|i| make_point("src", base + chrono::Duration::minutes(i)))
            .collect();

        acc.merge_wal_entries(merge_entries);

        // 3 original + 3 new (t3, t4, t5) = 6; 2 duplicates (t1, t2) skipped
        assert_eq!(acc.count(), 6);
    }

    // ========== TDD CYCLE 12: memory_estimate_bytes ==========

    #[test]
    fn test_memory_estimate_bytes() {
        let today = Utc::now().date_naive();
        let mut acc = Accumulator::new(today);
        let ts = Utc.with_ymd_and_hms(2026, 2, 8, 10, 0, 0).unwrap();

        for i in 0..100 {
            acc.add(make_point("src", ts + chrono::Duration::seconds(i)));
        }

        let estimate = acc.memory_estimate_bytes();
        assert!(estimate > 0, "Memory estimate should be positive");
        // 100 small points: reasonable range 1KB - 100KB
        assert!(
            estimate > 1000 && estimate < 100_000,
            "Memory estimate {} should be between 1000 and 100000 for 100 points",
            estimate
        );
    }

    #[test]
    fn test_memory_estimate_empty() {
        let today = Utc::now().date_naive();
        let acc = Accumulator::new(today);

        let estimate = acc.memory_estimate_bytes();
        // Empty accumulator should still account for HashMap overhead
        assert!(estimate > 0, "Even empty accumulator has base overhead");
    }

    // ========== TDD CYCLE 13: all_points_by_source returns reference ==========

    #[test]
    fn test_all_points_by_source_does_not_consume() {
        let today = Utc::now().date_naive();
        let mut acc = Accumulator::new(today);
        let ts = Utc.with_ymd_and_hms(2026, 2, 8, 10, 0, 0).unwrap();

        acc.add(make_point("src-a", ts));
        acc.add(make_point("src-b", ts + chrono::Duration::minutes(1)));

        // First access
        let snapshot1 = acc.all_points_by_source();
        assert_eq!(snapshot1.len(), 2);

        // Accumulator is NOT consumed -- can still call methods
        assert_eq!(acc.count(), 2);
        assert_eq!(acc.source_count(), 2);

        // Second access works
        let snapshot2 = acc.all_points_by_source();
        assert_eq!(snapshot2.len(), 2);
    }
}
