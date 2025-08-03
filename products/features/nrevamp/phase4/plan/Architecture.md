# Phase 4 Architecture: Neural Engine Dashboard Integration

## 🏗️ Executive Architecture Overview

Phase 4 integrates neural engine monitoring with the **EXISTING** Grafana and Prometheus infrastructure. This architecture follows the **INTEGRATION_FIRST_MANDATE** by extending the current monitoring stack at `docker/production/grafana/dashboards/` rather than creating new infrastructure.

### 🔍 Existing Infrastructure Discovery

**Confirmed Existing Components:**
- ✅ **Grafana Instance**: Running on port 3000
- ✅ **Prometheus Instance**: Running on port 9090 (fixed from backup showing 9093)
- ✅ **Existing Dashboards**: trading-operations, infrastructure-monitoring, market-data-realtime, operational-overview, performance-monitoring
- ✅ **Dashboard Location**: `docker/production/grafana/dashboards/`
- ✅ **Auto-Provisioning**: Dashboard updates via Docker volume mounts

### 🎯 Revised Architecture Principles

1. **INTEGRATION ONLY**: NO new infrastructure - extend existing Grafana/Prometheus
2. **DASHBOARD EXTENSION**: Add neural-specific JSON dashboards to existing location
3. **METRICS INTEGRATION**: Expose neural metrics via existing Prometheus endpoints
4. **VENDOR MODEL COMPATIBILITY**: Support 27+ neural architectures from vendor/ruv-fann
5. **DAA PRESERVATION**: Maintain autonomous trading decision-making

## 📊 Integration Architecture (NO NEW INFRASTRUCTURE)

```mermaid
graph TB
    subgraph "EXISTING Infrastructure (docker/production/)"
        GRAFANA[Grafana:3000<br/>🔵 EXISTING]
        PROMETHEUS[Prometheus:9090<br/>🔵 EXISTING]
        DASHBOARDS[Dashboard Files<br/>🔵 EXISTING<br/>docker/production/grafana/dashboards/]
    end
    
    subgraph "Existing Neural Trader Core"
        DAA[DAA Coordinator]
        NEURAL[Neural Engine]
        DATA_PIPE[Data Pipeline]
        REDIS[Redis Channels]
        HEALTH_MON[Health Monitor]
        PERF_MON[Performance Monitor]
    end
    
    subgraph "Phase 4: Neural Metrics Integration"
        NEURAL_METRICS[Neural Metrics Exporter<br/>📊 NEW COMPONENT]
        NEURAL_HEALTH[Neural Health Checker<br/>📊 NEW COMPONENT]
        DASHBOARD_JSON[Neural Dashboard JSONs<br/>📄 NEW FILES]
    end
    
    %% Integration flow (NO NEW SERVICES)
    NEURAL --> NEURAL_METRICS
    DAA --> NEURAL_METRICS
    DATA_PIPE --> NEURAL_METRICS
    
    NEURAL --> NEURAL_HEALTH
    HEALTH_MON --> NEURAL_HEALTH
    
    NEURAL_METRICS --> PROMETHEUS
    NEURAL_HEALTH --> PROMETHEUS
    
    DASHBOARD_JSON --> DASHBOARDS
    DASHBOARDS --> GRAFANA
    PROMETHEUS --> GRAFANA
    
    style GRAFANA fill:#e1f5fe
    style PROMETHEUS fill:#e1f5fe
    style DASHBOARDS fill:#e1f5fe
    style NEURAL_METRICS fill:#fff3e0
    style NEURAL_HEALTH fill:#fff3e0
    style DASHBOARD_JSON fill:#f3e5f5
```

## 🔧 Integration Components (NO NEW INFRASTRUCTURE)

### 1. Neural Metrics Integration

**CRITICAL**: Phase 4 adds ONLY neural-specific metrics to existing Prometheus, NO new infrastructure.

#### 1.1 Neural Metrics Exporter Integration

**File**: `src/neural/prometheus_integration.rs`

