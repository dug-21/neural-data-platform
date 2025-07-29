# Comprehensive Market Data Monitoring System Design

## Executive Summary

This document presents a comprehensive market data monitoring system designed to ensure continuous, high-quality data flow for the neural trader. The system features real-time health checks, intelligent alerting, automatic recovery mechanisms, and deep integration with Grafana for visualization. The design emphasizes proactive monitoring with 30-second no-data detection during trading hours and sophisticated latency tracking per symbol.

## System Architecture Overview

### Core Components

1. **Market Data Monitor Service** (Rust)
   - Real-time data flow validation
   - Symbol-specific latency tracking
   - Trading hours awareness
   - Data quality assessment

2. **Alert Engine** (Rust)
   - Immediate notification system
   - Escalation procedures
   - Trading hours aware alerting
   - Multi-channel alert delivery

3. **Recovery Coordinator** (Rust)
   - Automatic reconnection logic
   - Fallback data source management
   - Graceful degradation strategies
   - Circuit breaker implementation

4. **Metrics Collector** (Rust)
   - High-frequency metrics collection
   - Symbol-level granularity
   - Performance baselines
   - Anomaly detection

## 1. Data Flow Health Checks

### 1.1 Real-time Data Arrival Monitoring

```rust
// src/monitoring/market_data_monitor.rs
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use futures::stream::{Stream, StreamExt};

pub struct MarketDataMonitor {
    symbol_last_update: Arc<RwLock<HashMap<String, Instant>>>,
    symbol_latencies: Arc<RwLock<HashMap<String, VecDeque<Duration>>>>,
    health_status: Arc<RwLock<HashMap<String, HealthStatus>>>,
    trading_hours: TradingHoursChecker,
    alert_manager: Arc<AlertManager>,
}

#[derive(Clone, Debug)]
pub struct HealthStatus {
    pub is_healthy: bool,
    pub last_update: Instant,
    pub avg_latency_ms: f64,
    pub messages_per_second: f64,
    pub data_quality_score: f64,
    pub consecutive_misses: u32,
}

impl MarketDataMonitor {
    pub async fn monitor_data_flow(&self, data_stream: impl Stream<Item = MarketData>) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        
        tokio::spawn({
            let monitor = self.clone();
            async move {
                let mut stream = data_stream.fuse();
                
                loop {
                    tokio::select! {
                        Some(data) = stream.next() => {
                            monitor.process_market_data(data).await;
                        }
                        _ = interval.tick() => {
                            monitor.check_data_health().await;
                        }
                    }
                }
            }
        });
    }
    
    async fn process_market_data(&self, data: MarketData) {
        let arrival_time = Instant::now();
        let symbol = &data.symbol;
        
        // Update last seen time
        {
            let mut last_updates = self.symbol_last_update.write().await;
            if let Some(last_time) = last_updates.get(symbol) {
                let latency = arrival_time.duration_since(*last_time);
                self.record_latency(symbol, latency).await;
            }
            last_updates.insert(symbol.clone(), arrival_time);
        }
        
        // Validate data quality
        let quality_score = self.validate_data_quality(&data).await;
        
        // Update health status
        self.update_health_status(symbol, true, quality_score).await;
        
        // Record metrics
        self.record_metrics(&data, quality_score).await;
    }
    
    async fn check_data_health(&self) {
        let now = Instant::now();
        let is_trading_hours = self.trading_hours.is_market_open().await;
        
        let last_updates = self.symbol_last_update.read().await;
        let mut alerts = Vec::new();
        
        for (symbol, last_update) in last_updates.iter() {
            let time_since_update = now.duration_since(*last_update);
            
            // 30-second no-data detection during trading hours
            if is_trading_hours && time_since_update > Duration::from_secs(30) {
                alerts.push(DataAlert {
                    symbol: symbol.clone(),
                    alert_type: AlertType::NoDataReceived,
                    severity: AlertSeverity::Critical,
                    duration: time_since_update,
                    message: format!("No data received for {} seconds", time_since_update.as_secs()),
                });
                
                self.update_health_status(symbol, false, 0.0).await;
            } else if time_since_update > Duration::from_secs(60) {
                // More lenient during non-trading hours
                alerts.push(DataAlert {
                    symbol: symbol.clone(),
                    alert_type: AlertType::NoDataReceived,
                    severity: AlertSeverity::Warning,
                    duration: time_since_update,
                    message: format!("No data for {} (non-trading hours)", symbol),
                });
            }
        }
        
        // Send alerts
        for alert in alerts {
            self.alert_manager.send_alert(alert).await;
        }
    }
}
```

### 1.2 Latency Tracking per Symbol

```rust
// src/monitoring/latency_tracker.rs
use std::collections::VecDeque;

const LATENCY_WINDOW_SIZE: usize = 1000; // Keep last 1000 latency measurements

impl MarketDataMonitor {
    async fn record_latency(&self, symbol: &str, latency: Duration) {
        let mut latencies = self.symbol_latencies.write().await;
        
        let symbol_latencies = latencies
            .entry(symbol.to_string())
            .or_insert_with(|| VecDeque::with_capacity(LATENCY_WINDOW_SIZE));
        
        // Maintain sliding window
        if symbol_latencies.len() >= LATENCY_WINDOW_SIZE {
            symbol_latencies.pop_front();
        }
        symbol_latencies.push_back(latency);
        
        // Calculate percentiles
        let p50 = self.calculate_percentile(&symbol_latencies, 0.50);
        let p95 = self.calculate_percentile(&symbol_latencies, 0.95);
        let p99 = self.calculate_percentile(&symbol_latencies, 0.99);
        
        // Record metrics
        histogram!("market_data_latency_seconds", 
            "symbol" => symbol,
            "percentile" => "p50"
        ).record(p50.as_secs_f64());
        
        histogram!("market_data_latency_seconds",
            "symbol" => symbol,
            "percentile" => "p95"
        ).record(p95.as_secs_f64());
        
        histogram!("market_data_latency_seconds",
            "symbol" => symbol,
            "percentile" => "p99"
        ).record(p99.as_secs_f64());
        
        // Alert on high latency
        if p95 > Duration::from_millis(100) {
            self.alert_manager.send_alert(DataAlert {
                symbol: symbol.to_string(),
                alert_type: AlertType::HighLatency,
                severity: AlertSeverity::Warning,
                duration: p95,
                message: format!("P95 latency {}ms exceeds threshold", p95.as_millis()),
            }).await;
        }
    }
    
    fn calculate_percentile(&self, latencies: &VecDeque<Duration>, percentile: f64) -> Duration {
        if latencies.is_empty() {
            return Duration::ZERO;
        }
        
        let mut sorted: Vec<Duration> = latencies.iter().cloned().collect();
        sorted.sort();
        
        let index = ((sorted.len() as f64 - 1.0) * percentile) as usize;
        sorted[index]
    }
}
```

