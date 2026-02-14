//! Running statistics with exponential decay
//!
//! Provides online mean and standard deviation computation using
//! exponential moving averages. Used by MetricEmbedder for z-score
//! normalization of metric values.

/// Online running statistics with exponential decay.
///
/// Computes exponentially weighted mean and variance, giving more
/// weight to recent observations. Used for z-score normalization
/// in the embedding pipeline.
///
/// # Parameters
///
/// - `alpha`: Decay factor (0.0 to 1.0). Smaller values = slower decay = more history.
///   Default: 0.01 (retains ~99% of previous weight per update).
/// - `warmup_threshold`: Minimum observations before statistics are considered reliable.
///   Default: 168 (one week of hourly data).
#[derive(Debug, Clone)]
pub struct RunningStats {
    mean: f64,
    variance: f64,
    count: usize,
    alpha: f64,
}

impl RunningStats {
    /// Create a new RunningStats with the given decay factor.
    pub fn new(alpha: f64) -> Self {
        Self {
            mean: 0.0,
            variance: 0.0,
            count: 0,
            alpha,
        }
    }

    /// Create a new RunningStats with default parameters (alpha=0.01).
    pub fn default_params() -> Self {
        Self::new(0.01)
    }

    /// Update statistics with a new observation.
    pub fn update(&mut self, value: f64) {
        self.count += 1;

        if self.count == 1 {
            // First observation: set mean directly, variance = 0
            self.mean = value;
            self.variance = 0.0;
        } else {
            // Exponentially weighted update
            let diff = value - self.mean;
            self.mean += self.alpha * diff;
            // Exponentially weighted variance update
            self.variance = (1.0 - self.alpha) * (self.variance + self.alpha * diff * diff);
        }
    }

    /// Get the current mean.
    pub fn mean(&self) -> f64 {
        self.mean
    }

    /// Get the current standard deviation.
    pub fn std_dev(&self) -> f64 {
        self.variance.sqrt()
    }

    /// Get the current variance.
    pub fn variance(&self) -> f64 {
        self.variance
    }

    /// Get the number of observations.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Compute the z-score for a given value.
    ///
    /// Returns 0.0 if standard deviation is effectively zero (< 1e-10)
    /// to avoid division by zero.
    pub fn z_score(&self, value: f64) -> f64 {
        let std = self.std_dev();
        if std < 1e-10 {
            return 0.0;
        }
        (value - self.mean) / std
    }

    /// Check if we have enough observations for reliable statistics.
    pub fn is_warmed_up(&self, warmup_threshold: usize) -> bool {
        self.count >= warmup_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_observation() {
        let mut stats = RunningStats::new(0.01);
        stats.update(5.0);
        assert!((stats.mean() - 5.0).abs() < f64::EPSILON);
        assert!((stats.std_dev() - 0.0).abs() < f64::EPSILON);
        assert_eq!(stats.count(), 1);
    }

    #[test]
    fn test_known_series_mean() {
        // With exponential decay, the mean tracks the series.
        // Feed the sequence [1,2,3,4,5] repeatedly with alpha=0.1
        // to ensure convergence near 3.0.
        let mut stats = RunningStats::new(0.1);
        for _ in 0..20 {
            for v in 1..=5 {
                stats.update(v as f64);
            }
        }
        // After many cycles, mean converges to ~3.0
        assert!(
            (stats.mean() - 3.0).abs() < 0.5,
            "Mean {} should be approximately 3.0",
            stats.mean()
        );
        assert_eq!(stats.count(), 100);
    }

    #[test]
    fn test_z_score_of_mean_is_approximately_zero() {
        let mut stats = RunningStats::new(0.01);
        // Feed enough data for stable stats
        for i in 0..500 {
            stats.update((i % 100) as f64);
        }
        let z = stats.z_score(stats.mean());
        assert!(
            z.abs() < 0.01,
            "Z-score of mean should be approximately 0.0, got {}",
            z
        );
    }

    #[test]
    fn test_convergence_after_constant_values() {
        let mut stats = RunningStats::new(0.01);
        let constant_value = 42.0;
        // Seed with some variety first
        for i in 0..50 {
            stats.update(i as f64);
        }
        // Then feed constant value 500 times (alpha=0.01 needs ~460 updates
        // to move 99% of the way from old mean to new constant)
        for _ in 0..500 {
            stats.update(constant_value);
        }
        assert!(
            (stats.mean() - constant_value).abs() < 1.0,
            "After 500 constant values, mean {} should converge to {}",
            stats.mean(),
            constant_value
        );
    }

    #[test]
    fn test_count_increments() {
        let mut stats = RunningStats::new(0.01);
        assert_eq!(stats.count(), 0);
        stats.update(1.0);
        assert_eq!(stats.count(), 1);
        stats.update(2.0);
        assert_eq!(stats.count(), 2);
        stats.update(3.0);
        assert_eq!(stats.count(), 3);
    }

    #[test]
    fn test_z_score_zero_std_dev() {
        let mut stats = RunningStats::new(0.01);
        stats.update(5.0);
        // Only one observation, std dev = 0
        let z = stats.z_score(10.0);
        assert!(
            z.abs() < f64::EPSILON,
            "Z-score should be 0.0 when std dev is 0, got {}",
            z
        );
    }

    #[test]
    fn test_warmup_check() {
        let mut stats = RunningStats::new(0.01);
        assert!(!stats.is_warmed_up(168));
        for i in 0..168 {
            stats.update(i as f64);
        }
        assert!(stats.is_warmed_up(168));
    }

    #[test]
    fn test_default_params() {
        let stats = RunningStats::default_params();
        assert!((stats.alpha - 0.01).abs() < f64::EPSILON);
        assert_eq!(stats.count(), 0);
    }

    #[test]
    fn test_variance_positive() {
        let mut stats = RunningStats::new(0.1);
        stats.update(1.0);
        stats.update(10.0);
        stats.update(1.0);
        stats.update(10.0);
        assert!(
            stats.variance() > 0.0,
            "Variance should be positive for varying data"
        );
    }
}
