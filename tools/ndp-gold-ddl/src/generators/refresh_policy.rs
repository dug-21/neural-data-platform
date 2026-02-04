//! Refresh policy generator for continuous aggregates
//!
//! Generates add_continuous_aggregate_policy SQL statements.

use crate::config::RefreshPolicyConfig;
use crate::error::Result;
use crate::validation::granularity_to_suffix;

/// Generator for refresh policy DDL
pub struct RefreshPolicyGenerator;

impl RefreshPolicyGenerator {
    /// Create a new generator
    pub fn new() -> Self {
        Self
    }

    /// Generate refresh policy SQL for a view
    pub fn generate(
        &self,
        stream_id: &str,
        granularity: &str,
        policy: Option<&RefreshPolicyConfig>,
    ) -> Result<String> {
        let suffix = granularity_to_suffix(granularity);
        let view_name = format!("gold.{}_{}", stream_id.replace('-', "_"), suffix);

        let default_policy = RefreshPolicyConfig::default();
        let policy = policy.unwrap_or(&default_policy);

        Ok(format!(
            r#"-- Refresh policy for {view_name}
SELECT add_continuous_aggregate_policy('{view_name}',
    start_offset => INTERVAL '{start_offset}',
    end_offset => INTERVAL '{end_offset}',
    schedule_interval => INTERVAL '{schedule_interval}',
    if_not_exists => TRUE
);"#,
            view_name = view_name,
            start_offset = policy.start_offset,
            end_offset = policy.end_offset,
            schedule_interval = policy.schedule_interval,
        ))
    }

    /// Generate policy removal SQL (for recreate mode)
    pub fn generate_remove(&self, stream_id: &str, granularity: &str) -> String {
        let suffix = granularity_to_suffix(granularity);
        let view_name = format!("gold.{}_{}", stream_id.replace('-', "_"), suffix);

        format!(
            r#"-- Remove existing policy (if any)
SELECT remove_continuous_aggregate_policy('{view_name}', if_exists => TRUE);"#,
            view_name = view_name,
        )
    }
}

impl Default for RefreshPolicyGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_refresh_policy_default() {
        let generator = RefreshPolicyGenerator::new();
        let sql = generator.generate("air-quality", "1 hour", None).unwrap();

        assert!(sql.contains("add_continuous_aggregate_policy"));
        assert!(sql.contains("gold.air_quality_hourly"));
        assert!(sql.contains("4 hours"));
        assert!(sql.contains("15 minutes"));
        assert!(sql.contains("if_not_exists => TRUE"));
    }

    #[test]
    fn test_generate_refresh_policy_custom() {
        let generator = RefreshPolicyGenerator::new();
        let policy = RefreshPolicyConfig {
            start_offset: "8 hours".to_string(),
            end_offset: "30 minutes".to_string(),
            schedule_interval: "30 minutes".to_string(),
        };

        let sql = generator.generate("air-quality", "1 hour", Some(&policy)).unwrap();

        assert!(sql.contains("8 hours"));
        assert!(sql.contains("30 minutes"));
    }

    #[test]
    fn test_generate_remove_policy() {
        let generator = RefreshPolicyGenerator::new();
        let sql = generator.generate_remove("air-quality", "1 day");

        assert!(sql.contains("remove_continuous_aggregate_policy"));
        assert!(sql.contains("gold.air_quality_daily"));
        assert!(sql.contains("if_exists => TRUE"));
    }
}