### 1.3 Data Quality Validation

```rust
// src/monitoring/data_quality.rs
#[derive(Debug, Clone)]
pub struct DataQualityValidator {
    price_bounds: HashMap<String, PriceBounds>,
    volume_thresholds: HashMap<String, VolumeThreshold>,
    sequence_tracker: HashMap<String, u64>,
}

#[derive(Debug, Clone)]
struct PriceBounds {
    min_price: f64,
    max_price: f64,
    max_change_percent: f64,
}

impl MarketDataMonitor {
    async fn validate_data_quality(&self, data: &MarketData) -> f64 {
        let mut quality_score = 1.0;
        let mut issues = Vec::new();
        
        // Check for stale timestamps
        let data_age = SystemTime::now()
            .duration_since(data.timestamp)
            .unwrap_or(Duration::ZERO);
            
        if data_age > Duration::from_secs(5) {
            quality_score *= 0.5;
            issues.push("Stale timestamp");
        }
        
        // Validate price ranges
        if let Some(bounds) = self.get_price_bounds(&data.symbol).await {
            if data.price < bounds.min_price || data.price > bounds.max_price {
                quality_score *= 0.3;
                issues.push("Price out of bounds");
            }
        }
        
        // Check for missing fields
        if data.volume == 0.0 && self.trading_hours.is_market_open().await {
            quality_score *= 0.8;
            issues.push("Zero volume during trading hours");
        }
        
        // Validate bid/ask spread
        if let (Some(bid), Some(ask)) = (data.bid, data.ask) {
            let spread_percent = ((ask - bid) / bid) * 100.0;
            if spread_percent > 1.0 { // 1% spread threshold
                quality_score *= 0.7;
                issues.push("Wide bid/ask spread");
            }
        }
        
        // Check sequence numbers if available
        if let Some(seq) = data.sequence_number {
            if let Some(last_seq) = self.get_last_sequence(&data.symbol).await {
                if seq <= last_seq {
                    quality_score = 0.0; // Duplicate or out-of-order
                    issues.push("Sequence number issue");
                }
            }
            self.update_sequence(&data.symbol, seq).await;
        }
        
        // Log quality issues
        if quality_score < 1.0 {
            warn!("Data quality issues for {}: {:?}, score: {}", 
                data.symbol, issues, quality_score);
            
            counter!("market_data_quality_issues",
                "symbol" => data.symbol.clone(),
                "issues" => issues.join(",")
            ).increment(1);
        }
        
        gauge!("market_data_quality_score",
            "symbol" => data.symbol.clone()
        ).set(quality_score);
        
        quality_score
    }
}
```

## 2. Alert Mechanisms

### 2.1 Alert Engine Architecture

```rust
// src/monitoring/alert_engine.rs
use tokio::sync::mpsc;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub enum AlertType {
    NoDataReceived,
    HighLatency,
    DataQualityIssue,
    ConnectionLost,
    RecoveryInProgress,
    SystemDegraded,
}

#[derive(Debug, Clone)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
    Emergency,
}

#[derive(Debug, Clone)]
pub struct DataAlert {
    pub symbol: String,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub duration: Duration,
    pub message: String,
    pub timestamp: Instant,
    pub metadata: HashMap<String, String>,
}

pub struct AlertManager {
    alert_sender: mpsc::Sender<DataAlert>,
    grafana_client: GrafanaAlertClient,
    pagerduty_client: Option<PagerDutyClient>,
    slack_client: Option<SlackClient>,
    email_client: Option<EmailClient>,
    active_alerts: Arc<RwLock<HashSet<String>>>,
    escalation_rules: EscalationRules,
}

impl AlertManager {
    pub async fn start_alert_processor(&self) {
        let mut alert_receiver = self.alert_sender.subscribe();
        
        while let Some(alert) = alert_receiver.recv().await {
            // Deduplication
            let alert_key = format!("{}:{}:{:?}", 
                alert.symbol, alert.alert_type, alert.severity);
                
            let is_new_alert = {
                let mut active = self.active_alerts.write().await;
                active.insert(alert_key.clone())
            };
            
            if !is_new_alert && alert.severity != AlertSeverity::Emergency {
                continue; // Skip duplicate non-emergency alerts
            }
            
            // Process alert based on severity
            match alert.severity {
                AlertSeverity::Info => {
                    self.log_alert(&alert).await;
                    self.send_to_grafana(&alert).await;
                }
                AlertSeverity::Warning => {
                    self.log_alert(&alert).await;
                    self.send_to_grafana(&alert).await;
                    if let Some(slack) = &self.slack_client {
                        slack.send_alert(&alert).await;
                    }
                }
                AlertSeverity::Critical => {
                    self.log_alert(&alert).await;
                    self.send_to_grafana(&alert).await;
                    self.send_to_all_channels(&alert).await;
                    self.trigger_recovery(&alert).await;
                }
                AlertSeverity::Emergency => {
                    self.log_alert(&alert).await;
                    self.send_to_grafana(&alert).await;
                    self.send_to_all_channels(&alert).await;
                    self.page_on_call(&alert).await;
                    self.trigger_emergency_recovery(&alert).await;
                }
            }
            
            // Start escalation timer if needed
            if alert.severity >= AlertSeverity::Critical {
                self.start_escalation_timer(alert).await;
            }
        }
    }
}
```

