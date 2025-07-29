//! Market-aware scheduling system for neural training
//!
//! This module provides intelligent scheduling capabilities that respect global market
//! hours, optimize resource allocation, and handle emergency overrides.

pub mod market_aware_scheduler;
pub mod resource_allocator;
pub mod schedule_optimizer;
pub mod global_coordinator;

// Re-export key types
pub use market_aware_scheduler::{MarketAwareScheduler, SchedulerConfig, ScheduleDecision};
pub use resource_allocator::{ResourceAllocator, AllocationStrategy, ResourcePool};
pub use schedule_optimizer::{ScheduleOptimizer, OptimizationCriteria};
pub use global_coordinator::{GlobalCoordinator, CoordinationMode};