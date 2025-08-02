# Phase 2: Integration Specifications

## 🔗 Comprehensive Integration Architecture

### 1. DAA Integration Specification

#### 1.1 Enhanced DAA Coordinator Bridge

```rust
// src/integration/enhanced_daa_bridge.rs

/// Bridge preserving all DAA capabilities while adding sector intelligence
pub struct EnhancedDAABridge {
    /// Original DAA coordinator (preserved)
    legacy_coordinator: Arc<DaaCoordinator>,
    /// New hierarchical system
    master_coordinator: Arc<MasterDAACoordinator>,
    /// Performance integration
    performance_bridge: Arc<DAAPerformanceBridge>,
    /// Decision fusion engine
    decision_fusion: Arc<DecisionFusionEngine>,
    /// Configuration
    integration_config: DAAIntegrationConfig,
}

#[derive(Debug, Clone)]
pub struct DAAIntegrationConfig {
    /// Weighting between legacy and hierarchical decisions
    pub decision_weights: DecisionWeights,
    /// Consensus requirements
    pub consensus_threshold: f64,
    /// Performance tracking configuration
    pub performance_config: PerformanceTrackingConfig,
    /// Autonomous training triggers
    pub training_triggers: TrainingTriggerConfig,
}

#[derive(Debug, Clone)]
pub struct DecisionWeights {
    /// Weight for legacy DAA decision
    pub legacy_weight: f64,        // e.g., 0.3
    /// Weight for hierarchical DAA decision  
    pub hierarchical_weight: f64,  // e.g., 0.7
    /// Dynamic weighting based on confidence
    pub dynamic_weighting: bool,
}

impl EnhancedDAABridge {
    /// Make enhanced trading decision combining both systems
    pub async fn make_enhanced_decision(
        &self,
        market_context: &MarketContext,
        portfolio_state: &PortfolioState,
        historical_data: &[TimeSeriesData],
    ) -> Result<EnhancedTradingDecision> {
        
        // 1. Get legacy DAA decision (preserved functionality)
        let legacy_decision = self.legacy_coordinator
            .make_decision(market_context, None, historical_data)
            .await?;
        
        // 2. Get hierarchical DAA decision (sector-enhanced)
        let hierarchical_decision = self.master_coordinator
            .make_portfolio_decision(market_context, portfolio_state)
            .await?;
        
        // 3. Fuse decisions using configured weights and confidence
        let fused_decision = self.decision_fusion
            .fuse_decisions(legacy_decision, hierarchical_decision, market_context)
            .await?;
        
        // 4. Record for performance tracking and autonomous training
        self.performance_bridge
            .record_enhanced_decision(&fused_decision, market_context)
            .await?;
        
        // 5. Check if autonomous training should be triggered
        self.check_and_trigger_training(&fused_decision).await?;
        
        Ok(fused_decision)
    }
    
    /// Preserve 60% neural, 40% strategy voting at sector level
    async fn ensure_voting_preservation(
        &self,
        sector_decision: &SectorDecision,
    ) -> Result<()> {
        // Validate that each sector maintains the 60/40 split
        for (sector_id, decision) in &sector_decision.sector_votes {
            let neural_weight = decision.neural_contribution;
            let strategy_weight = decision.strategy_contribution;
            
            // Ensure 60/40 split is maintained (±5% tolerance)
            if (neural_weight - 0.6).abs() > 0.05 || 
               (strategy_weight - 0.4).abs() > 0.05 {
                warn!("Voting ratio deviation in sector {:?}: neural={:.2}, strategy={:.2}", 
                      sector_id, neural_weight, strategy_weight);
            }
        }
        Ok(())
    }
}
```

#### 1.2 Performance Integration Bridge

