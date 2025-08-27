# Redis Streams Channel Specification

## Executive Summary

This document specifies the complete Redis Streams channel architecture for the neural-trader platform. Redis Streams serve as the primary communication backbone, enabling real-time data flow between components while supporting backpressure handling, consumer groups, and guaranteed message delivery.

## Channel Architecture Overview

### Channel Hierarchy

```
┌─────────────────────────────────────────────────────────┐
│                 Redis Streams Architecture             │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌─────────────────┐  ┌─────────────────────────────┐   │
│  │ Symbol Channels │  │ Sector Aggregation Channels │   │
│  │ symbol/AAPL     │  │ sector/technology           │   │
│  │ symbol/MSFT     │  │ sector/financial            │   │
│  │ symbol/GOOGL    │  │ sector/healthcare           │   │
│  └─────────────────┘  └─────────────────────────────┘   │
│                                                         │
│  ┌─────────────────┐  ┌─────────────────────────────┐   │
│  │ Portfolio       │  │ Cross-Sector Analysis       │   │
│  │ Channels        │  │ Channels                    │   │
│  │ portfolio/      │  │ cross_sector/correlations   │   │
│  │ decisions       │  │ cross_sector/rotation       │   │
│  └─────────────────┘  └─────────────────────────────┘   │
│                                                         │
│  ┌─────────────────┐  ┌─────────────────────────────┐   │
│  │ ML Ops          │  │ Action Layer                │   │
│  │ Channels        │  │ Channels                    │   │
│  │ ml/training     │  │ action/executions           │   │
│  │ ml/inference    │  │ action/risk_events          │   │
│  └─────────────────┘  └─────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

## Channel Naming Conventions

### 1. Symbol-Level Data Channels

**Pattern**: `symbol/{SYMBOL_NAME}`

```
stream:symbol:AAPL         # Apple stock data
stream:symbol:MSFT         # Microsoft stock data  
stream:symbol:GOOGL        # Google stock data
stream:symbol:TSLA         # Tesla stock data
stream:symbol:NVDA         # NVIDIA stock data
```

### 2. Sector Aggregation Channels

**Pattern**: `sector/{SECTOR_NAME}`

```
stream:sector:technology           # XLK technology sector
stream:sector:financial            # XLF financial sector
stream:sector:healthcare           # XLV healthcare sector
stream:sector:energy               # XLE energy sector
stream:sector:consumer_discretionary # XLY consumer discretionary
stream:sector:consumer_staples     # XLP consumer staples
stream:sector:industrials          # XLI industrials sector
stream:sector:materials            # XLB materials sector
stream:sector:utilities            # XLU utilities sector
stream:sector:real_estate          # XLRE real estate sector
```

### 3. Portfolio Decision Channels

**Pattern**: `portfolio/{DECISION_TYPE}`

```
stream:portfolio:decisions         # Autonomous portfolio decisions
stream:portfolio:risk_metrics      # Risk assessment data
stream:portfolio:allocations       # Sector allocation updates
stream:portfolio:rebalancing       # Portfolio rebalancing events
```

### 4. Cross-Sector Analysis Channels

**Pattern**: `cross_sector/{ANALYSIS_TYPE}`

```
stream:cross_sector:correlations   # Inter-sector correlation matrices
stream:cross_sector:rotation       # Sector rotation analysis
stream:cross_sector:regime         # Market regime classification
stream:cross_sector:momentum       # Cross-sector momentum signals
```

### 5. ML Ops Channels

**Pattern**: `ml/{OPERATION_TYPE}`

```
stream:ml:training_requests        # Model training job requests
stream:ml:training_results         # Training completion notifications
stream:ml:model_updates           # New model version notifications
stream:ml:inference_requests      # Batch inference requests
stream:ml:performance_metrics     # Model performance tracking
```

### 6. Action Layer Channels

**Pattern**: `action/{ACTION_TYPE}`

```
stream:action:trade_executions     # Trade execution confirmations
stream:action:risk_violations     # Risk management alerts
stream:action:position_updates    # Position change notifications
stream:action:order_management    # Order lifecycle events
```

## Message Format Specifications

### Protocol Buffers Schema

All messages use Protocol Buffers for efficient serialization:

```proto
// File: proto/streaming_messages.proto
syntax = "proto3";

package neural_trader.streaming;

