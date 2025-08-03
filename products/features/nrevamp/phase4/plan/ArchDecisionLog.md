# Phase 4 Architectural Decision Records (ADRs)
# Data Type Scaling and Neural Engine Monitoring

**Critical Context**: All Phase 4 decisions build upon the successful Phase 3 implementation while following the Integration-First Mandate. We EXTEND existing monitoring and data capabilities rather than replace them.

**Phase 3 Foundation to Build Upon**:
- Dynamic data type discovery system operational
- Channel-agnostic Redis ingestion working with any pattern
- Real-time adaptive model training with <50ms parameter updates
- Enhanced performance tracking with comprehensive model analytics
- 90% memory reduction maintained through sector-based sharing
- Full DAA autonomous trading capabilities preserved and enhanced

---

## ADR-P4-001: Grafana vs Custom Monitoring Dashboard

**Status**: Accepted  
**Date**: 2025-08-02  
**Context**: Phase 4 requires comprehensive neural engine observability and monitoring to validate Phase 3 real-time adaptive training capabilities

### Decision

**ADOPT Grafana for neural engine monitoring** while preserving existing health monitoring systems.

We will integrate Grafana dashboards with the existing monitoring infrastructure rather than build custom solutions:

```rust
// EXTENSION to existing health monitoring
pub struct NeuralEngineMonitoring {
    // Preserve existing health checks
    existing_health_monitor: Arc<HealthMonitorService>,
    
    // NEW: Grafana integration (EXTENSION)
    grafana_client: GrafanaClient,
    metrics_collector: MetricsCollector,
    dashboard_manager: DashboardManager,
}

// NEW: Grafana-specific metrics structure
#[derive(Debug, Clone, Serialize)]
pub struct NeuralEngineMetrics {
    // Model performance metrics
    pub prediction_accuracy: f64,
    pub training_convergence_rate: f64,
    pub adaptation_frequency: f64,
    
    // Data type utilization
    pub active_data_types: HashMap<String, DataTypeUtilization>,
    pub data_completeness_scores: HashMap<String, f64>,
    
    // Real-time training visualization
    pub parameter_update_history: Vec<ParameterUpdate>,
    pub learning_curves: HashMap<String, Vec<LearningPoint>>,
    pub gradient_flows: HashMap<String, GradientMetrics>,
    
    // Model activation monitoring
    pub lazy_loading_events: Vec<ModelActivationEvent>,
    pub resource_usage_by_model: HashMap<String, ResourceMetrics>,
    
    // Performance correlation analysis
    pub trading_vs_prediction_correlation: f64,
    pub data_completeness_impact: HashMap<String, f64>,
}
```

### Options Considered

#### Option 1: Custom Dashboard Development
**Pros**:
- Full control over interface design
- Tight integration with existing code
- No external dependencies

**Cons**:
- Significant development time (4-6 weeks)
- Maintenance overhead
- Limited visualization capabilities compared to Grafana
- Duplicates existing monitoring patterns

#### Option 2: Grafana Integration (CHOSEN)
**Pros**:
- Industry-standard monitoring solution
- Rich visualization capabilities out-of-the-box
- Extensive alerting and notification features
- Time-series data handling optimized for neural engine metrics
- Integration with existing Prometheus/InfluxDB if available

**Cons**:
- Additional infrastructure dependency
- Learning curve for team
- Requires metrics formatting for Grafana consumption

#### Option 3: Extend Existing Health Monitoring
**Pros**:
- Builds on existing infrastructure
- No new dependencies

**Cons**:
- Limited visualization capabilities
- Not optimized for time-series neural engine data
- Would require significant enhancement to match Grafana features

### Rationale

**WHY GRAFANA**:
1. **Time-Series Optimization**: Neural engine metrics (learning curves, parameter updates, adaptation rates) are inherently time-series data that Grafana handles excellently
2. **Real-Time Visualization**: Phase 3's <50ms parameter updates require real-time dashboard capabilities
3. **Correlation Analysis**: Grafana's ability to overlay multiple metrics enables trading vs. prediction correlation tracking
4. **Alerting Integration**: Can integrate with existing health monitoring for comprehensive alert management
5. **Industry Standard**: Proven solution for ML/AI model monitoring

**Integration with Existing Systems**:
- Preserve all existing health monitoring endpoints
- Feed neural engine metrics to both existing health system AND Grafana
- Use existing Redis channels for metrics collection
- Integrate alerting with current notification systems

