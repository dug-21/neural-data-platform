# V2 Component Design - Neural Trader Platform (CORRECTED)

## Component Architecture Overview

This document details the component-level design for the Neural Trader V2 platform with THREE RUST BINARIES: neural-ml-ops for training, neural-trading for execution, and neural-core as shared library. NO Python, NO microservices - embedded ruv-FANN with DAA Coordinators.

## Core Components

### 1. EventBus Abstraction Layer

```yaml
component:
  name: "EventBus"
  type: "Infrastructure Service"
  technology: "Redis Streams with abstraction layer"
  
  interfaces:
    publish:
      signature: "async fn publish(topic: &str, event: Event) -> Result<EventId>"
      description: "Publish event to specified topic"
      
    subscribe:
      signature: "async fn subscribe(topics: Vec<String>) -> Result<EventStream>"
      description: "Subscribe to multiple event topics"
      
    consume_group:
      signature: "async fn consume_group(group: &str, consumer: &str) -> Result<EventStream>"
      description: "Consume events as part of consumer group"
  
  implementation:
    ```rust
    pub struct EventBus {
        redis_client: Arc<RedisClient>,
        serializer: Arc<EventSerializer>,
        router: Arc<EventRouter>,
        metrics: Arc<MetricsCollector>,
    }
    
    impl EventBus {
        pub async fn publish(&self, topic: &str, event: Event) -> Result<EventId> {
            // Serialize event
            let payload = self.serializer.serialize(&event)?;
            
            // Add to stream
            let id = self.redis_client
                .xadd(topic, &payload)
                .await?;
            
            // Update metrics
            self.metrics.increment_published(topic);
            
            Ok(EventId::from(id))
        }
        
        pub async fn subscribe(&self, topics: Vec<String>) -> Result<EventStream> {
            let mut streams = Vec::new();
            
            for topic in topics {
                let stream = self.redis_client
                    .xread_block(&topic, 0)
                    .await?;
                streams.push(stream);
            }
            
            Ok(EventStream::new(streams))
        }
    }
    ```
  
  configuration:
    connection_pool_size: 100
    max_retries: 3
    retry_delay_ms: 1000
    message_ttl_hours: 72
    max_stream_length: 1000000
```

### 2. Binary Separation: ML Ops Component (neural-ml-ops)