### 2.2 Grafana Integration

```rust
// src/monitoring/grafana_integration.rs
pub struct GrafanaAlertClient {
    api_url: String,
    api_key: String,
    client: reqwest::Client,
}

impl GrafanaAlertClient {
    pub async fn send_alert(&self, alert: &DataAlert) -> Result<()> {
        // Create Grafana annotation
        let annotation = json!({
            "dashboardUID": "market-data-monitoring",
            "panelId": self.get_panel_id_for_symbol(&alert.symbol),
            "time": alert.timestamp.elapsed().as_millis() as i64,
            "timeEnd": 0,
            "tags": [
                format!("severity:{:?}", alert.severity),
                format!("type:{:?}", alert.alert_type),
                format!("symbol:{}", alert.symbol),
            ],
            "text": alert.message,
        });
        
        self.client
            .post(&format!("{}/api/annotations", self.api_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&annotation)
            .send()
            .await?;
        
        // Update alert state in Grafana
        self.update_alert_state(&alert).await?;
        
        Ok(())
    }
    
    async fn update_alert_state(&self, alert: &DataAlert) -> Result<()> {
        let state = match alert.severity {
            AlertSeverity::Critical | AlertSeverity::Emergency => "alerting",
            AlertSeverity::Warning => "pending",
            _ => "ok",
        };
        
        let alert_rule = json!({
            "uid": format!("market-data-{}", alert.symbol),
            "state": state,
            "annotations": {
                "description": alert.message,
                "runbook_url": self.get_runbook_url(&alert.alert_type),
            },
            "labels": {
                "symbol": alert.symbol,
                "severity": format!("{:?}", alert.severity),
            }
        });
        
        self.client
            .post(&format!("{}/api/v1/provisioning/alert-rules", self.api_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&alert_rule)
            .send()
            .await?;
            
        Ok(())
    }
}
```

### 2.3 Trading Hours Aware Alerting

```rust
// src/monitoring/trading_hours.rs
use chrono::{DateTime, Utc, Weekday, Timelike};

pub struct TradingHoursChecker {
    market_calendar: MarketCalendar,
    timezone: chrono_tz::Tz,
}

impl TradingHoursChecker {
    pub async fn is_market_open(&self) -> bool {
        let now = Utc::now().with_timezone(&self.timezone);
        
        // Check if it's a weekend
        match now.weekday() {
            Weekday::Sat | Weekday::Sun => return false,
            _ => {}
        }
        
        // Check if it's a holiday
        if self.market_calendar.is_holiday(&now.date()).await {
            return false;
        }
        
        // Check regular trading hours (9:30 AM - 4:00 PM ET)
        let hour = now.hour();
        let minute = now.minute();
        let time_minutes = hour * 60 + minute;
        
        // Regular hours: 9:30 AM (570 min) to 4:00 PM (960 min)
        let is_regular_hours = time_minutes >= 570 && time_minutes <= 960;
        
        // Extended hours: 4:00 AM (240 min) to 8:00 PM (1200 min)
        let is_extended_hours = time_minutes >= 240 && time_minutes <= 1200;
        
        // Return based on configuration
        if self.market_calendar.include_extended_hours {
            is_extended_hours
        } else {
            is_regular_hours
        }
    }
    
    pub async fn get_alert_threshold(&self) -> Duration {
        if self.is_market_open().await {
            Duration::from_secs(30) // 30 seconds during trading
        } else {
            Duration::from_secs(300) // 5 minutes outside trading
        }
    }
}
```

### 2.4 Escalation Procedures

```rust
// src/monitoring/escalation.rs
pub struct EscalationRules {
    escalation_levels: Vec<EscalationLevel>,
    active_escalations: Arc<RwLock<HashMap<String, EscalationState>>>,
}

#[derive(Clone)]
struct EscalationLevel {
    delay: Duration,
    severity_threshold: AlertSeverity,
    actions: Vec<EscalationAction>,
}

#[derive(Clone)]
enum EscalationAction {
    NotifyTeam(String),
    PageOnCall,
    ExecuteRunbook(String),
    TriggerFailover,
    NotifyManagement,
}

impl AlertManager {
    async fn start_escalation_timer(&self, alert: DataAlert) {
        let escalation_key = format!("{}:{:?}", alert.symbol, alert.alert_type);
        
        tokio::spawn({
            let rules = self.escalation_rules.clone();
            let manager = self.clone();
            
            async move {
                for (level_idx, level) in rules.escalation_levels.iter().enumerate() {
                    // Wait for escalation delay
                    tokio::time::sleep(level.delay).await;
                    
                    // Check if alert is still active
                    if !manager.is_alert_active(&escalation_key).await {
                        break;
                    }
                    
                    // Execute escalation actions
                    for action in &level.actions {
                        match action {
                            EscalationAction::NotifyTeam(team) => {
                                manager.notify_team(team, &alert).await;
                            }
                            EscalationAction::PageOnCall => {
                                manager.page_on_call(&alert).await;
                            }
                            EscalationAction::ExecuteRunbook(runbook) => {
                                manager.execute_runbook(runbook, &alert).await;
                            }
                            EscalationAction::TriggerFailover => {
                                manager.trigger_failover(&alert).await;
                            }
                            EscalationAction::NotifyManagement => {
                                manager.notify_management(&alert).await;
                            }
                        }
                    }
                    
                    info!("Escalated alert to level {}: {:?}", level_idx + 1, alert);
                }
            }
        });
    }
}
```

## 3. Recovery Triggers

### 3.1 Automatic Reconnection

