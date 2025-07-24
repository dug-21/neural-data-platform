/**
 * Swarm Neural Integration
 * Connects the Neural Coordinator with the existing ruv-swarm system
 */

import { NeuralCoordinator } from './neural-coordinator.js';
import { EventEmitter } from 'events';

/**
 * Bridge between Neural Coordinator and Swarm System
 */
class SwarmNeuralIntegration extends EventEmitter {
  constructor(swarmCoordinator, config = {}) {
    super();
    
    this.swarmCoordinator = swarmCoordinator;
    this.neuralCoordinator = new NeuralCoordinator(config);
    
    // Track agent-pattern mappings
    this.agentPatternMap = new Map();
    this.patternAgentGroups = new Map();
    
    // Performance tracking by pattern-agent combination
    this.patternAgentPerformance = new Map();
    
    // Setup integration
    this.setupIntegration();
    
    console.log('Swarm Neural Integration initialized');
  }

  /**
   * Setup bidirectional integration between systems
   */
  setupIntegration() {
    // Listen to neural coordinator events
    this.neuralCoordinator.on('patternSwitch', (data) => {
      this.handleNeuralPatternSwitch(data);
    });
    
    this.neuralCoordinator.on('marketAnalysis', (data) => {
      this.updateSwarmStrategy(data);
    });
    
    // Listen to swarm events (if available)
    if (this.swarmCoordinator) {
      this.swarmCoordinator.on('agentSpawned', (agent) => {
        this.assignNeuralPattern(agent);
      });
      
      this.swarmCoordinator.on('taskCompleted', (result) => {
        this.updateNeuralLearning(result);
      });
    }
  }

  /**
   * Create neural-enhanced swarm session
   */
  async createNeuralSwarmSession(task, marketData) {
    try {
      // Analyze market conditions first
      const marketAnalysis = await this.neuralCoordinator.analyzeMarketCondition(marketData);
      
      // Get coordination recommendations
      const recommendations = await this.neuralCoordinator.getCoordinationRecommendations();
      
      // Initialize swarm with recommended configuration
      const swarmConfig = this.buildSwarmConfig(recommendations, marketAnalysis);
      
      // Create swarm agents with neural patterns
      const agents = await this.spawnNeuralAgents(recommendations.agentAllocation);
      
      // Create coordination session
      const neuralSession = await this.neuralCoordinator.createCoordinationSession(
        agents,
        task
      );
      
      // Return integrated session
      return {
        swarmId: swarmConfig.swarmId,
        neuralSessionId: neuralSession.id,
        pattern: marketAnalysis.pattern,
        marketCondition: marketAnalysis.condition,
        agents,
        config: swarmConfig,
        recommendations
      };
      
    } catch (error) {
      console.error('Error creating neural swarm session:', error);
      throw error;
    }
  }

  /**
   * Build swarm configuration based on neural recommendations
   */
  buildSwarmConfig(recommendations, marketAnalysis) {
    const config = {
      swarmId: `swarm_${Date.now()}`,
      topology: this.selectTopology(marketAnalysis.pattern),
      maxAgents: Object.values(recommendations.agentAllocation).reduce((a, b) => a + b, 0),
      strategy: this.mapPatternToStrategy(marketAnalysis.pattern),
      riskLevel: recommendations.riskLevel,
      coordinationPattern: marketAnalysis.pattern.name
    };
    
    return config;
  }

  /**
   * Select swarm topology based on cognitive pattern
   */
  selectTopology(pattern) {
    const topologyMap = {
      'convergent': 'hierarchical',  // Clear leadership for focused execution
      'divergent': 'mesh',           // Full connectivity for exploration
      'lateral': 'ring',             // Sequential for unique perspectives
      'systems': 'mesh',             // Full connectivity for holistic view
      'critical': 'star',            // Centralized for validation
      'adaptive': 'hierarchical'     // Flexible with clear coordination
    };
    
    return topologyMap[pattern.name] || 'mesh';
  }

  /**
   * Map cognitive pattern to swarm strategy
   */
  mapPatternToStrategy(pattern) {
    const strategyMap = {
      'convergent': 'sequential',    // Step-by-step execution
      'divergent': 'parallel',       // Explore multiple paths
      'lateral': 'adaptive',         // Flexible approach
      'systems': 'parallel',         // Multi-aspect analysis
      'critical': 'sequential',      // Careful validation
      'adaptive': 'adaptive'         // Dynamic adjustment
    };
    
    return strategyMap[pattern.name] || 'adaptive';
  }

  /**
   * Spawn agents with neural pattern assignments
   */
  async spawnNeuralAgents(agentAllocation) {
    const agents = [];
    
    for (const [agentType, count] of Object.entries(agentAllocation)) {
      for (let i = 0; i < count; i++) {
        const agent = {
          id: `${agentType}_${i + 1}_${Date.now()}`,
          type: agentType,
          status: 'active',
          neuralConfig: this.getNeuralConfigForType(agentType)
        };
        
        agents.push(agent);
        
        // Spawn in actual swarm if available
        if (this.swarmCoordinator?.spawnAgent) {
          await this.swarmCoordinator.spawnAgent({
            type: agentType,
            capabilities: this.getAgentCapabilities(agentType),
            metadata: {
              neuralPattern: agent.neuralConfig.pattern
            }
          });
        }
      }
    }
    
    return agents;
  }

