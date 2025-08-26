/// EventBus implementations module
///
/// This module provides concrete implementations of the EventBus trait:
/// - `InMemoryEventBus`: Thread-safe in-memory implementation for testing
/// - `RecordingEventBus`: Wrapper that records all operations for testing
/// - `RedisEventBus`: Production implementation using Redis Streams

pub mod inmemory;
pub mod recording;
pub mod redis;

// Test modules
#[cfg(test)]
mod verification_test;

pub use inmemory::InMemoryEventBus;
pub use recording::RecordingEventBus;
pub use redis::RedisEventBus;