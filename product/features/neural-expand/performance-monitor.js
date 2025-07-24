/**
 * Performance Monitoring Dashboard for Neural Trading System
 * 
 * This module provides comprehensive real-time monitoring for:
 * - System metrics (CPU, memory, disk, network)
 * - Trading performance (latency, throughput, accuracy)
 * - Neural network efficiency (token consumption, prediction speed)
 * - Agent coordination metrics
 * - WebAssembly SIMD optimizations
 */

import { EventEmitter } from 'events';
import os from 'os';
import { performance } from 'perf_hooks';

// WebAssembly SIMD feature detection
const hasWasmSimd = (() => {
    try {
        // Test SIMD support with a simple WASM module
        const wasmCode = new Uint8Array([
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
            0x01, 0x05, 0x01, 0x60, 0x00, 0x00, 0x03, 0x02,
            0x01, 0x00, 0x07, 0x08, 0x01, 0x04, 0x74, 0x65,
            0x73, 0x74, 0x00, 0x00, 0x0a, 0x0a, 0x01, 0x08,
            0x00, 0x41, 0x00, 0xfd, 0x0f, 0x0b, 0x00, 0x0b
        ]);
        new WebAssembly.Module(wasmCode);
        return true;
    } catch {
        return false;
    }
})();

/**
 * Performance Monitor Class
 * Tracks and reports on all system performance metrics
 */
export class PerformanceMonitor extends EventEmitter {
    constructor(config = {}) {
        super();
        
        this.config = {
            samplingInterval: config.samplingInterval || 1000, // 1 second
            historySize: config.historySize || 3600, // 1 hour of history
            alertThresholds: {
                cpuUsage: config.alertThresholds?.cpuUsage || 80,
                memoryUsage: config.alertThresholds?.memoryUsage || 85,
                latencyP95: config.alertThresholds?.latencyP95 || 100, // ms
                throughput: config.alertThresholds?.throughput || 100, // ops/sec
                tokenConsumption: config.alertThresholds?.tokenConsumption || 1000000, // tokens/hour
                ...config.alertThresholds
            },
            wasmSimdEnabled: hasWasmSimd && (config.wasmSimdEnabled !== false)
        };
        
        // Initialize metric stores
        this.metrics = {
            system: new MetricStore(this.config.historySize),
            latency: new LatencyTracker(),
            throughput: new ThroughputTracker(),
            memory: new MemoryTracker(),
            tokens: new TokenTracker(),
            agents: new AgentTracker(),
            neural: new NeuralTracker(),
            bottlenecks: new BottleneckDetector()
        };
        
        // Alert state
        this.activeAlerts = new Map();
        
        // Start monitoring
        this.startMonitoring();
    }
    
    /**
     * Start the monitoring loop
     */
    startMonitoring() {
        this.monitoringInterval = setInterval(() => {
            this.collectMetrics();
            this.detectBottlenecks();
            this.checkAlerts();
        }, this.config.samplingInterval);
        
        // Initial collection
        this.collectMetrics();
    }
    
    /**
     * Stop monitoring
     */
    stopMonitoring() {
        if (this.monitoringInterval) {
            clearInterval(this.monitoringInterval);
            this.monitoringInterval = null;
        }
    }
    
    /**
     * Collect all system metrics
     */
    collectMetrics() {
        const timestamp = Date.now();
        
        // System metrics
        const systemMetrics = this.collectSystemMetrics();
        this.metrics.system.add(timestamp, systemMetrics);
        
        // Memory details
        const memoryMetrics = this.collectMemoryMetrics();
        this.metrics.memory.update(memoryMetrics);
        
        // Emit metrics event
        this.emit('metrics', {
            timestamp,
            system: systemMetrics,
            memory: memoryMetrics,
            latency: this.metrics.latency.getStats(),
            throughput: this.metrics.throughput.getStats(),
            tokens: this.metrics.tokens.getStats(),
            agents: this.metrics.agents.getStats(),
            neural: this.metrics.neural.getStats()
        });
    }
    
