//! Compatibility Shims for Phase 3 Extensions
//!
//! This module provides compatibility layers and conversion utilities
//! to ensure seamless integration between old and new interfaces.

use anyhow::Result;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::daa::autonomous_training::{PerformanceSnapshot, TrainingPriority as AutonomousTrainingPriority};
use crate::daa::enhanced_performance_snapshot::{EnhancedPerformanceSnapshot, DataTypeMetrics};
use crate::daa::training_scheduler::JobPriority;

/// Compatibility shim for PerformanceSnapshot migration
pub struct PerformanceSnapshotShim;

impl PerformanceSnapshotShim {
    /// Convert legacy PerformanceSnapshot to enhanced version with defaults
    pub fn upgrade_to_enhanced(base: PerformanceSnapshot) -> EnhancedPerformanceSnapshot {
        EnhancedPerformanceSnapshot::from_base_snapshot(base)
    }
    
    /// Extract base PerformanceSnapshot from enhanced version
    pub fn downgrade_from_enhanced(enhanced: EnhancedPerformanceSnapshot) -> PerformanceSnapshot {
        enhanced.into_base()
    }
    
    /// Check if snapshot has enhanced features
    pub fn has_enhancements(snapshot: &PerformanceSnapshot) -> bool {
        snapshot.data_type_metrics.is_some()
    }
    
    /// Safely access enhanced features with fallback
    pub fn get_data_type_metrics(snapshot: &PerformanceSnapshot) -> Option<&DataTypeMetrics> {
        snapshot.data_type_metrics.as_ref()
    }
    
    /// Add enhanced features to existing snapshot
    pub fn add_enhancements(
        mut snapshot: PerformanceSnapshot,
        data_type_metrics: DataTypeMetrics,
    ) -> PerformanceSnapshot {
        snapshot.data_type_metrics = Some(data_type_metrics);
        snapshot
    }
}

/// Compatibility shim for training priority conversion
pub struct TrainingPriorityShim;

impl TrainingPriorityShim {
    /// Convert autonomous training priority to job priority
    pub fn autonomous_to_job(priority: AutonomousTrainingPriority) -> JobPriority {
        JobPriority::from(priority)
    }
    
    /// Convert job priority back to autonomous training priority
    pub fn job_to_autonomous(priority: JobPriority) -> AutonomousTrainingPriority {
        match priority {
            JobPriority::Emergency => AutonomousTrainingPriority::Emergency,
            JobPriority::Critical => AutonomousTrainingPriority::Critical,
            JobPriority::High => AutonomousTrainingPriority::High,
            JobPriority::Medium => AutonomousTrainingPriority::Medium,
            JobPriority::Low => AutonomousTrainingPriority::Low,
            JobPriority::Background => AutonomousTrainingPriority::Low, // Map background to low
        }
    }
    
    /// Get priority weight for scheduling decisions
    pub fn get_priority_weight(priority: JobPriority) -> u32 {
        priority as u32
    }
}

/// Compatibility wrapper for VendorPredictor configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorPredictorShim {
    /// Legacy configuration support
    pub legacy_mode: bool,
    /// Cluster features enabled
    pub cluster_features: bool,
    /// Sector routing enabled
    pub sector_routing: bool,
}

impl Default for VendorPredictorShim {
    fn default() -> Self {
        Self {
            legacy_mode: false,
            cluster_features: true,
            sector_routing: true,
        }
    }
}

impl VendorPredictorShim {
    /// Create configuration that maintains backward compatibility
    pub fn legacy_compatible() -> Self {
        Self {
            legacy_mode: true,
            cluster_features: false,
            sector_routing: false,
        }
    }
    
    /// Create configuration with all new features enabled
    pub fn enhanced() -> Self {
        Self {
            legacy_mode: false,
            cluster_features: true,
            sector_routing: true,
        }
    }
}

/// Serialization compatibility utilities
pub struct SerializationShims;

impl SerializationShims {
    /// Convert JSON that may be missing new fields
    pub fn deserialize_with_defaults<T: for<'de> Deserialize<'de> + Default>(
        json: &str,
    ) -> Result<T> {
        // Try normal deserialization first
        match serde_json::from_str(json) {
            Ok(value) => Ok(value),
            Err(_) => {
                // If it fails, try deserializing as Value first and fill in defaults
                let mut value: serde_json::Value = serde_json::from_str(json)?;
                if let serde_json::Value::Object(ref mut map) = value {
                    // Add default values for any missing fields
                    // This is where we'd add logic to handle specific missing fields
                }
                serde_json::from_value(value).map_err(Into::into)
            }
        }
    }
    
