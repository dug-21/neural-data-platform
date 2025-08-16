// Rust-native Feature Engineering Pipeline
// High-performance feature extraction and transformation for time series

pub mod statistical;
pub mod fourier;
pub mod wavelets;
pub mod technical;
pub mod pipeline;

use std::collections::VecDeque;
use serde::{Serialize, Deserialize};
use rayon::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    pub window_size: usize,
    pub statistical_features: bool,
    pub fourier_features: bool,
    pub wavelet_features: bool,
    pub technical_features: bool,
    pub normalize: bool,
    pub standardize: bool,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        FeatureConfig {
            window_size: 50,
            statistical_features: true,
            fourier_features: true,
            wavelet_features: false,
            technical_features: true,
            normalize: true,
            standardize: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FeatureVector {
    pub features: Vec<f32>,
    pub feature_names: Vec<String>,
    pub timestamp: i64,
}

impl FeatureVector {
    pub fn new() -> Self {
        FeatureVector {
            features: Vec::new(),
            feature_names: Vec::new(),
            timestamp: 0,
        }
    }
    
    pub fn add_feature(&mut self, name: String, value: f32) {
        self.features.push(value);
        self.feature_names.push(name);
    }
    
    pub fn extend(&mut self, other: &FeatureVector) {
        self.features.extend_from_slice(&other.features);
        self.feature_names.extend_from_slice(&other.feature_names);
    }
    
    pub fn len(&self) -> usize {
        self.features.len()
    }
    
    pub fn normalize(&mut self) {
        if let (Some(&min_val), Some(&max_val)) = (
            self.features.iter().min_by(|a, b| a.partial_cmp(b).unwrap()),
            self.features.iter().max_by(|a, b| a.partial_cmp(b).unwrap())
        ) {
            let range = max_val - min_val;
            if range > 0.0 {
                for feature in &mut self.features {
                    *feature = (*feature - min_val) / range;
                }
            }
        }
    }
    
    pub fn standardize(&mut self) {
        let mean = self.features.iter().sum::<f32>() / self.features.len() as f32;
        let variance = self.features.iter()
            .map(|x| (*x - mean).powi(2))
            .sum::<f32>() / self.features.len() as f32;
        let std_dev = variance.sqrt();
        
        if std_dev > 0.0 {
            for feature in &mut self.features {
                *feature = (*feature - mean) / std_dev;
            }
        }
    }
}

pub trait FeatureExtractor: Send + Sync {
    fn extract(&self, data: &[f32]) -> FeatureVector;
    fn get_feature_count(&self) -> usize;
    fn get_feature_names(&self) -> Vec<String>;
}

#[derive(Debug)]
pub struct FeaturePipeline {
    extractors: Vec<Box<dyn FeatureExtractor>>,
    config: FeatureConfig,
    normalization_stats: Option<(Vec<f32>, Vec<f32>)>, // (means, stds)
}

impl FeaturePipeline {
    pub fn new(config: FeatureConfig) -> Self {
        let mut pipeline = FeaturePipeline {
            extractors: Vec::new(),
            config,
            normalization_stats: None,
        };
        
        pipeline.setup_extractors();
        pipeline
    }
    
    fn setup_extractors(&mut self) {
        if self.config.statistical_features {
            self.extractors.push(Box::new(
                statistical::StatisticalExtractor::new(self.config.window_size)
            ));
        }
        
        if self.config.fourier_features {
            self.extractors.push(Box::new(
                fourier::FourierExtractor::new(self.config.window_size)
            ));
        }
        
        if self.config.wavelet_features {
            self.extractors.push(Box::new(
                wavelets::WaveletExtractor::new(self.config.window_size)
            ));
        }
        
        if self.config.technical_features {
            self.extractors.push(Box::new(
                technical::TechnicalExtractor::new(self.config.window_size)
            ));
        }
    }
    
    pub fn extract_features(&self, data: &[f32]) -> FeatureVector {
        let mut combined_features = FeatureVector::new();
        
        // Extract features in parallel
        let feature_vectors: Vec<FeatureVector> = self.extractors.par_iter()
            .map(|extractor| extractor.extract(data))
            .collect();
        
        // Combine all features
        for fv in feature_vectors {
            combined_features.extend(&fv);
        }
        
        // Apply normalization if configured
        let mut features = combined_features;
        if self.config.normalize {
            features.normalize();
        }
        
        if self.config.standardize {
            if let Some((means, stds)) = &self.normalization_stats {
                self.apply_standardization(&mut features, means, stds);
            } else {
                features.standardize();
            }
        }
        
        features
    }
    
    pub fn extract_windowed_features(&self, data: &[f32]) -> Vec<FeatureVector> {
        if data.len() < self.config.window_size {
            return Vec::new();
        }
        
        let mut features = Vec::new();
        
        for i in self.config.window_size..=data.len() {
            let window = &data[i - self.config.window_size..i];
            let mut feature_vector = self.extract_features(window);
            feature_vector.timestamp = i as i64;
            features.push(feature_vector);
        }
        
        features
    }
    
    pub fn fit_normalization_stats(&mut self, data: &[Vec<f32>]) {
        if !self.config.standardize {
            return;
        }
        
        let feature_count = self.get_feature_count();
        let mut sums = vec![0.0; feature_count];
        let mut sum_squares = vec![0.0; feature_count];
        let mut count = 0;
        
        for sample in data {
            let features = self.extract_features(sample);
            for (i, &value) in features.features.iter().enumerate() {
                sums[i] += value;
                sum_squares[i] += value * value;
            }
            count += 1;
        }
        
        if count > 0 {
            let means: Vec<f32> = sums.iter().map(|&s| s / count as f32).collect();
            let variances: Vec<f32> = sum_squares.iter().zip(&means)
                .map(|(&ss, &mean)| (ss / count as f32) - (mean * mean))
                .collect();
            let stds: Vec<f32> = variances.iter().map(|&v| v.sqrt().max(1e-8)).collect();
            
            self.normalization_stats = Some((means, stds));
        }
    }
    
    fn apply_standardization(&self, features: &mut FeatureVector, means: &[f32], stds: &[f32]) {
        for (i, feature) in features.features.iter_mut().enumerate() {
            if i < means.len() && i < stds.len() {
                *feature = (*feature - means[i]) / stds[i];
            }
        }
    }
    
    pub fn get_feature_count(&self) -> usize {
        self.extractors.iter().map(|e| e.get_feature_count()).sum()
    }
    
    pub fn get_feature_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for extractor in &self.extractors {
            names.extend(extractor.get_feature_names());
        }
        names
    }
}

