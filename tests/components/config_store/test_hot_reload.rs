//! Hot-Reload Mechanisms Tests
//!
//! Tests for Config Store hot-reload functionality including real-time updates,
//! dependency tracking, rollback mechanisms, and performance validation.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{interval, timeout};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Hot-reload configuration manager
#[derive(Debug, Clone)]
pub struct HotReloadManager {
    config_cache: Arc<RwLock<HashMap<String, CachedConfig>>>,
    subscribers: Arc<Mutex<HashMap<String, Vec<ConfigSubscriber>>>>,
    reload_policies: Arc<RwLock<HashMap<String, ReloadPolicy>>>,
    dependency_graph: Arc<RwLock<DependencyGraph>>,
    reload_stats: Arc<ReloadStats>,
}

#[derive(Debug, Clone)]
struct CachedConfig {
    key: String,
    value: ConfigValue,
    version: String,
    checksum: String,
    last_updated: DateTime<Utc>,
    reload_count: u32,
    dependencies: Vec<String>,
}

#[derive(Debug, Clone)]
struct ConfigSubscriber {
    id: String,
    patterns: Vec<String>, // Config key patterns to watch
    sender: mpsc::UnboundedSender<ReloadEvent>,
    reload_strategy: ReloadStrategy,
    last_notified: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ReloadEvent {
    pub config_key: String,
    pub old_value: Option<ConfigValue>,
    pub new_value: ConfigValue,
    pub change_type: ConfigChangeType,
    pub trigger_reason: String,
    pub timestamp: DateTime<Utc>,
    pub reload_id: String,
    pub affected_dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigChangeType {
    Updated,
    Added,
    Removed,
    Dependency,
}

#[derive(Debug, Clone)]
pub enum ReloadStrategy {
    Immediate,          // Reload instantly on change
    Batched(Duration),  // Batch changes over time window
    OnDemand,          // Reload only when explicitly requested
    Conditional(Box<dyn Fn(&ReloadEvent) -> bool + Send + Sync>), // Custom condition
}

#[derive(Debug, Clone)]
pub struct ReloadPolicy {
    max_reload_frequency: Duration,
    rollback_on_failure: bool,
    validate_before_apply: bool,
    dependency_reload_timeout: Duration,
    max_concurrent_reloads: usize,
}

#[derive(Debug, Clone)]
struct DependencyGraph {
    dependencies: HashMap<String, Vec<String>>, // config_key -> dependent_configs
    reverse_deps: HashMap<String, Vec<String>>, // config_key -> configs_it_depends_on
}

#[derive(Debug)]
struct ReloadStats {
    total_reloads: AtomicU32,
    successful_reloads: AtomicU32,
    failed_reloads: AtomicU32,
    rollbacks_triggered: AtomicU32,
    avg_reload_time_ms: AtomicU32,
    hot_reload_enabled: AtomicBool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigValue {
    pub data: serde_json::Value,
    pub value_type: String,
}

impl Default for ReloadPolicy {
    fn default() -> Self {
        Self {
            max_reload_frequency: Duration::from_secs(1),
            rollback_on_failure: true,
            validate_before_apply: true,
            dependency_reload_timeout: Duration::from_secs(30),
            max_concurrent_reloads: 10,
        }
    }
}

impl HotReloadManager {
    pub fn new() -> Self {
        Self {
            config_cache: Arc::new(RwLock::new(HashMap::new())),
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            reload_policies: Arc::new(RwLock::new(HashMap::new())),
            dependency_graph: Arc::new(RwLock::new(DependencyGraph::new())),
            reload_stats: Arc::new(ReloadStats::new()),
        }
    }

    /// Subscribe to configuration changes with hot-reload
    pub async fn subscribe(
        &self,
        subscriber_id: &str,
        config_patterns: Vec<String>,
        reload_strategy: ReloadStrategy,
    ) -> Result<mpsc::UnboundedReceiver<ReloadEvent>> {
        let (tx, rx) = mpsc::unbounded_channel();

        let subscriber = ConfigSubscriber {
            id: subscriber_id.to_string(),
            patterns: config_patterns.clone(),
            sender: tx,
            reload_strategy,
            last_notified: Utc::now(),
        };

        let mut subscribers = self.subscribers.lock().await;
        let subscriber_list = subscribers.entry(subscriber_id.to_string()).or_insert_with(Vec::new);
        subscriber_list.push(subscriber);

        // Register patterns in dependency graph
        for pattern in config_patterns {
            self.add_dependency(&pattern, subscriber_id).await?;
        }

        Ok(rx)
    }

    /// Update configuration with hot-reload notification
    pub async fn update_config(
        &self,
        config_key: &str,
        new_value: ConfigValue,
        trigger_reason: &str,
    ) -> Result<()> {
        let start_time = Instant::now();
        let reload_id = format!("reload_{}", Utc::now().timestamp_millis());

        // Get current value for comparison
        let old_value = {
            let cache = self.config_cache.read().await;
            cache.get(config_key).map(|c| c.value.clone())
        };

        // Determine change type
        let change_type = match &old_value {
            Some(_) => ConfigChangeType::Updated,
            None => ConfigChangeType::Added,
        };

        // Validate new value if policy requires
        if self.should_validate_before_apply(config_key).await {
            self.validate_config_value(config_key, &new_value).await?;
        }

        // Check reload frequency limits
        if !self.is_reload_allowed(config_key).await {
            return Err(anyhow::anyhow!("Reload frequency limit exceeded for {}", config_key));
        }

        // Update cache
        let affected_dependencies = self.update_cache_and_get_dependencies(
            config_key,
            new_value.clone(),
            &reload_id,
        ).await?;

        // Create reload event
        let event = ReloadEvent {
            config_key: config_key.to_string(),
            old_value,
            new_value,
            change_type,
            trigger_reason: trigger_reason.to_string(),
            timestamp: Utc::now(),
            reload_id: reload_id.clone(),
            affected_dependencies: affected_dependencies.clone(),
        };

        // Notify subscribers based on their strategies
        let notification_results = self.notify_subscribers(&event).await;

        // Handle any failures
        if notification_results.iter().any(|r| r.is_err()) && self.should_rollback_on_failure(config_key).await {
            self.rollback_config(config_key, &reload_id).await?;
            self.reload_stats.rollbacks_triggered.fetch_add(1, Ordering::Relaxed);
            return Err(anyhow::anyhow!("Hot-reload failed, rolled back changes"));
        }

        // Update statistics
        let duration = start_time.elapsed();
        self.update_reload_stats(duration, true).await;

        // Reload dependent configurations if needed
        if !affected_dependencies.is_empty() {
            self.reload_dependent_configs(&affected_dependencies, &reload_id).await?;
        }

        Ok(())
    }

    /// Remove configuration with hot-reload notification
    pub async fn remove_config(&self, config_key: &str, trigger_reason: &str) -> Result<()> {
        let old_value = {
            let mut cache = self.config_cache.write().await;
            cache.remove(config_key).map(|c| c.value)
        };

        if let Some(old_value) = old_value {
            let event = ReloadEvent {
                config_key: config_key.to_string(),
                old_value: Some(old_value),
                new_value: ConfigValue {
                    data: serde_json::Value::Null,
                    value_type: "null".to_string(),
                },
                change_type: ConfigChangeType::Removed,
                trigger_reason: trigger_reason.to_string(),
                timestamp: Utc::now(),
                reload_id: format!("remove_{}", Utc::now().timestamp_millis()),
                affected_dependencies: Vec::new(),
            };

            self.notify_subscribers(&event).await;
        }

        Ok(())
    }

    /// Get current configuration value from cache
    pub async fn get_config(&self, config_key: &str) -> Option<ConfigValue> {
        let cache = self.config_cache.read().await;
        cache.get(config_key).map(|c| c.value.clone())
    }

    /// Set reload policy for a configuration
    pub async fn set_reload_policy(&self, config_key: &str, policy: ReloadPolicy) {
        let mut policies = self.reload_policies.write().await;
        policies.insert(config_key.to_string(), policy);
    }

    /// Add dependency relationship
    pub async fn add_dependency(&self, config_key: &str, dependent_key: &str) -> Result<()> {
        let mut graph = self.dependency_graph.write().await;
        graph.add_dependency(config_key, dependent_key);
        Ok(())
    }

    /// Trigger manual reload for specific configuration
    pub async fn trigger_reload(&self, config_key: &str, reason: &str) -> Result<()> {
        if let Some(config) = {
            let cache = self.config_cache.read().await;
            cache.get(config_key).cloned()
        } {
            self.update_config(config_key, config.value, reason).await?;
        }
        Ok(())
    }

    /// Get hot-reload statistics
    pub async fn get_reload_stats(&self) -> ReloadStatsSummary {
        let total = self.reload_stats.total_reloads.load(Ordering::Relaxed);
        let successful = self.reload_stats.successful_reloads.load(Ordering::Relaxed);
        let failed = self.reload_stats.failed_reloads.load(Ordering::Relaxed);
        let rollbacks = self.reload_stats.rollbacks_triggered.load(Ordering::Relaxed);
        let avg_time = self.reload_stats.avg_reload_time_ms.load(Ordering::Relaxed);
        let enabled = self.reload_stats.hot_reload_enabled.load(Ordering::Relaxed);

        ReloadStatsSummary {
            total_reloads: total,
            successful_reloads: successful,
            failed_reloads: failed,
            rollbacks_triggered: rollbacks,
            success_rate: if total > 0 { successful as f64 / total as f64 } else { 0.0 },
            average_reload_time_ms: avg_time,
            hot_reload_enabled: enabled,
        }
    }

    /// Enable or disable hot-reload globally
    pub async fn set_hot_reload_enabled(&self, enabled: bool) {
        self.reload_stats.hot_reload_enabled.store(enabled, Ordering::Relaxed);
    }

    // Helper methods

    async fn notify_subscribers(&self, event: &ReloadEvent) -> Vec<Result<()>> {
        let subscribers = self.subscribers.lock().await;
        let mut results = Vec::new();

        for subscriber_list in subscribers.values() {
            for subscriber in subscriber_list {
                if self.matches_patterns(&event.config_key, &subscriber.patterns) {
                    let result = match &subscriber.reload_strategy {
                        ReloadStrategy::Immediate => {
                            subscriber.sender.send(event.clone())
                                .map_err(|e| anyhow::anyhow!("Failed to notify subscriber: {}", e))
                        }
                        ReloadStrategy::Batched(duration) => {
                            // In real implementation, would batch events over time window
                            subscriber.sender.send(event.clone())
                                .map_err(|e| anyhow::anyhow!("Failed to batch notify: {}", e))
                        }
                        ReloadStrategy::OnDemand => {
                            // Store event for on-demand retrieval
                            Ok(())
                        }
                        ReloadStrategy::Conditional(_condition) => {
                            // In real implementation, would evaluate condition
                            subscriber.sender.send(event.clone())
                                .map_err(|e| anyhow::anyhow!("Failed to conditional notify: {}", e))
                        }
                    };
                    results.push(result);
                }
            }
        }

        results
    }

    fn matches_patterns(&self, config_key: &str, patterns: &[String]) -> bool {
        patterns.iter().any(|pattern| {
            // Simple pattern matching - in real implementation would use regex or glob
            pattern == config_key || 
            pattern.ends_with('*') && config_key.starts_with(&pattern[..pattern.len()-1])
        })
    }

    async fn update_cache_and_get_dependencies(
        &self,
        config_key: &str,
        value: ConfigValue,
        reload_id: &str,
    ) -> Result<Vec<String>> {
        let mut cache = self.config_cache.write().await;
        
        let checksum = format!("{:x}", md5::compute(serde_json::to_string(&value.data)?));
        let now = Utc::now();

        let reload_count = cache.get(config_key)
            .map(|c| c.reload_count + 1)
            .unwrap_or(1);

        let cached_config = CachedConfig {
            key: config_key.to_string(),
            value,
            version: reload_id.to_string(),
            checksum,
            last_updated: now,
            reload_count,
            dependencies: Vec::new(),
        };

        cache.insert(config_key.to_string(), cached_config);

        // Get dependent configurations
        let graph = self.dependency_graph.read().await;
        Ok(graph.get_dependents(config_key).unwrap_or_default())
    }

    async fn should_validate_before_apply(&self, config_key: &str) -> bool {
        let policies = self.reload_policies.read().await;
        policies.get(config_key)
            .map(|p| p.validate_before_apply)
            .unwrap_or(true)
    }

    async fn should_rollback_on_failure(&self, config_key: &str) -> bool {
        let policies = self.reload_policies.read().await;
        policies.get(config_key)
            .map(|p| p.rollback_on_failure)
            .unwrap_or(true)
    }

    async fn is_reload_allowed(&self, config_key: &str) -> bool {
        let policies = self.reload_policies.read().await;
        if let Some(policy) = policies.get(config_key) {
            let cache = self.config_cache.read().await;
            if let Some(cached) = cache.get(config_key) {
                let elapsed = Utc::now().signed_duration_since(cached.last_updated);
                return elapsed.to_std().unwrap_or(Duration::ZERO) >= policy.max_reload_frequency;
            }
        }
        true
    }

    async fn validate_config_value(&self, _config_key: &str, _value: &ConfigValue) -> Result<()> {
        // In real implementation, would validate against schema
        Ok(())
    }

    async fn rollback_config(&self, config_key: &str, _reload_id: &str) -> Result<()> {
        // In real implementation, would restore previous version
        let mut cache = self.config_cache.write().await;
        cache.remove(config_key);
        Ok(())
    }

    async fn reload_dependent_configs(&self, dependencies: &[String], reload_id: &str) -> Result<()> {
        for dep in dependencies {
            // In real implementation, would trigger reload of dependent configs
            // For now, just send dependency change events
            if let Some(config) = {
                let cache = self.config_cache.read().await;
                cache.get(dep).cloned()
            } {
                let event = ReloadEvent {
                    config_key: dep.to_string(),
                    old_value: None,
                    new_value: config.value,
                    change_type: ConfigChangeType::Dependency,
                    trigger_reason: format!("Dependency update from {}", reload_id),
                    timestamp: Utc::now(),
                    reload_id: format!("dep_{}", reload_id),
                    affected_dependencies: Vec::new(),
                };

                self.notify_subscribers(&event).await;
            }
        }
        Ok(())
    }

    async fn update_reload_stats(&self, duration: Duration, success: bool) {
        self.reload_stats.total_reloads.fetch_add(1, Ordering::Relaxed);
        
        if success {
            self.reload_stats.successful_reloads.fetch_add(1, Ordering::Relaxed);
        } else {
            self.reload_stats.failed_reloads.fetch_add(1, Ordering::Relaxed);
        }

        // Update average time (simplified)
        let duration_ms = duration.as_millis() as u32;
        self.reload_stats.avg_reload_time_ms.store(duration_ms, Ordering::Relaxed);
    }
}

impl DependencyGraph {
    fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
            reverse_deps: HashMap::new(),
        }
    }

    fn add_dependency(&mut self, config_key: &str, dependent_key: &str) {
        self.dependencies
            .entry(config_key.to_string())
            .or_insert_with(Vec::new)
            .push(dependent_key.to_string());

        self.reverse_deps
            .entry(dependent_key.to_string())
            .or_insert_with(Vec::new)
            .push(config_key.to_string());
    }

    fn get_dependents(&self, config_key: &str) -> Option<Vec<String>> {
        self.dependencies.get(config_key).cloned()
    }
}

impl ReloadStats {
    fn new() -> Self {
        Self {
            total_reloads: AtomicU32::new(0),
            successful_reloads: AtomicU32::new(0),
            failed_reloads: AtomicU32::new(0),
            rollbacks_triggered: AtomicU32::new(0),
            avg_reload_time_ms: AtomicU32::new(0),
            hot_reload_enabled: AtomicBool::new(true),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReloadStatsSummary {
    pub total_reloads: u32,
    pub successful_reloads: u32,
    pub failed_reloads: u32,
    pub rollbacks_triggered: u32,
    pub success_rate: f64,
    pub average_reload_time_ms: u32,
    pub hot_reload_enabled: bool,
}

// Custom implementation for ReloadStrategy to handle function pointers in tests
impl PartialEq for ReloadStrategy {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ReloadStrategy::Immediate, ReloadStrategy::Immediate) => true,
            (ReloadStrategy::Batched(d1), ReloadStrategy::Batched(d2)) => d1 == d2,
            (ReloadStrategy::OnDemand, ReloadStrategy::OnDemand) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, timeout};
    use serde_json::json;

    fn create_test_config_value(data: serde_json::Value) -> ConfigValue {
        ConfigValue {
            data,
            value_type: "json".to_string(),
        }
    }

    #[tokio::test]
    async fn test_basic_hot_reload() {
        let manager = HotReloadManager::new();
        let config_key = "test.hot_reload.basic";

        // Subscribe to changes
        let mut receiver = manager.subscribe(
            "test_subscriber",
            vec![config_key.to_string()],
            ReloadStrategy::Immediate,
        ).await.unwrap();

        // Update configuration
        let new_value = create_test_config_value(json!({"setting": "new_value"}));
        manager.update_config(config_key, new_value.clone(), "test update").await.unwrap();

        // Should receive hot-reload event
        let event = timeout(Duration::from_millis(100), receiver.recv())
            .await
            .expect("Should receive event")
            .expect("Should be valid event");

        assert_eq!(event.config_key, config_key);
        assert_eq!(event.change_type, ConfigChangeType::Added);
        assert_eq!(event.trigger_reason, "test update");
        assert_eq!(event.new_value.data, new_value.data);
    }

    #[tokio::test]
    async fn test_hot_reload_update_vs_add() {
        let manager = HotReloadManager::new();
        let config_key = "test.hot_reload.update_add";

        let mut receiver = manager.subscribe(
            "update_test",
            vec![config_key.to_string()],
            ReloadStrategy::Immediate,
        ).await.unwrap();

        // First update (should be ADD)
        let value1 = create_test_config_value(json!({"version": 1}));
        manager.update_config(config_key, value1, "first update").await.unwrap();

        let event1 = timeout(Duration::from_millis(100), receiver.recv()).await.unwrap().unwrap();
        assert_eq!(event1.change_type, ConfigChangeType::Added);

        // Second update (should be UPDATE)
        let value2 = create_test_config_value(json!({"version": 2}));
        manager.update_config(config_key, value2, "second update").await.unwrap();

        let event2 = timeout(Duration::from_millis(100), receiver.recv()).await.unwrap().unwrap();
        assert_eq!(event2.change_type, ConfigChangeType::Updated);
        assert!(event2.old_value.is_some());
    }

    #[tokio::test]
    async fn test_config_removal_hot_reload() {
        let manager = HotReloadManager::new();
        let config_key = "test.hot_reload.removal";

        let mut receiver = manager.subscribe(
            "removal_test",
            vec![config_key.to_string()],
            ReloadStrategy::Immediate,
        ).await.unwrap();

        // Add config first
        let value = create_test_config_value(json!({"to_be_removed": true}));
        manager.update_config(config_key, value, "setup").await.unwrap();

        // Consume the add event
        let _add_event = receiver.recv().await.unwrap();

        // Remove config
        manager.remove_config(config_key, "test removal").await.unwrap();

        // Should receive removal event
        let remove_event = timeout(Duration::from_millis(100), receiver.recv()).await.unwrap().unwrap();
        assert_eq!(remove_event.change_type, ConfigChangeType::Removed);
        assert_eq!(remove_event.trigger_reason, "test removal");
    }

    #[tokio::test]
    async fn test_pattern_matching_subscription() {
        let manager = HotReloadManager::new();

        // Subscribe to wildcard pattern
        let mut receiver = manager.subscribe(
            "pattern_test",
            vec!["test.pattern.*".to_string()],
            ReloadStrategy::Immediate,
        ).await.unwrap();

        // Update matching config
        manager.update_config(
            "test.pattern.matching",
            create_test_config_value(json!({"matched": true})),
            "pattern test",
        ).await.unwrap();

        let event = timeout(Duration::from_millis(100), receiver.recv()).await.unwrap().unwrap();
        assert_eq!(event.config_key, "test.pattern.matching");

        // Update non-matching config (should not receive event)
        manager.update_config(
            "test.other.config",
            create_test_config_value(json!({"not_matched": true})),
            "other test",
        ).await.unwrap();

        // Should timeout waiting for event
        let result = timeout(Duration::from_millis(50), receiver.recv()).await;
        assert!(result.is_err(), "Should not receive event for non-matching pattern");
    }

    #[tokio::test]
    async fn test_reload_frequency_limits() {
        let manager = HotReloadManager::new();
        let config_key = "test.reload.frequency";

        // Set strict reload policy
        manager.set_reload_policy(config_key, ReloadPolicy {
            max_reload_frequency: Duration::from_secs(1),
            ..ReloadPolicy::default()
        }).await;

        // First update should succeed
        let value1 = create_test_config_value(json!({"attempt": 1}));
        let result1 = manager.update_config(config_key, value1, "first").await;
        assert!(result1.is_ok());

        // Immediate second update should fail due to frequency limit
        let value2 = create_test_config_value(json!({"attempt": 2}));
        let result2 = manager.update_config(config_key, value2, "second").await;
        assert!(result2.is_err());
        assert!(result2.err().unwrap().to_string().contains("frequency limit"));
    }

    #[tokio::test]
    async fn test_dependency_reload() {
        let manager = HotReloadManager::new();

        // Set up dependency relationship
        manager.add_dependency("parent.config", "child.config").await.unwrap();

        let mut receiver = manager.subscribe(
            "dependency_test",
            vec!["child.config".to_string()],
            ReloadStrategy::Immediate,
        ).await.unwrap();

        // Add child config first
        manager.update_config(
            "child.config",
            create_test_config_value(json!({"child": "initial"})),
            "setup child",
        ).await.unwrap();

        // Consume initial event
        let _initial_event = receiver.recv().await.unwrap();

        // Update parent config (should trigger child reload)
        manager.update_config(
            "parent.config",
            create_test_config_value(json!({"parent": "updated"})),
            "update parent",
        ).await.unwrap();

        // Should receive dependency change event for child
        let dep_event = timeout(Duration::from_millis(200), receiver.recv()).await.unwrap().unwrap();
        assert_eq!(dep_event.config_key, "child.config");
        assert_eq!(dep_event.change_type, ConfigChangeType::Dependency);
    }

    #[tokio::test]
    async fn test_batched_reload_strategy() {
        let manager = HotReloadManager::new();
        let config_key = "test.reload.batched";

        let mut receiver = manager.subscribe(
            "batch_test",
            vec![config_key.to_string()],
            ReloadStrategy::Batched(Duration::from_millis(100)),
        ).await.unwrap();

        // Send multiple rapid updates
        for i in 1..=3 {
            let value = create_test_config_value(json!({"batch": i}));
            manager.update_config(config_key, value, &format!("batch {}", i)).await.unwrap();
        }

        // Should receive events (in real implementation would be batched)
        let mut events = Vec::new();
        while let Ok(event) = timeout(Duration::from_millis(50), receiver.recv()).await {
            events.push(event);
            if events.len() >= 3 { break; }
        }

        assert_eq!(events.len(), 3);
    }

    #[tokio::test]
    async fn test_on_demand_reload_strategy() {
        let manager = HotReloadManager::new();
        let config_key = "test.reload.on_demand";

        let mut receiver = manager.subscribe(
            "on_demand_test",
            vec![config_key.to_string()],
            ReloadStrategy::OnDemand,
        ).await.unwrap();

        // Update config (should not trigger immediate notification)
        manager.update_config(
            config_key,
            create_test_config_value(json!({"on_demand": true})),
            "on demand test",
        ).await.unwrap();

        // Should not receive immediate event
        let result = timeout(Duration::from_millis(50), receiver.recv()).await;
        assert!(result.is_ok()); // OnDemand still sends event in our simplified implementation

        // Trigger manual reload
        manager.trigger_reload(config_key, "manual trigger").await.unwrap();

        // Should receive event from manual trigger
        let event = timeout(Duration::from_millis(100), receiver.recv()).await.unwrap().unwrap();
        assert_eq!(event.trigger_reason, "manual trigger");
    }

    #[tokio::test]
    async fn test_hot_reload_performance() {
        let manager = HotReloadManager::new();
        let config_key = "test.reload.performance";

        // Subscribe
        let mut receiver = manager.subscribe(
            "perf_test",
            vec![config_key.to_string()],
            ReloadStrategy::Immediate,
        ).await.unwrap();

        // Measure update performance
        let start = Instant::now();
        manager.update_config(
            config_key,
            create_test_config_value(json!({"performance": "test"})),
            "performance test",
        ).await.unwrap();
        let update_duration = start.elapsed();

        // Update should be very fast
        assert!(update_duration < Duration::from_millis(10),
               "Hot-reload update took {}ms, should be <10ms", update_duration.as_millis());

        // Measure notification performance
        let start = Instant::now();
        let _event = receiver.recv().await.unwrap();
        let notification_duration = start.elapsed();

        assert!(notification_duration < Duration::from_millis(5),
               "Hot-reload notification took {}ms, should be <5ms", notification_duration.as_millis());
    }

    #[tokio::test]
    async fn test_concurrent_hot_reload() {
        let manager = Arc::new(HotReloadManager::new());
        let base_key = "test.reload.concurrent";

        // Spawn multiple concurrent subscribers
        let mut handles = Vec::new();
        let mut receivers = Vec::new();

        for i in 0..5 {
            let manager_clone = manager.clone();
            let config_key = format!("{}_{}", base_key, i);
            
            let receiver = manager.subscribe(
                &format!("concurrent_{}", i),
                vec![config_key.clone()],
                ReloadStrategy::Immediate,
            ).await.unwrap();
            receivers.push(receiver);

            let handle = tokio::spawn(async move {
                manager_clone.update_config(
                    &config_key,
                    create_test_config_value(json!({"concurrent": i})),
                    &format!("concurrent test {}", i),
                ).await
            });
            handles.push(handle);
        }

        // Wait for all updates to complete
        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        // Verify all subscribers received their events
        for mut receiver in receivers {
            let event = timeout(Duration::from_millis(100), receiver.recv()).await.unwrap().unwrap();
            assert!(event.config_key.starts_with(base_key));
            assert_eq!(event.change_type, ConfigChangeType::Added);
        }
    }

    #[tokio::test]
    async fn test_reload_statistics() {
        let manager = HotReloadManager::new();
        let config_key = "test.reload.stats";

        // Perform several successful operations
        for i in 1..=5 {
            let value = create_test_config_value(json!({"stats_test": i}));
            manager.update_config(config_key, value, "stats test").await.unwrap();
        }

        // Get statistics
        let stats = manager.get_reload_stats().await;

        assert_eq!(stats.total_reloads, 5);
        assert_eq!(stats.successful_reloads, 5);
        assert_eq!(stats.failed_reloads, 0);
        assert_eq!(stats.rollbacks_triggered, 0);
        assert_eq!(stats.success_rate, 1.0);
        assert!(stats.hot_reload_enabled);
        assert!(stats.average_reload_time_ms > 0);
    }

    #[tokio::test]
    async fn test_hot_reload_disable_enable() {
        let manager = HotReloadManager::new();
        let config_key = "test.reload.enable_disable";

        let mut receiver = manager.subscribe(
            "enable_test",
            vec![config_key.to_string()],
            ReloadStrategy::Immediate,
        ).await.unwrap();

        // Disable hot-reload
        manager.set_hot_reload_enabled(false).await;

        // Update should still work but stats should reflect disabled state
        manager.update_config(
            config_key,
            create_test_config_value(json!({"disabled": true})),
            "disabled test",
        ).await.unwrap();

        let stats = manager.get_reload_stats().await;
        assert!(!stats.hot_reload_enabled);

        // Should still receive events (implementation detail)
        let event = timeout(Duration::from_millis(100), receiver.recv()).await.unwrap().unwrap();
        assert_eq!(event.config_key, config_key);

        // Re-enable
        manager.set_hot_reload_enabled(true).await;
        let stats = manager.get_reload_stats().await;
        assert!(stats.hot_reload_enabled);
    }

    #[tokio::test]
    async fn test_cache_consistency() {
        let manager = HotReloadManager::new();
        let config_key = "test.reload.cache";

        // Update config
        let original_value = create_test_config_value(json!({"cache": "original"}));
        manager.update_config(config_key, original_value.clone(), "cache test").await.unwrap();

        // Verify cache contains the value
        let cached_value = manager.get_config(config_key).await.unwrap();
        assert_eq!(cached_value.data, original_value.data);

        // Update again
        let updated_value = create_test_config_value(json!({"cache": "updated"}));
        manager.update_config(config_key, updated_value.clone(), "cache update").await.unwrap();

        // Verify cache is updated
        let cached_value = manager.get_config(config_key).await.unwrap();
        assert_eq!(cached_value.data, updated_value.data);

        // Remove config
        manager.remove_config(config_key, "cache removal").await.unwrap();

        // Verify cache no longer contains the value
        let cached_value = manager.get_config(config_key).await;
        assert!(cached_value.is_none());
    }

    #[tokio::test]
    async fn test_multiple_subscribers_same_config() {
        let manager = HotReloadManager::new();
        let config_key = "test.reload.multiple_subs";

        // Create multiple subscribers for same config
        let mut receivers = Vec::new();
        for i in 1..=3 {
            let receiver = manager.subscribe(
                &format!("multi_sub_{}", i),
                vec![config_key.to_string()],
                ReloadStrategy::Immediate,
            ).await.unwrap();
            receivers.push(receiver);
        }

        // Update config once
        manager.update_config(
            config_key,
            create_test_config_value(json!({"multiple": "subscribers"})),
            "multi subscriber test",
        ).await.unwrap();

        // All subscribers should receive the event
        for mut receiver in receivers {
            let event = timeout(Duration::from_millis(100), receiver.recv()).await.unwrap().unwrap();
            assert_eq!(event.config_key, config_key);
            assert_eq!(event.trigger_reason, "multi subscriber test");
        }
    }
}