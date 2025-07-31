//! Test suite for health monitoring implementation
//! 
//! This module contains comprehensive tests following TDD principles
//! to ensure the health monitoring system meets all requirements.

#[cfg(test)]
mod mcp_server_panic_fix_test;

#[cfg(test)]
mod async_health_monitor_test;

#[cfg(test)]
mod health_server_test;

#[cfg(test)]
mod component_health_checks_test;

#[cfg(test)]
mod integration_test;

// Re-export test utilities for use in other test modules
#[cfg(test)]
pub mod test_utils {
    use std::time::Duration;
    
    /// Default test timeout for health checks
    pub const DEFAULT_TEST_TIMEOUT: Duration = Duration::from_secs(5);
    
    /// Test database URL (can be overridden by environment variable)
    pub fn test_database_url() -> String {
        std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://localhost/neural_trader_test".to_string())
    }
    
    /// Test Redis URL (can be overridden by environment variable)
    pub fn test_redis_url() -> String {
        std::env::var("TEST_REDIS_URL")
            .unwrap_or_else(|_| "redis://localhost:6379".to_string())
    }
    
    /// Check if integration tests should be skipped (e.g., in CI without dependencies)
    pub fn should_skip_integration_tests() -> bool {
        std::env::var("SKIP_INTEGRATION_TESTS").is_ok()
    }
}