```rust
// neural-ml-ops binary - TRAINING ONLY
pub struct MLOpsEngine {
    // ruv-FANN training (NOT inference)
    training_pipeline: FANNTrainingPipeline,
    
    // Model registry (config-store backed)
    model_registry: ConfigStoreRegistry,
    
    // Feature computation (domain-agnostic)
    feature_engine: RustFeatureEngine,
    
    // Event publishing (to trading binary)
    publisher: RedisStreamPublisher,
    
    // NO DAA Coordinator (only in neural-trading)
    // NO inference engine (only in neural-trading)
}

impl MLOpsEngine {
    pub async fn train_model(&mut self, config: TrainingConfig) -> Result<ModelId> {
        // 1. Extract features using Rust feature engine
        let features = self.feature_engine.compute_training_features(
            config.symbol_list,
            config.time_range
        ).await?;
        
        // 2. Train ruv-FANN model
        let mut model = BaseModel::<TradingData>::new(
            config.network_topology,
            config.activation_function
        )?;
        
        let training_result = model.train(
            &features.training_data,
            &features.targets,
            config.epochs,
            config.learning_rate
        )?;
        
        // 3. Store trained model in config-store
        let model_id = self.model_registry.store_model(
            model,
            training_result.metrics,
            config.clone()
        ).await?;
        
        // 4. Publish model update event
        self.publisher.publish_model_update(ModelUpdateEvent {
            model_id: model_id.clone(),
            version: training_result.version,
            performance: training_result.metrics,
            config_store_path: format!("/models/{}", model_id),
        }).await?;
        
        Ok(model_id)
    }
    
    pub async fn compute_features(&mut self, market_data: MarketData) -> Result<()> {
        // Compute features (domain-agnostic indicators)
        let features = self.feature_engine.compute_features(&market_data)?;
        
        // Publish to Redis Streams for neural-trading to consume
        self.publisher.publish_features(FeatureUpdateEvent {
            symbol: market_data.symbol,
            timestamp: market_data.timestamp,
            features: features,
            metadata: FeatureMetadata {
                computation_time: Utc::now(),
                version: self.feature_engine.version(),
            },
        }).await?;
        
        Ok(())
    }
}
```
    
    feature_store:
      purpose: "Embedded Rust feature computation and serving"
      interfaces:
        - register_feature(definition: RustFeatureDefinition) -> FeatureId
        - compute_features(entity: TradingEntity, features: Vec<FeatureId>) -> TradingFeatures
        - get_online_features(keys: Vec<SymbolKey>) -> TradingFeatures
        - materialize_features(feature_set: FeatureSet) -> Result
      
      architecture:
        online_store: "EventBus (Redis Streams)"
        offline_store: "TimescaleDB"
        compute_engine: "Native Rust feature engine (embedded)"
    
    training_pipeline:
      purpose: "Automated model training and evaluation"
      interfaces:
        - submit_training_job(config: TrainingConfig) -> JobId
        - get_job_status(id: JobId) -> JobStatus
        - cancel_job(id: JobId) -> Result
      
      implementation:
        ```rust
        use ruv_fann::BaseModel;
        
        pub struct RustTrainingPipeline {
            feature_engine: FeatureEngine,
            model_registry: ConfigStoreModelRegistry,
            daa_coordinator: DAACoordinator,
        }
        
        impl RustTrainingPipeline {
            pub async fn train_model(&self, config: TrainingConfig) -> Result<ModelId> {
                // Load features using Rust feature engine
                let features = self.feature_engine.get_training_data(
                    &config.feature_set,
                    config.time_range
                ).await?;
                
                // Initialize ruv-FANN model
                let mut model = BaseModel::<TradingData>::new(
                    config.layers.clone(),
                    config.activation,
                    config.learning_rate
                )?;
                
                // Train model using ruv-FANN
                let history = model.fit(
                    &features.X,
                    &features.y,
                    config.epochs,
                    config.batch_size,
                    0.2  // validation_split
                )?;
                
                // Evaluate model
                let metrics = self.evaluate_model(&model, &features.X_test, &features.y_test)?;
                
                // Let DAA Coordinator assess model quality
                let assessment = self.daa_coordinator.assess_model(&model, &metrics)?;
                
                // Register model in config-store
                let model_id = self.model_registry.register(
                    model,
                    metrics,
                    config,
                    assessment
                ).await?;
                
                Ok(model_id)
            }
        }
        ```
    
    inference_engine:
      purpose: "Embedded low-latency ruv-FANN inference (no separate server)"
      interfaces:
        - predict(model: &BaseModel<TradingData>, input: TradingFeatures) -> TradingPrediction
        - batch_predict(model: &BaseModel<TradingData>, inputs: Vec<TradingFeatures>) -> Vec<TradingPrediction>
        - explain(model: &BaseModel<TradingData>, input: TradingFeatures) -> PredictionExplanation
      
      performance:
        latency_p99: "< 1ms (embedded, no network calls)"
        throughput: "> 100k req/sec (embedded)"
        availability: "Same as main trading service"