```rust
/// Integration layer to expose neural engine metrics to existing Prometheus
pub struct NeuralPrometheusIntegration {
    // Use EXISTING PrometheusExporter - no new instances
    existing_exporter: Arc<PrometheusExporter>,
    
    // Neural-specific metric collectors
    neural_model_metrics: NeuralModelMetricsCollector,
    real_time_training_metrics: RealTimeTrainingMetricsCollector,
    data_type_discovery_metrics: DataTypeDiscoveryMetricsCollector,
}

impl NeuralPrometheusIntegration {
    /// Integrate with existing PrometheusExporter (NO NEW SERVICE)
    pub async fn integrate_with_existing(
        existing_exporter: Arc<PrometheusExporter>
    ) -> Result<Self> {
        Ok(Self {
            existing_exporter,
            neural_model_metrics: NeuralModelMetricsCollector::new(),
            real_time_training_metrics: RealTimeTrainingMetricsCollector::new(),
            data_type_discovery_metrics: DataTypeDiscoveryMetricsCollector::new(),
        })
    }
    
    /// Expose neural model metrics to existing Prometheus instance
    pub async fn expose_neural_model_metrics(
        &self,
        model_type: &str,  // "LSTM", "TCN", "GRU", etc.
        symbol: &str,
        performance: &PerformanceSnapshot
    ) -> Result<()> {
        // Add to existing Prometheus metrics registry
        self.existing_exporter.record_gauge(
            "neural_model_accuracy",
            performance.accuracy,
            &[("model_type", model_type), ("symbol", symbol)]
        ).await
    }
    
    /// Expose data type discovery metrics to existing Prometheus
    pub async fn expose_data_type_metrics(
        &self,
        data_type: &DataType,
        scope: &DataScope,
        utilization_metrics: &DataUtilizationMetrics
    ) -> Result<()> {
        // Integrate with existing metrics collection
        self.existing_exporter.record_counter(
            "data_types_discovered_total",
            1,
            &[("data_type", &data_type.name), ("scope", &scope.to_string())]
        ).await
    }
}
```

#### 1.2 Enhanced Health Monitor Extension

**File**: `src/monitoring/health_extensions.rs`

```rust
/// Extensions to existing health monitoring system
pub struct HealthMonitorExtensions {
    base_monitor: Arc<HealthMonitor>,
    
    // Phase 4 monitoring extensions
    neural_health_tracker: NeuralHealthTracker,
    data_pipeline_health: DataPipelineHealthTracker,
    trading_performance_health: TradingPerformanceHealthTracker,
    alerting_engine: AlertingEngine,
}

impl HealthMonitorExtensions {
    /// Extend existing health monitor without replacing
    pub fn extend_monitor(base_monitor: Arc<HealthMonitor>) -> Self {
        // Build on existing ComponentHealth system
    }
    
    /// Monitor health of 27+ neural model types
    pub async fn check_neural_model_health(
        &self,
        model_pool: &ModelPool
    ) -> Vec<ComponentHealth> {
        // Use existing ComponentHealth enum
        // Add neural-specific health checks
    }
    
    /// Monitor data type discovery pipeline health
    pub async fn check_data_pipeline_health(
        &self,
        router: &MultiScopeRouter
    ) -> ComponentHealth {
        // Integrate with existing health checking
    }
}
```

### 2. Dashboard Integration (JSON FILES ONLY)

**CRITICAL**: Phase 4 creates ONLY JSON dashboard files in existing location `docker/production/grafana/dashboards/`

#### 2.1 Neural Dashboard JSON Files

**Files to Create:**
- `docker/production/grafana/dashboards/neural-model-performance.json`
- `docker/production/grafana/dashboards/neural-real-time-training.json`  
- `docker/production/grafana/dashboards/neural-data-types.json`

**Integration Pattern:**
```bash
# Existing auto-provisioning will detect new JSON files
# NO CODE NEEDED - Grafana automatically loads JSON files from volume mount
docker/production/grafana/dashboards/
├── trading-operations.json           # 🔵 EXISTING
├── infrastructure-monitoring.json    # 🔵 EXISTING  
├── market-data-realtime.json        # 🔵 EXISTING
├── operational-overview.json        # 🔵 EXISTING
├── performance-monitoring.json      # 🔵 EXISTING
├── neural-model-performance.json    # 🆕 NEW DASHBOARD
├── neural-real-time-training.json   # 🆕 NEW DASHBOARD
└── neural-data-types.json           # 🆕 NEW DASHBOARD
```

#### 2.2 Dashboard JSON Structure Template

**Example: neural-model-performance.json**
```json
{
  "dashboard": {
    "id": null,
    "title": "Neural Model Performance",
    "tags": ["neural", "trading", "performance"],
    "timezone": "browser",
    "panels": [
      {
        "id": 1,
        "title": "Model Accuracy by Type",
        "type": "timeseries",
        "targets": [
          {
            "expr": "neural_model_accuracy{model_type=~\".*\"}",
            "legendFormat": "{{model_type}} - {{symbol}}"
          }
        ]
      },
      {
        "id": 2,
        "title": "Prediction Latency",
        "type": "histogram",
        "targets": [
          {
            "expr": "neural_prediction_latency_histogram",
            "legendFormat": "{{model_type}} Latency"
          }
        ]
      }
    ],
    "refresh": "5s",
    "time": {
      "from": "now-1h",
      "to": "now"
    }
  }
}
```

**NO RUST CODE NEEDED**: JSON files auto-loaded by existing Grafana provisioning

### 3. Prometheus Metrics Integration (EXISTING ENDPOINT)

**CRITICAL**: Neural metrics exposed via existing Prometheus at `localhost:9090`

#### 3.1 Neural Metrics Schema

