//! DAA Coordinator Configuration
//!
//! Configuration types and defaults for the DAA coordination system.

use std::collections::HashMap;

/// Configuration for DAA coordination
#[derive(Debug, Clone)]
pub struct DaaConfig {
    /// Enable autonomous decision making
    pub enabled: bool,
    /// Minimum confidence for autonomous trades
    pub min_confidence: f64,
    /// Risk limit per trade
    pub max_risk_per_trade: f64,
    /// Maximum concurrent positions
    pub max_positions: usize,
    /// Neural model weights for decisions
    pub model_weights: HashMap<String, f64>,
    /// Consensus threshold for multi-agent decisions
    pub consensus_threshold: f64,
    /// Enable real-time adaptation
    pub enable_adaptation: bool,
}

impl Default for DaaConfig {
    fn default() -> Self {
        let mut model_weights = HashMap::new();
        model_weights.insert("NHITS".to_string(), 1.2);
        model_weights.insert("TCN".to_string(), 1.1);
        model_weights.insert("DeepAR".to_string(), 1.3);
        model_weights.insert("Transformer".to_string(), 1.4);
        model_weights.insert("MLP".to_string(), 0.8);

        Self {
            enabled: true,
            min_confidence: 0.75,
            max_risk_per_trade: 0.02,
            max_positions: 5,
            model_weights,
            consensus_threshold: 0.7,
            enable_adaptation: true,
        }
    }
}

/// Simplified confidence breakdown for DAA decisions
#[derive(Debug, Clone, Default)]
pub struct ConfidenceBreakdown {
    pub base_confidence: f64,
    pub ensemble_agreement: f64,
    pub historical_accuracy: f64,
    pub combined_confidence: f64,
}

/// Simplified retraining metrics
#[derive(Debug, Clone)]
pub struct RetrainingMetrics {
    pub urgency_score: f64,
    pub accuracy: f64,
    pub should_retrain: bool,
}

/// Performance metrics for DAA decisions
#[derive(Debug, Default, Clone)]
pub struct PerformanceMetrics {
    pub total_decisions: u64,
    pub profitable_decisions: u64,
    pub total_pnl: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub win_rate: f64,
    pub avg_confidence: f64,
    pub model_accuracy: HashMap<String, f64>,
}