```

### 3. Domain Registry Service

```yaml
component:
  name: "DomainRegistry"
  type: "Service Discovery and Configuration"
  technology: "etcd/Consul with custom wrapper"
  
  responsibilities:
    - Service registration and discovery
    - Configuration management
    - Health checking
    - Load balancing configuration
  
  interfaces:
    service_management:
      - register_service(service: ServiceDefinition) -> ServiceId
      - deregister_service(id: ServiceId) -> Result
      - discover_service(name: String, version: Option<String>) -> Vec<ServiceInstance>
      - health_check(id: ServiceId) -> HealthStatus
    
    configuration:
      - set_config(key: String, value: Value, options: ConfigOptions) -> Result
      - get_config(key: String) -> Option<Value>
      - watch_config(key: String) -> ConfigStream
      - list_configs(prefix: String) -> Vec<ConfigEntry>
  
  implementation:
    ```rust
    pub struct DomainRegistry {
        backend: Arc<dyn RegistryBackend>,
        cache: Arc<RwLock<HashMap<String, CachedEntry>>>,
        health_checker: Arc<HealthChecker>,
        event_bus: Arc<EventBus>,
    }
    
    impl DomainRegistry {
        pub async fn register_service(&self, service: ServiceDefinition) -> Result<ServiceId> {
            // Validate service definition
            service.validate()?;
            
            // Generate unique ID
            let id = ServiceId::new();
            
            // Store in backend
            self.backend.put(
                &format!("/services/{}/{}", service.name, id),
                &service.to_json()?
            ).await?;
            
            // Start health checking
            self.health_checker.start_checking(&id, &service.health_check).await;
            
            // Publish registration event
            self.event_bus.publish(
                "service.registered",
                ServiceRegisteredEvent { id: id.clone(), service }
            ).await?;
            
            Ok(id)
        }
        
        pub async fn discover_service(
            &self,
            name: String,
            version: Option<String>
        ) -> Result<Vec<ServiceInstance>> {
            // Check cache first
            let cache_key = format!("{}:{}", name, version.as_ref().unwrap_or(&"latest".to_string()));
            
            if let Some(cached) = self.get_cached(&cache_key).await {
                return Ok(cached);
            }
            
            // Query backend
            let prefix = format!("/services/{}", name);
            let entries = self.backend.list(&prefix).await?;
            
            // Filter by version and health
            let mut instances = Vec::new();
            for entry in entries {
                let service: ServiceDefinition = serde_json::from_str(&entry.value)?;
                
                if version.as_ref().map_or(true, |v| v == &service.version) {
                    if self.health_checker.is_healthy(&entry.id).await {
                        instances.push(ServiceInstance::from(service));
                    }
                }
            }
            
            // Cache result
            self.cache_result(&cache_key, &instances).await;
            
            Ok(instances)
        }
    }
    ```
  
  high_availability:
    consensus: "Raft"
    replication_factor: 3
    leader_election_timeout: "5s"
    snapshot_interval: "10m"
```

### 4. Market Data Service Component

```yaml
component:
  name: "MarketDataService"
  type: "Domain Service"
  domain: "Trading"
  
  responsibilities:
    - Real-time market data ingestion
    - Data normalization and validation
    - Historical data management
    - Market data distribution
  
  architecture:
    ```rust
    pub struct MarketDataService {
        // Ingestion layer
        connectors: HashMap<String, Box<dyn MarketDataConnector>>,
        
        // Processing layer
        normalizer: DataNormalizer,
        validator: DataValidator,
        enricher: DataEnricher,
        
        // Storage layer
        tick_store: TickStore,
        bar_store: BarStore,
        
        // Distribution layer
        event_bus: Arc<EventBus>,
        websocket_server: WebSocketServer,
        
        // Monitoring
        metrics: MetricsCollector,
    }
    ```
  
  data_flow:
    ```mermaid
    graph LR
        subgraph "Ingestion"
            C1[Exchange Connector]
            C2[Bloomberg Connector]
            C3[Reuters Connector]
        end
        
        subgraph "Processing"
            N[Normalizer]
            V[Validator]
            E[Enricher]
        end
        
        subgraph "Storage"
            TS[Tick Store]
            BS[Bar Store]
            CS[Cache Store]
        end
        
        subgraph "Distribution"
            EB[Event Bus]
            WS[WebSocket]
            GR[gRPC]
        end
        
        C1 --> N
        C2 --> N
        C3 --> N
        N --> V
        V --> E
        E --> TS
        E --> BS
        E --> CS
        E --> EB
        E --> WS
        E --> GR
    ```
  
  interfaces:
    ingestion:
      - connect(provider: Provider, config: Config) -> ConnectionId
      - disconnect(id: ConnectionId) -> Result
      - subscribe_symbol(symbol: String, subscription_type: SubscriptionType) -> Result
    
    query:
      - get_latest_tick(symbol: String) -> Option<Tick>
      - get_historical_ticks(symbol: String, range: TimeRange) -> Vec<Tick>
      - get_bars(symbol: String, interval: Interval, range: TimeRange) -> Vec<Bar>
    
    streaming:
      - stream_ticks(symbols: Vec<String>) -> TickStream
      - stream_bars(symbols: Vec<String>, interval: Interval) -> BarStream
```

### 3. Binary Separation: Trading Component (neural-trading)

```rust
// neural-trading binary - INFERENCE AND EXECUTION
pub struct TradingEngine {
    // DAA Coordinator (CRITICAL - only in domain binaries)
    daa_coordinator: DAACoordinator,
    