```rust
// src/monitoring/recovery_coordinator.rs
use tokio::time::{timeout, sleep};
use tokio_retry::{Retry, strategy::{ExponentialBackoff, jitter}};

pub struct RecoveryCoordinator {
    connection_manager: Arc<ConnectionManager>,
    fallback_sources: Vec<DataSource>,
    circuit_breaker: CircuitBreaker,
    recovery_strategies: HashMap<AlertType, Box<dyn RecoveryStrategy>>,
}

#[async_trait]
trait RecoveryStrategy: Send + Sync {
    async fn execute(&self, context: &RecoveryContext) -> Result<RecoveryOutcome>;
}

pub struct AutoReconnectStrategy {
    max_retries: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
}

#[async_trait]
impl RecoveryStrategy for AutoReconnectStrategy {
    async fn execute(&self, context: &RecoveryContext) -> Result<RecoveryOutcome> {
        let backoff = ExponentialBackoff::from_millis(self.initial_backoff.as_millis() as u64)
            .max_delay(self.max_backoff)
            .map(jitter);
        
        let result = Retry::spawn(backoff.take(self.max_retries as usize), || async {
            info!("Attempting reconnection to {} (attempt {})", 
                context.data_source.name, context.attempt_number);
            
            // Try to establish connection
            match context.connection_manager.connect(&context.data_source).await {
                Ok(connection) => {
                    // Validate connection with test query
                    if connection.test_connection().await.is_ok() {
                        info!("Successfully reconnected to {}", context.data_source.name);
                        return Ok(RecoveryOutcome::Success);
                    }
                }
                Err(e) => {
                    warn!("Reconnection failed: {}", e);
                }
            }
            
            Err(anyhow!("Connection attempt failed"))
        }).await;
        
        match result {
            Ok(outcome) => Ok(outcome),
            Err(_) => {
                error!("All reconnection attempts failed for {}", context.data_source.name);
                Ok(RecoveryOutcome::Failed)
            }
        }
    }
}
```

### 3.2 Fallback Data Sources

```rust
// src/monitoring/fallback_sources.rs
pub struct FallbackSourceManager {
    primary_source: DataSource,
    fallback_sources: Vec<DataSource>,
    source_health: Arc<RwLock<HashMap<String, SourceHealth>>>,
    active_source: Arc<RwLock<String>>,
}

#[derive(Clone)]
struct SourceHealth {
    is_healthy: bool,
    last_check: Instant,
    success_rate: f64,
    average_latency: Duration,
    priority: u8,
}

impl FallbackSourceManager {
    pub async fn handle_source_failure(&self, failed_source: &str) -> Result<()> {
        // Mark source as unhealthy
        {
            let mut health = self.source_health.write().await;
            if let Some(source_health) = health.get_mut(failed_source) {
                source_health.is_healthy = false;
                source_health.last_check = Instant::now();
            }
        }
        
        // Find best available fallback
        let best_fallback = self.find_best_fallback().await?;
        
        // Switch to fallback source
        self.switch_to_source(&best_fallback).await?;
        
        // Start recovery monitoring for failed source
        self.start_recovery_monitoring(failed_source).await;
        
        Ok(())
    }
    
    async fn find_best_fallback(&self) -> Result<DataSource> {
        let health = self.source_health.read().await;
        
        let mut available_sources: Vec<_> = self.fallback_sources
            .iter()
            .filter_map(|source| {
                health.get(&source.name).and_then(|h| {
                    if h.is_healthy {
                        Some((source, h))
                    } else {
                        None
                    }
                })
            })
            .collect();
        
        // Sort by priority and health metrics
        available_sources.sort_by(|a, b| {
            let score_a = a.1.priority as f64 * a.1.success_rate / a.1.average_latency.as_secs_f64();
            let score_b = b.1.priority as f64 * b.1.success_rate / b.1.average_latency.as_secs_f64();
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        available_sources
            .first()
            .map(|(source, _)| (*source).clone())
            .ok_or_else(|| anyhow!("No healthy fallback sources available"))
    }
    
    async fn switch_to_source(&self, source: &DataSource) -> Result<()> {
        info!("Switching to fallback source: {}", source.name);
        
        // Update connection
        self.connection_manager.switch_source(source).await?;
        
        // Update active source
        *self.active_source.write().await = source.name.clone();
        
        // Send notification
        self.alert_manager.send_alert(DataAlert {
            symbol: "SYSTEM".to_string(),
            alert_type: AlertType::RecoveryInProgress,
            severity: AlertSeverity::Info,
            duration: Duration::ZERO,
            message: format!("Switched to fallback source: {}", source.name),
            timestamp: Instant::now(),
            metadata: HashMap::new(),
        }).await;
        
        Ok(())
    }
}
```

### 3.3 Graceful Degradation

```rust
// src/monitoring/graceful_degradation.rs
pub struct DegradationManager {
    degradation_levels: Vec<DegradationLevel>,
    current_level: Arc<RwLock<usize>>,
    feature_flags: Arc<RwLock<FeatureFlags>>,
}

#[derive(Clone)]
struct DegradationLevel {
    name: String,
    threshold: f64, // System health score threshold
    disabled_features: Vec<Feature>,
    reduced_frequencies: HashMap<String, Duration>,
    message: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Feature {
    ExtendedHoursData,
    HighFrequencyUpdates,
    OptionsData,
    NewsFeeds,
    SocialSentiment,
    AlternativeData,
}

impl DegradationManager {
    pub async fn evaluate_system_health(&self, health_score: f64) -> Result<()> {
        let current = *self.current_level.read().await;
        
        // Find appropriate degradation level
        let new_level = self.degradation_levels
            .iter()
            .position(|level| health_score >= level.threshold)
            .unwrap_or(self.degradation_levels.len() - 1);
        
        if new_level != current {
            self.apply_degradation_level(new_level).await?;
        }
        
        Ok(())
    }
    
    async fn apply_degradation_level(&self, level: usize) -> Result<()> {
        let degradation = &self.degradation_levels[level];
        info!("Applying degradation level {}: {}", level, degradation.name);
        
        // Disable features
        {
            let mut flags = self.feature_flags.write().await;
            for feature in &degradation.disabled_features {
                flags.disable_feature(*feature);
                info!("Disabled feature: {:?}", feature);
            }
        }
        
        // Reduce update frequencies
        for (component, new_frequency) in &degradation.reduced_frequencies {
            self.update_component_frequency(component, *new_frequency).await?;
        }
        
        // Update current level
        *self.current_level.write().await = level;
        
        // Send notification
        self.alert_manager.send_alert(DataAlert {
            symbol: "SYSTEM".to_string(),
            alert_type: AlertType::SystemDegraded,
            severity: AlertSeverity::Warning,
            duration: Duration::ZERO,
            message: degradation.message.clone(),
            timestamp: Instant::now(),
            metadata: hashmap!{
                "degradation_level".to_string() => level.to_string(),
                "health_score".to_string() => health_score.to_string(),
            },
        }).await;
        
        Ok(())
    }
}
```