// Streaming feature extraction for real-time processing
#[derive(Debug)]
pub struct StreamingFeaturePipeline {
    pipeline: FeaturePipeline,
    buffer: VecDeque<f32>,
    window_size: usize,
}

impl StreamingFeaturePipeline {
    pub fn new(config: FeatureConfig) -> Self {
        let window_size = config.window_size;
        StreamingFeaturePipeline {
            pipeline: FeaturePipeline::new(config),
            buffer: VecDeque::with_capacity(window_size),
            window_size,
        }
    }
    
    pub fn update(&mut self, value: f32) -> Option<FeatureVector> {
        self.buffer.push_back(value);
        
        if self.buffer.len() > self.window_size {
            self.buffer.pop_front();
        }
        
        if self.buffer.len() == self.window_size {
            let window_data: Vec<f32> = self.buffer.iter().cloned().collect();
            Some(self.pipeline.extract_features(&window_data))
        } else {
            None
        }
    }
    
    pub fn get_current_features(&self) -> Option<FeatureVector> {
        if self.buffer.len() == self.window_size {
            let window_data: Vec<f32> = self.buffer.iter().cloned().collect();
            Some(self.pipeline.extract_features(&window_data))
        } else {
            None
        }
    }
}

// Feature importance analysis
#[derive(Debug, Clone)]
pub struct FeatureImportance {
    pub feature_names: Vec<String>,
    pub importance_scores: Vec<f32>,
    pub correlation_matrix: Vec<Vec<f32>>,
}

impl FeatureImportance {
    pub fn analyze(features: &[FeatureVector], targets: &[f32]) -> Self {
        let feature_count = features[0].features.len();
        let feature_names = features[0].feature_names.clone();
        
        // Calculate correlation with target
        let mut importance_scores = Vec::new();
        let mut correlation_matrix = vec![vec![0.0; feature_count]; feature_count];
        
        for i in 0..feature_count {
            let feature_values: Vec<f32> = features.iter()
                .map(|fv| fv.features[i])
                .collect();
            
            let correlation = Self::pearson_correlation(&feature_values, targets);
            importance_scores.push(correlation.abs());
            
            // Feature-to-feature correlations
            for j in 0..feature_count {
                let other_values: Vec<f32> = features.iter()
                    .map(|fv| fv.features[j])
                    .collect();
                
                correlation_matrix[i][j] = Self::pearson_correlation(&feature_values, &other_values);
            }
        }
        
        FeatureImportance {
            feature_names,
            importance_scores,
            correlation_matrix,
        }
    }
    
    fn pearson_correlation(x: &[f32], y: &[f32]) -> f32 {
        if x.len() != y.len() || x.is_empty() {
            return 0.0;
        }
        
        let n = x.len() as f32;
        let sum_x: f32 = x.iter().sum();
        let sum_y: f32 = y.iter().sum();
        let sum_x_sq: f32 = x.iter().map(|v| v * v).sum();
        let sum_y_sq: f32 = y.iter().map(|v| v * v).sum();
        let sum_xy: f32 = x.iter().zip(y).map(|(a, b)| a * b).sum();
        
        let numerator = n * sum_xy - sum_x * sum_y;
        let denominator = ((n * sum_x_sq - sum_x * sum_x) * (n * sum_y_sq - sum_y * sum_y)).sqrt();
        
        if denominator == 0.0 {
            0.0
        } else {
            numerator / denominator
        }
    }
    
    pub fn get_top_features(&self, k: usize) -> Vec<(String, f32)> {
        let mut indexed_scores: Vec<(usize, f32)> = self.importance_scores.iter()
            .enumerate()
            .map(|(i, &score)| (i, score))
            .collect();
        
        indexed_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        indexed_scores.into_iter()
            .take(k)
            .map(|(i, score)| (self.feature_names[i].clone(), score))
            .collect()
    }
}