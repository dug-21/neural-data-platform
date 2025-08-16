/**
 * Neural Coordinator Framework for Trading Platform
 * Implements cognitive pattern analysis and adaptive coordination strategies
 * Based on market conditions and trading performance
 */

import { EventEmitter } from 'events';
import { performance } from 'perf_hooks';

// Cognitive diversity patterns for neural coordination
const COGNITIVE_PATTERNS = {
  CONVERGENT: {
    name: 'convergent',
    description: 'Focused problem-solving, analytical, risk-averse',
    strengths: ['precision', 'optimization', 'risk_management'],
    marketConditions: ['trending', 'low_volatility'],
    weights: {
      analysis: 0.8,
      creativity: 0.2,
      riskTolerance: 0.3,
      speedVsAccuracy: 0.2, // Favors accuracy
    }
  },
  DIVERGENT: {
    name: 'divergent',
    description: 'Creative exploration, opportunity seeking',
    strengths: ['opportunity_detection', 'pattern_discovery', 'adaptation'],
    marketConditions: ['ranging', 'emerging_trends'],
    weights: {
      analysis: 0.3,
      creativity: 0.8,
      riskTolerance: 0.6,
      speedVsAccuracy: 0.7, // Favors speed
    }
  },
  LATERAL: {
    name: 'lateral',
    description: 'Non-linear thinking, pattern breaking, contrarian',
    strengths: ['contrarian_signals', 'anomaly_detection', 'innovation'],
    marketConditions: ['reversal_points', 'extreme_conditions'],
    weights: {
      analysis: 0.5,
      creativity: 0.7,
      riskTolerance: 0.7,
      speedVsAccuracy: 0.5, // Balanced
    }
  },
  SYSTEMS: {
    name: 'systems',
    description: 'Holistic view, interconnections, multi-asset correlation',
    strengths: ['correlation_analysis', 'portfolio_optimization', 'macro_view'],
    marketConditions: ['correlated_markets', 'sector_rotation'],
    weights: {
      analysis: 0.7,
      creativity: 0.4,
      riskTolerance: 0.5,
      speedVsAccuracy: 0.3, // Thorough analysis
    }
  },
  CRITICAL: {
    name: 'critical',
    description: 'Evaluation, judgment, validation, risk assessment',
    strengths: ['risk_assessment', 'validation', 'quality_control'],
    marketConditions: ['high_volatility', 'uncertain'],
    weights: {
      analysis: 0.9,
      creativity: 0.1,
      riskTolerance: 0.2,
      speedVsAccuracy: 0.1, // Maximum accuracy
    }
  },
  ADAPTIVE: {
    name: 'adaptive',
    description: 'Dynamic adaptation, learning from market feedback',
    strengths: ['real_time_learning', 'strategy_switching', 'evolution'],
    marketConditions: ['changing_regimes', 'dynamic'],
    weights: {
      analysis: 0.6,
      creativity: 0.6,
      riskTolerance: 0.5,
      speedVsAccuracy: 0.6, // Adaptive balance
    }
  }
};

// Market condition detection patterns
const MARKET_CONDITIONS = {
  TRENDING: {
    indicators: ['directional_movement', 'momentum', 'ma_alignment'],
    volatility: 'medium',
    preferredPatterns: ['convergent', 'systems'],
  },
  RANGING: {
    indicators: ['support_resistance', 'oscillators', 'mean_reversion'],
    volatility: 'low',
    preferredPatterns: ['divergent', 'lateral'],
  },
  HIGH_VOLATILITY: {
    indicators: ['atr', 'vix', 'price_swings'],
    volatility: 'high',
    preferredPatterns: ['critical', 'adaptive'],
  },
  REVERSAL: {
    indicators: ['divergence', 'extreme_levels', 'sentiment'],
    volatility: 'variable',
    preferredPatterns: ['lateral', 'critical'],
  },
  BREAKOUT: {
    indicators: ['volume_surge', 'range_break', 'momentum'],
    volatility: 'increasing',
    preferredPatterns: ['divergent', 'adaptive'],
  }
};