### 3.4 Circuit Breaker Implementation

```rust
// src/monitoring/circuit_breaker.rs
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

pub struct CircuitBreaker {
    failure_threshold: u32,
    success_threshold: u32,
    timeout: Duration,
    half_open_max_requests: u32,
    
    failure_count: AtomicU32,
    success_count: AtomicU32,
    last_failure_time: AtomicU64,
    state: Arc<RwLock<CircuitState>>,
}

#[derive(Clone, Copy, PartialEq)]
enum CircuitState {
    Closed,     // Normal operation
    Open,       // Failing, reject requests
    HalfOpen,   // Testing recovery
}

impl CircuitBreaker {
    pub async fn call<F, T>(&self, operation: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        let state = *self.state.read().await;
        
        match state {
            CircuitState::Open => {
                // Check if timeout has passed
                let last_failure = Duration::from_millis(
                    self.last_failure_time.load(Ordering::Relaxed)
                );
                
                if Instant::now().duration_since(UNIX_EPOCH) - last_failure > self.timeout {
                    // Try half-open
                    *self.state.write().await = CircuitState::HalfOpen;
                    self.success_count.store(0, Ordering::Relaxed);
                } else {
                    return Err(anyhow!("Circuit breaker is open"));
                }
            }
            CircuitState::HalfOpen => {
                let success_count = self.success_count.load(Ordering::Relaxed);
                if success_count >= self.half_open_max_requests {
                    return Err(anyhow!("Circuit breaker half-open limit reached"));
                }
            }
            CircuitState::Closed => {}
        }
        
        // Execute operation
        match operation.await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(e) => {
                self.on_failure().await;
                Err(e)
            }
        }
    }
    
    async fn on_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        let success_count = self.success_count.fetch_add(1, Ordering::Relaxed) + 1;
        
        let mut state = self.state.write().await;
        
        match *state {
            CircuitState::HalfOpen => {
                if success_count >= self.success_threshold {
                    *state = CircuitState::Closed;
                    info!("Circuit breaker closed after successful recovery");
                }
            }
            _ => {}
        }
    }
    
    async fn on_failure(&self) {
        let failure_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        self.last_failure_time.store(
            Instant::now().duration_since(UNIX_EPOCH).as_millis() as u64,
            Ordering::Relaxed
        );
        
        let mut state = self.state.write().await;
        
        match *state {
            CircuitState::Closed => {
                if failure_count >= self.failure_threshold {
                    *state = CircuitState::Open;
                    error!("Circuit breaker opened after {} failures", failure_count);
                }
            }
            CircuitState::HalfOpen => {
                *state = CircuitState::Open;
                error!("Circuit breaker reopened due to failure in half-open state");
            }
            _ => {}
        }
    }
}
```

## 4. Metrics to Track

### 4.1 Messages Per Second by Symbol

```yaml
# Prometheus metrics definition
market_data_messages_per_second:
  type: gauge
  labels: [symbol, source]
  description: "Real-time message rate per symbol"
  
market_data_messages_total:
  type: counter
  labels: [symbol, source, message_type]
  description: "Total messages received per symbol"
  
market_data_batch_size:
  type: histogram
  labels: [symbol, source]
  buckets: [1, 10, 50, 100, 500, 1000]
  description: "Batch size distribution for bulk updates"
```

### 4.2 Connection Uptime Percentage

```rust
// src/monitoring/uptime_tracker.rs
pub struct UptimeTracker {
    connection_states: Arc<RwLock<HashMap<String, ConnectionState>>>,
    uptime_calculator: Arc<RwLock<UptimeCalculator>>,
}

#[derive(Clone)]
struct ConnectionState {
    source_name: String,
    connected_since: Option<Instant>,
    total_uptime: Duration,
    total_downtime: Duration,
    state_changes: Vec<StateChange>,
}

impl UptimeTracker {
    pub async fn record_connection_state(&self, source: &str, is_connected: bool) {
        let mut states = self.connection_states.write().await;
        let state = states.entry(source.to_string()).or_insert_with(|| {
            ConnectionState {
                source_name: source.to_string(),
                connected_since: None,
                total_uptime: Duration::ZERO,
                total_downtime: Duration::ZERO,
                state_changes: Vec::new(),
            }
        });
        
        let now = Instant::now();
        
        match (state.connected_since, is_connected) {
            (None, true) => {
                // Connection established
                state.connected_since = Some(now);
                state.state_changes.push(StateChange {
                    timestamp: now,
                    new_state: ConnectionStateType::Connected,
                });
            }
            (Some(since), false) => {
                // Connection lost
                let uptime = now.duration_since(since);
                state.total_uptime += uptime;
                state.connected_since = None;
                state.state_changes.push(StateChange {
                    timestamp: now,
                    new_state: ConnectionStateType::Disconnected,
                });
            }
            _ => {} // No state change
        }
        
        // Calculate and record uptime percentage
        let uptime_percent = self.calculate_uptime_percentage(state).await;
        gauge!("market_data_connection_uptime_percent",
            "source" => source
        ).set(uptime_percent);
    }
    
    async fn calculate_uptime_percentage(&self, state: &ConnectionState) -> f64 {
        let now = Instant::now();
        let current_uptime = match state.connected_since {
            Some(since) => now.duration_since(since),
            None => Duration::ZERO,
        };
        
        let total_uptime = state.total_uptime + current_uptime;
        let total_time = total_uptime + state.total_downtime;
        
        if total_time.as_secs() == 0 {
            100.0
        } else {
            (total_uptime.as_secs_f64() / total_time.as_secs_f64()) * 100.0
        }
    }
}
```

### 4.3 Recovery Time Objectives (RTO)

