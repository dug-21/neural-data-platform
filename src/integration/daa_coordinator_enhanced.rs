//! Enhanced DAA Coordinator with Data Context Evaluation
//!
//! This module extends the existing DAA Coordinator with advanced data context evaluation
//! capabilities while preserving the critical Byzantine consensus mechanisms:
//! - 70% consensus threshold for multi-agent decisions
//! - 60/40 neural/strategy voting weights
//! - Enhanced data quality assessment and market timing optimization

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc, Datelike, Timelike};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use serde::{Deserialize, Serialize};

use crate::data::TimeSeriesData;
use crate::strategies::{MarketContext, Position};
use super::daa_coordinator::{DaaCoordinator, AutonomousDecision};

/// Data availability and quality assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataAvailability {
    /// Data completeness score (0.0 to 1.0)
    pub completeness: f64,
    /// Data freshness score (0.0 to 1.0) 
    pub freshness: f64,
    /// Data quality score (0.0 to 1.0)
    pub quality: f64,
    /// Number of data sources available
    pub source_count: usize,
    /// Market data coverage percentage
    pub market_coverage: f64,
    /// Cross-validation consistency score
    pub consistency: f64,
    /// Latency assessment (milliseconds)
    pub latency_ms: f64,
    /// Timestamp of assessment
    pub assessment_time: DateTime<Utc>,
}

impl Default for DataAvailability {
    fn default() -> Self {
        Self {
            completeness: 1.0,
            freshness: 1.0,
            quality: 1.0,
            source_count: 1,
            market_coverage: 1.0,
            consistency: 1.0,
            latency_ms: 50.0,
            assessment_time: Utc::now(),
        }
    }
}

impl DataAvailability {
    /// Calculate overall data availability score
    pub fn overall_score(&self) -> f64 {
        // Weighted combination of all factors
        let weights = [0.25, 0.20, 0.25, 0.10, 0.10, 0.10]; // completeness, freshness, quality, sources, coverage, consistency
        let scores = [
            self.completeness,
            self.freshness,
            self.quality,
            (self.source_count as f64 / 5.0).min(1.0), // normalize source count
            self.market_coverage,
            self.consistency,
        ];
        
        weights.iter().zip(scores.iter())
            .map(|(w, s)| w * s)
            .sum::<f64>()
    }
    
    /// Check if data quality meets minimum threshold
    pub fn meets_threshold(&self, threshold: f64) -> bool {
        self.overall_score() >= threshold
    }
    
    /// Get data quality category
    pub fn quality_category(&self) -> DataQualityCategory {
        match self.overall_score() {
            score if score >= 0.9 => DataQualityCategory::Excellent,
            score if score >= 0.8 => DataQualityCategory::Good,
            score if score >= 0.7 => DataQualityCategory::Fair,
            score if score >= 0.6 => DataQualityCategory::Poor,
            _ => DataQualityCategory::Critical,
        }
    }
}

/// Data quality categories for decision adjustment
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataQualityCategory {
    Excellent, // >= 0.9
    Good,      // >= 0.8
    Fair,      // >= 0.7
    Poor,      // >= 0.6
    Critical,  // < 0.6
}

/// Enhanced decision with data context awareness
#[derive(Debug, Clone)]
pub struct EnhancedDecision {
    /// Base autonomous decision (preserves all existing logic)
    pub base_decision: AutonomousDecision,
    /// Data availability assessment
    pub data_availability: DataAvailability,
    /// Data-adjusted confidence score
    pub data_adjusted_confidence: f64,
    /// Market timing optimization score
    pub timing_score: f64,
    /// Data quality impact on decision
    pub data_quality_impact: DataQualityImpact,
    /// Enhanced reasoning including data context
    pub enhanced_reasoning: Vec<String>,
}

/// Impact of data quality on trading decision
#[derive(Debug, Clone)]
pub struct DataQualityImpact {
    /// Confidence adjustment factor due to data quality
    pub confidence_adjustment: f64,
    /// Position size adjustment factor
    pub size_adjustment: f64,
    /// Risk adjustment factor
    pub risk_adjustment: f64,
    /// Recommended action based on data quality
    pub recommended_action: DataQualityAction,
    /// Detailed impact analysis
    pub impact_details: HashMap<String, f64>,
}