/**
 * Neural Coordinator class for managing cognitive patterns in trading
 */
class NeuralCoordinator extends EventEmitter {
  constructor(config = {}) {
    super();
    
    this.config = {
      learningRate: config.learningRate || 0.001,
      adaptationSpeed: config.adaptationSpeed || 0.1,
      memoryDepth: config.memoryDepth || 1000,
      patternSwitchThreshold: config.patternSwitchThreshold || 0.7,
      performanceWindow: config.performanceWindow || 100,
      ...config
    };

    // Current state
    this.currentPattern = COGNITIVE_PATTERNS.ADAPTIVE;
    this.marketCondition = null;
    this.activeStrategies = new Map();
    
    // Performance tracking
    this.performanceHistory = [];
    this.patternPerformance = new Map();
    this.marketConditionHistory = [];
    
    // Learning state
    this.patternWeights = new Map();
    this.conditionPatternSuccess = new Map();
    
    // Initialize pattern weights
    this.initializePatternWeights();
    
    // Coordination state
    this.coordinationSessions = new Map();
    this.agentPatternAssignments = new Map();
    
    console.log('Neural Coordinator initialized with config:', this.config);
  }

  /**
   * Initialize pattern weights for different market conditions
   */
  initializePatternWeights() {
    for (const [conditionName, condition] of Object.entries(MARKET_CONDITIONS)) {
      const conditionWeights = new Map();
      
      // Initialize with preferred patterns having higher weights
      for (const [patternName, pattern] of Object.entries(COGNITIVE_PATTERNS)) {
        const isPreferred = condition.preferredPatterns.includes(pattern.name);
        const initialWeight = isPreferred ? 0.7 : 0.3;
        conditionWeights.set(patternName, initialWeight);
      }
      
      this.patternWeights.set(conditionName, conditionWeights);
    }
    
    // Initialize pattern performance tracking
    for (const patternName of Object.keys(COGNITIVE_PATTERNS)) {
      this.patternPerformance.set(patternName, {
        totalTrades: 0,
        successfulTrades: 0,
        totalReturn: 0,
        avgResponseTime: 0,
        adaptationCount: 0
      });
    }
  }

  /**
   * Analyze market conditions and determine appropriate cognitive pattern
   */
  async analyzeMarketCondition(marketData) {
    const startTime = performance.now();
    
    try {
      // Extract market features
      const features = this.extractMarketFeatures(marketData);
      
      // Detect current market condition
      const condition = this.detectMarketCondition(features);
      
      // Update market condition history
      this.marketConditionHistory.push({
        condition,
        timestamp: Date.now(),
        features
      });
      
      // Trim history to memory depth
      if (this.marketConditionHistory.length > this.config.memoryDepth) {
        this.marketConditionHistory.shift();
      }
      
      this.marketCondition = condition;
      
      // Select optimal cognitive pattern
      const selectedPattern = await this.selectOptimalPattern(condition, features);
      
      // Check if pattern switch is needed
      if (this.shouldSwitchPattern(selectedPattern)) {
        await this.switchCognitivePattern(selectedPattern);
      }
      
      const analysisTime = performance.now() - startTime;
      
      this.emit('marketAnalysis', {
        condition,
        selectedPattern: selectedPattern.name,
        currentPattern: this.currentPattern.name,
        analysisTime,
        features
      });
      
      return {
        condition,
        pattern: this.currentPattern,
        confidence: this.calculatePatternConfidence(selectedPattern, condition)
      };
      
    } catch (error) {
      console.error('Error analyzing market condition:', error);
      throw error;
    }
  }

