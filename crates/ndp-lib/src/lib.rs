//! NDP Shared Operational Logic Library
//!
//! `ndp-lib` contains the business logic for NDP deployment operations.
//! It is consumed by the `ndp` CLI (thin wrapper) and, in the future,
//! by the MCP server.
//!
//! # Design Principles
//!
//! - **Library-first**: All operational logic lives here; consumers are thin wrappers.
//! - **Trait-based dependencies**: Functions take `&impl DbClient`, not concrete types.
//! - **Parsed structs, not file paths**: Functions accept data, not filesystem locations.
//! - **Structured output**: All mutations return `SyncReport` for consistent reporting.
//!
//! # Modules
//!
//! - [`db`] - Database client trait and PostgreSQL implementation
//! - [`config`] - Configuration loader trait and filesystem implementation
//! - [`dictionary`] - Dictionary sync logic (Phase B)
//! - [`dimension`] - Dimension sync logic (Phase C)
//! - [`error`] - Error types
//! - [`types`] - Shared types (SyncReport, SyncOptions)

pub mod config;
pub mod convert;
pub mod db;
pub mod dictionary;
pub mod dimension;
pub mod error;
pub mod types;

// Re-exports for convenience
pub use db::DbClient;
pub use error::{NdpLibError, Result};
pub use types::{SyncError, SyncOptions, SyncReport};
