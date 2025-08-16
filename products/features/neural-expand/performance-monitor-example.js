/**
 * Performance Monitor Integration Example
 * 
 * This example demonstrates how to integrate the performance monitor
 * with the Neural Trading System for comprehensive monitoring.
 */

import { createPerformanceMonitor, SimdOperations } from './performance-monitor.js';
import { EventEmitter } from 'events';

/**
 * Neural Trading System with integrated performance monitoring
 */
class NeuralTradingSystem extends EventEmitter {
    constructor(config = {}) {
        super();
        
        // Initialize performance monitor
        this.monitor = createPerformanceMonitor({
            samplingInterval: 1000, // 1 second
            alertThresholds: {
                cpuUsage: 75,
                memoryUsage: 80,
                latencyP95: 50, // 50ms for trading operations
                throughput: 200, // 200 ops/sec minimum
                tokenConsumption: 500000 // 500k tokens/hour budget
            },
            wasmSimdEnabled: true // Enable SIMD optimizations
        });
        
        // Set up event handlers
        this.setupMonitoringHandlers();
        
        // Simulated components
        this.dataIngestion = new DataIngestionSimulator(this.monitor);
        this.neuralPredictor = new NeuralPredictorSimulator(this.monitor);
        this.tradingEngine = new TradingEngineSimulator(this.monitor);
        this.agentCoordinator = new AgentCoordinatorSimulator(this.monitor);
    }
    
    /**
     * Set up monitoring event handlers
     */
    setupMonitoringHandlers() {
        // Handle performance metrics
        this.monitor.on('metrics', (metrics) => {
            console.log(`[METRICS] CPU: ${metrics.system.cpuUsage.toFixed(1)}%, Memory: ${metrics.system.memoryUsage.toFixed(1)}%`);
            
            // Log detailed metrics periodically
            if (Date.now() % 10000 < 1000) { // Every 10 seconds
                console.log('[METRICS] Detailed Report:');
                console.log(`  - Latency P95: ${metrics.latency.p95?.toFixed(1)}ms`);
                console.log(`  - Throughput: ${metrics.throughput.total?.toFixed(1)} ops/sec`);
                console.log(`  - Active Agents: ${metrics.agents.activeAgents}`);
                console.log(`  - Token Rate: ${metrics.tokens.hourlyRate} tokens/hour`);
            }
        });
        
        // Handle alerts
        this.monitor.on('alert', (alert) => {
            console.error(`[ALERT] ${alert.severity.toUpperCase()}: ${alert.message}`);
            
            // Take action based on alert type
            switch (alert.type) {
                case 'cpu':
                    this.handleHighCPU();
                    break;
                case 'memory':
                    this.handleHighMemory();
                    break;
                case 'latency':
                    this.handleHighLatency();
                    break;
            }
        });
        
        // Handle bottlenecks
        this.monitor.on('bottlenecks', (bottlenecks) => {
            console.warn('[BOTTLENECK] Detected performance issues:');
            bottlenecks.forEach(b => {
                console.warn(`  - ${b.type}: ${b.description}`);
                console.warn(`    Recommendation: ${b.recommendation}`);
            });
        });
        
        // Handle resolved alerts
        this.monitor.on('alert-resolved', (alert) => {
            console.log(`[RESOLVED] ${alert.type} alert cleared`);
        });
    }
    
    /**
     * Start the trading system
     */
    async start() {
        console.log('Starting Neural Trading System with Performance Monitoring...');
        
        // Start all components
        await Promise.all([
            this.dataIngestion.start(),
            this.neuralPredictor.start(),
            this.tradingEngine.start(),
            this.agentCoordinator.start()
        ]);
        
        // Start performance dashboard
        this.startDashboard();
        
        console.log('System started successfully!');
    }
    
    /**
     * Start real-time dashboard
     */
    startDashboard() {
        setInterval(() => {
            const dashboard = this.monitor.getDashboardData();
            const summary = this.monitor.getPerformanceSummary();
            
            console.clear();
            console.log('=== NEURAL TRADING SYSTEM PERFORMANCE DASHBOARD ===');
            console.log(`Uptime: ${Math.floor(summary.uptime / 60)} minutes`);
            console.log('');
            
            // System Health
            console.log('SYSTEM HEALTH:');
            console.log(`  CPU Usage: ${dashboard.system?.cpuUsage.toFixed(1)}% (avg: ${summary.averages.cpuUsage.toFixed(1)}%)`);
            console.log(`  Memory Usage: ${dashboard.system?.memoryUsage.toFixed(1)}% (avg: ${summary.averages.memoryUsage.toFixed(1)}%)`);
            console.log(`  Load Average: ${dashboard.system?.loadAverage['1m'].toFixed(2)}`);
            console.log('');
            
            // Performance Metrics
            console.log('PERFORMANCE:');
            console.log(`  Latency (P95): ${dashboard.latency?.p95?.toFixed(1) || 'N/A'}ms`);
            console.log(`  Throughput: ${dashboard.throughput?.total?.toFixed(1) || 0} ops/sec`);
            console.log(`  Predictions/sec: ${summary.efficiency.predictionsPerSecond.toFixed(2)}`);
            console.log(`  Cache Hit Rate: ${(summary.efficiency.cacheHitRate * 100).toFixed(1)}%`);
            console.log('');
            
            // Neural Network
            console.log('NEURAL NETWORK:');
            console.log(`  Accuracy: ${(dashboard.neural?.predictions?.accuracy * 100).toFixed(1)}%`);
            console.log(`  Training Progress: ${dashboard.neural?.training?.progress?.improvement?.toFixed(1)}% improvement`);
            console.log(`  Token Usage: ${dashboard.tokens?.hourlyRate || 0} tokens/hour`);
            console.log('');
            
            // Agent Coordination
            console.log('AGENT COORDINATION:');
            console.log(`  Total Agents: ${dashboard.agents?.totalAgents || 0}`);
            console.log(`  Active Agents: ${dashboard.agents?.activeAgents || 0}`);
            console.log('');
            
            // Optimizations
            console.log('OPTIMIZATIONS:');
            console.log(`  WASM SIMD: ${summary.optimization.wasmSimdEnabled ? 'Enabled' : 'Disabled'}`);
            console.log(`  Parallel Agents: ${summary.optimization.parallelAgents}`);
            console.log(`  Active Bottlenecks: ${summary.optimization.bottleneckCount}`);
            console.log('');
            
            // Active Alerts
            if (dashboard.alerts.length > 0) {
                console.log('ACTIVE ALERTS:');
                dashboard.alerts.forEach(alert => {
                    console.log(`  [${alert.severity.toUpperCase()}] ${alert.message}`);
                });
                console.log('');
            }
            
            console.log('==================================================');
        }, 5000); // Update every 5 seconds
    }
    