### Implementation Plan

1. **Preserve Existing Health Monitoring**: All current health checks remain functional
2. **Add Grafana Metrics Collection**: New service to collect and format neural engine metrics
3. **Create Grafana Dashboards**: Specialized dashboards for neural engine observability
4. **Integrate Alerting**: Connect Grafana alerts with existing health monitoring system
5. **Maintain Dual Monitoring**: Both systems operational for redundancy and comparison

---

## ADR-P4-002: Data Type Registration and Discovery Approach

**Status**: Accepted  
**Date**: 2025-08-02  
**Context**: Phase 4 requires scaling data types to validate Phase 3's dynamic discovery system while adding sentiment, economic, and alternative data modalities

### Decision

**CONTINUE with characteristic-based data type registration** established in Phase 3, extending it for new data modalities.

We will build upon the proven dynamic data type discovery system rather than create modality-specific registration:

```rust
// EXTENSION to existing Phase 3 data type discovery
pub struct EnhancedDataTypeRegistry {
    // EXISTING Phase 3 foundation preserved
    characteristic_matcher: CharacteristicMatcher,
    data_type_catalog: HashMap<String, DataTypeDefinition>,
    
    // NEW: Modality-specific extensions (EXTENSION)
    sentiment_characteristics: SentimentDataCharacteristics,
    economic_characteristics: EconomicDataCharacteristics,
    alternative_characteristics: AlternativeDataCharacteristics,
    
    // Enhanced discovery capabilities
    cross_modality_correlation: CrossModalityAnalyzer,
    data_quality_validator: DataQualityValidator,
}

// NEW: Extended data characteristics for Phase 4 modalities
#[derive(Debug, Clone)]
pub struct SentimentDataCharacteristics {
    pub news_sentiment_patterns: Vec<String>,
    pub social_media_indicators: Vec<String>,
    pub analyst_rating_formats: Vec<String>,
    pub sentiment_score_ranges: (f64, f64),
    pub temporal_resolution: Duration,
}

#[derive(Debug, Clone)]
pub struct EconomicDataCharacteristics {
    pub gdp_indicators: Vec<String>,
    pub inflation_metrics: Vec<String>,
    pub interest_rate_patterns: Vec<String>,
    pub employment_data_formats: Vec<String>,
    pub release_schedules: Vec<ReleaseSchedule>,
}

#[derive(Debug, Clone)]
pub struct AlternativeDataCharacteristics {
    pub satellite_imagery_metadata: ImageryMetadata,
    pub web_traffic_patterns: TrafficPatterns,
    pub transaction_data_formats: Vec<String>,
    pub weather_data_schemas: Vec<String>,
    pub geospatial_indicators: Vec<String>,
}
```

### Options Considered

#### Option 1: Modality-Specific Registration Systems
**Pros**:
- Specialized handling for each data type
- Optimized parsing for specific formats

**Cons**:
- Violates Phase 3's dynamic discovery principle
- Creates hardcoded assumptions
- Requires code changes for new data types
- Multiple parallel registration systems

#### Option 2: Characteristic-Based Extension (CHOSEN)
**Pros**:
- Builds on proven Phase 3 foundation
- Maintains zero-code-change principle for new data types
- Unified discovery mechanism across all modalities
- Scalable to unlimited data types

**Cons**:
- More complex characteristic definition
- Requires careful characteristic design

#### Option 3: Hybrid Approach
**Pros**:
- Flexibility for both approaches

**Cons**:
- Complexity overhead
- Inconsistent registration patterns
- Maintenance burden

### Rationale

**WHY CHARACTERISTIC-BASED EXTENSION**:
1. **Phase 3 Validation**: The characteristic-based approach successfully handled dynamic discovery
2. **Zero Code Changes**: New data types integrate without neural engine modifications
3. **Unified Architecture**: Single discovery mechanism handles all data modalities
4. **Future-Proof**: Approach scales to any future data type without architectural changes
5. **Performance Proven**: <50ms integration times demonstrated in Phase 3

**Examples of Characteristic-Based Discovery**:

```rust
// Sentiment data discovered by characteristics, not hardcoded types
let sentiment_characteristics = DataCharacteristics {
    data_frequency: DataFrequency::Irregular,
    data_scope: DataScope::SymbolSpecific,
    data_nature: DataNature::Sentiment,
    value_range: Some((0.0, 1.0)),
    required_fields: vec!["sentiment_score", "confidence"],
    optional_fields: vec!["news_source", "social_volume"],
};

// Economic data discovered by temporal and scope patterns
let economic_characteristics = DataCharacteristics {
    data_frequency: DataFrequency::Scheduled(ReleaseSchedule::Monthly),
    data_scope: DataScope::MarketWide,
    data_nature: DataNature::Economic,
    temporal_lag: Some(Duration::days(30)),
    impact_scope: vec![DataScope::MarketWide, DataScope::SectorWide],
};
```

### Implementation Plan

1. **Extend Existing Registry**: Add modality characteristics to Phase 3 registry
2. **Validate Discovery**: Confirm new data types auto-register without code changes
3. **Test Multi-Scope Routing**: Validate market-wide and sector-wide data distribution
4. **Measure Performance Impact**: Track model improvement with additional data types
5. **Monitor Resource Usage**: Ensure efficient scaling with new modalities

---

## ADR-P4-003: Monitoring Data Storage Strategy

**Status**: Accepted  
**Date**: 2025-08-02  
**Context**: Phase 4 requires persistent storage for neural engine monitoring data, learning curves, and performance correlation analysis

### Decision

**INTEGRATE TimescaleDB for time-series monitoring data** while preserving existing Redis for real-time operations.

We will add TimescaleDB as a specialized time-series backend for Grafana while maintaining all existing data storage patterns:

```rust
// EXTENSION to existing data storage architecture
pub struct HybridDataStorage {
    // EXISTING storage preserved
    redis_client: Arc<redis::Client>,          // Real-time operations
    existing_db: Arc<DatabaseConnection>,      // Existing persistent storage
    
    // NEW: Time-series monitoring storage (EXTENSION)
    timescale_client: Arc<TimescaleDBClient>,
    metrics_writer: MetricsWriter,
    data_retention_manager: DataRetentionManager,
}

// NEW: Time-series metrics schema for monitoring
#[derive(Debug, Clone)]
pub struct TimeSeriesMetrics {
    pub timestamp: DateTime<Utc>,
    pub metric_type: MetricType,
    pub model_id: String,
    pub symbol: String,
    pub sector: String,
    
    // Neural engine metrics
    pub prediction_accuracy: f64,
    pub training_loss: f64,
    pub parameter_updates: i32,
    pub adaptation_rate: f64,
    
    // Data type utilization
    pub active_data_types: i32,
    pub data_completeness: f64,
    pub new_types_discovered: i32,
    
    // Performance correlation
    pub trading_outcome: Option<f64>,
    pub prediction_confidence: f64,
    pub market_correlation: f64,
}
```

### Options Considered

#### Option 1: Extend Existing Database
**Pros**:
- No new infrastructure
- Consistent with Integration-First Mandate

**Cons**:
- Not optimized for time-series queries
- Limited time-series analytics capabilities
- Poor performance for Grafana dashboard queries

#### Option 2: Redis Time-Series Module
**Pros**:
- Builds on existing Redis infrastructure
- Good performance for time-series data

**Cons**:
- Limited analytics capabilities
- Memory-intensive for long-term storage
- Grafana integration challenges

#### Option 3: TimescaleDB Integration (CHOSEN)
**Pros**:
- Optimized for time-series neural engine data
- Excellent Grafana integration
- Efficient storage and query performance
- Advanced time-series analytics capabilities
- PostgreSQL compatibility ensures easy integration

**Cons**:
- Additional infrastructure component
- Learning curve for time-series database patterns

### Rationale

**WHY TIMESCALEDB**:
1. **Time-Series Optimization**: Neural engine metrics (learning curves, parameter updates, correlation tracking) are inherently time-series data
2. **Grafana Integration**: Native TimescaleDB support in Grafana for optimal dashboard performance
3. **Advanced Analytics**: Time-series aggregation functions ideal for model performance analysis
4. **Retention Management**: Automatic data retention policies for managing monitoring data volume
5. **Query Performance**: Optimized for the types of queries Grafana dashboards require

**Integration Strategy**:
- **Real-Time Operations**: Continue using Redis for immediate operational data
- **Existing Persistence**: Preserve current database for business logic
- **Monitoring Storage**: Use TimescaleDB specifically for neural engine observability data
- **Data Flow**: Metrics flow from neural engine → Redis (real-time) → TimescaleDB (persistence) → Grafana (visualization)

