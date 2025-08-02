//! Performance Optimization Module
//!
//! Comprehensive performance optimization for production deployment:
//! - Memory usage optimization (<50MB per symbol)
//! - Prediction latency optimization (<100ms)
//! - Resource management and lazy loading
//! - Performance monitoring and bottleneck analysis

pub mod optimizations;

pub use optimizations::{
    PerformanceOptimizer,
    OptimizationConfig,
    MemoryStats,
    PerformanceMetrics,
    PerformanceReport,
    GCResult,
    PerformanceOptimized,
};