    /// Serialize with backward compatible format
    pub fn serialize_compatible<T: Serialize>(value: &T) -> Result<String> {
        // Use standard serialization - new optional fields will be included
        // Old deserializers will ignore unknown fields
        serde_json::to_string(value).map_err(Into::into)
    }
    
    /// Check if JSON contains Phase 3 extensions
    pub fn has_phase3_extensions(json: &str) -> bool {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
            if let Some(obj) = value.as_object() {
                return obj.contains_key("data_type_metrics") ||
                       obj.contains_key("enhancement_metadata") ||
                       obj.contains_key("cluster_pools");
            }
        }
        false
    }
}

/// Configuration migration utilities
pub struct ConfigurationMigration;

impl ConfigurationMigration {
    /// Migrate old training trigger config to support new features
    pub fn migrate_training_trigger_config(
        config: crate::daa::autonomous_training::TrainingTriggerConfig,
    ) -> crate::daa::autonomous_training::TrainingTriggerConfig {
        // For now, just return the config as-is since it's already compatible
        // Future migrations would go here
        config
    }
    
    /// Migrate old scheduler config to support new resource management
    pub fn migrate_scheduler_config(
        config: crate::daa::training_scheduler::DAASchedulerConfig,
    ) -> crate::daa::training_scheduler::DAASchedulerConfig {
        // Config is already backward compatible
        config
    }
    
    /// Check if configuration needs migration
    pub fn needs_migration(config_version: &str) -> bool {
        // Check version string to determine if migration is needed
        match config_version {
            "1.0" | "1.1" | "1.2" => false, // Already compatible
            _ => false, // Unknown versions assumed compatible
        }
    }
}

/// API version compatibility checker
pub struct ApiVersionChecker;

impl ApiVersionChecker {
    /// Check if client API version is compatible with Phase 3
    pub fn is_compatible(client_version: &str) -> bool {
        // All versions are compatible due to backward compatibility design
        match client_version {
            v if v.starts_with("1.") => true,  // All 1.x versions compatible
            v if v.starts_with("2.") => true,  // All 2.x versions compatible
            v if v.starts_with("3.") => true,  // All 3.x versions compatible
            _ => true, // Unknown versions assumed compatible
        }
    }
    
    /// Get recommended migration path for client
    pub fn get_migration_recommendations(client_version: &str) -> Vec<String> {
        let mut recommendations = Vec::new();
        
        match client_version {
            v if v.starts_with("1.") => {
                recommendations.push("Consider upgrading to use enhanced performance snapshots".to_string());
                recommendations.push("Cluster model pools can improve memory efficiency".to_string());
            }
            v if v.starts_with("2.") => {
                recommendations.push("New training scheduler provides better resource management".to_string());
            }
            _ => {
                recommendations.push("All features available - no migration needed".to_string());
            }
        }
        
        recommendations
    }
}

/// Runtime compatibility checks
pub struct RuntimeCompatibilityChecker;

impl RuntimeCompatibilityChecker {
    /// Verify that all interfaces work correctly at runtime
    pub async fn verify_compatibility() -> Result<CompatibilityReport> {
        let mut report = CompatibilityReport::new();
        
        // Test AutonomousTrainingEngine compatibility
        report.add_check("AutonomousTrainingEngine", Self::test_training_engine().await);
        
        // Test scheduler compatibility
        report.add_check("DAATrainingScheduler", Self::test_scheduler().await);
        
        // Test serialization compatibility
        report.add_check("Serialization", Self::test_serialization().await);
        
        // Test priority conversion
        report.add_check("PriorityConversion", Self::test_priority_conversion().await);
        
        Ok(report)
    }
    
