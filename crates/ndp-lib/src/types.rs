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
#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    /// If true, generate SQL but do not execute against the database.
    pub dry_run: bool,
}