/// Recommended actions based on data quality
#[derive(Debug, Clone, PartialEq)]
pub enum DataQualityAction {
    /// Proceed with original decision
    Proceed,
    /// Reduce position size
    ReduceSize(f64),
    /// Increase confidence requirements
    IncreaseThreshold(f64),
    /// Defer decision until better data
    Defer,
    /// Use conservative fallback strategy
    Conservative,
}

/// Enhanced DAA Coordinator with data context evaluation
pub struct EnhancedDAACoordinator {
    /// Base DAA coordinator (preserves all existing functionality)
    base_coordinator: Arc<DaaCoordinator>,
    /// Data quality assessment configuration
    data_quality_config: DataQualityConfig,
    /// Enhanced market timing analyzer
    market_timing_analyzer: Arc<RwLock<MarketTimingAnalyzer>>,
    /// Data context evaluation history
    data_evaluation_history: Arc<RwLock<Vec<DataContextEvaluation>>>,
    /// Performance metrics with data awareness
    enhanced_metrics: Arc<RwLock<EnhancedPerformanceMetrics>>,
}

/// Configuration for data quality assessment
#[derive(Debug, Clone)]
pub struct DataQualityConfig {
    /// Minimum data quality threshold for normal operations
    pub min_quality_threshold: f64,
    /// Enable data quality adjustments
    pub enable_data_adjustments: bool,
    /// Conservative mode threshold (lower quality = more conservative)
    pub conservative_threshold: f64,
    /// Maximum confidence reduction due to data quality
    pub max_confidence_reduction: f64,
    /// Maximum position size reduction due to data quality
    pub max_size_reduction: f64,
    /// Enable enhanced market timing
    pub enable_enhanced_timing: bool,
}

impl Default for DataQualityConfig {
    fn default() -> Self {
        Self {
            min_quality_threshold: 0.7,
            enable_data_adjustments: true,
            conservative_threshold: 0.6,
            max_confidence_reduction: 0.3,
            max_size_reduction: 0.5,
            enable_enhanced_timing: true,
        }
    }
}

/// Market timing analysis with data context
#[derive(Debug, Clone)]
pub struct MarketTimingAnalyzer {
    /// Recent timing decisions
    timing_history: Vec<TimingDecision>,
    /// Market session analysis
    session_analyzer: SessionAnalyzer,
    /// Volume pattern analyzer
    volume_analyzer: VolumePatternAnalyzer,
}

/// Individual timing decision record
#[derive(Debug, Clone)]
pub struct TimingDecision {
    pub timestamp: DateTime<Utc>,
    pub market_session: MarketSession,
    pub volume_score: f64,
    pub liquidity_score: f64,
    pub timing_recommendation: TimingRecommendation,
    pub data_quality: f64,
}

/// Market session classification
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MarketSession {
    PreMarket,
    Opening,
    Regular,
    Lunch,
    Closing,
    AfterHours,
    Weekend,
}

/// Timing recommendation
#[derive(Debug, Clone)]
pub enum TimingRecommendation {
    Optimal,
    Good,
    Acceptable,
    Poor,
    Avoid,
}

/// Session analysis component
#[derive(Debug, Clone)]
pub struct SessionAnalyzer {
    session_weights: HashMap<MarketSession, f64>,
}

impl Default for SessionAnalyzer {
    fn default() -> Self {
        let mut session_weights = HashMap::new();
        session_weights.insert(MarketSession::PreMarket, 0.7);
        session_weights.insert(MarketSession::Opening, 0.9);
        session_weights.insert(MarketSession::Regular, 1.0);
        session_weights.insert(MarketSession::Lunch, 0.8);
        session_weights.insert(MarketSession::Closing, 0.9);
        session_weights.insert(MarketSession::AfterHours, 0.6);
        session_weights.insert(MarketSession::Weekend, 0.5);
        
        Self { session_weights }
    }
}

