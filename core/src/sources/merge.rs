//! Dual-source merge logic for deduplication and enrichment
//!
//! Merges readings from MQTT (real-time) and HTTP (extended fields) sources:
//! - Deduplicates by timestamp and metric
//! - Prioritizes MQTT for real-time data
//! - Uses HTTP for extended fields not available in MQTT

use chrono::{DateTime, Duration, Utc};
use std::collections::{HashMap, HashSet};
use tracing::{debug, trace};

use crate::traits::TimeSeriesPoint;

/// Configuration for merge strategy
#[derive(Debug, Clone)]
pub struct MergeConfig {
    /// Time window for considering readings as duplicates
    pub dedup_window: Duration,
    /// Metrics available only in HTTP (won't deduplicate)
    pub http_only_metrics: HashSet<String>,
}

impl Default for MergeConfig {
    fn default() -> Self {
        let mut http_only_metrics = HashSet::new();
        http_only_metrics.insert("pm10".to_string());
        http_only_metrics.insert("pm01".to_string());
        http_only_metrics.insert("tvoc".to_string());
        http_only_metrics.insert("nox_index".to_string());

        Self {
            dedup_window: Duration::seconds(5),
            http_only_metrics,
        }
    }
}

/// Merger for combining MQTT and HTTP readings
pub struct ReadingMerger {
    config: MergeConfig,
    /// Cache of recent MQTT readings for deduplication
    mqtt_cache: HashMap<String, HashMap<String, DateTime<Utc>>>,
}

impl ReadingMerger {
    /// Create a new reading merger
    pub fn new(config: MergeConfig) -> Self {
        Self {
            config,
            mqtt_cache: HashMap::new(),
        }
    }

    /// Create a cache key for deduplication
    fn cache_key(point: &TimeSeriesPoint) -> String {
        let metric = point.tags.get("metric").map(|s| s.as_str()).unwrap_or("unknown");
        format!("{}:{}", point.location_id, metric)
    }

    /// Check if a reading should be deduplicated
    fn is_duplicate(&self, point: &TimeSeriesPoint) -> bool {
        let key = Self::cache_key(point);
        let metric = point.tags.get("metric").map(|s| s.as_str()).unwrap_or("unknown");

        if let Some(metrics) = self.mqtt_cache.get(&point.location_id) {
            if let Some(last_seen) = metrics.get(metric) {
                // Compare timestamps of the actual readings
                let time_diff = if point.timestamp >= *last_seen {
                    point.timestamp - *last_seen
                } else {
                    *last_seen - point.timestamp
                };
                if time_diff < self.config.dedup_window {
                    trace!(
                        "Duplicate detected for {} - time diff: {}s",
                        key,
                        time_diff.num_seconds()
                    );
                    return true;
                }
            }
        }

        false
    }

    /// Update MQTT cache with new reading
    fn update_mqtt_cache(&mut self, point: &TimeSeriesPoint) {
        let metric = point.tags.get("metric").map(|s| s.to_string()).unwrap_or_else(|| "unknown".to_string());
        self.mqtt_cache
            .entry(point.location_id.clone())
            .or_insert_with(HashMap::new)
            .insert(metric, point.timestamp);
    }

    /// Clean old entries from cache
    fn clean_cache(&mut self, cutoff: DateTime<Utc>) {
        for metrics in self.mqtt_cache.values_mut() {
            metrics.retain(|_, timestamp| *timestamp > cutoff);
        }
        self.mqtt_cache.retain(|_, metrics| !metrics.is_empty());
    }

    /// Merge MQTT and HTTP readings
    ///
    /// Strategy:
    /// - MQTT readings are always included (real-time priority)
    /// - HTTP readings are included if:
    ///   - They are HTTP-only metrics (extended fields)
    ///   - They are not duplicates of recent MQTT readings
    pub fn merge(
        &mut self,
        mqtt_points: Vec<TimeSeriesPoint>,
        http_points: Vec<TimeSeriesPoint>,
    ) -> Vec<TimeSeriesPoint> {
        let now = Utc::now();
        let mut result = Vec::new();

        // Clean old cache entries
        let cutoff = now - self.config.dedup_window * 2;
        self.clean_cache(cutoff);

        // Add all MQTT points (they have priority)
        for point in mqtt_points {
            self.update_mqtt_cache(&point);
            result.push(point);
        }

        // Add HTTP points if they're not duplicates
        for point in http_points {
            let metric = point.tags.get("metric").map(|s| s.as_str()).unwrap_or("unknown");

            // HTTP-only metrics always get through
            if self.config.http_only_metrics.contains(metric) {
                debug!("Including HTTP-only metric: {}", metric);
                result.push(point);
                continue;
            }

            // Check for duplicates with MQTT
            if !self.is_duplicate(&point) {
                debug!(
                    "Including non-duplicate HTTP point: {} - {}",
                    point.location_id, metric
                );
                result.push(point);
            }
        }

        result
    }