### Implementation Plan

1. **Deploy TimescaleDB**: Set up TimescaleDB instance for monitoring data
2. **Preserve Existing Storage**: All current Redis and database operations unchanged
3. **Create Metrics Pipeline**: New service to write monitoring data to TimescaleDB
4. **Configure Grafana**: Connect Grafana to TimescaleDB for dashboard data
5. **Implement Retention Policies**: Manage monitoring data lifecycle

---

## ADR-P4-004: Real-Time vs Batch Monitoring Strategy

**Status**: Accepted  
**Date**: 2025-08-02  
**Context**: Phase 4 requires monitoring both real-time neural engine adaptation and batch training cycles from Phase 3

### Decision

**IMPLEMENT hybrid monitoring approach** combining real-time dashboard updates with batch analytics processing.

We will create a unified monitoring system that handles both Phase 3's <50ms parameter updates and traditional batch training analysis:

```rust
// Hybrid monitoring architecture
pub struct HybridMonitoringSystem {
    // Real-time monitoring (for <50ms parameter updates)
    real_time_monitor: RealTimeMonitor,
    streaming_metrics: StreamingMetricsCollector,
    
    // Batch analytics (for comprehensive analysis)
    batch_analyzer: BatchAnalyticsEngine,
    historical_aggregator: HistoricalDataAggregator,
    
    // Unified dashboard interface
    dashboard_coordinator: DashboardCoordinator,
    alert_manager: UnifiedAlertManager,
}

#[derive(Debug, Clone)]
pub struct RealTimeMonitor {
    pub update_frequency: Duration,           // 100ms for real-time updates
    pub streaming_window_size: Duration,     // 5-minute sliding window
    pub real_time_alerts: Vec<AlertRule>,
    pub dashboard_refresh_rate: Duration,    // 1-second Grafana refresh
}

#[derive(Debug, Clone)]
pub struct BatchAnalyticsEngine {
    pub analysis_frequency: Duration,        // 15-minute comprehensive analysis
    pub historical_window: Duration,         // 24-hour analysis window
    pub correlation_analysis: CorrelationConfig,
    pub trend_analysis: TrendAnalysisConfig,
}
```

### Options Considered

#### Option 1: Real-Time Only Monitoring
**Pros**:
- Immediate feedback on parameter updates
- Low latency alert system

**Cons**:
- High computational overhead
- Limited historical analysis
- Difficult to identify long-term trends

#### Option 2: Batch Only Monitoring
**Pros**:
- Comprehensive historical analysis
- Lower computational overhead
- Better trend identification

**Cons**:
- Delayed feedback on issues
- Cannot monitor real-time adaptation effectiveness
- Miss rapid performance degradation

#### Option 3: Hybrid Approach (CHOSEN)
**Pros**:
- Best of both approaches
- Real-time feedback with comprehensive analysis
- Optimal resource utilization
- Complete monitoring coverage

**Cons**:
- More complex architecture
- Requires coordination between systems

### Rationale

**WHY HYBRID MONITORING**:
1. **Real-Time Adaptation Monitoring**: Phase 3's <50ms parameter updates require immediate visibility
2. **Comprehensive Analysis**: Long-term trends and correlations need batch processing
3. **Resource Optimization**: Hybrid approach balances monitoring completeness with computational efficiency
4. **Alert Coordination**: Real-time alerts for immediate issues, batch analysis for trend-based alerts
5. **Dashboard Efficiency**: Real-time metrics for immediate visibility, batch analytics for deep insights

**Real-Time Monitoring Focus**:
- Parameter update success/failure rates
- Model adaptation effectiveness
- Immediate performance degradation detection
- Resource usage spikes
- Data type discovery events

**Batch Analytics Focus**:
- Long-term model performance trends
- Data type impact analysis
- Cross-model correlation patterns
- Trading performance correlation
- Resource optimization recommendations

### Implementation Plan

1. **Real-Time Stream**: Implement streaming metrics collection for immediate feedback
2. **Batch Processing**: Create comprehensive analysis pipelines for historical data
3. **Dashboard Integration**: Unified Grafana dashboards showing both real-time and batch data
4. **Alert Coordination**: Unified alerting system handling both immediate and trend-based alerts
5. **Performance Optimization**: Balance real-time monitoring overhead with analysis depth