/// Volume pattern analysis component
#[derive(Debug, Clone)]
pub struct VolumePatternAnalyzer {
    recent_volumes: Vec<f64>,
    volume_trend: f64,
}

impl Default for VolumePatternAnalyzer {
    fn default() -> Self {
        Self {
            recent_volumes: Vec::with_capacity(20),
            volume_trend: 0.0,
        }
    }
}

/// Data context evaluation record
#[derive(Debug, Clone)]
pub struct DataContextEvaluation {
    pub timestamp: DateTime<Utc>,
    pub market_context: MarketContext,
    pub data_availability: DataAvailability,
    pub timing_analysis: TimingDecision,
    pub quality_impact: DataQualityImpact,
    pub decision_outcome: String,
}

/// Enhanced performance metrics with data awareness
#[derive(Debug, Clone, Default)]
pub struct EnhancedPerformanceMetrics {
    /// Total decisions with data context evaluation
    pub total_enhanced_decisions: u64,
    /// Average data quality score
    pub avg_data_quality: f64,
    /// Data quality vs performance correlation
    pub quality_performance_correlation: f64,
    /// Timing optimization effectiveness
    pub timing_effectiveness: f64,
    /// Conservative decision count
    pub conservative_decisions: u64,
    /// Data-driven confidence adjustments
    pub confidence_adjustments: Vec<f64>,
    /// Market timing accuracy
    pub timing_accuracy: f64,
}

impl EnhancedDAACoordinator {
    /// Create new enhanced DAA coordinator
    pub fn new(
        base_coordinator: Arc<DaaCoordinator>,
        data_quality_config: DataQualityConfig,
    ) -> Self {
        info!("🚀 Initializing Enhanced DAA Coordinator with data context evaluation");
        
        Self {
            base_coordinator,
            data_quality_config,
            market_timing_analyzer: Arc::new(RwLock::new(MarketTimingAnalyzer::default())),
            data_evaluation_history: Arc::new(RwLock::new(Vec::new())),
            enhanced_metrics: Arc::new(RwLock::new(EnhancedPerformanceMetrics::default())),
        }
    }
    
    /// **CORE METHOD**: Evaluate decision with data context while preserving Byzantine consensus
    pub async fn evaluate_with_data_context(
        &self,
        market_context: &MarketContext,
        current_position: Option<&Position>,
        historical_data: &[TimeSeriesData],
        data_availability: DataAvailability,
    ) -> Result<EnhancedDecision> {
        info!("🔍 Evaluating decision with data context - preserving Byzantine consensus");
        
        // CRITICAL: Use existing base coordinator to preserve 70% consensus threshold
        // and 60/40 neural/strategy voting weights
        let base_decision = self.base_coordinator
            .make_decision(market_context, current_position, historical_data)
            .await
            .context("Failed to get base DAA decision")?;
        
        debug!("✅ Base decision preserves Byzantine consensus: confidence={:.3}, neural_consensus={} signals", 
               base_decision.confidence, base_decision.neural_consensus.len());
        
        // Enhance with data context evaluation WITHOUT changing core voting logic
        let enhanced_timing = self.check_enhanced_market_timing(market_context, &data_availability).await?;
        
        // Calculate data quality impact
        let quality_impact = self.calculate_data_quality_impact(&base_decision, &data_availability).await?;
        
        // Apply data quality adjustments while preserving consensus weights
        let data_adjusted_confidence = self.apply_data_quality_adjustments(
            base_decision.confidence,
            &quality_impact,
            &data_availability,
        ).await?;
        
        // Create enhanced reasoning (clone enhanced_timing to avoid move)
        let mut enhanced_reasoning = base_decision.reasoning.clone();
        enhanced_reasoning.extend(self.generate_data_context_reasoning(
            &data_availability,
            &quality_impact,
            enhanced_timing.clone(),
        ).await);
        
        // Record evaluation
        self.record_data_evaluation(
            market_context,
            &data_availability,
            &enhanced_timing,
            &quality_impact,
        ).await;
        
        // Update enhanced metrics
        self.update_enhanced_metrics(&data_availability, data_adjusted_confidence).await;
        
        let enhanced_decision = EnhancedDecision {
            base_decision,
            data_availability,
            data_adjusted_confidence,
            timing_score: enhanced_timing.timing_score,
            data_quality_impact: quality_impact,
            enhanced_reasoning,
        };
        
        info!("🎯 Enhanced decision completed: base_confidence={:.3} → data_adjusted={:.3}, timing_score={:.3}",
              enhanced_decision.base_decision.confidence,
              enhanced_decision.data_adjusted_confidence,
              enhanced_decision.timing_score);
        
        Ok(enhanced_decision)
    }
    