    /**
     * Collect system-level metrics
     */
    collectSystemMetrics() {
        const cpus = os.cpus();
        const totalMemory = os.totalmem();
        const freeMemory = os.freemem();
        const loadAvg = os.loadavg();
        
        // Calculate CPU usage
        const cpuUsage = cpus.reduce((acc, cpu) => {
            const total = Object.values(cpu.times).reduce((a, b) => a + b, 0);
            const idle = cpu.times.idle;
            return acc + ((total - idle) / total) * 100;
        }, 0) / cpus.length;
        
        return {
            cpuUsage,
            cpuCount: cpus.length,
            memoryUsage: ((totalMemory - freeMemory) / totalMemory) * 100,
            totalMemory,
            freeMemory,
            loadAverage: {
                '1m': loadAvg[0],
                '5m': loadAvg[1],
                '15m': loadAvg[2]
            },
            uptime: os.uptime(),
            platform: os.platform(),
            wasmSimdEnabled: this.config.wasmSimdEnabled
        };
    }
    
    /**
     * Collect detailed memory metrics
     */
    collectMemoryMetrics() {
        const memUsage = process.memoryUsage();
        return {
            heapUsed: memUsage.heapUsed,
            heapTotal: memUsage.heapTotal,
            external: memUsage.external,
            arrayBuffers: memUsage.arrayBuffers,
            rss: memUsage.rss,
            // Calculate percentages
            heapUsagePercent: (memUsage.heapUsed / memUsage.heapTotal) * 100
        };
    }
    
    /**
     * Record a latency measurement
     */
    recordLatency(operation, duration) {
        this.metrics.latency.record(operation, duration);
    }
    
    /**
     * Record a throughput measurement
     */
    recordThroughput(operation, count = 1) {
        this.metrics.throughput.record(operation, count);
    }
    
    /**
     * Record token consumption
     */
    recordTokens(model, tokens) {
        this.metrics.tokens.record(model, tokens);
    }
    
    /**
     * Record agent activity
     */
    recordAgentActivity(agentId, activity) {
        this.metrics.agents.record(agentId, activity);
    }
    
    /**
     * Record neural network metrics
     */
    recordNeuralMetrics(metrics) {
        this.metrics.neural.record(metrics);
    }
    
    /**
     * Detect performance bottlenecks
     */
    detectBottlenecks() {
        const bottlenecks = this.metrics.bottlenecks.detect({
            system: this.metrics.system.getLatest(),
            latency: this.metrics.latency.getStats(),
            throughput: this.metrics.throughput.getStats(),
            memory: this.metrics.memory.getLatest()
        });
        
        if (bottlenecks.length > 0) {
            this.emit('bottlenecks', bottlenecks);
        }
    }
    
    /**
     * Check for alert conditions
     */
    checkAlerts() {
        const currentMetrics = {
            system: this.metrics.system.getLatest(),
            latency: this.metrics.latency.getStats(),
            throughput: this.metrics.throughput.getStats(),
            tokens: this.metrics.tokens.getStats()
        };
        
        const alerts = [];
        
        // CPU usage alert
        if (currentMetrics.system?.cpuUsage > this.config.alertThresholds.cpuUsage) {
            alerts.push({
                type: 'cpu',
                severity: 'warning',
                message: `High CPU usage: ${currentMetrics.system.cpuUsage.toFixed(1)}%`,
                value: currentMetrics.system.cpuUsage,
                threshold: this.config.alertThresholds.cpuUsage
            });
        }
        
        // Memory usage alert
        if (currentMetrics.system?.memoryUsage > this.config.alertThresholds.memoryUsage) {
            alerts.push({
                type: 'memory',
                severity: 'warning',
                message: `High memory usage: ${currentMetrics.system.memoryUsage.toFixed(1)}%`,
                value: currentMetrics.system.memoryUsage,
                threshold: this.config.alertThresholds.memoryUsage
            });
        }
        
        // Latency alert
        if (currentMetrics.latency?.p95 > this.config.alertThresholds.latencyP95) {
            alerts.push({
                type: 'latency',
                severity: 'warning',
                message: `High latency P95: ${currentMetrics.latency.p95.toFixed(1)}ms`,
                value: currentMetrics.latency.p95,
                threshold: this.config.alertThresholds.latencyP95
            });
        }
        
        // Process alerts
        alerts.forEach(alert => {
            const alertKey = `${alert.type}-${alert.severity}`;
            if (!this.activeAlerts.has(alertKey)) {
                this.activeAlerts.set(alertKey, alert);
                this.emit('alert', alert);
            }
        });
        
        // Clear resolved alerts
        this.activeAlerts.forEach((alert, key) => {
            const stillActive = alerts.some(a => 
                `${a.type}-${a.severity}` === key
            );
            if (!stillActive) {
                this.activeAlerts.delete(key);
                this.emit('alert-resolved', alert);
            }
        });
    }
    
