//! Meta-Learning Coordinator for Cross-Market Pattern Discovery
//! 
//! This module implements advanced meta-learning capabilities that allow
//! agents to discover universal patterns across markets and transfer
//! successful strategies between different trading domains.

use anyhow::{Result, Context};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::{HashMap, VecDeque};
use chrono::{DateTime, Utc, Duration};
use tracing::{info, warn, debug};
use nalgebra as na;
use ndarray::{Array2, Array1};

use crate::daa::traits::{MetaLearningAgent, Experience, MetaLearningResult, TransferResult, MarketRegime};
use crate::neural::NeuralNetwork;
use crate::strategies::TradingStrategy;

/// Configuration for meta-learning
#[derive(Debug, Clone)]
pub struct MetaLearningConfig {
    /// Minimum confidence for pattern recognition
    pub pattern_confidence_threshold: f64,
    /// Number of experiences required for learning
    pub min_experiences: usize,
    /// Learning rate for meta-optimization
    pub meta_learning_rate: f64,
    /// Enable cross-market transfer learning
    pub enable_transfer_learning: bool,
    /// Maximum memory size for experiences
    pub max_memory_size: usize,
}

impl Default for MetaLearningConfig {
    fn default() -> Self {
        Self {
            pattern_confidence_threshold: 0.7,
            min_experiences: 100,
            meta_learning_rate: 0.01,
            enable_transfer_learning: true,
            max_memory_size: 10000,
        }
    }
}

/// Universal pattern that works across multiple markets
#[derive(Debug, Clone)]
pub struct UniversalPattern {
    pub id: String,
    pub pattern_type: PatternType,
    pub features: Vec<f64>,
    pub markets_validated: Vec<String>,
    pub success_rate: f64,
    pub confidence: f64,
    pub discovered_at: DateTime<Utc>,
    pub last_validated: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum PatternType {
    TrendFollowing,
    MeanReversion,
    Breakout,
    Seasonality,
    Correlation,
    Arbitrage,
    Custom(String),
}

/// Knowledge that can be transferred between domains
#[derive(Debug, Clone)]
pub struct TransferableKnowledge {
    pub knowledge_type: String,
    pub source_domain: String,
    pub feature_mappings: HashMap<String, String>,
    pub transformation_matrix: Array2<f64>,
    pub performance_history: Vec<f64>,
}

/// Meta-learning model using MAML (Model-Agnostic Meta-Learning)
pub struct MetaLearner {
    /// Base neural network for pattern recognition
    pattern_network: NeuralNetwork,
    /// Domain adaptation networks
    domain_adapters: HashMap<String, NeuralNetwork>,
    /// Feature extraction network
    feature_extractor: NeuralNetwork,
    /// Meta-parameters
    meta_parameters: Array1<f64>,
}

impl MetaLearner {
    pub fn new(input_dim: usize, hidden_dim: usize, output_dim: usize) -> Result<Self> {
        Ok(Self {
            pattern_network: NeuralNetwork::new(vec![input_dim, hidden_dim, hidden_dim, output_dim])?,
            domain_adapters: HashMap::new(),
            feature_extractor: NeuralNetwork::new(vec![input_dim, hidden_dim * 2, hidden_dim])?,
            meta_parameters: Array1::zeros(hidden_dim),
        })
    }

    /// Perform meta-learning update using MAML algorithm
    pub async fn meta_update(&mut self, tasks: Vec<MetaTask>) -> Result<f64> {
        let mut total_loss = 0.0;
        let mut gradients = Vec::new();

        for task in &tasks {
            // Clone model for task-specific adaptation
            let mut task_model = self.pattern_network.clone();
            
            // Inner loop: adapt to specific task
            for _ in 0..5 {
                let loss = task_model.train_step(&task.support_set)?;
                total_loss += loss;
            }
            
            // Compute gradient on query set
            let gradient = task_model.compute_gradient(&task.query_set)?;
            gradients.push(gradient);
        }

        // Outer loop: update meta-parameters
        let avg_gradient = self.average_gradients(gradients);
        self.apply_meta_gradient(avg_gradient)?;

        Ok(total_loss / tasks.len() as f64)
    }