---

## ADR-P4-005: Integration vs Replacement Strategy

**Status**: Accepted  
**Date**: 2025-08-02  
**Context**: Phase 4 must maintain Integration-First Mandate compliance while adding comprehensive neural engine monitoring

### Decision

**EXTEND existing monitoring and health systems** rather than replace them with new observability infrastructure.

We will build upon the existing health monitoring foundation while adding specialized neural engine observability:

```rust
// EXTENSION to existing health monitoring
pub struct IntegratedMonitoringSystem {
    // EXISTING health monitoring preserved
    existing_health_service: Arc<HealthMonitorService>,
    existing_performance_tracker: Arc<PerformanceTracker>,
    existing_alert_system: Arc<AlertManager>,
    
    // NEW: Neural engine specific monitoring (EXTENSION)
    neural_engine_monitor: NeuralEngineMonitor,
    model_performance_analyzer: ModelPerformanceAnalyzer,
    data_type_utilization_tracker: DataTypeUtilizationTracker,
    
    // Integration layer
    monitoring_coordinator: MonitoringCoordinator,
    unified_dashboard: UnifiedDashboardManager,
}

// Integration with existing health check framework
impl HealthCheckProvider for NeuralEngineMonitor {
    fn health_check(&self) -> HealthStatus {
        // Integrate neural engine health with existing health check system
        let model_health = self.check_model_health();
        let training_health = self.check_training_health();
        let data_health = self.check_data_pipeline_health();
        
        // Return status compatible with existing health framework
        HealthStatus::from_components(vec![model_health, training_health, data_health])
    }
}
```

### Options Considered

#### Option 1: Replace Existing Monitoring
**Pros**:
- Clean slate design
- Unified architecture

**Cons**:
- VIOLATES Integration-First Mandate
- Risk of losing existing monitoring capabilities
- Disruption to operational procedures
- Loss of historical monitoring data

#### Option 2: Parallel Monitoring Systems
**Pros**:
- No disruption to existing systems
- Specialized neural engine monitoring

**Cons**:
- Violates Integration-First Mandate
- Operational complexity with multiple systems
- Alert fatigue from duplicate notifications
- Resource overhead from parallel systems

#### Option 3: Integrated Extension (CHOSEN)
**Pros**:
- Complies with Integration-First Mandate
- Preserves existing operational procedures
- Unified monitoring experience
- Leverages existing infrastructure

**Cons**:
- More complex integration requirements
- Need to maintain compatibility with existing systems

### Rationale

**WHY INTEGRATED EXTENSION**:
1. **Integration-First Mandate Compliance**: Extends existing systems rather than replacing them
2. **Operational Continuity**: Preserves existing monitoring procedures and alert systems
3. **Resource Efficiency**: Leverages existing infrastructure and avoids duplication
4. **Historical Continuity**: Maintains existing monitoring data and trends
5. **Unified Experience**: Single monitoring interface combining traditional and neural engine metrics

**Specific Integration Points**:

```rust
// Health check integration
impl From<NeuralEngineStatus> for HealthStatus {
    fn from(neural_status: NeuralEngineStatus) -> Self {
        // Convert neural engine status to existing health check format
        match neural_status.overall_health {
            NeuralHealth::Optimal => HealthStatus::Healthy,
            NeuralHealth::Degraded => HealthStatus::Warning,
            NeuralHealth::Critical => HealthStatus::Critical,
        }
    }
}

// Performance tracking integration
impl PerformanceMetric for NeuralEngineMetrics {
    fn as_performance_metric(&self) -> StandardPerformanceMetric {
        // Convert neural engine metrics to existing performance tracking format
        StandardPerformanceMetric {
            timestamp: self.timestamp,
            component: "neural_engine".to_string(),
            metric_type: MetricType::Performance,
            value: self.overall_performance_score,
            metadata: self.to_metadata(),
        }
    }
}
```

### Implementation Plan

1. **Preserve Existing Systems**: All current health monitoring and alerting remains functional
2. **Add Neural Engine Extensions**: New monitoring capabilities integrated with existing framework
3. **Unified Dashboard**: Single interface combining traditional and neural engine monitoring
4. **Integrated Alerting**: Neural engine alerts use existing notification systems
5. **Gradual Enhancement**: Phased rollout to validate integration without disruption