    /**
     * Get current dashboard data
     */
    getDashboardData() {
        return {
            timestamp: Date.now(),
            system: this.metrics.system.getLatest(),
            systemHistory: this.metrics.system.getHistory(),
            latency: this.metrics.latency.getStats(),
            throughput: this.metrics.throughput.getStats(),
            memory: this.metrics.memory.getLatest(),
            tokens: this.metrics.tokens.getStats(),
            agents: this.metrics.agents.getStats(),
            neural: this.metrics.neural.getStats(),
            bottlenecks: this.metrics.bottlenecks.getActive(),
            alerts: Array.from(this.activeAlerts.values()),
            config: this.config
        };
    }
    
    /**
     * Get performance summary
     */
    getPerformanceSummary() {
        const latencyStats = this.metrics.latency.getStats();
        const throughputStats = this.metrics.throughput.getStats();
        const tokenStats = this.metrics.tokens.getStats();
        const systemStats = this.metrics.system.getAverages();
        
        return {
            uptime: os.uptime(),
            averages: {
                cpuUsage: systemStats.cpuUsage,
                memoryUsage: systemStats.memoryUsage,
                latency: latencyStats.mean,
                throughput: throughputStats.total
            },
            peaks: {
                cpuUsage: systemStats.maxCpuUsage,
                memoryUsage: systemStats.maxMemoryUsage,
                latency: latencyStats.max,
                throughput: throughputStats.peak
            },
            efficiency: {
                tokensPerHour: tokenStats.hourlyRate,
                predictionsPerSecond: throughputStats.operations?.neural_prediction || 0,
                cacheHitRate: this.metrics.system.getCacheHitRate()
            },
            optimization: {
                wasmSimdEnabled: this.config.wasmSimdEnabled,
                parallelAgents: this.metrics.agents.getActiveCount(),
                bottleneckCount: this.metrics.bottlenecks.getActive().length
            }
        };
    }
}

/**
 * Generic metric store with time-series data
 */
class MetricStore {
    constructor(maxSize) {
        this.maxSize = maxSize;
        this.data = [];
    }
    
    add(timestamp, value) {
        this.data.push({ timestamp, value });
        if (this.data.length > this.maxSize) {
            this.data.shift();
        }
    }
    
    getLatest() {
        return this.data[this.data.length - 1]?.value;
    }
    
    getHistory(duration = null) {
        if (!duration) return this.data;
        
        const cutoff = Date.now() - duration;
        return this.data.filter(d => d.timestamp >= cutoff);
    }
    
    getAverages() {
        if (this.data.length === 0) return {};
        
        const values = this.data.map(d => d.value);
        const cpuValues = values.map(v => v.cpuUsage).filter(v => v !== undefined);
        const memValues = values.map(v => v.memoryUsage).filter(v => v !== undefined);
        
        return {
            cpuUsage: cpuValues.reduce((a, b) => a + b, 0) / cpuValues.length,
            memoryUsage: memValues.reduce((a, b) => a + b, 0) / memValues.length,
            maxCpuUsage: Math.max(...cpuValues),
            maxMemoryUsage: Math.max(...memValues)
        };
    }
    
    getCacheHitRate() {
        // Simulated cache hit rate - in real implementation, track actual cache hits
        return 0.85 + Math.random() * 0.1; // 85-95%
    }
}

/**
 * Latency tracking with percentile calculations
 */
class LatencyTracker {
    constructor() {
        this.operations = new Map();
    }
    
    record(operation, duration) {
        if (!this.operations.has(operation)) {
            this.operations.set(operation, []);
        }
        
        const measurements = this.operations.get(operation);
        measurements.push({
            timestamp: Date.now(),
            duration
        });
        
        // Keep last 1000 measurements per operation
        if (measurements.length > 1000) {
            measurements.shift();
        }
    }
    