  /**
   * Extract relevant features from market data
   */
  extractMarketFeatures(marketData) {
    const { prices, volume, indicators } = marketData;
    
    // Calculate price-based features
    const returns = this.calculateReturns(prices);
    const volatility = this.calculateVolatility(returns);
    const trend = this.calculateTrend(prices);
    const momentum = this.calculateMomentum(prices);
    
    // Volume features
    const volumeProfile = this.analyzeVolumeProfile(volume);
    const volumeMomentum = this.calculateVolumeMomentum(volume);
    
    // Technical indicators
    const rsi = indicators?.rsi || this.calculateRSI(prices);
    const macd = indicators?.macd || this.calculateMACD(prices);
    const atr = indicators?.atr || this.calculateATR(marketData);
    
    // Market microstructure
    const spread = marketData.spread || 0;
    const orderImbalance = this.calculateOrderImbalance(marketData);
    
    return {
      volatility,
      trend,
      momentum,
      volumeProfile,
      volumeMomentum,
      rsi,
      macd,
      atr,
      spread,
      orderImbalance,
      returns
    };
  }

  /**
   * Detect market condition based on features
   */
  detectMarketCondition(features) {
    const conditions = [];
    
    // Trending detection
    if (Math.abs(features.trend) > 0.7 && features.momentum > 0.5) {
      conditions.push({ type: 'TRENDING', score: features.trend * features.momentum });
    }
    
    // Ranging detection
    if (features.volatility < 0.3 && Math.abs(features.trend) < 0.3) {
      conditions.push({ type: 'RANGING', score: (1 - features.volatility) * (1 - Math.abs(features.trend)) });
    }
    
    // High volatility detection
    if (features.volatility > 0.7 || features.atr > 2.0) {
      conditions.push({ type: 'HIGH_VOLATILITY', score: features.volatility });
    }
    
    // Reversal detection
    if (features.rsi > 70 || features.rsi < 30) {
      const reversalScore = Math.abs(features.rsi - 50) / 50;
      conditions.push({ type: 'REVERSAL', score: reversalScore });
    }
    
    // Breakout detection
    if (features.volumeMomentum > 1.5 && features.momentum > 0.6) {
      conditions.push({ type: 'BREAKOUT', score: features.volumeMomentum * features.momentum });
    }
    
    // Select condition with highest score
    if (conditions.length === 0) {
      return 'RANGING'; // Default condition
    }
    
    conditions.sort((a, b) => b.score - a.score);
    return conditions[0].type;
  }

  /**
   * Select optimal cognitive pattern for current conditions
   */
  async selectOptimalPattern(marketCondition, features) {
    // Get pattern weights for current market condition
    const conditionWeights = this.patternWeights.get(marketCondition) || new Map();
    
    // Calculate pattern scores
    const patternScores = new Map();
    
    for (const [patternName, pattern] of Object.entries(COGNITIVE_PATTERNS)) {
      let score = 0;
      
      // Base score from condition weights
      const baseWeight = conditionWeights.get(patternName) || 0.5;
      score += baseWeight * 0.4;
      
      // Performance history score
      const performance = this.patternPerformance.get(patternName);
      if (performance && performance.totalTrades > 0) {
        const successRate = performance.successfulTrades / performance.totalTrades;
        score += successRate * 0.3;
      }
      
      // Feature alignment score
      const alignmentScore = this.calculateFeatureAlignment(pattern, features);
      score += alignmentScore * 0.3;
      
      patternScores.set(patternName, score);
    }
    
    // Select pattern with highest score
    let bestPattern = null;
    let bestScore = -1;
    
    for (const [patternName, score] of patternScores) {
      if (score > bestScore) {
        bestScore = score;
        bestPattern = COGNITIVE_PATTERNS[patternName];
      }
    }
    
    return bestPattern || COGNITIVE_PATTERNS.ADAPTIVE;
  }

