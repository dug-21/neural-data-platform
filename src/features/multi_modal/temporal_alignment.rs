//! Temporal Alignment Engine
//! 
//! This module handles temporal alignment of features from different data modalities
//! that may have different update frequencies and time stamps.

use super::*;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use tracing::{debug, warn};

/// Temporal alignment engine
pub struct TemporalAlignmentEngine {
    alignment_window_seconds: u64,
    interpolation_strategy: InterpolationStrategy,
    alignment_tolerance_ms: i64,
}

/// Interpolation strategies for missing data points
#[derive(Debug, Clone)]
pub enum InterpolationStrategy {
    /// Forward fill (use last known value)
    ForwardFill,
    /// Backward fill (use next known value)
    BackwardFill,
    /// Linear interpolation between known points
    Linear,
    /// Cubic spline interpolation
    Spline,
    /// No interpolation (leave as NaN)
    None,
}

/// Aligned time series point
#[derive(Debug, Clone)]
pub struct AlignedDataPoint {
    pub timestamp: DateTime<Utc>,
    pub features: HashMap<String, f64>,
    pub interpolated_features: Vec<String>,
    pub alignment_quality: f64,
}

impl TemporalAlignmentEngine {
    /// Create new temporal alignment engine
    pub fn new(alignment_window_seconds: u64) -> Self {
        Self {
            alignment_window_seconds,
            interpolation_strategy: InterpolationStrategy::Linear,
            alignment_tolerance_ms: 60000, // 1 minute tolerance
        }
    }
    
    /// Align features from different modalities to common timestamps
    pub async fn align_features(
        &self,
        modality_features: &HashMap<DataModality, HashMap<String, f64>>,
        target_timestamp: DateTime<Utc>,
    ) -> Result<HashMap<DataModality, HashMap<String, f64>>> {
        debug!("Aligning features for timestamp: {}", target_timestamp);
        
        let mut aligned_features = HashMap::new();
        
        for (modality, features) in modality_features {
            let aligned_modality_features = self.align_modality_features(
                *modality,
                features,
                target_timestamp,
            ).await?;
            
            aligned_features.insert(*modality, aligned_modality_features);
        }
        
        Ok(aligned_features)
    }
    
    /// Align time series data to regular intervals
    pub async fn align_time_series(
        &self,
        time_series: &[(DateTime<Utc>, HashMap<DataModality, HashMap<String, f64>>)],
        interval_seconds: u64,
    ) -> Result<Vec<AlignedDataPoint>> {
        if time_series.is_empty() {
            return Ok(Vec::new());
        }
        
        // Determine alignment grid
        let start_time = time_series.first().unwrap().0;
        let end_time = time_series.last().unwrap().0;
        let mut aligned_points = Vec::new();
        
        let interval_duration = Duration::seconds(interval_seconds as i64);
        let mut current_time = self.round_to_interval(start_time, interval_seconds);
        
        while current_time <= end_time {
            let aligned_point = self.create_aligned_point(
                current_time,
                time_series,
                interval_seconds,
            ).await?;
            
            aligned_points.push(aligned_point);
            current_time = current_time + interval_duration;
        }
        
        debug!("Created {} aligned data points", aligned_points.len());
        Ok(aligned_points)
    }
    
    /// Align features for a specific modality
    async fn align_modality_features(
        &self,
        modality: DataModality,
        features: &HashMap<String, f64>,
        target_timestamp: DateTime<Utc>,
    ) -> Result<HashMap<String, f64>> {
        let expected_freq = modality.expected_frequency_seconds();
        let tolerance = Duration::milliseconds(self.alignment_tolerance_ms);
        
        // For now, return features as-is since we don't have historical context
        // In a full implementation, this would interpolate based on historical data
        let mut aligned_features = features.clone();
        
        // Add alignment metadata
        aligned_features.insert(
            format!("{}_alignment_quality", modality.as_str()),
            self.calculate_alignment_quality(modality, target_timestamp, expected_freq),
        );
        
        Ok(aligned_features)
    }
    
