# Comprehensive Model Performance Tracking

## Core Question: "Are All 27 Models Actually Valuable?"

Once we have all models running, we need comprehensive tracking to answer:
- Which models consistently outperform?
- Which models are redundant?
- Which combinations work best?
- Which models should be deactivated to save resources?

## Performance Tracking Architecture

### 1. Model Performance Metrics Database

```rust
// src/monitoring/model_performance_tracker.rs
pub struct ModelPerformanceTracker {
    /// Individual model metrics per symbol
    model_metrics: Arc<DashMap<(String, String), ModelMetrics>>, // (symbol, model_id)
    /// Ensemble performance tracking
    ensemble_metrics: Arc<DashMap<String, EnsembleMetrics>>, // symbol -> metrics
    /// Historical performance database
    performance_db: Arc<PerformanceDatabase>,
    /// Real-time performance dashboard
    dashboard: Arc<PerformanceDashboard>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    pub model_id: String,
    pub symbol: String,
    
    // Accuracy Metrics
    pub prediction_accuracy: f64,           // Directional accuracy %
    pub mape: f64,                         // Mean Absolute Percentage Error
    pub rmse: f64,                         // Root Mean Square Error
    pub mae: f64,                          // Mean Absolute Error
    pub r_squared: f64,                    // R-squared correlation
    
    // Trading Performance
    pub sharpe_ratio: f64,                 // Risk-adjusted returns
    pub win_rate: f64,                     // % of profitable predictions
    pub max_drawdown: f64,                 // Maximum loss streak
    pub profit_factor: f64,                // Gross profit / Gross loss
    pub calmar_ratio: f64,                 // Annual return / Max drawdown
    
    // Reliability Metrics
    pub prediction_count: u64,             // Total predictions made
    pub consecutive_failures: u32,         // Current failure streak
    pub confidence_calibration: f64,       // How well confidence matches accuracy
    pub prediction_latency_ms: f64,        // Average prediction time
    
    // Data Dependency Analysis
    pub performance_by_data_richness: HashMap<String, f64>, // Performance vs available data
    pub optimal_data_combination: Vec<DataType>,            // Best data combo for this model
    
    // Time-based Performance
    pub performance_trend_30d: f64,        // Performance trend (positive = improving)
    pub performance_by_time_of_day: HashMap<u8, f64>,      // Hour -> performance
    pub performance_by_market_regime: HashMap<MarketRegime, f64>,
    
    // Resource Usage
    pub memory_usage_mb: f64,              // Memory consumption
    pub cpu_usage_percent: f64,            // CPU utilization
    pub inference_cost_per_prediction: f64, // Resource cost
    
    // Timestamps
    pub last_updated: DateTime<Utc>,
    pub first_prediction: DateTime<Utc>,
    pub last_successful_prediction: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct EnsembleMetrics {
    pub symbol: String,
    pub total_models_active: usize,
    pub ensemble_accuracy: f64,
    pub ensemble_sharpe: f64,
    
    // Model Contribution Analysis
    pub model_contributions: HashMap<String, f64>, // Model ID -> contribution weight
    pub top_performers: Vec<String>,               // Best 5 models for this symbol
    pub underperformers: Vec<String>,              // Worst performers
    pub redundant_models: Vec<String>,             // Models that don't add value
    
    // Diversity Metrics
    pub prediction_correlation_matrix: HashMap<(String, String), f64>,
    pub ensemble_diversity_score: f64,            // How diverse are the predictions
    pub optimal_ensemble_size: usize,             // Best number of models for this symbol
}
```

### 2. Real-Time Performance Monitoring

