/**
 * Neural Trading Integration Module
 * Bridges neural coordination with trading strategies and swarm agents
 */

import { NeuralCoordinator, COGNITIVE_PATTERNS } from './neural-coordinator.js';
import { EventEmitter } from 'events';

/**
 * Integration layer between neural coordinator and trading system
 */
class NeuralTradingIntegration extends EventEmitter {
  constructor(config = {}) {
    super();
    
    this.config = {
      coordinationEnabled: true,
      patternAdaptationEnabled: true,
      performanceTrackingEnabled: true,
      riskAdjustmentEnabled: true,
      ...config
    };
    
    // Initialize neural coordinator
    this.neuralCoordinator = new NeuralCoordinator({
      learningRate: config.learningRate || 0.001,
      adaptationSpeed: config.adaptationSpeed || 0.1,
      memoryDepth: config.memoryDepth || 1000
    });
    
    // Trading state
    this.activeTrades = new Map();
    this.tradingStrategies = new Map();
    this.agentCoordination = new Map();
    
    // Performance tracking
    this.tradingPerformance = {
      totalTrades: 0,
      successfulTrades: 0,
      totalReturn: 0,
      sharpeRatio: 0,
      maxDrawdown: 0
    };
    
    // Setup event listeners
    this.setupEventListeners();
    
    console.log('Neural Trading Integration initialized');
  }

  /**
   * Setup event listeners for neural coordinator
   */
  setupEventListeners() {
    // Listen to pattern switches
    this.neuralCoordinator.on('patternSwitch', (data) => {
      this.handlePatternSwitch(data);
    });
    
    // Listen to market analysis
    this.neuralCoordinator.on('marketAnalysis', (data) => {
      this.handleMarketAnalysis(data);
    });
    
    // Listen to learning updates
    this.neuralCoordinator.on('learningUpdate', (data) => {
      this.handleLearningUpdate(data);
    });
  }

  /**
   * Process market data through neural coordinator
   */
  async processMarketData(marketData) {
    try {
      // Analyze market through neural coordinator
      const analysis = await this.neuralCoordinator.analyzeMarketCondition(marketData);
      
      // Get coordination recommendations
      const recommendations = await this.neuralCoordinator.getCoordinationRecommendations();
      
      // Adjust trading parameters based on cognitive pattern
      const tradingParams = this.adjustTradingParameters(analysis.pattern, analysis.confidence);
      
      // Generate trading signals with pattern influence
      const signals = await this.generateNeuralTradingSignals(
        marketData,
        analysis,
        recommendations,
        tradingParams
      );
      
      this.emit('marketProcessed', {
        analysis,
        recommendations,
        tradingParams,
        signals,
        timestamp: Date.now()
      });
      
      return {
        analysis,
        recommendations,
        tradingParams,
        signals
      };
      
    } catch (error) {
      console.error('Error processing market data:', error);
      throw error;
    }
  }

