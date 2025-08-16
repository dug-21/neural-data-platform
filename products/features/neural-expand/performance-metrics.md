# Performance Metrics and Benchmarking

## Overview

This document defines comprehensive performance metrics, benchmarking procedures, and optimization guidelines for the Neural-Trader platform. It includes latency requirements, throughput targets, resource utilization metrics, and continuous monitoring strategies.

## Performance Targets

### Latency Requirements

#### Real-Time Processing Pipeline
```rust
pub const LATENCY_TARGETS: LatencyTargets = LatencyTargets {
    // Data ingestion latency
    data_ingestion: LatencyTarget {
        p50: Duration::from_millis(5),
        p95: Duration::from_millis(10),
        p99: Duration::from_millis(20),
        max: Duration::from_millis(50),
    },
    
    // Feature extraction latency
    feature_extraction: LatencyTarget {
        p50: Duration::from_millis(2),
        p95: Duration::from_millis(5),
        p99: Duration::from_millis(10),
        max: Duration::from_millis(20),
    },
    
    // Neural prediction latency
    neural_prediction: LatencyTarget {
        p50: Duration::from_millis(10),
        p95: Duration::from_millis(25),
        p99: Duration::from_millis(50),
        max: Duration::from_millis(100),
    },
    
    // Strategy execution latency
    strategy_execution: LatencyTarget {
        p50: Duration::from_millis(5),
        p95: Duration::from_millis(15),
        p99: Duration::from_millis(30),
        max: Duration::from_millis(50),
    },
    
    // End-to-end processing latency
    end_to_end: LatencyTarget {
        p50: Duration::from_millis(25),
        p95: Duration::from_millis(50),
        p99: Duration::from_millis(100),
        max: Duration::from_millis(200),
    },
};
```

#### Latency Measurement Framework
```rust
pub struct LatencyMeasurement {
    start_time: Instant,
    end_time: Option<Instant>,
    operation: String,
    context: HashMap<String, String>,
}

impl LatencyMeasurement {
    pub fn start(operation: &str) -> Self {
        Self {
            start_time: Instant::now(),
            end_time: None,
            operation: operation.to_string(),
            context: HashMap::new(),
        }
    }
    
    pub fn end(mut self) -> Duration {
        self.end_time = Some(Instant::now());
        let duration = self.end_time.unwrap() - self.start_time;
        
        // Record metrics
        METRICS.record_latency(&self.operation, duration);
        
        duration
    }
    
    pub fn add_context(mut self, key: &str, value: &str) -> Self {
        self.context.insert(key.to_string(), value.to_string());
        self
    }
}

// Usage example
async fn process_market_data(data: MarketData) -> Result<ProcessedData> {
    let _measurement = LatencyMeasurement::start("process_market_data")
        .add_context("symbol", &data.symbol);
    
    // Processing logic here
    let processed = perform_processing(data).await?;
    
    Ok(processed)
}
```

### Throughput Requirements

#### System Throughput Targets
```rust
pub const THROUGHPUT_TARGETS: ThroughputTargets = ThroughputTargets {
    // Market data ingestion
    market_data_ingestion: ThroughputTarget {
        target_ops_per_second: 1000,
        peak_ops_per_second: 5000,
        sustained_duration: Duration::from_hours(8), // Full trading day
        burst_duration: Duration::from_minutes(5),
    },
    
    // Neural predictions
    neural_predictions: ThroughputTarget {
        target_ops_per_second: 100,
        peak_ops_per_second: 500,
        sustained_duration: Duration::from_hours(8),
        burst_duration: Duration::from_minutes(1),
    },
    
    // Trading signals
    trading_signals: ThroughputTarget {
        target_ops_per_second: 50,
        peak_ops_per_second: 200,
        sustained_duration: Duration::from_hours(8),
        burst_duration: Duration::from_minutes(2),
    },
    
    // Order processing
    order_processing: ThroughputTarget {
        target_ops_per_second: 20,
        peak_ops_per_second: 100,
        sustained_duration: Duration::from_hours(8),
        burst_duration: Duration::from_seconds(30),
    },
};
```