```rust
// src/monitoring/rto_tracker.rs
pub struct RTOTracker {
    incident_tracker: Arc<RwLock<HashMap<String, IncidentRecord>>>,
    rto_targets: HashMap<IncidentType, Duration>,
}

#[derive(Clone)]
struct IncidentRecord {
    incident_id: String,
    incident_type: IncidentType,
    start_time: Instant,
    detection_time: Option<Instant>,
    recovery_start_time: Option<Instant>,
    resolution_time: Option<Instant>,
    recovery_steps: Vec<RecoveryStep>,
}

impl RTOTracker {
    pub async fn record_incident_start(&self, incident_type: IncidentType) -> String {
        let incident_id = Uuid::new_v4().to_string();
        let record = IncidentRecord {
            incident_id: incident_id.clone(),
            incident_type,
            start_time: Instant::now(),
            detection_time: None,
            recovery_start_time: None,
            resolution_time: None,
            recovery_steps: Vec::new(),
        };
        
        self.incident_tracker.write().await.insert(incident_id.clone(), record);
        
        counter!("market_data_incidents_total",
            "type" => format!("{:?}", incident_type)
        ).increment(1);
        
        incident_id
    }
    
    pub async fn record_recovery_complete(&self, incident_id: &str) -> Result<()> {
        let mut tracker = self.incident_tracker.write().await;
        
        if let Some(incident) = tracker.get_mut(incident_id) {
            incident.resolution_time = Some(Instant::now());
            
            // Calculate recovery time
            let recovery_time = incident.resolution_time.unwrap()
                .duration_since(incident.start_time);
            
            // Check against RTO target
            let rto_target = self.rto_targets
                .get(&incident.incident_type)
                .unwrap_or(&Duration::from_secs(300)); // 5 min default
            
            let rto_met = recovery_time <= *rto_target;
            
            // Record metrics
            histogram!("market_data_recovery_time_seconds",
                "incident_type" => format!("{:?}", incident.incident_type),
                "rto_met" => rto_met.to_string()
            ).record(recovery_time.as_secs_f64());
            
            gauge!("market_data_rto_compliance_rate",
                "incident_type" => format!("{:?}", incident.incident_type)
            ).set(if rto_met { 1.0 } else { 0.0 });
            
            if !rto_met {
                warn!("RTO target missed for {:?}: {} > {}",
                    incident.incident_type,
                    humantime::format_duration(recovery_time),
                    humantime::format_duration(*rto_target)
                );
            }
        }
        
        Ok(())
    }
}
```

### 4.4 Data Gap Detection

```rust
// src/monitoring/gap_detector.rs
pub struct DataGapDetector {
    symbol_sequences: Arc<RwLock<HashMap<String, SequenceTracker>>>,
    gap_threshold: Duration,
    gap_alerts: Arc<RwLock<Vec<DataGap>>>,
}

#[derive(Clone)]
struct SequenceTracker {
    last_sequence: Option<u64>,
    last_timestamp: Option<Instant>,
    expected_interval: Duration,
    gaps_detected: Vec<DataGap>,
}

#[derive(Clone, Debug)]
struct DataGap {
    symbol: String,
    start_sequence: u64,
    end_sequence: u64,
    duration: Duration,
    detected_at: Instant,
    severity: GapSeverity,
}

impl DataGapDetector {
    pub async fn process_market_data(&self, data: &MarketData) {
        let mut trackers = self.symbol_sequences.write().await;
        
        let tracker = trackers.entry(data.symbol.clone())
            .or_insert_with(|| SequenceTracker {
                last_sequence: None,
                last_timestamp: None,
                expected_interval: Duration::from_millis(100), // Default 100ms
                gaps_detected: Vec::new(),
            });
        
        let now = Instant::now();
        
        // Check for sequence gaps
        if let (Some(last_seq), Some(current_seq)) = (tracker.last_sequence, data.sequence_number) {
            if current_seq > last_seq + 1 {
                let gap = DataGap {
                    symbol: data.symbol.clone(),
                    start_sequence: last_seq + 1,
                    end_sequence: current_seq - 1,
                    duration: now.duration_since(tracker.last_timestamp.unwrap_or(now)),
                    detected_at: now,
                    severity: self.classify_gap_severity(current_seq - last_seq - 1),
                };
                
                warn!("Data gap detected for {}: sequences {} to {} ({} missing)",
                    data.symbol, gap.start_sequence, gap.end_sequence,
                    gap.end_sequence - gap.start_sequence + 1
                );
                
                tracker.gaps_detected.push(gap.clone());
                self.record_gap_metrics(&gap).await;
                
                // Alert if severe
                if gap.severity >= GapSeverity::Major {
                    self.alert_manager.send_alert(DataAlert {
                        symbol: data.symbol.clone(),
                        alert_type: AlertType::DataQualityIssue,
                        severity: AlertSeverity::Warning,
                        duration: gap.duration,
                        message: format!("Data gap: {} messages missing", 
                            gap.end_sequence - gap.start_sequence + 1),
                        timestamp: now,
                        metadata: HashMap::new(),
                    }).await;
                }
            }
        }
        
        // Update tracker
        tracker.last_sequence = data.sequence_number;
        tracker.last_timestamp = Some(now);
    }
    
    fn classify_gap_severity(&self, missing_count: u64) -> GapSeverity {
        match missing_count {
            1..=5 => GapSeverity::Minor,
            6..=50 => GapSeverity::Major,
            51..=500 => GapSeverity::Severe,
            _ => GapSeverity::Critical,
        }
    }
    
    async fn record_gap_metrics(&self, gap: &DataGap) {
        counter!("market_data_gaps_detected",
            "symbol" => gap.symbol.clone(),
            "severity" => format!("{:?}", gap.severity)
        ).increment(1);
        
        gauge!("market_data_gap_size",
            "symbol" => gap.symbol.clone()
        ).set((gap.end_sequence - gap.start_sequence + 1) as f64);
        
        histogram!("market_data_gap_duration_seconds",
            "symbol" => gap.symbol.clone()
        ).record(gap.duration.as_secs_f64());
    }
}
```