  /**
   * Adjust trading parameters based on cognitive pattern
   */
  adjustTradingParameters(pattern, confidence) {
    const baseParams = {
      positionSize: 1.0,
      stopLoss: 0.02,
      takeProfit: 0.04,
      entryThreshold: 0.3,
      exitThreshold: 0.2,
      maxPositions: 3,
      riskPerTrade: 0.01
    };
    
    // Adjust based on pattern characteristics
    const adjustedParams = { ...baseParams };
    
    switch (pattern.name) {
      case 'convergent':
        // Conservative, focused on precision
        adjustedParams.positionSize *= 0.8;
        adjustedParams.stopLoss *= 0.8;
        adjustedParams.takeProfit *= 0.9;
        adjustedParams.entryThreshold *= 1.2;
        adjustedParams.maxPositions = 2;
        break;
        
      case 'divergent':
        // Exploratory, seeking opportunities
        adjustedParams.positionSize *= 1.1;
        adjustedParams.stopLoss *= 1.2;
        adjustedParams.takeProfit *= 1.3;
        adjustedParams.entryThreshold *= 0.9;
        adjustedParams.maxPositions = 4;
        break;
        
      case 'lateral':
        // Contrarian, pattern breaking
        adjustedParams.positionSize *= 0.9;
        adjustedParams.stopLoss *= 1.1;
        adjustedParams.takeProfit *= 1.5;
        adjustedParams.entryThreshold *= 0.8;
        adjustedParams.exitThreshold *= 0.7;
        break;
        
      case 'critical':
        // Risk-averse, validation focused
        adjustedParams.positionSize *= 0.6;
        adjustedParams.stopLoss *= 0.6;
        adjustedParams.takeProfit *= 0.8;
        adjustedParams.entryThreshold *= 1.5;
        adjustedParams.maxPositions = 1;
        adjustedParams.riskPerTrade *= 0.5;
        break;
        
      case 'systems':
        // Portfolio-oriented, correlation aware
        adjustedParams.positionSize *= 0.9;
        adjustedParams.maxPositions = 5;
        adjustedParams.riskPerTrade *= 0.8;
        break;
        
      case 'adaptive':
        // Dynamic adjustment based on confidence
        const confidenceMultiplier = 0.5 + confidence;
        adjustedParams.positionSize *= confidenceMultiplier;
        adjustedParams.entryThreshold *= (2 - confidenceMultiplier);
        break;
    }
    
    // Apply confidence adjustment
    adjustedParams.positionSize *= (0.5 + confidence * 0.5);
    
    return adjustedParams;
  }

  /**
   * Generate trading signals influenced by neural patterns
   */
  async generateNeuralTradingSignals(marketData, analysis, recommendations, tradingParams) {
    const signals = [];
    
    // Base signal generation
    const baseSignal = this.calculateBaseSignal(marketData);
    
    // Pattern-specific signal adjustments
    const patternAdjustment = this.getPatternSignalAdjustment(
      analysis.pattern,
      marketData,
      analysis.condition
    );
    
    // Combine signals
    const combinedSignal = baseSignal * 0.6 + patternAdjustment * 0.4;
    
    // Generate trading actions based on thresholds
    if (Math.abs(combinedSignal) > tradingParams.entryThreshold) {
      const direction = combinedSignal > 0 ? 'BUY' : 'SELL';
      
      signals.push({
        action: direction,
        strength: Math.abs(combinedSignal),
        confidence: analysis.confidence,
        pattern: analysis.pattern.name,
        marketCondition: analysis.condition,
        positionSize: this.calculatePositionSize(
          tradingParams.positionSize,
          analysis.confidence,
          Math.abs(combinedSignal)
        ),
        stopLoss: tradingParams.stopLoss,
        takeProfit: tradingParams.takeProfit,
        metadata: {
          baseSignal,
          patternAdjustment,
          combinedSignal,
          timestamp: Date.now()
        }
      });
    }
    
    return signals;
  }

  /**
   * Calculate base trading signal from market data
   */
  calculateBaseSignal(marketData) {
    const { prices, volume, indicators } = marketData;
    
    // Simple momentum signal
    const momentum = indicators?.momentum || 0;
    
    // RSI signal
    const rsi = indicators?.rsi || 50;
    const rsiSignal = (50 - rsi) / 50; // Contrarian RSI
    
    // Volume confirmation
    const volumeConfirmation = volume?.trend || 0;
    
    // Combine signals
    return momentum * 0.5 + rsiSignal * 0.3 + volumeConfirmation * 0.2;
  }

