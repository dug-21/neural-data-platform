// AIR-004: Stream Registry for Multi-Stream Data Platform
//
// This module provides stream configuration management backed by etcd

pub mod registry;

pub use registry::StreamRegistry;