---

## Cross-ADR Integration Summary

### Integration-First Mandate Compliance

All Phase 4 architectural decisions follow the Integration-First Mandate by:

1. **EXTENDING** existing health monitoring systems with neural engine observability
2. **PRESERVING** all existing Redis channels and data storage patterns
3. **BUILDING UPON** Phase 3's proven characteristic-based data type discovery
4. **INTEGRATING** with existing performance tracking and alert systems
5. **MAINTAINING** all DAA autonomous trading capabilities while adding monitoring

### Phase 3 Foundation Utilization

**Dynamic Data Type Discovery** (BUILDING UPON):
- Phase 3's characteristic-based system proven effective
- Phase 4 extends with new modality characteristics (sentiment, economic, alternative)
- Zero code changes required for new data types validated in Phase 3
- Multi-scope routing (symbol/market/sector/geographic) foundation established

**Real-Time Adaptive Training** (MONITORING AND VALIDATING):
- Phase 3's <50ms parameter updates require real-time monitoring
- Checkpoint and rollback system needs comprehensive tracking
- Live feedback loops require performance correlation analysis
- Model adaptation effectiveness needs continuous validation

**Enhanced Performance Tracking** (EXTENDING):
- Phase 3's enhanced performance snapshot provides monitoring foundation
- Existing DAA voting weights (60/40 neural/strategy) preserved and monitored
- Byzantine consensus (70% threshold) effectiveness tracked
- Trading performance correlation analysis builds on Phase 3 framework

### Monitoring Architecture Benefits

1. **Complete Visibility**: Real-time and batch monitoring provides comprehensive neural engine observability
2. **Proven Integration**: Building on existing systems ensures operational continuity
3. **Scalable Storage**: TimescaleDB optimized for time-series neural engine data
4. **Industry Standard**: Grafana provides professional-grade visualization capabilities
5. **Future Proof**: Monitoring architecture scales with additional data types and models

### Risk Mitigation

- **Operational Continuity**: All existing monitoring systems remain functional during Phase 4 deployment
- **Performance Impact**: Monitoring overhead minimized through hybrid real-time/batch approach
- **Data Quality**: TimescaleDB ensures reliable storage and query performance for monitoring data
- **Alert Management**: Integrated alerting prevents notification fatigue while ensuring comprehensive coverage
- **Rollback Capability**: Monitoring extensions can be disabled without affecting existing systems

---

## Implementation Sequence

### Phase 4A: Monitoring Foundation (Weeks 12-13)
1. Deploy TimescaleDB for time-series monitoring data storage
2. Implement Grafana dashboards integrated with existing health monitoring
3. Create neural engine metrics collection and storage pipeline
4. Add real-time monitoring for Phase 3's parameter update system
5. Validate monitoring integration with existing alert systems

### Phase 4B: Data Type Scaling (Weeks 14-15)
1. Extend Phase 3's characteristic-based registry with new data modalities
2. Add sentiment data integration (news sentiment, social media signals, analyst ratings)
3. Implement economic indicators integration (GDP, inflation, interest rates, unemployment)
4. Deploy alternative data capabilities (satellite imagery, web traffic, transaction data, weather)
5. Validate automatic model activation with new data types
6. Measure and analyze performance impact of additional data modalities

### Success Criteria
- [ ] Comprehensive Grafana dashboards provide actionable neural engine insights
- [ ] 3+ new data types (sentiment, economic, alternative) integrated with zero neural engine code changes
- [ ] Dynamic data type discovery automatically recognizes and uses new data modalities
- [ ] Real-time training monitoring shows live parameter updates and adaptation effectiveness
- [ ] Performance correlation tracking provides clear visibility into prediction → trading result pipeline
- [ ] Data type impact quantification delivers measurable contribution scores for each modality
- [ ] Resource utilization scaling efficient and predictable for additional data types
- [ ] Phase 3 adaptive training capabilities confirmed working through comprehensive monitoring
- [ ] All existing DAA autonomous trading functionality preserved and enhanced
- [ ] Integration-First Mandate compliance verified across all monitoring components

---

**Decision Authority**: Phase 4 Planning Swarm  
**Review Date**: 2025-08-02  
**Next Review**: Upon Phase 4B implementation completion  
**Integration Validation**: All decisions verified against Integration-First Mandate requirements