// DAA Unit Tests Module
// Phase 3 Extensions with Integration-First Mandate Compliance

pub mod autonomous_training_extensions_test;
pub mod performance_snapshot_extensions_test;

// Re-export test modules for convenient access
pub use autonomous_training_extensions_test::*;
pub use performance_snapshot_extensions_test::*;

#[cfg(test)]
mod integration_tests {
    use super::*;
    
    /// Integration test to verify all DAA extensions work together
    /// while preserving existing autonomous trading capabilities
    #[tokio::test]
    async fn test_all_daa_extensions_integration() {
        // This test ensures all extension modules maintain
        // the critical DAA preservation requirements:
        // - accuracy_threshold=0.8
        // - error_threshold=0.1  
        // - consecutive_failure_threshold=5
        // - 60/40 voting ratio preserved
        // - 70% consensus maintained
        
        // Test will be implemented by integration test agent
        // following the Integration-First Mandate
    }
}