  /**
   * Calculate how well a pattern aligns with current features
   */
  calculateFeatureAlignment(pattern, features) {
    let alignment = 0;
    
    // Check pattern strengths against market features
    if (pattern.strengths.includes('risk_management') && features.volatility > 0.6) {
      alignment += 0.3;
    }
    
    if (pattern.strengths.includes('opportunity_detection') && features.momentum > 0.5) {
      alignment += 0.3;
    }
    
    if (pattern.strengths.includes('anomaly_detection') && 
        (features.rsi > 70 || features.rsi < 30)) {
      alignment += 0.2;
    }
    
    if (pattern.strengths.includes('correlation_analysis') && features.orderImbalance > 0.7) {
      alignment += 0.2;
    }
    
    return Math.min(1.0, alignment);
  }

  /**
   * Determine if pattern switch is beneficial
   */
  shouldSwitchPattern(newPattern) {
    if (newPattern.name === this.currentPattern.name) {
      return false;
    }
    
    // Calculate switching cost
    const switchingCost = 0.1; // Base cost for switching
    
    // Calculate expected benefit
    const currentPerformance = this.patternPerformance.get(this.currentPattern.name);
    const newPerformance = this.patternPerformance.get(newPattern.name);
    
    let expectedBenefit = 0;
    
    if (currentPerformance && newPerformance) {
      const currentSuccess = currentPerformance.totalTrades > 0 
        ? currentPerformance.successfulTrades / currentPerformance.totalTrades 
        : 0.5;
      const newSuccess = newPerformance.totalTrades > 0 
        ? newPerformance.successfulTrades / newPerformance.totalTrades 
        : 0.5;
      
      expectedBenefit = newSuccess - currentSuccess;
    }
    
    // Switch if expected benefit exceeds threshold plus switching cost
    return expectedBenefit > (this.config.patternSwitchThreshold - 0.5) + switchingCost;
  }

  /**
   * Switch to new cognitive pattern
   */
  async switchCognitivePattern(newPattern) {
    const oldPattern = this.currentPattern;
    this.currentPattern = newPattern;
    
    // Update pattern performance
    const performance = this.patternPerformance.get(newPattern.name);
    if (performance) {
      performance.adaptationCount++;
    }
    
    // Notify all coordinated agents
    for (const [agentId, assignment] of this.agentPatternAssignments) {
      if (assignment.pattern === oldPattern.name) {
        await this.reassignAgentPattern(agentId, newPattern);
      }
    }
    
    this.emit('patternSwitch', {
      from: oldPattern.name,
      to: newPattern.name,
      timestamp: Date.now(),
      marketCondition: this.marketCondition
    });
    
    console.log(`Switched cognitive pattern from ${oldPattern.name} to ${newPattern.name}`);
  }

