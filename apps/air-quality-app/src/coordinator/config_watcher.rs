//! DP-021: Configuration Watcher for Hot-Reload
//!
//! This module provides integration between etcd configuration watches
//! and the SourceManager for hot-reload functionality.
//!
//! ## Architecture
//!
//! ```text
//! etcd watch ──> ConfigWatcher ──> SourceManager.on_config_change()
//!                     │
//!                     └──> Validates config
//!                     └──> Extracts stream_id from key
//!                     └──> Dispatches to hot-reload
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! let watcher = ConfigWatcher::new(source_manager);
//! watcher.start_watching(config_client).await?;
//! ```

use crate::coordinator::source_manager::{HotReloadResult, SourceManager};
use config_client::ConfigClient;
use neural_core::StreamConfig;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// DP-021: Configuration watcher that connects etcd watch to SourceManager
pub struct ConfigWatcher {
    source_manager: Arc<RwLock<SourceManager>>,
    cancel_token: CancellationToken,
    /// Track reload results for observability
    last_reload_results: Arc<RwLock<Vec<HotReloadResult>>>,
}

impl ConfigWatcher {
    /// Create a new ConfigWatcher
    pub fn new(source_manager: Arc<RwLock<SourceManager>>) -> Self {
        Self {
            source_manager,
            cancel_token: CancellationToken::new(),
            last_reload_results: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Start watching etcd for configuration changes
    ///
    /// This method spawns a background task that watches for changes to
    /// stream configurations in etcd and triggers hot-reload when changes
    /// are detected.
    ///
    /// # Arguments
    /// * `config_client` - The etcd config client to use for watching
    ///
    /// # Returns
    /// A handle that can be used to stop the watcher
    pub async fn start_watching(
        &self,
        config_client: Arc<ConfigClient>,
    ) -> Result<ConfigWatchHandle, ConfigWatchError> {
        let source_manager = self.source_manager.clone();
        let cancel_token = self.cancel_token.clone();
        let last_results = self.last_reload_results.clone();

        // The watch prefix for stream configs
        let watch_prefix = "/streams";

        info!(
            prefix = %watch_prefix,
            "Starting configuration watcher for hot-reload"
        );

        // Create watch using config_client's watch method
        let watch_handle = config_client
            .watch(watch_prefix, move |key, value| {
                // Extract stream_id from key like "/streams/air-quality/config"
                let stream_id = extract_stream_id(&key);

                if let Some(stream_id) = stream_id {
                    // Parse the config value
                    let config: Option<StreamConfig> =
                        value.and_then(|v| serde_json::from_value(v).ok());

                    debug!(
                        key = %key,
                        stream_id = %stream_id,
                        has_config = config.is_some(),
                        "Config change event received"
                    );

                    // Clone what we need for the async block
                    let sm = source_manager.clone();
                    let results = last_results.clone();
                    let sid = stream_id.clone();
                    let cfg = config;

                    // Spawn async task to handle the change
                    tokio::spawn(async move {
                        let mut manager = sm.write().await;
                        let result = manager.on_config_change(&sid, cfg).await;

                        // Store result for observability
                        let mut results_guard = results.write().await;
                        // Keep last 100 results
                        if results_guard.len() >= 100 {
                            results_guard.remove(0);
                        }
                        results_guard.push(result.clone());

                        if result.success {
                            info!(
                                stream_id = %sid,
                                sources_started = ?result.sources_started,
                                sources_stopped = ?result.sources_stopped,
                                duration_ms = result.duration_ms,
                                "Hot-reload completed successfully"
                            );
                        } else {
                            error!(
                                stream_id = %sid,
                                error = ?result.error,
                                "Hot-reload failed"
                            );
                        }
                    });
                } else {
                    debug!(
                        key = %key,
                        "Ignoring config change for non-stream key"
                    );
                }
            })
            .await
            .map_err(|e| ConfigWatchError::WatchFailed(e.to_string()))?;

        info!("Configuration watcher started");

        Ok(ConfigWatchHandle {
            _watch_handle: watch_handle,
            cancel_token,
        })
    }

    /// Get the last N reload results for observability
    pub async fn get_recent_results(&self, count: usize) -> Vec<HotReloadResult> {
        let results = self.last_reload_results.read().await;
        results.iter().rev().take(count).cloned().collect()
    }

    /// Stop the watcher
    pub fn stop(&self) {
        info!("Stopping configuration watcher");
        self.cancel_token.cancel();
    }
}

/// Handle for a running config watch
pub struct ConfigWatchHandle {
    _watch_handle: config_client::WatchHandle,
    cancel_token: CancellationToken,
}

impl ConfigWatchHandle {
    /// Stop watching
    pub async fn stop(self) {
        self.cancel_token.cancel();
    }
}

/// Errors from ConfigWatcher
#[derive(Debug, thiserror::Error)]
pub enum ConfigWatchError {
    #[error("Failed to start watch: {0}")]
    WatchFailed(String),

    #[error("Config client error: {0}")]
    ClientError(String),
}

/// Extract stream_id from an etcd key
///
/// Keys are expected to be in format: "/streams/{stream_id}/config"
fn extract_stream_id(key: &str) -> Option<String> {
    // Handle keys like "/streams/air-quality/config"
    let parts: Vec<&str> = key.trim_start_matches('/').split('/').collect();

    // We expect: ["streams", "{stream_id}", "config"]
    if parts.len() >= 3 && parts[0] == "streams" && parts[2] == "config" {
        Some(parts[1].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_stream_id_valid() {
        assert_eq!(
            extract_stream_id("/streams/air-quality/config"),
            Some("air-quality".to_string())
        );
        assert_eq!(
            extract_stream_id("/streams/outdoor-weather/config"),
            Some("outdoor-weather".to_string())
        );
        assert_eq!(
            extract_stream_id("/streams/test-123/config"),
            Some("test-123".to_string())
        );
    }

    #[test]
    fn test_extract_stream_id_invalid() {
        // Missing config suffix
        assert_eq!(extract_stream_id("/streams/air-quality"), None);
        // Wrong prefix
        assert_eq!(extract_stream_id("/configs/air-quality/config"), None);
        // Too short
        assert_eq!(extract_stream_id("/streams"), None);
        // Empty
        assert_eq!(extract_stream_id(""), None);
    }

    #[test]
    fn test_extract_stream_id_without_leading_slash() {
        // Should handle keys without leading slash
        assert_eq!(
            extract_stream_id("streams/air-quality/config"),
            Some("air-quality".to_string())
        );
    }
}