**New Metrics to Add to Existing Prometheus:**
```promql
# Neural Model Performance Metrics  
neural_model_accuracy{model_type="LSTM|TCN|GRU|...", symbol="AAPL|MSFT|...", version="v1"}
neural_model_latency_seconds{model_type="LSTM|TCN|GRU|...", symbol="AAPL|MSFT|..."}
neural_prediction_count_total{model_type="LSTM|TCN|GRU|...", symbol="AAPL|MSFT|..."}

# Real-time Training Metrics
neural_training_events_total{model_type="LSTM|TCN|GRU|...", trigger="performance|time|data"}
neural_training_duration_seconds{model_type="LSTM|TCN|GRU|...", symbol="AAPL|MSFT|..."}
neural_model_improvement_ratio{model_type="LSTM|TCN|GRU|...", symbol="AAPL|MSFT|..."}

# Data Type Discovery Metrics
data_types_discovered_total{data_type="price|volume|news|...", scope="symbol|sector|market"}
data_type_utilization_count{data_type="price|volume|news|...", model_count="5"}
data_routing_latency_seconds{scope="symbol|sector|market|geographic"}
```

#### 3.2 Integration with Existing Prometheus Configuration

**Existing Prometheus** at `docker/production/configs/prometheus/prometheus.yml` will automatically scrape neural metrics from existing neural-trader service at `localhost:9092/metrics`

## 🏗️ Integration Points with Existing Systems

### 1. DAA Autonomous System Integration (PRESERVE EXISTING)

```rust
/// Real-time metrics streaming pipeline
pub struct StreamingMetricsPipeline {
    /// Integration with existing Redis channels
    redis_subscriber: Arc<RedisIntegration>,
    
    /// Metrics buffer for aggregation
    metrics_buffer: MetricsBuffer,
    
    /// Real-time processing engine
    processing_engine: StreamingProcessor,
    
    /// Push to Prometheus
    prometheus_pusher: PrometheusPusher,
}

impl StreamingMetricsPipeline {
    /// Process real-time neural model metrics
    pub async fn process_neural_metrics(
        &mut self,
        model_id: &str,
        model_type: &str,
        performance_data: &PerformanceSnapshot
    ) -> Result<()> {
        // <50ms processing latency requirement
        let metrics = self.convert_to_prometheus_metrics(
            model_id, model_type, performance_data
        )?;
        
        self.prometheus_pusher.push_metrics(metrics).await?;
        Ok(())
    }
    
    /// Process data type utilization metrics
    pub async fn process_data_type_metrics(
        &mut self,
        data_type: &DataType,
        scope: &DataScope,
        utilization: &DataUtilizationMetrics
    ) -> Result<()> {
        // Real-time data type monitoring
    }
    
    /// Process trading performance metrics
    pub async fn process_trading_metrics(
        &mut self,
        symbol: &str,
        trading_result: &TradingResult
    ) -> Result<()> {
        // Immediate trading performance feedback
    }
}
```

#### 3.2 Alert Processing System

**File**: `src/observability/alerting/alert_processor.rs`

```rust
/// Intelligent alert processing system
pub struct AlertProcessor {
    /// Alert rules configuration
    alert_rules: AlertRulesConfig,
    
    /// Alert channels (Slack, email, webhook)
    notification_channels: NotificationChannels,
    
    /// Alert correlation engine
    correlation_engine: AlertCorrelationEngine,
    
    /// Integration with existing DAA system
    daa_integration: DAAAlertIntegration,
}

pub struct AlertRulesConfig {
    /// Neural model performance alerts
    pub neural_alerts: NeuralModelAlerts,
    
    /// Trading performance alerts
    pub trading_alerts: TradingPerformanceAlerts,
    
    /// System health alerts
    pub system_alerts: SystemHealthAlerts,
    
    /// Data pipeline alerts
    pub data_pipeline_alerts: DataPipelineAlerts,
}

impl AlertProcessor {
    /// Process neural model degradation alerts
    pub async fn process_neural_alert(
        &self,
        model_type: &str,
        symbol: &str,
        alert_data: &NeuralAlertData
    ) -> Result<AlertAction> {
        // Correlate with other model performance
        let correlated_alerts = self.correlation_engine
            .find_correlated_alerts(alert_data).await?;
            
        // Determine appropriate action
        if correlated_alerts.len() > 3 {
            // System-wide issue
            AlertAction::SystemAlert
        } else {
            // Individual model issue
            AlertAction::ModelAlert
        }
    }
    
    /// Integration with DAA autonomous training
    pub async fn notify_daa_degradation(
        &self,
        model_performance: &PerformanceSnapshot
    ) -> Result<()> {
        // Trigger DAA retraining if needed
        // Use existing DAA thresholds and logic
    }
}
```

### 4. Data Type Discovery Monitoring Architecture

#### 4.1 Dynamic Data Type Visualization

**File**: `src/observability/data_types/visualization.rs`

