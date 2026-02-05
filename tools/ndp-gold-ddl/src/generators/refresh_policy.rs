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
    ///
    /// Uses granularity-aware defaults if no explicit policy is provided:
    /// - Hourly aggregates: 15 min schedule, 4 hour lookback, 15 min end offset
    /// - Daily aggregates: 1 hour schedule, 3 day lookback, 1 hour end offset
    /// - Other: 30 min schedule, 4 hour lookback, 30 min end offset
    pub fn generate(
        &self,
        stream_id: &str,
        granularity: &str,
        policy: Option<&RefreshPolicyConfig>,
    ) -> Result<String> {
        let suffix = granularity_to_suffix(granularity);
        let view_name = format!("gold.{}_{}", stream_id.replace('-', "_"), suffix);

        // Use granularity-aware defaults
        let effective_policy = RefreshPolicyConfig::for_granularity(granularity, policy);

        Ok(format!(
            r#"-- Refresh policy for {view_name}
SELECT add_continuous_aggregate_policy('{view_name}',
    start_offset => INTERVAL '{start_offset}',
    end_offset => INTERVAL '{end_offset}',
    schedule_interval => INTERVAL '{schedule_interval}',
    if_not_exists => TRUE
);"#,
            view_name = view_name,
            start_offset = effective_policy.start_offset,
            end_offset = effective_policy.end_offset,
            schedule_interval = effective_policy.schedule_interval,
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

        let sql = generator
            .generate("air-quality", "1 hour", Some(&policy))
            .unwrap();

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

    // =========================================================================
    // v11-004: Granularity-Aware Policy Tests
    // =========================================================================

    #[test]
    fn test_generate_refresh_policy_daily_uses_daily_defaults() {
        let generator = RefreshPolicyGenerator::new();
        // No explicit policy - should use daily defaults
        let sql = generator.generate("air-quality", "1 day", None).unwrap();

        assert!(sql.contains("gold.air_quality_daily"));
        assert!(sql.contains("3 days")); // daily start_offset
        assert!(sql.contains("schedule_interval => INTERVAL '1 hour'")); // daily schedule
    }

    #[test]
    fn test_generate_refresh_policy_hourly_uses_hourly_defaults() {
        let generator = RefreshPolicyGenerator::new();
        let sql = generator.generate("air-quality", "1 hour", None).unwrap();

        assert!(sql.contains("gold.air_quality_hourly"));
        assert!(sql.contains("start_offset => INTERVAL '4 hours'")); // hourly start_offset
        assert!(sql.contains("schedule_interval => INTERVAL '15 minutes'")); // hourly schedule
    }

    #[test]
    fn test_generate_refresh_policy_custom_granularity_uses_other_defaults() {
        let generator = RefreshPolicyGenerator::new();
        let sql = generator
            .generate("air-quality", "15 minutes", None)
            .unwrap();

        assert!(sql.contains("gold.air_quality_15min"));
        assert!(sql.contains("schedule_interval => INTERVAL '30 minutes'")); // other default
    }

    #[test]
    fn test_generate_refresh_policy_explicit_overrides_defaults() {
        let generator = RefreshPolicyGenerator::new();
        let custom_policy = RefreshPolicyConfig {
            start_offset: "2 hours".to_string(),
            end_offset: "5 minutes".to_string(),
            schedule_interval: "10 minutes".to_string(),
        };

        // Even for daily, if explicit policy provided, use it
        let sql = generator
            .generate("air-quality", "1 day", Some(&custom_policy))
            .unwrap();

        assert!(sql.contains("2 hours")); // explicit start_offset
        assert!(sql.contains("5 minutes")); // explicit end_offset
        assert!(sql.contains("10 minutes")); // explicit schedule
    }

    #[test]
    fn test_generate_returns_sql_with_view_name() {
        let generator = RefreshPolicyGenerator::new();
        let sql = generator.generate("nws-forecast", "4 hours", None).unwrap();

        assert!(sql.contains("gold.nws_forecast_4hourly"));
    }
}
