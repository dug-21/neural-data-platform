//! Distributed tracing for request flow analysis
//! 
//! This module provides distributed tracing capabilities for:
//! - Request flow tracking across services
//! - Performance bottleneck identification  
//! - Error correlation and debugging
//! - Business transaction monitoring

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, info_span, Instrument, Span};
use uuid::Uuid;

/// Distributed tracer for request and operation tracking
#[derive(Clone)]
pub struct DistributedTracer {
    config: TracingConfig,
    active_traces: Arc<RwLock<HashMap<String, TraceContext>>>,
    span_processor: Arc<SpanProcessor>,
}

impl DistributedTracer {
    pub fn new(config: TracingConfig) -> Result<Self> {
        Ok(Self {
            config,
            active_traces: Arc::new(RwLock::new(HashMap::new())),
            span_processor: Arc::new(SpanProcessor::new()),
        })
    }

    /// Start a new trace for a business operation
    pub async fn start_trace(&self, operation: &str, metadata: TraceMetadata) -> TraceContext {
        let trace_id = Uuid::new_v4().to_string();
        let span_id = Uuid::new_v4().to_string();
        
        let context = TraceContext {
            trace_id: trace_id.clone(),
            span_id,
            parent_span_id: None,
            operation: operation.to_string(),
            metadata,
            start_time: Instant::now(),
            status: TraceStatus::Active,
            tags: HashMap::new(),
            child_spans: Vec::new(),
        };

        // Store active trace
        let mut active_traces = self.active_traces.write().await;
        active_traces.insert(trace_id.clone(), context.clone());

        info!("Started trace: {} for operation: {}", trace_id, operation);
        context
    }

    /// Start a child span within an existing trace
    pub async fn start_span(&self, parent_context: &TraceContext, operation: &str) -> TraceContext {
        let span_id = Uuid::new_v4().to_string();
        
        let context = TraceContext {
            trace_id: parent_context.trace_id.clone(),
            span_id,
            parent_span_id: Some(parent_context.span_id.clone()),
            operation: operation.to_string(),
            metadata: parent_context.metadata.clone(),
            start_time: Instant::now(),
            status: TraceStatus::Active,
            tags: HashMap::new(),
            child_spans: Vec::new(),
        };

        // Update parent trace with child span
        let mut active_traces = self.active_traces.write().await;
        if let Some(parent_trace) = active_traces.get_mut(&parent_context.trace_id) {
            parent_trace.add_child_span(context.clone());
        }

        context
    }

    /// Finish a trace or span
    pub async fn finish_trace(&self, mut context: TraceContext, status: TraceStatus) -> Result<()> {
        context.status = status;
        let duration = context.start_time.elapsed();

        // Process the completed span
        self.span_processor.process_span(&context, duration).await?;

        // Remove from active traces if it's a root trace
        if context.parent_span_id.is_none() {
            let mut active_traces = self.active_traces.write().await;
            active_traces.remove(&context.trace_id);
        }

        info!(
            "Finished trace: {} operation: {} duration: {:?} status: {:?}",
            context.trace_id, context.operation, duration, context.status
        );

        Ok(())
    }

    /// Add tags to a trace context
    pub fn add_tags(&self, context: &mut TraceContext, tags: HashMap<String, String>) {
        context.tags.extend(tags);
    }

    /// Get active traces count
    pub async fn get_active_traces_count(&self) -> usize {
        self.active_traces.read().await.len()
    }

    /// Get trace by ID
    pub async fn get_trace(&self, trace_id: &str) -> Option<TraceContext> {
        self.active_traces.read().await.get(trace_id).cloned()
    }
}

/// Trace context containing all tracing information
#[derive(Debug, Clone)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub operation: String,
    pub metadata: TraceMetadata,
    pub start_time: Instant,
    pub status: TraceStatus,
    pub tags: HashMap<String, String>,
    child_spans: Vec<TraceContext>,
}

impl TraceContext {
    /// Add a child span to this trace context
    fn add_child_span(&mut self, child: TraceContext) {
        self.child_spans.push(child);
    }

    /// Get all child spans
    pub fn get_child_spans(&self) -> &[TraceContext] {
        &self.child_spans
    }