import "google/protobuf/timestamp.proto";
import "google/protobuf/any.proto";

// Base message envelope for all stream messages
message StreamMessage {
  string message_id = 1;              // Unique message identifier
  string message_type = 2;            // Message type discriminator
  string source_service = 3;          // Originating service
  google.protobuf.Timestamp timestamp = 4;  // Message creation time
  string channel = 5;                 // Target channel
  google.protobuf.Any payload = 6;    // Strongly typed payload
  map<string, string> metadata = 7;   // Additional metadata
  string correlation_id = 8;          // Request correlation
}

// Market data message for symbol channels
message MarketDataMessage {
  string symbol = 1;
  double price = 2;
  double volume = 3;
  double bid = 4;
  double ask = 5;
  double open = 6;
  double high = 7;
  double low = 8;
  double close = 9;
  int64 timestamp = 10;
  map<string, double> technical_indicators = 11;
}

// Sector aggregation message
message SectorAggregationMessage {
  string sector_id = 1;
  string etf_symbol = 2;
  double etf_price = 3;
  double avg_price = 4;
  double total_volume = 5;
  double volatility = 6;
  double momentum = 7;
  int32 constituent_count = 8;
  repeated string constituent_symbols = 9;
  map<string, double> correlation_matrix = 10;
  SectorMetrics metrics = 11;
}

// Portfolio decision message
message PortfolioDecisionMessage {
  string decision_id = 1;
  DecisionType decision_type = 2;
  map<string, double> sector_allocations = 3;
  RiskMetrics risk_metrics = 4;
  double consensus_score = 5;
  double confidence = 6;
  string reasoning = 7;
  repeated string contributing_models = 8;
}

// ML Ops training request
message TrainingRequestMessage {
  string training_id = 1;
  string model_type = 2;
  TrainingConfig config = 3;
  DatasetReference dataset = 4;
  map<string, string> hyperparameters = 5;
  TrainingPriority priority = 6;
}

// Action layer execution message
message ActionExecutionMessage {
  string execution_id = 1;
  ActionType action_type = 2;
  string symbol = 3;
  TradeDirection direction = 4;
  double quantity = 5;
  double price = 6;
  ExecutionStatus status = 7;
  string error_message = 8;
}

// Supporting types
enum DecisionType {
  BUY = 0;
  SELL = 1;
  HOLD = 2;
  REBALANCE = 3;
}

enum TradeDirection {
  LONG = 0;
  SHORT = 1;
}

enum ExecutionStatus {
  PENDING = 0;
  EXECUTED = 1;
  FAILED = 2;
  CANCELLED = 3;
}

enum TrainingPriority {
  LOW = 0;
  NORMAL = 1;
  HIGH = 2;
  URGENT = 3;
}

message RiskMetrics {
  double var = 1;                    // Value at Risk
  double max_drawdown = 2;           // Maximum drawdown
  double sharpe_ratio = 3;           // Sharpe ratio
  double concentration_risk = 4;     // Concentration risk
  double correlation_risk = 5;       // Cross-sector correlation risk
}

message SectorMetrics {
  double beta = 1;
  double alpha = 2;
  double tracking_error = 3;
  double information_ratio = 4;
  double market_cap_weight = 5;
}
```

## Subscription Patterns

### 1. Consumer Group Configuration

Each service subscribes using consumer groups for load balancing:

```rust
// Consumer group patterns
pub struct ConsumerGroupConfig {
    pub group_name: String,
    pub consumer_name: String,
    pub start_id: String,      // "0" for beginning, "$" for new messages
    pub block_time_ms: u64,    // Blocking timeout
    pub count: usize,          // Messages per read
    pub ack_timeout_ms: u64,   // Message acknowledgment timeout
}

// Service-specific consumer groups
let consumer_groups = vec![
    // Trading Domain
    ConsumerGroupConfig {
        group_name: "trading-domain".to_string(),
        consumer_name: format!("trader-{}", instance_id),
        start_id: "0".to_string(),
        block_time_ms: 1000,
        count: 10,
        ack_timeout_ms: 30000,
    },
    
    // ML Ops Platform  
    ConsumerGroupConfig {
        group_name: "mlops-platform".to_string(),
        consumer_name: format!("mlops-{}", instance_id),
        start_id: "0".to_string(),
        block_time_ms: 5000,
        count: 5,
        ack_timeout_ms: 300000, // 5 minutes for training jobs
    },
    
    // Data Ingestion
    ConsumerGroupConfig {
        group_name: "data-ingestion".to_string(),
        consumer_name: format!("ingester-{}", instance_id),
        start_id: "$".to_string(), // Only new messages
        block_time_ms: 100,
        count: 50,
        ack_timeout_ms: 5000,
    },
];
```

### 2. Multi-Channel Subscription

```rust
// Multi-channel subscription for related data
pub struct MultiChannelSubscriber {
    redis_client: Arc<AsyncRedis>,
    subscriptions: HashMap<String, ChannelSubscription>,
    message_router: MessageRouter,
}