#### Throughput Measurement
```rust
pub struct ThroughputMeter {
    operation: String,
    start_time: Instant,
    operation_count: Arc<AtomicUsize>,
    window_size: Duration,
}

impl ThroughputMeter {
    pub fn new(operation: &str, window_size: Duration) -> Self {
        Self {
            operation: operation.to_string(),
            start_time: Instant::now(),
            operation_count: Arc::new(AtomicUsize::new(0)),
            window_size,
        }
    }
    
    pub fn record_operation(&self) {
        self.operation_count.fetch_add(1, Ordering::SeqCst);
    }
    
    pub fn get_current_throughput(&self) -> f64 {
        let elapsed = self.start_time.elapsed();
        let count = self.operation_count.load(Ordering::SeqCst);
        
        if elapsed.as_secs() == 0 {
            0.0
        } else {
            count as f64 / elapsed.as_secs() as f64
        }
    }
    
    pub async fn start_monitoring(&self) {
        let operation = self.operation.clone();
        let counter = self.operation_count.clone();
        let window_size = self.window_size;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(window_size);
            let mut last_count = 0;
            
            loop {
                interval.tick().await;
                let current_count = counter.load(Ordering::SeqCst);
                let ops_in_window = current_count - last_count;
                let throughput = ops_in_window as f64 / window_size.as_secs() as f64;
                
                METRICS.record_throughput(&operation, throughput);
                last_count = current_count;
            }
        });
    }
}
```

## Resource Utilization Metrics

### Memory Usage Monitoring
```rust
pub struct MemoryMonitor {
    process_id: u32,
    max_memory_gb: f64,
    warning_threshold: f64,
    critical_threshold: f64,
}

impl MemoryMonitor {
    pub fn new(max_memory_gb: f64) -> Self {
        Self {
            process_id: std::process::id(),
            max_memory_gb,
            warning_threshold: 0.8,  // 80% warning
            critical_threshold: 0.95, // 95% critical
        }
    }
    
    pub fn get_memory_usage(&self) -> MemoryUsage {
        let mut system = System::new_all();
        system.refresh_all();
        
        if let Some(process) = system.process(Pid::from_u32(self.process_id)) {
            let memory_bytes = process.memory();
            let memory_gb = memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
            
            MemoryUsage {
                used_gb: memory_gb,
                max_gb: self.max_memory_gb,
                percentage: memory_gb / self.max_memory_gb,
                status: self.classify_memory_status(memory_gb / self.max_memory_gb),
            }
        } else {
            MemoryUsage::default()
        }
    }
    
    fn classify_memory_status(&self, percentage: f64) -> MemoryStatus {
        if percentage > self.critical_threshold {
            MemoryStatus::Critical
        } else if percentage > self.warning_threshold {
            MemoryStatus::Warning
        } else {
            MemoryStatus::Normal
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryUsage {
    pub used_gb: f64,
    pub max_gb: f64,
    pub percentage: f64,
    pub status: MemoryStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MemoryStatus {
    Normal,
    Warning,
    Critical,
}
```