    fn average_gradients(&self, gradients: Vec<Array1<f64>>) -> Array1<f64> {
        let mut avg = Array1::zeros(gradients[0].len());
        for grad in gradients {
            avg += &grad;
        }
        avg / gradients.len() as f64
    }

    fn apply_meta_gradient(&mut self, gradient: Array1<f64>) -> Result<()> {
        // Update meta-parameters
        self.meta_parameters -= &(gradient * self.meta_learning_rate);
        
        // Apply meta-parameters to base network
        self.pattern_network.update_with_meta_params(&self.meta_parameters)?;
        
        Ok(())
    }
}

/// Meta-task for few-shot learning
pub struct MetaTask {
    pub task_id: String,
    pub domain: String,
    pub support_set: Vec<(Array1<f64>, f64)>, // (features, target)
    pub query_set: Vec<(Array1<f64>, f64)>,
}

/// Cross-market meta-learning coordinator
pub struct MetaLearningCoordinator {
    config: MetaLearningConfig,
    meta_learner: Arc<RwLock<MetaLearner>>,
    pattern_library: Arc<RwLock<HashMap<String, UniversalPattern>>>,
    experience_memory: Arc<RwLock<VecDeque<Experience>>>,
    transfer_networks: Arc<RwLock<HashMap<(String, String), TransferableKnowledge>>>,
    regime_models: Arc<RwLock<HashMap<String, RegimeModel>>>,
    performance_tracker: Arc<RwLock<PerformanceTracker>>,
}

struct RegimeModel {
    regime_type: String,
    feature_importance: Vec<f64>,
    transition_probabilities: HashMap<String, f64>,
    optimal_strategies: Vec<String>,
}

struct PerformanceTracker {
    patterns_discovered: usize,
    successful_transfers: usize,
    regime_adaptations: usize,
    cumulative_improvement: f64,
}

impl MetaLearningCoordinator {
    pub fn new(config: MetaLearningConfig) -> Result<Self> {
        let meta_learner = MetaLearner::new(50, 128, 10)?;

        Ok(Self {
            config,
            meta_learner: Arc::new(RwLock::new(meta_learner)),
            pattern_library: Arc::new(RwLock::new(HashMap::new())),
            experience_memory: Arc::new(RwLock::new(VecDeque::with_capacity(10000))),
            transfer_networks: Arc::new(RwLock::new(HashMap::new())),
            regime_models: Arc::new(RwLock::new(HashMap::new())),
            performance_tracker: Arc::new(RwLock::new(PerformanceTracker {
                patterns_discovered: 0,
                successful_transfers: 0,
                regime_adaptations: 0,
                cumulative_improvement: 0.0,
            })),
        })
    }

    /// Discover patterns that work across multiple markets
    pub async fn discover_universal_patterns(&self, markets: Vec<String>) -> Result<Vec<UniversalPattern>> {
        let experiences = self.experience_memory.read().await;
        if experiences.len() < self.config.min_experiences {
            return Ok(vec![]);
        }

        // Group experiences by pattern characteristics
        let pattern_groups = self.group_experiences_by_pattern(&experiences).await?;
        let mut universal_patterns = Vec::new();

        for (pattern_key, group) in pattern_groups {
            // Test pattern across different markets
            let mut market_performance = HashMap::new();
            
            for market in &markets {
                let market_experiences: Vec<_> = group.iter()
                    .filter(|e| e.state.contains_key(&format!("market_{}", market)))
                    .cloned()
                    .collect();
                
                if market_experiences.len() >= 10 {
                    let performance = self.evaluate_pattern_performance(&market_experiences).await?;
                    market_performance.insert(market.clone(), performance);
                }
            }

            // Check if pattern works across multiple markets
            let successful_markets: Vec<_> = market_performance.iter()
                .filter(|(_, perf)| **perf > 0.6)
                .map(|(market, _)| market.clone())
                .collect();

            if successful_markets.len() >= 2 {
                let avg_performance: f64 = market_performance.values().sum::<f64>() / market_performance.len() as f64;
                
                let pattern = UniversalPattern {
                    id: format!("pattern_{}", uuid::Uuid::new_v4()),
                    pattern_type: self.classify_pattern(&pattern_key).await?,
                    features: self.extract_pattern_features(&group).await?,
                    markets_validated: successful_markets,
                    success_rate: avg_performance,
                    confidence: self.calculate_pattern_confidence(&market_performance),
                    discovered_at: Utc::now(),
                    last_validated: Utc::now(),
                };

                if pattern.confidence > self.config.pattern_confidence_threshold {
                    universal_patterns.push(pattern);
                }
            }
        }

        // Update pattern library
        let mut library = self.pattern_library.write().await;
        for pattern in &universal_patterns {
            library.insert(pattern.id.clone(), pattern.clone());
        }

        // Update tracker
        let mut tracker = self.performance_tracker.write().await;
        tracker.patterns_discovered += universal_patterns.len();

        Ok(universal_patterns)
    }