impl MultiChannelSubscriber {
    pub async fn subscribe_to_channels(
        &mut self,
        channel_patterns: &[ChannelPattern],
    ) -> Result<()> {
        for pattern in channel_patterns {
            match pattern {
                ChannelPattern::SymbolGroup(symbols) => {
                    for symbol in symbols {
                        let channel = format!("stream:symbol:{}", symbol);
                        self.subscribe_to_channel(&channel, ConsumerType::SymbolProcessor).await?;
                    }
                }
                
                ChannelPattern::SectorGroup(sectors) => {
                    for sector in sectors {
                        let channel = format!("stream:sector:{}", sector);
                        self.subscribe_to_channel(&channel, ConsumerType::SectorAggregator).await?;
                    }
                }
                
                ChannelPattern::AllPortfolioChannels => {
                    let portfolio_channels = vec![
                        "stream:portfolio:decisions",
                        "stream:portfolio:risk_metrics",
                        "stream:portfolio:allocations",
                    ];
                    
                    for channel in portfolio_channels {
                        self.subscribe_to_channel(channel, ConsumerType::PortfolioManager).await?;
                    }
                }
                
                ChannelPattern::CrossSectorAnalysis => {
                    let cross_sector_channels = vec![
                        "stream:cross_sector:correlations",
                        "stream:cross_sector:rotation",
                        "stream:cross_sector:regime",
                    ];
                    
                    for channel in cross_sector_channels {
                        self.subscribe_to_channel(channel, ConsumerType::CrossSectorAnalyzer).await?;
                    }
                }
            }
        }
        
        Ok(())
    }
}
```

## Backpressure Handling

### 1. Flow Control Mechanisms

```rust
/// Backpressure handling for high-throughput channels
pub struct BackpressureController {
    channel_limits: HashMap<String, ChannelLimits>,
    current_loads: HashMap<String, LoadMetrics>,
    throttling_policies: HashMap<String, ThrottlingPolicy>,
}

#[derive(Debug, Clone)]
pub struct ChannelLimits {
    pub max_pending_messages: usize,
    pub max_memory_usage_mb: usize,
    pub max_consumer_lag_ms: u64,
    pub warning_threshold: f64,     // 0.0 to 1.0
    pub critical_threshold: f64,    // 0.0 to 1.0
}

impl BackpressureController {
    pub async fn check_backpressure(&mut self, channel: &str) -> Result<BackpressureStatus> {
        let limits = self.channel_limits.get(channel)
            .ok_or_else(|| Error::ChannelNotConfigured(channel.to_string()))?;
        
        let current_load = self.measure_channel_load(channel).await?;
        self.current_loads.insert(channel.to_string(), current_load.clone());
        
        // Calculate pressure metrics
        let message_pressure = current_load.pending_messages as f64 / limits.max_pending_messages as f64;
        let memory_pressure = current_load.memory_usage_mb as f64 / limits.max_memory_usage_mb as f64;
        let lag_pressure = current_load.consumer_lag_ms as f64 / limits.max_consumer_lag_ms as f64;
        
        let overall_pressure = message_pressure.max(memory_pressure).max(lag_pressure);
        
        // Determine status and apply throttling if needed
        let status = if overall_pressure >= limits.critical_threshold {
            self.apply_critical_throttling(channel).await?;
            BackpressureStatus::Critical
        } else if overall_pressure >= limits.warning_threshold {
            self.apply_warning_throttling(channel).await?;
            BackpressureStatus::Warning
        } else {
            self.clear_throttling(channel).await?;
            BackpressureStatus::Normal
        };
        
        Ok(status)
    }
    