### CPU Usage Monitoring
```rust
pub struct CPUMonitor {
    core_count: usize,
    target_cpu_percentage: f64,
    warning_threshold: f64,
    critical_threshold: f64,
}

impl CPUMonitor {
    pub fn new() -> Self {
        Self {
            core_count: num_cpus::get(),
            target_cpu_percentage: 70.0,  // Target 70% CPU usage
            warning_threshold: 80.0,      // 80% warning
            critical_threshold: 95.0,     // 95% critical
        }
    }
    
    pub async fn get_cpu_usage(&self) -> CPUUsage {
        let mut system = System::new_all();
        system.refresh_cpu();
        
        // Wait for CPU measurement
        tokio::time::sleep(Duration::from_millis(100)).await;
        system.refresh_cpu();
        
        let total_cpu_usage: f32 = system.cpus().iter()
            .map(|cpu| cpu.cpu_usage())
            .sum::<f32>() / self.core_count as f32;
        
        CPUUsage {
            percentage: total_cpu_usage as f64,
            core_count: self.core_count,
            per_core_usage: system.cpus().iter()
                .map(|cpu| cpu.cpu_usage() as f64)
                .collect(),
            status: self.classify_cpu_status(total_cpu_usage as f64),
        }
    }
    
    fn classify_cpu_status(&self, percentage: f64) -> CPUStatus {
        if percentage > self.critical_threshold {
            CPUStatus::Critical
        } else if percentage > self.warning_threshold {
            CPUStatus::Warning
        } else {
            CPUStatus::Normal
        }
    }
}
```

## Neural Network Performance Metrics

### Model Accuracy Tracking
```rust
pub struct ModelAccuracyTracker {
    model_name: String,
    predictions: VecDeque<PredictionRecord>,
    window_size: usize,
    accuracy_threshold: f64,
}

impl ModelAccuracyTracker {
    pub fn new(model_name: String, window_size: usize, accuracy_threshold: f64) -> Self {
        Self {
            model_name,
            predictions: VecDeque::new(),
            window_size,
            accuracy_threshold,
        }
    }
    
    pub fn record_prediction(&mut self, prediction: PredictionRecord) {
        self.predictions.push_back(prediction);
        
        // Keep only the last N predictions
        if self.predictions.len() > self.window_size {
            self.predictions.pop_front();
        }
    }
    
    pub fn calculate_accuracy(&self) -> ModelAccuracy {
        let total_predictions = self.predictions.len();
        if total_predictions == 0 {
            return ModelAccuracy::default();
        }
        
        let correct_predictions = self.predictions.iter()
            .filter(|pred| pred.is_correct())
            .count();
        
        let accuracy = correct_predictions as f64 / total_predictions as f64;
        
        ModelAccuracy {
            model_name: self.model_name.clone(),
            accuracy,
            total_predictions,
            correct_predictions,
            window_size: self.window_size,
            meets_threshold: accuracy >= self.accuracy_threshold,
        }
    }
    
    pub fn get_confidence_distribution(&self) -> ConfidenceDistribution {
        let mut confidence_buckets = vec![0; 10]; // 10 buckets for 0-1 confidence
        
        for prediction in &self.predictions {
            let bucket = (prediction.confidence * 10.0).min(9.0) as usize;
            confidence_buckets[bucket] += 1;
        }
        
        ConfidenceDistribution {
            buckets: confidence_buckets,
            total_predictions: self.predictions.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PredictionRecord {
    pub timestamp: DateTime<Utc>,
    pub predicted_value: f64,
    pub actual_value: Option<f64>,
    pub confidence: f64,
    pub prediction_horizon: Duration,
}

impl PredictionRecord {
    pub fn is_correct(&self) -> bool {
        if let Some(actual) = self.actual_value {
            // For directional accuracy
            let predicted_direction = self.predicted_value > 0.0;
            let actual_direction = actual > 0.0;
            predicted_direction == actual_direction
        } else {
            false
        }
    }
}
```

