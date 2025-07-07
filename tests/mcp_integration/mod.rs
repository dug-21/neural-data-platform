//! MCP Integration Tests Module

mod test_market_data;
mod test_cache_data;
mod test_predictions;
mod test_agent_decisions;
mod test_system_status;

// Re-export test utilities
pub use autonomous_platform::test_utils::*;