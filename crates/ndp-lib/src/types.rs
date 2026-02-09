//! Shared types for sync operations.

use std::time::Duration;

/// Result of a sync operation.
///
/// Returned by all sync/mutation operations. The CLI formats this for
/// human output; the MCP server (future) returns it as JSON.
#[derive(Debug, Clone)]
pub struct SyncReport {
    /// Entity type synced (e.g. "dictionary", "dimension").
    pub entity: String,
    /// Total items processed from source.
    pub items_processed: usize,
    /// Items newly created in target.
    pub items_created: usize,
    /// Items updated in target.
    pub items_updated: usize,
    /// Items deleted from target.
    pub items_deleted: usize,
    /// Non-fatal errors encountered during sync.
    pub errors: Vec<SyncError>,
    /// Wall-clock duration of the sync operation.
    pub duration: Duration,
}

/// A non-fatal error encountered during sync.
#[derive(Debug, Clone)]
pub struct SyncError {
    /// The item that caused the error (e.g. stream ID, row number).
    pub item: String,
    /// Human-readable error message.
    pub message: String,
}

/// Options for sync operations.
#[derive(Debug, Clone)]
pub struct SyncOptions {
    /// If true, generate SQL but do not execute against the database.
    pub dry_run: bool,
    /// If true, run semantic validation before mutating operations.
    /// Defaults to `true` so that all sync/recreate paths validate
    /// the configuration before generating DDL.
    pub validate: bool,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            validate: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_options_default_has_validate_true() {
        let opts = SyncOptions::default();
        assert!(
            opts.validate,
            "SyncOptions::default() should have validate = true"
        );
        assert!(!opts.dry_run);
    }

    #[test]
    fn test_sync_options_validate_can_be_disabled() {
        let opts = SyncOptions {
            validate: false,
            ..Default::default()
        };
        assert!(!opts.validate, "SyncOptions should allow validate = false");
    }

    #[test]
    fn test_sync_options_all_fields() {
        let opts = SyncOptions {
            dry_run: true,
            validate: false,
        };
        assert!(opts.dry_run);
        assert!(!opts.validate);
    }
}