### Model Performance Comparison
```rust
pub struct ModelPerformanceComparator {
    models: HashMap<String, ModelAccuracyTracker>,
    comparison_window: Duration,
}

impl ModelPerformanceComparator {
    pub fn new(comparison_window: Duration) -> Self {
        Self {
            models: HashMap::new(),
            comparison_window,
        }
    }
    
    pub fn add_model(&mut self, model_name: String, accuracy_threshold: f64) {
        let tracker = ModelAccuracyTracker::new(
            model_name.clone(),
            1000, // 1000 predictions window
            accuracy_threshold,
        );
        self.models.insert(model_name, tracker);
    }
    
    pub fn record_prediction(&mut self, model_name: &str, prediction: PredictionRecord) {
        if let Some(tracker) = self.models.get_mut(model_name) {
            tracker.record_prediction(prediction);
        }
    }
    
    pub fn get_model_rankings(&self) -> Vec<ModelRanking> {
        let mut rankings: Vec<ModelRanking> = self.models.iter()
            .map(|(name, tracker)| {
                let accuracy = tracker.calculate_accuracy();
                ModelRanking {
                    model_name: name.clone(),
                    accuracy: accuracy.accuracy,
                    predictions_count: accuracy.total_predictions,
                    meets_threshold: accuracy.meets_threshold,
                }
            })
            .collect();
        
        rankings.sort_by(|a, b| b.accuracy.partial_cmp(&a.accuracy).unwrap());
        rankings
    }
}

#[derive(Debug, Clone)]
pub struct ModelRanking {
    pub model_name: String,
    pub accuracy: f64,
    pub predictions_count: usize,
    pub meets_threshold: bool,
}
```

## Trading Performance Metrics

### Strategy Performance Tracking
```rust
pub struct StrategyPerformanceTracker {
    strategy_name: String,
    trades: Vec<TradeRecord>,
    initial_capital: f64,
    current_capital: f64,
    max_drawdown: f64,
    peak_capital: f64,
}

impl StrategyPerformanceTracker {
    pub fn new(strategy_name: String, initial_capital: f64) -> Self {
        Self {
            strategy_name,
            trades: Vec::new(),
            initial_capital,
            current_capital: initial_capital,
            max_drawdown: 0.0,
            peak_capital: initial_capital,
        }
    }
    
    pub fn record_trade(&mut self, trade: TradeRecord) {
        self.trades.push(trade.clone());
        
        // Update capital
        self.current_capital += trade.pnl;
        
        // Update peak and drawdown
        if self.current_capital > self.peak_capital {
            self.peak_capital = self.current_capital;
        }
        
        let current_drawdown = (self.peak_capital - self.current_capital) / self.peak_capital;
        if current_drawdown > self.max_drawdown {
            self.max_drawdown = current_drawdown;
        }
    }
    
    pub fn calculate_performance_metrics(&self) -> StrategyPerformanceMetrics {
        let total_trades = self.trades.len();
        let winning_trades = self.trades.iter().filter(|t| t.pnl > 0.0).count();
        let losing_trades = self.trades.iter().filter(|t| t.pnl < 0.0).count();
        
        let total_return = (self.current_capital - self.initial_capital) / self.initial_capital;
        let win_rate = if total_trades > 0 {
            winning_trades as f64 / total_trades as f64
        } else {
            0.0
        };
        
        let average_win = if winning_trades > 0 {
            self.trades.iter()
                .filter(|t| t.pnl > 0.0)
                .map(|t| t.pnl)
                .sum::<f64>() / winning_trades as f64
        } else {
            0.0
        };
        
        let average_loss = if losing_trades > 0 {
            self.trades.iter()
                .filter(|t| t.pnl < 0.0)
                .map(|t| t.pnl.abs())
                .sum::<f64>() / losing_trades as f64
        } else {
            0.0
        };
        
        let profit_factor = if average_loss > 0.0 {
            average_win / average_loss
        } else {
            f64::INFINITY
        };
        
        let sharpe_ratio = self.calculate_sharpe_ratio();
        
        StrategyPerformanceMetrics {
            strategy_name: self.strategy_name.clone(),
            total_return,
            win_rate,
            profit_factor,
            sharpe_ratio,
            max_drawdown: self.max_drawdown,
            total_trades,
            winning_trades,
            losing_trades,
            average_win,
            average_loss,
            current_capital: self.current_capital,
        }
    }
    
    fn calculate_sharpe_ratio(&self) -> f64 {
        if self.trades.len() < 2 {
            return 0.0;
        }
        
        let returns: Vec<f64> = self.trades.iter()
            .map(|t| t.pnl / self.initial_capital)
            .collect();
        
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / (returns.len() - 1) as f64;
        
        let std_dev = variance.sqrt();
        
        if std_dev == 0.0 {
            0.0
        } else {
            // Assuming risk-free rate of 0 for simplicity
            mean_return / std_dev * (252.0_f64).sqrt() // Annualized
        }
    }
}

#[derive(Debug, Clone)]
pub struct TradeRecord {
    pub symbol: String,
    pub entry_time: DateTime<Utc>,
    pub exit_time: DateTime<Utc>,
    pub entry_price: f64,
    pub exit_price: f64,
    pub quantity: f64,
    pub pnl: f64,
    pub commission: f64,
}

#[derive(Debug, Clone)]
pub struct StrategyPerformanceMetrics {
    pub strategy_name: String,
    pub total_return: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub average_win: f64,
    pub average_loss: f64,
    pub current_capital: f64,
}
```