    /// Create a tracing span for this context
    pub fn create_tracing_span(&self) -> Span {
        info_span!(
            "operation",
            trace_id = %self.trace_id,
            span_id = %self.span_id,
            parent_span_id = ?self.parent_span_id,
            operation = %self.operation,
            user_id = ?self.metadata.user_id,
            request_id = ?self.metadata.request_id,
        )
    }
}

/// Metadata associated with a trace
#[derive(Debug, Clone)]
pub struct TraceMetadata {
    pub user_id: Option<String>,
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub source_service: String,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
}

impl Default for TraceMetadata {
    fn default() -> Self {
        Self {
            user_id: None,
            request_id: None,
            session_id: None,
            source_service: "neural-trader".to_string(),
            client_ip: None,
            user_agent: None,
        }
    }
}

/// Status of a trace
#[derive(Debug, Clone, PartialEq)]
pub enum TraceStatus {
    Active,
    Success,
    Error(String),
    Cancelled,
}

/// Configuration for distributed tracing
#[derive(Debug, Clone)]
pub struct TracingConfig {
    pub enabled: bool,
    pub sample_rate: f64,
    pub max_span_duration: Duration,
    pub export_traces: bool,
    pub trace_endpoint: Option<String>,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sample_rate: 1.0,
            max_span_duration: Duration::from_secs(300), // 5 minutes
            export_traces: false,
            trace_endpoint: None,
        }
    }
}

/// Processes completed spans for analysis and export
pub struct SpanProcessor {
    completed_spans: Arc<RwLock<Vec<CompletedSpan>>>,
}