```rust
// src/integration/daa_performance_bridge.rs

/// Bridge feeding real-time performance data to DAA autonomous training
pub struct DAAPerformanceBridge {
    /// Performance tracking
    performance_tracker: Arc<ModelPerformanceTracker>,
    /// Autonomous training engine
    training_engine: Arc<AutonomousTrainingEngine>,
    /// Training scheduler
    training_scheduler: Arc<DAATrainingScheduler>,
    /// Performance analytics
    analytics_engine: Arc<PerformanceAnalyticsEngine>,
}

impl DAAPerformanceBridge {
    /// Record decision performance for autonomous training
    pub async fn record_enhanced_decision(
        &self,
        decision: &EnhancedTradingDecision,
        market_context: &MarketContext,
    ) -> Result<()> {
        // 1. Track model performance
        let performance_metrics = self.performance_tracker
            .track_decision_performance(decision, market_context)
            .await?;
        
        // 2. Check if performance triggers training
        if self.should_trigger_training(&performance_metrics).await? {
            let training_decision = self.create_training_decision(
                &performance_metrics,
                decision,
                market_context
            ).await?;
            
            // 3. Submit to autonomous training
            self.training_engine
                .evaluate_training_need(training_decision.performance_snapshot)
                .await?;
        }
        
        // 4. Update analytics for continuous improvement
        self.analytics_engine
            .update_performance_analytics(&performance_metrics)
            .await?;
        
        Ok(())
    }
    
    /// Enhanced performance snapshot including sector context
    async fn create_enhanced_performance_snapshot(
        &self,
        decision: &EnhancedTradingDecision,
        outcome: &TradingOutcome,
    ) -> Result<EnhancedPerformanceSnapshot> {
        let base_snapshot = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: outcome.prediction_accuracy,
            latency_ms: outcome.decision_latency_ms,
            error_rate: outcome.error_rate,
            recent_predictions: 50,
            confidence: decision.confidence,
            price_error: outcome.price_error,
            sharpe_ratio: outcome.sharpe_ratio,
            max_drawdown: outcome.max_drawdown,
            volatility: outcome.volatility,
            model_agreement: decision.model_agreement,
            consecutive_failures: outcome.consecutive_failures,
            trading_volume: outcome.trading_volume,
            profit_loss: outcome.profit_loss,
        };
        
        let enhanced_snapshot = EnhancedPerformanceSnapshot {
            base: base_snapshot,
            sector_performance: outcome.sector_performance_breakdown,
            hierarchical_performance: outcome.hierarchical_decision_accuracy,
            legacy_performance: outcome.legacy_decision_accuracy,
            fusion_effectiveness: outcome.decision_fusion_improvement,
            model_contributions: decision.model_contributions.clone(),
        };
        
        Ok(enhanced_snapshot)
    }
}
```

### 2. Redis Integration Specification

#### 2.1 Channel Preservation and Enhancement