## 5. Grafana Dashboard Configuration

### 5.1 Market Data Monitoring Dashboard

```yaml
# grafana/dashboards/market-data-monitoring.json
{
  "dashboard": {
    "title": "Market Data Monitoring",
    "uid": "market-data-monitoring",
    "refresh": "5s",
    "time": {
      "from": "now-1h",
      "to": "now"
    },
    "panels": [
      {
        "id": 1,
        "title": "Data Flow Health Overview",
        "type": "stat",
        "gridPos": { "x": 0, "y": 0, "w": 6, "h": 4 },
        "targets": [{
          "expr": "avg(market_data_health_score)",
          "legendFormat": "Overall Health"
        }],
        "fieldConfig": {
          "defaults": {
            "thresholds": {
              "mode": "absolute",
              "steps": [
                { "color": "red", "value": 0 },
                { "color": "yellow", "value": 0.8 },
                { "color": "green", "value": 0.95 }
              ]
            },
            "unit": "percentunit"
          }
        }
      },
      {
        "id": 2,
        "title": "Active Symbols",
        "type": "stat",
        "gridPos": { "x": 6, "y": 0, "w": 6, "h": 4 },
        "targets": [{
          "expr": "count(count by (symbol) (rate(market_data_messages_total[1m]) > 0))",
          "legendFormat": "Active Symbols"
        }]
      },
      {
        "id": 3,
        "title": "Message Rate by Symbol",
        "type": "graph",
        "gridPos": { "x": 0, "y": 4, "w": 12, "h": 8 },
        "targets": [{
          "expr": "rate(market_data_messages_total[1m])",
          "legendFormat": "{{symbol}}"
        }],
        "yaxes": [{
          "format": "short",
          "label": "Messages/sec"
        }]
      },
      {
        "id": 4,
        "title": "Latency Heatmap",
        "type": "heatmap",
        "gridPos": { "x": 12, "y": 4, "w": 12, "h": 8 },
        "targets": [{
          "expr": "histogram_quantile(0.95, sum(rate(market_data_latency_seconds_bucket[5m])) by (symbol, le))",
          "format": "heatmap"
        }],
        "dataFormat": "tsbuckets",
        "color": {
          "mode": "spectrum",
          "scheme": "interpolateRdYlGn",
          "reverse": true
        }
      },
      {
        "id": 5,
        "title": "Connection Uptime",
        "type": "gauge",
        "gridPos": { "x": 0, "y": 12, "w": 6, "h": 6 },
        "targets": [{
          "expr": "market_data_connection_uptime_percent",
          "legendFormat": "{{source}}"
        }],
        "fieldConfig": {
          "defaults": {
            "thresholds": {
              "mode": "absolute",
              "steps": [
                { "color": "red", "value": 0 },
                { "color": "yellow", "value": 95 },
                { "color": "green", "value": 99 }
              ]
            },
            "unit": "percent",
            "min": 0,
            "max": 100
          }
        }
      },
      {
        "id": 6,
        "title": "Data Quality Score",
        "type": "timeseries",
        "gridPos": { "x": 6, "y": 12, "w": 12, "h": 6 },
        "targets": [{
          "expr": "market_data_quality_score",
          "legendFormat": "{{symbol}}"
        }],
        "fieldConfig": {
          "defaults": {
            "custom": {
              "drawStyle": "line",
              "lineInterpolation": "smooth",
              "spanNulls": true
            },
            "thresholds": {
              "mode": "absolute",
              "steps": [
                { "color": "red", "value": 0 },
                { "color": "yellow", "value": 0.7 },
                { "color": "green", "value": 0.9 }
              ]
            }
          }
        }
      },
      {
        "id": 7,
        "title": "Alert History",
        "type": "table",
        "gridPos": { "x": 0, "y": 18, "w": 24, "h": 6 },
        "targets": [{
          "expr": "increase(market_data_alerts_total[1h])",
          "format": "table",
          "instant": true
        }],
        "transformations": [{
          "id": "organize",
          "options": {
            "excludeByName": {},
            "indexByName": {},
            "renameByName": {
              "symbol": "Symbol",
              "alert_type": "Alert Type",
              "severity": "Severity",
              "Value": "Count"
            }
          }
        }]
      },
      {
        "id": 8,
        "title": "Recovery Time Objectives",
        "type": "bargauge",
        "gridPos": { "x": 18, "y": 12, "w": 6, "h": 6 },
        "targets": [{
          "expr": "histogram_quantile(0.95, market_data_recovery_time_seconds_bucket)",
          "legendFormat": "{{incident_type}}"
        }],
        "fieldConfig": {
          "defaults": {
            "thresholds": {
              "mode": "absolute",
              "steps": [
                { "color": "green", "value": 0 },
                { "color": "yellow", "value": 60 },
                { "color": "red", "value": 300 }
              ]
            },
            "unit": "s"
          }
        }
      }
    ]
  }
}
```

### 5.2 Alert Rules Configuration

```yaml
# prometheus/rules/market-data-alerts.yml
groups:
  - name: market_data_alerts
    interval: 10s
    rules:
      - alert: NoMarketDataReceived
        expr: |
          (time() - market_data_last_update_timestamp) > 30
          and hour() >= 9 and hour() < 16
        for: 0m
        labels:
          severity: critical
          component: market_data
        annotations:
          summary: "No market data received for {{ $labels.symbol }}"
          description: "Symbol {{ $labels.symbol }} has not received data for {{ $value }} seconds during trading hours"
          runbook_url: "https://docs.neural-trader.com/runbooks/no-market-data"
      
      - alert: HighMarketDataLatency
        expr: |
          histogram_quantile(0.95, rate(market_data_latency_seconds_bucket[5m])) > 0.1
        for: 2m
        labels:
          severity: warning
          component: market_data
        annotations:
          summary: "High latency for {{ $labels.symbol }}"
          description: "P95 latency for {{ $labels.symbol }} is {{ $value }}s"
      
      - alert: MarketDataQualityDegraded
        expr: market_data_quality_score < 0.7
        for: 5m
        labels:
          severity: warning
          component: market_data
        annotations:
          summary: "Data quality degraded for {{ $labels.symbol }}"
          description: "Quality score: {{ $value }}"
      
      - alert: MarketDataConnectionDown
        expr: market_data_connection_uptime_percent < 95
        for: 5m
        labels:
          severity: critical
          component: market_data
        annotations:
          summary: "Connection uptime below threshold"
          description: "{{ $labels.source }} uptime: {{ $value }}%"
      
      - alert: MarketDataGapsDetected
        expr: increase(market_data_gaps_detected[5m]) > 10
        for: 1m
        labels:
          severity: warning
          component: market_data
        annotations:
          summary: "Multiple data gaps detected"
          description: "{{ $value }} gaps in last 5 minutes for {{ $labels.symbol }}"
```