  /**
   * Get neural configuration for agent type
   */
  getNeuralConfigForType(agentType) {
    const configs = {
      'researcher': {
        pattern: 'divergent',
        explorationRate: 0.8,
        learningSpeed: 0.7,
        riskTolerance: 0.6
      },
      'analyst': {
        pattern: 'critical',
        explorationRate: 0.3,
        learningSpeed: 0.5,
        riskTolerance: 0.2
      },
      'coder': {
        pattern: 'convergent',
        explorationRate: 0.2,
        learningSpeed: 0.6,
        riskTolerance: 0.3
      },
      'optimizer': {
        pattern: 'systems',
        explorationRate: 0.5,
        learningSpeed: 0.4,
        riskTolerance: 0.4
      },
      'coordinator': {
        pattern: 'adaptive',
        explorationRate: 0.5,
        learningSpeed: 0.8,
        riskTolerance: 0.5
      }
    };
    
    return configs[agentType] || configs['coordinator'];
  }

  /**
   * Get agent capabilities based on type
   */
  getAgentCapabilities(agentType) {
    const capabilities = {
      'researcher': ['data_analysis', 'pattern_discovery', 'hypothesis_generation'],
      'analyst': ['risk_assessment', 'validation', 'performance_analysis'],
      'coder': ['implementation', 'optimization', 'testing'],
      'optimizer': ['performance_tuning', 'resource_allocation', 'efficiency'],
      'coordinator': ['task_distribution', 'progress_tracking', 'decision_making']
    };
    
    return capabilities[agentType] || ['general_processing'];
  }

  /**
   * Handle neural pattern switch events
   */
  async handleNeuralPatternSwitch(data) {
    console.log(`Neural pattern switched from ${data.from} to ${data.to}`);
    
    // Update swarm topology if needed
    const newTopology = this.selectTopology({ name: data.to });
    
    if (this.swarmCoordinator?.updateTopology) {
      await this.swarmCoordinator.updateTopology(newTopology);
    }
    
    // Reassign agent behaviors
    for (const [agentId, pattern] of this.agentPatternMap) {
      if (pattern === data.from) {
        // Update agents that were using the old pattern
        this.updateAgentPattern(agentId, data.to);
      }
    }
    
    this.emit('neuralPatternUpdated', {
      from: data.from,
      to: data.to,
      affectedAgents: this.getAffectedAgents(data.from)
    });
  }

  /**
   * Update swarm strategy based on market analysis
   */
  async updateSwarmStrategy(marketAnalysis) {
    const newStrategy = this.mapPatternToStrategy(marketAnalysis.selectedPattern);
    
    if (this.swarmCoordinator?.updateStrategy) {
      await this.swarmCoordinator.updateStrategy(newStrategy);
    }
    
    // Update agent priorities based on market condition
    this.updateAgentPriorities(marketAnalysis.condition);
    
    this.emit('strategyUpdated', {
      pattern: marketAnalysis.selectedPattern,
      condition: marketAnalysis.condition,
      strategy: newStrategy
    });
  }

  /**
   * Assign neural pattern to newly spawned agent
   */
  async assignNeuralPattern(agent) {
    const pattern = this.getNeuralConfigForType(agent.type).pattern;
    
    this.agentPatternMap.set(agent.id, pattern);
    
    // Group agents by pattern
    if (!this.patternAgentGroups.has(pattern)) {
      this.patternAgentGroups.set(pattern, new Set());
    }
    this.patternAgentGroups.get(pattern).add(agent.id);
    
    // Initialize performance tracking
    const perfKey = `${pattern}_${agent.type}`;
    if (!this.patternAgentPerformance.has(perfKey)) {
      this.patternAgentPerformance.set(perfKey, {
        tasks: 0,
        successes: 0,
        totalTime: 0,
        avgQuality: 0
      });
    }
    
    console.log(`Assigned ${pattern} pattern to agent ${agent.id} (${agent.type})`);
  }

  /**
   * Update neural learning from swarm task results
   */
  async updateNeuralLearning(taskResult) {
    if (!taskResult.agentId) return;
    
    const pattern = this.agentPatternMap.get(taskResult.agentId);
    if (!pattern) return;
    
    // Convert swarm result to neural learning format
    const learningData = {
      pattern,
      success: taskResult.success || taskResult.status === 'completed',
      returnValue: taskResult.quality || taskResult.score || 0,
      responseTime: taskResult.duration || taskResult.executionTime || 1000,
      marketCondition: this.neuralCoordinator.marketCondition
    };
    
    // Update neural coordinator learning
    await this.neuralCoordinator.updatePatternLearning(learningData);
    
    // Update local performance tracking
    const agent = taskResult.agent || { type: 'unknown' };
    const perfKey = `${pattern}_${agent.type}`;
    const perf = this.patternAgentPerformance.get(perfKey);
    
    if (perf) {
      perf.tasks++;
      if (learningData.success) perf.successes++;
      perf.totalTime += learningData.responseTime;
      perf.avgQuality = (perf.avgQuality * (perf.tasks - 1) + learningData.returnValue) / perf.tasks;
    }
  }