```rust
// src/integration/redis_integration.rs

/// Redis integration preserving all existing channels while adding sector capabilities
pub struct RedisIntegrationManager {
    /// Existing symbol channels (preserved)
    symbol_channels: Arc<DashMap<String, SymbolRedisChannel>>,
    /// New sector channels
    sector_channels: Arc<DashMap<SectorId, SectorRedisChannel>>,
    /// Portfolio-level channel
    portfolio_channel: Arc<PortfolioRedisChannel>,
    /// Channel router
    channel_router: Arc<RedisChannelRouter>,
    /// Redis client pool
    redis_pool: Arc<RedisPool>,
}

/// Preserved symbol channel implementation
pub struct SymbolRedisChannel {
    pub symbol: String,
    pub channel_name: String,          // e.g., "symbol:AAPL:updates"
    pub publisher: Arc<RedisPublisher>,
    pub subscriber: Arc<RedisSubscriber>,
    /// Preserved message formats
    pub message_format: SymbolMessageFormat,
}

/// New sector channel implementation
pub struct SectorRedisChannel {
    pub sector_id: SectorId,
    pub channel_name: String,          // e.g., "sector:technology:metrics"
    pub publisher: Arc<RedisPublisher>,
    pub subscriber: Arc<RedisSubscriber>,
    /// Sector-specific message formats
    pub message_format: SectorMessageFormat,
}

impl RedisIntegrationManager {
    /// Process symbol update preserving existing behavior while adding sector aggregation
    pub async fn process_symbol_update(
        &self,
        symbol: &str,
        market_data: &MarketData,
    ) -> Result<()> {
        // 1. Preserve existing symbol channel behavior
        if let Some(channel) = self.symbol_channels.get(symbol) {
            let symbol_message = SymbolUpdateMessage {
                symbol: symbol.to_string(),
                timestamp: Utc::now(),
                price: market_data.price,
                volume: market_data.volume,
                bid: market_data.bid,
                ask: market_data.ask,
                metadata: market_data.metadata.clone(),
            };
            
            channel.publisher
                .publish(&channel.channel_name, &symbol_message)
                .await?;
        }
        
        // 2. Aggregate to sector and publish to sector channels
        let sector_id = self.get_symbol_sector(symbol)?;
        let sector_metrics = self.aggregate_to_sector_metrics(
            &sector_id, 
            symbol, 
            market_data
        ).await?;
        
        if let Some(sector_channel) = self.sector_channels.get(&sector_id) {
            let sector_message = SectorUpdateMessage {
                sector_id: sector_id.clone(),
                timestamp: Utc::now(),
                metrics: sector_metrics,
                contributing_symbols: vec![symbol.to_string()],
                etf_data: self.get_sector_etf_data(&sector_id).await?,
            };
            
            sector_channel.publisher
                .publish(&sector_channel.channel_name, &sector_message)
                .await?;
        }
        
        // 3. Update portfolio-level metrics if needed
        if self.should_update_portfolio_metrics(&sector_id, &sector_metrics) {
            self.update_portfolio_channel(&sector_id, &sector_metrics).await?;
        }
        
        Ok(())
    }
    
    /// Ensure backward compatibility with existing Redis consumers
    pub async fn validate_channel_compatibility(&self) -> Result<CompatibilityReport> {
        let mut report = CompatibilityReport::new();
        
        // Check existing symbol channels
        for symbol_channel in self.symbol_channels.iter() {
            let compatibility = self.check_symbol_channel_compatibility(
                &symbol_channel.channel_name,
                &symbol_channel.message_format
            ).await?;
            report.add_symbol_compatibility(symbol_channel.symbol.clone(), compatibility);
        }
        
        // Validate new sector channels don't conflict
        for sector_channel in self.sector_channels.iter() {
            let conflict_check = self.check_channel_name_conflicts(
                &sector_channel.channel_name
            ).await?;
            report.add_sector_validation(sector_channel.sector_id.clone(), conflict_check);
        }
        
        Ok(report)
    }
}
```

#### 2.2 Message Format Preservation

```rust
// src/integration/redis_messages.rs

/// Preserved symbol message format (no changes to existing structure)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SymbolUpdateMessage {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub price: f64,
    pub volume: f64,
    pub bid: f64,
    pub ask: f64,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    // NO CHANGES to existing fields - complete backward compatibility
}

/// New sector message format (additive only)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SectorUpdateMessage {
    pub sector_id: SectorId,
    pub timestamp: DateTime<Utc>,
    pub metrics: SectorMetrics,
    pub contributing_symbols: Vec<String>,
    pub etf_data: Option<ETFData>,
    pub aggregation_metadata: AggregationMetadata,
}

/// Portfolio-level message format
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PortfolioUpdateMessage {
    pub timestamp: DateTime<Utc>,
    pub portfolio_metrics: PortfolioMetrics,
    pub sector_allocations: HashMap<SectorId, f64>,
    pub rebalancing_signals: Vec<RebalancingSignal>,
    pub risk_metrics: PortfolioRiskMetrics,
}
```

### 3. Health Monitoring Integration

#### 3.1 Enhanced Health System

