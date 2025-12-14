/// Standard scaler for normalizing features using Z-score normalization
#[derive(Debug, Clone)]
pub struct StandardScaler {
    pub mean: f64,
    pub std: f64,
}

impl StandardScaler {
    /// Fit the scaler to data (calculate mean and std)
    pub fn fit(data: &[f64]) -> Self {
        if data.is_empty() {
            return Self {
                mean: f64::NAN,
                std: 0.0,
            };
        }

        let mean = data.iter().sum::<f64>() / data.len() as f64;

        let variance = data.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / data.len() as f64;

        let std = variance.sqrt();

        Self { mean, std }
    }

    /// Transform data using fitted parameters
    pub fn transform(&self, data: &[f64]) -> Vec<f64> {
        if self.std == 0.0 {
            // If std is zero, return zeros (data is constant)
            return vec![0.0; data.len()];
        }

        data.iter()
            .map(|x| (x - self.mean) / self.std)
            .collect()
    }

    /// Inverse transform normalized data back to original scale
    pub fn inverse_transform(&self, data: &[f64]) -> Vec<f64> {
        data.iter()
            .map(|x| x * self.std + self.mean)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fit_calculates_correct_mean() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let scaler = StandardScaler::fit(&data);

        assert!((scaler.mean - 3.0).abs() < 1e-10, "Mean should be 3.0");
    }

    #[test]
    fn test_fit_calculates_correct_std() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let scaler = StandardScaler::fit(&data);

        // std = sqrt(sum((x - mean)^2) / n)
        // For [1,2,3,4,5]: mean=3, variance=2, std=sqrt(2)≈1.414
        let expected_std = (2.0_f64).sqrt();
        assert!((scaler.std - expected_std).abs() < 1e-10,
                "Std should be approximately {}", expected_std);
    }

    #[test]
    fn test_transform_normalizes_data() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let scaler = StandardScaler::fit(&data);
        let normalized = scaler.transform(&data);

        // After normalization, mean should be ~0 and std should be ~1
        let norm_mean: f64 = normalized.iter().sum::<f64>() / normalized.len() as f64;
        assert!(norm_mean.abs() < 1e-10, "Normalized mean should be 0");

        let norm_variance: f64 = normalized.iter()
            .map(|x| (x - norm_mean).powi(2))
            .sum::<f64>() / normalized.len() as f64;
        let norm_std = norm_variance.sqrt();
        assert!((norm_std - 1.0).abs() < 1e-10, "Normalized std should be 1");
    }

    #[test]
    fn test_inverse_transform_recovers_original() {
        let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let scaler = StandardScaler::fit(&data);
        let normalized = scaler.transform(&data);
        let recovered = scaler.inverse_transform(&normalized);

        for (original, recovered_val) in data.iter().zip(recovered.iter()) {
            assert!((original - recovered_val).abs() < 1e-10,
                    "Original: {}, Recovered: {}", original, recovered_val);
        }
    }

    #[test]
    fn test_transform_single_value() {
        let data = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let scaler = StandardScaler::fit(&data);
        let normalized = scaler.transform(&[30.0]);

        // 30.0 is the mean, so it should normalize to 0
        assert!(normalized[0].abs() < 1e-10, "Mean value should normalize to 0");
    }

    #[test]
    fn test_transform_preserves_length() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let scaler = StandardScaler::fit(&data);
        let test_data = vec![6.0, 7.0, 8.0];
        let normalized = scaler.transform(&test_data);

        assert_eq!(normalized.len(), test_data.len(),
                   "Transform should preserve length");
    }

    #[test]
    fn test_constant_data_handling() {
        // Edge case: all values are the same
        let data = vec![5.0, 5.0, 5.0, 5.0];
        let scaler = StandardScaler::fit(&data);

        assert_eq!(scaler.mean, 5.0);
        assert_eq!(scaler.std, 0.0, "Std of constant data should be 0");

        // Transform should handle zero std gracefully
        let normalized = scaler.transform(&data);
        assert_eq!(normalized.len(), data.len());
    }

    #[test]
    fn test_empty_data_handling() {
        let data: Vec<f64> = vec![];
        let scaler = StandardScaler::fit(&data);

        assert!(scaler.mean.is_nan() || scaler.mean == 0.0);
    }
}
