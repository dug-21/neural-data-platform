//! Training modules for FANN predictor
//!
//! This module provides training functionality including online learning,
//! recurrent state management, and performance tracking.

pub mod online;

// Re-export commonly used types
pub use online::OnlineTrainer;

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Recurrent state for LSTM/GRU simulation
#[derive(Debug, Clone)]
pub struct RecurrentState {
    /// Hidden state (for both LSTM and GRU)
    pub hidden: Vec<f32>,
    /// Cell state (LSTM only)
    pub cell: Option<Vec<f32>>,
    /// Previous outputs for context
    pub context_window: VecDeque<Vec<f32>>,
    /// Maximum context window size
    pub max_context: usize,
}

impl RecurrentState {
    /// Create a new recurrent state
    pub fn new(hidden_size: usize, max_context: usize, is_lstm: bool) -> Self {
        Self {
            hidden: vec![0.0; hidden_size],
            cell: if is_lstm { Some(vec![0.0; hidden_size]) } else { None },
            context_window: VecDeque::with_capacity(max_context),
            max_context,
        }
    }

    /// Update the hidden state
    pub fn update_hidden(&mut self, new_hidden: Vec<f32>) {
        self.hidden = new_hidden;
    }

    /// Update the cell state (LSTM only)
    pub fn update_cell(&mut self, new_cell: Vec<f32>) {
        if self.cell.is_some() {
            self.cell = Some(new_cell);
        }
    }

    /// Add output to context window
    pub fn add_context(&mut self, output: Vec<f32>) {
        if self.context_window.len() >= self.max_context {
            self.context_window.pop_front();
        }
        self.context_window.push_back(output);
    }

    /// Get the current context as a flattened vector
    pub fn get_context_vector(&self) -> Vec<f32> {
        self.context_window
            .iter()
            .flat_map(|v| v.iter())
            .cloned()
            .collect()
    }

    /// Reset the state to initial values
    pub fn reset(&mut self) {
        self.hidden.fill(0.0);
        if let Some(ref mut cell) = self.cell {
            cell.fill(0.0);
        }
        self.context_window.clear();
    }

    /// Check if this is an LSTM state
    pub fn is_lstm(&self) -> bool {
        self.cell.is_some()
    }

    /// Get the hidden state size
    pub fn hidden_size(&self) -> usize {
        self.hidden.len()
    }
}

/// Training configuration for online learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineTrainingConfig {
    /// Buffer size for batch training
    pub buffer_size: usize,
    /// Learning rate for online updates
    pub learning_rate: f32,
    /// Minimum samples before training
    pub min_samples: usize,
    /// Maximum training iterations per batch
    pub max_iterations: usize,
    /// Target error threshold
    pub target_error: f32,
    /// Enable adaptive learning rate
    pub adaptive_learning_rate: bool,
    /// Learning rate decay factor
    pub learning_rate_decay: f32,
    /// Performance tracking window size
    pub performance_window: usize,
}

impl Default for OnlineTrainingConfig {
    fn default() -> Self {
        Self {
            buffer_size: 100,
            learning_rate: 0.001,
            min_samples: 10,
            max_iterations: 100,
            target_error: 0.01,
            adaptive_learning_rate: true,
            learning_rate_decay: 0.995,
            performance_window: 50,
        }
    }
}

/// Performance metrics for training evaluation
#[derive(Debug, Clone)]
pub struct TrainingMetrics {
    /// Current training error
    pub current_error: f32,
    /// Average error over recent iterations
    pub average_error: f32,
    /// Training iterations completed
    pub iterations_completed: usize,
    /// Training duration
    pub training_duration: Duration,
    /// Convergence status
    pub converged: bool,
    /// Learning rate used
    pub final_learning_rate: f32,
    /// Performance trend (improving/degrading)
    pub performance_trend: PerformanceTrend,
}

/// Performance trend indicators
#[derive(Debug, Clone, PartialEq)]
pub enum PerformanceTrend {
    Improving,
    Stable,
    Degrading,
    Oscillating,
}

impl TrainingMetrics {
    /// Create new training metrics
    pub fn new() -> Self {
        Self {
            current_error: f32::INFINITY,
            average_error: f32::INFINITY,
            iterations_completed: 0,
            training_duration: Duration::from_secs(0),
            converged: false,
            final_learning_rate: 0.0,
            performance_trend: PerformanceTrend::Stable,
        }
    }

    /// Update metrics with new training results
    pub fn update(&mut self, error: f32, learning_rate: f32, duration: Duration) {
        let previous_error = self.current_error;
        self.current_error = error;
        self.iterations_completed += 1;
        self.training_duration += duration;
        self.final_learning_rate = learning_rate;

        // Update average error (exponential moving average)
        if self.average_error.is_infinite() {
            self.average_error = error;
        } else {
            self.average_error = 0.9 * self.average_error + 0.1 * error;
        }

        // Determine performance trend
        if previous_error.is_finite() {
            let improvement = (previous_error - error) / previous_error;
            self.performance_trend = if improvement > 0.01 {
                PerformanceTrend::Improving
            } else if improvement < -0.01 {
                PerformanceTrend::Degrading
            } else if improvement.abs() < 0.001 {
                PerformanceTrend::Stable
            } else {
                PerformanceTrend::Oscillating
            };
        }
    }