```rust
// src/integration/health_integration.rs

/// Health monitoring integration with sector-level capabilities
pub struct EnhancedHealthMonitor {
    /// Existing health monitor (preserved)
    base_health_monitor: Arc<HealthMonitor>,
    /// Sector-level health monitoring
    sector_health_monitors: Arc<DashMap<SectorId, Arc<SectorHealthMonitor>>>,
    /// Master health coordinator
    master_health_coordinator: Arc<MasterHealthCoordinator>,
    /// Health metrics aggregator
    health_metrics_aggregator: Arc<HealthMetricsAggregator>,
}

impl EnhancedHealthMonitor {
    /// Comprehensive health check including sector systems
    pub async fn perform_comprehensive_health_check(&self) -> Result<ComprehensiveHealthStatus> {
        // 1. Preserve existing health checks
        let base_health = self.base_health_monitor
            .perform_health_check()
            .await?;
        
        // 2. Check sector-level health
        let mut sector_health = HashMap::new();
        for (sector_id, monitor) in self.sector_health_monitors.iter() {
            let sector_status = monitor.check_sector_health().await?;
            sector_health.insert(sector_id.clone(), sector_status);
        }
        
        // 3. Check master coordination health
        let master_health = self.master_health_coordinator
            .check_master_systems_health()
            .await?;
        
        // 4. Aggregate overall system health
        let comprehensive_status = ComprehensiveHealthStatus {
            base_health,
            sector_health,
            master_health,
            overall_status: self.calculate_overall_health_status(&base_health, &sector_health, &master_health),
            timestamp: Utc::now(),
        };
        
        // 5. Update health metrics for monitoring
        self.health_metrics_aggregator
            .update_health_metrics(&comprehensive_status)
            .await?;
        
        Ok(comprehensive_status)
    }
}

/// Sector-specific health monitoring
pub struct SectorHealthMonitor {
    pub sector_id: SectorId,
    /// Model pool health
    model_pool_monitor: Arc<ModelPoolHealthMonitor>,
    /// Shared feature extractor health
    feature_extractor_monitor: Arc<FeatureExtractorHealthMonitor>,
    /// DAA coordinator health
    daa_coordinator_monitor: Arc<DAACoordinatorHealthMonitor>,
    /// Data aggregation health
    aggregation_monitor: Arc<AggregationHealthMonitor>,
}

impl SectorHealthMonitor {
    /// Check all sector subsystems
    pub async fn check_sector_health(&self) -> Result<SectorHealthStatus> {
        let mut health_checks = Vec::new();
        
        // Check model pool
        let model_pool_health = self.model_pool_monitor
            .check_model_pool_health()
            .await?;
        health_checks.push(("model_pool".to_string(), model_pool_health));
        
        // Check shared feature extractor
        let feature_health = self.feature_extractor_monitor
            .check_feature_extractor_health()
            .await?;
        health_checks.push(("feature_extractor".to_string(), feature_health));
        
        // Check DAA coordinator
        let daa_health = self.daa_coordinator_monitor
            .check_daa_health()
            .await?;
        health_checks.push(("daa_coordinator".to_string(), daa_health));
        
        // Check data aggregation
        let aggregation_health = self.aggregation_monitor
            .check_aggregation_health()
            .await?;
        health_checks.push(("aggregation".to_string(), aggregation_health));
        
        Ok(SectorHealthStatus {
            sector_id: self.sector_id.clone(),
            subsystem_health: health_checks,
            overall_sector_health: self.calculate_sector_health_status(&health_checks),
            timestamp: Utc::now(),
        })
    }
}
```

### 4. Performance Tracking Integration

#### 4.1 Enhanced Performance System