```rust
impl ModelPerformanceTracker {
    /// Record prediction and track against actual outcome
    pub async fn record_prediction(
        &self,
        symbol: &str,
        model_id: &str,
        prediction: &PredictionResult,
        actual_outcome: Option<f64>, // None if outcome not yet known
        market_context: &MarketContext,
    ) -> Result<()> {
        let key = (symbol.to_string(), model_id.to_string());
        
        // Update prediction count
        let mut metrics = self.model_metrics.entry(key.clone())
            .or_insert_with(|| ModelMetrics::new(symbol, model_id));
        
        metrics.prediction_count += 1;
        metrics.last_updated = Utc::now();
        
        // If we have an outcome, calculate accuracy
        if let Some(actual) = actual_outcome {
            self.update_accuracy_metrics(&mut metrics, prediction, actual).await?;
            self.update_trading_metrics(&mut metrics, prediction, actual, market_context).await?;
        }
        
        // Update resource usage
        metrics.memory_usage_mb = self.get_model_memory_usage(model_id).await?;
        metrics.cpu_usage_percent = self.get_model_cpu_usage(model_id).await?;
        
        // Store in performance database for historical analysis
        self.performance_db.store_prediction_record(
            symbol, model_id, prediction, actual_outcome, market_context
        ).await?;
        
        Ok(())
    }
    
    /// Generate model value assessment report
    pub async fn generate_model_value_report(&self, symbol: &str) -> Result<ModelValueReport> {
        let all_models = self.get_models_for_symbol(symbol).await?;
        let mut model_rankings = Vec::new();
        
        for (model_id, metrics) in all_models {
            let value_score = self.calculate_model_value_score(&metrics).await?;
            model_rankings.push(ModelRanking {
                model_id: model_id.clone(),
                value_score,
                metrics: metrics.clone(),
                recommendation: self.get_model_recommendation(&metrics, value_score),
            });
        }
        
        // Sort by value score
        model_rankings.sort_by(|a, b| b.value_score.partial_cmp(&a.value_score).unwrap());
        
        Ok(ModelValueReport {
            symbol: symbol.to_string(),
            total_models: model_rankings.len(),
            top_performers: model_rankings.iter().take(5).cloned().collect(),
            underperformers: model_rankings.iter().rev().take(5).cloned().collect(),
            recommendations: self.generate_optimization_recommendations(&model_rankings),
            resource_savings_potential: self.calculate_resource_savings(&model_rankings),
        })
    }
    
    /// Calculate comprehensive model value score
    async fn calculate_model_value_score(&self, metrics: &ModelMetrics) -> Result<f64> {
        // Weighted scoring formula
        let accuracy_weight = 0.3;
        let trading_weight = 0.3;
        let reliability_weight = 0.2;
        let efficiency_weight = 0.2;
        
        let accuracy_score = (metrics.prediction_accuracy * 0.4 + 
                             (1.0 - metrics.mape / 100.0) * 0.3 + 
                             metrics.r_squared * 0.3).max(0.0);
        
        let trading_score = (metrics.sharpe_ratio / 3.0).min(1.0).max(0.0) * 0.4 +
                           metrics.win_rate * 0.3 +
                           (1.0 - metrics.max_drawdown) * 0.3;
        
        let reliability_score = (metrics.confidence_calibration * 0.4 +
                               (1.0 - metrics.consecutive_failures as f64 / 10.0).max(0.0) * 0.6);
        
        let efficiency_score = (1.0 - (metrics.memory_usage_mb / 1000.0).min(1.0)) * 0.5 +
                              (1.0 - (metrics.prediction_latency_ms / 1000.0).min(1.0)) * 0.5;
        
        let total_score = accuracy_score * accuracy_weight +
                         trading_score * trading_weight +
                         reliability_score * reliability_weight +
                         efficiency_score * efficiency_weight;
        
        Ok(total_score)
    }
}

#[derive(Debug, Clone)]
pub struct ModelValueReport {
    pub symbol: String,
    pub total_models: usize,
    pub top_performers: Vec<ModelRanking>,
    pub underperformers: Vec<ModelRanking>,
    pub recommendations: OptimizationRecommendations,
    pub resource_savings_potential: ResourceSavings,
}

#[derive(Debug, Clone)]
pub struct ModelRanking {
    pub model_id: String,
    pub value_score: f64,
    pub metrics: ModelMetrics,
    pub recommendation: ModelRecommendation,
}

#[derive(Debug, Clone)]
pub enum ModelRecommendation {
    Keep { reason: String },
    Optimize { changes: Vec<String> },
    Deactivate { reason: String, savings: ResourceSavings },
    Retrain { urgency: TrainingUrgency },
}

#[derive(Debug, Clone)]
pub struct OptimizationRecommendations {
    pub models_to_deactivate: Vec<String>,
    pub models_to_retrain: Vec<String>,
    pub ensemble_size_recommendation: usize,
    pub resource_optimization_potential: f64, // % savings possible
}
```

