/// Feature engineering for time series forecasting
use chrono::{DateTime, Utc, Timelike, Datelike};

/// Extract hour of day (0-23) from timestamp
pub fn hour_of_day(timestamp: &DateTime<Utc>) -> f64 {
    timestamp.hour() as f64
}

/// Extract day of week (0-6, Monday=0)
pub fn day_of_week(timestamp: &DateTime<Utc>) -> f64 {
    timestamp.weekday().num_days_from_monday() as f64
}

/// Check if timestamp is on weekend (Saturday or Sunday)
pub fn is_weekend(timestamp: &DateTime<Utc>) -> f64 {
    let day = timestamp.weekday();
    if day == chrono::Weekday::Sat || day == chrono::Weekday::Sun {
        1.0
    } else {
        0.0
    }
}

/// Create lag feature from time series data
///
/// # Arguments
/// * `data` - Time series values
/// * `lag_steps` - Number of steps to lag (e.g., 60 for 1-hour lag if data is per-minute)
pub fn lag_feature(data: &[f64], lag_steps: usize) -> Vec<f64> {
    let mut lagged = Vec::with_capacity(data.len());

    for i in 0..data.len() {
        if i < lag_steps {
            // Pad with 0.0 for insufficient history
            lagged.push(0.0);
        } else {
            lagged.push(data[i - lag_steps]);
        }
    }

    lagged
}

/// Calculate rolling mean
///
/// # Arguments
/// * `data` - Time series values
/// * `window_size` - Size of rolling window
pub fn rolling_mean(data: &[f64], window_size: usize) -> Vec<f64> {
    let mut rolling = Vec::with_capacity(data.len());

    for i in 0..data.len() {
        if i < window_size - 1 {
            // Not enough data for full window - use NaN
            rolling.push(f64::NAN);
        } else {
            let window = &data[i - window_size + 1..=i];
            let mean = window.iter().sum::<f64>() / window.len() as f64;
            rolling.push(mean);
        }
    }

    rolling
}

/// Calculate rolling standard deviation
///
/// # Arguments
/// * `data` - Time series values
/// * `window_size` - Size of rolling window
pub fn rolling_std(data: &[f64], window_size: usize) -> Vec<f64> {
    let mut rolling = Vec::with_capacity(data.len());

    for i in 0..data.len() {
        if i < window_size - 1 {
            // Not enough data for full window - use NaN
            rolling.push(f64::NAN);
        } else {
            let window = &data[i - window_size + 1..=i];
            let mean = window.iter().sum::<f64>() / window.len() as f64;
            let variance = window.iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>() / window.len() as f64;
            rolling.push(variance.sqrt());
        }
    }

    rolling
}

/// Feature vector for forecasting
#[derive(Debug, Clone)]
pub struct FeatureVector {
    pub timestamp: DateTime<Utc>,
    pub hour: f64,
    pub day_of_week: f64,
    pub is_weekend: f64,
    pub pm25: f64,
    pub co2: f64,
    pub voc_index: f64,
    pub temp_c: f64,
    pub humidity_pct: f64,
    pub lag_1h: f64,
    pub lag_3h: f64,
    pub lag_24h: f64,
    pub rolling_mean_1h: f64,
    pub rolling_std_1h: f64,
}

impl FeatureVector {
    /// Create a new feature vector
    pub fn new(timestamp: DateTime<Utc>) -> Self {
        Self {
            timestamp,
            hour: hour_of_day(&timestamp),
            day_of_week: day_of_week(&timestamp),
            is_weekend: is_weekend(&timestamp),
            pm25: 0.0,
            co2: 0.0,
            voc_index: 0.0,
            temp_c: 0.0,
            humidity_pct: 0.0,
            lag_1h: 0.0,
            lag_3h: 0.0,
            lag_24h: 0.0,
            rolling_mean_1h: 0.0,
            rolling_std_1h: 0.0,
        }
    }