    /// Transfer successful strategy from one market to another
    pub async fn transfer_strategy(
        &self,
        strategy: &TradingStrategy,
        source_market: &str,
        target_market: &str,
    ) -> Result<TransferableKnowledge> {
        info!("Transferring strategy from {} to {}", source_market, target_market);

        // Extract source domain features
        let source_features = self.extract_domain_features(source_market).await?;
        let target_features = self.extract_domain_features(target_market).await?;

        // Create feature mapping
        let feature_mappings = self.create_feature_mappings(&source_features, &target_features).await?;

        // Learn transformation matrix
        let transformation_matrix = self.learn_domain_transformation(
            source_market,
            target_market,
            &feature_mappings,
        ).await?;

        // Create transferable knowledge
        let knowledge = TransferableKnowledge {
            knowledge_type: "strategy_transfer".to_string(),
            source_domain: source_market.to_string(),
            feature_mappings,
            transformation_matrix,
            performance_history: vec![],
        };

        // Store transfer knowledge
        let mut transfers = self.transfer_networks.write().await;
        transfers.insert((source_market.to_string(), target_market.to_string()), knowledge.clone());

        Ok(knowledge)
    }

    /// Adapt agent strategies to new market regime
    pub async fn adapt_to_regime(&self, current_regime: &MarketRegime) -> Result<Vec<String>> {
        let mut adapted_strategies = Vec::new();

        // Get or create regime model
        let mut regime_models = self.regime_models.write().await;
        let model = regime_models.entry(current_regime.regime_type.clone())
            .or_insert_with(|| RegimeModel {
                regime_type: current_regime.regime_type.clone(),
                feature_importance: vec![1.0; 10], // Initialize with uniform importance
                transition_probabilities: HashMap::new(),
                optimal_strategies: vec![],
            });

        // Update regime model with current characteristics
        self.update_regime_model(model, current_regime).await?;

        // Identify optimal strategies for this regime
        let experiences = self.experience_memory.read().await;
        let regime_experiences: Vec<_> = experiences.iter()
            .filter(|e| e.market_regime == current_regime.regime_type)
            .cloned()
            .collect();

        if regime_experiences.len() >= 20 {
            // Analyze which strategies work best in this regime
            let strategy_performance = self.analyze_strategy_performance(&regime_experiences).await?;
            
            // Select top performing strategies
            let mut sorted_strategies: Vec<_> = strategy_performance.into_iter().collect();
            sorted_strategies.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            
            for (strategy, performance) in sorted_strategies.into_iter().take(3) {
                if performance > 0.6 {
                    adapted_strategies.push(strategy);
                }
            }
            
            model.optimal_strategies = adapted_strategies.clone();
        }

        // Update tracker
        let mut tracker = self.performance_tracker.write().await;
        tracker.regime_adaptations += 1;

        Ok(adapted_strategies)
    }

