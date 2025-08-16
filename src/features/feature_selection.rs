//! Adaptive Feature Selection for Neural Trading
//!
//! This module provides dynamic feature selection capabilities that adapt based on
//! model performance feedback and importance scores.

use anyhow::Result;
use std::collections::HashMap;

/// Adaptive feature selector that learns feature importance over time
#[derive(Debug)]
pub struct AdaptiveFeatureSelector {
    importance_threshold: f64,
    feature_importance: HashMap<String, f64>,
    usage_count: HashMap<String, usize>,
}

impl AdaptiveFeatureSelector {
    /// Create a new adaptive feature selector
    pub fn new(importance_threshold: f64) -> Self {
        Self {
            importance_threshold,
            feature_importance: HashMap::new(),
            usage_count: HashMap::new(),
        }
    }

    /// Select features based on importance scores
    pub async fn select_features(
        &self,
        features: &HashMap<String, f64>,
    ) -> Result<HashMap<String, f64>> {
        let mut selected = HashMap::new();

        for (name, value) in features {
            let importance = self.feature_importance.get(name).unwrap_or(&0.0);
            
            if *importance >= self.importance_threshold {
                selected.insert(name.clone(), *value);
            }
        }

        // If no features meet threshold, use top features
        if selected.is_empty() {
            let mut sorted_features: Vec<_> = features.iter().collect();
            sorted_features.sort_by(|a, b| {
                let imp_a = self.feature_importance.get(a.0).unwrap_or(&0.0);
                let imp_b = self.feature_importance.get(b.0).unwrap_or(&0.0);
                imp_b.partial_cmp(imp_a).unwrap_or(std::cmp::Ordering::Equal)
            });

            // Take top 50% of features
            let take_count = (features.len() / 2).max(1);
            for (name, value) in sorted_features.into_iter().take(take_count) {
                selected.insert(name.clone(), *value);
            }
        }

        Ok(selected)
    }

    /// Get current feature importance scores
    pub async fn get_importance_scores(&self) -> Result<HashMap<String, f64>> {
        Ok(self.feature_importance.clone())
    }

    /// Update feature importance based on model feedback
    pub async fn update_importance(
        &mut self,
        importance_scores: HashMap<String, f64>,
    ) -> Result<()> {
        for (feature, importance) in importance_scores {
            // Use exponential moving average to update importance
            let current = self.feature_importance.get(&feature).unwrap_or(&0.0);
            let alpha = 0.1; // Learning rate
            let updated = alpha * importance + (1.0 - alpha) * current;
            
            self.feature_importance.insert(feature.clone(), updated);
            
            // Track usage
            let count = self.usage_count.get(&feature).unwrap_or(&0);
            self.usage_count.insert(feature, count + 1);
        }

        Ok(())
    }

    /// Get feature usage statistics
    pub fn get_usage_stats(&self) -> HashMap<String, usize> {
        self.usage_count.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_feature_selection() {
        let mut selector = AdaptiveFeatureSelector::new(0.5);
        
        let mut features = HashMap::new();
        features.insert("feature_1".to_string(), 1.0);
        features.insert("feature_2".to_string(), 2.0);
        features.insert("feature_3".to_string(), 3.0);

        let selected = selector.select_features(&features).await.unwrap();
        assert!(!selected.is_empty());
    }

    #[tokio::test]
    async fn test_importance_update() {
        let mut selector = AdaptiveFeatureSelector::new(0.5);
        
        let mut importance = HashMap::new();
        importance.insert("feature_1".to_string(), 0.8);
        importance.insert("feature_2".to_string(), 0.3);

        selector.update_importance(importance).await.unwrap();
        
        let scores = selector.get_importance_scores().await.unwrap();
        assert!(scores.contains_key("feature_1"));
        assert!(scores.contains_key("feature_2"));
    }
}