    /// Merge single optional readings
    pub fn merge_optional(
        &mut self,
        mqtt_point: Option<TimeSeriesPoint>,
        http_point: Option<TimeSeriesPoint>,
    ) -> Option<TimeSeriesPoint> {
        match (mqtt_point, http_point) {
            (Some(mqtt), Some(http)) => {
                let merged = self.merge(vec![mqtt.clone()], vec![http]);
                // If both exist, prefer MQTT
                merged.into_iter().next().or(Some(mqtt))
            }
            (Some(mqtt), None) => Some(mqtt),
            (None, Some(http)) => Some(http),
            (None, None) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_point(
        location_id: &str,
        metric: &str,
        value: f64,
        timestamp: DateTime<Utc>,
    ) -> TimeSeriesPoint {
        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), metric.to_string());

        TimeSeriesPoint {
            timestamp,
            location_id: location_id.to_string(),
            value,
            tags,
        }
    }

    #[test]
    fn test_merger_creation() {
        let config = MergeConfig::default();
        let merger = ReadingMerger::new(config);

        assert_eq!(merger.mqtt_cache.len(), 0);
    }

    #[test]
    fn test_cache_key_generation() {
        let point = create_test_point("ABC123", "pm02", 12.5, Utc::now());
        let key = ReadingMerger::cache_key(&point);

        assert_eq!(key, "ABC123:pm02");
    }