  /**
   * Update agent pattern dynamically
   */
  updateAgentPattern(agentId, newPattern) {
    const oldPattern = this.agentPatternMap.get(agentId);
    
    // Update mapping
    this.agentPatternMap.set(agentId, newPattern);
    
    // Update groups
    if (oldPattern && this.patternAgentGroups.has(oldPattern)) {
      this.patternAgentGroups.get(oldPattern).delete(agentId);
    }
    
    if (!this.patternAgentGroups.has(newPattern)) {
      this.patternAgentGroups.set(newPattern, new Set());
    }
    this.patternAgentGroups.get(newPattern).add(agentId);
    
    // Notify agent if possible
    if (this.swarmCoordinator?.updateAgentBehavior) {
      this.swarmCoordinator.updateAgentBehavior(agentId, {
        pattern: newPattern,
        config: this.getNeuralConfigForPattern(newPattern)
      });
    }
  }

  /**
   * Get neural configuration for a specific pattern
   */
  getNeuralConfigForPattern(pattern) {
    const configs = {
      'convergent': { focus: 0.9, exploration: 0.1, speed: 0.3 },
      'divergent': { focus: 0.3, exploration: 0.9, speed: 0.7 },
      'lateral': { focus: 0.5, exploration: 0.7, speed: 0.6 },
      'systems': { focus: 0.7, exploration: 0.5, speed: 0.4 },
      'critical': { focus: 0.95, exploration: 0.05, speed: 0.2 },
      'adaptive': { focus: 0.6, exploration: 0.6, speed: 0.5 }
    };
    
    return configs[pattern] || configs['adaptive'];
  }

  /**
   * Update agent priorities based on market condition
   */
  updateAgentPriorities(marketCondition) {
    const priorityMap = {
      'TRENDING': {
        'analyst': 0.9,
        'optimizer': 0.8,
        'coder': 0.7,
        'researcher': 0.5,
        'coordinator': 0.6
      },
      'RANGING': {
        'researcher': 0.9,
        'analyst': 0.6,
        'coder': 0.8,
        'optimizer': 0.5,
        'coordinator': 0.7
      },
      'HIGH_VOLATILITY': {
        'analyst': 0.95,
        'coordinator': 0.9,
        'optimizer': 0.7,
        'researcher': 0.6,
        'coder': 0.5
      },
      'REVERSAL': {
        'researcher': 0.8,
        'analyst': 0.9,
        'coordinator': 0.8,
        'coder': 0.6,
        'optimizer': 0.6
      },
      'BREAKOUT': {
        'coder': 0.9,
        'optimizer': 0.85,
        'analyst': 0.7,
        'researcher': 0.8,
        'coordinator': 0.75
      }
    };
    
    const priorities = priorityMap[marketCondition] || priorityMap['RANGING'];
    
    // Apply priorities to swarm if possible
    if (this.swarmCoordinator?.updateAgentPriorities) {
      this.swarmCoordinator.updateAgentPriorities(priorities);
    }
  }

  /**
   * Get agents affected by pattern change
   */
  getAffectedAgents(pattern) {
    return Array.from(this.patternAgentGroups.get(pattern) || []);
  }

  /**
   * Get integration performance metrics
   */
  getPerformanceMetrics() {
    const metrics = {
      neuralMetrics: this.neuralCoordinator.getPatternMetrics(),
      patternAgentPerformance: Object.fromEntries(this.patternAgentPerformance),
      currentState: {
        pattern: this.neuralCoordinator.currentPattern.name,
        marketCondition: this.neuralCoordinator.marketCondition,
        activeAgents: this.agentPatternMap.size,
        patternDistribution: {}
      }
    };
    
    // Calculate pattern distribution
    for (const pattern of this.agentPatternMap.values()) {
      metrics.currentState.patternDistribution[pattern] = 
        (metrics.currentState.patternDistribution[pattern] || 0) + 1;
    }
    
    return metrics;
  }

  /**
   * Get recommended swarm configuration for current market
   */
  async getOptimalSwarmConfig(marketData) {
    const analysis = await this.neuralCoordinator.analyzeMarketCondition(marketData);
    const recommendations = await this.neuralCoordinator.getCoordinationRecommendations();
    
    return {
      topology: this.selectTopology(analysis.pattern),
      strategy: this.mapPatternToStrategy(analysis.pattern),
      agentAllocation: recommendations.agentAllocation,
      riskLevel: recommendations.riskLevel,
      pattern: analysis.pattern.name,
      confidence: analysis.confidence,
      recommendedStrategies: recommendations.strategies
    };
  }
}

export { SwarmNeuralIntegration };