  /**
   * Get pattern-specific signal adjustments
   */
  getPatternSignalAdjustment(pattern, marketData, marketCondition) {
    let adjustment = 0;
    
    switch (pattern.name) {
      case 'convergent':
        // Focus on trend continuation
        if (marketCondition === 'TRENDING') {
          adjustment = marketData.indicators?.trend || 0;
        }
        break;
        
      case 'divergent':
        // Look for breakout opportunities
        if (marketData.volume?.surge > 1.5) {
          adjustment = 0.5;
        }
        break;
        
      case 'lateral':
        // Contrarian signals at extremes
        const rsi = marketData.indicators?.rsi || 50;
        if (rsi > 70) adjustment = -0.6;
        if (rsi < 30) adjustment = 0.6;
        break;
        
      case 'critical':
        // Only strong, confirmed signals
        if (Math.abs(marketData.indicators?.momentum || 0) > 0.7) {
          adjustment = marketData.indicators.momentum * 0.5;
        }
        break;
        
      case 'systems':
        // Multi-factor confirmation
        const factors = [
          marketData.indicators?.trend || 0,
          marketData.indicators?.momentum || 0,
          marketData.volume?.trend || 0
        ];
        adjustment = factors.reduce((a, b) => a + b, 0) / factors.length;
        break;
        
      case 'adaptive':
        // Dynamic adjustment based on recent performance
        adjustment = this.getAdaptiveAdjustment(marketData, marketCondition);
        break;
    }
    
    return adjustment;
  }

  /**
   * Calculate adaptive adjustment based on recent performance
   */
  getAdaptiveAdjustment(marketData, marketCondition) {
    // Get recent performance for current condition
    const recentPerformance = this.neuralCoordinator.performanceHistory
      .filter(p => p.marketCondition === marketCondition)
      .slice(-10);
    
    if (recentPerformance.length === 0) {
      return 0;
    }
    
    // Calculate success rate
    const successRate = recentPerformance.filter(p => p.success).length / recentPerformance.length;
    
    // Adjust signal based on success rate
    if (successRate > 0.7) {
      // Increase signal strength when performing well
      return marketData.indicators?.momentum || 0;
    } else if (successRate < 0.3) {
      // Reverse signal when performing poorly
      return -(marketData.indicators?.momentum || 0);
    }
    
    return 0;
  }

  /**
   * Calculate position size with neural influence
   */
  calculatePositionSize(baseSize, confidence, signalStrength) {
    // Risk-adjusted position sizing
    let size = baseSize;
    
    // Adjust for confidence
    size *= (0.5 + confidence * 0.5);
    
    // Adjust for signal strength
    size *= Math.min(1.0, signalStrength);
    
    // Apply Kelly Criterion approximation
    const winRate = this.tradingPerformance.totalTrades > 0
      ? this.tradingPerformance.successfulTrades / this.tradingPerformance.totalTrades
      : 0.5;
    const avgWin = 0.04; // Average win (take profit)
    const avgLoss = 0.02; // Average loss (stop loss)
    
    if (winRate > 0 && avgLoss > 0) {
      const kellyFraction = (winRate * avgWin - (1 - winRate) * avgLoss) / avgWin;
      size *= Math.max(0.1, Math.min(0.25, kellyFraction)); // Cap Kelly fraction
    }
    
    return Math.max(0.1, Math.min(1.0, size));
  }