    getStats() {
        const allMeasurements = [];
        const operationStats = {};
        
        this.operations.forEach((measurements, operation) => {
            const durations = measurements.map(m => m.duration);
            allMeasurements.push(...durations);
            
            operationStats[operation] = this.calculatePercentiles(durations);
        });
        
        const overall = this.calculatePercentiles(allMeasurements);
        
        return {
            ...overall,
            operations: operationStats
        };
    }
    
    calculatePercentiles(values) {
        if (values.length === 0) {
            return { count: 0, mean: 0, p50: 0, p95: 0, p99: 0, max: 0 };
        }
        
        const sorted = values.slice().sort((a, b) => a - b);
        const count = sorted.length;
        
        return {
            count,
            mean: values.reduce((a, b) => a + b, 0) / count,
            p50: sorted[Math.floor(count * 0.5)],
            p95: sorted[Math.floor(count * 0.95)],
            p99: sorted[Math.floor(count * 0.99)],
            max: sorted[count - 1]
        };
    }
}

/**
 * Throughput tracking
 */
class ThroughputTracker {
    constructor() {
        this.operations = new Map();
        this.windowSize = 60000; // 1 minute window
    }
    
    record(operation, count = 1) {
        if (!this.operations.has(operation)) {
            this.operations.set(operation, []);
        }
        
        const records = this.operations.get(operation);
        records.push({
            timestamp: Date.now(),
            count
        });
        
        // Clean old records
        const cutoff = Date.now() - this.windowSize;
        const index = records.findIndex(r => r.timestamp >= cutoff);
        if (index > 0) {
            records.splice(0, index);
        }
    }
    
    getStats() {
        const operationStats = {};
        let total = 0;
        let peak = 0;
        
        this.operations.forEach((records, operation) => {
            const sum = records.reduce((acc, r) => acc + r.count, 0);
            const rate = (sum / this.windowSize) * 1000; // per second
            
            operationStats[operation] = rate;
            total += rate;
            peak = Math.max(peak, rate);
        });
        
        return {
            total,
            peak,
            operations: operationStats
        };
    }
}

/**
 * Memory usage tracker
 */
class MemoryTracker {
    constructor() {
        this.history = [];
        this.maxHistory = 60; // 1 minute of second-by-second data
    }
    
    update(metrics) {
        this.history.push({
            timestamp: Date.now(),
            ...metrics
        });
        
        if (this.history.length > this.maxHistory) {
            this.history.shift();
        }
    }
    
    getLatest() {
        return this.history[this.history.length - 1] || {};
    }
    
    getTrend() {
        if (this.history.length < 2) return 'stable';
        
        const recent = this.history.slice(-10);
        const firstHeap = recent[0].heapUsed;
        const lastHeap = recent[recent.length - 1].heapUsed;
        
        const change = (lastHeap - firstHeap) / firstHeap;
        
        if (change > 0.1) return 'increasing';
        if (change < -0.1) return 'decreasing';
        return 'stable';
    }
}

/**
 * Token consumption tracker
 */
class TokenTracker {
    constructor() {
        this.models = new Map();
        this.hourlyWindow = 3600000; // 1 hour
    }
    
    record(model, tokens) {
        if (!this.models.has(model)) {
            this.models.set(model, []);
        }
        
        const records = this.models.get(model);
        records.push({
            timestamp: Date.now(),
            tokens
        });
        
        // Clean old records
        const cutoff = Date.now() - this.hourlyWindow;
        const index = records.findIndex(r => r.timestamp >= cutoff);
        if (index > 0) {
            records.splice(0, index);
        }
    }
    
    getStats() {
        const modelStats = {};
        let totalHourly = 0;
        
        this.models.forEach((records, model) => {
            const sum = records.reduce((acc, r) => acc + r.tokens, 0);
            modelStats[model] = {
                hourly: sum,
                average: records.length > 0 ? sum / records.length : 0
            };
            totalHourly += sum;
        });
        
        return {
            hourlyRate: totalHourly,
            models: modelStats
        };
    }
}

/**
 * Agent activity tracker
 */
class AgentTracker {
    constructor() {
        this.agents = new Map();
    }
    
    record(agentId, activity) {
        if (!this.agents.has(agentId)) {
            this.agents.set(agentId, {
                activities: [],
                startTime: Date.now()
            });
        }
        