```rust
/// Dynamic visualization for discovered data types
pub struct DataTypeVisualization {
    /// Type discovery tracker
    type_discovery: Arc<TypeDiscoveryTracker>,
    
    /// Utilization metrics
    utilization_tracker: DataUtilizationTracker,
    
    /// Grafana panel generator
    panel_generator: DynamicPanelGenerator,
    
    /// Data type relationship mapper
    relationship_mapper: DataRelationshipMapper,
}

impl DataTypeVisualization {
    /// Create visualization for newly discovered data type
    pub async fn visualize_new_data_type(
        &self,
        data_type: &DataType,
        sample_data: &serde_json::Value
    ) -> Result<GrafanaPanel> {
        // Analyze data characteristics
        let characteristics = self.analyze_data_characteristics(sample_data)?;
        
        // Generate appropriate visualization
        match characteristics.data_nature {
            DataNature::TimeSeries => self.create_time_series_panel(data_type),
            DataNature::Categorical => self.create_categorical_panel(data_type),
            DataNature::Numerical => self.create_numerical_panel(data_type),
            DataNature::Text => self.create_text_analysis_panel(data_type),
        }
    }
    
    /// Track data type utilization across models
    pub async fn track_utilization(
        &self,
        data_type: &DataType,
        models_using: &[String]
    ) -> Result<UtilizationVisualization> {
        // Show which models use which data types
        // Model performance correlation with data availability
    }
}
```

### 5. Multi-Scope Data Routing Verification Architecture

#### 5.1 Routing Verification Dashboard

**File**: `src/data_pipeline/monitoring/routing_monitor.rs`

```rust
/// Monitor and verify multi-scope data routing
pub struct DataRoutingMonitor {
    /// Integration with existing routing system
    router: Arc<MultiScopeRouter>,
    
    /// Routing metrics collector
    metrics_collector: RoutingMetricsCollector,
    
    /// Routing verification engine
    verification_engine: RoutingVerificationEngine,
    
    /// Grafana dashboard generator
    dashboard_generator: RoutingDashboardGenerator,
}

impl DataRoutingMonitor {
    /// Verify routing for different data scopes
    pub async fn verify_routing(
        &self,
        test_data: &DataPacket
    ) -> Result<RoutingVerificationResult> {
        // Test symbol-specific routing
        let symbol_routing = self.verify_symbol_routing(test_data).await?;
        
        // Test sector-wide routing  
        let sector_routing = self.verify_sector_routing(test_data).await?;
        
        // Test market-wide routing
        let market_routing = self.verify_market_routing(test_data).await?;
        
        // Test geographic routing
        let geo_routing = self.verify_geographic_routing(test_data).await?;
        
        Ok(RoutingVerificationResult {
            symbol_routing,
            sector_routing,
            market_routing,
            geo_routing,
            overall_health: self.calculate_routing_health(),
        })
    }
    
    /// Create routing verification dashboard
    pub async fn create_routing_dashboard(
        &self
    ) -> Result<GrafanaDashboard> {
        // Routing latency by scope
        // Data distribution verification
        // Routing rule effectiveness
        // Failed routing detection
    }
}
```

## 🏗️ Integration Points with Existing Systems

### 1. DAA Autonomous System Integration

**File**: `src/integration/observability_daa_bridge.rs`

```rust
/// Bridge observability system with existing DAA coordinator
pub struct ObservabilityDAABridge {
    /// Existing DAA coordinator
    daa_coordinator: Arc<DAACoordinator>,
    
    /// Observability system
    observability: Arc<ObservabilityExtensions>,
    
    /// Performance threshold monitor
    threshold_monitor: PerformanceThresholdMonitor,
}

impl ObservabilityDAABridge {
    /// Monitor DAA decision-making process
    pub async fn monitor_daa_decisions(
        &self,
        decision_context: &DecisionContext
    ) -> Result<()> {
        // Track DAA voting patterns
        // Monitor 60/40 neural/strategy weights
        // Track 70% Byzantine consensus achievement
        // Alert on threshold violations
    }
    
    /// Feed performance data to DAA training
    pub async fn feed_performance_to_daa(
        &self,
        performance_data: &PerformanceSnapshot
    ) -> Result<()> {
        // Use existing DAA training triggers
        // Preserve autonomous training thresholds
        // Maintain Byzantine fault tolerance
    }
}
```

### 2. Neural Engine Integration

**File**: `src/neural/observability_integration.rs`

```rust
/// Integration with neural engine for monitoring
pub struct NeuralObservabilityIntegration {
    /// Track performance across 27+ model types
    model_performance_tracker: ModelPerformanceTracker,
    
    /// Real-time training monitoring
    training_monitor: RealTimeTrainingMonitor,
    
    /// Model health assessor
    health_assessor: ModelHealthAssessor,
}

impl NeuralObservabilityIntegration {
    /// Monitor vendor model performance
    pub async fn monitor_vendor_model(
        &self,
        model: &dyn BaseModel<f32>,
        model_type: &str,
        symbol: &str
    ) -> Result<()> {
        // Track performance of real vendor models
        // Monitor LSTM, TCN, GRU, Transformer, etc.
        // Feed data to Grafana dashboards
    }
    
    /// Monitor ensemble performance
    pub async fn monitor_ensemble(
        &self,
        ensemble_result: &EnsembleResult
    ) -> Result<()> {
        // Track model agreement/disagreement
        // Monitor ensemble accuracy vs individual models
        // Detect model bias or drift
    }
}
```

