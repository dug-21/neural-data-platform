/// Configuration store implementations module
/// 
/// This module contains various implementations of the ConfigStore trait:
/// - `in_memory`: Fast in-memory store for testing and development
/// - `redis`: Distributed Redis-based store (when redis-backend feature is enabled)
/// - `file`: File-based store for simple deployments

pub mod in_memory;
pub mod secure_in_memory;
pub mod redis;

#[cfg(test)]
mod redis_test;

// Re-export commonly used implementations
pub use in_memory::InMemoryConfigStore;
pub use secure_in_memory::SecureInMemoryConfigStore;
pub use redis::RedisConfigStore;