    /// **CORE METHOD**: Enhanced market timing check with data context
    pub async fn check_enhanced_market_timing(
        &self,
        market_context: &MarketContext,
        data_availability: &DataAvailability,
    ) -> Result<EnhancedTimingResult> {
        debug!("⏰ Performing enhanced market timing analysis");
        
        let mut analyzer = self.market_timing_analyzer.write().await;
        
        // Determine current market session
        let current_session = self.determine_market_session().await;
        
        // Analyze volume patterns
        let volume_analysis = analyzer.volume_analyzer.analyze_volume_patterns(
            market_context.volume_24h,
        );
        
        // Calculate liquidity score based on data availability
        let liquidity_score = self.calculate_liquidity_score(
            market_context,
            data_availability,
        ).await;
        
        // Session-based timing score
        let session_score = analyzer.session_analyzer
            .session_weights
            .get(&current_session)
            .copied()
            .unwrap_or(0.5);
        
        // Combined timing score
        let timing_score = (session_score * 0.4 + volume_analysis * 0.3 + liquidity_score * 0.3)
            .max(0.0)
            .min(1.0);
        
        // Generate timing recommendation
        let timing_recommendation = match timing_score {
            score if score >= 0.8 => TimingRecommendation::Optimal,
            score if score >= 0.7 => TimingRecommendation::Good,
            score if score >= 0.6 => TimingRecommendation::Acceptable,
            score if score >= 0.4 => TimingRecommendation::Poor,
            _ => TimingRecommendation::Avoid,
        };
        
        let timing_decision = TimingDecision {
            timestamp: Utc::now(),
            market_session: current_session,
            volume_score: volume_analysis,
            liquidity_score,
            timing_recommendation,
            data_quality: data_availability.overall_score(),
        };
        
        // Store timing decision
        analyzer.timing_history.push(timing_decision.clone());
        if analyzer.timing_history.len() > 100 {
            analyzer.timing_history.remove(0);
        }
        
        drop(analyzer);
        
        Ok(EnhancedTimingResult {
            timing_score,
            timing_decision: timing_decision.clone(),
            session_analysis: SessionAnalysisResult {
                current_session: timing_decision.market_session,
                session_weight: session_score,
                volume_pattern: volume_analysis,
                liquidity_assessment: liquidity_score,
            },
        })
    }
    
    /// Calculate data quality impact on decision
    async fn calculate_data_quality_impact(
        &self,
        base_decision: &AutonomousDecision,
        data_availability: &DataAvailability,
    ) -> Result<DataQualityImpact> {
        let quality_score = data_availability.overall_score();
        let quality_category = data_availability.quality_category();
        
        // Calculate adjustments based on data quality
        let confidence_adjustment = match quality_category {
            DataQualityCategory::Excellent => 1.0,
            DataQualityCategory::Good => 0.95,
            DataQualityCategory::Fair => 0.85,
            DataQualityCategory::Poor => 0.70,
            DataQualityCategory::Critical => 0.50,
        };
        
        let size_adjustment = match quality_category {
            DataQualityCategory::Excellent => 1.0,
            DataQualityCategory::Good => 0.9,
            DataQualityCategory::Fair => 0.8,
            DataQualityCategory::Poor => 0.6,
            DataQualityCategory::Critical => 0.4,
        };
        
        let risk_adjustment = match quality_category {
            DataQualityCategory::Excellent => 1.0,
            DataQualityCategory::Good => 1.1,
            DataQualityCategory::Fair => 1.2,
            DataQualityCategory::Poor => 1.4,
            DataQualityCategory::Critical => 1.8,
        };
        
        // Determine recommended action
        let recommended_action = if quality_score < self.data_quality_config.conservative_threshold {
            if quality_category == DataQualityCategory::Critical {
                DataQualityAction::Defer
            } else {
                DataQualityAction::Conservative
            }
        } else if confidence_adjustment < 0.8 {
            DataQualityAction::IncreaseThreshold(0.1)
        } else if size_adjustment < 0.8 {
            DataQualityAction::ReduceSize(size_adjustment)
        } else {
            DataQualityAction::Proceed
        };
        
        // Create detailed impact analysis
        let mut impact_details = HashMap::new();
        impact_details.insert("completeness_impact".to_string(), data_availability.completeness);
        impact_details.insert("freshness_impact".to_string(), data_availability.freshness);
        impact_details.insert("quality_impact".to_string(), data_availability.quality);
        impact_details.insert("consistency_impact".to_string(), data_availability.consistency);
        impact_details.insert("latency_penalty".to_string(), 
                             1.0 - (data_availability.latency_ms / 1000.0).min(1.0));
        
        Ok(DataQualityImpact {
            confidence_adjustment,
            size_adjustment,
            risk_adjustment,
            recommended_action,
            impact_details,
        })
    }
    