### 3. Performance Dashboard and Reporting

```rust
// src/monitoring/performance_dashboard.rs
pub struct PerformanceDashboard {
    web_server: Arc<DashboardServer>,
    metrics_aggregator: Arc<MetricsAggregator>,
}

impl PerformanceDashboard {
    /// Generate real-time performance summary
    pub async fn generate_live_summary(&self) -> Result<PerformanceSummary> {
        let all_symbols = self.get_active_symbols().await?;
        let mut summary = PerformanceSummary::default();
        
        for symbol in all_symbols {
            let symbol_report = self.metrics_aggregator
                .generate_model_value_report(&symbol)
                .await?;
            
            summary.total_models += symbol_report.total_models;
            summary.high_value_models += symbol_report.top_performers.len();
            summary.low_value_models += symbol_report.underperformers.len();
            
            // Aggregate resource usage
            summary.total_memory_mb += symbol_report.top_performers
                .iter()
                .map(|r| r.metrics.memory_usage_mb)
                .sum::<f64>();
        }
        
        summary.efficiency_ratio = summary.high_value_models as f64 / summary.total_models as f64;
        
        Ok(summary)
    }
    
    /// Export detailed performance report
    pub async fn export_performance_report(
        &self,
        format: ReportFormat,
        time_period: TimePeriod,
    ) -> Result<String> {
        let report = self.generate_comprehensive_report(time_period).await?;
        
        match format {
            ReportFormat::Json => Ok(serde_json::to_string_pretty(&report)?),
            ReportFormat::Csv => self.convert_to_csv(&report),
            ReportFormat::Html => self.generate_html_report(&report),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub total_models: usize,
    pub high_value_models: usize,    // Value score > 0.7
    pub medium_value_models: usize,  // Value score 0.4-0.7
    pub low_value_models: usize,     // Value score < 0.4
    pub efficiency_ratio: f64,       // High value / Total
    pub total_memory_mb: f64,
    pub avg_prediction_latency_ms: f64,
    pub resource_optimization_potential: f64,
}
```

### 4. DAA Autonomous Training Integration

**🚨 CRITICAL REQUIREMENT**: The DAA autonomous training system MUST receive all performance data to make informed training decisions.

