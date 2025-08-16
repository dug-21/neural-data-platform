/**
 * Test and demonstration of Neural Coordinator functionality
 */

import { NeuralCoordinator, COGNITIVE_PATTERNS, MARKET_CONDITIONS } from './neural-coordinator.js';
import { NeuralTradingIntegration } from './neural-trading-integration.js';

// Simulate market data for testing
function generateMarketData(scenario = 'trending') {
  const basePrice = 100;
  const dataPoints = 50;
  const prices = [];
  const volume = [];
  
  for (let i = 0; i < dataPoints; i++) {
    let price;
    let vol;
    
    switch (scenario) {
      case 'trending':
        // Upward trend with some noise
        price = basePrice + i * 0.5 + (Math.random() - 0.5) * 0.5;
        vol = 1000000 + Math.random() * 500000;
        break;
        
      case 'ranging':
        // Sideways movement
        price = basePrice + Math.sin(i * 0.2) * 2 + (Math.random() - 0.5) * 0.3;
        vol = 800000 + Math.random() * 200000;
        break;
        
      case 'volatile':
        // High volatility
        price = basePrice + (Math.random() - 0.5) * 5;
        vol = 500000 + Math.random() * 1500000;
        break;
        
      case 'reversal':
        // Trend reversal
        price = i < 25 
          ? basePrice + i * 0.3 
          : basePrice + 25 * 0.3 - (i - 25) * 0.4;
        vol = 1000000 + (i > 20 && i < 30 ? 1000000 : 0);
        break;
        
      default:
        price = basePrice + (Math.random() - 0.5) * 2;
        vol = 1000000;
    }
    
    prices.push(price);
    volume.push(vol);
  }
  
  return {
    prices,
    volume,
    currentPrice: prices[prices.length - 1],
    high: prices.map((p, i) => p + Math.random() * 0.5),
    low: prices.map((p, i) => p - Math.random() * 0.5),
    close: prices,
    spread: 0.01 + Math.random() * 0.02,
    bidVolume: volume[volume.length - 1] * 0.48,
    askVolume: volume[volume.length - 1] * 0.52,
    indicators: {
      rsi: 30 + Math.random() * 40,
      momentum: scenario === 'trending' ? 0.6 : 0.1,
      trend: scenario === 'trending' ? 0.7 : 0.1
    }
  };
}

// Test neural coordinator
async function testNeuralCoordinator() {
  console.log('=== Testing Neural Coordinator ===\n');
  
  const coordinator = new NeuralCoordinator({
    learningRate: 0.01,
    adaptationSpeed: 0.2,
    patternSwitchThreshold: 0.6
  });
  
  // Test 1: Market condition detection
  console.log('Test 1: Market Condition Detection');
  console.log('-'.repeat(40));
  
  const scenarios = ['trending', 'ranging', 'volatile', 'reversal'];
  
  for (const scenario of scenarios) {
    const marketData = generateMarketData(scenario);
    const analysis = await coordinator.analyzeMarketCondition(marketData);
    
    console.log(`\nScenario: ${scenario}`);
    console.log(`Detected condition: ${analysis.condition}`);
    console.log(`Selected pattern: ${analysis.pattern.name}`);
    console.log(`Confidence: ${(analysis.confidence * 100).toFixed(1)}%`);
  }
  
  // Test 2: Pattern learning
  console.log('\n\nTest 2: Pattern Learning from Trading Results');
  console.log('-'.repeat(40));
  
  // Simulate trading results
  const tradingResults = [
    { pattern: 'convergent', success: true, returnValue: 0.02, responseTime: 1000, marketCondition: 'TRENDING' },
    { pattern: 'convergent', success: true, returnValue: 0.015, responseTime: 1200, marketCondition: 'TRENDING' },
    { pattern: 'divergent', success: false, returnValue: -0.01, responseTime: 800, marketCondition: 'RANGING' },
    { pattern: 'lateral', success: true, returnValue: 0.03, responseTime: 1500, marketCondition: 'REVERSAL' },
    { pattern: 'critical', success: true, returnValue: 0.01, responseTime: 2000, marketCondition: 'HIGH_VOLATILITY' }
  ];
  
  for (const result of tradingResults) {
    await coordinator.updatePatternLearning(result);
  }
  
  const metrics = coordinator.getPatternMetrics();
  console.log('\nPattern Performance Metrics:');
  for (const [pattern, performance] of Object.entries(metrics)) {
    if (performance.totalTrades > 0) {
      console.log(`\n${pattern}:`);
      console.log(`  Success rate: ${(performance.successRate * 100).toFixed(1)}%`);
      console.log(`  Avg return: ${(performance.avgReturn * 100).toFixed(2)}%`);
      console.log(`  Avg response time: ${performance.avgResponseTime.toFixed(0)}ms`);
    }
  }
  
  // Test 3: Coordination session
  console.log('\n\nTest 3: Agent Coordination Session');
  console.log('-'.repeat(40));
  
  const agents = [
    { id: 'agent_1', type: 'researcher' },
    { id: 'agent_2', type: 'analyst' },
    { id: 'agent_3', type: 'coder' },
    { id: 'agent_4', type: 'optimizer' }
  ];
  
  const session = await coordinator.createCoordinationSession(agents, 'Analyze market opportunity');
  
  console.log(`\nSession ID: ${session.id}`);
  console.log(`Pattern: ${session.pattern.name}`);
  console.log(`Market condition: ${session.marketCondition || 'Unknown'}`);
  console.log('\nAgent assignments:');
  
  for (const [agentId, assignment] of session.assignments) {
    console.log(`  ${agentId} (${assignment.agentType}): ${assignment.pattern} pattern`);
  }
  
  // Test 4: Recommendations
  console.log('\n\nTest 4: Coordination Recommendations');
  console.log('-'.repeat(40));
  
  const recommendations = await coordinator.getCoordinationRecommendations();
  console.log('\nRecommendations:');
  console.log(`Primary pattern: ${recommendations.primaryPattern.name}`);
  console.log(`Risk level: ${recommendations.riskLevel}`);
  console.log('Agent allocation:');
  for (const [type, count] of Object.entries(recommendations.agentAllocation)) {
    console.log(`  ${type}: ${count} agents`);
  }
  console.log(`Strategies: ${recommendations.strategies.join(', ')}`);
  
  return coordinator;
}