    // Embedded ruv-FANN inference (NO separate service)
    model_cache: HashMap<ModelId, BaseModel<TradingData>>,
    inference_engine: EmbeddedFANNInference,
    
    // Market data processing
    market_data_processor: MarketDataProcessor,
    
    // Order execution
    order_executor: AlpacaOrderExecutor,
    
    // Event subscription from neural-ml-ops
    feature_subscriber: RedisStreamConsumer,
    model_subscriber: RedisStreamConsumer,
    
    // Event publishing
    signal_publisher: RedisStreamPublisher,
}

impl TradingEngine {
    pub async fn run(&mut self) -> Result<()> {
        // Subscribe to features from neural-ml-ops
        self.feature_subscriber.subscribe("features:computed", "trading-group").await?;
        
        // Subscribe to model updates from neural-ml-ops
        self.model_subscriber.subscribe("models:updates", "trading-group").await?;
        
        loop {
            tokio::select! {
                // Handle new features
                feature_event = self.feature_subscriber.next() => {
                    if let Some(features) = feature_event {
                        self.process_features(features).await?;
                    }
                }
                
                // Handle model updates
                model_event = self.model_subscriber.next() => {
                    if let Some(update) = model_event {
                        self.update_model(update).await?;
                    }
                }
                
                // Handle market data
                market_data = self.market_data_processor.next() => {
                    if let Some(data) = market_data {
                        self.process_market_data(data).await?;
                    }
                }
            }
        }
    }
    
    async fn process_features(&mut self, features: FeatureUpdateEvent) -> Result<()> {
        // Get relevant model from cache
        let model = self.model_cache.get(&features.model_id)
            .ok_or(TradingError::ModelNotFound)?;
        
        // Embedded ruv-FANN inference (< 1ms)
        let prediction = self.inference_engine.predict(model, &features.features)?;
        
        // DAA Coordinator makes trading decision
        let decision = self.daa_coordinator.coordinate_decision(
            prediction,
            features.symbol,
            self.get_market_context(&features.symbol)
        )?;
        
        // Execute if decision warrants action
        if let Some(action) = decision {
            self.execute_trading_action(action).await?;
        }
        
        Ok(())
    }
    
    async fn update_model(&mut self, update: ModelUpdateEvent) -> Result<()> {
        // Load new model from config-store
        let new_model = self.load_model_from_config_store(&update.config_store_path).await?;
        
        // Hot-reload model in cache
        self.model_cache.insert(update.model_id, new_model);
        
        info!("Model updated: {} -> version {}", update.model_id, update.version);
        Ok(())
    }
}
```
        
        // Execution
        executor: StrategyExecutor,
        scheduler: TaskScheduler,
        
        // State management
        position_tracker: PositionTracker,
        risk_manager: RiskManager,
        
        // Signal processing with ruv-FANN
        fann_signal_processor: FANNSignalProcessor,
        signal_aggregator: SignalAggregator,
        
        // Integration (embedded models, no separate ML platform)
        market_data: Arc<MarketDataService>,
        order_manager: Arc<OrderManagementService>,
        fann_models: HashMap<ModelId, BaseModel<TradingData>>,
    }
    
    #[async_trait]
    impl Strategy for CustomStrategy {
        async fn initialize(&mut self, context: &Context) -> Result<()> {
            // Load ruv-FANN model from embedded registry
            self.fann_model = context.fann_models
                .get(&self.config.model_id)
                .cloned()
                .ok_or(StrategyError::ModelNotFound)?;
            
            // Subscribe to market data
            self.market_stream = context.market_data
                .stream_ticks(self.config.symbols.clone())
                .await?;
            
            Ok(())
        }
        
        async fn on_tick(&mut self, tick: Tick, context: &Context) -> Result<Vec<Signal>> {
            // Update features
            self.feature_buffer.push(tick.clone());
            
            // Check if we have enough data
            if self.feature_buffer.len() < self.config.lookback {
                return Ok(Vec::new());
            }
            
            // Compute features
            let features = self.compute_features(&self.feature_buffer);
            
            // Get prediction from ruv-FANN model (embedded, no async needed)
            let prediction = self.fann_model.predict(&features)?;
            
            // Generate signals using DAA Coordinator
            let signals = context.daa_coordinator.coordinate_signals(
                prediction, 
                &tick, 
                &self.context
            )?;
            
            Ok(signals)
        }
        
        async fn on_signal(&mut self, signal: Signal, context: &Context) -> Result<Vec<Order>> {
            // Check risk limits
            if !context.risk_manager.check_limits(&signal).await? {
                return Ok(Vec::new());
            }
            
            // Calculate position size
            let size = context.risk_manager.calculate_position_size(
                &signal,
                context.portfolio
            ).await?;
            
            // Generate orders
            let orders = self.generate_orders(signal, size);
            
            Ok(orders)
        }
    }
    ```
  
  signal_processing:
    pipeline:
      - signal_generation
      - signal_validation
      - signal_filtering
      - signal_aggregation
      - signal_execution
    
    aggregation_rules:
      - majority_vote
      - weighted_average
      - confidence_threshold
      - correlation_filter