```rust
// src/daa/autonomous_training.rs - Enhanced with performance data
impl AutonomousTrainingEngine {
    /// Make training decisions based on comprehensive performance data
    pub async fn evaluate_training_need(
        &self,
        model_id: &str,
        symbol: &str
    ) -> Result<TrainingDecision> {
        // Get current performance metrics from tracker
        let performance_data = self.performance_tracker
            .get_model_metrics(symbol, model_id)
            .await?;
        
        // DAA decision logic enhanced with performance data
        let training_urgency = self.calculate_training_urgency(&performance_data)?;
        
        // Consider multiple performance factors
        let should_train = self.should_retrain_model(&performance_data)?;
        
        if should_train {
            info!("🚨 DAA Training Triggered for {} ({}): Accuracy: {:.1}%, Sharpe: {:.2}, Failures: {}", 
                model_id, symbol, 
                performance_data.prediction_accuracy * 100.0,
                performance_data.sharpe_ratio,
                performance_data.consecutive_failures
            );
        }
        
        Ok(TrainingDecision {
            should_train,
            urgency: training_urgency,
            performance_context: performance_data,
            training_strategy: self.select_training_strategy(&performance_data)?,
        })
    }
    
    /// Enhanced training urgency calculation with performance data
    fn calculate_training_urgency(&self, metrics: &ModelMetrics) -> Result<f64> {
        let mut urgency = 0.0;
        
        // Accuracy degradation (0.0 - 0.4 urgency)
        if metrics.prediction_accuracy < 0.8 {
            urgency += 0.4 * (0.8 - metrics.prediction_accuracy) / 0.8;
        }
        
        // Trading performance degradation (0.0 - 0.3 urgency)
        if metrics.sharpe_ratio < 1.0 {
            urgency += 0.3 * (1.0 - metrics.sharpe_ratio.max(0.0));
        }
        
        // Consecutive failures (0.0 - 0.2 urgency)
        urgency += 0.2 * (metrics.consecutive_failures as f64 / 10.0).min(1.0);
        
        // Performance trend (0.0 - 0.1 urgency)
        if metrics.performance_trend_30d < 0.0 {
            urgency += 0.1 * (-metrics.performance_trend_30d).min(1.0);
        }
        
        Ok(urgency.min(1.0))
    }
    
    /// Determine if model should be retrained based on performance
    fn should_retrain_model(&self, metrics: &ModelMetrics) -> Result<bool> {
        // Hard thresholds that trigger retraining
        let hard_triggers = vec![
            metrics.prediction_accuracy < 0.6,           // Very poor accuracy
            metrics.consecutive_failures >= 5,           // Too many failures
            metrics.sharpe_ratio < 0.5,                 // Poor risk-adjusted returns
            metrics.max_drawdown > 0.3,                 // Excessive losses
            metrics.performance_trend_30d < -0.2,       // Strong negative trend
        ];
        
        // Soft thresholds (need multiple to trigger)
        let soft_triggers = vec![
            metrics.prediction_accuracy < 0.75,
            metrics.win_rate < 0.6,
            metrics.confidence_calibration < 0.7,
            metrics.r_squared < 0.3,
        ];
        
        let hard_trigger_count = hard_triggers.iter().filter(|&&x| x).count();
        let soft_trigger_count = soft_triggers.iter().filter(|&&x| x).count();
        
        // Retrain if any hard trigger OR 3+ soft triggers
        Ok(hard_trigger_count > 0 || soft_trigger_count >= 3)
    }
}

#[derive(Debug, Clone)]
pub struct TrainingDecision {
    pub should_train: bool,
    pub urgency: f64,                    // 0.0 to 1.0
    pub performance_context: ModelMetrics,
    pub training_strategy: TrainingStrategy,
}

#[derive(Debug, Clone)]
pub enum TrainingStrategy {
    FullRetrain,                        // Complete retraining
    IncrementalUpdate,                  // Fine-tuning with recent data
    ArchitectureAdjustment,            // Modify model complexity
    DataAugmentation,                  // Add more training data
    EnsembleRebalancing,              // Adjust ensemble weights
}
```

### 5. Performance-Driven DAA Scheduler Integration

```rust
// src/daa/training_scheduler.rs - Enhanced with performance monitoring
impl DAATrainingScheduler {
    /// Schedule training based on real-time performance monitoring
    pub async fn schedule_performance_driven_training(&mut self) -> Result<()> {
        // Get all active models and their performance
        let all_models = self.performance_tracker.get_all_active_models().await?;
        
        for (symbol, model_id) in all_models {
            let metrics = self.performance_tracker
                .get_model_metrics(&symbol, &model_id)
                .await?;
            
            // DAA evaluates training need based on performance
            let decision = self.training_engine
                .evaluate_training_need(&model_id, &symbol)
                .await?;
            
            if decision.should_train {
                // Schedule training with urgency-based priority
                let priority = match decision.urgency {
                    x if x > 0.8 => TrainingPriority::Emergency,
                    x if x > 0.6 => TrainingPriority::Critical,  
                    x if x > 0.4 => TrainingPriority::High,
                    _ => TrainingPriority::Medium,
                };
                
                self.schedule_training_job(TrainingJob {
                    model_id,
                    symbol,
                    priority,
                    strategy: decision.training_strategy,
                    performance_context: decision.performance_context,
                    scheduled_time: self.calculate_optimal_training_time(decision.urgency)?,
                }).await?;
                
                info!("📅 Scheduled {} training for {} with {} priority based on performance data",
                    decision.training_strategy.name(), model_id, priority);
            }
        }
        
        Ok(())
    }
}
```