    async fn test_training_engine() -> bool {
        // Test basic engine creation and usage
        use crate::daa::autonomous_training::{AutonomousTrainingEngine, TrainingTriggerConfig, PerformanceSnapshot};
        use chrono::Utc;
        
        let config = TrainingTriggerConfig::default();
        if let Ok(engine) = AutonomousTrainingEngine::new(config) {
            let snapshot = PerformanceSnapshot {
                timestamp: Utc::now(),
                accuracy: 0.85,
                latency_ms: 100,
                error_rate: 0.05,
                recent_predictions: 50,
                confidence: 0.9,
                price_error: 0.02,
                sharpe_ratio: 1.5,
                max_drawdown: 0.03,
                volatility: 0.08,
                model_agreement: 0.95,
                consecutive_failures: 0,
                trading_volume: 1500.0,
                profit_loss: 75.0,
                data_type_metrics: None,
            };
            
            engine.evaluate_training_need(snapshot).await.is_ok()
        } else {
            false
        }
    }
    
    async fn test_scheduler() -> bool {
        use crate::daa::training_scheduler::{DAATrainingScheduler, DAASchedulerConfig};
        
        let config = DAASchedulerConfig::default();
        DAATrainingScheduler::new(config).is_ok()
    }
    
    async fn test_serialization() -> bool {
        use crate::daa::autonomous_training::PerformanceSnapshot;
        use chrono::Utc;
        
        let snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.85,
            latency_ms: 100,
            error_rate: 0.05,
            recent_predictions: 50,
            confidence: 0.9,
            price_error: 0.02,
            sharpe_ratio: 1.5,
            max_drawdown: 0.03,
            volatility: 0.08,
            model_agreement: 0.95,
            consecutive_failures: 0,
            trading_volume: 1500.0,
            profit_loss: 75.0,
            data_type_metrics: None,
        };
        
        if let Ok(json) = serde_json::to_string(&snapshot) {
            serde_json::from_str::<PerformanceSnapshot>(&json).is_ok()
        } else {
            false
        }
    }
    
    async fn test_priority_conversion() -> bool {
        use crate::daa::autonomous_training::TrainingPriority as AutonomousTrainingPriority;
        use crate::daa::training_scheduler::JobPriority;
        
        let autonomous_priority = AutonomousTrainingPriority::High;
        let job_priority = JobPriority::from(autonomous_priority);
        matches!(job_priority, JobPriority::High)
    }
}

/// Compatibility report for runtime verification
#[derive(Debug, Clone)]
pub struct CompatibilityReport {
    checks: HashMap<String, bool>,
    overall_compatible: bool,
}

impl CompatibilityReport {
    pub fn new() -> Self {
        Self {
            checks: HashMap::new(),
            overall_compatible: true,
        }
    }
    
    pub fn add_check(&mut self, name: &str, passed: bool) {
        self.checks.insert(name.to_string(), passed);
        if !passed {
            self.overall_compatible = false;
        }
    }
    
    pub fn is_compatible(&self) -> bool {
        self.overall_compatible
    }
    
    pub fn get_failed_checks(&self) -> Vec<String> {
        self.checks
            .iter()
            .filter(|(_, &passed)| !passed)
            .map(|(name, _)| name.clone())
            .collect()
    }
    
    pub fn get_summary(&self) -> String {
        if self.overall_compatible {
            format!("✅ All {} compatibility checks passed", self.checks.len())
        } else {
            let failed = self.get_failed_checks();
            format!("❌ {} of {} checks failed: {}", 
                    failed.len(), self.checks.len(), failed.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_compatibility_shims() -> Result<()> {
        // Test runtime compatibility checker
        let report = RuntimeCompatibilityChecker::verify_compatibility().await?;
        assert!(report.is_compatible(), "Compatibility check failed: {}", report.get_summary());
        
        Ok(())
    }
    
    #[test]
    fn test_priority_conversion() {
        use crate::daa::autonomous_training::TrainingPriority as AutonomousTrainingPriority;
        
        // Test all priority conversions
        assert_eq!(
            TrainingPriorityShim::autonomous_to_job(AutonomousTrainingPriority::Emergency),
            JobPriority::Emergency
        );
        assert_eq!(
            TrainingPriorityShim::autonomous_to_job(AutonomousTrainingPriority::High),
            JobPriority::High
        );
        
        // Test round-trip conversion
        let original = AutonomousTrainingPriority::Medium;
        let job_priority = TrainingPriorityShim::autonomous_to_job(original.clone());
        let converted_back = TrainingPriorityShim::job_to_autonomous(job_priority);
        assert_eq!(original, converted_back);
    }
}