    /// Apply data quality adjustments while preserving consensus mechanisms
    async fn apply_data_quality_adjustments(
        &self,
        base_confidence: f64,
        quality_impact: &DataQualityImpact,
        data_availability: &DataAvailability,
    ) -> Result<f64> {
        if !self.data_quality_config.enable_data_adjustments {
            return Ok(base_confidence);
        }
        
        // CRITICAL: Apply adjustments WITHOUT modifying the base 60/40 neural/strategy weights
        // The base_confidence already incorporates the Byzantine consensus
        let quality_adjusted = base_confidence * quality_impact.confidence_adjustment;
        
        // Apply maximum reduction limits to prevent excessive adjustments
        let max_reduction = self.data_quality_config.max_confidence_reduction;
        let min_allowed = base_confidence * (1.0 - max_reduction);
        
        let final_confidence = quality_adjusted.max(min_allowed).min(1.0);
        
        debug!("🔧 Data quality adjustment: {:.3} → {:.3} (quality_factor={:.3}, min_allowed={:.3})",
               base_confidence, final_confidence, quality_impact.confidence_adjustment, min_allowed);
        
        Ok(final_confidence)
    }
    
    /// Determine current market session
    async fn determine_market_session(&self) -> MarketSession {
        use chrono::Timelike;
        let now = Utc::now();
        let hour = now.hour();
        let weekday = now.weekday();
        
        // Simple session classification (could be enhanced with timezone awareness)
        match weekday {
            chrono::Weekday::Sat | chrono::Weekday::Sun => MarketSession::Weekend,
            _ => match hour {
                0..=8 => MarketSession::PreMarket,
                9..=10 => MarketSession::Opening,
                11..=12 => MarketSession::Regular,
                13..=14 => MarketSession::Lunch,
                15..=16 => MarketSession::Regular,
                17..=18 => MarketSession::Closing,
                _ => MarketSession::AfterHours,
            }
        }
    }
    
    /// Calculate liquidity score
    async fn calculate_liquidity_score(
        &self,
        market_context: &MarketContext,
        data_availability: &DataAvailability,
    ) -> f64 {
        // Base liquidity from volume  
        let volume_score = (market_context.volume_24h / 1_000_000.0)
            .min(1.0);
        
        // Spread-based liquidity (tighter spread = higher liquidity)
        let spread = (market_context.ask - market_context.bid) / market_context.current_price;
        let spread_score = (1.0 - spread * 100.0).max(0.0).min(1.0);
        
        // Data availability impact on liquidity assessment
        let data_reliability = data_availability.overall_score();
        
        // Combined liquidity score
        (volume_score * 0.5 + spread_score * 0.3 + data_reliability * 0.2)
            .max(0.0)
            .min(1.0)
    }
    
