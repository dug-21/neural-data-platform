/// Configuration store implementations module
/// 
/// This module contains various implementations of the ConfigStore trait:
/// - `in_memory`: Fast in-memory store for testing and development
/// - `redis`: Distributed Redis-based store (when redis-backend feature is enabled)
/// - `file`: File-based store for simple deployments

pub mod in_memory;
pub mod secure_in_memory;

// #[cfg(feature = "redis-backend")]
// pub mod redis;

// Re-export commonly used implementations
pub use in_memory::InMemoryConfigStore;
pub use secure_in_memory::SecureInMemoryConfigStore;