```rust
// src/integration/performance_integration.rs

/// Performance tracking integration with autonomous training
pub struct EnhancedPerformanceTracker {
    /// Base performance tracker (preserved)
    base_tracker: Arc<PerformanceTracker>,
    /// Sector-level performance tracking
    sector_trackers: Arc<DashMap<SectorId, Arc<SectorPerformanceTracker>>>,
    /// Model performance tracker
    model_tracker: Arc<ModelPerformanceTracker>,
    /// Trading performance tracker
    trading_tracker: Arc<TradingPerformanceTracker>,
    /// Analytics engine
    analytics_engine: Arc<PerformanceAnalyticsEngine>,
}

impl EnhancedPerformanceTracker {
    /// Track comprehensive performance across all systems
    pub async fn track_comprehensive_performance(
        &self,
        decision: &EnhancedTradingDecision,
        outcome: &TradingOutcome,
    ) -> Result<ComprehensivePerformanceMetrics> {
        
        // 1. Track base performance (preserved)
        let base_metrics = self.base_tracker
            .track_performance(&decision.base_decision, outcome)
            .await?;
        
        // 2. Track sector-level performance
        let mut sector_metrics = HashMap::new();
        for (sector_id, sector_decision) in &decision.sector_decisions {
            if let Some(tracker) = self.sector_trackers.get(sector_id) {
                let sector_outcome = outcome.sector_outcomes.get(sector_id).unwrap();
                let metrics = tracker
                    .track_sector_performance(sector_decision, sector_outcome)
                    .await?;
                sector_metrics.insert(sector_id.clone(), metrics);
            }
        }
        
        // 3. Track model performance
        let model_metrics = self.model_tracker
            .track_model_performance(&decision.model_predictions, outcome)
            .await?;
        
        // 4. Track trading performance
        let trading_metrics = self.trading_tracker
            .track_trading_performance(&decision.trading_actions, outcome)
            .await?;
        
        // 5. Generate comprehensive metrics
        let comprehensive_metrics = ComprehensivePerformanceMetrics {
            base_metrics,
            sector_metrics,
            model_metrics,
            trading_metrics,
            fusion_effectiveness: self.calculate_fusion_effectiveness(decision, outcome),
            timestamp: Utc::now(),
        };
        
        // 6. Update analytics for continuous improvement
        self.analytics_engine
            .analyze_comprehensive_performance(&comprehensive_metrics)
            .await?;
        
        Ok(comprehensive_metrics)
    }
    
    /// Feed performance data to autonomous training
    pub async fn feed_autonomous_training(
        &self,
        metrics: &ComprehensivePerformanceMetrics,
    ) -> Result<()> {
        // Check each model's performance and trigger training if needed
        for (model_id, model_metrics) in &metrics.model_metrics.individual_model_performance {
            if self.should_trigger_training_for_model(model_id, model_metrics).await? {
                let training_snapshot = self.create_training_snapshot(
                    model_id,
                    model_metrics,
                    &metrics
                ).await?;
                
                // Send to autonomous training engine
                // Integration with existing DAA training system
                self.send_to_autonomous_training(training_snapshot).await?;
            }
        }
        
        Ok(())
    }
}
```

### 5. Configuration Integration

#### 5.1 TOML Configuration System

```toml
# config/sector_integration.toml

[integration.daa]
# Preserve all existing DAA settings
preserve_legacy_daa = true
legacy_weight = 0.3
hierarchical_weight = 0.7
consensus_threshold = 0.7
voting_split_neural = 0.6
voting_split_strategy = 0.4

[integration.redis]
# Preserve all existing Redis channels
preserve_existing_channels = true
add_sector_channels = true
channel_prefix = "neural_trader"

[integration.redis.channels.symbols]
# Existing symbol channels (preserved)
AAPL = "symbol:AAPL:updates"
MSFT = "symbol:MSFT:updates"
GOOGL = "symbol:GOOGL:updates"

[integration.redis.channels.sectors]
# New sector channels
technology = "sector:technology:metrics"
financial = "sector:financial:metrics"
healthcare = "sector:healthcare:metrics"

[integration.health]
# Health monitoring configuration
enable_sector_health = true
health_check_interval_seconds = 30
comprehensive_health_enabled = true

[integration.performance]
# Performance tracking configuration
enable_enhanced_tracking = true
track_sector_performance = true
feed_autonomous_training = true
performance_window_minutes = 60
```