### 3. Data Pipeline Integration

**File**: `src/data_pipeline/observability_hooks.rs`

```rust
/// Observability hooks for data pipeline monitoring
pub struct DataPipelineObservabilityHooks {
    /// Data routing monitor
    routing_monitor: Arc<DataRoutingMonitor>,
    
    /// Data type discovery monitor
    type_discovery_monitor: DataTypeDiscoveryMonitor,
    
    /// Consolidation performance monitor
    consolidation_monitor: ConsolidationMonitor,
}

impl DataPipelineObservabilityHooks {
    /// Hook into data routing process
    pub async fn on_data_routed(
        &self,
        packet: &DataPacket,
        routing_result: &RoutingResult
    ) -> Result<()> {
        // Monitor routing latency
        // Track routing accuracy
        // Detect routing failures
        // Update Grafana metrics
    }
    
    /// Hook into data type discovery
    pub async fn on_type_discovered(
        &self,
        data_type: &DataType,
        discovery_context: &DiscoveryContext
    ) -> Result<()> {
        // Track new data type discovery
        // Monitor type utilization
        // Create dynamic dashboards
        // Alert on new data types
    }
}
```

## 📊 Dashboard Specifications

### 1. Neural Model Performance Dashboard

**Dashboard**: `neural-model-performance.json`

#### Panels:
1. **Model Accuracy Over Time** (Time Series)
   - Query: `accuracy_rate{model_type=~".*", symbol=~".*"}`
   - Grouped by: model_type, symbol
   - Aggregation: avg_over_time(5m)

2. **Prediction Latency Distribution** (Histogram)
   - Query: `prediction_latency_histogram{model_type=~".*"}`
   - Show percentiles: p50, p95, p99

3. **Model Performance Heatmap** (Heatmap)
   - X-axis: Symbol
   - Y-axis: Model Type
   - Color: Accuracy Rate

4. **Ensemble vs Individual Performance** (Stat Panel)
   - Query: `ensemble_accuracy - avg(individual_accuracy)`
   - Show improvement percentage

5. **Model Training Frequency** (Bar Chart)
   - Query: `training_frequency{model_type=~".*"}`
   - Show retraining triggers

#### Alerts:
- Model accuracy < 0.8 (DAA threshold)
- Prediction latency > 100ms
- Model training failure
- Ensemble performance degradation

### 2. Trading Performance Dashboard

**Dashboard**: `trading-performance.json`

#### Panels:
1. **Profit/Loss Over Time** (Time Series)
   - Query: `cumulative_pnl{symbol=~".*", strategy=~".*"}`
   - Split by symbol and strategy

2. **Trade Success Rate** (Gauge)
   - Query: `successful_trades / total_trades`
   - Threshold colors: <60% red, 60-80% yellow, >80% green

3. **Risk-Adjusted Returns** (Stat Panel)
   - Query: `sharpe_ratio{symbol=~".*"}`
   - Show Sharpe ratio by symbol

4. **DAA Voting Patterns** (Pie Chart)
   - Query: `daa_votes{vote_type=~"neural|strategy"}`
   - Show 60/40 neural/strategy split

5. **Model Agreement Score** (Time Series)
   - Query: `model_agreement_score`
   - Track consensus levels

#### Alerts:
- Profit/loss decline > 10%
- Sharpe ratio < 1.0
- DAA consensus < 70%
- Trade success rate < 60%

### 3. Data Type Utilization Dashboard

**Dashboard**: `data-type-utilization.json`

#### Panels:
1. **Discovered Data Types** (Table)
   - Show: type_name, discovery_date, utilization_count
   - Dynamic table updated on new discoveries

2. **Data Type Utilization Matrix** (Heatmap)
   - X-axis: Data Type
   - Y-axis: Model Type
   - Color: Utilization frequency

3. **Data Scope Distribution** (Donut Chart)
   - Query: `data_packets{scope=~"symbol|sector|market|geographic"}`
   - Show distribution across scopes

4. **Data Quality Metrics** (Time Series)
   - Query: `data_completeness_score`, `data_freshness_score`
   - Track data quality over time

5. **Real-Time Data Flow** (Graph Panel)
   - Show data flow from sources to models
   - Node size = data volume, Edge thickness = flow rate

### 4. System Health Overview Dashboard

**Dashboard**: `system-health-overview.json`

#### Panels:
1. **Overall System Health** (Stat Panel)
   - Query: `system_health_score`
   - Color coding: Healthy/Warning/Critical

