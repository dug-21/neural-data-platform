//! Generated protobuf modules - TEMPORARILY DISABLED FOR TESTING
//!
//! This module contains all generated protobuf code from the build.rs process.

// Allow clippy warnings for generated code
#![allow(clippy::all)]

// TEMPORARY: Proto generation disabled for testing
// TODO: Re-enable after fixing build.rs proto generation

// Stub types for compilation
pub mod neural_trader {
    pub mod common {
        pub mod v1 {
            // Stub for testing
        }
    }
    pub mod market_data {
        pub mod v1 {
            // Stub for testing
        }
    }
    pub mod trading {
        pub mod v1 {
            // Stub for testing
        }
    }
    pub mod models {
        pub mod v1 {
            // Stub for testing
        }
    }
    pub mod features {
        pub mod v1 {
            // Stub for testing
        }
    }
}

pub mod schemas {
    pub mod ingestion {
        // Stub types
        #[derive(Debug, Clone)]
        pub struct EventEnvelope {}
    }
    pub mod mlops {
        // Stub for testing
    }
    pub mod execution {
        // Stub for testing
    }
    pub mod action {
        // Stub for testing
    }
}

// Re-export commonly used types
pub use schemas::ingestion::EventEnvelope;
