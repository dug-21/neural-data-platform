//! Feature flags configuration module
//!
//! This module provides a centralized feature flag system for controlling
//! various features and technical debt cleanup initiatives in the neural trader.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;

/// Feature flags for controlling system behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Enforce all neural predictions to route through FANN predictor
    pub enforce_fann_routing: bool,

    /// Enable DAA (Distributed Autonomous Agents) orchestration
    pub enable_daa_orchestration: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            enforce_fann_routing: false,     // Enable in Phase 2
            enable_daa_orchestration: false, // Enable in Phase 3
        }
    }
}

impl FeatureFlags {
    /// Create feature flags from environment variables
    pub fn from_env() -> Result<Self> {
        let flags = Self {
            enforce_fann_routing: env::var("ENFORCE_FANN_ROUTING")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false),

            enable_daa_orchestration: env::var("ENABLE_DAA_ORCHESTRATION")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false),
        };

        Ok(flags)
    }

    /// Get the current feature flags from environment with caching
    pub fn get() -> Self {
        Self::from_env().unwrap_or_default()
    }

    /// Check if FANN routing should be enforced
    pub fn should_enforce_fann_routing(&self) -> bool {
        self.enforce_fann_routing
    }

    /// Check if DAA orchestration is enabled
    pub fn is_daa_orchestration_enabled(&self) -> bool {
        self.enable_daa_orchestration
    }

    /// Override feature flags for testing
    #[cfg(test)]
    pub fn test() -> Self {
        Self {
            enforce_fann_routing: true,
            enable_daa_orchestration: true,
        }
    }

    /// Create a builder for feature flags
    pub fn builder() -> FeatureFlagsBuilder {
        FeatureFlagsBuilder::default()
    }
}

/// Builder pattern for feature flags
#[derive(Default)]
pub struct FeatureFlagsBuilder {
    enforce_fann_routing: Option<bool>,
    enable_daa_orchestration: Option<bool>,
}

impl FeatureFlagsBuilder {
    pub fn enforce_fann_routing(mut self, value: bool) -> Self {
        self.enforce_fann_routing = Some(value);
        self
    }

    pub fn enable_daa_orchestration(mut self, value: bool) -> Self {
        self.enable_daa_orchestration = Some(value);
        self
    }

    pub fn build(self) -> FeatureFlags {
        let defaults = FeatureFlags::default();
        FeatureFlags {
            enforce_fann_routing: self
                .enforce_fann_routing
                .unwrap_or(defaults.enforce_fann_routing),
            enable_daa_orchestration: self
                .enable_daa_orchestration
                .unwrap_or(defaults.enable_daa_orchestration),
        }
    }
}

/// Percentage-based rollout for gradual feature deployment
pub fn should_use_feature(user_id: &str, feature_name: &str) -> bool {
    let env_var = format!("{}_PERCENTAGE", feature_name.to_uppercase());
    let percentage = env::var(&env_var)
        .ok()
        .and_then(|v| v.parse::<u8>().ok())
        .unwrap_or(0);

    if percentage >= 100 {
        return true;
    }

    if percentage == 0 {
        return false;
    }

    // Calculate hash for consistent user assignment
    let hash = calculate_hash(&format!("{}{}", user_id, feature_name));
    (hash % 100) < percentage
}

fn calculate_hash(input: &str) -> u8 {
    // Simple hash for demonstration - in production use a proper hash function
    input.bytes().fold(0u8, |acc, b| acc.wrapping_add(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_feature_flags() {
        let flags = FeatureFlags::default();
        assert!(!flags.enforce_fann_routing);
        assert!(!flags.enable_daa_orchestration);
    }

    #[test]
    fn test_feature_flags_builder() {
        let flags = FeatureFlags::builder().enforce_fann_routing(true).build();

        assert!(flags.enforce_fann_routing);
        assert!(!flags.enable_daa_orchestration);
    }

    #[test]
    fn test_percentage_rollout() {
        // Test with 0% rollout
        env::set_var("TEST_FEATURE_PERCENTAGE", "0");
        assert!(!should_use_feature("user123", "test_feature"));

        // Test with 100% rollout
        env::set_var("TEST_FEATURE_PERCENTAGE", "100");
        assert!(should_use_feature("user123", "test_feature"));

        // Clean up
        env::remove_var("TEST_FEATURE_PERCENTAGE");
    }
}