  /**
   * Create coordination session with cognitive pattern assignment
   */
  async createCoordinationSession(agents, task) {
    const sessionId = `session_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    
    const session = {
      id: sessionId,
      agents,
      task,
      pattern: this.currentPattern,
      marketCondition: this.marketCondition,
      startTime: Date.now(),
      assignments: new Map()
    };
    
    // Assign cognitive patterns to agents based on their roles
    for (const agent of agents) {
      const assignment = await this.assignCognitivePattern(agent, task);
      session.assignments.set(agent.id, assignment);
      this.agentPatternAssignments.set(agent.id, assignment);
    }
    
    this.coordinationSessions.set(sessionId, session);
    
    this.emit('sessionCreated', {
      sessionId,
      pattern: this.currentPattern.name,
      agentCount: agents.length,
      task
    });
    
    return session;
  }

  /**
   * Assign cognitive pattern to agent based on role and task
   */
  async assignCognitivePattern(agent, task) {
    // Determine pattern based on agent type and current market condition
    let assignedPattern = this.currentPattern;
    
    // Special assignments based on agent type
    switch (agent.type) {
      case 'researcher':
        // Researchers often benefit from divergent thinking
        if (this.marketCondition === 'RANGING' || this.marketCondition === 'REVERSAL') {
          assignedPattern = COGNITIVE_PATTERNS.DIVERGENT;
        }
        break;
        
      case 'analyst':
        // Analysts need critical thinking
        if (this.marketCondition === 'HIGH_VOLATILITY') {
          assignedPattern = COGNITIVE_PATTERNS.CRITICAL;
        }
        break;
        
      case 'optimizer':
        // Optimizers focus on systems thinking
        assignedPattern = COGNITIVE_PATTERNS.SYSTEMS;
        break;
        
      case 'coder':
        // Coders need convergent thinking for implementation
        assignedPattern = COGNITIVE_PATTERNS.CONVERGENT;
        break;
    }
    
    return {
      agentId: agent.id,
      agentType: agent.type,
      pattern: assignedPattern.name,
      weights: assignedPattern.weights,
      task,
      timestamp: Date.now()
    };
  }

  /**
   * Update pattern learning from trading results
   */
  async updatePatternLearning(tradingResult) {
    const { pattern, success, returnValue, responseTime, marketCondition } = tradingResult;
    
    // Update pattern performance
    const performance = this.patternPerformance.get(pattern);
    if (performance) {
      performance.totalTrades++;
      if (success) {
        performance.successfulTrades++;
      }
      performance.totalReturn += returnValue || 0;
      
      // Update average response time
      const oldAvg = performance.avgResponseTime;
      const newCount = performance.totalTrades;
      performance.avgResponseTime = (oldAvg * (newCount - 1) + responseTime) / newCount;
    }
    
    // Update pattern weights for market condition
    if (marketCondition) {
      const conditionWeights = this.patternWeights.get(marketCondition);
      if (conditionWeights) {
        const currentWeight = conditionWeights.get(pattern) || 0.5;
        const adjustment = success ? this.config.learningRate : -this.config.learningRate;
        const newWeight = Math.max(0.1, Math.min(0.9, currentWeight + adjustment));
        conditionWeights.set(pattern, newWeight);
      }
    }
    
    // Store in performance history
    this.performanceHistory.push({
      pattern,
      success,
      returnValue,
      responseTime,
      marketCondition,
      timestamp: Date.now()
    });
    
    // Trim history
    if (this.performanceHistory.length > this.config.memoryDepth) {
      this.performanceHistory.shift();
    }
    
    this.emit('learningUpdate', {
      pattern,
      performance: this.patternPerformance.get(pattern),
      marketCondition
    });
  }

  /**
   * Get coordination recommendations for current market
   */
  async getCoordinationRecommendations() {
    const recommendations = {
      primaryPattern: this.currentPattern,
      marketCondition: this.marketCondition,
      agentAllocation: {},
      riskLevel: 'medium',
      strategies: []
    };
    
    // Determine agent allocation based on pattern
    switch (this.currentPattern.name) {
      case 'convergent':
        recommendations.agentAllocation = {
          analyst: 2,
          optimizer: 2,
          coder: 1,
          researcher: 1
        };
        recommendations.riskLevel = 'low';
        recommendations.strategies = ['trend_following', 'risk_management'];
        break;
        
      case 'divergent':
        recommendations.agentAllocation = {
          researcher: 2,
          analyst: 1,
          coder: 2,
          optimizer: 1
        };
        recommendations.riskLevel = 'medium';
        recommendations.strategies = ['opportunity_seeking', 'pattern_discovery'];
        break;
        
      case 'critical':
        recommendations.agentAllocation = {
          analyst: 3,
          optimizer: 1,
          researcher: 1,
          coder: 1
        };
        recommendations.riskLevel = 'very_low';
        recommendations.strategies = ['risk_assessment', 'validation'];
        break;
        
      case 'lateral':
        recommendations.agentAllocation = {
          researcher: 2,
          analyst: 2,
          coder: 1,
          optimizer: 1
        };
        recommendations.riskLevel = 'high';
        recommendations.strategies = ['contrarian', 'anomaly_detection'];
        break;
        
      case 'systems':
        recommendations.agentAllocation = {
          optimizer: 2,
          analyst: 2,
          researcher: 1,
          coder: 1
        };
        recommendations.riskLevel = 'medium';
        recommendations.strategies = ['portfolio_optimization', 'correlation_trading'];
        break;
        
      case 'adaptive':
        recommendations.agentAllocation = {
          researcher: 1,
          analyst: 2,
          coder: 2,
          optimizer: 1
        };
        recommendations.riskLevel = 'dynamic';
        recommendations.strategies = ['adaptive_strategy', 'real_time_learning'];
        break;
    }
    
    return recommendations;
  }

  /**
   * Calculate pattern confidence based on historical performance
   */
  calculatePatternConfidence(pattern, marketCondition) {
    const performance = this.patternPerformance.get(pattern.name);
    if (!performance || performance.totalTrades < 10) {
      return 0.5; // Default confidence for new patterns
    }
    
    // Base confidence on success rate
    const successRate = performance.successfulTrades / performance.totalTrades;
    let confidence = successRate;
    
    // Adjust based on market condition alignment
    const conditionWeights = this.patternWeights.get(marketCondition);
    if (conditionWeights) {
      const weight = conditionWeights.get(pattern.name) || 0.5;
      confidence = confidence * 0.7 + weight * 0.3;
    }
    
    // Adjust based on recent performance
    const recentTrades = this.performanceHistory
      .filter(h => h.pattern === pattern.name)
      .slice(-20);
    
    if (recentTrades.length > 0) {
      const recentSuccess = recentTrades.filter(t => t.success).length / recentTrades.length;
      confidence = confidence * 0.6 + recentSuccess * 0.4;
    }
    
    return Math.max(0.1, Math.min(0.9, confidence));
  }

  /**
   * Get pattern performance metrics
   */
  getPatternMetrics() {
    const metrics = {};
    
    for (const [patternName, performance] of this.patternPerformance) {
      metrics[patternName] = {
        ...performance,
        successRate: performance.totalTrades > 0 
          ? performance.successfulTrades / performance.totalTrades 
          : 0,
        avgReturn: performance.totalTrades > 0 
          ? performance.totalReturn / performance.totalTrades 
          : 0
      };
    }
    
    return metrics;
  }

  /**
   * Get current coordination state
   */
  getCoordinationState() {
    return {
      currentPattern: this.currentPattern.name,
      marketCondition: this.marketCondition,
      activeSessions: this.coordinationSessions.size,
      patternMetrics: this.getPatternMetrics(),
      recentPerformance: this.performanceHistory.slice(-10),
      patternWeights: Object.fromEntries(this.patternWeights)
    };
  }

  // Helper methods for technical calculations
  
  calculateReturns(prices) {
    if (prices.length < 2) return [];
    const returns = [];
    for (let i = 1; i < prices.length; i++) {
      returns.push((prices[i] - prices[i - 1]) / prices[i - 1]);
    }
    return returns;
  }

  calculateVolatility(returns) {
    if (returns.length === 0) return 0;
    const mean = returns.reduce((a, b) => a + b, 0) / returns.length;
    const variance = returns.reduce((sum, r) => sum + Math.pow(r - mean, 2), 0) / returns.length;
    return Math.sqrt(variance);
  }

  calculateTrend(prices) {
    if (prices.length < 2) return 0;
    // Simple linear regression slope
    const n = prices.length;
    const xSum = (n * (n - 1)) / 2;
    const xSquaredSum = (n * (n - 1) * (2 * n - 1)) / 6;
    const ySum = prices.reduce((a, b) => a + b, 0);
    let xySum = 0;
    for (let i = 0; i < n; i++) {
      xySum += i * prices[i];
    }
    const slope = (n * xySum - xSum * ySum) / (n * xSquaredSum - xSum * xSum);
    return slope / (ySum / n); // Normalize by average price
  }

  calculateMomentum(prices, period = 10) {
    if (prices.length < period) return 0;
    const currentPrice = prices[prices.length - 1];
    const pastPrice = prices[prices.length - period];
    return (currentPrice - pastPrice) / pastPrice;
  }

  analyzeVolumeProfile(volume) {
    if (!volume || volume.length === 0) return { average: 0, trend: 0 };
    const avg = volume.reduce((a, b) => a + b, 0) / volume.length;
    const trend = this.calculateTrend(volume);
    return { average: avg, trend };
  }

  calculateVolumeMomentum(volume, period = 10) {
    if (!volume || volume.length < period) return 1;
    const recentVolume = volume.slice(-period).reduce((a, b) => a + b, 0) / period;
    const previousVolume = volume.slice(-2 * period, -period).reduce((a, b) => a + b, 0) / period;
    return previousVolume > 0 ? recentVolume / previousVolume : 1;
  }

  calculateRSI(prices, period = 14) {
    if (prices.length < period + 1) return 50;
    
    const changes = [];
    for (let i = 1; i < prices.length; i++) {
      changes.push(prices[i] - prices[i - 1]);
    }
    
    const gains = changes.map(c => c > 0 ? c : 0);
    const losses = changes.map(c => c < 0 ? -c : 0);
    
    const avgGain = gains.slice(-period).reduce((a, b) => a + b, 0) / period;
    const avgLoss = losses.slice(-period).reduce((a, b) => a + b, 0) / period;
    
    if (avgLoss === 0) return 100;
    const rs = avgGain / avgLoss;
    return 100 - (100 / (1 + rs));
  }

  calculateMACD(prices) {
    // Simplified MACD calculation
    const ema12 = this.calculateEMA(prices, 12);
    const ema26 = this.calculateEMA(prices, 26);
    const macd = ema12 - ema26;
    const signal = this.calculateEMA([macd], 9);
    return { macd, signal, histogram: macd - signal };
  }

  calculateEMA(prices, period) {
    if (prices.length === 0) return 0;
    const multiplier = 2 / (period + 1);
    let ema = prices[0];
    for (let i = 1; i < prices.length; i++) {
      ema = (prices[i] - ema) * multiplier + ema;
    }
    return ema;
  }

  calculateATR(marketData, period = 14) {
    // Simplified ATR calculation
    if (!marketData.high || !marketData.low || !marketData.close) return 0;
    const ranges = [];
    for (let i = 1; i < marketData.high.length; i++) {
      const highLow = marketData.high[i] - marketData.low[i];
      const highClose = Math.abs(marketData.high[i] - marketData.close[i - 1]);
      const lowClose = Math.abs(marketData.low[i] - marketData.close[i - 1]);
      ranges.push(Math.max(highLow, highClose, lowClose));
    }
    return ranges.slice(-period).reduce((a, b) => a + b, 0) / Math.min(period, ranges.length);
  }

  calculateOrderImbalance(marketData) {
    if (!marketData.bidVolume || !marketData.askVolume) return 0;
    const totalVolume = marketData.bidVolume + marketData.askVolume;
    if (totalVolume === 0) return 0;
    return (marketData.bidVolume - marketData.askVolume) / totalVolume;
  }

  async reassignAgentPattern(agentId, newPattern) {
    const assignment = this.agentPatternAssignments.get(agentId);
    if (assignment) {
      assignment.pattern = newPattern.name;
      assignment.weights = newPattern.weights;
      assignment.timestamp = Date.now();
      
      this.emit('agentPatternReassigned', {
        agentId,
        newPattern: newPattern.name,
        timestamp: Date.now()
      });
    }
  }
}

export { NeuralCoordinator, COGNITIVE_PATTERNS, MARKET_CONDITIONS };