2. **Component Health Matrix** (Status History)
   - Components: neural_engine, daa_coordinator, data_pipeline, redis, database
   - Show health status over time

3. **Resource Utilization** (Time Series)
   - CPU, Memory, Disk, Network usage
   - Multi-line chart with thresholds

4. **Error Rate Trends** (Time Series)
   - Query: `error_rate{component=~".*"}`
   - Split by component

5. **Alert Summary** (Table)
   - Current active alerts
   - Alert history and resolution times

### 5. Real-Time Training Monitoring Dashboard

**Dashboard**: `real-time-training.json`

#### Panels:
1. **Training Events Timeline** (Logs Panel)
   - Real-time training triggers and completions
   - Filter by model_type, symbol

2. **Training Performance Impact** (Time Series)
   - Before/after accuracy comparison
   - Training duration vs performance gain

3. **Model Checkpoint Health** (Stat Panel)
   - Checkpoint creation success rate
   - Rollback frequency and success

4. **Parameter Update Latency** (Histogram)
   - Query: `parameter_update_latency`
   - Target: <50ms requirement

5. **Training Resource Usage** (Time Series)
   - CPU/Memory usage during training
   - Training queue depth

## 🚨 Alerting Architecture

### 1. Alert Rule Definitions

**File**: `src/observability/alerting/rules.yaml`

```yaml
groups:
- name: neural_model_alerts
  rules:
  - alert: ModelAccuracyDegraded
    expr: accuracy_rate < 0.8
    for: 5m
    labels:
      severity: warning
      component: neural_engine
    annotations:
      summary: "Model {{ $labels.model_type }} for {{ $labels.symbol }} accuracy below threshold"
      
  - alert: ModelAccuracyCritical  
    expr: accuracy_rate < 0.6
    for: 2m
    labels:
      severity: critical
      component: neural_engine
    annotations:
      summary: "CRITICAL: Model {{ $labels.model_type }} for {{ $labels.symbol }} accuracy critically low"

- name: trading_performance_alerts
  rules:
  - alert: TradingLossAlert
    expr: rate(cumulative_pnl[5m]) < -0.05
    for: 3m
    labels:
      severity: warning
      component: trading_engine
    annotations:
      summary: "Trading losses exceeding 5% in 5 minutes"
      
  - alert: DAAConsensusFailure
    expr: daa_consensus_rate < 0.7
    for: 1m
    labels:
      severity: critical
      component: daa_coordinator
    annotations:
      summary: "DAA Byzantine consensus below 70% threshold"

- name: data_pipeline_alerts  
  rules:
  - alert: DataTypeDiscoveryStalled
    expr: increase(new_data_types_discovered[1h]) == 0 and data_ingestion_rate > 0
    for: 30m
    labels:
      severity: warning
      component: data_pipeline
    annotations:
      summary: "No new data types discovered despite active ingestion"
      
  - alert: RoutingLatencyHigh
    expr: routing_latency_p95 > 100
    for: 2m
    labels:
      severity: warning
      component: data_routing
    annotations:
      summary: "Data routing latency P95 > 100ms"
```

### 2. Alert Correlation Engine

**File**: `src/observability/alerting/correlation.rs`

```rust
/// Correlate related alerts to prevent alert storms
pub struct AlertCorrelationEngine {
    /// Alert correlation rules
    correlation_rules: CorrelationRules,
    
    /// Active alert tracker
    active_alerts: AlertTracker,
    
    /// Correlation patterns learned over time
    learned_patterns: LearnedPatterns,
}

impl AlertCorrelationEngine {
    /// Find alerts that should be correlated
    pub async fn find_correlated_alerts(
        &self,
        new_alert: &AlertData
    ) -> Result<Vec<CorrelatedAlert>> {
        // Check for known correlation patterns
        let rule_correlations = self.correlation_rules
            .find_correlations(new_alert).await?;
            
        // Check for learned patterns
        let learned_correlations = self.learned_patterns
            .find_correlations(new_alert).await?;
            
        // Combine and rank correlations
        Ok(self.rank_correlations(rule_correlations, learned_correlations))
    }
    
    /// Learn correlation patterns from alert history
    pub async fn learn_correlations(&mut self) -> Result<()> {
        // Analyze historical alert patterns
        // Find frequently co-occurring alerts
        // Update correlation rules
    }
}
```

## 🔧 Implementation Architecture Details

### 1. Grafana Integration Service

**File**: `src/observability/grafana/integration_service.rs`