    /// Create aligned data point from time series
    async fn create_aligned_point(
        &self,
        target_time: DateTime<Utc>,
        time_series: &[(DateTime<Utc>, HashMap<DataModality, HashMap<String, f64>>)],
        _interval_seconds: u64,
    ) -> Result<AlignedDataPoint> {
        let mut features = HashMap::new();
        let mut interpolated_features = Vec::new();
        let mut quality_scores = Vec::new();
        
        // Find closest data points for interpolation
        let (before_idx, after_idx) = self.find_surrounding_points(target_time, time_series);
        
        if let Some(before_idx) = before_idx {
            let before_data = &time_series[before_idx];
            
            // If we have exact match or very close match, use it directly
            let time_diff = (target_time - before_data.0).num_seconds().abs();
            if time_diff <= 60 {  // Within 1 minute
                for (modality, modality_features) in &before_data.1 {
                    for (feature_name, value) in modality_features {
                        let full_name = format!("{}_{}", modality.as_str(), feature_name);
                        features.insert(full_name, *value);
                    }
                }
                quality_scores.push(1.0 - (time_diff as f64 / 3600.0)); // Quality decreases with time
            } else if let Some(after_idx) = after_idx {
                // Interpolate between before and after points
                let after_data = &time_series[after_idx];
                let interpolated = self.interpolate_between_points(
                    before_data,
                    after_data,
                    target_time,
                ).await?;
                
                features.extend(interpolated.0);
                interpolated_features.extend(interpolated.1);
                quality_scores.push(interpolated.2);
            } else {
                // Use forward fill
                let before_data = &time_series[before_idx];
                for (modality, modality_features) in &before_data.1 {
                    for (feature_name, value) in modality_features {
                        let full_name = format!("{}_{}", modality.as_str(), feature_name);
                        features.insert(full_name.clone(), *value);
                        interpolated_features.push(full_name);
                    }
                }
                quality_scores.push(0.5); // Lower quality for forward fill
            }
        }
        
        let overall_quality = if quality_scores.is_empty() {
            0.0
        } else {
            quality_scores.iter().sum::<f64>() / quality_scores.len() as f64
        };
        
        Ok(AlignedDataPoint {
            timestamp: target_time,
            features,
            interpolated_features,
            alignment_quality: overall_quality,
        })
    }
    
    /// Find data points surrounding target time
    fn find_surrounding_points(
        &self,
        target_time: DateTime<Utc>,
        time_series: &[(DateTime<Utc>, HashMap<DataModality, HashMap<String, f64>>)],
    ) -> (Option<usize>, Option<usize>) {
        let mut before_idx = None;
        let mut after_idx = None;
        
        for (i, (timestamp, _)) in time_series.iter().enumerate() {
            if *timestamp <= target_time {
                before_idx = Some(i);
            } else if after_idx.is_none() {
                after_idx = Some(i);
                break;
            }
        }
        
        (before_idx, after_idx)
    }
    
    /// Interpolate between two data points
    async fn interpolate_between_points(
        &self,
        before: &(DateTime<Utc>, HashMap<DataModality, HashMap<String, f64>>),
        after: &(DateTime<Utc>, HashMap<DataModality, HashMap<String, f64>>),
        target_time: DateTime<Utc>,
    ) -> Result<(HashMap<String, f64>, Vec<String>, f64)> {
        let mut interpolated_features = HashMap::new();
        let mut interpolated_names = Vec::new();
        
        let total_duration = (after.0 - before.0).num_seconds() as f64;
        let target_duration = (target_time - before.0).num_seconds() as f64;
        let interpolation_ratio = target_duration / total_duration;
        
        // Interpolate features that exist in both time points
        for (modality, before_features) in &before.1 {
            if let Some(after_features) = after.1.get(modality) {
                for (feature_name, before_value) in before_features {
                    if let Some(after_value) = after_features.get(feature_name) {
                        let interpolated_value = match self.interpolation_strategy {
                            InterpolationStrategy::Linear => {
                                before_value + (after_value - before_value) * interpolation_ratio
                            }
                            InterpolationStrategy::ForwardFill => *before_value,
                            InterpolationStrategy::BackwardFill => *after_value,
                            InterpolationStrategy::Spline => {
                                // Simplified cubic interpolation (would need more points for true spline)
                                self.cubic_interpolation(*before_value, *after_value, interpolation_ratio)
                            }
                            InterpolationStrategy::None => f64::NAN,
                        };
                        
                        let full_name = format!("{}_{}", modality.as_str(), feature_name);
                        interpolated_features.insert(full_name.clone(), interpolated_value);
                        interpolated_names.push(full_name);
                    }
                }
            }
        }
        
        // Quality score based on interpolation distance and time gap
        let time_gap_hours = total_duration / 3600.0;
        let quality = (1.0 - interpolation_ratio.abs()).max(0.1) * (1.0 / (1.0 + time_gap_hours));
        
        Ok((interpolated_features, interpolated_names, quality))
    }
    
    /// Simple cubic interpolation
    fn cubic_interpolation(&self, y0: f64, y1: f64, t: f64) -> f64 {
        // Simplified cubic interpolation with zero derivatives at endpoints
        let t2 = t * t;
        let t3 = t2 * t;
        y0 * (2.0 * t3 - 3.0 * t2 + 1.0) + y1 * (-2.0 * t3 + 3.0 * t2)
    }
    