    #[test]
    fn test_mqtt_only_points() {
        let mut merger = ReadingMerger::new(MergeConfig::default());
        let now = Utc::now();

        let mqtt_points = vec![
            create_test_point("ABC123", "pm02", 12.5, now),
            create_test_point("ABC123", "co2", 450.0, now),
        ];

        let result = merger.merge(mqtt_points.clone(), vec![]);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].value, 12.5);
        assert_eq!(result[1].value, 450.0);
    }

    #[test]
    fn test_http_only_points() {
        let mut merger = ReadingMerger::new(MergeConfig::default());
        let now = Utc::now();

        let http_points = vec![
            create_test_point("ABC123", "pm10", 15.2, now),
            create_test_point("ABC123", "tvoc", 120.0, now),
        ];

        let result = merger.merge(vec![], http_points.clone());

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].tags.get("metric").unwrap(), "pm10");
        assert_eq!(result[1].tags.get("metric").unwrap(), "tvoc");
    }

    #[test]
    fn test_deduplication_same_metric() {
        let mut merger = ReadingMerger::new(MergeConfig::default());
        let now = Utc::now();

        let mqtt_points = vec![create_test_point("ABC123", "pm02", 12.5, now)];

        // HTTP point with same metric, same timestamp (should be deduplicated)
        let http_points = vec![create_test_point("ABC123", "pm02", 12.7, now)];

        let result = merger.merge(mqtt_points, http_points);

        // Should only have the MQTT point
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, 12.5); // MQTT value, not HTTP
    }

    #[test]
    fn test_deduplication_within_window() {
        let mut merger = ReadingMerger::new(MergeConfig::default());
        let now = Utc::now();

        let mqtt_points = vec![create_test_point("ABC123", "pm02", 12.5, now)];

        // HTTP point 2 seconds later (within 5s dedup window)
        let http_points = vec![create_test_point(
            "ABC123",
            "pm02",
            12.7,
            now + Duration::seconds(2),
        )];

        let result = merger.merge(mqtt_points, http_points);

        // Should only have the MQTT point
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, 12.5);
    }

    #[test]
    fn test_no_deduplication_outside_window() {
        let mut merger = ReadingMerger::new(MergeConfig::default());
        let now = Utc::now();

        let mqtt_points = vec![create_test_point("ABC123", "pm02", 12.5, now)];

        // HTTP point 10 seconds later (outside 5s dedup window)
        let http_points = vec![create_test_point(
            "ABC123",
            "pm02",
            12.7,
            now + Duration::seconds(10),
        )];

        let result = merger.merge(mqtt_points, http_points);

        // Should have both points
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_http_only_metrics_not_deduplicated() {
        let mut merger = ReadingMerger::new(MergeConfig::default());
        let now = Utc::now();

        let mqtt_points = vec![create_test_point("ABC123", "pm02", 12.5, now)];

        // HTTP-only metrics should always pass through
        let http_points = vec![
            create_test_point("ABC123", "pm10", 15.2, now),
            create_test_point("ABC123", "tvoc", 120.0, now),
            create_test_point("ABC123", "nox_index", 1.5, now),
        ];

        let result = merger.merge(mqtt_points, http_points);

        assert_eq!(result.len(), 4); // 1 MQTT + 3 HTTP-only
        assert!(result.iter().any(|p| p.tags.get("metric") == Some(&"pm02".to_string())));
        assert!(result.iter().any(|p| p.tags.get("metric") == Some(&"pm10".to_string())));
        assert!(result.iter().any(|p| p.tags.get("metric") == Some(&"tvoc".to_string())));
        assert!(result.iter().any(|p| p.tags.get("metric") == Some(&"nox_index".to_string())));
    }

    #[test]
    fn test_multiple_sensors() {
        let mut merger = ReadingMerger::new(MergeConfig::default());
        let now = Utc::now();

        let mqtt_points = vec![
            create_test_point("ABC123", "pm02", 12.5, now),
            create_test_point("DEF456", "pm02", 10.0, now),
        ];

        let http_points = vec![
            create_test_point("ABC123", "pm02", 12.7, now), // Duplicate
            create_test_point("DEF456", "pm10", 11.0, now), // HTTP-only
        ];

        let result = merger.merge(mqtt_points, http_points);

        assert_eq!(result.len(), 3); // 2 MQTT + 1 HTTP-only
        assert!(result.iter().any(|p| p.location_id == "ABC123" && p.tags.get("metric") == Some(&"pm02".to_string())));
        assert!(result.iter().any(|p| p.location_id == "DEF456" && p.tags.get("metric") == Some(&"pm02".to_string())));
        assert!(result.iter().any(|p| p.location_id == "DEF456" && p.tags.get("metric") == Some(&"pm10".to_string())));
    }

    #[test]
    fn test_cache_cleanup() {
        let mut merger = ReadingMerger::new(MergeConfig::default());
        let old_time = Utc::now() - Duration::seconds(20);
        let now = Utc::now();

        // Add old MQTT points
        let mqtt_points = vec![create_test_point("ABC123", "pm02", 12.5, old_time)];
        merger.merge(mqtt_points, vec![]);

        assert_eq!(merger.mqtt_cache.len(), 1);

        // Process new points (should trigger cleanup)
        let new_mqtt_points = vec![create_test_point("ABC123", "co2", 450.0, now)];
        merger.merge(new_mqtt_points, vec![]);

        // Old entry should be cleaned up
        let cache_entry = merger.mqtt_cache.get("ABC123").unwrap();
        assert!(!cache_entry.contains_key("pm02"));
        assert!(cache_entry.contains_key("co2"));
    }

    #[test]
    fn test_merge_optional_both_present() {
        let mut merger = ReadingMerger::new(MergeConfig::default());
        let now = Utc::now();

        let mqtt_point = Some(create_test_point("ABC123", "pm02", 12.5, now));
        let http_point = Some(create_test_point("ABC123", "pm02", 12.7, now));

        let result = merger.merge_optional(mqtt_point.clone(), http_point);

        assert!(result.is_some());
        let point = result.unwrap();
        assert_eq!(point.value, 12.5); // Prefers MQTT
    }

    #[test]
    fn test_merge_optional_mqtt_only() {
        let mut merger = ReadingMerger::new(MergeConfig::default());
        let now = Utc::now();

        let mqtt_point = Some(create_test_point("ABC123", "pm02", 12.5, now));

        let result = merger.merge_optional(mqtt_point.clone(), None);

        assert!(result.is_some());
        assert_eq!(result.unwrap().value, 12.5);
    }

    #[test]
    fn test_merge_optional_http_only() {
        let mut merger = ReadingMerger::new(MergeConfig::default());
        let now = Utc::now();

        let http_point = Some(create_test_point("ABC123", "pm10", 15.2, now));

        let result = merger.merge_optional(None, http_point.clone());

        assert!(result.is_some());
        assert_eq!(result.unwrap().value, 15.2);
    }

    #[test]
    fn test_merge_optional_none() {
        let mut merger = ReadingMerger::new(MergeConfig::default());

        let result = merger.merge_optional(None, None);

        assert!(result.is_none());
    }

    #[test]
    fn test_default_http_only_metrics() {
        let config = MergeConfig::default();

        assert!(config.http_only_metrics.contains("pm10"));
        assert!(config.http_only_metrics.contains("pm01"));
        assert!(config.http_only_metrics.contains("tvoc"));
        assert!(config.http_only_metrics.contains("nox_index"));
    }

    #[test]
    fn test_custom_dedup_window() {
        let config = MergeConfig {
            dedup_window: Duration::seconds(10),
            http_only_metrics: HashSet::new(),
        };

        assert_eq!(config.dedup_window, Duration::seconds(10));
    }
}
