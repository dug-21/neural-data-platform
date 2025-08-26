/// EventBus controllers for flow control and reliability
///
/// This module provides supporting systems for the EventBus:
/// - `BackpressureController`: Manages channel pressure and throttling
/// - `MessageBatcher`: Batches messages for efficient throughput
/// - `DeadLetterQueue`: Handles failed message processing with retry logic

pub mod backpressure;
pub mod batching;
pub mod dlq;

pub use backpressure::{BackpressureController, BackpressureStatus, ChannelLimits};
pub use batching::{MessageBatcher, BatchConfig, BatchDisposition};
pub use dlq::{DeadLetterQueue, DLQConfig, MessageDisposition, RetryPolicy};