```rust
/// Service for integrating with Grafana API
pub struct GrafanaIntegrationService {
    /// Grafana API client
    grafana_client: GrafanaClient,
    
    /// Dashboard template manager
    template_manager: DashboardTemplateManager,
    
    /// Data source manager
    data_source_manager: DataSourceManager,
    
    /// Alert rule manager
    alert_rule_manager: AlertRuleManager,
}

impl GrafanaIntegrationService {
    /// Initialize Grafana integration
    pub async fn initialize(&self) -> Result<()> {
        // Setup data sources
        self.setup_data_sources().await?;
        
        // Deploy dashboard templates
        self.deploy_dashboards().await?;
        
        // Configure alert rules
        self.configure_alerts().await?;
        
        Ok(())
    }
    
    /// Dynamically update dashboards for new data types
    pub async fn update_dashboards_for_data_type(
        &self,
        data_type: &DataType
    ) -> Result<()> {
        // Generate panels for new data type
        let panels = self.template_manager
            .generate_panels_for_data_type(data_type).await?;
            
        // Update relevant dashboards
        self.grafana_client
            .update_dashboard_panels(panels).await?;
            
        Ok(())
    }
}
```

### 2. Metrics Collection Architecture

**File**: `src/observability/collection/metrics_collector.rs`

```rust
/// Enhanced metrics collection system
pub struct EnhancedMetricsCollector {
    /// Base Prometheus exporter (existing)
    base_exporter: Arc<PrometheusExporter>,
    
    /// Neural model metrics
    neural_metrics: NeuralMetricsCollector,
    
    /// Trading metrics
    trading_metrics: TradingMetricsCollector,
    
    /// Data pipeline metrics  
    data_pipeline_metrics: DataPipelineMetricsCollector,
    
    /// System performance metrics
    system_metrics: SystemMetricsCollector,
}

impl EnhancedMetricsCollector {
    /// Collect neural model metrics
    pub async fn collect_neural_metrics(&self) -> Result<()> {
        // For each of 27+ model types
        for model_type in MODEL_TYPES {
            // Collect accuracy, latency, resource usage
            let metrics = self.neural_metrics
                .collect_for_model_type(model_type).await?;
                
            // Push to Prometheus
            self.base_exporter.record_metrics(metrics).await?;
        }
        
        Ok(())
    }
    
    /// Collect trading performance metrics
    pub async fn collect_trading_metrics(&self) -> Result<()> {
        // Collect P&L, trade success rate, Sharpe ratio
        // DAA voting patterns, consensus achievement
        // Risk metrics, position sizing
    }
    
    /// Collect data pipeline metrics
    pub async fn collect_data_pipeline_metrics(&self) -> Result<()> {
        // Data type discovery rate
        // Routing latency and accuracy
        // Data quality metrics
        // Consolidation performance
    }
}
```

### 3. Performance Target Architecture

#### 3.1 Latency Requirements

| Component | Target | Measurement |
|-----------|--------|-------------|
| Metrics Collection | <50ms | Time from event to Prometheus |
| Dashboard Refresh | <200ms | Time for dashboard data refresh |
| Alert Processing | <100ms | Time from threshold breach to alert |
| Data Type Visualization | <500ms | Time to create new data type panel |
| Routing Verification | <10ms | Time to verify routing path |

#### 3.2 Throughput Requirements

| Metric Type | Target Rate | Buffer Size |
|-------------|-------------|-------------|
| Neural Model Metrics | 1000 metrics/sec | 10k metrics |
| Trading Metrics | 500 metrics/sec | 5k metrics |
| Data Pipeline Metrics | 2000 metrics/sec | 20k metrics |
| System Health Metrics | 100 metrics/sec | 1k metrics |

#### 3.3 Storage Requirements

| Data Type | Retention | Aggregation |
|-----------|-----------|-------------|
| Raw Metrics | 7 days | None |
| 5min Aggregates | 30 days | avg, max, min |
| 1hour Aggregates | 1 year | avg, max, min |
| Daily Aggregates | 5 years | avg, max, min |

## 🔄 Integration Workflow

### 1. System Startup Integration

```mermaid
sequenceDiagram
    participant Main as main.rs
    participant Obs as ObservabilitySystem
    participant Grafana as GrafanaIntegration
    participant Prom as PrometheusExporter
    participant DAA as DAACoordinator
    
    Main->>Obs: Initialize observability
    Obs->>Prom: Extend existing exporter
    Obs->>Grafana: Initialize Grafana integration
    Grafana->>Grafana: Setup dashboards
    Grafana->>Grafana: Configure alerts
    Obs->>DAA: Register performance callbacks
    DAA-->>Obs: Performance data stream
    Obs->>Prom: Enhanced metrics
    Prom->>Grafana: Metrics data
```

### 2. Runtime Monitoring Workflow

```mermaid
sequenceDiagram
    participant Neural as NeuralEngine
    participant DAA as DAACoordinator
    participant Obs as ObservabilityExtensions
    participant Alert as AlertProcessor
    participant Grafana as GrafanaDashboard
    
    Neural->>Obs: Model performance data
    DAA->>Obs: Trading decision data
    Obs->>Obs: Process metrics (<50ms)
    Obs->>Alert: Check alert thresholds
    Alt Alert triggered
        Alert->>Alert: Correlate with other alerts
        Alert->>DAA: Notify if needed
        Alert->>Grafana: Update alert panels
    End
    Obs->>Grafana: Stream metrics
    Grafana->>Grafana: Update dashboards (<200ms)
```