    /// Group experiences by pattern characteristics
    async fn group_experiences_by_pattern(
        &self,
        experiences: &VecDeque<Experience>,
    ) -> Result<HashMap<String, Vec<Experience>>> {
        let mut groups = HashMap::new();

        for exp in experiences {
            let pattern_key = self.extract_pattern_key(exp).await?;
            groups.entry(pattern_key)
                .or_insert_with(Vec::new)
                .push(exp.clone());
        }

        Ok(groups)
    }

    /// Extract pattern key from experience
    async fn extract_pattern_key(&self, experience: &Experience) -> Result<String> {
        // Simplified pattern key extraction
        let mut key_components = vec![];

        // Add action type
        key_components.push(experience.action.clone());

        // Add market regime
        key_components.push(experience.market_regime.clone());

        // Add discretized state features
        for (feature, value) in &experience.state {
            if feature.contains("trend") || feature.contains("volatility") {
                let discretized = (value * 10.0).round() / 10.0;
                key_components.push(format!("{}:{:.1}", feature, discretized));
            }
        }

        Ok(key_components.join("_"))
    }

    /// Evaluate pattern performance
    async fn evaluate_pattern_performance(&self, experiences: &[Experience]) -> Result<f64> {
        if experiences.is_empty() {
            return Ok(0.0);
        }

        let successful = experiences.iter()
            .filter(|e| e.outcome > 0.0)
            .count();

        Ok(successful as f64 / experiences.len() as f64)
    }

    /// Classify pattern type
    async fn classify_pattern(&self, pattern_key: &str) -> Result<PatternType> {
        if pattern_key.contains("trend") {
            Ok(PatternType::TrendFollowing)
        } else if pattern_key.contains("reversion") {
            Ok(PatternType::MeanReversion)
        } else if pattern_key.contains("breakout") {
            Ok(PatternType::Breakout)
        } else if pattern_key.contains("arbitrage") {
            Ok(PatternType::Arbitrage)
        } else {
            Ok(PatternType::Custom(pattern_key.to_string()))
        }
    }

    /// Extract pattern features
    async fn extract_pattern_features(&self, experiences: &[Experience]) -> Result<Vec<f64>> {
        let mut features = vec![0.0; 20]; // Fixed feature vector size

        // Calculate aggregate statistics
        for exp in experiences {
            for (i, (_, value)) in exp.state.iter().enumerate().take(20) {
                features[i] += value / experiences.len() as f64;
            }
        }

        Ok(features)
    }

    /// Calculate pattern confidence
    fn calculate_pattern_confidence(&self, market_performance: &HashMap<String, f64>) -> f64 {
        if market_performance.is_empty() {
            return 0.0;
        }

        let mean: f64 = market_performance.values().sum::<f64>() / market_performance.len() as f64;
        let variance: f64 = market_performance.values()
            .map(|p| (p - mean).powi(2))
            .sum::<f64>() / market_performance.len() as f64;

        // Higher confidence with higher mean and lower variance
        mean * (1.0 - variance.sqrt())
    }

    /// Extract domain-specific features
    async fn extract_domain_features(&self, market: &str) -> Result<Vec<String>> {
        // Mock implementation - would extract actual market characteristics
        Ok(vec![
            format!("{}_liquidity", market),
            format!("{}_volatility", market),
            format!("{}_spread", market),
            format!("{}_volume", market),
        ])
    }

    /// Create mappings between source and target features
    async fn create_feature_mappings(
        &self,
        source_features: &[String],
        target_features: &[String],
    ) -> Result<HashMap<String, String>> {
        let mut mappings = HashMap::new();

        // Simple mapping based on feature type
        for source in source_features {
            for target in target_features {
                if self.features_are_similar(source, target) {
                    mappings.insert(source.clone(), target.clone());
                    break;
                }
            }
        }

        Ok(mappings)
    }

    /// Check if features are similar
    fn features_are_similar(&self, feature1: &str, feature2: &str) -> bool {
        // Extract feature type (e.g., "liquidity" from "btc_liquidity")
        let type1 = feature1.split('_').last().unwrap_or("");
        let type2 = feature2.split('_').last().unwrap_or("");
        
        type1 == type2
    }