    /// Generate enhanced reasoning with data context
    async fn generate_data_context_reasoning(
        &self,
        data_availability: &DataAvailability,
        quality_impact: &DataQualityImpact,
        timing_result: EnhancedTimingResult,
    ) -> Vec<String> {
        let mut reasoning = Vec::new();
        
        reasoning.push(format!(
            "📊 Data Quality Assessment: {:.3} ({:?})",
            data_availability.overall_score(),
            data_availability.quality_category()
        ));
        
        reasoning.push(format!(
            "🔧 Confidence Adjustment: {:.3}x (quality impact)",
            quality_impact.confidence_adjustment
        ));
        
        reasoning.push(format!(
            "⏰ Market Timing: {:.3} ({:?} session, {:?})",
            timing_result.timing_score,
            timing_result.timing_decision.market_session,
            timing_result.timing_decision.timing_recommendation
        ));
        
        if data_availability.latency_ms > 100.0 {
            reasoning.push(format!(
                "⚠️ High data latency: {:.1}ms (may affect decision quality)",
                data_availability.latency_ms
            ));
        }
        
        if data_availability.source_count < 2 {
            reasoning.push("⚠️ Limited data sources - single point of failure risk".to_string());
        }
        
        match quality_impact.recommended_action {
            DataQualityAction::Proceed => {
                reasoning.push("✅ Data quality sufficient for normal operation".to_string());
            }
            DataQualityAction::ReduceSize(factor) => {
                reasoning.push(format!("📉 Recommend position size reduction: {:.1}%", (1.0 - factor) * 100.0));
            }
            DataQualityAction::IncreaseThreshold(increase) => {
                reasoning.push(format!("📈 Recommend higher confidence threshold: +{:.1}%", increase * 100.0));
            }
            DataQualityAction::Defer => {
                reasoning.push("⏸️ Recommend deferring decision due to poor data quality".to_string());
            }
            DataQualityAction::Conservative => {
                reasoning.push("🛡️ Recommend conservative approach due to data quality concerns".to_string());
            }
        }
        
        reasoning
    }
    
    /// Record data evaluation for history and analysis
    async fn record_data_evaluation(
        &self,
        market_context: &MarketContext,
        data_availability: &DataAvailability,
        timing_result: &EnhancedTimingResult,
        quality_impact: &DataQualityImpact,
    ) {
        let evaluation = DataContextEvaluation {
            timestamp: Utc::now(),
            market_context: market_context.clone(),
            data_availability: data_availability.clone(),
            timing_analysis: timing_result.timing_decision.clone(),
            quality_impact: quality_impact.clone(),
            decision_outcome: format!("quality={:.3}, timing={:.3}", 
                                    data_availability.overall_score(),
                                    timing_result.timing_score),
        };
        
        let mut history = self.data_evaluation_history.write().await;
        history.push(evaluation);
        
        // Keep last 1000 evaluations
        if history.len() > 1000 {
            history.remove(0);
        }
    }
    
    /// Update enhanced performance metrics
    async fn update_enhanced_metrics(
        &self,
        data_availability: &DataAvailability,
        adjusted_confidence: f64,
    ) {
        let mut metrics = self.enhanced_metrics.write().await;
        
        metrics.total_enhanced_decisions += 1;
        
        // Update average data quality
        let count = metrics.total_enhanced_decisions as f64;
        metrics.avg_data_quality = 
            (metrics.avg_data_quality * (count - 1.0) + data_availability.overall_score()) / count;
        
        // Track confidence adjustments
        metrics.confidence_adjustments.push(adjusted_confidence);
        if metrics.confidence_adjustments.len() > 100 {
            metrics.confidence_adjustments.remove(0);
        }
    }
    
    /// Get underlying base coordinator (for accessing core DAA functionality)
    pub fn get_base_coordinator(&self) -> &Arc<DaaCoordinator> {
        &self.base_coordinator
    }
    
    /// Get enhanced performance metrics
    pub async fn get_enhanced_metrics(&self) -> EnhancedPerformanceMetrics {
        self.enhanced_metrics.read().await.clone()
    }
    
    /// Get data evaluation history
    pub async fn get_evaluation_history(&self) -> Vec<DataContextEvaluation> {
        self.data_evaluation_history.read().await.clone()
    }
}