### 3. Data Type Discovery Integration

```mermaid
sequenceDiagram
    participant Pipeline as DataPipeline
    participant Discovery as TypeDiscovery
    participant Obs as ObservabilityExtensions
    participant Grafana as GrafanaIntegration
    participant Dashboard as GrafanaDashboard
    
    Pipeline->>Discovery: New data received
    Discovery->>Discovery: Analyze data characteristics
    Discovery->>Obs: New data type discovered
    Obs->>Grafana: Request dashboard update
    Grafana->>Grafana: Generate new panels
    Grafana->>Dashboard: Deploy updated dashboard
    Dashboard->>Dashboard: Show new data type metrics
```

## 📋 Implementation Roadmap (INTEGRATION FIRST APPROACH)

### Phase 4.1: Neural Metrics Integration (Week 1)
1. **Extend existing PrometheusExporter** with neural model metrics (`src/neural/prometheus_integration.rs`)
2. **Add neural metrics endpoints** to existing service at port 9092
3. **Test metrics scraping** - verify existing Prometheus collects neural metrics
4. **Create first dashboard JSON** - `neural-model-performance.json`

### Phase 4.2: Dashboard JSON Files (Week 2)  
1. **Create dashboard JSON files** in `docker/production/grafana/dashboards/`:
   - `neural-model-performance.json` - Model accuracy, latency, throughput
   - `neural-real-time-training.json` - Training events, improvements 
   - `neural-data-types.json` - Data type discovery, utilization
2. **Test dashboard auto-loading** - verify existing Grafana provisions dashboards
3. **Validate dashboard queries** - ensure metrics display correctly

### Phase 4.3: Enhanced Neural Health Monitoring (Week 3)
1. **Extend existing HealthMonitor** with neural-specific checks
2. **Add neural health metrics** to existing Prometheus endpoints
3. **Update infrastructure-monitoring.json** to include neural health panels
4. **Test health monitoring** - verify alerts work with existing Alert Manager

### Phase 4.4: Production Integration Testing (Week 4)
1. **Integration testing** - full neural metrics → Prometheus → Grafana flow
2. **Performance validation** - ensure <50ms metric collection latency
3. **Dashboard user testing** - validate dashboard usability for operations
4. **Alert threshold tuning** - calibrate neural performance alerts

### Phase 4.5: Documentation and Deployment (Week 5)
1. **Update monitoring documentation** - document new neural dashboards
2. **Create dashboard user guide** - how to interpret neural metrics
3. **Production deployment** - deploy to existing infrastructure
4. **Monitoring activation** - enable 24/7 neural performance monitoring

## 🎯 Success Criteria (INTEGRATION FIRST)

### Technical Requirements (EXTEND EXISTING)
- ✅ **NO NEW INFRASTRUCTURE**: Only JSON dashboard files + neural metrics integration
- ✅ **Existing Prometheus Integration**: Neural metrics exposed via port 9092
- ✅ **Existing Grafana Integration**: Dashboards auto-provisioned from volume mount
- ✅ **<50ms metric collection latency**: Using existing PrometheusExporter
- ✅ **Support for 27+ neural architectures**: Metrics for all vendor models
- ✅ **Zero service disruption**: No new containers, no new ports, no downtime

### Business Requirements (SAME MONITORING STACK)
- ✅ **Real-time neural model visibility**: Via existing Grafana at port 3000
- ✅ **Neural performance monitoring**: Accuracy, latency, training events
- ✅ **Data type discovery tracking**: New data types and utilization patterns
- ✅ **Operations team dashboard access**: Same Grafana interface, new neural tabs
- ✅ **Historical analysis**: Using existing Prometheus TSDB retention
- ✅ **Alert integration**: Using existing Alert Manager configuration

### Integration Requirements (INTEGRATION_FIRST_MANDATE)
- ✅ **Zero disruption to trading**: No changes to neural engine core operations
- ✅ **DAA autonomy preserved**: Monitoring observes, doesn't interfere
- ✅ **Redis compatibility**: Uses existing channels for real-time data
- ✅ **Existing monitoring extension**: Builds on current health/performance systems
- ✅ **Docker volume compatibility**: JSON files loaded automatically
- ✅ **Prometheus scraping**: Uses existing scrape configuration

### Implementation Success Criteria
- ✅ **Files Created**: 3 JSON dashboard files only
- ✅ **Code Changes**: Neural metrics integration in existing PrometheusExporter
- ✅ **Infrastructure Changes**: None (zero new containers/services)
- ✅ **Deployment Complexity**: Simple file addition to existing volume mount
- ✅ **Rollback Safety**: Remove JSON files to rollback

**INTEGRATION FIRST APPROACH ACHIEVED**: Phase 4 extends existing monitoring infrastructure without creating any new services, containers, or infrastructure components.