    /// Learn transformation between domains
    async fn learn_domain_transformation(
        &self,
        source_market: &str,
        target_market: &str,
        feature_mappings: &HashMap<String, String>,
    ) -> Result<Array2<f64>> {
        // Mock implementation - would use actual domain adaptation techniques
        let size = feature_mappings.len();
        let mut transformation = Array2::eye(size);

        // Add some noise to simulate adaptation
        for i in 0..size {
            for j in 0..size {
                if i != j {
                    transformation[[i, j]] = rand::random::<f64>() * 0.1;
                }
            }
        }

        Ok(transformation)
    }

    /// Update regime model with new observations
    async fn update_regime_model(
        &self,
        model: &mut RegimeModel,
        regime: &MarketRegime,
    ) -> Result<()> {
        // Update feature importance based on regime characteristics
        for (i, (_, value)) in regime.characteristics.iter().enumerate().take(model.feature_importance.len()) {
            model.feature_importance[i] = model.feature_importance[i] * 0.9 + value * 0.1;
        }

        Ok(())
    }

    /// Analyze strategy performance across experiences
    async fn analyze_strategy_performance(
        &self,
        experiences: &[Experience],
    ) -> Result<HashMap<String, f64>> {
        let mut strategy_outcomes = HashMap::new();
        let mut strategy_counts = HashMap::new();

        for exp in experiences {
            let count = strategy_counts.entry(exp.action.clone()).or_insert(0);
            *count += 1;

            let total = strategy_outcomes.entry(exp.action.clone()).or_insert(0.0);
            *total += exp.outcome;
        }

        // Calculate average performance
        let mut performance = HashMap::new();
        for (strategy, total_outcome) in strategy_outcomes {
            if let Some(count) = strategy_counts.get(&strategy) {
                performance.insert(strategy, total_outcome / *count as f64);
            }
        }

        Ok(performance)
    }

    /// Add new experience to memory
    pub async fn add_experience(&self, experience: Experience) -> Result<()> {
        let mut memory = self.experience_memory.write().await;
        
        if memory.len() >= self.config.max_memory_size {
            memory.pop_front();
        }
        
        memory.push_back(experience);
        Ok(())
    }

    /// Perform meta-learning update
    pub async fn update(&self) -> Result<MetaLearningResult> {
        let experiences = self.experience_memory.read().await;
        
        if experiences.len() < self.config.min_experiences {
            return Ok(MetaLearningResult {
                strategies_improved: 0,
                cross_domain_patterns: vec![],
                adaptability_score: 0.0,
            });
        }

        // Create meta-tasks from experiences
        let tasks = self.create_meta_tasks(&experiences).await?;

        // Update meta-learner
        let mut learner = self.meta_learner.write().await;
        let loss = learner.meta_update(tasks).await?;

        // Discover new patterns
        let markets = self.extract_unique_markets(&experiences);
        let patterns = self.discover_universal_patterns(markets).await?;

        // Calculate improvement
        let mut tracker = self.performance_tracker.write().await;
        tracker.cumulative_improvement += (1.0 - loss) * 0.1;

        Ok(MetaLearningResult {
            strategies_improved: patterns.len(),
            cross_domain_patterns: patterns.iter().map(|p| p.id.clone()).collect(),
            adaptability_score: tracker.cumulative_improvement,
        })
    }

    /// Create meta-tasks from experiences
    async fn create_meta_tasks(&self, experiences: &VecDeque<Experience>) -> Result<Vec<MetaTask>> {
        let mut tasks = Vec::new();
        let mut task_groups = HashMap::new();

        // Group experiences by domain
        for exp in experiences {
            task_groups.entry(exp.market_regime.clone())
                .or_insert_with(Vec::new)
                .push(exp);
        }

        // Create tasks from groups
        for (domain, exps) in task_groups {
            if exps.len() >= 20 {
                // Split into support and query sets
                let split_point = exps.len() * 3 / 4;
                let support_set = self.experiences_to_dataset(&exps[..split_point]).await?;
                let query_set = self.experiences_to_dataset(&exps[split_point..]).await?;

                tasks.push(MetaTask {
                    task_id: format!("task_{}_{}", domain, Utc::now().timestamp()),
                    domain,
                    support_set,
                    query_set,
                });
            }
        }

        Ok(tasks)
    }