    async fn apply_critical_throttling(&self, channel: &str) -> Result<()> {
        warn!("Applying critical throttling to channel: {}", channel);
        
        // Reduce producer rate
        self.set_producer_rate_limit(channel, 0.25).await?;  // 25% of normal rate
        
        // Increase consumer parallelism
        self.scale_consumers(channel, ScalingAction::ScaleUp(2)).await?;
        
        // Enable message batching
        self.enable_batching(channel, BatchSize::Large).await?;
        
        Ok(())
    }
    
    async fn apply_warning_throttling(&self, channel: &str) -> Result<()> {
        info!("Applying warning throttling to channel: {}", channel);
        
        // Moderate rate limiting
        self.set_producer_rate_limit(channel, 0.75).await?;  // 75% of normal rate
        
        // Enable batching
        self.enable_batching(channel, BatchSize::Medium).await?;
        
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct LoadMetrics {
    pub pending_messages: usize,
    pub memory_usage_mb: usize,
    pub consumer_lag_ms: u64,
    pub message_rate_per_sec: f64,
    pub error_rate: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum BackpressureStatus {
    Normal,
    Warning,
    Critical,
}
```

### 2. Message Batching

```rust
/// Message batching for efficient throughput
pub struct MessageBatcher {
    batch_configs: HashMap<String, BatchConfig>,
    pending_batches: HashMap<String, PendingBatch>,
    flush_timers: HashMap<String, Instant>,
}

#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub max_batch_size: usize,
    pub max_wait_time_ms: u64,
    pub compression_enabled: bool,
    pub ordering_required: bool,
}

impl MessageBatcher {
    pub async fn add_message_to_batch(
        &mut self,
        channel: &str,
        message: StreamMessage,
    ) -> Result<Option<Vec<StreamMessage>>> {
        let config = self.batch_configs.get(channel)
            .ok_or_else(|| Error::BatchConfigNotFound(channel.to_string()))?;
        
        // Get or create pending batch
        let batch = self.pending_batches.entry(channel.to_string())
            .or_insert_with(|| PendingBatch::new(config.clone()));
        
        batch.add_message(message);
        
        // Check if batch should be flushed
        let should_flush = batch.messages.len() >= config.max_batch_size ||
                          self.batch_wait_time_exceeded(channel, config)?;
        
        if should_flush {
            let messages = batch.flush();
            self.flush_timers.remove(channel);
            Ok(Some(messages))
        } else {
            // Set flush timer if not already set
            if !self.flush_timers.contains_key(channel) {
                self.flush_timers.insert(channel.to_string(), Instant::now());
            }
            Ok(None)
        }
    }
}

#[derive(Debug)]
struct PendingBatch {
    messages: Vec<StreamMessage>,
    config: BatchConfig,
    created_at: Instant,
}

impl PendingBatch {
    fn new(config: BatchConfig) -> Self {
        Self {
            messages: Vec::new(),
            config,
            created_at: Instant::now(),
        }
    }
    
    fn add_message(&mut self, message: StreamMessage) {
        self.messages.push(message);
    }
    
    fn flush(&mut self) -> Vec<StreamMessage> {
        let messages = std::mem::take(&mut self.messages);
        self.created_at = Instant::now();
        messages
    }
}
```

## Dead Letter Queues

### 1. Message Failure Handling

```rust
/// Dead letter queue implementation for failed messages
pub struct DeadLetterQueue {
    redis_client: Arc<AsyncRedis>,
    dlq_config: DLQConfig,
    retry_policies: HashMap<String, RetryPolicy>,
}

#[derive(Debug, Clone)]
pub struct DLQConfig {
    pub max_retries: usize,
    pub retry_delay_base_ms: u64,
    pub retry_delay_multiplier: f64,
    pub dlq_retention_hours: u64,
    pub enable_poison_message_detection: bool,
}

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub backoff_strategy: BackoffStrategy,
    pub retry_conditions: Vec<RetryCondition>,
}

#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    Linear(u64),           // Linear backoff with fixed interval
    Exponential(f64),      // Exponential backoff with multiplier
    Fixed(u64),            // Fixed delay
}

