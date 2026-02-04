//! Planning module for Gold layer DDL deployment
//!
//! The planner coordinates between DDL generation and database state,
//! determining what actually needs to be created or updated.

pub mod sync;

pub use sync::{SyncPlan, SyncPlanner, CaAction};