```

### 6. Order Management Component

```yaml
component:
  name: "OrderManagementService"
  type: "Domain Service"
  domain: "Trading"
  
  responsibilities:
    - Order lifecycle management
    - Smart order routing
    - Execution algorithms
    - Fill management
  
  architecture:
    ```rust
    pub struct OrderManagementService {
        // Order management
        order_book: OrderBook,
        order_cache: OrderCache,
        
        // Execution
        execution_router: SmartOrderRouter,
        algo_engine: AlgorithmicExecutionEngine,
        
        // Venue connections
        venue_connectors: HashMap<VenueId, Box<dyn VenueConnector>>,
        
        // Risk and compliance
        pre_trade_risk: PreTradeRiskChecker,
        compliance_checker: ComplianceChecker,
        
        // Integration
        event_bus: Arc<EventBus>,
        position_tracker: Arc<PositionTracker>,
    }
    
    impl OrderManagementService {
        pub async fn submit_order(&self, order: Order) -> Result<OrderId> {
            // Pre-trade checks
            self.pre_trade_risk.check(&order).await?;
            self.compliance_checker.check(&order).await?;
            
            // Assign order ID
            let order_id = OrderId::new();
            
            // Route order
            let routing_decision = self.execution_router
                .route(&order)
                .await?;
            
            match routing_decision {
                RoutingDecision::Direct(venue) => {
                    self.send_to_venue(order_id, order, venue).await?;
                }
                RoutingDecision::Algorithmic(algo) => {
                    self.algo_engine.execute(order_id, order, algo).await?;
                }
                RoutingDecision::Split(splits) => {
                    for (venue, portion) in splits {
                        let child_order = order.split(portion);
                        self.send_to_venue(OrderId::new(), child_order, venue).await?;
                    }
                }
            }
            
            // Publish order event
            self.event_bus.publish(
                "order.submitted",
                OrderSubmittedEvent { order_id, order }
            ).await?;
            
            Ok(order_id)
        }
    }
    ```
  
  execution_algorithms:
    twap:
      description: "Time-weighted average price"
      parameters: ["duration", "slice_count"]
    
    vwap:
      description: "Volume-weighted average price"
      parameters: ["duration", "participation_rate"]
    
    iceberg:
      description: "Hidden quantity execution"
      parameters: ["display_size", "total_size"]
    
    sniper:
      description: "Aggressive liquidity taking"
      parameters: ["urgency", "max_spread"]
```

### 7. Performance Analytics Component

```yaml
component:
  name: "PerformanceAnalytics"
  type: "Domain Service"
  domain: "Analytics"
  
  architecture:
    ```python
    class PerformanceAnalytics:
        def __init__(self):
            self.metrics_engine = MetricsEngine()
            self.attribution_engine = AttributionEngine()
            self.risk_analytics = RiskAnalytics()
            self.report_generator = ReportGenerator()
        
        async def calculate_pnl(self, portfolio: Portfolio, timeframe: TimeFrame):
            """Calculate P&L for portfolio"""
            positions = await self.get_positions(portfolio, timeframe)
            prices = await self.get_prices(positions.symbols, timeframe)
            
            pnl = PnL()
            for position in positions:
                entry_price = position.entry_price
                current_price = prices[position.symbol]
                
                unrealized = (current_price - entry_price) * position.quantity
                realized = position.realized_pnl
                
                pnl.add(position.symbol, unrealized, realized)
            
            return pnl
        
        async def calculate_metrics(self, returns: Series) -> Metrics:
            """Calculate performance metrics"""
            return Metrics(
                total_return=returns.sum(),
                annualized_return=self.annualize(returns.mean()),
                volatility=returns.std() * sqrt(252),
                sharpe_ratio=self.sharpe(returns),
                sortino_ratio=self.sortino(returns),
                max_drawdown=self.max_drawdown(returns),
                calmar_ratio=self.calmar(returns),
                win_rate=self.win_rate(returns),
                profit_factor=self.profit_factor(returns)
            )
    ```
  
  real_time_dashboards:
    metrics:
      - pnl_live
      - positions_summary
      - risk_exposure
      - performance_attribution
    
    update_frequency: "1 second"
    
    visualization:
      - time_series_charts
      - heat_maps
      - scatter_plots
      - distribution_histograms