    /// Round timestamp to interval boundary
    fn round_to_interval(&self, timestamp: DateTime<Utc>, interval_seconds: u64) -> DateTime<Utc> {
        let seconds_since_epoch = timestamp.timestamp();
        let rounded_seconds = (seconds_since_epoch / interval_seconds as i64) * interval_seconds as i64;
        DateTime::from_timestamp(rounded_seconds, 0).unwrap_or(timestamp)
    }
    
    /// Calculate alignment quality for a modality
    fn calculate_alignment_quality(
        &self,
        modality: DataModality,
        target_timestamp: DateTime<Utc>,
        expected_frequency_seconds: u64,
    ) -> f64 {
        // In a real implementation, this would check how close the data timestamp
        // is to the expected update frequency for this modality
        let _ = (modality, target_timestamp, expected_frequency_seconds);
        0.85 // Placeholder quality score
    }
    
    /// Get alignment statistics
    pub async fn get_alignment_statistics(
        &self,
        aligned_data: &[AlignedDataPoint],
    ) -> AlignmentStatistics {
        if aligned_data.is_empty() {
            return AlignmentStatistics::default();
        }
        
        let quality_scores: Vec<f64> = aligned_data.iter()
            .map(|point| point.alignment_quality)
            .collect();
        
        let interpolated_counts: Vec<usize> = aligned_data.iter()
            .map(|point| point.interpolated_features.len())
            .collect();
        
        let avg_quality = quality_scores.iter().sum::<f64>() / quality_scores.len() as f64;
        let min_quality = quality_scores.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_quality = quality_scores.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        
        let avg_interpolated = interpolated_counts.iter().sum::<usize>() as f64 / interpolated_counts.len() as f64;
        let total_interpolated = interpolated_counts.iter().sum::<usize>();
        
        AlignmentStatistics {
            total_points: aligned_data.len(),
            average_quality: avg_quality,
            min_quality,
            max_quality,
            total_interpolated_features: total_interpolated,
            average_interpolated_per_point: avg_interpolated,
            alignment_success_rate: quality_scores.iter().filter(|&&q| q > 0.5).count() as f64 / quality_scores.len() as f64,
        }
    }
}

/// Alignment quality statistics
#[derive(Debug, Clone, Default)]
pub struct AlignmentStatistics {
    pub total_points: usize,
    pub average_quality: f64,
    pub min_quality: f64,
    pub max_quality: f64,
    pub total_interpolated_features: usize,
    pub average_interpolated_per_point: f64,
    pub alignment_success_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_temporal_alignment_engine_creation() {
        let engine = TemporalAlignmentEngine::new(300);
        assert_eq!(engine.alignment_window_seconds, 300);
    }

    #[tokio::test]
    async fn test_align_features() {
        let engine = TemporalAlignmentEngine::new(300);
        let mut modality_features = HashMap::new();
        
        let mut price_features = HashMap::new();
        price_features.insert("close".to_string(), 150.0);
        modality_features.insert(DataModality::Price, price_features);
        
        let target_time = Utc::now();
        let result = engine.align_features(&modality_features, target_time).await;
        
        assert!(result.is_ok());
        let aligned = result.unwrap();
        assert!(aligned.contains_key(&DataModality::Price));
    }

    #[test]
    fn test_cubic_interpolation() {
        let engine = TemporalAlignmentEngine::new(300);
        
        // Test interpolation at midpoint
        let result = engine.cubic_interpolation(0.0, 10.0, 0.5);
        assert!((result - 5.0).abs() < 0.1);
        
        // Test interpolation at endpoints
        assert!((engine.cubic_interpolation(0.0, 10.0, 0.0) - 0.0).abs() < 0.001);
        assert!((engine.cubic_interpolation(0.0, 10.0, 1.0) - 10.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_alignment_statistics() {
        let engine = TemporalAlignmentEngine::new(300);
        
        let aligned_data = vec![
            AlignedDataPoint {
                timestamp: Utc::now(),
                features: HashMap::new(),
                interpolated_features: vec!["feature1".to_string()],
                alignment_quality: 0.8,
            },
            AlignedDataPoint {
                timestamp: Utc::now(),
                features: HashMap::new(),
                interpolated_features: vec!["feature2".to_string(), "feature3".to_string()],
                alignment_quality: 0.9,
            },
        ];
        
        let stats = engine.get_alignment_statistics(&aligned_data).await;
        assert_eq!(stats.total_points, 2);
        assert!((stats.average_quality - 0.85).abs() < 0.01);
        assert_eq!(stats.total_interpolated_features, 3);
    }
}