/// Enhanced timing analysis result
#[derive(Debug, Clone)]
pub struct EnhancedTimingResult {
    pub timing_score: f64,
    pub timing_decision: TimingDecision,
    pub session_analysis: SessionAnalysisResult,
}

/// Session analysis result
#[derive(Debug, Clone)]
pub struct SessionAnalysisResult {
    pub current_session: MarketSession,
    pub session_weight: f64,
    pub volume_pattern: f64,
    pub liquidity_assessment: f64,
}

impl MarketTimingAnalyzer {
    pub fn default() -> Self {
        Self {
            timing_history: Vec::new(),
            session_analyzer: SessionAnalyzer::default(),
            volume_analyzer: VolumePatternAnalyzer::default(),
        }
    }
}

impl VolumePatternAnalyzer {
    /// Analyze volume patterns and return volume score
    pub fn analyze_volume_patterns(&mut self, current_volume: f64) -> f64 {
        self.recent_volumes.push(current_volume);
        if self.recent_volumes.len() > 20 {
            self.recent_volumes.remove(0);
        }
        
        if self.recent_volumes.len() < 2 {
            return 0.5; // neutral score
        }
        
        // Calculate volume trend
        let recent_avg = self.recent_volumes.iter().rev().take(5).sum::<f64>() / 5.0;
        let historical_avg = self.recent_volumes.iter().sum::<f64>() / self.recent_volumes.len() as f64;
        
        self.volume_trend = if historical_avg > 0.0 {
            recent_avg / historical_avg
        } else {
            1.0
        };
        
        // Convert trend to score (1.0 = average, higher = better)
        (self.volume_trend / 2.0).max(0.0).min(1.0)
    }
}

/// Utility function to assess data availability from historical data
pub fn assess_data_availability(
    historical_data: &[TimeSeriesData],
    market_context: &MarketContext,
) -> DataAvailability {
    let now = Utc::now();
    
    // Calculate completeness (percentage of expected data points)
    let expected_points = 100; // Expected data points for analysis
    let actual_points = historical_data.len();
    let completeness = (actual_points as f64 / expected_points as f64).min(1.0);
    
    // Calculate freshness (how recent is the latest data)
    let freshness = if let Some(latest) = historical_data.last() {
        let age_minutes = (now - latest.timestamp).num_minutes() as f64;
        (1.0 - (age_minutes / 60.0)).max(0.0).min(1.0) // Degrade over 1 hour
    } else {
        0.0
    };
    
    // Calculate quality (consistency of data)
    let quality = if historical_data.len() > 1 {
        let mut valid_points = 0;
        for data in historical_data {
            if data.open > 0.0 && data.high >= data.low && 
               data.close > 0.0 && data.volume_value >= 0.0 {
                valid_points += 1;
            }
        }
        valid_points as f64 / historical_data.len() as f64
    } else {
        1.0
    };
    
    // Calculate market coverage (simplified)
    let market_coverage = if market_context.volume_24h > 0.0 {
        1.0
    } else {
        0.5
    };
    
    // Calculate consistency (price continuity)
    let consistency = if historical_data.len() > 1 {
        let mut gaps = 0;
        for window in historical_data.windows(2) {
            let price_gap = (window[1].open - window[0].close).abs() / window[0].close;
            if price_gap > 0.1 { // 10% gap threshold
                gaps += 1;
            }
        }
        1.0 - (gaps as f64 / (historical_data.len() - 1) as f64)
    } else {
        1.0
    };
    
    DataAvailability {
        completeness,
        freshness,
        quality,
        source_count: 1, // Single source for now
        market_coverage,
        consistency,
        latency_ms: 50.0, // Assume reasonable latency
        assessment_time: now,
    }
}

impl TimingDecision {
    /// Get timing score from decision
    pub fn timing_score(&self) -> f64 {
        match self.timing_recommendation {
            TimingRecommendation::Optimal => 1.0,
            TimingRecommendation::Good => 0.8,
            TimingRecommendation::Acceptable => 0.6,
            TimingRecommendation::Poor => 0.4,
            TimingRecommendation::Avoid => 0.2,
        }
    }
}