### 6. Migration Strategy

#### 6.1 Zero-Downtime Migration Plan

```rust
// src/integration/migration_manager.rs

/// Manages zero-downtime migration to sector-based architecture
pub struct MigrationManager {
    /// Current system state
    current_state: Arc<RwLock<SystemState>>,
    /// Migration configuration
    migration_config: MigrationConfig,
    /// Health monitor during migration
    migration_health_monitor: Arc<MigrationHealthMonitor>,
    /// Rollback manager
    rollback_manager: Arc<RollbackManager>,
}

#[derive(Debug, Clone)]
pub enum SystemState {
    LegacyOnly,                    // Pre-migration state
    DualMode {                     // During migration
        legacy_weight: f64,
        sector_weight: f64,
    },
    SectorPrimary {               // Post-migration state
        legacy_fallback: bool,
    },
    SectorOnly,                   // Final state
}

impl MigrationManager {
    /// Execute phased migration with rollback capability
    pub async fn execute_migration(&self) -> Result<MigrationResult> {
        // Phase 1: Initialize sector systems alongside legacy
        self.initialize_sector_systems().await?;
        self.validate_sector_initialization().await?;
        
        // Phase 2: Start dual-mode operation
        self.enable_dual_mode(0.1, 0.9).await?; // 10% sector, 90% legacy
        self.monitor_dual_mode_performance().await?;
        
        // Phase 3: Gradually increase sector weight
        for weight in [0.2, 0.4, 0.6, 0.8] {
            self.adjust_dual_mode_weights(weight, 1.0 - weight).await?;
            self.validate_performance_at_weight(weight).await?;
            
            if !self.performance_meets_requirements().await? {
                return self.rollback_to_previous_state().await;
            }
        }
        
        // Phase 4: Full sector mode with legacy fallback
        self.enable_sector_primary_mode().await?;
        self.validate_sector_primary_performance().await?;
        
        // Phase 5: Sector-only mode (legacy disabled)
        if self.sector_performance_stable().await? {
            self.enable_sector_only_mode().await?;
        }
        
        Ok(MigrationResult::Success)
    }
    
    /// Rollback to previous stable state
    pub async fn rollback_to_previous_state(&self) -> Result<MigrationResult> {
        warn!("Performance degradation detected, initiating rollback");
        
        let current_state = self.current_state.read().await.clone();
        match current_state {
            SystemState::DualMode { .. } => {
                self.rollback_to_legacy_only().await?;
            }
            SystemState::SectorPrimary { .. } => {
                self.rollback_to_dual_mode().await?;
            }
            SystemState::SectorOnly => {
                self.rollback_to_sector_primary().await?;
            }
            _ => {}
        }
        
        Ok(MigrationResult::RolledBack)
    }
}
```

## 🎯 Integration Success Criteria

### DAA Integration
- **✅ All autonomous trading capabilities preserved**
- **✅ 60% neural, 40% strategy voting maintained**
- **✅ 70% consensus threshold preserved**
- **✅ Performance data feeding autonomous training**
- **✅ Byzantine fault tolerance enhanced**

### Redis Integration
- **✅ All existing channels preserved**
- **✅ Backward compatibility maintained**
- **✅ New sector channels operational**
- **✅ Portfolio-level aggregation working**
- **✅ No message format changes to existing channels**

### Health Monitoring Integration
- **✅ Existing health checks preserved**
- **✅ Sector-level health monitoring added**
- **✅ Master health coordination operational**
- **✅ Comprehensive health status available**
- **✅ Health-based autonomous training triggers**

### Performance Integration
- **✅ Enhanced performance tracking operational**
- **✅ Sector-level performance metrics available**
- **✅ Model performance feeding DAA training**
- **✅ Trading performance correlation analysis**
- **✅ Autonomous training triggers based on performance**

This comprehensive integration specification ensures that the sector-based architecture integrates seamlessly with all existing systems while providing enhanced capabilities for 100+ symbol support.