```

### 8. Monitoring & Observability Component

```yaml
component:
  name: "ObservabilityStack"
  type: "Infrastructure Service"
  
  subcomponents:
    metrics_collector:
      technology: "Prometheus"
      scrape_interval: "15s"
      retention: "30d"
      
      custom_metrics:
        - trading_signals_generated
        - orders_executed
        - model_inference_latency
        - feature_computation_time
    
    distributed_tracing:
      technology: "Jaeger/OpenTelemetry"
      sampling_rate: 0.1
      
      trace_points:
        - order_lifecycle
        - strategy_execution
        - model_inference
        - data_pipeline
    
    log_aggregation:
      technology: "ELK Stack"
      
      log_levels:
        - ERROR: "All errors and exceptions"
        - WARN: "Warnings and degraded performance"
        - INFO: "Key business events"
        - DEBUG: "Detailed execution flow"
    
    alerting:
      technology: "AlertManager"
      
      alert_rules:
        - name: "High Error Rate"
          condition: "error_rate > 0.01"
          severity: "critical"
        
        - name: "Model Drift"
          condition: "model_accuracy < 0.8"
          severity: "warning"
        
        - name: "Order Rejection"
          condition: "order_rejection_rate > 0.05"
          severity: "critical"
```

## Component Integration Patterns

### Event-Driven Integration

```yaml
integration_patterns:
  event_sourcing:
    description: "Store all changes as events"
    benefits:
      - Complete audit trail
      - Time travel debugging
      - Event replay capability
    
    implementation:
      event_store: "EventStore/Kafka"
      snapshot_interval: 1000
      retention_days: 90
  
  saga_pattern:
    description: "Distributed transaction management"
    use_cases:
      - Multi-step order execution
      - Complex strategy deployment
      - Model training pipelines
    
    implementation:
      orchestrator: "Temporal"
      compensation_strategy: "automatic"
  
  cqrs:
    description: "Command Query Responsibility Segregation"
    benefits:
      - Optimized read/write paths
      - Independent scaling
      - Simplified queries
    
    implementation:
      command_store: "PostgreSQL"
      query_store: "Elasticsearch"
      sync_mechanism: "CDC (Change Data Capture)"
```

## Component Lifecycle Management

### Deployment Pipeline

```yaml
deployment:
  stages:
    build:
      - compile_code
      - run_tests
      - build_container
      - scan_vulnerabilities
    
    test:
      - deploy_to_staging
      - run_integration_tests
      - run_performance_tests
      - run_chaos_tests
    
    release:
      - blue_green_deployment
      - canary_rollout
      - feature_flags
      - gradual_rollout
    
    monitor:
      - health_checks
      - metric_validation
      - error_rate_monitoring
      - rollback_triggers
```

### Version Management

```yaml
versioning:
  strategy: "Semantic Versioning"
  
  compatibility:
    backward: "Minor and patch versions"
    forward: "Patch versions only"
  
  deprecation:
    notice_period: "3 months"
    migration_guide: "Required"
    sunset_period: "6 months"
```

## Performance Requirements

```yaml
performance_targets:
  market_data_service:
    latency_p99: "< 1ms"
    throughput: "> 1M msgs/sec"
    availability: "99.99%"
  
  strategy_engine:
    signal_generation: "< 10ms"
    order_submission: "< 5ms"
    availability: "99.95%"
  
  ml_platform:
    inference_latency: "< 20ms"
    training_time: "< 1 hour"
    model_deployment: "< 5 minutes"
  
  order_management:
    order_latency: "< 2ms"
    fill_processing: "< 1ms"
    availability: "99.99%"
```