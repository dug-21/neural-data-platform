//! Shared constants for Gold DDL generators
//!
//! Single source of truth for schema names, column names, and other
//! values that are repeated across multiple generator files.

/// Default entity identifier column used across NDP streams.
pub const NDP_ENTITY_COLUMN: &str = "ndp_id";

/// Gold schema name. All Gold layer objects are created in this schema.
pub const GOLD_SCHEMA: &str = "gold";

/// Silver schema name. All Silver layer tables live here.
pub const SILVER_SCHEMA: &str = "silver";
