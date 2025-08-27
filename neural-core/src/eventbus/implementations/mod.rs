/// EventBus implementations module
///
/// This module provides concrete implementations of the EventBus trait:
/// 
/// LEGACY (DEPRECATED - Phase 4 eliminates Vec<u8> support):
/// - `InMemoryEventBus`: Thread-safe in-memory implementation for testing
/// - `RecordingEventBus`: Wrapper that records all operations for testing
/// - `RedisEventBus`: Production implementation using Redis Streams
/// 
/// PROTO-ONLY (Phase 4 enforcement - MANDATORY for all new code):
/// - `ProtoInMemoryEventBus`: Proto-only in-memory implementation

pub mod inmemory;
pub mod recording;
pub mod redis;
pub mod proto_inmemory;

// Test modules
#[cfg(test)]
mod verification_test;

// DEPRECATED: Legacy implementations (use proto-only versions instead)
pub use inmemory::InMemoryEventBus;
pub use recording::RecordingEventBus;
pub use redis::RedisEventBus;

// Proto-only implementations (Phase 4 enforcement)
pub use proto_inmemory::ProtoInMemoryEventBus;