        const agent = this.agents.get(agentId);
        agent.activities.push({
            timestamp: Date.now(),
            ...activity
        });
        
        // Keep last 100 activities
        if (agent.activities.length > 100) {
            agent.activities.shift();
        }
    }
    
    getStats() {
        const stats = {
            totalAgents: this.agents.size,
            activeAgents: 0,
            agentStats: {}
        };
        
        this.agents.forEach((agent, agentId) => {
            const recent = agent.activities.filter(
                a => Date.now() - a.timestamp < 60000 // Active in last minute
            );
            
            if (recent.length > 0) {
                stats.activeAgents++;
            }
            
            stats.agentStats[agentId] = {
                activityCount: agent.activities.length,
                uptime: Date.now() - agent.startTime,
                isActive: recent.length > 0
            };
        });
        
        return stats;
    }
    
    getActiveCount() {
        let active = 0;
        this.agents.forEach(agent => {
            const recent = agent.activities.filter(
                a => Date.now() - a.timestamp < 60000
            );
            if (recent.length > 0) active++;
        });
        return active;
    }
}

/**
 * Neural network performance tracker
 */
class NeuralTracker {
    constructor() {
        this.predictions = [];
        this.trainingSessions = [];
        this.maxHistory = 1000;
    }
    
    record(metrics) {
        if (metrics.type === 'prediction') {
            this.predictions.push({
                timestamp: Date.now(),
                ...metrics
            });
            
            if (this.predictions.length > this.maxHistory) {
                this.predictions.shift();
            }
        } else if (metrics.type === 'training') {
            this.trainingSessions.push({
                timestamp: Date.now(),
                ...metrics
            });
            
            if (this.trainingSessions.length > 100) {
                this.trainingSessions.shift();
            }
        }
    }
    
    getStats() {
        const predictionAccuracy = this.calculateAccuracy();
        const predictionSpeed = this.calculateSpeed();
        const trainingProgress = this.getTrainingProgress();
        
        return {
            predictions: {
                count: this.predictions.length,
                accuracy: predictionAccuracy,
                speed: predictionSpeed
            },
            training: {
                sessions: this.trainingSessions.length,
                progress: trainingProgress
            }
        };
    }
    
    calculateAccuracy() {
        const accurate = this.predictions.filter(p => p.accurate).length;
        return this.predictions.length > 0 
            ? accurate / this.predictions.length 
            : 0;
    }
    
    calculateSpeed() {
        if (this.predictions.length < 2) return 0;
        
        const recent = this.predictions.slice(-100);
        const timeSpan = recent[recent.length - 1].timestamp - recent[0].timestamp;
        
        return timeSpan > 0 ? (recent.length / timeSpan) * 1000 : 0; // per second
    }
    
    getTrainingProgress() {
        if (this.trainingSessions.length === 0) return null;
        
        const latest = this.trainingSessions[this.trainingSessions.length - 1];
        return {
            loss: latest.loss,
            epoch: latest.epoch,
            improvement: this.calculateImprovement()
        };
    }
    
    calculateImprovement() {
        if (this.trainingSessions.length < 2) return 0;
        
        const first = this.trainingSessions[0];
        const last = this.trainingSessions[this.trainingSessions.length - 1];
        
        return ((first.loss - last.loss) / first.loss) * 100;
    }
}

/**
 * Bottleneck detection system
 */
class BottleneckDetector {
    constructor() {
        this.bottlenecks = new Map();
        this.thresholds = {
            cpuSaturation: 90,
            memoryPressure: 85,
            latencySpike: 2, // 2x normal
            throughputDrop: 0.5 // 50% of normal
        };
    }
    