impl SpanProcessor {
    pub fn new() -> Self {
        Self {
            completed_spans: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Process a completed span
    pub async fn process_span(&self, context: &TraceContext, duration: Duration) -> Result<()> {
        let completed_span = CompletedSpan {
            trace_id: context.trace_id.clone(),
            span_id: context.span_id.clone(),
            parent_span_id: context.parent_span_id.clone(),
            operation: context.operation.clone(),
            duration,
            status: context.status.clone(),
            tags: context.tags.clone(),
            metadata: context.metadata.clone(),
            timestamp: chrono::Utc::now(),
        };

        // Store completed span
        let mut spans = self.completed_spans.write().await;
        spans.push(completed_span);

        // Keep only last 10000 spans to prevent memory bloat
        if spans.len() > 10000 {
            spans.drain(0..1000);
        }

        Ok(())
    }

    /// Get completed spans for analysis
    pub async fn get_completed_spans(&self) -> Vec<CompletedSpan> {
        self.completed_spans.read().await.clone()
    }

    /// Analyze spans for performance insights
    pub async fn analyze_performance(&self) -> PerformanceAnalysis {
        let spans = self.completed_spans.read().await;
        let total_spans = spans.len();
        
        if total_spans == 0 {
            return PerformanceAnalysis::default();
        }

        let mut operation_stats: HashMap<String, OperationStats> = HashMap::new();
        let mut total_duration = Duration::ZERO;
        let mut error_count = 0;

        for span in spans.iter() {
            total_duration += span.duration;
            
            if matches!(span.status, TraceStatus::Error(_)) {
                error_count += 1;
            }

            let stats = operation_stats.entry(span.operation.clone()).or_insert_with(OperationStats::default);
            stats.count += 1;
            stats.total_duration += span.duration;
            
            if span.duration > stats.max_duration {
                stats.max_duration = span.duration;
            }
            
            if stats.min_duration.is_zero() || span.duration < stats.min_duration {
                stats.min_duration = span.duration;
            }

            if matches!(span.status, TraceStatus::Error(_)) {
                stats.error_count += 1;
            }
        }

        // Calculate averages
        for stats in operation_stats.values_mut() {
            stats.avg_duration = stats.total_duration / stats.count as u32;
            stats.error_rate = stats.error_count as f64 / stats.count as f64;
        }

        PerformanceAnalysis {
            total_spans,
            total_duration,
            avg_duration: total_duration / total_spans as u32,
            error_rate: error_count as f64 / total_spans as f64,
            operation_stats,
        }
    }
}

/// Completed span information
#[derive(Debug, Clone)]
pub struct CompletedSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub operation: String,
    pub duration: Duration,
    pub status: TraceStatus,
    pub tags: HashMap<String, String>,
    pub metadata: TraceMetadata,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Statistics for a specific operation
#[derive(Debug, Default)]
pub struct OperationStats {
    pub count: usize,
    pub total_duration: Duration,
    pub avg_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub error_count: usize,
    pub error_rate: f64,
}

/// Performance analysis results
#[derive(Debug, Default)]
pub struct PerformanceAnalysis {
    pub total_spans: usize,
    pub total_duration: Duration,
    pub avg_duration: Duration,
    pub error_rate: f64,
    pub operation_stats: HashMap<String, OperationStats>,
}

/// Business operation tracer for specific trading operations
pub struct BusinessOperationTracer {
    tracer: DistributedTracer,
}

impl BusinessOperationTracer {
    pub fn new(tracer: DistributedTracer) -> Self {
        Self { tracer }
    }

    /// Trace a prediction operation
    pub async fn trace_prediction<F, T>(&self, model_name: &str, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        let metadata = TraceMetadata {
            source_service: "neural-predictor".to_string(),
            ..Default::default()
        };

        let mut context = self.tracer.start_trace("model_prediction", metadata).await;
        context.tags.insert("model_name".to_string(), model_name.to_string());

        let span = context.create_tracing_span();
        
        let result = operation.instrument(span).await;
        
        let status = match &result {
            Ok(_) => TraceStatus::Success,
            Err(e) => TraceStatus::Error(e.to_string()),
        };

        self.tracer.finish_trace(context, status).await?;
        result
    }

    /// Trace a trading operation
    pub async fn trace_trading<F, T>(&self, symbol: &str, action: &str, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        let metadata = TraceMetadata {
            source_service: "trading-engine".to_string(),
            ..Default::default()
        };

        let mut context = self.tracer.start_trace("trading_operation", metadata).await;
        context.tags.insert("symbol".to_string(), symbol.to_string());
        context.tags.insert("action".to_string(), action.to_string());

        let span = context.create_tracing_span();
        
        let result = operation.instrument(span).await;
        
        let status = match &result {
            Ok(_) => TraceStatus::Success,
            Err(e) => TraceStatus::Error(e.to_string()),
        };

        self.tracer.finish_trace(context, status).await?;
        result
    }

    /// Trace data processing operation
    pub async fn trace_data_processing<F, T>(&self, source: &str, operation: F) -> Result<T>
    where
        F: std::future::Future<Output = Result<T>>,
    {
        let metadata = TraceMetadata {
            source_service: "data-processor".to_string(),
            ..Default::default()
        };

        let mut context = self.tracer.start_trace("data_processing", metadata).await;
        context.tags.insert("data_source".to_string(), source.to_string());

        let span = context.create_tracing_span();
        
        let result = operation.instrument(span).await;
        
        let status = match &result {
            Ok(_) => TraceStatus::Success,
            Err(e) => TraceStatus::Error(e.to_string()),
        };

        self.tracer.finish_trace(context, status).await?;
        result
    }
}

impl Default for SpanProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_trace_lifecycle() {
        let tracer = DistributedTracer::new(TracingConfig::default()).unwrap();
        let metadata = TraceMetadata::default();
        
        let context = tracer.start_trace("test_operation", metadata).await;
        assert_eq!(context.operation, "test_operation");
        assert_eq!(context.status, TraceStatus::Active);
        
        tracer.finish_trace(context, TraceStatus::Success).await.unwrap();
    }

    #[tokio::test]
    async fn test_child_span() {
        let tracer = DistributedTracer::new(TracingConfig::default()).unwrap();
        let metadata = TraceMetadata::default();
        
        let parent_context = tracer.start_trace("parent_operation", metadata).await;
        let child_context = tracer.start_span(&parent_context, "child_operation").await;
        
        assert_eq!(child_context.trace_id, parent_context.trace_id);
        assert_eq!(child_context.parent_span_id, Some(parent_context.span_id.clone()));
        
        tracer.finish_trace(child_context, TraceStatus::Success).await.unwrap();
        tracer.finish_trace(parent_context, TraceStatus::Success).await.unwrap();
    }
}