## System Health Monitoring

### Health Check Framework
```rust
pub struct SystemHealthMonitor {
    components: Vec<Box<dyn HealthCheckComponent>>,
    health_check_interval: Duration,
    alert_thresholds: HealthThresholds,
}

impl SystemHealthMonitor {
    pub fn new(health_check_interval: Duration) -> Self {
        Self {
            components: Vec::new(),
            health_check_interval,
            alert_thresholds: HealthThresholds::default(),
        }
    }
    
    pub fn add_component(&mut self, component: Box<dyn HealthCheckComponent>) {
        self.components.push(component);
    }
    
    pub async fn start_monitoring(&self) {
        let mut interval = tokio::time::interval(self.health_check_interval);
        
        loop {
            interval.tick().await;
            
            let health_status = self.check_system_health().await;
            
            // Record metrics
            METRICS.record_health_status(&health_status);
            
            // Check for alerts
            self.check_and_send_alerts(&health_status).await;
        }
    }
    
    async fn check_system_health(&self) -> SystemHealthStatus {
        let mut component_statuses = Vec::new();
        
        for component in &self.components {
            let status = component.check_health().await;
            component_statuses.push(status);
        }
        
        let overall_status = self.determine_overall_health(&component_statuses);
        
        SystemHealthStatus {
            overall_status,
            component_statuses,
            timestamp: Utc::now(),
        }
    }
    
    fn determine_overall_health(&self, component_statuses: &[ComponentHealthStatus]) -> HealthStatus {
        if component_statuses.iter().any(|s| s.status == HealthStatus::Critical) {
            HealthStatus::Critical
        } else if component_statuses.iter().any(|s| s.status == HealthStatus::Warning) {
            HealthStatus::Warning
        } else {
            HealthStatus::Healthy
        }
    }
}

#[async_trait]
pub trait HealthCheckComponent: Send + Sync {
    async fn check_health(&self) -> ComponentHealthStatus;
    fn component_name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct ComponentHealthStatus {
    pub component_name: String,
    pub status: HealthStatus,
    pub message: String,
    pub metrics: HashMap<String, f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
}
```

## Benchmarking Framework