    /// Convert to flat vector for model input
    pub fn to_vec(&self) -> Vec<f64> {
        vec![
            self.hour,
            self.day_of_week,
            self.is_weekend,
            self.pm25,
            self.co2,
            self.voc_index,
            self.temp_c,
            self.humidity_pct,
            self.lag_1h,
            self.lag_3h,
            self.lag_24h,
            self.rolling_mean_1h,
            self.rolling_std_1h,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    fn create_timestamp(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Utc> {
        DateTime::<Utc>::from_naive_utc_and_offset(
            NaiveDateTime::new(
                chrono::NaiveDate::from_ymd_opt(year, month, day).unwrap(),
                chrono::NaiveTime::from_hms_opt(hour, 0, 0).unwrap(),
            ),
            Utc,
        )
    }

    #[test]
    fn test_hour_of_day_feature() {
        let timestamp = create_timestamp(2025, 1, 15, 14);
        let hour = hour_of_day(&timestamp);
        assert_eq!(hour, 14.0, "Hour should be 14");
    }

    #[test]
    fn test_hour_of_day_midnight() {
        let timestamp = create_timestamp(2025, 1, 15, 0);
        let hour = hour_of_day(&timestamp);
        assert_eq!(hour, 0.0, "Midnight should be 0");
    }

    #[test]
    fn test_hour_of_day_range() {
        for h in 0..24 {
            let timestamp = create_timestamp(2025, 1, 15, h);
            let hour = hour_of_day(&timestamp);
            assert!(hour >= 0.0 && hour < 24.0, "Hour should be in range [0, 24)");
        }
    }

    #[test]
    fn test_day_of_week_feature() {
        // January 13, 2025 is a Monday (0)
        let monday = create_timestamp(2025, 1, 13, 12);
        assert_eq!(day_of_week(&monday), 0.0, "Monday should be 0");

        // January 19, 2025 is a Sunday (6)
        let sunday = create_timestamp(2025, 1, 19, 12);
        assert_eq!(day_of_week(&sunday), 6.0, "Sunday should be 6");
    }

    #[test]
    fn test_is_weekend_feature() {
        // January 13, 2025 is a Monday
        let monday = create_timestamp(2025, 1, 13, 12);
        assert_eq!(is_weekend(&monday), 0.0, "Monday should not be weekend");

        // January 18, 2025 is a Saturday
        let saturday = create_timestamp(2025, 1, 18, 12);
        assert_eq!(is_weekend(&saturday), 1.0, "Saturday should be weekend");

        // January 19, 2025 is a Sunday
        let sunday = create_timestamp(2025, 1, 19, 12);
        assert_eq!(is_weekend(&sunday), 1.0, "Sunday should be weekend");
    }

    #[test]
    fn test_lag_1h_feature() {
        // 60 data points = 1 hour of per-minute data
        let data: Vec<f64> = (0..120).map(|i| i as f64).collect();
        let lag_60 = lag_feature(&data, 60);

        assert_eq!(lag_60.len(), data.len(), "Lag should preserve length");

        // First 60 values should be padded (e.g., with 0 or NaN)
        // Values from index 60 onwards should be the original values shifted
        for i in 60..lag_60.len() {
            assert_eq!(lag_60[i], data[i - 60], "Lag value at {} should match data at {}", i, i - 60);
        }
    }

    #[test]
    fn test_lag_3h_feature() {
        // 180 data points = 3 hours of per-minute data
        let data: Vec<f64> = (0..300).map(|i| i as f64 * 2.0).collect();
        let lag_180 = lag_feature(&data, 180);

        assert_eq!(lag_180.len(), data.len());

        for i in 180..lag_180.len() {
            assert_eq!(lag_180[i], data[i - 180]);
        }
    }

    #[test]
    fn test_lag_24h_feature() {
        // 1440 data points = 24 hours of per-minute data
        let data: Vec<f64> = (0..2000).map(|i| (i as f64).sin()).collect();
        let lag_1440 = lag_feature(&data, 1440);

        assert_eq!(lag_1440.len(), data.len());

        for i in 1440..lag_1440.len() {
            assert!((lag_1440[i] - data[i - 1440]).abs() < 1e-10);
        }
    }

    #[test]
    fn test_rolling_mean_1h() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let window_size = 3;
        let rolling = rolling_mean(&data, window_size);

        assert_eq!(rolling.len(), data.len());

        // First few values before full window might be NaN or computed with available data
        // Once window is full, check the rolling mean
        let expected_at_3 = (2.0 + 3.0 + 4.0) / 3.0;
        assert!((rolling[3] - expected_at_3).abs() < 1e-10,
                "Rolling mean at index 3 should be {}", expected_at_3);
    }

    #[test]
    fn test_rolling_mean_exact_window() {
        let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let rolling = rolling_mean(&data, 2);

        // rolling[0] might be NaN or 10.0
        // rolling[1] should be (10+20)/2 = 15
        // rolling[2] should be (20+30)/2 = 25
        assert!((rolling[1] - 15.0).abs() < 1e-10 || rolling[1].is_nan());
        assert!((rolling[2] - 25.0).abs() < 1e-10);
    }

    #[test]
    fn test_rolling_std_1h() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let window_size = 3;
        let rolling = rolling_std(&data, window_size);

        assert_eq!(rolling.len(), data.len());

        // Check that std is non-negative
        for val in rolling.iter() {
            assert!(*val >= 0.0 || val.is_nan(), "Std should be non-negative");
        }
    }

    #[test]
    fn test_rolling_std_constant_window() {
        let data = vec![5.0, 5.0, 5.0, 5.0, 5.0];
        let rolling = rolling_std(&data, 2);

        // Std of constant values should be 0
        for i in 1..rolling.len() {
            assert!(rolling[i].abs() < 1e-10 || rolling[i].is_nan(),
                    "Std of constant window should be 0");
        }
    }

    #[test]
    fn test_multi_pollutant_features() {
        let timestamp = create_timestamp(2025, 1, 15, 14);
        let feature = FeatureVector::new(timestamp);

        // Should have all required fields
        assert_eq!(feature.timestamp, timestamp);
        // Other fields should be initialized (values don't matter for this structural test)
    }

    #[test]
    fn test_feature_vector_to_vec() {
        let timestamp = create_timestamp(2025, 1, 15, 14);
        let mut feature = FeatureVector::new(timestamp);

        // Set some values
        feature.pm25 = 15.0;
        feature.co2 = 400.0;
        feature.hour = 14.0;

        let vec = feature.to_vec();

        // Should contain all features as a flat vector
        assert!(vec.len() > 0, "Feature vector should not be empty");
        assert!(vec.contains(&15.0), "Should contain pm25 value");
        assert!(vec.contains(&400.0), "Should contain co2 value");
        assert!(vec.contains(&14.0), "Should contain hour value");
    }

    #[test]
    fn test_normalization_zscore() {
        // Test that features can be normalized
        let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];

        // Calculate mean and std
        let mean = data.iter().sum::<f64>() / data.len() as f64;
        let variance = data.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / data.len() as f64;
        let std = variance.sqrt();

        // Normalize
        let normalized: Vec<f64> = data.iter()
            .map(|x| (x - mean) / std)
            .collect();

        // Check that normalized data has mean ~0 and std ~1
        let norm_mean = normalized.iter().sum::<f64>() / normalized.len() as f64;
        assert!(norm_mean.abs() < 1e-10, "Normalized mean should be 0");
    }
}