impl DeadLetterQueue {
    pub async fn handle_failed_message(
        &mut self,
        channel: &str,
        message: &StreamMessage,
        error: &ProcessingError,
    ) -> Result<MessageDisposition> {
        let retry_policy = self.retry_policies.get(channel)
            .cloned()
            .unwrap_or_default();
        
        // Get current retry count
        let retry_count = self.get_retry_count(&message.message_id).await?;
        
        // Check if should retry
        if retry_count < retry_policy.max_attempts && self.should_retry(error, &retry_policy) {
            // Schedule retry
            let retry_delay = self.calculate_retry_delay(&retry_policy, retry_count);
            self.schedule_retry(channel, message, retry_delay).await?;
            
            Ok(MessageDisposition::Retry {
                attempt: retry_count + 1,
                delay_ms: retry_delay,
            })
        } else {
            // Send to dead letter queue
            self.send_to_dlq(channel, message, error, retry_count).await?;
            
            Ok(MessageDisposition::DeadLetter {
                reason: format!("Max retries ({}) exceeded", retry_policy.max_attempts),
                final_error: error.clone(),
            })
        }
    }
    
    async fn send_to_dlq(
        &self,
        channel: &str,
        message: &StreamMessage,
        error: &ProcessingError,
        retry_count: usize,
    ) -> Result<()> {
        let dlq_channel = format!("dlq:{}", channel);
        
        let dlq_message = DLQMessage {
            original_message: message.clone(),
            original_channel: channel.to_string(),
            failure_reason: error.to_string(),
            retry_count,
            failed_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(self.dlq_config.dlq_retention_hours as i64),
        };
        
        // Serialize and add to DLQ stream
        let serialized = prost::Message::encode_to_vec(&dlq_message)?;
        self.redis_client.xadd(
            &dlq_channel,
            "*",
            &[("data", serialized)],
        ).await?;
        
        warn!(
            "Message {} sent to DLQ after {} retries: {}",
            message.message_id, retry_count, error
        );
        
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum MessageDisposition {
    Retry { attempt: usize, delay_ms: u64 },
    DeadLetter { reason: String, final_error: ProcessingError },
}
```

## Performance Expectations

### 1. Throughput Benchmarks

```rust
/// Performance expectations by channel type
pub struct ChannelPerformanceSpec {
    pub channel_pattern: String,
    pub expected_throughput_msgs_per_sec: u64,
    pub max_latency_p99_ms: u64,
    pub memory_usage_per_1k_msgs_mb: f64,
    pub consumer_group_scaling: ScalingSpec,
}

let performance_specs = vec![
    // High-frequency symbol channels
    ChannelPerformanceSpec {
        channel_pattern: "stream:symbol:*".to_string(),
        expected_throughput_msgs_per_sec: 10000,  // 10K msgs/sec per symbol
        max_latency_p99_ms: 50,                   // 50ms P99 latency
        memory_usage_per_1k_msgs_mb: 2.5,        // 2.5MB per 1K messages
        consumer_group_scaling: ScalingSpec {
            min_consumers: 2,
            max_consumers: 8,
            scale_up_threshold: 0.8,
            scale_down_threshold: 0.3,
        },
    },
    
    // Medium-frequency sector channels
    ChannelPerformanceSpec {
        channel_pattern: "stream:sector:*".to_string(),
        expected_throughput_msgs_per_sec: 1000,   // 1K msgs/sec per sector
        max_latency_p99_ms: 100,                  // 100ms P99 latency
        memory_usage_per_1k_msgs_mb: 5.0,        // 5MB per 1K messages
        consumer_group_scaling: ScalingSpec {
            min_consumers: 1,
            max_consumers: 4,
            scale_up_threshold: 0.7,
            scale_down_threshold: 0.2,
        },
    },
    
    // Low-frequency portfolio decision channels
    ChannelPerformanceSpec {
        channel_pattern: "stream:portfolio:*".to_string(),
        expected_throughput_msgs_per_sec: 100,    // 100 msgs/sec
        max_latency_p99_ms: 500,                  // 500ms P99 latency
        memory_usage_per_1k_msgs_mb: 10.0,       // 10MB per 1K messages
        consumer_group_scaling: ScalingSpec {
            min_consumers: 1,
            max_consumers: 2,
            scale_up_threshold: 0.6,
            scale_down_threshold: 0.1,
        },
    },
    
    // ML Ops training channels (batch processing)
    ChannelPerformanceSpec {
        channel_pattern: "stream:ml:*".to_string(),
        expected_throughput_msgs_per_sec: 10,     // 10 msgs/sec (large payloads)
        max_latency_p99_ms: 5000,                 // 5 seconds P99 latency
        memory_usage_per_1k_msgs_mb: 100.0,      // 100MB per 1K messages
        consumer_group_scaling: ScalingSpec {
            min_consumers: 1,
            max_consumers: 3,
            scale_up_threshold: 0.9,
            scale_down_threshold: 0.1,
        },
    },
];
```

### 2. Monitoring and Alerting

```rust
/// Performance monitoring for Redis Streams
pub struct StreamPerformanceMonitor {
    metrics_collector: MetricsCollector,
    alert_manager: AlertManager,
    performance_specs: HashMap<String, ChannelPerformanceSpec>,
}

impl StreamPerformanceMonitor {
    pub async fn monitor_channel_performance(&mut self) -> Result<()> {
        for (pattern, spec) in &self.performance_specs {
            let channels = self.get_matching_channels(pattern).await?;
            
            for channel in channels {
                let metrics = self.collect_channel_metrics(&channel).await?;
                
                // Check throughput
                if metrics.messages_per_second < spec.expected_throughput_msgs_per_sec as f64 * 0.8 {
                    self.alert_manager.send_alert(Alert::ThroughputBelowExpected {
                        channel: channel.clone(),
                        actual: metrics.messages_per_second,
                        expected: spec.expected_throughput_msgs_per_sec as f64,
                    }).await?;
                }
                
                // Check latency
                if metrics.latency_p99_ms > spec.max_latency_p99_ms {
                    self.alert_manager.send_alert(Alert::LatencyAboveThreshold {
                        channel: channel.clone(),
                        actual_ms: metrics.latency_p99_ms,
                        threshold_ms: spec.max_latency_p99_ms,
                    }).await?;
                }
                
                // Check memory usage
                let memory_per_1k = metrics.memory_usage_mb / (metrics.message_count / 1000.0);
                if memory_per_1k > spec.memory_usage_per_1k_msgs_mb * 1.5 {
                    self.alert_manager.send_alert(Alert::MemoryUsageHigh {
                        channel: channel.clone(),
                        actual_mb_per_1k: memory_per_1k,
                        expected_mb_per_1k: spec.memory_usage_per_1k_msgs_mb,
                    }).await?;
                }
                
                // Store metrics for trending
                self.metrics_collector.record_channel_metrics(&channel, &metrics).await?;
            }
        }
        
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ChannelMetrics {
    pub channel_name: String,
    pub messages_per_second: f64,
    pub latency_p50_ms: u64,
    pub latency_p90_ms: u64,
    pub latency_p99_ms: u64,
    pub memory_usage_mb: f64,
    pub message_count: f64,
    pub error_rate: f64,
    pub consumer_lag_ms: u64,
    pub active_consumers: usize,
    pub timestamp: DateTime<Utc>,
}
```

## Code Examples - Pub/Sub Implementation

### 1. Message Publisher

```rust
// File: src/streaming/publisher.rs
use crate::streaming::messages::*;

pub struct StreamPublisher {
    redis_client: Arc<AsyncRedis>,
    message_batcher: MessageBatcher,
    compression: CompressionConfig,
    metrics: PublisherMetrics,
}

impl StreamPublisher {
    pub async fn publish_market_data(
        &mut self,
        symbol: &str,
        market_data: MarketData,
    ) -> Result<String> {
        let channel = format!("stream:symbol:{}", symbol);
        
        // Create protobuf message
        let market_msg = MarketDataMessage {
            symbol: symbol.to_string(),
            price: market_data.close,
            volume: market_data.volume,
            bid: market_data.bid.unwrap_or(0.0),
            ask: market_data.ask.unwrap_or(0.0),
            open: market_data.open,
            high: market_data.high,
            low: market_data.low,
            close: market_data.close,
            timestamp: market_data.timestamp,
            technical_indicators: market_data.indicators.unwrap_or_default(),
        };
        
        // Wrap in stream envelope
        let stream_message = StreamMessage {
            message_id: Uuid::new_v4().to_string(),
            message_type: "MarketData".to_string(),
            source_service: "data-ingestion".to_string(),
            timestamp: Some(prost_types::Timestamp::from(SystemTime::now())),
            channel: channel.clone(),
            payload: Some(prost_types::Any::from_msg(&market_msg)?),
            metadata: hashmap!{
                "symbol".to_string() => symbol.to_string(),
                "exchange".to_string() => market_data.exchange.unwrap_or_default(),
            },
            correlation_id: "".to_string(),
        };
        
        // Add to batch or publish immediately
        if let Some(batch) = self.message_batcher.add_message_to_batch(&channel, stream_message).await? {
            self.publish_batch(&channel, &batch).await
        } else {
            Ok("batched".to_string())
        }
    }
    
    pub async fn publish_sector_data(
        &mut self,
        sector: &SectorId,
        sector_data: SectorData,
    ) -> Result<String> {
        let channel = format!("stream:sector:{}", sector.as_str());
        
        let sector_msg = SectorAggregationMessage {
            sector_id: sector.as_str().to_string(),
            etf_symbol: sector_data.etf_symbol,
            etf_price: sector_data.etf_price,
            avg_price: sector_data.avg_price,
            total_volume: sector_data.total_volume,
            volatility: sector_data.volatility,
            momentum: sector_data.momentum,
            constituent_count: sector_data.symbols_count as i32,
            constituent_symbols: sector_data.symbols,
            correlation_matrix: sector_data.correlation_matrix,
            metrics: Some(SectorMetrics {
                beta: 1.0,  // Would be calculated
                alpha: 0.0,
                tracking_error: 0.02,
                information_ratio: 0.5,
                market_cap_weight: 0.15,
            }),
        };
        
        let stream_message = StreamMessage {
            message_id: Uuid::new_v4().to_string(),
            message_type: "SectorAggregation".to_string(),
            source_service: "sector-aggregator".to_string(),
            timestamp: Some(prost_types::Timestamp::from(SystemTime::now())),
            channel: channel.clone(),
            payload: Some(prost_types::Any::from_msg(&sector_msg)?),
            metadata: hashmap!{
                "sector".to_string() => sector.as_str().to_string(),
                "constituent_count".to_string() => sector_data.symbols_count.to_string(),
            },
            correlation_id: "".to_string(),
        };
        
        self.publish_message(&channel, &stream_message).await
    }
    
    async fn publish_message(&mut self, channel: &str, message: &StreamMessage) -> Result<String> {
        let start_time = Instant::now();
        
        // Serialize message
        let serialized = prost::Message::encode_to_vec(message)?;
        
        // Compress if configured
        let data = if self.compression.enabled {
            self.compress_data(&serialized)?
        } else {
            serialized
        };
        
        // Publish to Redis Stream
        let message_id: String = self.redis_client
            .xadd(channel, "*", &[("data", data)])
            .await?;
        
        // Record metrics
        let latency = start_time.elapsed();
        self.metrics.record_publish(
            channel,
            serialized.len(),
            latency,
        ).await?;
        
        debug!(
            "Published message {} to channel {} ({}ms)",
            message.message_id,
            channel,
            latency.as_millis()
        );
        
        Ok(message_id)
    }
}
```

### 2. Message Consumer

```rust
// File: src/streaming/consumer.rs
use crate::streaming::messages::*;

pub struct StreamConsumer {
    redis_client: Arc<AsyncRedis>,
    consumer_config: ConsumerConfig,
    message_handlers: HashMap<String, Box<dyn MessageHandler>>,
    dlq: DeadLetterQueue,
    metrics: ConsumerMetrics,
}

impl StreamConsumer {
    pub async fn start_consuming(&mut self) -> Result<()> {
        info!("Starting stream consumer: {}", self.consumer_config.consumer_name);
        
        // Create consumer group if it doesn't exist
        for channel in &self.consumer_config.channels {
            let _result = self.redis_client
                .xgroup_create(channel, &self.consumer_config.group_name, "0", true)
                .await; // Ignore errors if group already exists
        }
        
        loop {
            match self.consume_messages().await {
                Ok(_) => {},
                Err(e) => {
                    error!("Consumer error: {}", e);
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                }
            }
        }
    }
    
    async fn consume_messages(&mut self) -> Result<()> {
        let channels: Vec<&str> = self.consumer_config.channels.iter().map(|s| s.as_str()).collect();
        
        // Read from multiple channels
        let results: HashMap<String, Vec<StreamId>> = self.redis_client
            .xreadgroup(
                &self.consumer_config.group_name,
                &self.consumer_config.consumer_name,
                &channels,
                &vec![">"; channels.len()], // Read new messages
                Some(self.consumer_config.count),
                Some(self.consumer_config.block_time_ms),
            )
            .await?;
        
        // Process messages from each channel
        for (channel, messages) in results {
            for stream_message in messages {
                self.process_stream_message(&channel, &stream_message).await?;
            }
        }
        
        Ok(())
    }
    
    async fn process_stream_message(
        &mut self,
        channel: &str,
        stream_message: &StreamId,
    ) -> Result<()> {
        let start_time = Instant::now();
        
        // Extract message data
        let data = stream_message.fields.get("data")
            .ok_or_else(|| Error::MissingMessageData)?;
        
        // Deserialize stream message
        let message: StreamMessage = prost::Message::decode(data.as_bytes())?;
        
        // Route message to appropriate handler
        let processing_result = if let Some(handler) = self.message_handlers.get(&message.message_type) {
            handler.handle_message(&message).await
        } else {
            warn!("No handler for message type: {}", message.message_type);
            Ok(MessageProcessingResult::Ignored)
        };
        
        // Handle processing result
        match processing_result {
            Ok(MessageProcessingResult::Success) => {
                // Acknowledge successful processing
                self.redis_client
                    .xack(channel, &self.consumer_config.group_name, &[&stream_message.id])
                    .await?;
                    
                self.metrics.record_successful_processing(
                    channel,
                    &message.message_type,
                    start_time.elapsed(),
                ).await?;
            }
            
            Ok(MessageProcessingResult::Ignored) => {
                // Acknowledge but don't count as success
                self.redis_client
                    .xack(channel, &self.consumer_config.group_name, &[&stream_message.id])
                    .await?;
            }
            
            Err(processing_error) => {
                // Handle failure with DLQ
                let disposition = self.dlq.handle_failed_message(
                    channel,
                    &message,
                    &processing_error,
                ).await?;
                
                match disposition {
                    MessageDisposition::Retry { .. } => {
                        // Don't acknowledge - message will be retried
                        self.metrics.record_retry(channel, &message.message_type).await?;
                    }
                    MessageDisposition::DeadLetter { .. } => {
                        // Acknowledge to remove from pending
                        self.redis_client
                            .xack(channel, &self.consumer_config.group_name, &[&stream_message.id])
                            .await?;
                        self.metrics.record_dead_letter(channel, &message.message_type).await?;
                    }
                }
            }
        }
        
        Ok(())
    }
}

/// Message handler trait for different message types
#[async_trait]
pub trait MessageHandler: Send + Sync {
    async fn handle_message(&self, message: &StreamMessage) -> Result<MessageProcessingResult>;
}

#[derive(Debug)]
pub enum MessageProcessingResult {
    Success,
    Ignored,
}

/// Market data message handler
pub struct MarketDataHandler {
    data_processor: Arc<dyn MarketDataProcessor>,
    neural_predictor: Arc<dyn NeuralPredictor>,
}

#[async_trait]
impl MessageHandler for MarketDataHandler {
    async fn handle_message(&self, message: &StreamMessage) -> Result<MessageProcessingResult> {
        // Extract and deserialize market data
        let payload = message.payload.as_ref()
            .ok_or_else(|| Error::MissingPayload)?;
        let market_data: MarketDataMessage = payload.to_msg()?;
        
        // Process market data
        let processed_data = self.data_processor
            .process_market_data(&market_data)
            .await?;
        
        // Trigger neural prediction if conditions are met
        if self.should_trigger_prediction(&market_data)? {
            let _prediction = self.neural_predictor
                .predict(&processed_data)
                .await?;
        }
        
        Ok(MessageProcessingResult::Success)
    }
}
```

## Redis Streams Configuration Summary

The Redis Streams architecture provides:

1. **Scalable Messaging**: High-throughput message delivery with consumer group load balancing
2. **Reliable Delivery**: Message persistence and acknowledgment-based processing
3. **Backpressure Management**: Intelligent throttling and batching to handle load spikes  
4. **Fault Tolerance**: Dead letter queues and retry mechanisms for failed messages
5. **Performance Monitoring**: Comprehensive metrics and alerting for operational visibility
6. **Protocol Buffer Efficiency**: Strongly typed, compact message serialization
7. **Multi-Channel Coordination**: Coordinated consumption across related channels

This architecture ensures reliable, high-performance real-time data flow throughout the neural-trader platform while maintaining operational excellence and fault tolerance.