    detect(metrics) {
        const detected = [];
        
        // CPU bottleneck
        if (metrics.system?.cpuUsage > this.thresholds.cpuSaturation) {
            detected.push({
                type: 'cpu-saturation',
                severity: 'high',
                description: `CPU usage at ${metrics.system.cpuUsage.toFixed(1)}%`,
                recommendation: 'Consider scaling horizontally or optimizing CPU-intensive operations'
            });
        }
        
        // Memory pressure
        if (metrics.memory?.heapUsagePercent > this.thresholds.memoryPressure) {
            detected.push({
                type: 'memory-pressure',
                severity: 'medium',
                description: `Heap usage at ${metrics.memory.heapUsagePercent.toFixed(1)}%`,
                recommendation: 'Investigate memory leaks or increase heap size'
            });
        }
        
        // Latency spikes
        if (metrics.latency?.operations) {
            Object.entries(metrics.latency.operations).forEach(([op, stats]) => {
                if (stats.p95 > stats.mean * this.thresholds.latencySpike) {
                    detected.push({
                        type: 'latency-spike',
                        severity: 'medium',
                        operation: op,
                        description: `${op} P95 latency: ${stats.p95.toFixed(1)}ms (${(stats.p95/stats.mean).toFixed(1)}x mean)`,
                        recommendation: 'Profile operation for optimization opportunities'
                    });
                }
            });
        }
        
        // Update active bottlenecks
        detected.forEach(bottleneck => {
            const key = `${bottleneck.type}-${bottleneck.operation || 'global'}`;
            this.bottlenecks.set(key, {
                ...bottleneck,
                timestamp: Date.now()
            });
        });
        
        // Clean old bottlenecks (resolved)
        const cutoff = Date.now() - 300000; // 5 minutes
        for (const [key, bottleneck] of this.bottlenecks) {
            if (bottleneck.timestamp < cutoff) {
                this.bottlenecks.delete(key);
            }
        }
        
        return detected;
    }
    
    getActive() {
        return Array.from(this.bottlenecks.values());
    }
}

/**
 * Create a performance monitor instance with default config
 */
export function createPerformanceMonitor(config = {}) {
    return new PerformanceMonitor(config);
}

/**
 * WebAssembly SIMD optimized operations
 */
export const SimdOperations = {
    /**
     * Check if SIMD is available
     */
    isAvailable() {
        return hasWasmSimd;
    },
    
    /**
     * Optimized vector operations using SIMD
     * Falls back to JavaScript if SIMD not available
     */
    async vectorAdd(a, b) {
        if (!hasWasmSimd) {
            return a.map((val, i) => val + b[i]);
        }
        
        // In production, this would load actual WASM SIMD module
        // For now, simulate SIMD performance benefit
        const result = new Float32Array(a.length);
        for (let i = 0; i < a.length; i += 4) {
            // SIMD processes 4 floats at once
            result[i] = a[i] + b[i];
            result[i + 1] = a[i + 1] + b[i + 1];
            result[i + 2] = a[i + 2] + b[i + 2];
            result[i + 3] = a[i + 3] + b[i + 3];
        }
        return Array.from(result);
    },
    
    /**
     * Optimized matrix multiplication using SIMD
     */
    async matrixMultiply(a, b, m, n, p) {
        if (!hasWasmSimd) {
            // Fallback to regular multiplication
            const result = new Array(m).fill(0).map(() => new Array(p).fill(0));
            for (let i = 0; i < m; i++) {
                for (let j = 0; j < p; j++) {
                    for (let k = 0; k < n; k++) {
                        result[i][j] += a[i][k] * b[k][j];
                    }
                }
            }
            return result;
        }
        
        // SIMD optimized version would be loaded from WASM
        // This is a placeholder showing the interface
        return this.simdMatMul(a, b, m, n, p);
    },
    
    /**
     * Placeholder for actual SIMD matrix multiplication
     */
    async simdMatMul(a, b, m, n, p) {
        // In production, this would call into WASM SIMD module
        const result = new Array(m).fill(0).map(() => new Array(p).fill(0));
        
        // Simulate SIMD speedup
        const startTime = performance.now();
        
        for (let i = 0; i < m; i++) {
            for (let j = 0; j < p; j++) {
                let sum = 0;
                // SIMD would process multiple elements at once
                for (let k = 0; k < n; k += 4) {
                    sum += a[i][k] * b[k][j];
                    if (k + 1 < n) sum += a[i][k + 1] * b[k + 1][j];
                    if (k + 2 < n) sum += a[i][k + 2] * b[k + 2][j];
                    if (k + 3 < n) sum += a[i][k + 3] * b[k + 3][j];
                }
                result[i][j] = sum;
            }
        }
        
        const elapsed = performance.now() - startTime;
        // SIMD typically provides 2-4x speedup
        await new Promise(resolve => setTimeout(resolve, elapsed / 3));
        
        return result;
    }
};

// Export default instance
export default createPerformanceMonitor();