// Test neural trading integration
async function testNeuralTradingIntegration() {
  console.log('\n\n=== Testing Neural Trading Integration ===\n');
  
  const integration = new NeuralTradingIntegration({
    learningRate: 0.01,
    adaptationSpeed: 0.2
  });
  
  // Test different market scenarios
  const testScenarios = [
    { name: 'Strong Uptrend', scenario: 'trending' },
    { name: 'Sideways Market', scenario: 'ranging' },
    { name: 'High Volatility', scenario: 'volatile' },
    { name: 'Trend Reversal', scenario: 'reversal' }
  ];
  
  for (const test of testScenarios) {
    console.log(`\nTest: ${test.name}`);
    console.log('-'.repeat(40));
    
    const marketData = generateMarketData(test.scenario);
    const result = await integration.processMarketData(marketData);
    
    console.log(`Market condition: ${result.analysis.condition}`);
    console.log(`Cognitive pattern: ${result.analysis.pattern.name}`);
    console.log(`Pattern confidence: ${(result.analysis.confidence * 100).toFixed(1)}%`);
    
    console.log('\nTrading parameters:');
    console.log(`  Position size: ${result.tradingParams.positionSize.toFixed(2)}`);
    console.log(`  Stop loss: ${(result.tradingParams.stopLoss * 100).toFixed(1)}%`);
    console.log(`  Take profit: ${(result.tradingParams.takeProfit * 100).toFixed(1)}%`);
    console.log(`  Entry threshold: ${result.tradingParams.entryThreshold.toFixed(2)}`);
    
    if (result.signals.length > 0) {
      console.log('\nGenerated signals:');
      for (const signal of result.signals) {
        console.log(`  ${signal.action} - Strength: ${signal.strength.toFixed(2)}, Size: ${signal.positionSize.toFixed(2)}`);
      }
    } else {
      console.log('\nNo trading signals generated');
    }
  }
  
  // Test trade execution and learning
  console.log('\n\nTest: Trade Execution and Learning');
  console.log('-'.repeat(40));
  
  const marketData = generateMarketData('trending');
  const { signals } = await integration.processMarketData(marketData);
  
  if (signals.length > 0) {
    const signal = signals[0];
    const trade = await integration.executeTrade(signal, marketData);
    
    console.log(`\nExecuted trade: ${trade.id}`);
    console.log(`Direction: ${signal.action}`);
    console.log(`Entry price: ${trade.entryPrice.toFixed(2)}`);
    console.log(`Pattern: ${trade.pattern}`);
    
    // Simulate trade result
    const profit = Math.random() > 0.5 ? 0.02 : -0.01;
    await integration.updateTradeResult(trade.id, {
      exitPrice: trade.entryPrice * (1 + profit),
      profit
    });
    
    console.log(`\nTrade result: ${profit > 0 ? 'WIN' : 'LOSS'} (${(profit * 100).toFixed(1)}%)`);
  }
  
  // Show final performance
  console.log('\n\nFinal Performance Metrics:');
  console.log('-'.repeat(40));
  
  const performance = integration.getPerformanceMetrics();
  console.log(`Total trades: ${performance.totalTrades}`);
  console.log(`Win rate: ${(performance.winRate * 100).toFixed(1)}%`);
  console.log(`Average return: ${(performance.avgReturn * 100).toFixed(2)}%`);
  
  console.log('\nPattern performance:');
  for (const [pattern, metrics] of Object.entries(performance.patternMetrics)) {
    if (metrics.totalTrades > 0) {
      console.log(`  ${pattern}: ${(metrics.successRate * 100).toFixed(1)}% success rate`);
    }
  }
}

// Run tests
async function runTests() {
  try {
    await testNeuralCoordinator();
    await testNeuralTradingIntegration();
    
    console.log('\n\n✅ All tests completed successfully!');
  } catch (error) {
    console.error('❌ Test failed:', error);
  }
}

// Execute tests if running directly
if (import.meta.url === `file://${process.argv[1]}`) {
  runTests();
}

export { testNeuralCoordinator, testNeuralTradingIntegration, generateMarketData };