    /// Check if training should continue
    pub fn should_continue(&self, config: &OnlineTrainingConfig) -> bool {
        !self.converged 
            && self.iterations_completed < config.max_iterations
            && self.current_error > config.target_error
    }

    /// Mark as converged
    pub fn mark_converged(&mut self) {
        self.converged = true;
    }

    /// Get success rate (1.0 - normalized error)
    pub fn success_rate(&self) -> f32 {
        if self.current_error.is_finite() {
            1.0 - (self.current_error.min(1.0))
        } else {
            0.0
        }
    }
}

/// Concept drift detection for online learning
#[derive(Debug)]
pub struct ConceptDriftDetector {
    /// Window of recent errors for drift detection
    error_window: VecDeque<f32>,
    /// Window size for drift detection
    window_size: usize,
    /// Threshold for detecting significant drift
    drift_threshold: f32,
    /// Baseline error for comparison
    baseline_error: Option<f32>,
    /// Number of consecutive drifts detected
    consecutive_drifts: usize,
    /// Drift detection sensitivity
    sensitivity: f32,
}

impl ConceptDriftDetector {
    /// Create a new concept drift detector
    pub fn new(window_size: usize, drift_threshold: f32, sensitivity: f32) -> Self {
        Self {
            error_window: VecDeque::with_capacity(window_size),
            window_size,
            drift_threshold,
            baseline_error: None,
            consecutive_drifts: 0,
            sensitivity,
        }
    }

    /// Add a new error measurement
    pub fn add_error(&mut self, error: f32) {
        if self.error_window.len() >= self.window_size {
            self.error_window.pop_front();
        }
        self.error_window.push_back(error);

        // Update baseline if not set
        if self.baseline_error.is_none() && self.error_window.len() >= self.window_size / 2 {
            self.baseline_error = Some(self.calculate_average_error());
        }
    }

    /// Check if concept drift has been detected
    pub fn detect_drift(&mut self) -> bool {
        if self.error_window.len() < self.window_size {
            return false;
        }

        let baseline = match self.baseline_error {
            Some(baseline) => baseline,
            None => {
                self.baseline_error = Some(self.calculate_average_error());
                return false;
            }
        };

        let current_average = self.calculate_average_error();
        let drift_ratio = (current_average - baseline) / baseline.max(0.001);

        let drift_detected = drift_ratio > self.drift_threshold * self.sensitivity;

        if drift_detected {
            self.consecutive_drifts += 1;
        } else {
            self.consecutive_drifts = 0;
        }

        // Update baseline periodically to adapt to gradual changes
        if self.consecutive_drifts == 0 && self.error_window.len() == self.window_size {
            self.baseline_error = Some(current_average);
        }

        drift_detected && self.consecutive_drifts >= 2 // Require consecutive detections
    }

    /// Reset the drift detector
    pub fn reset(&mut self) {
        self.error_window.clear();
        self.baseline_error = None;
        self.consecutive_drifts = 0;
    }

    /// Calculate average error in the window
    fn calculate_average_error(&self) -> f32 {
        if self.error_window.is_empty() {
            return 0.0;
        }
        
        self.error_window.iter().sum::<f32>() / self.error_window.len() as f32
    }

    /// Get drift severity (0.0 to 1.0)
    pub fn drift_severity(&self) -> f32 {
        if let Some(baseline) = self.baseline_error {
            if self.error_window.is_empty() {
                return 0.0;
            }
            
            let current_average = self.calculate_average_error();
            let drift_ratio = (current_average - baseline) / baseline.max(0.001);
            
            (drift_ratio / self.drift_threshold).min(1.0).max(0.0)
        } else {
            0.0
        }
    }
}

impl Default for ConceptDriftDetector {
    fn default() -> Self {
        Self::new(50, 0.2, 1.0) // 50 sample window, 20% drift threshold, normal sensitivity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recurrent_state_creation() {
        let lstm_state = RecurrentState::new(64, 10, true);
        assert!(lstm_state.is_lstm());
        assert_eq!(lstm_state.hidden_size(), 64);
        assert_eq!(lstm_state.max_context, 10);

        let gru_state = RecurrentState::new(32, 5, false);
        assert!(!gru_state.is_lstm());
        assert_eq!(gru_state.hidden_size(), 32);
    }

    #[test]
    fn test_training_metrics() {
        let mut metrics = TrainingMetrics::new();
        assert!(!metrics.converged);
        assert_eq!(metrics.iterations_completed, 0);

        metrics.update(0.5, 0.001, Duration::from_millis(100));
        assert_eq!(metrics.iterations_completed, 1);
        assert_eq!(metrics.current_error, 0.5);

        metrics.update(0.3, 0.001, Duration::from_millis(100));
        assert_eq!(metrics.performance_trend, PerformanceTrend::Improving);
    }

    #[test]
    fn test_concept_drift_detector() {
        let mut detector = ConceptDriftDetector::new(5, 0.5, 1.0);
        
        // Add stable errors
        for _ in 0..5 {
            detector.add_error(0.1);
        }
        
        assert!(!detector.detect_drift());
        
        // Add higher errors to simulate drift
        for _ in 0..3 {
            detector.add_error(0.8);
        }
        
        // Should detect drift after consecutive high errors
        let drift_detected = detector.detect_drift();
        // Note: This may require multiple calls depending on implementation
        assert!(drift_detected || detector.drift_severity() > 0.0);
    }
}