### 6. Automated Model Optimization

#[derive(Debug, Clone)]
pub struct OptimizationThresholds {
    pub min_value_score: f64,           // 0.3 - Below this, consider deactivation
    pub max_memory_usage_mb: f64,       // 500 - Memory limit per model
    pub max_consecutive_failures: u32,   // 10 - Max failures before concern
    pub min_prediction_count: u64,       // 100 - Min predictions before evaluation
}

impl ModelOptimizer {
    /// Run automated optimization check
    pub async fn run_optimization_cycle(&self) -> Result<OptimizationActions> {
        let mut actions = OptimizationActions::default();
        
        for symbol in self.get_all_symbols().await? {
            let report = self.performance_tracker
                .generate_model_value_report(&symbol)
                .await?;
            
            // Identify models to deactivate
            for ranking in &report.underperformers {
                if ranking.value_score < self.threshold_config.min_value_score
                    && ranking.metrics.prediction_count > self.threshold_config.min_prediction_count {
                    actions.deactivate_model.push(DeactivationAction {
                        symbol: symbol.clone(),
                        model_id: ranking.model_id.clone(),
                        reason: format!("Low value score: {:.3}", ranking.value_score),
                        memory_savings_mb: ranking.metrics.memory_usage_mb,
                    });
                }
            }
            
            // Identify redundant models (high correlation, similar performance)
            let redundant = self.find_redundant_models(&report).await?;
            actions.deactivate_model.extend(redundant);
        }
        
        Ok(actions)
    }
}
```

## Key Tracking Data Points

### 1. **Performance Metrics**
- ✅ Prediction accuracy, RMSE, MAE, R-squared
- ✅ Trading performance (Sharpe, win rate, drawdown)
- ✅ Confidence calibration
- ✅ Performance trends over time

### 2. **Resource Usage**
- ✅ Memory consumption per model
- ✅ CPU utilization
- ✅ Prediction latency
- ✅ Cost per prediction

### 3. **Value Assessment**
- ✅ Comprehensive value score (accuracy + trading + reliability + efficiency)
- ✅ Model ranking system
- ✅ Redundancy detection
- ✅ Optimization recommendations

### 4. **Reporting Capabilities**
- ✅ Real-time dashboard
- ✅ Detailed performance reports (JSON/CSV/HTML)
- ✅ Model deactivation recommendations
- ✅ Resource optimization potential

## Dashboard Visualization

The system provides clear answers to your questions:

```
📊 Model Performance Dashboard - AAPL

🏆 Top Performers (Value Score > 0.8):
├── TFT_Full: 0.87 (92% accuracy, 2.1 Sharpe, 45MB)
├── NHITS_Macro: 0.84 (89% accuracy, 1.9 Sharpe, 32MB)
└── DeepAR_Sentiment: 0.81 (88% accuracy, 1.7 Sharpe, 67MB)

⚠️  Underperformers (Value Score < 0.4):
├── MLP_Basic: 0.32 (67% accuracy, 0.8 Sharpe, 12MB) ❌ DEACTIVATE
├── DLinear_Price: 0.28 (61% accuracy, 0.6 Sharpe, 8MB) ❌ DEACTIVATE
└── GRU_PV: 0.35 (64% accuracy, 0.7 Sharpe, 28MB) ❌ DEACTIVATE

💡 Optimization Potential: 48MB memory savings, 3 redundant models
📈 Overall Efficiency: 68% (18/27 models providing value)
```

This comprehensive tracking system ensures you have all the data needed to answer "which models are actually valuable" and optimize your neural architecture for maximum performance per resource unit.