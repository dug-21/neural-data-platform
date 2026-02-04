//! Database connectivity module for ndp-gold-ddl
//!
//! Provides database abstraction for checking continuous aggregate existence.
//! Uses traits for mockability in unit tests.

pub mod client;
pub mod queries;

pub use client::{DbClient, DbError, PostgresClient};
pub use queries::{CaChecker, CaInfo, PostgresCaChecker};