  /**
   * Execute trade with neural coordination
   */
  async executeTrade(signal, marketData) {
    const tradeId = `trade_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    
    const trade = {
      id: tradeId,
      signal,
      entryPrice: marketData.currentPrice,
      entryTime: Date.now(),
      status: 'open',
      pattern: signal.pattern,
      marketCondition: signal.marketCondition
    };
    
    this.activeTrades.set(tradeId, trade);
    this.tradingPerformance.totalTrades++;
    
    // Create coordination session for trade execution
    if (this.config.coordinationEnabled) {
      const agents = this.getAvailableAgents();
      const session = await this.neuralCoordinator.createCoordinationSession(
        agents,
        `Execute ${signal.action} trade`
      );
      
      this.agentCoordination.set(tradeId, session);
    }
    
    this.emit('tradeExecuted', trade);
    
    return trade;
  }

  /**
   * Update trade result and learn from outcome
   */
  async updateTradeResult(tradeId, result) {
    const trade = this.activeTrades.get(tradeId);
    if (!trade) return;
    
    trade.exitPrice = result.exitPrice;
    trade.exitTime = Date.now();
    trade.status = 'closed';
    trade.profit = result.profit;
    trade.success = result.profit > 0;
    
    // Update performance metrics
    if (trade.success) {
      this.tradingPerformance.successfulTrades++;
    }
    this.tradingPerformance.totalReturn += result.profit;
    
    // Update neural learning
    await this.neuralCoordinator.updatePatternLearning({
      pattern: trade.pattern,
      success: trade.success,
      returnValue: result.profit,
      responseTime: trade.exitTime - trade.entryTime,
      marketCondition: trade.marketCondition
    });
    
    // Clean up
    this.activeTrades.delete(tradeId);
    this.agentCoordination.delete(tradeId);
    
    this.emit('tradeCompleted', trade);
  }

  /**
   * Handle pattern switch events
   */
  handlePatternSwitch(data) {
    console.log(`Pattern switched from ${data.from} to ${data.to}`);
    
    // Adjust all active trades
    for (const [tradeId, trade] of this.activeTrades) {
      if (trade.status === 'open') {
        // Consider closing trades that don't align with new pattern
        const shouldClose = this.shouldCloseTrade(trade, data.to);
        if (shouldClose) {
          this.emit('closeTradeSignal', { tradeId, reason: 'pattern_switch' });
        }
      }
    }
    
    // Notify connected systems
    this.emit('patternChanged', data);
  }

  /**
   * Determine if trade should be closed due to pattern change
   */
  shouldCloseTrade(trade, newPattern) {
    // Conservative patterns should close risky trades
    if (newPattern === 'critical' || newPattern === 'convergent') {
      return trade.signal.strength < 0.5 || trade.signal.confidence < 0.6;
    }
    
    // Pattern mismatch
    if (trade.pattern === 'divergent' && newPattern === 'convergent') {
      return true;
    }
    
    return false;
  }

  /**
   * Handle market analysis updates
   */
  handleMarketAnalysis(data) {
    // Update trading strategies based on market condition
    this.updateTradingStrategies(data.condition, data.selectedPattern);
  }

  /**
   * Update active trading strategies
   */
  updateTradingStrategies(marketCondition, pattern) {
    // Clear existing strategies
    this.tradingStrategies.clear();
    
    // Add strategies based on pattern and market condition
    const recommendations = this.neuralCoordinator.getCoordinationRecommendations();
    
    for (const strategy of recommendations.strategies) {
      this.tradingStrategies.set(strategy, {
        active: true,
        pattern,
        marketCondition,
        timestamp: Date.now()
      });
    }
  }

  /**
   * Handle learning updates
   */
  handleLearningUpdate(data) {
    // Log learning progress
    console.log(`Pattern ${data.pattern} performance updated:`, data.performance);
    
    // Emit for external monitoring
    this.emit('learningProgress', data);
  }

  /**
   * Get available agents (placeholder - would connect to actual swarm)
   */
  getAvailableAgents() {
    // In real implementation, this would query the swarm coordinator
    return [
      { id: 'agent_1', type: 'analyst', status: 'available' },
      { id: 'agent_2', type: 'researcher', status: 'available' },
      { id: 'agent_3', type: 'optimizer', status: 'available' }
    ];
  }

  /**
   * Get integration status
   */
  getStatus() {
    return {
      coordinatorState: this.neuralCoordinator.getCoordinationState(),
      activeTrades: this.activeTrades.size,
      tradingPerformance: this.tradingPerformance,
      activeStrategies: Array.from(this.tradingStrategies.keys()),
      config: this.config
    };
  }

  /**
   * Get performance metrics
   */
  getPerformanceMetrics() {
    const winRate = this.tradingPerformance.totalTrades > 0
      ? this.tradingPerformance.successfulTrades / this.tradingPerformance.totalTrades
      : 0;
    
    const avgReturn = this.tradingPerformance.totalTrades > 0
      ? this.tradingPerformance.totalReturn / this.tradingPerformance.totalTrades
      : 0;
    
    return {
      ...this.tradingPerformance,
      winRate,
      avgReturn,
      patternMetrics: this.neuralCoordinator.getPatternMetrics()
    };
  }
}

export { NeuralTradingIntegration };