    /**
     * Handle high CPU usage
     */
    handleHighCPU() {
        console.log('[MITIGATION] Reducing computational load...');
        // Implement CPU mitigation strategies
        this.neuralPredictor.reduceFrequency();
    }
    
    /**
     * Handle high memory usage
     */
    handleHighMemory() {
        console.log('[MITIGATION] Clearing caches and reducing memory usage...');
        // Implement memory mitigation strategies
        if (global.gc) {
            global.gc();
        }
    }
    
    /**
     * Handle high latency
     */
    handleHighLatency() {
        console.log('[MITIGATION] Optimizing processing pipeline...');
        // Implement latency mitigation strategies
        this.dataIngestion.enableBatching();
    }
}

/**
 * Simulated Data Ingestion Component
 */
class DataIngestionSimulator {
    constructor(monitor) {
        this.monitor = monitor;
        this.batchingEnabled = false;
    }
    
    async start() {
        setInterval(() => {
            const startTime = performance.now();
            
            // Simulate data ingestion
            const dataPoints = this.batchingEnabled ? 100 : 10;
            
            // Record metrics
            const duration = performance.now() - startTime + Math.random() * 5;
            this.monitor.recordLatency('data_ingestion', duration);
            this.monitor.recordThroughput('data_ingestion', dataPoints);
        }, 100);
    }
    
    enableBatching() {
        this.batchingEnabled = true;
    }
}

/**
 * Simulated Neural Predictor Component
 */
class NeuralPredictorSimulator {
    constructor(monitor) {
        this.monitor = monitor;
        this.frequency = 100; // ms
    }
    
    async start() {
        this.predictionInterval = setInterval(async () => {
            const startTime = performance.now();
            
            // Simulate neural prediction
            if (SimdOperations.isAvailable()) {
                // Use SIMD optimized operations
                const input = new Float32Array(1000).fill(Math.random());
                const weights = new Float32Array(1000).fill(Math.random());
                await SimdOperations.vectorAdd(input, weights);
            }
            
            // Record metrics
            const duration = performance.now() - startTime + Math.random() * 20;
            this.monitor.recordLatency('neural_prediction', duration);
            this.monitor.recordThroughput('neural_prediction', 1);
            
            // Simulate token consumption
            this.monitor.recordTokens('gpt-4', Math.floor(Math.random() * 1000));
            
            // Record neural metrics
            this.monitor.recordNeuralMetrics({
                type: 'prediction',
                accurate: Math.random() > 0.2,
                confidence: Math.random()
            });
        }, this.frequency);
    }
    
    reduceFrequency() {
        this.frequency = Math.min(this.frequency * 1.5, 1000);
        clearInterval(this.predictionInterval);
        this.start();
    }
}

/**
 * Simulated Trading Engine Component
 */
class TradingEngineSimulator {
    constructor(monitor) {
        this.monitor = monitor;
    }
    
    async start() {
        setInterval(() => {
            const startTime = performance.now();
            
            // Simulate trade execution
            const tradeCount = Math.floor(Math.random() * 5);
            
            // Record metrics
            const duration = performance.now() - startTime + Math.random() * 10;
            this.monitor.recordLatency('trade_execution', duration);
            this.monitor.recordThroughput('trade_execution', tradeCount);
        }, 500);
    }
}

/**
 * Simulated Agent Coordinator Component
 */
class AgentCoordinatorSimulator {
    constructor(monitor) {
        this.monitor = monitor;
        this.agents = [];
    }
    
    async start() {
        // Create simulated agents
        for (let i = 0; i < 5; i++) {
            const agentId = `agent_${i}`;
            this.agents.push(agentId);
            
            // Simulate agent activity
            setInterval(() => {
                this.monitor.recordAgentActivity(agentId, {
                    type: 'processing',
                    task: `task_${Math.floor(Math.random() * 10)}`
                });
            }, 1000 + Math.random() * 2000);
        }
    }
}

// Start the example system
async function main() {
    const system = new NeuralTradingSystem();
    
    // Handle graceful shutdown
    process.on('SIGINT', () => {
        console.log('\nShutting down gracefully...');
        system.monitor.stopMonitoring();
        process.exit(0);
    });
    
    // Start the system
    await system.start();
}

// Run if this is the main module
if (import.meta.url === `file://${process.argv[1]}`) {
    main().catch(console.error);
}

export { NeuralTradingSystem };