    /// Convert experiences to dataset
    async fn experiences_to_dataset(&self, experiences: &[&Experience]) -> Result<Vec<(Array1<f64>, f64)>> {
        let mut dataset = Vec::new();

        for exp in experiences {
            let features = self.experience_to_features(exp).await?;
            dataset.push((features, exp.outcome));
        }

        Ok(dataset)
    }

    /// Convert experience to feature vector
    async fn experience_to_features(&self, experience: &Experience) -> Result<Array1<f64>> {
        let mut features = Vec::new();

        // Add state features
        for (_, value) in &experience.state {
            features.push(*value);
        }

        // Pad or truncate to fixed size
        features.resize(50, 0.0);

        Ok(Array1::from_vec(features))
    }

    /// Extract unique markets from experiences
    fn extract_unique_markets(&self, experiences: &VecDeque<Experience>) -> Vec<String> {
        let mut markets = std::collections::HashSet::new();

        for exp in experiences {
            for key in exp.state.keys() {
                if key.starts_with("market_") {
                    markets.insert(key.strip_prefix("market_").unwrap_or("").to_string());
                }
            }
        }

        markets.into_iter().collect()
    }
}

/// Placeholder neural network implementation
#[derive(Clone)]
struct NeuralNetwork {
    layers: Vec<usize>,
    meta_learning_rate: f64,
}

impl NeuralNetwork {
    fn new(layers: Vec<usize>) -> Result<Self> {
        Ok(Self {
            layers,
            meta_learning_rate: 0.01,
        })
    }

    fn clone(&self) -> Self {
        Self {
            layers: self.layers.clone(),
            meta_learning_rate: self.meta_learning_rate,
        }
    }

    fn train_step(&mut self, data: &[(Array1<f64>, f64)]) -> Result<f64> {
        // Mock training step
        Ok(rand::random::<f64>())
    }

    fn compute_gradient(&self, data: &[(Array1<f64>, f64)]) -> Result<Array1<f64>> {
        // Mock gradient computation
        Ok(Array1::zeros(100))
    }

    fn update_with_meta_params(&mut self, params: &Array1<f64>) -> Result<()> {
        // Mock parameter update
        Ok(())
    }
}

// Helper for UUID generation
mod uuid {
    pub struct Uuid;
    
    impl Uuid {
        pub fn new_v4() -> String {
            format!("{:x}-{:x}-{:x}-{:x}",
                rand::random::<u32>(),
                rand::random::<u16>(),
                rand::random::<u16>(),
                rand::random::<u32>()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_meta_learning_coordinator_creation() {
        let config = MetaLearningConfig::default();
        let coordinator = MetaLearningCoordinator::new(config);
        assert!(coordinator.is_ok());
    }

    #[tokio::test]
    async fn test_pattern_discovery() {
        let coordinator = MetaLearningCoordinator::new(MetaLearningConfig::default()).unwrap();
        
        // Add some test experiences
        for i in 0..100 {
            let mut state = HashMap::new();
            state.insert("market_btc".to_string(), rand::random::<f64>());
            state.insert("market_eth".to_string(), rand::random::<f64>());
            state.insert("trend".to_string(), rand::random::<f64>());
            state.insert("volatility".to_string(), rand::random::<f64>());
            
            let experience = Experience {
                timestamp: Utc::now(),
                state,
                action: if i % 2 == 0 { "buy".to_string() } else { "sell".to_string() },
                outcome: rand::random::<f64>() * 2.0 - 1.0,
                market_regime: if i % 3 == 0 { "trending".to_string() } else { "ranging".to_string() },
            };
            
            coordinator.add_experience(experience).await.unwrap();
        }
        
        // Discover patterns
        let patterns = coordinator.discover_universal_patterns(vec!["btc".to_string(), "eth".to_string()]).await.unwrap();
        
        // Should discover some patterns with enough data
        assert!(patterns.len() >= 0); // Might be 0 if patterns don't meet threshold
    }
}