### Performance Benchmarking Suite
```rust
pub struct PerformanceBenchmark {
    name: String,
    setup_fn: Box<dyn Fn() -> Box<dyn std::any::Any>>,
    benchmark_fn: Box<dyn Fn(&mut dyn std::any::Any) -> Duration>,
    teardown_fn: Box<dyn Fn(Box<dyn std::any::Any>)>,
    iterations: usize,
    warmup_iterations: usize,
}

impl PerformanceBenchmark {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            setup_fn: Box::new(|| Box::new(())),
            benchmark_fn: Box::new(|_| Duration::from_millis(0)),
            teardown_fn: Box::new(|_| {}),
            iterations: 1000,
            warmup_iterations: 100,
        }
    }
    
    pub fn run(&self) -> BenchmarkResult {
        let mut measurements = Vec::new();
        
        // Warmup iterations
        for _ in 0..self.warmup_iterations {
            let mut setup_result = (self.setup_fn)();
            let _ = (self.benchmark_fn)(setup_result.as_mut());
            (self.teardown_fn)(setup_result);
        }
        
        // Actual benchmark iterations
        for _ in 0..self.iterations {
            let mut setup_result = (self.setup_fn)();
            let duration = (self.benchmark_fn)(setup_result.as_mut());
            (self.teardown_fn)(setup_result);
            
            measurements.push(duration);
        }
        
        BenchmarkResult::new(self.name.clone(), measurements)
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub name: String,
    pub measurements: Vec<Duration>,
    pub mean: Duration,
    pub median: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub min: Duration,
    pub max: Duration,
    pub std_dev: Duration,
}

impl BenchmarkResult {
    pub fn new(name: String, mut measurements: Vec<Duration>) -> Self {
        measurements.sort();
        
        let mean = measurements.iter().sum::<Duration>() / measurements.len() as u32;
        let median = measurements[measurements.len() / 2];
        let p95 = measurements[(measurements.len() as f64 * 0.95) as usize];
        let p99 = measurements[(measurements.len() as f64 * 0.99) as usize];
        let min = measurements[0];
        let max = measurements[measurements.len() - 1];
        
        let variance = measurements.iter()
            .map(|d| {
                let diff = d.as_nanos() as f64 - mean.as_nanos() as f64;
                diff * diff
            })
            .sum::<f64>() / measurements.len() as f64;
        
        let std_dev = Duration::from_nanos(variance.sqrt() as u64);
        
        Self {
            name,
            measurements,
            mean,
            median,
            p95,
            p99,
            min,
            max,
            std_dev,
        }
    }
}
```

### Benchmark Test Suite
```rust
pub struct BenchmarkSuite {
    benchmarks: Vec<PerformanceBenchmark>,
}

impl BenchmarkSuite {
    pub fn new() -> Self {
        Self {
            benchmarks: Vec::new(),
        }
    }
    
    pub fn add_benchmark(&mut self, benchmark: PerformanceBenchmark) {
        self.benchmarks.push(benchmark);
    }
    
    pub fn run_all(&self) -> Vec<BenchmarkResult> {
        self.benchmarks.iter()
            .map(|benchmark| {
                println!("Running benchmark: {}", benchmark.name);
                benchmark.run()
            })
            .collect()
    }
    
    pub fn create_neural_prediction_benchmarks() -> Self {
        let mut suite = Self::new();
        
        // Neural prediction benchmark
        let neural_benchmark = PerformanceBenchmark::new("neural_prediction")
            .setup(|| {
                // Setup test data
                let test_data = create_test_time_series_data(100);
                Box::new(test_data)
            })
            .benchmark(|setup_data| {
                let data = setup_data.downcast_ref::<Vec<TimeSeriesData>>().unwrap();
                let start = Instant::now();
                
                // Simulate neural prediction
                let _predictions = simulate_neural_prediction(data);
                
                start.elapsed()
            });
        
        suite.add_benchmark(neural_benchmark);
        
        // Feature extraction benchmark
        let feature_benchmark = PerformanceBenchmark::new("feature_extraction")
            .setup(|| {
                let market_data = create_test_market_data();
                Box::new(market_data)
            })
            .benchmark(|setup_data| {
                let data = setup_data.downcast_ref::<MarketData>().unwrap();
                let start = Instant::now();
                
                // Simulate feature extraction
                let _features = simulate_feature_extraction(data);
                
                start.elapsed()
            });
        
        suite.add_benchmark(feature_benchmark);
        
        suite
    }
}
```

## Alerting and Notification System