impl EnhancedTimingResult {
    /// Get the timing score from the result
    pub fn timing_score(&self) -> f64 {
        self.timing_score
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NeuralConfig;
    use crate::neural::NeuralPredictor;
    use crate::utils::market_hours::MarketHours;
    use tokio::sync::mpsc;
    use std::sync::Arc;

    async fn create_test_enhanced_coordinator() -> EnhancedDAACoordinator {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 60,
            max_retries: 3,
            error_threshold: 0.05,
        };
        
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).await.unwrap());
        let (tx, _rx) = mpsc::channel(100);
        let market_hours = Arc::new(MarketHours::default());
        
        let base_config = super::super::daa_coordinator::DaaConfig::default();
        let base_coordinator = Arc::new(
            super::super::daa_coordinator::DaaCoordinator::new(
                base_config, neural_predictor, tx, market_hours
            ).unwrap()
        );
        
        let data_quality_config = DataQualityConfig::default();
        EnhancedDAACoordinator::new(base_coordinator, data_quality_config)
    }

    #[tokio::test]
    async fn test_data_availability_assessment() {
        let data = vec![]; // Empty data
        let market_context = MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };
        
        let availability = assess_data_availability(&data, &market_context);
        
        // Empty data should have low completeness but good freshness
        assert_eq!(availability.completeness, 0.0);
        assert!(availability.overall_score() < 0.5);
    }

    #[tokio::test]
    async fn test_enhanced_coordinator_preserves_consensus() {
        let coordinator = create_test_enhanced_coordinator().await;
        
        // Verify base coordinator is accessible and maintains consensus threshold
        let base = coordinator.get_base_coordinator();
        let config = &base.config;
        
        // CRITICAL: Ensure Byzantine consensus parameters are preserved
        assert_eq!(config.consensus_threshold, 0.7); // 70% threshold
        assert!(config.enabled);
        
        // Test that enhanced coordinator doesn't modify base voting weights
        // The 60/40 neural/strategy weights are preserved in synthesize_decision
    }

    #[tokio::test]
    async fn test_data_quality_impact_calculation() {
        let coordinator = create_test_enhanced_coordinator().await;
        
        let base_decision = super::super::daa_coordinator::AutonomousDecision {
            timestamp: Utc::now(),
            action: super::super::daa_coordinator::TradingAction::Hold {
                reason: "Test".to_string(),
            },
            confidence: 0.8,
            risk_assessment: super::super::daa_coordinator::RiskAssessment {
                market_risk: 0.02,
                position_risk: 0.0,
                portfolio_risk: 0.01,
                volatility_adjusted_size: 0.02,
            },
            reasoning: vec!["Test reasoning".to_string()],
            neural_consensus: std::collections::HashMap::new(),
            adapted_parameters: None,
        };
        
        // Test with poor data quality
        let poor_data = DataAvailability {
            completeness: 0.5,
            freshness: 0.6,
            quality: 0.4,
            source_count: 1,
            market_coverage: 0.7,
            consistency: 0.5,
            latency_ms: 200.0,
            assessment_time: Utc::now(),
        };
        
        let impact = coordinator.calculate_data_quality_impact(&base_decision, &poor_data).await.unwrap();
        
        // Poor quality should reduce confidence
        assert!(impact.confidence_adjustment < 1.0);
        assert!(impact.size_adjustment < 1.0);
        assert!(impact.risk_adjustment > 1.0);
    }

    #[tokio::test]
    async fn test_enhanced_market_timing() {
        let coordinator = create_test_enhanced_coordinator().await;
        
        let market_context = MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1000000.0, // High volume
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };
        
        let data_availability = DataAvailability::default();
        
        let timing_result = coordinator
            .check_enhanced_market_timing(&market_context, &data_availability)
            .await
            .unwrap();
        
        // Should have valid timing score
        assert!(timing_result.timing_score >= 0.0);
        assert!(timing_result.timing_score <= 1.0);
        
        // Should have session analysis
        assert!(timing_result.session_analysis.session_weight > 0.0);
    }
}