## 6. Implementation Guidelines

### 6.1 Docker Compose Integration

```yaml
# docker-compose.yml additions
services:
  market-data-monitor:
    build:
      context: .
      dockerfile: docker/market-monitor/Dockerfile
    environment:
      - RUST_LOG=info
      - MONITORING_INTERVAL=1s
      - ALERT_WEBHOOK_URL=${ALERT_WEBHOOK_URL}
      - GRAFANA_API_KEY=${GRAFANA_API_KEY}
    depends_on:
      - redis
      - prometheus
      - grafana
    volumes:
      - ./config/monitoring:/app/config
    networks:
      - neural-trader-network
```

### 6.2 Configuration File

```toml
# config/monitoring/market-data-monitor.toml
[monitor]
check_interval = "1s"
no_data_threshold_trading = "30s"
no_data_threshold_non_trading = "5m"
latency_warning_threshold = "100ms"
latency_critical_threshold = "1s"

[symbols]
# Symbol-specific configuration
[symbols.AAPL]
expected_rate = 100  # messages per second
min_quality_score = 0.8

[symbols.BTC-USD]
expected_rate = 50
min_quality_score = 0.7
extended_hours = true

[recovery]
max_reconnect_attempts = 5
initial_backoff = "1s"
max_backoff = "30s"
circuit_breaker_threshold = 5
circuit_breaker_timeout = "60s"

[fallback_sources]
[[fallback_sources.sources]]
name = "primary"
url = "wss://stream.primary-provider.com"
priority = 1

[[fallback_sources.sources]]
name = "secondary"
url = "wss://stream.backup-provider.com"
priority = 2

[alerting]
[alerting.channels]
grafana = { enabled = true, url = "http://grafana:3000" }
slack = { enabled = true, webhook = "${SLACK_WEBHOOK}" }
pagerduty = { enabled = true, api_key = "${PAGERDUTY_API_KEY}" }

[alerting.escalation]
[[alerting.escalation.levels]]
delay = "5m"
actions = ["slack"]

[[alerting.escalation.levels]]
delay = "15m"
actions = ["pagerduty", "execute_runbook"]
```

### 6.3 Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_no_data_detection() {
        let monitor = MarketDataMonitor::new_test();
        let (tx, rx) = mpsc::channel(100);
        
        // Start monitoring
        monitor.monitor_data_flow(rx).await;
        
        // Send initial data
        tx.send(MarketData {
            symbol: "AAPL".to_string(),
            price: 150.0,
            timestamp: SystemTime::now(),
            ..Default::default()
        }).await.unwrap();
        
        // Wait for threshold
        tokio::time::sleep(Duration::from_secs(35)).await;
        
        // Check alerts
        let alerts = monitor.get_active_alerts().await;
        assert!(alerts.iter().any(|a| {
            a.symbol == "AAPL" && a.alert_type == AlertType::NoDataReceived
        }));
    }
    
    #[tokio::test]
    async fn test_automatic_recovery() {
        let coordinator = RecoveryCoordinator::new_test();
        let failed_source = DataSource {
            name: "primary".to_string(),
            url: "wss://failed.example.com".to_string(),
        };
        
        // Trigger recovery
        let outcome = coordinator.handle_source_failure(&failed_source).await.unwrap();
        
        // Verify fallback activated
        assert_eq!(coordinator.get_active_source().await, "secondary");
        
        // Simulate primary recovery
        coordinator.test_source_recovery(&failed_source).await;
        
        // Verify switchback
        tokio::time::sleep(Duration::from_secs(10)).await;
        assert_eq!(coordinator.get_active_source().await, "primary");
    }
}
```

## 7. Performance Considerations

### 7.1 Memory Management

- Use circular buffers for latency tracking (fixed memory usage)
- Implement metric cardinality limits
- Regular cleanup of old alert records
- Efficient data structures for high-frequency updates

### 7.2 CPU Optimization

- Batch metric updates to reduce syscalls
- Use lock-free data structures where possible
- Implement sampling for very high-frequency symbols
- Async processing for non-critical paths

## 8. Deployment Checklist

1. **Pre-deployment**:
   - [ ] Configure all data sources and fallbacks
   - [ ] Set up alert channels (Grafana, Slack, PagerDuty)
   - [ ] Define RTO targets for each incident type
   - [ ] Create runbooks for common issues

2. **Deployment**:
   - [ ] Deploy monitoring service
   - [ ] Import Grafana dashboards
   - [ ] Configure Prometheus scraping
   - [ ] Set up alert rules

3. **Post-deployment**:
   - [ ] Verify all symbols are being monitored
   - [ ] Test alert delivery to all channels
   - [ ] Simulate failures to test recovery
   - [ ] Monitor resource usage

## Conclusion

This comprehensive market data monitoring system provides:

1. **Real-time Health Monitoring**: Continuous validation of data flow with 30-second detection during trading hours
2. **Intelligent Alerting**: Multi-channel alerts with trading hours awareness and escalation procedures
3. **Automatic Recovery**: Circuit breakers, fallback sources, and graceful degradation
4. **Deep Observability**: Detailed metrics tracking with Grafana integration
5. **Production Readiness**: Robust error handling, testing, and performance optimization

The system is designed to maintain high-quality data flow for the neural trader while providing operators with comprehensive visibility and control over the market data infrastructure.