### Alert Configuration
```rust
pub struct AlertingSystem {
    alert_rules: Vec<AlertRule>,
    notification_channels: Vec<Box<dyn NotificationChannel>>,
}

impl AlertingSystem {
    pub fn new() -> Self {
        Self {
            alert_rules: Vec::new(),
            notification_channels: Vec::new(),
        }
    }
    
    pub fn add_alert_rule(&mut self, rule: AlertRule) {
        self.alert_rules.push(rule);
    }
    
    pub fn add_notification_channel(&mut self, channel: Box<dyn NotificationChannel>) {
        self.notification_channels.push(channel);
    }
    
    pub async fn check_alerts(&self, metrics: &SystemMetrics) {
        for rule in &self.alert_rules {
            if rule.should_alert(metrics) {
                let alert = Alert {
                    rule_name: rule.name.clone(),
                    severity: rule.severity.clone(),
                    message: rule.generate_message(metrics),
                    timestamp: Utc::now(),
                };
                
                self.send_alert(&alert).await;
            }
        }
    }
    
    async fn send_alert(&self, alert: &Alert) {
        for channel in &self.notification_channels {
            if let Err(e) = channel.send_alert(alert).await {
                eprintln!("Failed to send alert via {}: {}", channel.name(), e);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct AlertRule {
    pub name: String,
    pub condition: AlertCondition,
    pub threshold: f64,
    pub severity: AlertSeverity,
    pub cooldown: Duration,
    pub last_triggered: Option<DateTime<Utc>>,
}

impl AlertRule {
    pub fn should_alert(&self, metrics: &SystemMetrics) -> bool {
        // Check cooldown
        if let Some(last_triggered) = self.last_triggered {
            if Utc::now() - last_triggered < self.cooldown {
                return false;
            }
        }
        
        // Check condition
        match &self.condition {
            AlertCondition::LatencyAbove(metric_name) => {
                if let Some(value) = metrics.get_latency_metric(metric_name) {
                    value > self.threshold
                } else {
                    false
                }
            }
            AlertCondition::ThroughputBelow(metric_name) => {
                if let Some(value) = metrics.get_throughput_metric(metric_name) {
                    value < self.threshold
                } else {
                    false
                }
            }
            AlertCondition::MemoryUsageAbove => {
                metrics.memory_usage_percentage > self.threshold
            }
            AlertCondition::CPUUsageAbove => {
                metrics.cpu_usage_percentage > self.threshold
            }
            AlertCondition::AccuracyBelow(model_name) => {
                if let Some(accuracy) = metrics.get_model_accuracy(model_name) {
                    accuracy < self.threshold
                } else {
                    false
                }
            }
        }
    }
    
    pub fn generate_message(&self, metrics: &SystemMetrics) -> String {
        match &self.condition {
            AlertCondition::LatencyAbove(metric_name) => {
                format!("High latency detected: {} is above {}ms", 
                       metric_name, self.threshold)
            }
            AlertCondition::ThroughputBelow(metric_name) => {
                format!("Low throughput detected: {} is below {} ops/sec", 
                       metric_name, self.threshold)
            }
            AlertCondition::MemoryUsageAbove => {
                format!("High memory usage: {}% (threshold: {}%)", 
                       metrics.memory_usage_percentage, self.threshold)
            }
            AlertCondition::CPUUsageAbove => {
                format!("High CPU usage: {}% (threshold: {}%)", 
                       metrics.cpu_usage_percentage, self.threshold)
            }
            AlertCondition::AccuracyBelow(model_name) => {
                format!("Low model accuracy: {} is below {}%", 
                       model_name, self.threshold * 100.0)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum AlertCondition {
    LatencyAbove(String),
    ThroughputBelow(String),
    MemoryUsageAbove,
    CPUUsageAbove,
    AccuracyBelow(String),
}

#[derive(Debug, Clone)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}
```

---

*This performance metrics framework provides comprehensive monitoring, benchmarking, and alerting capabilities for the Neural-Trader platform. For implementation details and usage examples, refer to the monitoring and observability modules in the codebase.*